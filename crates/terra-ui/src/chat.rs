use bevy::{
    input_focus::{FocusCause, InputFocus},
    prelude::*,
};
use serde::Serialize;
use terra_core::{ControlsConfig, GameAction};
use terra_network::client::{
    ChatCommand, ChatEntry, ClientChat, ClientChatCommands, ClientSession,
};
use terra_player::{CaptureRequest, PlayerInputBlock, PlayerSet};
use tilt_ui::{ElementId, html_shared};

use crate::{
    inventory::InventoryState,
    menu::{MenuPage, MenuState},
};

#[cfg(test)]
#[path = "chat_tests.rs"]
mod tests;

#[derive(Resource, Default)]
pub(crate) struct ChatState {
    open: bool,
    resume_pending: bool,
    focus_pending: bool,
    scroll_frames: u8,
    draft: String,
    history: Vec<String>,
    history_index: Option<usize>,
    history_draft: String,
    revision: u64,
    visible_until: f64,
    completion: Option<Completion>,
    selected: usize,
    last_ping: u64,
    sleeping: bool,
    sleep_opened: bool,
}

struct Completion {
    prefix: String,
    last: String,
    index: usize,
}

impl ChatState {
    pub(crate) fn blocks_input(&self) -> bool {
        self.open || self.resume_pending
    }
}

#[derive(Message)]
pub enum ChatAction {
    Change(String),
    Send,
    Teleport(u64),
    Suggest(String),
    HelpPage(usize),
}

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct ChatInputSet;

#[derive(Clone, Serialize, PartialEq)]
pub struct ChatRow {
    id: String,
    author: String,
    text: String,
    locate: bool,
    name: String,
    coordinates: String,
    token: String,
    can_teleport: bool,
    spans: Vec<ChatSpan>,
}

#[derive(Clone, Serialize, PartialEq)]
pub struct ChatSpan {
    text: String,
    mention: bool,
}

impl From<&ChatEntry> for ChatRow {
    fn from(entry: &ChatEntry) -> Self {
        let located = entry.locate.as_ref();
        Self {
            id: entry.id.to_string(),
            author: entry
                .author
                .as_ref()
                .map_or_else(String::new, |name| format!("{name}: ")),
            text: entry.text.clone(),
            locate: located.is_some(),
            name: located
                .map(|result| result.name.clone())
                .unwrap_or_default(),
            coordinates: located
                .map(|result| {
                    let [x, y, z] = result.position;
                    format!("{x}, {y}, {z}")
                })
                .unwrap_or_default(),
            token: located
                .and_then(|result| result.token)
                .map(|token| token.to_string())
                .unwrap_or_default(),
            can_teleport: located.is_some_and(|result| result.token.is_some()),
            spans: vec![ChatSpan {
                text: entry.text.clone(),
                mention: false,
            }],
        }
    }
}

#[html_shared]
#[derive(Resource, Default, Serialize, PartialEq)]
pub struct ChatView {
    pub open: bool,
    pub visible: bool,
    pub draft: String,
    pub rows: Vec<ChatRow>,
    pub suggestions: Vec<ChatSuggestion>,
    pub has_suggestions: bool,
    pub style: String,
    pub help: bool,
    pub help_entries: Vec<HelpRow>,
    pub help_page: String,
    pub help_title: String,
    pub help_previous: String,
    pub help_next: String,
    pub has_help_previous: bool,
    pub has_help_next: bool,
}

#[derive(Clone, Serialize, PartialEq)]
pub struct HelpRow {
    usage: String,
    description: String,
}

#[derive(Clone, Serialize, PartialEq)]
pub struct ChatSuggestion {
    value: String,
    selected: bool,
}

pub(crate) fn install(app: &mut App) {
    app.init_resource::<ChatState>()
        .init_resource::<ChatView>()
        .add_message::<ChatAction>()
        .add_systems(
            PreUpdate,
            block_opening_frame.after(bevy::input::InputSystems),
        )
        .add_systems(
            Update,
            update
                .in_set(ChatInputSet)
                .before(crate::menu::MenuInputSet)
                .before(PlayerSet::Movement),
        )
        .add_systems(
            PostUpdate,
            focus_and_scroll.before(bevy::ui::UiSystems::Prepare),
        );
}

