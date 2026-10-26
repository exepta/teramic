//! Native views of generated biome features. No fixture blocks or world edits.
use bevy::{
    app::AppExit,
    prelude::*,
    render::view::screenshot::{Screenshot, ScreenshotCaptured, save_to_disk},
};
use std::{
    path::PathBuf,
    time::{Duration, Instant},
};
use terra_player::{ControllerMode, TerraCamera};
use terra_render::{RenderStatus, WaterStatus};
use terra_voxel::{MaterialId, SectionPos};
use terra_world::{
    SectionGenerator, TerraGenerator, TerrainGenerator,
    streaming::{ChunkFocus, ChunkStore, GenerationStatus, WorldGeneration, WorldTerrain},
};

#[derive(Clone, Copy)]
enum Scene {
    Cave,
    Mushroom,
    Oasis,
    Highland,
    GiantCave,
    WaterMushroom,
    WaterDesert,
    WaterHighland,
    WaterTransition,
    Cherry,
    CherryBrook,
}

impl Scene {
    fn name(self) -> &'static str {
        match self {
            Self::Cave => "cave",
            Self::Mushroom => "mushroom",
            Self::Oasis => "oasis",
            Self::Highland => "highland",
            Self::GiantCave => "giant-cave",
            Self::WaterMushroom => "water-mushroom",
            Self::WaterDesert => "water-desert",
            Self::WaterHighland => "water-highland",
            Self::WaterTransition => "water-transition",
            Self::Cherry => "cherry",
            Self::CherryBrook => "cherry-brook",
        }
    }

    fn water(self) -> bool {
        matches!(
            self,
            Self::WaterMushroom
                | Self::WaterDesert
                | Self::WaterHighland
                | Self::WaterTransition
                | Self::CherryBrook
        )
    }
}

struct Views {
    overview: Transform,
    close: Transform,
    anchor: Vec3,
    probes: Vec<([i64; 3], u16)>,
}

#[derive(Resource)]
struct BiomeSmoke {
    scene: Scene,
    views: Option<Views>,
    started: Instant,
    since: Instant,
    stage: usize,
    captures: usize,
    requested: bool,
    failed: bool,
}

pub(crate) fn install(app: &mut App) {
    if !std::env::args().any(|arg| arg == "--smoke-test-biomes") {
        return;
    }
    let (scene, failed) = match std::env::var("TERRA_BIOME_SCENE").as_deref() {
        Ok("cave") | Err(_) => (Scene::Cave, false),
        Ok("mushroom") => (Scene::Mushroom, false),
        Ok("oasis") => (Scene::Oasis, false),
        Ok("highland") => (Scene::Highland, false),
        Ok("giant-cave" | "giant_cave") => (Scene::GiantCave, false),
        Ok("water-mushroom") => (Scene::WaterMushroom, false),
        Ok("water-desert") => (Scene::WaterDesert, false),
        Ok("water-highland") => (Scene::WaterHighland, false),
        Ok("water-transition") => (Scene::WaterTransition, false),
        Ok("cherry") => (Scene::Cherry, false),
        Ok("cherry-brook") => (Scene::CherryBrook, false),
        _ => (Scene::Cave, true),
    };
    let now = Instant::now();
    app.insert_resource(ControllerMode::Fly)
        .insert_resource(BiomeSmoke {
            scene,
            views: None,
            started: now,
            since: now,
            stage: 0,
            captures: 0,
            requested: false,
            failed,
        })
        .add_systems(PostStartup, locate)
        .add_systems(Update, run.after(terra_player::PlayerSet::Movement));
}

