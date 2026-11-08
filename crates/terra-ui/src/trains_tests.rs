use super::*;
use terra_core::{
    GameMode, Inventory, PlayerUuid, WoodSpecies,
    trains::{TrainPart, VehicleView},
};
use terra_network::{
    client::TrainRequest,
    protocol::trains::{TrainCargoSnapshot, TrainResult},
};

fn app() -> App {
    let mut app = App::new();
    let mut session = ClientSession::default();
    session.ready = true;
    session.game_mode = GameMode::Creative;
    session.uuid = Some(PlayerUuid::from_u128(1));
    app.insert_resource(session)
        .insert_resource(crate::menu::MenuState {
            page: crate::menu::MenuPage::Playing,
            ..default()
        })
        .init_resource::<terra_player::PlayerInputBlock>()
        .init_resource::<ButtonInput<KeyCode>>()
        .init_resource::<ButtonInput<MouseButton>>()
        .add_message::<terra_player::CaptureRequest>();
    install(&mut app);
    app
}
fn car(id: u64) -> VehicleView {
    VehicleView {
        id,
        owner: PlayerUuid::from_u128(1),
        kind: VehicleKind::Freight,
        anchor: [0, 0, 0],
        parts: vec![TrainPart {
            offset: [0, 0, 0],
            material: 216,
            facing: 0,
            wood: WoodSpecies::Oak,
        }],
        couplings: [None, None],
        revision: 7,
        mass_kg: 2000,
    }
}
fn open(app: &mut App) {
    let mut state = app.world_mut().resource_mut::<TrainUiState>();
    state.open = true;
    state.station = Some([0, 0, 0]);
}
fn actions(app: &mut App, command: &str) {
    app.world_mut()
        .write_message(TrainUiAction::Command(command.into()));
    app.update();
}
fn requests(app: &mut App) -> Vec<TrainRequest> {
    app.world_mut()
        .resource_mut::<Messages<TrainRequest>>()
        .drain()
        .collect()
}

#[test]
fn responsive_vehicle_pages_are_sorted_and_clamped_without_scrollbars() {
    let mut app = app();
    app.world_mut()
        .resource_mut::<ClientSession>()
        .trains
        .vehicles = (1..=14).rev().map(car).collect();
    open(&mut app);
    app.update();
    let first = app.world().resource::<TrainView>();
    assert_eq!(
        first.rows.iter().map(|row| row.id).collect::<Vec<_>>(),
        vec![1, 2, 3, 4, 5, 6]
    );
    assert_eq!(first.page, "1 / 3");
    actions(&mut app, "next");
    assert_eq!(app.world().resource::<TrainView>().rows[0].id, 7);
    let window = app
        .world_mut()
        .spawn(Window {
            resolution: (1280, 720).into(),
            ..default()
        })
        .id();
    app.insert_resource(UiScale(1.5));
    app.update();
    let compact = app.world().resource::<TrainView>();
    assert_eq!(
        compact.rows.iter().map(|row| row.id).collect::<Vec<_>>(),
        vec![4, 5, 6]
    );
    assert_eq!(compact.page, "2 / 5");
    app.world_mut()
        .get_mut::<Window>(window)
        .unwrap()
        .resolution
        .set(3840., 2160.);
    app.insert_resource(UiScale(2.));
    actions(&mut app, "next");
    actions(&mut app, "next");
    assert_eq!(app.world().resource::<TrainView>().page, "3 / 3");
    assert_eq!(app.world().resource::<TrainView>().rows.len(), 2);
    actions(&mut app, "previous");
    assert_eq!(app.world().resource::<TrainView>().page, "2 / 3");
}

