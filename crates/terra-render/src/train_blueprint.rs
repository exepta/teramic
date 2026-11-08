//! Small visual-only building guides never create colliders or mutate train state.
use bevy::{
    light::{NotShadowCaster, NotShadowReceiver},
    prelude::*,
};
use std::collections::BTreeMap;
use terra_core::train_blueprint::{self, BlueprintCell, TrainBlueprint};
use terra_world::{
    edits::EditJournal,
    streaming::{ChunkStore, StreamingSet},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum GhostState {
    Missing,
    Blocked,
}

#[derive(Resource, Default)]
struct Ghosts {
    mesh: Handle<Mesh>,
    missing: Handle<StandardMaterial>,
    blocked: Handle<StandardMaterial>,
    cells: BTreeMap<[i64; 3], (Entity, GhostState)>,
}

pub(crate) fn install(app: &mut App) {
    app.init_resource::<TrainBlueprint>()
        .init_resource::<Ghosts>()
        .add_systems(Update, synchronize.after(StreamingSet::Receive));
}

fn ghost_state(
    part: BlueprintCell,
    chunks: &ChunkStore,
    journal: &EditJournal,
) -> Option<GhostState> {
    let material = chunks.block_at(part.cell);
    let facing = journal
        .object_state(part.cell)
        .map_or(0, |state| state.facing);
    if train_blueprint::matches(part, material, facing) {
        None
    } else if material == Some(0) {
        Some(GhostState::Missing)
    } else {
        Some(GhostState::Blocked)
    }
}

fn synchronize(
    mut commands: Commands,
    guide: Res<TrainBlueprint>,
    chunks: Res<ChunkStore>,
    journal: Res<EditJournal>,
    mut ghosts: ResMut<Ghosts>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let needed = guide
        .cells
        .iter()
        .filter_map(|part| ghost_state(*part, &chunks, &journal).map(|state| (part.cell, state)))
        .collect::<BTreeMap<_, _>>();
    let removed = ghosts
        .cells
        .keys()
        .filter(|cell| !needed.contains_key(*cell))
        .copied()
        .collect::<Vec<_>>();
    for cell in removed {
        if let Some((entity, _)) = ghosts.cells.remove(&cell) {
            commands.entity(entity).despawn();
        }
    }
    if needed.is_empty() {
        return;
    }
    if ghosts.mesh == Handle::default() {
        ghosts.mesh = meshes.add(Cuboid::new(0.96, 0.96, 0.96));
        let material = |color| StandardMaterial {
            base_color: color,
            alpha_mode: AlphaMode::Blend,
            unlit: true,
            perceptual_roughness: 1.0,
            double_sided: true,
            cull_mode: None,
            ..default()
        };
        ghosts.missing = materials.add(material(Color::srgba(0.16, 0.95, 0.35, 0.25)));
        ghosts.blocked = materials.add(material(Color::srgba(1.0, 0.15, 0.1, 0.32)));
    }
    for (cell, state) in needed {
        let material = if state == GhostState::Missing {
            ghosts.missing.clone()
        } else {
            ghosts.blocked.clone()
        };
        if let Some((entity, previous)) = ghosts.cells.get_mut(&cell) {
            if *previous != state {
                commands.entity(*entity).insert(MeshMaterial3d(material));
                *previous = state;
            }
        } else {
            let entity = commands
                .spawn((
                    Name::new("Train build guide"),
                    Mesh3d(ghosts.mesh.clone()),
                    MeshMaterial3d(material),
                    Transform::from_translation(Vec3::from_array(
                        cell.map(|axis| axis as f32 + 0.5),
                    )),
                    NotShadowCaster,
                    NotShadowReceiver,
                ))
                .id();
            ghosts.cells.insert(cell, (entity, state));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use terra_voxel::{BlockSection, global_to_section};
    use terra_world::streaming::LoadedSection;

    #[test]
    fn missing_conflicting_and_aligned_parts_have_distinct_visual_states() {
        let part = BlueprintCell {
            cell: [0, 2, 0],
            material: 211,
            facing: 1,
        };
        let mut chunks = ChunkStore::default();
        let mut journal = EditJournal::default();
        assert_eq!(
            ghost_state(part, &chunks, &journal),
            Some(GhostState::Blocked)
        );
        let (owner, local) = global_to_section(part.cell);
        let mut data = BlockSection::empty(owner).unwrap();
        chunks.loaded.insert(
            owner,
            LoadedSection {
                data: data.clone(),
                mesh: None,
                mesh_epoch: 0,
                water_epoch: 0,
                water: None,
                props: Vec::new(),
            },
        );
        assert_eq!(
            ghost_state(part, &chunks, &journal),
            Some(GhostState::Missing)
        );
        data.set_block(local, 211).unwrap();
        chunks.loaded.get_mut(&owner).unwrap().data = data;
        assert_eq!(
            ghost_state(part, &chunks, &journal),
            Some(GhostState::Blocked),
            "wrong wheelset axis stays red"
        );
        journal.record(part.cell, 211).unwrap();
        journal
            .set_object_state(
                part.cell,
                Some(terra_world::edits::BlockObjectState {
                    facing: 1,
                    ..default()
                }),
            )
            .unwrap();
        assert_eq!(
            ghost_state(part, &chunks, &journal),
            None,
            "placed matching parts disappear"
        );
    }
}
