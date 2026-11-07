//! Opt-in real Bergfried placement, UI lifecycle and persistent reload acceptance.
use bevy::{
    app::AppExit,
    prelude::*,
    render::view::screenshot::{Screenshot, ScreenshotCaptured, save_to_disk},
};
use std::{
    path::PathBuf,
    sync::{Mutex, mpsc},
    time::{Duration, Instant},
};
use terra_core::{
    Config, GameAction, InputBinding,
    kingdom::{KingdomId, KingdomState},
};
use terra_network::{client::ClientSession, protocol::session::EditOutcome};
use terra_player::{CameraInputState, TerraCamera, WorldTarget};
use terra_ui::{ElementId, KingdomMenuAction, KingdomView};
use terra_world::streaming::{BlockEditRequest, ChunkStore};

const ORIGINAL: &str = "Northwatch";
const RENAMED: &str = "Northwatch Keep";

pub(crate) fn requested() -> bool {
    (std::env::var("TERRA_KINGDOM_SMOKE").as_deref() == Ok("1")
        || crate::kingdom_ui_smoke::requested())
        && std::env::args().any(|arg| arg == "--smoke-test-inventory")
}

pub(crate) fn prepare(
    mut config: terra_core::GameConfig,
) -> Result<
    (
        terra_server::IntegratedServer,
        Vec<u8>,
        terra_core::PlayerProfile,
    ),
    String,
> {
    let root = std::env::var_os("TERRA_KINGDOM_SAVE_DIR")
        .map(PathBuf::from)
        .ok_or("Kingdom acceptance requires an isolated TERRA_KINGDOM_SAVE_DIR")?;
    if !root.is_absolute() {
        return Err("The kingdom acceptance save path must be absolute".into());
    }
    let path = terra_core::PlayerProfile::selected_path(&terra_core::menu_config_directory())
        .map_err(|error| error.to_string())?;
    let profile = terra_core::PlayerProfile::load_or_create(path).map_err(|e| e.to_string())?;
    config.server.game_mode = terra_core::GameMode::Creative;
    config.server.player_permissions.insert(
        profile.uuid,
        terra_core::PlayerPermissions {
            build: true,
            flight: true,
            game_mode: true,
            environment: true,
        },
    );
    let (host, token) = terra_server::IntegratedServer::start_with_save(config, Some(root))?;
    Ok((host, token, profile))
}

#[derive(Resource)]
struct Fixture {
    started: Instant,
    entered: Instant,
    stage: u8,
    keep: crate::inventory_benchmark::chest::ChestFixture,
    position: [i64; 3],
    id: Option<KingdomId>,
    reload: bool,
    release: Option<(Entity, u8)>,
    use_held: bool,
    old_edit: u64,
    look_ack: Option<u64>,
    captures: usize,
    capture_failed: bool,
    save: Option<Mutex<mpsc::Receiver<Result<(), String>>>>,
    saved: bool,
}

pub(crate) fn install(app: &mut App) {
    let now = Instant::now();
    app.insert_resource(Fixture {
        started: now,
        entered: now,
        stage: 0,
        keep: crate::inventory_benchmark::chest::ChestFixture::for_material(
            terra_core::castle::BERGFRIED,
        ),
        position: [0; 3],
        id: None,
        reload: std::env::var("TERRA_KINGDOM_RELOAD").as_deref() == Ok("1"),
        release: None,
        use_held: false,
        old_edit: 0,
        look_ack: None,
        captures: 0,
        capture_failed: false,
        save: None,
        saved: false,
    })
    .insert_resource(bevy::winit::WinitSettings::continuous())
    .add_systems(
        PreUpdate,
        run.after(bevy::input::InputSystems)
            .before(terra_ui::KingdomInputSet),
    );
}

fn advance(state: &mut Fixture, stage: u8) {
    state.stage = stage;
    state.entered = Instant::now();
    state.look_ack = None;
}

fn element(world: &mut World, id: &str) -> Option<Entity> {
    world
        .query::<(Entity, &ElementId, &ComputedNode)>()
        .iter(world)
        .find_map(|(entity, name, node)| {
            (name.0 == id && node.size().min_element() > 0.0).then_some(entity)
        })
}

fn click(world: &mut World, state: &mut Fixture, id: &str, next: u8) -> Result<(), String> {
    let target = element(world, id).ok_or_else(|| format!("Missing kingdom control {id}"))?;
    crate::inventory_smoke::pointer(world, target, Some(true))?;
    state.release = Some((target, next));
    state.entered = Instant::now();
    Ok(())
}

