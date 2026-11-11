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
        assert!(container_requests(&mut app).is_empty());
        assert!(requests(&mut app).is_empty());
        click(&mut app, "sort-player-name");
        assert_eq!(requests(&mut app).len(), usize::from(!held));
        assert!(container_requests(&mut app).is_empty());
    }
}
