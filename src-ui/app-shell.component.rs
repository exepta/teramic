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
    store: Res<UiBindingStore>,
    mut actions: MessageWriter<MenuAction>,
) {
    if let Ok(id) = elements.get(event.target) {
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
