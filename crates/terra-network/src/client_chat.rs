//! UI-facing queue. The UI never mutates an authoritative player position.
use super::*;
use crate::protocol::chat::{CHAT_HISTORY_LIMIT, sanitize};
pub use crate::protocol::chat::{ChatEntry, LocateResult};

#[derive(Resource, Default)]
pub struct ClientChat {
    pub entries: VecDeque<ChatEntry>,
    pub revision: u64,
    pub players: Vec<String>,
    pub help_page: Option<usize>,
    pub rules: bool,
    teleport_started: Option<Instant>,
    awaiting_destination: bool,
}

impl ClientChat {
    pub fn clear(&mut self) {
        self.entries.clear();
        self.help_page = None;
        self.rules = false;
        self.revision = self.revision.saturating_add(1);
    }

    pub fn show_help(&mut self, page: usize) {
        self.rules = false;
        self.help_page = Some(page.clamp(1, crate::protocol::commands::help_pages()));
        self.revision = self.revision.saturating_add(1);
    }
    pub fn show_rules(&mut self, page: usize) {
        self.rules = true;
        self.help_page = Some(page.clamp(1, crate::protocol::commands::rule_pages()));
        self.revision = self.revision.saturating_add(1);
    }

    pub fn push(&mut self, entry: ChatEntry) {
        if self.entries.len() >= CHAT_HISTORY_LIMIT {
            self.entries.pop_front();
        }
        self.entries.push_back(entry);
        self.revision = self.revision.saturating_add(1);
    }

    pub fn notice(&mut self, text: impl Into<String>) {
        self.push(ChatEntry {
            id: u64::MAX.saturating_sub(self.revision),
            author: None,
            text: text.into(),
            locate: None,
        });
    }
}

#[derive(Clone, Debug)]
pub enum ChatCommand {
    Send(String),
    Teleport(u64),
}

#[derive(Resource, Default)]
pub struct ClientChatCommands(pub VecDeque<ChatCommand>);

pub(super) fn install(app: &mut App) {
    app.init_resource::<ClientChat>()
        .init_resource::<ClientChatCommands>()
        .add_systems(Update, finish_teleport.after(reconcile))
        .add_systems(PostUpdate, send.before(MessageSystems::Send));
}

fn send(
    mut queue: ResMut<ClientChatCommands>,
    mut chat: ResMut<ClientChat>,
    session: Res<ClientSession>,
    mut senders: Query<&mut MessageSender<ClientControl>, With<Client>>,
) {
    if queue.0.is_empty() {
        return;
    }
    if !session.ready || session.error.is_some() {
        queue.0.clear();
        chat.notice("Chat is available after joining a world.");
        return;
    }
    let Some(mut sender) = senders.iter_mut().next() else {
        queue.0.clear();
        chat.notice("No connection to the server.");
        return;
    };
    for command in queue.0.drain(..).take(4) {
        match command {
            ChatCommand::Send(text) => match sanitize(&text) {
                Some(text) if text == "/clear" => chat.clear(),
                Some(text)
                    if matches!(
                        crate::protocol::commands::parse(&text),
                        Ok(Some(crate::protocol::commands::Command::GameRuleList(_)))
                    ) =>
                {
                    if let Ok(Some(crate::protocol::commands::Command::GameRuleList(page))) =
                        crate::protocol::commands::parse(&text)
                    {
                        chat.show_rules(page);
                    }
                }
                Some(text)
                    if matches!(
                        crate::protocol::commands::parse(&text),
                        Ok(Some(crate::protocol::commands::Command::Help(_)))
                    ) =>
                {
                    if let Ok(Some(crate::protocol::commands::Command::Help(page))) =
                        crate::protocol::commands::parse(&text)
                    {
                        chat.show_help(page);
                    }
                }
                Some(text) => sender.send::<ControlChannel>(ClientControl::Chat(text)),
                None => chat.notice("Chat messages must contain 1 to 256 characters."),
            },
            ChatCommand::Teleport(token) => {
                sender.send::<ControlChannel>(ClientControl::Teleport(token));
            }
        }
    }
}

pub(super) fn preparing(chat: &mut ClientChat, session: &mut ClientSession) {
    session.respawn_error = None;
    session.teleporting = true;
    chat.teleport_started = Some(Instant::now());
    chat.awaiting_destination = false;
}

pub(super) fn cancelled(chat: &mut ClientChat, session: &mut ClientSession) {
    if session.dead() {
        session.respawn_error = Some("Safe spawn unavailable. Please try again.".into());
    }
    session.teleporting = false;
    chat.teleport_started = None;
    chat.awaiting_destination = false;
}

pub(super) fn destination(
    chat: &mut ClientChat,
    session: &mut ClientSession,
    tick: u64,
    state: PlayerState,
) -> Result<(), String> {
    reset_prediction(session, tick, state)?;
    session.teleporting = true;
    chat.awaiting_destination = true;
    chat.teleport_started = Some(Instant::now());
    Ok(())
}

pub(super) fn reset_prediction(
    session: &mut ClientSession,
    tick: u64,
    state: PlayerState,
) -> Result<(), String> {
    if !valid_equipment_vitals(state.survival, state.equipment)
        || state.mounted.is_some_and(|mount| !mount.valid())
        || Some(state.player) != session.player
        || state.generation != session.generation.saturating_add(1)
        || state
            .position
            .iter()
            .any(|p| !p.is_finite() || p.abs() > 999_000.0)
    {
        return Err("Invalid server teleport destination".into());
    }
    session.generation = state.generation;
    session.mounted = state.mounted;
    session.dismount_requested = false;
    session.stats.survival = state.survival;
    session.stats.equipment = state.equipment;
    session.predicted_vitals = state.survival;
    session.respawn_error = None;
    session.sequence = 0;
    session.pending.clear();
    session.outbox.clear();
    session.pending_inputs = 0;
    session.latest = Some((tick, state));
    session.reconciled = tick.saturating_sub(1);
    Ok(())
}

fn finish_teleport(
    mut session: ResMut<ClientSession>,
    mut chat: ResMut<ClientChat>,
    chunks: Res<ChunkStore>,
    bodies: Query<&Transform, With<CharacterBody>>,
) {
    if !session.teleporting || !chat.awaiting_destination {
        return;
    }
    let Some(transform) = bodies.iter().next() else {
        return;
    };
    let position = transform.translation;
    // space_clear also rejects missing sections; coordinate teleports may land in air.
    if terra_physics::character::space_clear(&chunks, position) {
        cancelled(&mut chat, &mut session);
    } else if chat
        .teleport_started
        .is_some_and(|t| t.elapsed() > Duration::from_secs(30))
    {
        session.fail("Teleport destination did not finish loading");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn history_is_bounded_and_transport_reset_discards_old_prediction() {
        let mut chat = ClientChat::default();
        for i in 0..100 {
            chat.notice(format!("Message {i}"));
        }
        assert_eq!(chat.entries.len(), CHAT_HISTORY_LIMIT);
        assert_eq!(chat.entries.front().unwrap().text, "Message 36");
        let mut session = ClientSession {
            player: Some(PlayerId(1)),
            sequence: 100,
            ..default()
        };
        let state = PlayerState {
            player: PlayerId(1),
            generation: 1,
            position: [1.5, 40.0, -9.5],
            ..default()
        };
        destination(&mut chat, &mut session, 12, state).unwrap();
        assert_eq!(session.sequence, 0);
        assert_eq!(session.generation, 1);
        assert!(session.teleporting);
        assert!(destination(&mut chat, &mut session, 12, state).is_err());
    }
}
