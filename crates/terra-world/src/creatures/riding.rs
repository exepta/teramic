//! Shared mounted locomotion over the existing swept horse/voxel physics.
use super::{Behavior, Creature, Habitat, Kind, horses, needs, support};

pub const RIDER_SIZE: [f64; 3] = [0.6, 1.8, 0.6];
const JUMP_COST: f32 = 8.0;

#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct RideInput {
    /// World-space X/Z direction; diagonal input is normalized by the motor.
    pub direction: [f32; 2],
    /// A takeoff pulse, not a cooldown or an accumulated jump charge.
    pub jump: bool,
    pub sprint: bool,
}

pub fn seat_feet(horse: &Creature) -> [f64; 3] {
    [
        horse.position[0],
        horse.position[1] + horse.size()[1] * 0.72,
        horse.position[2],
    ]
}

fn clear(horse: &Creature, habitat: &Habitat) -> bool {
    habitat.clear_swimmer_rotated(horse.position, horse.size(), f64::from(horse.yaw))
        && habitat.clear_swimmer(seat_feet(horse), RIDER_SIZE)
}

fn grounded(horse: &Creature, habitat: &Habitat) -> bool {
    habitat
        .supported_sized(horse.position, horse.size(), f64::from(horse.yaw))
        .is_some_and(|ground| (ground[1] - horse.position[1]).abs() <= 0.03)
}

/// Advances age/needs, stamina and real ballistic motion. Invalid input is atomic.
pub fn step(
    horse: &mut Creature,
    stamina: &mut f32,
    habitat: &Habitat,
    input: RideInput,
    dt: f32,
) -> bool {
    let Some(traits) = horse.horse_traits() else {
        return false;
    };
    if !horse.valid()
        || horse.health <= 0.0
        || horse.juvenile()
        || !dt.is_finite()
        || dt <= 0.0
        || dt > 0.1
        || !stamina.is_finite()
        || !(0.0..=f32::from(traits.stamina)).contains(stamina)
        || input
            .direction
            .iter()
            .any(|v| !v.is_finite() || v.abs() > 1.001)
        || habitat
            .block(horse.position.map(|v| v.floor() as i64))
            .is_none()
    {
        return false;
    }
    horse.age += f64::from(dt);
    horse.timer = 0.0;
    horse.needs.goal = None;
    needs::update(horse, habitat, dt, false);
    if horse.health <= 0.0 {
        horse.behavior = Behavior::Dead;
        horse.velocity = [0.0; 3];
        return false;
    }
    let length = input.direction[0].hypot(input.direction[1]);
    let direction = input.direction.map(|v| f64::from(v / length.max(1.0)));
    let moving = length > 0.01;
    let sprinting = input.sprint && moving && *stamina > dt * 4.0;
    let speed = horse.kind.speed() * if sprinting { 3.2 } else { 2.0 };
    let mut jumped = false;
    if input.jump && grounded(horse, habitat) && *stamina >= JUMP_COST && clear(horse, habitat) {
        horse.velocity[1] = (24.0 * traits.jump_height).sqrt();
        horse.behavior = Behavior::Leap;
        *stamina -= JUMP_COST;
        jumped = true;
    }
    let count = (f64::from(dt) / (1.0 / 60.0)).ceil().max(1.0) as usize;
    let tick = dt / count as f32;
    for _ in 0..count {
        let before = horse.clone();
        let on_ground = grounded(horse, habitat) && horse.velocity[1] <= 0.0;
        if moving {
            let target = (-direction[0]).atan2(-direction[1]);
            let turn = (target - f64::from(horse.yaw) + std::f64::consts::PI)
                .rem_euclid(std::f64::consts::TAU)
                - std::f64::consts::PI;
            let mut turned = horse.clone();
            turned.yaw = (f64::from(horse.yaw)
                + turn.clamp(-6.0 * f64::from(tick), 6.0 * f64::from(tick)))
            .rem_euclid(std::f64::consts::TAU) as f32;
            if clear(&turned, habitat) {
                horse.yaw = turned.yaw;
            }
        }
        let target = direction.map(|v| if moving { v * speed } else { 0.0 });
        let delta = [
            target[0] - f64::from(horse.velocity[0]),
            target[1] - f64::from(horse.velocity[2]),
        ];
        let acceleration = if on_ground {
            if moving { 24.0 } else { 32.0 }
        } else {
            6.0
        };
        let fraction =
            (acceleration * f64::from(tick) / delta[0].hypot(delta[1]).max(0.000001)).min(1.0);
        horse.velocity[0] += (delta[0] * fraction) as f32;
        horse.velocity[2] += (delta[1] * fraction) as f32;
        let traveling = horse.velocity[0].hypot(horse.velocity[2]) > 0.001;
        let destination = [
            horse.position[0] + f64::from(horse.velocity[0]) * f64::from(tick),
            horse.position[1],
            horse.position[2] + f64::from(horse.velocity[2]) * f64::from(tick),
        ];
        if support::swimming(horse, habitat) {
            support::swim_toward(horse, habitat, destination, tick, speed);
        } else if on_ground {
            horse.velocity[1] = 0.0;
            if traveling
                && let Some(next) = habitat.travel_step(
                    horse.position,
                    destination,
                    Kind::Horse,
                    f64::from(horse.yaw),
                )
            {
                horse.position = next;
            }
            horse.behavior = if traveling {
                Behavior::Wander
            } else {
                Behavior::Rest
            };
        } else {
            horse.behavior = Behavior::Leap;
            horses::advance(horse, habitat, tick);
        }
        if !clear(horse, habitat) {
            let attempted = horse.position;
            horse.position = [attempted[0], before.position[1], attempted[2]];
            if clear(horse, habitat) {
                horse.velocity[1] = 0.0;
            } else {
                horse.position = [before.position[0], attempted[1], before.position[2]];
                if !clear(horse, habitat) {
                    horse.position = before.position;
                    horse.velocity[1] = 0.0;
                }
            }
        }
        for axis in [0, 2] {
            horse.velocity[axis] =
                ((horse.position[axis] - before.position[axis]) / f64::from(tick)) as f32;
        }
    }
    if sprinting {
        *stamina = (*stamina - dt * 4.0).max(0.0);
    } else if !jumped {
        *stamina = (*stamina + dt * 5.0).min(f32::from(traits.stamina));
    }
    horse.timer = 0.0;
    grounded(horse, habitat) && horse.velocity[1] <= 0.0
}

