use super::*;
#[test]
fn sleeping_opens_chat_and_waking_preserves_an_unfinished_message() {
    let mut app = app();
    {
        let mut session = app.world_mut().resource_mut::<ClientSession>();
        session.player = Some(terra_core::PlayerId(1));
        session
            .sleepers
            .push(terra_network::protocol::session::SleepingPlayer {
                player: terra_core::PlayerId(1),
                anchor: [0; 3],
                facing: 0,
            });
    }
    app.update();
    assert!(app.world().resource::<ChatView>().open);
    assert!(app.world().resource::<PlayerInputBlock>().0);
    app.world_mut()
        .write_message(ChatAction::Change("Good night".into()));
    app.update();
    app.world_mut()
        .resource_mut::<ClientSession>()
        .sleepers
        .clear();
    app.update();
    assert!(app.world().resource::<ChatView>().open);
    assert_eq!(app.world().resource::<ChatView>().draft, "Good night");
}

#[test]
fn automatically_opened_empty_bed_chat_closes_on_waking() {
    let mut app = app();
    {
        let mut session = app.world_mut().resource_mut::<ClientSession>();
        session.player = Some(terra_core::PlayerId(1));
        session
            .sleepers
            .push(terra_network::protocol::session::SleepingPlayer {
                player: terra_core::PlayerId(1),
                anchor: [0; 3],
                facing: 0,
            });
    }
    app.update();
    assert!(app.world().resource::<ChatView>().open);
    app.world_mut()
        .resource_mut::<ClientSession>()
        .sleepers
        .clear();
    app.update();
    assert!(!app.world().resource::<ChatView>().open);
}
#[test]
fn gamerules_use_the_chat_pager_with_eight_rows_per_page_and_real_values() {
    let mut app = app();
    app.insert_resource(terra_core::GameConfig::default());
    key(&mut app, KeyCode::Slash);
    app.world_mut()
        .resource_mut::<terra_core::GameConfig>()
        .world
        .game_rules
        .sleep_percentage = 50.0;
    app.world_mut()
        .write_message(ChatAction::Change("/gamerule list".into()));
    app.world_mut().write_message(ChatAction::Send);
    app.update();
    let view = app.world().resource::<ChatView>();
    assert_eq!(view.help_title, "World Gamerules");
    assert_eq!(view.help_page, "1 / 2");
    assert_eq!(view.help_entries.len(), 8);
    assert!(view.help_entries.len() <= 8);
    assert!(
        view.help_entries
            .iter()
            .any(|e| e.usage == "sleep_percentage = 50")
    );
    assert!(view.has_help_next && !view.has_help_previous);
    app.world_mut()
        .resource_mut::<terra_core::GameConfig>()
        .world
        .game_rules
        .sleep_percentage = 75.0;
    app.update();
    assert!(
        app.world()
            .resource::<ChatView>()
            .help_entries
            .iter()
            .any(|entry| entry.usage == "sleep_percentage = 75")
    );
    app.world_mut().write_message(ChatAction::HelpPage(2));
    app.update();
    let view = app.world().resource::<ChatView>();
    assert_eq!(view.help_page, "2 / 2");
    assert_eq!(view.help_entries.len(), 1);
    assert_eq!(view.help_entries[0].usage, "pvp = 0");
    assert!(!view.has_help_next && view.has_help_previous);
    app.world_mut()
        .resource_mut::<terra_core::GameConfig>()
        .world
        .game_rules
        .pvp = true;
    app.update();
    assert_eq!(
        app.world().resource::<ChatView>().help_entries[0].usage,
        "pvp = 1"
    );
}

