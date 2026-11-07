//! Shared meshes for framed doors/windows; only their hinged leaf animates.
use crate::blocks::BlockAssets;
use bevy::{
    asset::RenderAssetUsages,
    light::NotShadowCaster,
    mesh::{Indices, PrimitiveTopology},
    prelude::*,
};
use std::collections::{HashMap, HashSet};
use terra_world::{edits::EditJournal, streaming::ChunkStore};
mod windows;

const HINGE: Vec3 = Vec3::new(0.15, 0.0, 0.5);
const CENTER: Vec3 = Vec3::new(0.5, 0.0, 0.5);
const OPEN_ANGLE: f32 = -std::f32::consts::FRAC_PI_2;
const DURATION: f32 = 0.45;

#[derive(Clone)]
struct Part {
    mesh: Handle<Mesh>,
    material: Handle<StandardMaterial>,
    moving: bool,
    glass: bool,
    leaf: u8,
}

struct Placed {
    root: Entity,
    leaves: Vec<Entity>,
    material: u16,
    facing: u8,
    size: [u8; 2],
}

#[derive(Resource, Default)]
struct Openings {
    templates: HashMap<(u16, u8, u8), Vec<Part>>,
    placed: HashMap<[i64; 3], Placed>,
    next_scan: f32,
}

#[derive(Component)]
struct Leaf {
    fraction: f32,
    start: f32,
    target: f32,
    elapsed: f32,
    angle: f32,
}
impl Leaf {
    fn new(open: bool) -> Self {
        let fraction = f32::from(u8::from(open));
        Self {
            fraction,
            start: fraction,
            target: fraction,
            elapsed: DURATION,
            angle: OPEN_ANGLE,
        }
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
        self.elapsed = (self.elapsed + dt).min(DURATION);
        let t = self.elapsed / DURATION;
        let smooth = t * t * (3.0 - 2.0 * t);
        self.fraction = self.start + (self.target - self.start) * smooth;
    }
}

pub(crate) fn install(app: &mut App) {
    app.init_resource::<Openings>()
        .add_systems(Update, (synchronize, animate).chain());
}

fn has_loaded_part(
    chunks: &ChunkStore,
    position: [i64; 3],
    material: u16,
    object: terra_world::edits::BlockObjectState,
) -> bool {
    chunks.block_at(position) == Some(material)
        || object
            .peer
            .is_some_and(|peer| chunks.block_at(peer) == Some(material))
}

fn loaded_openings<'a>(
    journal: &'a EditJournal,
    chunks: &'a ChunkStore,
) -> impl Iterator<Item = ([i64; 3], u16, terra_world::edits::BlockObjectState)> + 'a {
    let _scope = terra_core::profiling::ScopeTimer::new("openings.refresh_candidates");
    let mut seen = HashSet::new();
    let candidates: Vec<_> = journal
        .object_sections()
        .filter(|section| chunks.loaded.contains_key(section))
        .flat_map(move |section| journal.object_states_in_section(section))
        .filter_map(move |(position, material, object)| {
            if !terra_world::objects::is_opening(material)
                || chunks.block_at(position) != Some(material)
            {
                return None;
            }
            let (position, object) = if object.secondary {
                let primary = object.peer?;
                let state = journal.object_state(primary)?;
                if state.secondary
                    || state.peer != Some(position)
                    || journal.block(primary) != Some(material)
                {
                    return None;
                }
                (primary, state)
            } else {
                (position, object)
            };
            (has_loaded_part(chunks, position, material, object) && seen.insert(position))
                .then_some((position, material, object))
        })
        .collect();
    candidates.into_iter()
}

