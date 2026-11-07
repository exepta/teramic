//! One bounded worker and material-batched render-only falling tree fragments.
mod geometry;
mod hell;
use super::BuildingWorld;
use crate::meshing::{GeometryCache, RenderedSections, builder::RenderOptions};
use bevy::{
    ecs::system::SystemParam,
    prelude::*,
    tasks::{AsyncComputeTaskPool, Task, futures_lite::future},
};
use std::collections::VecDeque;
use terra_core::TreeFallAnimation;
use terra_network::client::TreeCollapseEvent;
use terra_voxel::SectionPos;
use terra_world::{streaming::WorldTerrain, tree_felling::Collapse};

const MAX_ACTIVE: usize = 4;
const VISUAL_SECONDS: f32 = 3.8;

struct Waiting {
    event: Collapse,
    age: f32,
}
struct Pending {
    event: Collapse,
    originals: Vec<(SectionPos, Entity, u64)>,
    refresh: Option<Entity>,
    mode: TreeFallAnimation,
    task: Task<Result<geometry::Geometry, String>>,
}
struct Mask {
    position: SectionPos,
    original: Entity,
    epoch: u64,
    overlay: Entity,
    meshes: Vec<Handle<Mesh>>,
}
struct Active {
    event: Collapse,
    root: Entity,
    masks: Vec<Mask>,
    // Only these records and mesh components own temporary assets. Dropping
    // their handles lets Bevy retire them through its normal render lifecycle.
    meshes: Vec<Handle<Mesh>>,
    pieces: usize,
    age: f32,
    mode: TreeFallAnimation,
}

#[derive(Resource, Default)]
pub(super) struct FallingTrees {
    waiting: VecDeque<Waiting>,
    pending: Option<Pending>,
    active: Vec<Active>,
}

#[derive(Resource, Default, Debug)]
pub struct TreeFallStatus {
    pub active: usize,
    pub pieces: usize,
    pub preparing: bool,
    pub meshes: usize,
}

#[derive(Component)]
pub(super) struct Piece {
    foot: Vec3,
    direction: Vec3,
    band: usize,
    wood: bool,
    pivot: f32,
    minimum: Vec3,
    maximum: Vec3,
    age: f32,
    mode: TreeFallAnimation,
}

#[derive(SystemParam)]
pub(super) struct Scene<'w> {
    rendered: Res<'w, RenderedSections>,
    cache: Res<'w, GeometryCache>,
    terrain: Option<Res<'w, WorldTerrain>>,
    config: Option<Res<'w, terra_core::GameConfig>>,
    foliage: Option<Res<'w, crate::foliage::FoliageAssets>>,
    graphics: Option<Res<'w, terra_core::GraphicsConfig>>,
    materials: Option<Res<'w, Assets<StandardMaterial>>>,
}

pub(super) fn install(app: &mut App) {
    if app.get_sub_app(bevy::render::RenderApp).is_some() {
        app.add_plugins(MaterialPlugin::<hell::FallMaterial>::default());
    }
    app.init_resource::<FallingTrees>()
        .init_resource::<Assets<hell::FallMaterial>>()
        .init_resource::<TreeFallStatus>()
        .init_resource::<RenderedSections>()
        .init_resource::<GeometryCache>()
        .add_message::<TreeCollapseEvent>()
        .add_systems(Update, hell::animate.after(update));
}

