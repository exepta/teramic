use super::*;

#[test]
fn founding_all_settlement_kinds_sends_the_chosen_heraldry_without_mutating_the_replica() {
    for kind in SettlementKind::ALL {
        let mut app = app();
        if kind != SettlementKind::Kingdom {
            let owner = app.world().resource::<ClientSession>().uuid.unwrap();
            let mut session = app.world_mut().resource_mut::<ClientSession>();
            let parent = session
                .kingdoms
                .found(owner, [17 * 16, 0, 0], "Parent Realm")
                .unwrap();
            session
                .kingdoms
                .kingdoms
                .iter_mut()
                .find(|record| record.id == parent)
                .unwrap()
                .level = 3;
            if kind != SettlementKind::Outpost {
                session
                    .kingdoms
                    .claim_batch_with_rules(
                        parent,
                        &(0..17)
                            .map(|x| ([x, 0], ClaimKind::RealmTerritory))
                            .collect::<Vec<_>>(),
                        ClaimActor::Player(owner),
                        &terra_core::kingdom::KingdomRules::default(),
                        &[],
                    )
                    .unwrap();
            }
        }
        let before = app.world().resource::<ClientSession>().kingdoms.clone();
        open_empty(&mut app);
        act(&mut app, KingdomMenuAction::Kind(kind));
        act(&mut app, KingdomMenuAction::Color("rgb(12, 34, 56)".into()));
        act(&mut app, KingdomMenuAction::Icon(KingdomIcon::Star));
        act(&mut app, KingdomMenuAction::Name("Harbor".into()));
        let view = app.world().resource::<KingdomView>();
        assert_eq!(view.settlement_kind, kind.label());
        assert_eq!(view.color, "#0C2238");
        assert_eq!(view.emblem_source, "tilt-icon:star@32");
        assert!(view.can_found);
        assert!(requests(&mut app).is_empty());
        act(&mut app, KingdomMenuAction::Found);
        assert_eq!(
            requests(&mut app),
            [KingdomAction::FoundSettlement {
                position: [0, 0, 0],
                name: "Harbor".into(),
                kind,
                heraldry: KingdomHeraldry {
                    color: [12, 34, 56],
                    icon: KingdomIcon::Star
                },
            }]
        );
        assert_eq!(app.world().resource::<ClientSession>().kingdoms, before);
        let mut authoritative = before;
        let owner = app.world().resource::<ClientSession>().uuid.unwrap();
        authoritative
            .found_settlement_with_rules(
                owner,
                "Founder",
                [0, 0, 0],
                "Harbor",
                kind,
                KingdomHeraldry {
                    color: [12, 34, 56],
                    icon: KingdomIcon::Star,
                },
                &terra_core::kingdom::KingdomRules::default(),
                &[],
            )
            .unwrap();
        assert!(authoritative.valid());
        assert!(app.world().resource::<KingdomView>().pending);
    }
}

#[test]
fn translucent_or_invalid_banner_colors_block_submission_until_corrected() {
    let mut app = app();
    open_empty(&mut app);
    act(&mut app, KingdomMenuAction::Name("Harbor".into()));
    for color in ["#GG0033", "rgba(12, 34, 56, 0.5)", "transparent"] {
        act(&mut app, KingdomMenuAction::Color(color.into()));
        assert!(!app.world().resource::<KingdomView>().can_found);
        assert!(!app.world().resource::<KingdomView>().error.is_empty());
        act(&mut app, KingdomMenuAction::Found);
        assert!(requests(&mut app).is_empty());
    }
    act(&mut app, KingdomMenuAction::Color("#123456".into()));
    assert!(app.world().resource::<KingdomView>().can_found);
    assert!(app.world().resource::<KingdomView>().error.is_empty());
    for icon in KingdomIcon::ALL {
        assert!(
            tilt_ui::Icon::from_name(icon.glyph()).is_some() || {
                let source = emblem_source(icon);
                std::path::Path::new(&source).is_file()
            },
            "Missing native emblem {}",
            icon.key()
        );
    }
}

