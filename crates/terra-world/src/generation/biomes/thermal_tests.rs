use super::*;
use terra_core::{BiomeClimate, WorldConfig};

#[test]
fn thermal_weights_preserve_water_and_mass_with_a_real_temperate_band() {
    let original = BiomeSample {
        temperature: 0.5,
        humidity: 0.65,
        weights: BiomeWeights {
            ocean: 0.1,
            plains: 0.15,
            forest: 0.1,
            desert: 0.2,
            highland: 0.25,
            mushroom_park: 0.1,
            cliffted_desert: 0.1,
            cherry_forest: 0.0,
            spirit_blossom: 0.0,
            blood_forest: 0.0,
            ashlands: 0.0,
            savanna: 0.0,
            swamp: 0.0,
            petrified_forest: 0.0,
            crystal_highlands: 0.0,
            misty_coast: 0.0,
        },
    };
    for i in 0..=100 {
        let sample = BiomeSample {
            temperature: i as f64 / 100.0,
            ..original
        }
        .with_thermal_buffers();
        assert!((sample.weights.values().iter().sum::<f64>() - 1.0).abs() < 1e-12);
        assert_eq!(sample.weights.ocean, original.weights.ocean);
        assert_eq!(sample.weights.highland * sample.aridity(), 0.0);
        if (40..=60).contains(&i) {
            assert_eq!(sample.weights.highland + sample.aridity(), 0.0);
        }
    }
    for temperature in [0.28, 0.4, 0.6, 0.72] {
        let a = BiomeSample {
            temperature: temperature - 1e-6,
            ..original
        }
        .with_thermal_buffers();
        let b = BiomeSample {
            temperature: temperature + 1e-6,
            ..original
        }
        .with_thermal_buffers();
        assert!(
            a.weights
                .values()
                .into_iter()
                .zip(b.weights.values())
                .all(|(a, b)| (a - b).abs() < 0.0001)
        );
    }
}

#[test]
fn larger_regions_scale_climate_and_sanctuaries_in_world_space_without_chunk_edges() {
    let old = ClimateSampler::new(42042, 2048.0, 4096.0);
    let broad = old.clone().larger_regions(true);
    for [x, z] in [[-7139.5, 881.3], [345.1, -1899.7], [9910.2, 4791.1]] {
        let a = old.sample(x / 2.0, z / 2.0);
        let b = broad.sample(x, z);
        assert_eq!(a, b);
        assert_eq!(
            old.with_spirit_blossom(a, x / 2.0, z / 2.0),
            broad.with_spirit_blossom(b, x, z)
        );
        assert_eq!(
            old.with_blood_forest(a, x / 2.0, z / 2.0),
            broad.with_blood_forest(b, x, z)
        );
    }
}

#[test]
fn generated_neighboring_land_biomes_cannot_jump_from_hot_to_cold() {
    let terrain = crate::TerraGenerator::new(WorldConfig {
        landscape_revision: 8,
        rivers: false,
        water: false,
        trees: false,
        caves: false,
        cave_structure_revision: 0,
        structures_revision: 0,
        ..Default::default()
    })
    .unwrap();
    for z in (-8192..=8192).step_by(512) {
        for x in (-8192..=8192).step_by(512) {
            let p = [x as f64 + 0.5, z as f64 + 0.5];
            let sample = terrain.surface_column(p[0], p[1]);
            let kind = terrain.surface_biome(&sample);
            for delta in [[-16.0, 0.0], [16.0, 0.0], [0.0, -16.0], [0.0, 16.0]] {
                let neighbor = terrain.surface_column(p[0] + delta[0], p[1] + delta[1]);
                assert!(
                    BiomeClimate::allows_neighbor(
                        kind.climate(),
                        terrain.surface_biome(&neighbor).climate()
                    ),
                    "thermal jump at {p:?}"
                );
            }
            assert!((sample.biome.weights.values().iter().sum::<f64>() - 1.0).abs() < 1e-9);
        }
    }
}
