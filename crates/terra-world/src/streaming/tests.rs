use super::resources::PendingSections;
use super::*;
use crate::{TerraGenerator, edits::EditJournal};
use bevy::prelude::*;
use std::time::{Duration, Instant};
use terra_core::{GameConfig, TerraCorePlugin};
use terra_voxel::{MaterialId, SectionPos, global_to_section};

#[test]
fn spanning_web_retires_when_its_remote_anchor_is_removed_and_stays_removed_after_reload() {
    use crate::props::{PropId, webs};
    let prop = webs::spanning([-1, 11, 8], [1, 0, 0], 21, 10, PropId(4120)).unwrap();
    let remote = webs::anchors(&prop).unwrap()[1];
    assert_eq!(global_to_section(remote).0.x - prop.section.x, 2);
    let mut chunks = ChunkStore::default();
    for x in -1..=1 {
        let position = SectionPos::new(x, 0, 0);
        chunks.loaded.insert(
            position,
            LoadedSection {
                data: terra_voxel::BlockSection::empty(position).unwrap(),
                mesh: None,
                mesh_epoch: 0,
                water_epoch: 0,
                water: None,
                props: if position == prop.section {
                    vec![prop]
                } else {
                    Vec::new()
                },
            },
        );
    }
    let mut journal = EditJournal::default();
    assert!(journal.place_prop(prop).unwrap());
    for anchor in webs::anchors(&prop).unwrap() {
        let (owner, local) = global_to_section(anchor);
        chunks
            .loaded
            .get_mut(&owner)
            .unwrap()
            .data
            .set_block(local, 3)
            .unwrap();
        journal.record(anchor, 3).unwrap();
    }
    assert!(
        commit_block_edit(
            &mut chunks,
            &mut journal,
            &BlockEditRequest {
                position: remote,
                expected_material: 3,
                material: 0,
            },
            &Default::default()
        )
        .unwrap()
    );
    assert!(chunks.loaded[&prop.section].props[0].removed);
    assert!(journal.prop_removed(prop.section, prop.id));
    let mut replica = EditJournal::default();
    for section in journal.changed_sections() {
        replica
            .import_replica_delta(journal.section_delta(section))
            .unwrap();
    }
    let mut reloaded = vec![prop];
    replica.apply_prop_removals(prop.section, &mut reloaded);
    assert!(reloaded[0].removed);
    let mut journal = EditJournal::default();
    journal.place_prop(prop).unwrap();
    chunks.loaded.remove(&prop.section);
    let (remote_owner, local) = global_to_section(remote);
    chunks
        .loaded
        .get_mut(&remote_owner)
        .unwrap()
        .data
        .set_block(local, 3)
        .unwrap();
    journal.record(remote, 3).unwrap();
    assert!(
        commit_block_edit(
            &mut chunks,
            &mut journal,
            &BlockEditRequest {
                position: remote,
                expected_material: 3,
                material: 0,
            },
            &Default::default()
        )
        .unwrap()
    );
    assert!(journal.prop_removed(prop.section, prop.id));
    assert_eq!(journal.placed_props(prop.section).count(), 0);
    let mut reloaded = vec![prop];
    journal.apply_prop_removals(prop.section, &mut reloaded);
    assert!(
        reloaded[0].removed,
        "an unloaded owner cannot resurrect a cut attachment"
    );
}

#[test]
fn opening_pose_commits_rebuild_loaded_halos_without_another_block_edit() {
    use crate::edits::BlockObjectState;
    for material in 65..=70 {
        let mut app = app();
        app.insert_resource(ChunkFocus([0.5, 15.0, 0.5]));
        wait(&mut app, ready);
        let position = [-1, 15, -1];
        let mut cells = vec![position];
        if material <= 67 {
            cells.push([-1, 16, -1]);
        }
        for (index, &cell) in cells.iter().enumerate() {
            app.world_mut()
                .resource_scope(|world, mut chunks: Mut<ChunkStore>| {
                    let mut journal = world.resource_mut::<EditJournal>();
                    assert!(
                        commit_block_edit(
                            &mut chunks,
                            &mut journal,
                            &BlockEditRequest {
                                position: cell,
                                expected_material: 0,
                                material,
                            },
                            &Default::default()
                        )
                        .unwrap()
                    );
                    commit_object_state(
                        &mut chunks,
                        &mut journal,
                        cell,
                        Some(BlockObjectState {
                            peer: (cells.len() == 2).then(|| cells[1 - index]),
                            secondary: index == 1,
                            ..default()
                        }),
                    )
                    .unwrap();
                });
        }
        wait(&mut app, |world| {
            let chunks = world.resource::<ChunkStore>();
            let journal = world.resource::<EditJournal>();
            chunks
                .loaded
                .iter()
                .all(|(p, s)| s.mesh_epoch == journal.mesh_epoch(*p))
                && chunks.dirty.is_empty()
        });
        for _ in 0..3 {
            for &cell in &cells {
                app.world_mut()
                    .resource_scope(|world, mut chunks: Mut<ChunkStore>| {
                        let mut journal = world.resource_mut::<EditJournal>();
                        let mut state = journal.object_state(cell).unwrap();
                        state.facing ^= 4;
                        commit_object_state(&mut chunks, &mut journal, cell, Some(state)).unwrap();
                        assert!(chunks.dirty.contains(&global_to_section(cell).0));
                    });
            }
            wait(&mut app, |world| {
                let chunks = world.resource::<ChunkStore>();
                let journal = world.resource::<EditJournal>();
                chunks
                    .loaded
                    .iter()
                    .all(|(owner, s)| s.mesh_epoch == journal.mesh_epoch(*owner))
                    && chunks.dirty.is_empty()
            });
        }
    }
}

