use bevy::{
    diagnostic::{DiagnosticsStore, FrameTimeDiagnosticsPlugin},
    prelude::*,
};
use terra_player::{TargetWorld, TerraCamera, WorldTarget};
use terra_voxel::global_to_section;
use tilt_ui::html_shared;

#[html_shared]
#[derive(Resource, Default, serde::Serialize)]
pub struct DebugOverlay {
    pub visible: bool,
    pub build: String,
    pub frame_ms: String,
    pub facing: String,
    pub block_position: String,
    pub local_position: String,
    pub world: String,
    pub world_time: String,
    pub weather: String,
    pub seed: String,
    pub system: String,
    pub gpu: String,
    pub display: String,
    pub render_sections: String,
    pub streaming: String,
    pub network: String,
    pub entities: String,
    pub target_identifier: String,
    pub target_position: String,

    pub fps: String,
    pub fps_min: String,
    pub fps_max: String,
    pub cpu: String,
    pub ram: String,
    pub chunk: String,
    pub xyz: String,
    pub block_name: String,
    pub biome: String,
    pub cave_visible: bool,
    pub cave_biome: String,
    pub cave_region: String,
    pub cave_conditions: String,
    pub cave_boundary: String,
    pub cave_connector: String,
    pub cave_local: String,
    pub cave_fluid: String,
    pub animal_visible: bool,
    pub animal_name: String,
    pub animal_health: String,
    pub animal_food: String,
    pub animal_water: String,
    pub animal_behavior: String,
    pub animal_intention: String,
    pub animal_destination: String,
    pub animal_path: String,
}

