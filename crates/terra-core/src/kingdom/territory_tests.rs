use super::*;

fn player(n: u128) -> PlayerUuid {
    PlayerUuid::from_u128(n)
}
fn position(chunk: [i64; 2]) -> [i64; 3] {
    [chunk[0] * 16, 20, chunk[1] * 16]
}
fn found(
    registry: &mut KingdomRegistry,
    actor: PlayerUuid,
    chunk: [i64; 2],
    kind: SettlementKind,
    rules: &KingdomRules,
    domains: &[SettlementDomain],
) -> Result<KingdomId, KingdomError> {
    registry.found_settlement_with_rules(
        actor,
        "Founder",
        position(chunk),
        &format!("{} {}", kind.key(), registry.next_id),
        kind,
        Default::default(),
        rules,
        domains,
    )
}
fn root() -> (KingdomRegistry, KingdomId, KingdomRules) {
    let rules = KingdomRules::default();
    let mut registry = KingdomRegistry::default();
    let id = found(
        &mut registry,
        player(1),
        [0, 0],
        SettlementKind::Kingdom,
        &rules,
        &[],
    )
    .unwrap();
    (registry, id, rules)
}
fn claim(
    registry: &mut KingdomRegistry,
    id: KingdomId,
    chunk: [i64; 2],
    kind: ClaimKind,
    rules: &KingdomRules,
) -> Result<(), KingdomError> {
    registry.claim_with_rules(id, chunk, kind, ClaimActor::Player(player(1)), rules, &[])
}
fn territory(registry: &mut KingdomRegistry, id: KingdomId, end: i64, rules: &KingdomRules) {
    let changes = (1..=end)
        .map(|x| ([x, 0], ClaimKind::RealmTerritory))
        .collect::<Vec<_>>();
    registry
        .claim_batch_with_rules(id, &changes, ClaimActor::Player(player(1)), rules, &[])
        .unwrap();
}

#[test]
fn default_limits_match_each_settlement_kind_and_level() {
    let rules = KingdomRules::default();
    for (kind, base, level3) in [
        (SettlementKind::Kingdom, (32, 64), (44, 96)),
        (SettlementKind::Outpost, (10, 22), (14, 34)),
        (SettlementKind::Village, (10, 0), (14, 0)),
        (SettlementKind::City, (20, 0), (28, 0)),
    ] {
        let a = rules.limits(kind, 1);
        let b = rules.limits(kind, 3);
        assert_eq!((a.protected, a.territory), base);
        assert_eq!((b.protected, b.territory), level3);
    }
    assert!(rules.valid());
    assert_eq!(rules.outposts_per_level, 1);
    assert_eq!(rules.realm_min_border_gap, 16);
    assert_eq!(rules.city_min_build_gap, 10);
    assert_eq!(rules.city_min_kingdom_level, 3);
}

#[test]
fn founding_and_claim_rejections_distinguish_missing_membership_from_missing_authority() {
    let (mut registry, id, rules) = root();
    for operation in [
        registry.claim_with_rules(
            id,
            [1, 0],
            ClaimKind::RealmTerritory,
            ClaimActor::Player(player(2)),
            &rules,
            &[],
        ),
        registry.release_with_rules(player(2), id, [1, 0], &rules),
    ] {
        assert_eq!(operation, Err(KingdomError::NotMember));
    }
    assert_eq!(
        found(
            &mut registry,
            player(2),
            [17, 0],
            SettlementKind::Outpost,
            &rules,
            &[]
        ),
        Err(KingdomError::ParentKingdomRequired)
    );
    registry.join(player(2), id).unwrap();
    assert_eq!(
        registry.claim_with_rules(
            id,
            [1, 0],
            ClaimKind::RealmTerritory,
            ClaimActor::Player(player(2)),
            &rules,
            &[]
        ),
        Err(KingdomError::PermissionDenied)
    );
    assert_eq!(
        found(
            &mut registry,
            player(2),
            [17, 0],
            SettlementKind::Outpost,
            &rules,
            &[]
        ),
        Err(KingdomError::PermissionDenied)
    );
    assert!(registry.valid());
}

