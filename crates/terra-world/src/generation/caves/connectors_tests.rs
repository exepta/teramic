use super::*;
use terra_core::{WorldConfig, WorldSeed};

fn generator(seed: u64, revision: u8) -> TerraGenerator {
    TerraGenerator::new(WorldConfig {
        seed: WorldSeed(seed),
        cave_structure_revision: revision,
        ..Default::default()
    })
    .unwrap()
}

#[test]
fn legacy_cave_revisions_zero_through_six_keep_their_original_connector_fields() {
    for revision in 0..=6 {
        let terrain = generator(42042, revision);
        let caves_a = noise::Perlin::new(layer_seed(terrain.config().seed.0, 4));
        let caves_b = noise::Perlin::new(layer_seed(terrain.config().seed.0, 5));
        for x in [-384.5, -192.0, -16.5, 0.5, 191.5, 385.5] {
            for z in [-385.5, -16.0, 0.5, 192.0] {
                let variety = terrain.valleys.get([x * 0.0031 + 9.17, z * 0.0031 - 4.13]);
                let expected = match revision {
                    0 | 1 => 0.095 + super::super::biomes::smoothstep(-0.35, 0.4, variety) * 0.08,
                    2 => -0.055 + super::super::biomes::smoothstep(-0.18, 0.34, variety) * 0.205,
                    _ => {
                        (0.11 + super::super::biomes::smoothstep(-0.35, 0.4, variety) * 0.07)
                            * terrain.config().cave_biomes.connectors.density
                    }
                };
                let expected = if revision == 6 {
                    expected * 1.4
                } else {
                    expected
                };
                let shape = terrain.cave_shape(x, z);
                assert_eq!(
                    shape.tunnel_width.to_bits(),
                    expected.to_bits(),
                    "revision {revision}"
                );
                assert_eq!(shape.connector_limit, f64::INFINITY);
                assert_eq!(
                    terrain
                        .cave_region([(x / 192.0).floor() as i64, (z / 192.0).floor() as i64])
                        .presence,
                    f64::INFINITY
                );
                for y in [-72.5, -48.5, -24.5] {
                    let a = caves_a.get([x * 0.035 + 0.13, y * 0.045 + 0.41, z * 0.035 + 0.67])
                        * (1.0 + shape.fissure * 1.2);
                    let b = caves_b.get([x * 0.037 + 0.79, y * 0.040 + 0.23, z * 0.037 + 0.59])
                        * (1.0 - shape.fissure * 0.50);
                    let relief = match shape.kind {
                        CaveKind::Cheese => 5.0,
                        CaveKind::Dripstone => 3.0,
                        CaveKind::Spaghetti => 1.4,
                    };
                    let previous = ((expected - (a * a + b * b).sqrt()) * 20.0)
                        .max(shape.chamber_density(y) + (a + b) * relief);
                    assert_eq!(
                        terrain
                            .tunnel_density_with_shape([x, y, z], &shape)
                            .to_bits(),
                        previous.to_bits()
                    );
                }
            }
        }
    }
}

#[test]
fn revision_eight_moderately_widens_revision_seven_without_changing_network_presence() {
    for seed in [7, 42042, 98173] {
        let previous = generator(seed, 7);
        let wider = generator(seed, 8);
        let mut additional = 0;
        for z in (-192..192).step_by(6) {
            for x in (-192..192).step_by(6) {
                let (x, z) = (f64::from(x) + 0.5, f64::from(z) + 0.5);
                let a = previous.cave_shape(x, z);
                let b = wider.cave_shape(x, z);
                assert_eq!(b.tunnel_width.to_bits(), (a.tunnel_width * 1.12).to_bits());
                assert_eq!(b.connector_limit.to_bits(), a.connector_limit.to_bits());
                for y in [-72.5, -48.5, -24.5] {
                    let old = previous.tunnel_density_with_shape([x, y, z], &a);
                    let new = wider.tunnel_density_with_shape([x, y, z], &b);
                    assert!(new >= old);
                    additional += usize::from(old <= 0.0 && new > 0.0);
                }
            }
        }
        assert!(
            additional > 10,
            "seed {seed}: wider real passages, not a metadata-only revision"
        );
    }
}