#[test]
fn solid_placement_retires_only_the_intersecting_tall_frond_across_a_vertical_section_edge() {
    use crate::props::{PropId, PropKind, WorldProp};
    let support = [-1, 14, -1];
    let owner = global_to_section(support).0;
    let plant = WorldProp {
        id: PropId(79),
        kind: PropKind::Seagrass,
        position: [-0.5, 15.0, -0.5],
        rotation: 0.0,
        scale: 1.5,
        variant: 0,
        section: owner,
        removed: false,
    };
    let neighbor = WorldProp {
        id: PropId(80),
        position: [-3.5, 15.0, -0.5],
        ..plant
    };
    let hit = [-1, 18, -1];
    let mut chunks = ChunkStore::default();
    for position in [owner, global_to_section(hit).0] {
        chunks.loaded.insert(
            position,
            LoadedSection {
                data: terra_voxel::BlockSection::empty(position).unwrap(),
                mesh: None,
                mesh_epoch: 0,
                water_epoch: 0,
                water: None,
                props: if position == owner {
                    vec![plant, neighbor]
                } else {
                    Vec::new()
                },
            },
        );
    }
    let properties = terra_core::block_properties::BlockPropertiesCatalog::load(
        &terra_core::asset_directory().join("terra/blocks"),
    )
    .unwrap();
    let mut journal = EditJournal::default();
    assert!(
        commit_block_edit(
            &mut chunks,
            &mut journal,
            &BlockEditRequest {
                position: hit,
                expected_material: 0,
                material: MaterialId::Stone as u16,
            },
            &properties
        )
        .unwrap()
    );
    assert!(chunks.loaded[&owner].props[0].removed);
    assert!(!chunks.loaded[&owner].props[1].removed);
    let mut reloaded = EditJournal::default();
    reloaded.import_delta(journal.section_delta(owner)).unwrap();
    let mut regenerated = vec![plant, neighbor];
    reloaded.apply_prop_removals(owner, &mut regenerated);
    assert!(
        regenerated
            .iter()
            .find(|p| p.id == plant.id)
            .unwrap()
            .removed
    );
    assert!(
        !regenerated
            .iter()
            .find(|p| p.id == neighbor.id)
            .unwrap()
            .removed
    );
}

#[test]
fn prepared_fluid_queries_match_noise_with_live_overrides_and_negative_coordinates() {
    let terrain = TerraGenerator::new(terra_core::WorldConfig {
        rivers: true,
        ..Default::default()
    })
    .unwrap();
    let generator = crate::SectionGenerator::new(terrain.clone());
    let mut chunks = ChunkStore::default();
    let mut journal = EditJournal::default();
    let mut samples = Vec::new();
    for coordinate in [[-1, -1], [0, 0], [2, -3]] {
        let column = std::sync::Arc::new(generator.prepare_column(coordinate, true).unwrap());
        chunks.columns.insert(coordinate, column);
        for z in 0..16 {
            for x in 0..16 {
                for y in [-20, 0, 7, 8, 20] {
                    let p = [coordinate[0] * 16 + x, y, coordinate[1] * 16 + z];
                    assert_eq!(
                        chunks.fluid_cell_at(&terrain, &journal, p, 0),
                        crate::water::fluid_cell_at(&terrain, &journal, p, 0)
                    );
                    samples.push(p);
                }
            }
        }
    }
    let start = Instant::now();
    let mut old = 0;
    for &p in &samples {
        old += u64::from(
            std::hint::black_box(crate::water::fluid_cell_at(&terrain, &journal, p, 0)).amount,
        );
    }
    let old_time = start.elapsed();
    let start = Instant::now();
    let mut cached = 0;
    for &p in &samples {
        cached +=
            u64::from(std::hint::black_box(chunks.fluid_cell_at(&terrain, &journal, p, 0)).amount);
    }
    let cached_time = start.elapsed();
    assert_eq!(cached, old);
    println!(
        "fluid query microbenchmark: samples={} uncached_us={} prepared_us={}",
        samples.len(),
        old_time.as_micros(),
        cached_time.as_micros()
    );
    let p = [-1, 30, -1];
    let fluid = crate::water::FluidCell {
        amount: 4,
        source: false,
        flow: [1, 0],
    };
    journal.set_fluid(p, fluid).unwrap();
    assert_eq!(chunks.fluid_cell_at(&terrain, &journal, p, 0), fluid);
    assert_eq!(
        chunks.fluid_cell_at(&terrain, &journal, p, 3),
        crate::water::FluidCell::default()
    );
}

#[test]
fn prepared_water_need_probes_preserve_dry_fallback_disabled_and_replaced_cells() {
    use crate::water::FluidCell;
    let terrain = TerraGenerator::new(terra_core::WorldConfig {
        base_height: 3.0,
        hill_amplitude: 0.0,
        mountain_amplitude: 0.0,
        sea_level: 6.0,
        caves: false,
        trees: false,
        biomes: false,
        rivers: false,
        water: true,
        ..Default::default()
    })
    .unwrap();
    let generator = crate::SectionGenerator::new(terrain.clone());
    let mut chunks = ChunkStore::default();
    for coordinate in [[-1, -1], [0, 0]] {
        chunks.columns.insert(
            coordinate,
            std::sync::Arc::new(generator.prepare_column(coordinate, false).unwrap()),
        );
    }
    let mut journal = EditJournal::default();
    for p in [
        [-1, 5, -1],
        [0, 5, 0],
        [-17, 5, -17],
        [-1, 12, -1],
        [0, -16, 0],
    ] {
        for material in [0, 3, 25] {
            assert_eq!(
                chunks.fluid_cell_at(&terrain, &journal, p, material),
                crate::water::fluid_cell_at(&terrain, &journal, p, material)
            );
        }
    }
    assert!(
        chunks
            .fluid_cell_at(&terrain, &journal, [-1, 5, -1], 0)
            .source
    );
    assert_eq!(
        chunks.fluid_cell_at(&terrain, &journal, [-1, 12, -1], 0),
        FluidCell::default()
    );
    assert!(chunks.prepared_column([-2, -2]).is_none());
    assert!(
        chunks
            .fluid_cell_at(&terrain, &journal, [-17, 5, -17], 0)
            .source
    );
    journal
        .set_fluid(
            [-1, 5, -1],
            FluidCell {
                amount: 8,
                flow: [0, 0],
                source: true,
            },
        )
        .unwrap();
    for cell in [
        FluidCell::default(),
        FluidCell {
            amount: 3,
            flow: [1, -1],
            source: false,
        },
        FluidCell {
            amount: 8,
            flow: [0, 0],
            source: true,
        },
    ] {
        journal.set_fluid([-1, 5, -1], cell).unwrap();
        let cached = chunks.fluid_cell_at(&terrain, &journal, [-1, 5, -1], 0);
        let uncached = crate::water::fluid_cell_at(&terrain, &journal, [-1, 5, -1], 0);
        assert_eq!(cached, uncached);
        if cell.amount == 0 {
            assert!(journal.fluid([-1, 5, -1]).is_none());
            let natural =
                crate::water::fluid_cell_at(&terrain, &EditJournal::default(), [-1, 5, -1], 0);
            assert_eq!(cached, natural);
        } else {
            assert_eq!(cached, cell);
        }
        assert_eq!(
            chunks.fluid_cell_at(&terrain, &journal, [-1, 5, -1], 3),
            FluidCell::default()
        );
    }
    let mut config = terrain.config().clone();
    config.water = false;
    let dry = TerraGenerator::new(config).unwrap();
    assert_eq!(
        chunks.fluid_cell_at(&dry, &journal, [-1, 5, -1], 0),
        FluidCell::default()
    );
}

