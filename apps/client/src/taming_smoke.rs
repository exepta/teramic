//! Short real-UDP feeding, persistent ownership and target-HUD inspection.
use bevy::{
    prelude::*,
    render::view::screenshot::{Screenshot, ScreenshotCaptured, save_to_disk},
};
use std::{
    path::PathBuf,
    time::{Duration, Instant},
};
use terra_core::creatures::taming::{AnimalBond, CompanionMode};
use terra_core::creatures::{Creature, CreatureBehavior, CreatureKind, CreatureNeeds};
use terra_core::{CropItem, GameConfig, GameMode, ItemKind, PlayerProfile};
use terra_network::client::{ClientSession, CreatureActions, ReplicatedCreatures};

const COW: u64 = (1 << 62) + 113;
const LEGACY_COW: u64 = COW + 1;
const LEGACY_OFFSET: Vec3 = Vec3::new(8.0, 0.0, 0.0);
const WHEAT: ItemKind = ItemKind::Crop(CropItem::Wheat);

fn fixture_cows(ground: [i64; 3]) -> (Creature, Creature) {
    let animal = Creature {
        id: COW,
        kind: CreatureKind::Cow,
        home: [ground[0].div_euclid(32), ground[2].div_euclid(32)],
        position: [
            ground[0] as f64 + 0.5,
            ground[1] as f64 + 1.0,
            ground[2] as f64 - 2.7,
        ],
        velocity: [0.0; 3],
        yaw: std::f32::consts::PI,
        health: CreatureKind::Cow.health(),
        behavior: CreatureBehavior::Rest,
        timer: 60.0,
        age: 0.0,
        variant: 0,
        needs: CreatureNeeds::full(CreatureKind::Cow),
    };
    let mut legacy = animal.clone();
    legacy.id = LEGACY_COW;
    legacy.position[0] += f64::from(LEGACY_OFFSET.x);
    (animal, legacy)
}

pub(crate) fn prepare(
    mut config: GameConfig,
) -> Result<(terra_server::IntegratedServer, Vec<u8>, PlayerProfile), String> {
    config.server.game_mode = GameMode::Creative;
    config.server.natural_mob_limit = Some(0);
    config.world.biomes = false;
    config.world.landscape_revision = 0;
    config.world.trees = false;
    config.world.caves = false;
    config.world.rivers = false;
    config.world.water = false;
    config.world.base_height = 16.0;
    config.world.hill_amplitude = 0.0;
    config.world.mountain_amplitude = 0.0;
    config.world.time.paused = true;
    config.world.time.start_hour = 12.0;
    let path = std::env::var_os("TERRA_TAMING_SMOKE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            std::env::temp_dir().join(format!("terra-taming-{}", std::process::id()))
        });
    if path.exists() {
        return Err("Taming fixture requires a fresh isolated save directory".into());
    }
    let profile = PlayerProfile::load_for_client(
        PlayerProfile::selected_path(&terra_core::menu_config_directory())
            .map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())?;
    let mut journal = terra_world::edits::EditJournal::default();
    let terrain = terra_world::TerraGenerator::new(config.world.clone())
        .map_err(|error| error.to_string())?;
    let spawn = terra_world::spawn::find_land_spawn(&terrain, &journal, [8, 24])?;
    let (animal, legacy) = fixture_cows(spawn.ground);
    if !animal.valid() || !legacy.valid() {
        return Err("Invalid native taming fixture animal".into());
    }
    journal.gameplay_mut().creatures.animals.insert(COW, animal);
    journal
        .gameplay_mut()
        .creatures
        .animals
        .insert(LEGACY_COW, legacy);
    journal
        .gameplay_mut()
        .creatures
        .owners
        .insert(LEGACY_COW, profile.uuid);
    journal.gameplay_mut().animal_bonds.insert(
        LEGACY_COW,
        AnimalBond {
            player: profile.uuid,
            trust: 100,
            mode: CompanionMode::Follow,
            last_fed_age: None,
        },
    );
    journal
        .gameplay_mut()
        .inventories
        .entry(profile.uuid)
        .or_default()
        .insert(WHEAT, 8);
    terra_storage::save_edits(&path, config.world.id, &journal)?;
    let (host, token) = terra_server::IntegratedServer::start_with_save(config, Some(path))?;
    Ok((host, token, profile))
}

