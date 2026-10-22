use super::*;
use serde::Serialize;
use tilt_ui::html_shared;

#[derive(Clone, PartialEq)]
pub(super) struct SessionViewStamp {
    ready: bool,
    mode: GameMode,
    revision: u64,
    outcome: Option<terra_network::protocol::inventory::InventoryOutcome>,
    container_anchor: Option<[i64; 3]>,
    container_workbench: Option<[i64; 3]>,
    furnace: Option<terra_core::furnace::FurnaceView>,
    container_revisions: [Option<([i64; 3], u64)>; 2],
}

#[derive(Clone, Debug, Default, Serialize, PartialEq)]
pub struct InventoryItemView {
    pub id: String,
    pub item_id: String,
    pub index: String,
    pub command: String,
    pub name: String,
    pub identifier: String,
    pub description: String,
    pub category: String,
    pub count: String,
    pub preview_key: String,
    pub prop: bool,
    pub selected: bool,
    pub empty: bool,
    pub disabled: bool,
    pub missing: bool,
    pub hotkey: String,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct CategoryView {
    pub id: String,
    pub name: String,
    pub selected: bool,
    pub preview_key: String,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct EquipmentSlotView {
    pub kind: &'static str,
    pub name: &'static str,
}

#[html_shared]
#[derive(Resource, Default, Debug, Serialize, PartialEq)]
pub struct InventoryView {
    pub layout_style: String,
    pub open: bool,
    pub assembly: bool,
    pub container: bool,
    pub double_container: bool,
    pub linked_container: bool,
    pub linked_title: String,
    pub container_capacity: String,
    pub container_items: Vec<InventoryItemView>,
    pub furnace: bool,
    pub furnace_input: Vec<InventoryItemView>,
    pub furnace_fuel: Vec<InventoryItemView>,
    pub furnace_output: Vec<InventoryItemView>,
    pub furnace_bellows: Vec<InventoryItemView>,
    pub furnace_progress_style: String,
    pub furnace_fuel_style: String,
    pub furnace_progress: String,
    pub furnace_status: String,
    pub visible: bool,
    pub creative: bool,
    pub trash_available: bool,
    pub empty: bool,
    pub title: String,
    pub mode: String,
    pub capacity: String,
    pub search: String,
    pub selected_name: String,
    pub selected_count: String,
    pub selected_preview: String,
    pub selected_empty: bool,
    pub detail_name: String,
    pub detail_kind: String,
    pub detail_identifier: String,
    pub detail_registry_name: String,
    pub detail_source: String,
    pub detail_description: String,
    pub detail_preview: String,
    pub detail_count: String,
    pub detail_empty: bool,
    pub carried: String,
    pub error: String,
    pub can_craft: bool,
    pub items: Vec<InventoryItemView>,
    pub hotbar: Vec<InventoryItemView>,
    pub crafting: Vec<InventoryItemView>,
    pub craft_result: InventoryItemView,
    pub craft_results: Vec<InventoryItemView>,
    pub categories: Vec<CategoryView>,
    pub category_dropdown: bool,
    pub equipment: Vec<EquipmentSlotView>,
    pub browser_items: Vec<InventoryItemView>,
    pub browser_page: String,
    pub browser_previous: bool,
    pub browser_next: bool,
}

fn item_category(item: ItemKind, palette: &BlockPalette) -> &str {
    palette
        .item_categories
        .get(&item)
        .or_else(|| {
            item.block_material()
                .and_then(|id| palette.item_categories.get(&ItemKind::Block(id)))
        })
        .map_or(
            if matches!(item, ItemKind::Mineral(_)) {
                "resources"
            } else {
                "terrain"
            },
            String::as_str,
        )
}

fn item_metadata(item: ItemKind, palette: &BlockPalette) -> (String, String, String) {
    match item {
        ItemKind::Tool(_)
        | ItemKind::Stick
        | ItemKind::Mineral(_)
        | ItemKind::Animal(_)
        | ItemKind::Bellows(_)
        | ItemKind::Dye(_) => {
            let (name, description) =
                palette
                    .item_metadata
                    .get(&item)
                    .cloned()
                    .unwrap_or_else(|| {
                        (
                            item.identifier()
                                .split("::")
                                .last()
                                .unwrap_or("Item")
                                .replace('_', " "),
                            String::new(),
                        )
                    });
            (name, item.identifier(), description)
        }
        ItemKind::DyedBed { material, color } => {
            let (name, _, description) = item_metadata(ItemKind::Block(material), palette);
            (
                format!("{name} ({})", color.slug()),
                item.identifier(),
                description,
            )
        }
        ItemKind::Block(id) | ItemKind::WoodenBlock { material: id, .. } => palette
            .choices
            .iter()
            .find(|choice| choice.id == id)
            .map_or_else(
                || {
                    (
                        format!("Block {id}"),
                        format!("teramic::block_{id}"),
                        String::new(),
                    )
                },
                |choice| {
                    let identifier = if choice.identifier.contains("::") {
                        choice.identifier.clone()
                    } else {
                        format!("teramic::{}", choice.identifier)
                    };
                    let name = item
                        .wood_species()
                        .filter(|wood| *wood != terra_core::WoodSpecies::Oak)
                        .map_or_else(
                            || choice.name.clone(),
                            |wood| format!("{} ({})", choice.name, wood.slug()),
                        );
                    (name, identifier, choice.description.clone())
                },
            ),
        ItemKind::Prop(kind) => (
            kind.label().into(),
            item.identifier(),
            palette
                .prop_descriptions
                .get(&kind)
                .cloned()
                .unwrap_or_default(),
        ),
    }
}

pub(super) fn item_view(item: ItemKind, count: u16, palette: &BlockPalette) -> InventoryItemView {
    let (name, identifier, mut description) = item_metadata(item, palette);
    if let ItemKind::Bellows(kind) = item {
        use terra_core::furnace::BellowsKind;
        if !description.is_empty() {
            description.push('\n');
        }
        description.push_str(match kind {
            BellowsKind::Stone => "Smelting speed: +10%",
            BellowsKind::Iron => "Smelting speed: +20%",
            BellowsKind::Gold => "Smelting speed: +10%\nDouble result chance: 5%",
            BellowsKind::Ruby => "Smelting speed: +15%\nDouble result chance: 7.5%",
        });
    }
    if let Some(food) = terra_core::food(item) {
        use std::fmt::Write;
        if !description.is_empty() {
            description.push('\n');
        }
        let _ = write!(
            description,
            "Food: +{} hunger, +{} saturation\nEating: {} s",
            food.nutrition, food.saturation, food.eat_seconds
        );
        if let Some(regeneration) = &food.regeneration {
            let _ = write!(
                description,
                "\nRegeneration: +{} health/s for {} s",
                regeneration.health_per_second, regeneration.duration_seconds
            );
        }
        if let Some(remainder) = food.remainder {
            let (name, _, _) = item_metadata(remainder, palette);
            let _ = write!(description, "\nReturns: {name}");
        }
    }
    InventoryItemView {
        id: item_id(item),
        item_id: item_id(item),
        name,
        identifier,
        description,
        category: item_category(item, palette).into(),
        count: count.to_string(),
        preview_key: preview_key(item),
        prop: matches!(item, ItemKind::Prop(_)),
        ..Default::default()
    }
}

fn slot_view(
    inventory: &terra_core::Inventory,
    index: u8,
    palette: &BlockPalette,
    command: &str,
    selected: bool,
) -> InventoryItemView {
    let mut view = inventory.slot(usize::from(index)).map_or_else(
        || InventoryItemView {
            empty: true,
            ..Default::default()
        },
        |stack| item_view(stack.item, stack.count, palette),
    );
    view.id = format!("{command}-{index}");
    view.index = index.to_string();
    view.command = command.into();
    view.selected = selected;
    view
}

#[allow(clippy::too_many_arguments)]
pub(super) fn update_view(
    state: Res<InventoryState>,
    palette: Res<BlockPalette>,
    active: Res<ActiveItem>,
    context: InventoryContext,
    crafting: Res<crafting::CraftingView>,
    reservations: Res<crafting::CraftingState>,
    pointer: Option<Res<pointer::InventoryPointer>>,
    queue: Option<Res<pointer::InventoryQueue>>,
    mut stamp: Local<Option<SessionViewStamp>>,
    mut view: ResMut<InventoryView>,
) {
    let session = context.session.as_ref();
    let menu = context.menu.as_ref();
    let settings = &context.settings;
    let next_stamp = session.map(|session| SessionViewStamp {
        ready: session.ready,
        mode: session.game_mode,
        revision: session.inventory.revision,
        outcome: session.last_inventory,
        container_anchor: session.container.as_ref().map(|container| container.anchor),
        container_workbench: session
            .container
            .as_ref()
            .and_then(|container| container.workbench),
        furnace: session
            .container
            .as_ref()
            .and_then(|container| container.furnace.clone()),
        container_revisions: std::array::from_fn(|index| {
            session
                .container
                .as_ref()?
                .halves
                .get(index)
                .map(|half| (half.position, half.inventory.revision))
        }),
    });
    let session_changed = *stamp != next_stamp;
    *stamp = next_stamp;
    if !state.is_changed()
        && !palette.is_changed()
        && !active.is_changed()
        && !settings.is_changed()
        && !session_changed
        && !crafting.is_changed()
        && !reservations.is_changed()
        && !pointer.as_ref().is_some_and(|pointer| pointer.is_changed())
        && !queue.as_ref().is_some_and(|queue| queue.is_changed())
        && !menu.as_ref().is_some_and(|menu| menu.is_changed())
    {
        return;
    }
    let Some(session) = session.filter(|session| session.ready) else {
        if *view != InventoryView::default() {
            *view = InventoryView::default();
        }
        return;
    };
    let creative = session.game_mode == GameMode::Creative;
    let projection = pointer
        .as_ref()
        .and_then(|pointer| pointer.preview(session, &reservations, queue.as_deref()));
    let displayed_inventory = projection
        .as_ref()
        .map_or(&session.inventory, |p| &p.player);
    let container = session.container.as_ref();
    let furnace = container.and_then(|container| container.furnace.as_ref());
    let container_items = container.map_or_else(Vec::new, |container| {
        container
            .halves
            .iter()
            .enumerate()
            .flat_map(|(half_index, half)| {
                let inventory = projection
                    .as_ref()
                    .and_then(|p| p.halves.get(half_index))
                    .unwrap_or(&half.inventory);
                let slots = if furnace.is_some() {
                    4
                } else {
                    terra_core::containers::CHEST_SLOTS as u8
                };
                (0..slots).map(move |index| (inventory, index))
            })
            .enumerate()
            .map(|(index, (inventory, local))| {
                let mut slot = slot_view(inventory, local, &palette, "container", false);
                slot.id = format!("container-{index}");
                slot.index = index.to_string();
                slot
            })
            .collect::<Vec<_>>()
    });
    let selected = active
        .0
        .or_else(|| palette.selected().map(|choice| ItemKind::Block(choice.id)));
    let category = palette
        .creative_categories
        .iter()
        .find(|category| category.id == state.category)
        .map_or("all", |category| category.id.as_str());
    let query = state.search.to_lowercase();
    let browser: Vec<_> = available_items(GameMode::Creative, &session.inventory, &palette)
        .into_iter()
        .filter(|item| catalog_item(*item))
        .map(|item| {
            let mut slot = item_view(item, STACK_LIMIT, &palette);
            slot.command = "catalog".into();
            slot.count.clear();
            slot.selected = Some(item) == selected;
            slot
        })
        .filter(|slot| {
            (category == "all" || slot.category == category)
                && (query.is_empty()
                    || slot.name.to_lowercase().contains(&query)
                    || slot.identifier.to_lowercase().contains(&query))
        })
        .collect();
    let page_size = if state.page_capacity == 0 {
        45
    } else {
        state.page_capacity
    };
    let pages = browser.len().div_ceil(page_size).max(1);
    let page = state.page.min(pages - 1);
    let browser_items: Vec<_> = browser
        .into_iter()
        .skip(page * page_size)
        .take(page_size)
        .collect();
    let items: Vec<_> = (9..terra_core::INVENTORY_SLOTS as u8)
        .map(|index| {
            let mut slot = slot_view(
                displayed_inventory,
                index,
                &palette,
                "slot",
                state.source == Some(index),
            );
            if let Some(stack) = displayed_inventory.slot(usize::from(index)) {
                let available =
                    crafting::available_count(&reservations, usize::from(index), stack.count);
                slot.count = available.to_string();
                if available == 0 {
                    slot.empty = true;
                    slot.item_id.clear();
                }
            }
            slot
        })
        .collect();
    let focused = state.focused.or(selected);
    let mut details = focused
        .map(|item| {
            item_view(
                item,
                if creative {
                    STACK_LIMIT
                } else {
                    session.inventory.count(item)
                },
                &palette,
            )
        })
        .unwrap_or_default();
    if creative {
        details.count.clear();
    }
    let hotbar = (0..9)
        .map(|index| {
            let mut slot = slot_view(
                displayed_inventory,
                index,
                &palette,
                "hotbar",
                state.hotbar == index,
            );
            slot.hotkey = (index + 1).to_string();
            if state.open
                && let Some(stack) = displayed_inventory.slot(index as usize)
            {
                let available =
                    crafting::available_count(&reservations, index as usize, stack.count);
                slot.count = available.to_string();
                if available == 0 {
                    slot.empty = true;
                    slot.item_id.clear();
                }
            }
            slot
        })
        .collect();
    let craft_result = crafting
        .output
        .first()
        .cloned()
        .unwrap_or_else(|| InventoryItemView {
            empty: true,
            ..default()
        });
    let can_craft = crafting.can_cursor;
    let next = InventoryView {
        layout_style: if state.open && !view.open {
            "opacity: 0; pointer-events: none;".into()
        } else {
            view.layout_style.clone()
        },
        open: state.open,
        assembly: state.assembly,
        container: container
            .is_some_and(|container| container.workbench.is_none() && container.furnace.is_none()),
        furnace: furnace.is_some(),
        furnace_input: furnace
            .and_then(|_| container_items.first().cloned())
            .into_iter()
            .collect(),
        furnace_fuel: furnace
            .and_then(|_| container_items.get(1).cloned())
            .into_iter()
            .collect(),
        furnace_output: furnace
            .and_then(|_| container_items.get(2).cloned())
            .into_iter()
            .collect(),
        furnace_bellows: furnace
            .and_then(|_| container_items.get(3).cloned())
            .into_iter()
            .collect(),
        furnace_progress_style: format!(
            "width: {:.1}%;",
            furnace.map_or(0.0, |f| f.progress.clamp(0.0, 1.0) * 100.0)
        ),
        furnace_fuel_style: format!(
            "height: {:.1}%;",
            furnace.map_or(0.0, |f| {
                if f.burn_total > 0.0 {
                    (f.burn_remaining / f.burn_total).clamp(0.0, 1.0) * 100.0
                } else {
                    0.0
                }
            })
        ),
        furnace_progress: furnace.map_or_else(String::new, |f| {
            format!("{:.0}%", f.progress.clamp(0.0, 1.0) * 100.0)
        }),
        furnace_status: furnace.map_or_else(String::new, |f| {
            if f.smoke_cells > 0 {
                format!("Smoke: {} blocks", f.smoke_cells)
            } else if f.burn_remaining > 0.0 && f.vented {
                "Chimney vented".into()
            } else if f.burn_remaining > 0.0 {
                "Heating".into()
            } else {
                "Idle".into()
            }
        }),
        double_container: container
            .is_some_and(|container| container.workbench.is_none() && container.halves.len() > 1),
        linked_container: container.is_some_and(|container| container.workbench.is_some()),
        linked_title: container
            .map_or("", |container| {
                if container.halves.len() > 1 {
                    "Double Chest"
                } else {
                    "Chest"
                }
            })
            .into(),
        container_capacity: container.map_or_else(String::new, |container| {
            let occupied = container
                .halves
                .iter()
                .map(|half| half.inventory.occupied_slots())
                .sum::<usize>();
            format!(
                "{occupied} / {} slots",
                container.halves.len() * terra_core::containers::CHEST_SLOTS
            )
        }),
        container_items,
        visible: !session.dead() && menu.as_ref().is_none_or(|menu| menu.world_hud_active()),
        creative,
        trash_available: creative
            && (session.inventory.cursor.is_some()
                || session
                    .inventory
                    .slot(usize::from(
                        state.source.or(state.active_slot).unwrap_or(state.hotbar),
                    ))
                    .is_some()),
        empty: browser_items.is_empty(),
        title: container
            .map_or("Inventory", |container| {
                if container.furnace.is_some() {
                    return "Furnace";
                }
                if container.workbench.is_some() {
                    return "Workbench";
                }
                if container.halves.len() > 1 {
                    "Double Chest"
                } else {
                    "Chest"
                }
            })
            .into(),
        mode: session.game_mode.label().into(),
        capacity: format!(
            "{} / {} slots",
            session.inventory.occupied_slots(),
            terra_core::INVENTORY_SLOTS
        ),
        search: state.search.clone(),
        selected_name: selected
            .map(|item| item_metadata(item, &palette).0)
            .unwrap_or_default(),
        selected_count: selected
            .map(|item| {
                if creative {
                    STACK_LIMIT.to_string()
                } else {
                    session.inventory.count(item).to_string()
                }
            })
            .unwrap_or_default(),
        selected_preview: selected.map(preview_key).unwrap_or_default(),
        selected_empty: selected.is_none()
            || (!creative && selected.is_none_or(|item| session.inventory.count(item) == 0)),
        detail_name: details.name,
        detail_kind: details.category,
        detail_registry_name: details
            .identifier
            .split_once("::")
            .map_or(details.identifier.as_str(), |(_, name)| name)
            .into(),
        detail_source: format!(
            "von {}",
            details
                .identifier
                .split_once("::")
                .map_or("teramic", |(source, _)| source)
        ),
        detail_identifier: details.identifier,
        detail_description: details.description,
        detail_preview: details.preview_key,
        detail_count: details.count,
        detail_empty: focused.is_none(),
        carried: state
            .open
            .then_some(session.inventory.cursor)
            .flatten()
            .map(|stack| item_metadata(stack.item, &palette).0)
            .unwrap_or_default(),
        error: session
            .last_inventory
            .map_or("", |result| result.message())
            .into(),
        can_craft,
        items,
        hotbar,
        crafting: crafting.grid.clone(),
        craft_results: vec![craft_result.clone()],
        craft_result,
        browser_items,
        browser_page: format!("{} / {}", page + 1, pages),
        browser_previous: page > 0,
        browser_next: page + 1 < pages,
        equipment: [
            ("helmet", "Helmet"),
            ("earring", "Earring"),
            ("chestplate", "Chestplate"),
            ("necklace", "Necklace"),
            ("trousers", "Trousers"),
            ("belt", "Belt"),
            ("boots", "Boots"),
            ("socks", "Socks"),
        ]
        .into_iter()
        .map(|(kind, name)| EquipmentSlotView { kind, name })
        .collect(),
        category_dropdown: state.category_dropdown,
        categories: std::iter::once(CategoryView {
            id: "all".into(),
            name: "All".into(),
            selected: category == "all",
            preview_key: preview_key(ItemKind::Block(3)),
        })
        .chain(
            palette
                .creative_categories
                .iter()
                .filter(|entry| entry.id != "all")
                .map(|entry| CategoryView {
                    id: entry.id.clone(),
                    name: entry.name.clone(),
                    selected: entry.id == category,
                    preview_key: preview_key(entry.icon),
                }),
        )
        .collect(),
    };
    if *view != next {
        *view = next;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use terra_network::protocol::inventory::{ContainerHalf, ContainerSnapshot};

    #[test]
    fn food_tooltips_show_the_shared_nutrition_and_timed_regeneration() {
        use terra_core::creatures::AnimalProduct;
        let palette = BlockPalette::default();
        for item in [
            ItemKind::Animal(AnimalProduct::Fish),
            ItemKind::Animal(AnimalProduct::CookedFish),
            ItemKind::Prop(terra_core::ResourcePropKind::Mushroom),
        ] {
            let food = terra_core::food(item).unwrap();
            let view = item_view(item, 2, &palette);
            assert!(
                view.description
                    .contains(&format!("+{} hunger", food.nutrition))
            );
            assert!(
                view.description
                    .contains(&format!("+{} saturation", food.saturation))
            );
            assert!(
                view.description
                    .contains(&format!("Eating: {} s", food.eat_seconds))
            );
            assert_eq!(
                view.description.contains("Regeneration:"),
                food.regeneration.is_some()
            );
        }
        assert!(
            !item_view(ItemKind::Block(3), 1, &palette)
                .description
                .contains("Food:")
        );
    }

    fn view_app() -> App {
        let mut app = App::new();
        app.init_resource::<InventoryState>()
            .init_resource::<BlockPalette>()
            .init_resource::<ActiveItem>()
            .init_resource::<crate::SettingsView>()
            .init_resource::<crafting::CraftingView>()
            .init_resource::<crafting::CraftingState>()
            .init_resource::<InventoryView>()
            .init_resource::<ClientSession>()
            .add_systems(Update, update_view);
        app.world_mut().resource_mut::<ClientSession>().ready = true;
        app.world_mut().resource_mut::<InventoryState>().open = true;
        app
    }

    #[test]
    fn chest_view_has_two_or_four_rows_and_preserves_each_physical_slot_index() {
        let mut app = view_app();
        let anchor = [-12, 60, 8];
        app.world_mut().resource_mut::<ClientSession>().container = Some(ContainerSnapshot {
            anchor,
            workbench: None,
            furnace: None,
            halves: vec![ContainerHalf {
                position: anchor,
                inventory: default(),
            }],
        });
        app.update();
        let view = app.world().resource::<InventoryView>();
        assert_eq!(view.title, "Chest");
        assert!(view.container);
        assert_eq!(view.container_items.len(), 18);
        assert_eq!(view.container_items[17].index, "17");
        assert_eq!(view.container_items[17].command, "container");
        app.world_mut()
            .resource_mut::<ClientSession>()
            .container
            .as_mut()
            .unwrap()
            .halves
            .push(ContainerHalf {
                position: [-11, 60, 8],
                inventory: default(),
            });
        app.update();
        let view = app.world().resource::<InventoryView>();
        assert_eq!(view.title, "Double Chest");
        assert_eq!(view.container_items.len(), 36);
        assert_eq!(view.container_items[18].id, "container-18");
        assert_eq!(view.container_items[35].index, "35");
    }

    #[test]
    fn linked_chest_is_a_separate_side_view_and_does_not_replace_workbench_crafting() {
        let mut app = view_app();
        let anchor = [-12, 60, 8];
        app.world_mut().resource_mut::<ClientSession>().container = Some(ContainerSnapshot {
            anchor,
            workbench: Some([-11, 60, 8]),
            furnace: None,
            halves: vec![ContainerHalf {
                position: anchor,
                inventory: default(),
            }],
        });
        app.update();
        let view = app.world().resource::<InventoryView>();
        assert!(view.linked_container);
        assert!(!view.container);
        assert_eq!(view.title, "Workbench");
        assert_eq!(view.container_items.len(), 18);
        app.world_mut()
            .resource_mut::<ClientSession>()
            .container
            .as_mut()
            .unwrap()
            .workbench = None;
        app.update();
        assert!(!app.world().resource::<InventoryView>().linked_container);
        assert!(app.world().resource::<InventoryView>().container);
    }

    #[test]
    fn remote_chest_revision_refreshes_view_without_player_inventory_revision_change() {
        let mut app = view_app();
        let anchor = [4, 70, -9];
        app.world_mut().resource_mut::<ClientSession>().container = Some(ContainerSnapshot {
            anchor,
            workbench: None,
            furnace: None,
            halves: vec![ContainerHalf {
                position: anchor,
                inventory: default(),
            }],
        });
        app.update();
        app.update();
        let player_revision = app.world().resource::<ClientSession>().inventory.revision;
        {
            let mut session = app.world_mut().resource_mut::<ClientSession>();
            let half = &mut session.container.as_mut().unwrap().halves[0];
            half.inventory.insert(ItemKind::Block(3), 7);
        }
        app.update();
        assert_eq!(
            app.world().resource::<ClientSession>().inventory.revision,
            player_revision
        );
        let view = app.world().resource::<InventoryView>();
        assert_eq!(view.container_items[0].count, "7");
        assert_eq!(view.container_items[0].item_id, "block-3");
        assert_eq!(view.container_capacity, "1 / 18 slots");
        app.world_mut().resource_mut::<ClientSession>().container = None;
        app.update();
        let view = app.world().resource::<InventoryView>();
        assert!(!view.container);
        assert!(view.container_items.is_empty());
    }

    #[test]
    fn furnace_slots_and_live_progress_use_the_authoritative_container_snapshot() {
        let mut app = view_app();
        let anchor = [4, 70, -9];
        app.world_mut().resource_mut::<ClientSession>().container = Some(ContainerSnapshot {
            anchor,
            workbench: None,
            furnace: Some(terra_core::furnace::FurnaceView {
                progress: 0.25,
                burn_remaining: 12.0,
                burn_total: 24.0,
                recipe: None,
                smoke_cells: 0,
                vented: true,
            }),
            halves: vec![ContainerHalf {
                position: anchor,
                inventory: default(),
            }],
        });
        app.update();
        let view = app.world().resource::<InventoryView>();
        assert_eq!(view.title, "Furnace");
        assert!(view.furnace && !view.container);
        assert_eq!(view.container_items.len(), 4);
        assert_eq!(view.furnace_input[0].index, "0");
        assert_eq!(view.furnace_fuel[0].index, "1");
        assert_eq!(view.furnace_output[0].index, "2");
        assert_eq!(view.furnace_bellows[0].index, "3");
        assert_eq!(view.furnace_progress_style, "width: 25.0%;");
        assert_eq!(view.furnace_fuel_style, "height: 50.0%;");
        assert_eq!(view.furnace_status, "Chimney vented");
        assert_eq!(view.hotbar.len(), 9);
        assert_eq!(view.items.len(), terra_core::INVENTORY_SLOTS - 9);
        let player_slots: Vec<_> = view
            .hotbar
            .iter()
            .chain(&view.items)
            .map(|slot| slot.index.parse::<usize>().unwrap())
            .collect();
        assert_eq!(
            player_slots,
            (0..terra_core::INVENTORY_SLOTS).collect::<Vec<_>>()
        );
        app.update();
        {
            let mut session = app.world_mut().resource_mut::<ClientSession>();
            let furnace = session
                .container
                .as_mut()
                .unwrap()
                .furnace
                .as_mut()
                .unwrap();
            furnace.progress = 0.75;
            furnace.smoke_cells = 3;
        }
        app.update();
        let view = app.world().resource::<InventoryView>();
        assert_eq!(view.furnace_progress_style, "width: 75.0%;");
        assert_eq!(view.furnace_status, "Smoke: 3 blocks");
        for (burn_remaining, vented, smoke_cells, expected) in [
            (12.0, false, 0, "Heating"),
            (12.0, false, 2, "Smoke: 2 blocks"),
            (0.0, false, 2, "Smoke: 2 blocks"),
            (0.0, false, 0, "Idle"),
            (12.0, true, 0, "Chimney vented"),
        ] {
            {
                let mut session = app.world_mut().resource_mut::<ClientSession>();
                let furnace = session
                    .container
                    .as_mut()
                    .unwrap()
                    .furnace
                    .as_mut()
                    .unwrap();
                furnace.burn_remaining = burn_remaining;
                furnace.vented = vented;
                furnace.smoke_cells = smoke_cells;
            }
            app.update();
            assert_eq!(
                app.world().resource::<InventoryView>().furnace_status,
                expected
            );
        }
        tilt_ui::tilt_ui_css::parse_stylesheet(include_str!(
            "../../../src-ui/components/inventory-panel.component.css"
        ))
        .unwrap();
        tilt_ui::tilt_ui_css::parse_stylesheet(include_str!(
            "../../../src-ui/components/recipe-inspector.component.css"
        ))
        .unwrap();
    }

    #[test]
    fn bellows_tooltips_include_speed_and_variant_bonus() {
        use terra_core::furnace::BellowsKind;
        let palette = BlockPalette::default();
        for (kind, speed, chance) in [
            (BellowsKind::Stone, "10%", None),
            (BellowsKind::Iron, "20%", None),
            (BellowsKind::Gold, "10%", Some("5%")),
            (BellowsKind::Ruby, "15%", Some("7.5%")),
        ] {
            let view = item_view(ItemKind::Bellows(kind), 1, &palette);
            assert!(
                view.description
                    .contains(&format!("Smelting speed: +{speed}"))
            );
            assert_eq!(view.description.contains("Double result"), chance.is_some());
            if let Some(chance) = chance {
                assert!(
                    view.description
                        .contains(&format!("Double result chance: {chance}"))
                );
            }
        }
    }

    #[test]
    fn equipment_ghosts_use_supported_css_and_declare_all_eight_positions() {
        tilt_ui::tilt_ui_css::parse_stylesheet(include_str!(
            "../../../src-ui/components/equipment-slot.component.css"
        ))
        .unwrap();
        let template = include_str!("../../../src-ui/components/inventory-panel.component.html");
        assert!(
            template.contains("@for (equipment of inventoryView.equipment; track equipment.kind)")
        );
        let mut app = view_app();
        app.update();
        let equipment = &app.world().resource::<InventoryView>().equipment;
        assert_eq!(equipment.len(), 8);
        for (slot, kind) in equipment.iter().zip([
            "helmet",
            "earring",
            "chestplate",
            "necklace",
            "trousers",
            "belt",
            "boots",
            "socks",
        ]) {
            assert_eq!(slot.kind, kind);
        }
    }
}
