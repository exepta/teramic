use bevy::prelude::*;
use tilt_ui::{ElementId, HtmlEvent, ImageMetadata, html_fn};

struct InventoryIntent {
    pub item: String,
    pub index: String,
    pub command: String,
}

#[html_fn("inventory_hearth_view")]
fn inventory_hearth_view(
    In(_event): In<HtmlEvent>,
    view: Res<crate::InventoryView>,
    mut actions: MessageWriter<crate::InventoryAction>,
) {
    if view.hearth_navigation && view.hearth_navigation_enabled {
        actions.write(crate::InventoryAction::HearthView(view.hearth_show_fire));
    }
}

#[html_fn("inventory_brewing_recipe")]
fn inventory_brewing_recipe(
    In(event): In<HtmlEvent>,
    mut actions: MessageWriter<crate::InventoryAction>,
) {
    if let Some(recipe) = event
        .value
        .as_deref()
        .and_then(|value| value.parse::<u16>().ok())
        .filter(|id| terra_core::brewing::recipe(*id).is_some())
    {
        actions.write(crate::InventoryAction::BrewingRecipe(recipe));
    }
}

#[html_fn("inventory_campfire_lit")]
fn inventory_campfire_lit(
    In(event): In<HtmlEvent>,
    elements: Query<&ElementId>,
    mut actions: MessageWriter<crate::InventoryAction>,
) {
    if let Ok(id) = elements.get(event.target) {
        match id.0.as_str() {
            "campfire-light" => {
                actions.write(crate::InventoryAction::CampfireLit(true));
            }
            "campfire-extinguish" => {
                actions.write(crate::InventoryAction::CampfireLit(false));
            }
            _ => {}
        }
    }
}

#[html_fn("inventory_tunnel_random")]
fn inventory_tunnel_random(
    In(event): In<HtmlEvent>,
    mut actions: MessageWriter<crate::InventoryAction>,
) {
    if let Some(random) = event
        .value
        .as_deref()
        .and_then(|value| value.parse::<bool>().ok())
    {
        actions.write(crate::InventoryAction::TunnelRandom(random));
    }
}

#[html_fn("inventory_filter_mode")]
fn inventory_filter_mode(
    In(event): In<HtmlEvent>,
    mut actions: MessageWriter<crate::InventoryAction>,
) {
    if let Some(value) = event
        .value
        .as_deref()
        .and_then(|value| value.parse::<bool>().ok())
    {
        actions.write(crate::InventoryAction::FilterAllowed(value));
    }
}

#[html_fn("inventory_distributor_mode")]
fn inventory_distributor_mode(
    In(event): In<HtmlEvent>,
    elements: Query<&ElementId>,
    mut actions: MessageWriter<crate::InventoryAction>,
) {
    if let Ok(id) = elements.get(event.target)
        && let Some(output) =
            id.0.strip_prefix("distributor-filter-mode-")
                .and_then(|value| value.parse::<u8>().ok())
                .filter(|output| *output < 3)
        && let Some(allowed) = event
            .value
            .as_deref()
            .and_then(|value| value.parse::<bool>().ok())
    {
        actions.write(crate::InventoryAction::DistributorAllowed(output, allowed));
    }
}

#[html_fn("inventory_distributor_clear")]
fn inventory_distributor_clear(
    In(event): In<HtmlEvent>,
    elements: Query<&ElementId>,
    mut actions: MessageWriter<crate::InventoryAction>,
) {
    if let Ok(id) = elements.get(event.target)
        && let Some(output) =
            id.0.strip_prefix("distributor-filter-clear-")
                .and_then(|value| value.parse::<u8>().ok())
                .filter(|output| *output < 3)
    {
        actions.write(crate::InventoryAction::DistributorClear(output));
    }
}

#[html_fn("inventory_mixer_color")]
fn inventory_mixer_color(
    In(event): In<HtmlEvent>,
    mut actions: MessageWriter<crate::InventoryAction>,
) {
    if let Some(value) = event.value {
        actions.write(crate::InventoryAction::MixerColor(value));
    }
}

