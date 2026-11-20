use super::*;
use crate::streaming::LoadedSection;
use terra_voxel::{BlockSection, SectionPos};
fn fixture() -> (ChunkStore, EditJournal) {
    let mut chunks = ChunkStore::default();
    for z in -1..=1 {
        for x in -1..=1 {
            let p = SectionPos::new(x, 0, z);
            let mut data = BlockSection::empty(p).unwrap();
            for z in 0..16 {
                for x in 0..16 {
                    data.set_block([x, 1, z], 3).unwrap();
                }
            }
            chunks.loaded.insert(
                p,
                LoadedSection {
                    data,
                    mesh: None,
                    mesh_epoch: 0,
                    water_epoch: 0,
                    water: None,
                    props: Vec::new(),
                },
            );
        }
    }
    (chunks, EditJournal::default())
}
#[test]
fn source_spreads_four_cells_without_generating_chunks_and_drains_after_collection() {
    let (mut chunks, mut journal) = fixture();
    let origin = [0, 2, 0];
    assert!(deposit_from_bucket(&mut chunks, &mut journal, origin).unwrap());
    for _ in 0..160 {
        advance(&mut chunks, &mut journal).unwrap();
    }
    let cells: Vec<_> = journal
        .object_states()
        .filter(|(_, id, _)| *id == bile::FLUID)
        .collect();
    assert_eq!(cells.len(), 41);
    for (p, _, s) in &cells {
        assert!(in_range(*p, origin));
        assert!(s.valid_for(*p, bile::FLUID));
    }
    assert!(
        cells
            .iter()
            .any(|(p, _, s)| *p == [4, 2, 0] && s.liquid.unwrap().volume > 0)
    );
    assert_eq!(chunks.loaded.len(), 9);
    assert!(take_source(&mut chunks, &mut journal, origin).unwrap());
    for _ in 0..16 {
        advance(&mut chunks, &mut journal).unwrap();
    }
    assert!(!journal.object_states().any(|(_, id, _)| id == bile::FLUID));
}
#[test]
fn liquid_replica_preserves_source_and_unloaded_origin_freezes_the_flow() {
    let (mut chunks, mut journal) = fixture();
    let origin = [-1, 2, -1];
    deposit_from_bucket(&mut chunks, &mut journal, origin).unwrap();
    advance(&mut chunks, &mut journal).unwrap();
    let mut replica = EditJournal::default();
    for p in journal.changed_sections() {
        let bytes = serde_json::to_vec(&journal.section_delta(p)).unwrap();
        let delta = serde_json::from_slice(&bytes).unwrap();
        replica.import_delta(delta).unwrap();
    }
    assert_eq!(replica.object_state(origin), journal.object_state(origin));
    let section = global_to_section(origin).0;
    chunks.loaded.remove(&section);
    assert_eq!(advance(&mut chunks, &mut journal).unwrap().removed, 0);
    assert!(!deposit_from_bucket(&mut chunks, &mut journal, [100, 2, 100]).unwrap());
}
#[test]
fn acid_contact_follows_thin_surface_instead_of_whole_voxel() {
    let (mut chunks, mut journal) = fixture();
    let p = [0, 2, 0];
    set(
        &mut chunks,
        &mut journal,
        p,
        Some(LiquidState {
            origin: p,
            volume: 1,
            age: 0,
            source: true,
        }),
    )
    .unwrap();
    let body = terra_core::creatures::hitboxes::Hitbox {
        center: [0.5, 2.3, 0.5],
        size: [0.2; 3],
        axes: [[1., 0., 0.], [0., 1., 0.], [0., 0., 1.]],
    };
    assert!(!hitbox_contact(body, &chunks, &journal));
    assert!(hitbox_contact(
        terra_core::creatures::hitboxes::Hitbox {
            center: [0.5, 2.08, 0.5],
            ..body
        },
        &chunks,
        &journal
    ));
    assert_eq!(bile::FLOW_SECONDS, 0.2);
}

#[test]
fn unloaded_persistent_acid_counts_toward_the_global_growth_budget() {
    let (mut chunks, mut journal) = fixture();
    let p = [0, 2, 0];
    deposit_from_bucket(&mut chunks, &mut journal, p).unwrap();
    for i in 1..MAX_CELLS {
        let p = [1_000 + i as i64 * 5, 2, 1_000];
        journal.record(p, bile::FLUID).unwrap();
        journal
            .set_object_state(
                p,
                Some(BlockObjectState {
                    facing: 7,
                    liquid: Some(LiquidState {
                        origin: p,
                        volume: 8,
                        source: true,
                        age: 0,
                    }),
                    ..Default::default()
                }),
            )
            .unwrap();
    }
    let step = advance(&mut chunks, &mut journal).unwrap();
    assert_eq!(step.visited, 1);
    assert_eq!(step.spread, 0);
    assert_eq!(
        journal
            .object_states()
            .filter(|(_, id, _)| *id == bile::FLUID)
            .count(),
        MAX_CELLS
    );
}

#[test]
fn spit_sources_expire_after_five_seconds_even_after_unload_but_collected_buckets_persist() {
    let (mut chunks, mut journal) = fixture();
    let p = [0, 2, 0];
    deposit_from_spit(&mut chunks, &mut journal, p).unwrap();
    for _ in 0..10 {
        advance(&mut chunks, &mut journal).unwrap();
    }
    assert!(liquid(&chunks, &journal, p).unwrap().source);
    assert!(take_source(&mut chunks, &mut journal, p).unwrap());
    deposit_from_bucket(&mut chunks, &mut journal, p).unwrap();
    for _ in 0..30 {
        advance(&mut chunks, &mut journal).unwrap();
    }
    assert!(liquid(&chunks, &journal, p).unwrap().source);
    take_source(&mut chunks, &mut journal, p).unwrap();
    deposit_from_spit(&mut chunks, &mut journal, p).unwrap();
    chunks.loaded.remove(&global_to_section(p).0);
    for _ in 0..25 {
        advance(&mut chunks, &mut journal).unwrap();
    }
    assert_eq!(journal.block(p), Some(0));
    assert!(journal.object_state(p).is_none());
}
#[test]
fn first_acid_front_receives_only_paid_partial_volume_and_stays_valid_on_the_wire() {
    let (mut chunks, mut journal) = fixture();
    let p = [0, 2, 0];
    deposit_from_bucket(&mut chunks, &mut journal, p).unwrap();
    advance(&mut chunks, &mut journal).unwrap();
    let q = [1, 2, 0];
    let state = journal.object_state(q).unwrap();
    let fluid = state.liquid.unwrap();
    assert!(fluid.volume > 0 && fluid.volume < bile::CAPACITY / 2);
    assert!(state.valid_for(q, bile::FLUID));
    let bytes = serde_json::to_vec(&state).unwrap();
    assert_eq!(
        serde_json::from_slice::<BlockObjectState>(&bytes).unwrap(),
        state
    );
}

#[test]
fn children_recede_after_an_unloaded_spit_source_expires_across_a_section_edge() {
    let (mut chunks, mut journal) = fixture();
    let p = [-1, 2, 0];
    deposit_from_spit(&mut chunks, &mut journal, p).unwrap();
    advance(&mut chunks, &mut journal).unwrap();
    let child = [0, 2, 0];
    assert!(liquid(&chunks, &journal, child).is_some());
    chunks.loaded.remove(&global_to_section(p).0);
    for _ in 0..41 {
        advance(&mut chunks, &mut journal).unwrap();
    }
    assert_eq!(journal.block(p), Some(0));
    assert!(liquid(&chunks, &journal, child).is_none());
}
