use super::*;
use terra_core::{WorldConfig, WorldSeed};

fn config(seed: u64) -> WorldConfig {
    WorldConfig {
        seed: WorldSeed(seed),
        landscape_revision: 5,
        sea_level: 22.0,
        biome_definitions: Some(terra_core::BiomeDefinitions::default()),
        structures_revision: 0,
        trees: false,
        caves: false,
        rivers: false,
        ..Default::default()
    }
}

#[test]
fn revision_eight_lowlands_have_different_relief_within_the_same_profile() {
    let sampler = AdvancedSampler::new(&WorldConfig {
        landscape_revision: 8,
        ..config(42042)
    });
    let mut relief: [Vec<(f64, f64)>; 3] = std::array::from_fn(|_| Vec::new());
    for z in -16..=16 {
        for x in -16..=16 {
            let [x, z] = [f64::from(x) * 512.0 + 0.5, f64::from(z) * 512.0 + 0.5];
            let center = sampler.node(x, z);
            let profile = match center.profile() {
                TerrainKind::Plains => 0,
                TerrainKind::RollingPlains => 1,
                TerrainKind::Forest => 2,
                _ => continue,
            };
            if center.fields.continentalness < 0.28 {
                continue;
            }
            let heights = [
                sampler.node(x - 192.0, z).height,
                sampler.node(x + 192.0, z).height,
                sampler.node(x, z - 192.0).height,
                sampler.node(x, z + 192.0).height,
                center.height,
            ];
            let span = heights.into_iter().fold(f64::NEG_INFINITY, f64::max)
                - heights.into_iter().fold(f64::INFINITY, f64::min);
            // A uniform hillside has near-zero curvature even when steep.
            let curvature = (heights[0] + heights[1] - 2.0 * center.height).abs()
                + (heights[2] + heights[3] - 2.0 * center.height).abs();
            relief[profile].push((span, curvature));
        }
    }
    let mut varied_profiles = 0;
    for samples in &mut relief {
        if samples.len() < 12 {
            continue;
        }
        samples.sort_by(|a, b| a.0.total_cmp(&b.0));
        let low = samples[samples.len() / 10].0;
        let high = samples[samples.len() * 9 / 10].0;
        let broken = samples
            .iter()
            .filter(|(_, curvature)| *curvature > 1.0)
            .count();
        varied_profiles += usize::from(high > low * 1.8 && broken * 2 > samples.len());
        eprintln!(
            "lowland samples={} p10_relief={low} p90_relief={high} curved={broken}",
            samples.len()
        );
    }
    assert!(
        varied_profiles >= 2,
        "same-biome districts must include soft and broken relief"
    );
}

#[test]
fn revision_six_relief_changes_the_landscape_without_moving_climate_or_saved_profiles() {
    for seed in [42042, 7, 99173] {
        let saved = config(seed);
        let old = AdvancedSampler::new(&saved);
        let modern = AdvancedSampler::new(&WorldConfig {
            landscape_revision: 6,
            ..saved.clone()
        });
        assert_eq!(old.definitions, modern.definitions);
        let mut changed = 0;
        let mut inland = 0;
        let mut maximum_delta = 0.0_f64;
        for z in -12..=12 {
            for x in -12..=12 {
                let [x, z] = [f64::from(x) * 384.0 + 0.5, f64::from(z) * 384.0 + 0.5];
                let before = old.node(x, z);
                let after = modern.node(x, z);
                assert_eq!(before.fields, after.fields);
                assert_eq!(before.weights, after.weights);
                assert_eq!(before.overhangs, after.overhangs);
                assert!(after.height.is_finite() && (-119.0..=350.0).contains(&after.height));
                if before.fields.continentalness >= 0.24 {
                    inland += 1;
                    let delta = (before.height - after.height).abs();
                    maximum_delta = maximum_delta.max(delta);
                    changed += usize::from(delta > 1.0);
                } else if before.fields.continentalness <= -0.14 {
                    assert_eq!(
                        before.height.to_bits(),
                        after.height.to_bits(),
                        "ocean shoreline anchor moved"
                    );
                }
            }
        }
        eprintln!(
            "fantasy relief seed={seed}: inland={inland} changed={changed} maximum_delta={maximum_delta:.3}m"
        );
        assert!(inland >= 20 && changed * 2 > inland);
        assert!(
            maximum_delta > 15.0,
            "fantasy relief is too weak for seed{seed}: {maximum_delta}"
        );
        // Every saved revision below six continues through the identical node
        // function and immutable profile snapshot, including explicitly absent snapshots.
        for revision in 0..=5 {
            let legacy = AdvancedSampler::new(&WorldConfig {
                landscape_revision: revision,
                ..saved.clone()
            });
            for [x, z] in [[-128.0, 128.0], [-0.5, -0.5], [4096.5, -3072.5]] {
                assert_eq!(
                    legacy.node(x, z).values().map(f64::to_bits),
                    old.node(x, z).values().map(f64::to_bits)
                );
            }
        }
    }
}