#[html_fn("inventory_category_select")]
fn inventory_category_select(
    In(event): In<HtmlEvent>,
    mut actions: MessageWriter<crate::InventoryAction>,
) {
    if let Some(value) = event.value {
        actions.write(crate::InventoryAction::Category(value));
    }
}

#[html_fn("inventory_category")]
fn inventory_category(
    In(event): In<HtmlEvent>,
    view: Res<crate::InventoryView>,
    elements: Query<&ElementId>,
    parents: Query<&ChildOf>,
    children: Query<&Children>,
    mut actions: MessageWriter<crate::InventoryAction>,
) {
    if let Some(index) = materialized_category(event.target, &elements, &parents, &children)
        && let Some(category) = view.categories.get(index)
    {
        actions.write(crate::InventoryAction::Category(category.id.clone()));
    }
}

fn materialized_category(
    target: Entity,
    elements: &Query<&ElementId>,
    parents: &Query<&ChildOf>,
    children: &Query<&Children>,
) -> Option<usize> {
    if !elements
        .get(target)
        .is_ok_and(|id| id.0 == "inventory-category")
    {
        return None;
    }
    let mut root = target;
    loop {
        root = parents.get(root).ok()?.parent();
        if elements
            .get(root)
            .is_ok_and(|id| id.0 == "inventory-categories")
        {
            break;
        }
    }
    children
        .iter_descendants(root)
        .filter(|entity| {
            elements
                .get(*entity)
                .is_ok_and(|id| id.0 == "inventory-category")
        })
        .position(|entity| entity == target)
}

#[html_fn("inventory_search")]
fn inventory_search(In(event): In<HtmlEvent>, mut actions: MessageWriter<crate::InventoryAction>) {
    if let Some(value) = event.value {
        actions.write(crate::InventoryAction::Search(value));
    }
}

#[html_fn("inventory_pointer")]
fn inventory_pointer(
    In(event): In<HtmlEvent>,
    view: Res<crate::InventoryView>,
    elements: Query<&ElementId>,
    parents: Query<&ChildOf>,
    children: Query<&Children>,
    images: Query<&ImageMetadata>,
    mut actions: MessageWriter<crate::InventoryAction>,
) {
    let Some(mut intent) = materialized_slot(
        event.target,
        view.creative,
        &elements,
        &parents,
        &children,
        &images,
    ) else {
        return;
    };
    if let Some(item) = view_item(&view, &intent.command, intent.index.parse().ok()) {
        intent.item = item.into();
    }
    let index = intent.index.parse::<u8>().ok();
    let secondary = event
        .data
        .get("button")
        .is_some_and(|button| button == "Secondary");
    let action = match event.kind {
        "mousedown"
            if event
                .data
                .get("button")
                .is_none_or(|button| button == "Primary" || button == "Secondary") =>
        {
            Some(crate::InventoryAction::Pointer {
                command: intent.command.clone(),
                index,
                item: intent.item.clone(),
                secondary,
            })
        }
        "mouseenter" => Some(crate::InventoryAction::Hover {
            command: intent.command.clone(),
            index,
            item: intent.item.clone(),
        }),
        "mouseleave" => Some(crate::InventoryAction::Leave {
            command: intent.command.clone(),
            index,
            item: intent.item.clone(),
        }),
        _ => None,
    };
    if let Some(action) = action {
        actions.write(action);
    }
}

fn view_item<'a>(
    view: &'a crate::InventoryView,
    command: &str,
    index: Option<usize>,
) -> Option<&'a str> {
    let index = index?;
    let slot = match command {
        "slot" => view.items.get(index.checked_sub(9)?),
        "hotbar" => view.hotbar.get(index),
        "container" => view.container_items.get(index),
        "catalog" => view.browser_items.get(index),
        "equipment" => view.equipment.get(index).map(|slot| &slot.item),
        _ => None,
    }?;
    Some(slot.item_id.as_str())
}

