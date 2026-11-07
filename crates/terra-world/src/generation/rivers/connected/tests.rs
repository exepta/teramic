use super::*;
use crate::{TerrainGenerator, edits::EditJournal, water::fluid_cell_at};
use terra_core::{WorldConfig, WorldSeed};

fn terrain(seed: u64) -> TerraGenerator {
    TerraGenerator::new(WorldConfig {
        seed: WorldSeed(seed),
        landscape_revision: 3,
        river_terrain_revision: 1,
        sea_level: 22.0,
        rivers: true,
        caves: false,
        trees: false,
        structures_revision: 0,
        ..Default::default()
    })
    .unwrap()
}

fn regions(terrain: &TerraGenerator) -> Vec<HydrologyRegion> {
    (-2..=2)
        .flat_map(|z| (-2..=2).map(move |x| [x, z]))
        .filter_map(|[x, z]| {
            terrain.hydrology_region_at(
                f64::from(x) * REGION + 1024.0,
                f64::from(z) * REGION + 1024.0,
            )
        })
        .collect()
}

#[test]
fn connected_courses_reach_real_lakes_or_sea_and_vary_in_length() {
    let mut lengths = [f64::INFINITY, 0.0_f64];
    let mut coasts = 0;
    let mut lakes = 0;
    let mut courses = 0;
    let mut through_lakes = 0;
    for seed in [42042, 7, 99173] {
        let terrain = terrain(seed);
        for region in regions(&terrain) {
            courses += 1;
            lengths[0] = lengths[0].min(region.course_length);
            lengths[1] = lengths[1].max(region.course_length);
            assert!(region.course_length >= 800.0);
            assert!(region.reaches_sea || region.lake.is_some());
            assert!(region.course.windows(2).all(|p| p[1][1] <= p[0][1]));
            for point in [region.source, region.mouth] {
                let water = terrain.water_column(point[0], point[2]).unwrap();
                assert!(water.depth > 0.0 && water.bed < water.level);
            }
            if region.reaches_sea {
                coasts += 1;
                through_lakes += usize::from(region.lake.is_some());
                assert_eq!(region.mouth[1], terrain.config().sea_level);
                let p = region.mouth;
                assert!(terrain.climate_biome_at(p[0], p[2]).weights.ocean >= 0.35);
                assert!(terrain.connected_ocean_outlet([p[0], p[2]]));
                assert!(
                    terrain.base_height_with_biome(
                        p[0],
                        p[2],
                        terrain.climate_biome_at(p[0], p[2]),
                    ) < terrain.config().sea_level - 1.0
                );
            } else {
                lakes += 1;
                let lake = region.lake.unwrap();
                let end = terrain.lake_at(region.mouth[0], region.mouth[2]).unwrap();
                assert_eq!(end.level, lake.level);
                assert!(end.distance < end.radius);
                assert_eq!(region.mouth[1], lake.level);
            }
        }
    }
    assert!(
        courses >= 6 && coasts >= 2 && lakes >= 2,
        "{courses}/{coasts}/{lakes}"
    );
    assert!(
        lengths[1] - lengths[0] > 200.0,
        "uniform course lengths {lengths:?}"
    );
    assert!(
        through_lakes > 0,
        "need a lake with both inlet and sea-bound outlet"
    );
}

