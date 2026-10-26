use bevy::prelude::*;
use terra_core::creatures::taming::CompanionMode;
use tilt_ui::{ElementId, HtmlEvent, html_fn};

#[html_fn("pet_menu_action")]
fn pet_menu_action(In(event): In<HtmlEvent>, ids: Query<&ElementId>, mut actions: MessageWriter<crate::PetMenuAction>) {
    let Ok(id) = ids.get(event.target) else { return };
    let mode = match id.0.as_str() {
        "pet-follow" => CompanionMode::Follow,
        "pet-rest" => CompanionMode::Wait,
        "pet-none" => CompanionMode::None,
        "pet-close" => { actions.write(crate::PetMenuAction::Close); return; }
        _ => return,
    };
    actions.write(if event.kind == "mouseenter" { crate::PetMenuAction::Hover(mode) } else { crate::PetMenuAction::Select(mode) });
}
