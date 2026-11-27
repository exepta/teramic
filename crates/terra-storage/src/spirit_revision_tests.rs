//! Creation enables the current biome contract; reopening never changes a save.
use crate::{DedicatedStore, GameMode, WorldEntry, WorldLibrary, WorldManifest, save_edits};
use std::{fs, path::PathBuf};
use terra_core::{BiomeDefinitions, BiomeKind, Config, WorldConfig, WorldId, WorldSeed, save_toml};

struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        Self(std::env::temp_dir().join(format!(
            "terra-spirit-storage-{}",
            terra_core::PlayerUuid::new_v4()
        )))
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// Model an actual historical TOML document, not a newly serialized revision-zero
/// config. The field is absent and its biome profile array still has eight items.
fn make_legacy(world: &mut toml::Value) {
    let fields = world.as_table_mut().unwrap();
    fields.remove("ecology_revision");
    fields.remove("spirit_blossom_revision");
    fields.remove("blood_forest_revision");
    let entries = world["biome_definitions"]["entries"]
        .as_array_mut()
        .unwrap();
    assert_eq!(entries.len(), BiomeKind::ALL.len());
    entries.truncate(8);
    assert_eq!(entries.len(), 8);
}

#[test]
fn new_dedicated_and_library_worlds_enable_spirit_even_from_legacy_templates() {
    let template = WorldConfig::from_toml("seed=42042").unwrap();
    assert_eq!(template.spirit_blossom_revision, 0);
    let directory = Directory::new();
    let dedicated = directory.0.join("dedicated");
    let created = DedicatedStore::resolve_world_config(&dedicated, &template).unwrap();
    assert_eq!(created.spirit_blossom_revision, 2);
    let (store, _) = DedicatedStore::open(&dedicated, &created).unwrap();
    drop(store);
    let restarted = DedicatedStore::resolve_world_config(&dedicated, &template).unwrap();
    assert_eq!(restarted, created);
    assert_eq!(restarted.spirit_blossom_revision, 2);
    let (store, _) = DedicatedStore::open(&dedicated, &restarted).unwrap();
    drop(store);

    let library = WorldLibrary {
        root: directory.0.join("library"),
    };
    let entry = library
        .create(
            "New Spirit World",
            Some(42042),
            GameMode::Survival,
            &template,
        )
        .unwrap();
    assert_eq!(entry.manifest.world.spirit_blossom_revision, 2);
    assert_eq!(
        library
            .load(entry.manifest.world.id)
            .unwrap()
            .manifest
            .world,
        entry.manifest.world
    );
}

#[test]
fn actual_legacy_dedicated_toml_and_eight_profiles_beat_new_templates_across_restarts() {
    let directory = Directory::new();
    fs::create_dir_all(&directory.0).unwrap();
    let saved = WorldConfig {
        ecology_revision: 0,
        spirit_blossom_revision: 0,
        blood_forest_revision: 0,
        biome_definitions: Some(BiomeDefinitions::default()),
        ..Default::default()
    };
    let mut legacy = toml::Value::try_from(WorldManifest::new(saved.clone())).unwrap();
    make_legacy(&mut legacy["world"]);
    let manifest = directory.0.join("world.toml");
    save_toml(&legacy, &manifest).unwrap();
    assert!(
        !fs::read_to_string(&manifest)
            .unwrap()
            .contains("spirit_blossom_revision")
    );
    let requested = WorldConfig {
        ecology_revision: WorldConfig::CURRENT_ECOLOGY_REVISION,
        spirit_blossom_revision: 2,
        blood_forest_revision: 1,
        ..saved.clone()
    };
    for restart in 0..3 {
        let resolved = DedicatedStore::resolve_world_config(&directory.0, &requested).unwrap();
        assert_eq!(resolved.spirit_blossom_revision, 0);
        assert_eq!(resolved.blood_forest_revision, 0);
        assert!(resolved.same_terrain(&saved));
        assert_eq!(
            resolved
                .biome_definitions
                .as_ref()
                .unwrap()
                .entries
                .each_ref()
                .map(|profile| profile.biome),
            BiomeKind::ALL
        );
        let (store, mut journal) = DedicatedStore::open(&directory.0, &resolved).unwrap();
        if restart == 0 {
            journal.record([-1, 12, -17], 16).unwrap();
            save_edits(&directory.0, resolved.id, &journal).unwrap();
        } else {
            assert_eq!(journal.block([-1, 12, -17]), Some(16));
        }
        drop(store);
    }
    let persisted: WorldManifest = toml::from_str(&fs::read_to_string(manifest).unwrap()).unwrap();
    assert_eq!(persisted.world.spirit_blossom_revision, 0);
    assert!(persisted.world.same_terrain(&saved));
}

