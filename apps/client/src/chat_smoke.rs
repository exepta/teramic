//! Native TiltUI input, real integrated authority and an actual biome teleport.
use bevy::{
    app::AppExit,
    input::{
        ButtonState,
        keyboard::{Key, KeyboardInput},
    },
    input_focus::InputFocus,
    prelude::*,
    render::view::screenshot::{Screenshot, ScreenshotCaptured, save_to_disk},
    window::PrimaryWindow,
};
use std::{
    path::PathBuf,
    time::{Duration, Instant},
};
use terra_network::client::{ClientChat, ClientSession, LocateResult};
use terra_player::{CameraInputState, ControllerMode, PlayerBody, PlayerInputBlock};
use terra_render::RenderStatus;
use terra_ui::{
    ChatAction, ChatView, ElementId, HtmlEvent,
    menu::{MenuAction, MenuPage, MenuState},
};
use terra_world::{
    edits::EditJournal,
    generation::biomes::BiomeKind,
    streaming::{ChunkStore, GenerationStatus, WorldGeneration, WorldTerrain},
};

const COMMAND: &str = "/locate biome desert";

#[derive(Clone, Copy, Debug, PartialEq)]
enum Stage {
    Loading,
    Opening,
    InputGuard,
    Typing,
    Result,
    Wide,
    Resize,
    Narrow,
    CompactCaptured,
    SuggestionClick,
    Escape,
    Resume,
    Reopen,
    Teleport,
    Destination,
    FinalCapture,
    HelpFirst,
    HelpFirstCapture,
    HelpSecond,
    HelpSecondCapture,
    Weather,
    DismissHelp,
    Done,
}

#[derive(Resource)]
struct ChatSmoke {
    started: Instant,
    entered: Instant,
    stage: Stage,
    origin: Vec3,
    edited: usize,
    found: Option<LocateResult>,
    captures: usize,
    capture_error: bool,
    size: (u32, u32),
}

impl ChatSmoke {
    fn advance(&mut self, stage: Stage) {
        self.stage = stage;
        self.entered = Instant::now();
    }
    fn elapsed(&self, seconds: f32) -> bool {
        self.entered.elapsed().as_secs_f32() >= seconds
    }
}

#[derive(Default)]
struct Input {
    chat: bool,
    escape: bool,
    forward: bool,
    mouse: bool,
    down: bool,
    tab: bool,
}

pub(crate) fn install(app: &mut App) {
    if !std::env::args().any(|arg| arg == "--smoke-test-chat") {
        return;
    }
    let now = Instant::now();
    app.insert_resource(ChatSmoke {
        started: now,
        entered: now,
        stage: Stage::Loading,
        origin: Vec3::ZERO,
        edited: 0,
        found: None,
        captures: 0,
        capture_error: false,
        size: (1280, 720),
    })
    .insert_resource(ControllerMode::Walk)
    .insert_resource(bevy::winit::WinitSettings::continuous())
    .add_systems(PreUpdate, run.after(bevy::input::InputSystems));
}

fn run(world: &mut World) {
    world.resource_scope(|world, mut state: Mut<ChatSmoke>| {
        if state.stage == Stage::Done { return; }
        let mut input = Input::default();
        match advance(world, &mut state, &mut input) {
            Ok(true) => {
                tracing::info!(captures = state.captures, "Native chat: TiltUI keyboard, locate, colors, Escape, authoritative teleport/weather and help pagination verified");
                state.stage = Stage::Done;
                world.write_message(AppExit::Success);
            }
            Ok(false) => {}
            Err(error) => {
                tracing::error!(%error, stage = ?state.stage, "Native chat smoke failed");
                state.stage = Stage::Done;
                world.write_message(AppExit::error());
            }
        }
        for (key, pressed) in [(KeyCode::KeyT, input.chat), (KeyCode::Escape, input.escape), (KeyCode::KeyW, input.forward), (KeyCode::ArrowDown, input.down), (KeyCode::Tab, input.tab)] {
            let mut keys = world.resource_mut::<ButtonInput<KeyCode>>();
            if pressed { keys.press(key); } else { keys.release(key); }
        }
        for button in [MouseButton::Left, MouseButton::Right] {
            let mut buttons = world.resource_mut::<ButtonInput<MouseButton>>();
            if input.mouse { buttons.press(button); } else { buttons.release(button); }
        }
        // As in the network fixture, OS grabs are suppressed while gameplay
        // continues to use the production UI lock and authoritative controller.
        world.resource_mut::<CameraInputState>().captured = !world.resource::<PlayerInputBlock>().0;
    });
}