fn snapshots(
    event: &Collapse,
    world: &BuildingWorld,
    scene: &Scene,
) -> (Vec<geometry::Snapshot>, Vec<(SectionPos, Entity, u64)>) {
    let mut snapshots = Vec::new();
    let mut originals = Vec::new();
    for section in &event.sections {
        let Some(rendered) = scene.rendered.sections.get(&section.position) else {
            continue;
        };
        let Some((geometry, models)) = scene.cache.snapshot(section.position, rendered.mesh_epoch)
        else {
            continue;
        };
        let mut protected = Box::new([false; 4096]);
        for (i, _) in section.cells() {
            protected[usize::from(i)] = event.cell_position(section.position, i).is_some_and(|p| {
                world
                    .journal
                    .as_ref()
                    .is_some_and(|j| j.block(p).is_some_and(|m| m != 0))
            });
        }
        snapshots.push(geometry::Snapshot {
            position: section.position,
            geometry,
            models,
            protected,
            options: RenderOptions {
                detail_level: rendered.detail_level,
                water_level: scene
                    .config
                    .as_ref()
                    .and_then(|config| config.world.water.then_some(config.world.sea_level)),
                terrain: scene.terrain.as_ref().map(|t| t.0.clone()),
                exposure: None,
                tints: Default::default(),
                conditions: world
                    .journal
                    .as_ref()
                    .map(|j| j.surface_snapshot(section.position))
                    .unwrap_or_default(),
            },
        });
        originals.push((section.position, rendered.entity, rendered.mesh_epoch));
    }
    (snapshots, originals)
}

