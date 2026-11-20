use bevy::{
    app::TaskPoolPlugin,
    log::LogPlugin,
    prelude::*,
    render::{
        RenderPlugin,
        settings::{RenderCreation, WgpuSettings, WgpuSettingsPriority},
    },
    window::PresentMode,
};
use std::error::Error;
use terra_core::{
    ClientConfig, Config, GameConfig, GraphicsConfig, ServerConfig, TerraCorePlugin,
    WindowModeSetting, WorldConfig, config_directory,
};
use terra_network::{TerraNetworkPlugin, connection::NetworkRole};
use terra_physics::TerraPhysicsPlugin;
use terra_player::TerraPlayerPlugin;
use terra_render::{TerraRenderDiagnosticsPlugin, TerraRenderPlugin};
use terra_ui::TerraUiPlugin;
use terra_world::{TerraWorldPlugin, WorldMode};

#[path = "preview_demand.rs"]
mod preview_demand;
#[path = "startup_ui.rs"]
pub(crate) mod startup_ui;

pub fn build_app() -> Result<App, Box<dyn Error>> {
    let task_pool_options = crate::task_pools::options(
        std::env::args().any(|argument| argument.starts_with("--smoke-test")),
        std::env::var("TERRA_COMPUTE_THREADS").ok().as_deref(),
    )?;
    let compute_thread_cap = task_pool_options.compute.max_threads;
    terra_core::PlayerGender::from_arguments(std::env::args().skip(1))?;
    let directory = config_directory();
    let profile_path =
        terra_core::PlayerProfile::selected_path(&terra_core::menu_config_directory())?;
    let benchmark = std::env::args().any(|arg| arg == "--smoke-test-benchmark");
    let presentation_override = benchmark_presentation(
        benchmark,
        std::env::var("TERRA_BENCH_PRESENT_MODE").ok().as_deref(),
    )?;
    let integrated_benchmark =
        benchmark && std::env::var("TERRA_BENCH_MODE").is_ok_and(|mode| mode == "integrated");
    let benchmark_ui = validate_benchmark_ui(
        std::env::var("TERRA_BENCH_UI").is_ok_and(|value| value == "1"),
        benchmark,
        integrated_benchmark,
    )?;
    let benchmark_save_directory =
        std::env::var_os("TERRA_BENCH_SAVE_DIR").map(std::path::PathBuf::from);
    let benchmark_world_id = std::env::var("TERRA_BENCH_WORLD_ID")
        .map(Some)
        .or_else(|error| match error {
            std::env::VarError::NotPresent => Ok(None),
            error => Err(error),
        })?;
    if benchmark_world_id.is_some() && std::env::var_os("TERRA_WORLD_DIR").is_none() {
        return Err("TERRA_BENCH_WORLD_ID requires an explicit isolated TERRA_WORLD_DIR".into());
    }
    let benchmark_save = resolve_benchmark_save(
        integrated_benchmark,
        benchmark_save_directory.as_deref(),
        benchmark_world_id.as_deref(),
        &terra_storage::WorldLibrary::default(),
    )?;
    if std::env::args().any(|arg| arg == "--smoke-test-lod")
        && std::env::var("TERRA_BENCH_MODE").is_ok_and(|mode| mode == "integrated")
    {
        return Err("Integrated flythrough is not supported by the camera-only LOD fixture; use --smoke-test-benchmark with TERRA_BENCH_MODE=integrated for a real integrated-server measurement".into());
    }
    let building_fixture = std::env::args().any(|arg| arg == "--smoke-test-building");
    let fiverr_grass_fixture = crate::fiverr_grass_smoke::requested();
    let inventory_fixture = std::env::args().any(|arg| arg == "--smoke-test-inventory");
    let survival_fixture = std::env::args().any(|arg| arg == "--smoke-test-survival");
    let calendar_fixture = std::env::args().any(|arg| arg == "--smoke-test-calendar-hud");
    let effects_fixture = std::env::args().any(|arg| arg == "--smoke-test-effects");
    let taming_fixture = std::env::args().any(|arg| arg == "--smoke-test-taming");
    let breeding_fixture = std::env::args().any(|arg| arg == "--smoke-test-breeding");
    let riding_fixture = std::env::args().any(|arg| arg == "--smoke-test-riding");
    let pet_fixture = std::env::args().any(|arg| arg == "--smoke-test-pet-commands");
    let equipment_fixture = std::env::args().any(|arg| arg == "--smoke-test-equipment");
    let crusher_fixture = std::env::args().any(|arg| arg == "--smoke-test-crusher");
    let mixer_fixture = std::env::args().any(|arg| arg == "--smoke-test-mixer");
    let rgb_logistics_fixture = std::env::args().any(|arg| arg == "--smoke-test-rgb-logistics");
    let alchemy_fixture = std::env::args().any(|arg| arg == "--smoke-test-alchemy");
    let hydraulics_fixture = std::env::args().any(|arg| arg == "--smoke-test-hydraulics");
    let farming_fixture = std::env::args().any(|arg| arg == "--smoke-test-farming");
    let food_fixture = std::env::args().any(|arg| arg == "--smoke-test-food");
    let furnace_gameplay_fixture =
        std::env::args().any(|arg| arg == "--smoke-test-furnace-gameplay");
    let fixture = !integrated_benchmark
        && std::env::args().any(|argument| {
            argument.starts_with("--smoke-test")
                && !argument.starts_with("--smoke-test-network")
                && !argument.starts_with("--smoke-test-building")
                && !argument.starts_with("--smoke-test-chat")
                && argument != "--smoke-test-minimap"
                && !argument.starts_with("--smoke-test-inventory")
                && argument != "--smoke-test-camera"
                && argument != "--smoke-test-drops"
                && argument != "--smoke-test-fluid-source"
                && argument != "--smoke-test-survival"
                && argument != "--smoke-test-calendar-hud"
                && argument != "--smoke-test-effects"
                && argument != "--smoke-test-taming"
                && argument != "--smoke-test-breeding"
                && argument != "--smoke-test-riding"
                && argument != "--smoke-test-pet-commands"
                && argument != "--smoke-test-equipment"
                && argument != "--smoke-test-crusher"
                && argument != "--smoke-test-mixer"
                && argument != "--smoke-test-rgb-logistics"
                && argument != "--smoke-test-alchemy"
                && argument != "--smoke-test-hydraulics"
                && argument != "--smoke-test-farming"
                && argument != "--smoke-test-food"
                && argument != "--smoke-test-furnace-gameplay"
                && argument != "--smoke-test-loading"
                && !argument.starts_with("--smoke-test-menu")
        });
    let frontend = !std::env::args().any(|a| {
        a.starts_with("--smoke-test")
            && !a.starts_with("--smoke-test-menu")
            && a != "--smoke-test-loading"
    });
    let mut startup_errors = Vec::new();
    let mut client = load_config::<ClientConfig>(
        &directory.join("client.toml"),
        frontend,
        &mut startup_errors,
    )?;
    let benchmark_graphics = load_benchmark_graphics(
        &directory,
        &mut client,
        benchmark_ui
            || (std::env::args().any(|arg| arg.starts_with("--smoke-test"))
                && std::env::var("TERRA_BENCH_GRAPHICS").is_ok_and(|value| value == "1")),
    )?;
    let benchmark_scenario = if std::env::args().any(|arg| arg.starts_with("--smoke-test")) {
        BenchmarkScenario::parse(
            std::env::var("TERRA_BENCH_WEATHER").ok().as_deref(),
            std::env::var("TERRA_BENCH_CAMERA").ok().as_deref(),
            std::env::var("TERRA_BENCH_LOOK_AT").ok().as_deref(),
        )?
    } else {
        None
    };
    let mut world =
        load_config::<WorldConfig>(&directory.join("world.toml"), frontend, &mut startup_errors)?;
    if fiverr_grass_fixture {
        crate::fiverr_grass_smoke::configure(&mut world, &mut client);
    }
    if std::env::args().any(|arg| arg == "--smoke-test-waterfalls") {
        world.base_height = 16.0;
        world.hill_amplitude = 0.0;
        world.mountain_amplitude = 0.0;
        world.biomes = false;
        world.trees = false;
        world.caves = false;
        world.rivers = false;
        world.water = true;
        world.sea_level = 0.0;
        if std::env::var("TERRA_PIPE_AIR_SMOKE").as_deref() == Ok("1") {
            world.sea_level = 20.3;
            world.landscape_revision = 0;
            world.structures_revision = 0;
        }
        client.view_distance = 3;
        client.vertical_view_distance = 3;
    }
    if std::env::args().any(|arg| arg == "--smoke-test-tree-collapse") {
        world.base_height = 16.0;
        world.hill_amplitude = 0.0;
        world.mountain_amplitude = 0.0;
        world.biomes = false;
        world.trees = true;
        world.tree_density = 0.08;
        world.caves = false;
        world.rivers = false;
        world.water = false;
        client.fov_degrees = 60.0;
        client.view_distance = 3;
        client.vertical_view_distance = 3;
    }
    if std::env::args().any(|arg| arg == "--smoke-test-lanterns") {
        world.base_height = 16.0;
        world.hill_amplitude = 0.0;
        world.mountain_amplitude = 0.0;
        world.terrace_strength = 0.0;
        world.landscape_revision = 0;
        world.ecology_revision = 0;
        world.geology_revision = 0;
        world.structures_revision = 0;
        world.biomes = false;
        world.trees = false;
        world.caves = false;
        world.water = false;
        world.rivers = false;
        world.sea_level = 0.0;
        client.fov_degrees = 55.0;
        client.view_distance = 3;
        client.vertical_view_distance = 3;
        client.fog_distance = 0.0;
    }
    if std::env::args().any(|arg| arg == "--smoke-test-cave-structures")
        && std::env::var_os("TERRA_CAVE_SHELL_INSPECT").is_some()
    {
        world = WorldConfig::default();
    }
    if std::env::args().any(|arg| arg == "--smoke-test-biomes") {
        let scene = std::env::var("TERRA_BIOME_SCENE").unwrap_or_default();
        if scene.starts_with("kingdom-") || scene == "outpost" {
            // Native structure evidence must use the finder's exact terrain.
            world.structures_revision = 10;
            client.view_distance = 12;
            client.vertical_view_distance = 5;
            // The detached overview is 228 m from the keep. Disable distance
            // fog only in this isolated smoke view instead of streaming View24.
            client.fog_distance = 0.0;
        } else if scene.starts_with("reference-")
            || (scene.starts_with("picker-")
                && std::env::var_os("TERRA_REFERENCE_BIOMES").is_some())
            || matches!(
                scene.as_str(),
                "blood-props" | "spirit-props" | "rock-formations"
            )
        {
            // New ecology photographs must retain the requested terrain, not
            // enter the ecology-zero legacy regression fixture below.
            let requested_ecology = world.ecology_revision;
            if std::env::var_os("TERRA_REFERENCE_BIOMES").is_some() {
                // All eight panoramas and their plant close-ups use the same
                // fresh-world revision set, including the Blood picker.
                let (seed, time) = (world.seed, world.time);
                world = WorldConfig::default();
                world.seed = seed;
                world.time = time;
            }
            world.ecology_revision = if scene == "rock-formations"
                && std::env::var_os("TERRA_FANTASY_ROCKS").is_some()
            {
                requested_ecology.max(9)
            } else {
                9
            };
            if world.biome_definitions.is_none() {
                world.biome_definitions = Some(terra_core::BiomeDefinitions::load_directory(
                    terra_core::asset_directory().join("biome"),
                )?);
            }
            client.vertical_view_distance = client.vertical_view_distance.max(4);
            if scene.starts_with("picker-") {
                client.view_distance = 10;
                client.vertical_view_distance = 5;
            }
        } else if scene == "blood-forest" || scene == "picker-blood-forest" {
            world.blood_forest_revision = 1;
            if world.biome_definitions.is_none() {
                world.biome_definitions = Some(terra_core::BiomeDefinitions::load_directory(
                    terra_core::asset_directory().join("biome"),
                )?);
            }
            client.view_distance = 8;
            client.vertical_view_distance = 5;
        } else if scene == "cemetery" || scene.starts_with("grave-") {
            // Preserve the exact native finder config; never replace the saved
            // seed/revisions with the legacy ecology-zero gallery world.
            client.view_distance = 8;
            client.vertical_view_distance = 5;
        } else if scene.starts_with("picker-") {
            // Photograph current fresh-world biomes rather than the legacy
            // regression world's ecology-revision-zero terrain below.
            let requested_seed = world.seed;
            let requested_time = world.time;
            world = WorldConfig::default();
            world.seed = requested_seed;
            world.time = requested_time;
            world.biome_definitions = Some(terra_core::BiomeDefinitions::load_directory(
                terra_core::asset_directory().join("biome"),
            )?);
            world.structures.set_all(false);
            world.preferred_spawn_biome = None;
            client.view_distance = 10;
            client.vertical_view_distance = 5;
        } else if scene.starts_with("landscape-") || scene.starts_with("connected-") {
            // Keep the requested seed and saved terrain revision for paired views.
            client.vertical_view_distance = client.vertical_view_distance.max(4);
        } else {
            world = WorldConfig::default();
            world.landscape_revision = 2;
            // These fixed native regression views predate the opt-in landscape field.
            if !scene.starts_with("cherry") {
                world.landscape_revision = 0;
                world.ecology_revision = 0;
            }
            world.rivers = scene.starts_with("water-");
        }
    }
    if std::env::args().any(|arg| {
        matches!(
            arg.as_str(),
            "--smoke-test-ecology" | "--smoke-test-ores" | "--smoke-test-geology"
        )
    }) {
        world = WorldConfig::default();
    }
    if std::env::args().any(|arg| {
        matches!(
            arg.as_str(),
            "--smoke-test-rivers" | "--smoke-test-river-benchmark" | "--smoke-test-water-audio"
        )
    }) {
        world.rivers = true;
    }
    if let Some(entry) = benchmark_save.as_ref().and_then(|save| save.entry.as_ref()) {
        world = entry.manifest.world.clone();
    }
    // Match the opt-in handcrafted cave fixture's Y=18 surface before creating
    // generators or column caches. Production world configuration is untouched.
    if std::env::args().any(|a| a == "--smoke-test-caves")
        && (std::env::var_os("TERRA_TORCH_INSPECT").is_some()
            || std::env::var_os("TERRA_CAVE_LIGHT_INSPECT").is_some()
            || std::env::var_os("TERRA_CAVE_PORTAL_INSPECT").is_some()
            || std::env::var_os("TERRA_CAVE_LOD_SHADOW_INSPECT").is_some())
    {
        world.base_height = 18.0;
        world.hill_amplitude = 0.0;
        world.mountain_amplitude = 0.0;
        world.biomes = false;
        world.caves = false;
        world.water = std::env::var_os("TERRA_CAVE_LIGHT_INSPECT").is_some()
            || std::env::var_os("TERRA_CAVE_PORTAL_INSPECT").is_some();
        world.rivers = false;
        world.trees = false;
    }
    let connect_address = {
        if std::env::args().any(|a| {
            matches!(
                a.as_str(),
                "--smoke-test-creatures"
                    | "--smoke-test-tracks"
                    | "--smoke-test-spiders"
                    | "--smoke-test-web-constructions"
                    | "--smoke-test-stalker"
                    | "--smoke-test-mob-health"
                    | "--smoke-test-flower-wither"
            )
        }) {
            world.base_height = 32.0;
            world.hill_amplitude = 0.0;
            world.mountain_amplitude = 0.0;
            world.biomes = false;
            world.trees = false;
            world.caves = false;
            world.water = false;
            world.rivers = false;
        }
        let mut arguments = std::env::args().skip(1);
        let mut address = None;
        while let Some(argument) = arguments.next() {
            if argument == "--connect" {
                if address.is_some() {
                    return Err("--connect can only be provided once".into());
                }
                address = Some(
                    arguments
                        .next()
                        .ok_or("--connect requires IP:PORT")?
                        .parse::<std::net::SocketAddr>()?,
                );
            } else if argument == "--connect-token" {
                return Err("Join files are no longer needed; use --connect IP:PORT".into());
            }
        }
        address
    };
    if fixture && connect_address.is_some() {
        return Err("Rendering fixtures cannot join a multiplayer session".into());
    }
    if food_fixture && connect_address.is_some() {
        return Err("The food fixture requires its own temporary integrated world".into());
    }
    if calendar_fixture && connect_address.is_some() {
        return Err("The calendar/HUD fixture requires its own temporary integrated world".into());
    }
    if effects_fixture && connect_address.is_some() {
        return Err("The status-effect fixture requires its own temporary integrated world".into());
    }
    if taming_fixture && connect_address.is_some() {
        return Err("The taming fixture requires its own temporary integrated world".into());
    }
    if breeding_fixture && connect_address.is_some() {
        return Err("Breeding fixture requires its isolated integrated server".into());
    }
    if furnace_gameplay_fixture && connect_address.is_some() {
        return Err("The furnace fixture requires its own temporary integrated world".into());
    }
    if mixer_fixture && connect_address.is_some() {
        return Err("The Mixer fixture requires its own temporary integrated world".into());
    }
    if rgb_logistics_fixture && connect_address.is_some() {
        return Err("RGB logistics fixture requires its isolated integrated server".into());
    }
    if alchemy_fixture && connect_address.is_some() {
        return Err("Alchemy smoke requires the integrated server, not --connect".into());
    }
    if hydraulics_fixture && connect_address.is_some() {
        return Err("Hydraulics smoke requires its isolated integrated server".into());
    }
    if benchmark_ui && connect_address.is_some() {
        return Err("TERRA_BENCH_UI requires the integrated server, not --connect".into());
    }
    if connect_address.is_some() && std::env::args().any(|argument| argument == "--singleplayer") {
        return Err("Choose either --singleplayer or --connect".into());
    }
    let mut server = load_config::<ServerConfig>(
        &directory.join("server.toml"),
        frontend,
        &mut startup_errors,
    )?;
    if let Some(entry) = benchmark_save.as_ref().and_then(|save| save.entry.as_ref()) {
        server.game_mode = entry.mode;
        server.natural_mob_limit = None;
    }
    let config = GameConfig {
        client: client.clone(),
        server,
        world,
    };
    let mut survival_profile = None;
    let (integrated, token) = if fixture {
        (None, None)
    } else if !frontend {
        if let Some(address) = connect_address {
            let profile = terra_core::PlayerProfile::load_for_client(&profile_path)?;
            let token =
                terra_network::browser::join(address, &profile).map_err(std::io::Error::other)?;
            // Explicit network fixtures load this same persisted identity below.
            (None, Some(token))
        } else {
            let mut host_config = config.clone();
            host_config.server.allow_flight = true;
            host_config.server.game_mode = if let Some(entry) =
                benchmark_save.as_ref().and_then(|save| save.entry.as_ref())
            {
                entry.mode
            } else if benchmark_ui {
                config.server.game_mode
            } else if (building_fixture
                && !std::env::var("TERRA_BUILD_CREATIVE").is_ok_and(|value| value == "1"))
                || (inventory_fixture
                    && std::env::var("TERRA_INVENTORY_MODE").is_ok_and(|value| value == "survival"))
            {
                terra_core::GameMode::Survival
            } else {
                terra_core::GameMode::Creative
            };
            let (server, token) = if inventory_fixture && crate::kingdom_smoke::requested() {
                let (host, token, profile) =
                    crate::kingdom_smoke::prepare(host_config).map_err(std::io::Error::other)?;
                survival_profile = Some(profile);
                (host, token)
            } else if inventory_fixture && std::env::var_os("TERRA_CRAFTING_SMOKE").is_some() {
                let (host, token, profile) =
                    crate::crafting_smoke::prepare(host_config).map_err(std::io::Error::other)?;
                survival_profile = Some(profile);
                (host, token)
            } else if building_fixture && std::env::var_os("TERRA_FURNITURE_MINING_SMOKE").is_some()
            {
                let (host, token, profile) =
                    crate::building_smoke::prepare(host_config).map_err(std::io::Error::other)?;
                survival_profile = Some(profile);
                (host, token)
            } else if calendar_fixture {
                let (host, token, profile) =
                    crate::hud_smoke::prepare(host_config).map_err(std::io::Error::other)?;
                survival_profile = Some(profile);
                (host, token)
            } else if crusher_fixture {
                let (host, token, profile) =
                    crate::crusher_smoke::prepare(host_config).map_err(std::io::Error::other)?;
                survival_profile = Some(profile);
                (host, token)
            } else if hydraulics_fixture {
                let (host, token, profile) =
                    crate::hydraulics_smoke::prepare(host_config).map_err(std::io::Error::other)?;
                survival_profile = Some(profile);
                (host, token)
            } else if alchemy_fixture {
                let (host, token, profile) =
                    crate::alchemy_smoke::prepare(host_config).map_err(std::io::Error::other)?;
                survival_profile = Some(profile);
                (host, token)
            } else if rgb_logistics_fixture {
                let (host, token, profile) = crate::rgb_logistics_smoke::prepare(host_config)
                    .map_err(std::io::Error::other)?;
                survival_profile = Some(profile);
                (host, token)
            } else if mixer_fixture {
                let (host, token, profile) =
                    crate::mixer_smoke::prepare(host_config).map_err(std::io::Error::other)?;
                survival_profile = Some(profile);
                (host, token)
            } else if farming_fixture {
                let (host, token, profile) =
                    crate::farming_smoke::prepare(host_config).map_err(std::io::Error::other)?;
                survival_profile = Some(profile);
                (host, token)
            } else if equipment_fixture {
                let (host, token, profile) =
                    crate::equipment_smoke::prepare(host_config).map_err(std::io::Error::other)?;
                survival_profile = Some(profile);
                (host, token)
            } else if pet_fixture {
                let (host, token, profile) = crate::pet_commands_smoke::prepare(host_config)
                    .map_err(std::io::Error::other)?;
                survival_profile = Some(profile);
                (host, token)
            } else if riding_fixture {
                let (host, token, profile) =
                    crate::riding_smoke::prepare(host_config).map_err(std::io::Error::other)?;
                survival_profile = Some(profile);
                (host, token)
            } else if breeding_fixture {
                let (host, token, profile) =
                    crate::breeding_smoke::prepare(host_config).map_err(std::io::Error::other)?;
                survival_profile = Some(profile);
                (host, token)
            } else if taming_fixture {
                let (host, token, profile) =
                    crate::taming_smoke::prepare(host_config).map_err(std::io::Error::other)?;
                survival_profile = Some(profile);
                (host, token)
            } else if effects_fixture {
                let (host, token, profile) = crate::status_effect_smoke::prepare(host_config)
                    .map_err(std::io::Error::other)?;
                survival_profile = Some(profile);
                (host, token)
            } else if survival_fixture {
                let (host, token, profile) =
                    crate::survival_smoke::prepare(host_config).map_err(std::io::Error::other)?;
                survival_profile = Some(profile);
                (host, token)
            } else if food_fixture {
                let (host, token, profile) =
                    crate::food_smoke::prepare(host_config).map_err(std::io::Error::other)?;
                survival_profile = Some(profile);
                (host, token)
            } else if furnace_gameplay_fixture {
                let (host, token, profile) =
                    crate::furnace_smoke::prepare(host_config).map_err(std::io::Error::other)?;
                survival_profile = Some(profile);
                (host, token)
            } else {
                if let Some(save) = benchmark_save.as_ref() {
                    initialize_benchmark_save(save, &host_config.world)
                        .map_err(std::io::Error::other)?;
                    terra_server::IntegratedServer::start_with_save(
                        host_config,
                        Some(save.directory.clone()),
                    )
                    .map_err(std::io::Error::other)?
                } else {
                    terra_server::IntegratedServer::start(host_config)
                        .map_err(std::io::Error::other)?
                }
            };
            (Some(server), Some(token))
        }
    } else {
        (None, None)
    };
    let render_diagnostics = std::env::var("TERRA_RENDER_DIAGNOSTICS")
        .is_ok_and(|value| matches!(value.as_str(), "1" | "true"));
    let mut gpu_settings = WgpuSettings {
        memory_hints: crate::memory_hints::options(
            std::env::args().any(|argument| argument.starts_with("--smoke-test")),
            std::env::var("TERRA_WGPU_MEMORY_BLOCK_MIB").ok().as_deref(),
        )?,
        ..default()
    };
    let memory_hints = gpu_settings.memory_hints.clone();
    if render_diagnostics {
        // Functionality enables only features supported by the chosen adapter,
        // including timestamps/statistics, without requiring unsupported features.
        gpu_settings.priority = WgpuSettingsPriority::Functionality;
    }
    let mut app = App::new();
    if fiverr_grass_fixture {
        crate::fiverr_grass_smoke::register_source(&mut app).map_err(std::io::Error::other)?;
    }
    if food_fixture {
        app.insert_resource(terra_ui::menu::MenuPaths {
            configs: crate::food_smoke::menu_directory(),
            ..default()
        });
    }
    if furnace_gameplay_fixture {
        app.insert_resource(terra_ui::menu::MenuPaths {
            configs: crate::furnace_smoke::menu_directory(),
            ..default()
        });
    }
    if benchmark_ui {
        app.insert_resource(terra_ui::menu::MenuPaths {
            configs: directory.clone(),
            ..default()
        })
        .init_resource::<BenchmarkUi>()
        .add_systems(Last, benchmark_ui_update);
    }
    if let Some(graphics) = &benchmark_graphics {
        app.insert_resource(graphics.clone())
            .add_systems(PostStartup, benchmark_window);
    }
    if let Some(resolution) = BenchmarkResolution::requested(benchmark, benchmark_graphics.as_ref())
    {
        app.insert_resource(resolution);
    }
    if let Some(scenario) = benchmark_scenario {
        app.insert_resource(scenario).add_systems(
            Update,
            benchmark_scenario_update
                .after(terra_player::PlayerSet::Movement)
                .before(terra_player::PlayerSet::Targeting),
        );
    }
    app.add_plugins(TerraUiPlugin);
    if let Some(present_mode) = presentation_override {
        app.insert_resource(terra_ui::PresentationOverride(present_mode));
    }
    app.add_plugins(
        DefaultPlugins
            .set(TaskPoolPlugin { task_pool_options })
            .set(RenderPlugin {
                render_creation: RenderCreation::Automatic(Box::new(gpu_settings)),
                ..default()
            })
            .set(LogPlugin {
                filter: effective_log_filter(&client.log_filter),
                ..default()
            })
            .set(AssetPlugin {
                file_path: terra_core::asset_directory().to_string_lossy().into_owned(),
                ..default()
            })
            .set(WindowPlugin {
                primary_window: Some(Window {
                    title: "TERAMIC".into(),
                    resolution: (client.width, client.height).into(),
                    present_mode: if client.vsync {
                        PresentMode::AutoVsync
                    } else {
                        PresentMode::AutoNoVsync
                    },
                    mode: benchmark_graphics
                        .as_ref()
                        .map_or(bevy::window::WindowMode::Windowed, |graphics| {
                            benchmark_window_mode(graphics.window_mode)
                        }),
                    ..default()
                }),
                ..default()
            }),
    )
    .add_plugins(TerraCorePlugin(config))
    .add_plugins((
        TerraWorldPlugin {
            mode: WorldMode::Client,
        },
        TerraPhysicsPlugin,
        TerraPlayerPlugin,
        TerraRenderPlugin,
        crate::water_audio::WaterAudioPlugin,
        crate::crusher_audio::CrusherAudioPlugin,
    ))
    .add_systems(
        Update,
        (
            sync_block_previews,
            sync_workshop_preview,
            sync_character_preview,
            sync_pet_portrait,
            preview_demand::sync,
        ),
    );
    crate::memory_hints::install_report(&mut app, &memory_hints);
    if compute_thread_cap != bevy::app::TaskPoolOptions::default().compute.max_threads {
        tracing::info!(
            compute_thread_cap,
            compute_threads = bevy::tasks::ComputeTaskPool::get().thread_num(),
            async_threads = bevy::tasks::AsyncComputeTaskPool::get().thread_num(),
            io_threads = bevy::tasks::IoTaskPool::get().thread_num(),
            "Client worker budget; smoke overrides are isolated"
        );
    }
    crate::creature_audio::install(&mut app);
    if fixture && std::env::var("TERRA_FOLIAGE_REFERENCE").is_ok_and(|value| value == "1") {
        app.add_plugins(terra_render::FoliageReferenceBenchmark);
        tracing::info!(
            "Render comparison only: opaque still foliage reference; logical world and props unchanged"
        );
    }
    if std::env::args().any(|arg| arg == "--smoke-test-benchmark") {
        app.insert_resource(terra_player::ControllerMode::Walk);
    }
    if frontend || token.is_some() {
        app.add_plugins(TerraNetworkPlugin(NetworkRole::Client));
    }
    if !frontend
        && (connect_address.is_some()
            || benchmark_save
                .as_ref()
                .is_some_and(|save| save.entry.is_some()))
    {
        // Existing-world benchmarks use the saved player's identity and location,
        // including train ownership, rather than measuring an unrelated spawn.
        let profile = terra_core::PlayerProfile::load_for_client(&profile_path)?;
        app.insert_resource(profile);
    }
    if let Some(profile) = survival_profile {
        app.insert_resource(profile);
    }
    if let Some(token) = token {
        if frontend
            && app
                .world()
                .resource::<terra_render::RenderPreviewStartup>()
                .required
        {
            app.insert_resource(crate::sessions::StartupLaunch(token));
        } else {
            app.insert_resource(terra_network::client::ClientLaunch { token });
        }
        if frontend {
            let mut menu = app.world_mut().resource_mut::<terra_ui::menu::MenuState>();
            menu.page = terra_ui::menu::MenuPage::Loading;
            menu.status = "Connecting to server ...".into();
            menu.session_active = true;
        }
    }
    if let Some(server) = integrated {
        app.insert_resource(server);
    }
    if frontend {
        app.world_mut()
            .resource_mut::<terra_ui::menu::MenuState>()
            .error = startup_errors.join("\n");
        crate::sessions::install(&mut app);
        startup_ui::install(&mut app);
        if let Some(address) = connect_address {
            let mut menu = app.world_mut().resource_mut::<terra_ui::menu::MenuState>();
            menu.page = terra_ui::menu::MenuPage::Loading;
            menu.status = "Connecting to server ...".into();
            app.world_mut()
                .write_message(terra_ui::menu::SessionRequest::JoinAddress(address));
        }
    }

    if render_diagnostics {
        app.add_plugins(TerraRenderDiagnosticsPlugin);
    }
    if std::env::var_os("TERRA_BENCH_SECONDS").is_some()
        && std::env::args().any(|arg| arg.starts_with("--smoke-test"))
    {
        // Benchmark throughput must not depend on desktop focus or background throttling.
        app.insert_resource(bevy::winit::WinitSettings::continuous());
    }

    if !crate::saved_construction_smoke::requested() {
        crate::smoke_test::install(&mut app);
    }
    crate::saved_construction_smoke::install(&mut app).map_err(std::io::Error::other)?;
    crate::inventory_benchmark::install(&mut app)?;
    crate::performance_soak::install(&mut app);
    crate::main_diagnostics::install(&mut app);
    crate::performance_route::install(&mut app)?;
    crate::performance_edits::install(&mut app)?;
    crate::physics_smoke::install(&mut app);
    crate::settings_smoke::install(&mut app);
    crate::appearance_smoke::install(&mut app);
    crate::botanical_smoke::install(&mut app);
    crate::flower_wither_smoke::install(&mut app);
    crate::fiverr_grass_smoke::install(&mut app);
    crate::hud_smoke::install(&mut app);
    crate::status_effect_smoke::install(&mut app);
    crate::taming_smoke::install(&mut app);
    crate::breeding_smoke::install(&mut app);
    crate::riding_smoke::install(&mut app);
    crate::pet_commands_smoke::install(&mut app);
    crate::waterfall_smoke::install(&mut app);
    crate::tree_collapse_smoke::install(&mut app);
    crate::village_smoke::install(&mut app);
    crate::equipment_smoke::install(&mut app);
    crate::train_audio::install(&mut app);
    crate::crusher_smoke::install(&mut app);
    crate::mixer_smoke::install(&mut app);
    crate::rgb_logistics_smoke::install(&mut app);
    crate::alchemy_smoke::install(&mut app);
    crate::hydraulics_smoke::install(&mut app);
    crate::kinetic_smoke::install(&mut app);
    crate::farming_smoke::install(&mut app);
    crate::reef_smoke::install(&mut app);
    crate::ecology_smoke::install(&mut app);
    crate::ore_smoke::install(&mut app);
    crate::geology_smoke::install(&mut app);
    crate::cave_showcase::install(&mut app);
    crate::cave_structures_smoke::install(&mut app);
    crate::creature_smoke::install(&mut app);
    crate::spider_smoke::install(&mut app);
    crate::web_constructions_smoke::install(&mut app);
    crate::window_gallery_smoke::install(&mut app);
    crate::stalker_smoke::install(&mut app);
    crate::mob_health_smoke::install(&mut app);
    crate::track_smoke::install(&mut app);
    crate::surface_smoke::install(&mut app);
    crate::network_smoke::install(&mut app);
    crate::chat_smoke::install(&mut app);
    crate::minimap_smoke::install(&mut app);
    if std::env::var_os("TERRA_CRAFTING_SMOKE").is_some() {
        crate::crafting_smoke::install(&mut app);
    } else {
        crate::inventory_smoke::install(&mut app);
    }
    crate::camera_smoke::install(&mut app);
    crate::drop_smoke::install(&mut app);
    crate::fluid_source_smoke::install(&mut app);
    crate::menu_smoke::install(&mut app);
    crate::character_live::install(&mut app);
    crate::loading_smoke::install(&mut app);
    crate::systems_smoke::install(&mut app);
    crate::hydrology_smoke::install(&mut app);
    crate::foliage_smoke::install(&mut app);
    crate::fluid_smoke::install(&mut app);
    crate::building_smoke::install(&mut app);
    crate::materials_smoke::install(&mut app);
    crate::biome_smoke::install(&mut app);
    crate::daylight_smoke::install(&mut app);
    crate::lantern_smoke::install(&mut app);
    crate::season_smoke::install(&mut app);
    crate::survival_smoke::install(&mut app);
    crate::food_smoke::install(&mut app)?;
    crate::furnace_smoke::install(&mut app)?;
    crate::avatar_smoke::install(&mut app)?;
    crate::hand_smoke::install(&mut app);
    crate::runtime_health::install(&mut app);
    Ok(app)
}

