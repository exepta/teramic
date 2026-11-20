//! Opt-in placement acceptance in an unchanged copy of an existing saved world.
//! Uses the saved identity, real TAB/HTML/mouse input, UDP authority and disk save.
use bevy::{
    input::{ButtonState, mouse::MouseButtonInput},
    prelude::*,
    render::view::screenshot::{Screenshot, ScreenshotCaptured, save_to_disk},
    ui::UiGlobalTransform,
    window::{CursorMoved, PrimaryWindow, WindowEvent},
};
use std::{
    collections::BTreeSet,
    path::PathBuf,
    sync::{Mutex, mpsc},
    time::{Duration, Instant},
};
use terra_core::{GameAction, InputBinding, building_jobs::HOUSE_TEMPLATE, kingdom::KingdomId};
use terra_network::client::ClientSession;
use terra_player::{ConstructionCamera, TerraCamera};
use terra_ui::{BuildModeView, ElementId};

const DEFAULT_ORIGIN: [i64; 3] = [16, 16, 24];

#[derive(Debug, Clone, Copy)]
enum Confirmation {
    Mouse,
    OsMouse,
    Enter,
    Button,
}

pub(crate) fn requested() -> bool {
    std::env::var("TERRA_SAVED_CONSTRUCTION_SMOKE").as_deref() == Ok("1")
}

#[derive(Resource)]
struct Smoke {
    origin: [i64; 3],
    facing: u8,
    confirmation: Confirmation,
    button_targeted: bool,
    started: Instant,
    since: Instant,
    step: u8,
    finished: bool,
    keys: Vec<KeyCode>,
    mouse_release: bool,
    cursor: Option<Vec2>,
    kingdom: Option<KingdomId>,
    previous_jobs: BTreeSet<u64>,
    job: Option<u64>,
    save: Option<Mutex<mpsc::Receiver<Result<(), String>>>>,
    capture_pending: bool,
    captures: usize,
    output: PathBuf,
    directory: PathBuf,
}

pub(crate) fn install(app: &mut App) -> Result<(), String> {
    if !requested() {
        return Ok(());
    }
    if !app
        .world()
        .contains_resource::<crate::bootstrap::BenchmarkUi>()
    {
        return Err("Saved construction needs --smoke-test-benchmark, TERRA_BENCH_MODE=integrated and TERRA_BENCH_UI=1".into());
    }
    let directory = PathBuf::from(
        std::env::var_os("TERRA_BENCH_SAVE_DIR")
            .ok_or("Saved construction requires a copied TERRA_BENCH_SAVE_DIR")?,
    )
    .canonicalize()
    .map_err(|error| error.to_string())?;
    let original = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../worlds")
        .canonicalize()
        .map_err(|error| error.to_string())?;
    if directory.starts_with(original) {
        return Err(
            "Saved construction must use a copy outside the original worlds directory".into(),
        );
    }
    let output = PathBuf::from(
        std::env::var_os("TERRA_CAPTURE_DIR")
            .ok_or("Set TERRA_CAPTURE_DIR for saved construction")?,
    );
    std::fs::create_dir_all(&output).map_err(|error| error.to_string())?;
    let origin = match std::env::var("TERRA_SAVED_CONSTRUCTION_ORIGIN") {
        Ok(text) => {
            let coordinates: Vec<i64> = text
                .split(',')
                .map(|value| {
                    value.trim().parse::<i64>().map_err(|_| {
                        "Saved construction origin must contain three integers".to_string()
                    })
                })
                .collect::<Result<_, _>>()?;
            let origin: [i64; 3] = coordinates
                .try_into()
                .map_err(|_| "Saved construction origin must contain x,y,z")?;
            if origin.iter().any(|v| v.unsigned_abs() > 998_900) {
                return Err("Saved construction origin exceeds the construction limit".into());
            }
            origin
        }
        Err(std::env::VarError::NotPresent) => DEFAULT_ORIGIN,
        Err(error) => return Err(error.to_string()),
    };
    let facing = std::env::var("TERRA_SAVED_CONSTRUCTION_FACING")
        .unwrap_or_else(|_| "0".into())
        .parse::<u8>()
        .map_err(|_| "Saved construction facing must be 0..3")?;
    if facing > 3 {
        return Err("Saved construction facing must be 0..3".into());
    }
    let confirmation = match std::env::var("TERRA_SAVED_CONSTRUCTION_CONFIRM").as_deref() {
        Ok("mouse") | Err(std::env::VarError::NotPresent) => Confirmation::Mouse,
        Ok("os-mouse") => Confirmation::OsMouse,
        Ok("enter") => Confirmation::Enter,
        Ok("button") => Confirmation::Button,
        _ => {
            return Err(
                "Saved construction confirmation must be mouse, os-mouse, enter or button".into(),
            );
        }
    };
    let now = Instant::now();
    app.insert_resource(Smoke {
        origin,
        facing,
        confirmation,
        button_targeted: false,
        started: now,
        since: now,
        step: 0,
        finished: false,
        keys: Vec::new(),
        mouse_release: false,
        cursor: None,
        kingdom: None,
        previous_jobs: BTreeSet::new(),
        job: None,
        save: None,
        capture_pending: false,
        captures: 0,
        output,
        directory,
    })
    .insert_resource(bevy::winit::WinitSettings::continuous())
    .add_systems(
        First,
        maintain_cursor.in_set(bevy::picking::PickingSystems::PostInput),
    )
    .add_systems(Last, run);
    Ok(())
}

