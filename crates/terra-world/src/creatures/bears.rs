//! Bear decisions share the voxel habitat, needs, spawn identities and saved timers.
//! Only the authority commits effects; candidate fish are spatially indexed once per tick.
use super::{Behavior, Creature, Habitat, Kind, distance, navigation, needs, random};
use std::collections::BTreeMap;
use terra_core::creatures::{CreaturePopulation, NeedGoal, NeedKind};
#[cfg(test)]
#[path = "bear_tests.rs"]
mod tests;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Effect {
    None,
    StrikePlayer {
        index: usize,
        damage: f32,
    },
    CatchFish {
        id: u64,
    },
    Ignite {
        position: [i64; 3],
    },
    SpitBile {
        origin: [f64; 3],
        velocity: [f64; 3],
    },
}

#[derive(Default)]
pub struct PreyIndex(BTreeMap<[i64; 2], Vec<(u64, [f64; 3])>>);
impl PreyIndex {
    pub fn new<'a>(animals: impl Iterator<Item = &'a Creature>) -> Self {
        let mut result = Self::default();
        for a in animals.filter(|a| a.kind == Kind::Fish && a.health > 0.0) {
            result
                .0
                .entry(bin(a.position))
                .or_default()
                .push((a.id, a.position));
        }
        result
    }
    pub(super) fn nearest(&self, position: [f64; 3]) -> Option<(u64, [f64; 3])> {
        let center = bin(position);
        let mut best = 18.0_f64.powi(2);
        let mut prey = None;
        for dz in -1..=1 {
            for dx in -1..=1 {
                if let Some(group) = self.0.get(&[center[0] + dx, center[1] + dz]) {
                    for &(id, p) in group.iter().take(32) {
                        let d = distance(position, p);
                        if d < best && (position[1] - p[1]).abs() < 3.0 {
                            best = d;
                            prey = Some((id, p));
                        }
                    }
                }
            }
        }
        prey
    }
}
fn bin(p: [f64; 3]) -> [i64; 2] {
    [(p[0] / 18.0).floor() as i64, (p[2] / 18.0).floor() as i64]
}

/// Only a minority of otherwise occupied land habitats gain a solitary bear.
/// Probe real loaded surface water, so a drained river or underground gallery is not a shore.
pub fn near_water(h: &Habitat, p: [f64; 3]) -> bool {
    let climate = h.terrain.biome_at(p[0], p[2]);
    if climate.weights.ocean > 0.4 || climate.aridity() > 0.5 {
        return false;
    }
    [4.0, 8.0, 14.0, 20.0].into_iter().any(|r| {
        [
            [1.0, 0.0],
            [-1.0, 0.0],
            [0.0, 1.0],
            [0.0, -1.0],
            [0.707, 0.707],
            [-0.707, 0.707],
            [0.707, -0.707],
            [-0.707, -0.707],
        ]
        .into_iter()
        .any(|[dx, dz]| {
            let x = p[0] + dx * r;
            let z = p[2] + dz * r;
            h.terrain.water_column(x, z).is_some_and(|water| {
                water.depth >= 0.6
                    && (water.level - p[1]).abs() <= 4.0
                    && h.wet([x, water.level - 0.2, z])
            })
        })
    })
}

/// Checks edited voxels at paw/head height; opaque terrain and unloaded cells block a hit.
pub fn visible(h: &Habitat, a: [f64; 3], b: [f64; 3]) -> bool {
    super::perception::visible(h, a, b)
}

#[allow(clippy::too_many_arguments)]
pub fn step(
    a: &mut Creature,
    h: &Habitat,
    players: &[[f64; 3]],
    prey: &PreyIndex,
    dt: f32,
    search: bool,
    night: bool,
) -> Effect {
    step_with_navigation(a, h, players, prey, dt, search, night, None)
}

