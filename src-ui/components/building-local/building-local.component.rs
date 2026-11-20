use bevy::prelude::*;
use tilt_ui::{ElementId, HtmlEvent, UiBindingStore, UiStore, html_fn};

#[derive(Default, serde::Serialize, serde::Deserialize, UiStore)]
#[ui_store(mutable)]
pub struct BuildingIntent {
    pub npc: String,
    pub id: String,
    pub revision: String,
    pub template: String,
}

#[html_fn("building_command")]
fn building_command(
    In(event): In<HtmlEvent>,
    ids: Query<&ElementId>,
    parents: Query<&ChildOf>,
    store: Res<UiBindingStore>,
    mut actions: MessageWriter<crate::BuildingUiAction>,
) {
    use crate::BuildingUiAction as Action;
    let mut target = Some(event.target);
    for _ in 0..16 {
        let Some(entity) = target else {
            return;
        };
        if let Ok(id) = ids.get(entity) {
            let intent = store.get_store::<BuildingIntent>();
            let job = || {
                intent.as_ref().and_then(|intent| {
                    Some((intent.id.parse().ok()?, intent.revision.parse().ok()?))
                })
            };
            let worker = || intent.as_ref().and_then(|intent| intent.npc.parse().ok());
            let action = match id.0.as_str() {
                "building-refresh" => Some(Action::Refresh),
                "building-toggle-areas" => Some(Action::ToggleAreas),
                "building-pick-storage" => Some(Action::PickStorage),
                "building-save-storage" => Some(Action::SaveStorage),
                "building-pick-residential" => Some(Action::PickResidential),
                "building-save-residential" => Some(Action::SaveResidential),
                "building-disable-residential" => Some(Action::DisableResidential),
                "building-pick-house" => Some(Action::PickHouse),
                "building-queue-house" => Some(Action::QueueHouse),
                "building-select-template" => intent.as_ref().map(|intent| Action::Template(intent.template.clone())),
                "building-add-craftsman" => {
                    worker().map(|npc| Action::Craftsman { npc, enabled: true })
                }
                "building-remove-craftsman" => worker().map(|npc| Action::Craftsman {
                    npc,
                    enabled: false,
                }),
                "building-pause" => job().map(|(id, revision)| Action::Enabled {
                    id,
                    revision,
                    enabled: false,
                }),
                "building-resume" => job().map(|(id, revision)| Action::Enabled {
                    id,
                    revision,
                    enabled: true,
                }),
                "building-cancel" => job().map(|(id, revision)| Action::Cancel { id, revision }),
                _ => None,
            };
            if let Some(action) = action {
                actions.write(action);
                return;
            }
        }
        target = parents.get(entity).ok().map(ChildOf::parent);
    }
}

#[html_fn("building_facing")]
fn building_facing(In(event): In<HtmlEvent>, mut actions: MessageWriter<crate::BuildingUiAction>) {
    if let Some(value) = event.value {
        actions.write(crate::BuildingUiAction::Facing(value));
    }
}


#[html_fn("building_category")]
fn building_category(In(event): In<HtmlEvent>, mut actions: MessageWriter<crate::BuildingUiAction>) {
    if let Some(value) = event.value { actions.write(crate::BuildingUiAction::Category(value)); }
}
