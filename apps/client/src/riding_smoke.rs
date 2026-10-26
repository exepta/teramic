//! Short isolated real-UDP mount, seated rendering, jump and dismount fixture.
use bevy::{
    prelude::*,
    render::view::screenshot::{Screenshot, ScreenshotCaptured, save_to_disk},
};
use std::{
    path::PathBuf,
    time::{Duration, Instant},
};
use terra_core::{
    GameAction, GameConfig, GameMode, InputBinding, PlayerProfile,
    creatures::{
        Creature, CreatureBehavior, CreatureKind, CreatureNeeds,
        taming::{AnimalBond, CompanionMode},
    },
};
use terra_network::client::{ClientSession, CreatureActions, ReplicatedCreatures};

const HORSE: u64 = (1 << 62) + 117;

pub(crate) fn prepare(
    mut config: GameConfig,
) -> Result<(terra_server::IntegratedServer, Vec<u8>, PlayerProfile), String> {
    config.server.game_mode = GameMode::Survival;
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
    let path = std::env::var_os("TERRA_RIDING_SMOKE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            std::env::temp_dir().join(format!("terra-riding-{}", std::process::id()))
        });
    if path.exists() {
        return Err("Riding fixture requires a fresh isolated save".into());
    }
    let profile = PlayerProfile::load_for_client(
        PlayerProfile::selected_path(&terra_core::menu_config_directory())
            .map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    let mut journal = terra_world::edits::EditJournal::default();
    let terrain =
        terra_world::TerraGenerator::new(config.world.clone()).map_err(|e| e.to_string())?;
    let ground = terra_world::spawn::find_land_spawn(&terrain, &journal, [8, 24])?.ground;
    let position = [
        ground[0] as f64 + 0.5,
        ground[1] as f64 + 1.0,
        ground[2] as f64 - 2.0,
    ];
    let state = journal.gameplay_mut();
    state.creatures.animals.insert(
        HORSE,
        Creature {
            id: HORSE,
            kind: CreatureKind::Horse,
            home: [
                (position[0].floor() as i64).div_euclid(32),
                (position[2].floor() as i64).div_euclid(32),
            ],
            position,
            velocity: [0.0; 3],
            yaw: 0.0,
            health: 20.0,
            behavior: CreatureBehavior::Rest,
            timer: 60.0,
            age: 9000.0,
            variant: 0,
            needs: CreatureNeeds::full(CreatureKind::Horse),
        },
    );
    state.creatures.owners.insert(HORSE, profile.uuid);
    let mut bond = AnimalBond::owned(profile.uuid);
    bond.mode = CompanionMode::Wait;
    state.animal_bonds.insert(HORSE, bond);
    if !state.valid() {
        return Err("Invalid riding fixture population".into());
    }
    terra_storage::save_edits(&path, config.world.id, &journal)?;
    let (host, token) = terra_server::IntegratedServer::start_with_save(config, Some(path))?;
    Ok((host, token, profile))
}

