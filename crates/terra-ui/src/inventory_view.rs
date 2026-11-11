use super::*;
use serde::Serialize;
use tilt_ui::html_shared;

#[derive(Clone, PartialEq)]
pub(super) struct SessionViewStamp {
    ready: bool,
    mode: GameMode,
    revision: u64,
    equipment: terra_core::EquipmentSlots,
    outcome: Option<terra_network::protocol::inventory::InventoryOutcome>,
    container_anchor: Option<[i64; 3]>,
    container_view: Option<ContainerView>,
    attached_campfire: bool,
    container_material: Option<u16>,
    container_workbench: Option<[i64; 3]>,
    furnace: Option<terra_core::furnace::FurnaceView>,
    crusher: Option<terra_core::crusher::CrusherView>,
    mixer: Option<terra_core::mixer::MixerView>,
    cauldron: Option<terra_core::brewing::CauldronView>,
    campfire: Option<terra_core::campfire::CampfireView>,
    filter: Option<terra_core::logistics::FilterConfig>,
    distributor: Option<terra_core::logistics::DistributorConfig>,
    tunnel_random: Option<bool>,
    mixer_drive: Option<terra_network::protocol::session::KineticVisual>,
    container_revisions: [Option<([i64; 3], u64)>; 2],
}

#[derive(Clone, Debug, Default, Serialize, PartialEq)]
pub struct InventoryItemView {
    #[serde(skip)]
    pub item: Option<ItemKind>,
    #[serde(skip)]
    pub ghost_item: Option<ItemKind>,
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
    pub ghost_preview: String,
    pub ghost_count: String,
    pub ghost_name: String,
    pub prop: bool,
    pub selected: bool,
    pub empty: bool,
    pub disabled: bool,
    pub missing: bool,
    pub hotkey: String,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct CategoryView {
    #[serde(skip)]
    pub item: Option<ItemKind>,
    pub id: String,
    pub name: String,
    pub selected: bool,
    pub preview_key: String,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct DistributorFilterView {
    pub index: u8,
    pub name: String,
    pub slots: Vec<InventoryItemView>,
    pub allowed: bool,
    pub mode: String,
    pub has_item: bool,
    pub selecting: bool,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct EquipmentSlotView {
    pub kind: &'static str,
    pub slot_name: &'static str,
    #[serde(flatten)]
    pub item: InventoryItemView,
}

#[html_shared]
#[derive(Resource, Default, Debug, Serialize, PartialEq)]
pub struct InventoryView {
    /// Native identity of the authoritative snapshot used for this projection.
    #[serde(skip)]
    pub source_container: Option<([i64; 3], ContainerView)>,
    pub player: bool,
    pub browser_visible: bool,
    pub layout_style: String,
    pub prewarming: bool,
    pub open: bool,
    pub assembly: bool,
    pub container: bool,
    pub double_container: bool,
    pub linked_container: bool,
    pub linked_title: String,
    pub container_capacity: String,
    pub container_items: Vec<InventoryItemView>,
    pub storage_sortable: bool,
    pub sort_ready: bool,
    pub sort_player_open: bool,
    pub sort_storage_open: bool,
    pub furnace: bool,
    pub cauldron: alchemy::CauldronWorkspace,
    pub campfire: alchemy::CampfireWorkspace,
    pub hearth_navigation: bool,
    pub hearth_show_fire: bool,
    pub hearth_navigation_enabled: bool,
    pub furnace_input: Vec<InventoryItemView>,
    pub furnace_fuel: Vec<InventoryItemView>,
    pub furnace_output: Vec<InventoryItemView>,
    pub furnace_bellows: Vec<InventoryItemView>,
    pub furnace_progress_style: String,
    pub furnace_fuel_style: String,
    pub furnace_progress: String,
    pub furnace_status: String,
    pub crusher: bool,
    pub crusher_input: Vec<InventoryItemView>,
    pub crusher_output: Vec<InventoryItemView>,
    pub crusher_progress_style: String,
    pub crusher_working: bool,
    pub mixer: bool,
    pub mixer_input: Vec<InventoryItemView>,
    pub mixer_output: Vec<InventoryItemView>,
    pub mixer_color: String,
    pub mixer_progress_style: String,
    pub mixer_status: String,
    pub mixer_recipe: String,
    pub mixer_working: bool,
    pub filtered_funnel: bool,
    pub funnel_filter: Vec<InventoryItemView>,
    pub distributor_filters: Vec<DistributorFilterView>,
    pub distributor: bool,
    pub tunnel_options: bool,
    pub tunnel_random: bool,
    pub filter_allowed: bool,
    pub filter_mode: String,
    pub filter_selecting: bool,
    pub filter_has_item: bool,
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
    pub browser_capacity: usize,
    pub browser_pages: usize,
    pub browser_page: String,
    pub browser_previous: bool,
    pub browser_next: bool,
}

fn item_category(item: ItemKind, palette: &BlockPalette) -> &str {
    palette
        .item_categories
        .get(&item)
        .or_else(|| palette.item_categories.get(&item.preview_item()))
        .or_else(|| {
            item.cape()
                .and_then(|cape| palette.item_categories.get(&ItemKind::Cape(cape.undyed())))
        })
        .or_else(|| {
            if matches!(item, ItemKind::Dye(_)) {
                return palette
                    .item_categories
                    .get(&ItemKind::Dye(terra_core::beds::DyeColor::White));
            }
            item.block_material()
                .and_then(|id| palette.item_categories.get(&ItemKind::Block(id)))
        })
        .map_or(
            if matches!(item, ItemKind::Dye(_) | ItemKind::Mineral(_)) {
                "materials"
            } else {
                "terrain"
            },
            String::as_str,
        )
}

fn color_label(color: terra_core::beds::DyeColor) -> String {
    match color {
        terra_core::beds::DyeColor::Rgb(rgb) => mixer::format_target(rgb),
        _ => color.slug().into(),
    }
}

pub(super) fn item_metadata(item: ItemKind, palette: &BlockPalette) -> (String, String, String) {
    if let ItemKind::HeraldicCape { cape, heraldry } = item {
        let (name, _, description) = item_metadata(ItemKind::Cape(cape), palette);
        return (
            name,
            item.identifier(),
            format!("{description}\nHeraldry: {}", heraldry.icon.label()),
        );
    }
    if let ItemKind::Potion(id) = item
        && let Some(recipe) = terra_core::brewing::recipe(id)
    {
        return (
            recipe.name.clone(),
            item.identifier(),
            alchemy::effects(recipe),
        );
    }
    if let ItemKind::Dye(terra_core::beds::DyeColor::Rgb(rgb)) = item {
        let hex = mixer::format_target(rgb);
        return (
            format!("Mixed Dye {hex}"),
            item.identifier(),
            format!("RGB: {}, {}, {}", rgb[0], rgb[1], rgb[2]),
        );
    }
    if let ItemKind::Cape(cape) = item
        && cape.color != terra_core::beds::DyeColor::White
    {
        let (name, _, description) = item_metadata(ItemKind::Cape(cape.undyed()), palette);
        return (
            format!("{name} ({})", color_label(cape.color)),
            item.identifier(),
            description,
        );
    }
    match item {
        ItemKind::FilledWateringCan { tool, units } => {
            let (name, _, description) = item_metadata(ItemKind::Tool(tool), palette);
            (
                format!("{name} ({units}/{})", tool.watering_capacity()),
                item.identifier(),
                description,
            )
        }
        ItemKind::Tool(_)
        | ItemKind::Stick
        | ItemKind::Mineral(_)
        | ItemKind::Animal(_)
        | ItemKind::Bellows(_)
        | ItemKind::Armor(_)
        | ItemKind::Accessory(_)
        | ItemKind::Cape(_)
        | ItemKind::HeraldicCape { .. }
        | ItemKind::Dust(_)
        | ItemKind::Crop(_)
        | ItemKind::Flower(_)
        | ItemKind::Dye(_)
        | ItemKind::GlassBottle
        | ItemKind::WaterBottle
        | ItemKind::Potion(_)
        | ItemKind::Apple
        | ItemKind::AppleCore => {
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
                format!("{name} ({})", color_label(color)),
                item.identifier(),
                description,
            )
        }
        ItemKind::TintedBlock { color, .. } => {
            let canonical = terra_core::tints::canonical_item(item)
                .expect("Tinted blocks have canonical previews");
            let (name, _, description) = item_metadata(canonical, palette);
            (
                format!("{name} ({})", mixer::format_target(color)),
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
                    let mut identifier = if choice.identifier.contains("::") {
                        choice.identifier.clone()
                    } else {
                        format!("teramic::{}", choice.identifier)
                    };
                    if let ItemKind::WoodenBlock { wood, .. } = item {
                        identifier.push('#');
                        identifier.push_str(wood.slug());
                    }
                    let name = item
                        .wood_species()
                        .filter(|wood| {
                            *wood
                                != if id == terra_core::plumbing::BASIN {
                                    terra_core::WoodSpecies::Spruce
                                } else {
                                    terra_core::WoodSpecies::Oak
                                }
                        })
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
    if let Some(tool) = item.tool_kind().filter(|tool| tool.watering_capacity() > 0) {
        if !description.is_empty() {
            description.push('\n');
        }
        description.push_str(&format!(
            "Wasser: {} / {}",
            item.watering_units(),
            tool.watering_capacity()
        ));
    }
    if let Some(speed) = terra_core::brewing::spoon_speed(item) {
        if !description.is_empty() {
            description.push('\n');
        }
        description.push_str(&format!("Brewing speed: {speed:.1}x | Reusable"));
    }
    if let Some(seconds) = terra_core::brewing::drink_seconds(item) {
        if !description.is_empty() {
            description.push('\n');
        }
        description.push_str(&format!("Drink: {seconds:.1} s | Returns a glass bottle"));
    }
    if let Some(equipment) = terra_core::equipment_definition(item) {
        use std::fmt::Write;
        if !description.is_empty() {
            description.push('\n');
        }
        let _ = write!(
            description,
            "{} | Armor: {:.1}",
            equipment.slot.name(),
            equipment.armor_points
        );
        if equipment.health_bonus > 0.0 {
            let _ = write!(
                description,
                "\nMaximum health: +{:.0}",
                equipment.health_bonus
            );
        }
        if equipment.stamina_bonus > 0.0 {
            let _ = write!(
                description,
                "\nMaximum stamina: +{:.0}",
                equipment.stamina_bonus
            );
        }
    }
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
        item: Some(item),
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

#[derive(Default)]
pub(super) struct CatalogViewCache {
    entries: Vec<(ItemKind, InventoryItemView, String, String)>,
    ready: bool,
}

impl CatalogViewCache {
    fn rebuild(&mut self, palette: &BlockPalette) {
        self.entries = available_items(
            GameMode::Creative,
            &terra_core::Inventory::default(),
            palette,
        )
        .into_iter()
        .filter(|&item| catalog_item(item))
        .map(|item| {
            let mut view = item_view(item, STACK_LIMIT, palette);
            view.command = "catalog".into();
            view.count.clear();
            let name = view.name.to_lowercase();
            let identifier = view.identifier.to_lowercase();
            (item, view, name, identifier)
        })
        .collect();
        self.ready = true;
    }

    fn page(
        &self,
        category: &str,
        query: &str,
        size: usize,
        page: usize,
        selected: Option<ItemKind>,
    ) -> (Vec<InventoryItemView>, usize, usize) {
        let matches = |entry: &&(ItemKind, InventoryItemView, String, String)| {
            (category == "all" || entry.1.category == category)
                && (query.is_empty() || entry.2.contains(query) || entry.3.contains(query))
        };
        let pages = self
            .entries
            .iter()
            .filter(matches)
            .count()
            .div_ceil(size)
            .max(1);
        let page = page.min(pages - 1);
        let items = self
            .entries
            .iter()
            .filter(matches)
            .skip(page * size)
            .take(size)
            .map(|(item, view, _, _)| {
                let mut view = view.clone();
                view.selected = Some(*item) == selected;
                view
            })
            .collect();
        (items, pages, page)
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn update_view(
    mut state: ResMut<InventoryState>,
    palette: Res<BlockPalette>,
    active: Res<ActiveItem>,
    context: InventoryContext,
    crafting: Res<crafting::CraftingView>,
    reservations: Res<crafting::CraftingState>,
    pointer: Option<Res<pointer::InventoryPointer>>,
    queue: Option<Res<pointer::InventoryQueue>>,
    mut stamp: Local<Option<SessionViewStamp>>,
    mut catalog: Local<CatalogViewCache>,
    mut view: ResMut<InventoryView>,
) {
    let session = context.session.as_ref();
    let menu = context.menu.as_ref();
    let settings = &context.settings;
    let next_stamp = session.map(|session| SessionViewStamp {
        ready: session.ready,
        mode: session.game_mode,
        revision: session.inventory.revision,
        equipment: session.stats.equipment,
        outcome: session.last_inventory,
        container_anchor: session.container.as_ref().map(|container| container.anchor),
        container_view: session.container.as_ref().map(|container| container.view),
        attached_campfire: session
            .container
            .as_ref()
            .is_some_and(|container| container.attached_campfire),
        container_material: session
            .container
            .as_ref()
            .and_then(|container| context.chunks.as_ref()?.block_at(container.anchor)),
        container_workbench: session
            .container
            .as_ref()
            .and_then(|container| container.workbench),
        furnace: session
            .container
            .as_ref()
            .and_then(|container| container.furnace.clone()),
        crusher: session
            .container
            .as_ref()
            .and_then(|container| container.crusher.clone()),
        mixer: session
            .container
            .as_ref()
            .and_then(|container| container.mixer.clone()),
        cauldron: session
            .container
            .as_ref()
            .and_then(|container| container.cauldron.clone()),
        campfire: session
            .container
            .as_ref()
            .and_then(|container| container.campfire.clone()),
        filter: session
            .container
            .as_ref()
            .and_then(|container| container.filter),
        distributor: session
            .container
            .as_ref()
            .and_then(|container| container.distributor),
        tunnel_random: session
            .container
            .as_ref()
            .and_then(|container| container.tunnel_random),
        mixer_drive: session
            .container
            .as_ref()
            .filter(|container| container.mixer.is_some())
            .and_then(|container| {
                session
                    .kinetics
                    .iter()
                    .find(|drive| drive.position == container.anchor)
                    .copied()
            }),
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
    let crusher = container.and_then(|container| container.crusher.as_ref());
    let mixer = container.and_then(|container| container.mixer.as_ref());
    let cauldron = container.and_then(|container| container.cauldron.as_ref());
    let campfire = container.and_then(|container| container.campfire.as_ref());
    let funnel_filter = container.and_then(|container| {
        container.filter.map(|filter| {
            queue
                .as_ref()
                .and_then(|queue| queue.funnel_filter(container.anchor))
                .unwrap_or(filter)
        })
    });
    let distributor = container.and_then(|container| container.distributor);
    let distributor_filters = distributor.map_or_else(Vec::new, |distributor| {
        distributor
            .filters
            .into_iter()
            .enumerate()
            .map(|(index, confirmed)| {
                let index = index as u8;
                let config = queue
                    .as_ref()
                    .and_then(|queue| {
                        container
                            .and_then(|container| queue.distributor_filter(container.anchor, index))
                    })
                    .unwrap_or(confirmed);
                let selecting = state.filter_selecting && state.filter_output == index;
                DistributorFilterView {
                    index,
                    name: ["Left", "Forward", "Right"][usize::from(index)].into(),
                    slots: vec![filter::output_slot(index, config, selecting, &palette)],
                    allowed: config.allowed,
                    mode: if config.allowed { "Allowed" } else { "Blocked" }.into(),
                    has_item: config.item.is_some(),
                    selecting,
                }
            })
            .collect()
    });
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
                let slots = if cauldron.is_some() {
                    terra_core::brewing::SLOTS as u8
                } else if campfire.is_some() {
                    terra_core::campfire::SLOTS as u8
                } else if furnace.is_some() {
                    4
                } else if crusher.is_some() {
                    2
                } else if mixer.is_some() {
                    terra_core::mixer::SLOTS as u8
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
    let mixer_recipe = mixer.zip(container).and_then(|(machine, container)| {
        let inventory = projection
            .as_ref()
            .and_then(|p| p.halves.first())
            .or_else(|| container.halves.first().map(|half| &half.inventory))?;
        let target = queue
            .as_ref()
            .and_then(|q| q.mixer_target(container.anchor))
            .unwrap_or(machine.target);
        Some(mixer::recipe_view(
            &container_items,
            inventory,
            target,
            &palette,
        ))
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
    if !catalog.ready || palette.is_changed() {
        catalog.rebuild(&palette);
    }
    let page_size = if state.page_capacity == 0 {
        45
    } else {
        state.page_capacity
    };
    let (browser_items, pages, page) =
        catalog.page(category, &query, page_size, state.page, selected);
    if state.page != page {
        state.page = page;
    }
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
        storage_sortable: container.is_some_and(|container| container.sortable()),
        sort_ready: state.open
            && session.inventory.cursor.is_none()
            && queue.as_ref().is_none_or(|queue| queue.is_empty())
            && !crafting::busy(&reservations)
            && state.container_open_response.is_none(),
        sort_player_open: state.sort_menu == Some(false),
        sort_storage_open: state.sort_menu == Some(true),
        source_container: container.map(|container| (container.anchor, container.view)),
        hearth_navigation: container.is_some_and(|container| {
            container.attached_campfire
                && (container.cauldron.is_some() || container.campfire.is_some())
        }),
        hearth_show_fire: cauldron.is_some(),
        hearth_navigation_enabled: session.inventory.cursor.is_none()
            && queue.as_ref().is_none_or(|queue| queue.is_empty())
            && state.container_open_response.is_none(),
        cauldron: alchemy::cauldron(
            container,
            projection
                .as_ref()
                .and_then(|p| p.halves.first())
                .or_else(|| container?.halves.first().map(|half| &half.inventory)),
            &container_items,
            queue.as_deref(),
            &palette,
        ),
        campfire: alchemy::campfire(
            container,
            projection
                .as_ref()
                .and_then(|p| p.halves.first())
                .or_else(|| container?.halves.first().map(|half| &half.inventory)),
            &container_items,
            queue.as_deref(),
        ),
        prewarming: view.prewarming || !view.visible,
        layout_style: if state.open && !view.open {
            "opacity: 0; pointer-events: none;".into()
        } else {
            view.layout_style.clone()
        },
        open: state.open,
        player: container.is_none() && !state.assembly,
        browser_visible: !state.browser_collapsed,
        assembly: state.assembly,
        container: container.is_some_and(|container| {
            container.workbench.is_none()
                && container.furnace.is_none()
                && container.crusher.is_none()
                && container.mixer.is_none()
                && container.filter.is_none()
                && container.distributor.is_none()
                && container.tunnel_random.is_none()
                && container.cauldron.is_none()
                && container.campfire.is_none()
        }),
        filtered_funnel: funnel_filter.is_some() || distributor.is_some(),
        distributor_filters,
        distributor: distributor.is_some(),
        tunnel_options: container.is_some_and(|container| container.tunnel_random.is_some()),
        tunnel_random: container.is_some_and(|container| {
            queue
                .as_ref()
                .and_then(|queue| queue.tunnel_random(container.anchor))
                .or(container.tunnel_random)
                .unwrap_or(false)
        }),
        funnel_filter: funnel_filter
            .map(|config| filter::slot(config, state.filter_selecting, &palette))
            .into_iter()
            .collect(),
        filter_allowed: funnel_filter.is_none_or(|filter| filter.allowed),
        filter_mode: if funnel_filter.is_none_or(|filter| filter.allowed) {
            "Allowed"
        } else {
            "Blocked"
        }
        .into(),
        filter_selecting: state.filter_selecting,
        filter_has_item: funnel_filter.is_some_and(|filter| filter.item.is_some()),
        mixer: mixer.is_some(),
        mixer_input: mixer_recipe
            .as_ref()
            .map_or_else(Vec::new, |recipe| recipe.inputs.clone()),
        mixer_recipe: mixer_recipe
            .as_ref()
            .map_or_else(String::new, |recipe| recipe.requirements.clone()),
        mixer_output: mixer
            .and_then(|_| container_items.get(terra_core::mixer::OUTPUT).cloned())
            .into_iter()
            .collect(),
        mixer_color: mixer.map_or_else(String::new, |machine| {
            mixer::format_target(
                container
                    .and_then(|c| queue.as_ref().and_then(|q| q.mixer_target(c.anchor)))
                    .unwrap_or(machine.target),
            )
        }),
        mixer_progress_style: format!(
            "width: {:.1}%;",
            mixer.map_or(0.0, |machine| machine.progress.clamp(0.0, 1.0) * 100.0)
        ),
        mixer_working: mixer.is_some_and(|machine| machine.working)
            && container.is_some_and(|container| {
                session.kinetics.iter().any(|drive| {
                    drive.position == container.anchor && !drive.overloaded && drive.rpm != 0.0
                })
            }),
        mixer_status: mixer.map_or_else(String::new, |machine| {
            mixer::status(
                machine,
                container.and_then(|c| {
                    session
                        .kinetics
                        .iter()
                        .find(|drive| drive.position == c.anchor)
                }),
            )
        }),
        crusher: crusher.is_some(),
        crusher_input: crusher
            .and_then(|_| container_items.first().cloned())
            .into_iter()
            .collect(),
        crusher_output: crusher
            .and_then(|_| container_items.get(1).cloned())
            .into_iter()
            .collect(),
        crusher_progress_style: format!(
            "width: {:.1}%;",
            crusher.map_or(0.0, |c| c.progress.clamp(0.0, 1.0) * 100.0)
        ),
        crusher_working: crusher.is_some_and(|c| c.working),
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
        visible: !session.dead()
            && session.game_mode != GameMode::Spectator
            && menu.as_ref().is_none_or(|menu| menu.world_hud_active()),
        creative,
        trash_available: creative
            && (session.inventory.cursor.is_some()
                || session.inventory.occupied_slots() > 0
                || session.stats.equipment.iter().next().is_some()),
        empty: browser_items.is_empty(),
        title: container
            .map_or("Inventory", |container| {
                if container.cauldron.is_some() {
                    return "Cauldron";
                }
                if container.campfire.is_some() {
                    return "Campfire";
                }
                if container.furnace.is_some() {
                    return "Furnace";
                }
                if container.crusher.is_some() {
                    return "Crusher";
                }
                if container.mixer.is_some() {
                    return "Mixer";
                }
                if container.distributor.is_some() {
                    return "Belt Distributor";
                }
                if container.filter.is_some() {
                    return if container.tunnel_random.is_some() {
                        "Tunnel (Filtered)"
                    } else {
                        "Funnel (Filtered)"
                    };
                }
                if container.tunnel_random.is_some() {
                    return "Tunnel";
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
                    item.max_stack().to_string()
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
        browser_capacity: page_size,
        browser_pages: pages,
        browser_page: format!("{} / {}", page + 1, pages),
        browser_previous: page > 0,
        browser_next: page + 1 < pages,
        equipment: terra_core::EquipmentSlot::ALL
            .into_iter()
            .map(|slot| EquipmentSlotView {
                kind: slot.slug(),
                slot_name: slot.name(),
                item: {
                    let mut item = session.stats.equipment.slot(slot.index()).map_or_else(
                        || InventoryItemView {
                            empty: true,
                            ..default()
                        },
                        |stack| item_view(stack.item, stack.count, &palette),
                    );
                    item.id = format!("equipment-{}", slot.index());
                    item.index = slot.index().to_string();
                    item.command = "equipment".into();
                    item.count.clear();
                    item
                },
            })
            .collect(),
        category_dropdown: state.category_dropdown,
        categories: std::iter::once(CategoryView {
            item: Some(ItemKind::Block(3)),
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
                    item: Some(entry.icon),
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
    fn heraldic_capes_keep_the_base_cape_category_and_their_own_preview_identity() {
        let mut palette = BlockPalette::default();
        let base = ItemKind::Cape(terra_core::CapeItem::ALL_WHITE[0]);
        palette
            .item_categories
            .insert(base, "example::equipment".into());
        let cape =
            base.with_default_heraldry(Some(terra_core::kingdom::KingdomHeraldry::default()));
        let view = item_view(cape, 1, &palette);
        assert_eq!(view.category, "example::equipment");
        assert_eq!(view.identifier, cape.identifier());
        assert_ne!(view.preview_key, preview_key(base));
        palette
            .item_categories
            .insert(cape, "example::royal_cloth".into());
        assert_eq!(
            item_view(cape, 1, &palette).category,
            "example::royal_cloth"
        );
    }

    #[test]
    fn named_and_rgb_dyes_share_materials_without_catalog_growth_or_overriding_resource_packs() {
        use terra_core::beds::DyeColor;
        let mut palette = BlockPalette::default();
        for color in DyeColor::ALL {
            palette
                .item_categories
                .insert(ItemKind::Dye(color), "materials".into());
        }
        let mut catalog = CatalogViewCache::default();
        catalog.rebuild(&palette);
        let (page, _, _) = catalog.page("materials", "", 128, 0, None);
        assert_eq!(
            page.iter()
                .filter(|view| matches!(view.item, Some(ItemKind::Dye(_))))
                .count(),
            DyeColor::ALL.len()
        );
        assert!(page.iter().all(|item| item.category == "materials"));
        for mineral in terra_core::MineralItem::ALL {
            let item = ItemKind::Mineral(mineral);
            assert_eq!(item_view(item, 1, &palette).category, "materials");
            assert!(page.iter().any(|view| view.item == Some(item)));
        }
        let rgb = ItemKind::Dye(DyeColor::Rgb([71, 93, 184]));
        let view = item_view(rgb, 3, &palette);
        assert_eq!(view.category, "materials");
        assert_eq!(view.identifier, rgb.identifier());
        assert!(view.name.contains("#475DB8"));
        assert!(view.description.contains("71, 93, 184"));
        assert!(!catalog.entries.iter().any(|entry| entry.0 == rgb));

        palette.item_categories.clear();
        palette
            .item_categories
            .insert(ItemKind::Dye(DyeColor::White), "example::pigments".into());
        assert_eq!(item_view(rgb, 1, &palette).category, "example::pigments");
        palette
            .item_categories
            .insert(rgb, "example::special".into());
        assert_eq!(item_view(rgb, 1, &palette).category, "example::special");
        let iron = ItemKind::Mineral(terra_core::MineralItem::IronIngot);
        palette
            .item_categories
            .insert(iron, "example::metals".into());
        assert_eq!(item_view(iron, 1, &palette).category, "example::metals");
    }

    #[test]
    fn creative_mechanic_variants_have_distinct_species_metadata_and_previews() {
        use terra_core::WoodSpecies::{Birch, Cherry, Spruce};
        let palette = BlockPalette {
            choices: [141, 144, 148, 149]
                .into_iter()
                .map(|id| terra_player::interaction::BlockChoice {
                    id,
                    name: format!("Mechanical part {id}"),
                    identifier: ItemKind::Block(id).identifier(),
                    description: "A wooden mechanical part".into(),
                })
                .collect(),
            ..Default::default()
        };
        let mut catalog = CatalogViewCache::default();
        catalog.rebuild(&palette);
        let mut identifiers = std::collections::BTreeSet::new();
        let mut previews = std::collections::BTreeSet::new();
        for material in [141, 144, 148, 149] {
            for wood in [Birch, Spruce, Cherry] {
                let item = ItemKind::for_wood(material, wood);
                let entries: Vec<_> = catalog
                    .entries
                    .iter()
                    .filter(|(candidate, ..)| *candidate == item)
                    .collect();
                assert_eq!(entries.len(), 1);
                let view = &entries[0].1;
                assert_eq!(view.identifier, item.identifier());
                assert!(view.name.contains(wood.slug()));
                assert_eq!(view.item_id, item_id(item));
                assert_eq!(view.preview_key, preview_key(item));
                assert!(identifiers.insert(view.identifier.clone()));
                assert!(previews.insert(view.preview_key.clone()));
                let (page, _, _) =
                    catalog.page("all", &format!("#{}", wood.slug()), 128, 0, Some(item));
                assert!(
                    page.iter()
                        .any(|entry| entry.item_id == item_id(item) && entry.selected)
                );
            }
        }
    }

    #[test]
    fn catalog_metadata_is_reused_and_only_the_visible_filtered_page_is_cloned() {
        let mut palette = BlockPalette {
            choices: (1..=300)
                .map(|id| terra_player::interaction::BlockChoice {
                    id,
                    name: format!("Material {id}"),
                    identifier: format!("teramic::material_{id}"),
                    description: "A detailed description".repeat(20),
                })
                .collect(),
            ..Default::default()
        };
        let mut cache = CatalogViewCache::default();
        cache.rebuild(&palette);
        let pointer = cache.entries.as_ptr();
        for selected in [None, Some(ItemKind::Block(3)), Some(ItemKind::Block(18))] {
            let (page, pages, index) = cache.page("all", "material", 17, 1, selected);
            assert_eq!(page.len(), 17);
            assert_eq!(index, 1);
            assert!(pages > 10);
            assert_eq!(page[0].identifier, "teramic::material_18");
            assert_eq!(page[0].selected, selected == Some(ItemKind::Block(18)));
            assert_eq!(cache.entries.as_ptr(), pointer);
            assert!(cache.entries.iter().all(|(_, view, _, _)| !view.selected));
        }
        let (page, pages, index) = cache.page("all", "does-not-exist", 17, 500, None);
        assert!(page.is_empty());
        assert_eq!((pages, index), (1, 0));
        palette.choices[17].name = "Renamed Block".into();
        cache.rebuild(&palette);
        let (page, _, _) = cache.page("all", "renamed", 17, 0, None);
        assert_eq!(page.len(), 1);
        assert_eq!(page[0].name, item_metadata(ItemKind::Block(18), &palette).0);
        assert_eq!(page[0].name, "Renamed Block (spruce)");
    }

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
            view: ContainerView::Primary,
            attached_campfire: false,
            cauldron: None,
            campfire: None,
            tunnel_random: None,
            furnace: None,
            crusher: None,
            mixer: None,
            filter: None,
            distributor: None,
            halves: vec![ContainerHalf {
                position: anchor,
                inventory: default(),
            }],
        });
        app.update();
        let view = app.world().resource::<InventoryView>();
        assert_eq!(view.title, "Chest");
        assert_eq!(
            view.source_container,
            Some((anchor, ContainerView::Primary))
        );
        assert!(
            tilt_ui::serde_json::to_value(view)
                .unwrap()
                .get("source_container")
                .is_none()
        );
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
            view: ContainerView::Primary,
            attached_campfire: false,
            cauldron: None,
            campfire: None,
            tunnel_random: None,
            furnace: None,
            crusher: None,
            mixer: None,
            filter: None,
            distributor: None,
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
            view: ContainerView::Primary,
            attached_campfire: false,
            cauldron: None,
            campfire: None,
            tunnel_random: None,
            furnace: None,
            crusher: None,
            mixer: None,
            filter: None,
            distributor: None,
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
            view: ContainerView::Primary,
            attached_campfire: false,
            cauldron: None,
            campfire: None,
            tunnel_random: None,
            furnace: Some(terra_core::furnace::FurnaceView {
                progress: 0.25,
                burn_remaining: 12.0,
                burn_total: 24.0,
                recipe: None,
                smoke_cells: 0,
                vented: true,
            }),
            crusher: None,
            mixer: None,
            filter: None,
            distributor: None,
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
    fn equipment_cells_use_supported_css_and_declare_all_positions() {
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
        assert_eq!(equipment.len(), terra_core::EquipmentSlot::ALL.len());
        for (view, slot) in equipment.iter().zip(terra_core::EquipmentSlot::ALL) {
            assert_eq!(view.kind, slot.slug());
            assert_eq!(view.item.command, "equipment");
            assert_eq!(view.item.index, slot.index().to_string());
            assert!(view.item.empty);
        }
    }
}
