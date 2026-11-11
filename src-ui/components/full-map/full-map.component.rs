use bevy::prelude::*;
use tilt_ui::{ElementId, HtmlEvent, UiBindingStore, UiStore, html_fn};

#[derive(Default, serde::Serialize, serde::Deserialize, UiStore)]
#[ui_store(mutable)]
pub struct FullMapIntent {
    pub id: String,
    pub value: String,
}

#[html_fn("fullmap_icon_select")]
fn fullmap_icon_select(
    In(_event): In<HtmlEvent>,
    store: Res<UiBindingStore>,
    mut actions: MessageWriter<crate::minimap::fullmap::FullMapAction>,
) {
    let Some(intent) = store.get_store::<FullMapIntent>() else { return; };
    actions.write(crate::minimap::fullmap::FullMapAction::Change {
        control: "fullmap-icon".into(), value: intent.value.clone(),
    });
}

#[html_fn("fullmap_action")]
fn fullmap_action(
    In(event): In<HtmlEvent>,
    elements: Query<&ElementId>,
    store: Res<UiBindingStore>,
    mut actions: MessageWriter<crate::minimap::fullmap::FullMapAction>,
) {
    let Ok(control) = elements.get(event.target) else {
        return;
    };
    let id = if control.0 == "fullmap-point" {
        store
            .get_store::<FullMapIntent>()
            .and_then(|intent| intent.id.parse().ok())
    } else {
        None
    };
    actions.write(crate::minimap::fullmap::FullMapAction::Command {
        control: control.0.clone(),
        id,
    });
}

#[html_fn("fullmap_change")]
fn fullmap_change(
    In(event): In<HtmlEvent>,
    elements: Query<&ElementId>,
    mut actions: MessageWriter<crate::minimap::fullmap::FullMapAction>,
) {
    if let (Ok(control), Some(value)) = (elements.get(event.target), event.value) {
        actions.write(crate::minimap::fullmap::FullMapAction::Change {
            control: control.0.clone(),
            value,
        });
    }
}
