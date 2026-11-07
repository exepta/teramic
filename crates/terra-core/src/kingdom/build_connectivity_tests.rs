use super::*;

fn owner() -> PlayerUuid {
    PlayerUuid::from_u128(203)
}

fn fixture() -> (KingdomRegistry, KingdomId, KingdomRules) {
    let mut registry = KingdomRegistry::default();
    let rules = KingdomRules::default();
    let kingdom = registry
        .found_settlement_with_rules(
            owner(),
            "Builder",
            [0, 20, 0],
            "Connected Build Land",
            SettlementKind::Kingdom,
            KingdomHeraldry::default(),
            &rules,
            &[],
        )
        .unwrap();
    (registry, kingdom, rules)
}

fn apply(
    registry: &mut KingdomRegistry,
    kingdom: KingdomId,
    changes: &[([i64; 2], ClaimKind)],
    rules: &KingdomRules,
) -> Result<u16, KingdomError> {
    registry.claim_batch_with_rules(kingdom, changes, ClaimActor::Player(owner()), rules, &[])
}

#[test]
fn territory_cannot_bridge_a_build_island_or_a_diagonal_build_neighbor() {
    let (mut registry, kingdom, rules) = fixture();
    apply(
        &mut registry,
        kingdom,
        &[([1, 0], ClaimKind::RealmTerritory)],
        &rules,
    )
    .unwrap();
    for chunk in [[2, 0], [1, 1]] {
        let before = registry.clone();
        assert_eq!(
            apply(
                &mut registry,
                kingdom,
                &[(chunk, ClaimKind::ProtectedBuilding)],
                &rules
            ),
            Err(KingdomError::ClaimDisconnected)
        );
        assert_eq!(registry, before);
    }
    apply(
        &mut registry,
        kingdom,
        &[([0, 1], ClaimKind::ProtectedBuilding)],
        &rules,
    )
    .unwrap();
    apply(
        &mut registry,
        kingdom,
        &[([1, 1], ClaimKind::ProtectedBuilding)],
        &rules,
    )
    .unwrap();
    assert!(registry.valid());
}

#[test]
fn a_mixed_batch_requires_a_complete_build_path_in_its_final_state() {
    let (mut registry, kingdom, rules) = fixture();
    let before = registry.clone();
    assert_eq!(
        apply(
            &mut registry,
            kingdom,
            &[
                ([3, 0], ClaimKind::ProtectedBuilding),
                ([1, 0], ClaimKind::RealmTerritory),
                ([2, 0], ClaimKind::ProtectedBuilding),
            ],
            &rules
        ),
        Err(KingdomError::ClaimDisconnected)
    );
    assert_eq!(registry, before);
    assert_eq!(
        apply(
            &mut registry,
            kingdom,
            &[
                ([3, 0], ClaimKind::ProtectedBuilding),
                ([1, 0], ClaimKind::ProtectedBuilding),
                ([2, 0], ClaimKind::ProtectedBuilding),
            ],
            &rules
        ),
        Ok(3)
    );
    assert!(registry.valid());
}

fn loop_fixture() -> (KingdomRegistry, KingdomId, KingdomRules) {
    let (mut registry, kingdom, rules) = fixture();
    apply(
        &mut registry,
        kingdom,
        &[
            ([1, 0], ClaimKind::ProtectedBuilding),
            ([2, 0], ClaimKind::ProtectedBuilding),
            ([0, 1], ClaimKind::RealmTerritory),
            ([1, 1], ClaimKind::RealmTerritory),
            ([2, 1], ClaimKind::RealmTerritory),
        ],
        &rules,
    )
    .unwrap();
    (registry, kingdom, rules)
}

#[test]
fn releasing_or_converting_a_build_bridge_is_atomic_even_with_a_territory_detour() {
    let (mut registry, kingdom, rules) = loop_fixture();
    let before = registry.clone();
    assert_eq!(
        apply(
            &mut registry,
            kingdom,
            &[([1, 0], ClaimKind::RealmTerritory)],
            &rules
        ),
        Err(KingdomError::ClaimDisconnected)
    );
    assert_eq!(registry, before);
    assert_eq!(
        registry.release_with_rules(owner(), kingdom, [1, 0], &rules),
        Err(KingdomError::ClaimDisconnected)
    );
    assert_eq!(registry, before);
    registry
        .release_with_rules(owner(), kingdom, [2, 0], &rules)
        .unwrap();
    apply(
        &mut registry,
        kingdom,
        &[([1, 0], ClaimKind::RealmTerritory)],
        &rules,
    )
    .unwrap();
    assert!(registry.valid());
}

#[test]
fn a_batch_can_replace_a_build_bridge_with_an_alternative_build_path() {
    let (mut registry, kingdom, rules) = loop_fixture();
    assert_eq!(
        apply(
            &mut registry,
            kingdom,
            &[
                ([1, 0], ClaimKind::RealmTerritory),
                ([2, 1], ClaimKind::ProtectedBuilding),
                ([0, 1], ClaimKind::ProtectedBuilding),
                ([1, 1], ClaimKind::ProtectedBuilding),
            ],
            &rules
        ),
        Ok(4)
    );
    assert_eq!(
        registry.claim_at([2, 0]).unwrap().kind,
        ClaimKind::ProtectedBuilding
    );
    assert_eq!(
        registry.claim_at([1, 0]).unwrap().kind,
        ClaimKind::RealmTerritory
    );
    assert!(registry.valid());
}