#[test]
fn wider_connectors_retain_the_same_seeded_network_and_open_additional_real_passage_cells() {
    for seed in [7, 42042, 98173] {
        let old = generator(seed, 5);
        let wider = generator(seed, 6);
        let mut old_open = 0;
        let mut added = 0;
        for z in (-96..96).step_by(3) {
            for x in (-96..96).step_by(3) {
                let [x, z] = [f64::from(x) + 0.5, f64::from(z) + 0.5];
                let a = old.cave_shape(x, z);
                let b = wider.cave_shape(x, z);
                assert_eq!(b.tunnel_width.to_bits(), (a.tunnel_width * 1.4).to_bits());
                assert_eq!(
                    (a.kind, a.fissure, a.salt, a.chamber),
                    (b.kind, b.fissure, b.salt, b.chamber)
                );
                for y in [-72.5, -48.5, -24.5] {
                    let a = old.tunnel_density_with_shape([x, y, z], &a);
                    let b = wider.tunnel_density_with_shape([x, y, z], &b);
                    assert!(
                        b >= a,
                        "the widened field must not close a previous passage"
                    );
                    old_open += usize::from(a > 0.0);
                    added += usize::from(a <= 0.0 && b > 0.0);
                }
            }
        }
        assert!(
            old_open > 100 && added > 100,
            "seed {seed}: old={old_open}, additional={added}"
        );
    }
}

#[test]
fn widened_connector_samples_are_order_independent_and_smooth_at_negative_section_and_region_seams()
{
    let first = generator(77, 6);
    let reverse = generator(77, 6);
    let positions: Vec<_> = [-384.0, -192.0, -16.0, 0.0, 16.0, 192.0]
        .into_iter()
        .flat_map(|x| {
            [-64.0, -16.0, 0.0, 16.0]
                .into_iter()
                .map(move |z| [x, -48.5, z])
        })
        .collect();
    let forward: Vec<_> = positions
        .iter()
        .map(|&p| first.tunnel_density(p).to_bits())
        .collect();
    for (index, &p) in positions.iter().enumerate().rev() {
        assert_eq!(forward[index], reverse.tunnel_density(p).to_bits());
        for axis in [0, 2] {
            let mut before = p;
            let mut after = p;
            before[axis] -= 0.0001;
            after[axis] += 0.0001;
            assert!((first.tunnel_density(before) - first.tunnel_density(after)).abs() < 0.002);
        }
    }
}

#[test]
fn clustered_connectors_preserve_wide_passages_but_leave_coherent_rock_between_them() {
    for seed in [7, 42042, 98173] {
        let wide = generator(seed, 6);
        let clustered = generator(seed, 7);
        let mut old_open = 0;
        let mut new_open = 0;
        let mut solid_columns = 0;
        let mut adjacent_agree = 0;
        let mut distant_agree = 0;
        let mut columns = 0;
        for z in (-512..512).step_by(8) {
            for x in (-512..512).step_by(8) {
                let [x, z] = [f64::from(x) + 0.5, f64::from(z) + 0.5];
                let a = wide.cave_shape(x, z);
                let b = clustered.cave_shape(x, z);
                assert_eq!(a.tunnel_width.to_bits(), b.tunnel_width.to_bits());
                let active = b.connector_limit > 0.0;
                columns += 1;
                solid_columns += usize::from(!active);
                adjacent_agree +=
                    usize::from(active == (clustered.connector_presence(x + 4.0, z) > 0.0));
                distant_agree +=
                    usize::from(active == (clustered.connector_presence(x + 128.0, z) > 0.0));
                for y in [-72.5, -48.5, -24.5] {
                    let old = wide.tunnel_density_with_shape([x, y, z], &a);
                    let new = clustered.tunnel_density_with_shape([x, y, z], &b);
                    old_open += usize::from(old > 0.0);
                    new_open += usize::from(new > 0.0);
                    assert!(new <= old);
                    assert_eq!(new > 0.0, active && old > 0.0);
                }
            }
        }
        let solid = solid_columns as f64 / columns as f64;
        let retained = new_open as f64 / old_open as f64;
        let adjacent = adjacent_agree as f64 / columns as f64;
        let distant = distant_agree as f64 / columns as f64;
        println!(
            "seed {seed}: solid={solid:.3}, passage_retained={retained:.3}, adjacent={adjacent:.3}, distant={distant:.3}"
        );
        assert!((0.20..0.60).contains(&solid), "seed {seed}: solid={solid}");
        assert!(
            (0.35..0.85).contains(&retained),
            "seed {seed}: retained={retained}"
        );
        assert!(adjacent > 0.85 && adjacent > distant + 0.10);

        let mut longest_rock = 0;
        for z in [-384.5, -128.5, 128.5, 384.5] {
            let mut run = 0;
            for x in (-1024..1024).step_by(4) {
                if clustered.connector_presence(f64::from(x) + 0.5, z) <= 0.0 {
                    run += 4;
                    longest_rock = longest_rock.max(run);
                } else {
                    run = 0;
                }
            }
        }
        println!("seed {seed}: longest sampled solid band={longest_rock}m");
        assert!(
            (16..256).contains(&longest_rock),
            "seed {seed}: {longest_rock}m solid band"
        );
    }
}

