use bevy::prelude::*;
use tilt_ui::{ElementId, HtmlEvent, UiBindingStore, UiStore, html_fn};

#[derive(Default, serde::Serialize, serde::Deserialize, UiStore)]
#[ui_store(mutable)]
pub struct TrainIntent {
    pub command: String,
}

#[html_fn("train_workshop_action")]
fn train_workshop_action(
    In(event): In<HtmlEvent>,
    store: Option<Res<UiBindingStore>>,
    ids: Query<&ElementId>,
    parents: Query<&ChildOf>,
    mut actions: MessageWriter<crate::TrainUiAction>,
) {
    if ids.get(event.target).is_ok_and(|id| id.0 == "train-name") {
        if let Some(value) = event.value { actions.write(crate::TrainUiAction::Name(value)); }
        return;
    }
    let mut target = Some(event.target);
    for _ in 0..16 {
        let Some(entity) = target else { break };
        if let Ok(id) = ids.get(entity) {
            if matches!(
                id.0.as_str(),
                "train-slot" | "train-notch" | "train-vehicle"
            ) {
                if let Some(command) = store
                    .as_ref()
                    .and_then(|store| store.get_store::<TrainIntent>())
                    .map(|intent| &intent.command)
                    .filter(|command| train_command(command))
                {
                    actions.write(crate::TrainUiAction::Command(command.clone()));
                }
                break;
            } else if id.0 == "builder-autoload" {
                if let Some(value @ ("true" | "false")) = event.value.as_deref() {
                    actions.write(crate::TrainUiAction::Command(format!("builder-autoload:{value}")));
                }
                break;
            } else if id.0 == "drive-throttle" {
                if let Some(value) = event
                    .value
                    .as_ref()
                    .and_then(|value| value.parse::<f64>().ok())
                    && value.is_finite()
                    && value.fract().abs() < 1e-6
                    && (-2.0..=6.0).contains(&value)
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
            | "name-save"
            | "width:1"
            | "width:3"
            | "turn-left"
            | "turn-right"
            | "drive-start"
            | "wheelbase-dec"
            | "wheelbase-inc"
            | "blueprint"
            | "blueprint-build"
            | "blueprint-auto"
            | "blueprint-clear"
            | "clear"
            | "front"
            | "second-front"
            | "previous"
            | "next"
            | "workshop"
            | "cargo"
            | "cargo-sort-type"
            | "cargo-sort-name"
            | "cargo-sort-id"
            | "cargo-sort-menu"
            | "cargo-sort-dismiss"
            | "cargo-sort-quantity"
            | "cargo-sort-backpack-type"
            | "cargo-sort-backpack-name"
            | "cargo-sort-backpack-id"
            | "cargo-sort-backpack-menu"
            | "cargo-sort-backpack-quantity"
            | "seat"
            | "leave"
            | "destroy"
            | "destroy-confirm"
            | "destroy-cancel"
            | "assemble"
            | "dismantle"
            | "couple"
            | "uncouple"
            | "edit"
            | "water"
            | "engine"
            | "rail-builder"
            | "builder-size-width-less"
            | "builder-size-width-more"
            | "builder-size-height-less"
            | "builder-size-height-more"
            | "builder-size-height-auto"
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
