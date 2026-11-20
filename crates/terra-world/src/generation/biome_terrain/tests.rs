use super::*;
use terra_core::{WorldConfig, WorldSeed};

fn generator(seed: u64) -> TerraGenerator {
    TerraGenerator::new(WorldConfig {
        seed: WorldSeed(seed),
        landscape_revision: 4,
        sea_level: 22.0,
        biome_definitions: Some(terra_core::BiomeDefinitions::default()),
        structures_revision: 0,
        rivers: false,
        ..Default::default()
    })
    .unwrap()
}

fn pure(kind: BiomeKind) -> BiomeSample {
    let mut weights = [0.0; 10];
    weights[kind as usize] = 1.0;
    let [
        ocean,
        plains,
        forest,
        desert,
        highland,
        mushroom_park,
        cliffted_desert,
        cherry_forest,
        spirit_blossom,
        blood_forest,
    ] = weights;
    BiomeSample {
        temperature: 0.5,
        humidity: 0.65,
        weights: BiomeWeights {
            ocean,
            plains,
            forest,
            desert,
            highland,
            mushroom_park,
            cliffted_desert,
            cherry_forest,
            spirit_blossom,
            blood_forest,
        },
    }
}

#[test]
fn every_biome_respects_its_json_height_band_without_flat_clipping() {
    for seed in [42042, 777, 2262437629133911810] {
        let generator = generator(seed);
        for kind in BiomeKind::ALL {
            let profile = generator.biome_definitions().get(kind).terrain;
            let mut min = f64::INFINITY;
            let mut max = f64::NEG_INFINITY;
            let mut values = std::collections::HashSet::new();
            for i in -64..=64 {
                let x = f64::from(i * 73);
                let z = f64::from(i * i - 390);
                let h = generator.base_height_with_biome(x, z, pure(kind));
                assert!(
                    (profile.min_height..=profile.max_height).contains(&h),
                    "{kind:?} {h}"
                );
                min = min.min(h);
                max = max.max(h);
                values.insert((h * 1000.0).round() as i64);
            }
            assert!(
                max - min > (profile.max_height - profile.min_height) * 0.08,
                "{kind:?} became flat"
            );
            assert!(values.len() > 110, "{kind:?} clipped to repeated plateaus");
        }
    }
}

#[test]
fn configured_height_is_seeded_continuous_and_order_independent() {
    let a = generator(42042);
    let b = generator(42042);
    let c = generator(777);
    let mut changed = 0;
    for x in [-4096.0, -1024.0, -16.0, -0.001, 0.0, 16.0, 1024.0] {
        for z in [-4032.0, -16.0, 0.0, 16.0, 1552.0] {
            let sample = a.climate_biome_at(x, z);
            let h = a.base_height_with_biome(x, z, sample);
            let _ = b.base_height_with_biome(z, x, b.climate_biome_at(z, x));
            assert_eq!(h, b.base_height_with_biome(x, z, b.climate_biome_at(x, z)));
            let neighbor = a.base_height_with_biome(x + 0.001, z, a.climate_biome_at(x + 0.001, z));
            assert!((h - neighbor).abs() < 0.05, "height seam at {x},{z}");
            changed += usize::from(
                (h - c.base_height_with_biome(x, z, c.climate_biome_at(x, z))).abs() > 0.1,
            );
        }
    }
    assert!(changed > 20);
}

#[test]
fn changing_json_snapshot_changes_heights_soil_and_tree_density_only_for_new_contract() {
    let a = generator(42042);
    let mut config = a.config().clone();
    let plains =
        &mut config.biome_definitions.as_mut().unwrap().entries[BiomeKind::Plains as usize].terrain;
    plains.min_height = 100.0;
    plains.max_height = 120.0;
    plains.soil_depth = 2.0;
    plains.tree_density = 0.9;
    let b = TerraGenerator::new(config.clone()).unwrap();
    let sample = pure(BiomeKind::Plains);
    assert!(b.base_height_with_biome(-16.0, 32.0, sample) >= 100.0);
    assert_eq!(b.configured_soil_depth(sample), 2.0);
    assert_eq!(b.configured_tree_density(sample), 0.9);
    config.landscape_revision = 3;
    config.sea_level = 8.0;
    let legacy_a = TerraGenerator::new(config.clone()).unwrap();
    config.biome_definitions = None;
    let legacy_b = TerraGenerator::new(config).unwrap();
    assert_eq!(
        legacy_a.base_height_with_biome(-16.0, 32.0, sample),
        legacy_b.base_height_with_biome(-16.0, 32.0, sample)
    );
}