pub fn dismount(horse: &Creature, habitat: &Habitat) -> Option<[f64; 3]> {
    if horse.kind != Kind::Horse || !horse.valid() {
        return None;
    }
    let start = seat_feet(horse);
    let (sin, cos) = f64::from(horse.yaw).sin_cos();
    let size = horse.size();
    for extra in [0.0, 0.5] {
        for [x, z] in [
            [1.0, 0.0],
            [-1.0, 0.0],
            [0.0, 1.0],
            [1.0, 1.0],
            [-1.0, 1.0],
            [0.0, -1.0],
            [1.0, -1.0],
            [-1.0, -1.0],
        ] {
            let x = x * (size[0] * 0.5 + 0.65 + extra);
            let z = z * (size[2] * 0.5 + 0.65 + extra);
            let probe = [
                horse.position[0] + x * cos + z * sin,
                horse.position[1] + 0.5,
                horse.position[2] - x * sin + z * cos,
            ];
            let Some(end) = habitat.ground_sized(probe, RIDER_SIZE, 0.0) else {
                continue;
            };
            let distance = super::distance(start, end).sqrt();
            let count = (distance / 0.125).ceil().max(1.0) as usize;
            if (0..=count).all(|step| {
                let point = std::array::from_fn(|i| {
                    start[i] + (end[i] - start[i]) * step as f64 / count as f64
                });
                habitat.clear_swimmer(point, RIDER_SIZE)
            }) {
                return Some(end);
            }
        }
    }
    None
}

#[cfg(test)]
mod tests;