fn window(world: &mut World) -> Result<Entity, String> {
    world
        .query_filtered::<Entity, With<PrimaryWindow>>()
        .iter(world)
        .next()
        .ok_or("Saved construction has no primary window".into())
}

fn move_cursor(world: &mut World, point: Vec2) -> Result<(), String> {
    let entity = window(world)?;
    let mut viewport = world
        .get_mut::<Window>(entity)
        .ok_or("Saved construction window disappeared")?;
    let delta = viewport.cursor_position().map(|old| point - old);
    viewport
        .bypass_change_detection()
        .set_cursor_position(Some(point));
    let event = CursorMoved {
        window: entity,
        position: point,
        delta,
    };
    world.write_message(event.clone());
    world.write_message(WindowEvent::CursorMoved(event));
    Ok(())
}

fn maintain_cursor(world: &mut World) {
    if native_pointer(world.resource::<Smoke>()) {
        return;
    }
    if let Some(point) = world.resource::<Smoke>().cursor {
        let _ = move_cursor(world, point);
    }
}

fn native_pointer(smoke: &Smoke) -> bool {
    matches!(smoke.confirmation, Confirmation::OsMouse)
        || matches!(smoke.confirmation, Confirmation::Button)
            && std::env::var("TERRA_NATIVE_OS_POINTER").as_deref() == Ok("1")
}

/// Exercise the OS event path through the existing compositor-aware helper.
fn os_pointer(world: &mut World, point: Vec2, pressed: Option<bool>) -> Result<(), String> {
    let entity = window(world)?;
    let viewport = world
        .get::<Window>(entity)
        .ok_or("Missing native pointer window")?;
    let physical = point * viewport.resolution.scale_factor();
    let mut commands = vec![serde_json::json!({
        "move": [physical.x, physical.y],
        "size": [viewport.physical_width(), viewport.physical_height()],
        "wait": 0.1,
    })];
    if let Some(pressed) = pressed {
        commands.push(serde_json::json!({"button": u8::from(pressed), "wait": 0.1}));
    }
    crate::inventory_smoke::os_pointer(world, &commands)
}

fn mouse(world: &mut World, pressed: bool) -> Result<(), String> {
    let window = window(world)?;
    world.write_message(MouseButtonInput {
        button: MouseButton::Left,
        state: if pressed {
            ButtonState::Pressed
        } else {
            ButtonState::Released
        },
        window,
    });
    Ok(())
}

fn pulse(world: &mut World, smoke: &mut Smoke) -> Result<(), String> {
    let binding = world
        .resource::<terra_core::ControlsConfig>()
        .binding(GameAction::BuildMode);
    let keys = match binding {
        InputBinding::Key(key) => vec![key],
        InputBinding::Chord { modifier, key } => vec![modifier, key],
        InputBinding::Mouse(_) => {
            return Err("Saved construction expects a keyboard TAB binding".into());
        }
    };
    for key in &keys {
        crate::minimap_smoke::key_event(world, *key, true);
    }
    smoke.keys.extend(keys);
    Ok(())
}

