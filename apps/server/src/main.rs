mod console;
mod shutdown;
mod smoke_test;
use terra_core::{ClientConfig, Config, GameConfig, ServerConfig, WorldConfig, config_directory};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let directory = config_directory();
    let mut server = ServerConfig::load(directory.join("server.toml"))?;
    let mut world = WorldConfig::load(directory.join("world.toml"))?;
    let mut bind_override = None;
    let mut advertised = None;
    let mut smoke = false;
    let mut arguments = std::env::args().skip(1);
    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "--save-dir" => server.save_directory = Some(arguments.next().ok_or("--save-dir requires a path")?.into()),
            "--no-save" => server.save_directory = None,
            "--bind" => {
                bind_override = Some(
                    arguments
                        .next()
                        .ok_or("--bind requires an address")?
                        .parse::<std::net::SocketAddr>()?,
                )
            }
            "--advertise" => {
                advertised = Some(
                    arguments
                        .next()
                        .ok_or("--advertise requires an address")?
                        .parse::<std::net::SocketAddr>()?,
                )
            }
            "--token-out" => return Err("Join files are no longer needed. Clients connect by address using their UUID profile.".into()),
            "--smoke-test" => smoke = true,
            "--help" => {
                println!(
                    "terra-server [--bind IP:PORT] [--advertise IP:PORT] [--save-dir PATH | --no-save] [--smoke-test]\nTCP status/join uses the same port as UDP gameplay. LAN discovery: UDP 54545."
                );
                return Ok(());
            }
            _ => return Err(format!("Unknown server argument: {argument}").into()),
        }
    }
    server.validate()?;
    if advertised.is_some_and(|address| address.ip().is_unspecified() || address.port() == 0) {
        return Err("Advertised address must be reachable and have a nonzero port".into());
    }
    let bind = if smoke {
        ([127, 0, 0, 1], 0).into()
    } else {
        bind_override.unwrap_or(server.bind_address)
    };
    let save_path = (!smoke)
        .then_some(server.save_directory.as_ref())
        .flatten()
        .map(|path| {
            if path.is_absolute() {
                path.clone()
            } else {
                directory.parent().unwrap_or(&directory).join(path)
            }
        });
    world = resolve_startup_world(world, &server, save_path.as_deref())
        .map_err(std::io::Error::other)?;
    let persistence = if let Some(path) = save_path {
        Some(terra_storage::DedicatedStore::open(&path, &world).map_err(std::io::Error::other)?)
    } else {
        if world.ores.revision == 0 && !world.ores.enabled {
            world.ores = terra_core::OreGeneration::default();
        }
        world.geology_revision = 4;
        snapshot_temporary_profiles(&mut world, &terra_core::asset_directory())?;
        None
    };
    let world_id = world.id;
    let mut app = terra_server::headless_app(GameConfig {
        client: ClientConfig::default(),
        server,
        world,
    });
    if let Some((store, journal)) = persistence {
        terra_server::DedicatedPersistence::install(&mut app, store, journal, world_id);
    }
    app.insert_resource(terra_network::server::ServerEndpoint { bind, advertised });
    shutdown::install(&mut app)?;
    let _console = (!smoke).then(|| console::install(&mut app)).transpose()?;
    smoke_test::install(&mut app);
    if app.run().is_error() {
        return Err("Server exited with an error".into());
    }
    Ok(())
}

fn resolve_startup_world(
    mut world: WorldConfig,
    server: &ServerConfig,
    save_path: Option<&std::path::Path>,
) -> Result<WorldConfig, String> {
    // Snapshot the requested creation gap before resolution. Existing worlds
    // restore their immutable revision/gap even when live rules are overridden.
    if let Some(rules) = server.kingdom_rules {
        world.kingdom_rules = rules;
    }
    world.structure_border_gap = world.kingdom_rules.realm_min_border_gap;
    world = if let Some(path) = save_path {
        terra_storage::DedicatedStore::resolve_world_config(path, &world)?
    } else {
        world.structures_revision = terra_core::WorldConfig::CURRENT_STRUCTURES_REVISION;
        world
    };
    if let Some(rules) = server.kingdom_rules {
        world.kingdom_rules = rules;
    }
    world.validate().map_err(|e| e.to_string())?;
    Ok(world)
}

fn snapshot_temporary_profiles(
    world: &mut WorldConfig,
    assets: &std::path::Path,
) -> Result<(), terra_core::ConfigError> {
    // This path creates a fresh, unsaved world; missing save metadata elsewhere
    // must continue to select the immutable legacy fallback.
    if world.landscape_revision >= 5 && world.biome_definitions.is_none() {
        world.biome_definitions = Some(terra_core::BiomeDefinitions::load_directory(
            assets.join("biome"),
        )?);
    }
    Ok(())
}