#[test]
#[ignore = "opt-in wall-clock comparison; run after native GPU baselines"]
fn water_need_probe_radius_microbenchmark_compares_prepared_and_uncached_paths() {
    let terrain = TerraGenerator::new(terra_core::WorldConfig::default()).unwrap();
    let generator = crate::SectionGenerator::new(terrain.clone());
    let mut chunks = ChunkStore::default();
    for coordinate in [[-1, -1], [-1, 0], [0, -1], [0, 0]] {
        chunks.columns.insert(
            coordinate,
            std::sync::Arc::new(generator.prepare_column(coordinate, false).unwrap()),
        );
    }
    let journal = EditJournal::default();
    for radius in [2, 6] {
        let mut positions = Vec::new();
        for r in 1..=radius {
            for [dx, dz] in [
                [1, 0],
                [-1, 0],
                [0, 1],
                [0, -1],
                [1, 1],
                [-1, 1],
                [1, -1],
                [-1, -1],
            ] {
                for dy in [-2, -1, 0, 1] {
                    positions.push([-1 + dx * r, 3 + dy, -1 + dz * r]);
                }
            }
        }
        let expected: Vec<_> = positions
            .iter()
            .map(|&p| crate::water::fluid_cell_at(&terrain, &journal, p, 0))
            .collect();
        let actual: Vec<_> = positions
            .iter()
            .map(|&p| chunks.fluid_cell_at(&terrain, &journal, p, 0))
            .collect();
        assert_eq!(actual, expected);
        let mut elapsed = [Duration::ZERO; 2];
        let mut sums = [0u64; 2];
        // Alternate order so neither path always benefits from the first warm sweep.
        for repetition in 0..16 {
            for kind in [repetition % 2, 1 - repetition % 2] {
                let start = Instant::now();
                for &p in &positions {
                    let fluid = if kind == 0 {
                        crate::water::fluid_cell_at(&terrain, &journal, std::hint::black_box(p), 0)
                    } else {
                        chunks.fluid_cell_at(&terrain, &journal, std::hint::black_box(p), 0)
                    };
                    sums[kind] += u64::from(std::hint::black_box(fluid).amount);
                }
                elapsed[kind] += start.elapsed();
            }
        }
        assert_eq!(sums[0], sums[1]);
        println!(
            "water_need_probe_microbenchmark radius={radius} probes_per_search={} repetitions=16 uncached_ms={:.6} prepared_ms={:.6}",
            positions.len(),
            elapsed[0].as_secs_f64() * 1000.0,
            elapsed[1].as_secs_f64() * 1000.0
        );
    }
}

fn app() -> App {
    app_with_height(8.0)
}
fn app_with_height(height: f64) -> App {
    let mut config = GameConfig::default();
    config.world.landscape_revision = 3;
    config.world.sea_level = 8.0;
    config.client.view_distance = 1;
    config.client.vertical_view_distance = 1;
    config.world.hill_amplitude = 0.0;
    config.world.mountain_amplitude = 0.0;
    config.world.base_height = height;
    config.world.caves = false;
    config.world.trees = false;
    config.world.biomes = false;
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_plugins(TerraCorePlugin(config))
        .add_plugins(TerraWorldPlugin {
            mode: WorldMode::Client,
        });
    app.finish();
    app.cleanup();
    app
}

#[test]
fn live_view_distance_expands_to_32_chunks_and_shrinks_without_actor_movement() {
    let mut app = app_with_height(8.0);
    app.update();
    let center = global_to_section(
        app.world()
            .resource::<ChunkFocus>()
            .0
            .map(|v| v.floor() as i64),
    )
    .0;
    app.world_mut()
        .resource_mut::<GameConfig>()
        .client
        .view_distance = 32;
    app.update();
    let chunks = app.world().resource::<ChunkStore>();
    assert!(chunks.wanted.contains(&center.offset([32, 0, 0]).unwrap()));
    assert!(chunks.wanted.contains(&center.offset([-32, 0, 0]).unwrap()));
    assert!(!chunks.wanted.contains(&center.offset([33, 0, 0]).unwrap()));
    app.world_mut()
        .resource_mut::<GameConfig>()
        .client
        .view_distance = 2;
    app.update();
    let chunks = app.world().resource::<ChunkStore>();
    assert!(chunks.wanted.contains(&center.offset([2, 0, 0]).unwrap()));
    assert!(!chunks.wanted.contains(&center.offset([3, 0, 0]).unwrap()));
    assert!(!chunks.wanted.contains(&center.offset([32, 0, 0]).unwrap()));
}