#[test]
fn royal_free_border_is_symmetric_and_bad_batches_never_partially_commit() {
    let (mut registry, id, rules) = root();
    let before = registry.clone();
    assert_eq!(
        found(
            &mut registry,
            player(2),
            [16, 0],
            SettlementKind::Kingdom,
            &rules,
            &[]
        ),
        Err(KingdomError::RealmBorderTooClose)
    );
    assert_eq!(registry, before);
    found(
        &mut registry,
        player(2),
        [17, 0],
        SettlementKind::Kingdom,
        &rules,
        &[],
    )
    .unwrap();
    claim(
        &mut registry,
        id,
        [0, 1],
        ClaimKind::ProtectedBuilding,
        &rules,
    )
    .unwrap();
    let before = registry.clone();
    assert_eq!(
        claim(
            &mut registry,
            id,
            [1, 0],
            ClaimKind::ProtectedBuilding,
            &rules
        ),
        Err(KingdomError::RealmBorderTooClose)
    );
    assert_eq!(registry, before);
    assert_eq!(
        registry.claim_batch_with_rules(
            id,
            &[
                ([0, 2], ClaimKind::RealmTerritory),
                ([1, 0], ClaimKind::ProtectedBuilding)
            ],
            ClaimActor::Player(player(1)),
            &rules,
            &[]
        ),
        Err(KingdomError::RealmBorderTooClose)
    );
    assert_eq!(
        registry, before,
        "the first valid tile in a rejected batch must not be claimed"
    );
    assert!(registry.valid());
}

#[test]
fn native_realm_clearance_uses_full_bounds_and_not_a_self_created_capture_exemption() {
    let domain = SettlementDomain {
        id: 7202,
        center: position([20, 0]),
        bounds: [position([10, 0]), position([30, 0])],
        kind: DomainKind::Kingdom,
        native: Some(BotKingdom::Lutera),
    };
    assert!(domain.valid());
    let rules = KingdomRules::default();
    let mut registry = KingdomRegistry::default();
    for chunk in [[20, 0], [45, 0]] {
        let before = registry.clone();
        assert_eq!(
            found(
                &mut registry,
                player(1),
                chunk,
                SettlementKind::Kingdom,
                &rules,
                &[domain]
            ),
            Err(KingdomError::RealmBorderTooClose)
        );
        assert_eq!(registry, before);
    }
    let id = found(
        &mut registry,
        player(1),
        [47, 0],
        SettlementKind::Kingdom,
        &rules,
        &[domain],
    )
    .unwrap();
    assert_eq!(registry.get(id).unwrap().location, position([47, 0]));
    let before = registry.clone();
    assert_eq!(
        registry.claim_with_rules(
            id,
            [46, 0],
            ClaimKind::RealmTerritory,
            ClaimActor::Player(player(1)),
            &rules,
            &[domain]
        ),
        Err(KingdomError::RealmBorderTooClose)
    );
    assert_eq!(registry, before);
}

#[test]
fn connected_claims_can_extend_far_beyond_the_keep_and_policy_changes_are_live() {
    let (mut registry, id, mut rules) = root();
    territory(&mut registry, id, 40, &rules);
    assert!(registry.claim_at([40, 0]).is_some());
    let before = registry.clone();
    assert_eq!(
        claim(
            &mut registry,
            id,
            [50, 0],
            ClaimKind::RealmTerritory,
            &rules
        ),
        Err(KingdomError::ClaimDisconnected)
    );
    assert_eq!(registry, before);
    registry
        .claim_batch_with_rules(
            id,
            &(41..=50)
                .map(|x| ([x, 0], ClaimKind::RealmTerritory))
                .collect::<Vec<_>>(),
            ClaimActor::Player(player(1)),
            &rules,
            &[],
        )
        .unwrap();
    assert!(
        registry.can_build(player(9), position([50, 0])),
        "ordinary territory keeps open terrain access"
    );
    rules.require_adjacent_claims = false;
    claim(
        &mut registry,
        id,
        [700, 0],
        ClaimKind::RealmTerritory,
        &rules,
    )
    .unwrap();
    assert!(registry.claim_at([700, 0]).is_some());
    assert!(
        registry.valid(),
        "configured nonadjacent land remains persistable"
    );
    rules.require_adjacent_claims = true;
    claim(
        &mut registry,
        id,
        [51, 0],
        ClaimKind::RealmTerritory,
        &rules,
    )
    .unwrap();
    let before = registry.clone();
    assert_eq!(
        claim(
            &mut registry,
            id,
            [701, 0],
            ClaimKind::RealmTerritory,
            &rules
        ),
        Err(KingdomError::ClaimDisconnected)
    );
    assert_eq!(
        registry, before,
        "switching policy retains historical islands without permitting their expansion"
    );
    registry
        .release_with_rules(player(1), id, [700, 0], &rules)
        .unwrap();
    claim(
        &mut registry,
        id,
        [52, 0],
        ClaimKind::RealmTerritory,
        &rules,
    )
    .unwrap();
    assert!(registry.valid());
}