#[test]
fn plains_label_cannot_inherit_highland_elevation() {
    let generator = generator(42042);
    let mut checked = 0;
    for x in (-6000..6000).step_by(311) {
        for z in (-6000..6000).step_by(317) {
            let (x, z) = (f64::from(x), f64::from(z));
            let climate = generator.climate_biome_at(x, z);
            let height = generator.base_height_with_biome(x, z, climate);
            let mut ecology = generator.surface_ecology(climate, height).weights;
            ecology.ocean = 0.0;
            if ecology.dominant() == BiomeKind::Plains {
                assert!(height <= 60.0, "Plains at Y={height} ({x},{z})");
                checked += 1;
            }
            if ecology.dominant() == BiomeKind::Highland {
                assert!(
                    height >= 80.0,
                    "Highlands below configured range at {height}"
                );
            }
        }
    }
    assert!(checked > 20);
}

#[test]
fn deep_ocean_has_a_solid_cave_roof_after_all_chamber_csg() {
    for seed in [42042, 777] {
        let generator = generator(seed);
        let mut checked = 0;
        for x in (-7000..7000).step_by(997) {
            for z in (-7000..7000).step_by(991) {
                let (x, z) = (f64::from(x), f64::from(z));
                let column = generator.surface_column(x, z);
                if column.biome.weights.ocean <= 0.45
                    || column
                        .water_level
                        .is_none_or(|level| level - column.height < 8.0)
                {
                    continue;
                }
                for depth in [0.5, 1.5, 2.5, 3.5, 4.5, 5.5] {
                    assert!(
                        generator
                            .cave_density_with_column([x, column.height - depth, z], column)
                            .0
                            < 0.0
                    );
                }
                checked += 1;
            }
        }
        assert!(checked > 5, "no deep-ocean fixtures for seed {seed}");
    }
}

#[test]
fn explicit_flat_world_is_still_flat() {
    let generator = TerraGenerator::new(WorldConfig {
        base_height: 42.0,
        hill_amplitude: 0.0,
        mountain_amplitude: 0.0,
        rivers: false,
        structures_revision: 0,
        ..Default::default()
    })
    .unwrap();
    assert_eq!(
        generator.base_height_with_biome(-150.0, 200.0, pure(BiomeKind::Highland)),
        42.0
    );
}

#[test]
fn variable_profile_scale_does_not_amplify_far_world_gradients() {
    let field = Perlin::new(71);
    for x in [
        -100_000_000.0,
        -1_000_000.0,
        -16.0,
        0.0,
        1_000_000.0,
        100_000_000.0,
    ] {
        for scale in [32.0, 97.0, 256.0, 500.0, 8192.0] {
            let a = scaled_noise(&field, x, -1500.0, scale, [0.37, 0.71]);
            let b = scaled_noise(&field, x + 0.01, -1500.0, scale + 0.001, [0.37, 0.71]);
            assert!(
                (a - b).abs() < 0.005,
                "scale gradient at {x} / {scale}: {a} {b}"
            );
        }
    }
}

#[test]
fn foothill_ecology_blends_across_non_overlapping_climate_bands() {
    let terrain = generator(42042);
    let mut biome = pure(BiomeKind::Plains);
    biome.weights.plains = 0.6;
    biome.weights.highland = 0.4;
    let mut previous = terrain.surface_ecology(biome, 45.0);
    for step in 1..=4500 {
        let height = 45.0 + f64::from(step) * 0.01;
        let sample = terrain.surface_ecology(biome, height);
        for (a, b) in previous
            .weights
            .values()
            .into_iter()
            .zip(sample.weights.values())
        {
            assert!(
                (a - b).abs() < 0.005,
                "ecology seam at Y={height}: {previous:?} -> {sample:?}"
            );
        }
        assert!((sample.weights.values().iter().sum::<f64>() - 1.0).abs() < 1e-10);
        previous = sample;
    }
}

#[test]
fn legacy_eroded_shores_never_acquire_new_biomes_through_height_fallback() {
    let terrain = TerraGenerator::new(WorldConfig {
        landscape_revision: 4,
        blood_forest_revision: 0,
        spirit_blossom_revision: 0,
        ..Default::default()
    })
    .unwrap();
    for height in [-10., 0., 15., 25., 31.] {
        let result = terrain.surface_ecology(pure(BiomeKind::Plains), height);
        assert_eq!(result.weights.blood_forest, 0.);
        assert_eq!(result.weights.spirit_blossom, 0.);
        assert!((result.weights.values().iter().sum::<f64>() - 1.).abs() < 1e-9);
    }
}
