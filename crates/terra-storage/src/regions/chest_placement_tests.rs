use super::*;
use terra_core::{ItemKind, WoodSpecies, containers::ChestMerge};
use terra_voxel::{BlockSection, global_to_section};
use terra_world::streaming::{ChunkStore, LoadedSection, commit_block_edit, commit_object_state};

fn place(chunks: &mut ChunkStore, journal: &mut EditJournal, p: [i64; 3], merge: ChestMerge) {
    let plan =
        terra_world::placement::layout_with_chest(chunks, journal, p, 0, 29, [0.; 2], None, merge)
            .unwrap();
    terra_world::placement::environment(&plan, chunks, journal, true).unwrap();
    for edit in &plan.edits {
        assert!(commit_block_edit(chunks, journal, edit, &Default::default()).unwrap());
    }
    for (position, state) in plan.states {
        commit_object_state(chunks, journal, position, state).unwrap();
    }
}

#[test]
fn adjacent_double_chests_keep_separate_exact_inventories_pairs_claims_and_revisions_after_real_disk_reload()
 {
    let directory =
        std::env::temp_dir().join(format!("terra-chest-pairs-{}", PlayerUuid::new_v4()));
    let world = WorldId(24029);
    let mut chunks = ChunkStore::default();
    for x in [-1, 0] {
        let section = terra_voxel::SectionPos { x, y: 0, z: 0 };
        let mut data = BlockSection::empty(section).unwrap();
        for x in 0..16 {
            for z in 0..16 {
                data.set_block([x, 1, z], 3).unwrap();
            }
        }
        chunks.loaded.insert(
            section,
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
    let mut journal = EditJournal::default();
    for x in -1..=2 {
        let p = [x, 2, 8];
        place(&mut chunks, &mut journal, p, ChestMerge::Auto);
        let mut state = journal.object_state(p).unwrap();
        state.wood = if x % 2 == 0 {
            WoodSpecies::Birch
        } else {
            WoodSpecies::Spruce
        };
        journal.set_object_state(p, Some(state)).unwrap();
        let mut bag = Inventory::default();
        bag.insert(ItemKind::Block(3), (x + 17) as u16);
        bag.insert(ItemKind::Stick, (x + 4) as u16);
        journal.set_container(p, bag).unwrap();
    }
    // A persistent active realm must own its actual placed Bergfried.
    journal
        .record([1, 2, 12], terra_core::castle::BERGFRIED)
        .unwrap();
    let sovereign = PlayerUuid::new_v4();
    journal
        .gameplay_mut()
        .kingdoms
        .found(sovereign, [1, 2, 12], "Chest town")
        .unwrap();
    save_edits(&directory, world, &journal).unwrap();
    let mut loaded = load_edits(&directory, world).unwrap();
    for (x, peer) in [(-1, 0), (0, -1), (1, 2), (2, 1)] {
        let p = [x, 2, 8];
        assert_eq!(loaded.object_state(p), journal.object_state(p));
        assert_eq!(loaded.object_state(p).unwrap().peer, Some([peer, 2, 8]));
        assert_eq!(loaded.container_at(p), journal.container_at(p));
        let section = global_to_section(p).0;
        assert_eq!(
            loaded.section_delta(section),
            journal.section_delta(section)
        );
    }
    assert_eq!(loaded.gameplay().kingdoms, journal.gameplay().kingdoms);
    // A later pair can grow beside the persisted doubles without coalescing them.
    place(&mut chunks, &mut loaded, [3, 2, 8], ChestMerge::Separate);
    place(&mut chunks, &mut loaded, [4, 2, 8], ChestMerge::Auto);
    assert_eq!(
        loaded.object_state([3, 2, 8]).unwrap().peer,
        Some([4, 2, 8])
    );
    assert_eq!(
        loaded.object_state([2, 2, 8]),
        journal.object_state([2, 2, 8])
    );
    save_edits(&directory, world, &loaded).unwrap();
    let twice = load_edits(&directory, world).unwrap();
    for x in -1..=2 {
        assert_eq!(
            twice.container_at([x, 2, 8]),
            journal.container_at([x, 2, 8])
        );
    }
    assert_eq!(twice.object_state([4, 2, 8]).unwrap().peer, Some([3, 2, 8]));
    fs::remove_dir_all(directory).unwrap();
}