#[test]
fn help_is_five_styled_commands_per_page_and_clicks_do_not_send_chat() {
    use terra_network::protocol::commands::{CATALOGUE, HELP_PAGE_SIZE, help_page, help_pages};
    let pages = help_pages();
    let mut app = app();
    key(&mut app, KeyCode::Slash);
    app.world_mut()
        .write_message(ChatAction::Change("/help".into()));
    app.world_mut().write_message(ChatAction::Send);
    app.update();
    let view = app.world().resource::<ChatView>();
    assert!(view.help);
    assert_eq!(view.help_page, format!("1 / {pages}"));
    assert_eq!(view.help_entries.len(), 5);
    assert!(!view.has_help_previous && view.has_help_next);
    assert!(app.world().resource::<ClientChatCommands>().0.is_empty());
    app.world_mut().write_message(ChatAction::HelpPage(2));
    app.update();
    let view = app.world().resource::<ChatView>();
    assert_eq!(view.help_page, format!("2 / {pages}"));
    assert_eq!(view.help_entries.len(), 5);
    assert!(
        view.help_entries
            .iter()
            .map(|entry| entry.usage.as_str())
            .eq(help_page(2).iter().map(|entry| entry.usage))
    );
    assert!(view.has_help_previous);
    assert_eq!(view.has_help_next, pages > 2);
    app.world_mut().write_message(ChatAction::HelpPage(pages));
    app.update();
    app.world_mut().write_message(ChatAction::HelpPage(999));
    app.update();
    let view = app.world().resource::<ChatView>();
    assert_eq!(view.help_page, format!("{pages} / {pages}"));
    assert_eq!(
        view.help_entries.len(),
        CATALOGUE.len() - (pages - 1) * HELP_PAGE_SIZE
    );
    assert!(!view.has_help_next);
    app.world_mut().resource_mut::<ClientChat>().clear();
    app.update();
    assert!(!app.world().resource::<ChatView>().help);
}

#[test]
fn inventory_keeps_recent_chat_visible_without_stealing_its_keyboard_focus() {
    let mut app = app();
    app.init_resource::<InventoryState>();
    app.world_mut().resource_mut::<InventoryState>().open = true;
    app.world_mut()
        .resource_mut::<ClientChat>()
        .notice("Visible in inventory");
    app.update();
    assert!(app.world().resource::<ChatView>().visible);
    key(&mut app, KeyCode::Slash);
    assert!(!app.world().resource::<ChatView>().open);
    assert!(
        app.world()
            .resource::<ChatView>()
            .rows
            .iter()
            .any(|r| r.text.contains("Visible"))
    );
}

#[test]
fn closed_chat_waits_five_seconds_then_fades_over_two_seconds() {
    let mut app = app();
    app.world_mut()
        .resource_mut::<ClientChat>()
        .push(ChatEntry {
            id: 1,
            author: None,
            text: "Test".into(),
            locate: None,
        });
    key(&mut app, KeyCode::KeyT);
    key(&mut app, KeyCode::Escape);
    app.world_mut()
        .resource_mut::<Time>()
        .advance_by(std::time::Duration::from_secs(5));
    app.update();
    assert!(
        app.world()
            .resource::<ChatView>()
            .style
            .contains("opacity: 1.000")
    );
    app.world_mut()
        .resource_mut::<Time>()
        .advance_by(std::time::Duration::from_secs(1));
    app.update();
    assert!(
        app.world()
            .resource::<ChatView>()
            .style
            .contains("opacity: 0.500")
    );
    app.world_mut()
        .resource_mut::<Time>()
        .advance_by(std::time::Duration::from_secs(1));
    app.update();
    assert!(!app.world().resource::<ChatView>().visible);
}