#[allow(clippy::too_many_arguments)]
pub fn step_with_navigation(
    a: &mut Creature,
    h: &Habitat,
    players: &[[f64; 3]],
    prey: &PreyIndex,
    dt: f32,
    search: bool,
    night: bool,
    mut navigation: Option<&mut navigation::Navigation>,
) -> Effect {
    if a.kind != Kind::Bear
        || a.health <= 0.0
        || !dt.is_finite()
        || !(0.0..=0.25).contains(&dt)
        || dt == 0.0
        || h.block(a.position.map(|v| v.floor() as i64)).is_none()
    {
        return Effect::None;
    }
    a.age += f64::from(dt);
    let previous_timer = a.timer;
    a.timer = (a.timer - dt).max(0.0);
    needs::update(a, h, dt, search);
    if a.health <= 0.0 {
        a.behavior = Behavior::Dead;
        a.velocity = [0.0; 3];
        return Effect::None;
    }
    if a.behavior == Behavior::Climb {
        if let Some(nav) = navigation.as_deref_mut() {
            nav.forget(a.id);
        }
        climb(a, h, dt);
        return Effect::None;
    }

    let definition = terra_core::creatures::definitions::get(Kind::Bear);
    if let Some(combat) = definition.combat {
        let pursuing = matches!(a.behavior, Behavior::Attack | Behavior::Hunt);
        let radius = combat.aggro_radius * if pursuing { 1.75 } else { 1.0 };
        let nearest = super::perception::nearest_visible_at(
            h,
            head(a.position),
            a.position,
            players,
            radius,
            |_| true,
            head,
        );
        if let Some((index, player)) = nearest {
            let before = a.behavior;
            a.needs.goal = None;
            if before != Behavior::Attack {
                face(a, player, h);
            }
            if super::perception::in_melee_arc(a.yaw, a.position, player)
                && distance(a.position, player) <= combat.reach.powi(2)
                && visible(h, head(a.position), head(player))
            {
                a.behavior = Behavior::Attack;
                if let Some(nav) = navigation.as_deref_mut() {
                    nav.forget(a.id);
                    nav.intend(
                        a,
                        terra_core::creatures::intentions::IntentReason::Attack,
                        Some(player),
                    );
                }
                a.velocity[0] = 0.0;
                a.velocity[2] = 0.0;
                settle(a, h, dt);
                // Support edits can move the bear during settling; never apply
                // a strike using the reach or line of sight from its old position.
                if distance(a.position, player) <= combat.reach.powi(2)
                    && visible(h, head(a.position), head(player))
                    && combat.melee_contact(
                        before != Behavior::Attack,
                        previous_timer,
                        &mut a.timer,
                    )
                {
                    return Effect::StrikePlayer {
                        index,
                        damage: combat.damage,
                    };
                }
            } else {
                a.behavior = Behavior::Hunt;
                travel(
                    a,
                    h,
                    player,
                    dt,
                    definition.speed * 1.8,
                    search,
                    navigation.as_deref_mut(),
                );
            }
            return Effect::None;
        }
    }
    if a.behavior == Behavior::Attack {
        a.behavior = Behavior::Rest;
        a.timer = 1.5;
    }
    if night
        && definition.sleeps
        && a.needs.food > 15.0
        && a.needs.water > 5.0
        && !super::support::swimming(a, h)
        && let Some(ground) = h.ground_rotated(a.position, Kind::Bear, f64::from(a.yaw))
    {
        a.position = ground;
        if let Some(nav) = navigation.as_deref_mut() {
            nav.forget(a.id);
        }
        a.behavior = Behavior::Sleep;
        a.needs.goal = None;
        a.velocity = [0.0; 3];
        return Effect::None;
    }
    if a.behavior == Behavior::Sleep {
        a.behavior = Behavior::Rest;
        a.timer = 0.0;
    }

    if a.needs.food <= 15.0
        && a.needs.water > 2.0
        && let Some((id, target)) = prey.nearest(a.position)
    {
        if a.behavior != Behavior::Hunt {
            a.timer = 0.55;
        }
        a.behavior = Behavior::Hunt;
        a.needs.goal = None;
        face(a, target, h);
        if can_catch(a, target, h) {
            if let Some(nav) = navigation.as_deref_mut() {
                nav.forget(a.id);
            }
            a.velocity[0] = 0.0;
            a.velocity[2] = 0.0;
            settle(a, h, dt);
            if a.timer <= 0.0 {
                a.timer = 1.4;
                return Effect::CatchFish { id };
            }
        } else {
            travel(
                a,
                h,
                target,
                dt,
                definition.speed * 1.25,
                search,
                navigation.as_deref_mut(),
            );
        }
        return Effect::None;
    }
    if a.behavior == Behavior::Hunt {
        a.behavior = Behavior::Rest;
        a.needs.search_in = 0.0;
        a.timer = 1.0;
    }
    if a.needs.goal.is_some_and(|g| g.kind == NeedKind::Tree) {
        approach_tree(a, h, dt, search, navigation.as_deref_mut());
        return Effect::None;
    }
    if matches!(
        a.behavior,
        Behavior::Drink | Behavior::Rest | Behavior::Shake
    ) && a.timer > 0.0
    {
        a.velocity[0] = 0.0;
        a.velocity[2] = 0.0;
        if !super::support::swimming(a, h) {
            settle(a, h, dt);
            return Effect::None;
        }
    }
    if a.behavior == Behavior::SeekWater {
        let target = a.needs.goal.map(|g| g.position.map(|v| v as f64 + 0.5));
        if let Some(target) = target {
            travel(
                a,
                h,
                target,
                dt,
                definition.speed,
                search,
                navigation.as_deref_mut(),
            );
            return Effect::None;
        }
    }
    if search
        && a.age > 30.0
        && a.needs.food > 20.0
        && a.needs.water > 8.0
        && random(a.id, (a.age / 8.0) as u64).is_multiple_of(12)
        && let Some(tree) = find_tree(a, h)
    {
        a.needs.goal = Some(NeedGoal {
            kind: NeedKind::Tree,
            position: tree,
        });
        a.needs.search_in = 15.0;
        a.behavior = Behavior::Wander;
        a.timer = 12.0;
        approach_tree(a, h, dt, search, navigation.as_deref_mut());
        return Effect::None;
    }
    if a.timer <= 0.0 {
        let seed = random(a.id, (a.age * 10.0) as u64);
        let yaw = (seed % 628) as f32 * 0.01;
        if h.clear_swimmer_rotated(a.position, a.size(), f64::from(yaw)) {
            a.yaw = yaw;
        }
        a.behavior = if seed.is_multiple_of(4) {
            Behavior::Rest
        } else {
            Behavior::Wander
        };
        a.timer = 3.0 + (seed % 40) as f32 * 0.1;
    }
    if a.behavior == Behavior::Wander || super::support::swimming(a, h) {
        let target = [
            a.position[0] - f64::from(a.yaw.sin()) * 4.0,
            a.position[1],
            a.position[2] - f64::from(a.yaw.cos()) * 4.0,
        ];
        if !travel(a, h, target, dt, definition.speed, search, navigation) {
            a.behavior = Behavior::Rest;
            a.timer = 1.5;
        }
    } else {
        a.velocity[0] = 0.0;
        a.velocity[2] = 0.0;
        settle(a, h, dt);
    }
    Effect::None
}