fn shot(world: &mut World, name: &str) -> Result<(), String> {
    let root = std::env::var_os("TERRA_CAPTURE_DIR")
        .map(PathBuf::from)
        .ok_or("Set TERRA_CAPTURE_DIR for kingdom acceptance")?;
    std::fs::create_dir_all(&root).map_err(|e| e.to_string())?;
    world
        .spawn(Screenshot::primary_window())
        .observe(save_to_disk(root.join(format!("{name}.png"))))
        .observe(
            |event: On<ScreenshotCaptured>, mut state: ResMut<Fixture>| {
                let colors = event
                    .image
                    .data
                    .as_deref()
                    .unwrap_or_default()
                    .chunks_exact(4)
                    .step_by(97)
                    .take(4096)
                    .map(|pixel| [pixel[0], pixel[1], pixel[2]])
                    .collect::<std::collections::HashSet<_>>();
                state.capture_failed |= colors.len() < 16;
                state.captures += 1;
            },
        );
    Ok(())
}

fn use_binding(world: &mut World, pressed: bool) {
    let binding = world
        .get_resource::<terra_core::ControlsConfig>()
        .map_or_else(
            || GameAction::Use.default_binding(),
            |c| c.binding(GameAction::Use),
        );
    let key = |world: &mut World, code| {
        let mut keys = world.resource_mut::<ButtonInput<KeyCode>>();
        if pressed {
            keys.press(code);
        } else {
            keys.release(code);
        }
    };
    match binding {
        InputBinding::Key(code) => key(world, code),
        InputBinding::Chord {
            modifier,
            key: code,
        } => {
            key(world, modifier);
            key(world, code);
        }
        InputBinding::Mouse(button) => {
            let mut mouse = world.resource_mut::<ButtonInput<MouseButton>>();
            if pressed {
                mouse.press(button);
            } else {
                mouse.release(button);
            }
        }
    }
}

fn aim(world: &mut World, position: [i64; 3]) {
    let center = Vec3::from_array(position.map(|v| v as f32 + 0.5));
    let eye = world
        .resource::<ClientSession>()
        .confirmed_state()
        .map(|pose| Vec3::from_array(pose.position) + Vec3::Y * terra_player::PLAYER_EYE_OFFSET);
    for (mut camera, mut look) in world
        .query_filtered::<(&mut Transform, &mut terra_player::CameraLook), With<TerraCamera>>()
        .iter_mut(world)
    {
        let rotation = Transform::from_translation(eye.unwrap_or(camera.translation))
            .looking_at(center, Vec3::Y)
            .rotation;
        camera.rotation = rotation;
        if !look.rotation().abs_diff_eq(rotation, 0.00001) {
            *look = default();
        }
    }
}

/// Wait for ordinary movement packets to acknowledge the aligned real look.
/// Neither the server pose nor the replicated kingdom state is modified here.
fn gallery_aim_ready(world: &World, state: &mut Fixture) -> bool {
    let session = world.resource::<ClientSession>();
    let Some(pose) = session.confirmed_state() else {
        return false;
    };
    let eye = Vec3::from_array(pose.position) + Vec3::Y * terra_player::PLAYER_EYE_OFFSET;
    let expected =
        (Vec3::from_array(state.position.map(|v| v as f32 + 0.5)) - eye).normalize_or_zero();
    let actual =
        Quat::from_euler(EulerRot::YXZ, session.look[0], session.look[1], 0.0) * Vec3::NEG_Z;
    if actual.dot(expected) < 0.9999
        || !matches!(world.resource::<terra_player::TargetWorld>().0,Some(WorldTarget::Block(hit)) if hit.position==state.position)
    {
        state.look_ack = None;
        return false;
    }
    let initial = *state.look_ack.get_or_insert(pose.ack);
    pose.ack >= initial.saturating_add(2)
}

fn request_save(world: &World, state: &mut Fixture) -> Result<(), String> {
    state.save = Some(Mutex::new(
        world
            .resource::<terra_server::IntegratedServer>()
            .request_save()?,
    ));
    state.saved = false;
    Ok(())
}

fn save_ready(state: &mut Fixture) -> Result<bool, String> {
    let result = state.save.as_ref().map(|receiver| {
        receiver
            .lock()
            .map_err(|_| "Save receiver poisoned".to_string())?
            .try_recv()
            .map_err(|error| match error {
                mpsc::TryRecvError::Empty => String::new(),
                mpsc::TryRecvError::Disconnected => "Save service disconnected".into(),
            })
    });
    match result {
        Some(Ok(result)) => {
            result?;
            state.save = None;
            state.saved = true;
        }
        Some(Err(error)) if error.is_empty() => {}
        Some(Err(error)) => return Err(error),
        None => {}
    }
    Ok(state.saved)
}

