//! Parked vehicles reuse placed-block models, with one mesh per material.
use crate::{
    blocks::{BlockAssets, TemplateSet},
    meshing::{RenderedSections, builder::MeshBatch},
};
use bevy::{
    ecs::system::SystemParam,
    prelude::*,
    tasks::{AsyncComputeTaskPool, Task, futures_lite::future},
};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};
use terra_core::trains::{TrainBlockView, TrainPart};
use terra_world::{
    edits::EditJournal,
    streaming::{ChunkStore, WorldAccess},
};
mod accessories;
mod animation;
mod openings;
mod smoke;
pub(crate) mod workshop_preview;

type BlockViews = BTreeMap<u16, TrainBlockView>;

#[derive(Clone, Debug, PartialEq)]
struct GeometryKey {
    anchor: [i64; 3],
    parts: Vec<TrainPart>,
    blocks: BlockViews,
}
impl GeometryKey {
    fn matches(&self, anchor: [i64; 3], parts: &[TrainPart], blocks: &BlockViews) -> bool {
        self.anchor == anchor && self.parts == parts && self.blocks == *blocks
    }
}
struct Building {
    key: GeometryKey,
    task: Task<Result<PreparedGeometry, String>>,
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
    building: BTreeMap<u64, Building>,
    discarded: Vec<Task<Result<PreparedGeometry, String>>>,
    palettes: Arc<openings::Palettes>,
}

#[derive(Default)]
struct PreparedGeometry {
    batches: Vec<MeshBatch>,
    animated: Vec<AnimatedBatch>,
}
struct AnimatedBatch {
    batch: MeshBatch,
    animation: animation::Animation,
}
pub(crate) fn install(app: &mut App) {
    accessories::install(app);
    smoke::install(app);
    workshop_preview::install(app);
    app.init_resource::<Trains>()
        .add_systems(Update, synchronize.after(crate::meshing::upload_sections))
        .add_systems(Update, animation::animate.after(synchronize));
}

/// Local offsets and already-authored poses are baked once. Vehicle cargo,
/// couplings and gameplay revisions never participate in this geometry key.
#[cfg(test)]
fn geometry(
    parts: &[TrainPart],
    blocks: &BlockViews,
    templates: &TemplateSet,
    anchor: [i64; 3],
    palettes: &openings::Palettes,
) -> Result<Vec<MeshBatch>, String> {
    Ok(compose_geometry(parts, blocks, templates, anchor, palettes, false)?.batches)
}
fn compose_geometry(
    parts: &[TrainPart],
    blocks: &BlockViews,
    templates: &TemplateSet,
    anchor: [i64; 3],
    palettes: &openings::Palettes,
    animated: bool,
) -> Result<PreparedGeometry, String> {
    if parts.len() > terra_core::trains::MAX_PARTS {
        return Err("Train render part budget exceeded".into());
    }
    let mut merged = BTreeMap::<usize, MeshBatch>::new();
    let mut moving = Vec::new();
    let by_cell: BTreeMap<_, _> = parts.iter().map(|part| (part.offset, part)).collect();
    let mut frozen_openings = openings::frozen_openings(parts, blocks, templates, palettes)?;
    for (index, part) in parts.iter().enumerate() {
        if !templates.blocks.contains_key(&part.material) {
            return Err("Invalid train render part".into());
        }
        let presentation = blocks.get(&(index as u16));
        let neighbors = std::array::from_fn(|side| {
            let mut cell = part.offset;
            cell[side / 2] += if side % 2 == 0 { -1 } else { 1 };
            by_cell.get(&cell).map(|part| (part.material, part.facing))
        });
        let peer_wood = presentation
            .and_then(|view| view.peer)
            .and_then(|peer| {
                by_cell.get(&std::array::from_fn(|axis| part.offset[axis] + peer[axis]))
            })
            .map_or(part.wood, |peer| peer.wood);
        let batches = if let Some(batches) = frozen_openings.remove(&(index as u16)) {
            batches
        } else if terra_world::segments::is_segment(part.material) {
            let profile =
                terra_world::segments::profile(part.offset.map(i64::from), part.material, |cell| {
                    let cell = cell.map(|value| i16::try_from(value).ok());
                    let [Some(x), Some(y), Some(z)] = cell else {
                        return Some(0);
                    };
                    Some(by_cell.get(&[x, y, z]).map_or(0, |part| part.material))
                });
            let mut batches = BTreeMap::new();
            crate::meshing::segments::append(
                &mut batches,
                templates.blocks[&part.material].face_materials[0],
                part.material,
                std::array::from_fn(|axis| anchor[axis] + i64::from(part.offset[axis])),
                [0; 3],
                profile,
                crate::meshing::DetailLevel::Near,
            )?;
            batches.into_values().collect()
        } else {
            crate::meshing::custom::vehicle_model_geometry(
                part,
                presentation,
                peer_wood,
                neighbors,
                templates,
                std::array::from_fn(|axis| anchor[axis] + i64::from(part.offset[axis])),
            )?
        };
        for mut batch in batches {
            if animated && let Some(&role) = templates.train_moving_materials.get(&batch.material) {
                for point in &mut batch.positions {
                    for (axis, offset) in part.offset.into_iter().enumerate() {
                        point[axis] += f32::from(offset);
                    }
                }
                moving.push(AnimatedBatch {
                    batch,
                    animation: animation::descriptor(part, role),
                });
                continue;
            }
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
    Ok(PreparedGeometry {
        batches: merged.into_values().collect(),
        animated: moving,
    })
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

type VehicleSource<'a> = (
    u64,
    [i64; 3],
    &'a [TrainPart],
    &'a BlockViews,
    ([f64; 3], [f64; 4]),
    [[f64; 3]; 2],
);
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
            .map(|vehicle| {
                (
                    vehicle.id,
                    vehicle.anchor,
                    vehicle.parts.as_slice(),
                    &vehicle.blocks,
                    vehicle.model_transform_at(session.displayed_train_motion(vehicle)),
                    vehicle.model_bounds(),
                )
            })
            .collect();
    }
    if access.is_none_or(|access| access.authoritative()) {
        return journal
            .gameplay()
            .trains
            .vehicles
            .values()
            .map(|vehicle| {
                (
                    vehicle.id,
                    vehicle.anchor,
                    vehicle.parts.as_slice(),
                    &vehicle.blocks,
                    vehicle.model_transform(),
                    vehicle.model_bounds(),
                )
            })
            .collect();
    }
    Vec::new()
}

