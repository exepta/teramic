use super::*;
use crate::{TerrainGenerator, edits::EditJournal, water::fluid_cell_at};
use std::sync::{Arc, OnceLock};
use terra_core::{Config, WorldConfig, WorldSeed};

fn terrain(seed: u64, revision: u8) -> TerraGenerator {
    TerraGenerator::new(WorldConfig {
        seed: WorldSeed(seed),
        river_terrain_revision: revision,
        landscape_revision: 1,
        ecology_revision: 0,
        caves: false,
        trees: false,
        structures_revision: 0,
        rivers: true,
        ..Default::default()
    })
    .unwrap()
}

fn trunk() -> RiverPath {
    RiverPath {
        points: std::array::from_fn(|i| [0.0, 44.0 - i as f64 / 6.0, i as f64 * 24.0]),
        ground: [64.0; POINTS],
        extensions: [0.0; 2],
        width: 4.0,
        phase: 0.3,
        branch: None,
        brook: false,
        step: 24.0,
        frame: DrainageFrame {
            origin: [-1536.0, -1536.0],
            downstream: [0.0, 1.0],
        },
        lake: None,
    }
}

fn flat() -> TerraGenerator {
    TerraGenerator::new(WorldConfig {
        base_height: 64.0,
        hill_amplitude: 1e-9,
        mountain_amplitude: 0.0,
        biomes: false,
        ..terrain(42042, 2).config().clone()
    })
    .unwrap()
}

#[test]
fn long_channel_segments_cannot_cross_between_their_sampled_points() {
    let mut horizontal = trunk();
    horizontal.width = 2.0;
    horizontal.step = 35.0;
    horizontal.points = std::array::from_fn(|i| [0.0, 40.0, i as f64 * horizontal.step]);
    horizontal.frame = DrainageFrame {
        origin: [-4096.0 - 857.5, -4096.0],
        downstream: [1.0, 0.0],
    };
    let mut vertical = trunk();
    vertical.width = 2.0;
    vertical.step = 35.0;
    vertical.points = std::array::from_fn(|i| [0.0, 40.0, i as f64 * vertical.step]);
    vertical.frame = DrainageFrame {
        origin: [-4096.0, -4096.0 - 857.5],
        downstream: [0.0, 1.0],
    };
    let guard = horizontal.width + vertical.width + 8.0;
    for point in [world(&horizontal, 24), world(&horizontal, 25)] {
        assert!(segment_distance(point, world(&vertical, 24), world(&vertical, 25)) > guard);
    }
    assert_eq!(
        segment_pair_distance(
            world(&horizontal, 24),
            world(&horizontal, 25),
            world(&vertical, 24),
            world(&vertical, 25),
        ),
        0.0
    );
    assert!(crosses_course(&horizontal, &vertical, false));
    vertical.frame.origin[0] -= 2000.0;
    assert!(!crosses_course(&horizontal, &vertical, false));
}

#[test]
fn real_fork_keeps_shared_split_but_has_its_own_course_direction_and_mouth() {
    let terrain = flat();
    let main = trunk();
    let split = 16;
    let target = Outlet {
        point: [-800.0, -450.0],
        level: 36.0,
        sea: false,
    };
    let branch = terrain.distributary_path(&main, split, target).unwrap();
    assert_eq!(world(&branch, 0), world(&main, split));
    assert_eq!(branch.points[0][1], main.points[split][1]);
    assert!(distance(world(&branch, POINTS - 1), target.point) < 1e-9);
    assert!(distance(world(&branch, POINTS - 1), world(&main, POINTS - 1)) > 700.0);
    assert!(
        branch
            .points
            .windows(2)
            .all(|pair| pair[1][1] <= pair[0][1])
    );
    assert_eq!(branch.points[POINTS - 1][1], target.level);
    let parent_flow = terrain
        .sample_drainage(&main, world(&main, split)[0], world(&main, split)[1], None)
        .unwrap()
        .flow;
    let branch_point = world(&branch, POINTS / 2);
    let branch_flow = terrain
        .sample_tributary(&branch, branch_point[0], branch_point[1])
        .unwrap()
        .flow;
    let dot = parent_flow[0] * branch_flow[0] + parent_flow[1] * branch_flow[1];
    assert!(dot < 0.9, "independent downstream directions, dot={dot}");
    assert!(!crosses_course(&branch, &main, true));
}

