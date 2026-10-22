//! Cached exposed faces of voxel cloud banks. A whole sky is one draw, not an
//! entity per cloud voxel, and wind only updates its transform/uniforms.
use bevy::{
    asset::RenderAssetUsages,
    camera::visibility::NoFrustumCulling,
    light::{NotShadowCaster, NotShadowReceiver},
    mesh::{Indices, PrimitiveTopology},
    pbr::{ExtendedMaterial, MaterialExtension},
    prelude::*,
    render::render_resource::AsBindGroup,
    shader::ShaderRef,
    tasks::{AsyncComputeTaskPool, Task, futures_lite::future},
};
use std::collections::HashSet;
use terra_core::{GameConfig, WeatherQuality};
use terra_player::TerraCamera;
use terra_world::streaming::ChunkStore;

use super::precipitation::{WeatherRenderStatus, WeatherVisual};
mod sunshafts;

const SPACING: f32 = 260.0;
const CELL: f32 = 5.5;
const MIN_HEIGHT: f32 = 200.0;
const MAX_HEIGHT: f32 = 312.0;
const MAX_VERTICAL_GROWTH: f32 = 1.18;

#[derive(Asset, AsBindGroup, Reflect, Debug, Clone)]
struct CloudExtension {
    /// Cover, daylight, underwater visibility, lightning illumination.
    #[uniform(100)]
    weather: Vec4,
    /// Observer position and the sky's horizontal extent.
    #[uniform(101)]
    observer: Vec4,
    #[uniform(102)]
    haze: Vec4,
    /// Authoritative strike position and local flash strength.
    #[uniform(103)]
    lightning: Vec4,
    #[uniform(104)]
    lightning_tint: Vec4,
    /// Sun direction and the strength of shafts through clearing cloud gaps.
    #[uniform(105)]
    sun: Vec4,
}
impl MaterialExtension for CloudExtension {
    fn vertex_shader() -> ShaderRef {
        "shaders/terra_clouds.wgsl".into()
    }
    fn fragment_shader() -> ShaderRef {
        "shaders/terra_clouds.wgsl".into()
    }
    fn enable_prepass() -> bool {
        false
    }
    fn enable_shadows() -> bool {
        false
    }
}
type CloudMaterial = ExtendedMaterial<StandardMaterial, CloudExtension>;

#[derive(Resource, Default)]
struct Sky {
    entity: Option<Entity>,
    mesh: Option<Handle<Mesh>>,
    material: Option<Handle<CloudMaterial>>,
    anchor: Option<[i64; 2]>,
    drift: Vec2,
    quality: WeatherQuality,
    faces: usize,
    pending: Option<Task<(Mesh, usize)>>,
    pending_anchor: [i64; 2],
    pending_quality: WeatherQuality,
    shafts: sunshafts::Sunshafts,
    previous_cover: f32,
    clearing: f32,
}

pub(super) fn install(app: &mut App) {
    app.add_plugins(MaterialPlugin::<CloudMaterial>::default())
        .init_resource::<Sky>()
        .add_systems(
            Update,
            update
                .after(super::precipitation::WeatherSet::Particles)
                .after(crate::water::environment::update),
        );
}

#[derive(bevy::ecs::system::SystemParam)]
struct CloudWorld<'w> {
    time: Res<'w, Time>,
    config: Res<'w, GameConfig>,
    weather: Res<'w, WeatherVisual>,
    daylight: Res<'w, crate::scene::daylight::Daylight>,
    underwater: Res<'w, crate::UnderwaterState>,
    chunks: Res<'w, ChunkStore>,
    authority: Option<Res<'w, terra_core::WorldWeather>>,
    cave: Option<Res<'w, crate::CaveState>>,
}

