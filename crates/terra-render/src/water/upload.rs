use super::material::{WaterAssets, WaterMaterial};
use bevy::{
    asset::RenderAssetUsages,
    camera::primitives::MeshAabb,
    ecs::system::SystemParam,
    mesh::{Indices, PrimitiveTopology},
    prelude::*,
};
use std::collections::HashMap;
use terra_core::GameConfig;
use terra_voxel::SectionPos;
use terra_world::{edits::EditJournal, streaming::ChunkStore};

use super::biome::{ATTRIBUTE_BIOME_TINT, WaterTint};

#[derive(Resource, Default)]
pub struct WaterStatus {
    pub ready: bool,
    /// Every loaded water section has an initial uploaded representation. Later
    /// fluid updates can remain pending without blocking entry into the world.
    pub initial_ready: bool,
    pub visible_tiles: usize,
    pub triangles: usize,
    pub spray_particles: usize,
    pub mist_volumes: usize,
}

struct WaterTile {
    epoch: u64,
    render: Option<(Entity, Handle<Mesh>, usize)>,
}
#[derive(Resource, Default)]
pub(super) struct WaterTiles(HashMap<SectionPos, WaterTile>);

#[derive(Component)]
pub struct WaterSurface;

#[derive(Default)]
pub(super) struct WaterUploadQueue {
    pending: Vec<SectionPos>,
}

#[derive(SystemParam)]
pub(super) struct WaterWorld<'w> {
    chunks: ResMut<'w, ChunkStore>,
    journal: Res<'w, EditJournal>,
    config: Res<'w, GameConfig>,
    focus: Option<Res<'w, terra_world::streaming::ChunkFocus>>,
    terrain: Option<Res<'w, terra_world::streaming::WorldTerrain>>,
}

