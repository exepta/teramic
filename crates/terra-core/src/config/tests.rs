use super::*;

#[test]
fn cherry_ecology_is_saved_and_never_retroactively_enabled_in_old_worlds() {
    let legacy = WorldConfig::from_toml("seed=42042").unwrap();
    assert_eq!(legacy.ecology_revision, 0);
    let fresh = WorldConfig::default();
    assert_eq!(fresh.ecology_revision, 2);
    let serialized = toml::to_string(&fresh).unwrap();
    assert_eq!(WorldConfig::from_toml(&serialized).unwrap(), fresh);
    assert!(!fresh.same_terrain(&WorldConfig {
        ecology_revision: 0,
        ..fresh.clone()
    }));
    assert_eq!(
        WorldConfig::from_toml("ecology_revision=1")
            .unwrap()
            .ecology_revision,
        1
    );
    assert!(WorldConfig::from_toml("ecology_revision=3").is_err());
}

#[test]
fn waterfall_terrain_revision_is_saved_without_changing_legacy_world_contracts() {
    let legacy = WorldConfig::from_toml("seed=42042\nrivers=true").unwrap();
    assert_eq!(legacy.river_terrain_revision, 0);
    let fresh = WorldConfig::default();
    assert_eq!(fresh.river_terrain_revision, 1);
    assert_eq!(
        WorldConfig::from_toml(&toml::to_string(&fresh).unwrap()).unwrap(),
        fresh
    );
    assert!(!fresh.same_terrain(&WorldConfig {
        river_terrain_revision: 0,
        ..fresh.clone()
    }));
    for revision in 0..=1 {
        assert_eq!(
            WorldConfig::from_toml(&format!("river_terrain_revision={revision}"))
                .unwrap()
                .river_terrain_revision,
            revision
        );
    }
    assert!(WorldConfig::from_toml("river_terrain_revision=2").is_err());
    assert_eq!(
        WorldConfig::from_toml(include_str!("../../../../configs/world.toml"))
            .unwrap()
            .river_terrain_revision,
        1
    );
}

#[test]
fn ocean_contract_is_saved_without_retroactively_freezing_old_worlds() {
    let legacy = crate::WorldConfig::from_toml("seed = 42042").unwrap();
    assert_eq!(legacy.ocean_revision, 0);
    assert_eq!(legacy.cave_structure_revision, 0);
    let fresh = crate::WorldConfig::default();
    assert_eq!(fresh.ocean_revision, 1);
    assert_eq!(fresh.cave_structure_revision, 8);
    assert_eq!(
        crate::WorldConfig::from_toml(&toml::to_string(&fresh).unwrap()).unwrap(),
        fresh
    );
    assert!(crate::WorldConfig::from_toml("ocean_revision = 2").is_err());
    for revision in 0..=8 {
        assert_eq!(
            crate::WorldConfig::from_toml(&format!("cave_structure_revision={revision}"))
                .unwrap()
                .cave_structure_revision,
            revision
        );
    }
    assert!(crate::WorldConfig::from_toml("cave_structure_revision=9").is_err());
}

#[test]
fn structures_contract_is_explicit_for_fresh_worlds_and_absent_in_legacy_manifests() {
    let legacy = WorldConfig::from_toml("seed=42042").unwrap();
    assert_eq!(legacy.structures_revision, 0);
    let fresh = WorldConfig::default();
    assert_eq!(fresh.structures_revision, 5);
    assert_eq!(
        WorldConfig::from_toml(&toml::to_string(&fresh).unwrap()).unwrap(),
        fresh
    );
    assert!(!fresh.same_terrain(&WorldConfig {
        structures_revision: 0,
        ..fresh.clone()
    }));
    assert!(WorldConfig::from_toml("structures_revision=1").is_ok());
    assert!(WorldConfig::from_toml("structures_revision=2").is_ok());
    assert!(WorldConfig::from_toml("structures_revision=3").is_ok());
    assert!(WorldConfig::from_toml("structures_revision=4").is_ok());
    assert!(WorldConfig::from_toml("structures_revision=5").is_ok());
    assert!(WorldConfig::from_toml("structures_revision=6").is_err());
    assert_eq!(
        WorldConfig::from_toml(include_str!("../../../../configs/world.toml"))
            .unwrap()
            .structures_revision,
        5
    );
}