#[test]
fn appearance_waits_for_its_exact_ack_and_keeps_server_heraldry_unchanged_until_then() {
    let mut app = app();
    let id = founded(&mut app);
    let before = app.world().resource::<ClientSession>().kingdoms.clone();
    act(&mut app, KingdomMenuAction::Kind(SettlementKind::City));
    assert_eq!(
        app.world().resource::<KingdomView>().settlement_kind,
        "Kingdom"
    );
    act(&mut app, KingdomMenuAction::Color("#123456".into()));
    act(&mut app, KingdomMenuAction::Icon(KingdomIcon::Crown));
    act(&mut app, KingdomMenuAction::SaveAppearance);
    let heraldry = KingdomHeraldry {
        color: [0x12, 0x34, 0x56],
        icon: KingdomIcon::Crown,
    };
    assert_eq!(
        requests(&mut app),
        [KingdomAction::ChangeAppearance { id, heraldry }]
    );
    assert_eq!(app.world().resource::<ClientSession>().kingdoms, before);
    let expected = app
        .world()
        .resource::<KingdomMenuState>()
        .pending
        .unwrap()
        .0;
    reply_with_request(
        &mut app,
        expected - 1,
        KingdomOutcome::AppearanceChanged { id },
    );
    assert!(app.world().resource::<KingdomView>().pending);
    let owner = app.world().resource::<ClientSession>().uuid.unwrap();
    app.world_mut()
        .resource_mut::<ClientSession>()
        .kingdoms
        .change_appearance(owner, id, heraldry)
        .unwrap();
    reply(&mut app, KingdomOutcome::AppearanceChanged { id });
    assert!(!app.world().resource::<KingdomView>().pending);
    assert!(!app.world().resource::<KingdomView>().can_save_appearance);
    assert_eq!(app.world().resource::<KingdomView>().color, "#123456");
}

#[test]
fn authorized_officers_claim_convert_and_release_negative_chunks_with_server_confirmation() {
    let mut app = app();
    let member = app.world().resource::<ClientSession>().uuid.unwrap();
    let founder = PlayerUuid::from_u128(42044);
    let id = app
        .world_mut()
        .resource_mut::<ClientSession>()
        .kingdoms
        .found(founder, [0, 0, 0], "Riverkeep")
        .unwrap();
    app.world_mut()
        .resource_mut::<ClientSession>()
        .kingdoms
        .join(member, id)
        .unwrap();
    app.world_mut()
        .resource_mut::<ClientSession>()
        .kingdoms
        .set_role(founder, id, member, KingdomRole::Officer)
        .unwrap();
    app.world_mut().resource_mut::<ClientSession>().spawn.x = -0.25;
    act(&mut app, KingdomMenuAction::Open([0, 0, 0]));
    requests(&mut app);
    reply(&mut app, KingdomOutcome::Inspected { kingdom: Some(id) });
    assert!(!app.world().resource::<KingdomView>().founder_controls);
    assert!(app.world().resource::<KingdomView>().can_claim_territory);
    let chunk = [-1, 0];
    act(
        &mut app,
        KingdomMenuAction::Claim(ClaimKind::RealmTerritory),
    );
    assert_eq!(
        requests(&mut app),
        [KingdomAction::ClaimChunk {
            id,
            chunk,
            kind: ClaimKind::RealmTerritory
        }]
    );
    assert!(
        app.world()
            .resource::<ClientSession>()
            .kingdoms
            .claim_at(chunk)
            .is_none()
    );
    app.world_mut()
        .resource_mut::<ClientSession>()
        .kingdoms
        .claim(
            id,
            chunk,
            ClaimKind::RealmTerritory,
            ClaimActor::Player(member),
        )
        .unwrap();
    reply(
        &mut app,
        KingdomOutcome::Claimed {
            id,
            chunk,
            kind: ClaimKind::RealmTerritory,
        },
    );
    assert!(app.world().resource::<KingdomView>().can_claim_protected);
    assert!(!app.world().resource::<KingdomView>().can_claim_territory);
    act(
        &mut app,
        KingdomMenuAction::Claim(ClaimKind::ProtectedBuilding),
    );
    assert_eq!(
        requests(&mut app),
        [KingdomAction::ClaimChunk {
            id,
            chunk,
            kind: ClaimKind::ProtectedBuilding
        }]
    );
    assert_eq!(
        app.world()
            .resource::<ClientSession>()
            .kingdoms
            .claim_at(chunk)
            .unwrap()
            .kind,
        ClaimKind::RealmTerritory
    );
    app.world_mut()
        .resource_mut::<ClientSession>()
        .kingdoms
        .claim(
            id,
            chunk,
            ClaimKind::ProtectedBuilding,
            ClaimActor::Player(member),
        )
        .unwrap();
    reply(
        &mut app,
        KingdomOutcome::Claimed {
            id,
            chunk,
            kind: ClaimKind::ProtectedBuilding,
        },
    );
    assert_eq!(
        app.world().resource::<KingdomView>().protected_claims,
        "2 / 32"
    );
    assert!(app.world().resource::<KingdomView>().can_release);
    act(&mut app, KingdomMenuAction::Release);
    assert_eq!(
        requests(&mut app),
        [KingdomAction::ReleaseChunk { id, chunk }]
    );
    app.world_mut()
        .resource_mut::<ClientSession>()
        .kingdoms
        .release(member, id, chunk)
        .unwrap();
    reply(&mut app, KingdomOutcome::Released { id, chunk });
    assert_eq!(
        app.world().resource::<KingdomView>().current_claim,
        "Unclaimed land"
    );
}

