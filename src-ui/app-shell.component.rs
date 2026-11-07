use crate::menu::MenuAction;
use bevy::prelude::*;
use tilt_ui::{ElementId, HtmlEvent, UiBindingStore, UiStore, html_fn};

#[derive(Default, serde::Serialize, serde::Deserialize, UiStore)]
#[ui_store(mutable)]
pub struct MenuIntent {
    pub command: String,
    pub control: String,
}

#[html_fn("menu_command")]
fn menu_command(
    In(event): In<HtmlEvent>,
    elements: Query<&ElementId>,
    parents: Query<&ChildOf>,
    store: Res<UiBindingStore>,
    mut actions: MessageWriter<MenuAction>,
) {
    // Icons and captions can be the hit target; commands belong to the nearest
    // enclosing control, including static category tabs.
    let mut target = Some(event.target);
    let mut id = None;
    while let Some(entity) = target {
        if let Ok(element) = elements.get(entity) { id = Some(element); break; }
        target = parents.get(entity).ok().map(ChildOf::parent);
    }
    if let Some(id) = id {
        let command = if matches!(
            id.0.as_str(),
            "menu-action" | "world-card" | "server-card" | "row-binding" | "row-command"
        ) {
            store
                .get_store::<MenuIntent>()
                .map_or("", |s| s.command.as_str())
        } else {
            id.0.as_str()
        };
        actions.write(MenuAction::Command(command.into()));
    }
}
#[html_fn("menu_change")]
fn menu_change(
    In(event): In<HtmlEvent>,
    elements: Query<&ElementId>,
    store: Res<UiBindingStore>,
    mut actions: MessageWriter<MenuAction>,
) {
    if let (Ok(id), Some(value)) = (elements.get(event.target), event.value) {
        let control = if id.0.starts_with("row-") {
            store
                .get_store::<MenuIntent>()
                .map_or("", |s| s.control.as_str())
        } else {
            id.0.as_str()
        };
        actions.write(MenuAction::Change {
            control: control.into(),
            value,
        });
    }
}
