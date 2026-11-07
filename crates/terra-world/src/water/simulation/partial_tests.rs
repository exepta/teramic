use super::*;
use crate::{edits::BlockObjectState, streaming::LoadedSection};
use terra_core::WorldConfig;
use terra_voxel::{BlockSection, SectionPos};

fn fixture(origin: [i64; 3]) -> (TerraGenerator, ChunkStore, EditJournal) {
    let terrain = TerraGenerator::new(WorldConfig {
        sea_level: -100.0,
        rivers: false,
        ..Default::default()
    })
    .unwrap();
    let mut chunks = ChunkStore::default();
    let owner = global_to_section(origin).0;
    for dx in 0..=1 {
        let p = owner.offset([dx, 0, 0]).unwrap();
        chunks.loaded.insert(
            p,
            LoadedSection {
                data: BlockSection::from_blocks(p, 0, vec![3; 4096]).unwrap(),
                mesh: None,
                mesh_epoch: 0,
                water_epoch: 0,
                water: None,
                props: vec![],
            },
        );
    }
    (terrain, chunks, EditJournal::default())
}

fn put(chunks: &mut ChunkStore, journal: &mut EditJournal, p: [i64; 3], id: u16, facing: u8) {
    journal.record(p, id).unwrap();
    if super::super::space::partial(id)
        && (BlockObjectState {
            facing,
            ..Default::default()
        })
        .valid_for(p, id)
    {
        journal
            .set_object_state(
                p,
                Some(BlockObjectState {
                    facing,
                    ..Default::default()
                }),
            )
            .unwrap();
    }
    let (owner, local) = global_to_section(p);
    chunks
        .loaded
        .get_mut(&owner)
        .unwrap()
        .data
        .set_block(local, id)
        .unwrap();
}

#[test]
fn partial_water_wets_banks_across_section_seams_without_escaping_and_drains() {
    for x in [-1, 15] {
        let p = [x, 8, 5];
        let q = [x + 1, 8, 5];
        let r = [x + 2, 8, 5];
        let (terrain, mut chunks, mut journal) = fixture(p);
        put(
            &mut chunks,
            &mut journal,
            p,
            terra_voxel::MaterialId::FluidWater as u16,
            0,
        );
        put(&mut chunks, &mut journal, q, 82, 0);
        put(&mut chunks, &mut journal, r, 0, 0);
        // The lower slab seals a closed pocket below half height. This stream
        // regression needs a real outlet to prove recession after its source
        // stops; the separately tested sealed pocket must keep its finite water.
        put(&mut chunks, &mut journal, [r[0], r[1] - 1, r[2]], 0, 0);
        for _ in 0..30 {
            simulate_fluid_step(&terrain, &chunks, &mut journal, 4096).unwrap();
        }
        assert_eq!(journal.fluid(q).unwrap().amount, 8);
        assert!(!journal.fluid(q).unwrap().source);
        assert!(journal.fluid(r).is_none_or(|c| c.amount == 0));
        assert!(journal.fluid(r).is_none_or(|c| !c.source));
        let wet = chunks
            .water_column_at_point(&terrain, &journal, [q[0] as f64 + 0.5, 8.6, 5.5])
            .unwrap();
        assert_eq!(wet.bed, 8.5);
        assert_eq!(wet.level, 9.0);
        assert!(
            chunks
                .water_column_at_point(&terrain, &journal, [q[0] as f64 + 0.5, 8.25, 5.5])
                .is_none()
        );
        assert_eq!(
            super::super::fluid_motion_at(
                &terrain,
                &chunks,
                &journal,
                [q[0] as f64 + 0.5, 8.25, 5.5]
            )
            .immersion,
            0.0
        );
        let mut replica = EditJournal::default();
        for section in journal.changed_sections() {
            replica
                .import_delta(journal.section_delta(section))
                .unwrap();
        }
        assert_eq!(replica.fluid(q), journal.fluid(q));
        assert_eq!(
            chunks.water_column_at_point(&terrain, &replica, [q[0] as f64 + 0.5, 8.6, 5.5]),
            Some(wet)
        );
        put(&mut chunks, &mut journal, p, 3, 0);
        for _ in 0..80 {
            simulate_fluid_step(&terrain, &chunks, &mut journal, 4096).unwrap();
        }
        assert!(journal.fluid(q).is_none_or(|c| c.amount == 0 && !c.source));
        assert!(journal.fluid(r).is_none_or(|c| c.amount == 0 && !c.source));
    }
}

