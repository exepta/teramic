use super::*;
use serde::Serialize;
use terra_network::protocol::inventory::ContainerSnapshot;
use view::InventoryItemView;

#[derive(Clone, Default, Serialize, PartialEq, Debug)]
pub struct BrewChoice {
    pub value: String,
    pub name: String,
    pub selected: bool,
}

#[derive(Clone, Default, Serialize, PartialEq, Debug)]
pub struct CauldronWorkspace {
    pub active: bool,
    pub inputs: Vec<InventoryItemView>,
    pub water: Vec<InventoryItemView>,
    pub bottles: Vec<InventoryItemView>,
    pub spoon: Vec<InventoryItemView>,
    pub output: Vec<InventoryItemView>,
    pub recipes: Vec<BrewChoice>,
    pub recipe: String,
    pub progress: String,
    pub water_fill: String,
    pub water_amount: String,
    pub heated: bool,
    pub working: bool,
    pub status: String,
    pub effects: String,
}

#[derive(Clone, Default, Serialize, PartialEq, Debug)]
pub struct CampfireWorkspace {
    pub active: bool,
    pub input: Vec<InventoryItemView>,
    pub fuel: Vec<InventoryItemView>,
    pub output: Vec<InventoryItemView>,
    pub progress: String,
    pub fuel_fill: String,
    pub lit: bool,
    pub can_toggle: bool,
    pub toggle_title: String,
    pub status: String,
}