#[derive(Resource, Default)]
pub(crate) struct BenchmarkUi {
    pub(crate) ready: bool,
    resume_sent: bool,
}

struct BenchmarkSave {
    directory: std::path::PathBuf,
    entry: Option<terra_storage::WorldEntry>,
}

fn empty_benchmark_directory(directory: &std::path::Path) -> Result<(), String> {
    match std::fs::symlink_metadata(directory) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
            return Err("The benchmark save directory must be a real directory".into());
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error.to_string()),
        Ok(_) => {}
    }
    if std::fs::read_dir(directory)
        .map_err(|error| error.to_string())?
        .next()
        .transpose()
        .map_err(|error| error.to_string())?
        .is_some()
    {
        return Err(
            "A benchmark save without TERRA_BENCH_WORLD_ID requires a fresh empty directory".into(),
        );
    }
    Ok(())
}

fn resolve_benchmark_save(
    integrated: bool,
    directory: Option<&std::path::Path>,
    world_id: Option<&str>,
    library: &terra_storage::WorldLibrary,
) -> Result<Option<BenchmarkSave>, String> {
    if directory.is_none() && world_id.is_none() {
        return Ok(None);
    }
    if !integrated {
        return Err(
            "Persistent benchmarks require --smoke-test-benchmark and TERRA_BENCH_MODE=integrated"
                .into(),
        );
    }
    let directory = directory.ok_or("TERRA_BENCH_WORLD_ID requires TERRA_BENCH_SAVE_DIR")?;
    if !directory.is_absolute() {
        return Err("TERRA_BENCH_SAVE_DIR must be an absolute isolated path".into());
    }
    let entry = if let Some(value) = world_id {
        let parsed = if let Some(hex) = value.strip_prefix("0x") {
            u64::from_str_radix(hex, 16)
        } else {
            value.parse::<u64>()
        }
        .map_err(|_| "TERRA_BENCH_WORLD_ID must be a decimal ID or a hexadecimal 0x ID")?;
        let id = terra_core::WorldId(parsed);
        let entry = library.load(id)?;
        entry.compatible()?;
        let selected = std::fs::canonicalize(directory).map_err(|error| error.to_string())?;
        let saved = std::fs::canonicalize(library.path(id)).map_err(|error| error.to_string())?;
        let root = std::fs::canonicalize(&library.root).map_err(|error| error.to_string())?;
        if selected != saved || !saved.starts_with(root) {
            return Err("TERRA_BENCH_SAVE_DIR must resolve to the selected world inside the isolated TERRA_WORLD_DIR".into());
        }
        Some(entry)
    } else {
        empty_benchmark_directory(directory)?;
        None
    };
    Ok(Some(BenchmarkSave {
        directory: directory.into(),
        entry,
    }))
}

