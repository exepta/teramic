//! Aquatic decisions use the same loaded voxel habitat, identities and authority effects.
use super::{Behavior, Creature, Habitat, Kind, bears, distance, random};
use terra_core::creatures::CreaturePopulation;
use terra_core::creatures::definitions::{MobDefinition, SpawnWater};
mod depth;
pub(super) use depth::goal as depth_goal;
#[cfg(test)]
mod depth_tests;

#[cfg(test)]
mod tests;

/// Existing fish identities remain unchanged; only newly visited habitats choose these species.
pub(super) fn select_spawn_kind<'a>(
    seed: u64,
    water: SpawnWater,
    depth: f64,
    biome: &str,
    mut definition: impl FnMut(Kind) -> &'a MobDefinition,
) -> Option<Kind> {
    // Preserve the established selector's order and integer hash residues for
    // legacy/default JSONs, without changing stable species or creature IDs.
    const ORDER: [Kind; 10] = [
        Kind::Squid,
        Kind::MoonJelly,
        Kind::FireJelly,
        Kind::VoidJelly,
        Kind::SunJelly,
        Kind::CrystalJelly,
        Kind::ReefShark,
        Kind::HammerheadShark,
        Kind::WhaleShark,
        Kind::Fish,
    ];
    let weights: [f64; 10] = std::array::from_fn(|i| {
        let species = definition(ORDER[i]);
        if species.permits_spawn_biome(biome) {
            species.aquatic_spawn().weight(water, depth)
        } else {
            0.0
        }
    });
    let total: f64 = weights.iter().sum();
    if total <= 0.0 || !total.is_finite() {
        return None;
    }
    let mut draw = if weights.iter().all(|weight| weight.fract() == 0.0) {
        ((seed >> 16) % total as u64) as f64
    } else {
        // Fractional weights need a fractional draw, even when their sum is <1.
        (random(seed, 0x6171_7561) >> 11) as f64 / (1_u64 << 53) as f64 * total
    };
    for (kind, weight) in ORDER.into_iter().zip(weights) {
        if draw < weight {
            return Some(kind);
        }
        draw -= weight;
    }
    None
}

pub(super) fn spawn_depth_for(
    definition: &MobDefinition,
    seed: u64,
    water_depth: f64,
) -> Option<f64> {
    let settings = definition.aquatic_spawn();
    let height = definition.size[1];
    if let Some([min, max]) = settings.depth_below_surface {
        let min = min.max(height + 0.02);
        let max = max.min(water_depth - 0.2);
        (max >= min).then(|| min + (max - min) * (seed % 301) as f64 / 300.0)
    } else if definition.id == Kind::Fish {
        Some(1.5)
    } else {
        (water_depth >= height + 1.1).then_some(height + 0.7 + (seed % 300) as f64 * 0.01)
    }
}

fn center(a: &Creature) -> [f64; 3] {
    [
        a.position[0],
        a.position[1] + a.size()[1] * 0.5,
        a.position[2],
    ]
}

fn bottom(a: &Creature, h: &Habitat) -> Option<[f64; 3]> {
    let cell = a.position.map(|v| v.floor() as i64);
    for dy in 0..=10 {
        let below = [cell[0], cell[1] - dy - 1, cell[2]];
        let material = h.block(below)?;
        if terra_voxel::MaterialId::from_raw(material).is_some_and(|m| m.is_solid()) {
            let p = [a.position[0], below[1] as f64 + 1.02, a.position[2]];
            return h
                .clear_rotated(p, a.size(), true, f64::from(a.yaw))
                .then_some(p);
        }
    }
    None
}

fn attached(a: &Creature, h: &Habitat) -> bool {
    let p = center(a).map(|v| v.floor() as i64);
    [[-1, 0, 0], [1, 0, 0], [0, -1, 0], [0, 0, -1], [0, 0, 1]]
        .into_iter()
        .any(|delta| {
            h.block(std::array::from_fn(|i| p[i] + delta[i]))
                .and_then(terra_voxel::MaterialId::from_raw)
                .is_some_and(|m| m.is_solid())
        })
}