fn locate(world: &mut World) {
    let terrain = world.resource::<WorldTerrain>().0.clone();
    let scene = world.resource::<BiomeSmoke>().scene;
    let views = match scene {
        Scene::Cave => cave_views(&terrain),
        Scene::Mushroom => mushroom_views(&terrain),
        Scene::Oasis => oasis_views(&terrain),
        Scene::Highland => highland_views(&terrain),
        Scene::Cherry => cherry_views(&terrain),
        Scene::CherryBrook => cherry_brook_views(&terrain),
        Scene::GiantCave => giant_cave_views(&terrain),
        Scene::WaterMushroom
        | Scene::WaterDesert
        | Scene::WaterHighland
        | Scene::WaterTransition => water_biome_views(&terrain, scene),
    };
    let Some(views) = views else {
        world.resource_mut::<BiomeSmoke>().failed = true;
        error!(
            scene = scene.name(),
            "No generated feature found for native biome view"
        );
        return;
    };
    world
        .resource_mut::<terra_core::GameConfig>()
        .client
        .fov_degrees = 55.0;
    world
        .resource_mut::<bevy::gizmos::config::GizmoConfigStore>()
        .config_mut::<bevy::gizmos::config::DefaultGizmoConfigGroup>()
        .0
        .enabled = false;
    set_camera(world, views.overview);
    info!(scene = scene.name(), anchor = ?views.anchor, eye = ?views.overview.translation,
        "Native biome fixture uses generated terrain");
    world.resource_mut::<BiomeSmoke>().views = Some(views);
}

fn set_camera(world: &mut World, transform: Transform) {
    for mut camera in world
        .query_filtered::<&mut Transform, With<TerraCamera>>()
        .iter_mut(world)
    {
        *camera = transform;
    }
    world.resource_mut::<ChunkFocus>().0 = transform.translation.to_array().map(f64::from);
}

fn surface_eye(terrain: &TerraGenerator, mut eye: Vec3) -> Vec3 {
    eye.y = eye
        .y
        .max(terrain.surface_height(f64::from(eye.x), f64::from(eye.z)) as f32 + 2.0);
    eye
}

fn cave_views(terrain: &TerraGenerator) -> Option<Views> {
    for [mx, mz] in [[-2828.0, -4078.0], [-2307.0, -4119.0], [-2172.0, -4117.0]] {
        let mouth = terrain.surface_column(mx, mz);
        let Some([floor, _]) = mouth.cave_cavity else {
            continue;
        };
        for i in 0..8 {
            let angle = f64::from(i) * std::f64::consts::FRAC_PI_4;
            let direction = [angle.cos(), angle.sin()];
            let passage: Option<Vec<_>> = [6.0, 12.0, 24.0, 40.0]
                .into_iter()
                .map(|distance| {
                    let x = mx + direction[0] * distance;
                    let z = mz + direction[1] * distance;
                    let column = terrain.surface_column(x, z);
                    column.cave_cavity.map(|cavity| (x, z, column, cavity))
                })
                .collect();
            let Some(passage) = passage else { continue };
            let (_, _, last_column, last) = passage[3];
            if last[0] >= floor - 4.0 || last[1] + 3.0 > last_column.height {
                continue;
            }
            let forward = Vec3::new(direction[0] as f32, 0.0, direction[1] as f32);
            let mouth = Vec3::new(mx as f32, floor as f32, mz as f32);
            let overview = surface_eye(terrain, mouth - forward * 13.0 + Vec3::new(3.0, 5.0, 0.0));
            let (x, z, _, cavity) = passage[0];
            let close = Vec3::new(x as f32, cavity[0] as f32 + 2.0, z as f32);
            let (tx, tz, _, target) = passage[1];
            let (rx, rz, _, roof) = passage[3];
            let air = [
                x.floor() as i64,
                (cavity[0] + 1.5).floor() as i64,
                z.floor() as i64,
            ];
            let roof_block = [rx.floor() as i64, roof[1].ceil() as i64, rz.floor() as i64];
            let generator = SectionGenerator::new(terrain.clone());
            let roof_material = generator.block_at(roof_block).ok()?;
            if generator.block_at(air).ok()? != 0 || roof_material == 0 {
                continue;
            }
            return Some(Views {
                overview: Transform::from_translation(overview)
                    .looking_at(mouth + forward * 7.0 + Vec3::Y * 2.0, Vec3::Y),
                close: Transform::from_translation(close).looking_at(
                    Vec3::new(tx as f32, target[0] as f32 + 2.0, tz as f32),
                    Vec3::Y,
                ),
                anchor: mouth,
                probes: vec![(air, 0), (roof_block, roof_material)],
            });
        }
    }
    None
}

