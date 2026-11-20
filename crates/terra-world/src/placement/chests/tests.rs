use super::*;
use crate::streaming::{LoadedSection, commit_block_edit, commit_object_state};
use terra_core::{Inventory, ItemKind, WoodSpecies};
use terra_voxel::{BlockSection, SectionPos};

fn fixture() -> (ChunkStore, EditJournal) {
    let mut data = BlockSection::empty(SectionPos::default()).unwrap();
    for x in 0..16 {
        for z in 0..16 {
            data.set_block([x, 1, z], 3).unwrap();
        }
    }
    let mut chunks = ChunkStore::default();
    chunks.loaded.insert(
        SectionPos::default(),
        LoadedSection {
            data,
            mesh: None,
            mesh_epoch: 0,
            water_epoch: 0,
            water: None,
            props: Vec::new(),
        },
    );
    (chunks, EditJournal::default())
}
fn place(
    chunks: &mut ChunkStore,
    journal: &mut EditJournal,
    p: [i64; 3],
    merge: ChestMerge,
    facing: u8,
) {
    let plan = layout_with_chest(
        chunks,
        journal,
        p,
        0,
        29,
        [f32::from(facing) * std::f32::consts::FRAC_PI_2, 0.0],
        None,
        merge,
    )
    .unwrap();
    environment(&plan, chunks, journal, true).unwrap();
    let properties = BlockPropertiesCatalog::default();
    for edit in plan.edits {
        assert!(commit_block_edit(chunks, journal, &edit, &properties).unwrap());
    }
    for (p, state) in plan.states {
        commit_object_state(chunks, journal, p, state).unwrap();
    }
}

#[test]
fn two_doubles_and_a_single_can_touch_without_joining_groups_or_changing_their_real_contents() {
    let (mut chunks, mut journal) = fixture();
    for x in 4..9 {
        place(&mut chunks, &mut journal, [x, 2, 6], ChestMerge::Auto, 0);
        let mut bag = Inventory::default();
        bag.insert(ItemKind::Block(3), x as u16);
        journal.set_container([x, 2, 6], bag).unwrap();
    }
    for (x, peer) in [
        (4, Some([5, 2, 6])),
        (5, Some([4, 2, 6])),
        (6, Some([7, 2, 6])),
        (7, Some([6, 2, 6])),
        (8, None),
    ] {
        let state = journal.object_state([x, 2, 6]).unwrap();
        assert_eq!(state.peer, peer);
        assert_eq!(
            journal
                .container_at([x, 2, 6])
                .unwrap()
                .count(ItemKind::Block(3)),
            x as u16
        );
    }
    // Pair metadata also survives the public terrain replication delta. Private
    // inventories are covered by the real disk-persistence regression in terra-storage.
    let delta = journal.section_delta(SectionPos::default());
    let mut restored = EditJournal::default();
    restored.import_delta(delta).unwrap();
    for x in 4..9 {
        assert_eq!(
            restored.object_state([x, 2, 6]),
            journal.object_state([x, 2, 6])
        );
        assert!(restored.container_at([x, 2, 6]).is_none());
    }
}

#[test]
fn separate_and_ambiguous_placement_stay_single_while_explicit_target_pairs_only_the_chosen_free_side()
 {
    for selected in [None, Some([5, 2, 6]), Some([7, 2, 6])] {
        let (mut chunks, mut journal) = fixture();
        for x in [5, 7] {
            place(
                &mut chunks,
                &mut journal,
                [x, 2, 6],
                ChestMerge::Separate,
                0,
            );
        }
        let merge = selected.map_or(ChestMerge::Auto, ChestMerge::Target);
        place(&mut chunks, &mut journal, [6, 2, 6], merge, 1);
        assert_eq!(journal.object_state([6, 2, 6]).unwrap().peer, selected);
        for x in [5, 7] {
            assert_eq!(
                journal.object_state([x, 2, 6]).unwrap().peer,
                (selected == Some([x, 2, 6])).then_some([6, 2, 6])
            );
        }
    }
    let (mut chunks, mut journal) = fixture();
    place(&mut chunks, &mut journal, [5, 2, 6], ChestMerge::Auto, 0);
    place(
        &mut chunks,
        &mut journal,
        [6, 2, 6],
        ChestMerge::Separate,
        0,
    );
    assert!(journal.object_state([5, 2, 6]).unwrap().peer.is_none());
    assert!(journal.object_state([6, 2, 6]).unwrap().peer.is_none());
}

#[test]
fn targeted_join_inherits_all_existing_facings_preserves_wood_and_never_retargets_an_occupied_double()
 {
    for facing in 0..4 {
        let (mut chunks, mut journal) = fixture();
        let anchor = [7, 2, 7];
        place(
            &mut chunks,
            &mut journal,
            anchor,
            ChestMerge::Separate,
            facing,
        );
        let old = BlockObjectState {
            wood: WoodSpecies::Spruce,
            ..journal.object_state(anchor).unwrap()
        };
        journal.set_object_state(anchor, Some(old)).unwrap();
        let p = offset(anchor, old.right(), 1);
        place(
            &mut chunks,
            &mut journal,
            p,
            ChestMerge::Target(anchor),
            (facing + 1) % 4,
        );
        let a = journal.object_state(anchor).unwrap();
        let b = journal.object_state(p).unwrap();
        assert_eq!((a.facing, b.facing), (facing, facing));
        assert_eq!(a.wood, WoodSpecies::Spruce);
        assert_eq!(b.wood, WoodSpecies::Oak);
        let next = offset(p, old.right(), 1);
        place(
            &mut chunks,
            &mut journal,
            next,
            ChestMerge::Target(p),
            facing,
        );
        assert!(journal.object_state(next).unwrap().peer.is_none());
        assert_eq!(journal.object_state(anchor), Some(a));
        assert_eq!(journal.object_state(p), Some(b));
    }
}

#[test]
fn malicious_or_unloaded_targets_cannot_select_nonadjacent_chests_or_overflow_coordinates() {
    let (chunks, journal) = fixture();
    for target in [[i64::MIN, 2, i64::MAX], [15, 2, 6], [6, 3, 6], [5, 2, 6]] {
        assert!(
            layout_with_chest(
                &chunks,
                &journal,
                [6, 2, 6],
                0,
                29,
                [0.0; 2],
                None,
                ChestMerge::Target(target)
            )
            .is_err()
        );
    }
}