#[test]
fn independent_claim_pools_and_base_guards_use_authoritative_limits() {
    let mut app = app();
    let id = founded(&mut app);
    let owner = app.world().resource::<ClientSession>().uuid.unwrap();
    assert_eq!(
        app.world().resource::<KingdomView>().protected_claims,
        "1 / 32"
    );
    assert_eq!(
        app.world().resource::<KingdomView>().territory_claims,
        "0 / 64"
    );
    assert!(!app.world().resource::<KingdomView>().can_release);
    act(&mut app, KingdomMenuAction::Release);
    act(
        &mut app,
        KingdomMenuAction::Claim(ClaimKind::RealmTerritory),
    );
    assert!(requests(&mut app).is_empty());
    for x in 1..32 {
        app.world_mut()
            .resource_mut::<ClientSession>()
            .kingdoms
            .claim(
                id,
                [x, 0],
                ClaimKind::ProtectedBuilding,
                ClaimActor::Player(owner),
            )
            .unwrap();
    }
    app.world_mut().resource_mut::<ClientSession>().spawn.x = -0.25;
    update(app.world_mut());
    assert!(!app.world().resource::<KingdomView>().can_claim_protected);
    assert!(app.world().resource::<KingdomView>().can_claim_territory);
    act(
        &mut app,
        KingdomMenuAction::Claim(ClaimKind::ProtectedBuilding),
    );
    assert!(requests(&mut app).is_empty());
    app.world_mut()
        .resource_mut::<ClientSession>()
        .kingdoms
        .kingdoms[0]
        .level = 2;
    update(app.world_mut());
    let view = app.world().resource::<KingdomView>();
    assert_eq!(view.level, "Level 2");
    assert_eq!(view.protected_claims, "32 / 38");
    assert_eq!(view.territory_claims, "0 / 80");
    assert!(view.can_claim_protected);
}

#[test]
fn upgrades_show_real_costs_and_never_change_inventory_or_level_optimistically() {
    let mut app = app();
    let id = founded(&mut app);
    act(&mut app, KingdomMenuAction::Upgrade);
    assert!(requests(&mut app).is_empty());
    assert!(!app.world().resource::<KingdomView>().can_upgrade);
    {
        let mut session = app.world_mut().resource_mut::<ClientSession>();
        session.inventory.insert(
            terra_core::ItemKind::Block(terra_core::castle::STONE_WALL),
            8,
        );
        session.inventory.insert(
            terra_core::ItemKind::Mineral(terra_core::MineralItem::GoldIngot),
            1,
        );
    }
    update(app.world_mut());
    assert!(app.world().resource::<KingdomView>().can_upgrade);
    let before = app.world().resource::<ClientSession>().inventory.clone();
    act(&mut app, KingdomMenuAction::Upgrade);
    assert_eq!(requests(&mut app), [KingdomAction::Upgrade { id }]);
    assert_eq!(app.world().resource::<ClientSession>().inventory, before);
    assert_eq!(
        app.world()
            .resource::<ClientSession>()
            .kingdoms
            .get(id)
            .unwrap()
            .level,
        1
    );
    {
        let mut session = app.world_mut().resource_mut::<ClientSession>();
        session.kingdoms.kingdoms[0].level = 2;
        session.inventory = default();
    }
    reply(&mut app, KingdomOutcome::Upgraded { id, level: 2 });
    assert_eq!(app.world().resource::<KingdomView>().level, "Level 2");
    assert!(
        app.world()
            .resource::<KingdomView>()
            .upgrade_cost
            .contains("0 / 16")
    );
    app.world_mut()
        .resource_mut::<ClientSession>()
        .kingdoms
        .kingdoms[0]
        .level = 64;
    update(app.world_mut());
    assert!(!app.world().resource::<KingdomView>().can_upgrade);
    assert!(
        app.world()
            .resource::<KingdomView>()
            .upgrade_cost
            .contains("Maximum")
    );
    act(&mut app, KingdomMenuAction::Upgrade);
    assert!(requests(&mut app).is_empty());
}

