//! Exercise the authored slot bindings through TiltUI, including retained trees.
use super::*;
use serde::Serialize;
use tilt_ui::{
    ElementClasses, ParkedUiBindings, PropertyBinding, PropertyBindings, TiltUiCodePlugin,
    UiBindingStore, UiStore,
};

#[derive(Default, Serialize)]
#[serde(transparent)]
struct Slot(view::InventoryItemView);

impl UiStore for Slot {
    const STORE_KEY: &'static str = "Slot";
    const STORE_PATH: &'static str = "";
}

// Read the actual authored expressions, so changing the template can regress
// this test. The shared resources and expression evaluator are the real ones.
fn class_bindings() -> Vec<String> {
    include_str!("../../../src-ui/components/inventory-slot.component.html")
        .split("[class]=\"")
        .skip(1)
        .map(|tail| tail.split_once('"').unwrap().0.to_owned())
        .collect()
}

fn fixture(item: view::InventoryItemView, player: bool) -> (App, [Entity; 4]) {
    let mut app = App::new();
    app.add_plugins(TiltUiCodePlugin)
        .insert_resource(InventoryView {
            visible: true,
            player,
            prewarming: true,
            ..default()
        })
        .init_resource::<crafting::CraftingView>();
    app.world_mut()
        .resource_mut::<UiBindingStore>()
        .set_store(Slot(item));
    let bindings = class_bindings();
    assert_eq!(bindings.len(), 5);
    // Wrapper, button, ordinary count, ghost count. The image's missing class
    // is exercised by native preview tests and does not depend on opening.
    let mut nodes = Vec::new();
    for index in [0, 1, 3, 4] {
        let entity = app
            .world_mut()
            .spawn((
                Node::default(),
                PropertyBindings {
                    bindings: vec![PropertyBinding {
                        name: "class".into(),
                        expression: bindings[index].clone(),
                    }],
                },
            ))
            .id();
        if let Some(&root) = nodes.first() {
            app.world_mut().entity_mut(entity).insert(ChildOf(root));
        }
        nodes.push(entity);
    }
    app.update();
    (app, nodes.try_into().unwrap())
}

fn classes(app: &App, entity: Entity) -> String {
    app.world()
        .get::<ElementClasses>(entity)
        .unwrap()
        .classes
        .join(" ")
}

fn snapshot(app: &App, nodes: [Entity; 4]) -> Vec<(Entity, Vec<String>, u32)> {
    nodes
        .into_iter()
        .map(|entity| {
            let classes = app
                .world()
                .entity(entity)
                .get_ref::<ElementClasses>()
                .unwrap();
            (
                entity,
                classes.classes.clone(),
                classes.last_changed().get(),
            )
        })
        .collect()
}

#[test]
fn retained_slot_styles_are_ready_while_hidden_and_survive_open_close_and_restore() {
    for (command, id, player, expected) in [
        (
            "slot",
            "slot-9",
            true,
            "item-slot reference-slot square-player-slot",
        ),
        (
            "hotbar",
            "hotbar-0",
            true,
            "item-slot reference-slot square-player-slot",
        ),
        ("preview", "catalog-0", true, "item-slot reference-slot"),
        (
            "craft",
            "result",
            true,
            "item-slot reference-slot reference-result",
        ),
        (
            "container",
            "container-0",
            false,
            "item-slot reference-slot",
        ),
        (
            "container",
            "mixer-output",
            false,
            "item-slot reference-slot",
        ),
    ] {
        let (mut app, nodes) = fixture(
            view::InventoryItemView {
                command: command.into(),
                id: id.into(),
                count: "17".into(),
                ghost_count: "3".into(),
                ghost_preview: "Block preview 3".into(),
                ..default()
            },
            player,
        );
        assert_eq!(classes(&app, nodes[0]), expected);
        assert_eq!(classes(&app, nodes[2]), "item-count inventory-stack-count");
        assert_eq!(
            classes(&app, nodes[3]),
            "item-count mixer-ghost-count inventory-stack-count"
        );
        let prepared = snapshot(&app, nodes);
        for open in [true, false, true, false] {
            {
                let mut view = app.world_mut().resource_mut::<InventoryView>();
                view.prewarming = false;
                view.open = open;
            }
            app.update();
            assert_eq!(
                snapshot(&app, nodes),
                prepared,
                "{command}/{id}, open={open}"
            );
        }
        let parked = ParkedUiBindings::take(app.world_mut(), nodes[0]);
        app.world_mut().resource_mut::<InventoryView>().open = true;
        app.update();
        assert_eq!(snapshot(&app, nodes), prepared);
        parked.restore(app.world_mut());
        app.update();
        assert_eq!(snapshot(&app, nodes), prepared, "restored {command}/{id}");
    }
}

#[test]
fn real_recipe_machine_and_selection_changes_still_update_retained_slot_styles() {
    let (mut app, nodes) = fixture(
        view::InventoryItemView {
            command: "slot".into(),
            id: "slot-9".into(),
            count: "17".into(),
            ..default()
        },
        true,
    );
    let original = snapshot(&app, nodes);
    app.world_mut()
        .resource_mut::<crafting::CraftingView>()
        .recipe_open = true;
    app.update();
    assert_eq!(classes(&app, nodes[2]), "item-count");
    assert_eq!(classes(&app, nodes[3]), "item-count mixer-ghost-count");
    assert_ne!(snapshot(&app, nodes)[2].2, original[2].2);
    app.world_mut().resource_mut::<InventoryView>().player = false;
    app.update();
    assert_eq!(classes(&app, nodes[0]), "item-slot reference-slot");
    for (id, command, brewing, expected) in [
        (
            "recipe-result",
            "recipe-preview",
            false,
            "item-slot reference-slot reference-result",
        ),
        (
            "smelting-result",
            "recipe-preview",
            false,
            "item-slot reference-slot reference-result recipe-machine-result",
        ),
        (
            "crushing-result",
            "recipe-preview",
            false,
            "item-slot reference-slot reference-result recipe-machine-result",
        ),
        (
            "campfire-result",
            "recipe-preview",
            false,
            "item-slot reference-slot reference-result recipe-machine-result",
        ),
        (
            "smelting-input",
            "recipe-preview",
            false,
            "item-slot reference-slot recipe-machine-input",
        ),
        (
            "brewing-result",
            "recipe-preview",
            true,
            "item-slot reference-slot reference-result recipe-cauldron-output",
        ),
        (
            "brewing-input",
            "recipe-preview",
            true,
            "item-slot reference-slot recipe-cauldron-ingredient",
        ),
    ] {
        app.world_mut()
            .resource_mut::<crafting::CraftingView>()
            .brewing = brewing;
        app.world_mut()
            .resource_mut::<UiBindingStore>()
            .set_store(Slot(view::InventoryItemView {
                id: id.into(),
                command: command.into(),
                selected: true,
                ..default()
            }));
        app.update();
        assert_eq!(classes(&app, nodes[0]), expected);
        assert_eq!(classes(&app, nodes[1]), "item-cell selected");
    }
    let mut item = app
        .world()
        .resource::<UiBindingStore>()
        .get_store::<Slot>()
        .unwrap()
        .0
        .clone();
    item.missing = true;
    app.world_mut()
        .resource_mut::<UiBindingStore>()
        .set_store(Slot(item));
    app.update();
    assert_eq!(classes(&app, nodes[1]), "item-cell missing");
}
