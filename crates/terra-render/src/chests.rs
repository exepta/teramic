//! Shared authored chest geometry; viewer snapshots animate only the hinged lid.
use crate::blocks::BlockAssets;
use bevy::prelude::*;
use std::collections::{BTreeMap, HashMap, HashSet};
use terra_world::{
    edits::{BlockObjectState, EditJournal},
    streaming::ChunkStore,
};

const CENTER: Vec3 = Vec3::new(0.5, 0.0, 0.5);
const CUT: f32 = 0.5;
const SECONDS: f32 = 0.42;

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct Key {
    wood: terra_core::WoodSpecies,
    peer: terra_core::WoodSpecies,
    double: bool,
}
struct Piece {
    mesh: Handle<Mesh>,
    material: Handle<StandardMaterial>,
    lid: bool,
}
struct Placed {
    root: Entity,
    lid: Entity,
    key: Key,
    facing: u8,
}
#[derive(Resource, Default)]
pub(crate) struct Chests {
    templates: HashMap<Key, Vec<Piece>>,
    placed: HashMap<[i64; 3], Placed>,
    next: f32,
}
#[derive(Component)]
pub(crate) struct Lid {
    fraction: f32,
    start: f32,
    target: f32,
    elapsed: f32,
}
impl Lid {
    fn hides_outline(&self) -> bool {
        self.target > 0.0 || self.fraction > 0.0 || self.elapsed < SECONDS
    }
    fn target(&mut self, open: bool) {
        let target = f32::from(u8::from(open));
        if self.target != target {
            self.start = self.fraction;
            self.target = target;
            self.elapsed = 0.0;
        }
    }
    fn advance(&mut self, dt: f32) {
        self.elapsed = (self.elapsed + dt).min(SECONDS);
        let t = self.elapsed / SECONDS;
        self.fraction = self.start + (self.target - self.start) * t * t * (3.0 - 2.0 * t);
    }
}
impl Chests {
    pub(crate) fn hides_outline(
        &self,
        position: [i64; 3],
        journal: &EditJournal,
        lids: &Query<&Lid>,
    ) -> bool {
        let selected = journal.object_state(position).unwrap_or_default();
        let owner = if selected.secondary {
            selected.peer.unwrap_or(position)
        } else {
            position
        };
        self.placed
            .get(&owner)
            .and_then(|placed| lids.get(placed.lid).ok())
            .is_some_and(Lid::hides_outline)
    }
}
pub(crate) fn install(app: &mut App) {
    app.init_resource::<Chests>()
        .add_systems(Update, (synchronize, animate).chain());
}

fn parts(key: Key, blocks: &BlockAssets, meshes: &mut Assets<Mesh>) -> Option<Vec<Piece>> {
    let templates = blocks.templates.as_ref()?;
    let state = BlockObjectState {
        peer: key.double.then_some([1, 0, 0]),
        wood: key.wood,
        ..default()
    };
    let geometry =
        crate::meshing::custom::model_geometry(29, state, key.peer, [None; 6], [0; 6], templates)
            .ok()?;
    let hinge = Vec3::new(if key.double { 1.0 } else { 0.5 }, CUT, 0.12);
    let mut result = Vec::new();
    for lid in [false, true] {
        let mut clipped = crate::building::model_pieces::clip(&geometry, 1, CUT, !lid).ok()?;
        for batch in &mut clipped {
            if !lid {
                let count = batch.positions.len() as u32;
                batch.positions.extend_from_within(..);
                batch
                    .normals
                    .extend(batch.normals.clone().into_iter().map(|n| n.map(|v| -v)));
                batch.uvs.extend_from_within(..);
                if !batch.colors.is_empty() {
                    batch.colors.extend_from_within(..);
                }
                let reverse: Vec<_> = batch
                    .indices
                    .chunks_exact(3)
                    .flat_map(|t| [t[0] + count, t[2] + count, t[1] + count])
                    .collect();
                batch.indices.extend(reverse);
            }
            for point in &mut batch.positions {
                *point = (Vec3::from_array(*point) - if lid { hinge } else { CENTER }).to_array();
            }
            result.push(Piece {
                mesh: meshes.add(crate::previews::batch_mesh(batch)),
                material: blocks.materials.get(batch.material)?.clone(),
                lid,
            });
        }
    }
    Some(result)
}