fn available(world: &World) -> bool {
    !world
        .get_resource::<crate::pets::PetMenuState>()
        .is_some_and(|p| p.blocks_input())
        && world
            .get_resource::<ClientSession>()
            .is_some_and(|session| session.ready && session.error.is_none() && !session.dead())
        && world
            .get_resource::<MenuState>()
            .is_none_or(|menu| menu.page == MenuPage::Playing)
        && !world.resource::<crate::SettingsView>().open
        && !world
            .get_resource::<crate::minimap::fullmap::FullMapState>()
            .is_some_and(|map| map.blocks_input())
}

fn block_opening_frame(world: &mut World) {
    if available(world)
        && !world
            .get_resource::<InventoryState>()
            .is_some_and(InventoryState::blocks_input)
        && (action_pressed(world, GameAction::Chat) || action_pressed(world, GameAction::Command))
    {
        // FixedUpdate runs before the UI's Update: block that very first input tick too.
        world.resource_mut::<PlayerInputBlock>().0 = true;
    }
}

fn action_pressed(world: &World, action: GameAction) -> bool {
    let binding = world
        .get_resource::<ControlsConfig>()
        .map_or_else(|| action.default_binding(), |config| config.binding(action));
    binding.just_pressed(
        world.resource::<ButtonInput<KeyCode>>(),
        world.get_resource::<ButtonInput<MouseButton>>(),
    )
}

