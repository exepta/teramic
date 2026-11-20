//! Sand-only burrowing. The saved position is the surface anchor, not a fake
//! teleported body: core phase helpers define its submerged model and hitboxes.
use super::{Behavior as B, Creature, Habitat, Kind, bears::Effect, distance};
use terra_core::creatures::{definitions, sandworm::*};

#[cfg(test)]
mod tests;

pub(super) fn surface_species(biome: &str, seed: u64) -> Option<Kind> {
    (matches!(biome, "desert" | "cliffted_desert") && (seed >> 40).is_multiple_of(5))
        .then_some(Kind::SandWorm)
}

/// Dunes have stepped tops and a thin sand cap, rather than a flat three-block
/// slab. Check each column's actual surface before checking its substrate.
pub fn surface(h: &Habitat, candidate: [f64; 3]) -> Option<[f64; 3]> {
    let p = h.ground(candidate, Kind::SandWorm)?;
    if h.wet_footprint(p, Kind::SandWorm.size(), 0.0) {
        return None;
    }
    let mut lowest = i64::MAX;
    let mut highest = i64::MIN;
    for dx in [-0.95, 0.0, 0.95] {
        for dz in [-1.18, 0.0, 1.18] {
            let x = (p[0] + dx).floor() as i64;
            let z = (p[2] + dz).floor() as i64;
            let top = (0..=2).find_map(|drop| {
                let y = p[1].floor() as i64 - 1 - drop;
                let block = h.block([x, y, z])?;
                terra_voxel::MaterialId::from_raw(block)
                    .is_some_and(|m| m.is_solid())
                    .then_some((y, block))
            })?;
            lowest = lowest.min(top.0);
            highest = highest.max(top.0);
            if top.1 != terra_voxel::MaterialId::Sand as u16 {
                return None;
            }
            for depth in 1..=2 {
                let cell = [x, top.0 - depth, z];
                let substrate = h.block(cell)?;
                if substrate != terra_voxel::MaterialId::Sand as u16
                    && !(substrate == terra_voxel::MaterialId::Sandstone as u16
                        && h.journal.block(cell).is_none())
                {
                    return None;
                }
            }
        }
    }
    if highest - lowest > 1 {
        return None;
    }
    for above in 4..=8 {
        if h.block([
            p[0].floor() as i64,
            p[1].floor() as i64 + above,
            p[2].floor() as i64,
        ])
        .is_none_or(|block| terra_voxel::MaterialId::from_raw(block).is_none_or(|m| m.is_solid()))
        {
            return None;
        }
    }
    Some(p)
}

/// Commands place the exposed animal, so administrators can inspect and fight
/// it on any dry ground with sufficient space. Natural burrowing remains sand-only.
pub fn command_surface(h: &Habitat, candidate: [f64; 3]) -> Option<[f64; 3]> {
    surface(h, candidate).or_else(|| {
        let p = h.ground(candidate, Kind::SandWorm)?;
        (!h.wet_footprint(p, Kind::SandWorm.size(), 0.0) && h.clear(p, [2.0, 8.0, 2.5], false))
            .then_some(p)
    })
}

/// One rejected voxel sample must not discard an otherwise valid desert cell.
/// All probes remain near the deterministic slot and retain its saved identity.
pub(super) fn spawn_surface(h: &Habitat, candidate: [f64; 3]) -> Option<[f64; 3]> {
    for [dx, dz] in [
        [0., 0.],
        [-3., 0.],
        [3., 0.],
        [0., -3.],
        [0., 3.],
        [-3., -3.],
        [3., -3.],
        [-3., 3.],
        [3., 3.],
    ] {
        if let Some(point) = surface(h, [candidate[0] + dx, candidate[1], candidate[2] + dz]) {
            return Some(point);
        }
    }
    None
}

