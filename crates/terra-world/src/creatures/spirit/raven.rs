//! Ravens cruise above actual loaded canopy instead of following the insect
//! ground-hover rule. No chunk generation, chunk requests or new saved state.
use super::{Behavior, Creature, Habitat, distance, random};

const CLIMB_SPEED: f64 = 2.6;
const DESCENT_SPEED: f64 = 1.4;
const SCAN_DOWN: i64 = 64;
const SCAN_UP: i64 = 48;

fn natural_water(h: &Habitat, cell: [i64; 3]) -> crate::water::WaterColumns {
    #[cfg(test)]
    tests::record_water_resolution();
    h.chunks
        .prepared_column([cell[0].div_euclid(16), cell[2].div_euclid(16)])
        .and_then(|column| {
            column
                .surface_at(cell[0], cell[2])
                .map(|s| s.water_columns())
        })
        .unwrap_or_else(|| {
            h.terrain
                .water_columns(cell[0] as f64 + 0.5, cell[2] as f64 + 0.5)
        })
}

/// Highest occupied surface in the connected loaded column and the loaded sky
/// ceiling. Leaves, player structures and edited ground all contribute.
fn column(h: &Habitat, p: [f64; 3]) -> Option<(Option<f64>, f64)> {
    let cell = p.map(|v| v.floor() as i64);
    h.block(cell)?;
    let mut surface = None;
    let mut water = None;
    for down in 0..=SCAN_DOWN {
        let q = [cell[0], cell[1] - down, cell[2]];
        let Some(material) = h.block(q) else { break };
        if terra_voxel::MaterialId::from_raw(material).is_none_or(|m| m.is_solid()) {
            surface = Some(q[1] as f64 + 1.0);
            break;
        }
        if h.terrain.config().water && crate::water::can_hold_fluid(material) {
            // Resolve natural heights once for this X/Z column. The former
            // per-cell query could resample noise up to 65 times on a cache miss.
            let fluid = h.journal.fluid(q).unwrap_or_else(|| {
                crate::water::fluid::from_columns(
                    *water.get_or_insert_with(|| natural_water(h, cell)),
                    q[1],
                )
            });
            if fluid.amount > 0 {
                surface = Some(q[1] as f64 + fluid.height());
                break;
            }
        }
    }
    let mut ceiling = cell[1] as f64 + 1.0;
    for up in 1..=SCAN_UP {
        let q = [cell[0], cell[1] + up, cell[2]];
        let Some(material) = h.block(q) else { break };
        ceiling = q[1] as f64 + 1.0;
        if terra_voxel::MaterialId::from_raw(material).is_none_or(|m| m.is_solid()) {
            surface = Some(q[1] as f64 + 1.0);
        }
    }
    Some((surface, ceiling))
}

fn cruise_heading(a: &Creature) -> f32 {
    // A persisted age/identity defines one stable waypoint for eight seconds.
    // Recompute its heading as we move, so rate-limited steering continues over
    // subsequent ticks instead of discarding the target after one frame.
    let seed = random(a.id, (a.age / 8.0) as u64);
    let angle = (seed % 6283) as f64 * 0.001;
    let radius = 15.0 + ((seed >> 16) % 801) as f64 * 0.01;
    let target = [
        a.home[0] as f64 * 32.0 + 16.0 + angle.sin() * radius,
        a.home[1] as f64 * 32.0 + 16.0 + angle.cos() * radius,
    ];
    (a.position[0] - target[0]).atan2(a.position[2] - target[1]) as f32
}

fn cruise_height(a: &Creature, h: &Habitat) -> f64 {
    let Some((mut surface, ceiling)) = column(h, a.position) else {
        return a.position[1];
    };
    // Look ahead and around the wings before entering a taller grove. Unknown
    // neighbouring columns are not grounds for generating or teleporting there.
    let forward = [-f64::from(a.yaw.sin()), -f64::from(a.yaw.cos())];
    for offset in [
        [forward[0] * 9.0, forward[1] * 9.0],
        [7.0, 0.0],
        [-7.0, 0.0],
        [0.0, 7.0],
        [0.0, -7.0],
        [7.0, 7.0],
        [-7.0, 7.0],
        [7.0, -7.0],
        [-7.0, -7.0],
    ] {
        let p = [
            a.position[0] + offset[0],
            a.position[1],
            a.position[2] + offset[1],
        ];
        if let Some((Some(height), _)) = column(h, p) {
            surface = Some(surface.map_or(height, |old| old.max(height)));
        }
    }
    let clearance = 12.0 + (random(a.id, 0x7261_766e) % 801) as f64 * 0.01;
    let target = surface.map_or(a.position[1], |height| height + clearance);
    (target + (a.age * 0.3 + (a.id % 31) as f64).sin() * 0.45).min(ceiling - a.size()[1] - 0.15)
}

fn yaw_delta(from: f32, to: f32) -> f64 {
    (f64::from(to - from) + std::f64::consts::PI).rem_euclid(std::f64::consts::TAU)
        - std::f64::consts::PI
}

fn turn(from: f32, to: f32, dt: f64) -> f32 {
    (f64::from(from) + yaw_delta(from, to).clamp(-2.2 * dt, 2.2 * dt))
        .rem_euclid(std::f64::consts::TAU) as f32
}