fn find_material(
    terrain: &TerraGenerator,
    anchor: [f64; 2],
    material: MaterialId,
    radius: i64,
) -> Option<[i64; 3]> {
    let generator = SectionGenerator::new(terrain.clone());
    let center = anchor.map(|v| (v.floor() as i64).div_euclid(16));
    for ring in 0..=radius {
        for dz in -ring..=ring {
            for dx in -ring..=ring {
                if dx.abs().max(dz.abs()) != ring {
                    continue;
                }
                let coordinate = [center[0] + dx, center[1] + dz];
                let height = terrain.surface_height(
                    (coordinate[0] * 16 + 8) as f64,
                    (coordinate[1] * 16 + 8) as f64,
                );
                let column = generator.prepare_column(coordinate, false).ok()?;
                let bottom = (height.floor() as i64).div_euclid(16);
                for y in bottom..=bottom + 2 {
                    let position = SectionPos::new(coordinate[0], y, coordinate[1]);
                    let section = generator
                        .build_prepared(position, 0, &Default::default(), &column)
                        .ok()?;
                    for (index, &id) in section.data.blocks().iter().enumerate() {
                        if id != material as u16 {
                            continue;
                        }
                        let point = [
                            coordinate[0] * 16 + (index % 16) as i64,
                            y * 16 + (index / 16 % 16) as i64,
                            coordinate[1] * 16 + (index / 256) as i64,
                        ];
                        if point[1] as f64
                            > terrain.surface_height(point[0] as f64 + 0.5, point[2] as f64 + 0.5)
                                + 2.0
                        {
                            return Some(point);
                        }
                    }
                }
            }
        }
    }
    None
}

fn giant_cave_views(terrain: &TerraGenerator) -> Option<Views> {
    let center = [98_i64, -25, -92];
    let generator = SectionGenerator::new(terrain.clone());
    let air_width = (-40..=40)
        .filter(|dx| {
            terrain
                .sample([
                    (center[0] + dx) as f64 + 0.5,
                    center[1] as f64 + 0.5,
                    center[2] as f64 + 0.5,
                ])
                .density
                > 0.0
        })
        .count();
    if air_width < 60 {
        return None;
    }
    let floor = (-64..center[1]).rev().find_map(|y| {
        let point = [center[0], y, center[2]];
        let material = generator.block_at(point).ok()?;
        (material != 0).then_some((point, material))
    })?;
    let overview_eye = [center[0] - 24, center[1] + 3, center[2] + 12];
    let close_eye = [center[0] + 8, floor.0[1] + 6, center[2]];
    if [overview_eye, close_eye]
        .into_iter()
        .any(|point| generator.block_at(point).ok() != Some(0))
    {
        return None;
    }
    let center = Vec3::from_array(center.map(|v| v as f32 + 0.5));
    info!(
        ?floor,
        air_width, "Giant cave fixture uses a natural chamber, without extra lighting"
    );
    Some(Views {
        overview: Transform::from_translation(Vec3::from_array(
            overview_eye.map(|v| v as f32 + 0.5),
        ))
        .looking_at(center + Vec3::new(12.0, -4.0, -7.0), Vec3::Y),
        close: Transform::from_translation(Vec3::from_array(close_eye.map(|v| v as f32 + 0.5)))
            .looking_at(center + Vec3::new(15.0, 2.0, -10.0), Vec3::Y),
        anchor: center,
        probes: vec![(overview_eye, 0), (close_eye, 0), floor],
    })
}

