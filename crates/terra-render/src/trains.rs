//! Parked vehicles reuse placed-block models, with one mesh per material.
use crate::{
    blocks::{BlockAssets, TemplateSet},
    meshing::{RenderedSections, builder::MeshBatch},
};
use bevy::{ecs::system::SystemParam, prelude::*};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};
use terra_core::trains::TrainPart;
use terra_world::{
    edits::{BlockObjectState, EditJournal},
    streaming::{ChunkStore, WorldAccess},
};

#[derive(Clone, Debug, PartialEq, Eq)]
struct GeometryKey {
    anchor: [i64; 3],
    parts: Vec<TrainPart>,
}
impl GeometryKey {
    fn matches(&self, anchor: [i64; 3], parts: &[TrainPart]) -> bool {
        self.anchor == anchor && self.parts == parts
    }
}
struct Placed {
    root: Entity,
    key: GeometryKey,
    meshes: Vec<Handle<Mesh>>,
}
#[derive(Resource, Default)]
struct Trains {
    placed: BTreeMap<u64, Placed>,
    templates: Option<Arc<TemplateSet>>,
    failed: BTreeMap<u64, GeometryKey>,
}

pub(crate) fn install(app: &mut App) {
    app.init_resource::<Trains>()
        .add_systems(Update, synchronize.after(crate::meshing::upload_sections));
}

/// Local offsets and already-authored poses are baked once. Vehicle cargo,
/// couplings and gameplay revisions never participate in this geometry key.
fn geometry(parts: &[TrainPart], templates: &TemplateSet) -> Result<Vec<MeshBatch>, String> {
    if parts.len() > terra_core::trains::MAX_PARTS {
        return Err("Train render part budget exceeded".into());
    }
    let mut merged = BTreeMap::<usize, MeshBatch>::new();
    for part in parts {
        if !terra_core::train_parts::is_part(part.material) || part.facing > 3 {
            return Err("Invalid train render part".into());
        }
        let state = BlockObjectState {
            facing: part.facing,
            wood: part.wood,
            ..default()
        };
        for batch in crate::meshing::custom::model_geometry(
            part.material,
            state,
            part.wood,
            [None; 6],
            [0; 6],
            templates,
        )? {
            let target = merged.entry(batch.material).or_insert_with(|| MeshBatch {
                material: batch.material,
                ..default()
            });
            let base =
                u32::try_from(target.positions.len()).map_err(|_| "Train render index overflow")?;
            let start = target.positions.len();
            if batch.normals.len() != batch.positions.len()
                || batch.uvs.len() != batch.positions.len()
            {
                return Err("Train authored attributes are incomplete".into());
            }
            target.positions.extend(batch.positions.iter().map(|point| {
                std::array::from_fn(|axis| point[axis] + f32::from(part.offset[axis]))
            }));
            target.normals.extend_from_slice(&batch.normals);
            target.uvs.extend_from_slice(&batch.uvs);
            target.append_tangents(start, batch.tangents.iter().copied());
            if !target.colors.is_empty() || !batch.colors.is_empty() {
                target.colors.resize(start, [1.0; 4]);
                if batch.colors.is_empty() {
                    target.colors.resize(target.positions.len(), [1.0; 4]);
                } else if batch.colors.len() == batch.positions.len() {
                    target.colors.extend_from_slice(&batch.colors);
                } else {
                    return Err("Train authored colors are incomplete".into());
                }
            }
            for index in batch.indices {
                if index as usize >= batch.positions.len() {
                    return Err("Train authored index is invalid".into());
                }
                target.indices.push(
                    base.checked_add(index)
                        .ok_or("Train render index overflow")?,
                );
            }
        }
    }
    Ok(merged.into_values().collect())
}

fn release_meshes(meshes: &mut Assets<Mesh>, handles: impl IntoIterator<Item = Handle<Mesh>>) {
    for handle in handles {
        meshes.remove(handle.id());
    }
}
fn retire(placed: Placed, commands: &mut Commands, meshes: &mut Assets<Mesh>) {
    commands.entity(placed.root).despawn();
    release_meshes(meshes, placed.meshes);
}

type VehicleSource<'a> = (u64, [i64; 3], &'a [TrainPart]);
fn sources<'a>(
    session: Option<&'a terra_network::client::ClientSession>,
    access: Option<&WorldAccess>,
    journal: &'a EditJournal,
) -> Vec<VehicleSource<'a>> {
    if let Some(session) = session
        && session.ready
    {
        return session
            .trains
            .vehicles
            .iter()
            .map(|vehicle| (vehicle.id, vehicle.anchor, vehicle.parts.as_slice()))
            .collect();
    }
    if access.is_none_or(|access| access.authoritative()) {
        return journal
            .gameplay()
            .trains
            .vehicles
            .values()
            .map(|vehicle| (vehicle.id, vehicle.anchor, vehicle.parts.as_slice()))
            .collect();
    }
    Vec::new()
}

fn model_translation(anchor: [i64; 3]) -> Vec3 {
    Vec3::new(
        anchor[0] as f32,
        (anchor[1] as f64 + terra_core::trains::MODEL_Y_OFFSET) as f32,
        anchor[2] as f32,
    )
}

