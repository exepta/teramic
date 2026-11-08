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
            if id.0 == "drive-throttle" {
                if let Some(value) = event
                    .value
                    .as_ref()
                    .and_then(|value| value.parse::<f64>().ok())
                    && value.is_finite()
                    && value.fract().abs() < 1e-6
                    && (-3.0..=3.0).contains(&value)
                {
                    actions.write(crate::TrainUiAction::Command(format!(
                        "drive-throttle:{}",
                        value as i8
                    )));
                }
            } else {
                actions.write(crate::TrainUiAction::Command(id.0.clone()));
            }
            break;
        }
        target = parents.get(entity).ok().map(ChildOf::parent);
    }
}