fn mushroom_views(terrain: &TerraGenerator) -> Option<Views> {
    let stem = find_material(terrain, [-640.0, 640.0], MaterialId::MushroomStem, 6)?;
    let point = Vec3::from_array(stem.map(|v| v as f32 + 0.5));
    let offset = Vec3::new(24.0, 17.0, 28.0);
    let offset = std::env::var("TERRA_MUSHROOM_LOD_DISTANCE")
        .ok()
        .and_then(|value| value.parse::<f32>().ok())
        .filter(|distance| distance.is_finite() && (32.0..=200.0).contains(distance))
        .map_or(offset, |distance| offset.normalize() * distance);
    let eye = surface_eye(terrain, point + offset);
    let close = surface_eye(terrain, point + Vec3::new(10.0, 2.0, 12.0));
    Some(Views {
        overview: Transform::from_translation(eye).looking_at(point + Vec3::Y * 3.0, Vec3::Y),
        close: Transform::from_translation(close).looking_at(point + Vec3::Y * 3.0, Vec3::Y),
        anchor: point,
        probes: vec![(stem, MaterialId::MushroomStem as u16)],
    })
}

fn cherry_views(terrain: &TerraGenerator) -> Option<Views> {
    for z in (-4096..=4096).step_by(128) {
        for x in (-4096..=4096).step_by(128) {
            let column = terrain.surface_column(x as f64, z as f64);
            if column.biome.weights.cherry_forest < 0.65
                || column.water_level.is_some()
                || column.slope > 1.0
            {
                continue;
            }
            let Some(leaf) =
                find_material(terrain, [x as f64, z as f64], MaterialId::CherryLeaves, 3)
            else {
                continue;
            };
            let target = Vec3::from_array(leaf.map(|v| v as f32)) - Vec3::Y * 2.0;
            let overview = surface_eye(terrain, target + Vec3::new(24.0, 14.0, 30.0));
            let generator = SectionGenerator::new(terrain.clone());
            let close = [12.0, 18.0, 24.0]
                .into_iter()
                .find_map(|radius| {
                    (0..8).find_map(|n| {
                        let angle = n as f32 * std::f32::consts::FRAC_PI_4;
                        let eye = surface_eye(
                            terrain,
                            target + Vec3::new(angle.sin() * radius, 4.0, angle.cos() * radius),
                        );
                        (generator.block_at(eye.floor().as_i64vec3().to_array()).ok() == Some(0))
                            .then_some(eye)
                    })
                })
                .unwrap_or(overview);
            return Some(Views {
                overview: Transform::from_translation(overview).looking_at(target, Vec3::Y),
                close: Transform::from_translation(close).looking_at(target, Vec3::Y),
                anchor: target,
                probes: vec![(leaf, MaterialId::CherryLeaves as u16)],
            });
        }
    }
    None
}

fn cherry_brook_views(terrain: &TerraGenerator) -> Option<Views> {
    let generator = SectionGenerator::new(terrain.clone());
    for z in (-4096..=4096).step_by(16) {
        for cx in -8..=8 {
            let origin = cx as f64 * 512.0 - 100.0;
            if terrain.biome_at(origin, z as f64).weights.cherry_forest < 0.6 {
                continue;
            }
            for dx in (0..100).step_by(4) {
                let x = origin + f64::from(dx) + 0.5;
                let z = z as f64 + 0.5;
                let Some(river) = terrain.river_at(x, z) else {
                    continue;
                };
                if river.width > 3.5
                    || river.distance > 0.6
                    || river.level < terrain.config().sea_level + 1.0
                    || river.roof_blend > 0.1
                {
                    continue;
                }
                let Some(water) = terrain.water_column(x, z) else {
                    continue;
                };
                let bed = [x.floor() as i64, water.bed as i64 - 1, z.floor() as i64];
                let air = [bed[0], water.level.ceil() as i64, bed[2]];
                let material = generator.block_at(bed).ok()?;
                if material == 0 || generator.block_at(air).ok()? != 0 {
                    continue;
                }
                let target = Vec3::new(x as f32, water.level as f32, z as f32);
                let overview = surface_eye(terrain, target + Vec3::new(11.0, 12.0, 18.0));
                let close = surface_eye(terrain, target + Vec3::new(3.0, 2.0, 7.0));
                return Some(Views {
                    overview: Transform::from_translation(overview).looking_at(target, Vec3::Y),
                    close: Transform::from_translation(close).looking_at(target, Vec3::Y),
                    anchor: target,
                    probes: vec![(bed, material), (air, 0)],
                });
            }
        }
    }
    None
}