#[test]
fn shoreline_water_rebuilds_after_bank_edits_and_section_reload() {
    let mut app = app_with_height(4.0);
    wait(&mut app, ready);
    let owner = SectionPos::default();
    let diagonal = SectionPos::new(1, 0, 1);
    let count = |world: &World| {
        world.resource::<ChunkStore>().loaded[&owner]
            .water
            .as_ref()
            .unwrap()
            .indices
            .len()
    };
    let base = count(app.world());
    app.world_mut().write_message(BlockEditRequest {
        position: [15, 8, 15],
        expected_material: 0,
        material: MaterialId::Stone as u16,
    });
    wait(&mut app, |world| ready(world) && count(world) == base - 6);
    let chunks = app.world().resource::<ChunkStore>();
    assert_eq!(chunks.loaded[&diagonal].mesh_epoch, 1);
    assert!(chunks.loaded[&diagonal].water.as_ref().unwrap().depth_shore[0][1] > 0.0);
    app.world_mut().resource_mut::<ChunkFocus>().0 = [1000.0, 6.0, 1000.0];
    wait(&mut app, ready);
    assert!(
        !app.world()
            .resource::<ChunkStore>()
            .loaded
            .contains_key(&owner)
    );
    app.world_mut().resource_mut::<ChunkFocus>().0 = [8.0, 6.0, 24.0];
    wait(&mut app, |world| {
        ready(world) && world.resource::<ChunkStore>().loaded.contains_key(&owner)
    });
    assert_eq!(count(app.world()), base - 6);
    app.world_mut().write_message(BlockEditRequest {
        position: [15, 8, 15],
        expected_material: MaterialId::Stone as u16,
        material: 0,
    });
    wait(&mut app, |world| ready(world) && count(world) == base);
}
fn wait(app: &mut App, predicate: impl Fn(&World) -> bool) {
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        app.update();
        let settings = app.world().resource::<StreamingSettings>();
        assert!(
            app.world().resource::<PendingSections>().0.len()
                <= settings.max_jobs + usize::from(settings.meshes),
            "ordinary workers plus at most one bounded interactive edit slot"
        );
        if predicate(app.world()) {
            break;
        }
        assert!(!matches!(
            app.world().resource::<WorldGeneration>().status,
            GenerationStatus::Failed(_)
        ));
        assert!(Instant::now() < deadline, "streaming timed out");
        std::thread::sleep(Duration::from_millis(1));
    }
}
fn ready(world: &World) -> bool {
    world.resource::<WorldGeneration>().status == GenerationStatus::Ready
}

fn filled_chest_fixture() -> (ChunkStore, EditJournal, [i64; 3], terra_core::Inventory) {
    let position = [-1, 3, -1];
    let (owner, local) = global_to_section(position);
    let mut data = terra_voxel::BlockSection::empty(owner).unwrap();
    data.set_block(local, MaterialId::Chest as u16).unwrap();
    let mut chunks = ChunkStore::default();
    chunks.loaded.insert(
        owner,
        LoadedSection {
            data,
            mesh: None,
            mesh_epoch: 0,
            water_epoch: 0,
            water: None,
            props: Vec::new(),
        },
    );
    let mut journal = EditJournal::default();
    journal.record(position, MaterialId::Chest as u16).unwrap();
    let mut contents = terra_core::Inventory::default();
    contents.insert(terra_core::ItemKind::Stick, 17);
    contents.insert(terra_core::ItemKind::Block(3), 42);
    journal.set_container(position, contents.clone()).unwrap();
    (chunks, journal, position, contents)
}

#[test]
fn internal_block_edits_drop_nonempty_chest_contents_when_removed_or_replaced() {
    for material in [0, MaterialId::Stone as u16] {
        let (mut chunks, mut journal, position, contents) = filled_chest_fixture();
        let request = BlockEditRequest {
            position,
            expected_material: MaterialId::Chest as u16,
            material,
        };
        assert!(
            commit_block_edit(&mut chunks, &mut journal, &request, &Default::default()).unwrap()
        );
        assert_eq!(chunks.block_at(position), Some(material));
        assert!(journal.container_at(position).is_none());
        let drops = &journal.gameplay().drops;
        assert_eq!(drops.len(), contents.occupied_slots());
        for (drop, stack) in drops.iter().zip(&contents.slots) {
            assert_eq!(drop.stack, *stack);
            assert_eq!(drop.position, position.map(|value| value as f64 + 0.5));
            assert!(drop.valid());
        }
        let gameplay = journal.gameplay().clone();
        assert!(
            !commit_block_edit(&mut chunks, &mut journal, &request, &Default::default()).unwrap()
        );
        assert_eq!(journal.gameplay(), &gameplay);
        assert!(
            commit_block_edit(
                &mut chunks,
                &mut journal,
                &BlockEditRequest {
                    position,
                    expected_material: material,
                    material: MaterialId::Chest as u16,
                },
                &Default::default()
            )
            .unwrap()
        );
        assert!(
            journal.container_at(position).is_none(),
            "replacement chest cannot revive old items"
        );
        assert_eq!(journal.gameplay(), &gameplay);
    }
}

#[test]
fn chest_removal_capacity_ids_and_revision_failures_leave_ownership_unchanged() {
    for failure in [
        "capacity",
        "drop_ids",
        "gameplay_revision",
        "block_revision",
        "water_revision",
    ] {
        let (mut chunks, mut journal, position, contents) = filled_chest_fixture();
        let owner = global_to_section(position).0;
        match failure {
            "capacity" => {
                for _ in 0..terra_core::MAX_WORLD_DROPS - contents.occupied_slots() + 1 {
                    assert!(
                        journal
                            .gameplay_mut()
                            .drop_one([0.5, 3.5, 0.5], terra_core::ItemKind::Stick)
                    );
                }
            }
            "drop_ids" => journal.gameplay_mut().next_drop = u64::MAX - 1,
            "gameplay_revision" => journal.gameplay_mut().revision = u64::MAX - 2,
            "block_revision" | "water_revision" => {
                let mut delta = journal.section_delta(owner);
                if failure == "block_revision" {
                    delta.revision = u64::MAX;
                } else {
                    delta.fluid_epoch = u64::MAX;
                }
                journal.import_delta(delta).unwrap();
            }
            _ => unreachable!(),
        }
        let block = chunks.loaded[&owner].data.clone();
        let delta = journal.section_delta(owner);
        let game = journal.gameplay().clone();
        let epochs = [journal.mesh_epoch(owner), journal.water_mesh_epoch(owner)];
        assert!(
            commit_block_edit(
                &mut chunks,
                &mut journal,
                &BlockEditRequest {
                    position,
                    expected_material: MaterialId::Chest as u16,
                    material: 0,
                },
                &Default::default()
            )
            .is_err(),
            "{failure}"
        );
        assert_eq!(chunks.loaded[&owner].data, block, "{failure}");
        assert_eq!(journal.section_delta(owner), delta, "{failure}");
        assert_eq!(journal.gameplay(), &game, "{failure}");
        assert_eq!(journal.container_at(position), Some(&contents), "{failure}");
        assert_eq!(
            [journal.mesh_epoch(owner), journal.water_mesh_epoch(owner)],
            epochs,
            "{failure}"
        );
    }
}

