use super::{BlockObjectState, EditJournal};
use terra_core::{Inventory, ItemKind, ItemStack, WoodSpecies, containers::CHEST_SLOTS};

const FIRST: [i64; 3] = [-17, 15, -1];
const SECOND: [i64; 3] = [-16, 15, -1];

fn chest_pair(journal: &mut EditJournal) {
    for (position, peer, secondary) in [(FIRST, SECOND, false), (SECOND, FIRST, true)] {
        journal.record(position, 29).unwrap();
        journal
            .set_object_state(
                position,
                Some(BlockObjectState {
                    peer: Some(peer),
                    secondary,
                    wood: WoodSpecies::Birch,
                    ..Default::default()
                }),
            )
            .unwrap();
    }
}

fn filled_with_space(item: ItemKind, count: u16, revision: u64) -> Inventory {
    let mut slots = vec![
        ItemStack {
            item: ItemKind::Block(3),
            count: 128
        };
        CHEST_SLOTS
    ];
    slots[CHEST_SLOTS - 1] = ItemStack { item, count };
    Inventory {
        slots,
        revision,
        ..Default::default()
    }
}

fn drop_stack(journal: &mut EditJournal, item: ItemKind, count: u16) -> u64 {
    assert!(journal.gameplay_mut().drop_stack(
        [-16.5, 14.75, -0.5],
        ItemStack { item, count },
        [0.0, 0.0, 0.25],
        0.0,
    ));
    journal.gameplay().next_drop
}

#[test]
fn partner_revision_overflow_rolls_back_the_first_chest_and_the_entire_drop() {
    let mut journal = EditJournal::default();
    chest_pair(&mut journal);
    let item = ItemKind::for_wood(149, WoodSpecies::Cherry);
    let first = filled_with_space(item, 127, 19);
    let second = filled_with_space(item, 127, u64::MAX);
    journal.set_container(FIRST, first.clone()).unwrap();
    journal.set_container(SECOND, second.clone()).unwrap();
    let id = drop_stack(&mut journal, item, 4);
    let gameplay = journal.gameplay().clone();
    assert!(journal.import_drop_to_chest(FIRST, id).is_err());
    assert_eq!(journal.container_at(FIRST), Some(&first));
    assert_eq!(journal.container_at(SECOND), Some(&second));
    assert_eq!(journal.gameplay(), &gameplay);
    assert!(journal.gameplay().valid());
}

#[test]
fn partial_import_preserves_the_same_drop_id_and_metadata_at_physical_chest_capacity() {
    for paired in [false, true] {
        let mut journal = EditJournal::default();
        if paired {
            chest_pair(&mut journal);
            journal
                .set_container(FIRST, filled_with_space(ItemKind::Block(3), 128, 11))
                .unwrap();
        } else {
            journal.record(SECOND, 29).unwrap();
        }
        let item = ItemKind::for_wood(144, WoodSpecies::Spruce);
        journal
            .set_container(SECOND, filled_with_space(item, 126, 41))
            .unwrap();
        let id = drop_stack(&mut journal, item, 5);
        let mut expected_drop = journal.gameplay().drops[0].clone();
        expected_drop.stack.count = 3;
        let revision = journal.gameplay().revision;
        let first = journal.container_at(FIRST).cloned();
        assert!(
            journal
                .import_drop_to_chest(if paired { FIRST } else { SECOND }, id)
                .unwrap()
        );
        let second = journal.container_at(SECOND).unwrap();
        assert_eq!(second.slots.len(), CHEST_SLOTS);
        assert_eq!(
            second.slot(CHEST_SLOTS - 1),
            Some(ItemStack { item, count: 128 })
        );
        assert_eq!(second.revision, 42);
        assert_eq!(journal.container_at(FIRST), first.as_ref());
        assert_eq!(journal.gameplay().drops, [expected_drop]);
        assert_eq!(journal.gameplay().next_drop, id);
        assert_eq!(journal.gameplay().revision, revision + 1);
        let gameplay = journal.gameplay().clone();
        let second = second.clone();
        assert!(
            !journal
                .import_drop_to_chest(if paired { FIRST } else { SECOND }, id)
                .unwrap()
        );
        assert_eq!(journal.container_at(SECOND), Some(&second));
        assert_eq!(journal.gameplay(), &gameplay);
        assert!(journal.gameplay().valid());
    }
}
