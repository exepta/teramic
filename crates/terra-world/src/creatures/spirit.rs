//! Peaceful ambient fauna. Steering has a fixed collision-probe budget and never
//! asks unloaded chunks to generate synchronously. Saved position/age/ID drive motion.
use super::{Behavior, Creature, Habitat, Kind, distance, random};
pub mod caves;
mod raven;

pub const AURA_RADIUS: f64 = 6.0;

pub(super) fn surface_species(biome: &str, seed: u64) -> Option<Kind> {
    match biome {
        "spirit_blossom" => Some(match (seed >> 48) % 5 {
            0 => Kind::SpiritFox,
            1 | 2 => Kind::SpiritGloom,
            _ => Kind::Butterfly,
        }),
        "forest" | "cherry_forest" | "highland" if (seed >> 48) % 8 == 1 => Some(Kind::SpiritGloom),
        "blood_forest" if (seed >> 48).is_multiple_of(12) => Some(Kind::SpiritGloom),
        "mushroom_park" if (seed >> 48) % 16 == 1 => Some(Kind::SpiritGloom),
        "forest" | "cherry_forest" | "plains" if (seed >> 48).is_multiple_of(6) => {
            Some(Kind::Butterfly)
        }
        _ => None,
    }
}

/// One boolean per player, even if several spirits overlap. The server applies
/// this once per second through the existing strongest-refresh status-effect slot.
pub fn regeneration_near<'a>(
    position: [f64; 3],
    mut spirits: impl Iterator<Item = &'a Creature>,
) -> bool {
    spirits.any(|a| {
        a.kind == Kind::SpiritGloom
            && a.health > 0.0
            && distance(position, a.position) <= AURA_RADIUS * AURA_RADIUS
    })
}

pub(super) fn fly(a: &mut Creature, habitat: &Habitat, players: &[[f64; 3]], dt: f32) {
    if a.kind == Kind::Raven {
        raven::fly(a, habitat, players, dt);
        return;
    }
    let dt = f64::from(dt);
    a.age += dt;
    a.timer = (a.timer - dt as f32).max(0.0);
    let frightened =
        a.health < a.kind.health() && players.iter().any(|p| distance(*p, a.position) < 16.0);
    if a.timer == 0.0 || frightened {
        let seed = random(a.id, (a.age / 3.0) as u64);
        let home = [
            a.home[0] as f64 * 32.0 + 16.0,
            a.home[1] as f64 * 32.0 + 16.0,
        ];
        let heading = if (a.position[0] - home[0]).hypot(a.position[2] - home[1]) > 13.0 {
            (a.position[0] - home[0]).atan2(a.position[2] - home[1])
        } else if frightened {
            players
                .iter()
                .min_by(|x, y| distance(**x, a.position).total_cmp(&distance(**y, a.position)))
                .map_or(f64::from(a.yaw), |p| {
                    (p[0] - a.position[0]).atan2(p[2] - a.position[2])
                })
        } else {
            (seed % 6283) as f64 * 0.001
        };
        a.yaw = (heading as f32).rem_euclid(std::f32::consts::TAU);
        a.timer = 2.0 + (seed % 30) as f32 * 0.1;
        a.behavior = if frightened {
            Behavior::Flee
        } else {
            Behavior::Fly
        };
        // Close ground probing lets the insect perch without changing the
        // terrain. It departs again after this short nectar/rest interval.
        if a.kind == Kind::Butterfly && !frightened && seed.is_multiple_of(5) {
            for down in 0..=4 {
                let p = [
                    a.position[0],
                    a.position[1] - f64::from(down),
                    a.position[2],
                ];
                if let Some(ground) = habitat.ground_sized(p, a.size(), f64::from(a.yaw))
                    && (ground[1] - a.position[1]).abs() < 1.5
                {
                    a.behavior = Behavior::Land;
                    a.timer = 3.0;
                    break;
                }
            }
        }
    }
    a.velocity = [0.0; 3];
    if a.behavior == Behavior::Nectar {
        return;
    }
    if a.behavior == Behavior::Land {
        if let Some(ground) = habitat.ground_sized(a.position, a.size(), f64::from(a.yaw))
            && (a.position[1] - ground[1]).abs() < 0.08
        {
            a.position = ground;
            a.behavior = Behavior::Nectar;
            a.timer = 1.5;
        } else {
            let next = [a.position[0], a.position[1] - 0.5 * dt, a.position[2]];
            if habitat.clear(next, a.size(), false) && !habitat.wet(next) {
                a.position = next;
                a.velocity[1] = -0.5;
            } else {
                a.behavior = Behavior::Fly;
                a.timer = 0.0;
            }
        }
        return;
    }
    let speed = a.kind.speed() * if frightened { 1.7 } else { 1.0 };
    let vertical = (a.age * 1.3 + (a.id % 100) as f64).sin() * 0.25;
    let mut delta = [
        -f64::from(a.yaw.sin()) * speed * dt,
        vertical * dt,
        -f64::from(a.yaw.cos()) * speed * dt,
    ];
    // Keep near the actual edited floor. A solid cell beneath us triggers lift;
    // otherwise slowly descend, limiting flight to a few blocks over ground.
    let below = [a.position[0], a.position[1] - 1.1, a.position[2]];
    delta[1] += if habitat.clear(below, a.size(), false) {
        -0.12 * dt
    } else {
        0.55 * dt
    };
    let old = a.position;
    let next = std::array::from_fn(|i| old[i] + delta[i]);
    if habitat.clear(next, a.size(), false) && !habitat.wet(next) {
        a.position = next;
    } else {
        let above = [old[0], old[1] + dt * 0.6, old[2]];
        if habitat.clear(above, a.size(), false) && !habitat.wet(above) {
            a.position = above;
        }
        a.yaw = (a.yaw + dt as f32 * 2.4).rem_euclid(std::f32::consts::TAU);
        a.timer = a.timer.min(0.8);
    }
    a.velocity = std::array::from_fn(|i| ((a.position[i] - old[i]) / dt) as f32);
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn spawn_selection_has_all_species_and_respects_biomes() {
        let kinds: std::collections::BTreeSet<_> = (0..30)
            .filter_map(|n| surface_species("spirit_blossom", n << 48))
            .collect();
        assert_eq!(
            kinds,
            [Kind::Butterfly, Kind::SpiritGloom, Kind::SpiritFox].into()
        );
        for name in ["plains", "forest", "cherry_forest"] {
            assert_eq!(surface_species(name, 0), Some(Kind::Butterfly));
        }
        for name in ["desert", "ocean"] {
            for seed in 0..100 {
                assert_eq!(surface_species(name, seed << 48), None);
            }
        }
    }
}

#[cfg(test)]
#[path = "spirit_spawn_tests.rs"]
mod spawn_tests;