#[test]
fn covering_grass_records_dirt_across_negative_section_boundaries_and_reload() {
    let mut chunks = ChunkStore::default();
    let grass = [-1, 15, -1];
    let cover = [-1, 16, -1];
    for p in [grass, cover] {
        let owner = global_to_section(p).0;
        chunks.loaded.insert(
            owner,
            LoadedSection {
                data: terra_voxel::BlockSection::empty(owner).unwrap(),
                mesh: None,
                mesh_epoch: 0,
                water_epoch: 0,
                water: None,
                props: Vec::new(),
            },
        );
    }
    chunks
        .loaded
        .get_mut(&global_to_section(grass).0)
        .unwrap()
        .data
        .set_block(global_to_section(grass).1, MaterialId::Grass as u16)
        .unwrap();
    let mut journal = EditJournal::default();
    assert!(
        commit_block_edit(
            &mut chunks,
            &mut journal,
            &BlockEditRequest {
                position: cover,
                expected_material: 0,
                material: MaterialId::Stone as u16,
            },
            &Default::default(),
        )
        .unwrap()
    );
    assert_eq!(chunks.block_at(grass), Some(MaterialId::Dirt as u16));
    assert!(
        commit_block_edit(
            &mut chunks,
            &mut journal,
            &BlockEditRequest {
                position: cover,
                expected_material: MaterialId::Stone as u16,
                material: 0,
            },
            &Default::default(),
        )
        .unwrap()
    );
    assert_eq!(chunks.block_at(grass), Some(MaterialId::Dirt as u16));
    let mut replica = EditJournal::default();
    for p in journal.changed_sections() {
        replica.import_delta(journal.section_delta(p)).unwrap();
    }
    assert_eq!(replica.block(grass), Some(MaterialId::Dirt as u16));
    let generator = crate::generation::sections::SectionGenerator::new(
        crate::TerraGenerator::new(Default::default()).unwrap(),
    );
    let owner = global_to_section(grass).0;
    let reloaded = generator
        .build(
            owner,
            replica.revision(owner),
            &replica.snapshot(owner).unwrap(),
            false,
        )
        .unwrap();
    assert_eq!(
        reloaded.data.block(global_to_section(grass).1).unwrap(),
        MaterialId::Dirt as u16
    );
}

#[test]
fn structural_blocks_preserve_ground_but_retire_only_intersecting_props() {
    use crate::props::{PropId, PropKind, WorldProp};
    use terra_core::block_properties::BlockPropertiesCatalog;

    let mut properties =
        BlockPropertiesCatalog::load(&terra_core::asset_directory().join("terra/blocks")).unwrap();
    // The decision is data-driven, not a workbench/chest material-ID special case.
    properties.0.get_mut(&3).unwrap().struct_block = true;
    for material in [28, 29, 3] {
        let ground = [-1, 15, -1];
        let cover = [-1, 16, -1];
        let owner = global_to_section(ground).0;
        let covered = WorldProp {
            id: PropId(17),
            kind: PropKind::Flower,
            position: [-0.5, 16.0, -0.5],
            rotation: 0.0,
            scale: 0.6,
            variant: 0,
            section: owner,
            removed: false,
        };
        let neighbor = WorldProp {
            id: PropId(18),
            position: [-1.5, 16.0, -0.5],
            ..covered
        };
        let mut chunks = ChunkStore::default();
        for p in [ground, cover] {
            let position = global_to_section(p).0;
            chunks.loaded.insert(
                position,
                LoadedSection {
                    data: terra_voxel::BlockSection::empty(position).unwrap(),
                    mesh: None,
                    mesh_epoch: 0,
                    water_epoch: 0,
                    water: None,
                    props: if position == owner {
                        vec![covered, neighbor]
                    } else {
                        Vec::new()
                    },
                },
            );
        }
        chunks
            .loaded
            .get_mut(&owner)
            .unwrap()
            .data
            .set_block(global_to_section(ground).1, MaterialId::Grass as u16)
            .unwrap();
        let mut journal = EditJournal::default();
        journal.record(ground, MaterialId::Grass as u16).unwrap();
        assert!(
            commit_block_edit(
                &mut chunks,
                &mut journal,
                &BlockEditRequest {
                    position: cover,
                    expected_material: 0,
                    material,
                },
                &properties,
            )
            .unwrap()
        );
        assert_eq!(chunks.block_at(ground), Some(MaterialId::Grass as u16));
        assert!(chunks.loaded[&owner].props[0].removed);
        assert!(!chunks.loaded[&owner].props[1].removed);
        let mut replica = EditJournal::default();
        for p in journal.changed_sections() {
            replica.import_delta(journal.section_delta(p)).unwrap();
        }
        assert_eq!(replica.block(ground), Some(MaterialId::Grass as u16));
        assert_eq!(replica.section_delta(owner).removed_props, [covered.id]);
        let mut reloaded_props = vec![covered, neighbor];
        replica.apply_prop_removals(owner, &mut reloaded_props);
        assert!(
            reloaded_props
                .iter()
                .find(|prop| prop.id == covered.id)
                .unwrap()
                .removed
        );
        assert!(
            !reloaded_props
                .iter()
                .find(|prop| prop.id == neighbor.id)
                .unwrap()
                .removed
        );
        assert!(
            commit_block_edit(
                &mut chunks,
                &mut journal,
                &BlockEditRequest {
                    position: cover,
                    expected_material: material,
                    material: 0,
                },
                &properties,
            )
            .unwrap()
        );
        assert_eq!(chunks.block_at(ground), Some(MaterialId::Grass as u16));
    }
}