#[allow(clippy::too_many_arguments)]
pub(super) fn update(
    mut commands: Commands,
    world: BuildingWorld,
    scene: Scene,
    mut events: MessageReader<TreeCollapseEvent>,
    mut state: ResMut<FallingTrees>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut visible: Query<&mut Visibility>,
    mut status: ResMut<TreeFallStatus>,
    mut fall_materials: ResMut<Assets<hell::FallMaterial>>,
) {
    let dt = world.time.delta_secs().min(0.1);
    let active_world = world.active();
    for event in events.read() {
        if active_world
            && event.0.valid()
            && state.waiting.len() + state.active.len() + usize::from(state.pending.is_some())
                < MAX_ACTIVE
            && !state.active.iter().any(|t| t.event.foot == event.0.foot)
            && !state.waiting.iter().any(|t| t.event.foot == event.0.foot)
            && state
                .pending
                .as_ref()
                .is_none_or(|t| t.event.foot != event.0.foot)
        {
            state.waiting.push_back(Waiting {
                event: event.0.clone(),
                age: 0.0,
            });
        }
    }
    for tree in &mut state.active {
        tree.age += dt;
        tree.masks.retain_mut(|mask| {
            let current = scene.rendered.sections.get(&mask.position);
            let loaded = world.chunks.loaded.contains_key(&mask.position);
            let remaining = tree
                .event
                .sections
                .iter()
                .find(|s| s.position == mask.position)
                .is_some_and(|s| section_remaining(&tree.event, s, &world));
            let uploaded = current.is_none_or(|r| {
                world
                    .journal
                    .as_ref()
                    .is_some_and(|j| r.mesh_epoch == j.mesh_epoch(mask.position))
            });
            let done = !active_world || !loaded || !remaining && uploaded || tree.age > 24.0;
            if done {
                commands.entity(mask.overlay).try_despawn();
                for entity in [Some(mask.original), current.map(|r| r.entity)]
                    .into_iter()
                    .flatten()
                {
                    if let Ok(mut visibility) = visible.get_mut(entity) {
                        *visibility = Visibility::Inherited;
                    }
                }
                false
            } else {
                // Normal streaming can replace its root during the animation.
                // Keep only the current root masked, never modify world voxels.
                if let Some(current) = current
                    && let Ok(mut v) = visible.get_mut(current.entity)
                {
                    *v = Visibility::Hidden;
                }
                true
            }
        });
    }
    state.active.retain(|tree| {
        let keep = active_world && (tree.age < visual_seconds(tree.mode) || !tree.masks.is_empty());
        if !keep {
            commands.entity(tree.root).try_despawn();
        }
        keep
    });
    if !active_world {
        state.waiting.clear();
        state.pending = None;
    }
    if let Some(pending) = &mut state.pending
        && let Some(result) = future::block_on(future::poll_once(&mut pending.task))
    {
        let pending = state.pending.take().expect("polled falling tree task");
        match result {
            Ok(geometry) if active_world => {
                let root = if let Some(root) = pending.refresh {
                    let Some(active) = state.active.iter_mut().find(|a| a.root == root) else {
                        return;
                    };
                    for mask in active.masks.drain(..) {
                        commands.entity(mask.overlay).try_despawn();
                        for entity in [
                            Some(mask.original),
                            scene
                                .rendered
                                .sections
                                .get(&mask.position)
                                .map(|r| r.entity),
                        ]
                        .into_iter()
                        .flatten()
                        {
                            if let Ok(mut v) = visible.get_mut(entity) {
                                *v = Visibility::Inherited;
                            }
                        }
                    }
                    root
                } else {
                    commands
                        .spawn((
                            Transform::default(),
                            Visibility::default(),
                            Name::new("Falling tree"),
                        ))
                        .id()
                };
                let mut handles = Vec::new();
                let mut masks = Vec::new();
                for (position, batches) in geometry.terrain {
                    let Some(&(_, original, epoch)) =
                        pending.originals.iter().find(|(p, _, _)| *p == position)
                    else {
                        continue;
                    };
                    let still_needed = pending
                        .event
                        .sections
                        .iter()
                        .find(|s| s.position == position)
                        .is_some_and(|s| section_remaining(&pending.event, s, &world))
                        || scene.rendered.sections.get(&position).is_some_and(|r| {
                            world
                                .journal
                                .as_ref()
                                .is_some_and(|j| r.mesh_epoch != j.mesh_epoch(position))
                        });
                    if !still_needed {
                        if let Some(current) = scene.rendered.sections.get(&position)
                            && let Ok(mut v) = visible.get_mut(current.entity)
                        {
                            *v = Visibility::Inherited;
                        }
                        continue;
                    }
                    let overlay = commands
                        .spawn((
                            Transform::from_translation(Vec3::from_array(
                                position.origin().unwrap_or_default().map(|v| v as f32),
                            )),
                            Visibility::default(),
                            ChildOf(root),
                        ))
                        .id();
                    let mut terrain_handles = Vec::new();
                    for batch in batches {
                        spawn_batch(
                            &mut commands,
                            &world,
                            &scene,
                            &mut meshes,
                            &mut terrain_handles,
                            overlay,
                            batch,
                        );
                    }
                    if let Ok(mut visibility) = visible.get_mut(original) {
                        *visibility = Visibility::Hidden;
                    }
                    if let Some(current) = scene.rendered.sections.get(&position)
                        && let Ok(mut v) = visible.get_mut(current.entity)
                    {
                        *v = Visibility::Hidden;
                    }
                    masks.push(Mask {
                        position,
                        original,
                        epoch,
                        overlay,
                        meshes: terrain_handles,
                    });
                }
                let pieces = geometry.pieces.len() + geometry.hell.len();
                for (band, wood, batches) in geometry.pieces {
                    let pivot = (band as f32 + 0.5) * geometry.band_height;
                    let mut minimum = Vec3::splat(f32::INFINITY);
                    let mut maximum = Vec3::splat(f32::NEG_INFINITY);
                    for point in batches.iter().flat_map(|batch| &batch.positions) {
                        minimum = minimum.min(Vec3::from_array(*point));
                        maximum = maximum.max(Vec3::from_array(*point));
                    }
                    let foot = Vec3::from_array(pending.event.visual_origin().map(|v| v as f32))
                        + Vec3::new(0.5, 0.0, 0.5);
                    let group = commands
                        .spawn((
                            Piece {
                                foot,
                                direction: Vec3::new(
                                    pending.event.direction[0],
                                    0.0,
                                    pending.event.direction[1],
                                ),
                                band,
                                wood,
                                pivot,
                                minimum,
                                maximum,
                                age: 0.0,
                                mode: pending.mode,
                            },
                            Transform::from_translation(foot + Vec3::Y * pivot),
                            Visibility::default(),
                            ChildOf(root),
                        ))
                        .id();
                    for batch in batches {
                        spawn_batch(
                            &mut commands,
                            &world,
                            &scene,
                            &mut meshes,
                            &mut handles,
                            group,
                            batch,
                        );
                    }
                }
                if !geometry.hell.is_empty() {
                    let group = commands
                        .spawn((
                            Transform::from_translation(Vec3::from_array(
                                pending.event.visual_origin().map(|v| v as f32),
                            )),
                            Visibility::default(),
                            ChildOf(root),
                        ))
                        .id();
                    let mut materials = Vec::new();
                    for batch in geometry.hell {
                        let Some(base) = world
                            .blocks
                            .materials
                            .get(batch.mesh.material)
                            .and_then(|handle| scene.materials.as_ref()?.get(handle))
                            .cloned()
                        else {
                            continue;
                        };
                        let material = fall_materials.add(bevy::pbr::ExtendedMaterial {
                            base,
                            extension: hell::FallExtension {
                                age: Vec4::new(0.0, 5.0, 0.0, 0.0),
                            },
                        });
                        let mut mesh = crate::previews::batch_mesh(&batch.mesh);
                        mesh.insert_attribute(hell::ATTRIBUTE, batch.cells);
                        let mesh = meshes.add(mesh);
                        handles.push(mesh.clone());
                        materials.push(material.clone());
                        commands.spawn((
                            Mesh3d(mesh),
                            MeshMaterial3d(material),
                            Transform::default(),
                            Visibility::default(),
                            bevy::camera::visibility::NoFrustumCulling,
                            ChildOf(group),
                        ));
                    }
                    commands.entity(group).insert(hell::FallingBatch {
                        age: 0.0,
                        materials,
                    });
                }
                if pending.refresh.is_some() {
                    if let Some(active) = state.active.iter_mut().find(|a| a.root == root) {
                        active.masks = masks;
                    }
                } else {
                    state.active.push(Active {
                        event: pending.event,
                        root,
                        masks,
                        meshes: handles,
                        pieces,
                        age: 0.0,
                        mode: pending.mode,
                    });
                }
            }
            Err(error) => tracing::warn!(%error, "Falling tree render preparation failed"),
            _ => {}
        }
    }
    for waiting in &mut state.waiting {
        waiting.age += dt;
    }
    state.waiting.retain(|w| w.age < 6.0);
    if active_world
        && state.pending.is_none()
        && let Some(templates) = &world.blocks.templates
        && let Some(index) = state.waiting.iter().position(|w| {
            !state.active.iter().any(|a| {
                a.masks
                    .iter()
                    .any(|m| w.event.sections.iter().any(|s| s.position == m.position))
            })
        })
        && let Some(waiting) = state.waiting.remove(index)
    {
        let (snapshots, originals) = snapshots(&waiting.event, &world, &scene);
        let required = waiting
            .event
            .sections
            .iter()
            .filter(|s| scene.rendered.sections.contains_key(&s.position))
            .count();
        if originals.len() != required || world.chunks.block_at(waiting.event.foot).is_none() {
            state.waiting.push_back(waiting);
        } else {
            let templates = templates.clone();
            let event = waiting.event;
            let worker_event = event.clone();
            let mode = scene
                .graphics
                .as_ref()
                .map_or(TreeFallAnimation::default(), |graphics| {
                    graphics.tree_fall_animation
                });
            let grounds = if mode == TreeFallAnimation::HellFall {
                hell::grounds(&event, &world.chunks)
            } else {
                Default::default()
            };
            state.pending = Some(Pending {
                event,
                originals,
                refresh: None,
                mode,
                task: AsyncComputeTaskPool::get().spawn(async move {
                    let mut geometry = geometry::prepare(
                        &worker_event,
                        &snapshots,
                        &templates,
                        mode != TreeFallAnimation::HellFall,
                    )?;
                    if mode == TreeFallAnimation::HellFall {
                        geometry.hell =
                            hell::prepare(&worker_event, &snapshots, &templates, &grounds)?;
                    }
                    Ok(geometry)
                }),
            });
        }
    }
    if active_world
        && state.pending.is_none()
        && let Some(tree) = state.active.iter().find(|tree| {
            tree.masks.iter().any(|mask| {
                scene
                    .rendered
                    .sections
                    .get(&mask.position)
                    .is_some_and(|r| r.mesh_epoch != mask.epoch)
            })
        })
        && let Some(templates) = &world.blocks.templates
    {
        let root = tree.root;
        let event = tree.event.clone();
        let (snapshots, originals) = snapshots(&event, &world, &scene);
        let worker_event = event.clone();
        let templates = templates.clone();
        let mode = tree.mode;
        state.pending = Some(Pending {
            event,
            originals,
            refresh: Some(root),
            mode,
            task: AsyncComputeTaskPool::get().spawn(async move {
                geometry::prepare(&worker_event, &snapshots, &templates, false)
            }),
        });
    }
    status.active = state.active.len();
    status.preparing = state.pending.is_some();
    status.pieces = state
        .active
        .iter()
        .filter(|a| a.age < visual_seconds(a.mode))
        .map(|a| a.pieces)
        .sum();
    status.meshes = state
        .active
        .iter()
        .map(|a| a.meshes.len() + a.masks.iter().map(|m| m.meshes.len()).sum::<usize>())
        .sum();
}

