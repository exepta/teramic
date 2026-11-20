use bevy::prelude::*;
use tilt_ui::{ElementId, HtmlEvent, UiBindingStore, UiStore, html_fn};

#[derive(Default, serde::Serialize, serde::Deserialize, UiStore)]
#[ui_store(mutable)]
pub struct NpcWorkIntent {
    pub npc: String,
}

#[html_fn("npc_work_command")]
fn npc_work_command(
    In(event): In<HtmlEvent>,
    ids: Query<&ElementId>,
    parents: Query<&ChildOf>,
    store: Res<UiBindingStore>,
    mut actions: MessageWriter<crate::NpcWorkUiAction>,
) {
    let mut target = Some(event.target);
    for _ in 0..16 {
        let Some(entity) = target else { return };
        if let Ok(id) = ids.get(entity) {
            let action = match id.0.as_str() {
                "npc-work-close" => Some(crate::NpcWorkUiAction::Close),
                "npc-work-pick-area" => Some(crate::NpcWorkUiAction::PickArea),
                "npc-work-pick-storage" => Some(crate::NpcWorkUiAction::PickStorage),
                "npc-work-refresh" => Some(crate::NpcWorkUiAction::Refresh),
                "npc-work-toggle-areas" => Some(crate::NpcWorkUiAction::ToggleAreas),
                "npc-work-assign" => Some(crate::NpcWorkUiAction::Assign),
                "npc-work-select" => store
                    .get_store::<NpcWorkIntent>()
                    .and_then(|intent| intent.npc.parse().ok())
                    .map(crate::NpcWorkUiAction::Select),
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

#[html_fn("npc_work_change")]
fn npc_work_change(
    In(event): In<HtmlEvent>,
    ids: Query<&ElementId>,
    parents: Query<&ChildOf>,
    mut actions: MessageWriter<crate::NpcWorkUiAction>,
) {
    let Some(value) = event.value else { return };
    let mut target = Some(event.target);
    for _ in 0..16 {
        let Some(entity) = target else { return };
        if let Ok(id) = ids.get(entity) {
            if id.0 == "npc-work-enabled" {
                if let Ok(enabled) = value.parse() {
                    actions.write(crate::NpcWorkUiAction::Enabled(enabled));
                }
                return;
            }
            if let Some(field) = id.0.strip_prefix("npc-work-field-") {
                actions.write(crate::NpcWorkUiAction::Field {
                    field: field.into(),
                    value,
                });
                return;
            }
        }
        target = parents.get(entity).ok().map(ChildOf::parent);
    }
}