/// Repair the previously impossible spawn in already explored empty habitats.
/// Existing live or dead IDs are never replaced, so no kill can reroll a worm.
pub(super) fn recover_missing_spawn(
    population: &mut terra_core::creatures::CreaturePopulation,
    h: &Habitat,
    home: [i64; 2],
) -> bool {
    let Some(id) = terra_core::creatures::creature_id(home, 0) else {
        return false;
    };
    if population.animals.contains_key(&id)
        || population.animals.len() >= terra_core::creatures::MAX_CREATURES
    {
        return false;
    }
    let seed = super::random(id, h.terrain.config().seed.0);
    if (seed >> 32).is_multiple_of(3) {
        return false;
    }
    let x = (home[0] * 32 + 8 + (seed % 8) as i64) as f64 + 0.5;
    let z = (home[1] * 32 + 8 + ((seed >> 8) % 8) as i64) as f64 + 0.5;
    let Some(column) = h
        .chunks
        .surface_column_at(x.floor() as i64, z.floor() as i64)
    else {
        return false;
    };
    let biome = super::biome_slug(h.terrain.surface_biome(column));
    if surface_species(biome, seed) != Some(Kind::SandWorm)
        || !definitions::get(Kind::SandWorm).permits_spawn_biome(biome)
    {
        return false;
    }
    let Some(position) = spawn_surface(h, [x, column.height.floor() + 1.0, z]) else {
        return false;
    };
    if population
        .animals
        .values()
        .any(|other| other.health > 0.0 && distance(other.position, position) < 6.0_f64.powi(2))
    {
        return false;
    }
    population.animals.insert(
        id,
        Creature {
            id,
            kind: Kind::SandWorm,
            home,
            position,
            velocity: [0.0; 3],
            yaw: (seed % 628) as f32 * 0.01,
            health: Kind::SandWorm.health(),
            behavior: B::Burrowed,
            timer: 1.0 + (seed % 40) as f32 * 0.1,
            age: 0.0,
            variant: 0,
            needs: terra_core::creatures::CreatureNeeds::full(Kind::SandWorm),
        },
    );
    true
}

/// A recent ground vibration. Only the authority knows which actor made it
/// and can measure its direction between consecutive observations.
#[derive(Clone, Copy, Debug)]
pub struct Footfall {
    pub position: [f64; 3],
    pub velocity: [f64; 2],
    pub prey: bool,
}

pub fn step(a: &mut Creature, h: &Habitat, players: &[[f64; 3]], dt: f32) -> Effect {
    step_observed(a, h, players, players, dt)
}

pub fn step_observed(
    a: &mut Creature,
    h: &Habitat,
    targets: &[[f64; 3]],
    disturbances: &[[f64; 3]],
    dt: f32,
) -> Effect {
    let footfalls = disturbances.iter().map(|&position| Footfall {
        position,
        velocity: [0.; 2],
        prey: targets.contains(&position),
    });
    step_sensed(a, h, targets, footfalls, dt)
}

