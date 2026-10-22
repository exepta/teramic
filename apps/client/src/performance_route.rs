//! Opt-in real-input exploration route; prediction and authority move the body.
use bevy::{app::AppExit, prelude::*};
use std::{
    collections::HashSet,
    time::{Duration, Instant},
};
use terra_core::{ControlsConfig, GameAction, GameConfig, InputBinding};
use terra_network::client::ClientSession;
use terra_player::{CameraInputState, ControllerMode, PlayerBody, TerraCamera};
use terra_world::streaming::WorldTerrain;

#[path = "performance_route/walk.rs"]
mod walk;

const SPEED: f32 = 12.0;
const PERIOD: f64 = 120.0;
const LEG_SECONDS: f64 = 50.0;
const DISTANCE: f32 = SPEED * LEG_SECONDS as f32;

#[derive(Resource, Default)]
pub(crate) struct Route {
    walking: Option<walk::Walk>,
    delay: Duration,
    waiting_at: Option<Instant>,
    ready_at: Option<Instant>,
    cruising_at: Option<Instant>,
    origin: Vec3,
    cruise_y: f32,
    previous_phase: Option<(u64, &'static str)>,
    last_ack: u64,
    ack_at: Option<Instant>,
    maximum_outbound: f32,
    failed: bool,
}

impl Route {
    pub(crate) fn started(&self) -> bool {
        self.ready_at.is_some() || self.walking.as_ref().is_some_and(walk::Walk::started)
    }
}

pub(crate) fn install(app: &mut App) -> Result<(), String> {
    let Some(route) = std::env::var_os("TERRA_SOAK_ROUTE") else {
        return Ok(());
    };
    if route != "explore" && route != "walk" {
        return Err("TERRA_SOAK_ROUTE supports explore or walk (real networked movement)".into());
    }
    if !std::env::args().any(|arg| arg == "--smoke-test-benchmark")
        || !std::env::var("TERRA_BENCH_MODE").is_ok_and(|mode| mode == "integrated")
    {
        return Err("TERRA_SOAK_ROUTE=explore requires --smoke-test-benchmark and TERRA_BENCH_MODE=integrated".into());
    }
    let configured_delay = std::env::var("TERRA_SOAK_ROUTE_DELAY_SECONDS")
        .map(Some)
        .or_else(|error| match error {
            std::env::VarError::NotPresent => Ok(None),
            std::env::VarError::NotUnicode(_) => {
                Err("TERRA_SOAK_ROUTE_DELAY_SECONDS must contain a number".to_string())
            }
        })?;
    let delay = parse_delay(configured_delay.as_deref())?;
    app.insert_resource(Route {
        walking: (route == "walk").then(walk::Walk::default),
        delay,
        ..Default::default()
    })
    .add_systems(PreUpdate, drive.after(bevy::input::InputSystems));
    Ok(())
}

fn parse_delay(value: Option<&str>) -> Result<Duration, String> {
    let seconds = value.map_or(Ok(0.0), str::parse::<f64>).map_err(
        |_| "TERRA_SOAK_ROUTE_DELAY_SECONDS requires a finite number from 0 through 1200",
    )?;
    if !seconds.is_finite() || !(0.0..=1200.0).contains(&seconds) {
        return Err(
            "TERRA_SOAK_ROUTE_DELAY_SECONDS requires a finite number from 0 through 1200".into(),
        );
    }
    Ok(Duration::from_secs_f64(seconds))
}

fn route_target(elapsed: f64) -> (u64, &'static str, f32) {
    let cycle = (elapsed / PERIOD).floor() as u64;
    let phase = elapsed.rem_euclid(PERIOD);
    let (name, distance) = if phase < 10.0 {
        ("origin", 0.0)
    } else if phase < 60.0 {
        ("outbound", ((phase - 10.0) / LEG_SECONDS) as f32 * DISTANCE)
    } else if phase < 110.0 {
        ("return", ((110.0 - phase) / LEG_SECONDS) as f32 * DISTANCE)
    } else {
        ("origin", 0.0)
    };
    (cycle, name, distance)
}

fn drive(world: &mut World) {
    if crate::smoke_test::benchmark_ready_time(world).is_none() {
        return;
    }
    world.resource_scope(|world, mut route: Mut<Route>| {
        if route.failed {
            return;
        }
        if route.waiting_at.is_none() {
            route.waiting_at = Some(Instant::now());
            tracing::info!(
                delay_seconds = route.delay.as_secs_f64(),
                "Real-input performance route waiting at the captured benchmark pose"
            );
        }
        if route
            .waiting_at
            .is_some_and(|started| started.elapsed() < route.delay)
        {
            return;
        }
        if let Err(error) = advance(world, &mut route) {
            route.failed = true;
            tracing::error!(%error, "Real-input performance route failed");
            world.write_message(AppExit::error());
        }
    });
}

fn advance(world: &mut World, route: &mut Route) -> Result<(), String> {
    if let Some(walking) = route.walking.as_mut() {
        return walking.advance(world);
    }
    let session = world
        .get_resource::<ClientSession>()
        .ok_or("Performance route requires the real network client")?;
    if let Some(error) = &session.error {
        return Err(error.clone());
    }
    if !session.ready || session.teleporting {
        return Ok(());
    }
    if !session.allow_flight {
        return Err("Performance route requires authoritative creative flight permission".into());
    }
    let confirmed = session
        .confirmed_state()
        .ok_or("Performance route has no confirmed player state")?;
    let pending_inputs = session.pending_inputs;
    let Some(position) = world
        .query_filtered::<&Transform, With<PlayerBody>>()
        .iter(world)
        .next()
        .map(|body| body.translation)
    else {
        return Ok(());
    };
    let now = Instant::now();
    if route.ready_at.is_none() {
        route.ready_at = Some(now);
        route.origin = position;
        // A few one-time terrain probes select a safe cruise plane. The real
        // controller ascends there; neither body nor camera is teleported.
        let terrain = &world.resource::<WorldTerrain>().0;
        let highest = (0..=12)
            .map(|probe| {
                terrain.surface_height(
                    f64::from(position.x + DISTANCE * probe as f32 / 12.0),
                    f64::from(position.z),
                ) as f32
            })
            .fold(position.y, f32::max);
        route.cruise_y = (highest + 48.0).min(344.0);
        world.resource_mut::<GameConfig>().client.fly_speed = SPEED;
        tracing::info!(
            origin=?position.to_array(),
            authoritative=?confirmed.position,
            cruise_y=route.cruise_y,
            speed=SPEED,
            period=PERIOD,
            distance=DISTANCE,
            "Real-input performance route armed at the ready player's actual spawn"
        );
    }
    if route.last_ack != confirmed.ack {
        route.last_ack = confirmed.ack;
        route.ack_at = Some(now);
    } else if route
        .ack_at
        .is_some_and(|since| since.elapsed().as_secs_f64() > 15.0)
    {
        return Err("Authoritative movement acknowledgements stalled for 15 seconds".into());
    }
    let height_error = route.cruise_y - position.y;
    if route.cruising_at.is_none() && height_error.abs() <= 0.5 {
        route.cruising_at = Some(now);
    }
    if route.cruising_at.is_none()
        && route
            .ready_at
            .is_some_and(|started| started.elapsed().as_secs_f64() > 60.0)
    {
        return Err(
            "Performance route could not reach its cruise altitude within 60 seconds".into(),
        );
    }
    let (cycle, phase, distance) = route
        .cruising_at
        .map(|started| route_target(started.elapsed().as_secs_f64()))
        .unwrap_or((0, "climb", 0.0));
    let target = Vec3::new(route.origin.x + distance, route.cruise_y, route.origin.z);
    if route.previous_phase != Some((cycle, phase)) {
        if route
            .previous_phase
            .is_some_and(|(previous_cycle, _)| previous_cycle < cycle)
        {
            if route.maximum_outbound < DISTANCE * 0.75 {
                return Err(format!(
                    "Exploration reached only {:.1}m of the {DISTANCE}m route; movement was blocked",
                    route.maximum_outbound
                ));
            }
            if (position.x - route.origin.x).abs() > 8.0 {
                return Err("Exploration did not return to the original region".into());
            }
            route.maximum_outbound = 0.0;
        }
        tracing::info!(
            elapsed_ready_s=route.ready_at.map_or(0.0, |started| started.elapsed().as_secs_f64()),
            cycle,
            phase,
            position=?position.to_array(),
            authoritative=?confirmed.position,
            acknowledged_input=confirmed.ack,
            pending_inputs,
            target=?target.to_array(),
            "Real-input performance route phase"
        );
        route.previous_phase = Some((cycle, phase));
    }
    route.maximum_outbound = route.maximum_outbound.max(position.x - route.origin.x);
    if world.resource::<terra_player::PlayerInputBlock>().0 {
        return Err("Gameplay input became blocked during the exploration route".into());
    }
    *world.resource_mut::<ControllerMode>() = ControllerMode::Fly;
    world.resource_mut::<CameraInputState>().captured = true;
    for mut camera in world
        .query_filtered::<&mut Transform, With<TerraCamera>>()
        .iter_mut(world)
    {
        // read_walk_input synchronizes this aim before producing network input.
        camera.rotation = Quat::from_euler(EulerRot::YXZ, 0.0, -0.22, 0.0);
    }
    let error = target - position;
    let desired = [
        (GameAction::Right, error.x > 0.35),
        (GameAction::Left, error.x < -0.35),
        (GameAction::Backward, error.z > 0.35),
        (GameAction::Forward, error.z < -0.35),
        (GameAction::Jump, error.y > 0.35),
        (GameAction::FlyDown, error.y < -0.35),
    ];
    inject_bindings(world, &desired);
    Ok(())
}

fn inject_bindings(world: &mut World, desired: &[(GameAction, bool)]) {
    let controls = world.get_resource::<ControlsConfig>();
    let mut keys = HashSet::new();
    let mut mouse = HashSet::new();
    let mut controlled_keys = HashSet::new();
    let mut controlled_mouse = HashSet::new();
    for &(action, held) in desired {
        let binding = controls
            .and_then(|controls| controls.bindings.get(&action).copied())
            .unwrap_or_else(|| action.default_binding());
        match binding {
            InputBinding::Key(key) => {
                controlled_keys.insert(key);
                if held {
                    keys.insert(key);
                }
            }
            InputBinding::Chord { modifier, key } => {
                controlled_keys.extend([modifier, key]);
                if held {
                    keys.extend([modifier, key]);
                }
            }
            InputBinding::Mouse(button) => {
                controlled_mouse.insert(button);
                if held {
                    mouse.insert(button);
                }
            }
        }
    }
    let mut input = world.resource_mut::<ButtonInput<KeyCode>>();
    for key in controlled_keys {
        if keys.contains(&key) {
            input.press(key);
        } else {
            input.release(key);
        }
    }
    if let Some(mut input) = world.get_resource_mut::<ButtonInput<MouseButton>>() {
        for button in controlled_mouse {
            if mouse.contains(&button) {
                input.press(button);
            } else {
                input.release(button);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn route_repeats_identical_outbound_and_return_targets_every_two_minutes() {
        for elapsed in [0.0, 10.0, 30.0, 60.0, 90.0, 110.0, 119.0] {
            let (cycle, phase, position) = route_target(elapsed);
            let (next_cycle, next_phase, next_position) = route_target(elapsed + PERIOD);
            assert_eq!(cycle, 0);
            assert_eq!(next_cycle, 1);
            assert_eq!(phase, next_phase);
            assert!((position - next_position).abs() < 0.001);
        }
        assert_eq!(route_target(10.0).2, 0.0);
        assert_eq!(route_target(60.0).2, DISTANCE);
        assert_eq!(route_target(110.0).2, 0.0);
    }

    #[test]
    fn injected_gameplay_bindings_retain_held_input_without_repeated_edges() {
        let mut world = World::new();
        world.init_resource::<ControlsConfig>();
        world.init_resource::<ButtonInput<KeyCode>>();
        let desired = [(GameAction::Right, true), (GameAction::Left, false)];
        inject_bindings(&mut world, &desired);
        assert!(
            world
                .resource::<ButtonInput<KeyCode>>()
                .pressed(KeyCode::KeyD)
        );
        world.resource_mut::<ButtonInput<KeyCode>>().clear();
        inject_bindings(&mut world, &desired);
        let keys = world.resource::<ButtonInput<KeyCode>>();
        assert!(keys.pressed(KeyCode::KeyD));
        assert!(!keys.just_pressed(KeyCode::KeyD));
        assert!(!keys.pressed(KeyCode::KeyA));
    }

    #[test]
    fn route_delay_is_finite_bounded_and_optional() {
        assert_eq!(parse_delay(None).unwrap(), Duration::ZERO);
        assert_eq!(parse_delay(Some("900")).unwrap(), Duration::from_secs(900));
        assert_eq!(
            parse_delay(Some("0.25")).unwrap(),
            Duration::from_millis(250)
        );
        assert_eq!(
            parse_delay(Some("1200")).unwrap(),
            Duration::from_secs(1200)
        );
        for invalid in ["-1", "NaN", "inf", "1200.01", "invalid"] {
            assert!(parse_delay(Some(invalid)).is_err(), "accepted {invalid}");
        }
    }

    #[test]
    fn waiting_for_the_route_does_not_freeze_the_benchmark_scenario_early() {
        let mut route = Route {
            delay: Duration::from_secs(900),
            waiting_at: Some(Instant::now()),
            ..Default::default()
        };
        assert!(!route.started());
        route.ready_at = Some(Instant::now());
        assert!(route.started());
    }
}