#[test]
fn courses_have_no_dry_gaps_and_use_real_fluid_cells_across_negative_section_edges() {
    let terrain = terrain(42042);
    let regions: Vec<_> = regions(&terrain)
        .into_iter()
        .filter(|r| r.region[0] < 0 || r.region[1] < 0)
        .take(3)
        .collect();
    assert_eq!(regions.len(), 3);
    let journal = EditJournal::default();
    let mut crossed_sections = 0;
    for region in regions {
        let mut previous = None;
        let mut previous_level = f64::INFINITY;
        for pair in region.course.windows(2) {
            let steps =
                ((pair[1][0] - pair[0][0]).hypot(pair[1][2] - pair[0][2]) / 2.0).ceil() as usize;
            for step in 0..steps {
                let t = step as f64 / steps as f64;
                let x = pair[0][0] + (pair[1][0] - pair[0][0]) * t;
                let z = pair[0][2] + (pair[1][2] - pair[0][2]) * t;
                let river = terrain.river_at(x, z).unwrap();
                assert!(river.level <= previous_level + 1e-7);
                previous_level = river.level;
                let water = terrain.water_column(x, z).unwrap_or_else(|| {
                    panic!("dry course at seed42042 {x},{z} region{:?}", region.region)
                });
                assert!(water.bed < water.level);
                let cell = [
                    (x.floor() as i64).div_euclid(16),
                    (z.floor() as i64).div_euclid(16),
                ];
                if previous != Some(cell) {
                    crossed_sections += 1;
                    previous = Some(cell);
                    let gx = x.floor() as i64;
                    let gz = z.floor() as i64;
                    let actual = terrain
                        .water_column(gx as f64 + 0.5, gz as f64 + 0.5)
                        .unwrap();
                    let global = [gx, actual.bed.ceil() as i64, gz];
                    let material = terrain
                        .sample([gx as f64 + 0.5, global[1] as f64 + 0.5, gz as f64 + 0.5])
                        .material;
                    let fluid = fluid_cell_at(&terrain, &journal, global, material);
                    assert!(
                        fluid.source && fluid.amount > 0,
                        "missing source at {global:?}"
                    );
                }
            }
        }
    }
    assert!(crossed_sections > 100);
}

#[test]
fn lake_shores_are_supported_beyond_the_channel_and_basin_level_stays_constant() {
    let terrain = terrain(42042);
    let lakes: Vec<_> = regions(&terrain)
        .into_iter()
        .filter_map(|r| r.lake)
        .take(2)
        .collect();
    assert_eq!(lakes.len(), 2, "need actual generated lake fixtures");
    let mut shore = 0;
    let mut wet = 0;
    for lake in lakes {
        let reach = (lake.radius * 1.95 + BANK_REACH).ceil() as i32;
        // Check dense cross-sections rather than only accepting the centre.
        for dz in (-reach..=reach).step_by(3) {
            for dx in (-reach..=reach).step_by(3) {
                let x = (lake.center[0] + f64::from(dx)).floor() + 0.5;
                let z = (lake.center[1] + f64::from(dz)).floor() + 0.5;
                let Some(sample) = terrain.lake_at(x, z) else {
                    continue;
                };
                let Some(water) = terrain.water_column(x, z) else {
                    continue;
                };
                if sample.distance > sample.radius {
                    continue;
                }
                assert_eq!(water.level, lake.level);
                wet += 1;
                for [nx, nz] in [[x - 1.0, z], [x + 1.0, z], [x, z - 1.0], [x, z + 1.0]] {
                    if terrain.water_column(nx, nz).is_none() {
                        let bank = (terrain.surface_height(nx, nz) - 0.5).floor() + 1.0;
                        assert!(
                            bank >= water.level - 0.5,
                            "unsupported {nx},{nz}: {bank} < {}",
                            water.level
                        );
                        shore += 1;
                    }
                }
            }
        }
    }
    assert!(wet > 500 && shore > 30, "wet={wet}, shore={shore}");
}

#[test]
fn region_reload_query_order_and_negative_boundaries_preserve_hydrology() {
    let a = terrain(7);
    let b = terrain(7);
    let fixtures = regions(&a);
    assert!(fixtures.len() >= 2);
    for expected in fixtures.iter().rev() {
        let x = expected.region[0] as f64 * REGION + 1024.0;
        let z = expected.region[1] as f64 * REGION + 1024.0;
        assert_eq!(b.hydrology_region_at(x, z).as_ref(), Some(expected));
        a.rivers.1.lock().unwrap().clear();
        assert_eq!(a.hydrology_region_at(x, z).as_ref(), Some(expected));
    }
    for boundary in [-4096.0, -2048.0, 0.0, 2048.0] {
        for along in [-4096.5, -16.5, 0.5, 2048.5] {
            for [x, z] in [[boundary, along], [along, boundary]] {
                let left = a.surface_height(x - 0.00001, z - 0.00001);
                let right = a.surface_height(x + 0.00001, z + 0.00001);
                assert!((left - right).abs() < 0.001, "catchment seam {x},{z}");
            }
        }
    }
}