#[test]
fn a_local_notice_cannot_suppress_a_real_mention_or_repeat_its_sound() {
    let mut app = app();
    app.insert_resource(terra_core::PlayerProfile {
        uuid: terra_core::PlayerUuid::new_v4(),
        name: "Alice".into(),
        gender: terra_core::PlayerGender::default(),
        appearance: terra_core::CharacterAppearance::default(),
    });
    app.add_message::<terra_core::footsteps::UiSound>();
    app.world_mut().resource_mut::<ClientChat>().players = vec!["Alice".into(), "Bob".into()];
    app.world_mut()
        .resource_mut::<ClientChat>()
        .notice("local notice");
    app.update();
    app.world_mut()
        .resource_mut::<ClientChat>()
        .push(ChatEntry {
            id: 1,
            author: Some("Bob".into()),
            text: "Hi @Alice!".into(),
            locate: None,
        });
    app.update();
    let mut cursor = bevy::ecs::message::MessageCursor::<terra_core::footsteps::UiSound>::default();
    assert_eq!(
        cursor
            .read(
                app.world()
                    .resource::<Messages<terra_core::footsteps::UiSound>>()
            )
            .count(),
        1
    );
    app.update();
    assert_eq!(
        cursor
            .read(
                app.world()
                    .resource::<Messages<terra_core::footsteps::UiSound>>()
            )
            .count(),
        0
    );
}

#[test]
fn normal_chat_arrows_browse_history_and_restore_unsent_draft() {
    let mut app = app();
    key(&mut app, KeyCode::Slash);
    for text in ["/time set day", "/locate biome desert"] {
        app.world_mut()
            .write_message(ChatAction::Change(text.into()));
        app.world_mut().write_message(ChatAction::Send);
        app.update();
    }
    app.world_mut()
        .write_message(ChatAction::Change("unsent draft".into()));
    app.update();
    key(&mut app, KeyCode::ArrowUp);
    assert_eq!(
        app.world().resource::<ChatView>().draft,
        "/locate biome desert"
    );
    key(&mut app, KeyCode::ArrowUp);
    assert_eq!(app.world().resource::<ChatView>().draft, "/time set day");
    key(&mut app, KeyCode::ArrowUp);
    assert_eq!(app.world().resource::<ChatView>().draft, "/time set day");
    key(&mut app, KeyCode::ArrowDown);
    key(&mut app, KeyCode::ArrowDown);
    assert_eq!(app.world().resource::<ChatView>().draft, "unsent draft");
    key(&mut app, KeyCode::ArrowDown);
    assert_eq!(app.world().resource::<ChatView>().draft, "unsent draft");
    app.world_mut().resource_mut::<crate::SettingsView>().open = true;
    app.update();
    assert_eq!(app.world().resource::<ChatState>().history.len(), 2);
}

fn app() -> App {
    let mut app = App::new();
    app.init_resource::<Time>()
        .init_resource::<ButtonInput<KeyCode>>()
        .init_resource::<ButtonInput<MouseButton>>()
        .init_resource::<ChatState>()
        .init_resource::<ChatView>()
        .init_resource::<ClientChat>()
        .init_resource::<ClientChatCommands>()
        .init_resource::<crate::SettingsView>()
        .init_resource::<PlayerInputBlock>()
        .init_resource::<ClientSession>()
        .add_message::<ChatAction>()
        .add_message::<CaptureRequest>()
        .add_systems(Update, update);
    app.world_mut().resource_mut::<ClientSession>().ready = true;
    app
}

#[test]
fn asynchronous_teleport_owns_its_completion_even_when_chat_was_never_open() {
    let mut app = app();
    app.world_mut().resource_mut::<ClientSession>().teleporting = true;
    app.update();
    assert!(app.world().resource::<ChatState>().teleport_resume_pending);
    assert!(app.world().resource::<ChatState>().blocks_input());
    assert!(app.world().resource::<PlayerInputBlock>().0);
    app.world_mut()
        .resource_mut::<Messages<CaptureRequest>>()
        .clear();
    app.world_mut().resource_mut::<ClientSession>().teleporting = false;
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::KeyW);
    app.world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .press(MouseButton::Left);
    for _ in 0..3 {
        app.update();
    }
    assert!(app.world().resource::<ChatState>().teleport_resume_pending);
    assert!(app.world().resource::<PlayerInputBlock>().0);
    assert!(
        app.world_mut()
            .resource_mut::<Messages<CaptureRequest>>()
            .drain()
            .next()
            .is_none()
    );
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .reset_all();
    app.world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .reset_all();
    app.update();
    assert!(!app.world().resource::<ChatState>().blocks_input());
    assert!(!app.world().resource::<PlayerInputBlock>().0);
    assert_eq!(
        app.world_mut()
            .resource_mut::<Messages<CaptureRequest>>()
            .drain()
            .filter(|request| request.0)
            .count(),
        1
    );
    app.update();
    assert!(
        app.world_mut()
            .resource_mut::<Messages<CaptureRequest>>()
            .drain()
            .next()
            .is_none()
    );
}