fn visible(anchor: [i64; 3], parts: &[TrainPart], chunks: &ChunkStore) -> bool {
    parts.iter().any(|part| {
        chunks.loaded.contains_key(
            &terra_voxel::global_to_section(std::array::from_fn(|axis| {
                anchor[axis] + i64::from(part.offset[axis])
            }))
            .0,
        )
    })
}

// Avoid drawing a second copy while placed-part terrain meshes are retiring.
// Once spawned, unrelated terrain edits do not invalidate the vehicle itself.
fn terrain_retired(
    anchor: [i64; 3],
    parts: &[TrainPart],
    chunks: &ChunkStore,
    journal: &EditJournal,
    rendered: &RenderedSections,
) -> bool {
    let mut owners = BTreeSet::new();
    for part in parts {
        let cell = std::array::from_fn(|axis| anchor[axis] + i64::from(part.offset[axis]));
        let owner = terra_voxel::global_to_section(cell).0;
        if !chunks.loaded.contains_key(&owner) {
            continue;
        }
        if chunks
            .block_at(cell)
            .is_some_and(terra_core::train_parts::is_part)
        {
            return false;
        }
        owners.insert(owner);
    }
    owners.into_iter().all(|owner| {
        rendered
            .block_revisions
            .get(&owner)
            .is_some_and(|revision| *revision == journal.revision(owner))
    })
}

#[derive(SystemParam)]
struct Scene<'w> {
    blocks: Res<'w, BlockAssets>,
    journal: Res<'w, EditJournal>,
    chunks: Res<'w, ChunkStore>,
    rendered: Res<'w, RenderedSections>,
    session: Option<Res<'w, terra_network::client::ClientSession>>,
    access: Option<Res<'w, WorldAccess>>,
}
fn synchronize(
    mut commands: Commands,
    scene: Scene,
    mut state: ResMut<Trains>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    let same_templates = match (&state.templates, &scene.blocks.templates) {
        (Some(old), Some(next)) => Arc::ptr_eq(old, next),
        (None, None) => true,
        _ => false,
    };
    if !same_templates {
        for placed in std::mem::take(&mut state.placed).into_values() {
            retire(placed, &mut commands, &mut meshes);
        }
        state.failed.clear();
        state.templates = scene.blocks.templates.clone();
    }
    let desired = sources(
        scene.session.as_deref(),
        scene.access.as_deref(),
        &scene.journal,
    )
    .into_iter()
    .filter(|(_, anchor, parts)| visible(*anchor, parts, &scene.chunks))
    .collect::<Vec<_>>();
    let live: BTreeSet<_> = desired.iter().map(|(id, _, _)| *id).collect();
    let removed = state
        .placed
        .keys()
        .filter(|id| !live.contains(id))
        .copied()
        .collect::<Vec<_>>();
    for id in removed {
        if let Some(old) = state.placed.remove(&id) {
            retire(old, &mut commands, &mut meshes);
        }
    }
    state.failed.retain(|id, _| live.contains(id));
    let Some(templates) = scene.blocks.templates.as_deref() else {
        return;
    };
    for (id, anchor, parts) in desired {
        if state
            .placed
            .get(&id)
            .is_some_and(|placed| placed.key.matches(anchor, parts))
            || state
                .failed
                .get(&id)
                .is_some_and(|key| key.matches(anchor, parts))
        {
            continue;
        }
        if let Some(old) = state.placed.remove(&id) {
            retire(old, &mut commands, &mut meshes);
        }
        if !terrain_retired(
            anchor,
            parts,
            &scene.chunks,
            &scene.journal,
            &scene.rendered,
        ) {
            continue;
        }
        let batches = match geometry(parts, templates) {
            Ok(batches) => batches,
            Err(error) => {
                tracing::error!(vehicle=id,%error,"Cannot compose assembled train");
                state.failed.insert(
                    id,
                    GeometryKey {
                        anchor,
                        parts: parts.to_vec(),
                    },
                );
                continue;
            }
        };
        if batches
            .iter()
            .any(|batch| scene.blocks.materials.get(batch.material).is_none())
        {
            continue;
        }
        let root = commands
            .spawn((
                Name::new("Assembled train"),
                Transform::from_translation(model_translation(anchor)),
                Visibility::Inherited,
            ))
            .id();
        let mut handles = Vec::with_capacity(batches.len());
        for batch in batches {
            let mesh = meshes.add(crate::previews::batch_mesh(&batch));
            handles.push(mesh.clone());
            commands.spawn((
                Name::new("Train material batch"),
                Mesh3d(mesh),
                MeshMaterial3d(scene.blocks.materials[batch.material].clone()),
                Transform::default(),
                Visibility::Inherited,
                ChildOf(root),
            ));
        }
        state.failed.remove(&id);
        state.placed.insert(
            id,
            Placed {
                root,
                key: GeometryKey {
                    anchor,
                    parts: parts.to_vec(),
                },
                meshes: handles,
            },
        );
    }
}

#[cfg(test)]
mod tests;
