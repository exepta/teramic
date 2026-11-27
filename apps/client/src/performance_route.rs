//! Opt-in real-input exploration route; prediction and authority move the body.
use bevy::{app::AppExit, prelude::*};
use std::{
    collections::{HashMap, HashSet, VecDeque},
    time::{Duration, Instant},
};
use terra_core::{ControlsConfig, GameAction, GameConfig, InputBinding};
use terra_network::client::ClientSession;
use terra_player::{
    CameraInputState, ControllerMode, PLAYER_HALF_HEIGHT, PLAYER_RADIUS, PlayerBody, TerraCamera,
};
use terra_world::{
    edits::EditJournal,
    streaming::{ChunkStore, WorldTerrain},
};

#[path = "performance_route/frontier.rs"]
mod frontier;
#[path = "performance_route/walk.rs"]
mod walk;

const SPEED: f32 = 12.0;
const PERIOD: f64 = 120.0;
const LEG_SECONDS: f64 = 50.0;
const DISTANCE: f32 = SPEED * LEG_SECONDS as f32;
const APPROACH_RADIUS: i32 = 16;
const ASCENT_CLEARANCE: f32 = 32.0;
const APPROACH_TOLERANCE: f32 = 0.075;

#[derive(Resource, Default)]
pub(crate) struct Route {
    walking: Option<walk::Walk>,
    frontier: Option<frontier::Frontier>,
    speed: f32,
    delay: Duration,
    waiting_at: Option<Instant>,
    ready_at: Option<Instant>,
    cruising_at: Option<Instant>,
    origin: Vec3,
    approach: VecDeque<Vec3>,
    cruise_y: f32,
    previous_phase: Option<(u64, &'static str)>,
    last_ack: u64,
    ack_at: Option<Instant>,
    maximum_outbound: f32,
    failed: bool,
}

impl Route {
    pub(crate) fn is_frontier(&self) -> bool {
        self.frontier.is_some()
    }