#[test]
fn actual_legacy_library_toml_keeps_its_revision_when_loaded_played_and_saved() {
    let directory = Directory::new();
    let library = WorldLibrary {
        root: directory.0.clone(),
    };
    // A historical fixture must not depend on create's random identity or current
    // migration policy. toml::Value only supports signed integer values, unlike
    // the typed production serializer/deserializer, which supports full u64 IDs.
    let entry = WorldEntry {
        title: "Old World".into(),
        game_version: env!("CARGO_PKG_VERSION").into(),
        mode: GameMode::Creative,
        last_played: None,
        manifest: WorldManifest::new(WorldConfig {
            id: WorldId(42042),
            seed: WorldSeed(42042),
            biome_definitions: Some(BiomeDefinitions::default()),
            ..Default::default()
        }),
    };
    let id = entry.manifest.world.id;
    let path = directory.0.join("Old World");
    fs::create_dir_all(&path).unwrap();
    let manifest = path.join("world.toml");
    let mut legacy = toml::Value::try_from(&entry).unwrap();
    make_legacy(&mut legacy["manifest"]["world"]);
    save_toml(&legacy, &manifest).unwrap();
    let mut expected = entry.manifest.world;
    expected.ecology_revision = 0;
    expected.spirit_blossom_revision = 0;
    expected.blood_forest_revision = 0;
    for _ in 0..2 {
        let loaded = library.load(id).unwrap();
        assert_eq!(loaded.manifest.world.spirit_blossom_revision, 0);
        assert_eq!(loaded.manifest.world.blood_forest_revision, 0);
        assert!(loaded.manifest.world.same_terrain(&expected));
        let played = library.mark_played(id).unwrap();
        library.save(&played).unwrap();
        assert_eq!(
            library
                .load(id)
                .unwrap()
                .manifest
                .world
                .spirit_blossom_revision,
            0
        );
    }
    assert!(library.list().unwrap().1.is_empty());
}

#[test]
fn nine_profile_spirit_world_retains_zero_blood_revision_across_restarts() {
    let directory = Directory::new();
    fs::create_dir_all(&directory.0).unwrap();
    let saved = WorldConfig {
        ecology_revision: 11,
        spirit_blossom_revision: 2,
        blood_forest_revision: 0,
        biome_definitions: Some(BiomeDefinitions::default()),
        ..Default::default()
    };
    let mut legacy = toml::Value::try_from(WorldManifest::new(saved.clone())).unwrap();
    legacy["world"]
        .as_table_mut()
        .unwrap()
        .remove("blood_forest_revision");
    let entries = legacy["world"]["biome_definitions"]["entries"]
        .as_array_mut()
        .unwrap();
    assert_eq!(entries.len(), BiomeKind::ALL.len());
    entries.truncate(9);
    let manifest = directory.0.join("world.toml");
    save_toml(&legacy, &manifest).unwrap();
    let current = WorldConfig {
        ecology_revision: WorldConfig::CURRENT_ECOLOGY_REVISION,
        blood_forest_revision: 1,
        ..saved.clone()
    };
    for _ in 0..2 {
        let resolved = DedicatedStore::resolve_world_config(&directory.0, &current).unwrap();
        assert_eq!(resolved.blood_forest_revision, 0);
        assert_eq!(resolved.spirit_blossom_revision, 2);
        assert_eq!(
            resolved
                .biome_definitions
                .as_ref()
                .unwrap()
                .entries
                .each_ref()
                .map(|profile| profile.biome),
            BiomeKind::ALL
        );
        assert!(resolved.same_terrain(&saved));
        let (store, _) = DedicatedStore::open(&directory.0, &resolved).unwrap();
        drop(store);
    }
}