fn materialized_slot(
    target: Entity,
    _creative: bool,
    elements: &Query<&ElementId>,
    parents: &Query<&ChildOf>,
    children: &Query<&Children>,
    images: &Query<&ImageMetadata>,
) -> Option<InventoryIntent> {
    if !elements
        .get(target)
        .is_ok_and(|id| id.0 == "inventory-cell")
    {
        return None;
    }
    let mut current = target;
    let (root, command, offset) = loop {
        current = parents.get(current).ok()?.parent();
        let id = elements.get(current).ok().map(|id| id.0.as_str());
        let (command, offset) = match id {
            Some("inventory-grid") => ("slot", 9),
            Some("inventory-equipment") => ("equipment", 0),
            Some(
                "container-grid"
                | "workbench-container-grid"
                | "furnace-input"
                | "crusher-input"
                | "mixer-input"
                | "cauldron-inputs"
                | "campfire-input",
            ) => ("container", 0),
            Some("furnace-fuel") => ("container", 1),
            Some("crusher-output") => ("container", 1),
            Some("mixer-output") => ("container", terra_core::mixer::OUTPUT),
            Some("cauldron-water") => ("container", terra_core::brewing::WATER),
            Some("cauldron-bottles") => ("container", terra_core::brewing::BOTTLES),
            Some("cauldron-spoon") => ("container", terra_core::brewing::SPOON),
            Some("cauldron-output") => ("container", terra_core::brewing::OUTPUT),
            Some("campfire-fuel") => ("container", terra_core::campfire::FUEL),
            Some("campfire-output") => ("container", terra_core::campfire::OUTPUT),
            Some("funnel-filter-slot") => ("filter", 0),
            Some("distributor-filter-slots") => ("distributor-filter", 0),
            Some("furnace-output") => ("container", 2),
            Some("furnace-bellows") => ("container", 3),
            Some("inventory-browser-grid") => ("catalog", 0),
            Some("inventory-hotbar" | "inventory-hud-hotbar") => ("hotbar", 0),
            Some("inventory-crafting") => ("craft-cell", 0),
            Some("inventory-craft-result") => ("craft", 0),
            Some("crafting-grid") => ("bench-cell", 0),
            Some("crafting-output") => ("bench-output", 0),
            Some("crafting-stock") => ("bench-source", 0),
            Some("recipe-preview-grid" | "recipe-preview-output") => ("recipe-preview", 0),
            _ => continue,
        };
        break (current, command, offset);
    };
    let index = children
        .iter_descendants(root)
        .filter(|entity| {
            elements
                .get(*entity)
                .is_ok_and(|id| id.0 == "inventory-cell")
        })
        .position(|entity| entity == target)?;
    let index = index + offset;
    let item = children
        .iter_descendants(target)
        .find_map(|entity| {
            let alt = images.get(entity).ok()?.alt.as_deref()?;
            alt.strip_prefix("Block preview ")
                .map(|id| format!("block-{id}"))
                .or_else(|| {
                    alt.strip_prefix("Prop preview ")
                        .map(|slug| format!("prop-{slug}"))
                })
                .or_else(|| {
                    let identifier = alt.strip_prefix("Item preview ")?;
                    if let Some(rgb) = identifier.strip_prefix("teramic::mixed_dye#")
                        && rgb.len() == 6
                        && rgb.bytes().all(|byte| byte.is_ascii_hexdigit())
                    {
                        return Some(format!("item-mixed_dye#{rgb}"));
                    }
                    if let Some((identifier, wood)) = identifier.split_once('#') {
                        let material = (1..=terra_voxel::MaterialId::LAST).find(|&material| {
                            terra_voxel::MaterialId::from_raw(material)
                                .is_some_and(|material| material.identifier() == identifier)
                        })?;
                        Some(format!("block-{material}-{wood}"))
                    } else {
                        identifier
                            .strip_prefix("teramic::")
                            .map(|slug| format!("item-{slug}"))
                    }
                })
        })
        .unwrap_or_default();
    Some(InventoryIntent {
        command: command.into(),
        index: index.to_string(),
        item,
    })
}

