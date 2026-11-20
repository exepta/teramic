use super::*;
use terra_core::{PlayerUuid, npc_jobs::WorkOrder};
use terra_network::protocol::npc_work::{WorkResult, WorkSnapshot};

pub(super) fn app() -> App {
    let mut app = App::new();
    let owner = PlayerUuid::from_u128(1);
    let mut session = ClientSession::default();
    session.ready = true;
    session.uuid = Some(owner);
    session.work = WorkSnapshot {
        revision: 1,
        detail: None,
        workers: vec![WorkerView {
            npc: 7,
            name: "Ada".into(),
            position: [-12.5, 20.0, -8.5],
            order: Some(WorkOrder {
                id: 3,
                npc: 7,
                owner,
                kind: WorkKind::Miner,
                area: WorkArea::new([-20, 18, -16], [-12, 20, -8]).unwrap(),
                storage: [-10, 20, -4],
                priority: 5,
                enabled: true,
                revision: 1,
            }),
            status: WorkStatus::Waiting(WaitReason::MissingTool),
            carried_slots: 2,
        }],
    };
    app.insert_resource(session)
        .insert_resource(crate::KingdomView {
            visible: true,
            bound: true,
            work_tab: true,
            ..Default::default()
        })
        .init_resource::<WorkUiState>()
        .init_resource::<NpcWorkView>()
        .init_resource::<Time<Real>>()
        .add_message::<NpcWorkUiAction>()
        .add_message::<NpcWorkRequest>()
        .add_message::<terra_network::client::BuildingRequest>()
        .add_systems(Update, update);
    app.update();
    app
}

fn requests(app: &mut App) -> Vec<NpcWorkRequest> {
    app.world_mut()
        .resource_mut::<Messages<NpcWorkRequest>>()
        .drain()
        .collect()
}

fn act(app: &mut App, action: NpcWorkUiAction) {
    app.world_mut().write_message(action);
    app.update();
}

#[test]
fn saved_area_toggle_is_local_and_never_submits_or_visualizes_edited_drafts() {
    use terra_core::work_areas::{WorkAreaDisplay, WorkAreaTarget};
    let mut app = app();
    app.init_resource::<WorkAreaDisplay>();
    requests(&mut app);
    let before = app.world().resource::<ClientSession>().work.clone();
    act(
        &mut app,
        NpcWorkUiAction::Field {
            field: "min-x".into(),
            value: "100".into(),
        },
    );
    act(&mut app, NpcWorkUiAction::ToggleAreas);
    assert_eq!(
        app.world().resource::<WorkAreaDisplay>().target,
        Some(WorkAreaTarget::Worker(7))
    );
    assert!(app.world().resource::<NpcWorkView>().areas_shown);
    assert!(requests(&mut app).is_empty());
    assert_eq!(app.world().resource::<ClientSession>().work, before);
    let mut actual = None;
    terra_network::client::work_areas::visit_saved_areas(
        app.world().resource::<ClientSession>(),
        Some(WorkAreaTarget::Worker(7)),
        |mark| {
            if mark.kind == terra_core::work_areas::WorkAreaKind::Work {
                actual = Some(mark.bounds.min);
            }
        },
    );
    assert_eq!(actual, Some([-20, 18, -16]));
    act(&mut app, NpcWorkUiAction::ToggleAreas);
    assert!(app.world().resource::<WorkAreaDisplay>().target.is_none());
}

#[test]
fn work_tab_requests_snapshot_and_projects_actual_status_without_changing_authority() {
    let mut app = app();
    let sent = requests(&mut app);
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0].0.action, WorkAction::Inspect);
    let view = app.world().resource::<NpcWorkView>();
    assert!(view.visible && view.selected && view.can_assign && view.enabled);
    assert_eq!(view.status, "Werkzeug fehlt");
    assert_eq!(view.carried, "Fracht: 2 / 9 Inventarplätze");
    assert_eq!(view.storage_x, "-10");
    assert_eq!(view.kind, "miner");
    assert_eq!(view.area_size, "9 × 3 × 9");
    app.world_mut()
        .resource_mut::<crate::KingdomView>()
        .work_tab = false;
    act(&mut app, NpcWorkUiAction::Assign);
    assert!(requests(&mut app).is_empty());
    assert!(!app.world().resource::<NpcWorkView>().visible);
}

