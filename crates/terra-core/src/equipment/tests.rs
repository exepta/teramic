use super::*;
use crate::{
    DamageCause, Inventory, SurvivalVitals,
    crafting::{CraftDestination, CraftStation, RecipeBook},
};

fn armor(material: ArmorMaterial, piece: ArmorPiece) -> ItemKind {
    ItemKind::Armor(ArmorItem { material, piece })
}
fn suit(material: ArmorMaterial) -> EquipmentSlots {
    let mut slots = EquipmentSlots::default();
    for piece in ArmorPiece::ALL {
        assert!(slots.set(piece.slot(), Some(armor(material, piece))));
    }
    slots
}

#[test]
fn equipment_slot_identity_keeps_the_old_nine_slots_and_appends_the_cape() {
    assert_eq!(EquipmentSlot::Helmet.index(), 0);
    assert_eq!(EquipmentSlot::Earrings.index(), 1);
    assert_eq!(EquipmentSlot::Chestplate.index(), 2);
    assert_eq!(EquipmentSlot::Necklace.index(), 3);
    assert_eq!(EquipmentSlot::Pants.index(), 4);
    assert_eq!(EquipmentSlot::Belt.index(), 5);
    assert_eq!(EquipmentSlot::Boots.index(), 6);
    assert_eq!(EquipmentSlot::Socks.index(), 7);
    assert_eq!(EquipmentSlot::Gloves.index(), 8);
    assert_eq!(EquipmentSlot::Cape.index(), 9);
    for slot in EquipmentSlot::ALL {
        assert_eq!(EquipmentSlot::from_index(slot.index()), Some(slot));
        assert!(!slot.name().is_empty());
    }
    assert_eq!(EquipmentSlot::from_index(10), None);
}

#[test]
fn all_base_equipment_items_have_individual_valid_definitions_and_recipes() {
    let book = RecipeBook::builtins();
    assert!(book.error.is_none(), "{:?}", book.error);
    for item in ArmorItem::ALL
        .map(ItemKind::Armor)
        .into_iter()
        .chain(AccessoryKind::ALL.map(ItemKind::Accessory))
        .chain(CapeItem::ALL_WHITE.map(ItemKind::Cape))
    {
        let definition = equipment_definition(item).unwrap();
        assert!(definition.valid());
        assert_eq!(item.max_stack(), 1);
        assert!(ItemKind::EXTRA.contains(&item));
        assert!(
            book.iter()
                .any(|recipe| recipe.output.item == item && recipe.output.count == 1)
        );
    }
    assert_ne!(equipment_catalog_fingerprint(), 0);
    assert_eq!(
        equipment_catalog_fingerprint(),
        equipment_catalog_fingerprint()
    );
}

#[test]
fn equipment_rejects_wrong_slots_extra_counts_and_non_equipment_without_partial_changes() {
    let helmet = armor(ArmorMaterial::Iron, ArmorPiece::Helmet);
    let mut slots = EquipmentSlots::default();
    assert!(!slots.set(EquipmentSlot::Boots, Some(helmet)));
    assert!(!slots.set(EquipmentSlot::Helmet, Some(ItemKind::Block(3))));
    assert_eq!(slots, EquipmentSlots::default());
    assert!(slots.set(EquipmentSlot::Helmet, Some(helmet)));
    assert!(!slots.set(EquipmentSlot::Helmet, Some(helmet)));
    assert!(slots.valid());
    slots.slots[0].count = 2;
    assert!(!slots.valid());
    slots.slots[0] = ItemStack {
        item: armor(ArmorMaterial::Iron, ArmorPiece::Boots),
        count: 1,
    };
    assert!(!slots.valid());
    assert_eq!(slots.get(EquipmentSlot::Helmet), None);
}

#[test]
fn armor_tiers_and_accessory_bonuses_are_bounded_and_material_specific() {
    for (material, points) in [
        (ArmorMaterial::Leather, 10.0),
        (ArmorMaterial::Iron, 20.0),
        (ArmorMaterial::Gold, 16.0),
        (ArmorMaterial::Diamond, 30.0),
        (ArmorMaterial::Ruby, 35.0),
    ] {
        let loadout = suit(material);
        assert!((loadout.bonuses().armor_points - points).abs() < 0.001);
        let mut vitals = SurvivalVitals::default();
        vitals.apply_equipment(loadout);
        assert!(vitals.valid());
        vitals.hurt(10.0, DamageCause::Animal);
        assert!((vitals.health - (100.0 - 10.0 * (1.0 - points * 0.02))).abs() < 0.001);
    }
    let mut loadout = suit(ArmorMaterial::Ruby);
    for kind in AccessoryKind::ALL {
        assert!(loadout.set(kind.slot(), Some(ItemKind::Accessory(kind))));
    }
    assert_eq!(loadout.iter().count(), 9);
    assert_eq!(loadout.bonuses().health_bonus, 45.0);
    assert_eq!(loadout.bonuses().stamina_bonus, 40.0);
}

#[test]
fn equipping_and_unequipping_never_produces_free_health_or_stamina() {
    let mut vitals = SurvivalVitals {
        health: 60.0,
        stamina: 35.0,
        ..Default::default()
    };
    let loadout = suit(ArmorMaterial::Ruby);
    for _ in 0..20 {
        vitals.apply_equipment(loadout);
        assert_eq!(vitals.health, 60.0);
        assert_eq!(vitals.stamina, 35.0);
        assert_eq!(vitals.max_health, 130.0);
        assert_eq!(vitals.max_stamina, 115.0);
        vitals.apply_equipment(EquipmentSlots::default());
        assert_eq!(vitals.health, 60.0);
        assert_eq!(vitals.stamina, 35.0);
    }
    vitals.apply_equipment(loadout);
    vitals.health = vitals.max_health;
    vitals.stamina = vitals.max_stamina;
    vitals.apply_equipment(EquipmentSlots::default());
    assert_eq!(vitals.health, 100.0);
    assert_eq!(vitals.stamina, 100.0);
    assert!(vitals.valid());
}

