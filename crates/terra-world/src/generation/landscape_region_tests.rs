use super::*;
use crate::generation::biomes::BiomeWeights;
use terra_core::{WorldConfig, WorldSeed};

fn plains() -> BiomeSample {
    BiomeSample {
        temperature: 0.5,
        humidity: 0.5,
        weights: BiomeWeights {
            ocean: 0.0,
            plains: 1.0,
            forest: 0.0,
            desert: 0.0,
            highland: 0.0,
            mushroom_park: 0.0,
            cliffted_desert: 0.0,
            cherry_forest: 0.0,
        },
    }
}

#[test]
fn revision_two_regions_preserve_the_previous_executable_snapshots() {
    // Captured by the revision-two terrain_regions executable on 2026-10-26,
    // before the relief fix. Covers every landform and mixed-sign coordinates.
    for (seed, fixtures) in [
        (
            42042,
            [
                ([0.5, 0.5], 109.194792),
                ([-127.5, 128.5], 112.495480),
                ([-127.5, 320.5], 133.745418),
                ([-575.5, -63.5], 137.610825),
                ([-639.5, -255.5], 102.328064),
                ([-447.5, 64.5], 181.317531),
            ],
        ),
        (
            71,
            [
                ([-1279.5, 1280.5], 21.309014),
                ([448.5, -1727.5], 28.835784),
                ([576.5, -1663.5], 33.012949),
                ([704.5, -1663.5], 54.780108),
                ([832.5, -1599.5], 72.600251),
                ([768.5, -1727.5], 82.328894),
            ],
        ),
        (
            918273,
            [
                ([0.5, 0.5], 41.458685),
                ([448.5, 128.5], 105.899827),
                ([576.5, 128.5], 126.573064),
                ([768.5, 64.5], 126.091820),
                ([960.5, 64.5], 98.693153),
                ([640.5, 0.5], 144.567781),
            ],
        ),
    ] {
        let terrain = TerraGenerator::new(WorldConfig {
            seed: WorldSeed(seed),
            landscape_revision: 2,
            rivers: false,
            sea_level: 8.0,
            ..Default::default()
        })
        .unwrap();
        for ([x, z], expected) in fixtures {
            let actual = terrain.surface_height(x, z);
            assert!(
                (actual - expected).abs() <= 0.00000051,
                "saved revision2 changed: seed={seed} at={x},{z} height={actual:.12} expected={expected}"
            );
        }
    }
}

#[test]
fn revision_three_lowlands_retain_local_relief_without_a_constant_floor() {
    for seed in [42042, 71, 918273] {
        let terrain = TerraGenerator::new(WorldConfig {
            seed: WorldSeed(seed),
            landscape_revision: 3,
            ..Default::default()
        })
        .unwrap();
        let mut checked = [0; 5];
        let reference = terrain.config().base_height + plains().profile().elevation;
        for z in (-6144..=6144).step_by(192) {
            for x in (-6144..=6144).step_by(192) {
                let [x, z] = [f64::from(x) + 0.5, f64::from(z) + 0.5];
                let region = terrain.landscape_region(x, z);
                let Some(index) = region.weights.iter().position(|&weight| weight > 0.999) else {
                    continue;
                };
                if region.landmark > 0.01 {
                    continue;
                }
                let low = terrain.regional_landscape_relief(x, z, plains(), reference - 2.0);
                let high = terrain.regional_landscape_relief(x, z, plains(), reference + 2.0);
                assert!(
                    high - low > 0.45,
                    "relief lost in landform={index}: seed={seed} at={x},{z}, heights={low}..{high}"
                );
                checked[index] += 1;
            }
        }
        assert!(
            checked.into_iter().all(|count| count > 2),
            "seed={seed}, coverage={checked:?}"
        );
    }
}