#[test]
fn private_cargo_uses_shared_previews_and_acknowledged_revisions_without_local_moves() {
    let mut app = app();
    let mut cargo = Inventory::default();
    cargo.slots.push(ItemStack {
        item: terra_core::ItemKind::Block(3),
        count: 12,
    });
    {
        let mut session = app.world_mut().resource_mut::<ClientSession>();
        session.trains.vehicles.push(car(1));
        session.inventory.creative_set(
            0,
            ItemStack {
                item: terra_core::ItemKind::Apple,
                count: 3,
            },
        );
        session.train_cargo = Some(TrainCargoSnapshot {
            vehicle: 1,
            part: 0,
            revision: 7,
            inventory: cargo.clone(),
        });
    }
    open(&mut app);
    app.world_mut().resource_mut::<TrainUiState>().cargo = true;
    let inventory = app.world().resource::<ClientSession>().inventory.clone();
    app.update();
    let view = app.world().resource::<TrainView>();
    assert_eq!(view.cargo_slots.len(), 18);
    assert_eq!(view.player_slots.len(), 27);
    assert_eq!(view.cargo_slots[0].preview, "Block preview 3");
    actions(&mut app, "slot:27");
    let sent = requests(&mut app);
    assert_eq!(sent.len(), 1);
    assert_eq!(
        sent[0].0.action,
        TrainAction::Cargo {
            vehicle: 1,
            part: 0,
            revision: 7,
            player_revision: inventory.revision,
            gesture: ContainerGesture::QuickMove { slot: 27 },
            reserved: [0; 27]
        }
    );
    actions(&mut app, "slot:27");
    assert!(
        requests(&mut app).is_empty(),
        "one request remains pending until its matching response"
    );
    assert_eq!(app.world().resource::<ClientSession>().inventory, inventory);
    assert_eq!(
        app.world()
            .resource::<ClientSession>()
            .train_cargo
            .as_ref()
            .unwrap()
            .inventory,
        cargo
    );
    app.world_mut()
        .resource_mut::<ClientSession>()
        .last_train_result = Some(TrainResult {
        request: sent[0].0.request + 1,
        outcome: TrainOutcome::Inspected,
    });
    app.update();
    assert!(app.world().resource::<TrainView>().pending);
    app.world_mut()
        .resource_mut::<ClientSession>()
        .last_train_result = Some(TrainResult {
        request: sent[0].0.request,
        outcome: TrainOutcome::Rejected(TrainError::Stale),
    });
    app.update();
    let refresh = requests(&mut app);
    assert_eq!(refresh.len(), 1);
    assert_eq!(
        refresh[0].0.action,
        TrainAction::OpenCargo {
            vehicle: 1,
            part: 0
        }
    );
}

#[test]
fn closing_consumes_escape_and_disconnect_never_recaptures_the_cursor() {
    let mut app = app();
    open(&mut app);
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::Escape);
    app.update();
    assert!(!app.world().resource::<TrainView>().open);
    assert!(app.world().resource::<TrainUiState>().consumed_escape);
    let capture: Vec<_> = app
        .world_mut()
        .resource_mut::<Messages<terra_player::CaptureRequest>>()
        .drain()
        .collect();
    assert_eq!(capture.len(), 1);
    assert!(capture[0].0);
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .clear();
    open(&mut app);
    app.world_mut().resource_mut::<ClientSession>().ready = false;
    app.world_mut()
        .resource_mut::<crate::menu::MenuState>()
        .page = crate::menu::MenuPage::Main;
    app.update();
    assert!(!app.world().resource::<TrainView>().open);
    assert!(
        app.world_mut()
            .resource_mut::<Messages<terra_player::CaptureRequest>>()
            .drain()
            .next()
            .is_none()
    );
}

#[test]
fn coupling_uses_two_explicit_end_choices_and_registry_revision() {
    let mut app = app();
    {
        let mut session = app.world_mut().resource_mut::<ClientSession>();
        session.trains.vehicles = vec![car(1), car(2)];
        session.trains.revision = 9;
    }
    open(&mut app);
    actions(&mut app, "select:1");
    actions(&mut app, "select:2");
    actions(&mut app, "front");
    actions(&mut app, "couple");
    let requests = requests(&mut app);
    assert_eq!(requests.len(), 1);
    assert_eq!(
        requests[0].0.action,
        TrainAction::Couple {
            station: [0, 0, 0],
            first: 1,
            first_end: CouplerEnd::Front,
            second: 2,
            second_end: CouplerEnd::Rear,
            revision: 9
        }
    );
    assert!(app.world().resource::<terra_player::PlayerInputBlock>().0);
}

#[test]
fn leaving_a_train_seat_uses_the_configured_action_and_one_confirmed_request() {
    let mut app = app();
    app.world_mut().resource_mut::<ClientSession>().train_seat =
        Some(terra_core::trains::SeatBinding {
            player: PlayerUuid::from_u128(1),
            vehicle: 1,
            part: 0,
        });
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::ShiftLeft);
    app.update();
    let sent = requests(&mut app);
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0].0.action, TrainAction::Leave);
    app.update();
    assert!(requests(&mut app).is_empty());
    assert!(
        app.world().resource::<ClientSession>().train_seat.is_some(),
        "the client never speculatively clears its authoritative seat"
    );
}
