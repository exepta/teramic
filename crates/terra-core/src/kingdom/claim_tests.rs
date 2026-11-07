use super::*;

#[test]
fn separate_quotas_build_connections_actor_audit_and_protected_access_are_enforced() {
    let owner = PlayerUuid::from_u128(1);
    let visitor = PlayerUuid::from_u128(2);
    let mut registry = KingdomRegistry::default();
    let id = registry
        .found_settlement(
            owner,
            "Alice",
            [0, 20, 0],
            "Oak Village",
            SettlementKind::Village,
            KingdomHeraldry {
                color: [12, 34, 56],
                icon: KingdomIcon::Tree,
            },
        )
        .unwrap();
    assert_eq!(
        registry.claim_counts(id),
        ClaimCounts {
            protected: 1,
            territory: 0
        }
    );
    assert!(!registry.can_build(visitor, [15, -1000, 15]));
    assert!(registry.can_build(owner, [15, 1000, 15]));
    assert_eq!(claim_chunk([-1, 50, -17]), [-1, -2]);
    assert_eq!(
        registry.claim(
            id,
            [5, 0],
            ClaimKind::RealmTerritory,
            ClaimActor::Player(owner)
        ),
        Err(KingdomError::ClaimDisconnected)
    );
    for x in 1..32 {
        registry
            .claim(
                id,
                [x, 0],
                ClaimKind::ProtectedBuilding,
                ClaimActor::Player(owner),
            )
            .unwrap();
    }
    let before = registry.clone();
    assert_eq!(
        registry.claim(
            id,
            [32, 0],
            ClaimKind::ProtectedBuilding,
            ClaimActor::Player(owner)
        ),
        Err(KingdomError::ClaimLimitReached)
    );
    assert_eq!(registry, before);
    for x in 32..96 {
        registry
            .claim(
                id,
                [x, 0],
                ClaimKind::RealmTerritory,
                ClaimActor::Bot {
                    id: 77,
                    authorized_by: owner,
                },
            )
            .unwrap();
    }
    assert_eq!(
        registry.claim_counts(id),
        ClaimCounts {
            protected: 32,
            territory: 64
        }
    );
    let before_conversion = registry.clone();
    assert_eq!(
        registry.claim(
            id,
            [32, 0],
            ClaimKind::ProtectedBuilding,
            ClaimActor::Player(owner)
        ),
        Err(KingdomError::ClaimLimitReached)
    );
    assert_eq!(
        registry, before_conversion,
        "quota failure must retain the old territory kind and bot actor"
    );
    assert!(registry.can_build(visitor, [32 * 16, 20, 0]));
    assert_eq!(
        registry.claim_at([32, 0]).unwrap().actor,
        ClaimActor::Bot {
            id: 77,
            authorized_by: owner
        }
    );
    assert_eq!(
        registry.claim(
            id,
            [96, 0],
            ClaimKind::RealmTerritory,
            ClaimActor::Player(owner)
        ),
        Err(KingdomError::ClaimLimitReached)
    );
    assert_eq!(
        registry.release(owner, id, [1, 0]),
        Err(KingdomError::ClaimDisconnected)
    );
    assert_eq!(
        registry.release(owner, id, [0, 0]),
        Err(KingdomError::SeatMustStayClaimed)
    );
    assert_eq!(
        registry.release(visitor, id, [95, 0]),
        Err(KingdomError::NotMember)
    );
    registry.release(owner, id, [95, 0]).unwrap();
    let before_bridge_conversion = registry.clone();
    assert_eq!(
        registry.claim(
            id,
            [1, 0],
            ClaimKind::RealmTerritory,
            ClaimActor::Player(owner),
        ),
        Err(KingdomError::ClaimDisconnected)
    );
    assert_eq!(registry, before_bridge_conversion);
    registry
        .claim(
            id,
            [31, 0],
            ClaimKind::RealmTerritory,
            ClaimActor::Player(owner),
        )
        .unwrap();
    assert_eq!(
        registry.claim_at([31, 0]).unwrap().kind,
        ClaimKind::RealmTerritory,
        "a Build leaf may become Territory without severing other Build claims"
    );
    registry
        .claim(
            id,
            [31, 0],
            ClaimKind::ProtectedBuilding,
            ClaimActor::Bot {
                id: 78,
                authorized_by: owner,
            },
        )
        .unwrap();
    assert_eq!(
        registry.claim_at([31, 0]).unwrap().actor,
        ClaimActor::Bot {
            id: 78,
            authorized_by: owner
        }
    );
    assert!(registry.valid());
    registry.dissolve(owner, id).unwrap();
    assert!(registry.claims.is_empty());
    assert!(registry.region_at([0, 20, 0]).is_none());
    assert!(registry.can_build(visitor, [0, 20, 0]));
}

