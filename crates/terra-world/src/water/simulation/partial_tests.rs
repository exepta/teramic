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
    if super::super::space::partial(id) {
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
fn partial_water_flows_across_negative_and_positive_section_seams_and_drains() {
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
        for _ in 0..30 {
            simulate_fluid_step(&terrain, &chunks, &mut journal, 4096).unwrap();
        }
        assert_eq!(journal.fluid(q).unwrap().amount, 7);
        assert_eq!(journal.fluid(r).unwrap().amount, 6);
        let wet = chunks
            .water_column_at_point(&terrain, &journal, [q[0] as f64 + 0.5, 8.6, 5.5])
            .unwrap();
        assert_eq!(wet.bed, 8.5);
        assert_eq!(wet.level, 8.875);
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
        assert!(journal.fluid(q).is_none());
        assert!(journal.fluid(r).is_none());
    }
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
fn replacing_source_with_slab_keeps_no_source_and_remaining_water_recedes() {
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
    assert!(!journal.fluid(p).unwrap().source);
    assert_eq!(chunks.fluid_cell_at(&terrain, &journal, p, 82).amount, 8);
    for _ in 0..40 {
        simulate_fluid_step(&terrain, &chunks, &mut journal, 4096).unwrap();
    }
    assert!(journal.fluid(p).is_none());
    assert!(journal.fluid_cells().all(|(_, cell)| !cell.source));
}
