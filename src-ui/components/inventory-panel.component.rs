use bevy::prelude::*;
use tilt_ui::{ElementId, HtmlEvent, ImageMetadata, html_fn};

struct InventoryIntent {
    pub item: String,
    pub index: String,
    pub command: String,
}

#[html_fn("inventory_category_select")]
fn inventory_category_select(In(event): In<HtmlEvent>, mut actions: MessageWriter<crate::InventoryAction>) {
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
    let Some(intent) = materialized_slot(
        event.target,
        view.creative,
        &elements,
        &parents,
        &children,
        &images,
    ) else {
        return;
    };
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
            Some("container-grid" | "workbench-container-grid" | "furnace-input" | "crusher-input") => {
                ("container", 0)
            }
            Some("furnace-fuel") => ("container", 1),
            Some("crusher-output") => ("container", 1),
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
        "inventory-page-previous" => {
            actions.write(crate::InventoryAction::Page(-1));
        }
        "inventory-page-next" => {
            actions.write(crate::InventoryAction::Page(1));
        }
        _ => {}
    }
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
    fn furnace_slots_dispatch_clicks_and_hover_with_their_shared_inventory_indices() {
        let mut world = World::new();
        world.init_resource::<crate::InventoryView>();
        world.init_resource::<Messages<crate::InventoryAction>>();
        for (root_id, index, slug) in [
            ("furnace-input", 0, "raw_iron"),
            ("furnace-fuel", 1, "coal"),
            ("furnace-output", 2, "iron_ingot"),
            ("furnace-bellows", 3, "ruby_bellows"),
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
                assert_eq!(actual_index, Some(index));
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