fn sweep(a: &Creature, h: &Habitat, target: [f64; 3], yaw: f32) -> bool {
    let angle = yaw_delta(a.yaw, yaw);
    let steps = ((distance(a.position, target).sqrt() / 0.18).max(angle.abs() / 0.1))
        .ceil()
        .max(1.0) as usize;
    (1..=steps).all(|i| {
        let fraction = i as f64 / steps as f64;
        let p = std::array::from_fn(|axis| {
            a.position[axis] + (target[axis] - a.position[axis]) * fraction
        });
        h.clear_rotated(p, a.size(), false, f64::from(a.yaw) + angle * fraction)
    })
}

/// A landing target needs real dry support under the whole body. No artificial
/// descent to an unloaded floor or through foliage is accepted.
fn perch(a: &Creature, h: &Habitat) -> Option<[f64; 3]> {
    let start = a.position[1].floor() as i64;
    for down in 0..=32 {
        let cell = [
            a.position[0].floor() as i64,
            start - down,
            a.position[2].floor() as i64,
        ];
        let material = h.block(cell)?;
        if !terra_voxel::MaterialId::from_raw(material).is_none_or(|m| m.is_solid()) {
            continue;
        }
        let point = [a.position[0], cell[1] as f64 + 1.0, a.position[2]];
        let ground = h.ground_sized(point, a.size(), f64::from(a.yaw))?;
        return (ground[1] <= a.position[1] + 0.08 && !h.wet(ground)).then_some(ground);
    }
    None
}

pub(super) fn fly(a: &mut Creature, h: &Habitat, players: &[[f64; 3]], dt: f32) {
    let dt = f64::from(dt);
    let old = a.position;
    let old_age = a.age;
    a.age += dt;
    a.timer = (a.timer - dt as f32).max(0.0);
    a.velocity = [0.0; 3];
    let frightened =
        a.health < a.kind.health() && players.iter().any(|p| distance(*p, a.position) < 36.0);
    if frightened && matches!(a.behavior, Behavior::Nectar | Behavior::Land) {
        a.behavior = Behavior::Flee;
        a.timer = 0.0;
    }
    if a.behavior == Behavior::Nectar {
        if a.timer > 0.0 && perch(a, h).is_some_and(|p| (p[1] - a.position[1]).abs() < 0.08) {
            return;
        }
        a.behavior = Behavior::Fly;
        a.timer = 0.0;
    }
    // Rest is occasional (roughly every two minutes), never the repeated
    // near-ground landing roll used by butterflies. Existing age persists it.
    let period = 110.0 + (a.id % 31) as f64;
    let phase = (a.id % 29) as f64;
    if !frightened
        && a.age >= 60.0
        && a.behavior != Behavior::Land
        && ((old_age + phase) / period).floor() < ((a.age + phase) / period).floor()
        && perch(a, h).is_some()
    {
        a.behavior = Behavior::Land;
        a.timer = 26.0;
    }
    if a.behavior == Behavior::Land {
        if a.timer > 0.0
            && let Some(ground) = perch(a, h)
        {
            let next = [old[0], (old[1] - DESCENT_SPEED * dt).max(ground[1]), old[2]];
            if sweep(a, h, next, a.yaw) {
                a.position = next;
                a.velocity[1] = ((next[1] - old[1]) / dt) as f32;
                if (next[1] - ground[1]).abs() < 0.001 {
                    a.behavior = Behavior::Nectar;
                    a.timer = 3.0 + (a.id % 3) as f32;
                }
                return;
            }
        }
        a.behavior = Behavior::Fly;
        a.timer = 0.0;
    }
    let home = [
        a.home[0] as f64 * 32.0 + 16.0,
        a.home[1] as f64 * 32.0 + 16.0,
    ];
    let homeward = (old[0] - home[0]).hypot(old[2] - home[1]) > 28.0;
    let mut heading = cruise_heading(a);
    if homeward {
        heading = (old[0] - home[0]).atan2(old[2] - home[1]) as f32;
    } else if frightened
        && let Some(player) = players
            .iter()
            .min_by(|x, y| distance(**x, old).total_cmp(&distance(**y, old)))
    {
        heading = (player[0] - old[0]).atan2(player[2] - old[2]) as f32;
    }
    a.behavior = if frightened {
        Behavior::Flee
    } else {
        Behavior::Fly
    };
    let yaw = turn(a.yaw, heading, dt);
    let vertical = (cruise_height(a, h) - old[1]).clamp(-DESCENT_SPEED, CLIMB_SPEED) * dt;
    let speed = a.kind.speed() * if frightened { 1.7 } else { 1.0 };
    let next = [
        old[0] - f64::from(yaw.sin()) * speed * dt,
        old[1] + vertical,
        old[2] - f64::from(yaw.cos()) * speed * dt,
    ];
    if sweep(a, h, next, yaw) {
        a.position = next;
        a.yaw = yaw;
    } else {
        // Prefer rising around a trunk/ledge, then use a bounded side turn. A
        // roof or unloaded sky simply blocks the climb; nothing is generated.
        let above = [old[0], old[1] + CLIMB_SPEED * dt, old[2]];
        if vertical > 0.0 && sweep(a, h, above, a.yaw) {
            a.position = above;
        } else {
            for side in [1.0_f32, -1.0, 2.5, -2.5] {
                let yaw = turn(a.yaw, a.yaw + side, dt);
                let next = [
                    old[0] - f64::from(yaw.sin()) * speed * dt,
                    old[1],
                    old[2] - f64::from(yaw.cos()) * speed * dt,
                ];
                if sweep(a, h, next, yaw) {
                    a.position = next;
                    a.yaw = yaw;
                    break;
                }
            }
        }
        a.timer = a.timer.min(0.5);
    }
    a.velocity = std::array::from_fn(|i| ((a.position[i] - old[i]) / dt) as f32);
}

#[cfg(test)]
mod tests;