#[test]
fn draft_assignment_waits_for_authority_and_rejects_invalid_bounds_or_priority() {
    let mut app = app();
    requests(&mut app);
    let before = app.world().resource::<ClientSession>().work.clone();
    for (field, value) in [("kind", "farmer"), ("priority", "9"), ("storage-z", "-5")] {
        act(
            &mut app,
            NpcWorkUiAction::Field {
                field: field.into(),
                value: value.into(),
            },
        );
    }
    act(&mut app, NpcWorkUiAction::Assign);
    let sent = requests(&mut app);
    assert_eq!(sent.len(), 1);
    assert!(matches!(
        sent[0].0.action,
        WorkAction::Assign {
            npc: 7,
            kind: WorkKind::Farmer,
            storage: [-10, 20, -5],
            priority: 9,
            ..
        }
    ));
    assert_eq!(app.world().resource::<ClientSession>().work, before);
    assert!(app.world().resource::<NpcWorkView>().pending);
    act(&mut app, NpcWorkUiAction::Assign);
    assert!(requests(&mut app).is_empty());
    app.world_mut()
        .resource_mut::<ClientSession>()
        .last_work_result = Some(WorkResult {
        request: sent[0].0.request,
        error: Some(WorkError::Storage),
    });
    app.update();
    assert!(!app.world().resource::<NpcWorkView>().pending);
    assert!(
        app.world()
            .resource::<NpcWorkView>()
            .error
            .contains("Lager")
    );
    act(
        &mut app,
        NpcWorkUiAction::Field {
            field: "priority".into(),
            value: "10".into(),
        },
    );
    act(&mut app, NpcWorkUiAction::Assign);
    assert!(requests(&mut app).is_empty());
    assert!(
        app.world()
            .resource::<NpcWorkView>()
            .validation
            .contains("Priorität")
    );
    act(
        &mut app,
        NpcWorkUiAction::Field {
            field: "priority".into(),
            value: "5".into(),
        },
    );
    act(
        &mut app,
        NpcWorkUiAction::Field {
            field: "max-x".into(),
            value: "100".into(),
        },
    );
    act(&mut app, NpcWorkUiAction::Assign);
    assert!(requests(&mut app).is_empty());
    assert!(
        app.world()
            .resource::<NpcWorkView>()
            .validation
            .contains("32")
    );
}

#[test]
fn pause_uses_the_current_order_revision_and_never_locally_discards_cargo() {
    let mut app = app();
    requests(&mut app);
    let before = app.world().resource::<ClientSession>().work.clone();
    act(&mut app, NpcWorkUiAction::Enabled(false));
    let sent = requests(&mut app);
    assert_eq!(sent.len(), 1);
    assert_eq!(
        sent[0].0.action,
        WorkAction::SetEnabled {
            id: 3,
            revision: 1,
            enabled: false
        }
    );
    assert_eq!(app.world().resource::<ClientSession>().work, before);
    assert!(app.world().resource::<NpcWorkView>().enabled);
    app.world_mut()
        .resource_mut::<ClientSession>()
        .last_work_result = Some(WorkResult {
        request: sent[0].0.request,
        error: None,
    });
    app.world_mut().resource_mut::<ClientSession>().work.workers[0]
        .order
        .as_mut()
        .unwrap()
        .enabled = false;
    app.world_mut().resource_mut::<ClientSession>().work.workers[0]
        .order
        .as_mut()
        .unwrap()
        .revision += 1;
    app.world_mut().resource_mut::<ClientSession>().work.workers[0].status = WorkStatus::Paused;
    app.update();
    let view = app.world().resource::<NpcWorkView>();
    assert!(!view.enabled && !view.pending);
    assert_eq!(view.status, "Pausiert");
    assert_eq!(view.carried, "Fracht: 2 / 9 Inventarplätze");
}

#[test]
fn switching_workers_loads_their_own_draft_and_ignores_unknown_ids() {
    let mut app = app();
    requests(&mut app);
    let mut second = app.world().resource::<ClientSession>().work.workers[0].clone();
    second.npc = 8;
    second.name = "Lin".into();
    second.order = None;
    app.world_mut()
        .resource_mut::<ClientSession>()
        .work
        .workers
        .push(second);
    act(&mut app, NpcWorkUiAction::Select(8));
    let view = app.world().resource::<NpcWorkView>();
    assert_eq!(view.name, "Lin");
    assert!(view.storage_x.is_empty());
    assert!(!view.can_assign && !view.has_order);
    act(&mut app, NpcWorkUiAction::Select(999));
    assert_eq!(app.world().resource::<NpcWorkView>().name, "Lin");
    act(&mut app, NpcWorkUiAction::Select(7));
    assert_eq!(app.world().resource::<NpcWorkView>().storage_x, "-10");
}

#[test]
fn worker_ui_templates_and_styles_parse_with_the_real_tilt_loaders() {
    assert!(
        include_str!("../../../../src-ui/components/kingdom-menu/kingdom-menu.component.html")
            .contains("<kingdom-workers>")
    );
    assert!(
        include_str!(
            "../../../../src-ui/components/kingdom-workers/kingdom-workers.component.html"
        )
        .contains("<npc-work-panel>")
    );
    tilt_ui::tilt_ui_css::parse_stylesheet(include_str!(
        "../../../../src-ui/components/npc-work-panel/npc-work-panel.component.css"
    ))
    .unwrap();
}

