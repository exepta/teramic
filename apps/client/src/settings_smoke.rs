use bevy::{
    app::AppExit,
    input::InputSystems,
    prelude::*,
    window::{CursorGrabMode, CursorOptions, PresentMode, PrimaryWindow},
};
use std::{
    path::PathBuf,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use terra_core::{ClientConfig, Config, GameConfig};
use terra_player::{CameraInputState, PlayerInputBlock, PlayerSet, TargetWorld, TerraCamera};
use terra_render::{PropRenderStatus, RenderStatus};
use terra_ui::{
    ControlChecked, ElementId, HtmlEvent, NumericRange, OptionData, SettingsAction,
    SettingsConfigPath, SettingsView, UiDocumentState, UiLoadState,
};
use terra_world::{
    edits::EditJournal,
    streaming::{GenerationStatus, StreamingSet, WorldGeneration},
};

#[derive(Resource)]
struct SettingsSmoke {
    started: Instant,
    advanced: Instant,
    stage: u8,
    original: ClientConfig,
    temporary: PathBuf,
    error: Option<String>,
    edits: (usize, usize),
    inject_click: bool,
    finished: bool,
}

pub(crate) fn install(app: &mut App) {
    if !std::env::args().any(|arg| arg == "--smoke-test-settings") {
        return;
    }
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let temporary = std::env::temp_dir().join(format!(
        "terra-settings-smoke-{}-{timestamp}",
        std::process::id()
    ));
    let error = std::fs::create_dir(&temporary)
        .err()
        .map(|error| error.to_string());
    let original = app.world().resource::<GameConfig>().client.clone();
    app.insert_resource(SettingsConfigPath(temporary.join("client.toml")))
        .insert_resource(SettingsSmoke {
            started: Instant::now(),
            advanced: Instant::now(),
            stage: 0,
            original,
            temporary,
            error,
            edits: (0, 0),
            inject_click: false,
            finished: false,
        })
        .add_systems(PostStartup, aim_at_ground)
        .add_systems(PreUpdate, inject_blocked_input.after(InputSystems))
        .add_systems(
            Update,
            run.after(StreamingSet::Dispatch)
                .after(PlayerSet::Interaction),
        );
}

fn aim_at_ground(mut cameras: Query<&mut Transform, With<TerraCamera>>) {
    for mut camera in &mut cameras {
        let eye = camera.translation;
        *camera =
            Transform::from_translation(eye).looking_at(eye + Vec3::new(0.0, -4.0, -0.8), Vec3::Y);
    }
}

fn inject_blocked_input(
    mut test: ResMut<SettingsSmoke>,
    mut buttons: ResMut<ButtonInput<MouseButton>>,
    mut keys: ResMut<ButtonInput<KeyCode>>,
    mut camera: ResMut<CameraInputState>,
) {
    if test.inject_click {
        test.inject_click = false;
        // Deliberately keep the camera's captured bit true: the menu blocker,
        // not the smoke mode's released cursor, must stop mining and placement.
        camera.captured = true;
        buttons.press(MouseButton::Left);
        keys.press(KeyCode::KeyB);
    }
}

fn run(world: &mut World) {
    let Some(mut test) = world.remove_resource::<SettingsSmoke>() else {
        return;
    };
    if !test.finished {
        let outcome = if let Some(error) = test.error.take() {
            Err(error)
        } else if test.started.elapsed() > Duration::from_secs(150) {
            Err(format!("Settings smoke timed out at stage {}", test.stage))
        } else {
            step(world, &mut test)
        };
        if let Err(error) = outcome {
            tracing::error!(%error, "Settings smoke failed");
            test.finished = true;
            let _ = std::fs::remove_dir_all(&test.temporary);
            world.write_message(AppExit::error());
        }
    }
    world.insert_resource(test);
}

fn step(world: &mut World, test: &mut SettingsSmoke) -> Result<(), String> {
    if test.stage > 0 && test.advanced.elapsed() < Duration::from_millis(500) {
        return Ok(());
    }
    match test.stage {
        0 => {
            if world.resource::<WorldGeneration>().status != GenerationStatus::Ready
                || !world.resource::<RenderStatus>().ready
                || !world.resource::<PropRenderStatus>().ready
                || !matches!(
                    world.resource::<UiDocumentState>().0.load,
                    UiLoadState::Ready
                )
                || world.resource::<TargetWorld>().0.is_none()
            {
                return Ok(());
            }
            world.write_message(SettingsAction::Open);
        }
        1 => {
            require(world.resource::<SettingsView>().open, "Menu did not open")?;
            verify_widget_values(world)?;
            require(
                world.resource::<PlayerInputBlock>().0,
                "Menu did not block game input",
            )?;
            let mut windows = world.query_filtered::<&CursorOptions, With<PrimaryWindow>>();
            require(
                windows
                    .iter(world)
                    .all(|cursor| cursor.visible && cursor.grab_mode == CursorGrabMode::None),
                "Menu did not release the cursor",
            )?;
            for (id, value) in [
                ("settings-fov", "78"),
                ("settings-distance", "8"),
                ("settings-vsync", "false"),
                ("settings-overlay", "false"),
                ("settings-quality", "medium"),
                ("settings-waves", "0.25"),
                ("settings-reflections", "false"),
            ] {
                send_event(world, id, Some(value))?;
            }
        }
        2 => {
            verify_widget_values(world)?;
            let client = &world.resource::<GameConfig>().client;
            require(
                client.fov_degrees == 78.0
                    && client.view_distance == 8
                    && !client.vsync
                    && !client.debug_overlay,
                "Live menu values did not reach GameConfig",
            )?;
            require(
                world.resource::<SettingsView>().open,
                "Disabling the overlay closed the menu",
            )?;
            let mut cameras = world.query_filtered::<&Projection, With<TerraCamera>>();
            require(cameras.iter(world).all(|projection| matches!(projection, Projection::Perspective(p) if (p.fov - 78.0_f32.to_radians()).abs() < 0.00001)), "FOV was not applied to the live camera")?;
            let mut windows = world.query_filtered::<&Window, With<PrimaryWindow>>();
            require(
                windows
                    .iter(world)
                    .all(|window| window.present_mode == PresentMode::AutoNoVsync),
                "VSync was not applied to the live window",
            )?;
            let journal = world.resource::<EditJournal>();
            test.edits = (journal.edited_blocks(), journal.removed_props());
            test.inject_click = true;
            screenshot(world, false);
        }
        3 => {
            let journal = world.resource::<EditJournal>();
            require(
                (journal.edited_blocks(), journal.removed_props()) == test.edits,
                "A click or placement behind the menu edited the world",
            )?;
            world.resource_mut::<CameraInputState>().captured = false;
            world
                .resource_mut::<ButtonInput<MouseButton>>()
                .release(MouseButton::Left);
            world
                .resource_mut::<ButtonInput<KeyCode>>()
                .release(KeyCode::KeyB);
            send_event(world, "settings-cancel", None)?;
        }
        4 => {
            require(
                world.resource::<GameConfig>().client == test.original,
                "Cancel did not restore the opening settings",
            )?;
            require(
                !world.resource::<SettingsView>().open && !world.resource::<PlayerInputBlock>().0,
                "Cancel left the menu or game blocker active",
            )?;
            world.write_message(SettingsAction::Open);
        }
        5 => {
            send_event(world, "settings-fov", Some("82"))?;
            send_event(world, "settings-apply", None)?;
        }
        6 => {
            require(
                world.resource::<GameConfig>().client.fov_degrees == 82.0,
                "Apply did not retain the preview",
            )?;
            send_event(world, "settings-fov", Some("90"))?;
            send_event(world, "settings-cancel", None)?;
        }
        7 => {
            require(
                world.resource::<GameConfig>().client.fov_degrees == 82.0,
                "Cancel crossed the last Apply checkpoint",
            )?;
            world.write_message(SettingsAction::Open);
        }
        8 => {
            send_event(world, "settings-reset", None)?;
        }
        9 => {
            require(
                world.resource::<GameConfig>().client.fov_degrees
                    == ClientConfig::default().fov_degrees,
                "Reset did not preview the defaults",
            )?;
            send_event(world, "settings-cancel", None)?;
        }
        10 => {
            require(
                world.resource::<GameConfig>().client.fov_degrees == 82.0,
                "Reset preview could not be cancelled",
            )?;
            world.write_message(SettingsAction::Open);
        }
        11 => {
            send_event(world, "settings-save", None)?;
        }
        12 => {
            let saved = ClientConfig::load(&world.resource::<SettingsConfigPath>().0)
                .map_err(|error| error.to_string())?;
            require(
                saved == world.resource::<GameConfig>().client && saved.fov_degrees == 82.0,
                "Saved TOML did not roundtrip the displayed settings",
            )?;
            let mut windows = world.query_filtered::<&mut Window, With<PrimaryWindow>>();
            for mut window in windows.iter_mut(world) {
                window.resolution.set(640.0, 480.0);
            }
        }
        13 => {
            verify_widget_values(world)?;
            screenshot(world, true);
        }
        14 => {
            world.write_message(SettingsAction::Resume);
        }
        15 => {
            require(
                !world.resource::<SettingsView>().open && !world.resource::<PlayerInputBlock>().0,
                "Resume left the menu or input blocker active",
            )?;
            std::fs::remove_dir_all(&test.temporary).map_err(|error| error.to_string())?;
            tracing::info!(
                "Settings smoke completed: real TiltUI handlers, live FOV/view-distance/VSync, overlay independence, blocked mining/placement, cancel/apply/reset and isolated TOML save roundtrip; screenshots at normal and 640x480 logical size. Pointer drag behavior requires a separate native pointer test."
            );
            test.finished = true;
            world.write_message(AppExit::Success);
        }
        _ => {}
    }
    test.stage += 1;
    test.advanced = Instant::now();
    Ok(())
}

fn require(condition: bool, message: &str) -> Result<(), String> {
    if condition {
        Ok(())
    } else {
        Err(message.into())
    }
}

fn verify_widget_values(world: &mut World) -> Result<(), String> {
    let view = world.resource::<SettingsView>();
    let sliders = [
        ("settings-fov", view.fov),
        ("settings-distance", f32::from(view.distance)),
        ("settings-sensitivity", view.sensitivity),
        ("settings-waves", view.waves),
    ];
    let checks = [
        ("settings-vsync", view.vsync),
        ("settings-overlay", view.debug),
        ("settings-reflections", view.reflections),
    ];
    let selections = [
        view.mode.clone(),
        view.quality.clone(),
        view.weather_quality.clone(),
    ];
    let mut ranges = world.query::<(&ElementId, &NumericRange)>();
    for (id, expected) in sliders {
        require(
            ranges
                .iter(world)
                .any(|(element, range)| element.0 == id && (range.value - expected).abs() < 0.001),
            &format!("Slider {id} does not display the bound value {expected}"),
        )?;
    }
    let mut checked = world.query::<(&ElementId, &ControlChecked)>();
    for (id, expected) in checks {
        require(
            checked
                .iter(world)
                .any(|(element, state)| element.0 == id && state.0 == expected),
            &format!("Checkbox {id} does not display the bound value {expected}"),
        )?;
    }
    let mut options = world.query::<(&OptionData, &ControlChecked)>();
    for expected in selections {
        require(
            options
                .iter(world)
                .any(|(option, checked)| option.value == expected && checked.0),
            &format!("Choice does not display selected option {expected}"),
        )?;
    }
    Ok(())
}

fn send_event(world: &mut World, id: &str, value: Option<&str>) -> Result<(), String> {
    let mut query = world.query::<(Entity, &ElementId, &ComputedNode)>();
    let target = query
        .iter(world)
        .find(|(_, element, _)| element.0 == id)
        .map(|(entity, _, node)| (entity, node.size()));
    let Some((target, size)) = target else {
        return Err(format!("TiltUI control {id} is missing"));
    };
    require(
        size.x > 0.0 && size.y > 0.0,
        &format!("TiltUI control {id} has no layout"),
    )?;
    world.write_message(HtmlEvent {
        target,
        kind: if value.is_some() { "change" } else { "click" },
        value: value.map(str::to_owned),
        submitter: None,
        data: Default::default(),
        form_data: None,
        handler: if value.is_some() {
            "settings_change"
        } else {
            "settings_command"
        }
        .into(),
    });
    Ok(())
}

fn screenshot(world: &mut World, small: bool) {
    let mut path = std::env::var_os("TERRA_SCREENSHOT")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/tmp/terra-settings-menu.png"));
    if small {
        let name = path.file_stem().unwrap_or_default().to_string_lossy();
        path = path.with_file_name(format!("{name}-small.png"));
    }
    world
        .spawn(bevy::render::view::screenshot::Screenshot::primary_window())
        .observe(bevy::render::view::screenshot::save_to_disk(path));
}
