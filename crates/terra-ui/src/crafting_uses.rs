//! Reverse ingredient lookup in the same registries used by station simulation.
use super::*;
use terra_core::{ItemStack, ToolKind, WoodSpecies, beds::DyeColor};

#[derive(Clone, Copy)]
pub(super) enum Usage<'a> {
    Registered(RecipeChoice<'a>, ItemStack, Option<usize>),
    Generated {
        output: ItemStack,
        station: &'static str,
    },
}
impl Usage<'_> {
    pub(super) fn output(self) -> ItemStack {
        match self {
            Self::Registered(_, output, _) => output,
            Self::Generated { output, .. } => output,
        }
    }
    pub(super) fn station(self) -> &'static str {
        match self {
            Self::Registered(r, _, _) => category_metadata(r.category()).0,
            Self::Generated { station, .. } => station,
        }
    }
    pub(super) fn select(
        self,
        state: &mut CraftingState,
        inventory: &InventoryState,
        session: &ClientSession,
    ) {
        state.usage_input = None;
        state.recipe_item = Some(self.output().item);
        match self {
            Self::Registered(r, _, wood) => {
                if let Some(wood) = wood {
                    state.preview_wood = wood;
                }
                select_choice(state, r, inventory, session);
            }
            Self::Generated { .. } => {
                state.selected = None;
                state.selected_kind = RecipeKind::Craft;
                state.recipe_open = true;
                state.error.clear();
            }
        }
    }
}

pub(super) fn entries<'a>(
    book: &'a RecipeBook,
    item: ItemKind,
    session: &ClientSession,
) -> Vec<Usage<'a>> {
    let fuel = terra_core::furnace::fuel_seconds(item).is_some();
    let cauldron_supply = matches!(
        item,
        ItemKind::WaterBottle | ItemKind::GlassBottle | ItemKind::Tool(ToolKind::WaterBucket)
    ) || terra_core::brewing::spoon_speed(item).is_some();
    let mut choices: Vec<_> = book
        .iter()
        .filter(|r| {
            r.ingredients()
                .iter()
                .any(|ingredient| ingredient.matches(item))
        })
        .map(RecipeChoice::Craft)
        .chain(
            terra_core::furnace::recipes()
                .iter()
                .filter(|r| r.input == item || fuel)
                .map(RecipeChoice::Smelt),
        )
        .chain(
            terra_core::campfire::recipes()
                .iter()
                .filter(|r| r.input == item || fuel)
                .map(RecipeChoice::Campfire),
        )
        .chain(
            terra_core::crusher::recipes()
                .iter()
                .filter(|r| r.input == item)
                .map(RecipeChoice::Crush),
        )
        .chain(
            terra_core::brewing::recipes()
                .iter()
                .filter(|r| cauldron_supply || r.inputs.iter().any(|s| s.item == item))
                .map(RecipeChoice::Brew),
        )
        .collect();
    choices.sort_by(|a, b| {
        (a.category(), a.name(), a.selection().0).cmp(&(b.category(), b.name(), b.selection().0))
    });
    let mut result: Vec<_> = choices
        .into_iter()
        .map(|choice| {
            if let RecipeChoice::Craft(recipe) = choice {
                let wood = recipe
                    .ingredients()
                    .iter()
                    .any(|i| i.wood && i.matches(item))
                    .then(|| {
                        WOOD_PREVIEWS
                            .iter()
                            .position(|(_, mat)| ItemKind::Block(*mat) == item)
                    })
                    .flatten();
                Usage::Registered(choice, book.output_with_ingredient(recipe, item), wood)
            } else {
                Usage::Registered(choice, choice.output(), None)
            }
        })
        .collect();
    let mixer_target = session
        .container
        .as_ref()
        .and_then(|container| container.mixer.as_ref())
        .map(|m| m.target);
    if let ItemKind::Dye(color) = item {
        if let DyeColor::Rgb(_) = color {
            for base in [71, 72, 73, 103, 104, 105, 106, 148] {
                for wood in [
                    WoodSpecies::Oak,
                    WoodSpecies::Birch,
                    WoodSpecies::Spruce,
                    WoodSpecies::Cherry,
                ] {
                    if base != 148 && wood != WoodSpecies::Oak {
                        continue;
                    }
                    if let Some(output) =
                        terra_core::tints::recolored(ItemKind::for_wood(base, wood), color)
                    {
                        push_coloring(&mut result, output);
                    }
                }
            }
        } else {
            // RGB targets are continuous. Show the dye's own calibrated batch,
            // and the player's current custom blend if it actually consumes this pigment.
            let [r, g, b, _] = color.tint().map(|c| (c * 255.).round() as u8);
            for rgb in std::iter::once([r, g, b]).chain(mixer_target.filter(|t| *t != [r, g, b])) {
                let recipe = terra_core::mixer::recipe(rgb);
                if recipe.inputs.iter().any(|need| need.item == item) {
                    result.push(Usage::Generated {
                        output: recipe.output,
                        station: "Mixer",
                    });
                }
            }
        }
    } else if item
        .block_material()
        .is_some_and(terra_core::tints::dyeable)
    {
        // One inspectable example represents the custom RGB range for this base.
        let pigment = DyeColor::Rgb(mixer_target.unwrap_or([72, 143, 202]));
        if let Some(output) = terra_core::tints::recolored(item, pigment)
            && output != item
        {
            push_coloring(&mut result, output);
        }
    }
    result
}
fn push_coloring(entries: &mut Vec<Usage<'_>>, item: ItemKind) {
    entries.push(Usage::Generated {
        output: ItemStack { item, count: 1 },
        station: "Coloring Recipe",
    });
}