#[test]
fn fluid_only_jobs_preserve_terrain_and_replay_after_section_eviction() {
    use crate::water::FluidCell;
    let mut app = app_with_height(12.0);
    wait(&mut app, ready);
    app.world_mut()
        .insert_resource(WorldAccess::Replica { ready: true });
    let owner = SectionPos::default();
    let before = &app.world().resource::<ChunkStore>().loaded[&owner];
    let terrain_epoch = before.mesh_epoch;
    let terrain_revision = before.data.revision();
    let terrain_faces = before.mesh.as_ref().unwrap().faces.clone();
    let fluid = FluidCell {
        amount: 7,
        flow: [1, 0],
        source: false,
    };
    app.world_mut()
        .resource_mut::<EditJournal>()
        .set_fluid(
            [5, 12, 5],
            FluidCell {
                amount: 2,
                ..Default::default()
            },
        )
        .unwrap();
    app.update();
    // Replace an in-flight low-water snapshot before it can be published.
    app.world_mut()
        .resource_mut::<EditJournal>()
        .set_fluid([5, 12, 5], fluid)
        .unwrap();
    let rebuilt = |world: &World| {
        world
            .resource::<ChunkStore>()
            .loaded
            .get(&owner)
            .is_some_and(|s| {
                s.water_epoch == world.resource::<EditJournal>().water_mesh_epoch(owner)
                    && s.water
                        .as_ref()
                        .is_some_and(|patch| patch.positions.iter().any(|p| p[1] == 12.875))
            })
    };
    wait(&mut app, rebuilt);
    let section = &app.world().resource::<ChunkStore>().loaded[&owner];
    assert_eq!(section.mesh_epoch, terrain_epoch);
    assert_eq!(section.data.revision(), terrain_revision);
    assert_eq!(section.mesh.as_ref().unwrap().faces, terrain_faces);
    app.world_mut().resource_mut::<ChunkFocus>().0 = [1000.0, 14.0, 1000.0];
    wait(&mut app, ready);
    assert!(
        !app.world()
            .resource::<ChunkStore>()
            .loaded
            .contains_key(&owner)
    );
    app.world_mut().resource_mut::<ChunkFocus>().0 = [8.0, 14.0, 24.0];
    wait(&mut app, rebuilt);
    assert_eq!(
        app.world().resource::<EditJournal>().fluid([5, 12, 5]),
        Some(fluid)
    );
}
#[test]
fn streaming_eviction_replays_edits_and_remeshes_boundary_neighbors() {
    let mut app = app();
    wait(&mut app, ready);
    assert_eq!(app.world().resource::<ChunkStore>().loaded.len(), 27);
    assert_eq!(
        app.world().resource::<WorldGeneration>().prepared_columns,
        9
    );
    let position = [15, 7, 15];
    let original = app
        .world()
        .resource::<ChunkStore>()
        .block_at(position)
        .unwrap();
    let owner = SectionPos::default();
    let faces = app.world().resource::<ChunkStore>().loaded[&owner]
        .mesh
        .as_ref()
        .unwrap()
        .faces
        .len();
    app.world_mut().write_message(BlockEditRequest {
        position,
        expected_material: original,
        material: 0,
    });
    wait(&mut app, |world| {
        ready(world) && world.resource::<ChunkStore>().block_at(position) == Some(0)
    });
    let chunks = app.world().resource::<ChunkStore>();
    assert_eq!(chunks.loaded[&owner].data.revision(), 1);
    assert_eq!(chunks.loaded[&owner].mesh_epoch, 1);
    assert_eq!(chunks.loaded[&SectionPos::new(1, 0, 0)].mesh_epoch, 1);
    assert_eq!(chunks.loaded[&SectionPos::new(0, 0, 1)].mesh_epoch, 1);
    assert!(chunks.loaded[&owner].mesh.as_ref().unwrap().faces.len() > faces);
    assert_eq!(app.world().resource::<EditJournal>().edited_blocks(), 1);
    app.world_mut().resource_mut::<ChunkFocus>().0 = [1000.0, 10.0, 1000.0];
    app.update();
    assert!(
        !app.world()
            .resource::<ChunkStore>()
            .loaded
            .contains_key(&owner)
    );
    wait(&mut app, ready);
    assert!(app.world().resource::<ChunkStore>().columns.len() <= 9);
    app.world_mut().resource_mut::<ChunkFocus>().0 = [8.0, 11.0, 24.0];
    wait(&mut app, |world| {
        ready(world) && world.resource::<ChunkStore>().loaded.contains_key(&owner)
    });
    assert_eq!(
        app.world().resource::<ChunkStore>().block_at(position),
        Some(0)
    );
    assert_eq!(
        app.world().resource::<ChunkStore>().loaded[&owner]
            .data
            .revision(),
        1
    );
}
#[test]
fn consecutive_edits_cannot_be_overwritten_by_older_mesh_jobs() {
    let mut app = app();
    wait(&mut app, ready);
    let position = [15, 7, 15];
    let original = app
        .world()
        .resource::<ChunkStore>()
        .block_at(position)
        .unwrap();
    app.world_mut().write_message(BlockEditRequest {
        position,
        expected_material: original,
        material: 0,
    });
    app.update();
    app.world_mut().write_message(BlockEditRequest {
        position,
        expected_material: 0,
        material: MaterialId::Stone as u16,
    });
    wait(&mut app, |world| {
        ready(world)
            && world.resource::<ChunkStore>().block_at(position) == Some(MaterialId::Stone as u16)
    });
    let section = &app.world().resource::<ChunkStore>().loaded[&SectionPos::default()];
    assert_eq!(section.data.revision(), 2);
    assert_eq!(section.mesh_epoch, 2);
    assert_eq!(
        app.world()
            .resource::<EditJournal>()
            .mesh_epoch(SectionPos::new(1, 0, 0)),
        2
    );
}
#[test]
fn invalid_or_outdated_edits_do_not_mutate_the_world() {
    let mut app = app();
    wait(&mut app, ready);
    let position = [1, 7, 17];
    let material = app
        .world()
        .resource::<ChunkStore>()
        .block_at(position)
        .unwrap();
    app.world_mut().write_message(BlockEditRequest {
        position,
        expected_material: 65535,
        material: 0,
    });
    app.world_mut().write_message(BlockEditRequest {
        position: [1000; 3],
        expected_material: 3,
        material: 0,
    });
    app.update();
    assert_eq!(
        app.world().resource::<ChunkStore>().block_at(position),
        Some(material)
    );
    assert_eq!(app.world().resource::<EditJournal>().edited_blocks(), 0);
}