fn head(mut p: [f64; 3]) -> [f64; 3] {
    p[1] += 0.65;
    p
}
fn face(a: &mut Creature, p: [f64; 3], h: &Habitat) {
    let yaw = ((a.position[0] - p[0]).atan2(a.position[2] - p[2])) as f32;
    if h.clear_swimmer_rotated(a.position, a.size(), f64::from(yaw)) {
        a.yaw = yaw;
    }
}
fn can_catch(a: &Creature, p: [f64; 3], h: &Habitat) -> bool {
    distance(a.position, p) <= 2.25_f64.powi(2)
        && visible(h, head(a.position), [p[0], p[1] + 0.1, p[2]])
}

/// Revalidate the live target after all decisions. Two bears cannot consume one fish twice.
pub fn catch_fish(population: &mut CreaturePopulation, bear: u64, fish: u64, h: &Habitat) -> bool {
    let Some(predator) = population
        .animals
        .get(&bear)
        .filter(|a| a.kind == Kind::Bear && a.health > 0.0 && a.needs.food <= 15.0)
    else {
        return false;
    };
    let Some(prey) = population
        .animals
        .get(&fish)
        .filter(|a| a.kind == Kind::Fish && a.health > 0.0)
    else {
        return false;
    };
    if !can_catch(predator, prey.position, h) {
        return false;
    }
    if let Some(prey) = population.animals.get_mut(&fish) {
        prey.health = 0.0;
        prey.behavior = Behavior::Dead;
        prey.velocity = [0.0; 3];
    }
    if let Some(bear) = population.animals.get_mut(&bear) {
        bear.needs.food = (bear.needs.food + 18.0).min(Kind::Bear.food());
        bear.behavior = Behavior::Rest;
        bear.timer = 2.0;
    }
    true
}

