//! Inventory icons follow the displayed UI, never the complete world/item registry.
use bevy::prelude::*;
use std::collections::HashSet;
use terra_core::ItemKind;
use terra_network::client::ClientSession;
use terra_render::RenderItemPreviewRequests;
use terra_ui::{CraftingView, InventoryView, TargetView};

#[derive(Default)]
struct Demand {
    items: Vec<ItemKind>,
    seen: HashSet<ItemKind>,
    dynamic: usize,
    fixed: usize,
}
impl Demand {
    fn append(&mut self, item: Option<ItemKind>) {
        let Some(mut item) = item.filter(|item| item.valid()) else {
            return;
        };
        if matches!(item, ItemKind::Dye(terra_core::beds::DyeColor::Rgb(_))) {
            item = ItemKind::Dye(terra_core::beds::DyeColor::White);
        }
        let dynamic = terra_core::tints::canonical_item(item).is_some()
            || matches!(item,ItemKind::Cape(cape)if matches!(cape.color,terra_core::beds::DyeColor::Rgb(_)));
        let count = if dynamic {
            &mut self.dynamic
        } else {
            &mut self.fixed
        };
        if *count < if dynamic { 128 } else { 1024 } && self.seen.insert(item) {
            *count += 1;
            self.items.push(item);
        }
    }
}

fn visible_items(
    inventory: Option<&InventoryView>,
    crafting: Option<&CraftingView>,
    target: Option<&TargetView>,
    cursor: Option<ItemKind>,
    active: Option<ItemKind>,
) -> Vec<ItemKind> {
    let mut demand = Demand::default();
    if let Some(view) = inventory.filter(|view| view.visible) {
        for slot in &view.hotbar {
            demand.append(slot.item);
        }
        demand.append(active);
        if view.open {
            for slot in view
                .items
                .iter()
                .chain(&view.container_items)
                .chain(&view.funnel_filter)
                .chain(
                    view.distributor_filters
                        .iter()
                        .flat_map(|output| &output.slots),
                )
                .chain(&view.mixer_input)
                .chain(&view.cauldron.inputs)
                .chain(&view.browser_items)
            {
                demand.append(slot.item);
                demand.append(slot.ghost_item);
            }
            for slot in &view.equipment {
                demand.append(slot.item.item);
            }
            for category in &view.categories {
                demand.append(category.item);
            }
            demand.append(cursor);
            if view.assembly
                || !view.container
                    && !view.furnace
                    && !view.crusher
                    && !view.mixer
                    && !view.filtered_funnel
                    && !view.cauldron.active
                    && !view.campfire.active
            {
                for slot in view.crafting.iter().chain(&view.craft_results) {
                    demand.append(slot.item);
                }
                if let Some(crafting) = crafting {
                    for slot in crafting
                        .grid
                        .iter()
                        .chain(&crafting.stock)
                        .chain(&crafting.output)
                    {
                        demand.append(slot.item);
                    }
                }
            }
            if let Some(crafting) = crafting.filter(|crafting| crafting.recipe_open) {
                for category in &crafting.recipe_categories {
                    demand.append(category.item);
                }
                for slot in crafting
                    .recipe_grid
                    .iter()
                    .chain(&crafting.recipe_output)
                    .chain(&crafting.cauldron_model)
                    .chain(&crafting.cauldron_inputs)
                    .chain(&crafting.cauldron_tools)
                    .chain(crafting.wood_options.iter().flat_map(|wood| &wood.slots))
                    .chain(&crafting.furnace_model)
                    .chain(&crafting.furnace_fuel)
                    .chain(
                        crafting
                            .furnace_fuel_options
                            .iter()
                            .flat_map(|fuel| &fuel.slots),
                    )
                    .chain(crafting.recipe_alternatives.iter().flat_map(|alternative| {
                        alternative
                            .grid
                            .iter()
                            .chain(&alternative.input)
                            .chain(&alternative.output)
                    }))
                    .chain(
                        crafting
                            .ingredients
                            .iter()
                            .flat_map(|ingredient| &ingredient.slots),
                    )
                {
                    demand.append(slot.item);
                }
            }
        }
    }
    if let Some(target) = target.filter(|target| target.visible) {
        demand.append(target.tool_item);
    }
    demand.items
}