#[test]
fn revision_six_cache_boundaries_are_continuous_and_explicit_flat_worlds_stay_flat() {
    let config = WorldConfig {
        landscape_revision: 6,
        ..config(42042)
    };
    let sampler = AdvancedSampler::new(&config);
    let reversed = AdvancedSampler::new(&config);
    for cell in -16..=16 {
        let edge = f64::from(cell * 128);
        for [x, z] in [[edge, -97.5], [-97.5, edge], [edge, edge]] {
            let left = sampler.height(x - 0.00001, z - 0.00001);
            let right = sampler.height(x + 0.00001, z + 0.00001);
            assert!(
                (left - right).abs() < 0.002,
                "terrain seam at{x},{z}: {left} {right}"
            );
            assert_eq!(
                right.to_bits(),
                reversed.height(x + 0.00001, z + 0.00001).to_bits()
            );
            assert_eq!(
                left.to_bits(),
                reversed.height(x - 0.00001, z - 0.00001).to_bits()
            );
        }
    }
    let flat = TerraGenerator::new(WorldConfig {
        hill_amplitude: 0.0,
        mountain_amplitude: 0.0,
        ..config
    })
    .unwrap();
    for [x, z] in [[-2048.5, 8192.5], [-0.5, 0.5], [4096.5, -3072.5]] {
        assert_eq!(
            flat.terrain_base_height(x, z).to_bits(),
            flat.config().base_height.to_bits()
        );
        assert_eq!(
            flat.base_height_with_biome(x, z, flat.climate_biome_at(x, z))
                .to_bits(),
            flat.config().base_height.to_bits()
        );
    }
}

#[test]
fn original_saved_profiles_and_absent_snapshot_keep_original_geometry_bits() {
    for seed in [7, 42042, 99173] {
        let mut saved = config(seed);
        saved.biome_definitions.as_mut().unwrap().landscapes = LandscapeDefinitions::legacy_v5();
        let original = AdvancedSampler::new(&saved);
        let restored = AdvancedSampler::new(&saved);
        saved.biome_definitions = None;
        let legacy_without_snapshot = AdvancedSampler::new(&saved);
        assert_eq!(original.definitions, LandscapeDefinitions::legacy_v5());
        assert_eq!(legacy_without_snapshot.definitions, original.definitions);
        assert_ne!(
            original.definitions,
            AdvancedSampler::new(&config(seed)).definitions
        );
        for [x, z] in [
            [-4096.5, 768.5],
            [-128.0001, -128.5],
            [-128.0, -128.5],
            [-127.9999, -128.5],
            [-0.5, -0.5],
            [0.5, 0.5],
            [128.0, 128.0],
            [4096.5, -768.5],
        ] {
            let expected = eager_sample(&original, x, z).values().map(f64::to_bits);
            assert_eq!(restored.sample(x, z).values().map(f64::to_bits), expected);
            assert_eq!(
                legacy_without_snapshot
                    .sample(x, z)
                    .values()
                    .map(f64::to_bits),
                expected
            );
            assert_eq!(original.height(x, z).to_bits(), expected[7]);
            assert_eq!(legacy_without_snapshot.height(x, z).to_bits(), expected[7]);
        }
    }
}