// The optional hierarchy components keep offline player positions in world space.
#[allow(clippy::type_complexity)]
pub fn update_debug_overlay(
    time: Res<Time>,
    mut state: OverlayState,
    camera: Query<(&Transform, Option<&terra_player::CameraLook>), With<TerraCamera>>,
    windows: Query<&Window, With<bevy::window::PrimaryWindow>>,
    player: Query<
        (&Transform, Option<&GlobalTransform>, Option<&ChildOf>),
        With<terra_player::PlayerBody>,
    >,
    mut overlay: ResMut<DebugOverlay>,
    mut last_update: Local<f64>,
) {
    let allowed = state
        .menu
        .as_ref()
        .is_none_or(|menu| menu.world_hud_active());
    let visible = state.debug.enabled && allowed;
    let animal = state
        .creature_target
        .as_ref()
        .and_then(|target| target.0)
        .and_then(|id| state.creatures.as_ref()?.creatures.get(&id))
        .map(|track| &track.current);
    let animal_visible = allowed && state.debug.creature_inspection && animal.is_some();
    if let Some(inspection) = state.inspection.as_deref_mut() {
        inspection.0 = animal.filter(|_| animal_visible).map(|animal| animal.id);
    }
    if overlay.animal_visible != animal_visible {
        overlay.animal_visible = animal_visible;
    }
    if overlay.visible != visible {
        overlay.visible = visible;
    }
    if !visible && !overlay.animal_visible {
        return;
    }
    // Avoid formatting strings and invalidating HTML bindings on every render frame.
    if time.elapsed_secs_f64() - *last_update < 0.25 {
        return;
    }
    *last_update = time.elapsed_secs_f64();
    if let Some(animal) = animal.filter(|_| overlay.animal_visible) {
        overlay.animal_name = terra_core::creatures::definitions::get(animal.kind)
            .variant_title(animal.variant)
            .into();
        overlay.animal_health = format!("{:.1} / {:.0}", animal.health, animal.kind.health());
        overlay.animal_food = if animal.kind.food() == 0.0 {
            "--".into()
        } else {
            format!("{:.1} / {:.0}", animal.needs.food, animal.kind.food())
        };
        overlay.animal_water = if animal.kind.water() == 0.0 {
            "--".into()
        } else {
            format!("{:.1} / {:.0}", animal.needs.water, animal.kind.water())
        };
        overlay.animal_behavior = match animal.behavior {
            terra_core::creatures::CreatureBehavior::Rest => "Ruht",
            terra_core::creatures::CreatureBehavior::Wander => "Laeuft",
            terra_core::creatures::CreatureBehavior::Graze => "Frisst",
            terra_core::creatures::CreatureBehavior::Flee => "Flieht",
            terra_core::creatures::CreatureBehavior::Swim => "Schwimmt",
            terra_core::creatures::CreatureBehavior::Stranded => "Gestrandet",
            terra_core::creatures::CreatureBehavior::Dead => "Tot",
            terra_core::creatures::CreatureBehavior::SeekFood => "Sucht Futter",
            terra_core::creatures::CreatureBehavior::SeekWater => "Sucht Wasser",
            terra_core::creatures::CreatureBehavior::Drink => "Trinkt",
            terra_core::creatures::CreatureBehavior::Shake => "Schuettelt sich",
            terra_core::creatures::CreatureBehavior::Fly => "Fliegt",
            terra_core::creatures::CreatureBehavior::Land => "Landet",
            terra_core::creatures::CreatureBehavior::Nectar => "Sammelt Nektar",
            terra_core::creatures::CreatureBehavior::Crawl => "Krabbelt",
            terra_core::creatures::CreatureBehavior::Sleep => "Schlaeft",
            terra_core::creatures::CreatureBehavior::InNest => "Im Nest",
            terra_core::creatures::CreatureBehavior::Leap => "Springt",
            terra_core::creatures::CreatureBehavior::Hunt if animal.kind.is_spider() => {
                "Verfolgt Ziel"
            }
            terra_core::creatures::CreatureBehavior::Hunt => "Jagt Fische",
            terra_core::creatures::CreatureBehavior::Attack => "Greift an",
            terra_core::creatures::CreatureBehavior::Climb => "Klettert",
            terra_core::creatures::CreatureBehavior::Hold => "Haelt sich fest",
            terra_core::creatures::CreatureBehavior::Ink => "Versprueht Tinte",
            terra_core::creatures::CreatureBehavior::Ceiling => "An der Decke",
            terra_core::creatures::CreatureBehavior::Descend => "Seilt sich ab",
            terra_core::creatures::CreatureBehavior::Burrowed => "Unter dem Sand",
            terra_core::creatures::CreatureBehavior::Tremor => "Bodenbeben / Angriffswarnung",
            terra_core::creatures::CreatureBehavior::Emerge => "Taucht auf",
            terra_core::creatures::CreatureBehavior::Retreat => "Gräbt sich ein",
            terra_core::creatures::CreatureBehavior::DragonVolley => "Wirft eine Salve",
            terra_core::creatures::CreatureBehavior::DragonWard => "Baut einen Schild auf",
            terra_core::creatures::CreatureBehavior::DragonSurge => "Bündelt Elementarkraft",
            terra_core::creatures::CreatureBehavior::Spit if animal.kind.is_dragon() => {
                "Speit Elementaratem"
            }
            terra_core::creatures::CreatureBehavior::DragonBurrow => "Gräbt sich ein",
            terra_core::creatures::CreatureBehavior::Spit => "Speit Galle",
        }
        .into();
        let (intention, destination, path) = inspection_labels(
            state
                .creatures
                .as_ref()
                .and_then(|creatures| creatures.intentions.get(&animal.id)),
        );
        overlay.animal_intention = intention;
        overlay.animal_destination = destination;
        overlay.animal_path = path;
        if let Some(traits) = animal.horse_traits() {
            overlay.animal_behavior = format!(
                "{} / Ausdauer {} / Sprung {:.2} m",
                overlay.animal_behavior, traits.stamina, traits.jump_height
            );
        }
        if let Some(bee) = state
            .creatures
            .as_ref()
            .and_then(|c| c.ecology.bees.get(&animal.id))
        {
            overlay.animal_food = format!("Nektar {} / 100", bee.carried);
            overlay.animal_water = bee
                .flower
                .and_then(|id| {
                    state
                        .creatures
                        .as_ref()
                        .map(|c| format!("Bluete {} / 100", c.ecology.nectar(id)))
                })
                .unwrap_or_else(|| "Keine Bluete".into());
        }
    }
    overlay.block_name = match state.target.0 {
        Some(WorldTarget::Block(hit)) => state
            .chunks
            .block_at(hit.position)
            .map(|material| {
                state
                    .palette
                    .choices
                    .iter()
                    .find(|block| block.id == material)
                    .map(|block| block.name.clone())
                    .unwrap_or_else(|| format!("Block {material}"))
            })
            .unwrap_or_else(|| "--".into()),
        Some(WorldTarget::Prop(hit)) => match hit.prop.kind {
            terra_world::props::PropKind::Grass => "Grasbueschel",
            terra_world::props::PropKind::Flower => "Wildblumen",
            terra_world::props::PropKind::Fern => "Farn",
            terra_world::props::PropKind::Bush => "Busch",
            terra_world::props::PropKind::Mushroom => "Pilz",
            terra_world::props::PropKind::SmallPlant if hit.prop.variant >= 128 => "Kleiner Stein",
            terra_world::props::PropKind::SmallPlant => "Uferpflanze",
            terra_world::props::PropKind::BeeNest => "Bienennest",
            kind => terra_core::ResourcePropKind::from(kind).label(),
        }
        .into(),
        None => "--".into(),
    };
    overlay.build = format!("TERAMIC {}", env!("CARGO_PKG_VERSION"));
    overlay.frame_ms = state
        .diagnostics
        .get(&FrameTimeDiagnosticsPlugin::FRAME_TIME)
        .and_then(|diagnostic| diagnostic.smoothed())
        .map_or_else(
            || "--".into(),
            |milliseconds| format!("{milliseconds:.2} ms"),
        );
    overlay.system = format!(
        "{} / {} / {} logical threads",
        std::env::consts::OS,
        std::env::consts::ARCH,
        std::thread::available_parallelism()
            .map_or_else(|_| "--".into(), |threads| threads.to_string())
    );
    overlay.gpu = state.adapter.as_ref().map_or_else(
        || "--".into(),
        |adapter| format!("{} / {:?}", adapter.name, adapter.backend),
    );
    overlay.display = windows.iter().next().map_or_else(
        || "--".into(),
        |window| {
            format!(
                "{} x {} / {:?}",
                window.physical_width(),
                window.physical_height(),
                window.present_mode
            )
        },
    );
    overlay.world = state
        .info
        .as_ref()
        .map(|info| info.title.as_str())
        .filter(|title| !title.is_empty())
        .unwrap_or("--")
        .into();
    overlay.world_time = state.clock.as_ref().map_or_else(
        || "--".into(),
        |clock| {
            let minutes = (clock.hour() * 60.0).floor() as u32;
            format!(
                "Day {} / {:02}:{:02}{}",
                clock.day,
                minutes / 60,
                minutes % 60,
                if clock.paused { " / paused" } else { "" }
            )
        },
    );
    overlay.weather = state.weather.as_ref().map_or_else(
        || "--".into(),
        |weather| {
            let wind = weather.wind();
            format!(
                "{:?} / rain {:.0}% / wind {:.1}, {:.1}",
                weather.pattern,
                weather.intensity() * 100.0,
                wind[0],
                wind[1]
            )
        },
    );
    overlay.seed = state.terrain.as_ref().map_or_else(
        || "--".into(),
        |terrain| terrain.0.config().seed.0.to_string(),
    );
    let solids = state
        .chunks
        .loaded
        .values()
        .filter(|section| section.mesh.is_some())
        .count();
    let water = state
        .chunks
        .loaded
        .values()
        .filter(|section| section.water.is_some())
        .count();
    overlay.render_sections = format!(
        "{} loaded / {} solid meshes / {} water meshes",
        state.chunks.loaded.len(),
        solids,
        water
    );
    overlay.streaming = state.generation.as_ref().map_or_else(
        || "--".into(),
        |generation| {
            format!(
                "{} wanted / {} jobs / {} columns",
                generation.wanted_sections, generation.pending_jobs, generation.prepared_columns
            )
        },
    );
    overlay.network = state.session.as_ref().map_or_else(
        || "Local world".into(),
        |session| {
            format!(
                "{} / {} / {} pending inputs",
                if session.ready {
                    "Connected"
                } else {
                    "Not synchronized"
                },
                session.game_mode.label(),
                session.pending_inputs
            )
        },
    );
    overlay.entities = format!(
        "{} creature replicas / {} loose items",
        state
            .creatures
            .as_ref()
            .map_or(0, |creatures| creatures.creatures.len()),
        state
            .session
            .as_ref()
            .map_or(0, |session| session.drops.len())
    );
    let (target_identifier, target_position) =
        target_details(&state.target, &state.chunks, &state.palette);
    overlay.target_identifier = target_identifier;
    overlay.target_position = target_position;
    overlay.fps = state
        .diagnostics
        .get(&FrameTimeDiagnosticsPlugin::FPS)
        .and_then(|d| d.smoothed())
        .map(|fps| format!("{fps:.0}"))
        .unwrap_or_else(|| "--".into());
    overlay.fps_min = state
        .session_fps
        .as_ref()
        .and_then(|fps| fps.minimum)
        .map(|fps| format!("{fps:.1}"))
        .unwrap_or_else(|| "--".into());
    overlay.fps_max = state
        .session_fps
        .as_ref()
        .and_then(|fps| fps.maximum)
        .map(|fps| format!("{fps:.1}"))
        .unwrap_or_else(|| "--".into());
    overlay.cpu = state
        .process
        .cpu_percent
        .map(|value| format!("{value:.1}%"))
        .unwrap_or_else(|| "--".into());
    overlay.ram = state
        .process
        .resident_bytes
        .map(|bytes| format!("{:.0} MiB", bytes as f64 / 1048576.0))
        .unwrap_or_else(|| "--".into());
    overlay.facing = camera.iter().next().map_or_else(
        || "--".into(),
        |(transform, aim)| {
            facing_label(aim.map_or(transform.rotation, terra_player::CameraLook::rotation))
        },
    );
    if let Some(pos) = debug_position(
        state
            .session
            .as_ref()
            .and_then(|session| session.confirmed_state())
            .map(|player| player.position),
        player.iter().next(),
        camera.iter().next().map(|(transform, _)| transform),
    ) {
        let (chunk, _) = global_to_section([
            pos.x.floor() as i64,
            pos.y.floor() as i64,
            pos.z.floor() as i64,
        ]);
        overlay.chunk = format!("{}, {}, {}", chunk.x, chunk.y, chunk.z);
        let block = pos.to_array().map(|coordinate| coordinate.floor() as i64);
        overlay.block_position = format!("{}, {}, {}", block[0], block[1], block[2]);
        overlay.local_position = format!(
            "{}, {}, {}",
            block[0].rem_euclid(16),
            block[1].rem_euclid(16),
            block[2].rem_euclid(16)
        );
        overlay.xyz = format!("{:.1}  {:.1}  {:.1}", pos.x, pos.y, pos.z);
        overlay.cave_visible = false;
        if let Some(terrain) = &state.terrain {
            let p = [f64::from(pos.x), f64::from(pos.y), f64::from(pos.z)];
            overlay.cave_visible = terrain.0.config().cave_structure_revision >= 3
                && terrain.0.surface_height(p[0], p[2]) - p[1] > 5.0;
            if overlay.cave_visible {
                let cave = terrain
                    .0
                    .cave_ecology_sample(p, terrain.0.surface_column(p[0], p[2]));
                overlay.cave_biome = format!(
                    "{} / {}",
                    cave.primary.label(),
                    cave.secondary.map_or("--", |b| b.label())
                );
                overlay.cave_region = format!("{:016x}", cave.region_id);
                overlay.cave_conditions =
                    format!("{:.0}% / {:.0}%", cave.blend * 100.0, cave.moisture * 100.0);
                overlay.cave_boundary = format!("{:.1} m", cave.boundary_distance);
                overlay.cave_connector = if terrain.0.cave_connector_at(p) {
                    "Connector"
                } else {
                    "Room / Rock"
                }
                .into();
                let temperature = terrain.0.biome_at(p[0], p[2]).temperature;
                let (props, minerals) = state.chunks.loaded.get(&chunk).map_or((0, 0), |s| {
                    (
                        s.props.len(),
                        s.data
                            .blocks()
                            .iter()
                            .filter(|&&m| matches!(m, 51..=53))
                            .count(),
                    )
                });
                overlay.cave_local = format!("{temperature:.2} / {props} / {minerals}");
                overlay.cave_fluid = state
                    .fluid
                    .as_ref()
                    .filter(|_| state.access.as_ref().is_none_or(|a| a.authoritative()))
                    .map_or_else(
                        || "Replica".into(),
                        |d| format!("{} / {:.2} ms", d.visited, d.elapsed_ms),
                    );
            }
        }
        overlay.biome = state.terrain.as_ref().map_or_else(
            || "--".into(),
            |terrain| {
                use terra_world::generation::biomes::BiomeKind;
                match terrain
                    .0
                    .surface_biome(&terrain.0.surface_column(f64::from(pos.x), f64::from(pos.z)))
                {
                    BiomeKind::Ocean => "Ocean",
                    BiomeKind::Plains => "Plains",
                    BiomeKind::Forest => "Forest",
                    BiomeKind::Desert => "Desert",
                    BiomeKind::Highland => "Highland",
                    BiomeKind::MushroomPark => "Mushroom Park",
                    BiomeKind::ClifftedDesert => "Cliffted Desert",
                    BiomeKind::CherryForest => "Cherry Forest",
                    BiomeKind::SpiritBlossom => "Spirit Blossom",
                    BiomeKind::BloodForest => "Blood Forest",
                    BiomeKind::Ashlands => "Ashlands",
                    BiomeKind::Savanna => "Savanna",
                    BiomeKind::Swamp => "Swamp",
                    BiomeKind::PetrifiedForest => "Petrified Forest",
                    BiomeKind::CrystalHighlands => "Crystal Highlands",
                    BiomeKind::MistyCoast => "Misty Coast",
                }
                .into()
            },
        );
    } else {
        overlay.chunk = "--".into();
        overlay.xyz = "--".into();
        overlay.block_position = "--".into();
        overlay.local_position = "--".into();
        overlay.biome = "--".into();
        overlay.cave_visible = false;
    }
}