#[test]
fn outposts_are_counted_per_root_level_and_use_their_own_claim_limits() {
    let (mut registry, id, rules) = root();
    let outpost = found(
        &mut registry,
        player(1),
        [17, 0],
        SettlementKind::Outpost,
        &rules,
        &[],
    )
    .unwrap();
    assert_eq!(registry.get(outpost).unwrap().parent, Some(id));
    let before = registry.clone();
    assert_eq!(
        found(
            &mut registry,
            player(1),
            [34, 0],
            SettlementKind::Outpost,
            &rules,
            &[]
        ),
        Err(KingdomError::OutpostLimitReached)
    );
    assert_eq!(registry, before);
    registry
        .kingdoms
        .iter_mut()
        .find(|k| k.id == id)
        .unwrap()
        .level = 2;
    found(
        &mut registry,
        player(1),
        [34, 0],
        SettlementKind::Outpost,
        &rules,
        &[],
    )
    .unwrap();
    assert_eq!(
        found(
            &mut registry,
            player(1),
            [51, 0],
            SettlementKind::Outpost,
            &rules,
            &[]
        ),
        Err(KingdomError::OutpostLimitReached)
    );
    let limits = registry.limits_with_rules(outpost, &rules);
    assert_eq!((limits.protected, limits.territory), (10, 22));
    let changes = (18..=39)
        .map(|x| ([17, x], ClaimKind::RealmTerritory))
        .collect::<Vec<_>>();
    let mut loose = rules;
    loose.require_adjacent_claims = false;
    registry
        .claim_batch_with_rules(
            outpost,
            &changes,
            ClaimActor::Player(player(1)),
            &loose,
            &[],
        )
        .unwrap();
    let before = registry.clone();
    assert_eq!(
        claim(
            &mut registry,
            outpost,
            [17, 40],
            ClaimKind::RealmTerritory,
            &loose
        ),
        Err(KingdomError::ClaimLimitReached)
    );
    assert_eq!(registry, before);
    registry.dissolve(player(1), outpost).unwrap();
    found(
        &mut registry,
        player(1),
        [51, 0],
        SettlementKind::Outpost,
        &rules,
        &[],
    )
    .unwrap();
    assert!(registry.valid());
}