#[test]
fn lower_mountain_defaults_reduce_land_height_extremes_without_flattening_relief() {
    for seed in [42042, 7, 99173] {
        let new_config = config(seed);
        let mut old_config = new_config.clone();
        old_config.biome_definitions.as_mut().unwrap().landscapes =
            LandscapeDefinitions::legacy_v5();
        let old = AdvancedSampler::new(&old_config);
        let new = AdvancedSampler::new(&new_config);
        let mut old_land = Vec::new();
        let mut new_land = Vec::new();
        for z in -16..16 {
            for x in -16..16 {
                let [x, z] = [f64::from(x) * 512.0 + 256.5, f64::from(z) * 512.0 + 256.5];
                let original = old.sample(x, z);
                let updated = new.sample(x, z);
                assert_eq!(original.fields, updated.fields);
                if original.fields.continentalness >= 0.24 {
                    old_land.push(original.height);
                    new_land.push(updated.height);
                }
            }
        }
        assert!(
            old_land.len() >= 64,
            "insufficient inland coverage for seed{seed}"
        );
        old_land.sort_by(f64::total_cmp);
        new_land.sort_by(f64::total_cmp);
        let quantile =
            |samples: &[f64], percent: usize| samples[(samples.len() - 1) * percent / 100];
        let [old_low, old_high, old_max] = [
            quantile(&old_land, 10),
            quantile(&old_land, 95),
            *old_land.last().unwrap(),
        ];
        let [new_low, new_high, new_max] = [
            quantile(&new_land, 10),
            quantile(&new_land, 95),
            *new_land.last().unwrap(),
        ];
        eprintln!(
            "terrain defaults seed={seed} inland_samples={} old_p10={old_low:.6} \
             old_p95={old_high:.6} old_max={old_max:.6} new_p10={new_low:.6} \
             new_p95={new_high:.6} new_max={new_max:.6}",
            old_land.len()
        );
        assert!(new_max < old_max, "seed{seed} max: {new_max} >= {old_max}");
        assert!(
            new_high < old_high,
            "seed{seed} p95: {new_high} >= {old_high}"
        );
        assert!(
            new_high - new_low >= 32.0,
            "seed{seed} lost inland relief: p10={new_low} p95={new_high}"
        );
    }
}

fn eager_sample(sampler: &AdvancedSampler, x: f64, z: f64) -> TerrainNode {
    let gx = (x / SPACING).floor() as i64;
    let gz = (z / SPACING).floor() as i64;
    let key = [gx.div_euclid(CELLS), gz.div_euclid(CELLS)];
    let mut nodes = Vec::with_capacity(SIDE * SIDE);
    // Retain the original eager tile construction as a geometry reference.
    for dz in -1..=CELLS + 1 {
        for dx in -1..=CELLS + 1 {
            nodes.push(sampler.node(
                (key[0] * CELLS + dx) as f64 * SPACING,
                (key[1] * CELLS + dz) as f64 * SPACING,
            ));
        }
    }
    let ix = gx.rem_euclid(CELLS) as usize;
    let iz = gz.rem_euclid(CELLS) as usize;
    let tx = x / SPACING - gx as f64;
    let tz = z / SPACING - gz as f64;
    let mut rows = [[0.0; 19]; 4];
    for (row, values) in rows.iter_mut().enumerate() {
        let samples = std::array::from_fn::<_, 4, _>(|column| {
            nodes[ix + column + (iz + row) * SIDE].values()
        });
        for channel in 0..19 {
            values[channel] = cubic(samples.map(|s| s[channel]), tx);
        }
    }
    TerrainNode::from_values(std::array::from_fn(|channel| {
        cubic(rows.map(|s| s[channel]), tz)
    }))
}

#[test]
fn sparse_queries_initialize_only_the_stencil_and_reuse_neighboring_nodes() {
    for origin in [0.0, -128.0] {
        let sampler = AdvancedSampler::new(&config(42042));
        let key = [if origin < 0.0 { -1 } else { 0 }; 2];
        let tile = sampler.tile(key);
        let initialized = || {
            tile.nodes
                .iter()
                .filter(|node| node.get().is_some())
                .count()
        };
        assert_eq!(initialized(), 0);
        sampler.height(origin + 1.5, origin + 2.5);
        assert_eq!(initialized(), 16);
        sampler.sample(origin + 2.5, origin + 3.5);
        assert_eq!(initialized(), 16);
        sampler.height(origin + 17.5, origin + 2.5);
        assert_eq!(initialized(), 20);
        sampler.sample(origin + 17.5, origin + 18.5);
        assert_eq!(initialized(), 24);
    }
}