fn parts(
    id: u16,
    assets: &BlockAssets,
    meshes: &mut Assets<Mesh>,
    materials: &Assets<StandardMaterial>,
) -> Option<Vec<Part>> {
    let templates = assets.templates.as_ref()?;
    let block = templates.blocks.get(&id)?;
    let mut result = Vec::new();
    for primitive in &block.model {
        let moving = templates.hinge_materials.contains(&primitive.material);
        let origin = if moving { HINGE } else { CENTER };
        let material = assets.materials.get(primitive.material)?.clone();
        let glass = materials
            .get(&material)
            .is_some_and(|material| material.alpha_mode == AlphaMode::Blend);
        let mesh = Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::RENDER_WORLD,
        )
        .with_inserted_attribute(
            Mesh::ATTRIBUTE_POSITION,
            primitive
                .positions
                .iter()
                .map(|p| (Vec3::from_array(*p) - origin).to_array())
                .collect::<Vec<_>>(),
        )
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, primitive.normals.clone())
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, primitive.uvs.clone())
        .with_inserted_indices(Indices::U32(primitive.indices.clone()));
        result.push(Part {
            mesh: meshes.add(mesh),
            material,
            moving,
            glass,
            leaf: 0,
        });
    }
    Some(result)
}

#[allow(clippy::too_many_arguments)]
fn synchronize(
    mut commands: Commands,
    time: Res<Time>,
    journal: Res<EditJournal>,
    chunks: Res<ChunkStore>,
    assets: Res<BlockAssets>,
    mut meshes: ResMut<Assets<Mesh>>,
    materials: Res<Assets<StandardMaterial>>,
    mut state: ResMut<Openings>,
    mut leaves: Query<&mut Leaf>,
) {
    let now = time.elapsed_secs();
    if now < state.next_scan || assets.templates.is_none() {
        return;
    }
    state.next_scan = now + 0.1;
    let mut live = HashSet::new();
    let mut seen = HashSet::new();
    for (position, material, object) in loaded_openings(&journal, &chunks) {
        let window_group = terra_world::objects::is_window(material).then(|| {
            terra_world::objects::windows::group(position, material, object.facing, |p| {
                let id = chunks.block_at(p).or_else(|| journal.block(p))?;
                Some((id, journal.object_state(p)?.facing))
            })
        });
        let door_group = terra_world::objects::is_door(material)
            .then(|| {
                terra_world::objects::doors::group(position, material, object, |p| {
                    Some((
                        chunks.block_at(p).or_else(|| journal.block(p))?,
                        journal.object_state(p)?,
                    ))
                })
            })
            .flatten()
            .filter(|group| group.width == 2);
        let group = window_group.or_else(|| door_group.map(|group| group.geometry()));
        let position = group.map_or(position, |g| g.anchor);
        if !seen.insert(position) {
            continue;
        }
        let size = group.map_or([1, 1], |g| [g.width, g.height]);
        // A two-cell door can straddle a vertical section boundary. Its shared
        // model remains visible while either occupied half is still loaded.
        live.insert(position);
        if let Some(placed) = state.placed.get(&position) {
            if placed.material == material
                && placed.facing % 4 == object.facing % 4
                && placed.size == size
            {
                for (index, &entity) in placed.leaves.iter().enumerate() {
                    if let Ok(mut leaf) = leaves.get_mut(entity) {
                        leaf.target(
                            door_group.map_or(object.facing >= 4, |group| group.opened[index]),
                        );
                    }
                }
                continue;
            }
            commands.entity(placed.root).despawn();
            state.placed.remove(&position);
        }
        let key = (material, size[0], size[1]);
        if let std::collections::hash_map::Entry::Vacant(entry) = state.templates.entry(key) {
            let parts = if size != [1, 1] {
                windows::parts(material, size, &assets, &mut meshes, &materials)
            } else {
                parts(material, &assets, &mut meshes, &materials)
            };
            let Some(parts) = parts else {
                continue;
            };
            entry.insert(parts);
        }
        let Some(parts) = state.templates.get(&key) else {
            continue;
        };
        let open = object.facing >= 4;
        let root = commands
            .spawn((
                Name::new("Framed opening"),
                Transform::from_translation(Vec3::from_array(position.map(|v| v as f32)) + CENTER)
                    .with_rotation(Quat::from_rotation_y(
                        f32::from(object.facing % 4) * std::f32::consts::FRAC_PI_2,
                    )),
                Visibility::Inherited,
            ))
            .id();
        let mut leaf_entities = Vec::with_capacity(usize::from(size[0]));
        for index in 0..size[0] {
            let open = door_group.map_or(open, |group| group.opened[usize::from(index)]);
            let hinge = group.map_or(HINGE, |g| {
                Vec3::from_array(g.hinge(index).map(|v| v as f32))
            });
            let angle = if index == 0 { OPEN_ANGLE } else { -OPEN_ANGLE };
            let leaf = commands
                .spawn((
                    Name::new("Hinged leaf"),
                    Leaf {
                        angle,
                        ..Leaf::new(open)
                    },
                    Transform::from_translation(hinge - CENTER)
                        .with_rotation(Quat::from_rotation_y(if open { angle } else { 0.0 })),
                    Visibility::Inherited,
                    ChildOf(root),
                ))
                .id();
            leaf_entities.push(leaf);
        }
        for part in parts {
            let mut entity = commands.spawn((
                Mesh3d(part.mesh.clone()),
                MeshMaterial3d(part.material.clone()),
                Transform::default(),
                Visibility::Inherited,
                ChildOf(if part.moving {
                    leaf_entities[usize::from(part.leaf)]
                } else {
                    root
                }),
            ));
            if part.glass {
                entity.insert(NotShadowCaster);
            }
        }
        state.placed.insert(
            position,
            Placed {
                root,
                leaves: leaf_entities,
                material,
                facing: object.facing,
                size,
            },
        );
    }
    state.placed.retain(|position, entry| {
        if live.contains(position) {
            true
        } else {
            commands.entity(entry.root).despawn();
            false
        }
    });
}