pub(super) fn effects(recipe: &terra_core::brewing::BrewingRecipe) -> String {
    recipe
        .effects
        .iter()
        .map(|effect| {
            format!(
                "{} {}: {} s",
                terra_core::effect_definition(effect.kind).name,
                effect.level,
                effect.seconds
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

pub(super) fn select_recipe(
    selection: &mut InventorySelection,
    session: &ClientSession,
    recipe: u16,
) {
    let Some(container) = session.container.as_ref() else {
        return;
    };
    let Some(machine) = container.cauldron.as_ref() else {
        return;
    };
    if terra_core::brewing::recipe(recipe).is_some()
        && selection
            .queue
            .brewing_recipe(container.anchor)
            .or(machine.recipe)
            != Some(recipe)
    {
        selection.request(
            session,
            ServerAction::SetBrewingRecipe {
                station: container.anchor,
                recipe,
            },
        );
    }
}

pub(super) fn set_lit(selection: &mut InventorySelection, session: &ClientSession, lit: bool) {
    let Some(container) = session.container.as_ref() else {
        return;
    };
    let Some(machine) = container.campfire.as_ref() else {
        return;
    };
    if selection
        .queue
        .campfire_lit(container.anchor)
        .unwrap_or(machine.lit)
        != lit
    {
        selection.request(
            session,
            ServerAction::SetCampfireLit {
                station: container.anchor,
                lit,
            },
        );
    }
}

fn slots(actual: &[InventoryItemView], index: usize) -> Vec<InventoryItemView> {
    actual.get(index).cloned().into_iter().collect()
}

fn input_ghosts(
    actual: &[InventoryItemView],
    inventory: &terra_core::Inventory,
    recipe: Option<&terra_core::brewing::BrewingRecipe>,
    palette: &BlockPalette,
) -> Vec<InventoryItemView> {
    let mut missing = recipe.into_iter().flat_map(|recipe| {
        recipe.inputs.iter().filter_map(|required| {
            let count = inventory
                .slots
                .iter()
                .take(terra_core::brewing::INPUT_SLOTS)
                .filter(|stack| stack.item == required.item)
                .map(|stack| u32::from(stack.count))
                .sum::<u32>();
            let count = u32::from(required.count).saturating_sub(count);
            (count > 0).then_some(terra_core::ItemStack {
                count: count as u16,
                ..*required
            })
        })
    });
    (0..terra_core::brewing::INPUT_SLOTS)
        .map(|index| {
            let mut slot = actual
                .get(index)
                .cloned()
                .unwrap_or_else(|| InventoryItemView {
                    id: format!("container-{index}"),
                    index: index.to_string(),
                    command: "container".into(),
                    empty: true,
                    ..default()
                });
            if slot.empty
                && let Some(required) = missing.next()
            {
                let ghost = view::item_view(required.item, required.count, palette);
                slot.ghost_item = Some(required.item);
                slot.ghost_preview = ghost.preview_key;
                slot.ghost_count = ghost.count;
                slot.ghost_name = format!("{} x{}", ghost.name, required.count);
            }
            slot
        })
        .collect()
}

pub(super) fn cauldron(
    container: Option<&ContainerSnapshot>,
    inventory: Option<&terra_core::Inventory>,
    actual: &[InventoryItemView],
    queue: Option<&pointer::InventoryQueue>,
    palette: &BlockPalette,
) -> CauldronWorkspace {
    let Some((container, machine, inventory)) =
        container.and_then(|container| Some((container, container.cauldron.as_ref()?, inventory?)))
    else {
        return default();
    };
    let id = queue
        .and_then(|queue| queue.brewing_recipe(container.anchor))
        .or(machine.recipe);
    let recipe = id.and_then(terra_core::brewing::recipe);
    let has_inputs = recipe.is_some_and(|recipe| {
        recipe.inputs.iter().all(|required| {
            inventory
                .slots
                .iter()
                .take(terra_core::brewing::INPUT_SLOTS)
                .filter(|stack| stack.item == required.item)
                .map(|stack| u32::from(stack.count))
                .sum::<u32>()
                >= u32::from(required.count)
        })
    });
    let status = if recipe.is_none() {
        "No recipe"
    } else if !machine.heated {
        "Cold"
    } else if machine.water == 0 {
        "No water"
    } else if !has_inputs {
        "Missing ingredients"
    } else if inventory.slot(terra_core::brewing::BOTTLES).is_none() {
        "No bottles"
    } else if inventory
        .slot(terra_core::brewing::SPOON)
        .and_then(|stack| terra_core::brewing::spoon_speed(stack.item))
        .is_none()
    {
        "No spoon"
    } else if machine.working {
        "Brewing"
    } else {
        "Output blocked"
    };
    CauldronWorkspace {
        active: true,
        inputs: input_ghosts(actual, inventory, recipe, palette),
        water: slots(actual, terra_core::brewing::WATER),
        bottles: slots(actual, terra_core::brewing::BOTTLES),
        spoon: slots(actual, terra_core::brewing::SPOON),
        output: slots(actual, terra_core::brewing::OUTPUT),
        recipes: terra_core::brewing::recipes()
            .iter()
            .map(|recipe| BrewChoice {
                value: recipe.id.to_string(),
                name: recipe.name.clone(),
                selected: id == Some(recipe.id),
            })
            .collect(),
        recipe: id.map_or_else(String::new, |id| id.to_string()),
        progress: format!("width: {:.1}%;", machine.progress.clamp(0.0, 1.0) * 100.0),
        water_fill: format!(
            "height: {:.1}%;",
            f32::from(machine.water.min(terra_core::brewing::MAX_WATER))
                / f32::from(terra_core::brewing::MAX_WATER)
                * 100.0
        ),
        water_amount: format!("{} / {}", machine.water, terra_core::brewing::MAX_WATER),
        heated: machine.heated,
        working: machine.working,
        status: status.into(),
        effects: recipe.map_or_else(String::new, effects),
    }
}

pub(super) fn campfire(
    container: Option<&ContainerSnapshot>,
    inventory: Option<&terra_core::Inventory>,
    actual: &[InventoryItemView],
    queue: Option<&pointer::InventoryQueue>,
) -> CampfireWorkspace {
    let Some((container, machine, inventory)) =
        container.and_then(|container| Some((container, container.campfire.as_ref()?, inventory?)))
    else {
        return default();
    };
    let lit = queue
        .and_then(|queue| queue.campfire_lit(container.anchor))
        .unwrap_or(machine.lit);
    let fueled = machine.burn_remaining > 0.0
        || inventory
            .slot(terra_core::campfire::FUEL)
            .and_then(|stack| terra_core::furnace::fuel_seconds(stack.item))
            .is_some();
    CampfireWorkspace {
        active: true,
        input: slots(actual, terra_core::campfire::INPUT),
        fuel: slots(actual, terra_core::campfire::FUEL),
        output: slots(actual, terra_core::campfire::OUTPUT),
        progress: format!("width: {:.1}%;", machine.progress.clamp(0.0, 1.0) * 100.0),
        fuel_fill: format!(
            "height: {:.1}%;",
            if machine.burn_total > 0.0 {
                (machine.burn_remaining / machine.burn_total).clamp(0.0, 1.0) * 100.0
            } else {
                0.0
            }
        ),
        lit,
        can_toggle: lit || fueled,
        toggle_title: if lit { "Extinguish" } else { "Light campfire" }.into(),
        status: if !lit {
            "Extinguished"
        } else if !fueled {
            "No fuel"
        } else if machine.recipe.is_some() {
            "Roasting"
        } else {
            "Burning"
        }
        .into(),
    }
}
