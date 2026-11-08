use super::tests::{actions, app, car, open, requests, track_fixture};
use super::*;
use terra_core::{Inventory, PlayerUuid, trains::SeatBinding};
use terra_network::protocol::trains::{TrainCargoSnapshot, TrainResult};

fn driver_app() -> App {
    let mut app = app();
    let mut vehicle = car(1);
    vehicle.kind = VehicleKind::Locomotive;
    vehicle.anchor = [0, 2, 0];
    vehicle.parts[0].material = 214;
    vehicle.parts.push(terra_core::trains::TrainPart {
        offset: [0, 0, 1],
        material: 218,
        facing: 0,
        wood: Default::default(),
    });
    vehicle.motion.speed_mps = 2.75;
    vehicle.motion.fuel_available = true;
    {
        let mut session = app.world_mut().resource_mut::<ClientSession>();
        session.trains.vehicles.push(vehicle);
        session.train_seat = Some(SeatBinding {
            player: PlayerUuid::from_u128(1),
            vehicle: 1,
            part: 0,
        });
    }
    let mut camera = terra_player::CameraInputState::default();
    camera.captured = true;
    app.insert_resource(camera);
    app
}

fn clear_keys(app: &mut App) {
    let mut keys = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
    keys.release_all();
    keys.clear();
}

fn html_change(app: &mut App, id: &str, value: &str) {
    let target = app.world_mut().spawn(tilt_ui::ElementId(id.into())).id();
    let handler = tilt_ui::inventory::iter::<tilt_ui::HtmlHandlerRegistration>
        .into_iter()
        .find(|handler| handler.name == "train_workshop_action")
        .unwrap();
    let system = (handler.register)(app.world_mut());
    app.world_mut()
        .run_system_with(
            system,
            tilt_ui::HtmlEvent {
                target,
                kind: "change",
                value: Some(value.into()),
                submitter: None,
                data: Default::default(),
                form_data: None,
                handler: handler.name.into(),
            },
        )
        .unwrap();
    app.update();
}

#[test]
fn driving_hud_uses_authoritative_meter_speed_and_configured_controls_then_clears_on_exit() {
    let mut app = driver_app();
    app.update();
    let view = app.world().resource::<TrainView>();
    assert!(view.riding && view.driver);
    assert_eq!(view.drive_speed, "2,8 Blöcke/s");
    assert_eq!(view.drive_levels.len(), 7);
    assert!(view.drive_hint.contains('W') && view.drive_hint.contains('S'));
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::KeyW);
    app.update();
    let sent = requests(&mut app);
    assert_eq!(sent.len(), 1);
    assert_eq!(
        sent[0].0.action,
        TrainAction::Drive {
            vehicle: 1,
            throttle: -1,
            brake: false
        }
    );
    assert_eq!(
        app.world().resource::<ClientSession>().trains.vehicles[0]
            .motion
            .speed_mps,
        2.75
    );
    clear_keys(&mut app);
    app.world_mut().resource_mut::<ClientSession>().train_seat = None;
    app.update();
    let view = app.world().resource::<TrainView>();
    assert!(!view.riding && !view.driver && view.drive_speed.is_empty());
}

#[test]
fn driving_keyboard_and_hud_follow_rebound_gameplay_actions() {
    let mut app = driver_app();
    let mut controls = terra_core::ControlsConfig::default();
    controls
        .rebind(
            GameAction::Forward,
            terra_core::InputBinding::Key(KeyCode::ArrowUp),
        )
        .unwrap();
    app.insert_resource(controls);
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::KeyW);
    app.update();
    assert!(requests(&mut app).is_empty());
    clear_keys(&mut app);
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::ArrowUp);
    app.update();
    let sent = requests(&mut app);
    assert_eq!(sent.len(), 1);
    assert_eq!(
        sent[0].0.action,
        TrainAction::Drive {
            vehicle: 1,
            throttle: -1,
            brake: false
        }
    );
    assert!(
        app.world()
            .resource::<TrainView>()
            .drive_hint
            .contains("ArrowUp")
    );
}