fn initialize_benchmark_save(save: &BenchmarkSave, world: &WorldConfig) -> Result<(), String> {
    world.validate().map_err(|error| error.to_string())?;
    if let Some(entry) = &save.entry {
        if &entry.manifest.world != world {
            return Err(
                "The benchmark generator configuration does not match the selected saved world"
                    .into(),
            );
        }
        return Ok(());
    }
    empty_benchmark_directory(&save.directory)?;
    std::fs::create_dir_all(&save.directory).map_err(|error| error.to_string())?;
    terra_core::save_toml(
        &terra_storage::WorldManifest::new(world.clone()),
        save.directory.join("world.toml"),
    )
    .map_err(|error| error.to_string())
}

fn benchmark_presentation(
    benchmark: bool,
    requested: Option<&str>,
) -> Result<Option<PresentMode>, &'static str> {
    if !benchmark {
        return Ok(None);
    }
    match requested {
        None => Ok(None),
        Some("auto") => Ok(Some(PresentMode::AutoNoVsync)),
        Some("mailbox") => Ok(Some(PresentMode::Mailbox)),
        Some(_) => Err("TERRA_BENCH_PRESENT_MODE must be auto or mailbox"),
    }
}

fn validate_benchmark_ui(
    requested: bool,
    benchmark: bool,
    integrated: bool,
) -> Result<bool, &'static str> {
    if requested && !(benchmark && integrated) {
        return Err(
            "TERRA_BENCH_UI=1 requires --smoke-test-benchmark and TERRA_BENCH_MODE=integrated",
        );
    }
    Ok(requested)
}