#[test]
fn revision_three_plains_and_plateaus_have_fewer_large_flat_patches() {
    for seed in [42042, 71, 918273] {
        let old = TerraGenerator::new(WorldConfig {
            seed: WorldSeed(seed),
            landscape_revision: 2,
            ..Default::default()
        })
        .unwrap();
        let new = TerraGenerator::new(WorldConfig {
            landscape_revision: 3,
            ..old.config().clone()
        })
        .unwrap();
        let mut samples = [[0; 4]; 2];
        let mut ranges = [[0.0; 2]; 2];
        let mut flat_footprints = [[0; 2]; 2];
        let mut gradients = [[0.0; 2]; 2];
        for z in (-6144..=6144).step_by(192) {
            for x in (-6144..=6144).step_by(192) {
                let [x, z] = [f64::from(x) + 0.5, f64::from(z) + 0.5];
                let quadrant = usize::from(x >= 0.0) + usize::from(z >= 0.0) * 2;
                for (slot, kind) in [0, 2].into_iter().enumerate() {
                    if samples[slot][quadrant] >= 8
                        || ![[-40.0, -40.0], [-40.0, 40.0], [40.0, -40.0], [40.0, 40.0]]
                            .into_iter()
                            .chain([[0.0, 0.0]])
                            .all(|[dx, dz]| {
                                let sample = new.landscape_region(x + dx, z + dz);
                                sample.weights[kind] > 0.999 && sample.landmark < 0.001
                            })
                    {
                        continue;
                    }
                    for (revision, terrain) in [&old, &new].into_iter().enumerate() {
                        let mut heights = [f64::INFINITY, f64::NEG_INFINITY];
                        for dz in (-32..=32).step_by(8) {
                            for dx in (-32..=32).step_by(8) {
                                let [x, z] = [x + f64::from(dx), z + f64::from(dz)];
                                let h = terrain.base_height_with_biome(x, z, plains());
                                heights[0] = heights[0].min(h);
                                heights[1] = heights[1].max(h);
                                // A half-block span over sixteen metres is a
                                // genuinely flat area. Single voxel treads are
                                // expected on natural hills and alias with the
                                // absolute height's integer rounding phase.
                                let footprint: [f64; 9] = std::array::from_fn(|i| {
                                    let dx = (i % 3) as f64 * 8.0 - 8.0;
                                    let dz = (i / 3) as f64 * 8.0 - 8.0;
                                    terrain.base_height_with_biome(x + dx, z + dz, plains())
                                });
                                let low = footprint.into_iter().fold(f64::INFINITY, f64::min);
                                let high = footprint.into_iter().fold(f64::NEG_INFINITY, f64::max);
                                flat_footprints[slot][revision] += usize::from(high - low < 0.5);
                                gradients[slot][revision] += (footprint[5] - footprint[3])
                                    .hypot(footprint[7] - footprint[1])
                                    / 16.0;
                            }
                        }
                        ranges[slot][revision] += heights[1] - heights[0];
                    }
                    samples[slot][quadrant] += 1;
                }
            }
        }
        assert_eq!(samples, [[8; 4]; 2], "seed={seed}, samples={samples:?}");
        assert!(
            ranges[0][1] > ranges[0][0] * 1.20,
            "seed={seed}, plains summed 64m local relief={:?}",
            ranges[0]
        );
        for (slot, [old, new]) in flat_footprints.into_iter().enumerate() {
            eprintln!(
                "relief regression seed={seed} landform={slot} patches=32 old_new_flat_16m={old}/{new} old_new_range_64m={:?} old_new_gradient={:?}",
                ranges[slot].map(|sum| sum / 32.0),
                gradients[slot].map(|sum| sum / (32.0 * 81.0)),
            );
            assert!(
                old > 0 && new < old,
                "seed={seed}, landform={slot}, flat 16m footprints old={old}, new={new}"
            );
            assert!(
                gradients[slot][1] > gradients[slot][0],
                "seed={seed}, landform={slot}, mean local gradient did not improve: {:?}",
                gradients[slot]
            );
        }
    }
}

