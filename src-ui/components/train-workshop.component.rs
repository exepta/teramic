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
                break;
            } else if train_command(&id.0) {
                actions.write(crate::TrainUiAction::Command(id.0.clone()));
                break;
            }
        }
        target = parents.get(entity).ok().map(ChildOf::parent);
    }
}

fn train_command(id: &str) -> bool {
    matches!(
        id,
        "close"
            | "wheelbase-dec"
            | "wheelbase-inc"
            | "blueprint"
            | "blueprint-build"
            | "blueprint-clear"
            | "clear"
            | "front"
            | "second-front"
            | "previous"
            | "next"
            | "workshop"
            | "cargo"
            | "seat"
            | "assemble"
            | "dismantle"
            | "couple"
            | "uncouple"
            | "edit"
            | "engine"
            | "drive-more"
            | "drive-less"
            | "drive-stop"
            | "drive-brake"
            | "kind:locomotive"
            | "kind:tender"
            | "kind:passenger"
            | "kind:freight"
    ) || id
        .strip_prefix("slot:")
        .is_some_and(|slot| slot.parse::<u8>().is_ok())
        || id
            .strip_prefix("select:")
            .is_some_and(|vehicle| vehicle.parse::<u64>().is_ok())
        || id
            .strip_prefix("drive-throttle:")
            .is_some_and(|notch| notch.parse::<i8>().is_ok())
}