#[test]
fn actual_slider_callback_queues_only_the_latest_notch_without_changing_server_motion() {
    let mut app = driver_app();
    open(&mut app);
    html_change(&mut app, "drive-throttle", "1.0");
    let first = requests(&mut app);
    assert_eq!(first.len(), 1);
    assert_eq!(
        first[0].0.action,
        TrainAction::Drive {
            vehicle: 1,
            throttle: -1,
            brake: false
        }
    );
    html_change(&mut app, "drive-throttle", "2");
    html_change(&mut app, "drive-throttle", "3");
    assert!(requests(&mut app).is_empty());
    assert_eq!(app.world().resource::<TrainView>().drive_value, 3.0);
    assert_eq!(
        app.world().resource::<ClientSession>().trains.vehicles[0]
            .motion
            .throttle,
        0
    );
    app.world_mut()
        .resource_mut::<ClientSession>()
        .last_train_result = Some(TrainResult {
        request: first[0].0.request,
        outcome: TrainOutcome::Changed { vehicle: Some(1) },
    });
    app.update();
    let next = requests(&mut app);
    assert_eq!(next.len(), 1);
    assert_eq!(
        next[0].0.action,
        TrainAction::Drive {
            vehicle: 1,
            throttle: -3,
            brake: false
        }
    );
    assert_eq!(
        app.world().resource::<ClientSession>().trains.vehicles[0]
            .motion
            .speed_mps,
        2.75
    );
    html_change(&mut app, "drive-throttle", "NaN");
    html_change(&mut app, "drive-throttle", "9");
    assert!(requests(&mut app).is_empty());
}

#[test]
fn closing_driver_panel_preserves_the_acknowledgement_and_latest_queued_notch() {
    let mut app = driver_app();
    open(&mut app);
    html_change(&mut app, "drive-throttle", "1");
    let first = requests(&mut app).remove(0);
    html_change(&mut app, "drive-throttle", "3");
    actions(&mut app, "close");
    assert!(!app.world().resource::<TrainView>().open);
    assert!(requests(&mut app).is_empty());
    app.world_mut()
        .resource_mut::<ClientSession>()
        .last_train_result = Some(TrainResult {
        request: first.0.request,
        outcome: TrainOutcome::Changed { vehicle: Some(1) },
    });
    app.update();
    let sent = requests(&mut app);
    assert_eq!(sent.len(), 1);
    assert_eq!(
        sent[0].0.action,
        TrainAction::Drive {
            vehicle: 1,
            throttle: -3,
            brake: false
        }
    );
    assert!(!app.world().resource::<TrainView>().open);
}

#[test]
fn passengers_nonowners_and_unseated_players_cannot_send_driver_commands() {
    for role in 0..3 {
        let mut app = driver_app();
        {
            let mut session = app.world_mut().resource_mut::<ClientSession>();
            if role == 0 {
                session.trains.vehicles[0].parts[0].material = 215;
            } else if role == 1 {
                session.trains.vehicles[0].owner = PlayerUuid::from_u128(2);
            } else {
                session.train_seat = None;
            }
        }
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::KeyW);
        app.update();
        assert!(requests(&mut app).is_empty());
        open(&mut app);
        app.world_mut().resource_mut::<TrainUiState>().selected = Some(1);
        actions(&mut app, "drive-throttle:3");
        assert!(requests(&mut app).is_empty());
        assert!(!app.world().resource::<TrainView>().drive_controls);
    }
}

#[test]
fn engine_panel_accepts_only_real_fuels_and_keeps_player_stacks_until_acknowledged() {
    let mut app = driver_app();
    open(&mut app);
    let fuel = terra_core::furnace::fuels()[0].item;
    {
        let mut session = app.world_mut().resource_mut::<ClientSession>();
        session.inventory.creative_set(
            0,
            ItemStack {
                item: fuel,
                count: 5,
            },
        );
        session.inventory.creative_set(
            1,
            ItemStack {
                item: terra_core::ItemKind::Apple,
                count: 3,
            },
        );
    }
    actions(&mut app, "engine");
    let sent = requests(&mut app);
    assert_eq!(sent.len(), 1);
    assert_eq!(
        sent[0].0.action,
        TrainAction::OpenCargo {
            vehicle: 1,
            part: 1
        }
    );
    let cargo = TrainCargoSnapshot {
        vehicle: 1,
        part: 1,
        revision: 7,
        inventory: Inventory::default(),
    };
    {
        let mut session = app.world_mut().resource_mut::<ClientSession>();
        session.train_cargo = Some(cargo.clone());
        session.last_train_result = Some(TrainResult {
            request: sent[0].0.request,
            outcome: TrainOutcome::Cargo(cargo),
        });
    }
    app.update();
    let view = app.world().resource::<TrainView>();
    assert!(view.fuel_mode);
    assert!(view.player_slots[0].enabled);
    assert!(!view.player_slots[1].enabled);
    assert_eq!(view.title, "Motor · Befeuerung");
    let original = app.world().resource::<ClientSession>().inventory.clone();
    actions(&mut app, "slot:1");
    assert!(requests(&mut app).is_empty());
    actions(&mut app, "slot:0");
    let sent = requests(&mut app);
    assert_eq!(sent.len(), 1);
    assert!(matches!(
        sent[0].0.action,
        TrainAction::Cargo {
            part: 1,
            gesture: ContainerGesture::QuickMove { slot: 0 },
            ..
        }
    ));
    assert_eq!(app.world().resource::<ClientSession>().inventory, original);
}