fn travel(
    a: &mut Creature,
    h: &Habitat,
    target: [f64; 3],
    dt: f32,
    speed: f64,
    search: bool,
    mut navigation: Option<&mut navigation::Navigation>,
) -> bool {
    if let Some(nav) = navigation.as_deref_mut() {
        nav.intend(
            a,
            terra_core::creatures::intentions::CreatureIntent::state(a).reason,
            Some(target),
        );
    }
    if super::support::swimming(a, h) {
        let behavior = a.behavior;
        let moved = super::support::swim_toward(a, h, target, dt, speed);
        if matches!(
            behavior,
            Behavior::Attack | Behavior::Hunt | Behavior::SeekWater
        ) {
            a.behavior = behavior;
        }
        return moved;
    }
    if h.supported_sized(a.position, a.size(), f64::from(a.yaw))
        .is_none_or(|p| p[1] > a.position[1] + 0.001)
    {
        a.velocity[0] = 0.0;
        a.velocity[2] = 0.0;
        settle(a, h, dt);
        return false;
    }
    let before = a.position;
    let move_to = |goal: [f64; 3]| {
        let dx = goal[0] - before[0];
        let dz = goal[2] - before[2];
        let length = dx.hypot(dz).max(0.001);
        let amount = (speed * f64::from(dt)).min(length);
        let p = [
            before[0] + dx / length * amount,
            before[1],
            before[2] + dz / length * amount,
        ];
        let yaw = (before[0] - p[0]).atan2(before[2] - p[2]);
        if let Some(ground) = h.travel_step(before, p, Kind::Bear, yaw) {
            return Some(ground);
        }
        None
    };
    // Finish a cached detour before attempting the direct route again. A
    // single free step toward the target does not mean the obstacle is cleared.
    let detour = if navigation
        .as_ref()
        .is_some_and(|nav| nav.following_ground_route(a.id))
    {
        navigation::ground_waypoint(a, h, target, false, navigation.as_deref_mut())
            .and_then(move_to)
    } else {
        None
    };
    let direct = detour.is_none().then(|| move_to(target)).flatten();
    if direct.is_some()
        && let Some(nav) = navigation.as_deref_mut()
    {
        nav.ground_unblocked(a.id);
    }
    let next = detour.or(direct).or_else(|| {
        navigation::ground_waypoint(a, h, target, search, navigation).and_then(move_to)
    });
    if let Some(p) = next {
        a.yaw = ((before[0] - p[0]).atan2(before[2] - p[2])) as f32;
        a.position = p;
        a.velocity = std::array::from_fn(|i| ((p[i] - before[i]) / f64::from(dt)) as f32);
        return true;
    }
    a.velocity[0] = 0.0;
    a.velocity[2] = 0.0;
    settle(a, h, dt);
    false
}