fn update(world: &mut World) {
    let sleeping = world
        .get_resource::<ClientSession>()
        .is_some_and(|s| s.ready && s.sleeping().is_some());
    let entered_bed = sleeping && !world.resource::<ChatState>().sleeping;
    let woke = !sleeping && world.resource::<ChatState>().sleeping;
    world.resource_mut::<ChatState>().sleeping = sleeping;
    if entered_bed && !world.resource::<ChatState>().open {
        let mut state = world.resource_mut::<ChatState>();
        state.open = true;
        state.sleep_opened = true;
        state.focus_pending = true;
        state.resume_pending = false;
        state.scroll_frames = 3;
        world.write_message(CaptureRequest(false));
    } else if woke {
        if world.resource::<ChatState>().sleep_opened
            && world.resource::<ChatState>().open
            && world.resource::<ChatState>().draft.is_empty()
        {
            close(world);
        }
        world.resource_mut::<ChatState>().sleep_opened = false;
    }
    if world
        .get_resource::<MenuState>()
        .is_some_and(|m| m.page == MenuPage::HudLayout)
    {
        let position = world
            .get_resource::<crate::hud_layout::HudWidgetStyles>()
            .map_or_else(String::new, |s| s.chat.clone());
        let style = format!(
            "{position}height: 300px; min-height: 140px; box-sizing: border-box; border: 1px solid #80966a;"
        );
        *world.resource_mut::<ChatView>() = ChatView {
            visible: true,
            style,
            rows: vec![ChatRow::from(&ChatEntry {
                id: 0,
                author: None,
                text: "World Chat".into(),
                locate: None,
            })],
            ..Default::default()
        };
        return;
    }
    let active = available(world);
    let mut actions: Vec<_> = world
        .resource_mut::<Messages<ChatAction>>()
        .drain()
        .collect();
    if !active {
        if world
            .get_resource::<ClientSession>()
            .is_some_and(|s| s.ready)
            && world
                .get_resource::<MenuState>()
                .is_none_or(|m| m.world_hud_active())
        {
            // Input belongs to the foreground UI; the chat remains part of the world HUD.
            return;
        }
        if world.resource::<ChatState>().blocks_input() {
            let history = std::mem::take(&mut world.resource_mut::<ChatState>().history);
            *world.resource_mut::<ChatState>() = ChatState {
                history,
                ..Default::default()
            };
        }
        if *world.resource::<ChatView>() != ChatView::default() {
            *world.resource_mut::<ChatView>() = ChatView::default();
        }
        return;
    }
    let now = world.resource::<Time>().elapsed_secs_f64();
    let players = world
        .get_resource::<ClientChat>()
        .map(|chat| chat.players.clone())
        .unwrap_or_default();
    let keys = world.resource::<ButtonInput<KeyCode>>();
    let open = world.resource::<ChatState>().open;
    let key_enter = keys.just_pressed(KeyCode::Enter) || keys.just_pressed(KeyCode::NumpadEnter);
    let key_escape = keys.just_pressed(KeyCode::Escape);
    let inventory_open = world
        .get_resource::<InventoryState>()
        .is_some_and(InventoryState::blocks_input);
    let key_chat = !inventory_open && action_pressed(world, GameAction::Chat);
    let key_command = !inventory_open && action_pressed(world, GameAction::Command);
    let key_complete = keys.just_pressed(KeyCode::Tab);
    let complete_reverse = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
    let history_direction = i32::from(keys.just_pressed(KeyCode::ArrowDown))
        - i32::from(keys.just_pressed(KeyCode::ArrowUp));
    let released = keys.get_pressed().next().is_none()
        && world
            .resource::<ButtonInput<MouseButton>>()
            .get_pressed()
            .next()
            .is_none();
    if world.resource::<ChatState>().resume_pending && released {
        world.resource_mut::<ChatState>().resume_pending = false;
        world.resource_mut::<PlayerInputBlock>().0 = false;
        world.write_message(CaptureRequest(true));
    }
    if !open && !world.resource::<ChatState>().resume_pending && (key_chat || key_command) {
        let mut state = world.resource_mut::<ChatState>();
        state.open = true;
        state.focus_pending = true;
        state.scroll_frames = 3;
        state.draft = if key_command {
            "/".into()
        } else {
            String::new()
        };
        state.history_index = None;
        state.completion = None;
        state.selected = 0;
        world.write_message(CaptureRequest(false));
    } else if open {
        if key_escape {
            actions.clear();
            close(world);
        } else if key_enter {
            // Read TiltUI's actual editor too: its onchange may be delivered later this frame.
            let draft = world
                .query::<(&ElementId, &tilt_ui::EditableText)>()
                .iter(world)
                .find(|(id, _)| id.0 == "chat-input")
                .map(|(_, text)| text.value.clone());
            if let Some(draft) = draft {
                actions.push(ChatAction::Change(draft));
            }
            actions.push(ChatAction::Send);
        }
    }
    for action in actions {
        if !world.resource::<ChatState>().open {
            continue;
        }
        match action {
            ChatAction::Change(value) => {
                let value: String = value
                    .chars()
                    .filter(|c| !c.is_control())
                    .take(256)
                    .collect();
                let mut state = world.resource_mut::<ChatState>();
                if state.draft != value {
                    state.draft = value;
                    state.history_index = None;
                    state.completion = None;
                    state.selected = 0;
                }
            }
            ChatAction::Suggest(value) => {
                let mut state = world.resource_mut::<ChatState>();
                if candidates(&state, &players).contains(&value) {
                    state.draft = value;
                    state.history_index = None;
                    state.completion = None;
                    state.selected = 0;
                    state.focus_pending = true;
                }
            }
            ChatAction::Send => {
                let mut state = world.resource_mut::<ChatState>();
                let text = state.draft.trim().to_owned();
                if text.is_empty() {
                    continue;
                }
                if state.history.last() != Some(&text) {
                    state.history.push(text.clone());
                }
                if state.history.len() > 32 {
                    state.history.remove(0);
                }
                state.history_index = None;
                state.completion = None;
                state.draft.clear();
                state.focus_pending = true;
                if let Ok(Some(command)) = terra_network::protocol::commands::parse(&text) {
                    use terra_network::protocol::commands::Command;
                    if matches!(command, Command::Help(_) | Command::GameRuleList(_)) {
                        if let Some(mut chat) = world.get_resource_mut::<ClientChat>() {
                            match command {
                                Command::Help(page) => chat.show_help(page),
                                Command::GameRuleList(page) => chat.show_rules(page),
                                _ => {}
                            }
                        }
                        continue;
                    }
                }
                if let Some(mut queue) = world.get_resource_mut::<ClientChatCommands>() {
                    queue.0.push_back(ChatCommand::Send(text));
                }
            }
            ChatAction::Teleport(token) => {
                if world.resource::<ClientSession>().teleporting {
                    continue;
                }
                let valid = world.get_resource::<ClientChat>().is_some_and(|chat| {
                    chat.entries.iter().any(|entry| {
                        entry
                            .locate
                            .as_ref()
                            .is_some_and(|result| result.token == Some(token))
                    })
                });
                if valid {
                    if let Some(mut queue) = world.get_resource_mut::<ClientChatCommands>() {
                        queue.0.push_back(ChatCommand::Teleport(token));
                    }
                    world.resource_mut::<ChatState>().focus_pending = true;
                }
            }
            ChatAction::HelpPage(page) => {
                let rules = world.get_resource::<ClientChat>().is_some_and(|c| c.rules);
                let pages = if rules {
                    terra_network::protocol::commands::rule_pages()
                } else {
                    terra_network::protocol::commands::help_pages()
                };
                if (1..=pages).contains(&page)
                    && let Some(mut chat) = world.get_resource_mut::<ClientChat>()
                {
                    if rules {
                        chat.show_rules(page);
                    } else {
                        chat.show_help(page);
                    }
                }
            }
        }
    }
    if open && !key_enter && !key_escape && history_direction != 0 {
        let mut state = world.resource_mut::<ChatState>();
        if state.history_index.is_none()
            && (state.draft.starts_with('/') || !candidates(&state, &players).is_empty())
        {
            let count = candidates(&state, &players).len();
            if count > 0 {
                state.selected =
                    (state.selected as i32 + history_direction).rem_euclid(count as i32) as usize;
            }
        } else {
            browse_history(&mut state, history_direction);
        }
    }
    if open && key_complete && world.resource::<ChatState>().open {
        let editor = world
            .query::<(&ElementId, &tilt_ui::EditableText)>()
            .iter(world)
            .find(|(id, _)| id.0 == "chat-input")
            .map(|(_, text)| text.value.clone());
        let mut state = world.resource_mut::<ChatState>();
        if let Some(editor) = editor {
            state.draft = editor;
        }
        complete(&mut state, complete_reverse, &players);
    }
    if world.resource::<ChatState>().blocks_input() || world.resource::<ClientSession>().teleporting
    {
        world.resource_mut::<PlayerInputBlock>().0 = true;
    }
    let (revision, has_entries) = world
        .get_resource::<ClientChat>()
        .map_or((0, false), |chat| {
            (
                chat.revision,
                !chat.entries.is_empty() || chat.help_page.is_some(),
            )
        });
    let position = world
        .get_resource::<crate::hud_layout::HudWidgetStyles>()
        .map_or_else(String::new, |s| s.chat.clone());
    let (new_entries, open, visible, opacity, last_ping) = {
        let mut state = world.resource_mut::<ChatState>();
        let new_entries = revision != state.revision;
        if new_entries {
            if revision < state.revision {
                state.last_ping = 0;
            }
            state.revision = revision;
            state.visible_until = now + 7.0;
            state.scroll_frames = 3;
        }
        let open = state.open;
        let visible = open || (has_entries && now < state.visible_until);
        let opacity = if open {
            1.0
        } else {
            ((state.visible_until - now) / 2.0).clamp(0.0, 1.0)
        };
        (new_entries, open, visible, opacity, state.last_ping)
    };
    let style = format!("{position}opacity: {opacity:.3};");
    let mention = world
        .get_resource::<terra_core::PlayerProfile>()
        .is_some_and(|profile| {
            world.get_resource::<ClientChat>().is_some_and(|chat| {
                chat.entries.iter().any(|entry| {
                    entry.id > last_ping
                        && entry
                            .author
                            .as_ref()
                            .is_some_and(|name| name != &profile.name)
                        && terra_network::protocol::commands::mention_spans(&entry.text, &players)
                            .iter()
                            .any(|(s, m)| *m && s[1..].eq_ignore_ascii_case(&profile.name))
                })
            })
        });
    if mention
        && let Some(mut events) =
            world.get_resource_mut::<Messages<terra_core::footsteps::UiSound>>()
    {
        events.write(terra_core::footsteps::UiSound::Mention);
    }
    let newest = world
        .get_resource::<ClientChat>()
        .and_then(|c| {
            c.entries
                .iter()
                .filter(|e| e.author.is_some())
                .map(|e| e.id)
                .max()
        })
        .unwrap_or(last_ping);
    world.resource_mut::<ChatState>().last_ping = newest.max(last_ping);
    let state = world.resource::<ChatState>();
    let draft = state.draft.clone();
    let suggestions = if open {
        candidates(state, &players)
            .into_iter()
            .enumerate()
            .map(|(index, value)| ChatSuggestion {
                value,
                selected: index == state.selected,
            })
            .collect()
    } else {
        Vec::new()
    };
    let rules_changed = world
        .get_resource::<ClientChat>()
        .is_some_and(|chat| chat.rules && chat.help_page.is_some())
        && world
            .get_resource_ref::<terra_core::GameConfig>()
            .is_some_and(|config| config.is_changed());
    let view = world.resource::<ChatView>();
    if !new_entries
        && !rules_changed
        && view.open == open
        && view.visible == visible
        && view.draft == draft
        && view.style == style
        && view.suggestions == suggestions
    {
        return;
    }
    let rows = world
        .get_resource::<ClientChat>()
        .map(|chat| {
            chat.entries
                .iter()
                .skip(if open {
                    0
                } else {
                    chat.entries.len().saturating_sub(5)
                })
                .map(|entry| {
                    let mut row = ChatRow::from(entry);
                    row.spans =
                        terra_network::protocol::commands::mention_spans(&entry.text, &players)
                            .into_iter()
                            .map(|(text, mention)| ChatSpan { text, mention })
                            .collect();
                    row
                })
                .collect()
        })
        .unwrap_or_default();
    let next = ChatView {
        open,
        visible,
        draft,
        rows,
        has_suggestions: !suggestions.is_empty(),
        suggestions,
        style,
        ..help_view(world)
    };
    if *world.resource::<ChatView>() != next {
        *world.resource_mut::<ChatView>() = next;
    }
}