#[test]
fn branch_columns_share_negative_chunk_boundaries_and_have_solid_water_support() {
    let terrain = flat();
    let main = trunk();
    let split = 16;
    let branch = terrain
        .distributary_path(
            &main,
            split,
            Outlet {
                point: [-800.0, -450.0],
                level: 36.0,
                sea: false,
            },
        )
        .unwrap();
    let samples: Vec<_> = (0..=192)
        .map(|i| {
            let along = i as f64 / 192.0 * (POINTS - 1) as f64;
            let a = along.floor() as usize;
            let b = (a + 1).min(POINTS - 1);
            let [x, z] = branch.frame.world(
                branch.points[a][0] + (branch.points[b][0] - branch.points[a][0]) * along.fract(),
                along * branch.step,
            );
            [x.floor() + 0.5, z.floor() + 0.5]
        })
        .collect();
    let entry = OnceLock::new();
    assert!(
        entry
            .set(Some(Arc::new(RiverNetwork {
                main,
                tributaries: Vec::new(),
                distributaries: vec![Distributary {
                    path: branch,
                    split_index: split,
                    reaches_sea: false,
                }],
            })))
            .is_ok()
    );
    terrain
        .rivers
        .1
        .lock()
        .unwrap()
        .insert([-1, -1], Arc::new(entry));
    let mut sections = std::collections::BTreeSet::new();
    let edits = EditJournal::default();
    for [x, z] in samples {
        let column = terrain.surface_column(x, z);
        let water = terrain.water_column(x, z).unwrap();
        assert_eq!(column.water_bed(), water.bed);
        assert_eq!(column.water_level, Some(water.level));
        assert!(water.bed < water.level);
        let global = [x.floor() as i64, water.bed as i64, z.floor() as i64];
        sections.insert([global[0].div_euclid(16), global[2].div_euclid(16)]);
        let floor = terrain.sample([x, water.bed - 0.5, z]);
        assert!(floor.density <= 0.0, "unsupported water at {global:?}");
        let material = terrain.sample([x, water.bed + 0.5, z]).material;
        assert!(fluid_cell_at(&terrain, &edits, global, material).amount > 0);
        assert_eq!(terrain.surface_column(x, z), column);
    }
    assert!(
        sections.len() > 40,
        "must cross real negative chunk boundaries"
    );
}

#[test]
fn generated_distributaries_reach_reservoirs_and_survive_cache_eviction_and_reload() {
    let mut found = 0;
    let mut negative = 0;
    for seed in [42042, 7, 99173] {
        let terrain = terrain(seed, 2);
        let mut checked = false;
        'regions: for z in -2..=1 {
            for x in -2..=1 {
                let position = [
                    f64::from(x) * 2048.0 + 1024.0,
                    f64::from(z) * 2048.0 + 1024.0,
                ];
                let Some(region) = terrain.hydrology_region_at(position[0], position[1]) else {
                    continue;
                };
                assert!(region.distributaries.len() <= MAX_DISTRIBUTARIES);
                if region.distributaries.is_empty() {
                    continue;
                }
                let network = {
                    let cache = terrain.rivers.1.lock().unwrap();
                    Arc::clone(
                        cache
                            .get(&region.region)
                            .unwrap()
                            .get()
                            .unwrap()
                            .as_ref()
                            .unwrap(),
                    )
                };
                for (index, branch) in region.distributaries.iter().enumerate() {
                    found += 1;
                    negative += usize::from(branch.source[0] < 0.0 || branch.source[2] < 0.0);
                    assert_eq!(branch.source, region.course[branch.split_index]);
                    assert!(
                        terrain.terrain_base_height(branch.source[0], branch.source[2])
                            > terrain.config().sea_level + 2.0
                    );
                    assert!(
                        terrain
                            .exposed_land_reach(&network.main, branch.split_index, None)
                            .0
                            >= MIN_LAND_REACH
                    );
                    let (length, separated) = terrain.exposed_land_reach(
                        &network.distributaries[index].path,
                        0,
                        Some(&network.main),
                    );
                    assert!(
                        length >= MIN_LAND_REACH && separated,
                        "a fork must expose its independent channel on actual land"
                    );
                    assert!(
                        distance(
                            [branch.mouth[0], branch.mouth[2]],
                            [region.mouth[0], region.mouth[2]]
                        ) >= 24.0
                    );
                    assert!(
                        branch
                            .course
                            .windows(2)
                            .all(|pair| pair[1][1] <= pair[0][1])
                    );
                    if branch.reaches_sea {
                        assert_eq!(branch.mouth[1], terrain.config().sea_level);
                        assert!(terrain.connected_ocean_outlet([branch.mouth[0], branch.mouth[2]]));
                    } else {
                        let lake = terrain.lake_at(branch.mouth[0], branch.mouth[2]).unwrap();
                        assert!(lake.distance < lake.radius);
                        assert_eq!(branch.mouth[1], lake.level);
                    }
                    for point in branch.course {
                        let [x, z] = [point[0].floor() + 0.5, point[2].floor() + 0.5];
                        let water = terrain.water_column(x, z).unwrap_or_else(|| {
                            panic!(
                                "seed={seed} dry branch at {point:?}, sample={:?}",
                                terrain.river_at(x, z)
                            )
                        });
                        assert!(water.bed < water.level);
                        assert!(terrain.sample([x, water.bed - 0.5, z]).density <= 0.0);
                    }
                }
                let reload = TerraGenerator::new(terrain.config().clone()).unwrap();
                assert_eq!(
                    reload.hydrology_region_at(position[0], position[1]),
                    Some(region.clone())
                );
                terrain.rivers.1.lock().unwrap().clear();
                assert_eq!(
                    terrain.hydrology_region_at(position[0], position[1]),
                    Some(region)
                );
                checked = true;
                break 'regions;
            }
        }
        assert!(checked, "seed {seed} needs an actual branching catchment");
    }
    assert!(found >= 3);
    assert!(negative > 0);
}

