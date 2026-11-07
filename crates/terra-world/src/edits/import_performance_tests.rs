//! Opt-in dense construction-delta timing plus import/index integrity regressions.
use super::{EditJournal, SectionDelta};
use crate::edits::BlockObjectState;
use terra_core::infusion::{HOLD_BLOCK, HoldVisual};
use terra_voxel::SectionPos;

const STAIR: u16 = 74;

fn fixture(cells: usize) -> SectionDelta {
    let position = SectionPos::new(-2, -1, -2);
    let mut delta = SectionDelta {
        position,
        revision: 1,
        ..Default::default()
    };
    for index in 0..cells {
        let local = [
            (index % 16) as u8,
            ((index / 16) % 16) as u8,
            (index / 256) as u8,
        ];
        let material = if index % 512 == 0 { HOLD_BLOCK } else { STAIR };
        let state = BlockObjectState {
            facing: (index % 4) as u8,
            ..Default::default()
        };
        assert!(state.valid_for(
            position.global_block(local.map(usize::from)).unwrap(),
            material
        ));
        delta.blocks.push((local, material));
        delta.objects.push((local, state));
    }
    delta
}

fn canonical(mut delta: SectionDelta) -> SectionDelta {
    delta.blocks.sort_unstable_by_key(|(local, _)| *local);
    delta.objects.sort_unstable_by_key(|(local, _)| *local);
    delta
}

fn import(journal: &mut EditJournal, delta: SectionDelta) -> Result<Vec<SectionPos>, String> {
    #[cfg(feature = "bevy")]
    {
        journal.import_replica_delta(delta)
    }
    #[cfg(not(feature = "bevy"))]
    {
        journal.import_delta(delta)
    }
}

#[test]
fn dense_import_preserves_final_object_and_hold_indexes_and_rejects_invalid_updates() {
    let initial = fixture(128);
    let owner = initial.position;
    let first = initial.blocks[0].0;
    let replacement = initial.blocks[63].0;
    let removed = initial.blocks[127].0;
    let global = |local: [u8; 3]| owner.global_block(local.map(usize::from)).unwrap();
    let mut journal = EditJournal::default();
    import(&mut journal, initial.clone()).unwrap();
    journal.replace_infusion_views([HoldVisual {
        position: global(first),
        ..Default::default()
    }]);

    let mut next = initial.clone();
    next.revision = 2;
    next.blocks[0].1 = STAIR;
    next.blocks[63].1 = HOLD_BLOCK;
    next.blocks[127].1 = 0;
    next.objects.retain(|(local, _)| *local != removed);
    for (_, state) in next.objects.iter_mut().skip(8).take(16) {
        state.facing = (state.facing + 1) % 4;
    }
    import(&mut journal, next.clone()).unwrap();
    assert_eq!(journal.section_delta(owner), canonical(next.clone()));
    assert_eq!(journal.object_states_in_section(owner).count(), 127);
    assert_eq!(journal.object_sections().collect::<Vec<_>>(), vec![owner]);
    assert_eq!(
        journal.holds_in_section(owner).collect::<Vec<_>>(),
        vec![global(replacement)]
    );
    assert!(journal.infusion_view(global(first)).is_none());
    assert!(journal.infusion_view(global(replacement)).is_none());

    let expected = journal.section_delta(owner);
    let mesh_epoch = journal.mesh_epoch(owner);
    assert!(import(&mut journal, initial).unwrap().is_empty());
    assert_eq!(journal.section_delta(owner), expected);
    assert_eq!(journal.mesh_epoch(owner), mesh_epoch);
    let mut invalid = next.clone();
    invalid.revision = 3;
    invalid.objects[16].1.facing = u8::MAX;
    assert!(import(&mut journal, invalid).is_err());
    assert_eq!(journal.section_delta(owner), expected);
    assert_eq!(journal.mesh_epoch(owner), mesh_epoch);
    assert_eq!(
        journal.holds_in_section(owner).collect::<Vec<_>>(),
        vec![global(replacement)]
    );

    next.revision = 3;
    for (_, material) in &mut next.blocks {
        *material = 0;
    }
    next.objects.clear();
    import(&mut journal, next.clone()).unwrap();
    assert_eq!(journal.section_delta(owner), canonical(next));
    assert!(!journal.has_holds());
    assert_eq!(journal.object_sections().count(), 0);
}

