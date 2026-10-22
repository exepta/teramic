//! Explicit fixture driver: real TiltUI handlers, files and production session lifecycle.
use bevy::{
    app::AppExit,
    input::{
        ButtonState,
        keyboard::{Key, KeyboardInput},
    },
    prelude::*,
    window::PrimaryWindow,
};
use std::{
    path::PathBuf,
    time::{Duration, Instant},
};
use terra_core::{
    AudioConfig, Config, ControlsConfig, GameAction, GameplayConfig, GraphicsConfig, InputBinding,
    MapMode,
};
use terra_ui::{ElementId, HtmlEvent, UiDocumentState, UiLoadState, menu::*};

#[derive(Resource)]
struct MenuSmoke {
    started: Instant,
    advanced: Instant,
    stage: u16,
    output: PathBuf,
    session: bool,
    join: bool,
    lan: bool,
    pointer: bool,
    gameplay_only: bool,
    origin: Vec3,
    edit_count: usize,
    loading_captured: bool,
}
pub(crate) fn install(app: &mut App) {
    if !std::env::args().any(|a| a.starts_with("--smoke-test-menu")) {
        return;
    }
    let root = std::env::var_os("TERRA_MENU_SMOKE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            std::env::temp_dir().join(format!("teramic-menu-smoke-{}", std::process::id()))
        });
    let output = std::env::var_os("TERRA_MENU_SCREENSHOTS")
        .map(PathBuf::from)
        .unwrap_or_else(|| root.join("screenshots"));
    if let Err(error) = std::fs::create_dir_all(&output) {
        tracing::error!(%error,"Cannot create menu smoke output");
    }
    if std::env::args().any(|a| a == "--smoke-test-menu-small") {
        let graphics = GraphicsConfig {
            width: 960,
            height: 640,
            fps_limit: 75,
            ..default()
        };
        if let Err(error) = graphics.save(root.join("configs/graphics.toml")) {
            tracing::error!(%error, "Cannot prepare small-window menu fixture");
        }
    }
    // The fixture emits UI events without native window events. Keep its
    // reconciliation frames running even when the desktop gives another window focus.
    app.insert_resource(bevy::winit::WinitSettings::continuous())
        .insert_resource(MenuPaths {
            configs: root.join("configs"),
            worlds: terra_storage::WorldLibrary {
                root: root.join("worlds"),
            },
        })
        .insert_resource(MenuSmoke {
            started: Instant::now(),
            advanced: Instant::now(),
            stage: 0,
            output,
            session: std::env::args().any(|a| a == "--smoke-test-menu-session"),
            join: std::env::args().any(|a| a == "--smoke-test-menu-join"),
            lan: std::env::args().any(|a| a == "--smoke-test-menu-lan"),
            pointer: std::env::args().any(|a| a == "--smoke-test-menu-pointer"),
            gameplay_only: std::env::args().any(|a| a == "--smoke-test-menu-gameplay"),
            origin: Vec3::ZERO,
            edit_count: 0,
            loading_captured: false,
        })
        // ComputedNode assertions need this frame's PostUpdate layout.
        .add_systems(Last, run);
}
fn emit(world: &mut World, id: &str, value: Option<&str>) -> Result<(), String> {
    let direct = world
        .query::<(Entity, &ElementId)>()
        .iter(world)
        .find_map(|(entity, element)| (element.0 == id).then_some(entity));
    let entity = if let Some(entity) = direct {
        entity
    } else {
        let mut buttons = world
            .resource::<MenuView>()
            .buttons
            .iter()
            .chain(&world.resource::<WorldsView>().buttons)
            .chain(&world.resource::<ServersView>().buttons)
            .chain(&world.resource::<OptionsView>().buttons)
            .chain(&world.resource::<DialogView>().buttons);
        let button = buttons.find(|b| b.id == id).map(|b| b.title.clone());
        let row = world
            .resource::<OptionsView>()
            .rows
            .iter()
            .chain(&world.resource::<DialogView>().rows)
            .find(|r| r.id == id);
        let (element_id, title, scope) = if let Some(title) = button {
            ("menu-action".to_string(), title, false)
        } else if let Some(row) = row {
            (format!("row-{}", row.kind), row.title.clone(), true)
        } else if let Some(card) = world
            .resource::<WorldsView>()
            .cards
            .iter()
            .find(|c| c.id == id)
        {
            ("world-card".into(), card.title.clone(), false)
        } else if let Some(card) = world
            .resource::<ServersView>()
            .cards
            .iter()
            .find(|c| c.id == id)
        {
            ("server-card".into(), card.title.clone(), false)
        } else {
            return Err(format!("Unknown menu control: {id}"));
        };
        let candidates: Vec<_> = world
            .query::<(Entity, &ElementId)>()
            .iter(world)
            .filter(|(_, element)| element.0 == element_id)
            .map(|(entity, _)| entity)
            .collect();
        let in_dialog = world.resource::<DialogView>().visible;
        candidates
            .into_iter()
            .filter(|&entity| !in_dialog || has_ancestor_class(world, entity, "modal-panel"))
            .filter_map(|entity| {
                if scope {
                    scope_distance(world, entity, &title).map(|distance| (distance, entity))
                } else {
                    subtree_has_text(world, entity, &title, 0).then_some((0, entity))
                }
            })
            .min_by_key(|(distance, _)| *distance)
            .map(|(_, entity)| entity)
            .ok_or_else(|| format!("TiltUI control missing: {id} ({title})"))?
    };
    let attribute = if value.is_some() {
        "onchange"
    } else {
        "onclick"
    };
    let handler = world
        .get::<tilt_ui::StaticAttributes>(entity)
        .and_then(|a| a.attributes.iter().find(|a| a.name == attribute))
        .map(|a| a.value.clone())
        .ok_or_else(|| format!("Missing handler on {id}"))?;
    world.write_message(HtmlEvent {
        target: entity,
        kind: if value.is_some() { "change" } else { "click" },
        value: value.map(str::to_owned),
        submitter: None,
        data: Default::default(),
        form_data: None,
        handler,
    });
    Ok(())
}
fn has_ancestor_class(world: &World, mut entity: Entity, class: &str) -> bool {
    for _ in 0..24 {
        if world
            .get::<tilt_ui::ElementClasses>(entity)
            .is_some_and(|classes| classes.classes.iter().any(|value| value == class))
        {
            return true;
        }
        let Some(parent) = world.get::<ChildOf>(entity) else {
            return false;
        };
        entity = parent.parent();
    }
    false
}
fn has_ancestor_entity(world: &World, mut entity: Entity, ancestor: Entity) -> bool {
    for _ in 0..24 {
        if entity == ancestor {
            return true;
        }
        let Some(parent) = world.get::<ChildOf>(entity) else {
            return false;
        };
        entity = parent.parent();
    }
    false
}
fn subtree_has_text(world: &World, entity: Entity, title: &str, depth: usize) -> bool {
    if depth > 16 {
        return false;
    }
    world
        .get::<bevy::ui::widget::Text>(entity)
        .is_some_and(|text| text.0 == title)
        || world.get::<Children>(entity).is_some_and(|children| {
            children
                .iter()
                .any(|child| subtree_has_text(world, child, title, depth + 1))
        })
}
fn scope_distance(world: &World, mut entity: Entity, title: &str) -> Option<usize> {
    for distance in 0..8 {
        if subtree_has_text(world, entity, title, 0) {
            return Some(distance);
        }
        entity = world.get::<ChildOf>(entity)?.parent();
    }
    None
}
fn screenshot(world: &mut World, state: &MenuSmoke, name: &str) {
    world
        .spawn(bevy::render::view::screenshot::Screenshot::primary_window())
        .observe(bevy::render::view::screenshot::save_to_disk(
            state.output.join(format!("{name}.png")),
        ));
}
fn key(world: &mut World, key: KeyCode, pressed: bool) {
    if let Some(window) = world
        .query_filtered::<Entity, With<PrimaryWindow>>()
        .iter(world)
        .next()
    {
        world.write_message(KeyboardInput {
            key_code: key,
            logical_key: Key::Character("test".into()),
            state: if pressed {
                ButtonState::Pressed
            } else {
                ButtonState::Released
            },
            text: None,
            repeat: false,
            window,
        });
    }
}
fn pointer(world: &mut World, id: &str, title: &str, pressed: bool) -> Result<(), String> {
    use bevy::{
        camera::RenderTarget,
        picking::pointer::{
            Location, PointerAction, PointerButton, PointerId, PointerInput, PointerLocation,
        },
        window::WindowRef,
    };
    let window = world
        .query_filtered::<Entity, With<PrimaryWindow>>()
        .iter(world)
        .next()
        .ok_or("No primary window")?;
    let location = if pressed {
        let target = world
            .query::<(Entity, &ElementId)>()
            .iter(world)
            .find_map(|(entity, element)| {
                (element.0 == id && (title.is_empty() || subtree_has_text(world, entity, title, 0)))
                    .then_some(entity)
            })
            .ok_or_else(|| format!("Pointer target missing: {id}/{title}"))?;
        let position = world
            .get::<bevy::ui::UiGlobalTransform>(target)
            .ok_or("Pointer target has no layout")?
            .translation;
        let viewport = world.get::<Window>(window).ok_or("No window")?;
        let position = position / viewport.resolution.scale_factor();
        Location {
            target: RenderTarget::Window(WindowRef::Entity(window))
                .normalize(Some(window))
                .ok_or("No render target")?,
            position,
        }
    } else {
        world
            .query::<(&PointerId, &PointerLocation)>()
            .iter(world)
            .find_map(|(id, location)| {
                (*id == PointerId::Mouse)
                    .then(|| location.location.clone())
                    .flatten()
            })
            .ok_or("Mouse pointer location missing")?
    };
    // Exercise real hit testing and TiltUI click propagation without an OS cursor warp.
    if pressed {
        world.write_message(PointerInput::new(
            PointerId::Mouse,
            location.clone(),
            PointerAction::Move { delta: Vec2::ZERO },
        ));
    }
    world.write_message(PointerInput::new(
        PointerId::Mouse,
        location,
        if pressed {
            PointerAction::Press(PointerButton::Primary)
        } else {
            PointerAction::Release(PointerButton::Primary)
        },
    ));
    Ok(())
}
fn require(condition: bool, message: &str) -> Result<(), String> {
    if condition {
        Ok(())
    } else {
        Err(message.into())
    }
}