#[derive(Resource)]
struct Inspection {
    start: Instant,
    since: Instant,
    stage: u8,
    expected_trust: u8,
    feed_age: f64,
    captures: usize,
    bad_capture: bool,
    wait_position: Vec3,
    destination: Vec3,
    legacy_origin: Vec3,
    rendered_match: Option<(u8, bool)>,
    rendered_frames: u8,
    drained_frames: u8,
}

pub(crate) fn install(app: &mut App) {
    if !std::env::args().any(|arg| arg == "--smoke-test-taming") {
        return;
    }
    app.insert_resource(Inspection {
        start: Instant::now(),
        since: Instant::now(),
        stage: 0,
        expected_trust: 0,
        feed_age: -10.0,
        captures: 0,
        bad_capture: false,
        wait_position: Vec3::ZERO,
        destination: Vec3::ZERO,
        legacy_origin: Vec3::ZERO,
        rendered_match: None,
        rendered_frames: 0,
        drained_frames: 0,
    })
    .insert_resource(bevy::winit::WinitSettings::continuous())
    .insert_resource(terra_player::ControllerMode::Walk)
    .add_systems(PostStartup, setup)
    .add_systems(PreUpdate, aim.after(bevy::input::InputSystems))
    .add_systems(Last, run);
}

fn setup(world: &mut World) {
    let mut menu = world.resource_mut::<terra_ui::menu::MenuState>();
    menu.page = terra_ui::menu::MenuPage::Playing;
    menu.session_active = true;
    world.resource_mut::<terra_core::DebugSettings>().enabled = false;
    world
        .resource_mut::<terra_core::GameplayConfig>()
        .minimap
        .enabled = false;
    world.write_message(terra_player::CaptureRequest(true));
}

fn animal_position(world: &World, id: u64) -> Option<Vec3> {
    world
        .resource::<ReplicatedCreatures>()
        .creatures
        .get(&id)
        .map(|track| Vec3::from_array(track.current.position.map(|value| value as f32)))
}

fn position(world: &World) -> Option<Vec3> {
    animal_position(world, COW)
}

fn aim(world: &mut World) {
    world
        .resource_mut::<bevy::input::mouse::AccumulatedMouseMotion>()
        .delta = Vec2::ZERO;
    for mut window in world
        .query_filtered::<&mut Window, With<bevy::window::PrimaryWindow>>()
        .iter_mut(world)
    {
        window.bypass_change_detection().focused = true;
    }
    world
        .resource_mut::<terra_player::CameraInputState>()
        .captured = true;
    let Some(point) = position(world).map(|point| point + Vec3::Y * 0.6) else {
        return;
    };
    let eye = world
        .query_filtered::<&Transform, With<terra_player::PlayerBody>>()
        .iter(world)
        .next()
        .map(|body| body.translation + Vec3::Y * terra_player::PLAYER_EYE_OFFSET);
    for (mut camera, mut look) in world
        .query_filtered::<(&mut Transform, &mut terra_player::CameraLook), With<terra_player::TerraCamera>>()
        .iter_mut(world)
    {
        let rotation = Transform::from_translation(eye.unwrap_or(camera.translation))
            .looking_at(point, Vec3::Y).rotation;
        camera.rotation = rotation;
        // The production camera's external-aim sync runs before fixed input and targeting.
        if !look.rotation().abs_diff_eq(rotation, 0.000_01) {
            *look = terra_player::CameraLook::default();
        }
    }
}

fn aim_ready(world: &mut World) -> bool {
    let Some(point) = position(world).map(|point| point + Vec3::Y * 0.6) else {
        return false;
    };
    let Some(eye) = world
        .query_filtered::<&Transform, With<terra_player::PlayerBody>>()
        .iter(world)
        .next()
        .map(|body| body.translation + Vec3::Y * terra_player::PLAYER_EYE_OFFSET)
    else {
        return false;
    };
    let direction = (point - eye).normalize_or_zero();
    let local = world
        .query_filtered::<&terra_player::CameraLook, With<terra_player::TerraCamera>>()
        .iter(world)
        .next()
        .is_some_and(|look| (look.rotation() * Vec3::NEG_Z).dot(direction) >= 0.999_5);
    let look = world.resource::<ClientSession>().look;
    local
        && (Quat::from_euler(EulerRot::YXZ, look[0], look[1], 0.0) * Vec3::NEG_Z).dot(direction)
            >= 0.999_5
}