fn yaw_delta(from: f32, to: f32) -> f32 {
    (to - from + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI
}

fn sweep_pose(a: &Creature, h: &Habitat, start: [f64; 3], end: [f64; 3], yaw: f32) -> bool {
    let turn = yaw_delta(a.yaw, yaw);
    let steps = (distance(start, end).sqrt() / 0.25)
        .max(f64::from(turn.abs()) / 0.12)
        .ceil()
        .max(1.0) as usize;
    (1..=steps).all(|step| {
        h.clear_rotated(
            std::array::from_fn(|i| start[i] + (end[i] - start[i]) * step as f64 / steps as f64),
            a.size(),
            true,
            f64::from(a.yaw) + f64::from(turn) * step as f64 / steps as f64,
        )
    })
}

pub(super) fn swim_step(a: &Creature, h: &Habitat, end: [f64; 3], yaw: f32) -> bool {
    sweep_pose(a, h, a.position, end, yaw)
}

fn heading_for(a: &Creature, next: [f64; 3], dt: f32) -> f32 {
    let dx = (next[0] - a.position[0]) as f32;
    let dz = (next[2] - a.position[2]) as f32;
    if dx.hypot(dz) <= 0.025 * dt {
        return a.yaw;
    }
    let desired = (-dx).atan2(-dz);
    (a.yaw + yaw_delta(a.yaw, desired).clamp(-dt * 2.2, dt * 2.2)).rem_euclid(std::f32::consts::TAU)
}

/// No full-world path searches: a swept local course and a bounded vertical detour avoid banks.
fn travel(a: &mut Creature, h: &Habitat, goal: [f64; 3], dt: f32, speed: f64) {
    let before = a.position;
    let delta = std::array::from_fn::<_, 3, _>(|i| goal[i] - before[i]);
    let length = distance(goal, before).sqrt().max(0.001);
    let amount = (speed * f64::from(dt)).min(length);
    let flow = crate::water::fluid_motion_at(h.terrain, h.chunks, h.journal, center(a));
    let next = std::array::from_fn(|i| {
        before[i]
            + delta[i] / length * amount
            + f64::from(flow.velocity[i].clamp(-0.6, 0.6) * dt * 0.18)
    });
    let next_yaw = heading_for(a, next, dt);
    if sweep_pose(a, h, before, next, next_yaw) {
        a.position = next;
        a.yaw = next_yaw;
    } else {
        // A wall changes the chosen course once, rather than spinning every simulation tick.
        let yaw = a.yaw;
        let sign = if a.id.is_multiple_of(2) { 1.0 } else { -1.0 };
        for [turn, rise] in [
            [0.8, -0.5],
            [-0.8, 0.5],
            [1.7, 0.0],
            [0.0, -1.0],
            [0.0, 1.0],
        ] {
            let heading = f64::from(yaw) + turn * sign;
            let horizontal = if rise.abs() == 1.0 { 0.0 } else { 1.0 };
            let probe = [
                before[0] - heading.sin() * amount * horizontal,
                before[1] + rise * amount,
                before[2] - heading.cos() * amount * horizontal,
            ];
            let probe_yaw = heading_for(a, probe, dt);
            if sweep_pose(a, h, before, probe, probe_yaw) {
                a.position = probe;
                a.yaw = probe_yaw;
                a.timer = a.timer.max(1.2);
                break;
            }
        }
    }
    a.velocity = std::array::from_fn(|i| ((a.position[i] - before[i]) / f64::from(dt)) as f32);
}

/// Uses the bear authority effect contract so fish catches and bites are committed exactly once.
pub fn step(
    a: &mut Creature,
    h: &Habitat,
    players: &[[f64; 3]],
    prey: &bears::PreyIndex,
    dt: f32,
) -> bears::Effect {
    if !a.kind.is_aquatic()
        || a.kind == Kind::Fish
        || a.health <= 0.0
        || !dt.is_finite()
        || !(0.0..=0.25).contains(&dt)
        || dt == 0.0
        || h.block(a.position.map(|v| v.floor() as i64)).is_none()
    {
        return bears::Effect::None;
    }
    a.age += f64::from(dt);
    a.timer = (a.timer - dt).max(0.0);
    a.needs.search_in = (a.needs.search_in - dt).max(0.0);
    a.needs.food = (a.needs.food - dt * 0.025).max(0.0);
    if !h.wet(center(a)) {
        a.behavior = Behavior::Stranded;
        super::support::settle(a, h, dt);
        a.health = (a.health - dt * 0.5).max(0.0);
        if a.health == 0.0 {
            a.behavior = Behavior::Dead;
        }
        return bears::Effect::None;
    }
    if a.kind == Kind::Squid {
        let threat = players
            .iter()
            .filter(|p| h.wet(**p))
            .filter(|p| distance(**p, center(a)) < 2.5_f64.powi(2))
            .min_by(|x, y| distance(**x, a.position).total_cmp(&distance(**y, a.position)));
        if let Some(threat) = threat {
            if a.needs.search_in <= 0.0 && a.behavior != Behavior::Ink {
                a.behavior = Behavior::Ink;
                a.timer = 0.75;
                a.needs.search_in = 9.0;
            } else if a.behavior != Behavior::Ink || a.timer <= 0.0 {
                a.behavior = Behavior::Flee;
                a.timer = 2.5;
            }
            let goal = std::array::from_fn(|i| a.position[i] + (a.position[i] - threat[i]) * 3.0);
            travel(a, h, goal, dt, a.kind.speed() * 1.8);
            return bears::Effect::None;
        }
        if a.behavior == Behavior::Ink && a.timer <= 0.0 {
            a.behavior = Behavior::Flee;
            a.timer = 2.5;
        }
    }
    let definition = terra_core::creatures::definitions::get(a.kind);
    if a.kind.hostile_to_players()
        && let Some(combat) = definition.combat
    {
        let target = super::perception::nearest_visible(
            h,
            center(a),
            center(a),
            players,
            combat.aggro_radius,
            |p| h.wet(p),
        );
        if let Some((index, player)) = target {
            let goal = [player[0], player[1], player[2]];
            if a.behavior != Behavior::Attack {
                a.timer = 0.6;
            }
            a.behavior = Behavior::Attack;
            travel(a, h, goal, dt, a.kind.speed() * 1.25);
            if a.timer <= 0.0
                && distance(center(a), player) <= combat.reach.powi(2)
                && bears::visible(h, center(a), player)
            {
                a.timer = combat.cooldown;
                return bears::Effect::StrikePlayer {
                    index,
                    damage: combat.damage,
                };
            }
            return bears::Effect::None;
        }
    }
    if a.kind.is_shark()
        && a.needs.food < a.kind.food() - 0.1
        && let Some((id, target)) = prey.nearest(center(a))
    {
        if a.behavior != Behavior::Hunt {
            a.timer = 0.5;
        }
        a.behavior = Behavior::Hunt;
        travel(a, h, target, dt, a.kind.speed() * 1.15);
        if a.timer <= 0.0 && can_catch(a, target, h) {
            a.timer = 1.5;
            return bears::Effect::CatchFish { id };
        }
        return bears::Effect::None;
    }
    if a.behavior == Behavior::Hold && a.timer > 0.0 && attached(a, h) {
        a.velocity = [0.0; 3];
        return bears::Effect::None;
    }
    if a.kind.is_jelly()
        && a.behavior == Behavior::Rest
        && a.timer > 0.0
        && let Some(floor) = bottom(a, h)
    {
        if distance(floor, a.position) < 0.02 {
            a.velocity = [0.0; 3];
        } else {
            travel(a, h, floor, dt, 0.22);
        }
        return bears::Effect::None;
    }
    if a.timer <= 0.0
        || matches!(
            a.behavior,
            Behavior::Stranded | Behavior::Attack | Behavior::Hunt
        )
    {
        let seed = random(a.id, (a.age / 3.0) as u64);
        a.timer = 3.0 + (seed % 50) as f32 * 0.1;
        if a.kind == Kind::Squid && seed.is_multiple_of(5) && attached(a, h) {
            a.behavior = Behavior::Hold;
        } else if a.kind.is_jelly() && seed.is_multiple_of(4) && bottom(a, h).is_some() {
            a.behavior = Behavior::Rest;
            a.timer += 5.0;
        } else {
            a.behavior = Behavior::Swim;
            // Interpolate subsequent heading changes, rather than snapping the model.
            a.needs.goal = None;
        }
    }
    let turn = (random(a.id, (a.age / 6.0) as u64) % 2000) as f64 / 1000.0 - 1.0;
    let heading = f64::from(a.yaw) + turn * 0.22;
    let goal = depth::course(
        a,
        h,
        [
            a.position[0] - heading.sin() * 4.0,
            a.position[1],
            a.position[2] - heading.cos() * 4.0,
        ],
    );
    travel(
        a,
        h,
        goal,
        dt,
        definition.speed
            * if a.behavior == Behavior::Flee {
                1.7
            } else {
                1.0
            },
    );
    bears::Effect::None
}

fn can_catch(a: &Creature, target: [f64; 3], h: &Habitat) -> bool {
    distance(center(a), target) <= (a.size()[0] * 0.65 + 0.65).powi(2)
        && h.wet(target)
        && bears::visible(h, center(a), target)
}

pub fn catch_fish(population: &mut CreaturePopulation, shark: u64, fish: u64, h: &Habitat) -> bool {
    let Some(predator) = population
        .animals
        .get(&shark)
        .filter(|a| a.kind.is_shark() && a.health > 0.0 && a.needs.food < a.kind.food())
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
    if let Some(predator) = population.animals.get_mut(&shark) {
        predator.needs.food = (predator.needs.food + 12.0).min(predator.kind.food());
        predator.behavior = Behavior::Swim;
        predator.timer = 3.0;
    }
    true
}