#[test]
fn full_emblem_picker_searches_native_catalog_and_sends_persistable_selection() {
    let mut app = app();
    open_empty(&mut app);
    assert!(app.world().resource::<KingdomView>().icons.is_empty());
    act(&mut app, KingdomMenuAction::OpenIcons);
    let view = app.world().resource::<KingdomView>();
    assert!(view.icon_picker);
    for icon in tilt_ui::Icon::ALL {
        assert!(
            view.icons.iter().any(|choice| {
                KingdomIcon::from_key(&choice.key)
                    .is_some_and(|choice| choice.glyph() == icon.name())
            }),
            "Missing catalog icon {}",
            icon.name()
        );
    }
    act(&mut app, KingdomMenuAction::SearchIcons("shield".into()));
    let view = app.world().resource::<KingdomView>();
    assert!(!view.icons.is_empty());
    assert!(
        view.icons
            .iter()
            .all(|choice| choice.key.contains("shield"))
    );
    let key = view.icons[0].key.clone();
    let icon = KingdomIcon::from_key(&key).unwrap();
    act(&mut app, KingdomMenuAction::Icon(icon));
    let view = app.world().resource::<KingdomView>();
    assert!(!view.icon_picker);
    assert!(view.icons.is_empty());
    assert_eq!(view.emblem_source, emblem_source(icon));
    act(&mut app, KingdomMenuAction::Name("Dragonwatch".into()));
    act(&mut app, KingdomMenuAction::Found);
    assert!(matches!(requests(&mut app).as_slice(),
        [KingdomAction::FoundSettlement { heraldry, .. }] if heraldry.icon == icon));
}

#[test]
fn invalid_catalog_icons_cannot_replace_the_current_banner() {
    let mut app = app();
    open_empty(&mut app);
    let before = app.world().resource::<KingdomView>().emblem_source.clone();
    act(&mut app, KingdomMenuAction::Icon(KingdomIcon::Catalog(0)));
    assert_eq!(app.world().resource::<KingdomView>().emblem_source, before);
    assert!(requests(&mut app).is_empty());
}

#[test]
#[cfg(not(target_arch = "wasm32"))]
fn custom_legacy_emblems_rasterize_through_the_actual_native_image_loader() {
    let mut world = World::new();
    for icon in [KingdomIcon::Wheat, KingdomIcon::Hammer] {
        let entity = world
            .spawn((
                tilt_ui::ImageMetadata::default(),
                bevy::ui::widget::ImageNode::default(),
            ))
            .id();
        assert!(tilt_ui::set_image_source(
            &mut world,
            entity,
            Some(emblem_source(icon))
        ));
        let handle = &world
            .get::<bevy::ui::widget::ImageNode>(entity)
            .unwrap()
            .image;
        let image = world
            .resource::<Assets<Image>>()
            .get(handle)
            .expect("native SVG loader must produce an image, not an unresolved asset handle");
        assert_eq!(image.size(), UVec2::splat(32));
        assert!(
            image
                .data
                .as_ref()
                .unwrap()
                .chunks_exact(4)
                .any(|pixel| pixel[3] > 0),
            "{} raster must contain visible pixels",
            icon.key()
        );
    }
}