#[test]
fn armor_does_not_reduce_internal_poison_smoke_hunger_or_drowning() {
    for cause in [
        DamageCause::Poison,
        DamageCause::Smoke,
        DamageCause::Starvation,
        DamageCause::Drowning,
        DamageCause::Bleeding,
        DamageCause::Void,
    ] {
        let mut vitals = SurvivalVitals::default();
        vitals.apply_equipment(suit(ArmorMaterial::Ruby));
        vitals.hurt(10.0, cause);
        assert_eq!(vitals.health, 90.0);
    }
}

#[test]
fn gear_never_merges_in_backpack_cursor_drag_collect_or_container_transfers() {
    let item = armor(ArmorMaterial::Iron, ArmorPiece::Helmet);
    assert!(!ItemStack { item, count: 2 }.valid());
    let mut bag = Inventory::default();
    assert_eq!(bag.insert(item, 3), 0);
    assert_eq!(bag.occupied_slots(), 3);
    assert!(bag.slots.iter().all(|stack| stack.count == 1));
    assert!(bag.click(0, false));
    assert!(!bag.click(1, false));
    assert!(!bag.collect_from(1));
    assert!(!bag.collect(1));
    assert!(bag.valid());
    assert_eq!(bag.cursor.unwrap().count, 1);
    assert!(bag.click(3, false));
    assert!(bag.valid());
    let mut chest = Inventory::default();
    assert!(crate::containers::apply_gesture(
        &mut bag,
        std::slice::from_mut(&mut chest),
        crate::containers::ContainerGesture::QuickMove { slot: 1 }
    ));
    assert!(chest.valid());
    assert_eq!(chest.slot(0).unwrap().count, 1);
}

#[test]
fn gear_bulk_crafting_uses_separate_inventory_slots_and_cannot_overfill_cursor() {
    let item = armor(ArmorMaterial::Leather, ArmorPiece::Gloves);
    let book = RecipeBook::builtins();
    let recipe = book
        .iter()
        .find(|recipe| recipe.output.item == item)
        .unwrap();
    let mut bag = Inventory::default();
    bag.insert(
        ItemKind::Animal(crate::creatures::AnimalProduct::Leather),
        12,
    );
    let before = bag.clone();
    assert!(
        book.craft(
            &mut bag,
            recipe.key(),
            2,
            CraftStation::Workbench,
            CraftDestination::Cursor
        )
        .is_err()
    );
    assert_eq!(before, bag);
    book.craft(
        &mut bag,
        recipe.key(),
        2,
        CraftStation::Workbench,
        CraftDestination::Inventory,
    )
    .unwrap();
    assert_eq!(bag.count(item), 2);
    assert_eq!(
        bag.slots.iter().filter(|stack| stack.item == item).count(),
        2
    );
    assert!(bag.valid());
}

#[test]
fn equipment_text_roundtrip_and_legacy_stats_defaults_preserve_copy_contract() {
    fn is_copy<T: Copy>() {}
    is_copy::<EquipmentSlots>();
    is_copy::<crate::PlayerStats>();
    let loadout = suit(ArmorMaterial::Diamond);
    let text = toml::to_string(&loadout).unwrap();
    let loaded: EquipmentSlots = toml::from_str(&text).unwrap();
    assert_eq!(loaded, loadout);
    let old: crate::PlayerStats = toml::from_str("blocks_broken=4").unwrap();
    assert_eq!(old.equipment, EquipmentSlots::default());
    assert_eq!(old.survival.max_health, 100.0);
}

#[test]
fn capes_have_five_white_base_items_and_color_independent_tier_bonuses() {
    for cape in CapeItem::ALL {
        let item = ItemKind::Cape(cape);
        assert_eq!(EquipmentSlot::for_item(item), Some(EquipmentSlot::Cape));
        assert_eq!(item.max_stack(), 1);
        assert_eq!(
            equipment_definition(item),
            equipment_definition(ItemKind::Cape(cape.undyed()))
        );
        assert!(!EquipmentSlot::Chestplate.accepts(item));
        assert_eq!(
            ItemKind::EXTRA.contains(&item),
            cape.color == crate::beds::DyeColor::White
        );
    }
}

#[test]
fn cape_recoloring_accepts_already_dyed_cloth_without_changing_material_or_bonus() {
    for cape in CapeItem::ALL {
        let material = cape.material;
        let source = ItemKind::Cape(CapeItem {
            material,
            color: crate::beds::DyeColor::Blue,
        });
        let output = ItemKind::Cape(cape);
        let mut inventory = Inventory::default();
        inventory.insert(source, 1);
        inventory.insert(ItemKind::Dye(cape.color), 1);
        let recipe = RecipeBook::builtins()
            .iter()
            .find(|recipe| {
                recipe.output.item == output
                    && recipe
                        .ingredients()
                        .iter()
                        .any(|ingredient| ingredient.item == ItemKind::Dye(cape.color))
            })
            .unwrap();
        assert!(
            recipe
                .ingredients()
                .iter()
                .any(|ingredient| ingredient.matches(source))
        );
        RecipeBook::builtins()
            .craft(
                &mut inventory,
                recipe.key(),
                1,
                CraftStation::Hand,
                CraftDestination::Cursor,
            )
            .unwrap();
        assert_eq!(
            inventory.cursor,
            Some(ItemStack {
                item: output,
                count: 1
            })
        );
        assert_eq!(inventory.count(source), 0);
    }
}
