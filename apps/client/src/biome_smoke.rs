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
    generation::{LandformKind, TreeAppearance, TreeAppearanceSnapshot},
    streaming::{
        ChunkFoci, ChunkFocus, ChunkStore, GenerationStatus, WorldGeneration, WorldTerrain,
    },
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
    Landscape(LandformKind),
    LandscapeLandmark,
    ConnectedLake,
    ConnectedRiver,
    ConnectedFir,
    KingdomTemperate,
    KingdomWinter,
    Outpost,
    Picker(terra_core::BiomeKind),
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
            Self::Landscape(LandformKind::Plains) => "landscape-plains",
            Self::Landscape(LandformKind::Hills) => "landscape-hills",
            Self::Landscape(LandformKind::Plateau) => "landscape-plateau",
            Self::Landscape(LandformKind::Ridges) => "landscape-ridges",
            Self::Landscape(LandformKind::Basin) => "landscape-basin",
            Self::LandscapeLandmark => "landscape-landmark",
            Self::ConnectedLake => "connected-lake",
            Self::ConnectedRiver => "connected-river",
            Self::ConnectedFir => "connected-fir",
            Self::KingdomTemperate => "kingdom-temperate",
            Self::KingdomWinter => "kingdom-winter",
            Self::Outpost => "outpost",
            Self::Picker(kind) => match kind {
                terra_core::BiomeKind::Ocean => "picker-ocean",
                terra_core::BiomeKind::Plains => "picker-plains",
                terra_core::BiomeKind::Forest => "picker-forest",
                terra_core::BiomeKind::Desert => "picker-desert",
                terra_core::BiomeKind::Highland => "picker-highlands",
                terra_core::BiomeKind::MushroomPark => "picker-mushroom-park",
                terra_core::BiomeKind::ClifftedDesert => "picker-cliffted-desert",
                terra_core::BiomeKind::CherryForest => "picker-cherry-forest",
            },
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
                | Self::ConnectedLake
                | Self::ConnectedRiver
                | Self::Picker(terra_core::BiomeKind::Ocean)
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
    last_progress: Instant,
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
        Ok("landscape-plains") => (Scene::Landscape(LandformKind::Plains), false),
        Ok("landscape-hills") => (Scene::Landscape(LandformKind::Hills), false),
        Ok("landscape-plateau") => (Scene::Landscape(LandformKind::Plateau), false),
        Ok("landscape-ridges") => (Scene::Landscape(LandformKind::Ridges), false),
        Ok("landscape-basin") => (Scene::Landscape(LandformKind::Basin), false),
        Ok("landscape-landmark") => (Scene::LandscapeLandmark, false),
        Ok("connected-lake") => (Scene::ConnectedLake, false),
        Ok("connected-river") => (Scene::ConnectedRiver, false),
        Ok("connected-fir") => (Scene::ConnectedFir, false),
        Ok("kingdom-temperate") => (Scene::KingdomTemperate, false),
        Ok("kingdom-winter") => (Scene::KingdomWinter, false),
        Ok("outpost") => (Scene::Outpost, false),
        Ok("picker-ocean") => (Scene::Picker(terra_core::BiomeKind::Ocean), false),
        Ok("picker-plains") => (Scene::Picker(terra_core::BiomeKind::Plains), false),
        Ok("picker-forest") => (Scene::Picker(terra_core::BiomeKind::Forest), false),
        Ok("picker-desert") => (Scene::Picker(terra_core::BiomeKind::Desert), false),
        Ok("picker-highlands") => (Scene::Picker(terra_core::BiomeKind::Highland), false),
        Ok("picker-mushroom-park") => (Scene::Picker(terra_core::BiomeKind::MushroomPark), false),
        Ok("picker-cliffted-desert") => {
            (Scene::Picker(terra_core::BiomeKind::ClifftedDesert), false)
        }
        Ok("picker-cherry-forest") => (Scene::Picker(terra_core::BiomeKind::CherryForest), false),
        _ => (Scene::Cave, true),
    };
    let now = Instant::now();
    app.insert_resource(ControllerMode::Fly)
        .insert_resource(BiomeSmoke {
            scene,
            views: None,
            started: now,
            since: now,
            last_progress: now,
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
        Scene::Picker(kind) => picker_views(&terrain, kind),
        Scene::Cave => cave_views(&terrain),
        Scene::Mushroom => mushroom_views(&terrain),
        Scene::Oasis => oasis_views(&terrain),
        Scene::Highland => highland_views(&terrain),
        Scene::Cherry => cherry_views(&terrain),
        Scene::CherryBrook => cherry_brook_views(&terrain),
        Scene::Landscape(kind) => landscape_views(&terrain, Some(kind)),
        Scene::LandscapeLandmark => landscape_views(&terrain, None),
        Scene::ConnectedLake | Scene::ConnectedRiver => connected_hydrology_views(&terrain, scene),
        Scene::ConnectedFir => connected_fir_views(&terrain),
        Scene::KingdomTemperate | Scene::KingdomWinter | Scene::Outpost => {
            stronghold_views(&terrain, scene)
        }
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
    if matches!(
        scene,
        Scene::KingdomTemperate | Scene::KingdomWinter | Scene::Outpost
    ) {
        // Stream around the real settlement, so a distant overview camera never
        // excludes the rear keep or the ground beneath its own high viewpoint.
        world.insert_resource(ChunkFoci(Some(vec![
            views.anchor.to_array().map(f64::from),
        ])));
    }
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

fn stronghold_views(terrain: &TerraGenerator, scene: Scene) -> Option<Views> {
    use terra_world::generation::structures::{REGION_SIZE, SettlementLayout, StructureKind};
    let layout = match scene {
        Scene::KingdomTemperate => SettlementLayout::KingdomTemperate,
        Scene::KingdomWinter => SettlementLayout::KingdomWinter,
        Scene::Outpost => SettlementLayout::Outpost,
        _ => return None,
    };
    if terrain.config().structures_revision < 6 {
        warn!("Stronghold native view needs structures_revision6");
        return None;
    }
    let explicit = std::env::var("TERRA_STRUCTURE_REGION")
        .ok()
        .and_then(|s| serde_json::from_str::<[i64; 2]>(&s).ok());
    let started = Instant::now();
    let mut attempts = 0;
    let mut candidates = Vec::new();
    if let Some(region) = explicit {
        candidates.push(region);
    } else {
        let kind = if matches!(scene, Scene::Outpost) {
            StructureKind::Outpost
        } else {
            StructureKind::Kingdom
        };
        // Visit at most64 rings, planning at most128 matching royal candidates.
        // Rare revision8 sites must not build thousands of unrelated villages.
        'rings: for ring in 0_i64..=64 {
            for z in -ring..=ring {
                for x in -ring..=ring {
                    if x.abs().max(z.abs()) != ring {
                        continue;
                    }
                    let region = [x, z];
                    if terrain.royal_site_candidate(region) == Some(kind) {
                        candidates.push(region);
                        if candidates.len() >= 128 {
                            break 'rings;
                        }
                    }
                }
            }
        }
    }
    for region in candidates {
        if region.iter().any(|v| v.unsigned_abs() > 2500) {
            continue;
        }
        if started.elapsed() > Duration::from_secs(30) {
            warn!(
                attempts,
                scene = scene.name(),
                "Stronghold search reached its time bound"
            );
            break;
        }
        attempts += 1;
        let x = (region[0] * REGION_SIZE + 192) as f64;
        let z = (region[1] * REGION_SIZE + 192) as f64;
        if explicit.is_none() && !matches!(scene, Scene::Outpost) {
            let biome = terrain.biome_at(x, z);
            let y = terrain.surface_height(x, z);
            let winter = biome.weights.highland > 0.18 || y >= terrain.config().snow_height - 12.0;
            if winter != matches!(scene, Scene::KingdomWinter) {
                continue;
            }
        }
        let Some(summary) = terrain
            .structure_site(region)
            .filter(|s| s.layout() == Some(layout))
        else {
            continue;
        };
        let settlement = terrain.village_settlement(region)?;
        let civilians = settlement
            .residents
            .iter()
            .filter(|s| s.role < terra_core::creatures::villagers::CIVILIAN_ROLE_COUNT)
            .count();
        let soldiers = settlement.residents.len() - civilians;
        if !matches!(scene, Scene::Outpost)
            && (!(50..=80).contains(&civilians) || !(26..=64).contains(&soldiers))
            || matches!(scene, Scene::Outpost) && !(4..=10).contains(&soldiers)
        {
            continue;
        }
        let base = summary
            .buildings
            .iter()
            .map(|b| b.entrance[1])
            .min()
            .unwrap_or(summary.center[1]);
        let center = Vec3::new(
            (summary.bounds[0][0] + summary.bounds[1][0]) as f32 * 0.5,
            base as f32 + 14.0,
            (summary.bounds[0][2] + summary.bounds[1][2]) as f32 * 0.5,
        );
        let (distance, height) = if matches!(scene, Scene::Outpost) {
            (73.0, 54.0)
        } else {
            (155.0, 112.0)
        };
        let overview = surface_eye(
            terrain,
            center + Vec3::new(distance * 0.8, height, distance),
        );
        let close_target = center
            + Vec3::new(
                0.0,
                -6.0,
                if matches!(scene, Scene::Outpost) {
                    19.0
                } else {
                    43.0
                },
            );
        let close = surface_eye(
            terrain,
            close_target
                + if matches!(scene, Scene::Outpost) {
                    Vec3::new(0.0, 24.0, 48.0)
                } else {
                    Vec3::new(48.0, 40.0, 77.0)
                },
        );
        let generator = SectionGenerator::new(terrain.clone());
        if [overview, close]
            .into_iter()
            .any(|p| generator.block_at(p.floor().as_i64vec3().to_array()).ok() != Some(0))
        {
            continue;
        }
        let probes = settlement
            .residents
            .iter()
            .step_by((settlement.residents.len() / 4).max(1))
            .take(4)
            .filter_map(|s| generator.block_at(s.bed).ok().map(|m| (s.bed, m)))
            .collect::<Vec<_>>();
        if probes.len() < 4 || probes.iter().any(|(_, m)| !terra_core::beds::is_bed(*m)) {
            continue;
        }
        info!(scene=scene.name(),?region,seed=terrain.config().seed.0,?layout,civilians,soldiers,buildings=summary.buildings.len(),bounds=?summary.bounds,attempts,"Native stronghold selected actual authored structure and verified occupied-bed metadata");
        return Some(Views {
            overview: Transform::from_translation(overview).looking_at(center, Vec3::Y),
            close: Transform::from_translation(close).looking_at(close_target, Vec3::Y),
            anchor: center,
            probes,
        });
    }
    warn!(
        scene = scene.name(),
        attempts, "No matching generated stronghold found in bounded regional search"
    );
    None
}

fn connected_fir_views(terrain: &TerraGenerator) -> Option<Views> {
    if terrain.config().landscape_revision < 4
        || !terrain.config().trees
        || terrain.config().tree_density == 0.0
    {
        warn!("Connected fir view requires a world with naturally generated fir trees");
        return None;
    }
    let generator = SectionGenerator::new(terrain.clone());
    let started = Instant::now();
    let mut columns = 0;
    let mut classified_leaves = 0;
    let mut verified_leaves = 0;
    // Three 7x7 neighborhoods: at most 147 section-columns, never a global scan.
    // The first anchor contains a verified natural fir in current seed 42042.
    for anchor in [[4486_i64, -5756_i64], [4352, 256], [3840, -5375]] {
        let center = anchor.map(|value| value.div_euclid(16));
        for ring in 0_i64..=3 {
            for dz in -ring..=ring {
                for dx in -ring..=ring {
                    if dx.abs().max(dz.abs()) != ring {
                        continue;
                    }
                    columns += 1;
                    let coordinate = [center[0] + dx, center[1] + dz];
                    let height = terrain.surface_height(
                        (coordinate[0] * 16 + 8) as f64,
                        (coordinate[1] * 16 + 8) as f64,
                    );
                    let bottom = (height.floor() as i64).div_euclid(16) - 1;
                    for y in bottom..=bottom + 4 {
                        if started.elapsed() > Duration::from_secs(25) {
                            warn!(
                                seed = terrain.config().seed.0,
                                columns,
                                classified_leaves,
                                verified_leaves,
                                "Connected fir search timed out without a verified natural view"
                            );
                            return None;
                        }
                        let origin = [coordinate[0] * 16, y * 16, coordinate[1] * 16];
                        let appearance = terrain.tree_appearance(origin);
                        for z in 0..16 {
                            for x in 0..16 {
                                for local_y in 0..16 {
                                    let leaf = [origin[0] + x, origin[1] + local_y, origin[2] + z];
                                    if appearance.at(leaf, 7) != Some(TreeAppearance::Fir) {
                                        continue;
                                    }
                                    classified_leaves += 1;
                                    if generator.block_at(leaf).ok()? != 7 {
                                        continue;
                                    }
                                    verified_leaves += 1;
                                    if let Some(views) = fir_views_at(
                                        terrain,
                                        &generator,
                                        &appearance,
                                        leaf,
                                        columns,
                                    ) {
                                        return Some(views);
                                    }
                                    if started.elapsed() > Duration::from_secs(25) {
                                        warn!(
                                            seed = terrain.config().seed.0,
                                            columns,
                                            classified_leaves,
                                            verified_leaves,
                                            "Connected fir search timed out without a clear camera"
                                        );
                                        return None;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    warn!(
        seed = terrain.config().seed.0,
        columns,
        classified_leaves,
        verified_leaves,
        "Bounded connected fir search found no verified natural view"
    );
    None
}

fn fir_views_at(
    terrain: &TerraGenerator,
    generator: &SectionGenerator,
    appearance: &TreeAppearanceSnapshot,
    leaf: [i64; 3],
    columns: usize,
) -> Option<Views> {
    let target = Vec3::from_array(leaf.map(|value| value as f32 + 0.5));
    let close = [6.0, 10.0, 14.0].into_iter().find_map(|radius| {
        (0..8).find_map(|index| {
            let angle = index as f32 * std::f32::consts::FRAC_PI_4;
            let eye = surface_eye(
                terrain,
                target + Vec3::new(angle.sin() * radius, 1.5, angle.cos() * radius),
            );
            (generator.block_at(eye.floor().as_i64vec3().to_array()).ok() == Some(0)
                && (1..=8).all(|step| {
                    let point = eye.lerp(target, step as f32 * 0.1);
                    generator
                        .block_at(point.floor().as_i64vec3().to_array())
                        .ok()
                        == Some(0)
                }))
            .then_some(eye)
        })
    })?;
    let material = MaterialId::SpruceLog as u16;
    let trunk = (0_i64..=7).find_map(|ring| {
        (-ring..=ring).find_map(|dz| {
            (-ring..=ring).find_map(|dx| {
                if dx.abs().max(dz.abs()) != ring {
                    return None;
                }
                let point = [leaf[0] + dx, leaf[1], leaf[2] + dz];
                (appearance.at(point, material) == Some(TreeAppearance::Fir)
                    && generator.block_at(point).ok() == Some(material))
                .then_some(point)
            })
        })
    })?;
    let mut bottom = trunk[1];
    let mut top = trunk[1];
    for direction in [-1, 1] {
        for distance in 1..=40 {
            let point = [trunk[0], trunk[1] + direction * distance, trunk[2]];
            if appearance.at(point, material) != Some(TreeAppearance::Fir)
                || generator.block_at(point).ok()? != material
            {
                break;
            }
            if direction < 0 {
                bottom = point[1];
            } else {
                top = point[1];
            }
        }
    }
    let trunk_height = top - bottom + 1;
    if trunk_height < 14 {
        return None;
    }
    let center = Vec3::new(
        trunk[0] as f32 + 0.5,
        (bottom + top) as f32 * 0.5 + 2.0,
        trunk[2] as f32 + 0.5,
    );
    let overview = (0..8).find_map(|index| {
        let angle = index as f32 * std::f32::consts::FRAC_PI_4;
        let eye = surface_eye(
            terrain,
            center + Vec3::new(angle.sin() * 40.0, 18.0, angle.cos() * 40.0),
        );
        (generator.block_at(eye.floor().as_i64vec3().to_array()).ok() == Some(0)).then_some(eye)
    })?;
    info!(
        seed = terrain.config().seed.0,
        columns,
        ?leaf,
        ?trunk,
        trunk_bottom = bottom,
        trunk_top = top,
        trunk_height,
        "Connected fir fixture selected an actual natural fir leaf and measured its generated trunk"
    );
    Some(Views {
        overview: Transform::from_translation(overview).looking_at(center, Vec3::Y),
        close: Transform::from_translation(close).looking_at(target, Vec3::Y),
        anchor: center,
        probes: vec![(leaf, 7), (trunk, material)],
    })
}

fn connected_hydrology_views(terrain: &TerraGenerator, scene: Scene) -> Option<Views> {
    let anchor = std::env::var("TERRA_HYDRO_ANCHOR").ok()?;
    let [x, z] = serde_json::from_str::<[f64; 2]>(&anchor).ok()?;
    if !x.is_finite() || !z.is_finite() || terrain.config().landscape_revision < 3 {
        return None;
    }
    let water = terrain.water_column(x, z)?;
    let radius = match scene {
        Scene::ConnectedLake => {
            let lake = terrain.lake_at(x, z)?;
            if lake.distance > lake.radius || water.depth < 1.0 {
                return None;
            }
            lake.radius as f32
        }
        Scene::ConnectedRiver => {
            let river = terrain.river_at(x, z)?;
            if river.distance > river.width + 1.0
                || river.covered
                || water.level <= terrain.config().sea_level + 0.5
            {
                return None;
            }
            64.0
        }
        _ => return None,
    };
    let point = Vec3::new(x as f32, water.level as f32, z as f32);
    let overview = surface_eye(
        terrain,
        point + Vec3::new(radius * 1.15, radius * 0.9 + 30.0, radius * 0.85),
    );
    let close = surface_eye(terrain, point + Vec3::new(23.0, 20.0, 27.0));
    let generator = SectionGenerator::new(terrain.clone());
    let bed = [
        x.floor() as i64,
        water.bed.floor() as i64 - 1,
        z.floor() as i64,
    ];
    let air = [bed[0], water.level.ceil() as i64, bed[2]];
    let material = generator.block_at(bed).ok()?;
    if material == 0 || generator.block_at(air).ok()? != 0 {
        return None;
    }
    info!(
        seed = terrain.config().seed.0,
        scene = scene.name(),
        x,
        z,
        radius,
        level = water.level,
        depth = water.depth,
        "Connected hydrology fixture selected actual generated water and solid bed"
    );
    Some(Views {
        overview: Transform::from_translation(overview).looking_at(point, Vec3::Y),
        close: Transform::from_translation(close).looking_at(point, Vec3::Y),
        anchor: point,
        probes: vec![(bed, material), (air, 0)],
    })
}

fn landscape_views(terrain: &TerraGenerator, kind: Option<LandformKind>) -> Option<Views> {
    let explicit = match std::env::var("TERRA_LANDSCAPE_ANCHOR") {
        Ok(value) => Some(serde_json::from_str::<[f64; 2]>(&value).ok()?),
        Err(std::env::VarError::NotPresent) => None,
        Err(_) => return None,
    };
    landscape_views_selected(terrain, kind, explicit)
}

fn landscape_views_selected(
    terrain: &TerraGenerator,
    kind: Option<LandformKind>,
    explicit: Option<[f64; 2]>,
) -> Option<Views> {
    let score = |[x, z]: [f64; 2]| -> Option<f64> {
        if !x.is_finite() || !z.is_finite() {
            return None;
        }
        let region = terrain.landscape_region(x, z);
        let strength = if let Some(kind) = kind {
            if region.dominant() != kind {
                return None;
            }
            region.weights[kind as usize]
        } else {
            region.landmark
        };
        if strength < 0.65 {
            return None;
        }
        let biome = terrain.biome_at(x, z);
        let height = terrain.surface_height(x, z);
        if biome.weights.ocean > 0.3 || height < terrain.config().sea_level + 4.0 {
            return None;
        }
        let mut low = height;
        let mut high = height;
        for [dx, dz] in [[-48.0, 0.0], [48.0, 0.0], [0.0, -48.0], [0.0, 48.0]] {
            let h = terrain.surface_height(x + dx, z + dz);
            low = low.min(h);
            high = high.max(h);
        }
        let relief = high - low;
        let visible_relief = match kind {
            Some(LandformKind::Plains | LandformKind::Basin) => 1.0 / (1.0 + relief),
            _ => relief.min(40.0) / 40.0,
        };
        Some(strength + visible_relief * 0.2 - biome.weights.highland * 0.05)
    };
    let anchor = if let Some(anchor) = explicit {
        score(anchor)?;
        anchor
    } else {
        let mut best = None;
        for z in (-4096..=4096).step_by(128) {
            for x in (-4096..=4096).step_by(128) {
                let p = [f64::from(x) + 0.5, f64::from(z) + 0.5];
                if let Some(value) = score(p)
                    && best.is_none_or(|(old, _)| value > old)
                {
                    best = Some((value, p));
                }
            }
        }
        best?.1
    };
    let [x, z] = anchor;
    let height = terrain.surface_height(x, z);
    let point = Vec3::new(x as f32, height as f32, z as f32);
    let overview = landscape_camera(terrain, point, 90.0, 48.0);
    let close = landscape_camera(terrain, point, 34.0, 24.0);
    let floor = [
        x.floor() as i64,
        (height - 0.5).floor() as i64,
        z.floor() as i64,
    ];
    let air = [floor[0], floor[1] + 1, floor[2]];
    let generator = SectionGenerator::new(terrain.clone());
    let material = generator.block_at(floor).ok()?;
    if material == 0 || generator.block_at(air).ok()? != 0 {
        return None;
    }
    info!(
        seed = terrain.config().seed.0,
        revision = terrain.config().landscape_revision,
        ?kind,
        ?anchor,
        height,
        region = ?terrain.landscape_region(x, z),
        "Native landscape view selected real generated relief without edits"
    );
    Some(Views {
        overview,
        close,
        anchor: point,
        probes: vec![(floor, material), (air, 0)],
    })
}

fn landscape_camera(terrain: &TerraGenerator, point: Vec3, radius: f32, height: f32) -> Transform {
    let target = point + Vec3::Y * 3.0;
    for step in 0..8 {
        let angle = (step as f32 + 3.0) * std::f32::consts::FRAC_PI_4;
        let eye = point + Vec3::new(angle.cos() * radius, height, angle.sin() * radius);
        let clear = (0..20).all(|i| {
            let p = eye.lerp(target, i as f32 / 20.0);
            terrain.surface_height(f64::from(p.x), f64::from(p.z)) + 1.0 < f64::from(p.y)
        });
        if clear {
            return Transform::from_translation(eye).looking_at(target, Vec3::Y);
        }
    }
    // An enclosed depression has no guaranteed lateral sightline. Keep its
    // real surface in the loaded vertical span and inspect it from overhead.
    Transform::from_translation(point + Vec3::Y * height).looking_at(target, Vec3::Z)
}

/// Static chooser photography: bounded deterministic selection from the real
/// generator. No terrain edits, forced flat biome or fabricated backdrop.
fn picker_views(terrain: &TerraGenerator, kind: terra_core::BiomeKind) -> Option<Views> {
    use terra_core::BiomeKind;
    let index = BiomeKind::ALL.into_iter().position(|k| k == kind)?;
    let generator = SectionGenerator::new(terrain.clone());
    let mut selected = None;
    'search: for ring in 0_i64..=64 {
        for n in -ring..=ring {
            for [gx, gz] in [[n, -ring], [n, ring], [-ring, n], [ring, n]] {
                let [x, z] = [gx as f64 * 128.0 + 0.5, gz as f64 * 128.0 + 0.5];
                let biome = terrain.biome_at(x, z);
                if biome.weights.values()[index] < 0.72 {
                    continue;
                }
                let column = terrain.surface_column(x, z);
                if column.biome_kind(terrain.config().sea_level) != kind {
                    continue;
                }
                if kind != BiomeKind::Ocean
                    && (column.water_level.is_some()
                        || column.height < terrain.config().sea_level + 2.0)
                {
                    continue;
                }
                // Prefer a coherent panorama, not a narrow biome transition.
                if [[-48.0, 0.0], [48.0, 0.0], [0.0, -48.0], [0.0, 48.0]]
                    .into_iter()
                    .any(|[dx, dz]| terrain.biome_at(x + dx, z + dz).weights.values()[index] < 0.50)
                {
                    continue;
                }
                if matches!(kind, BiomeKind::Highland | BiomeKind::ClifftedDesert) {
                    let heights = [[-48.0, 0.0], [48.0, 0.0], [0.0, -48.0], [0.0, 48.0]]
                        .map(|[dx, dz]| terrain.surface_height(x + dx, z + dz));
                    let low = heights.into_iter().fold(column.height, f64::min);
                    let high = heights.into_iter().fold(column.height, f64::max);
                    if high - low < 8.0 {
                        continue;
                    }
                }
                if kind == BiomeKind::Forest {
                    let heights = [[-40.0, 0.0], [40.0, 0.0], [0.0, -40.0], [0.0, 40.0]]
                        .map(|[dx, dz]| terrain.surface_height(x + dx, z + dz));
                    let low = heights.into_iter().fold(column.height, f64::min);
                    let high = heights.into_iter().fold(column.height, f64::max);
                    if column.slope > 0.6 || high - low > 24.0 {
                        continue;
                    }
                    let roots = terrain.tree_appearance([
                        (x.floor() as i64).div_euclid(16) * 16,
                        0,
                        (z.floor() as i64).div_euclid(16) * 16,
                    ]);
                    let woodland = roots
                        .tree_roots()
                        .filter(|(root, appearance)| {
                            !matches!(appearance, TreeAppearance::Mushroom | TreeAppearance::Palm)
                                && (root[0] as f64 - x).abs() <= 24.0
                                && (root[2] as f64 - z).abs() <= 24.0
                        })
                        .take(4)
                        .count();
                    if woodland < 4 {
                        continue;
                    }
                }
                let visible = column
                    .water_level
                    .unwrap_or(column.height)
                    .max(column.height);
                // Fluids live separately from solid blocks. An ocean's surface
                // is Air in block_at; validate its real bed and open surface.
                let floor = if kind == BiomeKind::Ocean {
                    let Some(water) = terrain.water_column(x, z) else {
                        continue;
                    };
                    if water.depth < 3.0
                        || (water.level - terrain.config().sea_level).abs() > 0.01
                        || column.ocean.ice_depth > 0
                    {
                        continue;
                    }
                    water.bed as i64 - 1
                } else {
                    (visible - 0.5).floor() as i64
                };
                let probe = [x.floor() as i64, floor, z.floor() as i64];
                let air = [probe[0], visible.ceil() as i64, probe[2]];
                let Ok(material) = generator.block_at(probe) else {
                    continue;
                };
                if material == 0 || generator.block_at(air).ok() != Some(0) {
                    continue;
                }
                selected = Some(([x, z], visible, vec![(probe, material), (air, 0)]));
                break 'search;
            }
        }
    }
    let ([x, z], visible, probes) = selected?;
    let point = Vec3::new(x as f32, visible as f32, z as f32);
    let (radius, height) = match kind {
        BiomeKind::Highland | BiomeKind::ClifftedDesert => (80.0, 42.0),
        BiomeKind::Forest | BiomeKind::CherryForest | BiomeKind::MushroomPark => (62.0, 30.0),
        BiomeKind::Ocean => (72.0, 24.0),
        _ => (74.0, 28.0),
    };
    let photo_target = if kind == BiomeKind::Forest {
        point + Vec3::Y * 8.0
    } else {
        point
    };
    let overview = landscape_camera(terrain, photo_target, radius, height);
    info!(
        ?kind,
        seed = terrain.config().seed.0,
        ?probes,
        ?overview,
        "Native biome chooser photo uses actual generated terrain"
    );
    Some(Views {
        overview,
        close: overview,
        anchor: point,
        probes,
    })
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
        if state.stage == 3 { return; }
        if matches!(state.scene,Scene::Picker(_)) {
            if let Some(mut hud)=world.get_resource_mut::<terra_ui::HudVisibility>() {hud.visible=false;}
            // UI source photography uses the actual renderer, with no editor
            // gizmos or HUD layered over its pixels. This is an opt-in fixture.
            for mut projection in world.query_filtered::<&mut Projection,With<TerraCamera>>().iter_mut(world) {
                if let Projection::Perspective(p)=&mut *projection {p.far=p.far.max(512.0);}
            }
        }
        if matches!(
            state.scene,
            Scene::KingdomTemperate | Scene::KingdomWinter | Scene::Outpost
        ) {
            // The overview sits outside the streaming focus. Keep its complete
            // real settlement inside the camera frustum without loading sky chunks.
            for mut projection in world
                .query_filtered::<&mut Projection, With<TerraCamera>>()
                .iter_mut(world)
            {
                if let Projection::Perspective(p) = &mut *projection {
                    p.far = p.far.max(512.0);
                }
            }
        }
        let render = world.resource::<RenderStatus>();
        let generation = world.resource::<WorldGeneration>();
        if state.last_progress.elapsed() >= Duration::from_secs(10) {
            let water = world.resource::<WaterStatus>();
            let props = world.resource::<terra_render::PropRenderStatus>();
            info!(
                scene = state.scene.name(), stage = state.stage,
                focus = ?world.resource::<ChunkFocus>().0,
                generation = ?generation.status,
                loaded = generation.loaded_sections, wanted = generation.wanted_sections,
                generation_jobs = generation.pending_jobs,
                terrain_ready = render.ready, terrain_jobs = render.pending_jobs,
                terrain_columns = render.pending_columns.len(), terrain_error = ?render.error,
                water_ready = water.ready, water_initial_ready = water.initial_ready,
                props_ready = props.ready, prop_jobs = props.pending_jobs,
                prop_error = ?props.error,
                "Native biome fixture loading progress"
            );
            state.last_progress = Instant::now();
        }
        if state.failed
            || state.started.elapsed()
                > Duration::from_secs(
                    if matches!(
                        state.scene,
                        Scene::KingdomTemperate | Scene::KingdomWinter | Scene::Outpost
                    ) {
                        240
                    } else {
                        150
                    },
                )
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
            || matches!(
                state.scene,
                Scene::Cherry
                    | Scene::CherryBrook
                    | Scene::Landscape(_)
                    | Scene::LandscapeLandmark
                    | Scene::ConnectedLake
                    | Scene::ConnectedRiver
                    | Scene::ConnectedFir
                    | Scene::KingdomTemperate
                    | Scene::KingdomWinter
                    | Scene::Outpost
                    | Scene::Picker(_)
            ) && !world.resource::<terra_render::PropRenderStatus>().ready
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
        if matches!(state.scene,Scene::Picker(_)) {
            info!(scene=state.scene.name(),"Native biome preview passed: genuine generated biome, no HUD, full-resolution capture");
            world.write_message(AppExit::Success);state.stage=3;return;
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

    fn modern_world() -> terra_core::WorldConfig {
        terra_core::WorldConfig {
            landscape_revision: 5,
            sea_level: 22.0,
            rivers: true,
            biome_definitions: Some(terra_core::BiomeDefinitions::default()),
            ..default()
        }
    }

    #[test]
    fn connected_fir_view_rejects_worlds_without_natural_firs() {
        assert_eq!(Scene::ConnectedFir.name(), "connected-fir");
        assert!(!Scene::ConnectedFir.water());
        for config in [
            terra_core::WorldConfig {
                trees: false,
                ..modern_world()
            },
            terra_core::WorldConfig {
                tree_density: 0.0,
                ..modern_world()
            },
            terra_core::WorldConfig {
                landscape_revision: 3,
                ..modern_world()
            },
        ] {
            let terrain = TerraGenerator::new(config).unwrap();
            assert!(connected_fir_views(&terrain).is_none());
        }
    }

    #[test]
    fn connected_fir_view_selects_actual_fir_leaves_and_clear_air_cameras() {
        let terrain = TerraGenerator::new(modern_world()).unwrap();
        let generator = SectionGenerator::new(terrain.clone());
        let views =
            connected_fir_views(&terrain).expect("natural fir fixture in current seed 42042");
        assert_eq!(views.probes[0].1, 7);
        assert_eq!(views.probes[1].1, MaterialId::SpruceLog as u16);
        for (point, material) in views.probes {
            assert_eq!(generator.block_at(point).unwrap(), material);
            let origin = point.map(|value| value.div_euclid(16) * 16);
            assert_eq!(
                terrain.tree_appearance(origin).at(point, material),
                Some(TreeAppearance::Fir)
            );
        }
        for view in [views.overview, views.close] {
            assert!(view.translation.is_finite());
            assert!(view.rotation.is_finite());
            assert_eq!(
                generator
                    .block_at(view.translation.floor().as_i64vec3().to_array())
                    .unwrap(),
                0
            );
        }
    }

    #[test]
    fn landscape_gallery_selects_real_surface_and_finite_views_for_each_form() {
        let terrain = TerraGenerator::new(modern_world()).unwrap();
        let generator = SectionGenerator::new(terrain.clone());
        for kind in [
            Some(LandformKind::Plains),
            Some(LandformKind::Hills),
            Some(LandformKind::Plateau),
            Some(LandformKind::Ridges),
            Some(LandformKind::Basin),
            None,
        ] {
            let views = landscape_views_selected(&terrain, kind, None)
                .unwrap_or_else(|| panic!("missing generated landscape view for {kind:?}"));
            assert!(views.overview.translation.is_finite());
            assert!(views.close.translation.is_finite());
            assert!(views.anchor.y > terrain.config().sea_level as f32);
            for (position, material) in views.probes {
                assert_eq!(generator.block_at(position).unwrap(), material);
            }
        }
        assert!(landscape_views_selected(&terrain, None, Some([f64::NAN, 0.0])).is_none());
    }

    #[test]
    fn ocean_picker_validates_real_water_bed_and_open_surface() {
        let terrain = TerraGenerator::new(terra_core::WorldConfig::default()).unwrap();
        let views = picker_views(&terrain, terra_core::BiomeKind::Ocean)
            .expect("fresh world has a genuine ocean photo candidate");
        let water = terrain
            .water_column(f64::from(views.anchor.x), f64::from(views.anchor.z))
            .expect("photo anchor must contain generated water");
        assert!(water.depth >= 3.0);
        assert_eq!(f64::from(views.anchor.y), water.level);
        assert_eq!(water.level, terrain.config().sea_level);
        assert!(views.probes[0].1 != 0 && views.probes[1].1 == 0);
        let generator = SectionGenerator::new(terrain);
        for (position, material) in views.probes {
            assert_eq!(generator.block_at(position).unwrap(), material);
        }
    }

    #[test]
    fn water_palette_scenes_find_real_generated_water() {
        let terrain = TerraGenerator::new(terra_core::WorldConfig {
            // Match bootstrap's fixed pre-landscape water-palette captures.
            landscape_revision: 0,
            ecology_revision: 0,
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
