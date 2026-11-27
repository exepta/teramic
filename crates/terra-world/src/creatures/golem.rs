//! A guardian remembers its original perch across combat, unloading and restart.
use super::{
    Behavior as B, Creature, Habitat, Kind, distance, forest, navigation, perception, support,
};
use terra_core::creatures::{NeedGoal, NeedKind, definitions, golem as timing};
const TERRITORY_RADIUS: f64 = 18.;

/// Each 96 m district nominates one den. Only dry, loaded Petrified Forest
/// ground can claim it, including on a previous visit that found no occupant.
pub(super) fn habitat_cell(home: [i64; 2], seed: u64) -> bool {
    let district = home.map(|v| v.div_euclid(3));
    let Some(id) = terra_core::creatures::creature_id(district, 0) else {
        return false;
    };
    let draw = super::random(id, seed ^ 0xa97d_643c_11e2);
    home.map(|v| v.rem_euclid(3)) == [(draw % 3) as i64, ((draw >> 17) % 3) as i64]
}
fn potential_habitat(terrain: &crate::TerraGenerator, home: [i64; 2]) -> bool {
    if !habitat_cell(home, terrain.config().seed.0) {
        return false;
    }
    let center = home.map(|v| (v * 32 + 16) as f64);
    terrain
        .climate_biome_at(center[0], center[1])
        .weights
        .petrified_forest
        >= 0.35
}
pub fn populate(
    population: &mut super::CreaturePopulation,
    h: &Habitat,
    home: [i64; 2],
    players: &[[f64; 3]],
) -> bool {
    use terra_core::creatures::{CreatureNeeds, creature_id};
    use terra_voxel::MaterialId as M;
    if !potential_habitat(h.terrain, home)
        || population.animals.len() >= terra_core::creatures::MAX_CREATURES
        || (!population.explored.contains(&home)
            && population.explored.len() >= terra_core::creatures::MAX_SPAWN_CELLS)
    {
        return false;
    }
    // Older explored habitats may already contain herd animals. Preserve all
    // of them, including a dead guardian's identity, across repeated visits.
    let mut vacant = None;
    for slot in 0..4 {
        let Some(id) = creature_id(home, slot) else {
            return false;
        };
        match population.animals.get(&id) {
            Some(existing) if existing.kind == Kind::StoneGolem => return false,
            None if vacant.is_none() => vacant = Some(id),
            _ => {}
        }
    }
    let Some(id) = vacant else {
        return false;
    };
    if h.terrain.dragon_nest(home).is_some() {
        return false;
    }
    let draw = super::random(id, h.terrain.config().seed.0);
    let yaw = ((draw % 628) as f32) * 0.01;
    for i in 0..9 {
        // Visit all nine perches in a stable permutation. A fallen root or a narrow
        // tread at one point must not permanently invalidate the whole district.
        let cell = (draw % 9 + i * 5) % 9;
        let x = home[0] * 32 + 6 + 10 * (cell % 3) as i64;
        let z = home[1] * 32 + 6 + 10 * (cell / 3) as i64;
        let (cx, cz) = (x as f64 + 0.5, z as f64 + 0.5);
        let col = h.terrain.surface_column(cx, cz);
        if h.terrain.surface_biome(&col) != terra_core::BiomeKind::PetrifiedForest
            || col.water_level.is_some()
        {
            continue;
        }
        let Some(position) =
            h.ground_rotated([cx, col.water_bed(), cz], Kind::StoneGolem, f64::from(yaw))
        else {
            continue;
        };
        if players
            .iter()
            .any(|p| super::distance(*p, position) < 14.0_f64.powi(2))
            || population
                .animals
                .values()
                .any(|a| a.health > 0. && super::distance(a.position, position) < 6.0_f64.powi(2))
        {
            continue;
        }
        let surface = h
            .block([x, (position[1] - 0.01).floor() as i64, z])
            .and_then(M::from_raw);
        if !matches!(
            surface,
            Some(
                M::DustyGround
                    | M::CrackedStoneSoil
                    | M::FossiliferousStone
                    | M::WeatheredRuinStone
                    | M::Stone
                    | M::MineralStrata
            )
        ) {
            continue;
        }
        let mut needs = CreatureNeeds::full(Kind::StoneGolem);
        needs.goal = Some(NeedGoal {
            kind: NeedKind::Territory,
            position: position.map(|v| v.floor() as i64),
        });
        population.animals.insert(
            id,
            Creature {
                id,
                kind: Kind::StoneGolem,
                home,
                position,
                velocity: [0.; 3],
                yaw,
                health: Kind::StoneGolem.health(),
                behavior: B::Sleep,
                timer: 6. + (draw % 9) as f32,
                age: 0.,
                variant: ((draw >> 12) & 1) as u8,
                needs,
            },
        );
        population.explored.insert(home);
        return true;
    }
    false
}