#[test]
fn chamber_presence_uses_the_same_clusters_without_shrinking_selected_rooms() {
    for seed in [7, 42042, 98173] {
        let old = generator(seed, 6);
        let new = generator(seed, 7);
        let mut kept = 0;
        let mut suppressed = 0;
        for z in -6..6 {
            for x in -6..6 {
                let a = old.cave_region([x, z]);
                let b = new.cave_region([x, z]);
                assert_eq!(
                    (a.center, a.radius, a.height),
                    (b.center, b.radius, b.height)
                );
                assert_eq!(
                    b.presence.to_bits(),
                    new.connector_presence(b.center[0], b.center[2]).to_bits()
                );
                if a.kind == CaveKind::Spaghetti {
                    continue;
                }
                let center = [x as f64 * 192.0 + 96.0, z as f64 * 192.0 + 96.0];
                let room = new.cave_room_at(center[0], center[1]);
                if b.presence > 0.0 {
                    kept += 1;
                    assert_eq!(room.center[0], a.center[0]);
                    assert_eq!(room.center[2], a.center[2]);
                    assert_eq!((room.radius, room.height), (a.radius, a.height));
                } else {
                    suppressed += 1;
                    assert!(!room.active());
                    assert_eq!((room.radius, room.height), (a.radius, a.height));
                    assert!(room.water.is_none() && !room.dynamic_water);
                    assert!(new.cave_chamber_center(center[0], center[1]).is_none());
                }
            }
        }
        println!("seed {seed}: retained rooms={kept}, suppressed rooms={suppressed}");
        assert!(
            kept >= 10 && suppressed >= 10,
            "seed {seed}: kept={kept}, suppressed={suppressed}"
        );
    }
}

#[test]
fn actual_underground_air_volume_and_discovered_chambers_are_reduced_across_seeds() {
    underground_air_volume_reduction(1, true);
}

#[test]
fn revision_two_landscape_preserves_clustered_underground_air_subset_and_volume_reduction() {
    underground_air_volume_reduction(2, false);
}

fn underground_air_volume_reduction(landscape_revision: u8, numeric_density_contract: bool) {
    for seed in [7, 42042, 98173] {
        let old = TerraGenerator::new(WorldConfig {
            landscape_revision,
            ..generator(seed, 6).config().clone()
        })
        .unwrap();
        let new = TerraGenerator::new(WorldConfig {
            landscape_revision,
            ..generator(seed, 7).config().clone()
        })
        .unwrap();
        let mut air_before = 0;
        let mut air_after = 0;
        let mut chamber_columns_before = 0;
        let mut chamber_columns_after = 0;
        for z in (-512..512).step_by(16) {
            for x in (-512..512).step_by(16) {
                let [x, z] = [f64::from(x) + 0.5, f64::from(z) + 0.5];
                let a = old.surface_column(x, z);
                let b = new.surface_column(x, z);
                chamber_columns_before += usize::from(a.cave.active());
                chamber_columns_after += usize::from(b.cave.active());
                for y in (-112..-8).step_by(8) {
                    let y = f64::from(y) + 0.5;
                    if y > a.height - 8.0 {
                        continue;
                    }
                    let before = old.cave_density_with_column([x, y, z], a).0;
                    let after = new.cave_density_with_column([x, y, z], b).0;
                    assert!(
                        if numeric_density_contract {
                            after <= before
                        } else {
                            after <= 0.0 || before > 0.0
                        },
                        "seed {seed}: clustering may not create another cave network at [{x}, {y}, {z}]: {before}->{after}"
                    );
                    air_before += usize::from(before > 0.0);
                    air_after += usize::from(after > 0.0);
                }
            }
        }
        let retained = air_after as f64 / air_before as f64;
        println!(
            "seed {seed}: actual open samples {air_before}->{air_after}, chamber columns {chamber_columns_before}->{chamber_columns_after}"
        );
        assert!(
            air_after > 100 && (0.30..0.90).contains(&retained),
            "seed {seed}: air retained={retained}"
        );
        assert!(chamber_columns_after > 0 && chamber_columns_after < chamber_columns_before);
    }
}

#[test]
fn clustered_network_is_reload_order_independent_and_continuous_at_negative_boundaries() {
    let a = generator(98173, 7);
    let b = generator(98173, 7);
    let points: Vec<_> = [-384.0, -192.0, -16.0, 0.0, 16.0, 192.0]
        .into_iter()
        .flat_map(|x| {
            [-192.0, -16.0, 0.0, 16.0, 192.0]
                .into_iter()
                .map(move |z| [x, -48.5, z])
        })
        .collect();
    let expected: Vec<_> = points
        .iter()
        .map(|&p| a.tunnel_density(p).to_bits())
        .collect();
    for (i, &p) in points.iter().enumerate().rev() {
        assert_eq!(expected[i], b.tunnel_density(p).to_bits());
        for axis in [0, 2] {
            let mut before = p;
            let mut after = p;
            before[axis] -= 0.0001;
            after[axis] += 0.0001;
            assert!((a.tunnel_density(before) - a.tunnel_density(after)).abs() < 0.002);
        }
    }
}
