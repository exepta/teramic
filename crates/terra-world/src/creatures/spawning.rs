//! Bounded placement for administrator-spawned creatures in loaded, edited terrain.
use super::{Behavior, Creature, CreaturePopulation, Habitat, Kind};
use terra_core::creatures::{
    CreatureNeeds,
    spawning::{SpawnMob, next_command_id},
};

pub fn near(
    population: &CreaturePopulation,
    habitat: &Habitat,
    mob: SpawnMob,
    origin: [f64; 3],
    yaw: f32,
) -> Result<Creature, &'static str> {
    if !origin.iter().all(|v| v.is_finite() && v.abs() <= 998_900.0) || !yaw.is_finite() {
        return Err("The spawn origin is outside the world.");
    }
    let id = next_command_id(population).ok_or("The world creature limit has been reached.")?;
    let mut animal = Creature {
        id,
        kind: mob.kind,
        home: [0, 0],
        position: origin,
        velocity: [0.0; 3],
        yaw: yaw.rem_euclid(std::f32::consts::TAU),
        health: mob.kind.health(),
        behavior: if mob.kind == Kind::StoneGolem {
            Behavior::Sleep
        } else if mob.kind == Kind::LavaWorm {
            Behavior::Burrowed
        } else if mob.kind == Kind::SandWorm {
            Behavior::Attack
        } else if mob.kind.is_aquatic() || mob.kind == Kind::WaterDragon {
            Behavior::Swim
        } else if mob.kind.is_flying() {
            Behavior::Fly
        } else {
            Behavior::Rest
        },
        timer: if mob.kind == Kind::SandWorm {
            terra_core::creatures::sandworm::EXPOSED_SECONDS
        } else {
            1.0
        },
        age: 0.0,
        variant: mob.variant,
        needs: CreatureNeeds::full(mob.kind),
    };
    let size = animal.size();
    let (sin, cos) = f64::from(yaw).sin_cos();
    let ahead = if mob.kind.is_dragon() { 15.0 } else { 3.0 };
    let center = [origin[0] - sin * ahead, origin[2] - cos * ahead];
    let mut offsets: Vec<_> = (-4_i32..=4)
        .flat_map(|z| (-4_i32..=4).map(move |x| [x, z]))
        .collect();
    offsets.sort_unstable_by_key(|[x, z]| (x * x + z * z, *z, *x));
    for [dx, dz] in offsets {
        let x = (center[0] + f64::from(dx)).floor() + 0.5;
        let z = (center[1] + f64::from(dz)).floor() + 0.5;
        // Leave room around the actor even for the widest species.
        if (x - origin[0]).hypot(z - origin[2]) < size[0].max(size[2]) * 0.5 + 0.8 {
            continue;
        }
        for dy in [0, -1, 1, -2, 2, -3, 3, -4, 4, -5, -6, -7, -8] {
            let candidate = [x, origin[1].floor() + f64::from(dy) + 0.1, z];
            let position = if mob.kind == Kind::LavaWorm {
                let Some(p) = super::lava_worm::anchor(habitat, candidate) else {
                    continue;
                };
                p
            } else if mob.kind == Kind::SandWorm {
                let Some(position) = super::sandworm::command_surface(habitat, candidate) else {
                    continue;
                };
                position
            } else if mob.kind.is_aquatic() || mob.kind == Kind::WaterDragon {
                if !habitat.clear_rotated(candidate, size, true, yaw.into()) {
                    continue;
                }
                candidate
            } else if mob.kind.is_flying() {
                let candidate = [x, candidate[1] + 1.0, z];
                if !habitat.clear(candidate, size, false) {
                    continue;
                }
                candidate
            } else {
                let Some(position) = habitat.ground(candidate, mob.kind) else {
                    continue;
                };
                if mob.kind == Kind::Stalker
                    && !super::navigation::caves::covered(habitat, position, size[1])
                {
                    continue;
                }
                position
            };
            if population
                .animals
                .values()
                .filter(|other| other.health > 0.0)
                .any(|other| overlaps(position, size, other.position, other.size()))
            {
                continue;
            }
            animal.position = position;
            if mob.kind == Kind::SandWorm {
                animal.yaw = ((position[0] - origin[0]).atan2(position[2] - origin[2])) as f32;
            }
            animal.home = [
                (position[0].floor() as i64).div_euclid(32),
                (position[2].floor() as i64).div_euclid(32),
            ];
            return Ok(animal);
        }
    }
    Err(if mob.kind == Kind::LavaWorm {
        "No open lava pool nearby."
    } else if mob.kind == Kind::SandWorm {
        "No dry loaded ground with enough room nearby. Move to open ground."
    } else if mob.kind.is_aquatic() || mob.kind == Kind::WaterDragon {
        "No loaded water with enough room nearby. Move closer to deeper water."
    } else {
        "No safe loaded space nearby. Move to open ground."
    })
}

fn overlaps(a: [f64; 3], size_a: [f64; 3], b: [f64; 3], size_b: [f64; 3]) -> bool {
    let radius = (size_a[0].max(size_a[2]) + size_b[0].max(size_b[2])) * 0.5;
    (a[0] - b[0]).abs() < radius
        && (a[2] - b[2]).abs() < radius
        && a[1] < b[1] + size_b[1]
        && b[1] < a[1] + size_a[1]
}