fn debug_position(
    confirmed: Option<[f32; 3]>,
    body: Option<(&Transform, Option<&GlobalTransform>, Option<&ChildOf>)>,
    camera: Option<&Transform>,
) -> Option<Vec3> {
    confirmed
        .map(Vec3::from_array)
        .or_else(|| {
            body.and_then(|(local, global, parent)| {
                if parent.is_some() {
                    global.map(GlobalTransform::translation)
                } else {
                    Some(local.translation)
                }
            })
        })
        .or_else(|| camera.map(|camera| camera.translation))
}

fn facing_label(rotation: Quat) -> String {
    let direction = rotation * Vec3::NEG_Z;
    let cardinal = if direction.x.abs() > direction.z.abs() {
        if direction.x > 0.0 {
            "East (+X)"
        } else {
            "West (-X)"
        }
    } else if direction.z > 0.0 {
        "South (+Z)"
    } else {
        "North (-Z)"
    };
    let yaw = direction
        .x
        .atan2(-direction.z)
        .to_degrees()
        .rem_euclid(360.0);
    let pitch = (-direction.y).clamp(-1.0, 1.0).asin().to_degrees();
    format!("{cardinal} / yaw {yaw:.1} / pitch {pitch:.1}")
}

fn target_details(
    target: &TargetWorld,
    chunks: &terra_world::streaming::ChunkStore,
    palette: &terra_player::interaction::BlockPalette,
) -> (String, String) {
    match target.0 {
        Some(WorldTarget::Block(hit)) => {
            let material = chunks.block_at(hit.position);
            let identifier = material.map_or_else(
                || "Unloaded block".into(),
                |material| {
                    palette
                        .choices
                        .iter()
                        .find(|choice| choice.id == material)
                        .map_or_else(
                            || format!("Block {material}"),
                            |choice| choice.identifier.clone(),
                        )
                },
            );
            (
                identifier,
                format!(
                    "{}, {}, {} / face {}, {}, {}",
                    hit.position[0],
                    hit.position[1],
                    hit.position[2],
                    hit.normal[0],
                    hit.normal[1],
                    hit.normal[2]
                ),
            )
        }
        Some(WorldTarget::Prop(hit)) => (
            terra_core::ItemKind::Prop(hit.prop.kind.into()).identifier(),
            format!(
                "{:.1}, {:.1}, {:.1}",
                hit.prop.position[0], hit.prop.position[1], hit.prop.position[2]
            ),
        ),
        None => ("--".into(), "--".into()),
    }
}