fn update(
    mut commands: Commands,
    world: CloudWorld,
    cameras: Query<(&Transform, Option<&DistanceFog>), With<TerraCamera>>,
    mut sky: ResMut<Sky>,
    mut status: ResMut<WeatherRenderStatus>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<CloudMaterial>>,
) {
    let Some((camera, fog)) = cameras.iter().next() else {
        return;
    };
    let quality = world.config.client.weather_quality;
    if quality == WeatherQuality::Off || world.chunks.loaded.is_empty() {
        if let Some(entity) = sky.entity.take() {
            commands.entity(entity).despawn();
        }
        if let Some(mesh) = sky.mesh.take() {
            meshes.remove(mesh.id());
        }
        sky.anchor = None;
        sky.pending = None;
        sky.shafts.clear(&mut commands, &mut meshes);
        sky.previous_cover = world.weather.clouds;
        sky.clearing = 0.0;
        status.cloud_faces = 0;
        status.cloud_batches = 0;
        status.cloud_shaft_batches = 0;
        status.cloud_shaft_vertices = 0;
        return;
    }
    sky.drift += world.weather.wind * world.time.delta_secs() * 0.65;
    let anchor = [
        ((camera.translation.x - sky.drift.x) / SPACING).floor() as i64,
        ((camera.translation.z - sky.drift.y) / SPACING).floor() as i64,
    ];
    let radius = match quality {
        WeatherQuality::Low => 2,
        WeatherQuality::Medium | WeatherQuality::High => 3,
        WeatherQuality::Ultra => 4,
        WeatherQuality::Off => 0,
    };
    if sky.material.is_none() {
        sky.material = Some(materials.add(ExtendedMaterial {
            base: StandardMaterial {
                unlit: true,
                ..default()
            },
            extension: CloudExtension {
                weather: Vec4::ZERO,
                observer: Vec4::ZERO,
                haze: Vec4::ZERO,
                lightning: Vec4::ZERO,
                lightning_tint: Vec4::ONE,
                sun: Vec4::ZERO,
            },
        }));
    }
    if sky.pending.is_none() && (sky.anchor != Some(anchor) || sky.quality != quality) {
        sky.pending_anchor = anchor;
        sky.pending_quality = quality;
        sky.pending =
            Some(AsyncComputeTaskPool::get().spawn(async move { cloud_mesh(anchor, radius) }));
    }
    if let Some((mesh, faces)) = sky
        .pending
        .as_mut()
        .and_then(|task| future::block_on(future::poll_once(task)))
    {
        sky.faces = faces;
        if let Some(handle) = &sky.mesh {
            // Render-only extraction takes CPU vertex data. Reinsertion replaces
            // that emptied asset explicitly while preserving the shared GPU ID.
            if let Err(error) = meshes.insert(handle.id(), mesh) {
                tracing::error!(%error,"Unable to replace the voxel sky mesh");
                sky.pending = None;
                sky.anchor = None;
                if let Some(entity) = sky.entity.take() {
                    commands.entity(entity).despawn();
                }
                if let Some(handle) = sky.mesh.take() {
                    meshes.remove(handle.id());
                }
                sky.shafts.clear(&mut commands, &mut meshes);
                status.cloud_faces = 0;
                status.cloud_batches = 0;
                status.cloud_shaft_batches = 0;
                status.cloud_shaft_vertices = 0;
                return;
            }
        } else {
            let handle = meshes.add(mesh);
            if let Some(material) = &sky.material {
                sky.entity = Some(
                    commands
                        .spawn((
                            Name::new("Batched voxel cloud banks"),
                            Mesh3d(handle.clone()),
                            MeshMaterial3d(material.clone()),
                            Transform::default(),
                            NotShadowCaster,
                            NotShadowReceiver,
                            NoFrustumCulling,
                        ))
                        .id(),
                );
            }
            sky.mesh = Some(handle);
        }
        sky.anchor = Some(sky.pending_anchor);
        sky.quality = sky.pending_quality;
        sky.pending = None;
        let anchor = sky.pending_anchor;
        if matches!(
            sky.pending_quality,
            WeatherQuality::High | WeatherQuality::Ultra
        ) {
            sky.shafts
                .rebuild(anchor, &mut commands, &mut meshes, &mut materials);
        } else {
            sky.shafts.clear(&mut commands, &mut meshes);
        }
        status.cloud_uploads += 1;
    }
    if let Some(entity) = sky.entity {
        commands
            .entity(entity)
            .insert(Transform::from_translation(Vec3::new(
                sky.drift.x,
                0.0,
                sky.drift.y,
            )));
    }
    let clearing =
        sunshafts::clearing_strength(&mut sky, world.weather.clouds, world.time.delta_secs());
    let daylight = world.daylight.sun;
    let cave = world.cave.as_ref().map_or(0.0, |cave| cave.blend);
    let shaft_strength = if matches!(quality, WeatherQuality::High | WeatherQuality::Ultra) {
        clearing * (1.0 - cave) * (1.0 - world.underwater.blend)
    } else {
        0.0
    };
    let mut extension = None;
    if let Some(mut material) = sky.material.as_ref().and_then(|h| materials.get_mut(h)) {
        material.extension.weather = Vec4::new(
            world.weather.clouds,
            world.daylight.daylight,
            1.0 - world.underwater.blend,
            world.weather.flash,
        );
        material.extension.observer = camera
            .translation
            .extend((radius as f32 + 0.8) * SPACING * 9.0);
        let haze = fog
            .map_or(Color::srgb(0.53, 0.63, 0.70), |fog| fog.color)
            .to_linear();
        material.extension.haze = Vec4::new(haze.red, haze.green, haze.blue, 1.0);
        let event = world
            .authority
            .as_ref()
            .and_then(|weather| weather.latest_lightning);
        material.extension.lightning = event.map_or(Vec4::ZERO, |event| {
            Vec3::from_array(event.position.map(|value| value as f32)).extend(world.weather.flash)
        });
        material.extension.lightning_tint = event.map_or(Vec4::ONE, |event| {
            super::lightning::lightning_tint(event.id).extend(1.0)
        });
        material.extension.sun = daylight.extend(shaft_strength);
        extension = Some(material.extension.clone());
    }
    let mut shafts_active = false;
    if let Some(extension) = extension {
        let drift = sky.drift;
        shafts_active = sky
            .shafts
            .update(extension, drift, &mut commands, &mut materials);
    }
    status.cloud_faces = sky.faces;
    status.cloud_batches = usize::from(sky.entity.is_some());
    status.cloud_shaft_batches = usize::from(shafts_active);
    status.cloud_shaft_vertices = if shafts_active {
        sunshafts::VERTICES
    } else {
        0
    };
}

