use super::*;
use crate::TerraGenerator;
use terra_core::WorldConfig;

#[test]
fn climate_weights_are_normalized_varied_and_reproducible() {
    let a = ClimateSampler::new(42042, 2048.0, 4096.0);
    let b = ClimateSampler::new(42042, 2048.0, 4096.0);
    let c = ClimateSampler::new(42042 + (1 << 32), 2048.0, 4096.0);
    let mut maxima = [0.0_f64; 10];
    let mut different = false;
    for z in (-16384..=16384).step_by(512) {
        for x in (-16384..=16384).step_by(512) {
            let s = a.sample(f64::from(x), f64::from(z));
            assert_eq!(s, b.sample(f64::from(x), f64::from(z)));
            different |= s != c.sample(f64::from(x), f64::from(z));
            assert!((0.0..=1.0).contains(&s.temperature));
            assert!((0.0..=1.0).contains(&s.humidity));
            let values = s.weights.values();
            assert!((values.iter().sum::<f64>() - 1.0).abs() < 1e-12);
            for (i, value) in values.into_iter().enumerate() {
                assert!((-1e-12..=1.0).contains(&value));
                maxima[i] = maxima[i].max(value);
            }
        }
    }
    assert!(different);
    assert!(maxima[..6].iter().all(|&v| v > 0.7), "{maxima:?}");
}
#[test]
fn profiles_blend_every_weight_instead_of_selecting_a_dominant_biome() {
    let sample = |values: [f64; 6]| BiomeSample {
        temperature: 0.5,
        humidity: 0.5,
        weights: BiomeWeights {
            ocean: values[0],
            plains: values[1],
            forest: values[2],
            desert: values[3],
            highland: values[4],
            mushroom_park: values[5],
            cliffted_desert: 0.0,
            cherry_forest: 0.0,
            spirit_blossom: 0.0,
            blood_forest: 0.0,
        },
    };
    let weights = [0.10, 0.20, 0.25, 0.15, 0.10, 0.20];
    let mut elevation = 0.0;
    let mut forest = 0.0;
    for i in 0..6 {
        let mut basis = [0.0; 6];
        basis[i] = 1.0;
        let profile = sample(basis).profile();
        elevation += profile.elevation * weights[i];
        forest += profile.forest_cover * weights[i];
    }
    let blended = sample(weights).profile();
    assert!((blended.elevation - elevation).abs() < 1e-12);
    assert!((blended.forest_cover - forest).abs() < 1e-12);
}
#[test]
fn transitions_are_continuous_even_where_the_dominant_weight_changes() {
    let terrain = TerraGenerator::new(WorldConfig {
        landscape_revision: 1,
        ..Default::default()
    })
    .unwrap();
    let mut changes = 0;
    for z in [-4096.0, -1.0, 0.0, 4096.0] {
        let mut previous = terrain.climate_biome_at(-8192.0, z);
        for x in (-8188..=8192).step_by(4) {
            let x = f64::from(x);
            let next = terrain.climate_biome_at(x, z);
            changes += usize::from(next.weights.dominant() != previous.weights.dominant());
            for (a, b) in previous
                .weights
                .values()
                .into_iter()
                .zip(next.weights.values())
            {
                assert!(
                    (a - b).abs() < 0.06,
                    "biome seam at {x},{z}: {a} -> {b}; {previous:?} -> {next:?}"
                );
            }
            assert!((next.profile().soil_depth - previous.profile().soil_depth).abs() < 0.12);
            previous = next;
            assert!(
                (terrain.surface_height(x - 0.0001, z) - terrain.surface_height(x + 0.0001, z))
                    .abs()
                    < 0.005
            );
        }
    }
    assert!(changes >= 6, "must cross actual mixes: {changes}");
}

#[test]
fn highland_has_real_mountain_relief_and_never_appears_below_y80() {
    let terrain = TerraGenerator::new(WorldConfig {
        landscape_revision: 3,
        sea_level: 8.0,
        ..Default::default()
    })
    .unwrap();
    let mut highland = 0;
    let mut low_mountains = 0;
    for z in (-12288..=12288).step_by(256) {
        for x in (-12288..=12288).step_by(256) {
            let [x, z] = [f64::from(x) + 0.5, f64::from(z) + 0.5];
            let height = terrain.surface_height(x, z);
            let biome = terrain.biome_at(x, z);
            assert!((biome.weights.values().iter().sum::<f64>() - 1.0).abs() < 1e-12);
            if height <= 80.0 {
                assert_eq!(
                    biome.weights.highland, 0.0,
                    "low Highland at {x},{height},{z}"
                );
                low_mountains += usize::from(terrain.climate_biome_at(x, z).weights.highland > 0.3);
            }
            if biome.weights.dominant() == BiomeKind::Highland {
                highland += 1;
                assert!(height > 80.0);
            }
        }
    }
    assert!(
        highland > 50,
        "mountain field must actually create highlands: {highland}"
    );
    assert!(low_mountains > 50, "foothills must retain lowland ecology");
}

#[test]
fn altitude_gate_is_normalized_and_continuous_at_eighty_metres() {
    let sample = ClimateSampler::new(42042, 2048.0, 4096.0).sample(0.0, 0.0);
    for height in [79.9999, 80.0, 80.0001, 94.0, 108.0] {
        let biome = sample.at_height(height);
        assert!((biome.weights.values().iter().sum::<f64>() - 1.0).abs() < 1e-12);
        assert!(
            (biome.weights.highland - sample.at_height(height + 1e-5).weights.highland).abs()
                < 1e-6
        );
    }
    assert_eq!(sample.at_height(80.0).weights.highland, 0.0);
    assert_eq!(sample.at_height(108.0), sample);
}
#[test]
fn biome_scale_controls_transition_width_without_reseeding() {
    let narrow = ClimateSampler::new(7, 256.0, 512.0);
    let wide = ClimateSampler::new(7, 1024.0, 2048.0);
    for [x, z] in [[-1.0, -33.0], [100.0, 500.0], [-2048.0, 3096.0]] {
        assert_eq!(narrow.sample(x, z), wide.sample(x * 4.0, z * 4.0));
    }
}

#[test]
fn prepared_biome_preserves_forest_probability_in_all_regions() {
    for biomes in [false, true] {
        let terrain = TerraGenerator::new(WorldConfig {
            biomes,
            ..WorldConfig::default()
        })
        .unwrap();
        for z in (-8192..=8192).step_by(512) {
            for x in (-8192..=8192).step_by(512) {
                let [x, z] = [f64::from(x) + 0.5, f64::from(z) + 0.5];
                let biome = terrain.biome_at(x, z);
                assert_eq!(
                    terrain.forest_probability(x, z),
                    terrain.forest_probability_with_biome(x, z, biome)
                );
            }
        }
    }
}
