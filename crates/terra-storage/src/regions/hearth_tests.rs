use super::*;
use terra_core::{ItemKind, ItemStack, ToolKind, brewing, campfire};
use terra_world::edits::BlockObjectState;

fn fixture() -> (EditJournal, [i64; 3], [i64; 3], [i64; 3]) {
    let mut journal = EditJournal::default();
    let cauldron = [-1, 16, -1];
    let fire = [-1, 15, -1];
    let tunnel = [-2, 16, -1];
    for (position, material) in [
        (cauldron, brewing::MATERIAL),
        (fire, campfire::MATERIAL),
        (tunnel, terra_core::logistics::TUNNEL),
    ] {
        journal.record(position, material).unwrap();
        journal
            .set_object_state(position, Some(BlockObjectState::default()))
            .unwrap();
    }
    let mut inventory = Inventory {
        slots: vec![ItemStack::EMPTY; brewing::SLOTS],
        ..Default::default()
    };
    inventory.slots[brewing::SPOON] = ItemStack {
        item: ItemKind::Tool(ToolKind::GoldSpoon),
        count: 1,
    };
    inventory.slots[brewing::OUTPUT] = ItemStack {
        item: ItemKind::Potion(6),
        count: 3,
    };
    inventory.slots[brewing::BOTTLES] = ItemStack {
        item: ItemKind::GlassBottle,
        count: 2,
    };
    journal
        .commit_cauldron(
            cauldron,
            brewing::CauldronState {
                recipe: Some(6),
                water: 2,
                progress_seconds: 5.5,
            },
            inventory,
        )
        .unwrap();
    let mut inventory = Inventory {
        slots: vec![ItemStack::EMPTY; campfire::SLOTS],
        ..Default::default()
    };
    inventory.slots[campfire::FUEL] = ItemStack {
        item: ItemKind::Stick,
        count: 3,
    };
    let mut state = campfire::CampfireState::default();
    assert!(campfire::set_lit(&mut state, &mut inventory, true));
    assert!(campfire::advance(&mut state, &mut inventory, 0.75));
    journal.commit_campfire(fire, state, inventory).unwrap();
    journal.set_tunnel_random(tunnel, true).unwrap();
    (journal, cauldron, fire, tunnel)
}

