use super::*;
use terra_core::{AccessoryKind, ArmorItem, ArmorMaterial, ArmorPiece, ItemKind};

fn helmet(material: ArmorMaterial) -> ItemKind {
    ItemKind::Armor(ArmorItem {
        material,
        piece: ArmorPiece::Helmet,
    })
}

#[test]
fn equipment_clicks_pick_up_place_and_swap_exactly_one_matching_piece() {
    let iron = helmet(ArmorMaterial::Iron);
    let leather = helmet(ArmorMaterial::Leather);
    let mut inventory = Inventory {
        cursor: Some(ItemStack {
            item: iron,
            count: 1,
        }),
        ..Default::default()
    };
    let mut equipment = EquipmentSlots::default();
    assert!(click(
        &mut inventory,
        &mut equipment,
        EquipmentSlot::Helmet.index(),
        true
    ));
    assert_eq!(inventory.cursor, None);
    assert_eq!(equipment.get(EquipmentSlot::Helmet), Some(iron));
    assert_eq!(inventory.revision, 1);
    inventory.cursor = Some(ItemStack {
        item: leather,
        count: 1,
    });
    let before = (inventory.clone(), equipment);
    assert!(!click(
        &mut inventory,
        &mut equipment,
        EquipmentSlot::Helmet.index(),
        true
    ));
    assert_eq!((inventory.clone(), equipment), before);
    assert!(click(
        &mut inventory,
        &mut equipment,
        EquipmentSlot::Helmet.index(),
        false
    ));
    assert_eq!(inventory.cursor.unwrap().item, iron);
    assert_eq!(equipment.get(EquipmentSlot::Helmet), Some(leather));
    inventory.cursor = None;
    assert!(click(
        &mut inventory,
        &mut equipment,
        EquipmentSlot::Helmet.index(),
        true
    ));
    assert_eq!(inventory.cursor.unwrap().item, leather);
    assert_eq!(equipment.get(EquipmentSlot::Helmet), None);
    assert_eq!(inventory.revision, 3);
}

#[test]
fn wrong_slot_invalid_indices_and_overstacked_cursors_are_atomic_rejections() {
    let mut inventory = Inventory {
        cursor: Some(ItemStack {
            item: helmet(ArmorMaterial::Iron),
            count: 1,
        }),
        ..Default::default()
    };
    let mut equipment = EquipmentSlots::default();
    for slot in [EquipmentSlot::Boots.index(), 9, usize::MAX] {
        let before = (inventory.clone(), equipment);
        assert!(!click(&mut inventory, &mut equipment, slot, false));
        assert_eq!((inventory.clone(), equipment), before);
    }
    inventory.cursor.as_mut().unwrap().count = 128;
    let before = (inventory.clone(), equipment);
    assert!(!click(
        &mut inventory,
        &mut equipment,
        EquipmentSlot::Helmet.index(),
        false
    ));
    assert_eq!((inventory, equipment), before);
}

#[test]
fn shift_equip_and_unequip_respect_capacity_cursor_and_revision() {
    let iron = helmet(ArmorMaterial::Iron);
    let mut inventory = Inventory::default();
    inventory.creative_set(
        11,
        ItemStack {
            item: iron,
            count: 1,
        },
    );
    inventory.cursor = Some(ItemStack {
        item: ItemKind::Stick,
        count: 3,
    });
    let mut equipment = EquipmentSlots::default();
    let revision = inventory.revision;
    assert!(equip(&mut inventory, &mut equipment, 11));
    assert_eq!(inventory.slot(11), None);
    assert_eq!(inventory.cursor.unwrap().count, 3);
    assert_eq!(equipment.get(EquipmentSlot::Helmet), Some(iron));
    assert_eq!(inventory.revision, revision + 1);
    for slot in 0..terra_core::INVENTORY_SLOTS {
        inventory.creative_set(
            slot,
            ItemStack {
                item: ItemKind::Block(3),
                count: 128,
            },
        );
    }
    let before = (inventory.clone(), equipment);
    assert!(!unequip(
        &mut inventory,
        &mut equipment,
        EquipmentSlot::Helmet.index()
    ));
    assert_eq!((inventory.clone(), equipment), before);
    inventory.slots[5] = ItemStack::EMPTY;
    let revision = inventory.revision;
    assert!(unequip(
        &mut inventory,
        &mut equipment,
        EquipmentSlot::Helmet.index()
    ));
    assert_eq!(inventory.slot(5).unwrap().item, iron);
    assert_eq!(equipment.get(EquipmentSlot::Helmet), None);
    assert_eq!(inventory.revision, revision + 1);
    assert_eq!(inventory.cursor.unwrap().count, 3);
}

#[test]
fn all_nine_equipment_slots_accept_their_correct_item_and_never_duplicate_it() {
    for slot in EquipmentSlot::ALL {
        let item = ArmorItem::ALL
            .into_iter()
            .map(ItemKind::Armor)
            .chain(AccessoryKind::ALL.into_iter().map(ItemKind::Accessory))
            .chain(
                terra_core::CapeItem::ALL_WHITE
                    .into_iter()
                    .map(ItemKind::Cape),
            )
            .find(|&item| EquipmentSlot::for_item(item) == Some(slot))
            .unwrap();
        let mut inventory = Inventory::default();
        assert_eq!(inventory.insert(item, 1), 0);
        let mut equipment = EquipmentSlots::default();
        assert!(equip(&mut inventory, &mut equipment, 0));
        assert_eq!(inventory.count(item), 0);
        assert_eq!(equipment.iter().count(), 1);
        assert!(!equip(&mut inventory, &mut equipment, 0));
        assert!(unequip(&mut inventory, &mut equipment, slot.index()));
        assert_eq!(inventory.count(item), 1);
        assert_eq!(equipment.iter().count(), 0);
        assert!(!unequip(&mut inventory, &mut equipment, slot.index()));
    }
}

#[test]
fn saturated_revision_and_reserved_crafting_items_cannot_be_equipped() {
    let item = helmet(ArmorMaterial::Iron);
    let mut inventory = Inventory::default();
    inventory.insert(item, 1);
    let mut equipment = EquipmentSlots::default();
    let mut reserved = [0; terra_core::INVENTORY_SLOTS];
    reserved[0] = 1;
    let before = (inventory.clone(), equipment);
    assert!(!inventory.with_reserved(reserved, |bag| equip(bag, &mut equipment, 0)));
    assert_eq!((inventory.clone(), equipment), before);
    inventory.revision = u64::MAX;
    let before = (inventory.clone(), equipment);
    assert!(!equip(&mut inventory, &mut equipment, 0));
    assert_eq!((inventory, equipment), before);
}