fn advance(state: &mut Inspection, stage: u8) {
    state.stage = stage;
    state.since = Instant::now();
    info!(stage, trust = state.expected_trust, "Native taming stage");
}

fn capture(world: &mut World, name: &str) -> Result<(), String> {
    let folder = std::env::var_os("TERRA_CAPTURE_DIR")
        .map(PathBuf::from)
        .ok_or("Set TERRA_CAPTURE_DIR for taming inspection")?;
    std::fs::create_dir_all(&folder).map_err(|error| error.to_string())?;
    world
        .spawn(Screenshot::primary_window())
        .observe(save_to_disk(folder.join(format!("taming-{name}.png"))))
        .observe(
            |event: On<ScreenshotCaptured>, mut state: ResMut<Inspection>| {
                state.bad_capture |= event.image.data.as_ref().is_none_or(|data| {
                    data.chunks_exact(4)
                        .step_by(16)
                        .filter(|pixel| pixel[..3].iter().any(|value| *value > 30))
                        .count()
                        < 1_000
                });
                state.captures += 1;
            },
        );
    Ok(())
}

fn hud(world: &World, trust: u8, owned: bool) -> bool {
    let view = world.resource::<terra_ui::TargetView>();
    view.visible
        && view.mob
        && world.resource::<terra_player::TargetCreature>().0 == Some(COW)
        && view
            .stats
            .iter()
            .any(|row| row.id == "animal-trust" && row.bar && row.value == f32::from(trust))
        && view
            .stats
            .iter()
            .any(|row| row.id == "animal-owner" && row.text == if owned { "Du" } else { "Wild" })
        && view.stats.iter().all(|row| row.id != "animal-companion")
}

fn rendered_target_text(world: &mut World) -> Option<Vec<String>> {
    let card = world
        .query::<(Entity, &tilt_ui::ElementId, &ComputedNode)>()
        .iter(world)
        .find(|(_, id, node)| id.0 == "target-card" && !node.is_empty())
        .map(|(entity, _, _)| entity)?;
    let mut pending = vec![card];
    let mut text = Vec::new();
    while let Some(entity) = pending.pop() {
        if let Some(children) = world.get::<Children>(entity) {
            pending.extend(children.iter());
        }
        if let Some(value) = world.get::<Text>(entity) {
            text.push(value.0.trim().to_owned());
        }
    }
    Some(text)
}

fn rendered_labels_match(text: &[String], trust: u8, owned: bool) -> bool {
    let has = |value: &str| text.iter().any(|text| text == value);
    has("Vertrauen")
        && has("Besitzer")
        && has(&format!("{trust} / 100"))
        && has(if owned { "Du" } else { "Wild" })
        && !has("Begleiter")
        && !has(CompanionMode::Follow.label())
        && !has(CompanionMode::Wait.label())
}

fn feed(world: &mut World, state: &mut Inspection) -> Result<bool, String> {
    let creatures = world.resource::<ReplicatedCreatures>();
    let animal = &creatures
        .creatures
        .get(&COW)
        .ok_or("Fixture cow vanished")?
        .current;
    if animal.age - state.feed_age < 1.6
        || world.resource::<terra_player::TargetCreature>().0 != Some(COW)
    {
        return Ok(false);
    }
    state.feed_age = animal.age;
    state.expected_trust += 25;
    world
        .resource_mut::<CreatureActions>()
        .0
        .push_back((COW, true));
    Ok(true)
}

fn feed_acknowledged(trust: Option<u8>, wheat: u16, expected_trust: u8) -> bool {
    trust == Some(expected_trust) && wheat == 8 - u16::from(expected_trust / 25)
}

