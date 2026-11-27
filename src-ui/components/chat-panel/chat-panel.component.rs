use bevy::prelude::*;
use tilt_ui::{ElementId, HtmlEvent, UiBindingStore, UiStore, html_fn};

#[derive(Default, serde::Serialize, serde::Deserialize, UiStore)]
#[ui_store(mutable)]
pub struct ChatIntent { pub token: String, pub suggestion: String, pub help_page: String }

#[html_fn("chat_change")]
fn chat_change(In(event): In<HtmlEvent>, mut actions: MessageWriter<crate::ChatAction>) {
    if let Some(value) = event.value { actions.write(crate::ChatAction::Change(value)); }
}

#[html_fn("chat_action")]
fn chat_action(In(event): In<HtmlEvent>, elements: Query<&ElementId>, store: Res<UiBindingStore>, mut actions: MessageWriter<crate::ChatAction>) {
    let Ok(id) = elements.get(event.target) else { return; };
    match id.0.as_str() {
        "chat-send" => { actions.write(crate::ChatAction::Send); }
        "chat-help-dismiss" => { actions.write(crate::ChatAction::DismissHelp); }
        "chat-suggestion" => {
            if let Some(intent) = store.get_store::<ChatIntent>() {
                actions.write(crate::ChatAction::Suggest(intent.suggestion.clone()));
            }
        }
        "chat-teleport" => {
            if let Some(token) = store.get_store::<ChatIntent>().and_then(|intent| intent.token.parse().ok()) {
                actions.write(crate::ChatAction::Teleport(token));
            }
        }
        "chat-help-previous" | "chat-help-next" => {
            if let Some(page) = store.get_store::<ChatIntent>().and_then(|intent| intent.help_page.parse().ok()) {
                actions.write(crate::ChatAction::HelpPage(page));
            }
        }
        _ => {}
    }
}