fn highland_views(terrain: &TerraGenerator) -> Option<Views> {
    let generator = SectionGenerator::new(terrain.clone());
    for anchor in [[512.0, -512.0], [1024.0, 768.0], [-256.0, 256.0]] {
        let mut snow = None;
        let mut grass = None;
        for z in (-64..=64).step_by(8) {
            for x in (-64..=64).step_by(8) {
                let x = anchor[0] + f64::from(x) + 0.5;
                let z = anchor[1] + f64::from(z) + 0.5;
                let column = terrain.surface_column(x, z);
                if column.height < 80.0 {
                    continue;
                }
                let point = [
                    x.floor() as i64,
                    (column.height - 0.5).floor() as i64,
                    z.floor() as i64,
                ];
                let actual = generator.block_at(point).ok()?;
                if actual == MaterialId::Snow as u16
                    && snow.is_none_or(|previous: [i64; 3]| previous[1] < point[1])
                {
                    snow = Some(point);
                }
                if actual == MaterialId::Grass as u16 {
                    grass = Some(point);
                }
            }
        }
        let (Some(snow), Some(grass)) = (snow, grass) else {
            continue;
        };
        let Some(trunk) = find_spruce(terrain, anchor) else {
            continue;
        };
        let peak = Vec3::from_array(snow.map(|v| v as f32 + 0.5));
        let tree = Vec3::from_array(trunk.map(|v| v as f32 + 0.5));
        let overview = surface_eye(terrain, peak + Vec3::new(55.0, 34.0, 68.0));
        let close = surface_eye(terrain, tree + Vec3::new(28.0, 11.0, 34.0));
        info!(
            ?snow,
            ?grass,
            ?trunk,
            "Highland fixture found natural snow, grass and a single-column spruce"
        );
        return Some(Views {
            overview: Transform::from_translation(overview).looking_at(peak, Vec3::Y),
            close: Transform::from_translation(close).looking_at(tree + Vec3::Y * 3.0, Vec3::Y),
            anchor: peak,
            probes: vec![
                (snow, MaterialId::Snow as u16),
                (grass, MaterialId::Grass as u16),
                (trunk, MaterialId::Wood as u16),
            ],
        });
    }
    None
}

fn find_spruce(terrain: &TerraGenerator, anchor: [f64; 2]) -> Option<[i64; 3]> {
    let generator = SectionGenerator::new(terrain.clone());
    let center = anchor.map(|v| (v.floor() as i64).div_euclid(16));
    for ring in 0_i64..=3 {
        for dz in -ring..=ring {
            for dx in -ring..=ring {
                if dx.abs().max(dz.abs()) != ring {
                    continue;
                }
                let coordinate = [center[0] + dx, center[1] + dz];
                let column = generator.prepare_column(coordinate, false).ok()?;
                let surface = terrain.surface_height(
                    (coordinate[0] * 16 + 8) as f64,
                    (coordinate[1] * 16 + 8) as f64,
                );
                let bottom = (surface.floor() as i64).div_euclid(16) - 1;
                let sections: Option<Vec<_>> = (bottom..=bottom + 4)
                    .map(|y| {
                        generator
                            .build_prepared(
                                SectionPos::new(coordinate[0], y, coordinate[1]),
                                0,
                                &Default::default(),
                                &column,
                            )
                            .ok()
                            .map(|section| section.data)
                    })
                    .collect();
                let sections = sections?;
                let material = |x: usize, y: i64, z: usize| {
                    let index = usize::try_from(y.div_euclid(16) - bottom).ok()?;
                    sections.get(index).map(|section| {
                        section.blocks()[x + 16 * (y.rem_euclid(16) as usize + 16 * z)]
                    })
                };
                for z in 1..15 {
                    for x in 1..15 {
                        let global = [coordinate[0] * 16 + x as i64, coordinate[1] * 16 + z as i64];
                        let floor = (terrain
                            .surface_height(global[0] as f64 + 0.5, global[1] as f64 + 0.5)
                            - 0.5)
                            .floor() as i64
                            + 1;
                        let wood = Some(MaterialId::Wood as u16);
                        if !(0..17).all(|y| material(x, floor + y, z) == wood) {
                            continue;
                        }
                        if [6, 12].into_iter().any(|y| {
                            [[x - 1, z], [x + 1, z], [x, z - 1], [x, z + 1]]
                                .into_iter()
                                .any(|[nx, nz]| material(nx, floor + y, nz) == wood)
                        }) {
                            continue;
                        }
                        return Some([global[0], floor + 8, global[1]]);
                    }
                }
            }
        }
    }
    None
}

