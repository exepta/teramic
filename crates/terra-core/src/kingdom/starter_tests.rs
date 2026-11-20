use super::*;

fn founding() -> (
    GameplayState,
    KingdomRegistry,
    KingdomId,
    [[f64; 3]; STARTER_COUNT],
) {
    let owner = PlayerUuid::from_u128(80);
    let mut game = GameplayState::default();
    game.inventories
        .entry(owner)
        .or_default()
        .insert(crate::ItemKind::Block(3), 29);
    let mut next = game.kingdoms.clone();
    let id = next
        .found_with_name(owner, "Founder", [8, 2, 8], "First Kingdom")
        .unwrap();
    let positions = std::array::from_fn(|index| {
        [
            2.5 + (index % 3) as f64 * 2.,
            2.,
            2.5 + (index / 3) as f64 * 2.,
        ]
    });
    (game, next, id, positions)
}

#[test]
fn kingdom_founding_grants_exact_owned_roles_and_commits_only_once() {
    let (mut game, next, id, positions) = founding();
    let inventory = game.inventories.clone();
    game.commit_founding_with_starters(next.clone(), id, positions)
        .unwrap();
    assert_eq!(game.revision, 1);
    assert_eq!(game.creatures.animals.len(), STARTER_COUNT);
    assert_eq!(game.creatures.owners.len(), STARTER_COUNT);
    let grant = &game.kingdom_starters.grants[&id];
    assert_eq!(grant.founder, PlayerUuid::from_u128(80));
    for (slot, npc) in grant.ids.iter().enumerate() {
        let creature = &game.creatures.animals[npc];
        assert_eq!(creature.variant, STARTER_ROLES[slot]);
        assert_eq!(game.creatures.owners.get(npc), Some(&grant.founder));
        assert!(creature.valid());
        assert!(!crate::creatures::spawning::is_command_id(*npc));
        assert!(!crate::creatures::breeding::is_birth_id(*npc));
    }
    assert!(game.valid());
    assert_eq!(game.inventories, inventory);
    let committed = game.clone();
    assert_eq!(
        game.commit_founding_with_starters(next, id, positions),
        Err(KingdomError::KeepAlreadyBound)
    );
    assert_eq!(game, committed);
}

#[test]
fn invalid_positions_revision_capacity_and_identity_overflows_are_atomic() {
    for invalid in [f64::NAN, 1_000_001.] {
        let (mut game, next, id, mut positions) = founding();
        positions[17][0] = invalid;
        let before = game.clone();
        assert!(
            game.commit_founding_with_starters(next, id, positions)
                .is_err()
        );
        assert_eq!(game, before);
    }
    let (mut game, next, id, mut positions) = founding();
    positions[17] = positions[0];
    let before = game.clone();
    assert_eq!(
        game.commit_founding_with_starters(next, id, positions),
        Err(KingdomError::StarterAreaUnavailable)
    );
    assert_eq!(game, before);
    let (mut game, next, id, positions) = founding();
    game.revision = u64::MAX;
    let before = game.clone();
    assert_eq!(
        game.commit_founding_with_starters(next, id, positions),
        Err(KingdomError::CapacityReached)
    );
    assert_eq!(game, before);
    let (mut game, _, _, positions) = founding();
    game.kingdoms.next_id = u64::MAX - 1;
    let mut next = game.kingdoms.clone();
    let id = next
        .found(PlayerUuid::from_u128(80), [8, 2, 8], "Overflow")
        .unwrap();
    let before = game.clone();
    assert_eq!(
        game.commit_founding_with_starters(next, id, positions),
        Err(KingdomError::CapacityReached)
    );
    assert_eq!(game, before);
}

#[test]
fn dead_or_removed_starters_remain_receipted_without_replenishment_or_id_reuse() {
    let (mut game, next, id, positions) = founding();
    game.commit_founding_with_starters(next, id, positions)
        .unwrap();
    let first = game.kingdom_starters.grants[&id].ids;
    let dead = game.creatures.animals.get_mut(&first[0]).unwrap();
    dead.health = 0.;
    dead.behavior = CreatureBehavior::Dead;
    game.creatures.animals.remove(&first[1]);
    game.creatures.owners.remove(&first[1]);
    assert!(game.valid());
    let receipt = game.kingdom_starters.clone();
    game.kingdoms
        .dissolve(PlayerUuid::from_u128(80), id)
        .unwrap();
    let mut next = game.kingdoms.clone();
    let second = next
        .found(PlayerUuid::from_u128(80), [8, 2, 8], "Second Kingdom")
        .unwrap();
    game.commit_founding_with_starters(next, second, positions)
        .unwrap();
    assert!(game.valid());
    assert_eq!(game.kingdom_starters.grants[&id], receipt.grants[&id]);
    assert!(
        game.kingdom_starters.grants[&second]
            .ids
            .iter()
            .all(|npc| !first.contains(npc))
    );
    assert_eq!(game.creatures.animals[&first[0]].health, 0.);
    assert!(!game.creatures.animals.contains_key(&first[1]));
}