#[test]
fn submerged_ocean_courses_are_not_land_river_forks() {
    let terrain = TerraGenerator::new(WorldConfig {
        base_height: -64.0,
        ..flat().config().clone()
    })
    .unwrap();
    let mut path = trunk();
    path.ground.fill(4.0);
    for point in &mut path.points {
        point[1] = terrain.config().sea_level;
    }
    let point = world(&path, 16);
    assert!(
        terrain.terrain_base_height(point[0], point[1]) < terrain.config().sea_level - 1.0,
        "fixture needs an actual submerged seabed, not just low cached path ground"
    );
    let sample = terrain
        .sample_drainage(&path, point[0], point[1], None)
        .unwrap();
    assert!(!sample.covered && sample.roof_blend == 0.0);
    assert!(
        terrain.water_column(point[0], point[1]).is_some(),
        "ocean water alone is a false positive"
    );
    assert_eq!(terrain.exposed_land_reach(&path, 16, None).0, 0.0);
}

#[test]
fn old_hydrology_contracts_keep_their_original_routing_and_main_heightfield() {
    for revision in 0..=1 {
        let terrain = terrain(42042, revision);
        assert!(!terrain.connected_hydrology());
        assert!(terrain.hydrology_region_at(-1024.0, -1024.0).is_none());
        let path = std::env::temp_dir().join(format!(
            "teramic-river-contract-{}-{revision}-{}.toml",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        terrain.config().save(&path).unwrap();
        let saved = WorldConfig::load(&path).unwrap();
        std::fs::remove_file(&path).unwrap();
        assert_eq!(&saved, terrain.config());
        let restored = TerraGenerator::new(saved).unwrap();
        for [x, z] in [[-2048.5, -16.5], [-0.5, -0.5], [1024.5, 32.5]] {
            assert_eq!(terrain.surface_column(x, z), restored.surface_column(x, z));
        }
    }
    let modern = terrain(42042, 2);
    assert!(modern.connected_hydrology());
    let old = terrain(42042, 1);
    for [x, z] in [[-2048.5, -16.5], [-0.5, -0.5], [1024.5, 32.5]] {
        assert_eq!(
            modern.terrain_base_height(x, z),
            old.terrain_base_height(x, z)
        );
    }
}

#[test]
fn ocean_offshoots_bypass_intermediate_lakes_without_sampled_water_level_ascents() {
    let terrain = flat();
    let factory = TerraGenerator::new(WorldConfig {
        base_height: 64.0,
        hill_amplitude: 0.0,
        mountain_amplitude: 0.0,
        biomes: true,
        ..terrain.config().clone()
    })
    .unwrap();
    let mut lake = (-4..=4)
        .flat_map(|z| (-4..=4).map(move |x| [f64::from(x) * 512.0, f64::from(z) * 512.0]))
        .find_map(|center| factory.build_regional_lake(center, 0))
        .expect("uniform dry terrain must admit a validated reservoir");
    let mut main = trunk();
    for point in &mut main.points {
        point[1] = lake.level;
    }
    lake.center = world(&main, 32);
    lake.radius = 160.0;
    let crossing = Outlet {
        point: [-1200.0, -450.0],
        level: terrain.config().sea_level,
        sea: true,
    };
    let mut unsafe_route = terrain.distributary_path(&main, 16, crossing).unwrap();
    unsafe_route.lake = Some(lake);
    let sampled_levels = |path: &RiverPath| {
        (0..=512)
            .map(|i| {
                let at = f64::from(i) / 512.0 * (POINTS - 1) as f64;
                let a = at.floor() as usize;
                let b = (a + 1).min(POINTS - 1);
                let [x, z] = path.frame.world(
                    path.points[a][0] + (path.points[b][0] - path.points[a][0]) * at.fract(),
                    at * path.step,
                );
                terrain.sample_tributary(path, x, z).unwrap().level
            })
            .collect::<Vec<_>>()
    };
    assert!(
        sampled_levels(&unsafe_route)
            .windows(2)
            .any(|pair| pair[1] > pair[0] + 1e-6),
        "fixture must reproduce the actual shoreline-blend ascent"
    );
    main.lake = Some(lake);
    assert!(terrain.distributary_path(&main, 16, crossing).is_none());
    let bypass = terrain
        .distributary_path(
            &main,
            16,
            Outlet {
                point: [-600.0, -450.0],
                ..crossing
            },
        )
        .unwrap();
    assert!(
        sampled_levels(&bypass)
            .windows(2)
            .all(|pair| pair[1] <= pair[0] + 1e-6)
    );
}
