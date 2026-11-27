use super::*;
use crate::{Config, LandscapeDefinitions, TerrainKind, WorldConfig};
use serde_json::json;
use std::{fs, path::PathBuf};

struct Fixture(PathBuf);

#[test]
fn saved_selection_weights_default_to_legacy_and_validate_before_world_creation() {
    let definitions = BiomeDefinitions::default();
    assert_eq!(definitions.get(BiomeKind::Highland).selection_weight, 0.65);
    for kind in BiomeKind::ALL {
        let mut value = serde_json::to_value(definitions.get(kind)).unwrap();
        value.as_object_mut().unwrap().remove("selection_weight");
        let legacy: BiomeDefinition = serde_json::from_value(value).unwrap();
        assert_eq!(legacy.selection_weight, 1.0);
    }
    for invalid in [-0.1, 4.1, f64::INFINITY, f64::NAN] {
        let mut definitions = definitions.clone();
        definitions.entries[BiomeKind::Highland as usize].selection_weight = invalid;
        assert!(definitions.validate().is_err());
    }
    let mut config = WorldConfig::default();
    let mut changed = config.clone();
    changed.biome_definitions = Some(definitions.clone());
    assert!(!config.same_terrain(&changed));
    config.biome_definitions = Some(definitions);
    assert!(config.same_terrain(&changed));
    changed.biome_definitions.as_mut().unwrap().entries[BiomeKind::Highland as usize]
        .selection_weight = 0.8;
    assert!(!config.same_terrain(&changed));
}

impl Fixture {
    fn new() -> Self {
        let directory = std::env::temp_dir().join(format!(
            "terra-biome-config-{}",
            crate::PlayerUuid::new_v4()
        ));
        fs::create_dir(&directory).unwrap();
        let fixture = Self(directory);
        for biome in BiomeKind::ALL {
            fs::copy(
                crate::asset_directory()
                    .join("biome")
                    .join(biome.file_name()),
                fixture.0.join(biome.file_name()),
            )
            .unwrap();
        }
        fixture
    }

    fn write(&self, biome: BiomeKind, value: serde_json::Value) {
        fs::write(
            self.0.join(biome.file_name()),
            serde_json::to_string_pretty(&value).unwrap(),
        )
        .unwrap();
    }

    fn load(&self) -> Result<BiomeDefinitions, ConfigError> {
        BiomeDefinitions::load_directory(&self.0)
    }

