use super::*;
use terra_core::kingdom::{KingdomRegistry, KingdomState};
use terra_network::protocol::kingdom::KingdomResult;

fn app() -> App {
    let mut app = App::new();
    let owner = PlayerUuid::from_u128(42043);
    let mut session = ClientSession::default();
    session.ready = true;
    session.uuid = Some(owner);
    session.spawn = Vec3::new(0.5, 1.0, 0.5);
    session.allow_build = true;
    app.insert_resource(session)
        .init_resource::<KingdomMenuState>()
        .init_resource::<KingdomView>()
        .init_resource::<PlayerInputBlock>()
        .init_resource::<ButtonInput<KeyCode>>()
        .init_resource::<ButtonInput<MouseButton>>()
        .init_resource::<terra_player::CameraInputState>()
        .add_message::<KingdomMenuAction>()
        .add_message::<KingdomRequest>()
        .add_message::<CaptureRequest>();
    app
}

fn requests(app: &mut App) -> Vec<KingdomAction> {
    app.world_mut()
        .resource_mut::<Messages<KingdomRequest>>()
        .drain()
        .map(|request| request.0.action)
        .collect()
}

#[test]
fn kingdom_hotkey_opens_the_remote_keep_only_while_in_member_territory() {
    use terra_core::kingdom::{ClaimActor, ClaimKind};
    let mut app = app();
    let owner = app.world().resource::<ClientSession>().uuid.unwrap();
    let keep = [104, 4, 8];
    {
        let mut session = app.world_mut().resource_mut::<ClientSession>();
        let id = session.kingdoms.found(owner, keep, "Home realm").unwrap();
        for x in (0..6).rev() {
            session
                .kingdoms
                .claim(
                    id,
                    [x, 0],
                    ClaimKind::RealmTerritory,
                    ClaimActor::Player(owner),
                )
                .unwrap();
        }
        session.spawn = Vec3::new(8.5, 3.0, 8.5);
    }
    app.add_systems(Update, opening_frame);
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::KeyK);
    app.update();
    let actions = app
        .world_mut()
        .resource_mut::<Messages<KingdomMenuAction>>()
        .drain()
        .collect::<Vec<_>>();
    assert!(matches!(actions.as_slice(), [KingdomMenuAction::Open(p)] if *p == keep));
    for action in actions {
        app.world_mut().write_message(action);
    }
    update(app.world_mut());
    assert!(app.world().resource::<KingdomView>().visible);
    assert_eq!(
        requests(&mut app),
        vec![KingdomAction::Inspect { position: keep }]
    );
    app.world_mut().resource_mut::<ClientSession>().spawn = Vec3::new(-100.0, 3.0, -100.0);
    update(app.world_mut());
    assert!(!app.world().resource::<KingdomView>().visible);
}

fn act(app: &mut App, action: KingdomMenuAction) {
    app.world_mut().write_message(action);
    update(app.world_mut());
}

fn reply(app: &mut App, outcome: KingdomOutcome) {
    let request = app
        .world()
        .resource::<KingdomMenuState>()
        .pending
        .unwrap()
        .0;
    reply_with_request(app, request, outcome);
}

fn reply_with_request(app: &mut App, request: u64, outcome: KingdomOutcome) {
    {
        let mut session = app.world_mut().resource_mut::<ClientSession>();
        session.kingdom_response += 1;
        session.last_kingdom_result = Some(KingdomResult { request, outcome });
    }
    update(app.world_mut());
}