fn help_view(world: &World) -> ChatView {
    use terra_network::protocol::commands::{help_page, help_pages};
    let Some(page) = world.get_resource::<ClientChat>().and_then(|c| c.help_page) else {
        return ChatView::default();
    };
    let rules = world.resource::<ClientChat>().rules;
    let pages = if rules {
        terra_network::protocol::commands::rule_pages()
    } else {
        help_pages()
    };
    let entries = if rules {
        let Some(config) = world.get_resource::<terra_core::GameConfig>() else {
            return ChatView::default();
        };
        terra_network::protocol::commands::rule_page(page)
            .iter()
            .map(|rule| HelpRow {
                usage: format!("{} = {}", rule.name(), rule.value(&config.world)),
                description: rule.description().into(),
            })
            .collect()
    } else {
        help_page(page)
            .iter()
            .map(|entry| HelpRow {
                usage: entry.usage.into(),
                description: entry.description.into(),
            })
            .collect()
    };
    ChatView {
        help: true,
        help_title: if rules {
            "World Gamerules"
        } else {
            "TERAMIC Commands"
        }
        .into(),
        help_entries: entries,
        help_page: format!("{page} / {pages}"),
        help_previous: page.saturating_sub(1).to_string(),
        help_next: (page + 1).to_string(),
        has_help_previous: page > 1,
        has_help_next: page < pages,
        ..Default::default()
    }
}

