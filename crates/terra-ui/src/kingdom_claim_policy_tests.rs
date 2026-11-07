use super::*;

#[test]
fn claim_buttons_distinguish_territory_neighbors_from_a_build_route_and_refresh_live() {
    let mut app = app();
    let keep = [15, 0, 15];
    let owner = app.world().resource::<ClientSession>().uuid.unwrap();
    let id = {
        let mut session = app.world_mut().resource_mut::<ClientSession>();
        let id = session.kingdoms.found(owner, keep, "Corner Keep").unwrap();
        session
            .kingdoms
            .claim(
                id,
                [1, 0],
                ClaimKind::RealmTerritory,
                ClaimActor::Player(owner),
            )
            .unwrap();
        session.spawn = Vec3::new(16.25, 1.0, 16.25);
        id
    };
    act(&mut app, KingdomMenuAction::Open(keep));
    requests(&mut app);
    reply(&mut app, KingdomOutcome::Inspected { kingdom: Some(id) });
    let view = app.world().resource::<KingdomView>();
    assert!(view.visible && !view.can_claim_protected && view.can_claim_territory);
    let before = app.world().resource::<ClientSession>().kingdoms.clone();
    act(
        &mut app,
        KingdomMenuAction::Claim(ClaimKind::ProtectedBuilding),
    );
    assert!(requests(&mut app).is_empty());
    assert_eq!(app.world().resource::<ClientSession>().kingdoms, before);

    let mut config = terra_core::GameConfig::default();
    config.world.kingdom_rules.require_adjacent_claims = false;
    app.insert_resource(config);
    update(app.world_mut());
    let view = app.world().resource::<KingdomView>();
    assert!(view.can_claim_protected && view.can_claim_territory);
    assert!(view.claim_hint.contains("Adjacency is disabled"));
    app.world_mut()
        .resource_mut::<terra_core::GameConfig>()
        .world
        .kingdom_rules
        .require_adjacent_claims = true;
    update(app.world_mut());
    assert!(!app.world().resource::<KingdomView>().can_claim_protected);

    app.world_mut()
        .resource_mut::<ClientSession>()
        .kingdoms
        .claim(
            id,
            [0, 1],
            ClaimKind::ProtectedBuilding,
            ClaimActor::Player(owner),
        )
        .unwrap();
    update(app.world_mut());
    let view = app.world().resource::<KingdomView>();
    assert!(view.can_claim_protected && view.can_claim_territory);
    assert!(
        view.claim_hint
            .contains("through build land to the Bergfried")
    );
    assert_eq!(requests(&mut app), []);
}

#[test]
fn village_and_city_build_buttons_keep_parent_territory_overlays_usable() {
    for kind in [SettlementKind::Village, SettlementKind::City] {
        let mut app = app();
        let owner = app.world().resource::<ClientSession>().uuid.unwrap();
        let mut config = terra_core::GameConfig::default();
        config.world.kingdom_rules.city_min_kingdom_level = 1;
        config.world.kingdom_rules.city_min_build_gap = 0;
        let rules = config.world.kingdom_rules;
        app.insert_resource(config);
        let keep = [15, 0, 15];
        let (parent, child) = {
            let mut session = app.world_mut().resource_mut::<ClientSession>();
            let parent = session
                .kingdoms
                .found(owner, [-64, 0, 0], "Parent Realm")
                .unwrap();
            let territory = (-3..=1)
                .map(|x| ([x, 0], ClaimKind::RealmTerritory))
                .collect::<Vec<_>>();
            session
                .kingdoms
                .claim_batch_with_rules(parent, &territory, ClaimActor::Player(owner), &rules, &[])
                .unwrap();
            let child = session
                .kingdoms
                .found_settlement_with_rules(
                    owner,
                    "Founder",
                    keep,
                    "Dependent",
                    kind,
                    default(),
                    &rules,
                    &[],
                )
                .unwrap();
            session.spawn = Vec3::new(16.25, 1.0, 15.5);
            (parent, child)
        };
        act(&mut app, KingdomMenuAction::Open(keep));
        requests(&mut app);
        reply(
            &mut app,
            KingdomOutcome::Inspected {
                kingdom: Some(child),
            },
        );
        let view = app.world().resource::<KingdomView>();
        assert!(
            view.visible && view.can_claim_protected && !view.can_claim_territory,
            "{}",
            kind.key()
        );
        let before = app.world().resource::<ClientSession>().kingdoms.clone();
        act(
            &mut app,
            KingdomMenuAction::Claim(ClaimKind::ProtectedBuilding),
        );
        assert_eq!(
            requests(&mut app),
            [KingdomAction::ClaimChunk {
                id: child,
                chunk: [1, 0],
                kind: ClaimKind::ProtectedBuilding
            }]
        );
        assert_eq!(app.world().resource::<ClientSession>().kingdoms, before);
        let mut confirmed = before;
        confirmed
            .claim_with_rules(
                child,
                [1, 0],
                ClaimKind::ProtectedBuilding,
                ClaimActor::Player(owner),
                &rules,
                &[],
            )
            .unwrap();
        assert!(confirmed.claims.iter().any(|claim| claim.kingdom == parent
            && claim.chunk == [1, 0]
            && claim.kind == ClaimKind::RealmTerritory));
        assert!(confirmed.claims.iter().any(|claim| claim.kingdom == child
            && claim.chunk == [1, 0]
            && claim.kind == ClaimKind::ProtectedBuilding));
    }
}

#[test]
fn historical_build_islands_do_not_disable_valid_seat_connected_expansion() {
    let mut app = app();
    let id = founded(&mut app);
    let owner = app.world().resource::<ClientSession>().uuid.unwrap();
    {
        let mut session = app.world_mut().resource_mut::<ClientSession>();
        // Represent a historical saved island directly: current transactions
        // deliberately cannot create this disconnected Build claim.
        session
            .kingdoms
            .claims
            .push(terra_core::kingdom::KingdomClaim {
                kingdom: id,
                chunk: [20, 20],
                kind: ClaimKind::ProtectedBuilding,
                actor: ClaimActor::Player(owner),
            });
        session.kingdoms.revision += 1;
        assert!(session.kingdoms.valid());
        session.spawn.x = -0.25;
    }
    update(app.world_mut());
    let view = app.world().resource::<KingdomView>();
    assert!(view.can_claim_protected && view.can_claim_territory);
    let before = app.world().resource::<ClientSession>().kingdoms.clone();
    update(app.world_mut());
    assert_eq!(app.world().resource::<ClientSession>().kingdoms, before);
    assert!(requests(&mut app).is_empty());
}