#[test]
fn legacy_kingdoms_need_no_marker_and_corrupt_receipts_are_rejected() {
    let (mut legacy, next, _, _) = founding();
    legacy.kingdoms = next;
    assert!(legacy.valid());
    assert!(legacy.kingdom_starters.grants.is_empty());
    assert!(legacy.creatures.animals.is_empty());
    let (mut game, next, id, positions) = founding();
    game.commit_founding_with_starters(next, id, positions)
        .unwrap();
    let grant = game.kingdom_starters.grants.get_mut(&id).unwrap();
    grant.ids[1] = grant.ids[0];
    assert!(!game.valid());
    assert_eq!(starter_creature_id(KingdomId(0), 0), None);
    assert_eq!(starter_creature_id(KingdomId(u64::MAX), 0), None);
    assert_eq!(starter_creature_id(KingdomId(1), STARTER_COUNT), None);
}

#[test]
fn seventeen_remaining_creature_slots_reject_the_complete_founding_without_spending_items() {
    let (mut game, next, id, positions) = founding();
    let template = Creature {
        id: (1 << 62) + 1,
        kind: CreatureKind::Villager,
        home: [0; 2],
        position: [1.5, 2., 1.5],
        velocity: [0.; 3],
        yaw: 0.,
        health: CreatureKind::Villager.health(),
        behavior: CreatureBehavior::Rest,
        timer: 0.,
        age: 0.,
        variant: villagers::UNEMPLOYED,
        needs: CreatureNeeds::full(CreatureKind::Villager),
    };
    for serial in 1..=(MAX_CREATURES - STARTER_COUNT + 1) {
        let id = (1 << 62) + serial as u64;
        game.creatures.animals.insert(
            id,
            Creature {
                id,
                ..template.clone()
            },
        );
    }
    assert!(game.valid());
    let before = game.clone();
    assert_eq!(
        game.commit_founding_with_starters(next, id, positions),
        Err(KingdomError::CapacityReached)
    );
    assert_eq!(game, before);
}

#[test]
fn live_and_dead_starter_ownership_must_match_the_persistent_founder() {
    let (mut game, next, id, positions) = founding();
    game.commit_founding_with_starters(next, id, positions)
        .unwrap();
    let npc = game.kingdom_starters.grants[&id].ids[0];
    game.creatures.owners.insert(npc, PlayerUuid::from_u128(99));
    assert!(!game.valid());
    let creature = game.creatures.animals.get_mut(&npc).unwrap();
    creature.health = 0.;
    creature.behavior = CreatureBehavior::Dead;
    assert!(!game.valid());
}

#[test]
fn starter_homes_cover_distant_legal_claims_without_expanding_legacy_habitat_ids() {
    for keep in [[900_008, 2, -900_008], [-900_008, 2, 900_008]] {
        let mut game = GameplayState::default();
        let mut next = game.kingdoms.clone();
        let id = next
            .found(PlayerUuid::from_u128(80), keep, "Distant Kingdom")
            .unwrap();
        let home = super::super::claim_chunk(keep);
        assert!(
            home.iter()
                .any(|coordinate| coordinate.unsigned_abs() > 31_250)
        );
        let positions = std::array::from_fn(|i| {
            [
                (home[0] * super::super::CLAIM_CHUNK_SIZE) as f64 + 2.5 + (i % 3) as f64 * 2.,
                2.,
                (home[1] * super::super::CLAIM_CHUNK_SIZE) as f64 + 2.5 + (i / 3) as f64 * 2.,
            ]
        });
        game.commit_founding_with_starters(next, id, positions)
            .unwrap();
        assert!(game.valid());
        assert!(
            game.creatures
                .animals
                .values()
                .all(|creature| creature.home == home && creature.valid())
        );
        let mut legacy = game.creatures.animals.values().next().unwrap().clone();
        legacy.id = (1 << 62) + 1;
        assert!(
            !legacy.valid(),
            "Command/natural habitat bounds must remain unchanged"
        );
        let mut outside = game.creatures.animals.values().next().unwrap().clone();
        outside.home = [61_876, 0];
        assert!(
            !outside.valid(),
            "Starter homes cannot escape the claimable world"
        );
    }
}
