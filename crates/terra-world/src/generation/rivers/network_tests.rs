use super::*;
use crate::{TerrainGenerator, edits::EditJournal, water::fluid_cell_at};
use terra_core::{WorldConfig, WorldSeed};

#[test]
fn drainage_accumulation_is_acyclic_conservative_and_reaches_its_actual_outlet() {
    let heights =
        std::array::from_fn(|index| 120.0 - (index / GRID) as f64 * 10.0 - (index % GRID) as f64);
    let mut outlets = [None; CELLS];
    outlets[CELLS - 1] = Some((24, 10.0));
    let rainfall = std::array::from_fn(|index| 1.0 + index as f64 / CELLS as f64);
    let drainage = Drainage::build(heights, rainfall, outlets);
    let total: f64 = rainfall.into_iter().sum();
    assert!((drainage.accumulated[CELLS - 1] - total).abs() < 1e-10);
    for start in 0..CELLS {
        let (route, trunk) = drainage.route(start).unwrap();
        assert_eq!(trunk, 24);
        assert!(
            route
                .windows(2)
                .all(|pair| heights[pair[1]] < heights[pair[0]])
        );
        assert_eq!(
            route.len(),
            route
                .iter()
                .collect::<std::collections::BTreeSet<_>>()
                .len()
        );
        assert_eq!(drainage.route(start), Some((route, trunk)));
    }
    let dry = Drainage::build(heights, [0.0; CELLS], outlets);
    assert!(dry.accumulated.into_iter().all(|water| water == 0.0));
}

#[test]
fn enclosed_drainage_sinks_do_not_create_a_fake_connection_or_loop() {
    let mut heights = [20.0; CELLS];
    heights[24] = 0.0;
    let drainage = Drainage::build(heights, [1.0; CELLS], [None; CELLS]);
    assert!(drainage.route(24).is_none());
    assert!(drainage.route(16).is_none());
    assert!(drainage.receiver[24].is_none());
    let terminal: f64 = (0..CELLS)
        .filter(|&i| drainage.receiver[i].is_none())
        .map(|i| drainage.accumulated[i])
        .sum();
    assert_eq!(terminal, CELLS as f64);
}

#[test]
fn regional_eviction_removes_one_distant_completed_entry_and_never_inflight_cells() {
    use std::{
        collections::HashMap,
        sync::{Arc, OnceLock},
    };
    let mut cache = HashMap::new();
    for x in -128..128 {
        let entry = OnceLock::new();
        assert!(entry.set(None).is_ok());
        cache.insert([x, 0], Arc::new(entry));
    }
    let pending = Arc::new(OnceLock::new());
    cache.insert([-128, 0], Arc::clone(&pending));
    let near = Arc::clone(cache.get(&[0, 0]).unwrap());
    let victim = super::super::connected::completed_cache_victim(&cache, [128, 0]).unwrap();
    assert_eq!(victim, [-127, 0]);
    cache.remove(&victim);
    cache.insert([128, 0], Arc::new(OnceLock::new()));
    assert_eq!(cache.len(), 256);
    assert!(Arc::ptr_eq(cache.get(&[0, 0]).unwrap(), &near));
    assert!(Arc::ptr_eq(cache.get(&[-128, 0]).unwrap(), &pending));
    cache.retain(|_, entry| entry.get().is_none());
    assert!(super::super::connected::completed_cache_victim(&cache, [0, 0]).is_none());
}

#[test]
fn dry_trunk_bank_never_overrides_a_wet_incoming_tributary() {
    let terrain = terrain(42042, 5);
    let path = |x, width, brook| RiverPath {
        points: std::array::from_fn(|i| [0.0, 50.0, i as f64 * 8.0]),
        ground: [52.0; POINTS],
        extensions: [0.0; 2],
        width,
        phase: 0.0,
        branch: None,
        brook,
        step: 8.0,
        frame: DrainageFrame {
            origin: [x, 0.0],
            downstream: [0.0, 1.0],
        },
        lake: None,
    };
    let main = terrain
        .sample_drainage(&path(0.0, 3.0, false), 3.2, 0.0, None)
        .unwrap();
    let tributary = terrain
        .sample_drainage(&path(3.2, 2.0, true), 3.2, 0.0, None)
        .unwrap();
    assert!(main.distance > main.width && main.distance <= main.width + 1.0);
    assert_eq!(tributary.distance, 0.0);
    let selected = terrain
        .select_network_sample(3.2, 0.0, Some(main), Some(tributary))
        .unwrap();
    assert_eq!(selected.distance, 0.0);
    assert!((selected.bed_height() - 0.5).floor() + 1.0 < selected.level);
    let wet_main = terrain
        .sample_drainage(&path(0.0, 3.0, false), 0.0, 0.0, None)
        .unwrap();
    assert_eq!(
        terrain.select_network_sample(0.0, 0.0, Some(wet_main), Some(tributary)),
        Some(wet_main)
    );
}