fn hash(x: i64, z: i64, salt: u64) -> u64 {
    let mut value = (x as u64).wrapping_mul(0x9e3779b97f4a7c15)
        ^ (z as u64).wrapping_mul(0xbf58476d1ce4e5b9)
        ^ salt;
    value = (value ^ (value >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
    (value ^ (value >> 27)).wrapping_mul(0x94d049bb133111eb)
}
fn random(seed: u64, shift: u32) -> f32 {
    ((seed >> shift) & 255_u64) as f32 / 255.0
}

fn cloud_cells(seed: u64) -> HashSet<[i32; 3]> {
    cloud_cells_lod(seed, 1, 1)
}

fn cloud_cells_lod(seed: u64, horizontal_stride: i32, vertical_stride: i32) -> HashSet<[i32; 3]> {
    let mut cells = HashSet::new();
    let width = 0.96 + random(seed, 8) * 0.12;
    let angle = random(seed, 0) * std::f32::consts::TAU;
    let (sin, cos) = angle.sin_cos();
    let lobes = [
        (Vec3::new(0.0, 2.5, 0.0), Vec3::new(8.5, 4.2, 7.5)),
        (Vec3::new(-8.5, 1.5, -2.7), Vec3::new(6.4, 3.1, 6.6)),
        (Vec3::new(8.0, 1.8, 3.4), Vec3::new(6.8, 3.4, 6.1)),
        (Vec3::new(-4.1, -0.9, 5.8), Vec3::new(6.3, 2.8, 6.5)),
        (Vec3::new(4.6, -1.3, -5.7), Vec3::new(6.1, 2.7, 6.6)),
        (Vec3::new(-4.5, 4.2, -3.8), Vec3::new(5.6, 3.5, 5.0)),
        (Vec3::new(4.9, 4.0, -4.8), Vec3::new(5.6, 3.3, 5.0)),
        (Vec3::new(-6.4, 3.1, 6.6), Vec3::new(5.4, 3.0, 5.2)),
        (Vec3::new(5.9, 2.8, 8.0), Vec3::new(5.5, 3.0, 5.3)),
    ];
    let lobes: Vec<_> = lobes
        .into_iter()
        .enumerate()
        .map(|(i, (center, scale))| {
            let detail = hash(i as i64, 0, seed);
            let offset = Vec3::new(
                random(detail, 0) - 0.5,
                (random(detail, 8) - 0.5) * 1.4,
                random(detail, 16) - 0.5,
            );
            (center + offset, scale * (0.90 + random(detail, 24) * 0.20))
        })
        .collect();
    // Sampling the entire lower silhouette avoids a flat, clipped saucer base.
    let extent = (27 + horizontal_stride - 1) / horizontal_stride;
    for x in -extent..=extent {
        for y in -5 / vertical_stride..=10 / vertical_stride {
            for z in -extent..=extent {
                let px = (x * horizontal_stride) as f32 / (width * 1.4);
                let pz = (z * horizontal_stride) as f32 / 1.4;
                let p = Vec3::new(
                    px * cos + pz * sin,
                    (y * vertical_stride) as f32,
                    -px * sin + pz * cos,
                );
                let inside = lobes
                    .iter()
                    .any(|(center, scale)| ((p - center) / scale).length_squared() < 1.0);
                if inside {
                    cells.insert([x, y, z]);
                }
            }
        }
    }
    cells
}

fn face_runs(cells: &HashSet<[i32; 3]>, normal: [i32; 3]) -> Vec<([i32; 3], i32)> {
    let axis = if normal[0] != 0 {
        0
    } else if normal[1] != 0 {
        1
    } else {
        2
    };
    let run_axis = if axis == 0 { 2 } else { 0 };
    let fixed_axis = 3 - axis - run_axis;
    let mut exposed: Vec<_> = cells
        .iter()
        .copied()
        .filter(|cell| !cells.contains(&std::array::from_fn(|i| cell[i] + normal[i])))
        .collect();
    exposed.sort_unstable_by_key(|cell| [cell[axis], cell[fixed_axis], cell[run_axis]]);
    let mut runs: Vec<([i32; 3], i32)> = Vec::new();
    for cell in exposed {
        if let Some((start, length)) = runs.last_mut()
            && start[axis] == cell[axis]
            && start[fixed_axis] == cell[fixed_axis]
            && start[run_axis] + *length == cell[run_axis]
        {
            *length += 1;
        } else {
            runs.push((cell, 1));
        }
    }
    runs
}

fn bank_height(seed: u64, cells: &HashSet<[i32; 3]>, cell_height: f32) -> f32 {
    let low = cells.iter().map(|p| p[1]).min().unwrap_or(0) as f32 * cell_height;
    let high = (cells.iter().map(|p| p[1]).max().unwrap_or(0) + 1) as f32 * cell_height;
    let first = MIN_HEIGHT - low * MAX_VERTICAL_GROWTH;
    let last = MAX_HEIGHT - high * MAX_VERTICAL_GROWTH;
    // Altitude gets its own full-seed mix. Reading high seed bits directly
    // would collapse small or otherwise correlated seeds to the same base.
    let altitude = hash(seed as i64, 0, 0x414c_5449_5455_4445);
    first + (last - first).max(0.0) * random(altitude, 48)
}

fn cloud_mesh(anchor: [i64; 2], radius: i64) -> (Mesh, usize) {
    let mut positions = Vec::new();
    let mut normals = Vec::new();
    let mut colors = Vec::new();
    let mut uvs = Vec::new();
    let mut anchors = Vec::new();
    let mut indices = Vec::new();
    let faces = [
        ([1, 0, 0], [[1, 0, 0], [1, 1, 0], [1, 1, 1], [1, 0, 1]]),
        ([-1, 0, 0], [[0, 0, 1], [0, 1, 1], [0, 1, 0], [0, 0, 0]]),
        ([0, 1, 0], [[0, 1, 1], [1, 1, 1], [1, 1, 0], [0, 1, 0]]),
        ([0, -1, 0], [[0, 0, 0], [1, 0, 0], [1, 0, 1], [0, 0, 1]]),
        ([0, 0, 1], [[1, 0, 1], [1, 1, 1], [0, 1, 1], [0, 0, 1]]),
        ([0, 0, -1], [[0, 0, 0], [0, 1, 0], [1, 1, 0], [1, 0, 0]]),
    ];
    // Two coarse rings keep distant cumulus visible down to the horizon. The
    // near deck stays detailed; far puffs use fewer faces, not another draw.
    let mut banks = Vec::new();
    for scale in [1, 3, 9] {
        let origin = anchor.map(|coordinate| coordinate.div_euclid(scale));
        for gx in origin[0] - radius..=origin[0] + radius {
            for gz in origin[1] - radius..=origin[1] + radius {
                if scale > 1 && (gx - origin[0]).abs() <= 1 && (gz - origin[1]).abs() <= 1 {
                    continue;
                }
                banks.push((gx, gz, scale));
            }
        }
    }
    for (gx, gz, scale) in banks {
        let seed = hash(gx, gz, 0x434c4f5544 ^ scale as u64);
        let scale = scale as f32;
        let horizontal_stride = if scale == 1.0 { 1 } else { 3 };
        let vertical_stride = if scale == 1.0 { 1 } else { 2 };
        let cells = if scale == 1.0 {
            cloud_cells(seed)
        } else {
            cloud_cells_lod(seed, horizontal_stride, vertical_stride)
        };
        let cell_size = Vec3::new(
            CELL * horizontal_stride as f32 * scale,
            CELL * vertical_stride as f32,
            CELL * horizontal_stride as f32 * scale,
        );
        // Keep the entire bank inside the requested altitude band, including
        // shader growth during storms. Neighbouring banks retain distinct bases.
        let origin = Vec3::new(
            (gx as f32 * SPACING + random(seed, 0) * 24.0) * scale,
            bank_height(seed, &cells, cell_size.y),
            (gz as f32 * SPACING + random(seed, 32) * 24.0) * scale,
        );
        for (normal, corners) in faces {
            let run_axis = if normal[0] != 0 { 2 } else { 0 };
            for (cell, length) in face_runs(&cells, normal) {
                let first = positions.len() as u32;
                let face_light = 0.68 + normal[1] as f32 * 0.21 - normal[0] as f32 * 0.07;
                for corner in corners {
                    let p = origin
                        + Vec3::from_array(std::array::from_fn(|i| {
                            (cell[i] + corner[i] * if i == run_axis { length } else { 1 }) as f32
                        })) * cell_size;
                    positions.push(p.to_array());
                    normals.push(normal.map(|v| v as f32));
                    colors.push([
                        face_light,
                        random(seed, 40),
                        (((cell[1] * vertical_stride) as f32 + 5.0) / 15.0).clamp(0.0, 1.0),
                        1.0,
                    ]);
                    uvs.push([origin.x, origin.z]);
                    anchors.push([origin.y, 0.0]);
                }
                indices.extend([first, first + 1, first + 2, first, first + 2, first + 3]);
            }
        }
    }
    let count = positions.len() / 4;
    (
        Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::RENDER_WORLD,
        )
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
        .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, colors)
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uvs)
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_1, anchors)
        .with_inserted_indices(Indices::U32(indices)),
        count,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn render_only_cloud_mesh_reinsertion_keeps_its_handle_after_extraction() {
        let mut meshes = Assets::<Mesh>::default();
        let (mesh, _) = cloud_mesh([0, 0], 0);
        let handle = meshes.add(mesh);
        assert!(meshes.get_mut(&handle).unwrap().take_gpu_data().is_ok());
        let (replacement, faces) = cloud_mesh([-1, -2], 0);
        assert!(meshes.insert(handle.id(), replacement).is_ok());
        assert_eq!(meshes.get(&handle).unwrap().count_vertices(), faces * 4);
        assert!(meshes.get_mut(&handle).unwrap().take_gpu_data().is_ok());
        let (replacement, faces) = cloud_mesh([-1, -2], 1);
        assert!(meshes.insert(handle.id(), replacement).is_ok());
        assert_eq!(meshes.get(&handle).unwrap().count_vertices(), faces * 4);
    }
    #[test]
    fn cloud_banks_are_connected_volumes_with_only_exposed_valid_faces() {
        for seed in [0, 19, u64::MAX] {
            let cells = cloud_cells(seed);
            let mut seen = HashSet::new();
            let mut frontier = vec![*cells.iter().next().unwrap()];
            while let Some(p) = frontier.pop() {
                if !seen.insert(p) {
                    continue;
                }
                for offset in [
                    [1, 0, 0],
                    [-1, 0, 0],
                    [0, 1, 0],
                    [0, -1, 0],
                    [0, 0, 1],
                    [0, 0, -1],
                ] {
                    let q = std::array::from_fn(|i| p[i] + offset[i]);
                    if cells.contains(&q) && !seen.contains(&q) {
                        frontier.push(q);
                    }
                }
            }
            assert_eq!(cells.len(), seen.len());
            assert!(
                cells.iter().any(|p| p[1] < -2),
                "cloud bottom must not be clipped flat"
            );
            assert!(
                cells.iter().any(|p| p[1] >= 6),
                "the central puff must give the bank real volume"
            );
        }
        let (mesh, faces) = cloud_mesh([-4, -7], 3);
        assert!(faces > 10_000 && faces < 180_000, "{faces}");
        assert_eq!(mesh.count_vertices(), faces * 4);
        assert!(
            mesh.indices()
                .unwrap()
                .iter()
                .all(|index| index < mesh.count_vertices())
        );
    }

    #[test]
    fn broad_cloud_banks_can_join_into_a_deck_without_growing_tall_columns() {
        let cells = cloud_cells(19);
        let bounds = |axis: usize| {
            let min = cells.iter().map(|p| p[axis]).min().unwrap();
            let max = cells.iter().map(|p| p[axis]).max().unwrap();
            (max - min + 1) as f32 * CELL
        };
        assert!(bounds(0) > bounds(1) * 2.5);
        assert!(bounds(2) > bounds(1) * 2.5);
        assert!(bounds(0) * 2.0 > SPACING);
        assert!(bounds(2) * 2.0 > SPACING);
    }

    #[test]
    fn simplified_far_clouds_reach_the_low_sun_without_expanding_terrain_visibility() {
        let (mesh, faces) = cloud_mesh([0, 0], 3);
        let Some(bevy::mesh::VertexAttributeValues::Float32x3(positions)) =
            mesh.attribute(Mesh::ATTRIBUTE_POSITION)
        else {
            panic!("Expected cloud positions");
        };
        assert!(positions.iter().any(|p| p[0].abs() > 6_000.0));
        assert!(
            positions
                .iter()
                .all(|p| p[1] >= MIN_HEIGHT && p[1] <= MAX_HEIGHT)
        );
        assert!(
            faces < 180_000,
            "Far LOD exceeded the shared sky budget: {faces}"
        );
    }

    #[test]
    fn distinct_cloud_altitudes_stay_bounded_even_at_maximum_storm_growth() {
        let mut bases = HashSet::new();
        for seed in 0..24 {
            for (horizontal, vertical) in [(1, 1), (3, 2)] {
                let cells = cloud_cells_lod(seed, horizontal, vertical);
                let cell_height = CELL * vertical as f32;
                let origin = bank_height(seed, &cells, cell_height);
                bases.insert(origin.to_bits());
                for cell in &cells {
                    for corner in [0.0, 1.0] {
                        for growth in [0.0, 0.9, 1.18] {
                            let y = origin + (cell[1] as f32 + corner) * cell_height * growth;
                            assert!(
                                (MIN_HEIGHT - 0.000_1..=MAX_HEIGHT + 0.000_1).contains(&y),
                                "{seed}: {y}"
                            );
                        }
                    }
                }
            }
        }
        assert!(
            bases.len() >= 24,
            "Clouds must not share one flat base altitude"
        );
    }

    #[test]
    fn altitude_uses_low_seed_bits_even_when_bank_geometry_is_identical() {
        let cells = cloud_cells(19);
        let heights: Vec<_> = (0..24)
            .map(|seed| bank_height(seed, &cells, CELL))
            .collect();
        let distinct: HashSet<_> = heights.iter().map(|height| height.to_bits()).collect();
        assert!(
            distinct.len() >= 20,
            "Altitude must vary independently of puff geometry"
        );
        let low = heights.iter().copied().fold(f32::INFINITY, f32::min);
        let high = heights.iter().copied().fold(f32::NEG_INFINITY, f32::max);
        assert!(
            high - low > 12.0,
            "Different banks need visibly different bases, not float jitter"
        );
        assert_eq!(heights[7], bank_height(7, &cells, CELL));
    }

    #[test]
    fn merged_cloud_faces_preserve_every_exposed_voxel() {
        let cells = cloud_cells(19);
        for normal in [
            [1, 0, 0],
            [-1, 0, 0],
            [0, 1, 0],
            [0, -1, 0],
            [0, 0, 1],
            [0, 0, -1],
        ] {
            let expected = cells
                .iter()
                .filter(|cell| !cells.contains(&std::array::from_fn(|i| cell[i] + normal[i])))
                .count();
            let runs = face_runs(&cells, normal);
            assert_eq!(
                runs.iter()
                    .map(|(_, length)| *length as usize)
                    .sum::<usize>(),
                expected
            );
            let run_axis = if normal[0] != 0 { 2 } else { 0 };
            for (start, length) in runs {
                for step in 0..length {
                    let mut cell = start;
                    cell[run_axis] += step;
                    assert!(cells.contains(&cell));
                    assert!(!cells.contains(&std::array::from_fn(|i| cell[i] + normal[i])));
                }
            }
        }
    }
}