#[test]
fn saved_geology_policy_keeps_legacy_worlds_and_rejects_future_versions() {
    let legacy = crate::WorldConfig::from_toml("seed = 42042").unwrap();
    assert_eq!(legacy.geology_revision, 0);
    let new = crate::WorldConfig::default();
    assert_eq!(new.geology_revision, 4);
    assert_eq!(
        crate::WorldConfig::from_toml(&toml::to_string(&new).unwrap()).unwrap(),
        new
    );
    assert!(crate::WorldConfig::from_toml("geology_revision = 1").is_ok());
    assert!(crate::WorldConfig::from_toml("geology_revision = 2").is_ok());
    assert!(crate::WorldConfig::from_toml("geology_revision = 3").is_ok());
    assert!(crate::WorldConfig::from_toml("geology_revision = 4").is_ok());
    assert!(crate::WorldConfig::from_toml("geology_revision = 5").is_err());
}
use crate::WorldSeed;
#[test]
fn defaults_and_partial_toml() {
    GameConfig::default().validate().unwrap();
    let config = ClientConfig::from_toml("width = 1920").unwrap();
    assert_eq!(config.width, 1920);
    assert_eq!(config.height, 800);
    assert_eq!(config.fov_degrees, 45.0);
    assert_eq!(config.weather, WeatherKind::Clear);
    assert_eq!(config.weather_intensity, 1.0);
    assert_eq!(config.fly_speed, 20.0);
    assert_eq!(config.fly_boost_multiplier, 3.0);
    assert_eq!(config.fly_precision_multiplier, 0.25);
    assert_eq!(config.mouse_sensitivity, 0.0022);
    assert_eq!(config.view_distance, 12);
    assert_eq!(WorldConfig::from_toml("").unwrap().seed, WorldSeed(42042));
    let world = WorldConfig::from_toml("").unwrap();
    assert_eq!(world.tree_density, 0.8);
    assert_eq!(world.biome_scale, 2048.0);
    assert_eq!(world.ocean_scale, 4096.0);
    assert_eq!(world.terrace_height, 4.0);
    assert_eq!(world.terrace_strength, 0.72);
}
#[test]
fn repository_configs_are_valid() {
    ClientConfig::from_toml(include_str!("../../../../configs/client.toml")).unwrap();
    ServerConfig::from_toml(include_str!("../../../../configs/server.toml")).unwrap();
    WorldConfig::from_toml(include_str!("../../../../configs/world.toml")).unwrap();
    GameConfig::from_toml("[world]\nseed = 7").unwrap();
}

#[test]
fn world_time_defaults_roundtrip_and_reject_invalid_settings() {
    let old = WorldConfig::from_toml("seed = 7").unwrap();
    assert_eq!(old.time, crate::WorldTimeConfig::default());
    let configured = WorldConfig::from_toml(
        "[time]\nday_length_seconds = 7200.0\nstart_hour = 23.5\npaused = true",
    )
    .unwrap();
    assert_eq!(
        WorldConfig::from_toml(&toml::to_string(&configured).unwrap()).unwrap(),
        configured
    );
    for invalid in [
        "day_length_seconds = 0.0",
        "day_length_seconds = nan",
        "start_hour = 24.0",
        "start_hour = -1.0",
    ] {
        assert!(WorldConfig::from_toml(&format!("[time]\n{invalid}")).is_err());
    }
}
#[test]
fn missing_file_is_explicit() {
    let path = std::env::temp_dir().join(format!("terra-missing-{}.toml", std::process::id()));
    assert!(
        matches!(WorldConfig::load(path), Err(ConfigError::Io { source, .. }) if source.kind() == std::io::ErrorKind::NotFound)
    );
}
#[test]
fn legacy_prop_density_is_read_but_never_applied_or_written() {
    let graphics: super::GraphicsConfig = toml::from_str("vegetation_density = 0.0").unwrap();
    let mut client = super::ClientConfig::default();
    graphics.apply_to(&mut client);
    assert_eq!(client.vegetation_density, 1.0);
    assert!(
        !toml::to_string(&graphics)
            .unwrap()
            .contains("vegetation_density")
    );
}