fn oasis_views(terrain: &TerraGenerator) -> Option<Views> {
    let oasis = terrain.oasis_at(-2682.0, -153.0)?;
    let water = terrain.water_column(oasis.center[0], oasis.center[1])?;
    let point = Vec3::new(
        oasis.center[0] as f32,
        water.level as f32,
        oasis.center[1] as f32,
    );
    let palm = find_material(terrain, oasis.center, MaterialId::Wood, 4)?;
    let palm_point = Vec3::from_array(palm.map(|v| v as f32 + 0.5));
    let eye = surface_eye(terrain, point + Vec3::new(34.0, 27.0, 40.0));
    let inward = ((point - palm_point) * Vec3::new(1.0, 0.0, 1.0)).normalize_or_zero();
    let close = surface_eye(
        terrain,
        palm_point - inward * 15.0 + Vec3::new(6.0, 3.0, 0.0),
    );
    Some(Views {
        overview: Transform::from_translation(eye).looking_at(point + Vec3::Y * 2.0, Vec3::Y),
        close: Transform::from_translation(close)
            .looking_at(palm_point + inward * 3.0 + Vec3::Y * 4.0, Vec3::Y),
        anchor: point,
        probes: vec![(palm, MaterialId::Wood as u16)],
    })
}

fn water_biome_views(terrain: &TerraGenerator, scene: Scene) -> Option<Views> {
    let anchors: &[[f64; 2]] = match scene {
        Scene::WaterMushroom => &[[-1504.0, -672.0], [-6496.0, -7296.0]],
        Scene::WaterDesert => &[[-2682.0, -153.0], [-3072.0, 0.0], [2304.0, -8192.0]],
        Scene::WaterHighland => &[[512.0, -512.0], [1024.0, 768.0], [-256.0, 256.0]],
        Scene::WaterTransition => &[[-640.0, 640.0], [-2048.0, 0.0], [512.0, -512.0]],
        _ => return None,
    };
    let mut best = None;
    // Smoke-only bounded sampling; production startup and generation are untouched.
    // The half-cell convention agrees with actual voxel occupancy and water patches.
    for anchor in anchors {
        for dz in (-512..=512).step_by(16) {
            for dx in (-512..=512).step_by(16) {
                let point = [
                    anchor[0] + f64::from(dx) + 0.5,
                    anchor[1] + f64::from(dz) + 0.5,
                ];
                select_water_view(terrain, scene, point, &mut best);
            }
        }
    }
    let (_, coarse) = best?;
    for dz in (-14..=14).step_by(2) {
        for dx in (-14..=14).step_by(2) {
            select_water_view(
                terrain,
                scene,
                [coarse[0] + f64::from(dx), coarse[1] + f64::from(dz)],
                &mut best,
            );
        }
    }
    let (_, center) = best?;
    let water = terrain.water_column(center[0], center[1])?;
    let weights = terrain.biome_at(center[0], center[1]).weights.values();
    let point = Vec3::new(center[0] as f32, water.level as f32, center[1] as f32);
    let overview = surface_eye(terrain, point + Vec3::new(29.0, 30.0, 34.0));
    let close = surface_eye(terrain, point + Vec3::new(7.0, 9.0, 11.0));
    let generator = SectionGenerator::new(terrain.clone());
    let bed = [
        center[0].floor() as i64,
        water.bed as i64 - 1,
        center[1].floor() as i64,
    ];
    let air = [bed[0], water.level.ceil() as i64, bed[2]];
    let bed_material = generator.block_at(bed).ok()?;
    if bed_material == 0 || generator.block_at(air).ok()? != 0 {
        return None;
    }
    info!(
        scene = scene.name(),
        seed = terrain.config().seed.0,
        rivers = terrain.config().rivers,
        ?center,
        ?weights,
        depth = water.depth,
        level = water.level,
        "Water palette fixture selected real open-air generated water"
    );
    Some(Views {
        overview: Transform::from_translation(overview).looking_at(point, Vec3::Y),
        close: Transform::from_translation(close).looking_at(point, Vec3::Y),
        anchor: point,
        probes: vec![(bed, bed_material), (air, 0)],
    })
}