#[test]
fn isolated_half_height_pocket_retains_water_after_adjacent_supply_is_cut() {
    use std::collections::BTreeMap;
    for x in [-1, 15] {
        let p = [x, 8, 5];
        let q = [x + 1, 8, 5];
        let r = [x + 2, 8, 5];
        let (terrain, mut chunks, mut journal) = fixture(p);
        put(&mut chunks, &mut journal, p, 27, 0);
        put(&mut chunks, &mut journal, q, 82, 0);
        put(&mut chunks, &mut journal, r, 0, 0);
        journal
            .set_fluid(
                r,
                FluidCell {
                    amount: 4,
                    ..Default::default()
                },
            )
            .unwrap();
        for _ in 0..30 {
            simulate_fluid_step(&terrain, &chunks, &mut journal, 4096).unwrap();
        }
        assert_eq!(journal.fluid(q).unwrap().amount, 8);
        assert_eq!(journal.fluid(r).unwrap().amount, 4);
        put(&mut chunks, &mut journal, p, 3, 0);
        let total = |journal: &EditJournal| {
            journal
                .fluid_cells()
                .map(|(_, cell)| u64::from(cell.amount))
                .sum::<u64>()
        };
        let initial = total(&journal);
        let mut added = 0;
        let mut removed = 0;
        for _ in 0..80 {
            let step = simulate_fluid_step(&terrain, &chunks, &mut journal, 4096).unwrap();
            assert_eq!(step.sourced, 0);
            added += step.replenished;
            removed += step.drained + step.absorbed;
            assert_eq!(total(&journal) + removed, initial + added);
        }
        assert_eq!(journal.fluid(q).unwrap().amount, 0);
        assert_eq!(
            journal.fluid(r).unwrap(),
            FluidCell {
                amount: 4,
                ..Default::default()
            }
        );
        assert_eq!(total(&journal), 4);
        let column = chunks
            .water_column_at_point(&terrain, &journal, [r[0] as f64 + 0.5, 8.25, 5.5])
            .unwrap();
        assert_eq!(column.level, 8.5);
        assert_eq!(column.bed, 8.0);
        let expected: BTreeMap<_, _> = journal.fluid_cells().collect();
        let mut restored = EditJournal::default();
        for owner in journal.changed_sections() {
            restored.import_delta(journal.section_delta(owner)).unwrap();
        }
        restored.wake_fluid_neighbors(r);
        for _ in 0..80 {
            let step = simulate_fluid_step(&terrain, &chunks, &mut restored, 4096).unwrap();
            assert_eq!(
                step.sourced + step.replenished + step.drained + step.absorbed,
                0
            );
        }
        assert_eq!(restored.fluid_cells().collect::<BTreeMap<_, _>>(), expected);
        assert_eq!(total(&restored), 4);
    }
}

#[test]
fn terrain_path_keeps_its_lateral_gradient_but_actual_falling_water_fills_its_free_skin() {
    let p = [7, 8, 5];
    let q = [8, 8, 5];
    let (terrain, mut chunks, mut journal) = fixture(p);
    put(&mut chunks, &mut journal, p, 27, 0);
    put(
        &mut chunks,
        &mut journal,
        q,
        terra_voxel::MaterialId::Path as u16,
        0,
    );
    for _ in 0..30 {
        simulate_fluid_step(&terrain, &chunks, &mut journal, 4096).unwrap();
    }
    assert_eq!(chunks.fluid_cell_at(&terrain, &journal, q, 102).amount, 0);
    put(&mut chunks, &mut journal, [q[0], q[1] + 1, q[2]], 27, 0);
    for _ in 0..30 {
        simulate_fluid_step(&terrain, &chunks, &mut journal, 4096).unwrap();
    }
    let water = chunks.fluid_cell_at(&terrain, &journal, q, 102);
    assert_eq!(water.amount, 8);
    assert!(!water.source);
    let space = super::super::space::FluidSpace::at(&journal, q, |cell| chunks.block_at(cell));
    assert!(space.contains(water, [0.5, 0.97, 0.5]));
    assert!(!space.contains(water, [0.5, 0.90, 0.5]));
}