fn animate(time: Res<Time>, mut leaves: Query<(&mut Leaf, &mut Transform)>) {
    for (mut leaf, mut transform) in &mut leaves {
        if leaf.elapsed >= DURATION {
            continue;
        }
        leaf.advance(time.delta_secs());
        transform.rotation = Quat::from_rotation_y(leaf.angle * leaf.fraction);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blocks::{
        BlockTemplate, PrimitiveTemplate, TemplateSet, definitions::BlockRenderKind,
    };
    use std::sync::Arc;
    use terra_voxel::{BlockSection, SectionPos};
    use terra_world::{edits::BlockObjectState, streaming::LoadedSection};

    #[test]
    fn generated_door_metadata_index_tracks_vertical_borders_opening_changes_and_removal() {
        use terra_world::generation::structures::StructureObject;
        let owner = [-1, 15, -16];
        let peer = [-1, 16, -16];
        let object = BlockObjectState {
            peer: Some(peer),
            ..default()
        };
        let secondary = BlockObjectState {
            peer: Some(owner),
            secondary: true,
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
                    material: 65,
                    state: Some(object),
                    loot: None,
                    crop: None,
                },
                StructureObject {
                    position: peer,
                    material: 65,
                    state: Some(secondary),
                    loot: None,
                    crop: None,
                },
            ])
            .unwrap();
        assert_eq!(journal.object_sections().len(), 2);
        let (section, local) = terra_voxel::global_to_section(peer);
        let mut data = BlockSection::empty(section).unwrap();
        data.set_block(local, 65).unwrap();
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
            loaded_openings(&journal, &chunks).collect::<Vec<_>>(),
            vec![(owner, 65, object)]
        );
        let opened = BlockObjectState {
            facing: 4,
            ..object
        };
        journal.set_object_state(owner, Some(opened)).unwrap();
        journal
            .set_object_state(
                peer,
                Some(BlockObjectState {
                    facing: 4,
                    ..secondary
                }),
            )
            .unwrap();
        assert_eq!(
            loaded_openings(&journal, &chunks).collect::<Vec<_>>(),
            vec![(owner, 65, opened)]
        );
        chunks.loaded.clear();
        assert!(loaded_openings(&journal, &chunks).next().is_none());
        journal.record(owner, 0).unwrap();
        journal.record(peer, 0).unwrap();
        assert_eq!(journal.object_sections().len(), 0);
    }

    #[test]
    fn hinge_moves_smoothly_and_reverses_from_its_current_angle() {
        let mut leaf = Leaf::new(false);
        leaf.target(true);
        leaf.advance(DURATION / 4.0);
        assert!((leaf.fraction - 0.15625).abs() < 1.0e-6);
        let midway = leaf.fraction;
        leaf.target(false);
        assert_eq!(leaf.fraction, midway);
        leaf.advance(DURATION / 4.0);
        assert!(leaf.fraction > 0.0 && leaf.fraction < midway);
        leaf.advance(DURATION);
        assert_eq!(leaf.fraction, 0.0);
    }

    #[test]
    fn opening_has_no_initial_animation_flash_after_chunk_reload() {
        let mut leaf = Leaf::new(true);
        leaf.advance(1.0 / 60.0);
        assert_eq!(leaf.fraction, 1.0);
        assert_eq!(leaf.elapsed, DURATION);
        let point = HINGE + Quat::from_rotation_y(OPEN_ANGLE) * (Vec3::new(0.85, 1.0, 0.5) - HINGE);
        assert!((point - Vec3::new(0.15, 1.0, 1.2)).length() < 1.0e-6);
    }

    #[test]
    fn upper_door_half_keeps_shared_model_visible_after_owner_section_unloads() {
        let owner = [-1, 15, -16];
        let peer = [-1, 16, -16];
        let object = BlockObjectState {
            peer: Some(peer),
            ..default()
        };
        let (section, local) = terra_voxel::global_to_section(peer);
        let mut data = BlockSection::empty(section).unwrap();
        data.set_block(local, 65).unwrap();
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
        assert_eq!(chunks.block_at(owner), None);
        let mut journal = EditJournal::default();
        for (position, state) in [
            (owner, object),
            (
                peer,
                BlockObjectState {
                    peer: Some(owner),
                    secondary: true,
                    ..default()
                },
            ),
        ] {
            journal.record(position, 65).unwrap();
            journal.set_object_state(position, Some(state)).unwrap();
        }
        assert_eq!(
            loaded_openings(&journal, &chunks).collect::<Vec<_>>(),
            vec![(owner, 65, object)],
            "a loaded secondary must discover its unloaded primary without scanning journal history"
        );
        assert!(has_loaded_part(&chunks, owner, 65, object));
        assert!(!has_loaded_part(&chunks, owner, 66, object));
        assert!(!has_loaded_part(
            &chunks,
            owner,
            65,
            BlockObjectState::default()
        ));
        chunks
            .loaded
            .get_mut(&section)
            .unwrap()
            .data
            .set_block(local, 0)
            .unwrap();
        assert!(!has_loaded_part(&chunks, owner, 65, object));
        assert!(loaded_openings(&journal, &chunks).next().is_none());
    }

    #[test]
    fn loaded_opening_scan_ignores_unloaded_history_stale_cells_and_duplicate_door_halves() {
        let primary = [-1, 15, -16];
        let peer = [-1, 16, -16];
        let state = BlockObjectState {
            facing: 5,
            peer: Some(peer),
            ..default()
        };
        let mut journal = EditJournal::default();
        let mut chunks = ChunkStore::default();
        for (position, object) in [
            (primary, state),
            (
                peer,
                BlockObjectState {
                    facing: 5,
                    peer: Some(primary),
                    secondary: true,
                    ..default()
                },
            ),
        ] {
            journal.record(position, 65).unwrap();
            journal.set_object_state(position, Some(object)).unwrap();
            let (section, local) = terra_voxel::global_to_section(position);
            let mut data = BlockSection::empty(section).unwrap();
            data.set_block(local, 65).unwrap();
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
        }
        for x in 0..128 {
            let position = [1024 + x * 32, 2, -1];
            journal.record(position, 68).unwrap();
            journal
                .set_object_state(position, Some(BlockObjectState::default()))
                .unwrap();
        }
        let stale = [-2, 15, -16];
        journal.record(stale, 68).unwrap();
        journal
            .set_object_state(stale, Some(BlockObjectState::default()))
            .unwrap();
        assert_eq!(chunks.block_at(stale), Some(0));
        assert_eq!(
            loaded_openings(&journal, &chunks).collect::<Vec<_>>(),
            vec![(primary, 65, state)]
        );
        chunks
            .loaded
            .remove(&terra_voxel::global_to_section(primary).0);
        assert_eq!(
            loaded_openings(&journal, &chunks).collect::<Vec<_>>(),
            vec![(primary, 65, state)]
        );
        chunks.loaded.clear();
        assert!(loaded_openings(&journal, &chunks).next().is_none());
    }

    #[test]
    fn placed_opening_reuses_meshes_animates_and_cleans_up_on_unload() {
        let position = [1, 2, 3];
        let mut journal = EditJournal::default();
        journal.record(position, 68).unwrap();
        journal
            .set_object_state(position, Some(BlockObjectState::default()))
            .unwrap();
        let primitive = |material| PrimitiveTemplate {
            tangents: Vec::new(),
            positions: vec![[0.2, 0.2, 0.5], [0.8, 0.2, 0.5], [0.2, 0.8, 0.5]],
            normals: vec![[0.0, 0.0, 1.0]; 3],
            uvs: vec![[0.0; 2]; 3],
            indices: vec![0, 1, 2],
            material,
        };
        let mut templates = TemplateSet::default();
        templates.hinge_materials.insert(1);
        templates.blocks.insert(
            68,
            BlockTemplate {
                surface: None,
                render_kind: BlockRenderKind::CustomMesh,
                bevel_width: 0.0,
                faces: std::array::from_fn(|_| Vec::new()),
                face_materials: [0; 6],
                side_cap: None,
                details: Vec::new(),
                model: vec![primitive(0), primitive(1)],
            },
        );
        let mut materials = Assets::<StandardMaterial>::default();
        let mut assets = BlockAssets::default();
        assets.materials = vec![
            materials.add(StandardMaterial::default()),
            materials.add(StandardMaterial::default()),
        ];
        assets.templates = Some(Arc::new(templates));
        let section = SectionPos::new(0, 0, 0);
        let mut chunks = ChunkStore::default();
        let loaded = || {
            let mut data = BlockSection::empty(section).unwrap();
            data.set_block([1, 2, 3], 68).unwrap();
            LoadedSection {
                data,
                mesh: None,
                mesh_epoch: 0,
                water_epoch: 0,
                water: None,
                props: Vec::new(),
            }
        };
        chunks.loaded.insert(section, loaded());
        let mut app = App::new();
        app.insert_resource(Time::<()>::default())
            .insert_resource(assets)
            .insert_resource(materials)
            .insert_resource(Assets::<Mesh>::default())
            .insert_resource(journal)
            .insert_resource(chunks);
        install(&mut app);
        app.update();
        assert_eq!(app.world().resource::<Openings>().placed.len(), 1);
        assert_eq!(app.world().resource::<Assets<Mesh>>().len(), 2);
        app.world_mut()
            .resource_mut::<EditJournal>()
            .set_object_state(
                position,
                Some(BlockObjectState {
                    facing: 4,
                    ..default()
                }),
            )
            .unwrap();
        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(std::time::Duration::from_millis(110));
        app.update();
        let fraction = app
            .world_mut()
            .query::<&Leaf>()
            .single(app.world())
            .unwrap()
            .fraction;
        assert!(fraction > 0.0 && fraction < 0.5);
        app.world_mut().resource_mut::<ChunkStore>().loaded.clear();
        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(std::time::Duration::from_millis(110));
        app.update();
        assert!(app.world().resource::<Openings>().placed.is_empty());
        assert_eq!(
            app.world_mut().query::<&Leaf>().iter(app.world()).count(),
            0
        );
        app.world_mut()
            .resource_mut::<ChunkStore>()
            .loaded
            .insert(section, loaded());
        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(std::time::Duration::from_millis(110));
        app.update();
        assert_eq!(app.world().resource::<Assets<Mesh>>().len(), 2);
        assert_eq!(
            app.world_mut()
                .query::<&Leaf>()
                .single(app.world())
                .unwrap()
                .fraction,
            1.0
        );
    }
}