#[html_fn("inventory_outside")]
fn inventory_outside(
    In(event): In<HtmlEvent>,
    elements: Query<&ElementId>,
    mut actions: MessageWriter<crate::InventoryAction>,
) {
    if elements
        .get(event.target)
        .is_ok_and(|id| matches!(id.0.as_str(), "inventory-backdrop" | "inventory-shell"))
        && let Some(button) = event.data.get("button")
        && matches!(button.as_str(), "Primary" | "Secondary")
    {
        actions.write(crate::InventoryAction::Outside(button == "Secondary"));
    }
}

#[html_fn("inventory_action")]
fn inventory_action(
    In(event): In<HtmlEvent>,
    elements: Query<&ElementId>,
    mut actions: MessageWriter<crate::InventoryAction>,
) {
    let Ok(id) = elements.get(event.target) else {
        return;
    };
    match id.0.as_str() {
        "inventory-assembly" => {
            actions.write(crate::InventoryAction::Assembly(true));
        }
        "inventory-backpack" => {
            actions.write(crate::InventoryAction::Assembly(false));
        }
        "inventory-close" => {
            actions.write(crate::InventoryAction::Close);
        }
        "inventory-trash" => {
            actions.write(crate::InventoryAction::Trash);
        }
        "funnel-filter-clear" => {
            actions.write(crate::InventoryAction::FilterClear);
        }
        "inventory-page-previous" => {
            actions.write(crate::InventoryAction::Page(-1));
        }
        "inventory-page-next" => {
            actions.write(crate::InventoryAction::Page(1));
        }
        "inventory-browser-filter" => {
            actions.write(crate::InventoryAction::Category("all".into()));
        }
        _ => {}
    }
}