fn loaded_chests(
    chunks: &ChunkStore,
    journal: &EditJournal,
) -> BTreeMap<[i64; 3], BlockObjectState> {
    let _scope = terra_core::profiling::ScopeTimer::new("chests.refresh_candidates");
    let mut owners = BTreeMap::new();
    for section in journal
        .object_sections()
        .filter(|section| chunks.loaded.contains_key(section))
    {
        for (position, material, selected) in journal.object_states_in_section(section) {
            if material != 29 {
                continue;
            }
            let owner = if selected.secondary {
                selected.peer.unwrap_or(position)
            } else {
                position
            };
            let object = journal.object_state(owner).unwrap_or_default();
            if !object.secondary
                && journal.block(owner) == Some(29)
                && (chunks.block_at(owner) == Some(29)
                    || object.peer.is_some_and(|p| chunks.block_at(p) == Some(29)))
            {
                owners.entry(owner).or_insert(object);
            }
        }
    }
    owners
}

#[allow(clippy::too_many_arguments)]
fn synchronize(
    mut commands: Commands,
    time: Res<Time>,
    chunks: Res<ChunkStore>,
    journal: Res<EditJournal>,
    blocks: Res<BlockAssets>,
    session: Option<Res<terra_network::client::ClientSession>>,
    mining: Option<Res<terra_player::interaction::MiningVisual>>,
    building: Option<Res<crate::building::BuildingStatus>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut state: ResMut<Chests>,
    mut lids: Query<&mut Lid>,
    mut visibility: Query<&mut Visibility>,
) {
    if time.elapsed_secs() < state.next
        || blocks.templates.as_ref().is_none_or(|t| !t.animated_chests)
    {
        return;
    }
    state.next = time.elapsed_secs() + 0.05;
    let mut live = HashSet::new();
    for (position, object) in loaded_chests(&chunks, &journal) {
        live.insert(position);
        let key = Key {
            wood: object.wood,
            peer: object
                .peer
                .and_then(|p| journal.object_state(p))
                .map_or(object.wood, |p| p.wood),
            double: object.peer.is_some(),
        };
        if state
            .placed
            .get(&position)
            .is_some_and(|old| old.key != key || old.facing != object.facing)
            && let Some(old) = state.placed.remove(&position)
        {
            commands.entity(old.root).despawn();
        }
        if let std::collections::hash_map::Entry::Vacant(entry) = state.templates.entry(key) {
            let Some(parts) = parts(key, &blocks, &mut meshes) else {
                continue;
            };
            entry.insert(parts);
        }
        let open = session
            .as_ref()
            .is_some_and(|s| s.ready && s.open_chests.contains(&position));
        let isolated = building.as_ref().is_some_and(|b| b.isolated_block) && mining.as_ref().is_some_and(|m| matches!(m.target, Some(terra_network::protocol::session::MiningTarget::Block {position:p,expected:29}) if p==position || object.peer==Some(p)));
        if let Some(old) = state.placed.get(&position) {
            if let Ok(mut lid) = lids.get_mut(old.lid) {
                lid.target(open);
            }
            if let Ok(mut visible) = visibility.get_mut(old.root) {
                *visible = if isolated {
                    Visibility::Hidden
                } else {
                    Visibility::Inherited
                };
            }
            continue;
        }
        let root = commands
            .spawn((
                Name::new("Animated chest"),
                Transform::from_translation(Vec3::from_array(position.map(|v| v as f32)) + CENTER)
                    .with_rotation(Quat::from_rotation_y(
                        f32::from(object.facing) * std::f32::consts::FRAC_PI_2,
                    )),
                Visibility::Inherited,
            ))
            .id();
        let fraction = f32::from(u8::from(open));
        let hinge = Vec3::new(if key.double { 1.0 } else { 0.5 }, CUT, 0.12);
        let lid = commands
            .spawn((
                Name::new("Chest lid"),
                Lid {
                    fraction,
                    start: fraction,
                    target: fraction,
                    elapsed: SECONDS,
                },
                Transform::from_translation(hinge - CENTER)
                    .with_rotation(Quat::from_rotation_x(-1.35 * fraction)),
                Visibility::Inherited,
                ChildOf(root),
            ))
            .id();
        for part in &state.templates[&key] {
            commands.spawn((
                Mesh3d(part.mesh.clone()),
                MeshMaterial3d(part.material.clone()),
                Transform::default(),
                Visibility::Inherited,
                crate::miniature::layers(),
                terra_core::miniature::MiniatureMesh::Terrain,
                ChildOf(if part.lid { lid } else { root }),
            ));
        }
        state.placed.insert(
            position,
            Placed {
                root,
                lid,
                key,
                facing: object.facing,
            },
        );
    }
    state.placed.retain(|position, old| {
        if live.contains(position) {
            true
        } else {
            commands.entity(old.root).despawn();
            false
        }
    });
}
pub(crate) fn animate(time: Res<Time>, mut lids: Query<(&mut Lid, &mut Transform)>) {
    for (mut lid, mut pose) in &mut lids {
        if lid.elapsed < SECONDS {
            lid.advance(time.delta_secs());
            pose.rotation = Quat::from_rotation_x(-1.35 * lid.fraction);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_chests_remain_sparse_and_follow_loaded_peer_and_object_revisions() {
        use terra_voxel::{BlockSection, global_to_section};
        use terra_world::{generation::structures::StructureObject, streaming::LoadedSection};
        let owner = [-1, 2, -17];
        let peer = [0, 2, -17];
        let object = BlockObjectState {
            peer: Some(peer),
            ..default()
        };
        let mut journal = EditJournal::default();
        for x in 0..128 {
            journal.record([x * 16, 1, 128], 3).unwrap();
        }
        assert_eq!(journal.object_sections().len(), 0);
        journal
            .initialize_structures(&[
                StructureObject {
                    position: owner,
                    material: 29,
                    state: Some(object),
                    loot: None,
                    crop: None,
                },
                StructureObject {
                    position: peer,
                    material: 29,
                    state: Some(BlockObjectState {
                        peer: Some(owner),
                        secondary: true,
                        ..default()
                    }),
                    loot: None,
                    crop: None,
                },
            ])
            .unwrap();
        assert_eq!(journal.object_sections().len(), 2);
        let (section, local) = global_to_section(peer);
        let mut data = BlockSection::empty(section).unwrap();
        data.set_block(local, 29).unwrap();
        let mut chunks = ChunkStore::default();
        chunks.loaded.insert(
            section,
            LoadedSection {
                data,
                mesh: None,
                mesh_epoch: 0,
                water_epoch: 0,
                water: None,
                props: Vec::new(),
            },
        );
        assert_eq!(
            loaded_chests(&chunks, &journal),
            BTreeMap::from([(owner, object)])
        );
        let changed = BlockObjectState {
            facing: 3,
            wood: terra_core::WoodSpecies::Cherry,
            ..object
        };
        journal.set_object_state(owner, Some(changed)).unwrap();
        assert_eq!(
            loaded_chests(&chunks, &journal),
            BTreeMap::from([(owner, changed)])
        );
        journal.record(owner, 0).unwrap();
        journal.record(peer, 0).unwrap();
        assert_eq!(journal.object_sections().len(), 0);
        assert!(loaded_chests(&chunks, &journal).is_empty());
    }

    #[test]
    fn loaded_chest_index_keeps_cross_section_primary_without_scanning_unloaded_world_history() {
        let owner = [-1, 2, -17];
        let peer = [0, 2, -17];
        let mut journal = EditJournal::default();
        for position in [owner, peer] {
            journal.record(position, 29).unwrap();
        }
        journal
            .set_object_state(
                owner,
                Some(BlockObjectState {
                    peer: Some(peer),
                    ..default()
                }),
            )
            .unwrap();
        journal
            .set_object_state(
                peer,
                Some(BlockObjectState {
                    peer: Some(owner),
                    secondary: true,
                    ..default()
                }),
            )
            .unwrap();
        for x in 1..1000 {
            let position = [x * 16, 2, 400];
            journal.record(position, 29).unwrap();
            journal
                .set_object_state(position, Some(BlockObjectState::default()))
                .unwrap();
        }
        let mut chunks = ChunkStore::default();
        let (section, local) = terra_voxel::global_to_section(peer);
        let mut data = terra_voxel::BlockSection::empty(section).unwrap();
        data.set_block(local, 29).unwrap();
        chunks.loaded.insert(
            section,
            terra_world::streaming::LoadedSection {
                data,
                mesh: None,
                mesh_epoch: 0,
                water_epoch: 0,
                water: None,
                props: Vec::new(),
            },
        );
        assert_eq!(
            loaded_chests(&chunks, &journal)
                .keys()
                .copied()
                .collect::<Vec<_>>(),
            vec![owner]
        );
        chunks.loaded.clear();
        assert!(loaded_chests(&chunks, &journal).is_empty());
    }
    #[test]
    fn chest_lid_opens_smoothly_and_reverses_without_a_snap() {
        let mut lid = Lid {
            fraction: 0.0,
            start: 0.0,
            target: 0.0,
            elapsed: SECONDS,
        };
        assert!(!lid.hides_outline());
        lid.target(true);
        assert!(
            lid.hides_outline(),
            "suppress the intact collision cage from the first opening frame"
        );
        lid.advance(SECONDS * 0.5);
        assert!((lid.fraction - 0.5).abs() < 0.0001);
        lid.target(false);
        assert!(
            lid.hides_outline(),
            "closing lids must not show the already-closed cage"
        );
        assert!((lid.start - 0.5).abs() < 0.0001);
        lid.advance(SECONDS);
        assert_eq!(lid.fraction, 0.0);
        assert!(!lid.hides_outline());
        let forward = Quat::from_rotation_x(-1.35) * Vec3::Z;
        assert!(forward.y > 0.9, "front edge rises around the back hinge");
    }
}