fn browse_history(state: &mut ChatState, direction: i32) {
    if state.history.is_empty() || (state.history_index.is_none() && direction > 0) {
        return;
    }
    let length = state.history.len();
    if state.history_index.is_none() {
        state.history_draft = state.draft.clone();
    }
    let index =
        (state.history_index.unwrap_or(length) as i32 + direction).clamp(0, length as i32) as usize;
    state.draft = state
        .history
        .get(index)
        .cloned()
        .unwrap_or_else(|| state.history_draft.clone());
    state.history_index = (index < length).then_some(index);
    state.completion = None;
    state.selected = 0;
    state.focus_pending = true;
}

fn close(world: &mut World) {
    let until = world.resource::<Time>().elapsed_secs_f64() + 7.0;
    let mut state = world.resource_mut::<ChatState>();
    state.visible_until = until;
    state.open = false;
    state.resume_pending = true;
    state.focus_pending = false;
    state.completion = None;
    if let Some(mut focus) = world.get_resource_mut::<InputFocus>() {
        focus.clear();
    }
}

fn candidates(state: &ChatState, players: &[String]) -> Vec<String> {
    let prefix = state
        .completion
        .as_ref()
        .filter(|c| c.last == state.draft)
        .map_or(state.draft.as_str(), |c| c.prefix.as_str());
    terra_network::protocol::commands::suggestions(prefix, players)
}