#[test]
fn teleport_ack_transfers_input_to_open_modals_without_clearing_their_block() {
    for owner in ["inventory", "settings", "map", "chat"] {
        let mut app = app();
        app.world_mut().resource_mut::<ClientSession>().teleporting = true;
        app.update();
        assert!(app.world().resource::<ChatState>().teleport_resume_pending);
        match owner {
            "inventory" => {
                app.init_resource::<InventoryState>();
                app.world_mut().resource_mut::<InventoryState>().open = true;
            }
            "settings" => app.world_mut().resource_mut::<crate::SettingsView>().open = true,
            "map" => {
                app.init_resource::<crate::minimap::fullmap::FullMapState>();
                app.world_mut()
                    .resource_mut::<crate::minimap::fullmap::FullMapState>()
                    .open = true;
            }
            "chat" => app.world_mut().resource_mut::<ChatState>().open = true,
            _ => unreachable!(),
        }
        app.world_mut()
            .resource_mut::<Messages<CaptureRequest>>()
            .clear();
        app.world_mut().resource_mut::<ClientSession>().teleporting = false;
        app.update();
        assert!(
            !app.world().resource::<ChatState>().teleport_resume_pending,
            "{owner}"
        );
        assert!(app.world().resource::<PlayerInputBlock>().0, "{owner}");
        assert!(
            app.world_mut()
                .resource_mut::<Messages<CaptureRequest>>()
                .drain()
                .all(|request| !request.0),
            "{owner}"
        );
    }
}

#[test]
fn teleport_handoff_is_discarded_on_session_exit_without_recapturing() {
    let mut app = app();
    app.world_mut().resource_mut::<ClientSession>().teleporting = true;
    app.update();
    app.world_mut().resource_mut::<ClientSession>().ready = false;
    app.world_mut()
        .resource_mut::<Messages<CaptureRequest>>()
        .clear();
    app.update();
    assert!(!app.world().resource::<ChatState>().teleport_resume_pending);
    assert!(
        app.world_mut()
            .resource_mut::<Messages<CaptureRequest>>()
            .drain()
            .all(|request| !request.0)
    );
}

fn key(app: &mut App, key: KeyCode) {
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(key);
    app.update();
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .reset_all();
}

#[test]
fn typing_blocks_gameplay_and_escape_waits_for_release() {
    let mut app = app();
    key(&mut app, KeyCode::KeyT);
    assert!(app.world().resource::<ChatView>().open);
    assert!(app.world().resource::<PlayerInputBlock>().0);
    app.world_mut()
        .write_message(ChatAction::Change("/locate biome desert".into()));
    app.world_mut().write_message(ChatAction::Send);
    app.update();
    assert!(
        matches!(app.world().resource::<ClientChatCommands>().0.front(), Some(ChatCommand::Send(text)) if text == "/locate biome desert")
    );
    assert!(app.world().resource::<ChatView>().open);
    key(&mut app, KeyCode::Escape);
    assert!(!app.world().resource::<ChatView>().open);
    assert!(app.world().resource::<ChatState>().blocks_input());
    assert!(app.world().resource::<PlayerInputBlock>().0);
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .reset_all();
    app.update();
    assert!(!app.world().resource::<PlayerInputBlock>().0);
}

