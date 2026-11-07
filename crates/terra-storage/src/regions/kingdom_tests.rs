use super::*;
use terra_core::kingdom::{
    ClaimActor, ClaimKind, KingdomHeraldry, KingdomIcon, KingdomRegistry, KingdomState,
    SettlementKind, claim_chunk,
};

fn temporary() -> std::path::PathBuf {
    let root = std::env::temp_dir().join(format!(
        "teramic-kingdom-save-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(&root).unwrap();
    root
}

#[test]
fn named_members_keep_ids_anchors_history_and_break_protection_through_real_reload() {
    let root = temporary();
    let world = WorldId(43);
    let founder = PlayerUuid::from_u128(101);
    let member = PlayerUuid::from_u128(202);
    let anchor = [-17, 15, -1];
    let mut journal = EditJournal::default();
    journal
        .record(anchor, terra_core::castle::BERGFRIED)
        .unwrap();
    let id = journal
        .gameplay_mut()
        .kingdoms
        .found_settlement(
            founder,
            "Alice",
            anchor,
            "Stonehaven",
            SettlementKind::City,
            KingdomHeraldry {
                color: [40, 90, 140],
                icon: KingdomIcon::Mountain,
            },
        )
        .unwrap();
    journal
        .gameplay_mut()
        .kingdoms
        .join_with_name(member, "Bob", id)
        .unwrap();
    journal
        .gameplay_mut()
        .kingdoms
        .set_role(
            founder,
            id,
            member,
            terra_core::kingdom::KingdomRole::Officer,
        )
        .unwrap();
    let base = claim_chunk(anchor);
    journal
        .gameplay_mut()
        .kingdoms
        .claim(
            id,
            [base[0] + 1, base[1]],
            ClaimKind::RealmTerritory,
            ClaimActor::Bot {
                id: 77,
                authorized_by: member,
            },
        )
        .unwrap();
    let inventory = journal
        .gameplay_mut()
        .inventories
        .entry(founder)
        .or_default();
    inventory.insert(
        terra_core::ItemKind::Block(terra_core::castle::STONE_WALL),
        8,
    );
    inventory.insert(
        terra_core::ItemKind::Mineral(terra_core::MineralItem::GoldIngot),
        1,
    );
    journal.gameplay_mut().upgrade_kingdom(founder, id).unwrap();
    journal.gameplay_mut().revision = 19;
    save_edits(&root, world, &journal).unwrap();
    assert_eq!(&read(&root.join(INDEX)).unwrap()[12..16], b"ID12");
    let mut loaded = load_edits(&root, world).unwrap();
    assert_eq!(loaded.gameplay(), journal.gameplay());
    assert_eq!(loaded.gameplay().kingdoms.get(id).unwrap().level, 2);
    assert_eq!(
        loaded.gameplay().kingdoms.get(id).unwrap().kind,
        SettlementKind::City
    );
    assert_eq!(
        loaded
            .gameplay()
            .kingdoms
            .claim_at([base[0] + 1, base[1]])
            .unwrap()
            .actor,
        ClaimActor::Bot {
            id: 77,
            authorized_by: member
        }
    );
    assert_eq!(
        loaded.gameplay().kingdoms.get(id).unwrap().founder_name,
        "Alice"
    );
    assert_eq!(
        loaded.gameplay().kingdoms.get(id).unwrap().member_names,
        [(founder, "Alice".into()), (member, "Bob".into())]
    );
    let section = terra_voxel::global_to_section(anchor).0;
    let before = loaded.section_delta(section);
    assert_eq!(
        loaded.record(anchor, 0),
        Err(terra_voxel::blocks::BlockError::ProtectedStructure)
    );
    assert_eq!(loaded.section_delta(section), before);
    loaded
        .gameplay_mut()
        .kingdoms
        .dissolve(founder, id)
        .unwrap();
    loaded.record(anchor, 0).unwrap();
    save_edits(&root, world, &loaded).unwrap();
    let mut final_save = load_edits(&root, world).unwrap();
    assert_eq!(final_save.block(anchor), Some(0));
    assert_eq!(
        final_save.gameplay().kingdoms.get(id).unwrap().state,
        KingdomState::Dissolved
    );
    final_save
        .record(anchor, terra_core::castle::BERGFRIED)
        .unwrap();
    let next = final_save
        .gameplay_mut()
        .kingdoms
        .found(founder, anchor, "Stonehaven")
        .unwrap();
    assert!(next.0 > id.0);
    save_edits(&root, world, &final_save).unwrap();
    assert_eq!(
        load_edits(&root, world).unwrap().gameplay(),
        final_save.gameplay()
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn idx8_migrates_identities_without_silently_claiming_shared_legacy_columns() {
    let root = temporary();
    let world = WorldId(438);
    let founder = PlayerUuid::from_u128(1);
    let mut journal = EditJournal::default();
    let anchor = [0, 20, 0];
    journal
        .record([1, 20, 0], terra_core::castle::BERGFRIED)
        .unwrap();
    journal
        .record(anchor, terra_core::castle::BERGFRIED)
        .unwrap();
    let id = journal
        .gameplay_mut()
        .kingdoms
        .found_with_name(founder, "Alice", anchor, "Old Home")
        .unwrap();
    save_edits(&root, world, &journal).unwrap();
    let current = read_index(&root).unwrap();
    let mut old = KingdomIndex8 {
        prior: current.prior,
        kingdoms: KingdomRegistry8 {
            next_id: current.kingdoms.next_id,
            revision: current.kingdoms.revision,
            kingdoms: current
                .kingdoms
                .kingdoms
                .into_iter()
                .map(|k| KingdomRecord8 {
                    id: k.id,
                    name: k.name,
                    founder: k.founder,
                    founder_name: k.founder_name,
                    members: k.members,
                    member_names: k.member_names,
                    location: k.location,
                    state: k.state,
                })
                .collect(),
        },
    };
    let other = PlayerUuid::from_u128(2);
    let other_id = terra_core::kingdom::KingdomId(2);
    old.kingdoms.kingdoms.push(KingdomRecord8 {
        id: other_id,
        name: "Other old seat".into(),
        founder: other,
        founder_name: "Bob".into(),
        members: vec![other],
        member_names: vec![(other, "Bob".into())],
        location: [1, 20, 0],
        state: KingdomState::Active,
    });
    old.kingdoms.next_id = 3;
    old.kingdoms.revision = 2;
    fs::write(root.join(INDEX), encode(b"IDX8", &old).unwrap()).unwrap();
    let mut loaded = load_edits(&root, world).unwrap();
    let record = loaded.gameplay().kingdoms.get(id).unwrap();
    assert_eq!(record.founder, founder);
    assert_eq!(record.level, 1);
    assert_eq!(record.kind, SettlementKind::Kingdom);
    assert!(loaded.gameplay().kingdoms.claims.is_empty());
    assert_eq!(
        loaded.gameplay_mut().kingdoms.claim(
            id,
            [0, 0],
            ClaimKind::ProtectedBuilding,
            ClaimActor::Player(founder)
        ),
        Err(terra_core::kingdom::KingdomError::ClaimOccupied)
    );
    loaded
        .gameplay_mut()
        .kingdoms
        .dissolve(other, other_id)
        .unwrap();
    loaded
        .gameplay_mut()
        .kingdoms
        .claim(
            id,
            [0, 0],
            ClaimKind::ProtectedBuilding,
            ClaimActor::Player(founder),
        )
        .unwrap();
    save_edits(&root, world, &loaded).unwrap();
    assert_eq!(&read(&root.join(INDEX)).unwrap()[12..16], b"ID12");
    assert_eq!(
        load_edits(&root, world).unwrap().gameplay().kingdoms,
        loaded.gameplay().kingdoms
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn old_index_versions_default_to_no_kingdoms_and_upgrade_without_losing_prior_state() {
    for birth in [0, 1] {
        let root = temporary();
        let world = WorldId(430 + u64::from(birth));
        let mut journal = EditJournal::default();
        journal.gameplay_mut().next_animal_birth = birth;
        save_edits(&root, world, &journal).unwrap();
        let old_tag = if birth == 0 { b"IDX6" } else { b"IDX7" };
        assert_eq!(&read(&root.join(INDEX)).unwrap()[12..16], old_tag);
        let mut loaded = load_edits(&root, world).unwrap();
        assert_eq!(loaded.gameplay().kingdoms, KingdomRegistry::default());
        let anchor = [0, 17, 0];
        loaded
            .record(anchor, terra_core::castle::BERGFRIED)
            .unwrap();
        loaded
            .gameplay_mut()
            .kingdoms
            .found(PlayerUuid::from_u128(3), anchor, "New Crown")
            .unwrap();
        save_edits(&root, world, &loaded).unwrap();
        let after = load_edits(&root, world).unwrap();
        assert_eq!(after.gameplay().next_animal_birth, birth);
        assert_eq!(after.gameplay().kingdoms, loaded.gameplay().kingdoms);
        fs::remove_dir_all(root).unwrap();
    }
}

#[test]
fn missing_bergfried_rejects_save_without_overwriting_old_state_and_rejects_load() {
    let root = temporary();
    let world = WorldId(439);
    let mut journal = EditJournal::default();
    save_edits(&root, world, &journal).unwrap();
    let previous = read(&root.join(INDEX)).unwrap();
    journal
        .gameplay_mut()
        .kingdoms
        .found(PlayerUuid::from_u128(1), [0, 20, 0], "Missing Keep")
        .unwrap();
    assert!(
        save_edits(&root, world, &journal)
            .unwrap_err()
            .contains("Bergfried")
    );
    assert_eq!(read(&root.join(INDEX)).unwrap(), previous);
    assert_eq!(
        load_edits(&root, world).unwrap().gameplay().kingdoms,
        KingdomRegistry::default()
    );
    let mut invalid = read_index(&root).unwrap();
    invalid.kingdoms = journal.gameplay().kingdoms.clone();
    fs::write(
        root.join(INDEX),
        encode(
            b"ID12",
            &TreeIndex {
                prior: invalid,
                trees: Default::default(),
            },
        )
        .unwrap(),
    )
    .unwrap();
    assert!(
        load_edits(&root, world)
            .err()
            .unwrap()
            .contains("Bergfried")
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn acquired_cape_heraldry_survives_realm_changes_trade_equipment_and_real_save() {
    use terra_core::{CapeItem, EquipmentSlot, ItemKind};
    let root = temporary();
    let world = WorldId(196);
    let owner = PlayerUuid::from_u128(19601);
    let recipient = PlayerUuid::from_u128(19602);
    let mut journal = EditJournal::default();
    let heraldry = KingdomHeraldry {
        color: [27, 109, 181],
        icon: KingdomIcon::Anchor,
    };
    let id = journal
        .gameplay_mut()
        .kingdoms
        .found_settlement(
            owner,
            "Alice",
            [-17, 12, -33],
            "Cape makers",
            SettlementKind::Kingdom,
            heraldry,
        )
        .unwrap();
    let cape = ItemKind::Cape(CapeItem::ALL_WHITE[0])
        .with_default_heraldry(journal.gameplay().kingdoms.heraldry_for_member(owner));
    journal
        .gameplay_mut()
        .inventories
        .entry(owner)
        .or_default()
        .insert(cape, 1);
    // The same item is valid after transfer to somebody outside the realm.
    journal
        .gameplay_mut()
        .inventories
        .entry(recipient)
        .or_default()
        .insert(cape, 1);
    let mut stats = terra_core::PlayerStats::default();
    assert!(stats.equipment.set(EquipmentSlot::Cape, Some(cape)));
    stats.survival.apply_equipment(stats.equipment);
    assert!(journal.update_player_stats(recipient, stats));
    journal
        .gameplay_mut()
        .kingdoms
        .change_appearance(
            owner,
            id,
            KingdomHeraldry {
                color: [210, 40, 10],
                icon: KingdomIcon::Tree,
            },
        )
        .unwrap();
    journal.gameplay_mut().kingdoms.dissolve(owner, id).unwrap();
    save_edits(&root, world, &journal).unwrap();
    let loaded = load_edits(&root, world).unwrap();
    assert_eq!(loaded.gameplay().inventories[&owner].count(cape), 1);
    assert_eq!(loaded.gameplay().inventories[&recipient].count(cape), 1);
    assert_eq!(
        loaded.gameplay().player_stats[&recipient]
            .equipment
            .get(EquipmentSlot::Cape),
        Some(cape)
    );
    assert_eq!(loaded.gameplay().kingdoms, journal.gameplay().kingdoms);
    assert!(matches!(cape, ItemKind::HeraldicCape { heraldry: saved, .. } if saved == heraldry));
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn military_bed_identities_and_dead_tombstones_survive_real_save_without_rerolled_garrison() {
    use terra_core::creatures::{
        Creature, CreatureBehavior, CreatureKind, CreatureNeeds, soldiers::SoldierLoadout,
        villagers,
    };
    let root = temporary();
    let world = WorldId(197);
    let mut journal = EditJournal::default();
    let home = [-27, 16];
    for (index, loadout) in SoldierLoadout::ALL.into_iter().enumerate() {
        let slot = [128, 143, 511][index];
        let id = villagers::id(home, slot).unwrap();
        let creature = Creature {
            id,
            kind: CreatureKind::Villager,
            home,
            position: [-850.0 + index as f64 * 2.0, 20.0, 516.0],
            velocity: [0.0; 3],
            yaw: 0.0,
            health: if index == 1 { 0.0 } else { 20.0 },
            behavior: if index == 1 {
                CreatureBehavior::Dead
            } else {
                CreatureBehavior::Rest
            },
            timer: 0.0,
            age: 123.0,
            variant: loadout.role(),
            needs: CreatureNeeds::full(CreatureKind::Villager),
        };
        assert!(creature.valid());
        journal
            .gameplay_mut()
            .creatures
            .animals
            .insert(id, creature);
    }
    save_edits(&root, world, &journal).unwrap();
    let loaded = load_edits(&root, world).unwrap();
    assert_eq!(loaded.gameplay().creatures, journal.gameplay().creatures);
    let dead = villagers::id(home, 143).unwrap();
    assert_eq!(
        loaded.gameplay().creatures.animals[&dead].behavior,
        CreatureBehavior::Dead
    );
    assert_eq!(loaded.gameplay().creatures.animals.len(), 3);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn f57_roles_and_mutual_diplomacy_survive_real_save_and_idx9_migrates_without_escalation() {
    use terra_core::kingdom::{DiplomaticRelation, DiplomaticTarget, KingdomRole};
    let root = temporary();
    let world = WorldId(199);
    let founder = PlayerUuid::from_u128(1);
    let member = PlayerUuid::from_u128(2);
    let other = PlayerUuid::from_u128(3);
    let mut journal = EditJournal::default();
    for p in [[0, 20, 0], [32, 20, 0]] {
        journal.record(p, terra_core::castle::BERGFRIED).unwrap();
    }
    let r = &mut journal.gameplay_mut().kingdoms;
    let a = r.found(founder, [0, 20, 0], "First").unwrap();
    let b = r.found(other, [32, 20, 0], "Second").unwrap();
    r.join(member, a).unwrap();
    r.set_role(founder, a, member, KingdomRole::Worker).unwrap();
    r.set_relation(
        founder,
        a,
        DiplomaticTarget::Player(b),
        DiplomaticRelation::Allied,
    )
    .unwrap();
    r.set_relation(
        other,
        b,
        DiplomaticTarget::Player(a),
        DiplomaticRelation::Allied,
    )
    .unwrap();
    save_edits(&root, world, &journal).unwrap();
    let mut loaded = load_edits(&root, world).unwrap();
    assert_eq!(loaded.gameplay().kingdoms, journal.gameplay().kingdoms);
    assert_eq!(
        loaded.gameplay().kingdoms.role_for(a, member),
        Some(KingdomRole::Worker)
    );
    assert_eq!(
        loaded.gameplay().kingdoms.relation(a, b),
        DiplomaticRelation::Allied
    );
    loaded
        .gameplay_mut()
        .kingdoms
        .kick(founder, a, member)
        .unwrap();
    save_edits(&root, world, &loaded).unwrap();
    assert_eq!(
        load_edits(&root, world)
            .unwrap()
            .gameplay()
            .kingdoms
            .role_for(a, member),
        None
    );
    // Produce the exact frozen four-field registry of an old IDX9 file.
    let current = read_index(&root).unwrap();
    let mut records = current.kingdoms.kingdoms.clone();
    records[0].members.push(member);
    records[0].member_names.push((member, "Old member".into()));
    let old = KingdomIndex9 {
        prior: current.prior,
        kingdoms: KingdomRegistry9 {
            next_id: current.kingdoms.next_id,
            revision: current.kingdoms.revision,
            kingdoms: records.into_iter().map(Into::into).collect(),
            claims: current.kingdoms.claims,
        },
    };
    fs::write(root.join(INDEX), encode(b"IDX9", &old).unwrap()).unwrap();
    let migrated = load_edits(&root, world).unwrap();
    let r = &migrated.gameplay().kingdoms;
    assert_eq!(r.role_for(a, member), Some(KingdomRole::Member));
    assert!(!r.can_manage(member, a));
    assert_eq!(r.relation(a, b), DiplomaticRelation::Neutral);
    assert!(r.valid());
    save_edits(&root, world, &migrated).unwrap();
    assert_eq!(&read(&root.join(INDEX)).unwrap()[12..16], b"ID12");
    assert_eq!(load_edits(&root, world).unwrap().gameplay().kingdoms, *r);
    fs::remove_dir_all(root).unwrap();
}