fn benchmark_ui_update(world: &mut World) {
    if world.resource::<BenchmarkUi>().ready
        || !world
            .resource::<terra_network::client::ClientSession>()
            .ready
        || !world.resource::<terra_render::RenderStatus>().ready
    {
        return;
    }
    if world.resource::<terra_ui::menu::MenuState>().page != terra_ui::menu::MenuPage::Playing {
        if !world.resource::<BenchmarkUi>().resume_sent {
            world
                .resource_mut::<terra_ui::menu::MenuState>()
                .session_active = true;
            world.write_message(terra_ui::menu::MenuAction::Command("resume".into()));
            world.resource_mut::<BenchmarkUi>().resume_sent = true;
        }
        return;
    }
    let minimap_enabled = world
        .resource::<terra_core::GameplayConfig>()
        .minimap
        .enabled;
    let survival = world
        .resource::<terra_network::client::ClientSession>()
        .game_mode
        == terra_core::GameMode::Survival;
    if !world.resource::<terra_ui::InventoryView>().visible
        || (minimap_enabled && !world.resource::<terra_ui::minimap::MiniMapView>().visible)
        || (survival && !world.resource::<terra_ui::SurvivalView>().visible)
    {
        return;
    }
    let mut hotbar_ready = false;
    let mut map_ready = !minimap_enabled;
    let mut stats_ready = !survival;
    for (id, _node, image) in world
        .query::<(&terra_ui::ElementId, &ComputedNode, Option<&ImageNode>)>()
        .iter(world)
        .filter(|(_, node, _)| !node.is_empty())
    {
        match id.0.as_str() {
            "inventory-hud-hotbar" => hotbar_ready = true,
            "survival-hud" => stats_ready = true,
            "map-surface" if minimap_enabled => {
                map_ready = image
                    .and_then(|image| world.resource::<Assets<Image>>().get(&image.image))
                    .filter(|image| image.texture_descriptor.size.width == 320)
                    .and_then(|image| image.data.as_ref())
                    .is_some_and(|pixels| {
                        pixels
                            .chunks_exact(4)
                            .filter(|pixel| {
                                pixel[0].max(pixel[1]).max(pixel[2]) > 42 && pixel[3] == 255
                            })
                            .take(100)
                            .count()
                            == 100
                    });
            }
            _ => {}
        }
    }
    if hotbar_ready && map_ready && stats_ready {
        world.resource_mut::<BenchmarkUi>().ready = true;
        info!(
            minimap_enabled,
            survival, "Integrated benchmark real TiltUI HUD ready"
        );
    }
}