#[test]
fn cargo_button_finds_storage_behind_the_cab_and_skips_the_engine_fuel_inventory() {
    let mut app = driver_app();
    {
        let mut session = app.world_mut().resource_mut::<ClientSession>();
        session.trains.vehicles[0]
            .parts
            .push(terra_core::trains::TrainPart {
                offset: [0, 0, -3],
                material: 216,
                facing: 0,
                wood: Default::default(),
            });
    }
    open(&mut app);
    app.world_mut().resource_mut::<TrainUiState>().selected = Some(1);
    actions(&mut app, "cargo");
    let sent = requests(&mut app);
    assert_eq!(sent.len(), 1);
    assert_eq!(
        sent[0].0.action,
        TrainAction::OpenCargo {
            vehicle: 1,
            part: 2
        }
    );
    assert!(app.world().resource::<TrainView>().has_cargo);
}

#[test]
fn montage_waits_for_safe_exit_ack_before_requesting_dismantle() {
    let mut app = driver_app();
    track_fixture(&mut app, 0);
    open(&mut app);
    app.world_mut()
        .resource_mut::<ClientSession>()
        .trains
        .vehicles[0]
        .motion
        .speed_mps = 0.0;
    actions(&mut app, "edit");
    let leave = requests(&mut app);
    assert_eq!(leave.len(), 1);
    assert_eq!(leave[0].0.action, TrainAction::Leave);
    {
        let mut session = app.world_mut().resource_mut::<ClientSession>();
        session.train_seat = None;
        session.last_train_result = Some(TrainResult {
            request: leave[0].0.request,
            outcome: TrainOutcome::Seated(None),
        });
    }
    app.update();
    let edit = requests(&mut app);
    assert_eq!(edit.len(), 1);
    assert_eq!(
        edit[0].0.action,
        TrainAction::Dismantle {
            station: [3, 2, 2],
            vehicle: 1,
            revision: 7
        }
    );
    assert!(app.world().resource::<TrainUiState>().open);
    {
        let mut session = app.world_mut().resource_mut::<ClientSession>();
        session.trains.vehicles.clear();
        session.last_train_result = Some(TrainResult {
            request: edit[0].0.request,
            outcome: TrainOutcome::Changed { vehicle: Some(1) },
        });
    }
    app.update();
    let view = app.world().resource::<TrainView>();
    assert!(view.open && !view.selected);
    assert!(view.status.contains("Montage aktiv"));
}

#[test]
fn montage_rejects_a_moving_vehicle_before_requesting_world_edits() {
    let mut app = driver_app();
    track_fixture(&mut app, 0);
    open(&mut app);
    actions(&mut app, "edit");
    assert!(requests(&mut app).is_empty());
    assert!(
        app.world()
            .resource::<TrainView>()
            .status
            .contains("anhalten")
    );
}

#[test]
fn montage_availability_uses_the_drivers_current_station_after_travel() {
    let mut app = driver_app();
    track_fixture(&mut app, 0);
    open(&mut app);
    {
        let mut session = app.world_mut().resource_mut::<ClientSession>();
        session.trains.vehicles[0].motion.speed_mps = 0.0;
    }
    app.world_mut().resource_mut::<TrainUiState>().station = Some([3, 2, 2]);
    app.update();
    assert!(app.world().resource::<TrainView>().can_edit);
    app.world_mut()
        .resource_mut::<ClientSession>()
        .trains
        .vehicles[0]
        .motion
        .translation[0] = 100.0;
    app.update();
    assert!(!app.world().resource::<TrainView>().can_edit);
    assert!(requests(&mut app).is_empty());
}

#[test]
fn train_interaction_respects_debug_chords_and_wheelbase_controls_are_local_and_bounded() {
    let mut app = app();
    let mut state = None;
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::F1);
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::KeyB);
    assert!(!driving::Inputs::read(app.world_mut(), &mut state).interact);
    clear_keys(&mut app);
    track_fixture(&mut app, 0);
    open(&mut app);
    for _ in 0..20 {
        actions(&mut app, "wheelbase-inc");
    }
    assert_eq!(app.world().resource::<TrainView>().wheelbase, "15 m");
    for _ in 0..20 {
        actions(&mut app, "wheelbase-dec");
    }
    assert_eq!(app.world().resource::<TrainView>().wheelbase, "3 m");
    assert!(
        requests(&mut app).is_empty(),
        "guides never place free blocks or forge server actions"
    );
}