#[test]
fn villages_overlay_family_territory_including_outposts_but_cannot_claim_territory_themselves() {
    let (mut registry, id, rules) = root();
    claim(&mut registry, id, [1, 0], ClaimKind::RealmTerritory, &rules).unwrap();
    let village = found(
        &mut registry,
        player(1),
        [1, 0],
        SettlementKind::Village,
        &rules,
        &[],
    )
    .unwrap();
    assert_eq!(
        registry.claims.iter().filter(|c| c.chunk == [1, 0]).count(),
        2
    );
    assert_eq!(registry.claim_at([1, 0]).unwrap().kingdom, village);
    let before = registry.clone();
    assert_eq!(
        claim(
            &mut registry,
            village,
            [2, 0],
            ClaimKind::ProtectedBuilding,
            &rules
        ),
        Err(KingdomError::ParentTerritoryRequired)
    );
    assert_eq!(
        claim(
            &mut registry,
            village,
            [2, 0],
            ClaimKind::RealmTerritory,
            &rules
        ),
        Err(KingdomError::SettlementTerritoryForbidden)
    );
    assert_eq!(registry, before);
    claim(&mut registry, id, [2, 0], ClaimKind::RealmTerritory, &rules).unwrap();
    claim(
        &mut registry,
        village,
        [2, 0],
        ClaimKind::ProtectedBuilding,
        &rules,
    )
    .unwrap();
    assert_eq!(
        registry.release_with_rules(player(1), id, [2, 0], &rules),
        Err(KingdomError::ParentTerritoryInUse)
    );
    let outpost = found(
        &mut registry,
        player(1),
        [19, 0],
        SettlementKind::Outpost,
        &rules,
        &[],
    )
    .unwrap();
    claim(
        &mut registry,
        outpost,
        [20, 0],
        ClaimKind::RealmTerritory,
        &rules,
    )
    .unwrap();
    let distant_village = found(
        &mut registry,
        player(1),
        [20, 0],
        SettlementKind::Village,
        &rules,
        &[],
    )
    .unwrap();
    assert_eq!(registry.get(distant_village).unwrap().parent, Some(id));
    assert_eq!(
        registry.release_with_rules(player(1), outpost, [20, 0], &rules),
        Err(KingdomError::ParentTerritoryInUse)
    );
    registry.dissolve(player(1), distant_village).unwrap();
    registry
        .release_with_rules(player(1), outpost, [20, 0], &rules)
        .unwrap();
    assert!(registry.valid());
}

#[test]
fn cities_require_root_level_three_and_ten_free_build_chunks_in_both_expansion_directions() {
    let (mut registry, id, rules) = root();
    territory(&mut registry, id, 30, &rules);
    let before = registry.clone();
    assert_eq!(
        found(
            &mut registry,
            player(1),
            [11, 0],
            SettlementKind::City,
            &rules,
            &[]
        ),
        Err(KingdomError::KingdomLevelRequired)
    );
    assert_eq!(registry, before);
    registry
        .kingdoms
        .iter_mut()
        .find(|k| k.id == id)
        .unwrap()
        .level = 3;
    assert_eq!(
        found(
            &mut registry,
            player(1),
            [10, 0],
            SettlementKind::City,
            &rules,
            &[]
        ),
        Err(KingdomError::CityBorderTooClose)
    );
    let city = found(
        &mut registry,
        player(1),
        [11, 0],
        SettlementKind::City,
        &rules,
        &[],
    )
    .unwrap();
    let village = found(
        &mut registry,
        player(1),
        [22, 0],
        SettlementKind::Village,
        &rules,
        &[],
    )
    .unwrap();
    for (who, chunk) in [(id, [1, 0]), (village, [21, 0]), (city, [12, 0])] {
        let before = registry.clone();
        assert_eq!(
            claim(
                &mut registry,
                who,
                chunk,
                ClaimKind::ProtectedBuilding,
                &rules
            ),
            Err(KingdomError::CityBorderTooClose)
        );
        assert_eq!(registry, before);
    }
    assert_eq!(
        claim(
            &mut registry,
            city,
            [31, 0],
            ClaimKind::RealmTerritory,
            &rules
        ),
        Err(KingdomError::SettlementTerritoryForbidden)
    );
    claim(
        &mut registry,
        id,
        [0, 1],
        ClaimKind::ProtectedBuilding,
        &rules,
    )
    .unwrap();
    assert!(registry.valid());
}

