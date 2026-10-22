use crate::CraftingAction;
use bevy::prelude::*;
use tilt_ui::{ElementId, HtmlEvent, html_fn};
#[html_fn("crafting_command")]
fn crafting_command(
    In(event): In<HtmlEvent>,
    elements: Query<&ElementId>,
    mut actions: MessageWriter<CraftingAction>,
) {
    let Ok(id) = elements.get(event.target) else {
        return;
    };
    let action = match id.0.as_str() {
        "craft-clear-grid" => CraftingAction::Clear,
        "craft-close-recipe" => CraftingAction::CloseRecipe,
        "craft-apply-recipe" => CraftingAction::ApplyRecipe,
        "craft-alternative-previous" => CraftingAction::Alternative(-1),
        "craft-alternative-next" => CraftingAction::Alternative(1),
        _ => return,
    };
    actions.write(action);
}