#[derive(Resource)]
struct Inspection {
    start: Instant,
    since: Instant,
    stage: u8,
    captures: u8,
    blank: bool,
    base: [f64; 3],
    stamina: f32,
    peak: f64,
    server_peak: f64,
    jump_capture: bool,
}
pub(crate) fn install(app: &mut App) {
    if !std::env::args().any(|a| a == "--smoke-test-riding") {
        return;
    }
    app.insert_resource(Inspection {
        start: Instant::now(),
        since: Instant::now(),
        stage: 0,
        captures: 0,
        blank: false,
        base: [0.0; 3],
        stamina: 0.0,
        peak: 0.0,
        server_peak: 0.0,
        jump_capture: false,
    })
    .insert_resource(bevy::winit::WinitSettings::continuous())
    .insert_resource(terra_player::ControllerMode::Walk)
    .insert_resource(terra_player::CameraPerspective::ThirdPerson)
    .add_systems(PostStartup, setup)
    .add_systems(PreUpdate, drive.after(bevy::input::InputSystems))
    .add_systems(Last, run);
}
fn setup(world: &mut World) {
    if let Some(mut menu) = world.get_resource_mut::<terra_ui::menu::MenuState>() {
        menu.page = terra_ui::menu::MenuPage::Playing;
        menu.session_active = true;
    }
    world.resource_mut::<terra_core::DebugSettings>().enabled = false;
    world
        .resource_mut::<terra_core::GameplayConfig>()
        .minimap
        .enabled = false;
    world.write_message(terra_player::CaptureRequest(true));
}
fn binding(world: &mut World, action: GameAction, held: bool) {
    let binding = world
        .get_resource::<terra_core::ControlsConfig>()
        .map_or_else(|| action.default_binding(), |c| c.binding(action));
    let key = |world: &mut World, key| {
        let mut keys = world.resource_mut::<ButtonInput<KeyCode>>();
        if held {
            keys.press(key);
        } else {
            keys.release(key);
        }
    };
    match binding {
        InputBinding::Key(k) => key(world, k),
        InputBinding::Chord { modifier, key: k } => {
            key(world, modifier);
            key(world, k);
        }
        InputBinding::Mouse(button) => {
            let mut mouse = world.resource_mut::<ButtonInput<MouseButton>>();
            if held {
                mouse.press(button);
            } else {
                mouse.release(button);
            }
        }
    }
}
fn drive(world: &mut World) {
    world
        .resource_mut::<bevy::input::mouse::AccumulatedMouseMotion>()
        .delta = Vec2::ZERO;
    for mut w in world
        .query_filtered::<&mut Window, With<bevy::window::PrimaryWindow>>()
        .iter_mut(world)
    {
        w.bypass_change_detection().focused = true;
    }
    world
        .resource_mut::<terra_player::CameraInputState>()
        .captured = true;
    let (stage, elapsed) = {
        let s = world.resource::<Inspection>();
        (s.stage, s.since.elapsed())
    };
    let moving = stage == 4 && elapsed < Duration::from_millis(1250);
    binding(world, GameAction::Forward, moving);
    binding(world, GameAction::FlyDown, moving);
    binding(
        world,
        GameAction::Dismount,
        stage == 5 && elapsed < Duration::from_millis(150),
    );
    binding(
        world,
        GameAction::Jump,
        stage == 4 && elapsed < Duration::from_millis(150),
    );
    if stage <= 1 && world.resource::<ClientSession>().mounted.is_none() {
        let point = world
            .resource::<ReplicatedCreatures>()
            .creatures
            .get(&HORSE)
            .map(|t| Vec3::from_array(t.current.position.map(|v| v as f32)) + Vec3::Y * 0.975);
        let eye = world
            .query_filtered::<&Transform, With<terra_player::PlayerBody>>()
            .iter(world)
            .next()
            .map(|t| t.translation + Vec3::Y * terra_player::PLAYER_EYE_OFFSET);
        if let (Some(point), Some(eye)) = (point, eye) {
            let rotation = Transform::from_translation(eye)
                .looking_at(point, Vec3::Y)
                .rotation;
            for(mut transform,mut look)in world.query_filtered::<(&mut Transform,&mut terra_player::CameraLook),With<terra_player::TerraCamera>>().iter_mut(world) {
                transform.rotation=rotation;
                if !look.rotation().abs_diff_eq(rotation,0.00001) {*look=terra_player::CameraLook::default();}
            }
        }
    }
}
fn capture(world: &mut World, name: &str) -> Result<(), String> {
    let folder = std::env::var_os("TERRA_CAPTURE_DIR")
        .map(PathBuf::from)
        .ok_or("Set TERRA_CAPTURE_DIR")?;
    std::fs::create_dir_all(&folder).map_err(|e| e.to_string())?;
    world
        .spawn(Screenshot::primary_window())
        .observe(save_to_disk(folder.join(format!("riding-{name}.png"))))
        .observe(
            |event: On<ScreenshotCaptured>, mut state: ResMut<Inspection>| {
                state.blank |= event.image.data.as_ref().is_none_or(|bytes| {
                    bytes
                        .chunks_exact(4)
                        .step_by(16)
                        .filter(|p| p[..3].iter().any(|v| *v > 30))
                        .count()
                        < 64
                });
                state.captures += 1;
            },
        );
    Ok(())
}
fn mounted_model(world: &mut World) -> Result<(), String> {
    let horse_name = format!("Horse {HORSE}");
    let horse = world
        .query::<(&Name, &Transform)>()
        .iter(world)
        .find_map(|(name, t)| (name.as_str() == horse_name).then_some(*t))
        .ok_or("Mounted horse has no actual rendered mesh root")?;
    let (root, avatar) = world
        .query::<(Entity, &Name, &Transform)>()
        .iter(world)
        .find_map(|(e, n, t)| (n.as_str() == "Local player avatar").then_some((e, *t)))
        .ok_or("Third-person rider model is missing")?;
    let mounted = world
        .resource::<ClientSession>()
        .mounted
        .ok_or("Mount vanished before seated capture")?;
    let snapshot = mounted.horse();
    let pivot = Vec3::Y * ((snapshot.size()[1] * 0.72) as f32 / horse.scale.y);
    let seat = horse.transform_point(pivot);
    let pelvis = avatar.transform_point(Vec3::Y * -0.285);
    if !pelvis.abs_diff_eq(seat + Vec3::Y * 0.015, 0.02) {
        return Err(format!(
            "Horse/rider seat drift: pelvis {:?}, seat {:?}",
            pelvis, seat
        ));
    }
    let meshes = world.get::<Children>(root).map_or(0, |c| {
        c.iter()
            .filter(|e| world.get::<Mesh3d>(*e).is_some())
            .count()
    });
    if meshes < 6 {
        return Err("Rider has no shared articulated body meshes".into());
    }
    Ok(())
}
fn run(world: &mut World) {
    world.resource_scope(|world, mut s: Mut<Inspection>| {
        if s.stage == 255 {
            return;
        }
        if let Err(error) = step(world, &mut s) {
            error!(%error,stage=s.stage,"Native riding fixture failed");
            s.stage = 255;
            world.write_message(AppExit::error());
        }
    });
}
fn next(s: &mut Inspection, stage: u8) {
    s.stage = stage;
    s.since = Instant::now();
}
fn step(world: &mut World, s: &mut Inspection) -> Result<(), String> {
    if s.blank {
        return Err("Blank riding screenshot".into());
    }
    if s.start.elapsed() > Duration::from_secs(75) {
        return Err(format!("Timed out at stage {}", s.stage));
    }
    let session = world.resource::<ClientSession>();
    if let Some(error) = &session.error {
        return Err(error.clone());
    }
    if !session.ready
        || session.teleporting
        || !world.resource::<terra_render::RenderStatus>().ready
    {
        return Ok(());
    }
    let mounted = session.mounted;
    match s.stage {
        0 if world.resource::<terra_player::TargetCreature>().0 == Some(HORSE) => {
            world
                .resource_mut::<CreatureActions>()
                .0
                .push_back((HORSE, true));
            next(s, 1);
        }
        1 if mounted.is_some_and(|m| m.id == HORSE) => {
            next(s, 2);
        }
        2 if s.since.elapsed() > Duration::from_millis(700) => {
            mounted_model(world)?;
            capture(world, "seated-third-person")?;
            *world.resource_mut::<terra_player::CameraPerspective>() =
                terra_player::CameraPerspective::FirstPerson;
            next(s, 3);
        }
        3 if s.since.elapsed() > Duration::from_millis(700) => {
            capture(world, "seated-first-person")?;
            *world.resource_mut::<terra_player::CameraPerspective>() =
                terra_player::CameraPerspective::ThirdPerson;
            let m = mounted.ok_or("Mount lost before driving")?;
            s.base = m.position;
            s.stamina = m.stamina;
            s.peak = 0.0;
            s.server_peak = 0.0;
            next(s, 4);
        }
        4 => {
            let m = mounted.ok_or("Mount lost during controller jump")?;
            s.peak = s.peak.max(m.position[1] - s.base[1]);
            if let Some(track) = world
                .resource::<ReplicatedCreatures>()
                .creatures
                .get(&HORSE)
            {
                s.server_peak = s.server_peak.max(track.current.position[1] - s.base[1]);
            }
            if s.server_peak > 0.3 && !s.jump_capture {
                mounted_model(world)?;
                capture(world, "sprint-jump")?;
                s.jump_capture = true;
            }
            if s.since.elapsed() > Duration::from_secs(3) {
                if s.peak < 0.4
                    || s.server_peak < 0.3
                    || (m.position[0] - s.base[0]).hypot(m.position[2] - s.base[2]) < 2.0
                    || m.stamina >= s.stamina
                {
                    return Err(format!(
                        "Physical ride failed: predictedJump {}, serverJump {}, stamina {} -> {}, position {:?} -> {:?}",
                        s.peak, s.server_peak, s.stamina, m.stamina, s.base, m.position
                    ));
                }
                next(s, 5);
            }
        }
        5 if mounted.is_none() && s.since.elapsed() > Duration::from_millis(700) => {
            capture(world, "dismounted")?;
            next(s, 6);
        }
        6 if s.captures == 4 && s.since.elapsed() > Duration::from_millis(700) => {
            if let Some(host) = world.remove_resource::<terra_server::IntegratedServer>() {
                host.finish()?;
            }
            info!(
                predicted_jump = s.peak,
                authoritative_jump = s.server_peak,
                "Native riding passed: real UDP mount, pivot-locked third/first-person captures, configured forward/Shift-sprint/jump with authoritative displacement/stamina and X dismount"
            );
            s.stage = 255;
            world.write_message(AppExit::Success);
        }
        _ => {}
    }
    Ok(())
}
