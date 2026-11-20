use super::*;
use terra_core::{WorldConfig, WorldSeed};

#[test]
fn regional_biome_weights_stay_normalized_with_sanctuaries_and_across_tile_boundaries() {
    for seed in [42042, 7, 99173] {
        let terrain = TerraGenerator::new(WorldConfig {
            seed: WorldSeed(seed),
            landscape_revision: 7,
            ..Default::default()
        })
        .unwrap();
        for z in (-8192..=8192).step_by(512) {
            for x in (-8192..=8192).step_by(512) {
                let [x, z] = [x as f64, z as f64];
                let a = terrain.climate_biome_at(x - 0.001, z - 0.001);
                let b = terrain.climate_biome_at(x + 0.001, z + 0.001);
                for sample in [a, b] {
                    let weights = sample.weights.values();
                    assert!(
                        weights.iter().all(|w| (-1e-12..=1.0 + 1e-12).contains(w)),
                        "invalid biome cover at {x},{z}: {sample:?}"
                    );
                    assert!(
                        (weights.iter().sum::<f64>() - 1.0).abs() < 1e-10,
                        "lost biome cover at {x},{z}: {sample:?}"
                    );
                }
                assert!(
                    a.weights
                        .values()
                        .into_iter()
                        .zip(b.weights.values())
                        .all(|(a, b)| (a - b).abs() < 0.002)
                );
            }
        }
    }
}

#[test]
fn regional_ranges_redistribute_mountain_weight_without_moving_ocean_or_disabled_profiles() {
    let fields = TerrainFields {
        continentalness: 0.8,
        erosion: 0.2,
        temperature: 0.4,
        humidity: 0.25,
        weirdness: 0.5,
        peaks_valleys: 0.8,
        ridge_strength: 0.9,
    };
    let definitions = LandscapeDefinitions::default();
    let original = profile_weights(fields, 0.4, &definitions);
    let lowland = profile_weights_scaled(fields, 0.4, &definitions, 0.0);
    assert_eq!(&lowland[3..=5], &[0.0; 3]);
    assert_eq!(lowland[9], original[9]);
    assert!((lowland.iter().sum::<f64>() - 1.0).abs() < 1e-12);
    assert_eq!(
        profile_weights_scaled(fields, 0.4, &definitions, 1.0),
        original
    );
    let mut mountains_only = definitions.clone();
    for profile in &mut mountains_only.profiles {
        profile.weight = if profile.kind == TerrainKind::FantasyMountains {
            1.0
        } else {
            0.0
        };
    }
    assert_eq!(
        profile_weights_scaled(fields, 0.4, &mountains_only, 0.0)[4],
        1.0
    );
    for step in 1..=100 {
        let t = step as f64 / 100.0;
        let next = profile_weights_scaled(fields, 0.4, &definitions, t);
        let previous = profile_weights_scaled(fields, 0.4, &definitions, t - 0.000001);
        assert_eq!(next[9], original[9]);
        assert!(
            next.iter()
                .zip(previous)
                .all(|(a, b)| (a - b).abs() < 0.0001)
        );
    }
}

#[test]
fn mountains_are_regional_and_most_inland_landscape_remains_lowland() {
    let mut totals = [0_usize; 4];
    for seed in [42042, 7, 99173] {
        let config = WorldConfig {
            seed: WorldSeed(seed),
            landscape_revision: 7,
            biome_definitions: Some(terra_core::BiomeDefinitions::default()),
            ..Default::default()
        };
        let modern = AdvancedSampler::new(&config);
        let previous = AdvancedSampler::new(&WorldConfig {
            landscape_revision: 6,
            ..config
        });
        let mut counts = [0_usize; 4];
        for z in -20..=20 {
            for x in -20..=20 {
                let [x, z] = [x as f64 * 384.0 + 0.5, z as f64 * 384.0 + 0.5];
                let a = previous.node(x, z);
                if a.fields.continentalness < 0.24 {
                    continue;
                }
                let b = modern.node(x, z);
                counts[0] += 1;
                counts[1] += usize::from((3..=5).contains(&(a.profile() as usize)));
                counts[2] += usize::from((3..=5).contains(&(b.profile() as usize)));
                counts[3] += usize::from(b.weights[3..=5].iter().sum::<f64>() < 0.02);
                assert_eq!(a.fields, b.fields);
                assert_eq!(a.weights[9], b.weights[9]);
                assert!((b.weights.iter().sum::<f64>() - 1.0).abs() < 1e-10);
            }
        }
        eprintln!(
            "regional mountains seed={seed}: inland={} old_ranges={} new_ranges={} nonmountain={}",
            counts[0], counts[1], counts[2], counts[3]
        );
        assert!(counts[0] > 30);
        assert!(
            counts[2] * 2 < counts[1],
            "mountains remain too common: {counts:?}"
        );
        assert!(
            counts[3] * 10 > counts[0] * 6,
            "too few calm lowlands: {counts:?}"
        );
        for i in 0..4 {
            totals[i] += counts[i]
        }
    }
    assert!(totals[2] > 0, "distinct mountain ranges must still exist");
}

#[test]
fn lowland_plains_keep_soft_local_relief_without_cliff_shelves() {
    let config = WorldConfig {
        landscape_revision: 7,
        biome_definitions: Some(terra_core::BiomeDefinitions::default()),
        ..Default::default()
    };
    let modern = AdvancedSampler::new(&config);
    let mut checked = 0;
    for z in -16..=16 {
        for x in -16..=16 {
            let [x, z] = [x as f64 * 512.0 + 0.5, z as f64 * 512.0 + 0.5];
            let node = modern.node(x, z);
            if node.fields.continentalness < 0.30
                || node.weights[0] + node.weights[1] < 0.65
                || node.weights[3..=5].iter().sum::<f64>() > 0.01
            {
                continue;
            }
            let adjacent = [[-32.0, 0.0], [32.0, 0.0], [0.0, -32.0], [0.0, 32.0]]
                .map(|[dx, dz]| modern.node(x + dx, z + dz));
            if adjacent.iter().any(|n| {
                n.fields.continentalness < 0.30 || n.weights[3..=5].iter().sum::<f64>() > 0.01
            }) {
                continue;
            }
            let low = adjacent.iter().fold(node.height, |a, n| a.min(n.height));
            let high = adjacent.iter().fold(node.height, |a, n| a.max(n.height));
            assert!(
                high - low < 14.0,
                "Plains has mountain-scale relief at {x},{z}: {}",
                high - low
            );
            checked += 1;
        }
    }
    assert!(checked > 15, "missing real Plains regions: {checked}");
}

#[test]
fn saved_six_keeps_height_bits_from_the_previous_compiled_generator() {
    let terrain = TerraGenerator::new(WorldConfig {
        landscape_revision: 6,
        biome_definitions: Some(terra_core::BiomeDefinitions::default()),
        spirit_blossom_revision: 0,
        blood_forest_revision: 0,
        trees: false,
        caves: false,
        rivers: false,
        ..Default::default()
    })
    .unwrap();
    for ([x, z], bits) in [
        ([-4096.5, 768.5], 4_632_399_490_009_784_214_u64),
        ([-0.5, -0.5], 4_595_318_223_652_966_063),
        ([4096.5, -768.5], 4_634_751_818_940_362_115),
    ] {
        assert_eq!(
            terrain.terrain_base_height(x, z).to_bits(),
            bits,
            "saved terrain moved at {x},{z}"
        );
    }
}