fn advance(world: &mut World, state: &mut ChatSmoke, input: &mut Input) -> Result<bool, String> {
    let timeout = if state.stage == Stage::Loading {
        300
    } else {
        150
    };
    if state.started.elapsed() > Duration::from_secs(timeout) || state.capture_error {
        return Err("Timed out or captured a blank image".into());
    }
    if let Some(error) = world
        .resource::<ClientSession>()
        .error
        .as_ref()
        .or(world.resource::<RenderStatus>().error.as_ref())
    {
        return Err(error.clone());
    }
    match state.stage {
        Stage::Loading => {
            if !world.contains_resource::<terra_server::IntegratedServer>() {
                return Err("Chat smoke requires the real integrated server".into());
            }
            if !world.resource::<ClientSession>().ready || !scene_ready(world) {
                return Ok(false);
            }
            if world.resource::<MenuState>().page != MenuPage::Playing {
                world.write_message(MenuAction::Command("resume".into()));
                return Ok(false);
            }
            let Some(confirmed) = world.resource::<ClientSession>().confirmed_state() else {
                return Ok(false);
            };
            if !confirmed.grounded {
                return Ok(false);
            }
            state.origin = Vec3::from_array(confirmed.position);
            state.edited = world.resource::<EditJournal>().edited_blocks();
            if let Some(window) = world
                .query_filtered::<&Window, With<PrimaryWindow>>()
                .iter(world)
                .next()
            {
                state.size = (window.physical_width(), window.physical_height());
            }
            input.chat = true;
            state.started = Instant::now();
            state.advance(Stage::Opening);
        }
        Stage::Opening if state.elapsed(0.25) => {
            if !world.resource::<ChatView>().open || !world.resource::<PlayerInputBlock>().0 {
                return Err("T did not open chat and block gameplay".into());
            }
            let entity = element(world, "chat-input").ok_or("TiltUI chat editor missing")?;
            if world.resource::<InputFocus>().get() != Some(entity) {
                return Err("Native chat editor did not receive keyboard focus".into());
            }
            world.resource_mut::<InputFocus>().clear();
            input.forward = true;
            input.mouse = true;
            state.advance(Stage::InputGuard);
        }
        Stage::InputGuard => {
            input.forward = !state.elapsed(0.3);
            input.mouse = input.forward;
            if !input.forward {
                let editor =
                    element(world, "chat-input").ok_or("Chat closed after losing focus")?;
                if !world.resource::<ChatView>().open
                    || world.resource::<InputFocus>().get() != Some(editor)
                {
                    return Err(
                        "Chat must stay open and recover editor focus after an outside click"
                            .into(),
                    );
                }
                let current = world
                    .resource::<ClientSession>()
                    .confirmed_state()
                    .ok_or("Missing authoritative player")?;
                let delta = Vec3::from_array(current.position) - state.origin;
                if delta.x.abs() + delta.z.abs() > 0.1
                    || world.resource::<EditJournal>().edited_blocks() != state.edited
                    || world.resource::<ClientSession>().mining.is_some()
                {
                    return Err("Chat leaked movement or block interaction".into());
                }
                type_text(world, COMMAND)?;
                state.advance(Stage::Typing);
            }
        }
        Stage::Typing if state.elapsed(0.4) => {
            let editor = element(world, "chat-input")
                .and_then(|e| world.get::<tilt_ui::EditableText>(e))
                .ok_or("TiltUI chat editor disappeared")?;
            if editor.value != COMMAND || world.resource::<ChatView>().draft != COMMAND {
                return Err(format!(
                    "Keyboard text did not reach TiltUI binding: {:?} / {:?}",
                    editor.value,
                    world.resource::<ChatView>().draft
                ));
            }
            world.write_message(ChatAction::Send);
            state.advance(Stage::Result);
        }
        Stage::Result => {
            let found = world
                .resource::<ClientChat>()
                .entries
                .iter()
                .find_map(|entry| entry.locate.clone());
            if let Some(found) = found {
                if found.name != "Desert" || found.token.is_none() {
                    return Err(format!(
                        "Expected a real teleportable Desert result: {found:?}"
                    ));
                }
                let terrain = &world.resource::<WorldTerrain>().0;
                let biome = terrain.surface_biome(&terrain.surface_column(
                    found.position[0] as f64 + 0.5,
                    found.position[2] as f64 + 0.5,
                ));
                if biome != BiomeKind::Desert {
                    return Err("Locate result does not match generated biome".into());
                }
                state.found = Some(found);
                let name = world.resource::<terra_core::PlayerProfile>().name.clone();
                world.write_message(ChatAction::Change(format!("Hallo @{name}!")));
                world.write_message(ChatAction::Send);
                world.write_message(ChatAction::Change("/time set ".into()));
                state.advance(Stage::Wide);
            } else if state.elapsed(20.0) {
                return Err(format!(
                    "Server locate did not return a result: {:?}",
                    world.resource::<ClientChat>().entries
                ));
            }
        }
        Stage::Wide if state.elapsed(0.5) => {
            require_text_colors(world, state.found.as_ref())?;
            let suggestions = world
                .query::<(&ElementId, &ComputedNode)>()
                .iter(world)
                .filter(|(id, node)| id.0 == "chat-suggestion" && node.size().y > 0.0)
                .count();
            if suggestions != 7 || !world.resource::<ChatView>().has_suggestions {
                return Err(format!(
                    "Expected seven rendered time suggestions, got {suggestions}"
                ));
            }
            capture(world, "chat-locate-wide")?;
            state.advance(Stage::Resize);
        }
        Stage::Resize if state.captures >= 1 && state.elapsed(0.4) => {
            resize(world, (960, 640));
            input.down = true;
            input.tab = true;
            state.advance(Stage::Narrow);
        }
        Stage::Narrow if state.elapsed(0.7) => {
            require_text_colors(world, state.found.as_ref())?;
            if world.resource::<ChatView>().draft != "/time set night" {
                return Err("Arrow/Tab did not complete the selected time suggestion".into());
            }
            capture(world, "chat-locate-compact")?;
            state.advance(Stage::CompactCaptured);
        }
        Stage::CompactCaptured if state.captures >= 2 => {
            click(world, "chat-suggestion")?;
            state.advance(Stage::SuggestionClick);
        }
        Stage::SuggestionClick if state.elapsed(0.2) => {
            if world.resource::<ChatView>().draft != "/time set day" {
                return Err(format!(
                    "TiltUI suggestion click did not fill the editor: {:?}",
                    world.resource::<ChatView>().draft
                ));
            }
            input.escape = true;
            state.advance(Stage::Escape);
        }
        Stage::Escape => {
            input.escape = true;
            if state.elapsed(0.2) {
                if world.resource::<ChatView>().open
                    || world.resource::<MenuState>().page != MenuPage::Playing
                    || world.resource::<terra_ui::SettingsView>().open
                {
                    return Err("Escape did not close only the chat".into());
                }
                if !world.resource::<PlayerInputBlock>().0 {
                    return Err("Escape was not guarded until release".into());
                }
                input.escape = false;
                resize(world, state.size);
                state.advance(Stage::Resume);
            }
        }
        Stage::Resume if state.elapsed(0.3) => {
            if world.resource::<PlayerInputBlock>().0 {
                return Err("Chat closing left gameplay permanently blocked".into());
            }
            input.chat = true;
            state.advance(Stage::Reopen);
        }
        Stage::Reopen if state.elapsed(0.4) => {
            if !world.resource::<ChatView>().open {
                return Err("Chat did not reopen for teleport".into());
            }
            click(world, "chat-teleport")?;
            state.advance(Stage::Teleport);
        }
        Stage::Teleport => {
            let session = world.resource::<ClientSession>();
            if session.teleporting && !world.resource::<PlayerInputBlock>().0 && state.elapsed(0.2)
            {
                return Err("Teleport loading did not block input".into());
            }
            if let Some(confirmed) = session.confirmed_state()
                && confirmed.generation == 1
                && !session.teleporting
                && scene_ready(world)
            {
                let found = state.found.as_ref().ok_or("Missing locate result")?;
                let expected = Vec3::new(
                    found.position[0] as f32 + 0.5,
                    found.position[1] as f32,
                    found.position[2] as f32 + 0.5,
                );
                let actual = Vec3::from_array(confirmed.position);
                if (actual.x - expected.x).abs() > 0.2
                    || (actual.z - expected.z).abs() > 0.2
                    || (actual.y - expected.y).abs() > 1.1
                {
                    return Err("Authoritative teleport did not reach the located position".into());
                }
                let body = world
                    .query_filtered::<&Transform, With<PlayerBody>>()
                    .iter(world)
                    .next()
                    .ok_or("Missing local player body")?
                    .translation;
                if body.distance(actual) > 0.2 {
                    return Err("Predicted body did not reconcile to teleport".into());
                }
                let ground = [found.position[0], found.position[1] - 1, found.position[2]];
                if world
                    .resource::<ChunkStore>()
                    .block_at(ground)
                    .is_none_or(|m| m == 0)
                {
                    return Err("Teleport destination was not loaded and supported".into());
                }
                tracing::info!(from = ?state.origin, to = ?actual, generation = confirmed.generation, "Native chat teleport confirmed by server");
                if !world.resource::<ChatView>().open {
                    return Err("Teleport closed chat without Escape".into());
                }
                state.advance(Stage::Destination);
            } else if state.elapsed(40.0) {
                return Err(format!(
                    "Teleport did not finish: {:?}",
                    world.resource::<ClientChat>().entries
                ));
            }
        }
        Stage::Destination if state.elapsed(1.0) => {
            capture(world, "chat-desert-destination")?;
            state.advance(Stage::FinalCapture);
        }
        Stage::FinalCapture if state.captures >= 3 => {
            world.write_message(ChatAction::Change("/help".into()));
            world.write_message(ChatAction::Send);
            state.advance(Stage::HelpFirst);
        }
        Stage::HelpFirst if state.elapsed(0.5) => {
            require_help_page(world, 1)?;
            capture(world, "chat-help-page1")?;
            state.advance(Stage::HelpFirstCapture);
        }
        Stage::HelpFirstCapture if state.captures >= 4 => {
            click(world, "chat-help-next")?;
            state.advance(Stage::HelpSecond);
        }
        Stage::HelpSecond if state.elapsed(0.5) => {
            require_help_page(world, 2)?;
            capture(world, "chat-help-page2")?;
            state.advance(Stage::HelpSecondCapture);
        }
        Stage::HelpSecondCapture if state.captures >= 5 => {
            click(world, "chat-help-previous")?;
            world.write_message(ChatAction::Change("/weather set storm".into()));
            world.write_message(ChatAction::Send);
            state.advance(Stage::Weather);
        }
        Stage::Weather if state.elapsed(0.5) => {
            require_help_page(world, 1)?;
            if world.resource::<terra_core::WorldWeather>().pattern
                == terra_core::WeatherPattern::Storm
                && world
                    .resource::<ClientChat>()
                    .entries
                    .iter()
                    .any(|entry| entry.text == "Weather changing to storm.")
            {
                world.write_message(ChatAction::Change("/weather set clear".into()));
                world.write_message(ChatAction::Send);
                click(world, "chat-help-dismiss")?;
                state.advance(Stage::DismissHelp);
                return Ok(false);
            }
            if state.elapsed(10.0) {
                return Err("Native /weather set did not reach real server authority".into());
            }
        }
        Stage::DismissHelp if state.elapsed(0.4) => {
            let view = world.resource::<ChatView>();
            if !view.open || view.help || view.rows.is_empty() {
                return Err("Dismissing help should leave chat and message history visible".into());
            }
            return Ok(true);
        }
        _ => {}
    }
    Ok(false)
}