fn select_template(world: &mut World) -> Result<(), String> {
    let index = world
        .resource::<BuildModeView>()
        .templates
        .iter()
        .position(|row| row.id == HOUSE_TEMPLATE)
        .ok_or("Standard house is absent from the actual server catalog")?;
    let mut cards: Vec<_> = world
        .query::<(Entity, &ElementId, &ComputedNode, &UiGlobalTransform)>()
        .iter(world)
        .filter(|(_, id, node, _)| id.0 == "build-mode-template" && node.size().min_element() > 0.0)
        .map(|(entity, _, _, pose)| (entity, pose.transform_point2(Vec2::ZERO).y))
        .collect();
    cards.sort_by(|a, b| a.1.total_cmp(&b.1));
    let target = cards
        .get(index)
        .ok_or("Standard house has no rendered HTML card")?
        .0;
    if !world
        .get::<tilt_ui::StaticAttributes>(target)
        .is_some_and(|attributes| {
            attributes.attributes.iter().any(|attribute| {
                attribute.name == "onclick" && attribute.value.contains("build_mode_action")
            })
        })
    {
        return Err("Actual standard-house card has no construction HTML handler".into());
    }
    if world
        .resource_mut::<tilt_ui::UiBindingStore>()
        .set_json_path(
            "buildModeIntent.template",
            serde_json::Value::String(HOUSE_TEMPLATE.into()),
        )
        .is_none()
    {
        return Err("Standard-house HTML intent alias is absent".into());
    }
    world.write_message(tilt_ui::HtmlEvent {
        target,
        kind: "click",
        value: None,
        submitter: None,
        data: Default::default(),
        form_data: None,
        handler: "build_mode_action".into(),
    });
    Ok(())
}

fn ground_pointer(world: &mut World, origin: [i64; 3]) -> Result<Vec2, String> {
    let ground = Vec3::from_array(origin.map(|v| v as f32)) + Vec3::new(0.5, 0.005, 0.5);
    let point = world
        .query_filtered::<(&Camera, &GlobalTransform), With<TerraCamera>>()
        .iter(world)
        .next()
        .and_then(|(camera, pose)| camera.world_to_viewport(pose, ground).ok())
        .ok_or("Saved ground is outside the real construction camera")?;
    let entity = window(world)?;
    let viewport = world
        .get::<Window>(entity)
        .ok_or("Missing saved-world viewport")?;
    if point.cmplt(Vec2::splat(4.0)).any()
        || point
            .cmpgt(Vec2::new(viewport.width() - 4.0, viewport.height() - 4.0))
            .any()
    {
        return Err(format!(
            "Saved ground projects outside viewport at {point:?}"
        ));
    }
    let physical = point * viewport.resolution.scale_factor();
    if world
        .query::<(&ElementId, &ComputedNode, &UiGlobalTransform)>()
        .iter(world)
        .any(|(id, node, pose)| {
            matches!(
                id.0.as_str(),
                "build-mode-panel"
                    | "build-mode-toolbar"
                    | "build-mode-help"
                    | "build-mode-costs"
                    | "mini-map"
                    | "kingdom-resources"
                    | "work-area-overlay"
            ) && !node.is_empty()
                && node.contains_point(*pose, physical)
        })
    {
        return Err(format!(
            "Saved ground projects behind an actual HUD panel at {point:?}"
        ));
    }
    Ok(point)
}

fn capture(world: &mut World, smoke: &mut Smoke, name: &str) {
    world
        .spawn(Screenshot::primary_window())
        .observe(save_to_disk(smoke.output.join(format!("{name}.png"))))
        .observe(|_: On<ScreenshotCaptured>, mut smoke: ResMut<Smoke>| {
            smoke.capture_pending = false;
            smoke.captures += 1;
        });
    smoke.capture_pending = true;
}

fn verify_disk(world: &World, smoke: &Smoke) -> Result<(), String> {
    let journal = terra_storage::load_edits(
        &smoke.directory,
        world.resource::<terra_core::GameConfig>().world.id,
    )?;
    let game = journal.gameplay();
    let id = smoke
        .job
        .ok_or("Missing actual saved construction job ID")?;
    let job = game
        .buildings
        .jobs
        .get(&id)
        .ok_or("Acknowledged saved-world job was not written to disk")?;
    if !game.valid()
        || job.template != HOUSE_TEMPLATE
        || job.origin != smoke.origin
        || job.facing != smoke.facing
        || Some(job.kingdom) != smoke.kingdom
        || Some(job.owner) != world.resource::<ClientSession>().uuid
        || !job.free_resources
    {
        return Err(format!(
            "Saved-world job changed during disk roundtrip: {job:?}"
        ));
    }
    Ok(())
}

fn advance(smoke: &mut Smoke) {
    smoke.step += 1;
    smoke.since = Instant::now();
    info!(
        step = smoke.step,
        "Saved construction acceptance step completed"
    );
}