#[test]
fn chat_survives_outside_clicks_editor_blur_and_window_focus_loss_until_escape() {
    let mut app = app();
    app.init_resource::<InputFocus>();
    let editor = app
        .world_mut()
        .spawn((
            ElementId("chat-input".into()),
            tilt_ui::EditableText::new(String::new(), tilt_ui::InputType::Text, false, false),
        ))
        .id();
    let window = app
        .world_mut()
        .spawn((Window::default(), bevy::window::PrimaryWindow))
        .id();
    key(&mut app, KeyCode::KeyT);
    app.world_mut().resource_mut::<ChatState>().focus_pending = false;
    app.world_mut()
        .write_message(ChatAction::Change("unfinished draft".into()));
    app.world_mut()
        .resource_mut::<InputFocus>()
        .set(editor, FocusCause::Navigated);
    app.update();
    app.world_mut().resource_mut::<InputFocus>().clear();
    app.world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .press(MouseButton::Left);
    app.update();
    assert!(app.world().resource::<ChatView>().open);
    assert!(app.world().resource::<PlayerInputBlock>().0);
    app.world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .reset_all();
    app.world_mut().get_mut::<Window>(window).unwrap().focused = false;
    app.update();
    assert!(app.world().resource::<ChatView>().open);
    assert_eq!(app.world().resource::<ChatView>().draft, "unfinished draft");
    app.world_mut().get_mut::<Window>(window).unwrap().focused = true;
    app.update();
    assert!(app.world().resource::<ChatView>().open);
    key(&mut app, KeyCode::Escape);
    assert!(!app.world().resource::<ChatView>().open);
    assert!(
        !include_str!("../../../src-ui/components/chat-panel/chat-panel.component.html")
            .contains("chat-close")
    );
}

#[test]
fn chat_restores_its_editor_focus_after_clicking_outside() {
    let mut app = app();
    app.init_resource::<InputFocus>()
        .add_systems(PostUpdate, focus_and_scroll);
    let editor = app
        .world_mut()
        .spawn((
            ElementId("chat-input".into()),
            tilt_ui::EditableText::new(String::new(), tilt_ui::InputType::Text, false, false),
        ))
        .id();
    key(&mut app, KeyCode::KeyT);
    assert_eq!(app.world().resource::<InputFocus>().get(), Some(editor));
    app.world_mut().resource_mut::<InputFocus>().clear();
    app.update();
    assert!(app.world().resource::<ChatView>().open);
    assert_eq!(app.world().resource::<InputFocus>().get(), Some(editor));
}

#[test]
fn locate_rows_keep_colorable_fields_and_only_received_tokens_can_be_clicked() {
    let mut app = app();
    app.world_mut()
        .resource_mut::<ClientChat>()
        .push(ChatEntry {
            id: 4,
            author: None,
            text: String::new(),
            locate: Some(terra_network::client::LocateResult {
                name: "Desert".into(),
                position: [-420, 38, 817],
                token: Some(9),
            }),
        });
    key(&mut app, KeyCode::Slash);
    let view = app.world().resource::<ChatView>();
    assert_eq!(view.draft, "/");
    assert_eq!(view.rows[0].name, "Desert");
    assert_eq!(view.rows[0].coordinates, "-420, 38, 817");
    app.world_mut().write_message(ChatAction::Teleport(3));
    app.update();
    assert!(app.world().resource::<ClientChatCommands>().0.is_empty());
    app.world_mut().write_message(ChatAction::Teleport(9));
    app.update();
    assert!(matches!(
        app.world().resource::<ClientChatCommands>().0.front(),
        Some(ChatCommand::Teleport(9))
    ));
    assert!(app.world().resource::<ChatView>().open);
}

#[test]
fn chat_does_not_open_over_menus_or_before_a_session_is_ready() {
    let mut app = app();
    app.world_mut().resource_mut::<ClientSession>().ready = false;
    key(&mut app, KeyCode::KeyT);
    assert!(!app.world().resource::<ChatView>().open);
    app.world_mut().resource_mut::<ClientSession>().ready = true;
    app.world_mut().insert_resource(MenuState {
        page: MenuPage::Pause,
        ..default()
    });
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .reset_all();
    key(&mut app, KeyCode::KeyT);
    assert!(!app.world().resource::<ChatView>().open);
}