fn scene_ready(world: &World) -> bool {
    world.resource::<WorldGeneration>().status == GenerationStatus::Ready
        && world.resource::<RenderStatus>().ready
}

fn element(world: &mut World, id: &str) -> Option<Entity> {
    world
        .query::<(Entity, &ElementId)>()
        .iter(world)
        .find(|(_, element)| element.0 == id)
        .map(|(entity, _)| entity)
}

fn type_text(world: &mut World, value: &str) -> Result<(), String> {
    let window = world
        .query_filtered::<Entity, With<PrimaryWindow>>()
        .iter(world)
        .next()
        .ok_or("Missing native window")?;
    for c in value.chars() {
        for state in [ButtonState::Pressed, ButtonState::Released] {
            world.write_message(KeyboardInput {
                key_code: KeyCode::KeyA,
                logical_key: Key::Character(c.to_string().into()),
                state,
                text: (state == ButtonState::Pressed).then(|| c.to_string().into()),
                repeat: false,
                window,
            });
        }
    }
    Ok(())
}

fn click(world: &mut World, id: &str) -> Result<(), String> {
    // @for rows can have any ECS iteration order; select the requested visible text.
    let entity = world
        .query::<(Entity, &ElementId)>()
        .iter(world)
        .find(|(entity, element)| {
            element.0 == id
                && (id != "chat-suggestion" || contains_text(world, *entity, "/time set day"))
        })
        .map(|(entity, _)| entity)
        .ok_or_else(|| format!("TiltUI control missing: {id}"))?;
    let handler = world
        .get::<tilt_ui::StaticAttributes>(entity)
        .and_then(|attributes| attributes.attributes.iter().find(|a| a.name == "onclick"))
        .map(|a| a.value.clone())
        .ok_or("TiltUI click handler missing")?;
    world.write_message(HtmlEvent {
        target: entity,
        kind: "click",
        value: None,
        submitter: None,
        data: default(),
        form_data: None,
        handler,
    });
    Ok(())
}