#[test]
fn lazy_stencils_preserve_original_eager_geometry_bits_at_tile_boundaries() {
    for seed in [7, 42042, 99173] {
        let sampler = AdvancedSampler::new(&config(seed));
        for x in [
            -128.0001, -128.0, -127.9999, -16.5, -0.5, 0.5, 127.9999, 128.0, 128.0001,
        ] {
            let z = -x * 0.71 - 128.0;
            let expected = eager_sample(&sampler, x, z);
            assert_eq!(sampler.height(x, z).to_bits(), expected.height.to_bits());
            assert_eq!(
                sampler.sample(x, z).values().map(f64::to_bits),
                expected.values().map(f64::to_bits),
                "seed{seed} at{x},{z}"
            );
        }
    }
}

#[test]
fn parallel_cold_queries_share_the_same_lazy_stencil_and_eager_geometry_bits() {
    let sampler = Arc::new(AdvancedSampler::new(&config(99173)));
    let point = [-128.5, 16.5];
    let expected = eager_sample(&sampler, point[0], point[1])
        .values()
        .map(f64::to_bits);
    let key = [
        ((point[0] / SPACING).floor() as i64).div_euclid(CELLS),
        ((point[1] / SPACING).floor() as i64).div_euclid(CELLS),
    ];
    let tile = sampler.tile(key);
    assert!(tile.nodes.iter().all(|node| node.get().is_none()));
    let start = std::sync::Barrier::new(4);
    std::thread::scope(|scope| {
        for _ in 0..4 {
            let sampler = Arc::clone(&sampler);
            let start = &start;
            scope.spawn(move || {
                start.wait();
                assert_eq!(
                    sampler
                        .sample(point[0], point[1])
                        .values()
                        .map(f64::to_bits),
                    expected
                );
            });
        }
    });
    assert_eq!(
        tile.nodes
            .iter()
            .filter(|node| node.get().is_some())
            .count(),
        16
    );
}

#[test]
fn cache_keeps_active_tiles_and_uses_a_private_tile_when_every_entry_is_busy() {
    let sampler = AdvancedSampler::new(&config(7));
    let mut active: Vec<_> = (0..CACHE_LIMIT)
        .map(|index| sampler.tile([index as i64, 0]))
        .collect();
    let private_key = [CACHE_LIMIT as i64, 0];
    let private = sampler.tile(private_key);
    let expected = sampler.node((private_key[0] * CELLS - 1) as f64 * SPACING, -SPACING);
    assert_eq!(
        sampler.node_at(&private, 0).values().map(f64::to_bits),
        expected.values().map(f64::to_bits)
    );
    {
        let cache = sampler.cache.lock().unwrap();
        assert_eq!(cache.tiles.len(), CACHE_LIMIT);
        assert_eq!(cache.order.len(), CACHE_LIMIT);
        assert!(!cache.tiles.contains_key(&private_key));
        for tile in &active {
            assert!(Arc::ptr_eq(cache.tiles.get(&tile.key).unwrap(), tile));
        }
    }
    let released = active.remove(0);
    let released_key = released.key;
    drop(released);
    let replacement_key = [CACHE_LIMIT as i64 + 1, 0];
    let replacement = sampler.tile(replacement_key);
    let cache = sampler.cache.lock().unwrap();
    assert_eq!(cache.tiles.len(), CACHE_LIMIT);
    assert_eq!(cache.order.len(), CACHE_LIMIT);
    assert!(!cache.tiles.contains_key(&released_key));
    assert!(Arc::ptr_eq(
        cache.tiles.get(&replacement_key).unwrap(),
        &replacement
    ));
    for tile in active {
        assert!(Arc::ptr_eq(cache.tiles.get(&tile.key).unwrap(), &tile));
    }
}