fn load_benchmark_graphics(
    directory: &std::path::Path,
    client: &mut ClientConfig,
    enabled: bool,
) -> Result<Option<GraphicsConfig>, terra_core::ConfigError> {
    if !enabled {
        return Ok(None);
    }
    let graphics = GraphicsConfig::load(directory.join("graphics.toml"))?;
    graphics.apply_to(client);
    Ok(Some(graphics))
}

fn benchmark_window_mode(setting: WindowModeSetting) -> bevy::window::WindowMode {
    use bevy::window::{MonitorSelection, VideoModeSelection, WindowMode};
    match setting {
        WindowModeSetting::Windowed => WindowMode::Windowed,
        WindowModeSetting::Borderless => {
            WindowMode::BorderlessFullscreen(MonitorSelection::Primary)
        }
        WindowModeSetting::Fullscreen => {
            WindowMode::Fullscreen(MonitorSelection::Primary, VideoModeSelection::Current)
        }
    }
}

/// Preserve the requested physical render size independently of later window
/// manager constraints. Ordinary GUI fixtures are allowed to resize their UI.
#[derive(Resource, Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct BenchmarkResolution {
    pub(crate) physical: [u32; 2],
}

impl BenchmarkResolution {
    fn requested(benchmark: bool, graphics: Option<&GraphicsConfig>) -> Option<Self> {
        if !benchmark {
            return None;
        }
        graphics.map(|graphics| Self {
            physical: [graphics.width, graphics.height],
        })
    }
}