#[test]
fn held_track_use_does_not_open_bergfried_but_place_and_other_items_still_do() {
    use terra_core::{ControlsConfig, InputBinding, ItemKind, WoodSpecies};
    use terra_voxel::{BlockSection, SectionPos, blocks::BlockHit};
    use terra_world::streaming::{ChunkStore, LoadedSection};
    for (item, use_key, place, opens) in [
        (
            Some(ItemKind::for_wood(196, WoodSpecies::Birch)),
            KeyCode::KeyR,
            false,
            false,
        ),
        (
            Some(ItemKind::for_wood(196, WoodSpecies::Spruce)),
            KeyCode::KeyY,
            false,
            false,
        ),
        (Some(ItemKind::Block(203)), KeyCode::KeyR, false, false),
        (
            Some(ItemKind::for_wood(196, WoodSpecies::Cherry)),
            KeyCode::KeyR,
            true,
            true,
        ),
        (Some(ItemKind::Block(3)), KeyCode::KeyR, false, true),
        (None, KeyCode::KeyR, false, true),
    ] {
        let mut app = app();
        let mut controls = ControlsConfig::default();
        controls
            .rebind(GameAction::Use, InputBinding::Key(use_key))
            .unwrap();
        controls
            .rebind(GameAction::Place, InputBinding::Mouse(MouseButton::Right))
            .unwrap();
        app.insert_resource(controls)
            .insert_resource(terra_player::interaction::ActiveItem(item))
            .add_systems(Update, opening_frame);
        app.world_mut()
            .resource_mut::<terra_player::CameraInputState>()
            .captured = true;
        let mut data = BlockSection::empty(SectionPos::default()).unwrap();
        data.set_block([0; 3], terra_core::castle::BERGFRIED)
            .unwrap();
        let mut chunks = ChunkStore::default();
        chunks.loaded.insert(
            SectionPos::default(),
            LoadedSection {
                data,
                mesh: None,
                mesh_epoch: 0,
                water_epoch: 0,
                water: None,
                props: Vec::new(),
            },
        );
        app.insert_resource(chunks)
            .insert_resource(terra_player::TargetWorld(Some(
                terra_player::WorldTarget::Block(BlockHit {
                    position: [0; 3],
                    normal: [0, 1, 0],
                    distance: 2.0,
                }),
            )));
        if place {
            app.world_mut()
                .resource_mut::<ButtonInput<MouseButton>>()
                .press(MouseButton::Right);
        } else {
            app.world_mut()
                .resource_mut::<ButtonInput<KeyCode>>()
                .press(use_key);
        }
        app.update();
        assert_eq!(
            app.world()
                .resource::<KingdomMenuState>()
                .position
                .is_some(),
            opens
        );
        assert_eq!(app.world().resource::<PlayerInputBlock>().0, opens);
        let actions = app
            .world_mut()
            .resource_mut::<Messages<KingdomMenuAction>>()
            .drain()
            .collect::<Vec<_>>();
        assert_eq!(actions.len(), usize::from(opens));
        assert!(
            actions
                .iter()
                .all(|action| matches!(action, KingdomMenuAction::Open([0, 0, 0])))
        );
    }
}

#[test]
fn late_inspect_and_found_responses_cannot_complete_a_reopened_dialog() {
    for founding in [false, true] {
        let mut app = app();
        act(&mut app, KingdomMenuAction::Open([0, 0, 0]));
        requests(&mut app);
        if founding {
            reply(&mut app, KingdomOutcome::Inspected { kingdom: None });
            act(&mut app, KingdomMenuAction::Name("Oakwatch".into()));
            act(&mut app, KingdomMenuAction::Found);
            requests(&mut app);
        }
        let old = app
            .world()
            .resource::<KingdomMenuState>()
            .pending
            .unwrap()
            .0;
        act(&mut app, KingdomMenuAction::Close);
        act(&mut app, KingdomMenuAction::Open([0, 0, 0]));
        requests(&mut app);
        let new = app
            .world()
            .resource::<KingdomMenuState>()
            .pending
            .unwrap()
            .0;
        assert!(new > old);
        reply_with_request(
            &mut app,
            old,
            if founding {
                KingdomOutcome::Founded { id: KingdomId(1) }
            } else {
                KingdomOutcome::Rejected(terra_core::kingdom::KingdomError::Occluded)
            },
        );
        let view = app.world().resource::<KingdomView>();
        assert!(view.visible && view.pending);
        assert!(view.error.is_empty());
        assert_eq!(view.status, "Waiting for the server…");
        reply(&mut app, KingdomOutcome::Inspected { kingdom: None });
        assert!(!app.world().resource::<KingdomView>().pending);
    }
}