    fn write_landscapes(&self, value: serde_json::Value) {
        fs::write(
            self.0.join("terrain_profiles.json"),
            serde_json::to_string_pretty(&value).unwrap(),
        )
        .unwrap();
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn builtin_ranges_and_marine_habitats_match_the_world_contract() {
    let definitions = BiomeDefinitions::default();
    definitions.validate().unwrap();
    for (biome, minimum, maximum) in [
        (BiomeKind::Plains, 32.0, 60.0),
        (BiomeKind::Highland, 80.0, 200.0),
        (BiomeKind::Desert, 50.0, 100.0),
        (BiomeKind::Forest, 40.0, 90.0),
        (BiomeKind::CherryForest, 45.0, 115.0),
        (BiomeKind::Ocean, -98.0, 22.0),
    ] {
        let terrain = definitions.get(biome).terrain;
        assert_eq!((terrain.min_height, terrain.max_height), (minimum, maximum));
    }
    for kind in BiomeKind::ALL {
        assert_eq!(definitions.get(kind).biome, kind);
        assert_eq!(
            definitions.get(kind).ocean.is_some(),
            kind == BiomeKind::Ocean
        );
    }
    let ocean = definitions.get(BiomeKind::Ocean).ocean.unwrap();
    assert!(ocean.warm.coral_density > 0.0);
    assert_eq!(ocean.cold.coral_density, 0.0);
    assert_eq!(ocean.swamp.coral_density, 0.0);
    assert!(ocean.warm.floor.sand > ocean.cold.floor.sand);
    assert!(ocean.cold.floor.gravel + ocean.cold.floor.mud > 0.5);
    assert!(ocean.swamp.floor.mud + ocean.swamp.floor.dirt + ocean.swamp.floor.sand > 0.8);
    assert!(ocean.clay_frequency > 0.0 && ocean.clay_frequency < 1.0);
    for habitat in [ocean.warm, ocean.cold, ocean.swamp] {
        assert!(habitat.seagrass_density > 0.0);
        assert!((habitat.floor.values().into_iter().sum::<f64>() - 1.0).abs() < 1e-12);
    }
}

#[test]
fn shipped_assets_load_and_missing_biome_files_report_their_path() {
    let loaded = BiomeDefinitions::load_directory(crate::asset_directory().join("biome")).unwrap();
    assert_eq!(loaded, BiomeDefinitions::default());
    let fixture = Fixture::new();
    let missing = fixture.0.join(BiomeKind::Forest.file_name());
    fs::remove_file(&missing).unwrap();
    match fixture.load().unwrap_err() {
        ConfigError::Io { path, source } => {
            assert_eq!(path, missing);
            assert_eq!(source.kind(), std::io::ErrorKind::NotFound);
        }
        other => panic!("expected missing biome path, got {other}"),
    }
}

#[test]
fn invalid_height_ranges_and_biome_order_are_rejected() {
    for (minimum, maximum) in [
        (60.0, 32.0),
        (32.0, 32.0),
        (-121.0, 60.0),
        (32.0, 351.0),
        (f64::NAN, 60.0),
        (32.0, f64::INFINITY),
    ] {
        let mut definitions = BiomeDefinitions::default();
        let terrain = &mut definitions.entries[BiomeKind::Plains as usize].terrain;
        terrain.min_height = minimum;
        terrain.max_height = maximum;
        assert!(
            definitions.validate().is_err(),
            "range {minimum}..{maximum}"
        );
    }
    let mut definitions = BiomeDefinitions::default();
    definitions
        .entries
        .swap(BiomeKind::Forest as usize, BiomeKind::Plains as usize);
    assert!(definitions.validate().is_err());
    let fixture = Fixture::new();
    let mut plains =
        serde_json::to_value(BiomeDefinitions::default().get(BiomeKind::Plains)).unwrap();
    plains["terrain"]["min_height"] = json!(75.0);
    fixture.write(BiomeKind::Plains, plains);
    assert!(fixture.load().is_err());
    fixture.write(
        BiomeKind::Plains,
        serde_json::to_value(BiomeDefinitions::default().get(BiomeKind::Forest)).unwrap(),
    );
    assert!(
        fixture.load().is_err(),
        "a biome ID must match its file name"
    );
}

#[test]
fn unknown_json_fields_and_incorrect_ocean_sections_are_rejected() {
    let fixture = Fixture::new();
    for nested in [false, true] {
        let mut plains =
            serde_json::to_value(BiomeDefinitions::default().get(BiomeKind::Plains)).unwrap();
        if nested {
            plains["terrain"]["height_offset"] = json!(5.0);
        } else {
            plains["unrecognized"] = json!(true);
        }
        fixture.write(BiomeKind::Plains, plains);
        let error = fixture.load().unwrap_err().to_string();
        assert!(
            error.contains("plains.json") && error.contains("unknown field"),
            "{error}"
        );
    }
    let mut definitions = BiomeDefinitions::default();
    definitions.entries[BiomeKind::Plains as usize].ocean = Some(OceanBiomeConfig::default());
    assert!(definitions.validate().is_err());
    let mut definitions = BiomeDefinitions::default();
    definitions.entries[BiomeKind::Ocean as usize].ocean = None;
    assert!(definitions.validate().is_err());
    let mut ocean =
        serde_json::to_value(BiomeDefinitions::default().get(BiomeKind::Ocean)).unwrap();
    ocean["ocean"]["cold"]["floor"]["peat"] = json!(0.2);
    assert!(serde_json::from_value::<BiomeDefinition>(ocean).is_err());
}

#[test]
fn invalid_marine_floor_weights_are_rejected_for_every_habitat() {
    for floor in [
        MarineFloorWeights {
            sand: 0.0,
            gravel: 0.0,
            mud: 0.0,
            dirt: 0.0,
            stone: 0.0,
        },
        MarineFloorWeights {
            sand: -1.0,
            ..Default::default()
        },
        MarineFloorWeights {
            mud: f64::NAN,
            ..Default::default()
        },
        MarineFloorWeights {
            dirt: f64::INFINITY,
            ..Default::default()
        },
        MarineFloorWeights {
            sand: f64::MAX,
            gravel: f64::MAX,
            ..Default::default()
        },
    ] {
        for habitat in 0..3 {
            let mut definitions = BiomeDefinitions::default();
            let ocean = definitions.entries[BiomeKind::Ocean as usize]
                .ocean
                .as_mut()
                .unwrap();
            match habitat {
                0 => ocean.warm.floor = floor,
                1 => ocean.cold.floor = floor,
                _ => ocean.swamp.floor = floor,
            }
            assert!(
                definitions.normalize().is_err(),
                "habitat {habitat}, floor {floor:?}"
            );
        }
    }
    let fixture = Fixture::new();
    let mut ocean =
        serde_json::to_value(BiomeDefinitions::default().get(BiomeKind::Ocean)).unwrap();
    ocean["ocean"]["swamp"]["floor"]["sand"] = json!(-0.25);
    fixture.write(BiomeKind::Ocean, ocean);
    assert!(fixture.load().is_err());
}

#[test]
fn custom_json_values_are_normalized_and_saved_as_an_immutable_terrain_snapshot() {
    let fixture = Fixture::new();
    let mut plains =
        serde_json::to_value(BiomeDefinitions::default().get(BiomeKind::Plains)).unwrap();
    plains["terrain"]["min_height"] = json!(34.0);
    plains["terrain"]["max_height"] = json!(57.0);
    plains["terrain"]["tree_density"] = json!(0.31);
    fixture.write(BiomeKind::Plains, plains.clone());
    let mut ocean =
        serde_json::to_value(BiomeDefinitions::default().get(BiomeKind::Ocean)).unwrap();
    ocean["ocean"]["warm"]["floor"] =
        json!({ "sand": 8.0, "gravel": 0.0, "mud": 2.0, "dirt": 0.0, "stone": 0.0 });
    fixture.write(BiomeKind::Ocean, ocean);
    let definitions = fixture.load().unwrap();
    assert_eq!(
        definitions
            .get(BiomeKind::Ocean)
            .ocean
            .unwrap()
            .warm
            .floor
            .sand,
        0.8
    );
    assert_eq!(
        definitions
            .get(BiomeKind::Ocean)
            .ocean
            .unwrap()
            .warm
            .floor
            .mud,
        0.2
    );
    let world = WorldConfig {
        sea_level: 17.5,
        biome_definitions: Some(definitions),
        ..Default::default()
    };
    let path = fixture.0.join("world.toml");
    world.save(&path).unwrap();
    plains["terrain"]["max_height"] = json!(59.0);
    fixture.write(BiomeKind::Plains, plains);
    let changed_assets = fixture.load().unwrap();
    let restored = WorldConfig::load(&path).unwrap();
    assert_eq!(restored, world);
    assert!(restored.same_terrain(&world));
    assert_eq!(restored.sea_level, 17.5);
    let saved_plains = restored
        .biome_definitions
        .as_ref()
        .unwrap()
        .get(BiomeKind::Plains)
        .terrain;
    assert_eq!(
        (
            saved_plains.min_height,
            saved_plains.max_height,
            saved_plains.tree_density
        ),
        (34.0, 57.0, 0.31)
    );
    assert!(!world.same_terrain(&WorldConfig {
        biome_definitions: Some(changed_assets),
        ..world.clone()
    }));
    assert!(!world.same_terrain(&WorldConfig {
        biome_definitions: None,
        ..world.clone()
    }));
    assert!(!world.same_terrain(&WorldConfig {
        sea_level: 18.5,
        ..world.clone()
    }));
}

#[test]
fn missing_saved_biome_metadata_preserves_legacy_sea_level_and_absent_snapshot() {
    let legacy = WorldConfig::from_toml("seed = 77").unwrap();
    assert_eq!(legacy.landscape_revision, 0);
    assert_eq!(legacy.sea_level, 8.0);
    assert!(legacy.biome_definitions.is_none());
    let serialized = toml::to_string(&legacy).unwrap();
    assert!(!serialized.contains("biome_definitions"));
    assert_eq!(WorldConfig::from_toml(&serialized).unwrap(), legacy);
    let custom = WorldConfig::from_toml("seed = 77\nsea_level = 14.5").unwrap();
    assert_eq!(custom.sea_level, 14.5);
    assert!(custom.biome_definitions.is_none());
    assert_eq!(
        WorldConfig::from_toml(&toml::to_string(&custom).unwrap()).unwrap(),
        custom
    );
    let fresh = WorldConfig::default();
    assert_eq!(fresh.landscape_revision, 9);
    assert_eq!(fresh.sea_level, 8.0);
    assert!(!fresh.rivers);
    assert!(fresh.biome_definitions.is_none());
}

#[test]
fn optional_landscape_file_loads_custom_profiles_and_saves_an_immutable_snapshot() {
    let fixture = Fixture::new();
    assert!(!fixture.0.join("terrain_profiles.json").exists());
    assert_eq!(fixture.load().unwrap(), BiomeDefinitions::default());

    let mut profiles = LandscapeDefinitions {
        warp_strength: 37.5,
        dune_wind_angle: -37.0,
        ..Default::default()
    };
    let forest = &mut profiles.profiles[TerrainKind::Forest as usize];
    forest.min_height = 41.5;
    forest.max_height = 87.5;
    forest.weight = 0.45;
    fixture.write_landscapes(serde_json::to_value(&profiles).unwrap());
    let definitions = fixture.load().unwrap();
    assert_eq!(definitions.landscapes, profiles);
    assert_eq!(definitions.entries, BiomeDefinitions::default().entries);
    let world = WorldConfig {
        landscape_revision: 5,
        biome_definitions: Some(definitions),
        ..Default::default()
    };
    let saved = fixture.0.join("world.toml");
    world.save(&saved).unwrap();
    profiles.warp_strength = 128.0;
    fixture.write_landscapes(serde_json::to_value(profiles).unwrap());
    let restored = WorldConfig::load(&saved).unwrap();
    assert_eq!(restored, world);
    assert!(restored.same_terrain(&world));
    assert!(!restored.same_terrain(&WorldConfig {
        biome_definitions: Some(fixture.load().unwrap()),
        ..world
    }));
    fs::remove_file(fixture.0.join("terrain_profiles.json")).unwrap();
    assert_eq!(fixture.load().unwrap(), BiomeDefinitions::default());
}

#[test]
fn landscape_json_rejects_unknown_missing_fields_and_nonfinite_literals_with_its_path() {
    let fixture = Fixture::new();
    let defaults = serde_json::to_value(LandscapeDefinitions::default()).unwrap();
    for nested in [false, true] {
        let mut value = defaults.clone();
        if nested {
            value["profiles"][0]["height_offset"] = json!(5.0);
        } else {
            value["unrecognized"] = json!(true);
        }
        fixture.write_landscapes(value);
        let error = fixture.load().unwrap_err().to_string();
        assert!(
            error.contains("terrain_profiles.json") && error.contains("unknown field"),
            "{error}"
        );
    }
    for nested in [false, true] {
        let mut value = defaults.clone();
        if nested {
            value["profiles"][0]
                .as_object_mut()
                .unwrap()
                .remove("roughness");
        } else {
            value.as_object_mut().unwrap().remove("warp_scale");
        }
        fixture.write_landscapes(value);
        let error = fixture.load().unwrap_err().to_string();
        assert!(
            error.contains("terrain_profiles.json") && error.contains("missing field"),
            "{error}"
        );
    }
    for literal in ["NaN", "Infinity", "-Infinity"] {
        fs::write(
            fixture.0.join("terrain_profiles.json"),
            format!("{{\"warp_scale\":{literal}}}"),
        )
        .unwrap();
        let error = fixture.load().unwrap_err().to_string();
        assert!(
            error.contains("terrain_profiles.json"),
            "{literal}: {error}"
        );
        assert!(
            !error.contains("missing field"),
            "nonfinite JSON must fail before required-field validation: {error}"
        );
    }
}

#[test]
fn landscape_loader_rejects_changed_ids_incorrect_counts_and_invalid_settings() {
    let fixture = Fixture::new();
    let defaults = serde_json::to_value(LandscapeDefinitions::default()).unwrap();
    let mut reordered = defaults.clone();
    reordered["profiles"].as_array_mut().unwrap().swap(0, 1);
    fixture.write_landscapes(reordered);
    assert!(fixture.load().is_err());
    let mut duplicate = defaults.clone();
    duplicate["profiles"][1]["kind"] = json!("plains");
    fixture.write_landscapes(duplicate);
    assert!(fixture.load().is_err());
    let mut unknown = defaults.clone();
    unknown["profiles"][0]["kind"] = json!("unknown_terrain");
    fixture.write_landscapes(unknown);
    assert!(fixture.load().is_err());
    for count in [9, 11] {
        let mut value = defaults.clone();
        let profiles = value["profiles"].as_array_mut().unwrap();
        if count == 9 {
            profiles.pop();
        } else {
            profiles.push(profiles[0].clone());
        }
        fixture.write_landscapes(value);
        assert!(fixture.load().is_err(), "profile count {count}");
    }
    for (field, value) in [
        ("warp_scale", 63.9),
        ("warp_scale", 8192.1),
        ("warp_strength", -0.1),
        ("warp_strength", 512.1),
        ("region_scale", 127.9),
        ("region_scale", 16384.1),
        ("dune_wind_angle", -360.1),
        ("dune_wind_angle", 360.1),
    ] {
        let mut profiles = defaults.clone();
        profiles[field] = json!(value);
        fixture.write_landscapes(profiles);
        assert!(fixture.load().is_err(), "{field}={value}");
    }
    for (field, value) in [
        ("min_height", -120.1),
        ("min_height", 60.0),
        ("max_height", 32.0),
        ("max_height", 350.1),
        ("horizontal_scale", 31.9),
        ("horizontal_scale", 8192.1),
        ("roughness", -0.1),
        ("roughness", 1.1),
        ("ridge_strength", 1.1),
        ("erosion", 1.1),
        ("overhangs", 1.1),
        ("weight", -0.1),
        ("weight", 10.1),
    ] {
        let mut profiles = defaults.clone();
        profiles["profiles"][0][field] = json!(value);
        fixture.write_landscapes(profiles);
        assert!(fixture.load().is_err(), "profile {field}={value}");
    }
    for all_profiles in [false, true] {
        let mut value = defaults.clone();
        for profile in value["profiles"].as_array_mut().unwrap() {
            if all_profiles || profile["kind"] == "ocean" {
                profile["weight"] = json!(0.0);
            }
        }
        fixture.write_landscapes(value);
        assert!(fixture.load().is_err());
    }
}

#[test]
fn missing_saved_landscapes_restore_immutable_original_profiles_and_keep_saved_biomes() {
    for revision in [4, 5] {
        let mut world = WorldConfig {
            landscape_revision: revision,
            sea_level: 17.5,
            biome_definitions: Some(BiomeDefinitions::default()),
            ..Default::default()
        };
        let definitions = world.biome_definitions.as_mut().unwrap();
        definitions.landscapes = LandscapeDefinitions::legacy_v5();
        definitions.entries[BiomeKind::Forest as usize]
            .terrain
            .tree_density = 0.29;
        let mut json = serde_json::to_value(&world).unwrap();
        json["biome_definitions"]
            .as_object_mut()
            .unwrap()
            .remove("landscapes");
        let restored: WorldConfig = serde_json::from_value(json).unwrap();
        restored.validate().unwrap();
        assert_eq!(restored, world);
        assert!(restored.same_terrain(&world));
        let mut toml = toml::Value::try_from(&world).unwrap();
        toml["biome_definitions"]
            .as_table_mut()
            .unwrap()
            .remove("landscapes");
        assert_eq!(
            WorldConfig::from_toml(&toml::to_string(&toml).unwrap()).unwrap(),
            world
        );
    }
}

#[test]
fn complete_old_landscape_snapshots_roundtrip_without_adopting_new_defaults() {
    let mut original = WorldConfig {
        landscape_revision: 5,
        sea_level: 22.0,
        biome_definitions: Some(BiomeDefinitions::default()),
        ..Default::default()
    };
    original.biome_definitions.as_mut().unwrap().landscapes = LandscapeDefinitions::legacy_v5();
    for restored in [
        serde_json::from_str::<WorldConfig>(&serde_json::to_string(&original).unwrap()).unwrap(),
        WorldConfig::from_toml(&toml::to_string(&original).unwrap()).unwrap(),
    ] {
        assert_eq!(restored, original);
        assert!(restored.same_terrain(&original));
        assert_ne!(
            restored.biome_definitions,
            WorldConfig::default().biome_definitions
        );
    }
}

#[test]
fn legacy_eight_biome_snapshots_append_spirit_profile_without_retrogen() {
    let original = BiomeDefinitions::default();
    let mut json = serde_json::to_value(&original).unwrap();
    json["entries"].as_array_mut().unwrap().truncate(8);
    let restored: BiomeDefinitions = serde_json::from_value(json).unwrap();
    assert_eq!(&restored.entries[..8], &original.entries[..8]);
    assert_eq!(
        restored.get(BiomeKind::SpiritBlossom),
        original.get(BiomeKind::SpiritBlossom)
    );
    restored.validate().unwrap();
    let old = WorldConfig::from_toml("seed=42").unwrap();
    assert_eq!(old.spirit_blossom_revision, 0);
    assert_eq!(WorldConfig::default().spirit_blossom_revision, 2);
}

#[test]
fn climate_classes_allow_temperate_neighbors_but_separate_desert_from_alpine() {
    use super::BiomeClimate;
    assert_eq!(BiomeKind::Highland.climate(), BiomeClimate::Cold);
    assert_eq!(BiomeKind::Desert.climate(), BiomeClimate::Hot);
    assert_eq!(BiomeKind::ClifftedDesert.climate(), BiomeClimate::Hot);
    assert_eq!(BiomeKind::Forest.climate(), BiomeClimate::Warm);
    for a in [BiomeClimate::Cold, BiomeClimate::Warm, BiomeClimate::Hot] {
        for b in [BiomeClimate::Cold, BiomeClimate::Warm, BiomeClimate::Hot] {
            assert_eq!(a.allows_neighbor(b), b.allows_neighbor(a));
            assert_eq!(
                a.allows_neighbor(b),
                a == b || a == BiomeClimate::Warm || b == BiomeClimate::Warm
            );
        }
    }
}