fn benchmark_window(
    graphics: Res<GraphicsConfig>,
    mut windows: Query<&mut Window, With<bevy::window::PrimaryWindow>>,
) {
    for mut window in &mut windows {
        window
            .resolution
            .set_physical_resolution(graphics.width, graphics.height);
    }
}

fn effective_log_filter(custom: &str) -> String {
    format!("{}{custom}", bevy::log::DEFAULT_FILTER)
}

#[derive(Resource)]
pub(crate) struct BenchmarkScenario {
    weather: Option<(terra_core::WeatherPattern, u8, String)>,
    eye: Option<Vec3>,
    target: Option<Vec3>,
    submitted: bool,
    pub(crate) ready: bool,
}

impl BenchmarkScenario {
    fn parse(
        weather: Option<&str>,
        eye: Option<&str>,
        target: Option<&str>,
    ) -> Result<Option<Self>, String> {
        let weather = weather.map(|weather| {
            let command = format!("/weather set {}", weather.trim());
            match terra_network::protocol::commands::parse(&command) {
                Ok(Some(terra_network::protocol::commands::Command::Weather(pattern, level))) => Ok((pattern, level, command)),
                _ => Err("TERRA_BENCH_WEATHER needs clear, rain, snow [1-5], storm, thunder or thunderstorm".to_string()),
            }
        }).transpose()?;
        let eye = eye.map(parse_benchmark_point).transpose()?;
        let target = target.map(parse_benchmark_point).transpose()?;
        if target.is_some() && eye.is_none() {
            return Err("TERRA_BENCH_LOOK_AT requires TERRA_BENCH_CAMERA".into());
        }
        if let (Some(eye), Some(target)) = (eye, target)
            && eye.distance_squared(target) < 0.0001
        {
            return Err("Benchmark camera and look-at point must be distinct".into());
        }
        Ok((weather.is_some() || eye.is_some()).then_some(Self {
            weather,
            eye,
            target,
            submitted: false,
            ready: false,
        }))
    }
}

fn parse_benchmark_point(value: &str) -> Result<Vec3, String> {
    let values: Vec<_> = value
        .split(|c: char| c == ',' || c.is_ascii_whitespace())
        .filter(|part| !part.is_empty())
        .map(str::parse::<f32>)
        .collect();
    let [Ok(x), Ok(y), Ok(z)] = values.as_slice() else {
        return Err("Benchmark coordinates need three finite comma/space-separated numbers".into());
    };
    let point = Vec3::new(*x, *y, *z);
    if !point.is_finite()
        || point.x.abs() > 999_000.0
        || point.z.abs() > 999_000.0
        || !(-127.0..=355.0).contains(&point.y)
    {
        return Err("Benchmark coordinates must be inside world bounds (Y -127..355)".into());
    }
    Ok(point)
}

#[derive(bevy::ecs::system::SystemParam)]
struct BenchmarkWorld<'w, 's> {
    session: Option<Res<'w, terra_network::client::ClientSession>>,
    ui: Option<Res<'w, BenchmarkUi>>,
    config: Res<'w, GameConfig>,
    chat: Option<ResMut<'w, terra_network::client::ClientChatCommands>>,
    weather: ResMut<'w, terra_core::WorldWeather>,
    rendered: Res<'w, terra_render::WeatherRenderStatus>,
    mode: ResMut<'w, terra_player::ControllerMode>,
    focus: ResMut<'w, terra_world::streaming::ChunkFocus>,
    cameras: Query<'w, 's, &'static mut Transform, With<terra_player::TerraCamera>>,
}

fn benchmark_scenario_update(
    mut scenario: ResMut<BenchmarkScenario>,
    mut world: BenchmarkWorld,
    route: Option<Res<crate::performance_route::Route>>,
    edits: Option<Res<crate::performance_edits::Edits>>,
) {
    if route.is_some_and(|route| route.started()) || edits.is_some_and(|edits| edits.started()) {
        return;
    }
    use terra_network::client::ChatCommand;
    let networked = world.session.is_some();
    if world.session.as_ref().is_some_and(|session| !session.ready) {
        return;
    }
    // Do not load a distant destination alongside the initial HUD/spawn region.
    // This uses the same real command once the optional integrated HUD is ready.
    if !scenario.submitted && networked && world.ui.as_ref().is_some_and(|ui| !ui.ready) {
        return;
    }
    if !scenario.submitted {
        if networked {
            let Some(chat) = world.chat.as_mut() else {
                return;
            };
            if let Some((_, _, command)) = &scenario.weather {
                chat.0.push_back(ChatCommand::Send(command.clone()));
            }
            if let Some(eye) = scenario.eye {
                let feet = eye
                    - Vec3::Y
                        * (terra_player::PLAYER_EYE_OFFSET
                            + terra_physics::character::PLAYER_HALF_HEIGHT
                            + 0.02);
                chat.0.push_back(ChatCommand::Send(format!(
                    "/tp {} {} {}",
                    feet.x, feet.y, feet.z
                )));
            }
        } else if let Some((pattern, level, _)) = &scenario.weather {
            world.weather.set_pattern(*pattern);
            world.weather.snow_level = *level;
            let transition = world.weather.transition_seconds;
            world.weather.advance(transition, world.config.world.seed.0);
            world.weather.wetness = if matches!(
                pattern,
                terra_core::WeatherPattern::Rain
                    | terra_core::WeatherPattern::Storm
                    | terra_core::WeatherPattern::Thunder
                    | terra_core::WeatherPattern::Thunderstorm
            ) {
                1.0
            } else {
                0.0
            };
        }
        tracing::info!(networked, weather=?scenario.weather, eye=?scenario.eye, target=?scenario.target, "Explicit benchmark scenario submitted (network requests use authoritative chat commands)");
        scenario.submitted = true;
    }
    if let Some(eye) = scenario.eye {
        *world.mode = terra_player::ControllerMode::Fly;
        for mut camera in &mut world.cameras {
            if !networked {
                camera.translation = eye;
                world.focus.0 = eye.to_array().map(f64::from);
            }
            let target = scenario
                .target
                .unwrap_or(eye + Vec3::new(-4.0, -3.5, -12.0));
            let position = camera.translation;
            if position.distance_squared(target) > 0.0001 {
                camera.look_at(target, Vec3::Y);
            }
        }
    }
    let position_ready = scenario.eye.is_none_or(|eye| {
        world
            .cameras
            .iter()
            .any(|camera| camera.translation.distance_squared(eye) < 1.0)
    });
    let weather_ready = scenario.weather.as_ref().is_none_or(|(pattern, level, _)| {
        world.weather.pattern == *pattern
            && world.weather.snow_level == *level
            && world.weather.elapsed_seconds + 0.01 >= world.weather.transition_seconds
            && match pattern {
                terra_core::WeatherPattern::Clear => {
                    world.rendered.rain.max(world.rendered.snow) < 0.01
                }
                terra_core::WeatherPattern::Snow | terra_core::WeatherPattern::Blizzard => {
                    world.rendered.snow > 0.5 && world.rendered.quads > 0
                }
                _ => world.rendered.rain > 0.5 && world.rendered.quads > 0,
            }
    });
    scenario.ready = position_ready && weather_ready;
}