#[test]
fn new_lakes_vary_in_size_and_river_toggle_keeps_the_same_basin() {
    let terrain = terrain(99173);
    let lakes: Vec<_> = regions(&terrain)
        .into_iter()
        .filter_map(|r| r.lake)
        .collect();
    assert!(lakes.len() >= 2);
    let min = lakes.iter().map(|l| l.radius).fold(f64::INFINITY, f64::min);
    let max = lakes.iter().map(|l| l.radius).fold(0.0_f64, f64::max);
    assert!(max > 70.0 && max - min > 15.0, "lake radii {min}..{max}");
    let disabled = TerraGenerator::new(WorldConfig {
        rivers: false,
        ..terrain.config().clone()
    })
    .unwrap();
    let lake = lakes[0];
    assert!(disabled.river_at(lake.center[0], lake.center[1]).is_none());
    assert_eq!(
        terrain.lake_at(lake.center[0], lake.center[1]),
        disabled.lake_at(lake.center[0], lake.center[1])
    );
    let no_water = TerraGenerator::new(WorldConfig {
        water: false,
        ..terrain.config().clone()
    })
    .unwrap();
    assert!(no_water.lake_at(lake.center[0], lake.center[1]).is_none());
    assert!(no_water.river_at(lake.center[0], lake.center[1]).is_none());
}

#[test]
fn traced_union_lake_edges_and_fractional_river_banks_have_no_unsupported_water() {
    let mut lakes = 0;
    let mut banks = 0;
    for seed in [42042, 7, 99173] {
        let terrain = terrain(seed);
        for region in regions(&terrain) {
            if let Some(lake) = region.lake {
                lakes += 1;
                for angle in 0..360 {
                    let (sin, cos) = (f64::from(angle) * std::f64::consts::PI / 180.0).sin_cos();
                    let mut lower = 0.0;
                    let mut upper = lake.radius * 2.1 + BANK_REACH;
                    for _ in 0..16 {
                        let radius = (lower + upper) * 0.5;
                        let x = lake.center[0] + cos * radius;
                        let z = lake.center[1] + sin * radius;
                        if terrain.water_column(x, z).is_some_and(|water| {
                            water.level == lake.level
                                && terrain.lake_at(x, z).is_some_and(|s| s.distance < s.radius)
                        }) {
                            lower = radius;
                        } else {
                            upper = radius;
                        }
                    }
                    let x = (lake.center[0] + cos * lower).floor() as i64;
                    let z = (lake.center[1] + sin * lower).floor() as i64;
                    for dz in -1..=1 {
                        for dx in -1..=1 {
                            banks += assert_supported_shore(&terrain, x + dx, z + dz);
                        }
                    }
                }
            }
            for pair in region.course.windows(2) {
                for t in [0.0, 0.25, 0.5, 0.75] {
                    let x = pair[0][0] + (pair[1][0] - pair[0][0]) * t;
                    let z = pair[0][2] + (pair[1][2] - pair[0][2]) * t;
                    let sample = terrain.river_at(x, z).unwrap();
                    let reach = sample.width.ceil() as i32 + 2;
                    for across in -reach..=reach {
                        let gx = (x + f64::from(sample.flow[1]) * f64::from(across)).floor() as i64;
                        let gz = (z - f64::from(sample.flow[0]) * f64::from(across)).floor() as i64;
                        banks += assert_supported_shore(&terrain, gx, gz);
                    }
                }
            }
        }
    }
    assert!(
        lakes >= 4 && banks > 300,
        "lakes={lakes}, checked_edges={banks}"
    );
}