#[test]
fn angled_tributaries_keep_real_water_at_nearest_voxel_centres_in_all_directions() {
    let terrain = terrain(42042, 5);
    for direction in [[0.0, 1.0], [1.0, 0.0], [0.0, -1.0], [-1.0, 0.0], [0.6, 0.8]] {
        for slope in [-4.0, -1.0, 0.0, 1.0, 4.0] {
            for level in [49.05, 49.49, 49.95] {
                let path = RiverPath {
                    points: std::array::from_fn(|i| {
                        [slope * i as f64 * 8.0, level, i as f64 * 8.0]
                    }),
                    ground: [52.0; POINTS],
                    extensions: [0.0; 2],
                    width: 1.25,
                    phase: 0.0,
                    branch: None,
                    brook: true,
                    step: 8.0,
                    frame: DrainageFrame {
                        origin: [-31.2, -16.6],
                        downstream: direction,
                    },
                    lake: None,
                };
                for point in path.points {
                    let world = path.frame.world(point[0], point[2]);
                    let [x, z] = world.map(|value| value.floor() + 0.5);
                    let sample = terrain.sample_tributary(&path, x, z).unwrap();
                    assert!(
                        sample.distance < sample.width,
                        "slope{slope} direction{direction:?} at{x},{z}: {sample:?}"
                    );
                    let bed = (sample.bed_height() - 0.5).floor() + 1.0;
                    assert!(
                        bed < sample.level,
                        "dry rounded bed slope{slope} direction{direction:?}: {sample:?}"
                    );
                }
            }
        }
    }
}

fn terrain(seed: u64, revision: u8) -> TerraGenerator {
    TerraGenerator::new(WorldConfig {
        seed: WorldSeed(seed),
        landscape_revision: revision,
        river_terrain_revision: 1,
        sea_level: 22.0,
        biome_definitions: Some(terra_core::BiomeDefinitions::default()),
        caves: false,
        trees: false,
        structures_revision: 0,
        rivers: true,
        ..Default::default()
    })
    .unwrap()
}

#[test]
fn revision_five_tributaries_have_real_connected_water_and_survive_reload_and_edits() {
    let mut tributaries = 0;
    let mut negative_sections = 0;
    for seed in [42042, 7, 99173] {
        let terrain = terrain(seed, 5);
        let mut tested = 0;
        for z in -2..=1 {
            for x in -2..=1 {
                let position = [
                    f64::from(x) * 2048.0 + 1024.0,
                    f64::from(z) * 2048.0 + 1024.0,
                ];
                let Some(region) = terrain.hydrology_region_at(position[0], position[1]) else {
                    continue;
                };
                assert!(region.tributaries.len() <= MAX_TRIBUTARIES);
                for tributary in &region.tributaries {
                    tributaries += 1;
                    assert!(tributary.accumulated_runoff > 0.0);
                    assert!(
                        tributary
                            .course
                            .windows(2)
                            .all(|pair| pair[1][1] <= pair[0][1])
                    );
                    let main = terrain
                        .river_at(tributary.mouth[0], tributary.mouth[2])
                        .unwrap();
                    assert!((main.level - tributary.mouth[1]).abs() < 1e-6);
                    for point in tributary.course {
                        let water = terrain
                            .water_column(point[0], point[2])
                            .unwrap_or_else(|| panic!("dry tributary seed{seed}: {point:?}, river={:?}, base={}, surface={}, lake={:?}, oasis={:?}", terrain.river_at(point[0],point[2]), terrain.terrain_base_height(point[0],point[2]), terrain.surface_height(point[0],point[2]), terrain.lake_at(point[0],point[2]), terrain.oasis_at(point[0],point[2])));
                        assert!(water.bed < water.level);
                        let gx = point[0].floor() as i64;
                        let gz = point[2].floor() as i64;
                        let actual = terrain
                            .water_column(gx as f64 + 0.5, gz as f64 + 0.5)
                            .unwrap_or_else(|| panic!("dry tributary voxel seed{seed}: centre={point:?} voxel={gx},{gz}, river={:?}, base={}, surface={}, lake={:?}, oasis={:?}", terrain.river_at(gx as f64+0.5,gz as f64+0.5), terrain.terrain_base_height(gx as f64+0.5,gz as f64+0.5), terrain.surface_height(gx as f64+0.5,gz as f64+0.5), terrain.lake_at(gx as f64+0.5,gz as f64+0.5), terrain.oasis_at(gx as f64+0.5,gz as f64+0.5)));
                        let global = [gx, actual.bed.ceil() as i64, gz];
                        negative_sections += usize::from(global[0] < 0 || global[2] < 0);
                        let material = terrain
                            .sample([
                                global[0] as f64 + 0.5,
                                global[1] as f64 + 0.5,
                                global[2] as f64 + 0.5,
                            ])
                            .material;
                        let fluid =
                            fluid_cell_at(&terrain, &EditJournal::default(), global, material);
                        assert!(
                            fluid.amount > 0 && fluid.source,
                            "missing natural fluid at {global:?}"
                        );
                        let mut edits = EditJournal::default();
                        edits
                            .record(global, terra_voxel::MaterialId::Stone as u16)
                            .unwrap();
                        assert_eq!(
                            fluid_cell_at(
                                &terrain,
                                &edits,
                                global,
                                terra_voxel::MaterialId::Stone as u16
                            )
                            .amount,
                            0
                        );
                    }
                }
                if !region.tributaries.is_empty() {
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
                    tested += 1;
                    if tested >= 2 {
                        break;
                    }
                }
            }
            if tested >= 2 {
                break;
            }
        }
    }
    assert!(
        tributaries >= 3,
        "need real connected tributaries across seeds, found {tributaries}"
    );
    assert!(negative_sections > 0);
}

#[test]
fn revisions_three_and_four_retain_unbranched_catchment_topology() {
    for revision in [3, 4] {
        let terrain = terrain(42042, revision);
        for position in [[-1024.0, -1024.0], [1024.0, 1024.0], [-3072.0, 1024.0]] {
            if let Some(region) = terrain.hydrology_region_at(position[0], position[1]) {
                assert!(region.tributaries.is_empty());
            }
        }
    }
}