fn section_remaining(
    event: &Collapse,
    section: &terra_world::tree_felling::CollapseSection,
    world: &BuildingWorld,
) -> bool {
    section.cells().any(|(i, wood)| {
        event.cell_position(section.position, i).is_some_and(|p| {
            world.journal.as_ref().is_none_or(|j| j.block(p).is_none())
                && world.chunks.block_at(p)
                    == Some(if wood {
                        event.wood_material
                    } else {
                        event.leaves_material
                    })
        })
    })
}

#[allow(clippy::too_many_arguments)]
fn spawn_batch(
    commands: &mut Commands,
    world: &BuildingWorld,
    scene: &Scene,
    meshes: &mut Assets<Mesh>,
    handles: &mut Vec<Handle<Mesh>>,
    parent: Entity,
    batch: crate::meshing::builder::MeshBatch,
) {
    if batch.positions.is_empty() || batch.indices.is_empty() {
        return;
    }
    let slot = batch.material;
    let mesh = meshes.add(crate::previews::batch_mesh(&batch));
    handles.push(mesh.clone());
    let mut entity = commands.spawn((
        Mesh3d(mesh),
        Transform::default(),
        Visibility::default(),
        ChildOf(parent),
    ));
    if let Some(material) = scene.foliage.as_ref().and_then(|f| f.materials.get(&slot)) {
        entity.insert(MeshMaterial3d(material.clone()));
    } else if let Some(material) = world.blocks.materials.get(slot) {
        entity.insert(MeshMaterial3d(material.clone()));
    }
}

