use crate::{SAVE_FORMAT_VERSION, WorldManifest, load_edits};
use std::path::{Path, PathBuf};
use terra_core::{Config, WorldConfig, save_toml};
use terra_world::edits::EditJournal;

pub struct DedicatedStore {
    pub edits_path: PathBuf,
    _lock: std::fs::File,
}
impl DedicatedStore {
    /// A missing legacy feature field means "use the saved world's policy",
    /// not "turn ores off on every restart". Other terrain settings stay explicit.
    pub fn resolve_world_config(
        directory: &Path,
        requested: &WorldConfig,
    ) -> Result<WorldConfig, String> {
        requested.validate().map_err(|error| error.to_string())?;
        let mut manifest = directory.join("world.toml");
        if !manifest.exists()
            && directory.file_name().is_some_and(|name| name == "worlds")
            && let Some(parent) = directory.parent()
        {
            let legacy = parent.join("saves/dedicated/world.toml");
            if legacy.is_file() {
                manifest = legacy;
            }
        }
        let saved = match std::fs::read_to_string(&manifest) {
            Ok(text) => {
                let saved: WorldManifest =
                    toml::from_str(&text).map_err(|error| error.to_string())?;
                if !matches!(saved.save_format_version, 1 | SAVE_FORMAT_VERSION)
                    || !saved.world.ores.valid()
                {
                    return Err("Incompatible dedicated ore-generation metadata".into());
                }
                Some(saved.world)
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
            Err(error) => return Err(error.to_string()),
        };
        let mut result = requested.clone();
        if !requested.bounded_world {
            result.bounded_world = saved.as_ref().is_none_or(|world| world.bounded_world);
        }
        if let Some(saved) = &saved {
            result.natural_mob_limit = saved.natural_mob_limit;
            terra_core::WorldRuleSettings::from_world(saved).apply(&mut result);
        }
        if requested.ores.revision == 0 && !requested.ores.enabled {
            result.ores = saved
                .as_ref()
                .map_or_else(terra_core::OreGeneration::default, |world| {
                    world.ores.clone()
                });
        }
        if requested.geology_revision == 0 {
            result.geology_revision = saved.as_ref().map_or(4, |world| world.geology_revision);
        }
        // Saved terrain always wins over a newer global template; no retrogen.
        result.landscape_revision = saved.as_ref().map_or_else(
            || {
                if requested.landscape_revision == 0 {
                    WorldConfig::CURRENT_LANDSCAPE_REVISION
                } else {
                    requested.landscape_revision
                }
            },
            |world| world.landscape_revision,
        );
        if let Some(saved) = &saved {
            result.sea_level = saved.sea_level;
            result.biome_definitions = saved.biome_definitions.clone();
            result.structures = saved.structures;
        } else if result.biome_definitions.is_none() {
            result.biome_definitions = Some(
                terra_core::BiomeDefinitions::load_directory(
                    terra_core::asset_directory().join("biome"),
                )
                .map_err(|error| error.to_string())?,
            );
        }
        result.rivers = saved
            .as_ref()
            .map_or(requested.rivers, |world| world.rivers);
        // Existing saves own this contract, even when a new template defaults to 1.
        result.ocean_revision = saved.as_ref().map_or(1, |world| world.ocean_revision);
        result.ecology_revision = saved
            .as_ref()
            .map_or(WorldConfig::CURRENT_ECOLOGY_REVISION, |world| {
                world.ecology_revision
            });
        result.blood_forest_revision = saved
            .as_ref()
            .map_or(1, |world| world.blood_forest_revision);
        result.spirit_blossom_revision = saved
            .as_ref()
            .map_or(2, |world| world.spirit_blossom_revision);
        result.cave_structure_revision = saved
            .as_ref()
            .map_or(WorldConfig::CURRENT_CAVE_STRUCTURE_REVISION, |world| {
                world.cave_structure_revision
            });
        result.structures_revision = saved.as_ref().map_or(
            terra_core::WorldConfig::CURRENT_STRUCTURES_REVISION,
            |world| world.structures_revision,
        );
        result.structure_border_gap = saved
            .as_ref()
            .map_or(requested.structure_border_gap, |world| {
                world.structure_border_gap
            });
        result.river_terrain_revision = saved.as_ref().map_or_else(
            || match requested.river_terrain_revision {
                0 => WorldConfig::CURRENT_RIVER_TERRAIN_REVISION,
                revision => revision,
            },
            |world| world.river_terrain_revision,
        );
        if let Some(saved) = &saved {
            result.cave_biomes = saved.cave_biomes.clone();
        }
        result.ravine_revision = saved.as_ref().map_or_else(
            || match requested.ravine_revision {
                0 => WorldConfig::CURRENT_RAVINE_REVISION,
                revision => revision,
            },
            |world| world.ravine_revision,
        );
        result.validate().map_err(|error| error.to_string())?;
        Ok(result)
    }
    /// Never reinterpret an existing save with another seed or generator config.
    pub fn open(directory: &Path, config: &WorldConfig) -> Result<(Self, EditJournal), String> {
        config.validate().map_err(|error| error.to_string())?;
        std::fs::create_dir_all(directory).map_err(|error| error.to_string())?;
        let lock = std::fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(directory.join("session.lock"))
            .map_err(|error| error.to_string())?;
        lock.try_lock().map_err(|error| {
            format!(
                "Cannot exclusively open dedicated save {}: {error}",
                directory.display()
            )
        })?;
        let manifest_path = directory.join("world.toml");
        // The shipped dedicated default moved from saves/dedicated to worlds.
        // Copy through the validated journal; keep the old save as a backup.
        if directory.file_name().is_some_and(|n| n == "worlds")
            && !manifest_path.exists()
            && let Some(parent) = directory.parent()
        {
            let legacy = parent.join("saves/dedicated");
            if legacy.join("world.toml").is_file() {
                let (_legacy_lock, journal) = Self::open(&legacy, config)?;
                crate::save_edits(directory, config.id, &journal)?;
                save_toml(&WorldManifest::new(config.clone()), &manifest_path)
                    .map_err(|e| e.to_string())?;
            }
        }
        match std::fs::read_to_string(&manifest_path) {
            Ok(text) => {
                let mut manifest: WorldManifest =
                    toml::from_str(&text).map_err(|error| error.to_string())?;
                // Clock preferences do not alter terrain identity or generation.
                let time_changed = manifest.world.time != config.time;
                let population_changed =
                    manifest.world.natural_mob_limit != config.natural_mob_limit;
                let kingdom_changed = manifest.world.kingdom_rules != config.kingdom_rules;
                manifest.world.time = config.time;
                manifest.world.natural_mob_limit = config.natural_mob_limit;
                manifest.world.kingdom_rules = config.kingdom_rules;
                if !matches!(manifest.save_format_version, 1 | SAVE_FORMAT_VERSION)
                    || manifest.world != *config
                {
                    return Err("Dedicated save configuration does not match this world. Restore its world config or choose another save directory.".into());
                }
                if time_changed || population_changed || kingdom_changed {
                    save_toml(&manifest, &manifest_path).map_err(|error| error.to_string())?;
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                if directory.join("edits.toml").exists()
                    || directory.join("level.data").exists()
                    || directory.join(".save-transaction").exists()
                    || directory
                        .join("regions")
                        .read_dir()
                        .is_ok_and(|mut entries| entries.next().is_some())
                {
                    return Err("Refusing to load orphaned edits without a world manifest".into());
                }
                save_toml(&WorldManifest::new(config.clone()), &manifest_path)
                    .map_err(|error| error.to_string())?;
            }
            Err(error) => return Err(error.to_string()),
        }
        let edits_path = directory.to_path_buf();
        let journal = load_edits(&edits_path, config.id)?;
        Ok((
            Self {
                edits_path,
                _lock: lock,
            },
            journal,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use terra_core::BiomeDefinitions;
    fn fresh_directory() -> PathBuf {
        std::env::temp_dir().join(format!(
            "terra-ore-policy-{}",
            terra_core::PlayerUuid::new_v4()
        ))
    }
    #[test]
    fn generation_gap_is_snapshotted_for_new_worlds_and_immutable_when_live_policy_changes() {
        let root = fresh_directory();
        let mut requested = WorldConfig {
            structure_border_gap: 7,
            kingdom_rules: terra_core::kingdom::KingdomRules {
                realm_min_border_gap: 7,
                ..Default::default()
            },
            ..Default::default()
        };
        let created = DedicatedStore::resolve_world_config(&root, &requested).unwrap();
        assert_eq!(
            created.structures_revision,
            WorldConfig::CURRENT_STRUCTURES_REVISION
        );
        assert_eq!(created.structure_border_gap, 7);
        let (store, mut journal) = DedicatedStore::open(&root, &created).unwrap();
        journal.record([-1, 20, -1], 3).unwrap();
        crate::save_edits(&root, created.id, &journal).unwrap();
        drop(store);
        let mut changed = created.clone();
        changed.kingdom_rules.realm_min_border_gap = 50;
        changed.kingdom_rules.require_adjacent_claims = false;
        assert!(changed.same_terrain(&created));
        let (store, loaded) = DedicatedStore::open(&root, &changed).unwrap();
        assert_eq!(loaded.block([-1, 20, -1]), Some(3));
        drop(store);
        requested.structure_border_gap = 123;
        requested.kingdom_rules.realm_min_border_gap = 123;
        let resolved = DedicatedStore::resolve_world_config(&root, &requested).unwrap();
        assert_eq!(resolved.kingdom_rules, changed.kingdom_rules);
        assert_eq!(resolved.structure_border_gap, 7);
        assert!(resolved.same_terrain(&created));
        let before = std::fs::read(root.join("world.toml")).unwrap();
        let mut invalid = resolved.clone();
        invalid.structure_border_gap += 1;
        assert!(DedicatedStore::open(&root, &invalid).is_err());
        assert_eq!(std::fs::read(root.join("world.toml")).unwrap(), before);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn missing_legacy_generation_gap_defaults_to_sixteen_and_never_follows_new_policy() {
        for revision in [0, 6, 7, 8] {
            let root = fresh_directory();
            let saved = WorldConfig {
                structures_revision: revision,
                kingdom_rules: terra_core::kingdom::KingdomRules {
                    realm_min_border_gap: 3,
                    ..Default::default()
                },
                ..Default::default()
            };
            let (store, _) = DedicatedStore::open(&root, &saved).unwrap();
            drop(store);
            let file = root.join("world.toml");
            let mut value: toml::Value =
                toml::from_str(&std::fs::read_to_string(&file).unwrap()).unwrap();
            value
                .get_mut("world")
                .unwrap()
                .as_table_mut()
                .unwrap()
                .remove("structure_border_gap");
            save_toml(&value, &file).unwrap();
            let requested = WorldConfig {
                structure_border_gap: 77,
                kingdom_rules: terra_core::kingdom::KingdomRules {
                    realm_min_border_gap: 77,
                    ..Default::default()
                },
                ..Default::default()
            };
            let resolved = DedicatedStore::resolve_world_config(&root, &requested).unwrap();
            assert_eq!(resolved.structures_revision, revision);
            assert_eq!(resolved.structure_border_gap, 16);
            assert_eq!(resolved.kingdom_rules, saved.kingdom_rules);
            assert!(resolved.same_terrain(&saved));
            std::fs::remove_dir_all(root).unwrap();
        }
    }
    fn remove_ore_metadata(path: &Path) {
        let text = std::fs::read_to_string(path).unwrap();
        let mut value: toml::Value = toml::from_str(&text).unwrap();
        value
            .get_mut("world")
            .unwrap()
            .as_table_mut()
            .unwrap()
            .remove("ores");
        save_toml(&value, path).unwrap();
    }
    #[test]
    fn fresh_dedicated_save_promotes_ores_and_reuses_its_stamp_on_restart_without_global_config_changes()
     {
        let root = fresh_directory();
        let requested = WorldConfig::from_toml("seed = 77").unwrap();
        assert!(!requested.ores.enabled);
        let resolved = DedicatedStore::resolve_world_config(&root, &requested).unwrap();
        assert!(resolved.ores.enabled);
        assert_eq!(resolved.ores.revision, 1);
        assert_eq!(resolved.geology_revision, 4);
        assert_eq!(resolved.landscape_revision, 9);
        assert_eq!(resolved.sea_level, 8.0);
        assert_eq!(
            resolved.biome_definitions,
            Some(BiomeDefinitions::default())
        );
        assert!(!resolved.rivers);
        assert_eq!(resolved.ocean_revision, 1);
        assert_eq!(resolved.cave_structure_revision, 13);
        assert_eq!(
            resolved.structures_revision,
            WorldConfig::CURRENT_STRUCTURES_REVISION
        );
        assert_eq!(resolved.river_terrain_revision, 7);
        assert_eq!(resolved.seed, requested.seed);
        assert_eq!(resolved.generator_version, requested.generator_version);
        let (store, mut journal) = DedicatedStore::open(&root, &resolved).unwrap();
        journal.record([-1, -20, -1], 31).unwrap();
        crate::save_edits(&store.edits_path, resolved.id, &journal).unwrap();
        drop(store);
        let reloaded = DedicatedStore::resolve_world_config(&root, &requested).unwrap();
        assert_eq!(reloaded, resolved);
        let (store, journal) = DedicatedStore::open(&root, &reloaded).unwrap();
        assert_eq!(journal.block([-1, -20, -1]), Some(31));
        drop(store);
        assert!(!requested.ores.enabled);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn landscape_revision_survives_new_templates_restart_and_saved_rule_changes() {
        for revision in 0..=9 {
            let root = fresh_directory();
            let mut biome_definitions = BiomeDefinitions::default();
            biome_definitions.entries[terra_core::BiomeKind::Plains as usize]
                .terrain
                .max_height -= 1.0 + f64::from(revision);
            biome_definitions.landscapes.warp_strength += 1.0 + f64::from(revision);
            biome_definitions.landscapes.profiles
                [terra_core::config::TerrainKind::Forest as usize]
                .max_height -= 1.0 + f64::from(revision);
            let saved = WorldConfig {
                landscape_revision: revision,
                rivers: revision % 2 == 1,
                sea_level: if revision < 4 {
                    8.0 + f64::from(revision)
                } else {
                    22.0
                },
                biome_definitions: if revision < 4 && revision % 2 == 0 {
                    None
                } else {
                    Some(biome_definitions)
                },
                ..Default::default()
            };
            let (store, mut journal) = DedicatedStore::open(&root, &saved).unwrap();
            journal.record([-17, 12, -1], 16).unwrap();
            crate::save_edits(&store.edits_path, saved.id, &journal).unwrap();
            drop(store);
            for requested_revision in 0..=5 {
                let requested = WorldConfig {
                    landscape_revision: requested_revision,
                    ..Default::default()
                };
                let mut resolved = DedicatedStore::resolve_world_config(&root, &requested).unwrap();
                assert_eq!(resolved.landscape_revision, revision);
                assert_eq!(resolved.rivers, saved.rivers);
                assert_eq!(resolved.sea_level, saved.sea_level);
                assert_eq!(resolved.biome_definitions, saved.biome_definitions);
                assert!(resolved.same_terrain(&saved));
                resolved.time.paused = !resolved.time.paused;
                let (store, restored) = DedicatedStore::open(&root, &resolved).unwrap();
                assert_eq!(restored.block([-17, 12, -1]), Some(16));
                drop(store);
                let manifest: WorldManifest =
                    toml::from_str(&std::fs::read_to_string(root.join("world.toml")).unwrap())
                        .unwrap();
                assert_eq!(manifest.world.landscape_revision, revision);
                assert!(manifest.world.same_terrain(&saved));
            }
            std::fs::remove_dir_all(root).unwrap();
        }
    }

    #[test]
    fn revision_four_manifest_without_new_profiles_reopens_without_retrogen_or_template_changes() {
        let root = fresh_directory();
        let mut saved = WorldConfig {
            landscape_revision: 4,
            sea_level: 22.0,
            biome_definitions: Some(BiomeDefinitions::default()),
            rivers: false,
            caves: false,
            trees: false,
            structures_revision: 0,
            ..Default::default()
        };
        saved.biome_definitions.as_mut().unwrap().landscapes =
            terra_core::LandscapeDefinitions::legacy_v5();
        saved.biome_definitions.as_mut().unwrap().entries[terra_core::BiomeKind::Plains as usize]
            .terrain
            .max_height = 58.0;
        let (store, mut journal) = DedicatedStore::open(&root, &saved).unwrap();
        journal.record([-17, 12, -1], 16).unwrap();
        crate::save_edits(&store.edits_path, saved.id, &journal).unwrap();
        drop(store);
        let path = root.join("world.toml");
        let mut legacy: toml::Value =
            toml::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        legacy["world"]["biome_definitions"]
            .as_table_mut()
            .unwrap()
            .remove("landscapes");
        save_toml(&legacy, &path).unwrap();
        let mut requested = WorldConfig {
            landscape_revision: 5,
            sea_level: 17.0,
            ..saved.clone()
        };
        requested
            .biome_definitions
            .as_mut()
            .unwrap()
            .landscapes
            .warp_strength += 32.0;
        let resolved = DedicatedStore::resolve_world_config(&root, &requested).unwrap();
        assert_eq!(resolved, saved);
        let (store, restored) = DedicatedStore::open(&root, &resolved).unwrap();
        assert_eq!(restored.block([-17, 12, -1]), Some(16));
        drop(store);

        let reference = terra_world::TerraGenerator::new(saved.clone()).unwrap();
        let restored = terra_world::TerraGenerator::new(resolved.clone()).unwrap();
        let mut changed_profiles = resolved;
        let landscapes = &mut changed_profiles
            .biome_definitions
            .as_mut()
            .unwrap()
            .landscapes;
        landscapes.warp_strength += 48.0;
        landscapes.profiles[terra_core::config::TerrainKind::Forest as usize].max_height -= 3.0;
        let changed_profiles = terra_world::TerraGenerator::new(changed_profiles).unwrap();
        for [x, z] in [
            [-4096.5, -768.5],
            [-16.5, 32.5],
            [0.5, 0.5],
            [512.5, -512.5],
            [4096.5, 768.5],
        ] {
            let height = reference.surface_height(x, z).to_bits();
            assert_eq!(restored.surface_height(x, z).to_bits(), height);
            assert_eq!(
                changed_profiles.surface_height(x, z).to_bits(),
                height,
                "revision-four sampling must ignore newly introduced landscape profiles"
            );
        }
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn fresh_main_landscapes_respect_explicit_river_opt_out() {
        for rivers in [false, true] {
            let root = fresh_directory();
            let requested = WorldConfig {
                rivers,
                ..Default::default()
            };
            let resolved = DedicatedStore::resolve_world_config(&root, &requested).unwrap();
            assert_eq!(resolved.landscape_revision, 9);
            assert_eq!(resolved.rivers, rivers);
            let (store, _) = DedicatedStore::open(&root, &resolved).unwrap();
            drop(store);
            let changed_template = WorldConfig {
                rivers: !rivers,
                ..requested
            };
            let reopened = DedicatedStore::resolve_world_config(&root, &changed_template).unwrap();
            assert!(reopened.same_terrain(&resolved));
            std::fs::remove_dir_all(root).unwrap();
        }
    }

    #[test]
    fn fresh_dedicated_worlds_preserve_explicit_terrain_revisions_and_sea_levels() {
        for (revision, sea_level, expected) in [
            (0, 8.0, 8.0),
            (3, 8.0, 8.0),
            (3, 13.0, 13.0),
            (4, 8.0, 8.0),
            (4, 22.0, 22.0),
            (5, 8.0, 8.0),
            (5, 22.0, 22.0),
        ] {
            let root = fresh_directory();
            let requested = WorldConfig {
                landscape_revision: revision,
                sea_level,
                ..Default::default()
            };
            let resolved = DedicatedStore::resolve_world_config(&root, &requested).unwrap();
            assert_eq!(
                resolved.landscape_revision,
                if revision == 0 { 9 } else { revision }
            );
            assert_eq!(resolved.sea_level, expected);
            let (store, _) = DedicatedStore::open(&root, &resolved).unwrap();
            drop(store);
            let changed = WorldConfig {
                sea_level: 17.0,
                ..requested
            };
            let restored = DedicatedStore::resolve_world_config(&root, &changed).unwrap();
            assert_eq!(restored.sea_level, expected);
            assert!(restored.same_terrain(&resolved));
            std::fs::remove_dir_all(root).unwrap();
        }
    }

    #[test]
    fn new_chambers_do_not_upgrade_saved_cave_revisions_or_overwrite_edits() {
        for revision in 0..WorldConfig::default().cave_structure_revision {
            let root = fresh_directory();
            let saved = WorldConfig {
                cave_structure_revision: revision,
                ..Default::default()
            };
            let (store, mut journal) = DedicatedStore::open(&root, &saved).unwrap();
            journal.record([-17, -20, -1], 37).unwrap();
            crate::save_edits(&store.edits_path, saved.id, &journal).unwrap();
            drop(store);
            let resolved =
                DedicatedStore::resolve_world_config(&root, &WorldConfig::default()).unwrap();
            assert_eq!(resolved.cave_structure_revision, revision);
            assert_eq!(resolved.generator_version, saved.generator_version);
            assert_eq!(resolved.id, saved.id);
            let (store, restored) = DedicatedStore::open(&root, &resolved).unwrap();
            assert_eq!(restored.block([-17, -20, -1]), Some(37));
            drop(store);
            std::fs::remove_dir_all(root).unwrap();
        }
    }
    #[test]
    fn structures_never_upgrade_a_saved_world_contract_or_change_its_identity() {
        let root = fresh_directory();
        let saved = WorldConfig::from_toml("id=171\nseed=91").unwrap();
        assert_eq!(saved.structures_revision, 0);
        let (store, mut journal) = DedicatedStore::open(&root, &saved).unwrap();
        journal.record([-17, 12, -1], 16).unwrap();
        crate::save_edits(&store.edits_path, saved.id, &journal).unwrap();
        drop(store);
        let manifest = root.join("world.toml");
        let mut legacy: toml::Value =
            toml::from_str(&std::fs::read_to_string(&manifest).unwrap()).unwrap();
        legacy
            .get_mut("world")
            .unwrap()
            .as_table_mut()
            .unwrap()
            .remove("structures_revision");
        save_toml(&legacy, &manifest).unwrap();
        let requested = WorldConfig {
            structures_revision: 1,
            ..saved.clone()
        };
        let resolved = DedicatedStore::resolve_world_config(&root, &requested).unwrap();
        assert_eq!(resolved.structures_revision, 0);
        assert_eq!(resolved.id, saved.id);
        assert_eq!(resolved.seed, saved.seed);
        let (store, restored) = DedicatedStore::open(&root, &resolved).unwrap();
        assert_eq!(restored.block([-17, 12, -1]), Some(16));
        drop(store);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn old_village_switch_retains_royal_policy_and_new_flags_survive_dedicated_restart() {
        for legacy in [true, false] {
            let root = fresh_directory();
            let saved = WorldConfig {
                structures_revision: if legacy { 6 } else { 7 },
                structures: terra_core::StructureSettings {
                    villages: false,
                    kingdoms: false,
                    outposts: !legacy,
                    ..Default::default()
                },
                ..Default::default()
            };
            let (store, _) = DedicatedStore::open(&root, &saved).unwrap();
            drop(store);
            if legacy {
                let file = root.join("world.toml");
                let mut manifest: toml::Value =
                    toml::from_str(&std::fs::read_to_string(&file).unwrap()).unwrap();
                let structures = manifest
                    .get_mut("world")
                    .unwrap()
                    .get_mut("structures")
                    .unwrap()
                    .as_table_mut()
                    .unwrap();
                structures.remove("kingdoms");
                structures.remove("outposts");
                save_toml(&manifest, file).unwrap();
            }
            let resolved =
                DedicatedStore::resolve_world_config(&root, &WorldConfig::default()).unwrap();
            assert_eq!(resolved.structures_revision, saved.structures_revision);
            assert_eq!(
                resolved.structures, saved.structures,
                "A new server template must not re-enable disabled structures in existing saves"
            );
            let (store, _) = DedicatedStore::open(&root, &resolved).unwrap();
            drop(store);
            std::fs::remove_dir_all(root).unwrap();
        }
    }

    #[test]
    fn new_structure_revisions_preserve_existing_sites_and_player_edits() {
        for revision in [1, 2, 3, 10, 11, 12, 13] {
            let root = fresh_directory();
            let saved = WorldConfig {
                structures_revision: revision,
                ravine_revision: 1,
                ..Default::default()
            };
            let (store, mut journal) = DedicatedStore::open(&root, &saved).unwrap();
            journal.record([-17, 12, -1], 16).unwrap();
            crate::save_edits(&store.edits_path, saved.id, &journal).unwrap();
            drop(store);
            let resolved =
                DedicatedStore::resolve_world_config(&root, &WorldConfig::default()).unwrap();
            assert_eq!(resolved.structures_revision, revision);
            assert!(resolved.same_terrain(&saved));
            let (store, restored) = DedicatedStore::open(&root, &resolved).unwrap();
            assert_eq!(restored.block([-17, 12, -1]), Some(16));
            drop(store);
            std::fs::remove_dir_all(root).unwrap();
        }
    }
    #[test]
    fn saved_ecology_revisions_preserve_legacy_manifests_and_player_edits() {
        for revision in 0..WorldConfig::CURRENT_ECOLOGY_REVISION {
            let root = fresh_directory();
            let saved = WorldConfig {
                ecology_revision: revision,
                ..Default::default()
            };
            let (store, mut journal) = DedicatedStore::open(&root, &saved).unwrap();
            journal.record([-17, 12, -1], 16).unwrap();
            crate::save_edits(&store.edits_path, saved.id, &journal).unwrap();
            drop(store);
            if revision == 0 {
                let path = root.join("world.toml");
                let mut manifest: toml::Value =
                    toml::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
                manifest["world"]
                    .as_table_mut()
                    .unwrap()
                    .remove("ecology_revision");
                save_toml(&manifest, &path).unwrap();
            }
            let requested = WorldConfig {
                ecology_revision: WorldConfig::CURRENT_ECOLOGY_REVISION,
                ..saved.clone()
            };
            let resolved = DedicatedStore::resolve_world_config(&root, &requested).unwrap();
            assert_eq!(resolved.ecology_revision, revision);
            assert!(resolved.same_terrain(&saved));
            let (store, journal) = DedicatedStore::open(&root, &resolved).unwrap();
            assert_eq!(journal.block([-17, 12, -1]), Some(16));
            drop(store);
            std::fs::remove_dir_all(root).unwrap();
        }
    }

    #[test]
    fn river_erosion_does_not_reinterpret_legacy_waterfalls_or_player_edits() {
        for revision in 0..=5 {
            let root = fresh_directory();
            let saved = WorldConfig {
                river_terrain_revision: revision,
                rivers: true,
                ..Default::default()
            };
            let (store, mut journal) = DedicatedStore::open(&root, &saved).unwrap();
            journal.record([-17, 12, -1], 16).unwrap();
            crate::save_edits(&store.edits_path, saved.id, &journal).unwrap();
            drop(store);
            if revision == 0 {
                let manifest = root.join("world.toml");
                let mut legacy: toml::Value =
                    toml::from_str(&std::fs::read_to_string(&manifest).unwrap()).unwrap();
                legacy
                    .get_mut("world")
                    .unwrap()
                    .as_table_mut()
                    .unwrap()
                    .remove("river_terrain_revision");
                save_toml(&legacy, &manifest).unwrap();
            }
            let requested = WorldConfig {
                river_terrain_revision: 6,
                ..saved.clone()
            };
            let resolved = DedicatedStore::resolve_world_config(&root, &requested).unwrap();
            assert_eq!(resolved.river_terrain_revision, revision);
            assert!(resolved.same_terrain(&saved));
            let (store, restored) = DedicatedStore::open(&root, &resolved).unwrap();
            assert_eq!(restored.block([-17, 12, -1]), Some(16));
            drop(store);
            std::fs::remove_dir_all(root).unwrap();
        }
    }
    #[test]
    fn existing_geology_one_keeps_its_terrain_and_region_edits() {
        let root = fresh_directory();
        let saved = WorldConfig {
            geology_revision: 1,
            ocean_revision: 0,
            cave_structure_revision: 0,
            ..Default::default()
        };
        let (store, mut journal) = DedicatedStore::open(&root, &saved).unwrap();
        journal.record([-1, -20, -1], 37).unwrap();
        crate::save_edits(&store.edits_path, saved.id, &journal).unwrap();
        drop(store);
        let requested = WorldConfig::from_toml("seed = 42042").unwrap();
        let resolved = DedicatedStore::resolve_world_config(&root, &requested).unwrap();
        assert_eq!(resolved.geology_revision, 1);
        assert_eq!(resolved.landscape_revision, saved.landscape_revision);
        assert_eq!(resolved.ocean_revision, 0);
        assert_eq!(resolved.cave_structure_revision, 0);
        assert_eq!(
            DedicatedStore::resolve_world_config(&root, &WorldConfig::default())
                .unwrap()
                .ocean_revision,
            0
        );
        let (store, journal) = DedicatedStore::open(&root, &resolved).unwrap();
        assert_eq!(journal.block([-1, -20, -1]), Some(37));
        drop(store);
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn missing_feature_metadata_in_existing_and_migrated_dedicated_worlds_stays_disabled() {
        let root = fresh_directory();
        let legacy = root.join("saves/dedicated");
        let destination = root.join("worlds");
        let requested = WorldConfig::from_toml("seed = 77").unwrap();
        let (store, mut journal) = DedicatedStore::open(&legacy, &requested).unwrap();
        journal.record([-1, -20, -1], 3).unwrap();
        crate::save_edits(&store.edits_path, requested.id, &journal).unwrap();
        drop(store);
        remove_ore_metadata(&legacy.join("world.toml"));
        for target in [&legacy, &destination] {
            let config = DedicatedStore::resolve_world_config(target, &requested).unwrap();
            assert!(!config.ores.enabled);
            assert_eq!(config.ores.revision, 0);
            assert_eq!(config.landscape_revision, 0);
            assert_eq!(config.seed, requested.seed);
            let (store, journal) = DedicatedStore::open(target, &config).unwrap();
            assert_eq!(journal.block([-1, -20, -1]), Some(3));
            drop(store);
        }
        let saved = DedicatedStore::resolve_world_config(&destination, &requested).unwrap();
        assert!(!saved.ores.enabled);
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn explicit_native_ore_configuration_is_never_replaced_by_default_policy() {
        let root = fresh_directory();
        let mut requested = WorldConfig::default();
        requested.ores.enabled = false;
        requested.ores.iron.frequency = 0.14;
        requested.ores.salt.radius = 3.0;
        let resolved = DedicatedStore::resolve_world_config(&root, &requested).unwrap();
        assert_eq!(resolved.ores, requested.ores);
        assert!(requested.biome_definitions.is_none());
        let expected = WorldConfig {
            biome_definitions: Some(BiomeDefinitions::default()),
            ..requested.clone()
        };
        assert_eq!(resolved, expected);
        let (store, _) = DedicatedStore::open(&root, &resolved).unwrap();
        drop(store);
        let restarted = DedicatedStore::resolve_world_config(&root, &requested).unwrap();
        assert_eq!(restarted.ores, requested.ores);
        assert_eq!(restarted, resolved);
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn old_dedicated_default_is_copied_to_unnamed_world_root_without_destroying_backup() {
        let root = std::env::temp_dir().join(format!(
            "terra-default-migration-{}",
            terra_core::PlayerUuid::new_v4()
        ));
        let old = root.join("saves/dedicated");
        let new = root.join("worlds");
        let config = WorldConfig::default();
        let (store, mut journal) = DedicatedStore::open(&old, &config).unwrap();
        journal.record([-1, 20, -1], 27).unwrap();
        crate::save_edits(&store.edits_path, config.id, &journal).unwrap();
        drop(store);
        let (store, loaded) = DedicatedStore::open(&new, &config).unwrap();
        assert_eq!(loaded.block([-1, 20, -1]), Some(27));
        assert!(old.join("world.toml").is_file());
        assert!(new.join("world.toml").is_file());
        assert!(new.join("regions/r.-1.-1.region").is_file());
        drop(store);
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn new_block_ids_and_inventory_roundtrip_through_the_existing_save() {
        let root = std::env::temp_dir().join(format!(
            "terra-material-save-{}",
            terra_core::PlayerUuid::new_v4()
        ));
        let config = WorldConfig::default();
        let (store, mut journal) = DedicatedStore::open(&root, &config).unwrap();
        let players: Vec<_> = (0..usize::from(terra_voxel::MaterialId::LAST)
            .div_ceil(terra_core::INVENTORY_SLOTS))
            .map(|_| terra_core::PlayerUuid::new_v4())
            .collect();
        let material_ids = || (1..=u16::MAX).filter(|&id| terra_core::ItemKind::Block(id).valid());
        for (index, id) in material_ids().enumerate() {
            journal.record([-20 + i64::from(id), 20, -1], id).unwrap();
            journal
                .gameplay_mut()
                .inventories
                .entry(players[index / terra_core::INVENTORY_SLOTS])
                .or_default()
                .insert(terra_core::ItemKind::Block(id), 1);
        }
        crate::save_edits(&store.edits_path, config.id, &journal).unwrap();
        drop(store);
        let (store, loaded) = DedicatedStore::open(&root, &config).unwrap();
        for (index, id) in material_ids().enumerate() {
            assert_eq!(loaded.block([-20 + i64::from(id), 20, -1]), Some(id));
            assert_eq!(
                loaded.gameplay().inventories[&players[index / terra_core::INVENTORY_SLOTS]]
                    .count(terra_core::ItemKind::Block(id)),
                1
            );
        }
        assert_eq!(
            loaded.fluid([-6, 20, -1]).unwrap_or_default(),
            terra_world::water::FluidCell::default()
        );
        drop(store);
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn dedicated_save_reloads_the_existing_journal_and_rejects_world_mismatch() {
        let root = std::env::temp_dir().join(format!(
            "terra-dedicated-{}",
            terra_core::PlayerUuid::new_v4()
        ));
        let config = WorldConfig::default();
        let (store, mut journal) = DedicatedStore::open(&root, &config).unwrap();
        journal.record([-1, 0, -1], 0).unwrap();
        crate::save_edits(&store.edits_path, config.id, &journal).unwrap();
        assert!(DedicatedStore::open(&root, &config).is_err());
        drop(store);
        let (_, loaded) = DedicatedStore::open(&root, &config).unwrap();
        assert_eq!(loaded.edited_blocks(), 1);
        let mut other = config;
        other.seed.0 += 1;
        assert!(DedicatedStore::open(&root, &other).is_err());
        std::fs::write(root.join("world.toml"), "invalid").unwrap();
        assert!(DedicatedStore::open(&root, &other).is_err());
        std::fs::remove_dir_all(root).unwrap();
    }
}