#[test]
fn kingdom_reach_uses_the_same_eight_meter_eye_origin_as_the_server() {
    let mut app = app();
    app.world_mut().resource_mut::<ClientSession>().spawn = Vec3::new(7.5, 0.5, 0.5);
    assert!(near_keep(app.world(), [0, 0, 0]));
    open_empty(&mut app);
    assert!(app.world().resource::<KingdomView>().visible);
    app.world_mut().resource_mut::<ClientSession>().spawn = Vec3::new(9.5, 0.5, 0.5);
    update(app.world_mut());
    assert!(!app.world().resource::<KingdomView>().visible);
    assert!(app.world().resource::<KingdomMenuState>().resume_pending);
    act(&mut app, KingdomMenuAction::Open([0, 0, 0]));
    assert!(requests(&mut app).is_empty());
    assert!(app.world().resource::<KingdomMenuState>().resume_pending);
}

fn open_empty(app: &mut App) {
    act(app, KingdomMenuAction::Open([0, 0, 0]));
    assert!(matches!(
        requests(app).as_slice(),
        [KingdomAction::Inspect {
            position: [0, 0, 0]
        }]
    ));
    assert!(app.world().resource::<KingdomView>().pending);
    reply(app, KingdomOutcome::Inspected { kingdom: None });
}

fn founded(app: &mut App) -> KingdomId {
    let owner = app.world().resource::<ClientSession>().uuid.unwrap();
    let id = app
        .world_mut()
        .resource_mut::<ClientSession>()
        .kingdoms
        .found(owner, [0, 0, 0], "Oakwatch")
        .unwrap();
    act(app, KingdomMenuAction::Open([0, 0, 0]));
    requests(app);
    reply(app, KingdomOutcome::Inspected { kingdom: Some(id) });
    id
}

#[test]
fn founding_waits_for_authority_and_rejected_names_remain_editable() {
    let mut app = app();
    open_empty(&mut app);
    assert!(app.world().resource::<KingdomView>().visible);
    assert!(app.world().resource::<PlayerInputBlock>().0);
    assert!(!app.world().resource::<KingdomView>().can_found);
    act(&mut app, KingdomMenuAction::Found);
    assert!(requests(&mut app).is_empty());
    assert!(!app.world().resource::<KingdomView>().error.is_empty());
    act(&mut app, KingdomMenuAction::Name("  Oakwatch  ".into()));
    assert!(app.world().resource::<KingdomView>().can_found);
    act(&mut app, KingdomMenuAction::Found);
    assert!(matches!(requests(&mut app).as_slice(),
        [KingdomAction::FoundSettlement { position: [0, 0, 0], name, kind: SettlementKind::Kingdom, .. }] if name == "Oakwatch"));
    assert!(
        app.world()
            .resource::<ClientSession>()
            .kingdoms
            .kingdoms
            .is_empty()
    );
    assert!(app.world().resource::<KingdomView>().pending);
    reply(
        &mut app,
        KingdomOutcome::Rejected(terra_core::kingdom::KingdomError::NameTaken),
    );
    assert!(!app.world().resource::<KingdomView>().pending);
    assert!(!app.world().resource::<KingdomView>().error.is_empty());
    act(&mut app, KingdomMenuAction::Name("New Oakwatch".into()));
    assert!(app.world().resource::<KingdomView>().error.is_empty());
}

#[test]
fn dissolve_requires_explicit_confirmation_and_does_not_mutate_the_replica() {
    let mut app = app();
    let id = founded(&mut app);
    assert!(app.world().resource::<KingdomView>().founder_controls);
    act(&mut app, KingdomMenuAction::Dissolve);
    assert!(requests(&mut app).is_empty());
    act(&mut app, KingdomMenuAction::ConfirmDissolve);
    assert!(app.world().resource::<KingdomView>().confirm_dissolve);
    act(&mut app, KingdomMenuAction::CancelDissolve);
    assert!(!app.world().resource::<KingdomView>().confirm_dissolve);
    act(&mut app, KingdomMenuAction::ConfirmDissolve);
    act(&mut app, KingdomMenuAction::Dissolve);
    assert_eq!(requests(&mut app), [KingdomAction::Dissolve { id }]);
    assert_eq!(
        app.world()
            .resource::<ClientSession>()
            .kingdoms
            .get(id)
            .unwrap()
            .state,
        KingdomState::Active
    );
}

