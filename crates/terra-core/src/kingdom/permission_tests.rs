use super::*;
fn uuid(n: u128) -> PlayerUuid {
    PlayerUuid::from_u128(n)
}
fn realm(registry: &mut KingdomRegistry, n: u128, x: i64) -> KingdomId {
    registry
        .found(uuid(n), [x, 20, 0], &format!("Realm {n}"))
        .unwrap()
}
#[test]
fn role_matrix_is_authoritative_and_revocation_releases_all_old_access() {
    let mut registry = KingdomRegistry::default();
    let id = realm(&mut registry, 1, 0);
    registry.join(uuid(2), id).unwrap();
    let permissions = [
        KingdomPermission::Build,
        KingdomPermission::Break,
        KingdomPermission::Inventories,
        KingdomPermission::NpcWork,
    ];
    assert_eq!(registry.role_for(id, uuid(1)), Some(KingdomRole::Founder));
    for (role, expected) in [
        (KingdomRole::Visitor, [false, false, false, false]),
        (KingdomRole::Builder, [true, true, true, false]),
        (KingdomRole::Worker, [false, false, true, true]),
        (KingdomRole::Member, [true, true, true, true]),
        (KingdomRole::Officer, [true, true, true, true]),
    ] {
        registry.set_role(uuid(1), id, uuid(2), role).unwrap();
        assert_eq!(
            permissions.map(|p| registry.can(uuid(2), p, [1, 100, 1])),
            expected
        );
        assert_eq!(
            registry.can(uuid(2), KingdomPermission::Openings, [1, 100, 1]),
            expected[2]
        );
    }
    registry.kick(uuid(1), id, uuid(2)).unwrap();
    assert_eq!(registry.role_for(id, uuid(2)), None);
    assert!(
        permissions
            .into_iter()
            .all(|p| !registry.can(uuid(2), p, [1, 20, 1]))
    );
    registry.join(uuid(2), id).unwrap();
    assert_eq!(registry.role_for(id, uuid(2)), Some(KingdomRole::Visitor));
    registry
        .claim(
            id,
            [1, 0],
            ClaimKind::ProtectedBuilding,
            ClaimActor::Player(uuid(1)),
        )
        .unwrap();
    assert!(!registry.can(uuid(2), KingdomPermission::Inventories, [16, 20, 1]));
    registry.release(uuid(1), id, [1, 0]).unwrap();
    assert!(
        permissions
            .into_iter()
            .all(|p| registry.can(uuid(2), p, [16, 20, 1]))
    );
    registry.dissolve(uuid(1), id).unwrap();
    assert!(registry.roles.is_empty());
    assert!(registry.valid());
}
#[test]
fn officers_cannot_promote_themselves_manage_peers_or_remove_the_founder() {
    let mut registry = KingdomRegistry::default();
    let id = realm(&mut registry, 1, 0);
    for p in 2..=4 {
        registry.join(uuid(p), id).unwrap();
    }
    registry
        .set_role(uuid(1), id, uuid(2), KingdomRole::Officer)
        .unwrap();
    registry
        .set_role(uuid(1), id, uuid(3), KingdomRole::Officer)
        .unwrap();
    let before = registry.clone();
    for (target, role) in [
        (1, KingdomRole::Visitor),
        (2, KingdomRole::Builder),
        (3, KingdomRole::Visitor),
        (4, KingdomRole::Officer),
        (4, KingdomRole::Founder),
    ] {
        assert_eq!(
            registry.set_role(uuid(2), id, uuid(target), role),
            Err(KingdomError::PermissionDenied)
        );
        assert_eq!(registry, before);
    }
    assert_eq!(
        registry.kick(uuid(2), id, uuid(1)),
        Err(KingdomError::PermissionDenied)
    );
    registry
        .set_role(uuid(2), id, uuid(4), KingdomRole::Worker)
        .unwrap();
    assert!(registry.can_manage(uuid(2), id));
    registry.kick(uuid(2), id, uuid(4)).unwrap();
    assert!(registry.valid());
}
#[test]
fn alliance_requires_both_managers_and_revocation_blocks_inventory_and_transport_immediately() {
    let mut registry = KingdomRegistry::default();
    let a = realm(&mut registry, 1, 0);
    let b = realm(&mut registry, 2, 32);
    registry.join(uuid(3), a).unwrap();
    let ta = DiplomaticTarget::Player(a);
    let tb = DiplomaticTarget::Player(b);
    assert_eq!(
        registry.set_relation(uuid(3), a, tb, DiplomaticRelation::Allied),
        Err(KingdomError::PermissionDenied)
    );
    registry
        .set_relation(uuid(1), a, tb, DiplomaticRelation::Allied)
        .unwrap();
    assert_eq!(registry.relation(a, b), DiplomaticRelation::Neutral);
    assert!(!registry.can_transport([1, 20, 1], [33, 20, 1]));
    registry
        .set_relation(uuid(2), b, ta, DiplomaticRelation::Allied)
        .unwrap();
    assert_eq!(registry.relation(a, b), DiplomaticRelation::Allied);
    assert!(registry.can_transport([1, 20, 1], [33, 20, 1]));
    assert!(registry.can(uuid(1), KingdomPermission::Inventories, [33, 20, 1]));
    assert!(!registry.can(uuid(1), KingdomPermission::Build, [33, 20, 1]));
    assert!(!registry.can(uuid(3), KingdomPermission::Inventories, [33, 20, 1]));
    registry
        .set_relation(uuid(2), b, ta, DiplomaticRelation::Hostile)
        .unwrap();
    assert_eq!(registry.relation(a, b), DiplomaticRelation::Hostile);
    assert!(!registry.can_transport([1, 20, 1], [33, 20, 1]));
    assert!(!registry.can(uuid(1), KingdomPermission::NpcWork, [33, 20, 1]));
    registry
        .set_relation(uuid(1), a, tb, DiplomaticRelation::Neutral)
        .unwrap();
    assert_eq!(registry.relation(a, b), DiplomaticRelation::Hostile);
    registry.dissolve(uuid(2), b).unwrap();
    assert!(registry.relations.is_empty());
    assert_eq!(registry.relation(a, b), DiplomaticRelation::Neutral);
    assert!(registry.valid());
}
#[test]
fn native_diplomacy_and_captured_settlement_permissions_follow_live_ownership() {
    let mut registry = KingdomRegistry::default();
    let id = realm(&mut registry, 1, 0);
    let domain = SettlementDomain {
        id: 10,
        center: [32, 20, 0],
        bounds: [[32, 0, 0], [63, 40, 31]],
        kind: DomainKind::Village,
        native: Some(BotKingdom::Helteria),
    };
    let position = [50, 20, 0];
    assert!(registry.can_in_domain(uuid(1), KingdomPermission::NpcWork, position, &domain));
    registry
        .set_relation(
            uuid(1),
            id,
            DiplomaticTarget::Bot(BotKingdom::Helteria),
            DiplomaticRelation::Allied,
        )
        .unwrap();
    assert!(registry.can_in_domain(uuid(1), KingdomPermission::NpcWork, position, &domain));
    assert!(registry.can_in_domain(uuid(1), KingdomPermission::Build, position, &domain));
    registry
        .set_relation(
            uuid(1),
            id,
            DiplomaticTarget::Bot(BotKingdom::Lutera),
            DiplomaticRelation::Allied,
        )
        .unwrap();
    assert_eq!(
        registry.relation_to(id, DiplomaticTarget::Bot(BotKingdom::Lutera)),
        DiplomaticRelation::Neutral
    );
    registry.kingdoms[0].level = 2;
    assert_eq!(
        registry.relation_to(id, DiplomaticTarget::Bot(BotKingdom::Lutera)),
        DiplomaticRelation::Allied
    );
    assert_eq!(
        registry.set_relation(
            uuid(1),
            id,
            DiplomaticTarget::Bot(BotKingdom::Petora),
            DiplomaticRelation::Allied
        ),
        Err(KingdomError::InvalidRelation)
    );
    registry
        .claim(
            id,
            [1, 0],
            ClaimKind::ProtectedBuilding,
            ClaimActor::Player(uuid(1)),
        )
        .unwrap();
    registry
        .claim(
            id,
            [2, 0],
            ClaimKind::ProtectedBuilding,
            ClaimActor::Player(uuid(1)),
        )
        .unwrap();
    assert!(registry.can_in_domain(uuid(1), KingdomPermission::Build, position, &domain));
    assert!(!registry.can_in_domain(uuid(2), KingdomPermission::Build, position, &domain));
    registry.release(uuid(1), id, [2, 0]).unwrap();
    assert!(registry.can_in_domain(uuid(2), KingdomPermission::Build, position, &domain));
    assert!(registry.valid());
}