#[test]
fn upgrade_payment_is_atomic_owner_only_and_grows_both_limits_without_free_setters() {
    let owner = PlayerUuid::from_u128(1);
    let other = PlayerUuid::from_u128(2);
    let mut state = crate::GameplayState::default();
    let id = state.kingdoms.found(owner, [0, 20, 0], "Capital").unwrap();
    let stone = crate::ItemKind::Block(crate::castle::STONE_WALL);
    let gold = crate::ItemKind::Mineral(crate::MineralItem::GoldIngot);
    state.inventories.entry(owner).or_default().insert(stone, 8);
    let before = state.clone();
    assert_eq!(
        state.upgrade_kingdom(other, id),
        Err(KingdomError::PermissionDenied)
    );
    assert_eq!(state, before);
    assert_eq!(
        state.upgrade_kingdom(owner, id),
        Err(KingdomError::UpgradeMaterialsMissing)
    );
    assert_eq!(state, before);
    state.inventories.get_mut(&owner).unwrap().insert(gold, 1);
    let inventory_revision = state.inventories[&owner].revision;
    assert_eq!(state.upgrade_kingdom(owner, id), Ok(2));
    assert_eq!(
        state.kingdoms.get(id).unwrap().limits(),
        ClaimLimits {
            protected: 38,
            territory: 80
        }
    );
    assert_eq!(state.inventories[&owner].count(stone), 0);
    assert_eq!(state.inventories[&owner].count(gold), 0);
    assert_eq!(state.inventories[&owner].revision, inventory_revision + 1);
    assert!(state.valid());
    state.kingdoms.kingdoms[0].level = 64;
    let before = state.clone();
    assert_eq!(
        state.upgrade_kingdom(owner, id),
        Err(KingdomError::UpgradeLimitReached)
    );
    assert_eq!(state, before);
}

#[test]
fn malformed_claims_and_outsider_heraldry_cannot_change_a_settlement() {
    let owner = PlayerUuid::from_u128(1);
    let other = PlayerUuid::from_u128(2);
    let mut registry = KingdomRegistry::default();
    let id = registry.found(owner, [-1, 20, -1], "Home").unwrap();
    assert_eq!(
        registry.claim(
            id,
            [-2, -1],
            ClaimKind::RealmTerritory,
            ClaimActor::Player(other)
        ),
        Err(KingdomError::NotMember)
    );
    let heraldry = KingdomHeraldry {
        color: [8, 16, 32],
        icon: KingdomIcon::Crown,
    };
    assert_eq!(
        registry.change_appearance(other, id, heraldry),
        Err(KingdomError::PermissionDenied)
    );
    registry.change_appearance(owner, id, heraldry).unwrap();
    assert_eq!(registry.heraldry_at([-1, 500, -1]), Some(heraldry));
    let mut invalid = registry.clone();
    invalid.claims.push(invalid.claims[0]);
    assert!(!invalid.valid());
    invalid = registry.clone();
    invalid.claims[0].chunk = [-5, -5];
    assert!(!invalid.valid());
    invalid = registry;
    invalid.claims[0].actor = ClaimActor::Bot {
        id: 0,
        authorized_by: owner,
    };
    assert!(!invalid.valid());
}