#[test]
fn inverted_and_vertical_slabs_do_not_transmit_through_solid_faces() {
    for facing in [4, 5, 6, 7, 8, 9] {
        let p = [7, 8, 5];
        let q = [8, 8, 5];
        let (terrain, mut chunks, mut journal) = fixture(p);
        put(
            &mut chunks,
            &mut journal,
            p,
            terra_voxel::MaterialId::FluidWater as u16,
            0,
        );
        put(&mut chunks, &mut journal, q, 82, facing);
        for _ in 0..40 {
            simulate_fluid_step(&terrain, &chunks, &mut journal, 4096).unwrap();
        }
        let cell = chunks.fluid_cell_at(&terrain, &journal, q, 82);
        if matches!(facing, 5 | 9) {
            assert_eq!(cell.amount, 0, "facing {facing}");
        }
        if facing == 4 {
            assert_eq!(cell.amount, 4);
        }
        let space = super::super::space::FluidSpace::at(&journal, q, |r| chunks.block_at(r));
        for point in [[0.25, 0.25, 0.25], [0.75, 0.75, 0.75]] {
            let global = std::array::from_fn(|i| point[i] + q[i] as f64);
            if !space.contains(cell, point) {
                assert_eq!(
                    super::super::fluid_motion_at(&terrain, &chunks, &journal, global).immersion,
                    0.0
                );
            }
        }
    }
}

#[test]
fn unload_discards_work_owners_not_persistent_water_or_loaded_waiters() {
    let mut journal = EditJournal::default();
    let loaded = SectionPos::new(0, 0, 0);
    let distant = SectionPos::new(30, 0, 0);
    let local_cell = [15, 8, 5];
    let remote_cell = [480, 8, 5];
    journal
        .set_fluid(
            remote_cell,
            FluidCell {
                amount: 8,
                source: true,
                ..Default::default()
            },
        )
        .unwrap();
    journal.sleep_fluid(local_cell, SectionPos::new(1, 0, 0));
    journal.sleep_fluid(remote_cell, SectionPos::new(31, 0, 0));
    journal.suspend_unloaded_fluids(|p| p == loaded);
    let counts = journal.fluid_work_counts();
    assert_eq!(counts.active, 0);
    assert_eq!(counts.sleeping, 1);
    assert_eq!(counts.recorded_cells, 1);
    journal.resume_fluids(distant);
    assert!(journal.take_fluid_work(256).contains(&remote_cell));
    assert!(journal.fluid(remote_cell).unwrap().source);
}

#[test]
fn replacing_source_with_slab_waterlogs_it_until_a_full_block_removes_the_source() {
    let p = [7, 8, 5];
    let (terrain, mut chunks, mut journal) = fixture(p);
    put(
        &mut chunks,
        &mut journal,
        p,
        terra_voxel::MaterialId::FluidWater as u16,
        0,
    );
    assert!(journal.fluid(p).unwrap().source);
    put(&mut chunks, &mut journal, p, 82, 0);
    assert!(journal.fluid(p).unwrap().source);
    assert_eq!(chunks.fluid_cell_at(&terrain, &journal, p, 82).amount, 8);
    for _ in 0..40 {
        simulate_fluid_step(&terrain, &chunks, &mut journal, 4096).unwrap();
    }
    assert!(journal.fluid(p).unwrap().source);
    put(&mut chunks, &mut journal, p, 3, 0);
    for _ in 0..40 {
        simulate_fluid_step(&terrain, &chunks, &mut journal, 4096).unwrap();
    }
    assert!(journal.fluid(p).is_none_or(|c| c.amount == 0 && !c.source));
    assert!(journal.fluid_cells().all(|(_, cell)| !cell.source));
}

