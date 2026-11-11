use super::*;
use terra_core::inventory_sort::SortOrder;

fn click(app: &mut App, id: &str) {
    let button = app.world_mut().spawn(tilt_ui::ElementId(id.into())).id();
    let icon = app.world_mut().spawn(ChildOf(button)).id();
    let handler = tilt_ui::inventory::iter::<tilt_ui::HtmlHandlerRegistration>
        .into_iter()
        .find(|entry| entry.name == "inventory_sort")
        .unwrap();
    let system = (handler.register)(app.world_mut());
    app.world_mut()
        .run_system_with(
            system,
            tilt_ui::HtmlEvent {
                target: icon,
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
fn funnel_dropup_toggles_locally_and_closes_after_a_choice_or_dismissal() {
    let mut app = app(GameMode::Survival);
    action(&mut app, InventoryAction::Toggle);
    assert!(
        app.world()
            .resource::<InventoryView>()
            .sort_player_order
            .is_empty()
    );
    assert!(
        app.world()
            .resource::<InventoryView>()
            .sort_storage_order
            .is_empty()
    );
    click(&mut app, "sort-player-menu");
    assert!(app.world().resource::<InventoryView>().sort_player_open);
    assert!(!app.world().resource::<InventoryView>().sort_storage_open);
    assert!(requests(&mut app).is_empty());
    assert!(container_requests(&mut app).is_empty());
    click(&mut app, "sort-player-menu");
    assert!(!app.world().resource::<InventoryView>().sort_player_open);
    click(&mut app, "sort-player-menu");
    click(&mut app, "inventory-sort-dismiss");
    assert!(!app.world().resource::<InventoryView>().sort_player_open);
    assert!(requests(&mut app).is_empty());
    click(&mut app, "sort-player-menu");
    click(&mut app, "sort-player-name");
    assert!(!app.world().resource::<InventoryView>().sort_player_open);
    assert_eq!(requests(&mut app).len(), 1);
}

#[test]
fn native_sort_buttons_route_player_and_connected_storage_separately_without_prediction() {
    for (id, storage, order) in [
        ("sort-player-name", false, SortOrder::Alphabetical),
        ("sort-player-id", false, SortOrder::Id),
        ("sort-player-quantity", false, SortOrder::Quantity),
        ("sort-storage-name", true, SortOrder::Alphabetical),
        ("sort-storage-id", true, SortOrder::Id),
        ("sort-storage-quantity", true, SortOrder::Quantity),
        ("sort-linked-name", true, SortOrder::Alphabetical),
        ("sort-linked-id", true, SortOrder::Id),
        ("sort-linked-quantity", true, SortOrder::Quantity),
    ] {
        let mut app = app(GameMode::Survival);
        action(&mut app, InventoryAction::Toggle);
        {
            let mut session = app.world_mut().resource_mut::<ClientSession>();
            session.inventory.creative_set(
                14,
                terra_core::ItemStack {
                    item: ItemKind::Block(3),
                    count: 7,
                },
            );
            session.container = Some(chest_snapshot([1, 20, 1], true));
        }
        app.update();
        assert!(app.world().resource::<InventoryView>().sort_ready);
        assert!(app.world().resource::<InventoryView>().storage_sortable);
        let before = app.world().resource::<ClientSession>().inventory.clone();
        click(&mut app, id);
        assert_eq!(app.world().resource::<ClientSession>().inventory, before);
        assert!(!app.world().resource::<InventoryView>().sort_ready);
        let selected = match order {
            SortOrder::Alphabetical => "name",
            SortOrder::Id => "id",
            SortOrder::Quantity => "quantity",
            SortOrder::Category => "type",
        };
        let view = app.world().resource::<InventoryView>();
        assert_eq!(view.sort_player_order, if storage { "" } else { selected });
        assert_eq!(view.sort_storage_order, if storage { selected } else { "" });
        if storage {
            let sent = container_requests(&mut app);
            assert_eq!(sent.len(), 1);
            assert_eq!(
                sent[0].action,
                terra_core::containers::ContainerGesture::Sort { order }
            );
            assert_eq!(sent[0].anchor, [1, 20, 1]);
            assert_eq!(sent[0].revisions, [7, 11]);
            assert!(requests(&mut app).is_empty());
        } else {
            let sent = requests(&mut app);
            assert_eq!(sent.len(), 1);
            assert_eq!(
                sent[0].action,
                ServerAction::Sort {
                    order,
                    reserved: [0; terra_core::INVENTORY_SLOTS]
                }
            );
            assert_eq!(sent[0].revision, before.revision);
            assert!(container_requests(&mut app).is_empty());
        }
        click(&mut app, id);
        assert!(requests(&mut app).is_empty());
        assert!(container_requests(&mut app).is_empty());
    }
}

#[test]
fn sort_controls_reject_held_items_and_functional_machine_storage() {
    for held in [false, true] {
        let mut app = app(GameMode::Survival);
        action(&mut app, InventoryAction::Toggle);
        {
            let mut session = app.world_mut().resource_mut::<ClientSession>();
            session.container = Some(filtered_funnel_snapshot([1, 20, 1]));
            if held {
                session.inventory.cursor = Some(terra_core::ItemStack {
                    item: ItemKind::Block(3),
                    count: 1,
                });
            }
            session.inventory.revision += 1;
        }
        app.update();
        assert!(!app.world().resource::<InventoryView>().storage_sortable);
        assert_eq!(app.world().resource::<InventoryView>().sort_ready, !held);
        click(&mut app, "sort-storage-name");
        assert!(
            app.world()
                .resource::<InventoryView>()
                .sort_storage_order
                .is_empty()
        );
        assert!(container_requests(&mut app).is_empty());
        assert!(requests(&mut app).is_empty());
        click(&mut app, "sort-player-name");
        assert_eq!(
            app.world().resource::<InventoryView>().sort_player_order,
            if held { "" } else { "name" }
        );
        assert_eq!(requests(&mut app).len(), usize::from(!held));
        assert!(container_requests(&mut app).is_empty());
    }
}

fn acknowledge_sort(app: &mut App) {
    {
        let mut session = app.world_mut().resource_mut::<ClientSession>();
        session.inventory_response += 1;
        session.last_inventory =
            Some(terra_network::protocol::inventory::InventoryOutcome::Applied);
    }
    app.update();
}

#[test]
fn chosen_sort_survives_menu_dismissal_and_inventory_reopening_without_resorting() {
    let mut app = app(GameMode::Survival);
    action(&mut app, InventoryAction::Toggle);
    click(&mut app, "sort-player-name");
    assert_eq!(requests(&mut app).len(), 1);
    acknowledge_sort(&mut app);
    assert!(app.world().resource::<InventoryView>().sort_ready);

    click(&mut app, "sort-player-menu");
    assert!(app.world().resource::<InventoryView>().sort_player_open);
    assert_eq!(
        app.world().resource::<InventoryView>().sort_player_order,
        "name"
    );
    click(&mut app, "inventory-sort-dismiss");
    assert!(!app.world().resource::<InventoryView>().sort_player_open);
    assert_eq!(
        app.world().resource::<InventoryView>().sort_player_order,
        "name"
    );

    action(&mut app, InventoryAction::Toggle);
    assert!(!app.world().resource::<InventoryView>().open);
    action(&mut app, InventoryAction::Toggle);
    assert!(app.world().resource::<InventoryView>().open);
    click(&mut app, "sort-player-menu");
    assert_eq!(
        app.world().resource::<InventoryView>().sort_player_order,
        "name"
    );
    assert!(requests(&mut app).is_empty());
    assert!(container_requests(&mut app).is_empty());
}

#[test]
fn player_and_storage_choices_are_independent_and_other_chests_do_not_inherit_a_check() {
    let mut app = app(GameMode::Survival);
    let anchor = [-3, 20, 1];
    app.world_mut().resource_mut::<ClientSession>().container = Some(chest_snapshot(anchor, false));
    action(&mut app, InventoryAction::Toggle);

    click(&mut app, "sort-player-id");
    assert_eq!(requests(&mut app).len(), 1);
    acknowledge_sort(&mut app);
    click(&mut app, "sort-storage-quantity");
    assert_eq!(container_requests(&mut app).len(), 1);
    acknowledge_sort(&mut app);
    let view = app.world().resource::<InventoryView>();
    assert_eq!(view.sort_player_order, "id");
    assert_eq!(view.sort_storage_order, "quantity");

    click(&mut app, "sort-storage-menu");
    assert!(app.world().resource::<InventoryView>().sort_storage_open);
    assert_eq!(
        app.world().resource::<InventoryView>().sort_storage_order,
        "quantity"
    );
    click(&mut app, "inventory-sort-dismiss");
    app.world_mut().resource_mut::<ClientSession>().container =
        Some(chest_snapshot([anchor[0] + 8, anchor[1], anchor[2]], false));
    app.update();
    let view = app.world().resource::<InventoryView>();
    assert_eq!(view.sort_player_order, "id");
    assert!(view.sort_storage_order.is_empty());
    assert!(requests(&mut app).is_empty());
    assert!(container_requests(&mut app).is_empty());
}

#[test]
fn blocked_sort_requests_do_not_change_the_last_selected_choice() {
    let mut app = app(GameMode::Survival);
    action(&mut app, InventoryAction::Toggle);
    click(&mut app, "sort-player-id");
    assert_eq!(requests(&mut app).len(), 1);
    click(&mut app, "sort-player-quantity");
    assert_eq!(
        app.world().resource::<InventoryView>().sort_player_order,
        "id"
    );
    assert!(requests(&mut app).is_empty());

    acknowledge_sort(&mut app);
    app.world_mut()
        .resource_mut::<ClientSession>()
        .inventory
        .cursor = Some(terra_core::ItemStack {
        item: ItemKind::Block(3),
        count: 1,
    });
    app.update();
    click(&mut app, "sort-player-name");
    assert_eq!(
        app.world().resource::<InventoryView>().sort_player_order,
        "id"
    );
    assert!(requests(&mut app).is_empty());
    assert!(container_requests(&mut app).is_empty());
}

#[test]
fn authored_sort_bindings_keep_checks_and_selected_classes_in_sync_for_all_targets() {
    use tilt_ui::{ElementClasses, PropertyBinding, PropertyBindings, TiltUiCodePlugin};

    let template =
        include_str!("../../../src-ui/components/inventory-panel/inventory-panel.component.html");
    let mut app = App::new();
    app.add_plugins(TiltUiCodePlugin)
        .init_resource::<InventoryView>();
    let mut nodes = Vec::new();
    for target in ["player", "storage", "linked"] {
        for order in ["menu", "name", "id", "quantity"] {
            let id = format!("sort-{target}-{order}");
            let source = template
                .split_once(&format!("<button id=\"{id}\""))
                .unwrap_or_else(|| panic!("Missing authored sort button {id}"))
                .1
                .split_once("</button>")
                .unwrap()
                .0;
            let expression = source
                .split_once("[class]=\"")
                .unwrap()
                .1
                .split_once('"')
                .unwrap()
                .0;
            let button = app
                .world_mut()
                .spawn((
                    Node::default(),
                    PropertyBindings {
                        bindings: vec![PropertyBinding {
                            name: "class".into(),
                            expression: expression.into(),
                        }],
                    },
                ))
                .id();
            let check = (order != "menu").then(|| {
                let (condition, branch) = source
                    .split_once("@if (")
                    .unwrap()
                    .1
                    .split_once(") {")
                    .unwrap();
                assert!(
                    branch.split_once('}').unwrap().0.contains("name=\"check\""),
                    "{id}"
                );
                // Evaluate the actual @if predicate through the runtime's boolean bindings.
                app.world_mut()
                    .spawn((
                        Node::default(),
                        ChildOf(button),
                        PropertyBindings {
                            bindings: vec![PropertyBinding {
                                name: "class.visible".into(),
                                expression: condition.into(),
                            }],
                        },
                    ))
                    .id()
            });
            nodes.push((id, button, check));
        }
    }

    for (player, storage, player_open, storage_open) in [
        ("", "", false, false),
        ("name", "quantity", true, false),
        ("name", "id", false, true),
        ("quantity", "id", false, false),
        ("id", "name", true, false),
        ("", "", false, false),
    ] {
        {
            let mut view = app.world_mut().resource_mut::<InventoryView>();
            view.sort_player_order = player.into();
            view.sort_storage_order = storage.into();
            view.sort_player_open = player_open;
            view.sort_storage_open = storage_open;
        }
        app.update();
        for (id, button, check) in &nodes {
            let target_is_player = id.starts_with("sort-player-");
            let order = id.rsplit_once('-').unwrap().1;
            let expected = if order == "menu" {
                if target_is_player {
                    player_open
                } else {
                    storage_open
                }
            } else {
                order == if target_is_player { player } else { storage }
            };
            let classes = &app.world().get::<ElementClasses>(*button).unwrap().classes;
            assert_eq!(
                classes.iter().any(|class| class == "selected"),
                expected,
                "{id}, player={player}, storage={storage}"
            );
            if let Some(check) = check {
                let visible = app
                    .world()
                    .get::<ElementClasses>(*check)
                    .is_some_and(|classes| classes.classes.iter().any(|class| class == "visible"));
                assert_eq!(
                    visible, expected,
                    "check for {id}, player={player}, storage={storage}"
                );
            } else {
                assert!(
                    classes
                        .iter()
                        .any(|class| class == "inventory-action-button"),
                    "{id}"
                );
            }
        }
    }
}