fn complete(state: &mut ChatState, reverse: bool, players: &[String]) {
    let continuing = state
        .completion
        .as_ref()
        .is_some_and(|c| c.last == state.draft);
    let prefix = if continuing {
        state
            .completion
            .as_ref()
            .map(|c| c.prefix.clone())
            .unwrap_or_default()
    } else {
        state.draft.to_lowercase()
    };
    if !prefix.starts_with('/') && !prefix.contains('@') {
        state.completion = None;
        return;
    }
    let candidates = candidates(state, players);
    if candidates.is_empty() {
        state.completion = None;
        return;
    }
    let previous = state.completion.as_ref().map_or(0, |c| c.index);
    let index = if continuing && previous != usize::MAX {
        if reverse {
            (previous + candidates.len() - 1) % candidates.len()
        } else {
            (previous + 1) % candidates.len()
        }
    } else if reverse {
        candidates.len() - 1
    } else {
        state.selected.min(candidates.len() - 1)
    };
    state.draft = candidates[index].clone();
    state.history_index = None;
    state.selected = index;
    state.completion = Some(Completion {
        prefix,
        index,
        last: state.draft.clone(),
    });
    state.focus_pending = true;
}

fn focus_and_scroll(
    mut state: ResMut<ChatState>,
    mut focus: Option<ResMut<InputFocus>>,
    inputs: Query<(Entity, &ElementId), With<tilt_ui::EditableText>>,
    mut lists: Query<(&ElementId, &ComputedNode, &mut bevy::ui::ScrollPosition)>,
) {
    if state.open
        && let Some((entity, _)) = inputs.iter().find(|(_, id)| id.0 == "chat-input")
        && let Some(focus) = focus.as_mut()
        && (state.focus_pending || focus.get() != Some(entity))
    {
        focus.set(entity, FocusCause::Navigated);
        state.focus_pending = false;
    }
    if state.scroll_frames > 0 {
        for (id, node, mut scroll) in &mut lists {
            if id.0 == "chat-history" {
                scroll.0.y =
                    (node.content_size().y - node.size().y).max(0.0) * node.inverse_scale_factor;
            }
        }
        state.scroll_frames -= 1;
    }
    for (id, node, mut scroll) in &mut lists {
        if id.0 == "chat-suggestions" {
            let height = node.size().y * node.inverse_scale_factor;
            let top = state.selected as f32 * 26.0;
            if top < scroll.0.y {
                scroll.0.y = top;
            }
            if top + 32.0 > scroll.0.y + height {
                scroll.0.y = (top + 32.0 - height).max(0.0);
            }
        }
    }
}
