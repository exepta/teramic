use super::*;
use terra_core::kingdom::{
    DiplomaticRelation, DiplomaticTarget, KingdomHeraldry, KingdomRole, KingdomRules,
    SettlementKind,
};

fn temporary() -> std::path::PathBuf {
    let path = std::env::temp_dir().join(format!(
        "teramic-hierarchy-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(&path).unwrap();
    path
}

#[test]
fn frozen_id10_and_id11_preserve_independent_legacy_settlements_roles_and_trees_without_inventing_parents()
 {
    for trees in [false, true] {
        let root = temporary();
        let world = WorldId(20201);
        let mut journal = EditJournal::default();
        let founders = [
            PlayerUuid::from_u128(1),
            PlayerUuid::from_u128(2),
            PlayerUuid::from_u128(3),
        ];
        let mut ids = Vec::new();
        for (index, kind) in [
            SettlementKind::Kingdom,
            SettlementKind::Village,
            SettlementKind::City,
        ]
        .into_iter()
        .enumerate()
        {
            let anchor = [index as i64 * 32, 20, 0];
            journal
                .record(anchor, terra_core::castle::BERGFRIED)
                .unwrap();
            ids.push(
                journal
                    .gameplay_mut()
                    .kingdoms
                    .found_settlement(
                        founders[index],
                        "Founder",
                        anchor,
                        &format!("Legacy {index}"),
                        kind,
                        KingdomHeraldry::default(),
                    )
                    .unwrap(),
            );
        }
        let member = PlayerUuid::from_u128(4);
        let registry = &mut journal.gameplay_mut().kingdoms;
        registry.join(member, ids[0]).unwrap();
        registry
            .set_role(founders[0], ids[0], member, KingdomRole::Worker)
            .unwrap();
        registry
            .set_relation(
                founders[0],
                ids[0],
                DiplomaticTarget::Player(ids[1]),
                DiplomaticRelation::Allied,
            )
            .unwrap();
        registry
            .set_relation(
                founders[1],
                ids[1],
                DiplomaticTarget::Player(ids[0]),
                DiplomaticRelation::Allied,
            )
            .unwrap();
        if trees {
            let tree = terra_core::TreeRecord {
                id: 9001,
                root: [8, 21, 8],
                species: terra_core::TreeSpecies::Apple,
                seed: 10,
                planted_by: Some(founders[0]),
                stage: terra_core::TreeStage::GeneratedFelled,
                growth_seconds: 1400.,
                bounds: None,
                wood: vec![[1, 0, 0]],
                leaves: vec![[1, 1, 0]],
                fruit: None,
                revision: 3,
            }
            .with_bounds();
            for (offsets, material) in [
                (&tree.wood, tree.species.wood()),
                (&tree.leaves, tree.species.leaves()),
            ] {
                for p in offsets {
                    journal.record(tree.global(*p), material).unwrap();
                }
            }
            assert!(journal.gameplay_mut().trees.insert(tree));
        }
        save_edits(&root, world, &journal).unwrap();
        let current: TreeIndex = decode(b"ID12", &read(&root.join(INDEX)).unwrap()).unwrap();
        let prior: KingdomIndex10 = current.prior.into();
        let bytes = if trees {
            encode(
                b"ID11",
                &TreeIndex11 {
                    prior,
                    trees: current.trees,
                },
            )
            .unwrap()
        } else {
            encode(b"ID10", &prior).unwrap()
        };
        fs::write(root.join(INDEX), bytes).unwrap();
        let loaded = load_edits(&root, world).unwrap();
        assert_eq!(loaded.gameplay(), journal.gameplay());
        assert!(
            loaded
                .gameplay()
                .kingdoms
                .kingdoms
                .iter()
                .all(|k| k.parent.is_none())
        );
        assert_eq!(
            loaded.gameplay().kingdoms.get(ids[1]).unwrap().kind,
            SettlementKind::Village
        );
        assert_eq!(
            loaded.gameplay().kingdoms.get(ids[2]).unwrap().kind,
            SettlementKind::City
        );
        save_edits(&root, world, &loaded).unwrap();
        assert_eq!(&read(&root.join(INDEX)).unwrap()[12..16], b"ID12");
        assert_eq!(
            load_edits(&root, world).unwrap().gameplay(),
            journal.gameplay()
        );
        fs::remove_dir_all(root).unwrap();
    }
}

#[test]
fn id12_roundtrips_real_parented_outpost_and_rejects_invalid_hierarchy_atomically() {
    let root = temporary();
    let world = WorldId(20202);
    let owner = PlayerUuid::from_u128(1);
    let mut journal = EditJournal::default();
    for p in [[0, 20, 0], [1024, 20, 0]] {
        journal.record(p, terra_core::castle::BERGFRIED).unwrap();
    }
    let registry = &mut journal.gameplay_mut().kingdoms;
    let main = registry
        .found_settlement(
            owner,
            "Alice",
            [0, 20, 0],
            "Crown",
            SettlementKind::Kingdom,
            Default::default(),
        )
        .unwrap();
    let child = registry
        .found_settlement_with_rules(
            owner,
            "Alice",
            [1024, 20, 0],
            "Watch",
            SettlementKind::Outpost,
            Default::default(),
            &KingdomRules::default(),
            &[],
        )
        .unwrap();
    assert_eq!(registry.get(child).unwrap().parent, Some(main));
    save_edits(&root, world, &journal).unwrap();
    let mut loaded = load_edits(&root, world).unwrap();
    assert_eq!(loaded.gameplay(), journal.gameplay());
    assert_eq!(
        loaded.gameplay().kingdoms.get(child).unwrap().kind,
        SettlementKind::Outpost
    );
    let bytes = read(&root.join(INDEX)).unwrap();
    loaded
        .gameplay_mut()
        .kingdoms
        .kingdoms
        .iter_mut()
        .find(|k| k.id == child)
        .unwrap()
        .parent = Some(child);
    assert!(save_edits(&root, world, &loaded).is_err());
    assert_eq!(read(&root.join(INDEX)).unwrap(), bytes);
    let mut corrupt: TreeIndex = decode(b"ID12", &bytes).unwrap();
    corrupt.prior.kingdoms = loaded.gameplay().kingdoms.clone();
    fs::write(root.join(INDEX), encode(b"ID12", &corrupt).unwrap()).unwrap();
    assert!(load_edits(&root, world).is_err());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn frozen_three_field_rule_toml_defaults_new_policy_and_real_save_preserves_custom_policy() {
    // Rules were persisted in TOML only; no ID*/PLY* format embeds this struct.
    #[derive(Serialize)]
    struct WorldRulesBefore202 {
        game_rules: terra_core::GameRules,
        seasons: terra_core::SeasonConfig,
        time: terra_core::WorldTimeConfig,
    }
    let root = temporary();
    let config = terra_core::WorldConfig {
        id: WorldId(20203),
        ..Default::default()
    };
    let old = WorldRulesBefore202 {
        game_rules: config.game_rules,
        seasons: config.seasons,
        time: config.time,
    };
    let text = toml::to_string(&old).unwrap();
    let migrated: terra_core::WorldRuleSettings = toml::from_str(&text).unwrap();
    assert_eq!(migrated.kingdom_rules, KingdomRules::default());
    terra_core::save_toml(
        &crate::WorldManifest::new(config.clone()),
        root.join("world.toml"),
    )
    .unwrap();
    let mut rules = migrated;
    rules.kingdom_rules.require_adjacent_claims = false;
    rules.kingdom_rules.realm_min_border_gap = 7;
    rules.kingdom_rules.outposts_per_level = 3;
    rules.kingdom_rules.settlements_follow_kingdom_level = true;
    let mut journal = EditJournal::default();
    journal.world_rules = Some(rules);
    save_edits(&root, config.id, &journal).unwrap();
    let saved: crate::WorldManifest =
        toml::from_str(&fs::read_to_string(root.join("world.toml")).unwrap()).unwrap();
    assert_eq!(
        terra_core::WorldRuleSettings::from_world(&saved.world),
        rules
    );
    assert_eq!(saved.world.seed, config.seed);
    assert_eq!(saved.world.ecology_revision, config.ecology_revision);
    assert!(load_edits(&root, config.id).is_ok());
    fs::remove_dir_all(root).unwrap();
}