#[test]
fn visitors_cannot_issue_founder_actions_but_can_request_membership() {
    let mut app = app();
    let founder = PlayerUuid::from_u128(42044);
    let id = app
        .world_mut()
        .resource_mut::<ClientSession>()
        .kingdoms
        .found(founder, [0, 0, 0], "Riverkeep")
        .unwrap();
    act(&mut app, KingdomMenuAction::Open([0, 0, 0]));
    requests(&mut app);
    reply(&mut app, KingdomOutcome::Inspected { kingdom: Some(id) });
    assert!(!app.world().resource::<KingdomView>().founder_controls);
    assert!(app.world().resource::<KingdomView>().can_join);
    for action in [
        KingdomMenuAction::Rename,
        KingdomMenuAction::ConfirmDissolve,
        KingdomMenuAction::Dissolve,
        KingdomMenuAction::Kick(founder),
        KingdomMenuAction::SaveAppearance,
        KingdomMenuAction::Upgrade,
        KingdomMenuAction::Claim(ClaimKind::ProtectedBuilding),
        KingdomMenuAction::Release,
        KingdomMenuAction::Color("#FF0000".into()),
        KingdomMenuAction::Icon(KingdomIcon::Crown),
    ] {
        act(&mut app, action);
        assert!(requests(&mut app).is_empty());
    }
    act(&mut app, KingdomMenuAction::Join);
    assert_eq!(requests(&mut app), [KingdomAction::Join { id }]);
}

#[test]
fn disconnect_clears_only_owned_focus_and_closes_the_panel() {
    let mut app = app();
    open_empty(&mut app);
    let dialog = app
        .world_mut()
        .spawn(ElementId("kingdom-dialog".into()))
        .id();
    let input = app.world_mut().spawn(ChildOf(dialog)).id();
    app.init_resource::<InputFocus>();
    app.world_mut()
        .resource_mut::<InputFocus>()
        .set(input, FocusCause::Navigated);
    app.world_mut().resource_mut::<ClientSession>().ready = false;
    update(app.world_mut());
    assert!(!app.world().resource::<KingdomView>().visible);
    assert!(!app.world().resource::<KingdomMenuState>().blocks_input());
    assert!(app.world().resource::<InputFocus>().get().is_none());
    assert!(app.world().resource::<ClientSession>().kingdoms == KingdomRegistry::default());
}

#[test]
fn kingdom_template_styles_use_registered_components_and_parse() {
    let shell = include_str!("../../../src-ui/components/kingdom-menu/kingdom-menu.component.html");
    let templates = [
        shell,
        include_str!("../../../src-ui/components/kingdom-general/kingdom-general.component.html"),
        include_str!("../../../src-ui/components/kingdom-members/kingdom-members.component.html"),
        include_str!(
            "../../../src-ui/components/kingdom-diplomacy/kingdom-diplomacy.component.html"
        ),
        include_str!("../../../src-ui/components/kingdom-workers/kingdom-workers.component.html"),
        include_str!("../../../src-ui/components/kingdom-army/kingdom-army.component.html"),
    ];
    for css in [
        include_str!("../../../src-ui/components/kingdom-menu/kingdom-menu.component.css"),
        include_str!("../../../src-ui/components/kingdom-general/kingdom-general.component.css"),
        include_str!("../../../src-ui/components/kingdom-members/kingdom-members.component.css"),
        include_str!(
            "../../../src-ui/components/kingdom-diplomacy/kingdom-diplomacy.component.css"
        ),
        include_str!("../../../src-ui/components/kingdom-workers/kingdom-workers.component.css"),
        include_str!("../../../src-ui/components/kingdom-army/kingdom-army.component.css"),
        include_str!("../../../src-ui/styles/kingdom-sections.css"),
    ] {
        tilt_ui::tilt_ui_css::parse_stylesheet(css).unwrap();
    }
    for name in [
        "kingdom-menu",
        "kingdom-general",
        "kingdom-members",
        "kingdom-diplomacy",
        "kingdom-workers",
        "kingdom-army",
    ] {
        assert!(
            crate::TILT_UI_COMPONENTS
                .iter()
                .any(|component| component.name == name),
            "{name} is not registered"
        );
    }
    for id in [
        "kingdom-dialog",
        "kingdom-name",
        "kingdom-found",
        "kingdom-rename",
        "kingdom-join",
        "kingdom-dissolve",
        "kingdom-kind-village",
        "kingdom-icons",
        "kingdom-icon-search",
        "kingdom-icon-option",
        "kingdom-save-appearance",
        "kingdom-upgrade",
        "kingdom-claim-protected",
        "kingdom-claim-territory",
        "kingdom-release",
        "kingdom-select-member",
        "kingdom-select-realm",
        "kingdom-edit-role",
        "kingdom-set-role",
        "kingdom-relation-allied",
    ] {
        assert!(
            templates
                .iter()
                .any(|template| template.contains(&format!("id=\"{id}\""))),
            "missing real control {id}"
        );
    }
    for (tab, component) in [
        ("overview", "kingdom-general"),
        ("members", "kingdom-members"),
        ("diplomacy", "kingdom-diplomacy"),
        ("building", "building-local"),
        ("work", "kingdom-workers"),
        ("army", "kingdom-army"),
    ] {
        assert!(shell.contains(&format!("id=\"kingdom-tab-{tab}\"")));
        assert!(shell.contains(&format!("<{component}>")));
    }
    let shared = include_str!("../../../src-ui/styles/kingdom-sections.css");
    assert!(shared.contains(".keep-section .keep-action { display: flex; flex-direction: row;"));
    assert!(shared.contains("width: auto;"));
    assert!(shared.contains(".keep-section .keep-action::value"));
}