#[test]
fn multi_noise_is_seeded_and_continuous_at_negative_tile_and_section_boundaries() {
    for seed in [7, 42042, 99173] {
        let a = AdvancedSampler::new(&config(seed));
        let b = AdvancedSampler::new(&config(seed));
        let other = AdvancedSampler::new(&config(seed ^ 0x9e3779b9));
        let mut changed = 0;
        for x in [-256.0, -128.0, -16.0, 0.0, 16.0, 128.0, 256.0, 1_000_000.0] {
            let p = a.sample(x, -128.0);
            assert_eq!(p.height, a.height(x, -128.0));
            let _ = b.sample(x + 256.0, -128.0);
            assert_eq!(p.values(), b.sample(x, -128.0).values());
            let before = a.sample(x - 0.001, -128.0);
            let after = a.sample(x + 0.001, -128.0);
            assert!(
                (before.height - after.height).abs() < 0.05,
                "height seam at {x}"
            );
            assert!((before.fields.continentalness - after.fields.continentalness).abs() < 0.001);
            assert!((p.weights.iter().sum::<f64>() - 1.0).abs() < 1e-10);
            assert!(p.weights.into_iter().all(|w| (0.0..=1.0).contains(&w)));
            changed += usize::from((p.height - other.sample(x, -128.0).height).abs() > 0.01);
        }
        assert!(changed > 4);
    }
}

#[test]
fn far_world_samples_preserve_finite_local_variation_without_i32_noise_overflow() {
    let sampler = AdvancedSampler::new(&config(42042));
    for x in [
        -1_099_511_627_000.0,
        -100_000_000.0,
        100_000_000.0,
        1_099_511_627_000.0,
    ] {
        let a = sampler.sample(x, -x * 0.71);
        let b = sampler.sample(x + 0.01, -x * 0.71);
        assert!(a.values().into_iter().all(f64::is_finite));
        assert!((a.height - b.height).abs() < 0.1);
        assert!((-119.0..=350.0).contains(&a.height));
    }
    for edge in [-NOISE_PERIOD, 0.0, NOISE_PERIOD] {
        let a = field(&sampler.broad, edge - 0.0001, 19.0, 1.0);
        let b = field(&sampler.broad, edge + 0.0001, 19.0, 1.0);
        assert!((a - b).abs() < 0.01, "folding seam {a} / {b}");
    }
}

#[test]
fn cold_warm_parallel_and_evicted_tiles_have_identical_results_and_bounded_memory() {
    let sampler = Arc::new(AdvancedSampler::new(&config(99173)));
    let point = [-128.5, 16.5];
    let expected = sampler
        .sample(point[0], point[1])
        .values()
        .map(f64::to_bits);
    std::thread::scope(|scope| {
        for _ in 0..4 {
            let sampler = sampler.clone();
            scope.spawn(move || {
                for _ in 0..32 {
                    assert_eq!(
                        sampler
                            .sample(point[0], point[1])
                            .values()
                            .map(f64::to_bits),
                        expected
                    );
                }
            });
        }
    });
    for tile in 0..CACHE_LIMIT + 3 {
        sampler.sample(tile as f64 * 128.0 + 16.0, 8192.0);
    }
    let cache = sampler.cache.lock().unwrap();
    assert_eq!(cache.tiles.len(), CACHE_LIMIT);
    assert_eq!(cache.order.len(), CACHE_LIMIT);
    assert!(!cache.tiles.contains_key(&[-2, 0]));
    drop(cache);
    assert_eq!(
        sampler
            .sample(point[0], point[1])
            .values()
            .map(f64::to_bits),
        expected
    );
}

#[test]
fn profile_weights_are_normalized_and_every_landscape_is_selectable() {
    let definitions = LandscapeDefinitions::default();
    let mut present = [false; 10];
    for continentalness in [-0.8, 0.0, 0.5] {
        for humidity in [0.1, 0.6, 0.95] {
            for temperature in [0.1, 0.5, 0.95] {
                for peaks_valleys in [-0.8, 0.2, 0.9] {
                    for district in [0.1, 0.5, 0.95] {
                        let weights = profile_weights(
                            TerrainFields {
                                continentalness,
                                humidity,
                                temperature,
                                peaks_valleys,
                                ridge_strength: 0.8,
                                erosion: 0.3,
                                weirdness: 0.5,
                            },
                            district,
                            &definitions,
                        );
                        assert!((weights.iter().sum::<f64>() - 1.0).abs() < 1e-10);
                        for (found, weight) in present.iter_mut().zip(weights) {
                            *found |= weight > 0.01;
                        }
                    }
                }
            }
        }
    }
    assert!(present.into_iter().all(|p| p));
}

