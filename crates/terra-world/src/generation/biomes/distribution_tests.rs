use super::*;
use crate::TerraGenerator;
use terra_core::{BiomeDefinitions, WorldConfig, WorldSeed};

fn terrain(seed: u64, definitions: Option<BiomeDefinitions>) -> TerraGenerator {
    TerraGenerator::new(WorldConfig {
        seed: WorldSeed(seed),
        biome_definitions: definitions,
        landscape_revision: 1,
        river_terrain_revision: 3,
        caves: false,
        trees: false,
        structures_revision: 0,
        ..Default::default()
    })
    .unwrap()
}

#[test]
fn unit_selection_snapshots_preserve_legacy_columns_and_climate_exactly() {
    let mut definitions = BiomeDefinitions::default();
    for entry in &mut definitions.entries {
        entry.selection_weight = 1.0;
    }
    for seed in [42042, 7, 99173] {
        let legacy = terrain(seed, None);
        let saved = terrain(seed, Some(definitions.clone()));
        for [x, z] in [
            [-4096.5, 768.5],
            [-16.001, -16.001],
            [-16.0, -16.0],
            [-15.999, -15.999],
            [-0.5, -0.5],
            [0.5, 0.5],
            [4096.5, -768.5],
        ] {
            assert_eq!(legacy.biome_at(x, z), saved.biome_at(x, z));
            assert_eq!(legacy.surface_column(x, z), saved.surface_column(x, z));
        }
    }
}

#[test]
fn weighted_highlands_are_less_common_without_changing_continents_or_reload_order() {
    let mut old_count = 0;
    let mut new_count = 0;
    for seed in [42042, 7, 99173, 12345, 86420] {
        let legacy = terrain(seed, None);
        let config = Some(BiomeDefinitions::default());
        let current = terrain(seed, config.clone());
        let reloaded = terrain(seed, config);
        for z in (-4096..4096).step_by(256) {
            for x in (-4096..4096).step_by(256) {
                let [x, z] = [f64::from(x) + 53.5, f64::from(z) + 79.5];
                let before = legacy.climate_biome_at(x, z);
                let after = current.climate_biome_at(x, z);
                assert_eq!(before.weights.ocean, after.weights.ocean);
                assert!((after.weights.values().iter().sum::<f64>() - 1.0).abs() < 1e-12);
                old_count +=
                    usize::from(legacy.biome_at(x, z).weights.dominant() == BiomeKind::Highland);
                new_count +=
                    usize::from(current.biome_at(x, z).weights.dominant() == BiomeKind::Highland);
                assert_eq!(current.surface_column(x, z), reloaded.surface_column(x, z));
            }
        }
    }
    assert!(new_count > 0, "highlands must remain present");
    assert!(new_count < old_count * 9 / 10, "{old_count} -> {new_count}");
}

#[test]
fn zero_land_weights_fall_back_without_nan_or_changing_the_ocean() {
    let mut definitions = BiomeDefinitions::default();
    for entry in &mut definitions.entries {
        entry.selection_weight = 0.0;
    }
    let sample = terrain(42042, None).biome_at(-512.5, 512.5);
    assert_eq!(sample.with_distribution(&definitions), sample);
}