#[test]
fn water_flows_around_pipes_across_chunk_seams_and_recedes_when_source_is_removed() {
    use terra_core::plumbing;
    for x in [-1, 15] {
        for material in plumbing::WOOD_PIPE..=plumbing::PUMP {
            let p = [x, 8, 5];
            let q = [x + 1, 8, 5];
            let r = [x + 2, 8, 5];
            let (terrain, mut chunks, mut journal) = fixture(p);
            put(
                &mut chunks,
                &mut journal,
                p,
                terra_voxel::MaterialId::FluidWater as u16,
                0,
            );
            put(&mut chunks, &mut journal, q, material, 0);
            put(&mut chunks, &mut journal, r, 0, 0);
            for _ in 0..30 {
                simulate_fluid_step(&terrain, &chunks, &mut journal, 4096).unwrap();
            }
            let external = chunks.fluid_cell_at(&terrain, &journal, q, material);
            assert!(
                external.amount > 0,
                "water must pass around material {material}"
            );
            assert!(!external.source);
            assert!(journal.fluid(r).is_some_and(|cell| cell.amount > 0));
            assert!(
                journal.plumbing_at(q).is_none(),
                "external water must not fill the pipe's internal storage"
            );
            let space =
                super::super::space::FluidSpace::at(&journal, q, |cell| chunks.block_at(cell));
            assert!(space.contains(external, [0.03, 0.5, 0.03]));
            assert!(!space.contains(external, [0.5; 3]));
            put(&mut chunks, &mut journal, p, 3, 0);
            for _ in 0..80 {
                simulate_fluid_step(&terrain, &chunks, &mut journal, 4096).unwrap();
            }
            assert!(journal.fluid(q).is_none_or(|cell| cell.amount == 0));
            assert!(journal.fluid(r).is_none_or(|cell| cell.amount == 0));
        }
    }
}

fn server_pool_fixture(terrain: TerraGenerator) -> (ChunkStore, EditJournal, TerraGenerator) {
    let mut chunks = ChunkStore::default();
    for x in -1..=0 {
        for z in -1..=0 {
            let position = SectionPos::new(x, 1, z);
            chunks.loaded.insert(
                position,
                LoadedSection {
                    data: BlockSection::from_blocks(position, 0, vec![3; 4096]).unwrap(),
                    mesh: None,
                    mesh_epoch: 0,
                    water_epoch: 0,
                    water: None,
                    props: vec![],
                },
            );
        }
    }
    (chunks, EditJournal::default(), terrain)
}

fn real_edit(chunks: &mut ChunkStore, journal: &mut EditJournal, p: [i64; 3], material: u16) {
    let expected_material = chunks.block_at(p).unwrap();
    assert!(
        crate::streaming::commit_block_edit(
            chunks,
            journal,
            &crate::streaming::BlockEditRequest {
                position: p,
                expected_material,
                material
            },
            &Default::default()
        )
        .unwrap()
    );
}

fn server_water_ticks(chunks: ChunkStore, journal: EditJournal, terrain: TerraGenerator) -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .insert_resource(WorldAccess::Server)
        .insert_resource(WorldTerrain(terrain))
        .insert_resource(chunks)
        .insert_resource(journal)
        .insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
            std::time::Duration::from_millis(110),
        ))
        .add_systems(Update, tick);
    // Every positive frame exceeds the actual 100 ms authority interval.
    for _ in 0..35 {
        app.update();
    }
    assert!(app.world().resource::<Time>().elapsed_secs_f64() > 3.5);
    app
}

