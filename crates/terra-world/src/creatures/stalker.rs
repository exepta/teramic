//! Cave predator decisions; the authority alone applies strikes and swept pulls.
use super::{Behavior, Creature, Habitat, Kind, distance, navigation, random};
use std::collections::{HashMap, VecDeque};
use terra_core::{
    PlayerUuid,
    creatures::{CreatureNeeds, CreaturePopulation, intentions::IntentReason},
};

pub const PLAYER_HALF_HEIGHT: f64 = 0.9;
const MAX_MINDS: usize = 64;
#[cfg(test)]
#[path = "stalker/tests.rs"]
mod tests;

#[derive(Clone, Copy)]
pub struct Player {
    pub id: PlayerUuid,
    pub position: [f64; 3],
    pub look: [f32; 2],
    pub held_light: bool,
    pub targetable: bool,
}
#[derive(Debug, Clone, Copy)]
pub struct Pull {
    pub player: PlayerUuid,
    pub creature: u64,
    pub destination: [f64; 3],
    pub remaining_meters: f32,
}
#[derive(Default)]
pub struct Outcome {
    pub strike: Option<(PlayerUuid, f32)>,
    pub pull: Option<Pull>,
}
#[derive(Default)]
pub struct Stalkers {
    minds: HashMap<u64, Mind>,
    serial: u64,
}
struct Mind {
    touched: u64,
    target: Option<PlayerUuid>,
    path: VecDeque<[f64; 3]>,
    search_in: f32,
    ignored: f32,
    anger: f32,
    recoiled: f32,
    last_distance: f64,
    health: f32,
    remaining: f64,
    last_player: [f64; 3],
    grab_stuck: f32,
    lit: bool,
    corner: Option<CornerPeek>,
    last_corner: Option<[i64; 3]>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PeekPhase {
    Approach,
    Pause,
    Reveal,
    Observe,
}
struct CornerPeek {
    cell: [i64; 3],
    anchor: [f64; 3],
    reveal: Option<[f64; 3]>,
    phase: PeekPhase,
    wait: f32,
    elapsed: f32,
}
impl Stalkers {
    pub fn retain(&mut self, population: &CreaturePopulation) {
        self.minds.retain(|id, _| {
            population
                .animals
                .get(id)
                .is_some_and(|a| a.kind == Kind::Stalker && a.health > 0.0)
        });
    }
    pub fn step(
        &mut self,
        a: &mut Creature,
        h: &Habitat,
        players: &[Player],
        dt: f32,
        search: bool,
        nav: &mut navigation::Navigation,
    ) -> Outcome {
        if a.kind != Kind::Stalker
            || !a.valid()
            || a.health <= 0.0
            || !dt.is_finite()
            || !(0.0..=0.25).contains(&dt)
            || dt == 0.0
        {
            return Outcome::default();
        }
        a.age += f64::from(dt);
        a.timer = (a.timer - dt).max(0.0);
        a.needs.search_in = (a.needs.search_in - dt).max(0.0);
        super::support::settle(a, h, dt);
        if h.supported_sized(a.position, a.size(), f64::from(a.yaw))
            .is_none()
        {
            self.minds.remove(&a.id);
            a.behavior = Behavior::Rest;
            a.velocity[0] = 0.0;
            a.velocity[2] = 0.0;
            nav.intend(a, IntentReason::Unsupported, None);
            return Outcome::default();
        }
        if !navigation::caves::covered(h, a.position, a.size()[1]) {
            a.behavior = Behavior::Rest;
            a.velocity = [0.0; 3];
            self.minds.remove(&a.id);
            return Outcome::default();
        }
        self.serial = self.serial.saturating_add(1);
        if self.minds.len() >= MAX_MINDS
            && !self.minds.contains_key(&a.id)
            && let Some(oldest) = self
                .minds
                .iter()
                .min_by_key(|(id, mind)| (mind.touched, **id))
                .map(|(id, _)| *id)
        {
            self.minds.remove(&oldest);
        }
        let serial = self.serial;
        let mind = self.minds.entry(a.id).or_insert_with(|| Mind {
            touched: serial,
            target: None,
            path: VecDeque::new(),
            search_in: 0.0,
            ignored: 0.0,
            anger: 0.0,
            recoiled: 0.0,
            last_distance: 50.0,
            health: a.health,
            remaining: 0.0,
            last_player: [0.0; 3],
            grab_stuck: 0.0,
            lit: false,
            corner: None,
            last_corner: None,
        });
        mind.touched = serial;
        let mut search_budget = navigation::caves::SearchBudget::default();
        mind.search_in = (mind.search_in - dt).max(0.0);
        mind.recoiled = (mind.recoiled - dt).max(0.0);
        mind.anger = (mind.anger - dt).max(0.0);
        if a.health < mind.health {
            mind.anger = 14.0;
        }
        mind.health = a.health;
        if search && mind.search_in <= 0.0 {
            mind.search_in = 0.9;
            a.needs.search_in = 0.9;
            mind.lit = near_light(h, a.position);
            if mind.remaining <= 0.0 && mind.corner.is_none() {
                let mut candidates: Vec<_> = players
                    .iter()
                    .filter(|p| {
                        valid_player(p)
                            && p.targetable
                            && distance(a.position, p.position) <= 50.0_f64.powi(2)
                    })
                    .collect();
                candidates.sort_by(|x, y| {
                    distance(a.position, x.position)
                        .total_cmp(&distance(a.position, y.position))
                        .then(x.id.cmp(&y.id))
                });
                let mut found = None;
                for player in candidates.into_iter().take(2) {
                    let feet = [
                        player.position[0],
                        player.position[1] - PLAYER_HALF_HEIGHT,
                        player.position[2],
                    ];
                    if let Some(route) =
                        navigation::caves::route_budgeted(h, a.position, feet, &mut search_budget)
                    {
                        found = Some((player, route));
                        break;
                    }
                }
                if let Some((player, route)) = found {
                    if mind.target != Some(player.id) {
                        mind.ignored = 0.0;
                        mind.last_distance = 50.0;
                        mind.last_corner = None;
                    }
                    mind.target = Some(player.id);
                    mind.path = route.points;
                } else {
                    mind.target = None;
                    mind.path.clear();
                }
            }
        }
        let Some(player) = mind.target.and_then(|id| {
            players
                .iter()
                .find(|p| p.id == id && p.targetable && valid_player(p))
        }) else {
            mind.target = None;
            mind.remaining = 0.0;
            mind.path.clear();
            mind.corner = None;
            a.behavior = Behavior::Rest;
            a.velocity = [0.0; 3];
            return Outcome::default();
        };
        let feet = [
            player.position[0],
            player.position[1] - PLAYER_HALF_HEIGHT,
            player.position[2],
        ];
        if distance(a.position, player.position) > 51.0_f64.powi(2)
            || !navigation::caves::covered(h, feet, 1.8)
        {
            mind.target = None;
            mind.remaining = 0.0;
            mind.corner = None;
            a.velocity = [0.0; 3];
            a.behavior = Behavior::Rest;
            return Outcome::default();
        }
        let range = distance(a.position, feet).sqrt();
        let eyes = eye(a.position);
        let player_eye = [
            player.position[0],
            player.position[1] + 0.72,
            player.position[2],
        ];
        let visible = super::bears::visible(h, eyes, player_eye);
        let seen = visible && looking(player, eyes);
        let light = mind.lit
            || players.iter().any(|nearby| {
                nearby.held_light
                    && valid_player(nearby)
                    && distance(a.position, nearby.position) < 14.0_f64.powi(2)
                    && super::bears::visible(
                        h,
                        eyes,
                        [
                            nearby.position[0],
                            nearby.position[1] + 0.72,
                            nearby.position[2],
                        ],
                    )
            });
        if light {
            mind.remaining = 0.0;
            mind.ignored = 0.0;
            mind.anger = 0.0;
            mind.corner = None;
            a.behavior = Behavior::Flee;
            retreat(a, h, feet, dt, 1.8);
            nav.intend(a, IntentReason::Threat, None);
            mind.last_distance = range;
            return Outcome::default();
        }
        if mind.remaining > 0.0 {
            let dragged = distance(mind.last_player, player.position).sqrt();
            mind.last_player = player.position;
            mind.remaining = (mind.remaining - dragged).max(0.0);
            mind.grab_stuck = if dragged < 0.01 {
                mind.grab_stuck + dt
            } else {
                0.0
            };
            if range > 4.0
                || dragged > 3.0
                || !visible
                || mind.grab_stuck > 1.5
                || h.walk_step(
                    a.position,
                    feet,
                    Kind::Stalker,
                    (a.position[0] - feet[0]).atan2(a.position[2] - feet[2]),
                )
                .is_none()
            {
                release_grip(a, mind, 2.0);
                return Outcome::default();
            }
            a.behavior = Behavior::Hold;
            if range < 2.0 {
                follow(a, h, mind, dt, 2.1);
            }
            nav.intend(a, IntentReason::Hunt, mind.path.front().copied());
            if mind.path.is_empty() || mind.remaining <= 0.02 {
                release_grip(a, mind, 5.0);
                return Outcome::default();
            }
            return Outcome {
                strike: None,
                pull: Some(Pull {
                    player: player.id,
                    creature: a.id,
                    destination: pull_destination(player.position, a.position, mind.remaining),
                    remaining_meters: mind.remaining as f32,
                }),
            };
        }
        if seen && a.behavior == Behavior::Flee && range + 0.12 < mind.last_distance {
            mind.anger = 14.0;
        }
        let entering_stalk_range = range <= 15.0 && mind.last_distance > 15.0;
        mind.last_distance = range;
        if mind.anger > 0.0 {
            mind.corner = None;
            a.behavior = Behavior::Attack;
            nav.intend(a, IntentReason::Attack, Some(feet));
            if range < 2.2 && visible {
                a.velocity = [0.0; 3];
                face(a, feet);
                if a.timer <= 0.0 {
                    a.timer = 1.25;
                    return Outcome {
                        strike: Some((player.id, 8.0)),
                        pull: None,
                    };
                }
            } else {
                follow(a, h, mind, dt, 9.6);
            }
            return Outcome::default();
        }
        if seen {
            if a.behavior != Behavior::Flee {
                mind.recoiled = 2.0;
            }
            a.behavior = Behavior::Flee;
            mind.ignored = 0.0;
            mind.corner = None;
            retreat(a, h, feet, dt, 1.1);
            nav.intend(a, IntentReason::Threat, None);
            return Outcome::default();
        }
        if entering_stalk_range {
            mind.ignored = 0.0;
        }
        mind.ignored += dt;
        if range <= 1.8
            && visible
            && mind.ignored >= 6.0
            && a.timer <= 0.0
            && search
            && let Some(route) =
                navigation::caves::deeper_budgeted(h, a.position, &mut search_budget)
            && route.length >= navigation::caves::DRAG_DISTANCE
        {
            mind.path = route.points;
            mind.remaining = navigation::caves::DRAG_DISTANCE;
            mind.last_player = player.position;
            mind.grab_stuck = 0.0;
            mind.corner = None;
            a.behavior = Behavior::Hold;
            return Outcome {
                strike: None,
                pull: Some(Pull {
                    player: player.id,
                    creature: a.id,
                    destination: pull_destination(player.position, a.position, mind.remaining),
                    remaining_meters: 40.0,
                }),
            };
        }
        if a.timer > 0.0 && a.behavior == Behavior::Rest {
            a.velocity = [0.0; 3];
            return Outcome::default();
        }
        a.behavior = if range <= 15.0 {
            Behavior::Hunt
        } else {
            Behavior::Wander
        };
        nav.intend(a, IntentReason::Stalk, Some(feet));
        if range <= 15.0 && mind.ignored > 3.0 {
            if mind.corner.is_none() && !visible {
                mind.corner = route_corner(h, a.position, &mind.path, player_eye, mind.last_corner);
            }
            if mind.corner.is_some() {
                peek_corner(a, h, mind, feet, dt);
                return Outcome::default();
            }
        } else if range > 15.0 {
            mind.corner = None;
        }
        if range > 15.0 || mind.ignored > 3.0 {
            follow(a, h, mind, dt, if range > 15.0 { 0.85 } else { 1.2 });
        } else {
            a.velocity = [0.0; 3];
            face(a, feet);
        }
        Outcome::default()
    }
}

fn release_grip(a: &mut Creature, mind: &mut Mind, recovery: f32) {
    mind.remaining = 0.0;
    mind.ignored = 0.0;
    mind.grab_stuck = 0.0;
    mind.path.clear();
    a.behavior = Behavior::Rest;
    a.velocity = [0.0; 3];
    a.timer = a.timer.max(recovery);
}

fn valid_player(player: &Player) -> bool {
    navigation::valid_point(player.position) && player.look.into_iter().all(f32::is_finite)
}

fn looking(player: &Player, target: [f64; 3]) -> bool {
    let yaw = f64::from(player.look[0]);
    let pitch = f64::from(player.look[1]);
    let forward = [
        -yaw.sin() * pitch.cos(),
        pitch.sin(),
        -yaw.cos() * pitch.cos(),
    ];
    let delta = [
        target[0] - player.position[0],
        target[1] - player.position[1] - 0.72,
        target[2] - player.position[2],
    ];
    let length = delta.iter().map(|v| v * v).sum::<f64>().sqrt().max(0.01);
    delta
        .into_iter()
        .zip(forward)
        .map(|(a, b)| a * b / length)
        .sum::<f64>()
        > 0.78
}
fn eye(p: [f64; 3]) -> [f64; 3] {
    [p[0], p[1] + 2.12, p[2]]
}
fn face(a: &mut Creature, target: [f64; 3]) {
    a.yaw = ((a.position[0] - target[0]).atan2(a.position[2] - target[2]) as f32)
        .rem_euclid(std::f32::consts::TAU);
}
fn retreat(a: &mut Creature, h: &Habitat, target: [f64; 3], dt: f32, speed: f64) {
    let length = (a.position[0] - target[0])
        .hypot(a.position[2] - target[2])
        .max(0.01);
    let goal = [
        a.position[0] + (a.position[0] - target[0]) / length * speed * f64::from(dt),
        a.position[1],
        a.position[2] + (a.position[2] - target[2]) / length * speed * f64::from(dt),
    ];
    move_to(a, h, goal, dt);
    face(a, target);
}
fn follow(a: &mut Creature, h: &Habitat, mind: &mut Mind, dt: f32, speed: f64) {
    follow_until(a, h, mind, dt, speed, None);
}
fn follow_until(
    a: &mut Creature,
    h: &Habitat,
    mind: &mut Mind,
    dt: f32,
    speed: f64,
    stop: Option<[f64; 3]>,
) {
    let previous = a.position;
    let mut remaining = speed * f64::from(dt);
    for _ in 0..4 {
        if stop.is_some_and(|p| distance(p, a.position) < 0.0001) {
            break;
        }
        while mind
            .path
            .front()
            .is_some_and(|p| distance(*p, a.position) < 0.0001)
        {
            mind.path.pop_front();
        }
        let Some(target) = mind.path.front().copied() else {
            break;
        };
        let length = distance(a.position, target).sqrt().max(0.001);
        let t = (remaining / length).min(1.0);
        let goal = std::array::from_fn(|i| a.position[i] + (target[i] - a.position[i]) * t);
        if !move_to(a, h, goal, dt) {
            mind.path.clear();
            mind.search_in = 0.0;
            a.needs.search_in = 0.0;
            break;
        }
        remaining -= length * t;
        if remaining < 0.001 || stop.is_some_and(|p| distance(p, a.position) < 0.0001) {
            break;
        }
    }
    a.velocity =
        std::array::from_fn(|i| ((a.position[i] - previous[i]) / f64::from(dt.max(0.001))) as f32);
    a.velocity[1] = 0.0;
}

fn route_corner(
    h: &Habitat,
    position: [f64; 3],
    path: &VecDeque<[f64; 3]>,
    player_eye: [f64; 3],
    previous_corner: Option<[i64; 3]>,
) -> Option<CornerPeek> {
    let points: Vec<_> = std::iter::once(position)
        .chain(
            path.iter()
                .copied()
                .filter(|p| distance(*p, position) >= 0.0001),
        )
        .take(8)
        .collect();
    let mut previous = position;
    for point in points.iter().copied().skip(1) {
        if distance(position, point) > 3.5_f64.powi(2)
            || h.walk_step(
                previous,
                point,
                Kind::Stalker,
                (previous[0] - point[0]).atan2(previous[2] - point[2]),
            )
            .is_none()
            || !navigation::caves::covered(h, point, Kind::Stalker.size()[1])
        {
            break;
        }
        // The inner rock edge can reveal the player before the grid route
        // itself turns. Stop on its last occluded waypoint, not at a fake bend.
        if super::bears::visible(h, eye(point), player_eye) {
            let cell = previous.map(|v| v.floor() as i64);
            if previous_corner != Some(cell) {
                return Some(CornerPeek {
                    cell,
                    anchor: previous,
                    reveal: Some(point),
                    phase: PeekPhase::Approach,
                    wait: 0.65,
                    elapsed: 0.0,
                });
            }
            break;
        }
        previous = point;
    }
    let mut blocked_corner = None;
    for (index, turn) in points.windows(3).enumerate() {
        let incoming = [turn[1][0] - turn[0][0], turn[1][2] - turn[0][2]];
        let outgoing = [turn[2][0] - turn[1][0], turn[2][2] - turn[1][2]];
        let lengths = incoming[0].hypot(incoming[1]) * outgoing[0].hypot(outgoing[1]);
        let cell = turn[1].map(|v| v.floor() as i64);
        if lengths < 0.001
            || (incoming[0] * outgoing[0] + incoming[1] * outgoing[1]) / lengths > 0.5
            || previous_corner == Some(cell)
            || distance(position, turn[0]) > 1.6_f64.powi(2)
            || super::bears::visible(h, eye(turn[0]), player_eye)
        {
            continue;
        }
        let mut previous = turn[0];
        let mut reveal = None;
        // Inspect only existing route edges; every reveal remains on swept,
        // supported cave geometry rather than an artificial lateral offset.
        for point in points.iter().copied().skip(index + 1).take(4) {
            if distance(turn[0], point) > 3.5_f64.powi(2) {
                break;
            }
            if h.walk_step(
                previous,
                point,
                Kind::Stalker,
                (previous[0] - point[0]).atan2(previous[2] - point[2]),
            )
            .is_none()
                || !navigation::caves::covered(h, point, Kind::Stalker.size()[1])
            {
                break;
            }
            if super::bears::visible(h, eye(point), player_eye) {
                reveal = Some(point);
                break;
            }
            previous = point;
        }
        let peek = CornerPeek {
            cell,
            anchor: turn[0],
            reveal,
            phase: PeekPhase::Approach,
            wait: 0.65,
            elapsed: 0.0,
        };
        if peek.reveal.is_some() {
            return Some(peek);
        }
        let incoming_length = incoming[0].hypot(incoming[1]);
        let straight = [
            turn[1][0] + incoming[0] / incoming_length,
            turn[1][1],
            turn[1][2] + incoming[1] / incoming_length,
        ];
        if blocked_corner.is_none()
            && h.walk_step(
                turn[1],
                straight,
                Kind::Stalker,
                (-incoming[0]).atan2(-incoming[1]),
            )
            .is_none()
        {
            blocked_corner = Some(peek);
        }
    }
    blocked_corner
}

fn peek_corner(a: &mut Creature, h: &Habitat, mind: &mut Mind, feet: [f64; 3], dt: f32) {
    let Some(mut peek) = mind.corner.take() else {
        return;
    };
    a.behavior = Behavior::Hunt;
    peek.elapsed += dt;
    let mut finished = peek.elapsed >= 8.0;
    match peek.phase {
        PeekPhase::Approach => {
            follow_until(a, h, mind, dt, 1.0, Some(peek.anchor));
            if distance(a.position, peek.anchor) < 0.0001 {
                peek.phase = PeekPhase::Pause;
                a.velocity = [0.0; 3];
            }
        }
        PeekPhase::Pause | PeekPhase::Observe => {
            a.velocity = [0.0; 3];
            face(a, feet);
            peek.wait -= dt;
            if peek.wait <= 0.0 {
                if peek.phase == PeekPhase::Pause && peek.reveal.is_some() {
                    peek.phase = PeekPhase::Reveal;
                } else {
                    finished = true;
                }
            }
        }
        PeekPhase::Reveal => {
            follow_until(a, h, mind, dt, 0.85, peek.reveal);
            if peek
                .reveal
                .is_some_and(|p| distance(a.position, p) < 0.0001)
            {
                peek.phase = PeekPhase::Observe;
                peek.wait = 0.75;
            }
        }
    }
    if finished || mind.path.is_empty() {
        mind.last_corner = Some(peek.cell);
        mind.search_in = 0.0;
        a.needs.search_in = 0.0;
    } else {
        mind.corner = Some(peek);
    }
}
fn pull_destination(player: [f64; 3], stalker: [f64; 3], remaining: f64) -> [f64; 3] {
    let target = [stalker[0], stalker[1] + PLAYER_HALF_HEIGHT, stalker[2]];
    let length = distance(player, target).sqrt().max(0.001);
    let t = (remaining / length).min(1.0);
    std::array::from_fn(|i| player[i] + (target[i] - player[i]) * t)
}
fn move_to(a: &mut Creature, h: &Habitat, target: [f64; 3], dt: f32) -> bool {
    let yaw = (a.position[0] - target[0]).atan2(a.position[2] - target[2]);
    let Some(next) = h
        .walk_step(a.position, target, Kind::Stalker, yaw)
        .filter(|p| navigation::caves::covered(h, *p, a.size()[1]))
    else {
        a.velocity = [0.0; 3];
        return false;
    };
    let previous = a.position;
    a.position = next;
    face(a, target);
    a.velocity =
        std::array::from_fn(|i| ((next[i] - previous[i]) / f64::from(dt.max(0.001))) as f32);
    a.velocity[1] = 0.0;
    true
}

fn near_light(h: &Habitat, position: [f64; 3]) -> bool {
    let section = terra_voxel::global_to_section(position.map(|v| v.floor() as i64)).0;
    for dx in -1..=1 {
        for dz in -1..=1 {
            for dy in -1..=1 {
                let Ok(section) = section.offset([dx, dy, dz]) else {
                    continue;
                };
                for (p, material, state) in h.journal.object_states_in_section(section).take(96) {
                    if state.secondary || h.block(p) != Some(material) {
                        continue;
                    }
                    let radius = match material {
                        40 | terra_core::lantern::LANTERN => 14.0,
                        28 | 99 => 4.0,
                        59 if h
                            .journal
                            .furnace_at(p)
                            .is_some_and(|f| f.burn_remaining > 0.0) =>
                        {
                            4.0
                        }
                        _ => continue,
                    };
                    let light = if material == terra_core::lantern::LANTERN {
                        let offset = terra_core::lantern::emission(state.facing);
                        std::array::from_fn(|i| p[i] as f64 + offset[i])
                    } else if material == 59 {
                        let yaw = f64::from(state.facing) * std::f64::consts::FRAC_PI_2;
                        [
                            p[0] as f64 + 0.5 - yaw.sin() * 0.58,
                            p[1] as f64 + 0.85,
                            p[2] as f64 + 0.5 - yaw.cos() * 0.58,
                        ]
                    } else {
                        [
                            p[0] as f64 + 0.5,
                            p[1] as f64 + if material == 40 { 1.43 } else { 1.17 },
                            p[2] as f64 + 0.5,
                        ]
                    };
                    if distance(position, light) < radius * radius
                        && super::bears::visible(h, eye(position), light)
                    {
                        return true;
                    }
                }
                if h.chunks.loaded.get(&section).is_some_and(|s| {
                    s.props
                        .iter()
                        .filter(|p| p.kind == crate::props::PropKind::Scarecrow && !p.removed)
                        .take(8)
                        .any(|p| {
                            distance(position, p.position) < 16.0
                                && super::bears::visible(
                                    h,
                                    eye(position),
                                    [p.position[0], p.position[1] + 1.2, p.position[2]],
                                )
                        })
                }) {
                    return true;
                }
            }
        }
    }
    // Natural luminous mushrooms have no object-state anchor. Restrict the
    // query to their actual four-metre radius; Mind::lit caches this sample.
    let center = position.map(|v| v.floor() as i64);
    for dx in -4..=4 {
        for dz in -4..=4 {
            for dy in -4..=4 {
                let cell = [center[0] + dx, center[1] + dy, center[2] + dz];
                let light = cell.map(|v| v as f64 + 0.5);
                if distance(position, light) < 16.0
                    && h.block(cell) == Some(56)
                    && super::bears::visible(h, eye(position), light)
                {
                    return true;
                }
            }
        }
    }
    false
}

pub fn spawn(
    population: &mut CreaturePopulation,
    h: &Habitat,
    home: [i64; 2],
    player_y: f64,
    available: usize,
) -> usize {
    let Some(id) = terra_core::creatures::stalker_id(home) else {
        return 0;
    };
    if available == 0
        || population.animals.len() >= terra_core::creatures::MAX_CREATURES
        || population.animals.contains_key(&id)
        || !terra_core::creatures::definitions::get(Kind::Stalker)
            .permits_cave_spawn(random(id, h.terrain.config().seed.0), 11)
    {
        return 0;
    }
    let mut sections: Vec<_> = h
        .chunks
        .loaded
        .keys()
        .filter(|p| p.x.div_euclid(2) == home[0] && p.z.div_euclid(2) == home[1])
        .copied()
        .collect();
    sections.sort_unstable_by(|a, b| {
        ((a.y * 16) as f64 - player_y)
            .abs()
            .total_cmp(&((b.y * 16) as f64 - player_y).abs())
            .then(a.y.cmp(&b.y))
            .then(a.x.cmp(&b.x))
            .then(a.z.cmp(&b.z))
    });
    for section in sections.into_iter().take(8) {
        for probe in 0..4 {
            let hash = random(id, probe ^ section.y as u64);
            let p = [
                (section.x * 16 + (hash % 12 + 2) as i64) as f64 + 0.5,
                (section.y * 16 + ((hash >> 10) % 16) as i64) as f64,
                (section.z * 16 + ((hash >> 5) % 12 + 2) as i64) as f64 + 0.5,
            ];
            let Some(p) = h.ground(p, Kind::Stalker) else {
                continue;
            };
            if terra_core::creatures::definitions::get(Kind::Stalker).permits_spawn_biome(
                super::biome_slug(
                    h.terrain
                        .surface_column(p[0], p[2])
                        .biome
                        .weights
                        .dominant(),
                ),
            ) && navigation::caves::covered(h, p, Kind::Stalker.size()[1])
                && !near_light(h, p)
                && population
                    .animals
                    .values()
                    .all(|a| a.health <= 0.0 || distance(a.position, p) > 16.0)
            {
                population.animals.insert(
                    id,
                    Creature {
                        id,
                        kind: Kind::Stalker,
                        home,
                        position: p,
                        velocity: [0.0; 3],
                        yaw: 0.0,
                        health: Kind::Stalker.health(),
                        behavior: Behavior::Rest,
                        timer: 2.0,
                        age: 0.0,
                        variant: 0,
                        needs: CreatureNeeds::full(Kind::Stalker),
                    },
                );
                return 1;
            }
        }
    }
    0
}
