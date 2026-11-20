use super::*;
use terra_core::{BiomeDefinitions, WorldConfig, WorldSeed};

fn terrain(seed: u64, scale: f64, revision: u8) -> TerraGenerator {
    TerraGenerator::new(WorldConfig {
        seed: WorldSeed(seed),
        biome_scale: scale,
        landscape_revision: revision,
        biome_definitions: Some(BiomeDefinitions::default()),
        rivers: false,
        caves: false,
        trees: false,
        structures_revision: 0,
        ..Default::default()
    })
    .unwrap()
}

#[test]
fn dry_interiors_span_at_least_twenty_four_chunks_across_multiple_seeds() {
    let mut kinds = [0; 10];
    for seed in [42042, 7, 99173] {
        for scale in [256.0_f64, 2048.0] {
            let world = terrain(seed, scale, 9);
            let mut interiors = 0;
            let spacing = spacing(world.config());
            let bend = Bend::new(spacing, seed);
            for cz in -4..=4 {
                for cx in -4..=4 {
                    let [x, z] = bend.world(center(seed, [cx, cz], spacing));
                    let base = world.surface_column(x, z);
                    if base.biome.weights.ocean > 0.05 || base.water_level.is_some() {
                        continue;
                    }
                    let kind = world.surface_biome(&base);
                    // Sample a 192 m radius around the centre at 16 bearings.
                    for bearing in 0..16 {
                        let (s, c) = (std::f64::consts::TAU * f64::from(bearing) / 16.0).sin_cos();
                        let column = world.surface_column(x + 192.0 * c, z + 192.0 * s);
                        if column.biome.weights.ocean > 0.45 {
                            continue;
                        }
                        assert_eq!(world.surface_biome(&column), kind, "seed {seed}, {cx},{cz}");
                    }
                    interiors += 1;
                    kinds[kind as usize] += 1;
                }
            }
            assert!(interiors >= 12, "{seed}, {scale}: {interiors}");
        }
    }
    assert!(kinds.iter().filter(|n| **n > 0).count() >= 6, "{kinds:?}");
}

#[test]
fn regions_blend_across_boundaries_and_keep_the_original_seabed_climate() {
    let new = terrain(42042, 2048., 9);
    let old = terrain(42042, 2048., 8);
    let mut blends = 0;
    let mut changes = 0;
    for z in [-1763.5, -16.5, 1227.5] {
        let mut last: Option<BiomeSample> = None;
        for x in (-3072..=3072).step_by(8) {
            let x = f64::from(x) + 0.5;
            let sample = new.climate_biome_at(x, z);
            let legacy = old.climate_biome_at(x, z);
            assert_eq!(sample.weights.ocean, legacy.weights.ocean);
            assert_eq!(sample.temperature, legacy.temperature);
            assert_eq!(sample.humidity, legacy.humidity);
            let values = sample.weights.values();
            assert!((values.iter().sum::<f64>() - 1.0).abs() < 1e-10);
            if let Some(prev) = last {
                assert!(
                    values
                        .into_iter()
                        .zip(prev.weights.values())
                        .all(|(a, b)| (a - b).abs() < 0.18)
                );
                assert!(BiomeClimate::allows_neighbor(
                    sample.weights.dominant().climate(),
                    prev.weights.dominant().climate()
                ));
                changes += usize::from(prev.weights.dominant() != sample.weights.dominant());
            }
            blends += usize::from(values[1..].iter().filter(|&&w| w > 0.1).count() > 1);
            last = Some(sample);
        }
    }
    assert!(
        changes >= 4 && blends >= 25,
        "{changes} changes, {blends} blended samples"
    );
}