#[test]
fn duplicate_block_validation_keeps_any_match_and_last_stored_value() {
    let owner = SectionPos::new(-2, -1, -2);
    let local = [8, 8, 8];
    let global = owner.global_block(local.map(usize::from)).unwrap();
    let mut journal = EditJournal::default();
    // Existing saves accept a tint if any duplicate material is dyeable, and
    // retain the last block value. Indexed validation must retain that behavior.
    let delta = SectionDelta {
        position: owner,
        revision: 1,
        blocks: vec![(local, 103), (local, STAIR)],
        objects: vec![(local, BlockObjectState::default())],
        tints: vec![(local, [7, 8, 9])],
        ..Default::default()
    };
    import(&mut journal, delta.clone()).unwrap();
    assert_eq!(journal.block(global), Some(STAIR));
    assert_eq!(
        journal.object_state(global),
        Some(BlockObjectState::default())
    );
    assert_eq!(journal.tint_at(global), Some([7, 8, 9]));
    let expected = journal.section_delta(owner);
    let mut invalid = delta;
    invalid.revision = 2;
    invalid.objects.push((local, BlockObjectState::default()));
    assert!(import(&mut journal, invalid).is_err());
    assert_eq!(journal.section_delta(owner), expected);
}

#[test]
fn failed_block_record_keeps_indexes_consistent_with_partial_import() {
    let initial = fixture(1);
    let owner = initial.position;
    let global = owner.global_block([0; 3]).unwrap();
    let unrelated = [global[0] + 32, global[1], global[2]];
    let mut journal = EditJournal::default();
    import(&mut journal, initial.clone()).unwrap();
    journal.record(unrelated, HOLD_BLOCK).unwrap();
    journal
        .set_object_state(unrelated, Some(BlockObjectState::default()))
        .unwrap();
    journal.replace_infusion_views([global, unrelated].map(|position| HoldVisual {
        position,
        ..Default::default()
    }));
    let epoch = journal.mesh_epoch(owner);
    journal.water_mesh_epochs.insert(owner, u64::MAX);
    let mut next = initial;
    next.revision = 2;
    next.blocks[0].1 = 3;
    next.objects.clear();
    assert!(import(&mut journal, next).is_err());
    // Existing import failure semantics retain the block written before a
    // water-epoch overflow. Sparse indexes must describe those actual records.
    assert_eq!(journal.block(global), Some(3));
    assert!(journal.object_state(global).is_none());
    assert_eq!(journal.holds_in_section(owner).count(), 0);
    assert!(!journal.object_sections().any(|section| section == owner));
    assert!(journal.infusion_view(global).is_none());
    assert!(journal.infusion_view(unrelated).is_some());
    assert_eq!(journal.mesh_epoch(owner), epoch + 1);
    assert_eq!(journal.water_mesh_epoch(owner), u64::MAX);
}

