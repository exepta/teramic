use super::{Behavior, Creature, Forage, Habitat, Kind, distance, random};
use crate::props::PropKind;
use terra_core::creatures::ecology::{BeeState, Ecology, NestState};
/// Stable chronotypes stagger bedtime and waking without runtime random IDs.
pub fn sleep_at(id: u64, phase: f64) -> bool {
    let offset = (random(id, 0xbed) % 1000) as f64 / 1000.0;
    phase >= 19.0 / 24.0 + offset * 0.09 || phase < 5.0 / 24.0 + offset * 0.04
}
#[cfg(test)]
#[path = "ecology_tests.rs"]
mod tests;

pub fn discover(eco: &mut Ecology, habitat: &Habitat) {
    for section in habitat.chunks.loaded.values() {
        for p in &section.props {
            if p.kind == PropKind::BeeNest
                && !p.removed
                && !habitat.journal.prop_removed(p.section, p.id)
                && eco.nests.len() < 16_384
            {
                eco.nests.entry(p.id.0).or_insert(NestState {
                    position: p.position,
                    nectar: 0,
                    honey: 0,
                    progress: 0.0,
                });
            }
        }
    }
    eco.nests.retain(|id, n| {
        let owner = terra_voxel::global_to_section(n.position.map(|v| v.floor() as i64)).0;
        // Wall nests may belong to the neighboring trunk's section.
        ![-1, 0, 1].into_iter().any(|dx| {
            [-1, 0, 1].into_iter().any(|dz| {
                owner
                    .offset([dx, 0, dz])
                    .is_ok_and(|p| habitat.journal.prop_removed(p, crate::props::PropId(*id)))
            })
        })
    });
}

pub fn step(
    animal: &mut Creature,
    eco: &mut Ecology,
    habitat: &Habitat,
    players: &[[f64; 3]],
    dt: f32,
    search: bool,
    night: bool,
) -> Forage {
    step_with_navigation(animal, eco, habitat, players, dt, search, night, None)
}
#[allow(clippy::too_many_arguments)]
pub fn step_with_navigation(
    animal: &mut Creature,
    eco: &mut Ecology,
    habitat: &Habitat,
    players: &[[f64; 3]],
    dt: f32,
    search: bool,
    night: bool,
    navigation: Option<&mut super::navigation::Navigation>,
) -> Forage {
    if animal.health <= 0.0 || !(0.0..=0.25).contains(&dt) || dt == 0.0 {
        return Forage::None;
    }
    if habitat
        .chunks
        .block_at(animal.position.map(|v| v.floor() as i64))
        .is_none()
    {
        return Forage::None;
    }
    if animal.kind == Kind::Bee {
        bee(
            animal,
            eco,
            habitat,
            dt,
            night && terra_core::creatures::definitions::get(Kind::Bee).sleeps,
            search,
            navigation,
        );
        return Forage::None;
    }
    if animal.kind == Kind::Bear {
        super::bears::step_with_navigation(
            animal,
            habitat,
            players,
            &super::bears::PreyIndex::default(),
            dt,
            search,
            night,
            navigation,
        );
        return Forage::None;
    }
    if animal.kind == Kind::Fish && fish_leap(animal, habitat, dt) {
        return Forage::None;
    }
    if night
        && terra_core::creatures::definitions::get(animal.kind).sleeps
        && animal.behavior != Behavior::Flee
        && animal.behavior != Behavior::Leap
        && !super::support::swimming(animal, habitat)
        && let Some(ground) =
            habitat.ground_rotated(animal.position, animal.kind, f64::from(animal.yaw))
    {
        animal.position = ground;
        animal.behavior = Behavior::Sleep;
        animal.velocity = [0.0; 3];
        animal.age += f64::from(dt);
        animal.needs.goal = None;
        animal.needs.food = (animal.needs.food - dt * 0.002).max(0.0);
        animal.needs.water = (animal.needs.water - dt * 0.003).max(0.0);
        return Forage::None;
    }
    if animal.behavior == Behavior::Sleep {
        animal.behavior = Behavior::Wander;
        animal.timer = 0.0;
    }
    super::step_with_flowers(animal, habitat, players, dt, search, Some(eco), navigation)
}