fn contains_text(world: &World, entity: Entity, expected: &str) -> bool {
    world
        .get::<Text>(entity)
        .is_some_and(|text| text.0 == expected)
        || world.get::<Children>(entity).is_some_and(|children| {
            children
                .iter()
                .any(|child| contains_text(world, child, expected))
        })
}

fn require_help_page(world: &mut World, page: usize) -> Result<(), String> {
    use terra_network::protocol::commands::{HELP_PAGE_SIZE, help_page, help_pages};
    let expected = help_page(page);
    let pages = help_pages();
    let label = format!("{page} / {pages}");
    let view = world.resource::<ChatView>();
    if expected.is_empty()
        || expected.len() > HELP_PAGE_SIZE
        || !view.open
        || !view.help
        || view.help_page != label
        || view.help_entries.len() != expected.len()
        || view.has_help_previous != (page > 1)
        || view.has_help_next != (page < pages)
    {
        return Err(format!(
            "Help state differs from command page {label}: actual_page={:?}, rows={}, expected_rows={}, previous={}, next={}",
            view.help_page,
            view.help_entries.len(),
            expected.len(),
            view.has_help_previous,
            view.has_help_next,
        ));
    }
    let rows = world
        .query::<(Entity, &tilt_ui::ElementClasses, &ComputedNode)>()
        .iter(world)
        .filter_map(|(entity, classes, node)| {
            (classes
                .classes
                .iter()
                .any(|class| class == "chat-help-entry")
                && !node.is_empty())
            .then_some(entity)
        })
        .collect::<Vec<_>>();
    if rows.len() != expected.len() {
        return Err(format!(
            "Help page {label} rendered {} rows, expected {}",
            rows.len(),
            expected.len()
        ));
    }
    for entry in expected {
        let usage = terra_ui::localized_text(world, entry.usage);
        let description = terra_ui::localized_text(world, entry.description);
        if !rows.iter().any(|row| {
            contains_text(world, *row, &usage) && contains_text(world, *row, &description)
        }) {
            return Err(format!(
                "Help page {label} did not render its command and description: {}",
                entry.usage
            ));
        }
    }
    for (id, disabled) in [
        ("chat-help-previous", page == 1),
        ("chat-help-next", page == pages),
    ] {
        let button = element(world, id).ok_or_else(|| format!("Missing help navigation {id}"))?;
        if world.get::<bevy::ui::InteractionDisabled>(button).is_some() != disabled {
            return Err(format!(
                "Help page {label} has incorrect navigation state for {id}"
            ));
        }
    }
    Ok(())
}