fn sync_pet_portrait(
    view: Res<terra_ui::PetMenuView>,
    mut rendered: ResMut<terra_render::creatures::RenderPetPortrait>,
    mut ui: ResMut<terra_ui::PetPortrait>,
) {
    let request = view.visible.then_some(view.id);
    if rendered.request != request {
        rendered.request = request;
    }
    let ready = rendered.ready && rendered.id == request && request.is_some();
    if ui.id != rendered.id || ui.image != rendered.image || ui.ready != ready {
        ui.id = rendered.id;
        ui.image = rendered.image.clone();
        ui.ready = ready;
    }
}

fn sync_workshop_preview(
    rendered: Res<terra_render::WorkshopPreviewImage>,
    mut nodes: Query<(&tilt_ui::ElementId, &mut bevy::ui::widget::ImageNode)>,
) {
    if !rendered.ready {
        return;
    }
    for (id, mut node) in &mut nodes {
        if id.0 == "train-model-preview" && node.image != rendered.image {
            node.image = rendered.image.clone();
        }
    }
}

fn sync_character_preview(
    mut commands: Commands,
    selected: Option<Res<terra_ui::CharacterPreviewSelection>>,
    current: Option<Res<terra_render::RenderCharacterPreviewSelection>>,
    rendered: Res<terra_render::RenderCharacterPreviewImage>,
    mut ui: ResMut<terra_ui::CharacterPreviewImage>,
) {
    if let Some(selected) = selected {
        let next = terra_render::RenderCharacterPreviewSelection {
            gender: selected.gender,
            appearance: selected.appearance,
            rotation: selected.rotation,
        };
        if current.as_deref() != Some(&next) {
            commands.insert_resource(next);
        }
    } else if current.is_some() {
        commands.remove_resource::<terra_render::RenderCharacterPreviewSelection>();
    }
    if ui.image != rendered.image
        || ui.ready != rendered.ready
        || ui.hairstyles != rendered.hairstyles
    {
        ui.hairstyles = rendered.hairstyles.clone();
        ui.image = rendered.image.clone();
        ui.ready = rendered.ready;
    }
}

fn sync_block_previews(
    rendered: Res<terra_render::RenderBlockPreviews>,
    mut ui: ResMut<terra_ui::BlockPreviewImages>,
    (player, mut player_ui): (
        Res<terra_render::PlayerPreviewImage>,
        ResMut<terra_ui::PlayerPreviewImage>,
    ),
    (props, mut props_ui): (
        Res<terra_render::RenderPropPreviews>,
        ResMut<terra_ui::PropPreviewImages>,
    ),
    (items, mut items_ui): (
        Res<terra_render::RenderItemPreviews>,
        ResMut<terra_ui::ItemPreviewImages>,
    ),
    (time, mut blink): (Res<Time>, Local<terra_render::AvatarBlink>),
    (portrait_rotation, live_portrait, mut live_selection): (
        Res<terra_ui::InventoryPortraitRotation>,
        Res<terra_render::RenderInventoryPortraitImage>,
        ResMut<terra_render::RenderInventoryPortraitSelection>,
    ),
) {
    if rendered.is_changed() && ui.0 != rendered.images {
        ui.0.clone_from(&rendered.images);
    }
    if items.is_changed() && items_ui.0 != items.images {
        items_ui.0.clone_from(&items.images);
    }
    let selected = terra_render::RenderInventoryPortraitSelection {
        active: portrait_rotation.active,
        rotation: portrait_rotation.rotation,
    };
    if *live_selection != selected {
        *live_selection = selected;
    }
    if live_portrait.ready && selected.active {
        if player_ui.0 != live_portrait.image {
            player_ui.0 = live_portrait.image.clone();
        }
    } else if player.ready {
        let portrait = if blink.advance(time.delta_secs()) > 0.3 {
            &player.closed_image
        } else {
            &player.image
        };
        if player_ui.0 != *portrait {
            player_ui.0 = portrait.clone();
        }
    }
    if props.is_changed() && props_ui.0 != props.images {
        props_ui.0.clone_from(&props.images);
    }
}