#[test]
fn region_weights_are_a_smooth_nonnegative_partition() {
    for index in -1500..=1500 {
        let selector = f64::from(index) / 1000.0;
        let weights = region_weights(selector);
        assert!(
            weights
                .into_iter()
                .all(|weight| (0.0..=1.0).contains(&weight))
        );
        assert!((weights.into_iter().sum::<f64>() - 1.0).abs() < 1e-12);
        let next = region_weights(selector + 1e-6);
        assert!(
            weights
                .into_iter()
                .zip(next)
                .all(|(a, b)| (a - b).abs() < 1e-4)
        );
    }
}

#[test]
fn regional_fields_are_seeded_repeatable_and_independent_of_sampling_order() {
    for seed in [42042, 71, 918273] {
        let a = TerraGenerator::new(WorldConfig {
            seed: WorldSeed(seed),
            ..Default::default()
        })
        .unwrap();
        let b = TerraGenerator::new(a.config().clone()).unwrap();
        let other = TerraGenerator::new(WorldConfig {
            seed: WorldSeed(seed + 1),
            ..a.config().clone()
        })
        .unwrap();
        let points = [
            [-4096.5, -8192.5],
            [-16.0, 16.0],
            [-0.001, -0.001],
            [0.0, 0.0],
            [16.0, 32.0],
            [4096.5, -8192.5],
        ];
        let samples = points.map(|[x, z]| a.landscape_region(x, z));
        for i in (0..points.len()).rev() {
            let [x, z] = points[i];
            assert_eq!(samples[i], b.landscape_region(x, z));
            assert_eq!(
                a.base_height_with_biome(x, z, plains()).to_bits(),
                b.base_height_with_biome(x, z, plains()).to_bits()
            );
        }
        assert!(
            points
                .into_iter()
                .zip(samples)
                .any(|([x, z], sample)| other.landscape_region(x, z) != sample)
        );
    }
}

#[test]
fn large_regions_have_all_landforms_and_rare_clustered_landmarks_across_seeds() {
    for seed in [42042, 71, 918273] {
        let terrain = TerraGenerator::new(WorldConfig {
            seed: WorldSeed(seed),
            ..Default::default()
        })
        .unwrap();
        let mut coverage = [0; 5];
        let mut landmarks = 0;
        let mut clustered = 0;
        let mut samples = 0;
        for z in (-6144..=6144).step_by(96) {
            for x in (-6144..=6144).step_by(96) {
                let x = f64::from(x) + 0.5;
                let z = f64::from(z) + 0.5;
                let region = terrain.landscape_region(x, z);
                for (count, weight) in coverage.iter_mut().zip(region.weights) {
                    *count += usize::from(weight > 0.8);
                }
                if region.landmark > 0.05 {
                    landmarks += 1;
                    clustered +=
                        usize::from(terrain.landscape_region(x + 2.0, z + 2.0).landmark > 0.01);
                }
                samples += 1;
            }
        }
        assert!(
            coverage.into_iter().all(|count| count > 20),
            "seed={seed} coverage={coverage:?}"
        );
        assert!(
            landmarks > 0 && landmarks < samples / 20,
            "seed={seed} landmarks={landmarks}/{samples}"
        );
        assert!(
            clustered * 10 > landmarks * 8,
            "landmarks must be patches, not isolated single blocks"
        );
    }
}

#[test]
fn forms_change_macro_geometry_not_only_surface_detail() {
    let terrain = TerraGenerator::new(WorldConfig::default()).unwrap();
    let mut stats = [(0.0, 0_usize, f64::INFINITY, f64::NEG_INFINITY); 5];
    for z in (-4096..=4096).step_by(64) {
        for x in (-4096..=4096).step_by(64) {
            let [x, z] = [f64::from(x) + 0.5, f64::from(z) + 0.5];
            let region = terrain.landscape_region(x, z);
            for (i, weight) in region.weights.into_iter().enumerate() {
                if weight < 0.95 || region.landmark > 0.01 {
                    continue;
                }
                let height = terrain.regional_landscape_relief(x, z, plains(), 26.0);
                stats[i].0 += height;
                stats[i].1 += 1;
                stats[i].2 = stats[i].2.min(height);
                stats[i].3 = stats[i].3.max(height);
            }
        }
    }
    assert!(stats.into_iter().all(|(_, count, _, _)| count > 20));
    let means = stats.map(|(sum, count, _, _)| sum / count as f64);
    assert!(
        means[2] > means[0] + 6.0,
        "plateaus must be elevated: {means:?}"
    );
    assert!(
        means[4] < means[0] - 6.0,
        "basins must be broad lowlands: {means:?}"
    );
    assert!(
        stats[1].3 - stats[1].2 > (stats[0].3 - stats[0].2) * 2.0,
        "hills must be rougher than plains: {stats:?}"
    );
    assert!(
        stats[3].3 - stats[3].2 > 7.0,
        "ridges need recognisable elevation changes: {stats:?}"
    );
}