fn run(world: &mut World) {
    world.resource_scope(|world, mut smoke: Mut<Smoke>| {
        if smoke.finished {
            return;
        }
        let result = drive(world, &mut smoke);
        match result {
            Ok(true) => {
                smoke.finished = true;
                info!(job = smoke.job, captures = smoke.captures, origin = ?smoke.origin,
                    "Saved construction passed: real copied-world TAB, standard-house HTML selection, orange preview, ground click, authoritative job and acknowledged disk save");
                world.write_message(AppExit::Success);
            }
            Ok(false) => {}
            Err(error) => {
                smoke.finished = true;
                error!(%error, step = smoke.step, view = ?world.resource::<BuildModeView>(),
                    "Saved construction acceptance failed");
                world.write_message(AppExit::error());
            }
        }
    });
}

fn drive(world: &mut World, smoke: &mut Smoke) -> Result<bool, String> {
    for key in smoke.keys.drain(..) {
        crate::minimap_smoke::key_event(world, key, false);
    }
    if smoke.mouse_release {
        if native_pointer(smoke) {
            os_pointer(
                world,
                smoke.cursor.ok_or("Missing native pointer position")?,
                Some(false),
            )?;
        } else {
            mouse(world, false)?;
        }
        smoke.mouse_release = false;
    }
    if smoke.started.elapsed() > Duration::from_secs(180) {
        return Err(format!(
            "Saved construction timed out at step {}",
            smoke.step
        ));
    }
    if let Some(error) = &world.resource::<ClientSession>().error {
        return Err(error.clone());
    }
    if smoke.capture_pending || smoke.since.elapsed() < Duration::from_millis(400) {
        return Ok(false);
    }
    match smoke.step {
        0 => {
            if !world.resource::<crate::bootstrap::BenchmarkUi>().ready {
                return Ok(false);
            }
            let session = world.resource::<ClientSession>();
            let Some(body) = session.confirmed_state().filter(|_| session.ready) else {
                return Ok(false);
            };
            smoke.kingdom = session.uuid.and_then(|actor| {
                session
                    .kingdoms
                    .active_for_member(actor)
                    .map(|record| record.id)
            });
            if smoke.kingdom.is_none() {
                return Err("Saved player has no actual kingdom membership".into());
            }
            smoke.previous_jobs = session.buildings.jobs.keys().copied().collect();
            info!(uuid = ?session.uuid, position = ?body.position, kingdom = ?smoke.kingdom,
                "Saved construction is using the real saved player and kingdom");
            pulse(world, smoke)?;
        }
        1 => {
            let view = world.resource::<BuildModeView>();
            if !world.resource::<ConstructionCamera>().active
                || !view.catalog
                || view.templates.is_empty()
            {
                return Ok(false);
            }
            select_template(world)?;
        }
        2 => {
            let view = world.resource::<BuildModeView>();
            if !view.placing
                || view.catalog
                || !view.preview_ready
                || view.template_id != HOUSE_TEMPLATE
            {
                return Ok(false);
            }
            if view.facing != format!("{}°", u16::from(smoke.facing) * 90) {
                crate::minimap_smoke::key_event(world, KeyCode::KeyR, true);
                smoke.keys.push(KeyCode::KeyR);
                smoke.since = Instant::now();
                return Ok(false);
            }
            if world
                .resource::<terra_world::streaming::ChunkStore>()
                .block_at([smoke.origin[0], smoke.origin[1] - 1, smoke.origin[2]])
                .is_none()
            {
                return Ok(false);
            }
            let point = ground_pointer(world, smoke.origin)?;
            smoke.cursor = Some(point);
            if native_pointer(smoke) {
                os_pointer(world, point, None)?;
            } else {
                move_cursor(world, point)?;
            }
        }
        3 => {
            let view = world.resource::<BuildModeView>();
            let expected = format!(
                "{} / {} / {}",
                smoke.origin[0], smoke.origin[1], smoke.origin[2]
            );
            if view.location != expected {
                if smoke.since.elapsed() > Duration::from_secs(5) {
                    return Err(format!(
                        "Saved ground ray selected {} instead of {}",
                        view.location, expected
                    ));
                }
                return Ok(false);
            }
            if !world
                .query::<(&Name, &Mesh3d, &Visibility)>()
                .iter(world)
                .any(|(name, _, visibility)| {
                    name.as_str() == "Construction blueprint preview"
                        && *visibility != Visibility::Hidden
                })
            {
                return Ok(false);
            }
            capture(world, smoke, "saved-construction-preview");
        }
        4 => match smoke.confirmation {
            Confirmation::Mouse => {
                mouse(world, true)?;
                smoke.mouse_release = true;
            }
            Confirmation::OsMouse => {
                os_pointer(
                    world,
                    smoke.cursor.ok_or("Missing native ground pointer")?,
                    Some(true),
                )?;
                smoke.mouse_release = true;
            }
            Confirmation::Enter => {
                crate::minimap_smoke::key_event(world, KeyCode::Enter, true);
                smoke.keys.push(KeyCode::Enter);
            }
            Confirmation::Button => {
                if !smoke.button_targeted {
                    if !world.resource::<BuildModeView>().can_place {
                        return Ok(false);
                    }
                    let point = world
                        .query::<(&ElementId, &ComputedNode, &UiGlobalTransform)>()
                        .iter(world)
                        .find(|(id, node, _)| id.0 == "build-mode-place" && !node.is_empty())
                        .map(|(_, _, pose)| pose.transform_point2(Vec2::ZERO))
                        .ok_or("Actual place button is absent")?;
                    let entity = window(world)?;
                    let scale = world
                        .get::<Window>(entity)
                        .ok_or("Missing place button window")?
                        .resolution
                        .scale_factor();
                    let point = point / scale;
                    smoke.cursor = Some(point);
                    if native_pointer(smoke) {
                        os_pointer(world, point, None)?;
                    } else {
                        move_cursor(world, point)?;
                    }
                    smoke.button_targeted = true;
                    smoke.since = Instant::now();
                    return Ok(false);
                }
                if native_pointer(smoke) {
                    os_pointer(
                        world,
                        smoke.cursor.ok_or("Missing place button pointer")?,
                        Some(true),
                    )?;
                } else {
                    mouse(world, true)?;
                }
                smoke.mouse_release = true;
            }
        },
        5 => {
            let view = world.resource::<BuildModeView>();
            if !view.pending && !view.error.is_empty() {
                return Err(format!(
                    "Real saved ground click was rejected: {}",
                    view.error
                ));
            }
            if smoke.since.elapsed() > Duration::from_secs(10)
                && !view.pending
                && view.feedback.is_empty()
            {
                let hover = world
                    .get_resource::<bevy::picking::hover::HoverMap>()
                    .map(|map| {
                        map.iter()
                            .flat_map(|(_, hits)| {
                                hits.keys().map(|entity| {
                                    format!("{entity:?}: {:?}", world.get::<ElementId>(*entity))
                                })
                            })
                            .collect::<Vec<_>>()
                    });
                return Err(format!(
                    "Selected confirmation {:?} created no placement request; actual hovered elements: {hover:?}",
                    smoke.confirmation
                ));
            }
            let session = world.resource::<ClientSession>();
            let Some(job) = session.buildings.jobs.values().find(|job| {
                !smoke.previous_jobs.contains(&job.id)
                    && Some(job.kingdom) == smoke.kingdom
                    && job.template == HOUSE_TEMPLATE
                    && job.origin == smoke.origin
                    && job.facing == smoke.facing
            }) else {
                return Ok(false);
            };
            if view.pending || view.placing || view.feedback != "Bauauftrag bestätigt" {
                return Ok(false);
            }
            if Some(job.owner) != session.uuid || !job.free_resources {
                return Err("Actual saved-world job has the wrong owner or Creative policy".into());
            }
            smoke.job = Some(job.id);
            info!(job = job.id, origin = ?job.origin, template = %job.template, confirmation=?smoke.confirmation,
                "Saved construction real placement and authoritative QueueHouse acknowledgement received");
            capture(world, smoke, "saved-construction-queued");
        }
        6 => {
            smoke.save = Some(Mutex::new(
                world
                    .resource::<terra_server::IntegratedServer>()
                    .request_save()?,
            ));
        }
        7 => {
            let reply = smoke
                .save
                .as_ref()
                .ok_or("Missing saved-world save receiver")?
                .lock()
                .map_err(|_| "Saved-world save receiver was poisoned")?
                .try_recv();
            match reply {
                Ok(result) => result?,
                Err(mpsc::TryRecvError::Empty) => return Ok(false),
                Err(mpsc::TryRecvError::Disconnected) => {
                    return Err("Saved-world save service disconnected".into());
                }
            }
            verify_disk(world, smoke)?;
            if smoke.captures != 2 {
                return Err("Saved construction did not complete both real screenshots".into());
            }
            info!(
                job = smoke.job,
                "Saved construction acknowledged save and disk reload passed"
            );
            return Ok(true);
        }
        _ => return Err("Unknown saved construction acceptance step".into()),
    }
    advance(smoke);
    Ok(false)
}