fn callback(app: &mut App, handler: &str, id: &str, value: Option<&str>) {
    let button = app.world_mut().spawn(tilt_ui::ElementId(id.into())).id();
    let target = app.world_mut().spawn(ChildOf(button)).id();
    let handler = tilt_ui::inventory::iter::<tilt_ui::HtmlHandlerRegistration>
        .into_iter()
        .find(|entry| entry.name == handler)
        .unwrap();
    let system = (handler.register)(app.world_mut());
    app.world_mut()
        .run_system_with(
            system,
            tilt_ui::HtmlEvent {
                target,
                kind: if value.is_some() { "change" } else { "click" },
                value: value.map(String::from),
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
fn native_work_callbacks_resolve_children_and_reject_malformed_switch_values() {
    let mut app = app();
    requests(&mut app);
    app.init_resource::<tilt_ui::UiBindingStore>();
    callback(
        &mut app,
        "npc_work_change",
        "npc-work-field-priority",
        Some("8"),
    );
    assert_eq!(app.world().resource::<NpcWorkView>().priority, "8");
    callback(
        &mut app,
        "npc_work_change",
        "npc-work-enabled",
        Some("invalid"),
    );
    assert!(requests(&mut app).is_empty());
    assert!(!app.world().resource::<NpcWorkView>().pending);
    callback(&mut app, "npc_work_command", "npc-work-refresh", None);
    let sent = requests(&mut app);
    assert_eq!(sent.len(), 1);
    // Refresh retains this worker's private inventory subscription in the keep.
    assert_eq!(sent[0].0.action, WorkAction::InspectNpc { npc: 7 });
    assert!(!app.world().resource::<NpcWorkView>().pending);
    callback(
        &mut app,
        "npc_work_change",
        "npc-work-enabled",
        Some("false"),
    );
    assert_eq!(
        requests(&mut app)[0].0.action,
        WorkAction::SetEnabled {
            id: 3,
            revision: 1,
            enabled: false,
        }
    );
}

#[test]
fn a_craftsman_assignment_uses_no_work_area_and_waits_for_the_building_authority() {
    let mut app = app();
    requests(&mut app);
    let owner = PlayerUuid::from_u128(1);
    let kingdom = app
        .world_mut()
        .resource_mut::<ClientSession>()
        .kingdoms
        .found(owner, [0, 20, 0], "Craft town")
        .unwrap();
    act(
        &mut app,
        NpcWorkUiAction::Field {
            field: "kind".into(),
            value: "craftsman".into(),
        },
    );
    act(
        &mut app,
        NpcWorkUiAction::Field {
            field: "min-x".into(),
            value: String::new(),
        },
    );
    act(
        &mut app,
        NpcWorkUiAction::Field {
            field: "storage-x".into(),
            value: String::new(),
        },
    );
    assert!(app.world().resource::<NpcWorkView>().craftsman);
    assert!(
        app.world().resource::<NpcWorkView>().can_assign,
        "craftsmen do not invent placeholder work-area/depot coordinates"
    );
    act(&mut app, NpcWorkUiAction::Assign);
    let sent: Vec<_> = app
        .world_mut()
        .resource_mut::<Messages<terra_network::client::BuildingRequest>>()
        .drain()
        .filter(|request| {
            matches!(
                request.0.action,
                terra_network::protocol::building_jobs::BuildingAction::AssignCraftsman { .. }
            )
        })
        .collect();
    assert_eq!(sent.len(), 1);
    assert_eq!(
        sent[0].0.action,
        terra_network::protocol::building_jobs::BuildingAction::AssignCraftsman {
            kingdom,
            npc: 7,
            enabled: true
        }
    );
    assert!(app.world().resource::<NpcWorkView>().pending);
    assert!(
        !app.world()
            .resource::<ClientSession>()
            .buildings
            .is_craftsman(7)
    );
    app.world_mut()
        .resource_mut::<ClientSession>()
        .last_building_result = Some(terra_network::protocol::building_jobs::BuildingResult {
        request: sent[0].0.request,
        error: Some(terra_core::building_jobs::BuildError::Protected),
    });
    app.update();
    assert!(!app.world().resource::<NpcWorkView>().pending);
    assert!(
        app.world()
            .resource::<NpcWorkView>()
            .error
            .contains("Berechtigung")
    );
}

#[test]
fn keep_worker_selection_requests_and_projects_the_actual_private_inventory() {
    let mut app = app();
    requests(&mut app);
    act(&mut app, NpcWorkUiAction::Select(7));
    assert!(
        requests(&mut app)
            .iter()
            .any(|request| request.0.action == WorkAction::InspectNpc { npc: 7 })
    );
    let mut inventory = terra_core::Inventory::default();
    assert_eq!(
        inventory.insert(terra_core::ItemKind::Tool(terra_core::ToolKind::RubyAxe), 1),
        0
    );
    assert_eq!(
        inventory.insert(
            terra_core::ItemKind::Prop(terra_core::ResourcePropKind::SpruceSapling),
            128
        ),
        0
    );
    app.world_mut().resource_mut::<ClientSession>().work.detail =
        Some(terra_network::protocol::npc_work::WorkerDetail { npc: 7, inventory });
    app.update();
    let view = app.world().resource::<NpcWorkView>();
    assert!(view.visible && !view.direct && view.inventory_ready);
    assert_eq!(view.private_inventory.len(), 9);
    assert_eq!(view.private_inventory[1].count, "128");
}