pub(super) fn animate(
    mut commands: Commands,
    world: BuildingWorld,
    mut pieces: Query<(Entity, &mut Piece, &mut Transform)>,
) {
    for (entity, mut piece, mut transform) in &mut pieces {
        piece.age += world.time.delta_secs().min(0.1);
        if !world.active() || piece.age >= visual_seconds(piece.mode) {
            commands.entity(entity).try_despawn();
            continue;
        }
        *transform = pose(&piece);
        let center = transform.translation
            + transform.rotation * ((piece.minimum + piece.maximum) * 0.5) * transform.scale;
        if piece.mode == TreeFallAnimation::Current
            && piece.age > 1.15
            && let Some(floor) = floor(&world.chunks, center, piece.foot.y)
        {
            let mut lowest = f32::INFINITY;
            for x in [piece.minimum.x, piece.maximum.x] {
                for y in [piece.minimum.y, piece.maximum.y] {
                    for z in [piece.minimum.z, piece.maximum.z] {
                        lowest = lowest
                            .min((transform.rotation * (Vec3::new(x, y, z) * transform.scale)).y);
                    }
                }
            }
            transform.translation.y = transform.translation.y.max(floor - lowest + 0.01);
        }
    }
}

fn pose(piece: &Piece) -> Transform {
    if piece.mode == TreeFallAnimation::Magic {
        let t = (piece.age / 5.0).clamp(0.0, 1.0);
        let scale = 1.0 - t * t * (3.0 - 2.0 * t);
        return Transform::from_translation(piece.foot + Vec3::Y * piece.pivot * scale)
            .with_scale(Vec3::splat(scale));
    }
    let progress = ((piece.age - 0.08) / 1.15).clamp(0.0, 1.0);
    let ease = progress * progress * (3.0 - 2.0 * progress);
    let axis = Vec3::Y.cross(piece.direction);
    let rotation = Quat::from_axis_angle(axis, ease * ease * 0.82);
    let detach = (piece.age - if piece.wood { 1.12 } else { 0.86 }).max(0.0);
    let sideways = Vec3::new(-piece.direction.z, 0.0, piece.direction.x);
    let spread = (piece.band as f32 * 2.399_963_1).sin();
    let gravity = if piece.wood { 6.5 } else { 2.2 };
    let translation = piece.foot
        + rotation * Vec3::Y * piece.pivot
        + piece.direction * detach * (0.75 + piece.band as f32 * 0.18)
        + sideways * detach * spread * if piece.wood { 0.25 } else { 0.8 }
        - Vec3::Y * gravity * detach * detach;
    let fracture = Quat::from_euler(
        EulerRot::XYZ,
        detach * spread * 0.2,
        detach * spread * 0.25,
        detach * 0.1,
    );
    let disappear = ((piece.age - if piece.wood { 2.9 } else { 2.45 }) / 0.8).clamp(0.0, 1.0);
    Transform::from_translation(translation)
        .with_rotation(rotation * fracture)
        .with_scale(Vec3::splat(
            1.0 - disappear * disappear * (3.0 - 2.0 * disappear),
        ))
}