#[test]
fn bot_attitudes_allow_friendly_actions_neutral_openings_and_no_hostile_actions() {
    let mut registry = KingdomRegistry::default();
    let visitor = uuid(9);
    let permissions = [
        KingdomPermission::Build,
        KingdomPermission::Break,
        KingdomPermission::Inventories,
        KingdomPermission::NpcWork,
        KingdomPermission::Openings,
    ];
    let domain = |native| SettlementDomain {
        id: 99,
        center: [64, 20, 64],
        bounds: [[48, 0, 48], [95, 60, 95]],
        kind: DomainKind::Kingdom,
        native: Some(native),
    };
    let p = [70, 20, 70];
    for (bot, expected) in [
        (BotKingdom::Helteria, [true; 5]),
        (BotKingdom::Petora, [false; 5]),
        (BotKingdom::Lutera, [false, false, false, false, true]),
        (BotKingdom::Betross, [false, false, false, false, true]),
    ] {
        assert_eq!(
            permissions.map(|permission| registry.can_in_domain(
                visitor,
                permission,
                p,
                &domain(bot)
            )),
            expected
        );
    }
    let id = realm(&mut registry, 1, 0);
    registry.join(visitor, id).unwrap();
    // Native attitudes do not require a player to found a realm or gain a role.
    assert!(
        permissions
            .into_iter()
            .all(|permission| registry.can_in_domain(
                visitor,
                permission,
                p,
                &domain(BotKingdom::Helteria)
            ))
    );
    registry
        .set_relation(
            uuid(1),
            id,
            DiplomaticTarget::Bot(BotKingdom::Helteria),
            DiplomaticRelation::Hostile,
        )
        .unwrap();
    assert!(
        permissions
            .into_iter()
            .all(|permission| !registry.can_in_domain(
                visitor,
                permission,
                p,
                &domain(BotKingdom::Helteria)
            ))
    );
    registry
        .set_relation(
            uuid(1),
            id,
            DiplomaticTarget::Bot(BotKingdom::Lutera),
            DiplomaticRelation::Allied,
        )
        .unwrap();
    assert!(!registry.can_in_domain(
        visitor,
        KingdomPermission::Break,
        p,
        &domain(BotKingdom::Lutera)
    ));
    registry.kingdoms[0].level = 2;
    assert!(
        permissions
            .into_iter()
            .all(|permission| registry.can_in_domain(
                visitor,
                permission,
                p,
                &domain(BotKingdom::Lutera)
            ))
    );
}
#[test]
fn duplicate_or_stale_capabilities_are_invalid_and_legacy_members_keep_full_nonmanager_access() {
    let mut registry = KingdomRegistry::default();
    let id = realm(&mut registry, 1, 0);
    registry.join(uuid(2), id).unwrap();
    registry.migrate_legacy_roles();
    assert_eq!(registry.role_for(id, uuid(2)), Some(KingdomRole::Member));
    assert!(registry.can(uuid(2), KingdomPermission::NpcWork, [1, 20, 1]));
    assert!(!registry.can_manage(uuid(2), id));
    assert!(registry.valid());
    registry.roles.push(registry.roles[0]);
    assert!(!registry.valid());
    registry.roles.pop();
    registry.roles[0].member = uuid(9);
    assert!(!registry.valid());
}