pub fn step_sensed(
    a: &mut Creature,
    h: &Habitat,
    targets: &[[f64; 3]],
    footfalls: impl Iterator<Item = Footfall>,
    dt: f32,
) -> Effect {
    if a.kind != Kind::SandWorm || a.health <= 0.0 || !dt.is_finite() || dt <= 0.0 || dt > 0.25 {
        return Effect::None;
    }
    let previous_timer = a.timer;
    a.age += f64::from(dt);
    a.timer = (a.timer - dt).max(0.0);
    a.velocity = [0.0; 3];
    let burrowable = surface(h, a.position);
    let commanded = terra_core::creatures::spawning::is_command_id(a.id);
    // Phase clocks do not require distant roof/substrate samples. Once a breach
    // starts it must finish and disappear even while its terrain streams out.
    match a.behavior {
        B::Leap => {
            if a.timer <= 0.0 {
                let landing = leap_destination(a);
                let valid = surface(h, landing);
                a.position = valid.unwrap_or(landing);
                a.behavior = if valid.is_some() {
                    B::Emerge
                } else {
                    B::Burrowed
                };
                a.timer = if valid.is_some() {
                    EMERGE_SECONDS
                } else {
                    BURROW_SECONDS
                };
            }
            return Effect::None;
        }
        B::Retreat if a.timer <= 0.0 => {
            a.behavior = B::Burrowed;
            a.timer = BURROW_SECONDS * (1.0 - 0.45 * injury(a));
            a.needs.goal = None;
            return Effect::None;
        }
        B::Retreat => return Effect::None,
        B::Spit => {
            if a.timer <= 0.0 {
                a.behavior = B::Retreat;
                a.timer = RETREAT_SECONDS;
                a.needs.goal = None;
            }
            return Effect::None;
        }
        B::Hunt => return charge(a, h, targets, dt),
        B::Emerge => {
            let contact = EMERGE_SECONDS * (1.0 - KNOCKUP_PHASE);
            let charged_hit = a
                .needs
                .goal
                .is_some_and(|g| g.kind == terra_core::creatures::NeedKind::AttackAim);
            if !charged_hit
                && previous_timer > contact
                && a.timer <= contact
                && let Some((index, _)) = targets.iter().enumerate().take(512).find(|(_, p)| {
                    p.iter().all(|v| v.is_finite())
                        && (p[0] - a.position[0]).hypot(p[2] - a.position[2]) <= 1.45
                        && (0.0..=3.2).contains(&(p[1] - a.position[1]))
                        && super::perception::visible(
                            h,
                            [a.position[0], a.position[1] + 0.15, a.position[2]],
                            **p,
                        )
                })
            {
                return Effect::StrikePlayer {
                    index,
                    damage: KNOCKUP_DAMAGE,
                };
            }
            if a.timer <= 0.0 {
                a.needs.goal = None;
                let eye = mouth_position(a);
                a.behavior = if burrowable.is_some() || commanded {
                    B::Attack
                } else {
                    B::Retreat
                };
                a.timer = if a.behavior == B::Attack {
                    // At stand-off distance open with the raised spit posture.
                    // Starting after the bite's recovery preserves the existing
                    // neck/mandible pose and leaves a visible windup to dodge.
                    let ranged = super::perception::nearest_visible(
                        h,
                        eye,
                        a.position,
                        targets,
                        SPIT_RANGE,
                        |_| true,
                    )
                    .is_some_and(|(_, p)| distance(a.position, p) > 10.0_f64.powi(2));
                    if ranged {
                        EXPOSED_SECONDS * 0.64
                    } else {
                        EXPOSED_SECONDS
                    }
                } else {
                    RETREAT_SECONDS
                };
            }
            return Effect::None;
        }
        B::Attack if a.timer <= 0.0 => {
            a.behavior = B::Retreat;
            a.timer = RETREAT_SECONDS;
            return Effect::None;
        }
        B::Attack if burrowable.is_none() && !commanded => return Effect::None,
        B::Attack => return attack(a, h, targets, previous_timer, dt, burrowable.is_some()),
        B::Tremor => {
            if a.timer <= 0.0 {
                if a.needs
                    .goal
                    .is_some_and(|g| g.kind == terra_core::creatures::NeedKind::AttackAim)
                    && burrowable.is_some()
                {
                    // This goal was locked when the rumble began. A charge
                    // commits to that line so a warned victim can sidestep it.
                    a.behavior = B::Hunt;
                    a.timer = CHARGE_SECONDS;
                    return Effect::None;
                }
                a.needs.goal = None;
                // A close encounter erupts vertically from the warned hole.
                // The travelling arc is locomotion toward distant prey, not
                // a mandatory fourteen-metre detour before every bite.
                let distant_travel = targets
                    .iter()
                    .copied()
                    .chain(footfalls.map(|signal| signal.position))
                    .take(512)
                    .filter(|p| p.iter().all(|v| v.is_finite()))
                    .map(|p| distance(p, a.position))
                    .min_by(f64::total_cmp)
                    .is_some_and(|distance| {
                        (9.0_f64.powi(2)..=24.0_f64.powi(2)).contains(&distance)
                    });
                if distant_travel && burrowable.is_some() && leap_clear(a, h) {
                    a.behavior = B::Leap;
                    a.timer = LEAP_SECONDS;
                } else if burrowable.is_some()
                    || commanded && command_surface(h, a.position).is_some()
                {
                    a.behavior = B::Emerge;
                    a.timer = EMERGE_SECONDS;
                } else {
                    a.behavior = B::Burrowed;
                    a.timer = BURROW_SECONDS;
                }
            }
            return Effect::None;
        }
        _ => {}
    }
    let Some(base) =
        burrowable.or_else(|| commanded.then(|| command_surface(h, a.position)).flatten())
    else {
        if h.block(a.position.map(|v| v.floor() as i64)).is_some()
            && let Some(recovered) = spawn_surface(h, a.position)
        {
            a.position = recovered;
            a.behavior = B::Burrowed;
        }
        return Effect::None;
    };
    a.position = base;
    a.behavior = B::Burrowed;
    let moving = footfalls
        .take(512)
        .filter(|signal| {
            let p = signal.position;
            p.iter().all(|v| v.is_finite())
                && (p[1] - a.position[1] - 0.9).abs() < 2.0
                && distance(p, a.position) <= 32.0_f64.powi(2)
        })
        .min_by(|x, y| {
            y.prey.cmp(&x.prey).then_with(|| {
                distance(x.position, a.position).total_cmp(&distance(y.position, a.position))
            })
        });
    if let Some(signal) = moving {
        let target = signal.position;
        let dx = target[0] - a.position[0];
        let dz = target[2] - a.position[2];
        let range = dx.hypot(dz);
        if a.timer <= 0.0 {
            a.behavior = B::Tremor;
            a.timer = WARNING_SECONDS;
            let aim = if signal.prey && (5.0..=20.0).contains(&range) {
                let intercept = intercept_position(signal, range);
                let aim = if intercept != target && surface(h, intercept).is_some() {
                    intercept
                } else {
                    target
                };
                a.needs.goal = Some(terra_core::creatures::NeedGoal {
                    kind: terra_core::creatures::NeedKind::AttackAim,
                    position: aim.map(|v| v.floor() as i64),
                });
                aim
            } else {
                a.needs.goal = None;
                target
            };
            a.yaw = ((a.position[0] - aim[0]).atan2(a.position[2] - aim[2])) as f32;
        } else if range > 1.75 {
            let mut aim = intercept_position(signal, range);
            let pressure = f64::from(injury(a));
            // A wounded worm skirts the target during recovery, folding back
            // toward it as the next breach becomes available.
            if pressure > 0.0 && signal.prey && range < 18.0 {
                let flank = pressure * flank_side(a) * f64::from(a.timer.min(3.0));
                aim[0] -= dz / range * flank;
                aim[2] += dx / range * flank;
            }
            pursue(a, h, aim, dt, 1.25 + 0.35 * pressure);
        }
    } else {
        // Territorial patrol is real subterranean movement, independent of
        // hostility. It never breaches merely because a quiet observer exists.
        // Six-metre local routes keep a patrol near its saved home and sand.
        let home = [
            a.home[0] as f64 * 32.0 + 16.0,
            a.home[1] as f64 * 32.0 + 16.0,
        ];
        let sector = (a.age / 6.0).floor() as u64;
        let phase = super::random(a.id ^ sector, h.terrain.config().seed.0);
        let angle = (phase % 6283) as f64 * 0.001;
        let target = [
            home[0] + angle.sin() * 6.0,
            a.position[1],
            home[1] + angle.cos() * 6.0,
        ];
        travel(a, h, target, dt, 0.28);
    }
    Effect::None
}