#[test]
fn failed_fluid_invalidation_resynchronizes_preceding_object_changes() {
    let owner = SectionPos::new(-2, -1, -2);
    let hold = [8, 8, 8];
    let water = [15, 8, 8];
    let global = owner.global_block(hold.map(usize::from)).unwrap();
    let mut initial = SectionDelta {
        position: owner,
        revision: 1,
        blocks: vec![(hold, HOLD_BLOCK), (water, 0)],
        objects: vec![(hold, BlockObjectState::default())],
        ..Default::default()
    };
    let mut journal = EditJournal::default();
    import(&mut journal, initial.clone()).unwrap();
    journal.replace_infusion_views([HoldVisual {
        position: global,
        ..Default::default()
    }]);
    let epoch = journal.mesh_epoch(owner);
    let neighbor = owner.offset([1, 0, 0]).unwrap();
    journal.water_mesh_epochs.insert(neighbor, u64::MAX);
    initial.revision = 2;
    initial.blocks[0].1 = 3;
    initial.objects.clear();
    initial.fluids.push((
        water,
        crate::water::FluidCell {
            amount: 4,
            ..Default::default()
        },
    ));
    assert!(import(&mut journal, initial).is_err());
    assert_eq!(journal.block(global), Some(3));
    assert!(journal.object_state(global).is_none());
    assert!(!journal.has_holds());
    assert_eq!(journal.object_sections().count(), 0);
    assert!(journal.infusion_view(global).is_none());
    assert!(
        journal
            .fluid(owner.global_block(water.map(usize::from)).unwrap())
            .is_none()
    );
    assert_eq!(journal.mesh_epoch(owner), epoch + 1);
    assert_eq!(journal.water_mesh_epoch(neighbor), u64::MAX);
}

#[test]
fn ordinary_block_records_update_object_indexes_immediately() {
    let position = [-17, -1, -17];
    let mut journal = EditJournal::default();
    journal.record(position, HOLD_BLOCK).unwrap();
    journal
        .set_object_state(position, Some(BlockObjectState::default()))
        .unwrap();
    journal.replace_infusion_views([HoldVisual {
        position,
        ..Default::default()
    }]);
    assert!(journal.has_holds());
    assert_eq!(journal.object_sections().count(), 1);
    journal.record(position, 3).unwrap();
    assert!(!journal.has_holds());
    assert_eq!(journal.object_sections().count(), 0);
    assert!(journal.infusion_view(position).is_none());
}

#[test]
#[ignore = "opt-in O07 microbenchmark; run alone with --ignored --nocapture"]
fn benchmark_dense_section_import_updates() {
    use std::{hint::black_box, time::Instant};

    fn report(name: &str, samples: &[f64]) {
        let mut sorted = samples.to_vec();
        sorted.sort_by(f64::total_cmp);
        let rank = |fraction: f64| {
            sorted[((sorted.len() as f64 * fraction).ceil() as usize).saturating_sub(1)]
        };
        println!(
            "O07_IMPORT_BENCH phase={name} blocks=4096 objects=4096 updates={} total_ms={:.6} mean_ms={:.6} p50_ms={:.6} p95_ms={:.6} p99_ms={:.6} max_ms={:.6}",
            samples.len(),
            samples.iter().sum::<f64>(),
            samples.iter().sum::<f64>() / samples.len() as f64,
            rank(0.5),
            rank(0.95),
            rank(0.99),
            sorted.last().unwrap(),
        );
    }

    let mut delta = fixture(4096);
    let owner = delta.position;
    let mut journal = EditJournal::default();
    import(&mut journal, delta.clone()).unwrap();
    let mut samples = Vec::with_capacity(200);
    for update in 0..200 {
        delta.revision += 1;
        let index = (update * 313 + 17) % delta.objects.len();
        delta.objects[index].1.facing = (delta.objects[index].1.facing + 1) % 4;
        let payload = delta.clone();
        let started = Instant::now();
        black_box(import(&mut journal, payload).unwrap());
        samples.push(started.elapsed().as_secs_f64() * 1000.0);
    }
    report("single-object-edits", &samples);

    samples.clear();
    for _ in 0..4 {
        delta.revision += 1;
        for (_, state) in &mut delta.objects {
            state.facing = (state.facing + 1) % 4;
        }
        let payload = delta.clone();
        let started = Instant::now();
        black_box(import(&mut journal, payload).unwrap());
        samples.push(started.elapsed().as_secs_f64() * 1000.0);
    }
    report("all-object-edits", &samples);
    assert_eq!(journal.section_delta(owner), canonical(delta));
    assert_eq!(journal.object_states_in_section(owner).count(), 4096);
    assert_eq!(journal.holds_in_section(owner).count(), 8);
    assert_eq!(journal.object_sections().collect::<Vec<_>>(), vec![owner]);
}