fn bee(
    a: &mut Creature,
    eco: &mut Ecology,
    h: &Habitat,
    dt: f32,
    night: bool,
    search: bool,
    navigation: Option<&mut super::navigation::Navigation>,
) {
    if !eco.bees.contains_key(&a.id) {
        let nearest = eco
            .nests
            .iter()
            .filter(|(_, n)| distance(n.position, a.position) < 64.0_f64.powi(2))
            .min_by(|(_, a1), (_, b)| {
                distance(a1.position, a.position).total_cmp(&distance(b.position, a.position))
            })
            .map(|(&id, _)| id);
        if let Some(nest) = nearest {
            eco.bees.insert(
                a.id,
                BeeState {
                    nest,
                    ..Default::default()
                },
            );
        }
    }
    if a.behavior == Behavior::Nectar {
        let flower = super::bees::nearby(a, h)
            .filter(|p| {
                p.kind == PropKind::Flower
                    && eco
                        .bees
                        .get(&a.id)
                        .and_then(|b| b.flower)
                        .is_none_or(|id| id == p.id.0)
                    && !p.removed
                    && !h.journal.prop_removed(p.section, p.id)
                    && distance(p.position, a.position) < 2.25
            })
            .min_by(|x, y| {
                let head = |p: &crate::props::WorldProp| {
                    [
                        p.position[0],
                        p.position[1] + f64::from(p.visual_scale()) * 0.85,
                        p.position[2],
                    ]
                };
                distance(head(x), a.position)
                    .total_cmp(&distance(head(y), a.position))
                    .then(x.id.cmp(&y.id))
            })
            .copied();
        if let Some(f) = flower {
            if let Some(b) = eco.bees.get_mut(&a.id) {
                b.flower = Some(f.id.0);
            }
            if a.timer > dt {
                super::step_with_flowers(a, h, &[], dt, search, Some(eco), navigation);
                return;
            }
            let capacity = eco.bees.get(&a.id).map_or(0, |b| 100 - b.carried);
            let amount = eco.take_nectar(f.id.0, f.position, 25.min(capacity));
            if let Some(b) = eco.bees.get_mut(&a.id) {
                b.carried += amount;
                b.flower = Some(f.id.0);
            }
        }
        if a.timer <= dt {
            a.behavior = Behavior::Fly;
            a.timer = 0.0;
            a.needs.goal = None;
        }
    }
    let state = eco.bees.get(&a.id).cloned().unwrap_or_default();
    if let Some(nest) = eco.nests.get(&state.nest).cloned() {
        let rotation = super::bees::nearby(a, h)
            .find(|p| p.id.0 == state.nest)
            .map_or(0.0, |p| f64::from(p.rotation));
        let target = [
            nest.position[0] + rotation.sin() * 0.4,
            nest.position[1] + 0.25,
            nest.position[2] + rotation.cos() * 0.4,
        ];
        if state.inside {
            a.age += f64::from(dt);
            a.velocity = [0.0; 3];
            a.timer = (a.timer - dt).max(0.0);
            if night || a.timer > 0.0 {
                a.behavior = Behavior::InNest;
                return;
            }
            if h.clear(target, Kind::Bee.size(), false) {
                a.position = target;
                a.behavior = Behavior::Fly;
                a.timer = 1.0;
                if let Some(b) = eco.bees.get_mut(&a.id) {
                    b.inside = false;
                }
            }
            return;
        }
        if night || state.carried >= 50 || distance(a.position, nest.position) > 100.0_f64.powi(2) {
            a.age += f64::from(dt);
            a.behavior = Behavior::Fly;
            if distance(a.position, target) < 0.65_f64.powi(2) {
                if let Some(n) = eco.nests.get_mut(&state.nest) {
                    n.nectar = (n.nectar + u16::from(state.carried)).min(1000);
                }
                let room = eco.inside(state.nest) < 10;
                if let Some(b) = eco.bees.get_mut(&a.id) {
                    b.carried = 0;
                    b.inside = room;
                }
                a.behavior = if room {
                    Behavior::InNest
                } else {
                    Behavior::Rest
                };
                a.velocity = [0.0; 3];
                a.timer = 2.0 + (a.id % 5) as f32 * 0.2;
            } else {
                super::navigation::fly_cached(a, h, target, dt, navigation);
            }
            return;
        }
    } else if let Some(b) = eco.bees.get_mut(&a.id) {
        b.inside = false;
    }
    if a.needs.goal.is_none()
        && a.behavior != Behavior::Nectar
        && let Some(b) = eco.bees.get_mut(&a.id)
    {
        b.flower = None;
    }
    super::step_with_flowers(a, h, &[], dt, search, Some(eco), navigation);
    let flower = if a.behavior == Behavior::Nectar {
        eco.bees.get(&a.id).and_then(|b| b.flower).or_else(|| {
            super::bees::nearby(a, h)
                .filter(|p| {
                    p.kind == PropKind::Flower
                        && !p.removed
                        && !h.journal.prop_removed(p.section, p.id)
                })
                .filter(|p| {
                    distance(
                        [
                            p.position[0],
                            p.position[1] + f64::from(p.visual_scale()) * 0.85,
                            p.position[2],
                        ],
                        a.position,
                    ) < 0.25
                })
                .min_by_key(|p| p.id)
                .map(|p| p.id.0)
        })
    } else {
        a.needs
            .goal
            .filter(|g| g.kind == terra_core::creatures::NeedKind::Food)
            .and_then(|g| {
                super::bees::nearby(a, h)
                    .filter(|p| {
                        p.kind == PropKind::Flower
                            && p.position.map(|v| v.floor() as i64) == g.position
                    })
                    .min_by(|x, y| {
                        distance(x.position, a.position)
                            .total_cmp(&distance(y.position, a.position))
                            .then(x.id.cmp(&y.id))
                    })
                    .map(|p| p.id.0)
            })
    };
    if let Some(b) = eco.bees.get_mut(&a.id) {
        b.flower = flower;
    }
}
fn fish_leap(a: &mut Creature, h: &Habitat, dt: f32) -> bool {
    if a.behavior != Behavior::Leap
        && a.age > 3.0
        && random(a.id, (a.age * 10.0) as u64).is_multiple_of(240)
        && h.wet(a.position)
        && !h.wet([a.position[0], a.position[1] + 0.6, a.position[2]])
        && h.clear_swimmer(
            [a.position[0], a.position[1] + 1.8, a.position[2]],
            a.kind.size(),
        )
    {
        a.behavior = Behavior::Leap;
        a.timer = 2.2;
        a.velocity = [-a.yaw.sin() * 0.8, 5.5, -a.yaw.cos() * 0.8];
    }
    if a.behavior != Behavior::Leap {
        return false;
    }
    a.age += f64::from(dt);
    a.timer = (a.timer - dt).max(0.0);
    a.velocity[1] -= 9.8 * dt;
    let next = std::array::from_fn(|i| a.position[i] + f64::from(a.velocity[i] * dt));
    if h.clear_swimmer(next, a.kind.size()) {
        a.position = next;
    } else {
        a.velocity[1] = a.velocity[1].min(0.0);
    }
    if (a.velocity[1] < 0.0 && h.wet(a.position)) || a.timer == 0.0 {
        a.behavior = Behavior::Swim;
        a.timer = 3.0;
    }
    true
}