/// Escalation starts at half health. Full warning and attack animation
/// durations still apply; it is the underground repositioning that accelerates.
fn injury(a: &Creature) -> f32 {
    (1.0 - 2.0 * a.health / a.kind.health()).clamp(0.0, 1.0)
}

fn flank_side(a: &Creature) -> f64 {
    if a.id.count_ones().is_multiple_of(2) {
        1.0
    } else {
        -1.0
    }
}

/// Anticipate at most three metres. Once a charge is announced its aim stays
/// fixed even if the target reverses, stops, or another creature walks closer.
fn intercept_position(signal: Footfall, range: f64) -> [f64; 3] {
    let [vx, vz] = signal.velocity;
    let speed = vx.hypot(vz);
    if !signal.prey || !speed.is_finite() || speed < 1.0 {
        return signal.position;
    }
    let lead_time = (range / (Kind::SandWorm.speed() * CHARGE_PACE))
        .min(0.75)
        .min(3.0 / speed);
    [
        signal.position[0] + vx * lead_time,
        signal.position[1],
        signal.position[2] + vz * lead_time,
    ]
}

fn pursue(a: &mut Creature, h: &Habitat, aim: [f64; 3], dt: f32, pace: f64) {
    let before = a.position;
    travel(a, h, aim, dt, pace);
    if a.position != before {
        return;
    }
    // On a blocked pursuit try skirting the edge of the obstacle. Charges
    // still use straight swept travel and never curve around a missed target.
    let dx = aim[0] - before[0];
    let dz = aim[2] - before[2];
    let side = flank_side(a);
    for sign in [side, -side] {
        // Voxel faces are axis aligned. Rotating the target vector here can
        // point both trial diagonals into the same corner as the target moves.
        let tangent = if dx.abs() > dz.abs() {
            [before[0], before[1], before[2] + sign * dx]
        } else {
            [before[0] - sign * dz, before[1], before[2]]
        };
        travel(a, h, tangent, dt, pace);
        if a.position != before {
            break;
        }
    }
}