#[test]
fn column_edges_and_negative_origin_have_no_region_or_height_jumps() {
    let terrain = TerraGenerator::new(WorldConfig::default()).unwrap();
    for cell in -80..=80 {
        let edge = f64::from(cell * 16);
        for (x, z) in [(edge, -97.5), (-97.5, edge), (edge, edge)] {
            let a = terrain.landscape_region(x - 1e-5, z - 1e-5);
            let b = terrain.landscape_region(x + 1e-5, z + 1e-5);
            assert!(
                a.weights
                    .into_iter()
                    .zip(b.weights)
                    .all(|(a, b)| (a - b).abs() < 1e-5)
            );
            let a = terrain.base_height_with_biome(x - 1e-5, z - 1e-5, plains());
            let b = terrain.base_height_with_biome(x + 1e-5, z + 1e-5, plains());
            assert!(
                (a - b).abs() < 0.002,
                "height discontinuity at {x},{z}: {a} {b}"
            );
        }
    }
}

#[test]
fn ocean_beds_and_explicit_flat_worlds_do_not_receive_landform_uplift() {
    let terrain = TerraGenerator::new(WorldConfig::default()).unwrap();
    let mut ocean = plains();
    ocean.weights.ocean = 1.0;
    ocean.weights.plains = 0.0;
    for x in -12..=12 {
        let x = f64::from(x) * 151.0;
        assert_eq!(
            terrain
                .regional_landscape_relief(x, -x, ocean, -23.75)
                .to_bits(),
            (-23.75_f64).to_bits()
        );
    }
    let flat = TerraGenerator::new(WorldConfig {
        hill_amplitude: 0.0,
        mountain_amplitude: 0.0,
        rivers: false,
        water: false,
        ..Default::default()
    })
    .unwrap();
    for x in -12..=12 {
        assert_eq!(
            flat.base_height_with_biome(f64::from(x) * 151.0, 17.5, plains())
                .to_bits(),
            flat.config().base_height.to_bits()
        );
    }
}

#[test]
fn low_amplitude_basins_do_not_raise_submerged_land_to_sea_level() {
    let terrain = TerraGenerator::new(WorldConfig {
        base_height: 0.0,
        hill_amplitude: 0.1,
        mountain_amplitude: 0.0,
        sea_level: 8.0,
        ..Default::default()
    })
    .unwrap();
    let biome = plains();
    let reference = terrain.config().base_height + biome.profile().elevation;
    assert!(reference < terrain.config().sea_level);
    let mut checked = 0;
    for z in (-4096..=4096).step_by(128) {
        for x in (-4096..=4096).step_by(128) {
            let [x, z] = [f64::from(x) + 0.5, f64::from(z) + 0.5];
            if terrain.landscape_region(x, z).weights[4] != 1.0 {
                continue;
            }
            for offset in [-0.05, 0.0, 0.05] {
                let height = terrain.regional_landscape_relief(x, z, biome, reference + offset);
                assert!(
                    height <= reference + 0.01,
                    "small amplitudes must not create sea-level uplift: {height} > {reference}"
                );
                assert!(height >= reference - 0.2);
            }
            checked += 1;
        }
    }
    assert!(checked > 10, "fixture must include pure basin regions");
}