fn assert_supported_shore(terrain: &TerraGenerator, x: i64, z: i64) -> usize {
    let x = x as f64 + 0.5;
    let z = z as f64 + 0.5;
    let Some(water) = terrain.water_column(x, z) else {
        return 0;
    };
    let mut checked = 0;
    for [nx, nz] in [[x - 1.0, z], [x + 1.0, z], [x, z - 1.0], [x, z + 1.0]] {
        if terrain.water_column(nx, nz).is_some() {
            continue;
        }
        let bank = (terrain.surface_height(nx, nz) - 0.5).floor() + 1.0;
        assert!(
            bank >= water.level - 0.5,
            "unsupported shore seed={} at {x},{z} -> {nx},{nz}: bank={bank} level={} river={:?} neighbor_river={:?} lake={:?} neighbor_lake={:?} neighbor_raw={}",
            terrain.config().seed.0,
            water.level,
            terrain.river_at(x, z),
            terrain.river_at(nx, nz),
            terrain.lake_at(x, z),
            terrain.lake_at(nx, nz),
            terrain.base_height_with_biome(nx, nz, terrain.climate_biome_at(nx, nz)),
        );
        checked += 1;
    }
    checked
}

#[test]
fn fractional_shore_voxels_keep_their_wet_neighbors_supported() {
    let terrain = terrain(7);
    assert_supported_shore(&terrain, 1126, 3219);
}

#[test]
fn continuous_lake_connections_hold_one_level_and_never_reverse_the_flow() {
    let mut connections = 0;
    for seed in [42042, 7, 99173] {
        let terrain = terrain(seed);
        for region in regions(&terrain).into_iter().filter(|r| r.lake.is_some()) {
            let lake = region.lake.unwrap();
            let mut previous = f64::INFINITY;
            for pair in region.course.windows(2) {
                let steps = ((pair[1][0] - pair[0][0]).hypot(pair[1][2] - pair[0][2]) / 2.0).ceil()
                    as usize;
                for step in 0..steps {
                    let t = step as f64 / steps as f64;
                    let x = pair[0][0] + (pair[1][0] - pair[0][0]) * t;
                    let z = pair[0][2] + (pair[1][2] - pair[0][2]) * t;
                    let river = terrain.river_at(x, z).unwrap();
                    assert!(
                        river.level <= previous + 1e-7,
                        "reversed lake connection seed={seed} at {x},{z}: {previous} -> {}",
                        river.level,
                    );
                    previous = river.level;
                    let water = terrain.water_column(x, z).unwrap();
                    assert!(water.depth > 0.0);
                    if terrain.lake_at(x, z).is_some_and(|s| s.distance < s.radius) {
                        assert_eq!(river.level, lake.level);
                        assert_eq!(water.level, lake.level);
                        connections += 1;
                    }
                }
            }
        }
    }
    assert!(connections > 100);
}

#[test]
fn arbitrary_course_frames_preserve_world_flow_and_cross_chunk_coordinates() {
    for downstream in [[1.0, 0.0], [-1.0, 0.0], [0.0, -1.0], [0.6, 0.8]] {
        let frame = DrainageFrame {
            origin: [-2048.5, -16.5],
            downstream,
        };
        for [cross, along] in [[0.0, 0.0], [-16.0, 32.0], [8.5, 1856.0]] {
            let [x, z] = frame.world(cross, along);
            let projected = frame.project(x, z);
            assert!((projected[0] - cross).abs() < 1e-9);
            assert!((projected[1] - along).abs() < 1e-9);
        }
    }
    let terrain = terrain(7);
    for region in regions(&terrain).into_iter().take(3) {
        for pair in region.course.windows(2) {
            let p = pair[0];
            let sample = terrain.river_at(p[0], p[2]).unwrap();
            assert!((sample.flow[0].hypot(sample.flow[1]) - 1.0).abs() < 1e-5);
            let dx = pair[1][0] - p[0];
            let dz = pair[1][2] - p[2];
            assert!(f64::from(sample.flow[0]) * dx + f64::from(sample.flow[1]) * dz > 0.0);
        }
    }
}