/// Fast swept travel on the warned line. The sand ridge is the visible attack;
/// the buried anatomy remains absent from meshes and hit targets throughout.
fn charge(a: &mut Creature, h: &Habitat, targets: &[[f64; 3]], dt: f32) -> Effect {
    let Some(goal) = a
        .needs
        .goal
        .filter(|g| g.kind == terra_core::creatures::NeedKind::AttackAim)
    else {
        a.behavior = B::Burrowed;
        a.timer = BURROW_SECONDS;
        return Effect::None;
    };
    let target = goal.position.map(|v| v as f64 + 0.5);
    let before = a.position;
    travel(a, h, target, dt, CHARGE_PACE);
    let movement = [a.position[0] - before[0], a.position[2] - before[2]];
    let length2 = movement[0] * movement[0] + movement[1] * movement[1];
    for (index, victim) in targets.iter().enumerate().take(512) {
        if !victim.iter().all(|v| v.is_finite()) || (victim[1] - a.position[1]).abs() > 3.0 {
            continue;
        }
        let t = if length2 > 1e-8 {
            (((victim[0] - before[0]) * movement[0] + (victim[2] - before[2]) * movement[1])
                / length2)
                .clamp(0.0, 1.0)
        } else {
            0.0
        };
        let point = [
            before[0] + movement[0] * t,
            a.position[1] + 0.5,
            before[2] + movement[1] * t,
        ];
        if (victim[0] - point[0]).hypot(victim[2] - point[2]) <= 1.35
            && super::perception::visible(h, point, *victim)
        {
            a.behavior = B::Emerge;
            // Retain the existing aim until the rise finishes as a consumed
            // charge marker. This keeps the complete vertical animation while
            // preventing an immediate second hit from the underfoot window.
            a.timer = EMERGE_SECONDS;
            return Effect::StrikePlayer {
                index,
                damage: CHARGE_DAMAGE,
            };
        }
    }
    if a.timer <= 0.0 || length2 < 1e-6 || distance(a.position, target) < 1.75_f64.powi(2) {
        a.behavior = B::Emerge;
        a.timer = EMERGE_SECONDS;
        a.needs.goal = None;
    }
    Effect::None
}

fn leap_clear(a: &Creature, h: &Habitat) -> bool {
    let (sin, cos) = f64::from(a.yaw).sin_cos();
    for step in 0..=8 {
        let distance = f64::from(LEAP_DISTANCE) * f64::from(step) / 8.0;
        let point = [
            a.position[0] - sin * distance,
            a.position[1],
            a.position[2] - cos * distance,
        ];
        let Some(ground) = surface(h, point) else {
            return false;
        };
        if (ground[1] - a.position[1]).abs() > 1.01 {
            return false;
        }
        for y in [2.0, 4.0, 6.0] {
            if !h.clear(
                [point[0], a.position[1] + y, point[2]],
                [2.0, 2.0, 2.0],
                false,
            ) {
                return false;
            }
        }
    }
    true
}

/// Swept short probes keep pursuit and idle motion on confirmed dry sand.
fn travel(a: &mut Creature, h: &Habitat, target: [f64; 3], dt: f32, pace: f64) {
    let before = a.position;
    let dx = target[0] - before[0];
    let dz = target[2] - before[2];
    let distance = dx.hypot(dz);
    if distance < 0.001 {
        return;
    }
    let amount = (a.kind.speed() * f64::from(dt) * pace).min(distance);
    let mut next = before;
    for part in 1..=3 {
        let p = [
            before[0] + dx / distance * amount * f64::from(part) / 3.0,
            before[1],
            before[2] + dz / distance * amount * f64::from(part) / 3.0,
        ];
        let Some(p) = surface(h, p) else {
            break;
        };
        if (p[1] - next[1]).abs() > 1.01 {
            break;
        }
        next = p;
    }
    a.position = next;
    a.velocity = std::array::from_fn(|i| ((next[i] - before[i]) / f64::from(dt)) as f32);
    if next != before {
        a.yaw = ((-dx).atan2(-dz) as f32).rem_euclid(std::f32::consts::TAU);
    }
}

