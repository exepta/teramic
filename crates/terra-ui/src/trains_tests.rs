use super::*;
use terra_core::{
    GameMode, Inventory, PlayerUuid, WoodSpecies,
    trains::{TrainPart, VehicleView},
};
use terra_network::{
    client::TrainRequest,
    protocol::trains::{TrainCargoSnapshot, TrainResult},
};

pub(super) fn app() -> App {
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
pub(super) fn car(id: u64) -> VehicleView {
    VehicleView {
        steam: Default::default(),
        name: String::new(),
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
        blocks: Default::default(),
        couplings: [None, None],
        revision: 7,
        mass_kg: 2000,
        motion: Default::default(),
    }
}
pub(super) fn open(app: &mut App) {
    let mut state = app.world_mut().resource_mut::<TrainUiState>();
    state.open = true;
    state.station = Some([0, 0, 0]);
}
pub(super) fn actions(app: &mut App, command: &str) {
    app.world_mut()
        .write_message(TrainUiAction::Command(command.into()));
    app.update();
}
pub(super) fn requests(app: &mut App) -> Vec<TrainRequest> {
    app.world_mut()
        .resource_mut::<Messages<TrainRequest>>()
        .drain()
        .collect()
}

fn click_workshop(app: &mut App, command: &str) {
    let button = app
        .world_mut()
        .spawn(tilt_ui::ElementId(command.into()))
        .id();
    let label = app.world_mut().spawn(ChildOf(button)).id();
    let handler = tilt_ui::inventory::iter::<tilt_ui::HtmlHandlerRegistration>
        .into_iter()
        .find(|registration| registration.name == "train_workshop_action")
        .unwrap();
    let system = (handler.register)(app.world_mut());
    app.world_mut()
        .run_system_with(
            system,
            tilt_ui::HtmlEvent {
                target: label,
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

#[test]
fn native_locomotive_choice_adds_the_body_after_the_functional_skeleton_is_complete() {
    let mut app = app();
    open(&mut app);
    track_fixture(&mut app, 0);
    click_workshop(&mut app, "kind:locomotive");
    let initial = app.world().resource::<TrainBlueprint>().clone();
    let mut chassis = *initial
        .cells
        .iter()
        .find(|part| part.material == 210)
        .unwrap();
    chassis.cell[2] += 2;
    let expected = TrainBlueprint {
        station: initial.station,
        kind: initial.kind,
        cells: train_blueprint::building_template_with_width(
            chassis.cell,
            chassis.facing,
            initial.kind,
            4,
            3,
        ),
    };
    let skeleton: Vec<_> = expected
        .cells
        .iter()
        .filter(|part| matches!(part.material, 210..=214 | 218 | 242))
        .copied()
        .collect();
    for part in &skeleton {
        let (section, local) = terra_voxel::global_to_section(part.cell);
        app.world_mut()
            .resource_mut::<terra_world::streaming::ChunkStore>()
            .loaded
            .get_mut(&section)
            .unwrap()
            .data
            .set_block(local, part.material)
            .unwrap();
        let mut journal = app
            .world_mut()
            .resource_mut::<terra_world::edits::EditJournal>();
        journal.record(part.cell, part.material).unwrap();
        journal
            .set_object_state(
                part.cell,
                Some(terra_world::edits::BlockObjectState {
                    // A full cube's hidden orientation must not shift or reject
                    // a valid skeleton whose bogies define its physical axle.
                    facing: if part.material == 210 {
                        (part.facing + 1) % 4
                    } else if part.material == 218 {
                        (part.facing + 3) % 4
                    } else {
                        part.facing
                    },
                    ..default()
                }),
            )
            .unwrap();
    }
    *app.world_mut().resource_mut::<TrainBlueprint>() = TrainBlueprint::default();
    click_workshop(&mut app, "blueprint");
    assert_eq!(app.world().resource::<TrainBlueprint>(), &expected);
    let view = app.world().resource::<TrainView>();
    assert!(
        view.blueprint_progress
            .contains(&format!("{} / {}", skeleton.len(), expected.cells.len()))
            && view.blueprint_progress.contains("optional")
    );
    click_workshop(&mut app, "blueprint-build");
    assert!(!app.world().resource::<TrainView>().open);
    assert_eq!(app.world().resource::<TrainBlueprint>(), &expected);
    assert!(
        requests(&mut app).is_empty(),
        "the full body guide is a suggestion, never free construction"
    );
}

#[test]
fn only_a_successful_assembly_acknowledgement_hides_the_current_build_guide() {
    let mut app = app();
    open(&mut app);
    track_fixture(&mut app, 0);
    click_workshop(&mut app, "kind:locomotive");
    let guide = app.world().resource::<TrainBlueprint>().clone();
    let chassis = *guide
        .cells
        .iter()
        .find(|part| part.material == 210)
        .unwrap();
    let (section, local) = terra_voxel::global_to_section(chassis.cell);
    app.world_mut()
        .resource_mut::<terra_world::streaming::ChunkStore>()
        .loaded
        .get_mut(&section)
        .unwrap()
        .data
        .set_block(local, 210)
        .unwrap();
    let mut car = car(1);
    car.parts[0].material = 215;
    {
        let mut session = app.world_mut().resource_mut::<ClientSession>();
        session.trains.vehicles.push(car);
        session.last_train_result = Some(TrainResult {
            request: 44,
            outcome: TrainOutcome::Changed { vehicle: Some(1) },
        });
    }
    app.world_mut().resource_mut::<TrainUiState>().pending = Some(44);
    app.update();
    assert_eq!(
        app.world().resource::<TrainBlueprint>(),
        &guide,
        "unrelated Changed notifications do not remove a build guide"
    );
    click_workshop(&mut app, "clear");
    for error in [
        TrainError::BogieAlignment,
        TrainError::Wheelbase,
        TrainError::Unsupported,
    ] {
        click_workshop(&mut app, "assemble");
        let sent = requests(&mut app);
        assert_eq!(sent.len(), 1);
        assert!(matches!(sent[0].0.action, TrainAction::Assemble { .. }));
        app.world_mut()
            .resource_mut::<ClientSession>()
            .last_train_result = Some(TrainResult {
            request: sent[0].0.request,
            outcome: TrainOutcome::Rejected(error),
        });
        app.update();
        assert_eq!(app.world().resource::<TrainBlueprint>(), &guide);
        let status = &app.world().resource::<TrainView>().status;
        assert!(status.contains(match error {
            TrainError::BogieAlignment => "Achslinie",
            TrainError::Wheelbase => "3 bis 15",
            _ => "Gleisabschnitt",
        }));
    }
    click_workshop(&mut app, "assemble");
    let sent = requests(&mut app);
    app.world_mut()
        .resource_mut::<ClientSession>()
        .last_train_result = Some(TrainResult {
        request: sent[0].0.request,
        outcome: TrainOutcome::Changed { vehicle: Some(1) },
    });
    app.update();
    assert!(app.world().resource::<TrainBlueprint>().cells.is_empty());
}

#[test]
fn closing_the_native_workshop_keeps_assembly_ack_tracking_and_clears_completed_ghosts() {
    let mut app = app();
    open(&mut app);
    track_fixture(&mut app, 0);
    click_workshop(&mut app, "kind:locomotive");
    let chassis = *app
        .world()
        .resource::<TrainBlueprint>()
        .cells
        .iter()
        .find(|part| part.material == 210)
        .unwrap();
    let (section, local) = terra_voxel::global_to_section(chassis.cell);
    app.world_mut()
        .resource_mut::<terra_world::streaming::ChunkStore>()
        .loaded
        .get_mut(&section)
        .unwrap()
        .data
        .set_block(local, 210)
        .unwrap();
    click_workshop(&mut app, "assemble");
    let sent = requests(&mut app);
    assert_eq!(sent.len(), 1);
    assert!(matches!(sent[0].0.action, TrainAction::Assemble { .. }));
    click_workshop(&mut app, "close");
    assert!(!app.world().resource::<TrainView>().open);
    assert_eq!(
        app.world().resource::<TrainUiState>().pending,
        Some(sent[0].0.request)
    );
    assert!(!app.world().resource::<TrainBlueprint>().cells.is_empty());
    assert!(requests(&mut app).is_empty());
    app.world_mut()
        .resource_mut::<terra_world::streaming::ChunkStore>()
        .loaded
        .get_mut(&section)
        .unwrap()
        .data
        .set_block(local, 0)
        .unwrap();
    app.world_mut()
        .resource_mut::<ClientSession>()
        .last_train_result = Some(TrainResult {
        request: sent[0].0.request,
        outcome: TrainOutcome::Changed { vehicle: Some(1) },
    });
    app.update();
    assert!(!app.world().resource::<TrainView>().open);
    assert!(app.world().resource::<TrainBlueprint>().cells.is_empty());
    assert_eq!(app.world().resource::<TrainUiState>().pending, None);
    assert!(!app.world().resource::<TrainUiState>().pending_assembly);
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

pub(super) fn track_fixture(app: &mut App, facing: u8) {
    use terra_voxel::{BlockSection, global_to_section};
    use terra_world::{
        edits::EditJournal,
        streaming::{ChunkStore, LoadedSection},
    };
    let mut chunks = ChunkStore::default();
    let mut journal = EditJournal::default();
    fn set(chunks: &mut ChunkStore, cell: [i64; 3], material: u16) {
        let (owner, local) = global_to_section(cell);
        chunks
            .loaded
            .entry(owner)
            .or_insert_with(|| LoadedSection {
                data: BlockSection::empty(owner).unwrap(),
                mesh: None,
                mesh_epoch: 0,
                water_epoch: 0,
                water: None,
                props: Vec::new(),
            })
            .data
            .set_block(local, material)
            .unwrap();
    }
    for x in -9..=9 {
        for y in -2..=6 {
            for z in -9..=9 {
                set(&mut chunks, [x, y, z], 0);
            }
        }
    }
    let rotate = |[x, y, z]: [i64; 3]| match facing {
        0 => [x, y, z],
        1 => [z, y, -x],
        2 => [-x, y, -z],
        _ => [-z, y, x],
    };
    for anchor in [[0, 0, 0], rotate([0, 0, 4])] {
        let piece =
            terra_core::rails::Piece::new(anchor, terra_core::rails::STRAIGHT, facing).unwrap();
        for point in piece.supports().iter() {
            set(
                &mut chunks,
                std::array::from_fn(|axis| anchor[axis] + i64::from(point.support[axis])),
                3,
            );
        }
        let plan =
            terra_world::rails::plan(&chunks, &journal, anchor, piece.material, facing).unwrap();
        terra_world::rails::commit(&mut chunks, &mut journal, &plan).unwrap();
    }
    let station = rotate([3, 2, 2]);
    set(&mut chunks, station, 217);
    journal.record(station, 217).unwrap();
    app.insert_resource(chunks).insert_resource(journal);
    app.world_mut().resource_mut::<TrainUiState>().station = Some(station);
}

#[test]
fn registered_html_handler_accepts_label_and_icon_children_and_local_choices_while_inspection_waits()
 {
    use tilt_ui::{ElementId, HtmlEvent};
    let mut app = app();
    open(&mut app);
    app.world_mut().resource_mut::<TrainUiState>().pending = Some(123);
    let button = app
        .world_mut()
        .spawn(ElementId("kind:passenger".into()))
        .id();
    let label = app.world_mut().spawn(ChildOf(button)).id();
    let handler = tilt_ui::inventory::iter::<tilt_ui::HtmlHandlerRegistration>
        .into_iter()
        .find(|registration| registration.name == "train_workshop_action")
        .expect("The authored template handler must be registered by the real macro");
    let system = (handler.register)(app.world_mut());
    app.world_mut()
        .run_system_with(
            system,
            HtmlEvent {
                target: label,
                kind: "click",
                value: None,
                submitter: None,
                data: Default::default(),
                form_data: None,
                handler: handler.name.into(),
            },
        )
        .unwrap();
    app.update();
    assert_eq!(app.world().resource::<TrainView>().kind, "Personenwagen");
    assert!(app.world().resource::<TrainView>().pending);
    assert!(
        requests(&mut app).is_empty(),
        "kind choices are visual and never forge server requests"
    );
}

#[test]
fn workshop_restores_pointer_events_under_the_noninteractive_game_overlay() {
    use tilt_ui::tilt_ui_css::{PointerEvents, StyleDeclaration, parse_stylesheet};
    let game = parse_stylesheet(include_str!("../../../src-ui/pages/game.component.css")).unwrap();
    let workshop = parse_stylesheet(include_str!(
        "../../../src-ui/components/train-workshop.component.css"
    ))
    .unwrap();
    assert!(
        game.rules[0]
            .declarations
            .iter()
            .any(|declaration| matches!(
                declaration,
                StyleDeclaration::PointerEvents(PointerEvents::None)
            ))
    );
    assert!(
        workshop.rules[0]
            .declarations
            .iter()
            .any(|declaration| matches!(
                declaration,
                StyleDeclaration::PointerEvents(PointerEvents::Auto)
            )),
        "the workshop must override inherited pointer-events:none"
    );
}

#[test]
fn all_vehicle_guides_snap_both_bogies_to_complete_rails_and_persist_after_closing() {
    for facing in 0..4 {
        let mut app = app();
        open(&mut app);
        track_fixture(&mut app, facing);
        for kind in ["locomotive", "passenger", "tender", "freight"] {
            actions(&mut app, &format!("kind:{kind}"));
            let guide = app.world().resource::<TrainBlueprint>();
            assert_eq!(
                guide
                    .cells
                    .iter()
                    .filter(|part| part.material == 211)
                    .count(),
                2
            );
            if kind == "locomotive" {
                assert!(guide.cells.len() > 30);
                assert!(guide.cells.iter().any(|part| part.material == 250));
            } else {
                assert_eq!(guide.cells.len(), 26);
            }
            assert!(
                guide
                    .cells
                    .iter()
                    .filter(|part| matches!(part.material, 210 | 211 | 213))
                    .all(|part| part.facing % 2 == facing % 2)
            );
            for part in guide.cells.iter().filter(|part| part.material == 211) {
                assert!(terra_world::trains::bogie_supported_with_facing(
                    app.world().resource::<terra_world::streaming::ChunkStore>(),
                    app.world().resource::<terra_world::edits::EditJournal>(),
                    part.cell,
                    part.facing,
                ));
            }
        }
        let expected = app.world().resource::<TrainBlueprint>().clone();
        actions(&mut app, "blueprint-build");
        assert!(!app.world().resource::<TrainView>().open);
        assert!(app.world().resource::<TrainView>().blueprint);
        assert_eq!(app.world().resource::<TrainBlueprint>(), &expected);
        open(&mut app);
        actions(&mut app, "blueprint-clear");
        assert!(app.world().resource::<TrainBlueprint>().cells.is_empty());
    }
}

#[test]
fn missing_or_incomplete_rails_give_actionable_feedback_and_world_changes_clear_the_guide() {
    let mut app = app();
    open(&mut app);
    actions(&mut app, "kind:locomotive");
    assert!(!app.world().resource::<TrainView>().blueprint);
    assert!(
        app.world()
            .resource::<TrainView>()
            .status
            .contains("Gleis verlängern")
    );
    track_fixture(&mut app, 0);
    actions(&mut app, "kind:locomotive");
    assert!(app.world().resource::<TrainView>().blueprint);
    app.world_mut().resource_mut::<ClientSession>().ready = false;
    app.update();
    assert!(app.world().resource::<TrainBlueprint>().cells.is_empty());
}

#[test]
fn ordinary_chest_on_an_assembled_vehicle_opens_the_same_private_cargo_flow() {
    let mut app = app();
    let mut controls = terra_core::ControlsConfig::default();
    controls
        .rebind(
            GameAction::Place,
            terra_core::InputBinding::Mouse(MouseButton::Right),
        )
        .unwrap();
    app.insert_resource(controls);
    let mut vehicle = car(1);
    vehicle.parts[0].material = 29;
    app.world_mut()
        .resource_mut::<ClientSession>()
        .trains
        .vehicles
        .push(vehicle);
    let mut capture = terra_player::CameraInputState::default();
    capture.captured = true;
    app.insert_resource(capture)
        .insert_resource(terra_player::TargetTrain(Some(
            terra_world::trains::TrainHit {
                vehicle: 1,
                part: 0,
                distance: 2.0,
            },
        )));
    app.world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .press(MouseButton::Right);
    app.update();
    assert!(app.world().resource::<TrainUiState>().open);
    let sent = requests(&mut app);
    assert_eq!(sent.len(), 1);
    assert_eq!(
        sent[0].0.action,
        TrainAction::OpenCargo {
            vehicle: 1,
            part: 0
        }
    );
}

#[test]
fn build_guide_never_labels_a_ghost_behind_a_nearer_wall() {
    let mut app = app();
    open(&mut app);
    track_fixture(&mut app, 0);
    let station = app.world().resource::<TrainUiState>().station;
    *app.world_mut().resource_mut::<TrainBlueprint>() = TrainBlueprint {
        station,
        kind: VehicleKind::Locomotive,
        cells: vec![train_blueprint::BlueprintCell {
            cell: [0, 2, -2],
            material: 210,
            facing: 0,
        }],
    };
    app.world_mut().spawn((
        terra_player::TerraCamera,
        Transform::from_xyz(0.5, 2.5, 2.5),
    ));
    app.update();
    assert!(
        app.world()
            .resource::<TrainView>()
            .blueprint_target
            .contains("Zug-Fahrgestell")
    );
    app.insert_resource(terra_player::TargetWorld(Some(
        terra_player::WorldTarget::Block(terra_voxel::blocks::BlockHit {
            position: [0, 2, 0],
            normal: [0, 0, 1],
            distance: 1.5,
        }),
    )));
    app.update();
    assert!(
        !app.world()
            .resource::<TrainView>()
            .blueprint_target
            .contains("Zug-Fahrgestell")
    );
}

#[test]
fn a_new_connection_or_replaced_world_cannot_replay_old_build_ghosts() {
    let mut app = app();
    open(&mut app);
    track_fixture(&mut app, 0);
    actions(&mut app, "kind:locomotive");
    assert!(!app.world().resource::<TrainBlueprint>().cells.is_empty());
    app.world_mut().resource_mut::<TrainUiState>().identity =
        Some((None, Some(PlayerUuid::from_u128(1))));
    let connection = app.world_mut().spawn_empty().id();
    app.world_mut().resource_mut::<ClientSession>().connection = Some(connection);
    app.update();
    assert!(app.world().resource::<TrainBlueprint>().cells.is_empty());
    open(&mut app);
    actions(&mut app, "kind:locomotive");
    assert!(!app.world().resource::<TrainBlueprint>().cells.is_empty());
    app.insert_resource(terra_world::streaming::ChunkStore::default());
    app.update();
    assert!(app.world().resource::<TrainBlueprint>().cells.is_empty());
}

#[test]
fn workshop_panel_fits_hd_through_4k_without_enlarging_text_independently() {
    let content = Vec2::new(880.0, 720.0);
    for (viewport, scale) in [
        (Vec2::new(1280.0, 720.0), 1.5),
        (Vec2::new(1920.0, 1080.0), 1.5),
        (Vec2::new(2560.0, 1440.0), 2.0),
        (Vec2::new(3840.0, 2160.0), 2.0),
    ] {
        let logical = viewport / scale;
        let fitted = panel_scale(logical, content);
        assert!(fitted <= 1.0);
        assert!(content.x * fitted <= logical.x * 0.94 + 0.01);
        assert!(content.y * fitted <= logical.y * 0.92 + 0.01);
    }
}

#[test]
fn real_auto_build_handler_submits_the_displayed_guide_with_the_inventory_revision() {
    let mut app = app();
    open(&mut app);
    track_fixture(&mut app, 0);
    click_workshop(&mut app, "kind:freight");
    let guide = app.world().resource::<TrainBlueprint>().clone();
    let chassis = *guide
        .cells
        .iter()
        .find(|part| part.material == 210)
        .unwrap();
    click_workshop(&mut app, "blueprint-auto");
    let sent = requests(&mut app);
    assert_eq!(sent.len(), 1);
    assert_eq!(
        sent[0].0.action,
        TrainAction::Build {
            station: guide.station.unwrap(),
            chassis: chassis.cell,
            facing: chassis.facing,
            kind: VehicleKind::Freight,
            wheelbase: 4,
            width: 3,
            player_revision: app.world().resource::<ClientSession>().inventory.revision
        }
    );
    assert!(app.world().resource::<TrainView>().pending);
}
#[test]
fn name_input_uses_its_actual_html_handler_and_saves_the_edited_train_name() {
    let mut app = app();
    open(&mut app);
    app.world_mut()
        .resource_mut::<ClientSession>()
        .trains
        .vehicles
        .push(car(1));
    app.world_mut().resource_mut::<TrainUiState>().selected = Some(1);
    let entity = app
        .world_mut()
        .spawn(tilt_ui::ElementId("train-name".into()))
        .id();
    let handler = tilt_ui::inventory::iter::<tilt_ui::HtmlHandlerRegistration>
        .into_iter()
        .find(|registration| registration.name == "train_workshop_action")
        .unwrap();
    let system = (handler.register)(app.world_mut());
    app.world_mut()
        .run_system_with(
            system,
            tilt_ui::HtmlEvent {
                target: entity,
                kind: "change",
                value: Some("TERAMIC EXPRESS".into()),
                submitter: None,
                data: default(),
                form_data: None,
                handler: handler.name.into(),
            },
        )
        .unwrap();
    app.update();
    assert_eq!(app.world().resource::<TrainView>().name, "TERAMIC EXPRESS");
    click_workshop(&mut app, "name-save");
    let sent = requests(&mut app);
    assert_eq!(sent.len(), 1);
    assert!(
        matches!(&sent[0].0.action,TrainAction::Name{vehicle:1,name,..} if name=="TERAMIC EXPRESS")
    );
}
#[test]
fn typing_e_does_not_close_the_train_name_input_but_escape_does() {
    use bevy::input_focus::{FocusCause, InputFocus};
    let mut app = app();
    open(&mut app);
    app.init_resource::<InputFocus>();
    let entity = app
        .world_mut()
        .spawn(tilt_ui::ElementId("train-name".into()))
        .id();
    app.world_mut()
        .resource_mut::<InputFocus>()
        .set(entity, FocusCause::Navigated);
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::KeyE);
    app.update();
    assert!(app.world().resource::<TrainView>().open);
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .clear();
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::Escape);
    app.update();
    assert!(!app.world().resource::<TrainView>().open);
}

#[test]
fn width_buttons_update_the_real_preview_and_auto_build_request() {
    let mut app = app();
    open(&mut app);
    track_fixture(&mut app, 0);
    click_workshop(&mut app, "width:1");
    assert_eq!(app.world().resource::<TrainView>().width, "1");
    assert!(
        app.world()
            .resource::<TrainBlueprint>()
            .cells
            .iter()
            .all(|part| part.cell[0] == app.world().resource::<TrainBlueprint>().cells[0].cell[0])
    );
    assert!(
        app.world()
            .resource::<train_blueprint::WorkshopPreviewRequest>()
            .visible
    );
    click_workshop(&mut app, "blueprint-auto");
    assert!(
        requests(&mut app)
            .iter()
            .any(|request| matches!(request.0.action, TrainAction::Build { width: 1, .. }))
    );
}

#[test]
fn selecting_a_second_vehicle_prefills_the_facing_free_coupler_ends() {
    let mut app = app();
    let mut first = car(1);
    first.parts = [([0, 0, 0], 210), ([0, 0, -3], 213), ([0, 0, 5], 213)]
        .into_iter()
        .map(|(offset, material)| TrainPart {
            offset,
            material,
            facing: 0,
            wood: Default::default(),
        })
        .collect();
    let mut second = first.clone();
    second.id = 2;
    second.anchor = [0, 0, 9];
    app.world_mut()
        .resource_mut::<ClientSession>()
        .trains
        .vehicles = vec![first, second];
    open(&mut app);
    actions(&mut app, "select:1");
    actions(&mut app, "select:2");
    assert!(app.world().resource::<TrainUiState>().front);
    assert!(!app.world().resource::<TrainUiState>().second_front);
    actions(&mut app, "couple");
    assert!(matches!(
        requests(&mut app)[0].0.action,
        TrainAction::Couple {
            first_end: CouplerEnd::Front,
            second_end: CouplerEnd::Rear,
            ..
        }
    ));
}