fn select_water_view(
    terrain: &TerraGenerator,
    scene: Scene,
    point: [f64; 2],
    best: &mut Option<(f64, [f64; 2])>,
) {
    let weights = terrain.biome_at(point[0], point[1]).weights;
    let prominence = match scene {
        Scene::WaterMushroom => weights.mushroom_park,
        Scene::WaterDesert => weights.desert + weights.cliffted_desert,
        Scene::WaterHighland => weights.highland,
        Scene::WaterTransition => {
            let special =
                weights.mushroom_park + weights.desert + weights.cliffted_desert + weights.highland;
            if !(0.2..=0.8).contains(&special) {
                return;
            }
            4.0 * special * (1.0 - special)
        }
        _ => return,
    };
    let minimum = match scene {
        Scene::WaterMushroom | Scene::WaterDesert => 0.75,
        Scene::WaterHighland => 0.65,
        _ => 0.35,
    };
    if prominence < minimum {
        return;
    }
    let Some(water) = terrain.water_column(point[0], point[1]) else {
        return;
    };
    if water.depth < 1.0 || terrain.surface_height(point[0], point[1]) >= water.level {
        return;
    }
    let openness = [[-4.0, 0.0], [4.0, 0.0], [0.0, -4.0], [0.0, 4.0]]
        .into_iter()
        .filter(|offset| {
            terrain
                .water_column(point[0] + offset[0], point[1] + offset[1])
                .is_some_and(|neighbor| (neighbor.level - water.level).abs() < 0.5)
        })
        .count();
    let mut score = prominence * 4.0 + openness as f64 * 0.1 + water.depth.min(6.0) * 0.02;
    if matches!(scene, Scene::WaterTransition) {
        let gradient = [[-32.0, 0.0], [32.0, 0.0], [0.0, -32.0], [0.0, 32.0]]
            .into_iter()
            .filter_map(|offset| {
                let p = [point[0] + offset[0], point[1] + offset[1]];
                terrain.water_column(p[0], p[1])?;
                let other = terrain.biome_at(p[0], p[1]).weights;
                Some(
                    (weights.desert + weights.cliffted_desert
                        - other.desert
                        - other.cliffted_desert)
                        .abs()
                        + (weights.highland - other.highland).abs()
                        + (weights.mushroom_park - other.mushroom_park).abs(),
                )
            })
            .fold(0.0_f64, f64::max);
        if gradient < 0.08 {
            return;
        }
        score += gradient * 6.0;
    }
    if best.is_none_or(|(previous, _)| score > previous) {
        *best = Some((score, point));
    }
}