#[test]
fn dependent_levels_can_be_independent_or_inherited_without_silently_charging_an_inherited_upgrade()
{
    let (mut registry, id, mut rules) = root();
    let child = found(
        &mut registry,
        player(1),
        [17, 0],
        SettlementKind::Outpost,
        &rules,
        &[],
    )
    .unwrap();
    let mut game = crate::GameplayState {
        kingdoms: registry,
        ..Default::default()
    };
    let bag = game.inventories.entry(player(1)).or_default();
    bag.insert(crate::ItemKind::Block(crate::castle::STONE_WALL), 64);
    bag.insert(crate::ItemKind::Mineral(crate::MineralItem::GoldIngot), 8);
    assert_eq!(
        game.upgrade_kingdom_with_rules(player(1), child, &rules),
        Ok(2)
    );
    assert_eq!(
        game.upgrade_kingdom_with_rules(player(1), id, &rules),
        Ok(2)
    );
    assert_eq!(
        game.upgrade_kingdom_with_rules(player(1), id, &rules),
        Ok(3)
    );
    assert_eq!(game.kingdoms.limits_with_rules(child, &rules).protected, 12);
    rules.settlements_follow_kingdom_level = true;
    assert_eq!(game.kingdoms.limits_with_rules(child, &rules).protected, 14);
    let before = game.clone();
    assert_eq!(
        game.upgrade_kingdom_with_rules(player(1), child, &rules),
        Err(KingdomError::SettlementLevelInherited)
    );
    assert_eq!(game, before);
    assert_eq!(game.kingdoms.get(child).unwrap().level, 2);
    rules.outpost_build_base = 100;
    assert_eq!(
        game.kingdoms.limits_with_rules(child, &rules).protected,
        104
    );
    assert!(game.kingdoms.valid());
}

#[test]
fn family_roles_members_heraldry_and_transport_follow_live_parent_state() {
    let (mut registry, id, rules) = root();
    claim(&mut registry, id, [1, 0], ClaimKind::RealmTerritory, &rules).unwrap();
    let village = found(
        &mut registry,
        player(1),
        [1, 0],
        SettlementKind::Village,
        &rules,
        &[],
    )
    .unwrap();
    let outpost = found(
        &mut registry,
        player(1),
        [18, 0],
        SettlementKind::Outpost,
        &rules,
        &[],
    )
    .unwrap();
    let stranger = found(
        &mut registry,
        player(9),
        [100, 100],
        SettlementKind::Kingdom,
        &rules,
        &[],
    )
    .unwrap();
    registry
        .join_with_name(player(2), "Member", outpost)
        .unwrap();
    for child in [village, outpost] {
        assert_eq!(
            registry.role_for(child, player(2)),
            Some(KingdomRole::Visitor)
        );
        assert!(!registry.can(
            player(2),
            KingdomPermission::Build,
            registry.get(child).unwrap().location
        ));
    }
    registry
        .set_role(player(1), village, player(2), KingdomRole::Worker)
        .unwrap();
    for child in [village, outpost] {
        assert_eq!(
            registry.role_for(child, player(2)),
            Some(KingdomRole::Worker)
        );
        assert!(registry.can(
            player(2),
            KingdomPermission::NpcWork,
            registry.get(child).unwrap().location
        ));
        assert!(!registry.can_manage(player(2), child));
    }
    registry
        .set_role(player(1), outpost, player(2), KingdomRole::Officer)
        .unwrap();
    for child in [id, village, outpost] {
        assert!(registry.can_manage(player(2), child));
        assert!(!registry.can_set_role(player(2), child, player(2), KingdomRole::Founder));
        assert!(!registry.can_set_role(player(2), child, player(1), KingdomRole::Visitor));
    }
    let heraldry = KingdomHeraldry {
        color: [7, 31, 99],
        icon: KingdomIcon::Star,
    };
    registry
        .change_appearance(player(1), outpost, heraldry)
        .unwrap();
    for a in [id, village, outpost] {
        assert_eq!(registry.get(a).unwrap().heraldry, heraldry);
        for b in [id, village, outpost] {
            assert!(registry.can_transport_between(
                Some(DiplomaticTarget::Player(a)),
                Some(DiplomaticTarget::Player(b))
            ));
        }
        assert!(!registry.can_transport_between(
            Some(DiplomaticTarget::Player(a)),
            Some(DiplomaticTarget::Player(stranger))
        ));
        assert!(!registry.can_transport_between(Some(DiplomaticTarget::Player(a)), None));
    }
    registry.kick(player(1), village, player(2)).unwrap();
    for child in [id, village, outpost] {
        assert_eq!(registry.role_for(child, player(2)), None);
        assert!(!registry.get(child).unwrap().members.contains(&player(2)));
    }
    assert!(registry.valid());
}