    pub(crate) fn started(&self) -> bool {
        self.ready_at.is_some() || self.walking.as_ref().is_some_and(walk::Walk::started)
    }
}

pub(crate) fn install(app: &mut App) -> Result<(), String> {
    let Some(route) = std::env::var_os("TERRA_SOAK_ROUTE") else {
        return Ok(());
    };
    if route != "explore" && route != "walk" && route != "frontier" {
        return Err(
            "TERRA_SOAK_ROUTE supports explore, walk or frontier (real networked movement)".into(),
        );
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
    let speed = parse_speed(std::env::var("TERRA_SOAK_ROUTE_SPEED").ok().as_deref())?;
    let frontier = if route == "frontier" {
        Some(frontier::Frontier::with_heights(
            std::env::var("TERRA_FRONTIER_HEIGHTS").ok().as_deref(),
        )?)
    } else {
        None
    };
    app.insert_resource(Route {
        walking: (route == "walk").then(walk::Walk::default),
        frontier,
        speed,
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

fn parse_speed(value: Option<&str>) -> Result<f32, String> {
    let speed = value
        .map_or(Ok(SPEED), str::parse::<f32>)
        .map_err(|_| "TERRA_SOAK_ROUTE_SPEED requires a finite number from 1 through 20")?;
    if !speed.is_finite() || !(1.0..=20.0).contains(&speed) {
        return Err("TERRA_SOAK_ROUTE_SPEED requires a finite number from 1 through 20".into());
    }
    Ok(speed)
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
    let frontier = route.frontier.is_some();
    let planned_distance = if frontier {
        frontier::DISTANCE
    } else {
        route.speed * LEG_SECONDS as f32
    };
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
        let probes = if frontier {
            (planned_distance / 32.0).ceil() as u32
        } else {
            12
        };
        let highest = (0..=probes)
            .map(|probe| {
                terrain.surface_height(
                    f64::from(position.x + planned_distance * probe as f32 / probes as f32),
                    f64::from(position.z),
                ) as f32
            })
            .fold(position.y, f32::max);
        route.cruise_y = route
            .frontier
            .as_ref()
            .and_then(|f| f.flight_y(0.0))
            .unwrap_or_else(|| (highest + 48.0).min(344.0));
        route.approach = ascent_approach(position, route.cruise_y, |from, to| {
            loaded_sweep_clear(world, from, to)
        })
        .ok_or("No loaded, collision-free ascent column reachable within 16m of the spawn")?;
        set_route_speed(world, route.speed);
        tracing::info!(
            origin=?position.to_array(),
            authoritative=?confirmed.position,
            cruise_y=route.cruise_y,
            speed=route.speed,
            period=PERIOD,
            distance=planned_distance,
            approach_waypoints=route.approach.len(),
            ascent_column=?route.approach.back().copied().unwrap_or(position).to_array(),
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
    let was_approaching = !route.approach.is_empty();
    while route
        .approach
        .front()
        .is_some_and(|target| target.distance(position) <= APPROACH_TOLERANCE)
    {
        route.approach.pop_front();
    }
    if was_approaching && route.approach.is_empty() {
        route.origin = position;
        tracing::info!(origin=?position.to_array(), "Real-input route reached its unobstructed ascent column");
    }
    let approaching = !route.approach.is_empty();
    if let Some(y) = route
        .frontier
        .as_ref()
        .and_then(|f| f.flight_y(position.x - route.origin.x))
    {
        route.cruise_y = y;
    }
    let height_error = route.cruise_y - position.y;
    if !approaching && route.cruising_at.is_none() && height_error.abs() <= 0.5 {
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
    let (cycle, phase, distance) = if approaching {
        (0, "canopy-exit", 0.0)
    } else if let Some(flight) = route.frontier.as_mut() {
        if route.cruising_at.is_some() {
            flight.target(position.x - route.origin.x)
        } else {
            (0, "climb", 0.0)
        }
    } else {
        route
            .cruising_at
            .map(|started| {
                let (cycle, phase, distance) = route_target(started.elapsed().as_secs_f64());
                (cycle, phase, distance * route.speed / SPEED)
            })
            .unwrap_or((0, "climb", 0.0))
    };
    let target =
        route.approach.front().copied().unwrap_or_else(|| {
            Vec3::new(route.origin.x + distance, route.cruise_y, route.origin.z)
        });
    if approaching && !loaded_sweep_clear(world, position, target) {
        return Err(
            "The loaded ascent approach became obstructed before reaching its waypoint".into(),
        );
    }
    if route.previous_phase != Some((cycle, phase)) {
        if route
            .previous_phase
            .is_some_and(|(previous_cycle, _)| previous_cycle < cycle)
        {
            if route.maximum_outbound < planned_distance * 0.75 {
                return Err(format!(
                    "Exploration reached only {:.1}m of the {planned_distance}m route; movement was blocked",
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
    if let Some(flight) = route.frontier.as_mut() {
        flight.sample(world, now, phase, position, confirmed.position[0]);
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
        let yaw = if frontier {
            -std::f32::consts::FRAC_PI_2
        } else {
            0.0
        };
        camera.rotation = Quat::from_euler(EulerRot::YXZ, yaw, -0.22, 0.0);
    }
    let error = target - position;
    // Slow down at corners so flight acceleration cannot cut across an unchecked cell.
    let speed = if approaching {
        (error.length() * 2.5).clamp(0.2, 2.5)
    } else {
        route.speed
    };
    set_route_speed(world, speed);
    let deadzone = if approaching { 0.025 } else { 0.35 };
    let (side, back) = if frontier {
        (error.z, -error.x)
    } else {
        (error.x, error.z)
    };
    let desired = [
        (
            GameAction::Sprint,
            frontier && phase == "outbound" && error.x > 6.0,
        ),
        (GameAction::Right, side > deadzone),
        (GameAction::Left, side < -deadzone),
        (GameAction::Backward, back > deadzone),
        (GameAction::Forward, back < -deadzone),
        (GameAction::Jump, error.y > deadzone),
        (GameAction::FlyDown, error.y < -deadzone),
    ];
    inject_bindings(world, &desired);
    Ok(())
}

fn set_route_speed(world: &mut World, speed: f32) {
    if world.resource::<GameConfig>().client.fly_speed != speed {
        world.resource_mut::<GameConfig>().client.fly_speed = speed;
    }
}

fn ascent_approach(
    origin: Vec3,
    cruise_y: f32,
    mut clear: impl FnMut(Vec3, Vec3) -> bool,
) -> Option<VecDeque<Vec3>> {
    if !origin.is_finite() || !cruise_y.is_finite() {
        return None;
    }
    // The cruise plane may not be streamed yet. Check the entire local canopy
    // clearance against loaded cells; authority checks the remaining ascent normally.
    let top = cruise_y.min(origin.y + ASCENT_CLEARANCE).max(origin.y);
    let point = |[x, z]: [i32; 2]| origin + Vec3::new(x as f32, 0.0, z as f32);
    let mut parents = HashMap::from([([0, 0], [0, 0])]);
    let mut pending = VecDeque::from([[0, 0]]);
    while let Some(cell) = pending.pop_front() {
        let current = point(cell);
        if clear(current, Vec3::new(current.x, top, current.z)) {
            let mut path = VecDeque::new();
            let mut cursor = cell;
            while cursor != [0, 0] {
                path.push_front(point(cursor));
                cursor = parents[&cursor];
            }
            return Some(path);
        }
        for [dx, dz] in [[1, 0], [-1, 0], [0, 1], [0, -1]] {
            let next = [cell[0] + dx, cell[1] + dz];
            if next[0] * next[0] + next[1] * next[1] > APPROACH_RADIUS * APPROACH_RADIUS
                || parents.contains_key(&next)
                || !clear(current, point(next))
            {
                continue;
            }
            parents.insert(next, cell);
            pending.push_back(next);
        }
    }
    None
}

fn swept_cells_clear(
    from: Vec3,
    to: Vec3,
    mut clear: impl FnMut([i64; 3], Vec3, Vec3) -> bool,
) -> bool {
    if !from.is_finite() || !to.is_finite() {
        return false;
    }
    // An AABB enclosing the capsule sweep is conservative, including a small
    // horizontal steering margin. Keep the real half-height so the floor is not hit.
    let half = Vec3::new(PLAYER_RADIUS + 0.1, PLAYER_HALF_HEIGHT, PLAYER_RADIUS + 0.1);
    let minimum = from.min(to) - half;
    let maximum = from.max(to) + half;
    let low = minimum.floor().as_i64vec3();
    let high = (maximum - Vec3::splat(0.001)).floor().as_i64vec3();
    for z in low.z..=high.z {
        for y in low.y..=high.y {
            for x in low.x..=high.x {
                if !clear([x, y, z], minimum, maximum) {
                    return false;
                }
            }
        }
    }
    true
}

fn loaded_sweep_clear(world: &World, from: Vec3, to: Vec3) -> bool {
    let chunks = world.resource::<ChunkStore>();
    let journal = world.resource::<EditJournal>();
    let properties = world.resource::<terra_core::block_properties::BlockPropertiesCatalog>();
    swept_cells_clear(from, to, |cell, minimum, maximum| {
        chunks.block_at(cell).is_some()
            && !terra_world::interaction_shapes::space_intersects(
                chunks,
                journal,
                properties,
                cell,
                minimum.to_array().map(f64::from),
                maximum.to_array().map(f64::from),
            )
    })
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
    fn flight_speed_override_preserves_the_comparison_default_and_tests_real_saved_speed() {
        assert_eq!(parse_speed(None).unwrap(), SPEED);
        assert_eq!(parse_speed(Some("20")).unwrap(), 20.0);
        assert_eq!(parse_speed(Some("2.5")).unwrap(), 2.5);
        for invalid in ["0", "-1", "NaN", "inf", "20.01", "invalid"] {
            assert!(parse_speed(Some(invalid)).is_err(), "accepted {invalid}");
        }
    }

    #[test]
    fn an_unchanged_flight_speed_does_not_invalidate_the_world_configuration() {
        let mut world = World::new();
        world.init_resource::<GameConfig>();
        set_route_speed(&mut world, SPEED);
        let changed = world
            .get_resource_ref::<GameConfig>()
            .unwrap()
            .last_changed();
        world.increment_change_tick();
        for _ in 0..128 {
            set_route_speed(&mut world, SPEED);
        }
        assert_eq!(
            world
                .get_resource_ref::<GameConfig>()
                .unwrap()
                .last_changed(),
            changed
        );
        set_route_speed(&mut world, 2.5);
        assert_eq!(world.resource::<GameConfig>().client.fly_speed, 2.5);
        assert_ne!(
            world
                .get_resource_ref::<GameConfig>()
                .unwrap()
                .last_changed(),
            changed
        );
    }

    fn voxel_clear(from: Vec3, to: Vec3, mut block: impl FnMut([i64; 3]) -> Option<u16>) -> bool {
        swept_cells_clear(from, to, |cell, _, _| block(cell) == Some(0))
    }

    #[test]
    fn a_spawn_below_a_tree_canopy_reaches_a_loaded_free_column_before_ascending() {
        let origin = Vec3::new(8.5, 103.91, 24.5);
        let blocks = |[x, y, z]: [i64; 3]| {
            Some(if y <= 102 {
                1
            } else if (110..116).contains(&y) && (5..=12).contains(&x) && (20..=28).contains(&z) {
                7
            } else {
                0
            })
        };
        assert!(!voxel_clear(origin, origin + Vec3::Y * 32.0, blocks));
        let path = ascent_approach(origin, 200.0, |from, to| voxel_clear(from, to, blocks))
            .expect("the clear natural column beside the canopy is reachable");
        assert_eq!(
            path.len(),
            4,
            "the nearest free canopy edge is four cells away"
        );
        assert!((path.back().unwrap().distance(origin) - 4.0).abs() < 0.001);
        let mut previous = origin;
        for &point in &path {
            assert_eq!(point.y, origin.y);
            assert!((point.distance(previous) - 1.0).abs() < 0.001);
            assert!(voxel_clear(previous, point, blocks));
            previous = point;
        }
        assert!(voxel_clear(previous, previous + Vec3::Y * 32.0, blocks));
    }

    #[test]
    fn the_ascent_approach_routes_around_a_wall_without_cutting_its_corner() {
        let origin = Vec3::new(-0.5, 1.91, -0.5);
        let blocks = |[x, y, z]: [i64; 3]| {
            Some(
                if y <= 0
                    || y == 6 && [x, z] != [2, -1]
                    || x == 0 && (1..=4).contains(&y) && (-2..=0).contains(&z)
                {
                    3
                } else {
                    0
                },
            )
        };
        assert!(!voxel_clear(origin, origin + Vec3::X, blocks));
        let path = ascent_approach(origin, 34.0, |from, to| voxel_clear(from, to, blocks))
            .expect("the wall has a reachable route around its end");
        assert_eq!(path.back(), Some(&Vec3::new(2.5, origin.y, -0.5)));
        assert!(path.len() > 3);
        let mut previous = origin;
        for point in path {
            assert!(voxel_clear(previous, point, blocks));
            previous = point;
        }
    }

    #[test]
    fn unknown_columns_and_out_of_radius_openings_are_not_treated_as_safe() {
        let origin = Vec3::new(0.5, 1.91, 0.5);
        assert!(
            ascent_approach(origin, 100.0, |from, to| {
                voxel_clear(from, to, |[_, y, _]| (y < 16).then_some(0))
            })
            .is_none()
        );
        let mut visited = 0;
        assert!(
            ascent_approach(origin, 100.0, |from, to| {
                visited += 1;
                if to.y > from.y {
                    assert!((from - origin).with_y(0.0).length() <= 16.001);
                    from.x >= 17.5
                } else {
                    true
                }
            })
            .is_none()
        );
        assert!(visited <= 33 * 33 * 5);
        assert!(ascent_approach(Vec3::NAN, 100.0, |_, _| true).is_none());
        assert!(ascent_approach(origin, f32::INFINITY, |_, _| true).is_none());
        assert!(
            ascent_approach(origin, 100.0, |_, _| true)
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn runtime_sweeps_require_loaded_cells_and_respect_the_real_shared_collider() {
        use terra_voxel::{BlockSection, SectionPos};
        use terra_world::streaming::LoadedSection;
        let mut world = World::new();
        world.init_resource::<ChunkStore>();
        world.init_resource::<EditJournal>();
        world.init_resource::<terra_core::block_properties::BlockPropertiesCatalog>();
        let origin = Vec3::new(8.5, 2.91, 8.5);
        assert!(!loaded_sweep_clear(&world, origin, origin + Vec3::Y));
        let section = SectionPos::new(0, 0, 0);
        world.resource_mut::<ChunkStore>().loaded.insert(
            section,
            LoadedSection {
                data: BlockSection::empty(section).unwrap(),
                mesh: None,
                mesh_epoch: 0,
                water_epoch: 0,
                water: None,
                props: Vec::new(),
            },
        );
        assert!(loaded_sweep_clear(&world, origin, origin + Vec3::Y));
        world
            .resource_mut::<ChunkStore>()
            .loaded
            .get_mut(&section)
            .unwrap()
            .data
            .set_block([8, 4, 8], 7)
            .unwrap();
        assert!(!loaded_sweep_clear(&world, origin, origin + Vec3::Y));
        assert!(!loaded_sweep_clear(&world, origin, origin + Vec3::Y * 20.0));
    }

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