#[html_fn("inventory_recipe_book")]
fn inventory_recipe_book(
    In(_event): In<HtmlEvent>,
    mut actions: MessageWriter<crate::InventoryAction>,
) {
    actions.write(crate::InventoryAction::ToggleBrowser);
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::system::{RunSystemOnce, SystemState};

    fn resolve(world: &mut World, target: Entity, creative: bool) -> Option<InventoryIntent> {
        let mut state = SystemState::<(
            Query<&ElementId>,
            Query<&ChildOf>,
            Query<&Children>,
            Query<&ImageMetadata>,
        )>::new(world);
        let (elements, parents, children, images) = state
            .get(world)
            .expect("Query state belongs to the fixture world");
        materialized_slot(target, creative, &elements, &parents, &children, &images)
    }

    #[test]
    fn pointer_targets_use_materialized_empty_slot_positions_and_real_preview_metadata() {
        let mut world = World::new();
        let grid = world.spawn(ElementId("inventory-grid".into())).id();
        let first = world
            .spawn((ElementId("inventory-cell".into()), ChildOf(grid)))
            .id();
        let second = world
            .spawn((ElementId("inventory-cell".into()), ChildOf(grid)))
            .id();
        world.spawn((
            ImageMetadata {
                alt: Some("Prop preview flower".into()),
                ..default()
            },
            ChildOf(second),
        ));
        let empty = resolve(&mut world, first, false).unwrap();
        assert_eq!(empty.command, "slot");
        assert_eq!(empty.index, "9");
        assert!(empty.item.is_empty());
        let flower = resolve(&mut world, second, false).unwrap();
        assert_eq!(flower.index, "10");
        assert_eq!(flower.item, "prop-flower");
    }

    #[test]
    fn station_slots_dispatch_clicks_and_hover_with_their_shared_inventory_indices() {
        let mut world = World::new();
        world.init_resource::<crate::InventoryView>();
        world.init_resource::<Messages<crate::InventoryAction>>();
        for (root_id, index, slug) in [
            ("furnace-input", 0, "raw_iron"),
            ("furnace-fuel", 1, "coal"),
            ("furnace-output", 2, "iron_ingot"),
            ("furnace-bellows", 3, "ruby_bellows"),
            ("cauldron-inputs", 0, "red_flower"),
            ("cauldron-water", terra_core::brewing::WATER, "water_bottle"),
            (
                "cauldron-bottles",
                terra_core::brewing::BOTTLES,
                "glass_bottle",
            ),
            ("cauldron-spoon", terra_core::brewing::SPOON, "gold_spoon"),
            (
                "cauldron-output",
                terra_core::brewing::OUTPUT,
                "regeneration_potion",
            ),
            ("campfire-input", terra_core::campfire::INPUT, "beef"),
            ("campfire-fuel", terra_core::campfire::FUEL, "coal"),
            (
                "campfire-output",
                terra_core::campfire::OUTPUT,
                "cooked_beef",
            ),
        ] {
            let root = world.spawn(ElementId(root_id.into())).id();
            let wrapper = world.spawn(ChildOf(root)).id();
            let target = world
                .spawn((ElementId("inventory-cell".into()), ChildOf(wrapper)))
                .id();
            world.spawn((
                ImageMetadata {
                    alt: Some(format!("Item preview teramic::{slug}")),
                    ..default()
                },
                ChildOf(target),
            ));
            let resolved = resolve(&mut world, target, false).unwrap();
            assert_eq!(resolved.command, "container");
            assert_eq!(resolved.index, index.to_string());
            assert_eq!(resolved.item, format!("item-{slug}"));
            for (kind, button) in [
                ("mousedown", "Primary"),
                ("mousedown", "Secondary"),
                ("mouseenter", "Primary"),
                ("mouseleave", "Primary"),
            ] {
                world
                    .run_system_once_with(
                        inventory_pointer,
                        HtmlEvent {
                            target,
                            kind,
                            value: None,
                            submitter: None,
                            data: [("button".into(), button.into())].into(),
                            form_data: None,
                            handler: "inventory_pointer".into(),
                        },
                    )
                    .unwrap();
                let action = world
                    .resource_mut::<Messages<crate::InventoryAction>>()
                    .drain()
                    .next()
                    .unwrap();
                let (command, actual_index, item) = match action {
                    crate::InventoryAction::Pointer {
                        command,
                        index,
                        item,
                        secondary,
                    } => {
                        assert_eq!(kind, "mousedown");
                        assert_eq!(secondary, button == "Secondary");
                        (command, index, item)
                    }
                    crate::InventoryAction::Hover {
                        command,
                        index,
                        item,
                    } => {
                        assert_eq!(kind, "mouseenter");
                        (command, index, item)
                    }
                    crate::InventoryAction::Leave {
                        command,
                        index,
                        item,
                    } => {
                        assert_eq!(kind, "mouseleave");
                        (command, index, item)
                    }
                    action => panic!("Unexpected furnace slot action: {action:?}"),
                };
                assert_eq!(command, "container");
                assert_eq!(actual_index, Some(index as u8));
                assert_eq!(item, format!("item-{slug}"));
            }
        }
    }

    #[test]
    fn stale_filter_hover_targets_are_ignored_before_reading_slot_data() {
        let mut world = World::new();
        let grid = world.spawn(ElementId("inventory-browser-grid".into())).id();
        let old = world
            .spawn((ElementId("inventory-cell".into()), ChildOf(grid)))
            .id();
        world.entity_mut(old).despawn();
        assert!(resolve(&mut world, old, true).is_none());
        let current = world
            .spawn((ElementId("inventory-cell".into()), ChildOf(grid)))
            .id();
        world.spawn((
            ImageMetadata {
                alt: Some("Block preview 21".into()),
                ..default()
            },
            ChildOf(current),
        ));
        let current = resolve(&mut world, current, true).unwrap();
        assert_eq!(current.command, "catalog");
        assert_eq!(current.item, "block-21");
    }

    #[test]
    fn shared_backpack_and_crafting_roots_keep_their_commands_in_both_modes() {
        for creative in [false, true] {
            for (root, command, index) in [
                ("inventory-grid", "slot", "9"),
                ("container-grid", "container", "0"),
                ("inventory-browser-grid", "catalog", "0"),
                ("inventory-crafting", "craft-cell", "0"),
                ("inventory-craft-result", "craft", "0"),
                ("crafting-grid", "bench-cell", "0"),
                ("crafting-output", "bench-output", "0"),
                ("recipe-preview-grid", "recipe-preview", "0"),
                ("recipe-preview-output", "recipe-preview", "0"),
                ("funnel-filter-slot", "filter", "0"),
            ] {
                let mut world = World::new();
                let root = world.spawn(ElementId(root.into())).id();
                let target = world
                    .spawn((ElementId("inventory-cell".into()), ChildOf(root)))
                    .id();
                let resolved = resolve(&mut world, target, creative).unwrap();
                assert_eq!(resolved.command, command);
                assert_eq!(resolved.index, index);
            }
        }
    }

    #[test]
    fn materialized_rgb_inventory_items_use_the_exact_view_identity() {
        let mut world = World::new();
        world.init_resource::<crate::InventoryView>();
        world.init_resource::<Messages<crate::InventoryAction>>();
        {
            let mut view = world.resource_mut::<crate::InventoryView>();
            view.items.push(Default::default());
            view.items[0].item_id = "item-conveyor_belt#cherry#112233".into();
        }
        let grid = world.spawn(ElementId("inventory-grid".into())).id();
        let target = world
            .spawn((ElementId("inventory-cell".into()), ChildOf(grid)))
            .id();
        world.spawn((
            ImageMetadata {
                alt: Some("Item preview teramic::conveyor_belt#cherry#112233".into()),
                ..default()
            },
            ChildOf(target),
        ));
        world
            .run_system_once_with(
                inventory_pointer,
                HtmlEvent {
                    target,
                    kind: "mouseenter",
                    value: None,
                    submitter: None,
                    data: default(),
                    form_data: None,
                    handler: "inventory_pointer".into(),
                },
            )
            .unwrap();
        let action = world
            .resource_mut::<Messages<crate::InventoryAction>>()
            .drain()
            .next()
            .unwrap();
        assert!(
            matches!(action, crate::InventoryAction::Hover { command, index: Some(9), item } if command == "slot" && item == "item-conveyor_belt#cherry#112233")
        );
    }

    #[test]
    fn filter_switch_rejects_malformed_values_and_emits_boolean_mode() {
        let mut world = World::new();
        world.init_resource::<Messages<crate::InventoryAction>>();
        let target = world.spawn(ElementId("funnel-filter-mode".into())).id();
        for value in ["false", "true", "blocked", ""] {
            world
                .run_system_once_with(
                    inventory_filter_mode,
                    HtmlEvent {
                        target,
                        kind: "change",
                        value: Some(value.into()),
                        submitter: None,
                        data: default(),
                        form_data: None,
                        handler: "inventory_filter_mode".into(),
                    },
                )
                .unwrap();
        }
        let messages = world
            .resource_mut::<Messages<crate::InventoryAction>>()
            .drain()
            .collect::<Vec<_>>();
        assert_eq!(messages.len(), 2);
        assert!(matches!(
            messages[0],
            crate::InventoryAction::FilterAllowed(false)
        ));
        assert!(matches!(
            messages[1],
            crate::InventoryAction::FilterAllowed(true)
        ));
    }

    #[test]
    fn distributor_native_handlers_keep_output_indices_and_reject_invalid_rows() {
        let mut world = World::new();
        world.init_resource::<Messages<crate::InventoryAction>>();
        for output in 0..=3 {
            let target = world
                .spawn(ElementId(format!("distributor-filter-mode-{output}")))
                .id();
            world
                .run_system_once_with(
                    inventory_distributor_mode,
                    HtmlEvent {
                        target,
                        kind: "change",
                        value: Some("false".into()),
                        submitter: None,
                        data: default(),
                        form_data: None,
                        handler: "inventory_distributor_mode".into(),
                    },
                )
                .unwrap();
            let target = world
                .spawn(ElementId(format!("distributor-filter-clear-{output}")))
                .id();
            world
                .run_system_once_with(
                    inventory_distributor_clear,
                    HtmlEvent {
                        target,
                        kind: "click",
                        value: None,
                        submitter: None,
                        data: default(),
                        form_data: None,
                        handler: "inventory_distributor_clear".into(),
                    },
                )
                .unwrap();
        }
        let actions: Vec<_> = world
            .resource_mut::<Messages<crate::InventoryAction>>()
            .drain()
            .collect();
        assert_eq!(actions.len(), 6);
        for output in 0..3 {
            assert!(
                matches!(actions[output * 2], crate::InventoryAction::DistributorAllowed(index, false) if index == output as u8)
            );
            assert!(
                matches!(actions[output * 2 + 1], crate::InventoryAction::DistributorClear(index) if index == output as u8)
            );
        }
        let root = world
            .spawn(ElementId("distributor-filter-slots".into()))
            .id();
        let mut slots = Vec::new();
        for _ in 0..3 {
            let row = world.spawn(ChildOf(root)).id();
            slots.push(
                world
                    .spawn((ElementId("inventory-cell".into()), ChildOf(row)))
                    .id(),
            );
        }
        for (index, target) in slots.into_iter().enumerate() {
            let resolved = resolve(&mut world, target, false).unwrap();
            assert_eq!(resolved.command, "distributor-filter");
            assert_eq!(resolved.index, index.to_string());
        }
    }

    #[test]
    fn crafted_wood_preview_resolves_the_variant_item_instead_of_a_tool_slug() {
        let mut world = World::new();
        let root = world.spawn(ElementId("inventory-grid".into())).id();
        let target = world
            .spawn((ElementId("inventory-cell".into()), ChildOf(root)))
            .id();
        world.spawn((
            ImageMetadata {
                alt: Some("Item preview teramic::chest#birch".into()),
                ..default()
            },
            ChildOf(target),
        ));
        assert_eq!(
            resolve(&mut world, target, false).unwrap().item,
            "block-29-birch"
        );
    }

    #[test]
    fn category_click_resolves_materialized_order_and_ignores_unmounted_tabs() {
        let mut world = World::new();
        let root = world.spawn(ElementId("inventory-categories".into())).id();
        let mut tabs = Vec::new();
        for _ in 0..3 {
            let wrapper = world.spawn(ChildOf(root)).id();
            tabs.push(
                world
                    .spawn((ElementId("inventory-category".into()), ChildOf(wrapper)))
                    .id(),
            );
        }
        let mut state =
            SystemState::<(Query<&ElementId>, Query<&ChildOf>, Query<&Children>)>::new(&mut world);
        {
            let (elements, parents, children) = state.get(&world).unwrap();
            assert_eq!(
                materialized_category(tabs[2], &elements, &parents, &children),
                Some(2)
            );
        }
        world.entity_mut(tabs[1]).despawn();
        let (elements, parents, children) = state.get(&world).unwrap();
        assert!(materialized_category(tabs[1], &elements, &parents, &children).is_none());
        assert_eq!(
            materialized_category(tabs[2], &elements, &parents, &children),
            Some(1)
        );
    }

    #[test]
    fn only_blank_shell_and_backdrop_are_outside_drop_targets() {
        let mut world = World::new();
        world.init_resource::<Messages<crate::InventoryAction>>();
        let backdrop = world.spawn(ElementId("inventory-backdrop".into())).id();
        let shell = world
            .spawn((ElementId("inventory-shell".into()), ChildOf(backdrop)))
            .id();
        let strip = world
            .spawn((ElementId("inventory-categories".into()), ChildOf(shell)))
            .id();
        let tab = world
            .spawn((ElementId("inventory-category".into()), ChildOf(strip)))
            .id();
        let panel = world
            .spawn((ElementId("inventory-panel".into()), ChildOf(shell)))
            .id();
        for target in [shell, strip, tab, panel, backdrop] {
            world
                .run_system_cached_with(
                    inventory_outside,
                    HtmlEvent {
                        target,
                        kind: "mousedown",
                        value: None,
                        submitter: None,
                        data: [("button".into(), "Primary".into())].into_iter().collect(),
                        form_data: None,
                        handler: "inventory_outside".into(),
                    },
                )
                .unwrap();
        }
        let actions = world
            .resource_mut::<Messages<crate::InventoryAction>>()
            .drain()
            .collect::<Vec<_>>();
        assert_eq!(actions.len(), 2);
        assert!(
            actions
                .iter()
                .all(|action| matches!(action, crate::InventoryAction::Outside(false)))
        );
    }
}