fn require_text_colors(world: &mut World, found: Option<&LocateResult>) -> Result<(), String> {
    let [x, y, z] = found.ok_or("Missing locate result")?.position;
    for (text, expected) in [
        (
            terra_ui::localized_text(world, "Desert"),
            [242.0, 223.0, 196.0],
        ),
        (format!("{x}, {y}, {z}"), [242.0, 223.0, 196.0]),
        (
            terra_ui::localized_text(world, "[Click Me]"),
            [245.0, 211.0, 110.0],
        ),
    ] {
        let color = world
            .query::<(&Text, &TextColor)>()
            .iter(world)
            .find(|(value, _)| value.0 == text)
            .map(|(_, color)| color.0.to_srgba())
            .ok_or_else(|| format!("Visible chat text missing: {text}"))?;
        if [color.red, color.green, color.blue]
            .into_iter()
            .zip(expected)
            .any(|(actual, expected)| (actual * 255.0 - expected).abs() > 3.0)
            || color.alpha < 0.99
        {
            return Err(format!(
                "Chat text color is incorrect for {text}: {color:?}"
            ));
        }
    }
    Ok(())
}

fn resize(world: &mut World, size: (u32, u32)) {
    for mut window in world
        .query_filtered::<&mut Window, With<PrimaryWindow>>()
        .iter_mut(world)
    {
        window.set_cursor_position(None);
        window.resolution.set_physical_resolution(size.0, size.1);
    }
}

fn capture(world: &mut World, name: &str) -> Result<(), String> {
    let directory = std::env::var_os("TERRA_CAPTURE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::temp_dir().join("terra-chat"));
    std::fs::create_dir_all(&directory).map_err(|error| error.to_string())?;
    world
        .spawn(Screenshot::primary_window())
        .observe(save_to_disk(directory.join(format!("{name}.png"))))
        .observe(
            |event: On<ScreenshotCaptured>, mut state: ResMut<ChatSmoke>| {
                let distinct: std::collections::HashSet<_> = event
                    .image
                    .data
                    .as_deref()
                    .unwrap_or_default()
                    .chunks_exact(4)
                    .step_by(101)
                    .take(4096)
                    .map(|rgba| [rgba[0], rgba[1], rgba[2]])
                    .collect();
                state.capture_error |= distinct.len() < 16;
                state.captures += 1;
            },
        );
    Ok(())
}