fn capture(world: &mut World, scene: Scene, close: bool) -> Result<(), String> {
    let directory = std::env::var_os("TERRA_CAPTURE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::temp_dir().join("terra-biomes"));
    std::fs::create_dir_all(&directory).map_err(|error| error.to_string())?;
    let view = if close { "close" } else { "overview" };
    world
        .spawn(Screenshot::primary_window())
        .observe(save_to_disk(
            directory.join(format!("{}-{view}.png", scene.name())),
        ))
        .observe(|_: On<ScreenshotCaptured>, mut state: ResMut<BiomeSmoke>| {
            state.captures += 1;
        });
    Ok(())
}

fn run(world: &mut World) {
    world.resource_scope(|world, mut state: Mut<BiomeSmoke>| {
        if state.stage == 3 {
            return;
        }
        let render = world.resource::<RenderStatus>();
        let generation = world.resource::<WorldGeneration>();
        if state.failed
            || state.started.elapsed() > Duration::from_secs(150)
            || render.error.is_some()
            || matches!(generation.status, GenerationStatus::Failed(_))
        {
            error!(
                scene = state.scene.name(),
                stage = state.stage,
                "Native biome fixture failed"
            );
            world.write_message(AppExit::error());
            state.stage = 3;
            return;
        }
        if !render.ready
            || !world.resource::<WaterStatus>().ready
            || matches!(state.scene, Scene::Cherry | Scene::CherryBrook)
                && !world.resource::<terra_render::PropRenderStatus>().ready
            || generation.status != GenerationStatus::Ready
        {
            state.since = Instant::now();
            return;
        }
        if state.since.elapsed() < Duration::from_secs(3) {
            return;
        }
        let Some(views) = &state.views else {
            state.failed = true;
            return;
        };
        let chunks = world.resource::<ChunkStore>();
        if views
            .probes
            .iter()
            .any(|(point, material)| chunks.block_at(*point) != Some(*material))
        {
            state.failed = true;
            return;
        }
        if matches!(state.scene, Scene::Mushroom)
            && !chunks.loaded.values().any(|section| {
                section.data.blocks().iter().any(|&id| {
                    (MaterialId::PurpleMushroomCap as u16..=MaterialId::WhiteMushroomCap as u16)
                        .contains(&id)
                })
            })
        {
            state.failed = true;
            return;
        }
        if (matches!(state.scene, Scene::Oasis) || state.scene.water())
            && world.resource::<WaterStatus>().triangles == 0
        {
            state.failed = true;
            return;
        }
        if !state.requested {
            if let Err(error) = capture(world, state.scene, state.stage == 1) {
                error!(%error, "Biome capture failed");
                state.failed = true;
                return;
            }
            state.requested = true;
            return;
        }
        if state.captures <= state.stage {
            return;
        }
        if state.stage == 0 {
            let close = state.views.as_ref().map(|views| views.close);
            if let Some(close) = close {
                set_camera(world, close);
            }
            state.stage = 1;
            state.requested = false;
            state.since = Instant::now();
        } else {
            info!(
                scene = state.scene.name(),
                "Native biome fixture passed: generated feature uploaded and two views captured"
            );
            world.write_message(AppExit::Success);
            state.stage = 3;
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn water_palette_scenes_find_real_generated_water() {
        let terrain = TerraGenerator::new(terra_core::WorldConfig {
            rivers: true,
            ..default()
        })
        .unwrap();
        for scene in [
            Scene::WaterMushroom,
            Scene::WaterDesert,
            Scene::WaterHighland,
            Scene::WaterTransition,
        ] {
            let views = water_biome_views(&terrain, scene)
                .unwrap_or_else(|| panic!("no natural water fixture for {}", scene.name()));
            let column = terrain
                .water_column(f64::from(views.anchor.x), f64::from(views.anchor.z))
                .unwrap_or_else(|| panic!("selected fixture is dry: {}", scene.name()));
            assert!(column.depth >= 1.0);
            assert!(views.overview.translation.is_finite() && views.close.translation.is_finite());
            assert!((views.anchor.y - column.level as f32).abs() < 0.001);
            let weights = terrain
                .biome_at(f64::from(views.anchor.x), f64::from(views.anchor.z))
                .weights;
            match scene {
                Scene::WaterMushroom => assert!(weights.mushroom_park >= 0.75),
                Scene::WaterDesert => assert!(weights.desert + weights.cliffted_desert >= 0.75),
                Scene::WaterHighland => assert!(weights.highland >= 0.65),
                Scene::WaterTransition => assert!((0.2..=0.8).contains(
                    &(weights.mushroom_park
                        + weights.desert
                        + weights.cliffted_desert
                        + weights.highland)
                )),
                _ => unreachable!(),
            }
            println!(
                "{} anchor={:?} weights={:?} depth={:.3}",
                scene.name(),
                views.anchor,
                weights.values(),
                column.depth
            );
        }
    }
}