fn run(world: &mut World) {
    world.resource_scope(|world, mut state: Mut<Fixture>| {
        let result = (|| -> Result<bool, String> {
            if state.started.elapsed() > Duration::from_secs(180) {
                return Err(format!("Kingdom acceptance timed out at stage {}", state.stage));
            }
            if state.capture_failed {
                return Err("A kingdom screenshot contained no rendered UI".into());
            }
            if state.use_held {
                use_binding(world, false);
                state.use_held = false;
            }
            world.resource_mut::<CameraInputState>().captured = matches!(state.stage, 0 | 2);
            if crate::kingdom_ui_smoke::requested() && ((2..=5).contains(&state.stage) || state.stage==100) {aim(world,state.position);}
            if let Some((target, next)) = state.release {
                if state.entered.elapsed() >= Duration::from_millis(180) {
                    crate::inventory_smoke::pointer(world, target, Some(false))?;
                    state.release = None;
                    advance(&mut state, next);
                }
                return Ok(false);
            }
            if state.stage > 2 && state.entered.elapsed() < Duration::from_millis(350) {
                return Ok(false);
            }
            match state.stage {
                0 => {
                    let session = world.resource::<ClientSession>();
                    if !session.ready || session.confirmed_state().is_none()
                        || world.resource::<ChunkStore>().loaded.is_empty()
                        || world.resource::<tilt_ui::UiDocumentState>().0.load != tilt_ui::UiLoadState::Ready
                    { return Ok(false); }
                    if world.resource::<terra_ui::menu::MenuState>().page != terra_ui::menu::MenuPage::Playing {
                        world.write_message(terra_ui::menu::MenuAction::Command("resume".into()));
                        return Ok(false);
                    }
                    if state.reload {
                        let Some(uuid) = session.uuid else { return Ok(false) };
                        let Some(kingdom) = session.kingdoms.active_for_member(uuid).cloned() else { return Ok(false) };
                        if kingdom.name != RENAMED || kingdom.founder != uuid || kingdom.members != [uuid] {
                            return Err("Reloaded kingdom identity/name/members do not match the saved state".into());
                        }
                        state.position = kingdom.location;
                        state.id = Some(kingdom.id);
                    } else {
                        if !state.keep.prepare(world)? { return Ok(false); }
                        state.position = state.keep.position();
                    }
                    aim(world,state.position);
                    advance(&mut state,2);
                }
                2 => {
                    aim(world, state.position);
                    if state.entered.elapsed() < Duration::from_millis(600)
                        || !matches!(world.resource::<terra_player::TargetWorld>().0,
                            Some(WorldTarget::Block(hit)) if hit.position == state.position)
                    { return Ok(false); }
                    if crate::kingdom_ui_smoke::requested() {
                        if !gallery_aim_ready(world,&mut state){return Ok(false);}
                        world.write_message(KingdomMenuAction::Open(state.position));
                        advance(&mut state,3);return Ok(false);
                    }
                    // Pass through the real remappable gameplay interaction.
                    use_binding(world, true);
                    state.use_held = true;
                    advance(&mut state, 3);
                }
                3 => {
                    let view = world.resource::<KingdomView>();
                    if !view.visible || view.pending || element(world, "kingdom-dialog").is_none() {
                        return Ok(false);
                    }
                    let view = world.resource::<KingdomView>();
                    if !view.error.is_empty() { return Err(view.error.clone()); }
                    if state.reload {
                        if !view.bound || !view.founder_controls || view.title != RENAMED {
                            return Err("Reloaded Bergfried did not open the authoritative founder UI".into());
                        }
                        shot(world, "kingdom-reloaded")?;
                        click(world, &mut state, "kingdom-dissolve-prompt", 10)?;
                    } else {
                        if view.bound { return Err("Fresh Bergfried was already bound".into()); }
                        shot(world, "kingdom-unbound")?;
                        if crate::kingdom_ui_smoke::requested() {
                            world.write_message(KingdomMenuAction::Name(ORIGINAL.into()));
                        } else {
                            crate::inventory_smoke::type_text(world, ORIGINAL)?;
                        }
                        advance(&mut state, 4);
                    }
                }
                4 => {
                    let view = world.resource::<KingdomView>();
                    if view.name != ORIGINAL || !view.can_found { return Ok(false); }
                    if crate::kingdom_ui_smoke::requested() {
                        if !gallery_aim_ready(world,&mut state){return Ok(false);}
                        world.write_message(KingdomMenuAction::Found);
                        advance(&mut state, 5);
                    } else {
                        click(world, &mut state, "kingdom-found", 5)?;
                    }
                }
                5 => {
                    let view=world.resource::<KingdomView>();
                    if !view.pending && !view.error.is_empty(){return Err(format!("Authoritative Found rejected: {}",view.error));}
                    let session = world.resource::<ClientSession>();
                    let Some(kingdom) = session.kingdoms.active_at(state.position).cloned() else { return Ok(false) };
                    if world.resource::<KingdomView>().pending { return Ok(false); }
                    if kingdom.name != ORIGINAL || Some(kingdom.founder) != session.uuid {
                        return Err("Found result does not match authoritative kingdom identity".into());
                    }
                    state.id = Some(kingdom.id);
                    if crate::kingdom_ui_smoke::requested() {
                        advance(&mut state, 100);
                        return Ok(false);
                    }
                    state.old_edit = session.last_edit.map_or(0, |(id, _)| id);
                    shot(world, "kingdom-founded")?;
                    world.write_message(BlockEditRequest {
                        position: state.position, expected_material: terra_core::castle::BERGFRIED, material: 0,
                    });
                    advance(&mut state, 6);
                }
                6 => {
                    let session = world.resource::<ClientSession>();
                    let Some((id, outcome)) = session.last_edit else { return Ok(false) };
                    if id <= state.old_edit { return Ok(false); }
                    if outcome == EditOutcome::Applied
                        || world.resource::<ChunkStore>().block_at(state.position) != Some(terra_core::castle::BERGFRIED)
                    { return Err("The active kingdom Bergfried could be removed".into()); }
                    info!(?outcome, "Native kingdom active Bergfried removal rejected by authority");
                    world.write_message(KingdomMenuAction::Name(RENAMED.into()));
                    advance(&mut state, 7);
                }
                7 => {
                    let view = world.resource::<KingdomView>();
                    if view.name != RENAMED || !view.can_rename { return Ok(false); }
                    click(world, &mut state, "kingdom-rename", 8)?;
                }
                8 => {
                    let session = world.resource::<ClientSession>();
                    if !session.kingdoms.active_at(state.position).is_some_and(|k| k.name == RENAMED)
                        || world.resource::<KingdomView>().pending
                    { return Ok(false); }
                    shot(world, "kingdom-renamed")?;
                    request_save(world, &mut state)?;
                    advance(&mut state, 9);
                }
                9 => {
                    if !save_ready(&mut state)? || state.captures < 3 { return Ok(false); }
                    info!(id = ?state.id, position = ?state.position, captures = state.captures,
                        "Native kingdom creation passed: actual block placement and remappable use, typed name, OS Found/Rename clicks, protected keep, authoritative registry and acknowledged persistent save");
                    return Ok(true);
                }
                10 => {
                    if !world.resource::<KingdomView>().confirm_dissolve { return Ok(false); }
                    shot(world, "kingdom-dissolve-confirm")?;
                    click(world, &mut state, "kingdom-dissolve", 11)?;
                }
                11 => {
                    let session = world.resource::<ClientSession>();
                    let Some(id) = state.id else { return Err("Missing saved kingdom ID".into()) };
                    if session.kingdoms.get(id).is_none_or(|k| k.state != KingdomState::Dissolved)
                        || world.resource::<KingdomView>().pending
                    { return Ok(false); }
                    if session.kingdoms.active_for_member(session.uuid.unwrap()).is_some() {
                        return Err("Dissolved kingdom still owns membership".into());
                    }
                    shot(world, "kingdom-dissolved")?;
                    click(world, &mut state, "kingdom-close", 12)?;
                }
                12 => {
                    if world.resource::<KingdomView>().visible
                        || world.resource::<terra_player::PlayerInputBlock>().0
                    { return Ok(false); }
                    state.old_edit = world.resource::<ClientSession>().last_edit.map_or(0, |(id, _)| id);
                    world.write_message(BlockEditRequest {
                        position: state.position, expected_material: terra_core::castle::BERGFRIED, material: 0,
                    });
                    advance(&mut state, 13);
                }
                13 => {
                    let Some((id, outcome)) = world.resource::<ClientSession>().last_edit else { return Ok(false) };
                    if id <= state.old_edit { return Ok(false); }
                    if outcome != EditOutcome::Applied { return Err(format!("Dissolved Bergfried removal failed: {outcome:?}")); }
                    if world.resource::<ChunkStore>().block_at(state.position) != Some(0) { return Ok(false); }
                    request_save(world, &mut state)?;
                    advance(&mut state, 14);
                }
                14 => {
                    if !save_ready(&mut state)? || state.captures < 3 { return Ok(false); }
                    info!(id = ?state.id, captures = state.captures,
                        "Native kingdom reload passed: persisted ID/name/founder/member/location, OS dissolve confirmation, released membership, unprotected block removal and acknowledged save");
                    return Ok(true);
                }
                100 => return crate::kingdom_ui_smoke::run(world),
                _ => return Err("Unknown kingdom acceptance stage".into()),
            }
            Ok(false)
        })();
        match result {
            Ok(true) => { world.write_message(AppExit::Success); }
            Ok(false) => {}
            Err(error) => {
                error!(stage = state.stage, %error, "Native kingdom acceptance failed");
                world.write_message(AppExit::error());
            }
        }
    });
}