fn screenshot_drain_frame(previous: u8, pending: usize) -> u8 {
    if pending == 0 {
        previous.saturating_add(1).min(2)
    } else {
        0
    }
}

fn run(world: &mut World) {
    world.resource_scope(|world, mut state: Mut<Inspection>| {
        if state.start.elapsed() > Duration::from_secs(115) {
            error!(stage = state.stage, "Native taming fixture timed out");
            world.write_message(AppExit::error());
            return;
        }
        if let Err(error) = step(world, &mut state) {
            error!(%error, stage = state.stage, "Native taming fixture failed");
            world.write_message(AppExit::error());
        }
    });
}

fn step(world: &mut World, state: &mut Inspection) -> Result<(), String> {
    if state.bad_capture {
        return Err("Blank native taming screenshot".into());
    }
    let pending_captures = if state.stage == 9 {
        world.query::<&Screenshot>().iter(world).count()
    } else {
        0
    };
    if state.stage == 9 && state.since.elapsed() > Duration::from_secs(5) {
        return Err(format!(
            "Screenshot drain timed out: captured {}, pending {pending_captures}, screenshot-free frames {}, queued item previews {:?}",
            state.captures,
            state.drained_frames,
            world
                .get_resource::<terra_render::RenderBlockPreviews>()
                .map(|previews| previews.pending),
        ));
    }
    if state.stage == 2 && state.since.elapsed() > Duration::from_secs(5) {
        let session = world.resource::<ClientSession>();
        let trust = world
            .resource::<ReplicatedCreatures>()
            .bonds
            .get(&COW)
            .map(|bond| bond.trust);
        let wheat = session.inventory.count(WHEAT);
        if !feed_acknowledged(trust, wheat, state.expected_trust) {
            return Err(format!(
                "Feed acknowledgement timed out: trust {trust:?}, expected {}; wheat {wheat}, expected {}; inventory revision {}",
                state.expected_trust,
                8 - u16::from(state.expected_trust / 25),
                session.inventory.revision,
            ));
        }
    }
    let Some(rendered) = rendered_target_text(world) else {
        return Ok(());
    };
    let expected = match state.stage {
        0 => Some((0, false)),
        2 => Some((state.expected_trust, state.expected_trust == 100)),
        5 | 7 | 8 => Some((100, true)),
        _ => None,
    };
    if let Some((trust, owned)) = expected {
        if !rendered_labels_match(&rendered, trust, owned) {
            state.rendered_match = None;
            state.rendered_frames = 0;
            return Ok(());
        }
        if state.rendered_match == expected {
            state.rendered_frames = state.rendered_frames.saturating_add(1).min(2);
        } else {
            state.rendered_match = expected;
            state.rendered_frames = 1;
        }
        if state.rendered_frames < 2 {
            return Ok(());
        }
    } else {
        state.rendered_match = None;
        state.rendered_frames = 0;
    }
    if !aim_ready(world) {
        return Ok(());
    }
    let session = world.resource::<ClientSession>();
    if let Some(error) = session.error.as_ref() {
        return Err(error.clone());
    }
    if !session.ready
        || session.teleporting
        || !world.resource::<terra_render::RenderStatus>().ready
    {
        return Ok(());
    }
    let Some(cow) = position(world) else {
        return Ok(());
    };
    let Some(legacy) = animal_position(world, LEGACY_COW) else {
        return Ok(());
    };
    let creatures = world.resource::<ReplicatedCreatures>();
    let Some(legacy_bond) = creatures.bonds.get(&LEGACY_COW) else {
        return Ok(());
    };
    if legacy_bond.owner != session.uuid
        || legacy_bond.trust != 100
        || legacy_bond.mode != CompanionMode::Wait
    {
        return Err("Saved Follow ownership did not replicate as inactive Wait".into());
    }
    if state.stage > 0 && legacy.distance(state.legacy_origin) > 0.15 {
        return Err("Saved Follow reactivated movement after loading".into());
    }
    match state.stage {
        0 if hud(world, 0, false) => {
            if session.inventory.count(WHEAT) != 8 {
                return Err("Prepared wheat did not replicate".into());
            }
            state.legacy_origin = cow + LEGACY_OFFSET;
            if legacy.distance(state.legacy_origin) > 0.15 {
                return Err("Saved Follow moved before the first replicated snapshot".into());
            }
            world.write_message(terra_ui::InventoryAction::Hotbar(0));
            capture(world, "wild")?;
            advance(state, 1);
        }
        1 if state.captures >= 1 && state.since.elapsed() > Duration::from_millis(600) => {
            let sent = feed(world, state)?;
            if sent {
                advance(state, 2);
            }
        }
        2 => {
            let bond = world
                .resource::<ReplicatedCreatures>()
                .bonds
                .get(&COW)
                .copied();
            if feed_acknowledged(
                bond.map(|bond| bond.trust),
                session.inventory.count(WHEAT),
                state.expected_trust,
            ) && hud(world, state.expected_trust, state.expected_trust == 100)
            {
                if state.expected_trust == 25 {
                    capture(world, "trust-25")?;
                    advance(state, 3);
                } else if state.expected_trust < 100 {
                    advance(state, 3);
                } else {
                    if bond.and_then(|bond| bond.owner) != session.uuid {
                        return Err("Tamed cow has the wrong owner UUID".into());
                    }
                    if bond.is_none_or(|bond| bond.mode != CompanionMode::Wait) {
                        return Err("Newly tamed cow unexpectedly enabled Follow".into());
                    }
                    state.wait_position = cow;
                    capture(world, "owned")?;
                    world.write_message(terra_ui::InventoryAction::Hotbar(1));
                    advance(state, 4);
                }
            }
        }
        3 if state.captures >= 2 => {
            let sent = feed(world, state)?;
            if sent {
                advance(state, 2);
            }
        }
        4 if state.captures >= 3 && state.since.elapsed() > Duration::from_millis(750) => {
            if world
                .resource::<terra_network::client::SelectedInventorySlot>()
                .0
                != Some(1)
                || world
                    .resource::<terra_player::interaction::ActiveItem>()
                    .0
                    .is_some()
                || session.inventory.slot(1).is_some()
            {
                return Err("Empty-hand interaction did not use a real empty selected slot".into());
            }
            world
                .resource_mut::<CreatureActions>()
                .0
                .push_back((COW, true));
            advance(state, 5);
        }
        5 if state.since.elapsed() > Duration::from_secs(1) && hud(world, 100, true) => {
            if creatures
                .bonds
                .get(&COW)
                .is_none_or(|bond| bond.mode != CompanionMode::Wait)
                || cow.distance(state.wait_position) > 0.15
            {
                return Err("Empty-hand interaction reactivated companion behavior".into());
            }
            capture(world, "empty-hand-no-toggle")?;
            advance(state, 6);
        }
        6 if state.captures >= 4 => {
            state.destination = cow + Vec3::new(4.05, 0.02, 0.0);
            world
                .resource_mut::<terra_network::client::ClientChatCommands>()
                .0
                .push_back(terra_network::client::ChatCommand::Send(format!(
                    "/tp {} {} {}",
                    state.destination.x, state.destination.y, state.destination.z
                )));
            advance(state, 7);
        }
        7 if state.since.elapsed() > Duration::from_secs(2) => {
            let body = world
                .query_filtered::<&Transform, With<terra_player::PlayerBody>>()
                .single(world)
                .map_err(|error| error.to_string())?
                .translation;
            let feet = body - Vec3::Y * terra_physics::character::PLAYER_HALF_HEIGHT;
            if feet.distance(state.destination) > 0.25 {
                return Ok(());
            }
            if cow.distance(state.wait_position) > 0.15 {
                return Err("Owned cow followed its moving owner".into());
            }
            if !hud(world, 100, true) {
                return Ok(());
            }
            world
                .resource_mut::<CreatureActions>()
                .0
                .push_back((COW, true));
            advance(state, 8);
        }
        8 if state.since.elapsed() > Duration::from_secs(2) && hud(world, 100, true) => {
            if creatures
                .bonds
                .get(&COW)
                .is_none_or(|bond| bond.mode != CompanionMode::Wait)
                || cow.distance(state.wait_position) > 0.15
            {
                return Err("Second empty-hand interaction reactivated Follow".into());
            }
            capture(world, "owner-moved-no-follow")?;
            advance(state, 9);
        }
        9 if state.captures >= 5 && state.since.elapsed() > Duration::from_millis(600) => {
            if session.inventory.count(WHEAT) != 4 {
                return Err("Empty-hand commands consumed feed".into());
            }
            state.drained_frames = screenshot_drain_frame(state.drained_frames, pending_captures);
            if state.drained_frames < 2 {
                return Ok(());
            }
            info!(
                captures = state.captures,
                trust = 100,
                feed_consumed = 4,
                cow_distance = cow.distance(state.wait_position),
                legacy_distance = legacy.distance(state.legacy_origin),
                "Native F05 passed: real UDP food consumption, trust, owner, inactive legacy Follow, inert empty-hand interactions and target HUD"
            );
            world.write_message(AppExit::Success);
        }
        _ => {}
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn captures_reject_a_stale_rendered_card_even_after_logical_ownership_changes() {
        let stale = ["Vertrauen", "75 / 100", "Besitzer", "Wild"].map(str::to_owned);
        assert!(!rendered_labels_match(&stale, 100, true));
        let fresh = ["Vertrauen", "100 / 100", "Besitzer", "Du"].map(str::to_owned);
        assert!(rendered_labels_match(&fresh, 100, true));
        assert!(!rendered_labels_match(&fresh, 75, false));
        assert!(rendered_labels_match(&stale, 75, false));
        for obsolete in ["Begleiter", "Folgt", "Wartet"] {
            let mut text = fresh.to_vec();
            text.push(obsolete.to_owned());
            assert!(!rendered_labels_match(&text, 100, true));
        }
    }
    #[test]
    fn fixture_food_and_identity_use_the_real_registered_taming_rules() {
        assert!(terra_core::creatures::spawning::is_command_id(COW));
        assert!(terra_core::creatures::spawning::is_command_id(LEGACY_COW));
        assert_eq!(
            terra_core::creatures::taming::food_gain(CreatureKind::Cow, WHEAT),
            Some(25)
        );
        assert_eq!(
            100 / terra_core::creatures::taming::food_gain(CreatureKind::Cow, WHEAT).unwrap(),
            4
        );
    }

    #[test]
    fn prepared_cows_respect_saved_creature_validation_limits() {
        for ground in [[8, 16, 24], [-48, 16, -24]] {
            let (cow, legacy) = fixture_cows(ground);
            assert!(cow.valid());
            assert!(legacy.valid());
            assert_eq!(cow.timer, 60.0);
            assert_eq!(legacy.position[0] - cow.position[0], 8.0);
            let mut population = terra_core::creatures::CreaturePopulation::default();
            population.animals.insert(COW, cow);
            population.animals.insert(LEGACY_COW, legacy);
            assert!(population.valid());
        }
    }

    #[test]
    fn feeding_waits_for_both_replicated_snapshots_without_relaxing_exact_consumption() {
        assert!(!feed_acknowledged(Some(25), 8, 25));
        assert!(!feed_acknowledged(None, 7, 25));
        assert!(!feed_acknowledged(Some(0), 7, 25));
        assert!(!feed_acknowledged(Some(25), 6, 25));
        assert!(feed_acknowledged(Some(25), 7, 25));
        assert!(!feed_acknowledged(Some(75), 4, 100));
        assert!(!feed_acknowledged(Some(100), 5, 100));
        assert!(!feed_acknowledged(Some(100), 3, 100));
        assert!(feed_acknowledged(Some(100), 4, 100));
    }

    #[test]
    fn fixture_exit_waits_for_two_consecutive_screenshot_free_frames() {
        let mut frames = screenshot_drain_frame(0, 0);
        assert_eq!(frames, 1);
        frames = screenshot_drain_frame(frames, 1);
        assert_eq!(frames, 0);
        frames = screenshot_drain_frame(frames, 0);
        assert_eq!(frames, 1);
        frames = screenshot_drain_frame(frames, 0);
        assert_eq!(frames, 2);
        assert_eq!(screenshot_drain_frame(frames, 0), 2);
        assert_eq!(screenshot_drain_frame(frames, 2), 0);
    }
}