#[path = "kingdom_claim_tests.rs"]
mod claim_tests;

#[path = "kingdom_claim_policy_tests.rs"]
mod claim_policy_tests;

#[test]
fn member_role_controls_wait_for_authority_and_show_the_exact_capability_matrix() {
    let mut app = app();
    let id = founded(&mut app);
    let owner = app.world().resource::<ClientSession>().uuid.unwrap();
    let member = PlayerUuid::from_u128(42045);
    app.world_mut()
        .resource_mut::<ClientSession>()
        .kingdoms
        .join_with_name(member, "A new member", id)
        .unwrap();
    act(&mut app, KingdomMenuAction::Tab("members".into()));
    let view = app.world().resource::<KingdomView>();
    assert!(view.members_tab && view.manager_controls);
    assert_eq!(view.capabilities.len(), 4);
    assert!(view.capabilities.iter().all(|p| p.allowed));
    let row = view
        .members
        .iter()
        .find(|r| r.id == member.to_string())
        .unwrap();
    assert_eq!(row.role, "Visitor");
    assert_eq!(row.permissions, "No protected-land permissions");
    assert!(row.can_edit_role && row.can_kick);
    act(&mut app, KingdomMenuAction::EditRole(member));
    let row = app
        .world()
        .resource::<KingdomView>()
        .members
        .iter()
        .find(|r| r.id == member.to_string())
        .unwrap();
    assert!(row.editing);
    assert_eq!(row.roles.len(), 5);
    assert!(!row.roles.iter().any(|r| r.key == "founder"));
    act(
        &mut app,
        KingdomMenuAction::SetRole {
            member,
            role: KingdomRole::Worker,
        },
    );
    assert_eq!(
        requests(&mut app),
        [KingdomAction::SetRole {
            id,
            member,
            role: KingdomRole::Worker
        }]
    );
    assert_eq!(
        app.world()
            .resource::<ClientSession>()
            .kingdoms
            .role_for(id, member),
        Some(KingdomRole::Visitor)
    );
    app.world_mut()
        .resource_mut::<ClientSession>()
        .kingdoms
        .set_role(owner, id, member, KingdomRole::Worker)
        .unwrap();
    reply(
        &mut app,
        KingdomOutcome::RoleChanged {
            id,
            member,
            role: KingdomRole::Worker,
        },
    );
    let row = app
        .world()
        .resource::<KingdomView>()
        .members
        .iter()
        .find(|r| r.id == member.to_string())
        .unwrap();
    assert_eq!(row.role, "Worker");
    assert_eq!(row.permissions, "Inventories · NPC Work");
    assert!(!row.editing);
}
#[test]
fn officer_and_visitor_controls_follow_live_roles_without_escalation_or_stale_editors() {
    let mut app = app();
    let id = founded(&mut app);
    let founder = app.world().resource::<ClientSession>().uuid.unwrap();
    let officer = PlayerUuid::from_u128(7);
    let member = PlayerUuid::from_u128(8);
    {
        let mut session = app.world_mut().resource_mut::<ClientSession>();
        session.kingdoms.join(officer, id).unwrap();
        session.kingdoms.join(member, id).unwrap();
        session
            .kingdoms
            .set_role(founder, id, officer, KingdomRole::Officer)
            .unwrap();
        session.uuid = Some(officer);
    }
    act(&mut app, KingdomMenuAction::Tab("members".into()));
    let view = app.world().resource::<KingdomView>();
    assert!(view.manager_controls);
    assert!(!view.founder_controls);
    assert!(
        view.members
            .iter()
            .filter(|r| r.id == founder.to_string() || r.id == officer.to_string())
            .all(|r| !r.can_edit_role && !r.can_kick)
    );
    let row = view
        .members
        .iter()
        .find(|r| r.id == member.to_string())
        .unwrap();
    assert!(
        !row.roles
            .iter()
            .any(|r| r.key == "officer" || r.key == "founder")
    );
    for action in [
        KingdomMenuAction::SetRole {
            member,
            role: KingdomRole::Officer,
        },
        KingdomMenuAction::Kick(founder),
        KingdomMenuAction::SetRole {
            member: officer,
            role: KingdomRole::Founder,
        },
        KingdomMenuAction::ConfirmDissolve,
    ] {
        act(&mut app, action);
        assert!(requests(&mut app).is_empty());
    }
    act(&mut app, KingdomMenuAction::OpenIcons);
    assert!(app.world().resource::<KingdomView>().icon_picker);
    app.world_mut()
        .resource_mut::<ClientSession>()
        .kingdoms
        .set_role(founder, id, officer, KingdomRole::Visitor)
        .unwrap();
    update(app.world_mut());
    let view = app.world().resource::<KingdomView>();
    assert!(!view.manager_controls && !view.icon_picker);
    assert!(view.capabilities.iter().all(|p| !p.allowed));
    assert!(view.members.iter().all(|r| !r.can_edit_role && !r.can_kick));
    for action in [
        KingdomMenuAction::Rename,
        KingdomMenuAction::Claim(ClaimKind::ProtectedBuilding),
        KingdomMenuAction::SetRole {
            member,
            role: KingdomRole::Worker,
        },
        KingdomMenuAction::SetRelation {
            target: DiplomaticTarget::Bot(BotKingdom::Helteria),
            relation: DiplomaticRelation::Allied,
        },
    ] {
        act(&mut app, action);
        assert!(requests(&mut app).is_empty());
    }
    app.world_mut()
        .resource_mut::<ClientSession>()
        .kingdoms
        .set_role(founder, id, officer, KingdomRole::Member)
        .unwrap();
    update(app.world_mut());
    let view = app.world().resource::<KingdomView>();
    assert_eq!(view.own_role, "Member");
    assert!(!view.manager_controls);
    assert!(
        view.capabilities
            .iter()
            .all(|permission| permission.allowed)
    );
    assert!(
        view.members
            .iter()
            .all(|member| !member.can_edit_role && !member.can_kick)
    );
}
#[test]
fn diplomacy_distinguishes_offers_from_effective_relations_and_explains_npc_conditions() {
    let mut app = app();
    let id = founded(&mut app);
    let owner = app.world().resource::<ClientSession>().uuid.unwrap();
    let other = PlayerUuid::from_u128(99);
    let foreign = app
        .world_mut()
        .resource_mut::<ClientSession>()
        .kingdoms
        .found(other, [32, 0, 0], "Another realm")
        .unwrap();
    act(&mut app, KingdomMenuAction::Tab("diplomacy".into()));
    let view = app.world().resource::<KingdomView>();
    assert!(view.diplomacy_tab);
    assert_eq!(view.diplomacy.len(), 5);
    assert!(
        !view
            .diplomacy
            .iter()
            .find(|r| r.key == "bot:petora")
            .unwrap()
            .can_ally
    );
    let target = DiplomaticTarget::Player(foreign);
    act(
        &mut app,
        KingdomMenuAction::SetRelation {
            target,
            relation: DiplomaticRelation::Allied,
        },
    );
    assert_eq!(
        requests(&mut app),
        [KingdomAction::SetRelation {
            id,
            target,
            relation: DiplomaticRelation::Allied
        }]
    );
    app.world_mut()
        .resource_mut::<ClientSession>()
        .kingdoms
        .set_relation(owner, id, target, DiplomaticRelation::Allied)
        .unwrap();
    reply(
        &mut app,
        KingdomOutcome::RelationChanged {
            id,
            target,
            relation: DiplomaticRelation::Allied,
        },
    );
    let key = target_key(target);
    let row = app
        .world()
        .resource::<KingdomView>()
        .diplomacy
        .iter()
        .find(|r| r.key == key)
        .unwrap();
    assert!(row.pending_alliance);
    assert_eq!(row.relation_key, "neutral");
    assert_eq!(row.offered, "Allied");
    app.world_mut()
        .resource_mut::<ClientSession>()
        .kingdoms
        .set_relation(
            other,
            foreign,
            DiplomaticTarget::Player(id),
            DiplomaticRelation::Allied,
        )
        .unwrap();
    update(app.world_mut());
    let row = app
        .world()
        .resource::<KingdomView>()
        .diplomacy
        .iter()
        .find(|r| r.key == key)
        .unwrap();
    assert_eq!(row.relation_key, "allied");
    assert!(!row.pending_alliance);
    app.world_mut()
        .resource_mut::<ClientSession>()
        .kingdoms
        .set_relation(
            owner,
            id,
            DiplomaticTarget::Bot(BotKingdom::Lutera),
            DiplomaticRelation::Allied,
        )
        .unwrap();
    update(app.world_mut());
    let row = app
        .world()
        .resource::<KingdomView>()
        .diplomacy
        .iter()
        .find(|r| r.key == "bot:lutera")
        .unwrap();
    assert!(row.pending_alliance && row.hint.contains("level 2"));
    app.world_mut()
        .resource_mut::<ClientSession>()
        .kingdoms
        .kingdoms[0]
        .level = 2;
    update(app.world_mut());
    let row = app
        .world()
        .resource::<KingdomView>()
        .diplomacy
        .iter()
        .find(|r| r.key == "bot:lutera")
        .unwrap();
    assert_eq!(row.relation_key, "allied");
    assert!(!row.pending_alliance);
}