fn load_config<T: Config + Default>(
    path: &std::path::Path,
    recover: bool,
    errors: &mut Vec<String>,
) -> Result<T, terra_core::ConfigError> {
    match T::load(path) {
        Ok(config) => Ok(config),
        Err(error) if recover => {
            errors.push(format!(
                "{}: {error}. Using defaults without overwriting the file.",
                path.display()
            ));
            Ok(T::default())
        }
        Err(error) => Err(error),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn benchmark_save_root(label: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "terra-benchmark-save-{label}-{}",
            terra_core::PlayerUuid::new_v4()
        ))
    }

    #[test]
    fn persistent_benchmark_settings_are_explicit_validated_and_do_not_create_files_when_off() {
        let root = benchmark_save_root("disabled");
        let library = terra_storage::WorldLibrary { root: root.clone() };
        assert!(
            resolve_benchmark_save(false, None, None, &library)
                .unwrap()
                .is_none()
        );
        assert!(resolve_benchmark_save(false, Some(&root), None, &library).is_err());
        assert!(resolve_benchmark_save(false, None, Some("1"), &library).is_err());
        assert!(resolve_benchmark_save(true, None, Some("1"), &library).is_err());
        assert!(
            resolve_benchmark_save(
                true,
                Some(std::path::Path::new("relative-save")),
                None,
                &library
            )
            .is_err()
        );
        assert!(resolve_benchmark_save(true, Some(&root), Some("not-an-id"), &library).is_err());
        assert!(!root.exists());
    }

    #[test]
    fn a_fresh_persistent_benchmark_initializes_the_existing_manifest_and_save_format() {
        let root = benchmark_save_root("fresh");
        let library = terra_storage::WorldLibrary {
            root: root.join("library"),
        };
        let selected = resolve_benchmark_save(true, Some(&root), None, &library)
            .unwrap()
            .unwrap();
        assert!(!root.exists());
        let world = WorldConfig::default();
        initialize_benchmark_save(&selected, &world).unwrap();
        assert!(root.join("world.toml").is_file());
        assert!(terra_storage::load_edits(&root, world.id).is_ok());
        assert!(
            terra_storage::load_edits(&root, terra_core::WorldId(world.id.0.wrapping_add(1)))
                .is_err()
        );
        assert!(resolve_benchmark_save(true, Some(&root), None, &library).is_err());
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn an_existing_persistent_benchmark_keeps_the_complete_saved_world_and_game_mode() {
        let root = benchmark_save_root("existing");
        let library = terra_storage::WorldLibrary { root: root.clone() };
        let entry = library
            .create(
                "Isolated saved world",
                Some(8675309),
                terra_core::GameMode::Survival,
                &WorldConfig::default(),
            )
            .unwrap();
        let directory = library.path(entry.manifest.world.id);
        let selected = resolve_benchmark_save(
            true,
            Some(&directory),
            Some(&format!("0x{:x}", entry.manifest.world.id.0)),
            &library,
        )
        .unwrap()
        .unwrap();
        let saved = selected.entry.as_ref().unwrap();
        assert_eq!(saved.manifest.world, entry.manifest.world);
        assert_eq!(saved.mode, terra_core::GameMode::Survival);
        let before = std::fs::read(directory.join("world.toml")).unwrap();
        initialize_benchmark_save(&selected, &entry.manifest.world).unwrap();
        let mut wrong = entry.manifest.world.clone();
        wrong.seed = terra_core::WorldSeed(wrong.seed.0.wrapping_add(1));
        assert!(initialize_benchmark_save(&selected, &wrong).is_err());
        assert_eq!(std::fs::read(directory.join("world.toml")).unwrap(), before);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn persistent_benchmarks_reject_mismatched_paths_and_incompatible_world_entries() {
        let root = benchmark_save_root("invalid");
        let library = terra_storage::WorldLibrary {
            root: root.join("library"),
        };
        let mut entry = library
            .create(
                "Saved world",
                Some(42042),
                terra_core::GameMode::Creative,
                &WorldConfig::default(),
            )
            .unwrap();
        let id = entry.manifest.world.id.0.to_string();
        let directory = library.path(entry.manifest.world.id);
        let wrong = root.join("other");
        std::fs::create_dir_all(&wrong).unwrap();
        assert!(resolve_benchmark_save(true, Some(&wrong), Some(&id), &library).is_err());
        entry.game_version = "unsupported-version".into();
        terra_core::save_toml(&entry, directory.join("world.toml")).unwrap();
        assert!(resolve_benchmark_save(true, Some(&directory), Some(&id), &library).is_err());
        assert!(std::fs::read_dir(&wrong).unwrap().next().is_none());
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn benchmark_ui_requires_the_real_integrated_static_benchmark() {
        assert_eq!(validate_benchmark_ui(false, false, false), Ok(false));
        assert_eq!(validate_benchmark_ui(true, true, true), Ok(true));
        assert!(validate_benchmark_ui(true, true, false).is_err());
        assert!(validate_benchmark_ui(true, false, true).is_err());
        assert!(validate_benchmark_ui(true, false, false).is_err());
    }

    #[test]
    fn native_presentation_control_is_explicit_validated_and_inert_in_normal_gameplay() {
        assert_eq!(benchmark_presentation(false, Some("auto")), Ok(None));
        assert_eq!(benchmark_presentation(false, Some("invalid")), Ok(None));
        assert_eq!(benchmark_presentation(true, None), Ok(None));
        assert_eq!(
            benchmark_presentation(true, Some("auto")),
            Ok(Some(PresentMode::AutoNoVsync))
        );
        assert_eq!(
            benchmark_presentation(true, Some("mailbox")),
            Ok(Some(PresentMode::Mailbox))
        );
        assert!(benchmark_presentation(true, Some("invalid")).is_err());
    }

    #[test]
    fn benchmark_graphics_are_opt_in_and_keep_non_graphics_client_tuning() {
        let directory =
            std::env::temp_dir().join(format!("terra-benchmark-graphics-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&directory);
        std::fs::create_dir_all(&directory).unwrap();
        let mut client = ClientConfig {
            max_mesh_jobs: 3,
            ..default()
        };
        let original = client.clone();
        assert!(
            load_benchmark_graphics(&directory, &mut client, false)
                .unwrap()
                .is_none()
        );
        assert_eq!(client, original);
        assert!(load_benchmark_graphics(&directory, &mut client, true).is_err());
        let graphics = GraphicsConfig {
            width: 3840,
            height: 2160,
            render_distance: 20,
            lod_distance: 1.5,
            cave_quality: terra_core::CaveQuality::Ultra,
            shadow_quality: terra_core::ShadowQuality::High,
            weather: terra_core::WeatherKind::Rain,
            ..default()
        };
        graphics.save(directory.join("graphics.toml")).unwrap();
        assert_eq!(
            load_benchmark_graphics(&directory, &mut client, true).unwrap(),
            Some(graphics)
        );
        assert_eq!(
            (client.width, client.height, client.view_distance),
            (3840, 2160, 20)
        );
        assert_eq!(client.lod_near_distance, 144.0);
        assert_eq!(client.max_mesh_jobs, 3);
        assert_eq!(client.weather, terra_core::WeatherKind::Rain);
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn benchmark_window_uses_physical_not_dpi_scaled_resolution() {
        let mut app = App::new();
        app.insert_resource(GraphicsConfig {
            width: 3840,
            height: 2160,
            ..default()
        })
        .add_systems(Update, benchmark_window);
        let entity = app
            .world_mut()
            .spawn((
                Window {
                    resolution: bevy::window::WindowResolution::new(1280, 720)
                        .with_scale_factor_override(1.5),
                    ..default()
                },
                bevy::window::PrimaryWindow,
            ))
            .id();
        app.update();
        let window = app.world().get::<Window>(entity).unwrap();
        assert_eq!(
            (window.physical_width(), window.physical_height()),
            (3840, 2160)
        );
        assert_eq!(window.scale_factor(), 1.5);
    }

    #[test]
    fn initial_fullscreen_benchmark_selects_a_monitor_before_the_window_exists() {
        use bevy::window::{MonitorSelection, VideoModeSelection, WindowMode};
        assert_eq!(
            benchmark_window_mode(WindowModeSetting::Windowed),
            WindowMode::Windowed
        );
        assert_eq!(
            benchmark_window_mode(WindowModeSetting::Borderless),
            WindowMode::BorderlessFullscreen(MonitorSelection::Primary)
        );
        assert_eq!(
            benchmark_window_mode(WindowModeSetting::Fullscreen),
            WindowMode::Fullscreen(MonitorSelection::Primary, VideoModeSelection::Current)
        );
    }

    #[test]
    fn benchmark_resolution_contract_requires_the_explicit_benchmark_graphics() {
        let graphics = GraphicsConfig {
            width: 3840,
            height: 2124,
            ..default()
        };
        assert_eq!(BenchmarkResolution::requested(true, None), None);
        assert_eq!(BenchmarkResolution::requested(false, Some(&graphics)), None);
        assert_eq!(
            BenchmarkResolution::requested(true, Some(&graphics)),
            Some(BenchmarkResolution {
                physical: [3840, 2124]
            })
        );
    }

    #[test]
    fn benchmark_scenarios_are_explicit_validated_and_bounded() {
        assert!(
            BenchmarkScenario::parse(None, None, None)
                .unwrap()
                .is_none()
        );
        let scenario =
            BenchmarkScenario::parse(Some("snow 5"), Some("10, 24, -40"), Some("0 16 -60"))
                .unwrap()
                .unwrap();
        assert!(matches!(
            scenario.weather,
            Some((terra_core::WeatherPattern::Blizzard, 5, _))
        ));
        assert_eq!(scenario.eye, Some(Vec3::new(10.0, 24.0, -40.0)));
        for bad in ["NaN 0 0", "0 999 0", "1000001 4 0", "1 2"] {
            assert!(parse_benchmark_point(bad).is_err());
        }
        assert!(BenchmarkScenario::parse(Some("mist"), None, None).is_err());
        assert!(BenchmarkScenario::parse(None, None, Some("1 2 3")).is_err());
        assert!(BenchmarkScenario::parse(None, Some("1 2 3"), Some("1 2 3")).is_err());
    }

    #[test]
    fn user_log_directives_follow_the_upstream_noise_defaults() {
        let filter = effective_log_filter("info,calloop::loop_logic=trace");
        assert!(filter.starts_with(bevy::log::DEFAULT_FILTER));
        assert!(filter.contains("calloop::loop_logic=error"));
        assert!(filter.ends_with("info,calloop::loop_logic=trace"));
        assert!(!filter.contains("bevy_render::slab_allocator=off"));
    }
}