fn validate_minimap_popup(world: &mut World) -> Result<(), String> {
    let viewport = world
        .query_filtered::<&Window, With<PrimaryWindow>>()
        .iter(world)
        .next()
        .ok_or("Primary window missing")?;
    let viewport = Vec2::new(
        viewport.resolution.physical_width() as f32,
        viewport.resolution.physical_height() as f32,
    );
    let panel = world
        .query::<(Entity, &tilt_ui::ElementClasses)>()
        .iter(world)
        .find(|(_, classes)| {
            classes
                .classes
                .iter()
                .any(|class| class == "minimap-options")
        })
        .map(|(entity, _)| entity)
        .ok_or("MiniMap settings panel missing")?;
    let panel_node = world
        .get::<ComputedNode>(panel)
        .ok_or("MiniMap panel has no layout")?;
    let panel_center = world
        .get::<bevy::ui::UiGlobalTransform>(panel)
        .ok_or("MiniMap panel has no transform")?
        .translation;
    let panel_min = panel_center - panel_node.size() * 0.5;
    let panel_max = panel_center + panel_node.size() * 0.5;
    require(
        panel_min.min_element() >= -1.0
            && panel_max.x <= viewport.x + 1.0
            && panel_max.y <= viewport.y + 1.0,
        "MiniMap settings popup extends beyond the viewport",
    )?;
    let done = world
        .query::<(Entity, &ElementId)>()
        .iter(world)
        .find(|(entity, id)| {
            id.0 == "menu-action"
                && has_ancestor_class(world, *entity, "minimap-options")
                && subtree_has_text(world, *entity, "Done", 0)
        })
        .map(|(entity, _)| entity)
        .ok_or("MiniMap popup Done button missing")?;
    let done_node = world
        .get::<ComputedNode>(done)
        .ok_or("MiniMap Done has no layout")?;
    let done_center = world
        .get::<bevy::ui::UiGlobalTransform>(done)
        .ok_or("MiniMap Done has no transform")?
        .translation;
    require(
        done_node.size().min_element() > 0.0
            && done_center.y + done_node.size().y * 0.5 <= panel_max.y + 1.0,
        "MiniMap popup footer is clipped",
    )
}
fn run(world: &mut World) {
    let Some(mut state) = world.remove_resource::<MenuSmoke>() else {
        return;
    };
    let result = if state.started.elapsed() > Duration::from_secs(150) {
        Err(format!("Menu smoke timed out at stage {}", state.stage))
    } else if state.advanced.elapsed() < Duration::from_millis(450) {
        Ok(false)
    } else {
        step(world, &mut state)
    };
    match result {
        Ok(true) => {
            state.stage += 1;
            state.advanced = Instant::now();
            tracing::info!(stage = state.stage, "Menu smoke advanced");
        }
        Ok(false) => {}
        Err(error) => {
            tracing::error!(%error,stage=state.stage,"Menu smoke failed");
            world.write_message(AppExit::error());
            state.stage = u16::MAX;
        }
    }
    world.insert_resource(state);
}
fn step(world: &mut World, state: &mut MenuSmoke) -> Result<bool, String> {
    match state.stage {
        0 => {
            if state.started.elapsed() < Duration::from_secs(2) {
                return Ok(false);
            }
            if !matches!(
                world.resource::<UiDocumentState>().0.load,
                UiLoadState::Ready
            ) {
                return Ok(false);
            }
            if !world
                .query::<&ElementId>()
                .iter(world)
                .any(|element| element.0 == "menu-action")
            {
                return Ok(false);
            }
            require(
                world.resource::<MenuState>().page == MenuPage::Main,
                "Main menu must be first",
            )?;
            require(
                world
                    .resource::<terra_world::streaming::ChunkStore>()
                    .loaded
                    .is_empty(),
                "Main menu must not generate terrain",
            )?;
            require(
                !world.contains_resource::<terra_server::IntegratedServer>(),
                "Main menu started a server",
            )?;
            screenshot(world, state, "main-menu");
            if state.lan {
                emit(world, "multiplayer", None)?;
                state.stage = 399;
                return Ok(true);
            }
            if state.pointer {
                pointer(world, "menu-action", "Singleplayer", true)?;
                state.stage = 999;
                return Ok(true);
            }
            if state.gameplay_only {
                emit(world, "options", None)?;
                state.stage = 689;
                return Ok(true);
            }
            emit(world, "singleplayer", None)?;
        }
        1 => {
            require(
                world.resource::<WorldMenu>().entries.is_empty(),
                "Smoke directory must be fresh",
            )?;
            screenshot(world, state, "worlds-empty");
            emit(world, "world-create", None)?;
        }
        2 => {
            emit(world, "world-name", Some("Cedar Valley"))?;
            emit(world, "world-seed", Some("42042"))?;
            pointer(world, "world-mode", "", true)?;
            state.stage = 299;
        }
        3 => {
            screenshot(world, state, "create-world");
            emit(world, "world-create-confirm", None)?;
        }
        4 => {
            require(
                world.resource::<WorldMenu>().entries.len() == 1,
                "Create World did not persist a real entry",
            )?;
            if world.resource::<DialogView>().visible {
                return Ok(false);
            }
            screenshot(world, state, "world-selected");
            emit(world, "back", None)?;
        }
        5 => {
            emit(world, "options", None)?;
        }
        6 => {
            let resolutions = world
                .resource::<OptionsView>()
                .rows
                .iter()
                .find(|row| row.id == "graphics-resolution")
                .ok_or("Resolution setting missing")?
                .choices
                .iter()
                .map(|choice| choice.value.clone())
                .collect::<Vec<_>>();
            let rendered = world
                .query::<&tilt_ui::OptionData>()
                .iter(world)
                .map(|option| option.value.clone())
                .collect::<Vec<_>>();
            require(
                resolutions.len() > 1 && resolutions.iter().all(|value| rendered.contains(value)),
                "Monitor resolutions did not materialize as real TiltUI dropdown options",
            )?;
            require(
                !world
                    .query::<&tilt_ui::OptionData>()
                    .iter(world)
                    .any(|option| option.label.contains("{{")),
                "Dropdown label contains unresolved template interpolation",
            )?;
            screenshot(world, state, "graphics");
            require(
                world
                    .resource::<OptionsView>()
                    .rows
                    .iter()
                    .any(|r| r.id == "graphics-ui-fps" && r.value == "60"),
                "UI FPS control missing or wrong default",
            )?;
            require(
                ["24", "30", "40", "60", "90", "120", "144", "240"]
                    .iter()
                    .all(|v| rendered.iter().any(|actual| actual == v)),
                "UI FPS choices did not materialize in TiltUI",
            )?;
            emit(world, "graphics-ui-fps", Some("120"))?;
            emit(world, "graphics-block-detail", Some("medium"))?;
            emit(world, "graphics-fov", Some("86"))?;
            emit(world, "graphics-shadow", Some("low"))?;
            emit(world, "graphics-fps", Some("60"))?;
            emit(world, "graphics-fog", Some("85"))?;
            emit(world, "graphics-chunk-animation", Some("fade"))?;
            emit(world, "graphics-scale", Some("90"))?;
        }
        7 => {
            let viewport = world
                .query_filtered::<&Window, With<PrimaryWindow>>()
                .iter(world)
                .next()
                .ok_or("Primary window missing")?
                .size();
            let expected_scale = (world.resource::<GraphicsConfig>().ui_scale
                * 1.0_f32.min(viewport.x / 1000.0).min(viewport.y / 740.0))
            .min(viewport.x / 900.0)
            .min(viewport.y / 640.0)
            .max(0.1);
            require(
                (world.resource::<bevy::ui::UiScale>().0 - expected_scale).abs() < 0.001,
                "UI Scale changed before Apply",
            )?;
            screenshot(world, state, "graphics-ui-scale");
            pointer(world, "row-select", "1280 x 800", true)?;
            state.stage = 599;
        }
        600 => {
            pointer(world, "row-select", "1280 x 800", false)?;
        }
        601 => {
            require(
                world
                    .query::<&tilt_ui::ChoiceBoxParts>()
                    .iter(world)
                    .any(|choice| choice.open),
                "Resolution popup did not remain open",
            )?;
            screenshot(world, state, "resolution-dropdown");
            emit(world, "apply", None)?;
            state.stage = 7;
        }
        8 => {
            require(
                world
                    .resource::<terra_core::GameConfig>()
                    .client
                    .fov_degrees
                    == 86.0,
                "Apply did not affect actual camera configuration",
            )?;
            let path = world.resource::<MenuPaths>().configs.join("graphics.toml");
            let saved = GraphicsConfig::load(path).map_err(|e| e.to_string())?;
            require(
                saved.fov == 86.0
                    && (saved.ui_scale - 0.9).abs() < 0.001
                    && saved.ui_fps == 120
                    && saved.blocks.detail_quality == terra_core::BlockDetailQuality::Medium,
                "Graphics TOML did not reload",
            )?;
            let viewport = world
                .query_filtered::<&Window, With<PrimaryWindow>>()
                .iter(world)
                .next()
                .ok_or("Primary window missing")?
                .size();
            let applied_scale = (saved.ui_scale
                * 1.0_f32.min(viewport.x / 1000.0).min(viewport.y / 740.0))
            .min(viewport.x / 900.0)
            .min(viewport.y / 640.0)
            .max(0.1);
            require(
                (world.resource::<bevy::ui::UiScale>().0 - applied_scale).abs() < 0.001,
                "Apply did not change actual UI Scale",
            )?;
            screenshot(world, state, "graphics-ui-scale-applied");
            emit(world, "controls", None)?;
        }
        9 => {
            screenshot(world, state, "controls");
            require(
                !world.resource::<ControlsConfig>().auto_climb,
                "Auto-Climb must default to off",
            )?;
            emit(world, "controls-auto-climb", Some("true"))?;
            emit(world, "bind-0", None)?;
        }
        10 => {
            require(
                world.resource::<OptionsDraft>().rebinding == Some(GameAction::Forward),
                "Binding field did not arm",
            )?;
            key(world, KeyCode::ArrowUp, true);
        }
        11 => {
            key(world, KeyCode::ArrowUp, false);
            require(
                world
                    .resource::<OptionsDraft>()
                    .controls
                    .binding(GameAction::Forward)
                    == InputBinding::Key(KeyCode::ArrowUp),
                "Next key was not captured",
            )?;
            emit(world, "apply", None)?;
        }
        12 => {
            require(
                world.resource::<ControlsConfig>().auto_climb,
                "Auto-Climb switch was not applied",
            )?;
            let controls_path = world.resource::<MenuPaths>().configs.join("controls.toml");
            require(
                ControlsConfig::load(controls_path)
                    .map_err(|e| e.to_string())?
                    .auto_climb,
                "Auto-Climb was not persisted",
            )?;
            require(
                world
                    .resource::<ControlsConfig>()
                    .binding(GameAction::Forward)
                    == InputBinding::Key(KeyCode::ArrowUp),
                "Gameplay bindings were not applied",
            )?;
            emit(world, "bind-1", None)?;
        }
        13 => {
            key(world, KeyCode::ArrowUp, true);
        }
        14 => {
            key(world, KeyCode::ArrowUp, false);
            require(
                world.resource::<MenuState>().error.contains("already"),
                "Binding conflict was not reported",
            )?;
            screenshot(world, state, "binding-conflict");
            key(world, KeyCode::Escape, true);
        }
        15 => {
            key(world, KeyCode::Escape, false);
            require(
                world.resource::<MenuState>().page == MenuPage::Options
                    && world.resource::<OptionsDraft>().rebinding.is_none(),
                "Escape closed menu while rebinding",
            )?;
            emit(world, "controls-auto-climb", Some("false"))?;
            emit(world, "apply", None)?;
            emit(world, "audio", None)?;
            state.stage = 499;
        }
        16 => {
            emit(world, "graphics-brightness", Some("110"))?;
        }
        17 => {
            emit(world, "back", None)?;
        }
        18 => {
            require(
                world.resource::<MenuState>().dialog == MenuDialog::Discard,
                "Unsaved settings were silently accepted",
            )?;
            screenshot(world, state, "discard");
            emit(world, "discard-confirm", None)?;
        }
        19 => {
            require(
                *world.resource::<AudioConfig>() == AudioConfig::default(),
                "Discard applied an unsaved audio draft",
            )?;
            emit(world, "multiplayer", None)?;
        }
        20 => {
            screenshot(world, state, "servers-empty");
            emit(world, "server-add", None)?;
        }
        21 => {
            emit(world, "server-name", Some("Local Workshop"))?;
            if state.join {
                let address = std::env::var("TERRA_MENU_JOIN_ADDRESS")
                    .map_err(|_| "TERRA_MENU_JOIN_ADDRESS is required")?
                    .parse::<std::net::SocketAddr>()
                    .map_err(|e| e.to_string())?;
                emit(world, "server-address", Some(&address.ip().to_string()))?;
                emit(world, "server-port", Some(&address.port().to_string()))?;
            } else {
                emit(world, "server-address", Some("127.0.0.1"))?;
                emit(world, "server-port", Some("5000"))?;
            }
        }
        22 => {
            screenshot(world, state, "add-server");
            emit(world, "server-save", None)?;
        }
        23 => {
            require(
                world.resource::<ServerMenu>().list.servers.len() == 1,
                "Add Server was not persisted",
            )?;
            screenshot(world, state, "server-selected");
            emit(world, "server-edit", None)?;
        }
        24 => {
            emit(world, "server-address", Some("http://invalid"))?;
        }
        25 => {
            emit(world, "server-save", None)?;
        }
        26 => {
            require(
                !world.resource::<MenuState>().error.is_empty(),
                "Invalid endpoint was accepted",
            )?;
            screenshot(world, state, "server-error");
            emit(world, "server-address", Some("localhost"))?;
        }
        27 => {
            emit(world, "server-save", None)?;
        }
        28 => {
            require(
                world.resource::<ServerMenu>().list.servers[0].address == "localhost",
                "Edit Server did not replace entry",
            )?;
            emit(world, "back", None)?;
        }
        29 => {
            if state.join {
                emit(world, "multiplayer", None)?;
                state.stage = 199;
            } else {
                emit(world, "options", None)?;
            }
        }
        30 => {
            let current = world.resource::<GraphicsConfig>();
            let next = if current.width == 1280 && current.height == 720 {
                "1280x800"
            } else {
                "1280x720"
            };
            emit(world, "graphics-resolution", Some(next))?;
        }
        31 => {
            emit(world, "apply", None)?;
        }
        32 => {
            require(
                world.resource::<MenuState>().dialog == MenuDialog::Display,
                "Display changes need confirmation",
            )?;
            screenshot(world, state, "display-confirm");
        }
        33 => {
            if world.resource::<OptionsDraft>().display.is_some() {
                return Ok(false);
            }
            require(
                world.resource::<MenuState>().dialog == MenuDialog::None,
                "Display rollback did not close dialog",
            )?;
            emit(world, "back", None)?;
        }
        34 => {
            emit(world, "singleplayer", None)?;
        }
        35 => {
            if state.session {
                emit(world, "world-play", None)?;
            } else {
                emit(world, "world-delete", None)?;
            }
        }
        36 => {
            if state.session {
                state.stage = 99;
                return Ok(true);
            }
            screenshot(world, state, "delete-world");
            emit(world, "world-delete-confirm", None)?;
        }
        37 => {
            require(
                world.resource::<WorldMenu>().entries.is_empty(),
                "Delete World left a manifest",
            )?;
            let path = world.resource::<MenuPaths>().configs.join("controls.toml");
            require(
                ControlsConfig::load(path)
                    .map_err(|e| e.to_string())?
                    .binding(GameAction::Forward)
                    == InputBinding::Key(KeyCode::ArrowUp),
                "Controls reload did not preserve binding",
            )?;
            tracing::info!(
                "Native menu smoke passed: pages, create/delete, server add/edit, controls, conflict, Escape, apply, discard, display timeout"
            );
            emit(world, "back", None)?;
        }
        38 => emit(world, "quit", None)?,
        100 => {
            if !state.loading_captured
                && world
                    .resource::<LoadingProgress>()
                    .fraction
                    .is_some_and(|fraction| (0.05..0.95).contains(&fraction))
            {
                screenshot(world, state, "loading-screen");
                state.loading_captured = true;
            }
            if !world
                .resource::<terra_network::client::ClientSession>()
                .ready
                || !world.resource::<terra_render::RenderStatus>().ready
                || world.resource::<MenuState>().page != MenuPage::Playing
            {
                return Ok(false);
            }
            let Some(body) = world
                .query_filtered::<&Transform, With<terra_player::PlayerBody>>()
                .iter(world)
                .next()
            else {
                return Ok(false);
            };
            state.origin = body.translation;
            require(
                world.contains_resource::<terra_server::IntegratedServer>(),
                "Play World did not start an integrated host",
            )?;
            world
                .resource_mut::<terra_player::CameraInputState>()
                .captured = true;
            key(world, KeyCode::ArrowUp, true);
        }
        101 => {
            if state.advanced.elapsed() < Duration::from_secs(1) {
                return Ok(false);
            }
            key(world, KeyCode::ArrowUp, false);
            let confirmed = world
                .resource::<terra_network::client::ClientSession>()
                .confirmed_state()
                .ok_or("No server state")?;
            require(
                Vec3::from_array(confirmed.position).distance(state.origin) > 1.0,
                "Rebound forward key did not move authoritative player",
            )?;
            screenshot(world, state, "playing");
            let mut cameras =
                world.query_filtered::<&mut Transform, With<terra_player::TerraCamera>>();
            for mut camera in cameras.iter_mut(world) {
                camera.rotation = Quat::from_euler(EulerRot::YXZ, 0.0, -1.4, 0.0);
            }
        }
        102 => match world.resource::<terra_player::TargetWorld>().0 {
            Some(terra_player::WorldTarget::Block(hit)) => {
                let material = world
                    .resource::<terra_world::streaming::ChunkStore>()
                    .block_at(hit.position)
                    .ok_or("Missing target")?;
                world.write_message(terra_world::streaming::BlockEditRequest {
                    position: hit.position,
                    expected_material: material,
                    material: 0,
                });
            }
            Some(terra_player::WorldTarget::Prop(hit)) => {
                world.write_message(terra_world::streaming::PropRemoveRequest {
                    section: hit.prop.section,
                    id: hit.prop.id,
                });
            }
            None => return Err("No natural target for session save regression".into()),
        },
        103 => {
            let journal = world.resource::<terra_world::edits::EditJournal>();
            state.edit_count = journal.edited_blocks() + journal.removed_props();
            if state.edit_count == 0 {
                return Ok(false);
            }
            key(world, KeyCode::Escape, true);
        }
        104 => {
            key(world, KeyCode::Escape, false);
            require(
                world.resource::<MenuState>().page == MenuPage::Pause,
                "Escape did not open Game Menu",
            )?;
            screenshot(world, state, "game-menu");
            emit(world, "save-world", None)?;
            state.stage = 179;
        }
        105 => {
            if world.resource::<MenuState>().page != MenuPage::Main {
                return Ok(false);
            }
            require(
                !world.contains_resource::<terra_server::IntegratedServer>(),
                "Returning to menu retained the integrated server",
            )?;
            emit(world, "singleplayer", None)?;
        }
        106 => {
            emit(world, "world-play", None)?;
        }
        107 => {
            if !world
                .resource::<terra_network::client::ClientSession>()
                .ready
                || world.resource::<MenuState>().page != MenuPage::Playing
            {
                return Ok(false);
            }
            let journal = world.resource::<terra_world::edits::EditJournal>();
            require(
                journal.edited_blocks() + journal.removed_props() == state.edit_count,
                "Saved journal did not survive host restart",
            )?;
            world.write_message(MenuAction::Command("pause".into()));
        }
        108 => {
            emit(world, "leave", None)?;
        }
        109 => {
            if world.resource::<MenuState>().page != MenuPage::Main {
                return Ok(false);
            }
            tracing::info!(
                "Native menu session smoke passed: integrated host, rebound movement, authoritative edit, final save, host restart and journal reload"
            );
            world.write_message(AppExit::Success);
        }
        200 => {
            emit(world, "server-join", None)?;
        }
        201 => {
            let session = world.resource::<terra_network::client::ClientSession>();
            if std::env::var_os("TERRA_MENU_EXPECT_CONNECTION_FAILURE").is_some() {
                require(
                    !session.ready,
                    "Unreachable-server fixture unexpectedly connected",
                )?;
                if world.resource::<MenuState>().error.is_empty() {
                    return Ok(false);
                }
                if !world
                    .query::<&bevy::ui::widget::Text>()
                    .iter(world)
                    .any(|text| text.0 == world.resource::<MenuState>().error)
                {
                    return Ok(false);
                }
                screenshot(world, state, "connection-failed");
                emit(world, "cancel-connect", None)?;
                state.stage = 209;
                return Ok(true);
            }
            if let Some(error) = &session.error {
                return Err(error.clone());
            }
            if !session.ready || world.resource::<MenuState>().page != MenuPage::Playing {
                return Ok(false);
            }
            require(
                !world.contains_resource::<terra_server::IntegratedServer>(),
                "Join Server started a second local host",
            )?;
            screenshot(world, state, "dedicated-joined");
            world.write_message(MenuAction::Command("pause".into()));
        }
        202 => {
            emit(world, "leave", None)?;
        }
        203 => {
            if world.resource::<MenuState>().page != MenuPage::Main {
                return Ok(false);
            }
            tracing::info!(
                "Native menu join smoke passed: saved endpoint, automatic UUID-profile handshake, terrain replica and clean leave"
            );
            world.write_message(AppExit::Success);
        }
        210 => {
            if world.resource::<MenuState>().page != MenuPage::Main {
                return Ok(false);
            }
            tracing::info!(
                "Native menu connection-failure smoke passed: actual unreachable server, visible failure and clean return"
            );
            world.write_message(AppExit::Success);
        }
        180 => {
            if world.resource::<SessionInfo>().saving
                || world.resource::<MenuState>().status != "World saved"
            {
                return Ok(false);
            }
            emit(world, "options", None)?;
        }
        181 => {
            require(
                world.resource::<MenuState>().page == MenuPage::Options,
                "Game Menu Options failed",
            )?;
            emit(world, "back", None)?;
        }
        182 => {
            require(
                world.resource::<MenuState>().page == MenuPage::Pause,
                "Options did not return to Game Menu",
            )?;
            emit(world, "resume", None)?;
        }
        183 => {
            require(
                world.resource::<MenuState>().page == MenuPage::Playing,
                "Resume failed",
            )?;
            key(world, KeyCode::Escape, true);
        }
        184 => {
            key(world, KeyCode::Escape, false);
            emit(world, "leave", None)?;
            state.stage = 104;
        }
        300 => pointer(world, "", "", false)?,
        301 => {
            let popup = world
                .query::<(&ElementId, &tilt_ui::ChoiceBoxParts)>()
                .iter(world)
                .find(|(id, _)| id.0 == "world-mode")
                .map(|(_, parts)| *parts)
                .ok_or("World mode dropdown missing")?;
            let labels: Vec<_> = world
                .query::<(Entity, &bevy::ui::widget::Text)>()
                .iter(world)
                .filter(|(entity, text)| {
                    has_ancestor_entity(world, *entity, popup.popup)
                        && matches!(text.0.as_str(), "Survival" | "Creative")
                })
                .map(|(entity, text)| {
                    (
                        text.0.clone(),
                        world
                            .get::<ComputedNode>(entity)
                            .is_some_and(|node| node.size().min_element() > 0.0),
                    )
                })
                .collect();
            let ready =
                popup.open && labels.len() == 2 && labels.iter().all(|(_, visible)| *visible);
            if !ready && state.advanced.elapsed() < Duration::from_secs(5) {
                return Ok(false);
            }
            require(
                ready,
                &format!(
                    "World mode dropdown did not finish layout within 5s: open={}, labels={labels:?}",
                    popup.open
                ),
            )?;
            screenshot(world, state, "mode-dropdown");
            pointer(world, "mode-creative", "", true)?;
        }
        302 => pointer(world, "", "", false)?,
        303 => {
            require(
                world.resource::<WorldMenu>().mode == "creative",
                "Native dropdown selection failed",
            )?;
            state.stage = 2;
        }
        400 => {
            let address = std::env::var("TERRA_MENU_JOIN_ADDRESS")
                .map_err(|_| "TERRA_MENU_JOIN_ADDRESS is required")?;
            let id = format!("lan-{address}");
            if !world
                .resource::<ServersView>()
                .cards
                .iter()
                .any(|card| card.id == id)
            {
                return Ok(false);
            }
            require(
                world.resource::<ServerMenu>().list.servers.is_empty(),
                "LAN discovery must not persist invented saved entries",
            )?;
            screenshot(world, state, "lan-discovered");
            emit(world, &id, None)?;
        }
        401 => {
            require(
                world.resource::<ServerMenu>().lan_selected.is_some(),
                "LAN selection failed",
            )?;
            emit(world, "server-join", None)?;
            state.stage = 200;
        }
        500 => {
            require(
                world.resource::<OptionsDraft>().tab == OptionsTab::Audio,
                "Audio tab did not open",
            )?;
            let ready = world
                .query::<(Entity, &ElementId)>()
                .iter(world)
                .any(|(entity, id)| {
                    id.0 == "row-slider" && scope_distance(world, entity, "Master Volume").is_some()
                });
            if !ready {
                if state.advanced.elapsed() > Duration::from_secs(6) {
                    return Err("Audio controls did not finish asynchronous materialization".into());
                }
                return Ok(false);
            }
            screenshot(world, state, "audio");
            for (id, value) in [
                ("audio-master", "80"),
                ("audio-ambience", "70"),
                ("audio-animals", "55"),
                ("audio-footsteps", "65"),
                ("audio-water", "60"),
                ("audio-river", "40"),
                ("audio-waterfall", "90"),
            ] {
                emit(world, id, Some(value))?;
            }
        }
        501 => {
            require(
                world.resource::<AudioConfig>().master_volume == 1.0,
                "Audio draft was applied before Apply",
            )?;
            emit(world, "apply", None)?;
        }
        502 => {
            let expected = AudioConfig {
                master_volume: 0.8,
                ambience_volume: 0.7,
                animals_volume: 0.55,
                footsteps_volume: 0.65,
                water_volume: 0.6,
                river_volume: 0.4,
                waterfall_volume: 0.9,
                muted: false,
            };
            let path = world.resource::<MenuPaths>().configs.join("audio.toml");
            require(
                *world.resource::<AudioConfig>() == expected
                    && AudioConfig::load(path).map_err(|e| e.to_string())? == expected,
                "Audio settings did not apply and reload from TOML",
            )?;
            screenshot(world, state, "audio-applied");
            emit(world, "audio-muted", Some("true"))?;
        }
        503 => emit(world, "apply", None)?,
        504 => {
            require(
                world.resource::<AudioConfig>().muted,
                "Mute was not applied",
            )?;
            emit(world, "reset", None)?;
        }
        505 => {
            require(
                world.resource::<OptionsDraft>().audio == AudioConfig::default()
                    && world.resource::<AudioConfig>().muted,
                "Audio Reset must change only the draft until Apply",
            )?;
            emit(world, "apply", None)?;
        }
        506 => {
            let path = world.resource::<MenuPaths>().configs.join("audio.toml");
            require(
                AudioConfig::load(path).map_err(|e| e.to_string())? == AudioConfig::default(),
                "Audio defaults were not persisted",
            )?;
            emit(world, "multiplayer-options", None)?;
            state.stage = 606;
        }
        607 => {
            require(
                world.resource::<OptionsDraft>().tab == OptionsTab::Multiplayer,
                "Multiplayer options did not open",
            )?;
            screenshot(world, state, "multiplayer-options");
            require(
                world
                    .resource::<OptionsDraft>()
                    .multiplayer
                    .cursor_color
                    .is_none(),
                "Default cursor color must be automatic",
            )?;
            require(
                world
                    .resource::<OptionsView>()
                    .rows
                    .iter()
                    .any(|row| row.id == "multiplayer-color" && row.enabled),
                "Colorpicker is disabled while automatic color is active",
            )?;
        }
        608 => {
            emit(world, "multiplayer-color", Some("#e15c45"))?;
            emit(world, "multiplayer-names", Some("false"))?;
            emit(world, "apply", None)?;
        }
        609 => {
            let path = world
                .resource::<MenuPaths>()
                .configs
                .join("multiplayer.toml");
            let config = terra_core::MultiplayerConfig::load(path).map_err(|e| e.to_string())?;
            require(
                !config.player_names
                    && config.chest_cursors
                    && config.cursor_color == Some([225, 92, 69]),
                "Multiplayer options did not persist",
            )?;
            screenshot(world, state, "multiplayer-options-applied");
            let picker = world
                .query::<(Entity, &ElementId)>()
                .iter(world)
                .find(|(_, id)| id.0 == "row-color")
                .map(|(entity, _)| entity)
                .ok_or("Cursor colorpicker missing")?;
            require(
                tilt_ui::set_color_picker_open(world, picker, true),
                "Cursor palette did not open",
            )?;
        }
        610 => {
            screenshot(world, state, "multiplayer-color-palette");
            let picker = world
                .query::<(Entity, &ElementId)>()
                .iter(world)
                .find(|(_, id)| id.0 == "row-color")
                .map(|(entity, _)| entity)
                .ok_or("Cursor colorpicker missing")?;
            tilt_ui::set_color_picker_open(world, picker, false);
            emit(world, "reset", None)?;
            emit(world, "apply", None)?;
            emit(world, "audio", None)?;
        }
        611 => {
            emit(world, "gameplay-options", None)?;
            state.stage = 699;
        }
        690 => {
            require(
                world.resource::<MenuState>().page == MenuPage::Options,
                "Gameplay fixture could not open Options",
            )?;
            emit(world, "gameplay-options", None)?;
            state.stage = 699;
        }
        700 => {
            require(
                world.resource::<OptionsDraft>().tab == OptionsTab::Gameplay
                    && world
                        .resource::<OptionsView>()
                        .rows
                        .iter()
                        .any(|row| row.id == "hud-edit")
                    && world
                        .resource::<OptionsView>()
                        .rows
                        .iter()
                        .any(|row| row.id == "minimap-settings")
                    && world
                        .resource::<OptionsView>()
                        .rows
                        .iter()
                        .any(|row| row.id == "view-bobbing" && row.checked),
                "Gameplay tab must contain HUD, MiniMap and walking-camera settings",
            )?;
            screenshot(world, state, "gameplay-options");
            emit(world, "view-bobbing", Some("false"))?;
            emit(world, "minimap-settings", None)?;
        }
        701 => {
            require(
                world.resource::<MenuState>().dialog == MenuDialog::Minimap
                    && world.resource::<DialogView>().rows.len() == 13,
                "MiniMap popup settings did not materialize",
            )?;
            validate_minimap_popup(world)?;
            screenshot(world, state, "minimap-settings");
            for (id, value) in [
                ("minimap-size", "300"),
                ("minimap-fps", "40"),
                ("minimap-icon-scale", "2.0"),
                ("minimap-mode", "cave"),
                ("minimap-rotate", "true"),
                ("minimap-mobs", "false"),
                ("minimap-coordinates", "false"),
            ] {
                emit(world, id, Some(value))?;
            }
        }
        702 => {
            let draft = &world.resource::<OptionsDraft>().gameplay.minimap;
            require(
                draft.size == 300
                    && draft.fps == 40
                    && draft.icon_scale == 2.0
                    && draft.mode == MapMode::Cave
                    && draft.rotate
                    && !draft.mobs
                    && !draft.coordinates
                    && *world.resource::<GameplayConfig>() == GameplayConfig::default(),
                "MiniMap widgets must update the draft without applying it",
            )?;
            screenshot(world, state, "minimap-settings-edited");
            emit(world, "dialog-cancel", None)?;
        }
        703 => {
            require(
                world.resource::<MenuState>().dialog == MenuDialog::None
                    && *world.resource::<GameplayConfig>() == GameplayConfig::default(),
                "MiniMap Done must return to Options without applying",
            )?;
            emit(world, "apply", None)?;
        }
        704 => {
            let path = world.resource::<MenuPaths>().configs.join("gameplay.toml");
            let saved = GameplayConfig::load(path).map_err(|error| error.to_string())?;
            require(
                saved == *world.resource::<GameplayConfig>()
                    && saved.minimap.size == 300
                    && saved.minimap.fps == 40
                    && saved.minimap.icon_scale == 2.0
                    && saved.minimap.mode == MapMode::Cave
                    && !saved.view_bobbing,
                "MiniMap and walking-camera settings did not apply and reload from gameplay.toml",
            )?;
            emit(world, "minimap-settings", None)?;
        }
        705 => {
            validate_minimap_popup(world)?;
            emit(world, "minimap-reset", None)?;
        }
        706 => {
            let expected = GameplayConfig {
                view_bobbing: false,
                ..default()
            };
            let path = world.resource::<MenuPaths>().configs.join("gameplay.toml");
            let saved = GameplayConfig::load(path).map_err(|error| error.to_string())?;
            require(
                world.resource::<OptionsDraft>().gameplay == expected
                    && world.resource::<GameplayConfig>().minimap.mode == MapMode::Cave
                    && !world.resource::<GameplayConfig>().view_bobbing
                    && saved == *world.resource::<GameplayConfig>(),
                "MiniMap Reset must stage only MiniMap defaults and preserve the applied walking-camera preference and file",
            )?;
            screenshot(world, state, "minimap-settings-reset");
            emit(world, "dialog-cancel", None)?;
        }
        707 => {
            require(
                world.resource::<MenuState>().dialog == MenuDialog::None,
                "MiniMap Reset Done did not return to Options",
            )?;
            emit(world, "apply", None)?;
        }
        708 => {
            let expected = GameplayConfig {
                view_bobbing: false,
                ..default()
            };
            let path = world.resource::<MenuPaths>().configs.join("gameplay.toml");
            let saved = GameplayConfig::load(path).map_err(|error| error.to_string())?;
            require(
                *world.resource::<GameplayConfig>() == expected && saved == expected,
                "MiniMap defaults must apply without resetting the walking-camera preference",
            )?;
            emit(world, "reset", None)?;
            emit(world, "apply", None)?;
            emit(world, "audio", None)?;
        }
        709 => {
            let path = world.resource::<MenuPaths>().configs.join("gameplay.toml");
            require(
                *world.resource::<GameplayConfig>() == GameplayConfig::default()
                    && GameplayConfig::load(path).map_err(|error| error.to_string())?
                        == GameplayConfig::default(),
                "Full Gameplay Reset must restore and persist all defaults",
            )?;
            if state.gameplay_only {
                emit(world, "back", None)?;
                return Ok(true);
            }
            emit(world, "audio-river", Some("45"))?;
            emit(world, "graphics", None)?;
            state.stage = 15;
        }
        710 if state.gameplay_only => {
            require(
                world.resource::<MenuState>().page == MenuPage::Main,
                "Gameplay options did not return to Main Menu",
            )?;
            tracing::info!(
                "Native Gameplay menu smoke passed: real TiltUI tab, MiniMap popup, widgets, staged Apply, TOML reload, Reset, bounded footer and Back"
            );
            emit(world, "quit", None)?;
        }
        1000 => pointer(world, "", "", false)?,
        1001 => {
            require(
                world.resource::<MenuState>().page == MenuPage::Worlds,
                "Pointer click did not open Worlds",
            )?;
            pointer(world, "menu-action", "Back", true)?;
        }
        1002 => pointer(world, "", "", false)?,
        1003 => {
            require(
                world.resource::<MenuState>().page == MenuPage::Main,
                "Pointer Back failed",
            )?;
            pointer(world, "menu-action", "Options", true)?;
        }
        1004 => pointer(world, "", "", false)?,
        1005 => {
            require(
                world.resource::<MenuState>().page == MenuPage::Options,
                "Pointer Options failed",
            )?;
            screenshot(world, state, "pointer-options");
            pointer(world, "controls", "", true)?;
        }
        1006 => pointer(world, "", "", false)?,
        1007 => {
            require(
                world.resource::<OptionsDraft>().tab == OptionsTab::Controls,
                "Pointer Controls tab failed",
            )?;
            screenshot(world, state, "pointer-controls");
            pointer(world, "audio", "", true)?;
        }
        1008 => pointer(world, "", "", false)?,
        1009 => {
            require(
                world.resource::<OptionsDraft>().tab == OptionsTab::Audio,
                "Pointer Audio tab failed",
            )?;
            screenshot(world, state, "pointer-audio");
            pointer(world, "menu-action", "Back", true)?;
        }
        1010 => pointer(world, "", "", false)?,
        1011 => {
            require(
                world.resource::<MenuState>().page == MenuPage::Main,
                "Pointer Options Back failed",
            )?;
            pointer(world, "menu-action", "Quit Game", true)?;
        }
        1012 => {
            pointer(world, "", "", false)?;
            tracing::info!(
                "Native pointer smoke passed: pointer input, hit testing, click/release, Worlds, Back, Options, Controls, Audio and Quit"
            );
        }
        _ => {}
    }
    Ok(true)
}