#[test]
fn member_and_realm_selection_are_local_and_only_accept_current_registry_rows() {
    let mut app = app();
    let id = founded(&mut app);
    let member = PlayerUuid::from_u128(765);
    app.world_mut()
        .resource_mut::<ClientSession>()
        .kingdoms
        .join(member, id)
        .unwrap();
    act(&mut app, KingdomMenuAction::Tab("members".into()));
    act(&mut app, KingdomMenuAction::SelectMember(member));
    assert!(requests(&mut app).is_empty());
    assert!(
        app.world()
            .resource::<KingdomView>()
            .members
            .iter()
            .find(|row| row.id == member.to_string())
            .unwrap()
            .selected
    );
    act(
        &mut app,
        KingdomMenuAction::SelectMember(PlayerUuid::from_u128(999)),
    );
    assert_eq!(
        app.world().resource::<KingdomMenuState>().selected_member,
        Some(member)
    );
    act(&mut app, KingdomMenuAction::Tab("diplomacy".into()));
    act(
        &mut app,
        KingdomMenuAction::SelectRealm("bot:lutera".into()),
    );
    assert!(
        app.world()
            .resource::<KingdomView>()
            .diplomacy
            .iter()
            .find(|row| row.key == "bot:lutera")
            .unwrap()
            .selected
    );
    act(
        &mut app,
        KingdomMenuAction::SelectRealm("player:999999".into()),
    );
    assert_eq!(
        app.world()
            .resource::<KingdomMenuState>()
            .selected_realm
            .as_deref(),
        Some("bot:lutera")
    );
    act(&mut app, KingdomMenuAction::Tab("army".into()));
    let view = app.world().resource::<KingdomView>();
    assert!(
        view.army_tab
            && !view.overview_tab
            && !view.members_tab
            && !view.diplomacy_tab
            && !view.work_tab
            && !view.building_tab
    );
    assert!(requests(&mut app).is_empty());
}