#[cfg(test)]
mod terrain_tests {
    use super::*;

    #[test]
    fn startup_resolves_saved_policy_then_applies_only_explicit_overrides_without_retrogen() {
        let root = std::env::temp_dir().join(format!(
            "terra-server-policy-{}",
            terra_core::PlayerUuid::new_v4()
        ));
        let saved = WorldConfig {
            structures_revision: 8,
            structure_border_gap: 16,
            kingdom_rules: terra_core::kingdom::KingdomRules {
                realm_min_border_gap: 7,
                ..Default::default()
            },
            ..Default::default()
        };
        let (store, _) = terra_storage::DedicatedStore::open(&root, &saved).unwrap();
        drop(store);
        let mut template = WorldConfig::default();
        template.kingdom_rules.realm_min_border_gap = 98;
        let mut server = ServerConfig::default();
        let resolved = resolve_startup_world(template.clone(), &server, Some(&root)).unwrap();
        assert_eq!(resolved.kingdom_rules, saved.kingdom_rules);
        assert!(resolved.same_terrain(&saved));
        let mut rules = saved.kingdom_rules;
        rules.realm_min_border_gap = 23;
        rules.kingdom_build_base = 55;
        server.kingdom_rules = Some(rules);
        let overridden = resolve_startup_world(template.clone(), &server, Some(&root)).unwrap();
        assert_eq!(overridden.kingdom_rules, rules);
        assert_eq!(overridden.structure_border_gap, 16);
        assert!(overridden.same_terrain(&saved));
        let (store, _) = terra_storage::DedicatedStore::open(&root, &overridden).unwrap();
        drop(store);
        assert_eq!(
            resolve_startup_world(template.clone(), &ServerConfig::default(), Some(&root))
                .unwrap()
                .kingdom_rules,
            rules
        );
        let fresh = root.join("fresh");
        let created = resolve_startup_world(template.clone(), &server, Some(&fresh)).unwrap();
        assert_eq!(
            created.structures_revision,
            WorldConfig::CURRENT_STRUCTURES_REVISION
        );
        assert_eq!(created.structure_border_gap, 23);
        assert_eq!(created.kingdom_rules, rules);
        let temporary = resolve_startup_world(template, &server, None).unwrap();
        assert_eq!(
            temporary.structures_revision,
            WorldConfig::CURRENT_STRUCTURES_REVISION
        );
        assert_eq!(temporary.structure_border_gap, 23);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn fresh_snapshotless_no_save_world_uses_current_asset_profiles() {
        let mut world = WorldConfig {
            landscape_revision: 7,
            sea_level: 22.0,
            biome_definitions: None,
            ..Default::default()
        };
        snapshot_temporary_profiles(&mut world, &terra_core::asset_directory()).unwrap();
        world.validate().unwrap();
        assert_eq!(
            world.biome_definitions.unwrap().landscapes,
            terra_core::LandscapeDefinitions::default()
        );
    }

    #[test]
    fn explicit_profiles_and_legacy_revisions_do_not_read_new_assets() {
        let missing = std::path::Path::new("/nonexistent-teramic-profile-test");
        let mut explicit = WorldConfig {
            landscape_revision: 5,
            sea_level: 22.0,
            biome_definitions: Some(terra_core::BiomeDefinitions::default()),
            ..Default::default()
        };
        explicit.biome_definitions.as_mut().unwrap().landscapes =
            terra_core::LandscapeDefinitions::legacy_v5();
        let expected = explicit.clone();
        snapshot_temporary_profiles(&mut explicit, missing).unwrap();
        assert_eq!(explicit, expected);
        let mut legacy = WorldConfig {
            landscape_revision: 4,
            biome_definitions: None,
            ..Default::default()
        };
        snapshot_temporary_profiles(&mut legacy, missing).unwrap();
        assert!(legacy.biome_definitions.is_none());
    }

    #[test]
    fn saved_legacy_terrain_does_not_require_or_adopt_advanced_profiles() {
        let missing = std::path::Path::new("/nonexistent-teramic-profile-test");
        let mut world = WorldConfig {
            landscape_revision: 1,
            sea_level: 8.0,
            rivers: false,
            ..Default::default()
        };
        let expected = world.clone();
        snapshot_temporary_profiles(&mut world, missing).unwrap();
        assert_eq!(world, expected);
        assert_eq!(world.landscape_revision, 1);
        assert_eq!(world.sea_level, 8.0);
        assert!(!world.rivers);
        assert!(world.biome_definitions.is_none());
    }
}