#[test]
fn territory_may_extend_from_build_land_and_from_territory() {
    let (mut registry, kingdom, rules) = fixture();
    apply(
        &mut registry,
        kingdom,
        &[([1, 0], ClaimKind::ProtectedBuilding)],
        &rules,
    )
    .unwrap();
    apply(
        &mut registry,
        kingdom,
        &[
            ([2, 1], ClaimKind::RealmTerritory),
            ([1, 1], ClaimKind::RealmTerritory),
        ],
        &rules,
    )
    .unwrap();
    apply(
        &mut registry,
        kingdom,
        &[([3, 1], ClaimKind::RealmTerritory)],
        &rules,
    )
    .unwrap();
    assert!(registry.valid());
}

#[test]
fn historical_build_islands_are_preserved_but_cannot_expand_and_can_be_repaired() {
    let (mut registry, kingdom, rules) = fixture();
    registry.claims.push(KingdomClaim {
        kingdom,
        chunk: [10, 0],
        kind: ClaimKind::ProtectedBuilding,
        actor: ClaimActor::Player(owner()),
    });
    assert!(registry.valid());
    let decoded: KingdomRegistry =
        serde_json::from_str(&serde_json::to_string(&registry).unwrap()).unwrap();
    assert_eq!(decoded, registry);
    assert!(decoded.valid());
    apply(
        &mut registry,
        kingdom,
        &[([0, 1], ClaimKind::ProtectedBuilding)],
        &rules,
    )
    .unwrap();
    let before = registry.clone();
    assert_eq!(
        apply(
            &mut registry,
            kingdom,
            &[([10, 1], ClaimKind::ProtectedBuilding)],
            &rules
        ),
        Err(KingdomError::ClaimDisconnected)
    );
    assert_eq!(registry, before);
    let territory = (1..10)
        .map(|x| ([x, 0], ClaimKind::RealmTerritory))
        .collect::<Vec<_>>();
    apply(&mut registry, kingdom, &territory, &rules).unwrap();
    let repair = (1..10)
        .rev()
        .map(|x| ([x, 0], ClaimKind::ProtectedBuilding))
        .collect::<Vec<_>>();
    apply(&mut registry, kingdom, &repair, &rules).unwrap();
    apply(
        &mut registry,
        kingdom,
        &[([10, 1], ClaimKind::ProtectedBuilding)],
        &rules,
    )
    .unwrap();
    assert!(registry.valid());
}

#[test]
fn an_explicit_disabled_adjacency_policy_allows_disconnected_build_claims() {
    let (mut registry, kingdom, rules) = fixture();
    let unrestricted = KingdomRules {
        require_adjacent_claims: false,
        ..rules
    };
    apply(
        &mut registry,
        kingdom,
        &[([20, 0], ClaimKind::ProtectedBuilding)],
        &unrestricted,
    )
    .unwrap();
    let before = registry.clone();
    assert_eq!(
        apply(
            &mut registry,
            kingdom,
            &[([21, 0], ClaimKind::ProtectedBuilding)],
            &rules
        ),
        Err(KingdomError::ClaimDisconnected)
    );
    assert_eq!(registry, before);
    registry
        .release_with_rules(owner(), kingdom, [20, 0], &unrestricted)
        .unwrap();
    assert!(registry.valid());
}

#[test]
fn each_dependent_village_uses_its_own_build_seat_and_not_parent_territory() {
    let (mut registry, kingdom, rules) = fixture();
    let territory = (1..=5)
        .map(|x| ([x, 0], ClaimKind::RealmTerritory))
        .collect::<Vec<_>>();
    apply(&mut registry, kingdom, &territory, &rules).unwrap();
    let village = registry
        .found_settlement_with_rules(
            owner(),
            "Builder",
            [4 * 16, 20, 0],
            "Village Seat",
            SettlementKind::Village,
            KingdomHeraldry::default(),
            &rules,
            &[],
        )
        .unwrap();
    let before = registry.clone();
    assert_eq!(
        apply(
            &mut registry,
            village,
            &[([2, 0], ClaimKind::ProtectedBuilding)],
            &rules
        ),
        Err(KingdomError::ClaimDisconnected)
    );
    assert_eq!(registry, before);
    apply(
        &mut registry,
        village,
        &[([3, 0], ClaimKind::ProtectedBuilding)],
        &rules,
    )
    .unwrap();
    apply(
        &mut registry,
        village,
        &[([2, 0], ClaimKind::ProtectedBuilding)],
        &rules,
    )
    .unwrap();
    assert!(registry.valid());
}

#[test]
fn legacy_claim_entry_points_apply_the_same_build_only_rule() {
    let (mut registry, kingdom, _) = fixture();
    let actor = ClaimActor::Player(owner());
    registry
        .claim(kingdom, [1, 0], ClaimKind::RealmTerritory, actor)
        .unwrap();
    let before = registry.clone();
    assert_eq!(
        registry.claim(kingdom, [2, 0], ClaimKind::ProtectedBuilding, actor),
        Err(KingdomError::ClaimDisconnected)
    );
    assert_eq!(registry, before);
    assert_eq!(
        registry.claim_batch(
            kingdom,
            &[
                ([1, 1], ClaimKind::RealmTerritory),
                ([2, 1], ClaimKind::ProtectedBuilding),
            ],
            actor
        ),
        Err(KingdomError::ClaimDisconnected)
    );
    assert_eq!(registry, before);
}

#[test]
fn legacy_release_and_conversion_cannot_cut_a_build_bridge_through_territory() {
    let (mut registry, kingdom, _) = loop_fixture();
    let before = registry.clone();
    assert_eq!(
        registry.claim(
            kingdom,
            [1, 0],
            ClaimKind::RealmTerritory,
            ClaimActor::Player(owner())
        ),
        Err(KingdomError::ClaimDisconnected)
    );
    assert_eq!(registry, before);
    assert_eq!(
        registry.release(owner(), kingdom, [1, 0]),
        Err(KingdomError::ClaimDisconnected)
    );
    assert_eq!(registry, before);
}
