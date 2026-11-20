use super::*;
use terra_network::protocol::npc_work::{WorkSnapshot, WorkerDetail};

fn fixture() -> App {
    let mut app = super::super::tests::app();
    app.world_mut().resource_mut::<crate::KingdomView>().visible = false;
    app.init_resource::<ButtonInput<KeyCode>>()
        .init_resource::<ButtonInput<MouseButton>>()
        .init_resource::<terra_player::TargetBlock>()
        .init_resource::<PlayerInputBlock>()
        .init_resource::<WorldInteractionBlock>()
        .add_message::<CaptureRequest>();
    {
        let mut state = app.world_mut().resource_mut::<WorkUiState>();
        state.direct = true;
        state.selected = Some(7);
    }
    app.update();
    app.world_mut()
        .resource_mut::<Messages<NpcWorkRequest>>()
        .clear();
    app
}

fn target(app: &mut App, position: [i64; 3]) {
    app.world_mut()
        .resource_mut::<terra_player::TargetBlock>()
        .0 = Some(terra_voxel::blocks::BlockHit {
        position,
        normal: [0, 1, 0],
        distance: 3.0,
    });
    let mut mouse = app.world_mut().resource_mut::<ButtonInput<MouseButton>>();
    mouse.release(MouseButton::Left);
    mouse.clear();
    mouse.press(MouseButton::Left);
}

#[test]
fn direct_dialog_reuses_private_authority_and_world_area_only_changes_the_local_draft() {
    let mut app = fixture();
    let authority = app.world().resource::<ClientSession>().work.clone();
    app.world_mut().write_message(NpcWorkUiAction::PickArea);
    app.update();
    let hud = app.world().resource::<NpcWorkView>();
    assert!(!hud.visible && hud.selection_active && hud.selection_hint.contains("erste Ecke"));
    input_frame(app.world_mut());
    assert!(
        !app.world().resource::<PlayerInputBlock>().0,
        "world selection permits movement"
    );
    assert!(
        app.world().resource::<WorldInteractionBlock>().0,
        "world selection forbids edits"
    );
    target(&mut app, [-3, 11, -5]);
    pick(app.world_mut());
    assert!(matches!(
        app.world().resource::<WorkUiState>().selection,
        Some(Selection::Area {
            first: Some([-3, 11, -5])
        })
    ));
    target(&mut app, [-1, 13, -2]);
    pick(app.world_mut());
    app.update();
    let view = app.world().resource::<NpcWorkView>();
    assert!(view.direct && view.visible && !view.selection_active);
    assert_eq!(
        (
            view.min_x.as_str(),
            view.max_y.as_str(),
            view.max_z.as_str()
        ),
        ("-3", "13", "-2")
    );
    assert_eq!(app.world().resource::<ClientSession>().work, authority);
    assert!(
        !app.world_mut()
            .resource_mut::<Messages<NpcWorkRequest>>()
            .drain()
            .any(|request| matches!(request.0.action, WorkAction::Assign { .. })),
        "selection does not silently submit an order"
    );
}

#[test]
fn escape_cancels_area_selection_without_overwriting_the_order_and_consumes_the_pause_key() {
    let mut app = fixture();
    let original = app.world().resource::<WorkUiState>().draft.min.clone();
    app.world_mut().write_message(NpcWorkUiAction::PickArea);
    app.update();
    input_frame(app.world_mut());
    target(&mut app, [-3, 11, -5]);
    pick(app.world_mut());
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::Escape);
    input_frame(app.world_mut());
    app.update();
    let state = app.world().resource::<WorkUiState>();
    assert!(state.direct && state.selection.is_none() && state.consumed_escape);
    assert_eq!(state.draft.min, original);
    assert!(app.world().resource::<PlayerInputBlock>().0);
}

#[test]
fn incomplete_or_oversized_world_selection_preserves_the_draft_and_stays_in_selection_mode() {
    let mut app = fixture();
    let original = app.world().resource::<WorkUiState>().draft.min.clone();
    app.world_mut().write_message(NpcWorkUiAction::PickArea);
    app.update();
    input_frame(app.world_mut());
    target(&mut app, [-3, 11, -5]);
    pick(app.world_mut());
    target(&mut app, [100, 11, -5]);
    pick(app.world_mut());
    let state = app.world().resource::<WorkUiState>();
    assert!(state.selection.is_some() && !state.error.is_empty());
    assert_eq!(state.draft.min, original);
}

#[test]
fn real_private_inventory_pads_only_authoritative_slots_and_is_never_invented_while_loading() {
    let mut app = fixture();
    assert!(inventory_slots(app.world().resource::<ClientSession>(), Some(7), None).is_empty());
    let mut worker = terra_core::npc_jobs::WorkerState::default();
    assert_eq!(worker.insert(terra_core::ItemKind::Block(3), 17), 0);
    app.world_mut().resource_mut::<ClientSession>().work.detail = Some(WorkerDetail {
        npc: 7,
        inventory: worker.inventory.clone(),
    });
    let slots = inventory_slots(app.world().resource::<ClientSession>(), Some(7), None);
    assert_eq!(slots.len(), 9);
    assert_eq!(slots[0].count, "17");
    assert!(!slots[0].empty && slots[1..].iter().all(|slot| slot.empty));
    assert_eq!(
        slots[0].identifier,
        terra_core::ItemKind::Block(3).identifier()
    );
    assert!(inventory_slots(app.world().resource::<ClientSession>(), Some(8), None).is_empty());
    assert!(app.world().resource::<ClientSession>().work.valid());
    let invalid = WorkSnapshot {
        detail: Some(WorkerDetail {
            npc: 8,
            inventory: worker.inventory,
        }),
        ..app.world().resource::<ClientSession>().work.clone()
    };
    assert!(
        !invalid.valid(),
        "detail must refer to a visible authorized worker"
    );
}