#[test]
fn raw_chat_text_is_not_html_and_teleport_freezes_input_until_ready() {
    let entry = ChatEntry {
        id: 1,
        author: Some("Player".into()),
        text: "<button>text</button>".into(),
        locate: None,
    };
    let row = ChatRow::from(&entry);
    assert_eq!(row.text, entry.text);
    assert!(!row.can_teleport);
    let mut app = app();
    app.world_mut().resource_mut::<ClientSession>().teleporting = true;
    app.update();
    assert!(app.world().resource::<PlayerInputBlock>().0);
    app.world_mut()
        .resource_mut::<ClientChat>()
        .notice("message");
    app.update();
    assert!(app.world().resource::<ChatView>().visible);
    app.world_mut().resource_mut::<ClientChat>().clear();
    app.update();
    assert!(!app.world().resource::<ChatView>().visible);
}

#[test]
fn chat_and_command_follow_rebound_controls() {
    let mut app = app();
    let mut controls = ControlsConfig::default();
    controls
        .rebind(
            GameAction::Chat,
            terra_core::InputBinding::Key(KeyCode::KeyY),
        )
        .unwrap();
    controls
        .rebind(
            GameAction::Command,
            terra_core::InputBinding::Key(KeyCode::F8),
        )
        .unwrap();
    app.insert_resource(controls);
    key(&mut app, KeyCode::KeyT);
    assert!(!app.world().resource::<ChatView>().open);
    key(&mut app, KeyCode::Slash);
    assert!(!app.world().resource::<ChatView>().open);
    key(&mut app, KeyCode::F8);
    assert_eq!(app.world().resource::<ChatView>().draft, "/");
    assert!(app.world().resource::<PlayerInputBlock>().0);
}

#[test]
fn completion_cycles_both_directions_and_resets_after_editing() {
    let mut state = ChatState {
        draft: "/locate biome ".into(),
        ..default()
    };
    complete(&mut state, false, &[]);
    assert_eq!(state.draft, "/locate biome desert");
    complete(&mut state, false, &[]);
    assert_eq!(state.draft, "/locate biome forest");
    complete(&mut state, true, &[]);
    assert_eq!(state.draft, "/locate biome desert");
    state.draft = "/locate biome m".into();
    complete(&mut state, false, &[]);
    assert_eq!(state.draft, "/locate biome mushroom_park");
    state.draft = "ordinary chat".into();
    complete(&mut state, false, &[]);
    assert_eq!(state.draft, "ordinary chat");
}

#[test]
fn command_arrows_select_suggestions_and_tab_accepts_the_selected_entry() {
    let mut app = app();
    app.world_mut().resource_mut::<ClientChat>().players = vec!["Alice".into(), "Bob".into()];
    key(&mut app, KeyCode::Slash);
    app.world_mut()
        .write_message(ChatAction::Change("/time set ".into()));
    app.update();
    assert_eq!(app.world().resource::<ChatView>().suggestions.len(), 7);
    key(&mut app, KeyCode::ArrowDown);
    key(&mut app, KeyCode::Tab);
    assert_eq!(app.world().resource::<ChatView>().draft, "/time set night");
    key(&mut app, KeyCode::Tab);
    assert_eq!(
        app.world().resource::<ChatView>().draft,
        "/time set morning"
    );
    app.world_mut()
        .write_message(ChatAction::Change("/tp a".into()));
    app.update();
    app.world_mut()
        .write_message(ChatAction::Suggest("/tp Alice".into()));
    app.update();
    assert_eq!(app.world().resource::<ChatView>().draft, "/tp Alice");
    assert!(app.world().resource::<ClientChatCommands>().0.is_empty());
}