#[allow(clippy::too_many_arguments)] // Independent ECS resources retain change detection.
pub(super) fn sync(
    session: Option<Res<ClientSession>>,
    inventory: Option<Res<InventoryView>>,
    crafting: Option<Res<CraftingView>>,
    target: Option<Res<TargetView>>,
    active: Option<Res<terra_player::interaction::ActiveItem>>,
    requests: Option<ResMut<RenderItemPreviewRequests>>,
    mut previous: Local<Vec<ItemKind>>,
    mut session_stamp: Local<Option<(bool, Option<ItemKind>)>>,
) {
    let Some(mut requests) = requests else {
        return;
    };
    let ready = session.as_ref().is_some_and(|session| session.ready);
    let cursor = session
        .as_ref()
        .and_then(|session| session.inventory.cursor.map(|stack| stack.item));
    let stamp = Some((ready, cursor));
    if *session_stamp == stamp
        && !inventory.as_ref().is_some_and(|view| view.is_changed())
        && !crafting.as_ref().is_some_and(|view| view.is_changed())
        && !target.as_ref().is_some_and(|view| view.is_changed())
        && !active.as_ref().is_some_and(|view| view.is_changed())
    {
        return;
    }
    *session_stamp = stamp;
    let items = if ready {
        visible_items(
            inventory.as_deref(),
            crafting.as_deref(),
            target.as_deref(),
            cursor,
            active.and_then(|active| active.0),
        )
    } else {
        Vec::new()
    };
    if *previous != items {
        requests.replace(items.iter().copied());
        *previous = items;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn page(items: &[ItemKind]) -> InventoryView {
        let mut view = InventoryView {
            visible: true,
            open: true,
            ..default()
        };
        for &item in items {
            view.browser_items.push(Default::default());
            view.browser_items.last_mut().unwrap().item = Some(item);
        }
        view
    }
    #[test]
    fn normal_startup_does_not_request_the_registry_or_hidden_catalog() {
        assert!(visible_items(None, None, None, None, None).is_empty());
        let mut view = page(&ItemKind::EXTRA);
        view.open = false;
        view.hotbar.push(Default::default());
        view.hotbar[0].item = Some(ItemKind::Block(3));
        assert_eq!(
            visible_items(Some(&view), None, None, None, None),
            [ItemKind::Block(3)]
        );
    }
    #[test]
    fn every_fixed_catalog_type_is_requested_only_on_its_visible_page() {
        let all: Vec<_> = ItemKind::EXTRA
            .into_iter()
            .chain([
                ItemKind::Block(3),
                ItemKind::Prop(terra_core::ResourcePropKind::Flower),
            ])
            .collect();
        let mut seen = HashSet::new();
        for items in all.chunks(17) {
            let view = page(items);
            let actual = visible_items(Some(&view), None, None, None, None);
            assert_eq!(actual, items);
            for item in actual {
                assert!(seen.insert(item));
            }
        }
        assert_eq!(seen.len(), all.len());
    }
    #[test]
    fn visible_ghosts_and_recipe_items_are_not_lost_or_replaced_by_the_whole_catalog() {
        let mut view = page(&[ItemKind::Block(3)]);
        view.cauldron.inputs.push(Default::default());
        view.cauldron.inputs[0].ghost_item = Some(ItemKind::WaterBottle);
        let mut crafting = CraftingView {
            recipe_open: true,
            ..default()
        };
        crafting.recipe_output.push(Default::default());
        crafting.recipe_output[0].item = Some(ItemKind::Potion(6));
        assert_eq!(
            visible_items(Some(&view), Some(&crafting), None, None, None),
            [
                ItemKind::WaterBottle,
                ItemKind::Block(3),
                ItemKind::Potion(6)
            ]
        );
        crafting.recipe_open = false;
        assert_eq!(
            visible_items(Some(&view), Some(&crafting), None, None, None),
            [ItemKind::WaterBottle, ItemKind::Block(3)]
        );
    }
    #[test]
    fn recipe_station_tabs_request_their_icons_only_while_the_recipe_is_open() {
        let view = page(&[]);
        let mut crafting = CraftingView {
            recipe_open: true,
            ..default()
        };
        for material in [161, 28, 161] {
            crafting.recipe_categories.push(Default::default());
            crafting.recipe_categories.last_mut().unwrap().item = Some(ItemKind::Block(material));
        }
        crafting.recipe_alternatives.push(Default::default());
        let alternative = crafting.recipe_alternatives.last_mut().unwrap();
        alternative.grid.push(Default::default());
        alternative.grid[0].item = Some(ItemKind::Block(163));
        alternative.output.push(Default::default());
        alternative.output[0].item = Some(ItemKind::WaterBottle);
        assert_eq!(
            visible_items(Some(&view), Some(&crafting), None, None, None),
            [
                ItemKind::Block(161),
                ItemKind::Block(28),
                ItemKind::Block(163),
                ItemKind::WaterBottle
            ]
        );
        crafting.recipe_open = false;
        assert!(visible_items(Some(&view), Some(&crafting), None, None, None).is_empty());
    }

    #[test]
    fn dynamic_requests_stay_bounded_without_starving_fixed_items_or_rgb_dyes() {
        let mut demand = Demand::default();
        for value in 0..1000_u16 {
            let item = ItemKind::TintedBlock {
                material: 148,
                wood: terra_core::WoodSpecies::Cherry,
                color: [value as u8, (value >> 8) as u8, 12],
            };
            demand.append(Some(item));
            demand.append(Some(item));
        }
        demand.append(Some(ItemKind::Block(3)));
        demand.append(Some(ItemKind::Dye(terra_core::beds::DyeColor::Rgb([
            1, 2, 3,
        ]))));
        assert_eq!(demand.dynamic, 128);
        assert_eq!(demand.fixed, 2);
        assert!(
            demand
                .items
                .contains(&ItemKind::Dye(terra_core::beds::DyeColor::White))
        );
    }

    #[test]
    fn target_icons_remain_requested_with_the_inventory_closed() {
        let mut view = page(&[]);
        let target = TargetView {
            visible: true,
            tool_item: Some(ItemKind::Tool(terra_core::ToolKind::IronPickaxe)),
            ..default()
        };
        assert_eq!(
            visible_items(Some(&view), None, Some(&target), None, None),
            [ItemKind::Tool(terra_core::ToolKind::IronPickaxe)]
        );
        view.open = false;
        assert_eq!(
            visible_items(Some(&view), None, Some(&target), None, None),
            [ItemKind::Tool(terra_core::ToolKind::IronPickaxe),]
        );
        assert!(
            visible_items(Some(&view), None, Some(&TargetView::default()), None, None).is_empty()
        );
    }
}