#[test]
fn ten_profile_pre_volcanic_save_preserves_custom_terrain_and_disables_ashlands_after_restart() {
    let directory = Directory::new();
    fs::create_dir_all(&directory.0).unwrap();
    let mut profiles = BiomeDefinitions::default();
    profiles.entries[BiomeKind::Desert as usize].selection_weight = 0.72;
    let saved = WorldConfig {
        ecology_revision: 15,
        biome_definitions: Some(profiles),
        ..Default::default()
    };
    let mut legacy = toml::Value::try_from(WorldManifest::new(saved.clone())).unwrap();
    let entries = legacy["world"]["biome_definitions"]["entries"]
        .as_array_mut()
        .unwrap();
    assert_eq!(
        entries.pop().unwrap()["biome"].as_str(),
        Some("misty_coast")
    );
    assert_eq!(
        entries.pop().unwrap()["biome"].as_str(),
        Some("crystal_highlands")
    );
    assert_eq!(
        entries.pop().unwrap()["biome"].as_str(),
        Some("petrified_forest")
    );
    assert_eq!(entries.pop().unwrap()["biome"].as_str(), Some("swamp"));
    assert_eq!(entries.pop().unwrap()["biome"].as_str(), Some("savanna"));
    assert_eq!(entries.pop().unwrap()["biome"].as_str(), Some("ashlands"));
    assert_eq!(entries.len(), 10);
    save_toml(&legacy, directory.0.join("world.toml")).unwrap();

    for _ in 0..2 {
        let resolved =
            DedicatedStore::resolve_world_config(&directory.0, &WorldConfig::default()).unwrap();
        assert!(resolved.same_terrain(&saved));
        assert_eq!(resolved.ecology_revision, 15);
        assert_eq!(
            terra_world::TerraGenerator::new(resolved.clone())
                .unwrap()
                .biome_at(5901.5, -8180.5)
                .weights
                .ashlands,
            0.
        );
        let (store, _) = DedicatedStore::open(&directory.0, &resolved).unwrap();
        drop(store);
    }
}

#[test]
fn eleven_profile_save_keeps_its_ecology_and_ashlands_after_a_savanna_upgrade() {
    let directory = Directory::new();
    fs::create_dir_all(&directory.0).unwrap();
    let mut profiles = BiomeDefinitions::default();
    profiles.entries[BiomeKind::Ashlands as usize].selection_weight = 0.85;
    let saved = WorldConfig {
        ecology_revision: 16,
        biome_definitions: Some(profiles),
        ..Default::default()
    };
    let mut document = toml::Value::try_from(WorldManifest::new(saved.clone())).unwrap();
    let entries = document["world"]["biome_definitions"]["entries"]
        .as_array_mut()
        .unwrap();
    assert_eq!(
        entries.pop().unwrap()["biome"].as_str(),
        Some("misty_coast")
    );
    assert_eq!(
        entries.pop().unwrap()["biome"].as_str(),
        Some("crystal_highlands")
    );
    assert_eq!(
        entries.pop().unwrap()["biome"].as_str(),
        Some("petrified_forest")
    );
    assert_eq!(entries.pop().unwrap()["biome"].as_str(), Some("swamp"));
    assert_eq!(entries.pop().unwrap()["biome"].as_str(), Some("savanna"));
    save_toml(&document, directory.0.join("world.toml")).unwrap();
    for _ in 0..2 {
        let config =
            DedicatedStore::resolve_world_config(&directory.0, &WorldConfig::default()).unwrap();
        assert!(config.same_terrain(&saved));
        let terrain = terra_world::TerraGenerator::new(config.clone()).unwrap();
        assert_eq!(terrain.biome_at(-96.5, 1152.5).weights.savanna, 0.);
        let (store, _) = DedicatedStore::open(&directory.0, &config).unwrap();
        drop(store);
    }
}