#[test]
fn direct_dialog_and_hud_parse_with_real_tilt_loaders() {
    for css in [
        include_str!("../../../../src-ui/components/npc-work-dialog/npc-work-dialog.component.css"),
        include_str!(
            "../../../../src-ui/components/npc-work-selection/npc-work-selection.component.css"
        ),
    ] {
        tilt_ui::tilt_ui_css::parse_stylesheet(css).unwrap();
    }
}

#[test]
fn surface_area_includes_only_the_first_level_above_ground_and_retains_world_and_height_limits() {
    for kind in [WorkKind::Farmer, WorkKind::Collector, WorkKind::Lumberjack] {
        let area = selected_area(kind, [-4, -1, -3], [2, -1, 3]).unwrap();
        assert_eq!(area.min, [-4, -1, -3]);
        assert_eq!(area.max, [2, 0, 3]);
        assert!(selected_area(kind, [0, 0, 0], [1, 15, 1]).is_err());
        assert!(selected_area(kind, [0, 999_000, 0], [1, 999_000, 1]).is_err());
    }
    let exact = selected_area(WorkKind::Miner, [-4, -1, -3], [2, -1, 3]).unwrap();
    assert_eq!(exact.min[1], exact.max[1]);
    assert!(selected_area(WorkKind::Miner, [0, 0, 0], [1, 15, 1]).is_ok());
}

#[test]
fn right_click_on_a_civilian_opens_the_direct_panel_and_blocks_generic_greet_or_build_input() {
    use terra_core::creatures::{Creature, CreatureBehavior, CreatureKind, CreatureNeeds};
    use terra_network::client::CreatureTrack;
    let mut app = fixture();
    app.world_mut().resource_mut::<WorkUiState>().direct = false;
    let mut camera = terra_player::CameraInputState::default();
    camera.captured = true;
    app.insert_resource(camera)
        .insert_resource(terra_player::TargetCreature(Some(7)))
        .init_resource::<ReplicatedCreatures>()
        .init_resource::<terra_network::client::CreatureActions>();
    let npc = Creature {
        id: 7,
        kind: CreatureKind::Villager,
        home: [0, 0],
        position: [2.5, 1.0, 2.5],
        velocity: [0.0; 3],
        yaw: 0.0,
        health: CreatureKind::Villager.health(),
        behavior: CreatureBehavior::Rest,
        timer: 0.0,
        age: 0.0,
        variant: terra_core::creatures::villagers::UNEMPLOYED,
        needs: CreatureNeeds::full(CreatureKind::Villager),
    };
    app.world_mut()
        .resource_mut::<ReplicatedCreatures>()
        .creatures
        .insert(
            7,
            CreatureTrack {
                playback: Default::default(),
                previous: npc.clone(),
                current: npc,
                received: std::time::Instant::now(),
                interval: 0.1,
            },
        );
    app.world_mut()
        .resource_mut::<terra_network::client::CreatureActions>()
        .0
        .push_back((7, true));
    app.world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .press(MouseButton::Right);
    input_frame(app.world_mut());
    app.update();
    assert!(app.world().resource::<NpcWorkView>().direct);
    assert!(app.world().resource::<PlayerInputBlock>().0);
    assert!(
        app.world()
            .resource::<terra_network::client::CreatureActions>()
            .0
            .is_empty()
    );
    assert!(
        app.world_mut()
            .resource_mut::<Messages<NpcWorkRequest>>()
            .drain()
            .any(|request| matches!(request.0.action, WorkAction::InspectNpc { npc: 7 }))
    );
}

#[test]
fn initial_private_inspection_rejection_is_visible_without_blocking_periodic_refresh() {
    use terra_network::protocol::npc_work::{WorkError, WorkResult};
    let mut app = fixture();
    app.world_mut().resource_mut::<WorkUiState>().was_visible = false;
    app.update();
    let request = app
        .world_mut()
        .resource_mut::<Messages<NpcWorkRequest>>()
        .drain()
        .find(|request| matches!(request.0.action, WorkAction::InspectNpc { npc: 7 }))
        .unwrap();
    assert!(app.world().resource::<WorkUiState>().pending.is_none());
    app.world_mut()
        .resource_mut::<ClientSession>()
        .last_work_result = Some(WorkResult {
        request: request.0.request,
        error: Some(WorkError::Protected),
    });
    app.update();
    assert!(
        app.world()
            .resource::<NpcWorkView>()
            .error
            .contains("Berechtigung")
    );
    assert!(!app.world().resource::<NpcWorkView>().pending);
    app.world_mut()
        .resource_mut::<Time<Real>>()
        .advance_by(std::time::Duration::from_secs(3));
    app.update();
    let refreshed = app
        .world_mut()
        .resource_mut::<Messages<NpcWorkRequest>>()
        .drain()
        .find(|request| matches!(request.0.action, WorkAction::InspectNpc { npc: 7 }))
        .unwrap();
    assert!(refreshed.0.request > request.0.request);
    app.world_mut()
        .resource_mut::<ClientSession>()
        .last_work_result = Some(WorkResult {
        request: refreshed.0.request,
        error: None,
    });
    app.update();
    assert!(app.world().resource::<NpcWorkView>().error.is_empty());
}