#[test]
fn tiny_positive_ocean_weights_keep_finite_normalized_marine_transitions() {
    for ocean_weight in [1e-20, f64::MIN_POSITIVE] {
        let mut definitions = LandscapeDefinitions::default();
        definitions.profiles[TerrainKind::Ocean as usize].weight = ocean_weight;
        definitions.validate().unwrap();
        for continentalness in [-1.0, -0.16, -0.15, 0.0, 0.19, 0.20, 1.0] {
            let weights = profile_weights(
                TerrainFields {
                    continentalness,
                    humidity: 0.6,
                    temperature: 0.5,
                    peaks_valleys: 0.2,
                    ridge_strength: 0.8,
                    erosion: 0.3,
                    weirdness: 0.5,
                },
                0.5,
                &definitions,
            );
            assert!(
                weights
                    .into_iter()
                    .all(|w| w.is_finite() && (0.0..=1.0).contains(&w))
            );
            assert!((weights.iter().sum::<f64>() - 1.0).abs() < 1e-10);
            if continentalness <= -0.16 {
                assert_eq!(weights[TerrainKind::Ocean as usize], 1.0);
            }
        }
    }
}

#[test]
fn ocean_ridges_and_erosion_change_only_bed_relief_and_never_add_overhangs() {
    let mut base = config(42042);
    let profiles = &mut base.biome_definitions.as_mut().unwrap().landscapes.profiles;
    for profile in profiles.iter_mut() {
        profile.weight = if profile.kind == TerrainKind::Ocean {
            1.0
        } else {
            0.0
        };
    }
    let ocean = &mut profiles[TerrainKind::Ocean as usize];
    ocean.ridge_strength = 0.0;
    ocean.erosion = 0.0;
    ocean.overhangs = 0.0;
    let reference = AdvancedSampler::new(&base);
    let variants = std::array::from_fn::<_, 3, _>(|index| {
        let mut changed = base.clone();
        let ocean = &mut changed
            .biome_definitions
            .as_mut()
            .unwrap()
            .landscapes
            .profiles[TerrainKind::Ocean as usize];
        match index {
            0 => ocean.ridge_strength = 1.0,
            1 => ocean.erosion = 1.0,
            _ => ocean.overhangs = 1.0,
        }
        AdvancedSampler::new(&changed)
    });
    let mut changes = [0; 2];
    for x in [-4096.0, -2048.0, -512.0, 0.0, 512.0, 2048.0, 4096.0] {
        for z in [-1536.0, -128.0, 311.0, 1536.0] {
            let expected = reference.node(x, z);
            assert_eq!(expected.weights[TerrainKind::Ocean as usize], 1.0);
            for (index, sampler) in variants.iter().enumerate() {
                let actual = sampler.node(x, z);
                assert_eq!(actual.fields, expected.fields);
                assert_eq!(actual.weights, expected.weights);
                assert!(actual.height.is_finite());
                assert_eq!(actual.overhangs, 0.0);
                if index < 2 {
                    changes[index] += usize::from((actual.height - expected.height).abs() > 0.01);
                } else {
                    assert_eq!(actual.height, expected.height);
                }
            }
        }
    }
    assert!(changes.into_iter().all(|count| count > 0));
}

#[test]
fn warped_ecology_does_not_sharpen_regional_fields_into_abrupt_biome_edges() {
    for seed in [7, 42042, 99173] {
        let terrain = TerraGenerator::new(config(seed)).unwrap();
        for z in [-4096.0, -1.0, 4096.0] {
            let mut previous = terrain.climate_biome_at(-8192.0, z);
            for x in (-8188..=8192).step_by(4) {
                let next = terrain.climate_biome_at(f64::from(x), z);
                for (a, b) in previous
                    .weights
                    .values()
                    .into_iter()
                    .zip(next.weights.values())
                {
                    assert!(
                        (a - b).abs() < 0.06,
                        "seed {seed}, {x},{z}: {previous:?} -> {next:?}"
                    );
                }
                previous = next;
            }
        }
    }
}

