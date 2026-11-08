use bevy::prelude::*;
use tilt_ui::{ElementId, HtmlEvent, html_fn};

#[html_fn("train_workshop_action")]
fn train_workshop_action(
    In(event): In<HtmlEvent>,
    ids: Query<&ElementId>,
    parents: Query<&ChildOf>,
    mut actions: MessageWriter<crate::TrainUiAction>,
) {
    let mut target = Some(event.target);
    for _ in 0..16 {
        let Some(entity) = target else { break };
        if let Ok(id) = ids.get(entity) {
            actions.write(crate::TrainUiAction::Command(id.0.clone()));
            break;
        }
        target = parents.get(entity).ok().map(ChildOf::parent);
    }
}