#[test]
fn twelve_profile_save_keeps_its_savanna_settings_and_disables_swamps() {
    let directory = Directory::new();
    fs::create_dir_all(&directory.0).unwrap();
    let mut profiles = BiomeDefinitions::default();
    profiles.entries[BiomeKind::Savanna as usize].selection_weight = 0.79;
    let saved = WorldConfig {
        ecology_revision: 17,
        biome_definitions: Some(profiles),
        ..Default::default()
    };
    let mut document = toml::Value::try_from(WorldManifest::new(saved.clone())).unwrap();
    let entries = document["world"]["biome_definitions"]["entries"]
        .as_array_mut()
        .unwrap();
    assert_eq!(
        entries.pop().unwrap()["biome"].as_str(),
        Some("misty_coast")
    );
    assert_eq!(
        entries.pop().unwrap()["biome"].as_str(),
        Some("crystal_highlands")
    );
    assert_eq!(
        entries.pop().unwrap()["biome"].as_str(),
        Some("petrified_forest")
    );
    assert_eq!(entries.pop().unwrap()["biome"].as_str(), Some("swamp"));
    save_toml(&document, directory.0.join("world.toml")).unwrap();
    let resolved =
        DedicatedStore::resolve_world_config(&directory.0, &WorldConfig::default()).unwrap();
    assert!(resolved.same_terrain(&saved));
    let terrain = terra_world::TerraGenerator::new(resolved).unwrap();
    assert_eq!(terrain.biome_at(5264.5, -7311.5).weights.swamp, 0.);
}

#[test]
fn fourteen_profile_world_keeps_its_alpine_range_when_crystal_highlands_are_added() {
    let directory = Directory::new();
    fs::create_dir_all(&directory.0).unwrap();
    let saved = WorldConfig {
        ecology_revision: 20,
        biome_definitions: Some(BiomeDefinitions::default()),
        ..Default::default()
    };
    let mut document = toml::Value::try_from(WorldManifest::new(saved.clone())).unwrap();
    let entries = document["world"]["biome_definitions"]["entries"]
        .as_array_mut()
        .unwrap();
    assert_eq!(
        entries.pop().unwrap()["biome"].as_str(),
        Some("misty_coast")
    );
    assert_eq!(
        entries.pop().unwrap()["biome"].as_str(),
        Some("crystal_highlands")
    );
    save_toml(&document, directory.0.join("world.toml")).unwrap();
    let resolved =
        DedicatedStore::resolve_world_config(&directory.0, &WorldConfig::default()).unwrap();
    assert!(resolved.same_terrain(&saved));
    let terrain = terra_world::TerraGenerator::new(resolved).unwrap();
    assert_eq!(
        terrain.biome_at(-1392.5, 400.5).weights.crystal_highlands,
        0.
    );
}

#[test]
fn existing_spirit_revision_one_is_not_upgraded_by_revision_two_template() {
    let directory = Directory::new();
    let saved = WorldConfig {
        spirit_blossom_revision: 1,
        ..Default::default()
    };
    let (store, _) = DedicatedStore::open(&directory.0, &saved).unwrap();
    drop(store);
    let current = WorldConfig {
        spirit_blossom_revision: 2,
        ..saved.clone()
    };
    for _ in 0..2 {
        let resolved = DedicatedStore::resolve_world_config(&directory.0, &current).unwrap();
        assert_eq!(resolved.spirit_blossom_revision, 1);
        assert!(resolved.same_terrain(&saved));
        let (store, _) = DedicatedStore::open(&directory.0, &resolved).unwrap();
        drop(store);
    }
}