#[test]
fn new_profile_settings_change_only_new_surface_contract_and_maintain_coherent_ecology() {
    let config = config(42042);
    let a = TerraGenerator::new(config.clone()).unwrap();
    let mut changed = config.clone();
    for profile in &mut changed
        .biome_definitions
        .as_mut()
        .unwrap()
        .landscapes
        .profiles
    {
        if profile.kind != TerrainKind::Ocean {
            profile.min_height += 1.0;
            profile.max_height += 1.0;
        }
    }
    let b = TerraGenerator::new(changed.clone()).unwrap();
    let mut changes = 0;
    for x in (-4096..4096).step_by(503) {
        let x = f64::from(x);
        let biome = a.climate_biome_at(x, 311.0);
        assert!((biome.weights.values().into_iter().sum::<f64>() - 1.0).abs() < 1e-9);
        changes +=
            usize::from((a.surface_height(x, 311.0) - b.surface_height(x, 311.0)).abs() > 0.01);
    }
    assert!(changes > 3);
    changed.landscape_revision = 4;
    let old_a = TerraGenerator::new(changed.clone()).unwrap();
    changed
        .biome_definitions
        .as_mut()
        .unwrap()
        .landscapes
        .warp_strength = 0.0;
    let old_b = TerraGenerator::new(changed).unwrap();
    for x in [-128.5, 0.5, 128.5] {
        assert_eq!(
            old_a.surface_height(x, 311.0),
            old_b.surface_height(x, 311.0)
        );
    }
}

#[test]
fn selective_density_carves_supported_overhangs_without_raising_envelope_or_wet_beds() {
    let terrain = TerraGenerator::new(config(99173)).unwrap();
    let mut base = terrain.surface_column(31.5, -47.5);
    base.height = 120.0;
    base.surface_shape.strength = 1.0;
    let mut carved = 0;
    for z in -24..24 {
        for x in -24..24 {
            for y in [80.5, 100.5, 108.5, 116.5, 119.5, 120.5, 125.5] {
                let p = [f64::from(x) + 0.5, y, f64::from(z) + 0.5];
                let original = y - base.height;
                let density = terrain.advanced.density(p, base, original);
                assert!(density >= original);
                if !(3.0..28.0).contains(&(base.height - y)) {
                    assert_eq!(density, original);
                }
                carved += usize::from(original < 0.0 && density > 0.0);
            }
        }
    }
    assert!(carved > 0);
    for x in (-4000..4000).step_by(701) {
        let column = terrain.surface_column(f64::from(x), 197.5);
        if column.water_level.is_some() {
            assert_eq!(column.surface_shape.strength, 0.0);
        }
    }
}

#[test]
fn adjacent_negative_sections_match_world_sampling_and_survive_reload() {
    let terrain = TerraGenerator::new(config(7)).unwrap();
    let generator = super::super::SectionGenerator::new(terrain.clone());
    let center = terrain.surface_height(-0.5, -0.5).floor() as i64;
    let mut expected = Vec::new();
    for x in [-1, 0] {
        let pos = terra_voxel::SectionPos::new(x, center.div_euclid(16), -1);
        let section = generator.build(pos, 0, &Default::default(), false).unwrap();
        for z in [0_usize, 15] {
            for y in [0_usize, 15] {
                for local_x in [0_usize, 15] {
                    let p = [
                        pos.x * 16 + local_x as i64,
                        pos.y * 16 + y as i64,
                        pos.z * 16 + z as i64,
                    ];
                    let material = section.data.blocks()[local_x + 16 * (y + 16 * z)];
                    assert_eq!(material, generator.block_at(p).unwrap());
                }
            }
        }
        expected.push((pos, section.data.blocks().to_vec()));
    }
    let reload = super::super::SectionGenerator::new(TerraGenerator::new(config(7)).unwrap());
    for (pos, blocks) in expected.into_iter().rev() {
        assert_eq!(
            reload
                .build(pos, 0, &Default::default(), false)
                .unwrap()
                .data
                .blocks(),
            blocks
        );
    }
}

#[test]
fn debug_sample_uses_real_final_density_and_water_level() {
    let terrain = TerraGenerator::new(config(7)).unwrap();
    let p = [-16.5, 44.5, 128.5];
    let debug = terrain.terrain_debug_sample(p);
    assert_eq!(debug.height, terrain.surface_height(p[0], p[2]));
    assert_eq!(debug.density, f64::from(terrain.sample(p).density));
    assert_eq!(
        debug.water_level,
        terrain.surface_column(p[0], p[2]).water_level
    );
    assert!(debug.river_strength >= 0.0 && debug.river_strength <= 1.0);
}