fn attack(
    a: &mut Creature,
    h: &Habitat,
    players: &[[f64; 3]],
    previous_timer: f32,
    dt: f32,
    moving: bool,
) -> Effect {
    let Some(combat) = definitions::get(a.kind).combat else {
        return Effect::None;
    };
    let progress = phase_progress(B::Attack, a.timer);
    let eye = mouth_position(a);
    let visible = super::perception::nearest_visible(
        h,
        eye,
        a.position,
        players,
        combat.aggro_radius,
        |_| true,
    );
    // After its opening strike the worm can abandon a lost target. A wounded
    // worm also escapes a melee surround rather than standing up for a useless
    // ranged shot. A retreat always shows the normal exposed dive animation.
    if (0.40..SPIT_PHASE).contains(&progress)
        && moving
        && !players.is_empty()
        && (visible.is_none()
            || (injury(a) > 0.0
                && visible.is_some_and(|(_, p)| distance(a.position, p) <= combat.reach.powi(2))))
    {
        a.behavior = B::Retreat;
        a.timer = RETREAT_SECONDS;
        a.needs.goal = None;
        return Effect::None;
    }
    if progress < SPIT_PHASE - 0.10
        && let Some((_, target)) = visible
    {
        a.yaw = ((a.position[0] - target[0]).atan2(a.position[2] - target[2])) as f32;
        if moving
            && (0.30..0.52).contains(&progress)
            && distance(a.position, target) > (combat.reach + 1.0).powi(2)
        {
            travel(a, h, target, dt, 0.45);
        }
    }
    let crossed = |phase| {
        previous_timer > EXPOSED_SECONDS * (1.0 - phase)
            && a.timer <= EXPOSED_SECONDS * (1.0 - phase)
    };
    if crossed(BITE_PHASE)
        && let Some((index, target)) = super::perception::nearest_visible(
            h,
            [a.position[0], a.position[1] + 1.25, a.position[2]],
            a.position,
            players,
            combat.reach + 1.0,
            |_| true,
        )
    {
        a.yaw = ((a.position[0] - target[0]).atan2(a.position[2] - target[2])) as f32;
        let mut biting = a.clone();
        biting.timer = EXPOSED_SECONDS * (1.0 - BITE_PHASE);
        let bite_mouth = mouth_position(&biting);
        if distance(bite_mouth, target) <= 1.45_f64.powi(2)
            && super::perception::visible(h, bite_mouth, target)
        {
            return Effect::StrikePlayer {
                index,
                damage: combat.damage,
            };
        }
    }
    if crossed(SPIT_PHASE) {
        // Lock a real, currently visible vulnerable enemy. Creative observers,
        // empty idle cycles and targets behind terrain never provoke a spit.
        if let Some((_, target)) =
            visible.filter(|(_, target)| distance(a.position, *target) > 3.5_f64.powi(2))
        {
            a.yaw = ((a.position[0] - target[0]).atan2(a.position[2] - target[2])) as f32;
            a.needs.goal = Some(terra_core::creatures::NeedGoal {
                kind: terra_core::creatures::NeedKind::AttackAim,
                position: target.map(|v| v.floor().clamp(-998_999.0, 998_999.0) as i64),
            });
            if let Some((origin, velocity)) = aimed_spit_launch(a).filter(|(origin, velocity)| {
                clear_spit_arc(h, *origin, *velocity, a.needs.goal.unwrap().position)
            }) {
                a.behavior = B::Spit;
                a.timer = SPIT_SECONDS;
                return Effect::SpitBile { origin, velocity };
            }
            a.needs.goal = None;
        }
    }
    Effect::None
}

/// The actual lob, not just an eye-to-eye sightline, must clear loaded terrain.
fn clear_spit_arc(h: &Habitat, origin: [f64; 3], velocity: [f64; 3], aim: [i64; 3]) -> bool {
    let target = aim.map(|v| v as f64 + 0.5);
    let speed = velocity[0].hypot(velocity[2]);
    if speed < 0.001 {
        return false;
    }
    let flight = (target[0] - origin[0]).hypot(target[2] - origin[2]) / speed;
    let mut before = origin;
    for step in 1..=16 {
        let t = flight * f64::from(step) / 16.0;
        let point = std::array::from_fn::<_, 3, _>(|i| {
            origin[i] + velocity[i] * t
                - if i == 1 {
                    0.5 * f64::from(SPIT_GRAVITY) * t * t
                } else {
                    0.0
                }
        });
        let delta = std::array::from_fn::<_, 3, _>(|i| point[i] - before[i]);
        let distance = delta.iter().map(|v| v * v).sum::<f64>().sqrt();
        if distance > 0.0001
            && terra_voxel::blocks::raycast_blocks(
                before,
                delta.map(|v| v / distance),
                distance,
                |p| {
                    Some(h.block(p).map_or(3, |raw| {
                        if terra_voxel::MaterialId::from_raw(raw).is_none_or(|m| m.is_solid()) {
                            raw
                        } else {
                            0
                        }
                    }))
                },
            )
            .is_some()
        {
            return false;
        }
        before = point;
    }
    true
}