pub(super) fn upload_water(
    mut commands: Commands,
    mut world: WaterWorld,
    assets: Res<WaterAssets>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut tiles: ResMut<WaterTiles>,
    mut status: ResMut<WaterStatus>,
    (mut budget, mut queue): (
        Option<ResMut<crate::MeshUploadStatus>>,
        Local<WaterUploadQueue>,
    ),
) {
    let _scope = terra_core::profiling::ScopeTimer::new("water.upload");
    if status.ready
        && !world.chunks.is_changed()
        && !world.journal.is_changed()
        && !world.config.is_changed()
    {
        return;
    }
    let removed: Vec<_> = tiles
        .0
        .keys()
        .copied()
        .filter(|p| !world.config.world.water || !world.chunks.loaded.contains_key(p))
        .collect();
    for p in removed {
        if let Some(tile) = tiles.0.remove(&p)
            && let Some((entity, mesh, _)) = tile.render
        {
            crate::transitions::retire(
                &mut commands,
                entity,
                vec![mesh],
                world.config.client.chunk_animation,
            );
        }
    }
    if !world.config.world.water {
        queue.pending.clear();
        *status = WaterStatus {
            ready: true,
            initial_ready: true,
            ..default()
        };
        return;
    }
    let mut uploads = 0;
    let mut ready = true;
    let mut initial_ready = true;
    // Keep upload-budget retries instead of scanning every dry loaded section
    // again while the small water queue drains.
    if world.chunks.is_changed() || world.journal.is_changed() || world.config.is_changed() {
        queue.pending.clear();
        queue
            .pending
            .extend(world.chunks.loaded.iter().filter_map(|(&p, section)| {
                let pending = section.water.is_some()
                    || section.water_epoch != world.journal.water_mesh_epoch(p)
                    || tiles
                        .0
                        .get(&p)
                        .is_some_and(|tile| tile.epoch != section.water_epoch);
                pending.then_some(p)
            }));
    }
    let focus = world.focus.as_ref().map_or([0.0; 3], |f| f.0);
    queue.pending.sort_by_key(|p| {
        (
            crate::meshing::lod::distance_squared(*p, focus) as u64,
            p.y,
            p.z,
            p.x,
        )
    });
    let pending = std::mem::take(&mut queue.pending);
    for position in pending {
        let section = &world.chunks.loaded[&position];
        let represented = tiles.0.contains_key(&position);
        if section.water_epoch != world.journal.water_mesh_epoch(position) {
            ready = false;
            initial_ready &= represented;
            queue.pending.push(position);
            continue;
        }
        if tiles
            .0
            .get(&position)
            .is_some_and(|tile| tile.epoch == section.water_epoch)
        {
            continue;
        }
        let renderable = section
            .water
            .as_ref()
            .is_some_and(|patch| !patch.positions.is_empty() && !patch.indices.is_empty());
        if renderable && uploads == 4 {
            ready = false;
            initial_ready &= represented;
            queue.pending.push(position);
            continue;
        }
        let vertices = section
            .water
            .as_ref()
            .map_or(0, |patch| patch.positions.len());
        if renderable
            && budget
                .as_mut()
                .is_some_and(|b| !b.reserve(crate::upload_budget::UploadKind::Water, vertices))
        {
            ready = false;
            initial_ready &= represented;
            queue.pending.push(position);
            continue;
        }
        let epoch = section.water_epoch;
        let patch = if section.water.is_some() {
            world
                .chunks
                .bypass_change_detection()
                .loaded
                .get_mut(&position)
                .and_then(|section| section.water.take())
        } else {
            None
        };
        let render = if let Some(patch) =
            patch.filter(|patch| !patch.positions.is_empty() && !patch.indices.is_empty())
        {
            let Ok(origin) = position.origin() else {
                ready = false;
                initial_ready &= represented;
                queue.pending.push(position);
                continue;
            };
            let triangles = patch.indices.len() / 3;
            let spray = super::spray::sources(&patch.positions, &patch.surface_effects);
            let normals = if patch.normals.is_empty() {
                vec![[0.0, 1.0, 0.0]; patch.positions.len()]
            } else {
                patch.normals
            };
            let flow = if patch.flow.is_empty() {
                vec![[0.0; 2]; patch.positions.len()]
            } else {
                patch.flow
            };
            let mut surface_effects = if patch.surface_effects.is_empty() {
                vec![[0.0; 4]; patch.positions.len()]
            } else {
                patch.surface_effects
            };
            let biome_tints = world.terrain.as_ref().map(|terrain| {
                let tint = WaterTint::new(&terrain.0, origin);
                for ((effect, point), normal) in surface_effects
                    .iter_mut()
                    .zip(&patch.positions)
                    .zip(&normals)
                {
                    if effect[3] < 0.5
                        && normal[1] > 0.95
                        && (origin[1] as f32 + point[1] - world.config.world.sea_level as f32).abs()
                            < 0.05
                    {
                        effect[2] = tint.ocean_at(*point);
                    }
                }
                patch
                    .positions
                    .iter()
                    .map(|&p| tint.at(p))
                    .collect::<Vec<_>>()
            });
            let mut mesh = Mesh::new(
                PrimitiveTopology::TriangleList,
                RenderAssetUsages::RENDER_WORLD,
            )
            .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, patch.positions)
            .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
            .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, patch.depth_shore)
            .with_inserted_attribute(Mesh::ATTRIBUTE_UV_1, flow)
            .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, surface_effects)
            .with_inserted_attribute(
                super::material::ATTRIBUTE_WAVE_WEIGHT,
                if patch.wave_weights.len() == vertices {
                    patch.wave_weights
                } else {
                    vec![0.0; vertices]
                },
            )
            .with_inserted_indices(Indices::U32(patch.indices));
            if let Some(tints) = biome_tints {
                mesh.insert_attribute(ATTRIBUTE_BIOME_TINT, tints);
            }
            let bounds = mesh.compute_aabb().map(|mut bounds| {
                bounds.half_extents.y += 0.24;
                bounds
            });
            let handle = meshes.add(mesh);
            let entity = commands
                .spawn((
                    WaterSurface,
                    spray,
                    crate::miniature::layers(),
                    terra_core::miniature::MiniatureMesh::Water,
                    Mesh3d(handle.clone()),
                    MeshMaterial3d::<WaterMaterial>(assets.0.clone()),
                    Transform::from_translation(Vec3::from_array(origin.map(|v| v as f32))),
                    Visibility::default(),
                    Name::new(format!("Water {}, {}", position.x, position.z)),
                ))
                .id();
            if let Some(bounds) = bounds {
                commands.entity(entity).insert(bounds);
            }
            uploads += 1;
            if tiles
                .0
                .get(&position)
                .is_none_or(|tile| tile.render.is_none())
            {
                crate::transitions::enter(
                    &mut commands,
                    entity,
                    world.config.client.chunk_animation,
                );
            }
            Some((entity, handle, triangles))
        } else {
            None
        };
        if let Some(old) = tiles.0.insert(position, WaterTile { epoch, render }) {
            remove_tile(old, &mut commands, &mut meshes);
        }
    }
    status.ready = ready;
    status.initial_ready = initial_ready;
    status.visible_tiles = tiles.0.values().filter(|t| t.render.is_some()).count();
    status.triangles = tiles
        .0
        .values()
        .filter_map(|t| t.render.as_ref().map(|(_, _, count)| count))
        .sum();
}

fn remove_tile(tile: WaterTile, commands: &mut Commands, meshes: &mut Assets<Mesh>) {
    if let Some((entity, mesh, _)) = tile.render {
        commands.entity(entity).despawn();
        meshes.remove(mesh.id());
    }
}