#[test]
fn rejects_invalid_values_and_unknown_fields() {
    for input in [
        "width = 0",
        "fov_degrees = nan",
        "fov_degrees = 44.0",
        "fov_degrees = 111.0",
        "weather = 'storm'",
        "weather_intensity = nan",
        "weather_intensity = -0.1",
        "weather_intensity = 1.1",
        "height = 99999",
        "title = ''",
        "widht = 1000",
        "width = 'wide'",
        "view_distance = 33",
        "fog_distance = nan",
        "fog_distance = -0.1",
        "fog_distance = 1.6",
        "vertical_view_distance = 5",
        "max_chunk_jobs = 0",
        "max_mesh_jobs = 0",
        "max_mesh_jobs = 33",
        "mesh_uploads_per_frame = 0",
        "mesh_uploads_per_frame = 33",
        "lod_near_distance = nan",
        "lod_far_distance = 96.0",
        "lod_hysteresis = 32.0",
        "lod_hysteresis = -1.0",
        "prop_lod_near_distance = nan",
        "prop_lod_far_distance = inf",
        "prop_lod_near_distance = 16.0",
        "prop_lod_far_distance = 100.0",
        "lod_near_distance = 16.0\nlod_far_distance = 100.0\nlod_hysteresis = 32.0",
        "water_quality = 'ultra'",
        "water_wave_strength = nan",
        "water_wave_strength = 2.1",
        "mouse_sensitivity = nan",
        "mouse_sensitivity = 0.0",
        "mouse_sensitivity = 0.5",
        "fly_speed = -1.0",
        "fly_speed = inf",
        "fly_boost_multiplier = nan",
        "fly_boost_multiplier = 0.0",
        "fly_precision_multiplier = 0.0",
        "fly_precision_multiplier = 1.1",
    ] {
        assert!(ClientConfig::from_toml(input).is_err(), "{input}");
    }
    for input in [
        "tick_rate = 0",
        "max_players = 0",
        "bind_address = 'bad'",
        "bind_address = '127.0.0.1:0'",
        "simulation_distance = 9",
    ] {
        assert!(ServerConfig::from_toml(input).is_err(), "{input}");
    }
    for input in [
        "hill_amplitude = -1.0",
        "base_height = nan",
        "snow_height = 0.0",
        "generator_version = 999",
        "generator_version = 2",
        "generator_version = 3",
        "generator_version = 4",
        "ocean_scale = nan",
        "ocean_scale = 100.0",
        "ocean_scale = 40000.0",
        "tree_density = -0.01",
        "tree_density = 1.01",
        "tree_density = nan",
        "biome_scale = 255.0",
        "biome_scale = inf",
        "terrace_height = 0.0",
        "terrace_height = nan",
        "terrace_strength = 1.1",
        "terrace_strength = nan",
    ] {
        assert!(WorldConfig::from_toml(input).is_err(), "{input}");
    }
}

#[test]
fn wide_views_fog_and_animations_roundtrip_without_changing_legacy_defaults() {
    let old = GraphicsConfig::from_toml("").unwrap();
    assert_eq!(old.chunk_animation, ChunkAnimation::None);
    assert_eq!(old.tree_fall_animation, super::TreeFallAnimation::Current);
    assert_eq!(old.fog_distance, 1.0);
    let graphics = GraphicsConfig::from_toml(
        "render_distance = 32\nfog_distance = 0.0\nchunk_animation = 'fade_slow'",
    )
    .unwrap();
    let encoded = toml::to_string(&graphics).unwrap();
    assert_eq!(GraphicsConfig::from_toml(&encoded).unwrap(), graphics);
    let mut client = ClientConfig::default();
    graphics.apply_to(&mut client);
    client.validate().unwrap();
    assert_eq!(client.view_distance, 32);
    assert_eq!(client.fog_distance, 0.0);
    assert_eq!(client.chunk_animation, ChunkAnimation::FadeSlow);
    assert!(GraphicsConfig::from_toml("render_distance = 33").is_err());
    assert!(!old.water_volumetric_mist);
    for mode in ["fade", "fade_slow", "bounce", "rise", "slide", "none"] {
        let graphics = GraphicsConfig::from_toml(&format!(
            "chunk_animation = '{mode}'\nwater_volumetric_mist = true"
        ))
        .unwrap();
        let saved = toml::to_string(&graphics).unwrap();
        assert_eq!(GraphicsConfig::from_toml(&saved).unwrap(), graphics);
        graphics.apply_to(&mut client);
        assert!(client.water_volumetric_mist);
    }
    for mode in ["current", "hell_fall", "magic"] {
        let graphics =
            GraphicsConfig::from_toml(&format!("tree_fall_animation = '{mode}'")).unwrap();
        let saved = toml::to_string(&graphics).unwrap();
        assert_eq!(GraphicsConfig::from_toml(&saved).unwrap(), graphics);
    }
    assert!(GraphicsConfig::from_toml("tree_fall_animation = 'other'").is_err());
}
#[test]
fn natural_population_and_new_terrain_contract_are_saved_without_retrogen() {
    let legacy = WorldConfig::from_toml("seed = 42").unwrap();
    assert!(!legacy.bounded_world);
    assert_eq!(legacy.natural_mob_limit, 256);
    let mut world = WorldConfig::default();
    assert!(world.bounded_world);
    world.natural_mob_limit = 512;
    assert_eq!(
        WorldConfig::from_toml(&toml::to_string(&world).unwrap()).unwrap(),
        world
    );
    assert!(WorldConfig::from_toml("natural_mob_limit = 4097").is_err());
    assert_eq!(
        super::ServerConfig::from_toml("natural_mob_limit = 128")
            .unwrap()
            .natural_mob_limit,
        Some(128)
    );
}
