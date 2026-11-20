use super::selection::{self, SelectionKind};
use super::*;
use terra_player::{CaptureRequest, PlayerInputBlock, TargetBlock, WorldInteractionBlock};

fn picking_app() -> App {
    let mut app = App::new();
    app.init_resource::<BuildingUiState>()
        .init_resource::<crate::npc_work::WorkUiState>()
        .init_resource::<WorldInteractionBlock>()
        .init_resource::<PlayerInputBlock>()
        .init_resource::<TargetBlock>()
        .init_resource::<ButtonInput<MouseButton>>()
        .init_resource::<ButtonInput<KeyCode>>()
        .init_resource::<crate::KingdomView>()
        .init_resource::<Time<Real>>()
        .add_message::<CaptureRequest>()
        .add_message::<crate::KingdomMenuAction>()
        .add_message::<BuildingRequest>();
    let mut session = ClientSession::default();
    session.ready = true;
    app.insert_resource(session);
    app
}

fn click(app: &mut App, position: [i64; 3]) {
    app.world_mut().resource_mut::<TargetBlock>().0 = Some(terra_voxel::blocks::BlockHit {
        position,
        normal: [0, 1, 0],
        distance: 2.0,
    });
    let mut mouse = app.world_mut().resource_mut::<ButtonInput<MouseButton>>();
    mouse.release(MouseButton::Left);
    mouse.clear();
    mouse.press(MouseButton::Left);
}

#[test]
fn storage_rectangle_includes_chests_above_selected_negative_ground_but_residential_keeps_surface_corridor()
 {
    let storage =
        selection::selected_area(SelectionKind::Storage, [-1, 7, -3], [-5, 6, -8]).unwrap();
    assert_eq!(storage.min, [-5, 6, -8]);
    assert_eq!(storage.max, [-1, 8, -3]);
    assert!(storage.contains([-3, 8, -5]));
    let residential =
        selection::selected_area(SelectionKind::Residential, [-1, 7, -3], [-5, 6, -8]).unwrap();
    assert_eq!(residential.max, [-1, 7, -3]);
    assert!(selection::selected_area(SelectionKind::Storage, [0, 0, 0], [64, 0, 0]).is_err());
}

#[test]
fn building_world_selection_sets_only_draft_and_blocks_final_click_until_keep_returns() {
    let mut app = picking_app();
    app.world_mut()
        .resource_scope(|world, mut state: Mut<BuildingUiState>| {
            state.keep = Some([2, 5, 3]);
            selection::begin(world, &mut state, SelectionKind::Storage);
            state.armed = true;
        });
    assert!(
        app.world()
            .resource::<crate::npc_work::WorkUiState>()
            .blocks_input()
    );
    assert!(
        app.world()
            .resource::<crate::npc_work::WorkUiState>()
            .selection_active()
    );
    click(&mut app, [-5, 7, -2]);
    selection::pick(app.world_mut());
    assert!(app.world().resource::<BuildingUiState>().storage.is_none());
    click(&mut app, [-1, 7, -7]);
    selection::pick(app.world_mut());
    let state = app.world().resource::<BuildingUiState>();
    assert_eq!(
        state.storage,
        Some(BuildArea::new([-5, 7, -7], [-1, 8, -2]).unwrap())
    );
    assert!(state.returning && state.selection.is_none());
    assert!(app.world().resource::<WorldInteractionBlock>().0);
    assert!(app.world().resource::<PlayerInputBlock>().0);
    assert!(
        app.world_mut()
            .resource_mut::<Messages<BuildingRequest>>()
            .drain()
            .next()
            .is_none(),
        "picking never silently submits a material-consuming project"
    );
    let actions: Vec<_> = app
        .world_mut()
        .resource_mut::<Messages<crate::KingdomMenuAction>>()
        .drain()
        .collect();
    assert!(
        actions
            .iter()
            .any(|action| matches!(action, crate::KingdomMenuAction::Open([2, 5, 3])))
    );
    assert!(
        !actions
            .iter()
            .any(|action| matches!(action, crate::KingdomMenuAction::Tab(_)))
    );
    {
        let mut keep = app.world_mut().resource_mut::<crate::KingdomView>();
        keep.visible = true;
        keep.bound = true;
        keep.pending = true;
    }
    selection::input(app.world_mut());
    assert!(
        app.world().resource::<BuildingUiState>().returning,
        "wait for the authoritative keep inspection before switching tabs"
    );
    app.world_mut().resource_mut::<crate::KingdomView>().pending = false;
    selection::input(app.world_mut());
    assert!(!app.world().resource::<BuildingUiState>().returning);
    assert!(
        app.world_mut()
            .resource_mut::<Messages<crate::KingdomMenuAction>>()
            .drain()
            .any(|action| matches!(action,crate::KingdomMenuAction::Tab(tab) if tab=="building"))
    );
}

#[test]
fn escape_keeps_previous_building_draft_and_consumes_pause_while_returning_to_keep() {
    let mut app = picking_app();
    let mut session = ClientSession::default();
    session.ready = true;
    app.insert_resource(session);
    let previous = BuildArea::new([-4, 3, -4], [4, 4, 4]).unwrap();
    app.world_mut()
        .resource_scope(|world, mut state: Mut<BuildingUiState>| {
            state.storage = Some(previous);
            state.keep = Some([0, 3, 0]);
            selection::begin(world, &mut state, SelectionKind::Storage);
        });
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::Escape);
    selection::input(app.world_mut());
    assert_eq!(
        app.world().resource::<BuildingUiState>().storage,
        Some(previous)
    );
    assert!(app.world().resource::<BuildingUiState>().returning);
    assert!(
        app.world()
            .resource::<crate::npc_work::WorkUiState>()
            .consumed_escape
    );
    assert!(app.world().resource::<WorldInteractionBlock>().0);
}