fn inspection_labels(
    intent: Option<&terra_core::creatures::intentions::CreatureIntent>,
) -> (String, String, String) {
    let point = |p: [f64; 3]| format!("{:.1}, {:.1}, {:.1}", p[0], p[1], p[2]);
    let Some(intent) = intent.filter(|intent| intent.valid()) else {
        return (
            "Warte auf Serverentscheidung".into(),
            "--".into(),
            "--".into(),
        );
    };
    let route = if intent.path.is_empty() {
        if intent.blocked {
            "Blockiert / Umleitung wartet"
        } else {
            "Kein gespeicherter Graphpfad"
        }
        .into()
    } else {
        intent
            .path
            .iter()
            .copied()
            .map(point)
            .collect::<Vec<_>>()
            .join(" > ")
    };
    (
        intent.reason.label().into(),
        intent
            .destination
            .map_or_else(|| "Kein festes Ziel".into(), point),
        route,
    )
}

#[derive(bevy::ecs::system::SystemParam)]
pub struct OverlayState<'w> {
    diagnostics: Res<'w, DiagnosticsStore>,
    session_fps: Option<Res<'w, crate::debug::fps::SessionFps>>,
    info: Option<Res<'w, crate::menu::SessionInfo>>,
    clock: Option<Res<'w, terra_core::WorldClock>>,
    weather: Option<Res<'w, terra_core::WorldWeather>>,
    session: Option<Res<'w, terra_network::client::ClientSession>>,
    generation: Option<Res<'w, terra_world::streaming::WorldGeneration>>,
    adapter: Option<Res<'w, bevy::render::renderer::RenderAdapterInfo>>,
    debug: Res<'w, terra_core::DebugSettings>,
    menu: Option<Res<'w, crate::menu::MenuState>>,
    palette: Res<'w, terra_player::interaction::BlockPalette>,
    target: Res<'w, TargetWorld>,
    process: Res<'w, crate::AppProcessStats>,
    chunks: Res<'w, terra_world::streaming::ChunkStore>,
    terrain: Option<Res<'w, terra_world::streaming::WorldTerrain>>,
    fluid: Option<Res<'w, terra_world::water::FluidDiagnostics>>,
    access: Option<Res<'w, terra_world::streaming::WorldAccess>>,
    creature_target: Option<Res<'w, terra_player::interaction::TargetCreature>>,
    creatures: Option<Res<'w, terra_network::client::ReplicatedCreatures>>,
    inspection: Option<ResMut<'w, terra_network::client::CreatureInspection>>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use terra_player::interaction::{BlockChoice, BlockPalette};
    use terra_voxel::{BlockSection, SectionPos, blocks::BlockHit};
    use terra_world::streaming::{ChunkStore, LoadedSection};
    #[test]
    fn debug_coordinates_use_confirmed_world_position_and_respect_parented_bodies_offline() {
        let mut world = World::new();
        let predicted = Vec3::new(128.5, 72.0, -64.5);
        let parent = world.spawn(Transform::from_translation(predicted)).id();
        let body = world
            .spawn((
                terra_player::PlayerBody,
                Transform::IDENTITY,
                GlobalTransform::from_translation(predicted),
                ChildOf(parent),
            ))
            .id();
        let body_data = || {
            Some((
                world.get::<Transform>(body).unwrap(),
                world.get::<GlobalTransform>(body),
                world.get::<ChildOf>(body),
            ))
        };
        let camera = Transform::from_xyz(130.0, 74.0, -63.0);
        let confirmed = [128.25, 71.75, -64.75];
        assert_eq!(
            world.get::<Transform>(body).unwrap().translation,
            Vec3::ZERO
        );
        assert_eq!(
            debug_position(Some(confirmed), body_data(), Some(&camera)),
            Some(Vec3::from_array(confirmed))
        );
        assert_eq!(
            debug_position(None, body_data(), Some(&camera)),
            Some(predicted)
        );
        assert_eq!(
            debug_position(None, None, Some(&camera)),
            Some(camera.translation)
        );
    }
    #[test]
    fn facing_follows_aim_rotation_in_all_four_cardinal_directions() {
        assert!(facing_label(Quat::IDENTITY).starts_with("North (-Z) / yaw 0.0"));
        assert!(
            facing_label(Quat::from_rotation_y(-std::f32::consts::FRAC_PI_2))
                .starts_with("East (+X)")
        );
        assert!(
            facing_label(Quat::from_rotation_y(std::f32::consts::FRAC_PI_2))
                .starts_with("West (-X)")
        );
        assert!(
            facing_label(Quat::from_rotation_y(std::f32::consts::PI)).starts_with("South (+Z)")
        );
        assert!(facing_label(Quat::from_rotation_x(0.5)).contains("pitch -28.6"));
    }
    #[test]
    fn mob_intentions_format_only_authoritative_targets_and_real_route_points() {
        use terra_core::creatures::intentions::{CreatureIntent, IntentReason};
        let intent = CreatureIntent {
            id: 4,
            reason: IntentReason::DayRefuge,
            destination: Some([-16.5, -20.0, 3.5]),
            path: vec![[-15.5, -20.0, 3.5]],
            blocked: false,
        };
        let (reason, target, route) = inspection_labels(Some(&intent));
        assert!(reason.contains("Unterschlupf"));
        assert_eq!(target, "-16.5, -20.0, 3.5");
        assert_eq!(route, "-15.5, -20.0, 3.5");
        let (_, target, route) = inspection_labels(None);
        assert_eq!((target.as_str(), route.as_str()), ("--", "--"));
    }
    #[test]
    fn targeted_animal_vitals_render_without_general_debug_and_hide_with_target_loss() {
        use terra_core::creatures::*;
        use terra_network::client::{CreatureTrack, ReplicatedCreatures};
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
                std::time::Duration::from_millis(300),
            ))
            .init_resource::<DiagnosticsStore>()
            .init_resource::<DebugOverlay>()
            .insert_resource(terra_core::DebugSettings {
                creature_inspection: true,
                ..default()
            })
            .init_resource::<crate::AppProcessStats>()
            .init_resource::<ChunkStore>()
            .init_resource::<TargetWorld>()
            .init_resource::<BlockPalette>()
            .init_resource::<terra_player::interaction::TargetCreature>()
            .init_resource::<ReplicatedCreatures>()
            .add_systems(Update, update_debug_overlay);
        let id = creature_id([0, 0], 0).unwrap();
        let animal = Creature {
            id,
            home: [0, 0],
            kind: CreatureKind::Sheep,
            position: [0.0, 3.0, 0.0],
            velocity: [0.0; 3],
            yaw: 0.0,
            health: 7.0,
            behavior: CreatureBehavior::Drink,
            timer: 2.0,
            age: 1.0,
            variant: 0,
            needs: CreatureNeeds::full(CreatureKind::Sheep),
        };
        app.world_mut()
            .resource_mut::<ReplicatedCreatures>()
            .creatures
            .insert(
                id,
                CreatureTrack {
                    playback: Default::default(),
                    previous: animal.clone(),
                    current: animal,
                    received: std::time::Instant::now(),
                    interval: 0.1,
                },
            );
        app.world_mut()
            .resource_mut::<terra_player::interaction::TargetCreature>()
            .0 = Some(id);
        app.update();
        app.update();
        let view = app.world().resource::<DebugOverlay>();
        assert!(!view.visible && view.animal_visible);
        assert_eq!(
            (
                &*view.animal_name,
                &*view.animal_health,
                &*view.animal_food,
                &*view.animal_water
            ),
            (
                definitions::get(CreatureKind::Sheep).variant_title(0),
                "7.0 / 7",
                "35.0 / 35",
                "25.0 / 25"
            )
        );
        assert_eq!(view.animal_behavior, "Trinkt");
        for (kind, behavior, label) in [
            (
                CreatureKind::Squid,
                CreatureBehavior::Hold,
                "Haelt sich fest",
            ),
            (
                CreatureKind::Squid,
                CreatureBehavior::Ink,
                "Versprueht Tinte",
            ),
            (CreatureKind::MoonJelly, CreatureBehavior::Swim, "Schwimmt"),
            (CreatureKind::FireJelly, CreatureBehavior::Swim, "Schwimmt"),
            (CreatureKind::VoidJelly, CreatureBehavior::Swim, "Schwimmt"),
            (CreatureKind::SunJelly, CreatureBehavior::Swim, "Schwimmt"),
            (
                CreatureKind::CrystalJelly,
                CreatureBehavior::Swim,
                "Schwimmt",
            ),
            (
                CreatureKind::ReefShark,
                CreatureBehavior::Attack,
                "Greift an",
            ),
            (
                CreatureKind::HammerheadShark,
                CreatureBehavior::Hunt,
                "Jagt Fische",
            ),
            (
                CreatureKind::WhaleShark,
                CreatureBehavior::Attack,
                "Greift an",
            ),
        ] {
            {
                let mut animals = app.world_mut().resource_mut::<ReplicatedCreatures>();
                let animal = &mut animals.creatures.get_mut(&id).unwrap().current;
                animal.kind = kind;
                animal.behavior = behavior;
                animal.health = kind.health();
                animal.needs = CreatureNeeds::full(kind);
            }
            app.update();
            let view = app.world().resource::<DebugOverlay>();
            assert!(view.animal_visible && !view.visible);
            assert_eq!(view.animal_name, definitions::get(kind).variant_title(0));
            assert_eq!(view.animal_behavior, label);
            assert_eq!(
                view.animal_health,
                format!("{:.1} / {:.0}", kind.health(), kind.health())
            );
        }
        {
            let mut animals = app.world_mut().resource_mut::<ReplicatedCreatures>();
            let track = animals.creatures.get_mut(&id).unwrap();
            track.current.kind = CreatureKind::Fish;
            track.current.health = 3.0;
            track.current.needs = CreatureNeeds::full(CreatureKind::Fish);
        }
        app.update();
        let view = app.world().resource::<DebugOverlay>();
        assert_eq!((&*view.animal_food, &*view.animal_water), ("--", "--"));
        app.world_mut()
            .resource_mut::<terra_player::interaction::TargetCreature>()
            .0 = None;
        app.update();
        assert!(!app.world().resource::<DebugOverlay>().animal_visible);
    }
    #[test]
    fn hud_uses_target_material_and_player_position_with_offline_camera_fallback() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_plugins(FrameTimeDiagnosticsPlugin::default())
            .insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
                std::time::Duration::from_secs_f64(1.0 / 60.0),
            ))
            .init_resource::<DebugOverlay>()
            .init_resource::<crate::debug::fps::SessionFps>()
            .insert_resource(terra_core::DebugSettings {
                enabled: true,
                ..default()
            })
            .init_resource::<crate::AppProcessStats>()
            .init_resource::<ChunkStore>()
            .init_resource::<TargetWorld>()
            .insert_resource(BlockPalette {
                choices: vec![
                    BlockChoice {
                        id: 1,
                        name: "Meadow Grass".into(),
                        identifier: "teramic::grass".into(),
                        description: String::new(),
                    },
                    BlockChoice {
                        id: 3,
                        name: "Stone".into(),
                        identifier: "teramic::stone".into(),
                        description: String::new(),
                    },
                ],
                selected: 1,
                prop_descriptions: default(),
                ..default()
            })
            .add_systems(Update, update_debug_overlay);
        {
            let mut fps = app
                .world_mut()
                .resource_mut::<crate::debug::fps::SessionFps>();
            fps.minimum = Some(2.0);
            fps.maximum = Some(144.0);
        }
        app.insert_resource(crate::menu::SessionInfo {
            title: "Debug snapshot".into(),
            ..default()
        });
        app.insert_resource(terra_core::WorldClock {
            day: 7,
            phase: 0.625,
            paused: true,
            ..default()
        });
        app.insert_resource(terra_core::WorldWeather {
            pattern: terra_core::WeatherPattern::Rain,
            from_intensity: 0.25,
            target_intensity: 0.25,
            from_wind: [1.5, -0.5],
            target_wind: [1.5, -0.5],
            ..default()
        });
        app.insert_resource(terra_world::streaming::WorldGeneration {
            wanted_sections: 14,
            pending_jobs: 3,
            prepared_columns: 9,
            ..default()
        });
        let mut session = terra_network::client::ClientSession::default();
        session.ready = true;
        session.pending_inputs = 6;
        app.insert_resource(session);
        app.world_mut()
            .resource_mut::<crate::AppProcessStats>()
            .cpu_percent = Some(12.5);
        app.world_mut()
            .resource_mut::<crate::AppProcessStats>()
            .resident_bytes = Some(32 * 1024 * 1024);
        let position = SectionPos::new(-1, 0, -1);
        let mut data = BlockSection::empty(position).unwrap();
        data.set_block([15, 0, 15], 1).unwrap();
        app.world_mut().resource_mut::<ChunkStore>().loaded.insert(
            position,
            LoadedSection {
                data,
                mesh: None,
                mesh_epoch: 0,
                water_epoch: 0,
                props: Vec::new(),
                water: None,
            },
        );
        app.world_mut().resource_mut::<TargetWorld>().0 = Some(WorldTarget::Block(BlockHit {
            position: [-1, 0, -1],
            normal: [0, 1, 0],
            distance: 2.0,
        }));
        app.world_mut()
            .spawn((TerraCamera, Transform::from_xyz(-0.5, 2.0, -0.5)));
        for _ in 0..30 {
            app.update();
        }
        let overlay = app.world().resource::<DebugOverlay>();
        assert_eq!(overlay.block_name, "Meadow Grass");
        assert_eq!(overlay.chunk, "-1, 0, -1");
        assert_eq!(overlay.xyz, "-0.5  2.0  -0.5");
        assert_eq!(overlay.block_position, "-1, 2, -1");
        assert_eq!(overlay.local_position, "15, 2, 15");
        assert_eq!(overlay.target_identifier, "teramic::grass");
        assert_eq!(overlay.target_position, "-1, 0, -1 / face 0, 1, 0");
        assert_eq!(overlay.world, "Debug snapshot");
        assert_eq!(overlay.world_time, "Day 7 / 15:00 / paused");
        assert_eq!(overlay.weather, "Rain / rain 25% / wind 1.5, -0.5");
        assert_eq!(overlay.streaming, "14 wanted / 3 jobs / 9 columns");
        assert_eq!(
            overlay.render_sections,
            "1 loaded / 0 solid meshes / 0 water meshes"
        );
        assert!(
            overlay.network.contains("Connected") && overlay.network.contains("6 pending inputs")
        );
        assert_eq!(overlay.cpu, "12.5%");
        assert_eq!(overlay.ram, "32 MiB");
        assert_eq!(
            (overlay.gpu.as_str(), overlay.display.as_str()),
            ("--", "--")
        );
        assert!(overlay.visible);
        assert_eq!(overlay.fps_min, "2.0");
        assert_eq!(overlay.fps_max, "144.0");
        app.world_mut()
            .resource_mut::<terra_core::DebugSettings>()
            .enabled = false;
        app.update();
        assert!(!app.world().resource::<DebugOverlay>().visible);
        app.world_mut()
            .resource_mut::<terra_core::DebugSettings>()
            .enabled = true;
        app.world_mut().resource_mut::<TargetWorld>().0 = None;
        for _ in 0..30 {
            app.update();
        }
        assert_eq!(app.world().resource::<DebugOverlay>().block_name, "--");

        // This fixture checks player/camera ownership with legacy climate names.
        let terrain = terra_world::TerraGenerator::new(terra_core::WorldConfig {
            landscape_revision: 1,
            ..Default::default()
        })
        .unwrap();
        let camera_biome = terrain.biome_at(-0.5, -0.5).weights.dominant();
        let player_position = (-16..=16)
            .flat_map(|x| (-16..=16).map(move |z| (x, z)))
            .map(|(x, z)| Vec3::new(x as f32 * 512.0 + 0.5, 7.0, z as f32 * 512.0 + 0.5))
            .find(|position| {
                terrain
                    .biome_at(f64::from(position.x), f64::from(position.z))
                    .weights
                    .dominant()
                    != camera_biome
            })
            .expect("fixture needs distinct camera and player biomes");
        let player_biome = format!(
            "{:?}",
            terrain
                .biome_at(f64::from(player_position.x), f64::from(player_position.z))
                .weights
                .dominant()
        );
        app.insert_resource(terra_world::streaming::WorldTerrain(terrain));
        app.world_mut().spawn((
            terra_player::PlayerBody,
            Transform::from_translation(player_position),
        ));
        for _ in 0..30 {
            app.update();
        }
        let overlay = app.world().resource::<DebugOverlay>();
        assert_eq!(overlay.biome, player_biome);
        assert_eq!(
            overlay.xyz,
            format!(
                "{:.1}  {:.1}  {:.1}",
                player_position.x, player_position.y, player_position.z
            )
        );
        let (section, _) =
            global_to_section(player_position.to_array().map(|value| value.floor() as i64));
        assert_eq!(
            overlay.chunk,
            format!("{}, {}, {}", section.x, section.y, section.z)
        );

        app.init_resource::<crate::inventory::InventoryState>();
        app.world_mut()
            .resource_mut::<crate::inventory::InventoryState>()
            .open = true;
        app.update();
        assert!(app.world().resource::<DebugOverlay>().visible);
        assert!(app.world().resource::<terra_core::DebugSettings>().enabled);
        app.world_mut()
            .resource_mut::<crate::inventory::InventoryState>()
            .open = false;
        app.update();
        assert!(app.world().resource::<DebugOverlay>().visible);

        app.init_resource::<crate::SettingsView>();
        app.world_mut().resource_mut::<crate::SettingsView>().open = true;
        app.update();
        assert!(app.world().resource::<DebugOverlay>().visible);
        app.world_mut().resource_mut::<crate::SettingsView>().open = false;
        app.insert_resource(crate::menu::MenuState {
            page: crate::menu::MenuPage::Pause,
            session_active: true,
            ..default()
        });
        app.update();
        assert!(app.world().resource::<DebugOverlay>().visible);
        app.world_mut()
            .resource_mut::<crate::menu::MenuState>()
            .page = crate::menu::MenuPage::Playing;
        app.update();
        assert!(app.world().resource::<DebugOverlay>().visible);
        assert!(app.world().resource::<terra_core::DebugSettings>().enabled);
    }
}
