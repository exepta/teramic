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
        behavior: if mob.kind.is_aquatic() {
            Behavior::Swim
        } else if mob.kind == Kind::Bee {
            Behavior::Fly
        } else {
            Behavior::Rest
        },
        timer: 1.0,
        age: 0.0,
        variant: mob.variant,
        needs: CreatureNeeds::full(mob.kind),
    };
    let size = animal.size();
    let (sin, cos) = f64::from(yaw).sin_cos();
    let center = [origin[0] - sin * 3.0, origin[2] - cos * 3.0];
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
            let position = if mob.kind.is_aquatic() {
                if !habitat.clear(candidate, size, true) {
                    continue;
                }
                candidate
            } else if mob.kind == Kind::Bee {
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
            animal.home = [
                (position[0].floor() as i64).div_euclid(32),
                (position[2].floor() as i64).div_euclid(32),
            ];
            return Ok(animal);
        }
    }
    Err(if mob.kind.is_aquatic() {
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