#[test]
fn elevated_cameras_keep_valley_surfaces_loaded_after_vertical_and_horizontal_moves() {
    let mut app = app();
    // The fixed camera cube alone starts at y=6; the flat surface is in y=0.
    app.world_mut().resource_mut::<ChunkFocus>().0 = [8.0, 120.0, 24.0];
    wait(&mut app, ready);
    assert_eq!(
        app.world().resource::<ChunkStore>().block_at([8, 7, 24]),
        Some(MaterialId::Sand as u16)
    );
    assert!(app.world().resource::<ChunkStore>().loaded.len() > 27);
    app.world_mut().resource_mut::<ChunkFocus>().0 = [8.0, 152.0, 24.0];
    wait(&mut app, ready);
    assert_eq!(
        app.world().resource::<ChunkStore>().block_at([8, 7, 24]),
        Some(MaterialId::Sand as u16)
    );
    app.world_mut().resource_mut::<ChunkFocus>().0 = [24.0, 152.0, 24.0];
    wait(&mut app, ready);
    assert_eq!(
        app.world().resource::<ChunkStore>().block_at([24, 7, 24]),
        Some(MaterialId::Sand as u16)
    );
    let chunks = app.world().resource::<ChunkStore>();
    assert!(
        chunks
            .surface_ranges
            .keys()
            .all(|[x, z]| (0..=2).contains(x) && (0..=2).contains(z))
    );
}

#[test]
fn removing_one_prop_preserves_neighbors_terrain_and_survives_section_reload() {
    let mut app = app_with_height(16.0);
    wait(&mut app, ready);
    let (owner, selected, neighbors) = app
        .world()
        .resource::<ChunkStore>()
        .loaded
        .iter()
        .find_map(|(&owner, section)| {
            (section.props.len() > 1)
                .then(|| (owner, section.props[0], section.props[1..].to_vec()))
        })
        .unwrap();
    let epoch = app.world().resource::<ChunkStore>().loaded[&owner].mesh_epoch;
    app.world_mut().write_message(PropRemoveRequest {
        id: selected.id,
        section: owner,
    });
    app.update();
    let section = &app.world().resource::<ChunkStore>().loaded[&owner];
    assert!(
        section
            .props
            .iter()
            .find(|prop| prop.id == selected.id)
            .unwrap()
            .removed
    );
    for neighbor in &neighbors {
        let current = section
            .props
            .iter()
            .find(|prop| prop.id == neighbor.id)
            .unwrap();
        if crate::props::placement::support(neighbor) == crate::props::placement::support(&selected)
        {
            let mut expected = *neighbor;
            expected.position = current.position;
            assert_eq!(*current, expected);
            assert_eq!(
                crate::props::placement::support(current),
                crate::props::placement::support(neighbor)
            );
        } else {
            assert_eq!(current, neighbor);
        }
    }
    let after_removal = section.props.clone();
    assert_eq!(section.mesh_epoch, epoch);
    assert_eq!(app.world().resource::<EditJournal>().prop_epoch(owner), 1);
    assert!(app.world().resource::<PendingSections>().0.is_empty());
    // A stale duplicate request must not create an additional revision.
    app.world_mut().write_message(PropRemoveRequest {
        id: selected.id,
        section: owner,
    });
    app.update();
    assert_eq!(app.world().resource::<EditJournal>().prop_epoch(owner), 1);
    app.world_mut().resource_mut::<ChunkFocus>().0 = [1000.0, 18.0, 1000.0];
    wait(&mut app, ready);
    assert!(
        !app.world()
            .resource::<ChunkStore>()
            .loaded
            .contains_key(&owner)
    );
    app.world_mut().resource_mut::<ChunkFocus>().0 = [8.0, 18.0, 24.0];
    wait(&mut app, ready);
    let section = &app.world().resource::<ChunkStore>().loaded[&owner];
    assert!(
        section
            .props
            .iter()
            .find(|prop| prop.id == selected.id)
            .unwrap()
            .removed
    );
    assert_eq!(section.props.len(), after_removal.len());
    for expected in &after_removal {
        assert_eq!(
            section.props.iter().find(|prop| prop.id == expected.id),
            Some(expected)
        );
    }
    assert_eq!(section.mesh_epoch, epoch);
}