fn visual_seconds(mode: TreeFallAnimation) -> f32 {
    if mode == TreeFallAnimation::Current {
        VISUAL_SECONDS
    } else {
        5.0
    }
}

fn floor(chunks: &terra_world::streaming::ChunkStore, center: Vec3, base: f32) -> Option<f32> {
    let x = center.x.floor() as i64;
    let z = center.z.floor() as i64;
    let start = center.y.min(base + 2.0).floor() as i64;
    (0..24).find_map(|i| {
        let y = start - i;
        chunks
            .block_at([x, y, z])
            .filter(|m| !matches!(m, 0 | 7..=9 | 132 | 133 | 195 | 14 | 27 | 40 | 50..=56))
            .map(|_| y as f32 + 1.0)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn whole_tree_tilts_before_its_bands_separate_and_then_releases_geometry() {
        let mut piece = Piece {
            foot: Vec3::ZERO,
            direction: Vec3::X,
            band: 2,
            wood: true,
            pivot: 7.0,
            minimum: Vec3::splat(-0.5),
            maximum: Vec3::splat(0.5),
            age: 0.0,
            mode: TreeFallAnimation::Current,
        };
        let original = pose(&piece);
        assert!(original.translation.abs_diff_eq(Vec3::Y * 7.0, 0.001));
        piece.age = 0.9;
        let tilt = pose(&piece);
        assert!(tilt.translation.x > 1.0 && tilt.translation.y < 7.0);
        assert_eq!(tilt.scale, Vec3::ONE);
        piece.age = 2.0;
        let fallen = pose(&piece);
        assert!(fallen.translation.y < tilt.translation.y);
        piece.age = VISUAL_SECONDS;
        assert!(pose(&piece).scale.abs_diff_eq(Vec3::ZERO, 0.001));
    }

    #[test]
    fn magic_shrinks_the_complete_tree_in_place_smoothly_for_exactly_five_seconds() {
        let mut piece = Piece {
            foot: Vec3::new(10.5, 12.0, -4.5),
            direction: Vec3::X,
            band: 2,
            wood: true,
            pivot: 7.0,
            minimum: Vec3::splat(-0.5),
            maximum: Vec3::splat(0.5),
            age: 0.0,
            mode: TreeFallAnimation::Magic,
        };
        assert_eq!(pose(&piece).scale, Vec3::ONE);
        piece.age = 2.5;
        assert_eq!(pose(&piece).scale, Vec3::splat(0.5));
        assert_eq!(pose(&piece).translation, piece.foot + Vec3::Y * 3.5);
        assert_eq!(pose(&piece).rotation, Quat::IDENTITY);
        piece.age = 5.0;
        assert_eq!(pose(&piece).scale, Vec3::ZERO);
        assert_eq!(pose(&piece).translation, piece.foot);
        assert_eq!(visual_seconds(piece.mode), 5.0);
    }
}

#[cfg(test)]
mod lifecycle_tests;