fn log(h: &Habitat, p: [i64; 3]) -> bool {
    matches!(h.block(p), Some(6 | 10 | 36 | 134))
}
fn find_tree(a: &Creature, h: &Habitat) -> Option<[i64; 3]> {
    let p = a.position.map(|v| v.floor() as i64);
    for r in 1..=6 {
        for [dx, dz] in [
            [1, 0],
            [-1, 0],
            [0, 1],
            [0, -1],
            [1, 1],
            [-1, -1],
            [1, -1],
            [-1, 1],
        ] {
            let tree = [p[0] + dx * r, p[1], p[2] + dz * r];
            if (0..4).all(|dy| log(h, [tree[0], tree[1] + dy, tree[2]])) {
                return Some(tree);
            }
        }
    }
    None
}
fn approach_tree(
    a: &mut Creature,
    h: &Habitat,
    dt: f32,
    search: bool,
    navigation: Option<&mut navigation::Navigation>,
) {
    let Some(goal) = a.needs.goal.filter(|g| g.kind == NeedKind::Tree) else {
        return;
    };
    let tree = goal.position;
    if a.timer <= 0.0 || !log(h, tree) {
        a.needs.goal = None;
        a.behavior = Behavior::Rest;
        a.timer = 3.0;
        return;
    }
    let radius = Kind::Bear.size()[0].max(Kind::Bear.size()[2]) * 0.5;
    let target = [
        [radius + 0.53, 0.0],
        [-radius - 0.53, 0.0],
        [0.0, radius + 0.53],
        [0.0, -radius - 0.53],
    ]
    .into_iter()
    .filter_map(|[dx, dz]| {
        h.ground_rotated(
            [
                tree[0] as f64 + 0.5 + dx,
                a.position[1],
                tree[2] as f64 + 0.5 + dz,
            ],
            Kind::Bear,
            f64::from(a.yaw),
        )
    })
    .min_by(|x, y| distance(*x, a.position).total_cmp(&distance(*y, a.position)));
    let Some(target) = target else {
        a.timer = 0.0;
        return;
    };
    if distance(a.position, target) < 0.12_f64.powi(2) {
        a.behavior = Behavior::Climb;
        a.timer = 16.0;
        face(
            a,
            [tree[0] as f64 + 0.5, a.position[1], tree[2] as f64 + 0.5],
            h,
        );
        a.velocity = [0.0; 3];
    } else {
        travel(a, h, target, dt, Kind::Bear.speed(), search, navigation);
    }
}
fn climb(a: &mut Creature, h: &Habitat, dt: f32) {
    let Some(goal) = a.needs.goal.filter(|g| g.kind == NeedKind::Tree) else {
        a.behavior = Behavior::Rest;
        settle(a, h, dt);
        return;
    };
    let tree = goal.position;
    let supported = log(h, [tree[0], a.position[1].floor() as i64, tree[2]])
        || log(h, [tree[0], (a.position[1] + 0.8).floor() as i64, tree[2]]);
    let before = a.position;
    let mut next = before;
    let ascent = a.timer > 10.0 && supported;
    let descent = a.timer <= 7.0 || !supported;
    if ascent {
        next[1] += f64::from(dt) * 1.0;
    }
    if descent {
        next[1] -= f64::from(dt) * 1.2;
    }
    if descent
        && let Some(ground) = h.ground_rotated(next, Kind::Bear, f64::from(a.yaw))
        && ground[1] >= next[1]
        && ground[1] <= before[1] + 0.01
    {
        a.position = ground;
        a.velocity = std::array::from_fn(|i| ((ground[i] - before[i]) / f64::from(dt)) as f32);
        a.behavior = Behavior::Rest;
        a.needs.goal = None;
        a.needs.search_in = 15.0;
        a.timer = 6.0;
        return;
    }
    if next[1] > tree[1] as f64 + 6.0
        || !h.clear_rotated(next, Kind::Bear.size(), false, f64::from(a.yaw))
        || (ascent && !log(h, [tree[0], (next[1] + 0.6).floor() as i64, tree[2]]))
    {
        a.timer = a.timer.min(7.0);
        next = before;
    }
    if h.clear_rotated(next, Kind::Bear.size(), false, f64::from(a.yaw)) {
        a.position = next;
    }
    a.velocity = std::array::from_fn(|i| ((a.position[i] - before[i]) / f64::from(dt)) as f32);
}
fn settle(a: &mut Creature, h: &Habitat, dt: f32) {
    super::support::settle(a, h, dt);
}
