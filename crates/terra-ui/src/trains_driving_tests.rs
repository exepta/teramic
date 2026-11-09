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

fn html_click(app: &mut App, command: &str, child: u8) {
    let id = if command.starts_with("slot:") {
        let mut store = tilt_ui::UiBindingStore::default();
        store.set_store(
            crate::__tilt_ui_generated_components::train_workshop_component::TrainIntent {
                command: command.into(),
            },
        );
        app.insert_resource(store);
        "train-slot"
    } else {
        command
    };
    let button = app.world_mut().spawn(tilt_ui::ElementId(id.into())).id();
    let target = match child {
        1 => app
            .world_mut()
            .spawn((
                ChildOf(button),
                tilt_ui::ElementId("slot-art".into()),
                bevy::ui::widget::ImageNode::default(),
            ))
            .id(),
        2 => app
            .world_mut()
            .spawn((
                ChildOf(button),
                tilt_ui::ElementId("slot-count".into()),
                Text::new("5"),
            ))
            .id(),
        _ => button,
    };
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
                kind: "click",
                value: None,
                submitter: None,
                data: default(),
                form_data: None,
                handler: handler.name.into(),
            },
        )
        .unwrap();
    app.update();
}

fn opened_cargo(app: &mut App, command: &str, part: u16, inventory: Inventory) {
    html_click(app, command, 2);
    let sent = requests(app);
    assert_eq!(sent.len(), 1);
    assert_eq!(
        sent[0].0.action,
        TrainAction::OpenCargo { vehicle: 1, part }
    );
    let cargo = TrainCargoSnapshot {
        vehicle: 1,
        part,
        revision: 7,
        inventory,
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
}

/// Produce the acknowledgement with the real inventory transaction, never by
/// manually subtracting a mirrored item count in the UI fixture.
fn acknowledge_gesture(app: &mut App, request: terra_network::client::TrainRequest) {
    let TrainAction::Cargo {
        vehicle,
        part,
        revision,
        player_revision,
        gesture,
        reserved,
    } = request.0.action
    else {
        panic!("cargo action")
    };
    let session = app.world().resource::<ClientSession>();
    let mut player = session.inventory.clone();
    let original = session.train_cargo.clone().unwrap();
    assert_eq!(
        (
            original.vehicle,
            original.part,
            original.revision,
            player.revision
        ),
        (vehicle, part, revision, player_revision)
    );
    let mut inventories = [original.inventory];
    assert!(terra_core::containers::apply_gesture_reserved(
        &mut player,
        &mut inventories,
        gesture,
        reserved
    ));
    let cargo = TrainCargoSnapshot {
        vehicle,
        part,
        revision: revision + 1,
        inventory: inventories[0].clone(),
    };
    {
        let mut session = app.world_mut().resource_mut::<ClientSession>();
        session.inventory = player;
        session.trains.vehicles[0].revision = cargo.revision;
        session.train_cargo = Some(cargo.clone());
        session.last_train_result = Some(TrainResult {
            request: request.0.request,
            outcome: TrainOutcome::Cargo(cargo),
        });
    }
    app.update();
}

fn stale_refresh(app: &mut App, request: u64) -> terra_network::client::TrainRequest {
    app.world_mut()
        .resource_mut::<ClientSession>()
        .last_train_result = Some(TrainResult {
        request,
        outcome: TrainOutcome::Rejected(TrainError::Stale),
    });
    app.update();
    let sent = requests(app);
    assert_eq!(sent.len(), 1);
    assert!(matches!(
        sent[0].0.action,
        TrainAction::OpenCargo {
            vehicle: 1,
            part: 1
        }
    ));
    sent.into_iter().next().unwrap()
}

fn refreshed_cargo(app: &mut App, request: u64, inventory: Inventory) {
    let cargo = TrainCargoSnapshot {
        vehicle: 1,
        part: 1,
        revision: 9,
        inventory,
    };
    {
        let mut session = app.world_mut().resource_mut::<ClientSession>();
        session.train_cargo = Some(cargo.clone());
        session.last_train_result = Some(TrainResult {
            request,
            outcome: TrainOutcome::Cargo(cargo),
        });
    }
    app.update();
}

#[test]
fn one_native_coal_click_replays_once_after_an_authoritative_stale_token_refresh() {
    let coal = terra_core::ItemKind::Mineral(terra_core::MineralItem::Coal);
    for cursor in [false, true] {
        let mut app = driver_app();
        open(&mut app);
        let stack = ItemStack {
            item: coal,
            count: 5,
        };
        {
            let mut session = app.world_mut().resource_mut::<ClientSession>();
            if cursor {
                session.inventory.cursor = Some(stack);
            } else {
                session.inventory.creative_set(0, stack);
            }
        }
        opened_cargo(&mut app, "engine", 1, Inventory::default());
        let before = app.world().resource::<ClientSession>().inventory.clone();
        html_click(&mut app, if cursor { "slot:27" } else { "slot:0" }, 2);
        let initial = requests(&mut app).remove(0);
        let expected = match &initial.0.action {
            TrainAction::Cargo { gesture, .. } => gesture.clone(),
            _ => panic!("gesture"),
        };
        let refresh = stale_refresh(&mut app, initial.0.request);
        app.world_mut()
            .resource_mut::<ClientSession>()
            .trains
            .vehicles[0]
            .revision = 9;
        assert_eq!(
            app.world()
                .resource::<ClientSession>()
                .train_cargo
                .as_ref()
                .unwrap()
                .revision,
            7
        );
        assert_eq!(app.world().resource::<ClientSession>().inventory, before);
        refreshed_cargo(&mut app, refresh.0.request, Inventory::default());
        let replay = requests(&mut app);
        assert_eq!(replay.len(), 1);
        assert!(
            matches!(&replay[0].0.action, TrainAction::Cargo { revision: 9, gesture, .. } if *gesture == expected)
        );
        assert_eq!(app.world().resource::<ClientSession>().inventory, before);
        acknowledge_gesture(&mut app, replay.into_iter().next().unwrap());
        assert_eq!(
            app.world()
                .resource::<ClientSession>()
                .train_cargo
                .as_ref()
                .unwrap()
                .inventory
                .count(coal),
            5
        );
        assert!(
            app.world()
                .resource::<ClientSession>()
                .inventory
                .cursor
                .is_none()
        );
        assert_eq!(
            app.world()
                .resource::<ClientSession>()
                .inventory
                .count(coal),
            0
        );
        app.update();
        assert!(requests(&mut app).is_empty());
    }
}

#[test]
fn a_second_stale_result_aborts_the_coal_gesture_without_repeating_or_consuming_items() {
    let coal = terra_core::ItemKind::Mineral(terra_core::MineralItem::Coal);
    let mut app = driver_app();
    open(&mut app);
    app.world_mut()
        .resource_mut::<ClientSession>()
        .inventory
        .creative_set(
            0,
            ItemStack {
                item: coal,
                count: 5,
            },
        );
    opened_cargo(&mut app, "engine", 1, Inventory::default());
    html_click(&mut app, "slot:0", 1);
    let first = requests(&mut app).remove(0);
    let refresh = stale_refresh(&mut app, first.0.request);
    refreshed_cargo(&mut app, refresh.0.request, Inventory::default());
    let replay = requests(&mut app).remove(0);
    app.world_mut()
        .resource_mut::<ClientSession>()
        .last_train_result = Some(TrainResult {
        request: replay.0.request,
        outcome: TrainOutcome::Rejected(TrainError::Stale),
    });
    app.update();
    assert!(requests(&mut app).is_empty());
    assert!(app.world().resource::<TrainUiState>().cargo_retry.is_none());
    assert_eq!(
        app.world()
            .resource::<ClientSession>()
            .inventory
            .count(coal),
        5
    );
}

#[test]
fn stale_cursor_deposits_cancel_on_close_back_cursor_identity_or_destination_changes() {
    let coal = terra_core::ItemKind::Mineral(terra_core::MineralItem::Coal);
    for change in 0..6 {
        let mut app = driver_app();
        open(&mut app);
        app.world_mut()
            .resource_mut::<ClientSession>()
            .inventory
            .cursor = Some(ItemStack {
            item: coal,
            count: 5,
        });
        opened_cargo(&mut app, "engine", 1, Inventory::default());
        html_click(&mut app, "slot:27", 0);
        let first = requests(&mut app).remove(0);
        let refresh = stale_refresh(&mut app, first.0.request);
        let mut destination = Inventory::default();
        match change {
            0 => html_click(&mut app, "close", 0),
            1 => html_click(&mut app, "workshop", 0),
            2 => {
                app.world_mut()
                    .resource_mut::<ClientSession>()
                    .inventory
                    .cursor = Some(ItemStack {
                    item: coal,
                    count: 4,
                })
            }
            3 => {
                app.world_mut().resource_mut::<ClientSession>().uuid =
                    Some(PlayerUuid::from_u128(2))
            }
            4 => {
                destination.insert(terra_core::furnace::fuels()[0].item, 1);
            }
            _ => {
                app.world_mut()
                    .resource_mut::<ClientSession>()
                    .trains
                    .vehicles[0]
                    .parts[1]
                    .material = 216
            }
        }
        refreshed_cargo(&mut app, refresh.0.request, destination);
        assert!(
            requests(&mut app).is_empty(),
            "changed cursor/panel/type must never replay a swap"
        );
        assert!(
            app.world()
                .resource::<ClientSession>()
                .inventory
                .cursor
                .is_some()
        );
        if change == 0 {
            assert!(!app.world().resource::<TrainView>().open);
        }
        if change == 1 {
            assert!(!app.world().resource::<TrainView>().cargo);
        }
    }
}

#[test]
fn a_stale_quick_move_does_not_replay_when_the_clicked_source_stack_changes() {
    let coal = terra_core::ItemKind::Mineral(terra_core::MineralItem::Coal);
    for changed in [
        ItemStack {
            item: coal,
            count: 4,
        },
        ItemStack {
            item: terra_core::ItemKind::Apple,
            count: 5,
        },
    ] {
        let mut app = driver_app();
        open(&mut app);
        app.world_mut()
            .resource_mut::<ClientSession>()
            .inventory
            .creative_set(
                0,
                ItemStack {
                    item: coal,
                    count: 5,
                },
            );
        opened_cargo(&mut app, "engine", 1, Inventory::default());
        html_click(&mut app, "slot:0", 2);
        let first = requests(&mut app).remove(0);
        let refresh = stale_refresh(&mut app, first.0.request);
        app.world_mut()
            .resource_mut::<ClientSession>()
            .inventory
            .creative_set(0, changed);
        refreshed_cargo(&mut app, refresh.0.request, Inventory::default());
        assert!(requests(&mut app).is_empty());
        assert_eq!(
            app.world().resource::<ClientSession>().inventory.slot(0),
            Some(changed)
        );
        assert!(app.world().resource::<TrainUiState>().cargo_retry.is_none());
    }
}

#[test]
fn native_coal_stack_clicks_from_button_image_and_count_move_only_after_the_cargo_ack() {
    let coal = terra_core::ItemKind::Mineral(terra_core::MineralItem::Coal);
    assert!(terra_core::furnace::fuel_seconds(coal).is_some());
    for target in 0..3 {
        let mut app = driver_app();
        open(&mut app);
        {
            let mut session = app.world_mut().resource_mut::<ClientSession>();
            session.inventory.creative_set(
                0,
                ItemStack {
                    item: coal,
                    count: 5,
                },
            );
            session.trains.vehicles[0].motion.throttle = 3;
            session.trains.vehicles[0].motion.fuel_available = false;
        }
        opened_cargo(&mut app, "engine", 1, Inventory::default());
        assert!(app.world().resource::<TrainView>().player_slots[0].enabled);
        html_click(&mut app, "slot:0", target);
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
        assert_eq!(
            app.world()
                .resource::<ClientSession>()
                .inventory
                .count(coal),
            5
        );
        acknowledge_gesture(&mut app, sent.into_iter().next().unwrap());
        let view = app.world().resource::<TrainView>();
        assert!(view.open && view.cargo && view.fuel_mode);
        assert_eq!(view.cargo_slots[0].count, "5");
        assert!(view.player_slots[0].empty);
        assert_eq!(
            app.world()
                .resource::<ClientSession>()
                .inventory
                .count(coal),
            0
        );
        assert_eq!(
            app.world()
                .resource::<ClientSession>()
                .train_cargo
                .as_ref()
                .unwrap()
                .inventory
                .count(coal),
            5
        );
        html_click(&mut app, "slot:27", target);
        let sent = requests(&mut app);
        assert_eq!(sent.len(), 1);
        acknowledge_gesture(&mut app, sent.into_iter().next().unwrap());
        assert_eq!(
            app.world()
                .resource::<ClientSession>()
                .inventory
                .count(coal),
            5
        );
        assert!(app.world().resource::<TrainView>().cargo_slots[0].empty);
    }
}

#[test]
fn a_held_coal_stack_can_be_deposited_on_an_empty_engine_slot_in_one_native_click() {
    let coal = terra_core::ItemKind::Mineral(terra_core::MineralItem::Coal);
    let mut app = driver_app();
    open(&mut app);
    app.world_mut()
        .resource_mut::<ClientSession>()
        .inventory
        .cursor = Some(ItemStack {
        item: coal,
        count: 7,
    });
    opened_cargo(&mut app, "engine", 1, Inventory::default());
    let view = app.world().resource::<TrainView>();
    assert!(view.held && view.cargo_slots[0].empty && view.cargo_slots[0].enabled);
    assert_eq!(view.held_count, "7");
    html_click(&mut app, "slot:27", 0);
    let sent = requests(&mut app);
    assert_eq!(sent.len(), 1);
    assert!(matches!(
        sent[0].0.action,
        TrainAction::Cargo {
            gesture: ContainerGesture::Click {
                slot: 27,
                secondary: false
            },
            ..
        }
    ));
    assert_eq!(
        app.world()
            .resource::<ClientSession>()
            .inventory
            .cursor
            .unwrap()
            .count,
        7
    );
    acknowledge_gesture(&mut app, sent.into_iter().next().unwrap());
    assert!(
        app.world()
            .resource::<ClientSession>()
            .inventory
            .cursor
            .is_none()
    );
    assert_eq!(
        app.world().resource::<TrainView>().cargo_slots[0].count,
        "7"
    );
    assert!(!app.world().resource::<TrainView>().held);
}

#[test]
fn a_held_nonfuel_stack_can_return_to_an_empty_player_slot_without_entering_the_engine() {
    let mut app = driver_app();
    open(&mut app);
    app.world_mut()
        .resource_mut::<ClientSession>()
        .inventory
        .cursor = Some(ItemStack {
        item: terra_core::ItemKind::Apple,
        count: 3,
    });
    opened_cargo(&mut app, "engine", 1, Inventory::default());
    let view = app.world().resource::<TrainView>();
    assert!(view.player_slots[0].empty && view.player_slots[0].enabled);
    assert!(view.cargo_slots.iter().all(|slot| !slot.enabled));
    html_click(&mut app, "slot:27", 0);
    assert!(requests(&mut app).is_empty());
    assert!(
        app.world()
            .resource::<ClientSession>()
            .inventory
            .cursor
            .is_some()
    );
    html_click(&mut app, "slot:0", 0);
    let sent = requests(&mut app);
    assert_eq!(sent.len(), 1);
    acknowledge_gesture(&mut app, sent.into_iter().next().unwrap());
    let session = app.world().resource::<ClientSession>();
    assert!(session.inventory.cursor.is_none());
    assert_eq!(session.inventory.count(terra_core::ItemKind::Apple), 3);
    assert!(
        session
            .train_cargo
            .as_ref()
            .unwrap()
            .inventory
            .slots
            .is_empty()
    );
}

#[test]
fn native_cargo_button_uses_the_current_ridden_vehicle_even_after_clearing_row_selection() {
    for material in [29, 216] {
        let mut app = driver_app();
        open(&mut app);
        app.world_mut()
            .resource_mut::<ClientSession>()
            .trains
            .vehicles[0]
            .parts
            .push(terra_core::trains::TrainPart {
                offset: [0, 0, -3],
                material,
                facing: 0,
                wood: default(),
            });
        html_click(&mut app, "clear", 2);
        assert!(app.world().resource::<TrainUiState>().selected.is_none());
        assert!(app.world().resource::<TrainView>().has_cargo);
        let mut cargo = Inventory::default();
        cargo.insert(terra_core::ItemKind::Block(3), 3);
        opened_cargo(&mut app, "cargo", 2, cargo);
        assert!(!app.world().resource::<TrainView>().fuel_mode);
        html_click(&mut app, "slot:27", 1);
        let sent = requests(&mut app);
        assert_eq!(sent.len(), 1);
        acknowledge_gesture(&mut app, sent.into_iter().next().unwrap());
        assert_eq!(
            app.world()
                .resource::<ClientSession>()
                .inventory
                .count(terra_core::ItemKind::Block(3)),
            3
        );
    }
}

#[test]
fn driving_hud_uses_authoritative_meter_speed_and_configured_controls_then_clears_on_exit() {
    let mut app = driver_app();
    app.update();
    let view = app.world().resource::<TrainView>();
    assert!(view.riding && view.driver);
    assert_eq!(view.drive_speed, "2,8 Blöcke/s");
    assert_eq!(view.drive_levels.len(), 9);
    assert!(
        view.drive_levels
            .iter()
            .any(|step| step.command == "drive-throttle:6" && !step.enabled)
    );
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

#[test]
fn destroying_requires_confirmation_and_acknowledged_dismount_before_deletion() {
    let mut app = driver_app();
    app.world_mut()
        .resource_mut::<ClientSession>()
        .trains
        .vehicles[0]
        .motion = Default::default();
    open(&mut app);
    app.world_mut().resource_mut::<TrainUiState>().selected = Some(1);
    html_click(&mut app, "destroy-confirm", 0);
    assert!(requests(&mut app).is_empty());
    html_click(&mut app, "destroy", 1);
    assert!(app.world().resource::<TrainView>().confirm_destroy);
    assert!(requests(&mut app).is_empty());
    html_click(&mut app, "destroy-cancel", 0);
    assert!(!app.world().resource::<TrainView>().confirm_destroy);
    html_click(&mut app, "destroy", 0);
    html_click(&mut app, "destroy-confirm", 0);
    let commands = requests(&mut app);
    assert_eq!(commands.len(), 1);
    assert_eq!(commands[0].0.action, TrainAction::Leave);
    let revision = app.world().resource::<ClientSession>().trains.vehicles[0].revision;
    {
        let mut session = app.world_mut().resource_mut::<ClientSession>();
        session.train_seat = None;
        session.last_train_result = Some(TrainResult {
            request: commands[0].0.request,
            outcome: TrainOutcome::Seated(None),
        });
    }
    app.update();
    let commands = requests(&mut app);
    assert_eq!(commands.len(), 1);
    assert_eq!(
        commands[0].0.action,
        TrainAction::Destroy {
            vehicle: 1,
            revision
        }
    );
    {
        let mut session = app.world_mut().resource_mut::<ClientSession>();
        session.trains.vehicles.clear();
        session.last_train_result = Some(TrainResult {
            request: commands[0].0.request,
            outcome: TrainOutcome::Changed { vehicle: None },
        });
    }
    app.update();
    assert!(app.world().resource::<TrainUiState>().selected.is_none());
    assert!(!app.world().resource::<TrainView>().confirm_destroy);
}
