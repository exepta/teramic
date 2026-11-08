use bevy::prelude::*;
use tilt_ui::{ElementId, HtmlEvent, html_fn};

#[html_fn("train_workshop_action")]
fn train_workshop_action(
    In(event): In<HtmlEvent>,
    ids: Query<&ElementId>,
    mut actions: MessageWriter<crate::TrainUiAction>,
) {
    if let Ok(id) = ids.get(event.target) {
        actions.write(crate::TrainUiAction::Command(id.0.clone()));
    }
}