pub fn step(
    a: &mut Creature,
    h: &Habitat,
    players: &[[f64; 3]],
    dt: f32,
    search: bool,
    nav: Option<&mut navigation::Navigation>,
    mut strike: impl FnMut(usize, f32),
) {
    if a.kind != Kind::StoneGolem
        || a.health <= 0.
        || !(0.0..=0.25).contains(&dt)
        || dt == 0.
        || h.block(a.position.map(|x| x.floor() as i64)).is_none()
    {
        return;
    }
    let anchor = a
        .needs
        .goal
        .get_or_insert_with(|| NeedGoal {
            kind: NeedKind::Territory,
            position: a.position.map(|x| x.floor() as i64),
        })
        .position;
    let home = [
        anchor[0] as f64 + 0.5,
        anchor[1] as f64,
        anchor[2] as f64 + 0.5,
    ];
    let inside = |p: [f64; 3]| (p[0] - home[0]).hypot(p[2] - home[2]) <= TERRITORY_RADIUS;
    support::settle(a, h, dt);
    let before = a.timer;
    a.age += f64::from(dt);
    a.timer = (a.timer - dt).max(0.);
    a.needs.search_in = if search {
        0.6
    } else {
        (a.needs.search_in - dt).max(0.)
    };
    let combat = definitions::get(a.kind).combat.unwrap();
    let eye = [a.position[0], a.position[1] + 2.85, a.position[2]];
    if matches!(a.behavior, B::Emerge | B::Attack | B::Tremor | B::Shake) {
        let action = a.behavior;
        a.velocity[0] = 0.;
        a.velocity[2] = 0.;
        let impact = if action == B::Tremor {
            timing::SLAM_IMPACT
        } else {
            definitions::MobCombat::MELEE_IMPACT_PHASE
        };
        let at = timing::duration(action) * (1. - impact);
        if matches!(action, B::Attack | B::Tremor) && before > at && a.timer <= at {
            let radius = if action == B::Tremor {
                timing::SLAM_RADIUS
            } else {
                combat.reach
            };
            let accepts = |p: [f64; 3]| {
                inside(p)
                    && (p[1] - a.position[1] - 0.9).abs() <= 2.1
                    && (action == B::Tremor || perception::in_melee_arc(a.yaw, a.position, p))
            };
            if action == B::Attack {
                if let Some((i, _)) = perception::nearest_visible(
                    h,
                    eye,
                    [a.position[0], a.position[1] + 0.9, a.position[2]],
                    players,
                    radius,
                    accepts,
                ) {
                    strike(i, combat.damage);
                }
            } else {
                for (i, &p) in players.iter().enumerate() {
                    if accepts(p)
                        && (p[0] - a.position[0]).hypot(p[2] - a.position[2]) <= radius
                        && perception::visible(h, eye, p)
                    {
                        strike(i, combat.damage * 1.35);
                    }
                }
            }
        }
        if a.timer == 0. {
            if action == B::Emerge {
                a.behavior = B::Wander;
                a.timer = 13. + (a.id % 5) as f32;
            } else {
                a.behavior = B::Hunt;
            }
        }
        return;
    }
    let sleeping = matches!(a.behavior, B::Sleep | B::Rest);
    let target = perception::nearest_visible(
        h,
        eye,
        a.position,
        players,
        if sleeping { 8. } else { combat.aggro_radius },
        inside,
    );
    if a.behavior == B::Flee || sleeping && target.is_some() {
        a.behavior = B::Emerge;
        a.timer = timing::WAKE_SECONDS;
        a.velocity = [0.; 3];
        return;
    }
    if let Some((_, p)) = target {
        let target = [p[0], p[1] - 0.9, p[2]];
        let d = distance(a.position, target).sqrt();
        a.yaw = ((a.position[0] - p[0]).atan2(a.position[2] - p[2]) as f32)
            .rem_euclid(std::f32::consts::TAU);
        let slam = ((a.age * 0.27) as u64).wrapping_add(a.id).is_multiple_of(2);
        if (slam && d < timing::SLAM_RADIUS - 0.55) || d < combat.reach {
            a.behavior = if slam { B::Tremor } else { B::Attack };
            a.timer = timing::duration(a.behavior);
            a.velocity = [0.; 3];
        } else {
            a.behavior = B::Hunt;
            a.timer = 0.;
            forest::travel(a, h, target, dt, search, nav);
        }
    } else if sleeping && (a.timer > 0. || a.age <= f64::from(dt) + 0.001) {
        if a.timer == 0. {
            a.timer = 9. + (a.id % 8) as f32;
        }
        a.behavior = B::Sleep;
    } else if sleeping {
        a.behavior = B::Emerge;
        a.timer = timing::WAKE_SECONDS;
        a.velocity = [0.; 3];
    } else if a.behavior == B::Wander && a.timer > 0. {
        // The deadline stays fixed during this patrol leg, including across save/load.
        let bearing = ((a.age + f64::from(a.timer)).round() + (a.id % 113) as f64) * 2.39996;
        let destination = [
            home[0] + 6.5 * bearing.cos(),
            home[1],
            home[2] + 6.5 * bearing.sin(),
        ];
        a.behavior = B::Wander;
        if (a.position[0] - destination[0]).hypot(a.position[2] - destination[2]) > 0.75 {
            forest::travel(a, h, destination, dt, search, nav);
        } else {
            a.timer = 0.;
            a.velocity[0] = 0.;
            a.velocity[2] = 0.;
        }
    } else if (a.position[0] - home[0]).hypot(a.position[2] - home[2]) > 0.9 {
        a.behavior = B::Wander;
        forest::travel(a, h, home, dt, search, nav);
    } else {
        a.behavior = B::Sleep;
        a.timer = 12. + (a.id % 9) as f32;
        a.velocity[0] = 0.;
        a.velocity[2] = 0.;
    }
}