#[test]
fn planted_groups_reflow_identically_after_replication_and_journal_reload() {
    use crate::props::{PropId, PropKind, WorldProp, placement};

    let support = [-1, 15, -1];
    let owner = global_to_section(support).0;
    let make_prop = |id, cell| {
        let mut prop = WorldProp {
            id: PropId(id),
            kind: PropKind::Flower,
            position: [0.0; 3],
            rotation: 0.0,
            scale: 0.6,
            variant: 1,
            section: owner,
            removed: false,
        };
        placement::align(&mut prop, cell, 0);
        placement::reflow(std::slice::from_mut(&mut prop));
        prop
    };
    let neighbor = make_prop(900, [-2, 15, -1]);
    let make_chunks = || {
        let mut chunks = ChunkStore::default();
        chunks.loaded.insert(
            owner,
            LoadedSection {
                data: terra_voxel::BlockSection::empty(owner).unwrap(),
                mesh: None,
                mesh_epoch: 7,
                water_epoch: 0,
                water: None,
                props: vec![neighbor],
            },
        );
        chunks
    };
    let active = |props: &[WorldProp]| {
        let mut props: Vec<_> = props.iter().copied().filter(|prop| !prop.removed).collect();
        props.sort_unstable_by_key(|prop| prop.id);
        props
    };
    let mut chunks = make_chunks();
    let mut journal = EditJournal::default();
    let mut replica = make_chunks();
    let mut replica_journal = EditJournal::default();
    for (index, id) in [17, 3, 29, 11].into_iter().enumerate() {
        let previous_records = journal.section_delta(owner).placed_props;
        assert!(commit_prop_placement(&mut chunks, &mut journal, make_prop(id, support)).unwrap());
        assert_eq!(journal.prop_epoch(owner), index as u64 + 1);
        let mut delta = journal.section_delta(owner);
        for stored in previous_records {
            assert_eq!(
                delta.placed_props.iter().find(|p| p.id == stored.id),
                Some(&stored)
            );
        }
        // Imported HashMap order and repeated snapshots must not change group anchors.
        delta.placed_props.reverse();
        replica
            .apply_confirmed_delta(&mut replica_journal, delta.clone())
            .unwrap();
        replica
            .apply_confirmed_delta(&mut replica_journal, delta)
            .unwrap();
        let authoritative = active(&chunks.loaded[&owner].props);
        assert_eq!(active(&replica.loaded[&owner].props), authoritative);
        assert_eq!(
            authoritative.iter().find(|p| p.id == neighbor.id),
            Some(&neighbor)
        );
        for current in authoritative.iter().filter(|p| p.id != neighbor.id) {
            assert_eq!(placement::support(current), support);
            let mut stored = *journal
                .section_delta(owner)
                .placed_props
                .iter()
                .find(|p| p.id == current.id)
                .unwrap();
            stored.position = current.position;
            assert_eq!(*current, stored);
        }
        if index == 0 {
            let only = authoritative.iter().find(|p| p.id != neighbor.id).unwrap();
            assert_eq!(only.position, [-0.5, 16.0, -0.5]);
        }
        assert_eq!(chunks.loaded[&owner].mesh_epoch, 7);
        assert!(chunks.dirty.is_empty());
    }
    assert!(!commit_prop_placement(&mut chunks, &mut journal, make_prop(42, support)).unwrap());
    let request = PropRemoveRequest {
        id: PropId(17),
        section: owner,
    };
    assert!(commit_prop_removal(&mut chunks, &mut journal, &request).unwrap());
    assert!(!commit_prop_removal(&mut chunks, &mut journal, &request).unwrap());
    assert_eq!(journal.prop_epoch(owner), 5);
    let expected = active(&chunks.loaded[&owner].props);
    let mut restored = EditJournal::default();
    restored.import_delta(journal.section_delta(owner)).unwrap();
    let mut loaded = vec![neighbor];
    restored.apply_prop_removals(owner, &mut loaded);
    restored.apply_prop_removals(owner, &mut loaded);
    assert_eq!(active(&loaded), expected);
    assert!(restored.prop_removed(owner, request.id));
    assert!(!loaded.iter().any(|p| p.id == request.id));
    replica
        .apply_confirmed_delta(&mut replica_journal, journal.section_delta(owner))
        .unwrap();
    assert_eq!(active(&replica.loaded[&owner].props), expected);
    assert_eq!(chunks.loaded[&owner].mesh_epoch, 7);
}

#[test]
fn covered_plants_do_not_reappear_after_mining_or_reload() {
    let mut app = app_with_height(16.0);
    wait(&mut app, ready);
    let (owner, covered, neighbors) = app
        .world()
        .resource::<ChunkStore>()
        .loaded
        .iter()
        .find_map(|(&owner, section)| {
            section.props.iter().find_map(|candidate| {
                let support = crate::props::placement::support(candidate);
                let (covered, neighbors): (Vec<_>, Vec<_>) = section
                    .props
                    .iter()
                    .copied()
                    .partition(|prop| crate::props::placement::support(prop) == support);
                (covered.len() > 1 && !neighbors.is_empty()).then_some((owner, covered, neighbors))
            })
        })
        .unwrap();
    let above = covered[0].position.map(|v| v.floor() as i64);
    assert_ne!(
        global_to_section(above).0,
        owner,
        "exercise vertical owner boundary"
    );
    app.world_mut().write_message(BlockEditRequest {
        position: above,
        expected_material: 0,
        material: 3,
    });
    wait(&mut app, ready);
    assert_eq!(
        app.world().resource::<EditJournal>().prop_epoch(owner),
        covered.len() as u64
    );
    app.world_mut().write_message(BlockEditRequest {
        position: above,
        expected_material: 3,
        material: 0,
    });
    wait(&mut app, ready);
    let check = |world: &World| {
        let section = &world.resource::<ChunkStore>().loaded[&owner];
        // Dirt support can suppress generation entirely; a tombstone need not
        // have a rendered/generated counterpart after the section is rebuilt.
        let journal = world.resource::<EditJournal>();
        assert_eq!(journal.prop_epoch(owner), covered.len() as u64);
        assert_eq!(
            journal.section_delta(owner).removed_props.len(),
            covered.len()
        );
        for covered in &covered {
            assert!(
                !section
                    .props
                    .iter()
                    .any(|prop| prop.id == covered.id && !prop.removed)
            );
            assert!(journal.prop_removed(owner, covered.id));
        }
        for neighbor in &neighbors {
            assert_eq!(
                section.props.iter().find(|prop| prop.id == neighbor.id),
                Some(neighbor)
            );
        }
        assert_eq!(
            world
                .resource::<ChunkStore>()
                .block_at([above[0], above[1] - 1, above[2]]),
            Some(MaterialId::Dirt as u16)
        );
    };
    check(app.world());
    app.world_mut().resource_mut::<ChunkFocus>().0 = [1000.0, 18.0, 1000.0];
    wait(&mut app, ready);
    assert!(
        !app.world()
            .resource::<ChunkStore>()
            .loaded
            .contains_key(&owner)
    );
    app.world_mut().resource_mut::<ChunkFocus>().0 = [8.0, 18.0, 24.0];
    wait(&mut app, ready);
    check(app.world());
}