#[test]
fn rga10_preserves_partial_brewing_burn_fuel_exact_potions_and_random_mode_across_reloads() {
    let root = std::env::temp_dir().join(format!("terra-hearths-{}", PlayerUuid::new_v4()));
    let world = WorldId(1640);
    let (mut journal, cauldron, fire, tunnel) = fixture();
    let legacy = [512, 8, 0];
    journal.record(legacy, 160).unwrap();
    journal
        .set_object_state(legacy, Some(BlockObjectState::default()))
        .unwrap();
    journal.set_distributor(legacy, Default::default()).unwrap();
    for _ in 0..3 {
        save_edits(&root, world, &journal).unwrap();
        assert_eq!(
            read(&root.join(region_name([-1, -1]))).unwrap().get(12..16),
            Some(b"RG10".as_slice())
        );
        assert_eq!(
            read(&root.join(region_name([1, 0]))).unwrap().get(12..16),
            Some(b"RGA9".as_slice())
        );
        let loaded = load_edits(&root, world).unwrap();
        assert_eq!(loaded.cauldron_at(cauldron), journal.cauldron_at(cauldron));
        assert_eq!(loaded.campfire_at(fire), journal.campfire_at(fire));
        assert_eq!(
            loaded.container_at(cauldron),
            journal.container_at(cauldron)
        );
        assert_eq!(loaded.container_at(fire), journal.container_at(fire));
        assert!(loaded.tunnel_random_at(tunnel));
        assert!(!loaded.tunnel_random_at(legacy));
        journal = loaded;
    }
    journal.record(cauldron, 0).unwrap();
    journal.record(fire, 0).unwrap();
    journal.record(tunnel, 0).unwrap();
    assert!(journal.cauldrons().next().is_none());
    assert!(journal.campfires().next().is_none());
    assert!(journal.container_at(cauldron).is_none());
    save_edits(&root, world, &journal).unwrap();
    let loaded = load_edits(&root, world).unwrap();
    assert!(loaded.cauldrons().next().is_none() && loaded.campfires().next().is_none());
    assert!(!loaded.tunnel_random_at(tunnel));
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn rga10_rejects_orphan_duplicate_invalid_or_misplaced_hearth_metadata() {
    let root = std::env::temp_dir().join(format!("terra-bad-hearths-{}", PlayerUuid::new_v4()));
    let world = WorldId(1641);
    let (journal, cauldron, fire, _) = fixture();
    save_edits(&root, world, &journal).unwrap();
    let path = root.join(region_name([-1, -1]));
    let original = read(&path).unwrap();
    for mutation in 0..6 {
        let mut data: HearthRegion = decode(b"RG10", &original).unwrap();
        match mutation {
            0 => data.cauldrons[0].0[0] -= 1,
            1 => data.cauldrons.push(data.cauldrons[0].clone()),
            2 => data.cauldrons[0].1.water = brewing::MAX_WATER + 1,
            3 => data.campfires[0].1.burn_remaining = f32::NAN,
            4 => data.random_tunnels[0] = fire,
            _ => data.random_tunnels.push(data.random_tunnels[0]),
        }
        std::fs::write(&path, encode(b"RG10", &data).unwrap()).unwrap();
        assert!(
            load_edits(&root, world).is_err(),
            "mutation {mutation}, {cauldron:?}"
        );
    }
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn rga11_keeps_compound_inventories_and_fire_state_separate_and_preserves_old_stacked_hearths() {
    let root = std::env::temp_dir().join(format!("terra-compound-{}", PlayerUuid::new_v4()));
    let world = WorldId(1650);
    let (mut journal, cauldron, old_fire, _) = fixture();
    let old = journal.campfire_at(old_fire).unwrap().clone();
    let old_inventory = journal.container_at(old_fire).unwrap().clone();
    let mut pose = journal.object_state(cauldron).unwrap();
    pose.facing |= brewing::ATTACHED_FIRE;
    journal.set_object_state(cauldron, Some(pose)).unwrap();
    let mut attached = campfire::AttachedCampfire {
        state: old.clone(),
        inventory: old_inventory.clone(),
    };
    attached.inventory.slots[campfire::INPUT] = ItemStack {
        item: campfire::recipes()[0].input,
        count: 4,
    };
    attached.inventory.slots[campfire::OUTPUT] = campfire::recipes()[0].output;
    journal
        .commit_attached_campfire(cauldron, attached.clone())
        .unwrap();
    let kettle = journal.container_at(cauldron).unwrap().clone();
    let section = terra_voxel::global_to_section(cauldron).0;
    let delta = journal.section_delta(section);
    let mut replica = EditJournal::default();
    replica.import_delta(delta.clone()).unwrap();
    assert!(replica.has_attached_campfire(cauldron));
    assert!(
        replica.attached_campfire_at(cauldron).is_none(),
        "public terrain must not leak private inventories"
    );
    let epoch = journal.mesh_epoch(section);
    attached.state.burn_remaining -= 0.25;
    journal
        .commit_attached_campfire(cauldron, attached.clone())
        .unwrap();
    assert_eq!(journal.section_delta(section), delta);
    assert_eq!(journal.mesh_epoch(section), epoch);
    for _ in 0..3 {
        save_edits(&root, world, &journal).unwrap();
        let bytes = read(&root.join(region_name([-1, -1]))).unwrap();
        assert_eq!(bytes.get(12..16), Some(b"RG11".as_slice()));
        let loaded = load_edits(&root, world).unwrap();
        assert!(loaded.has_attached_campfire(cauldron));
        assert_eq!(loaded.attached_campfire_at(cauldron), Some(&attached));
        assert_eq!(loaded.container_at(cauldron), Some(&kettle));
        assert_eq!(loaded.campfire_at(old_fire), Some(&old));
        assert_eq!(loaded.container_at(old_fire), Some(&old_inventory));
        journal = loaded;
    }
    journal.record(cauldron, 0).unwrap();
    assert!(!journal.has_attached_campfire(cauldron));
    assert!(journal.attached_campfire_at(cauldron).is_none());
    assert!(journal.campfire_at(old_fire).is_some());
    save_edits(&root, world, &journal).unwrap();
    assert!(
        load_edits(&root, world)
            .unwrap()
            .attached_campfires()
            .next()
            .is_none()
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn rga11_rejects_missing_duplicate_or_corrupt_attached_state_without_overwriting_kettle_slots() {
    let root = std::env::temp_dir().join(format!("terra-bad-compound-{}", PlayerUuid::new_v4()));
    let world = WorldId(1651);
    let (mut journal, cauldron, fire, _) = fixture();
    let mut pose = journal.object_state(cauldron).unwrap();
    pose.facing |= brewing::ATTACHED_FIRE;
    journal.set_object_state(cauldron, Some(pose)).unwrap();
    journal
        .commit_attached_campfire(cauldron, Default::default())
        .unwrap();
    save_edits(&root, world, &journal).unwrap();
    let path = root.join(region_name([-1, -1]));
    let original = read(&path).unwrap();
    for mutation in 0..5 {
        let mut data: CompoundHearthRegion = decode(b"RG11", &original).unwrap();
        match mutation {
            0 => data.attached_campfires.clear(),
            1 => data
                .attached_campfires
                .push(data.attached_campfires[0].clone()),
            2 => data.attached_campfires[0].0 = fire,
            3 => data.attached_campfires[0].1.state.burn_remaining = f32::NAN,
            _ => data.attached_campfires[0].1.inventory.slots = vec![ItemStack::EMPTY; 4],
        }
        std::fs::write(&path, encode(b"RG11", &data).unwrap()).unwrap();
        assert!(load_edits(&root, world).is_err(), "mutation {mutation}");
    }
    std::fs::remove_dir_all(root).unwrap();
}