#[cfg(test)]
fn model_translation(anchor: [i64; 3]) -> Vec3 {
    Vec3::new(
        anchor[0] as f32,
        (anchor[1] as f64 + terra_core::trains::MODEL_Y_OFFSET) as f32,
        anchor[2] as f32,
    )
}

fn pose_transform(pose: ([f64; 3], [f64; 4])) -> Transform {
    Transform::from_translation(Vec3::from_array(pose.0.map(|value| value as f32)))
        .with_rotation(Quat::from_array(pose.1.map(|value| value as f32)))
}
fn visible(bounds: [[f64; 3]; 2], chunks: &ChunkStore) -> bool {
    let low = terra_voxel::global_to_section(bounds[0].map(|v| v.floor() as i64)).0;
    let high = terra_voxel::global_to_section(bounds[1].map(|v| v.floor() as i64)).0;
    (low.x..=high.x).any(|x| {
        (low.y..=high.y).any(|y| {
            (low.z..=high.z).any(|z| {
                chunks
                    .loaded
                    .contains_key(&terra_voxel::SectionPos::new(x, y, z))
            })
        })
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
        if chunks.block_at(cell).is_some_and(|material| {
            material != 0 && material != terra_voxel::MaterialId::FluidWater as u16
        }) {
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
    materials: Res<'w, Assets<StandardMaterial>>,
}
fn synchronize(
    mut commands: Commands,
    scene: Scene,
    mut state: ResMut<Trains>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    // A cancelled native composition may still be running inside its poll.
    // Keep ownership until it finishes so rapid edits cannot exceed two jobs.
    state
        .discarded
        .retain_mut(|task| future::block_on(future::poll_once(task)).is_none());
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
        let old = std::mem::take(&mut state.building);
        state
            .discarded
            .extend(old.into_values().map(|job| job.task));
        state.templates = scene.blocks.templates.clone();
        state.palettes = Arc::new(openings::palettes(&scene.blocks, &scene.materials));
    }
    let desired = sources(
        scene.session.as_deref(),
        scene.access.as_deref(),
        &scene.journal,
    )
    .into_iter()
    .filter(|(_, _, _, _, _, bounds)| visible(*bounds, &scene.chunks))
    .collect::<Vec<_>>();
    let live: BTreeSet<_> = desired.iter().map(|(id, ..)| *id).collect();
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
    let obsolete = state
        .building
        .iter()
        .filter(|(id, job)| {
            !desired
                .iter()
                .any(|(current, anchor, parts, blocks, _, _)| {
                    *id == current && job.key.matches(*anchor, parts, blocks)
                })
        })
        .map(|(&id, _)| id)
        .collect::<Vec<_>>();
    for id in obsolete {
        let job = state.building.remove(&id).unwrap();
        state.discarded.push(job.task);
    }
    if scene.blocks.templates.is_none() {
        return;
    }
    for (id, anchor, parts, blocks, pose, _) in desired {
        if let Some(placed) = state.placed.get(&id) {
            commands.entity(placed.root).insert(pose_transform(pose));
        }
        if state
            .placed
            .get(&id)
            .is_some_and(|placed| placed.key.matches(anchor, parts, blocks))
            || state
                .failed
                .get(&id)
                .is_some_and(|key| key.matches(anchor, parts, blocks))
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
        if !state.building.contains_key(&id) {
            if state.building.len() + state.discarded.len() >= 2 {
                continue;
            }
            let key = GeometryKey {
                anchor,
                parts: parts.to_vec(),
                blocks: blocks.clone(),
            };
            let input = key.clone();
            let templates = scene.blocks.templates.as_ref().unwrap().clone();
            let palettes = state.palettes.clone();
            let task = AsyncComputeTaskPool::get().spawn(async move {
                compose_geometry(
                    &input.parts,
                    &input.blocks,
                    &templates,
                    input.anchor,
                    &palettes,
                    true,
                )
            });
            state.building.insert(id, Building { key, task });
        }
        let Some(result) = state
            .building
            .get_mut(&id)
            .and_then(|job| future::block_on(future::poll_once(&mut job.task)))
        else {
            continue;
        };
        let key = state.building.remove(&id).unwrap().key;
        let prepared = match result {
            Ok(batches) => batches,
            Err(error) => {
                tracing::error!(vehicle=id,%error,"Cannot compose assembled train");
                state.failed.insert(id, key);
                continue;
            }
        };
        if prepared
            .batches
            .iter()
            .chain(prepared.animated.iter().map(|part| &part.batch))
            .any(|batch| scene.blocks.materials.get(batch.material).is_none())
        {
            continue;
        }
        let root = commands
            .spawn((
                Name::new("Assembled train"),
                pose_transform(pose),
                Visibility::Inherited,
            ))
            .id();
        let mut handles = Vec::with_capacity(prepared.batches.len() + prepared.animated.len());
        for batch in prepared.batches {
            let mesh = meshes.add(crate::previews::batch_mesh(&batch));
            handles.push(mesh.clone());
            let mut entity = commands.spawn((
                Name::new("Train material batch"),
                Mesh3d(mesh),
                MeshMaterial3d(scene.blocks.materials[batch.material].clone()),
                Transform::default(),
                Visibility::Inherited,
                ChildOf(root),
            ));
            if scene
                .materials
                .get(&scene.blocks.materials[batch.material])
                .is_some_and(|material| material.alpha_mode == AlphaMode::Blend)
            {
                entity.insert(bevy::light::NotShadowCaster);
            }
        }
        for part in prepared.animated {
            let mesh = meshes.add(crate::previews::batch_mesh(&part.batch));
            handles.push(mesh.clone());
            commands.spawn((
                Name::new("Animated train running gear"),
                Mesh3d(mesh),
                MeshMaterial3d(scene.blocks.materials[part.batch.material].clone()),
                Transform::default(),
                Visibility::Inherited,
                ChildOf(root),
                animation::AnimatedPart {
                    vehicle: id,
                    animation: part.animation,
                },
            ));
        }
        state.failed.remove(&id);
        state.placed.insert(
            id,
            Placed {
                root,
                key,
                meshes: handles,
            },
        );
    }
}

#[cfg(test)]
mod tests;