#[test]
fn server_scheduler_preserves_waterlogged_inlet_in_a_large_placed_pool_above_natural_water() {
    let terrain = TerraGenerator::new(WorldConfig {
        base_height: 16.,
        sea_level: -100.,
        hill_amplitude: 0.,
        mountain_amplitude: 0.,
        biomes: false,
        caves: false,
        rivers: false,
        trees: false,
        ..Default::default()
    })
    .unwrap();
    let (mut chunks, mut journal, terrain) = server_pool_fixture(terrain);
    let inlet = [-6, 18, -1];
    let mut sources = vec![];
    for x in -10..=-6 {
        for z in -3..=1 {
            for y in 17..=18 {
                let p = [x, y, z];
                real_edit(&mut chunks, &mut journal, p, 27);
                if p != inlet {
                    sources.push(p);
                }
            }
            for y in 19..=20 {
                real_edit(&mut chunks, &mut journal, [x, y, z], 0);
            }
        }
    }
    real_edit(
        &mut chunks,
        &mut journal,
        inlet,
        terra_core::plumbing::IRON_PIPE,
    );
    journal
        .set_object_state(
            inlet,
            Some(BlockObjectState {
                facing: 4,
                ..Default::default()
            }),
        )
        .unwrap();
    let above = [inlet[0], inlet[1] + 1, inlet[2]];
    real_edit(
        &mut chunks,
        &mut journal,
        above,
        terra_core::plumbing::IRON_PIPE,
    );
    journal
        .set_object_state(
            above,
            Some(BlockObjectState {
                facing: 4,
                ..Default::default()
            }),
        )
        .unwrap();
    assert_eq!(sources.len(), 49);
    assert_eq!(
        journal.fluid(inlet),
        Some(FluidCell {
            amount: 8,
            source: true,
            ..Default::default()
        })
    );
    assert_eq!(
        super::super::fluid::from_columns(terrain.water_columns(-5.5, -0.5), 18).amount,
        0
    );
    let app = server_water_ticks(chunks, journal, terrain);
    let journal = app.world().resource::<EditJournal>();
    assert_eq!(
        journal.fluid(inlet),
        Some(FluidCell {
            amount: 8,
            source: true,
            ..Default::default()
        }),
        "pipe exterior must stay flush with the actual placed source body"
    );
    assert_eq!(
        sources
            .into_iter()
            .map(|p| {
                let cell = journal.fluid(p).unwrap();
                assert!(cell.source);
                u16::from(cell.amount)
            })
            .sum::<u16>(),
        392
    );
}

#[test]
fn server_scheduler_uses_the_highest_actual_source_instead_of_clamping_placed_water_to_natural_level()
 {
    let terrain = TerraGenerator::new(WorldConfig {
        base_height: 3.3,
        sea_level: 7.3,
        hill_amplitude: 0.,
        mountain_amplitude: 0.,
        biomes: false,
        caves: false,
        rivers: false,
        trees: false,
        landscape_revision: 0,
        structures_revision: 0,
        ..Default::default()
    })
    .unwrap();
    let inlet = [-8, 7, -8];
    let mut chunks = ChunkStore::default();
    for x in -1..=0 {
        for z in -1..=0 {
            let position = SectionPos::new(x, 0, z);
            chunks.loaded.insert(
                position,
                LoadedSection {
                    data: BlockSection::from_blocks(position, 0, vec![3; 4096]).unwrap(),
                    mesh: None,
                    mesh_epoch: 0,
                    water_epoch: 0,
                    water: None,
                    props: vec![],
                },
            );
        }
    }
    let mut journal = EditJournal::default();
    // -X is sampled first and supplies the implicit fractional level (3).
    // The later +X explicit source has a genuinely higher full-cell level (8).
    let lower = [inlet[0] - 1, inlet[1], inlet[2]];
    let higher = [inlet[0] + 1, inlet[1], inlet[2]];
    real_edit(&mut chunks, &mut journal, lower, 0);
    real_edit(&mut chunks, &mut journal, higher, 27);
    real_edit(&mut chunks, &mut journal, inlet, 27);
    real_edit(
        &mut chunks,
        &mut journal,
        inlet,
        terra_core::plumbing::IRON_PIPE,
    );
    journal
        .set_object_state(
            inlet,
            Some(BlockObjectState {
                facing: 4,
                ..Default::default()
            }),
        )
        .unwrap();
    // Exercise ambient supply on a finite exterior, rather than trivially
    // preserving the source flag retained by ordinary waterlogged placement.
    journal
        .set_fluid(
            inlet,
            FluidCell {
                amount: 8,
                source: false,
                ..Default::default()
            },
        )
        .unwrap();
    assert!(!journal.fluid(inlet).unwrap().source);
    assert_eq!(chunks.fluid_cell_at(&terrain, &journal, lower, 0).amount, 3);
    assert_eq!(journal.fluid(higher).unwrap().amount, 8);
    let app = server_water_ticks(chunks, journal, terrain);
    let journal = app.world().resource::<EditJournal>();
    assert_eq!(
        journal.fluid(inlet),
        Some(FluidCell {
            amount: 8,
            ..Default::default()
        })
    );
    assert_eq!(
        journal.fluid(higher),
        Some(FluidCell {
            amount: 8,
            source: true,
            ..Default::default()
        })
    );
}
