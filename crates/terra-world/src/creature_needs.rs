use super::{Habitat, distance};
use crate::{
    edits::EditJournal,
    props::{PropId, PropKind, WorldProp, placement::support},
    streaming::{ChunkStore, PropRemoveRequest, commit_prop_removal},
};
use terra_core::creatures::{
    Creature, CreatureBehavior as Behavior, CreatureKind as Kind, NeedGoal, NeedKind,
};
use terra_voxel::{SectionPos, global_to_section};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Forage {
    None,
    Prop { section: SectionPos, id: PropId },
    Pollinate { section: SectionPos, id: PropId },
}

fn edible(kind: Kind, prop: &WorldProp) -> bool {
    !prop.removed
        && !prop.hidden_growth()
        && !prop.wall_attached()
        && match kind {
            Kind::Cow | Kind::Sheep | Kind::Horse | Kind::Elephant | Kind::Zebra => {
                prop.kind == PropKind::Grass || prop.kind == PropKind::Wheat && prop.variant == 7
            }
            Kind::Pig => {
                matches!(prop.kind, PropKind::Mushroom | PropKind::Flower)
                    || (prop.kind == PropKind::SmallPlant && prop.variant < 128)
                    || matches!(prop.kind, PropKind::Corn | PropKind::Lettuce) && prop.variant == 7
            }
            _ => false,
        }
}

fn food_at(habitat: &Habitat, kind: Kind, position: [i64; 3]) -> Option<WorldProp> {
    let section = habitat.chunks.loaded.get(&global_to_section(position).0)?;
    section
        .props
        .iter()
        .find(|p| {
            edible(kind, p)
                && support(p) == position
                && !habitat.journal.prop_removed(p.section, p.id)
        })
        .copied()
}

fn grazable_ground(kind: Kind, habitat: &Habitat, ground: [i64; 3]) -> bool {
    use terra_voxel::MaterialId as M;
    if matches!(kind, Kind::Hen | Kind::Rooster) {
        return super::poultry::forage_ground(habitat, ground);
    }
    matches!(kind, Kind::Zebra | Kind::Elephant)
        && matches!(
            habitat.block(ground).and_then(M::from_raw),
            Some(M::SavannaTurf | M::DrySavannaTurf)
        )
}

fn find_food(animal: &Creature, habitat: &Habitat) -> Option<NeedGoal> {
    let [x, y, z] = animal.position.map(|v| v.floor() as i64);
    let soil = [x, y - 1, z];
    if grazable_ground(animal.kind, habitat, soil) {
        return Some(NeedGoal {
            kind: NeedKind::Food,
            position: soil,
        });
    }
    let owner = global_to_section([x, y, z]).0;
    let mut nearest = None;
    let mut best = 8.0_f64.powi(2);
    let mut inspected = 0;
    // At most 2048 stored props, in stable section order, only when hungry.
    for dy in -1..=0 {
        for dz in -1..=1 {
            for dx in -1..=1 {
                let Ok(p) = owner.offset([dx, dy, dz]) else {
                    continue;
                };
                let Some(section) = habitat.chunks.loaded.get(&p) else {
                    continue;
                };
                for prop in &section.props {
                    inspected += 1;
                    if inspected > 2048 {
                        return nearest;
                    }
                    if !edible(animal.kind, prop) || habitat.journal.prop_removed(p, prop.id) {
                        continue;
                    }
                    let d = distance(animal.position, prop.position);
                    if d < best && (prop.position[1] - animal.position[1]).abs() <= 1.05 {
                        best = d;
                        nearest = Some(NeedGoal {
                            kind: NeedKind::Food,
                            position: support(prop),
                        });
                    }
                }
            }
        }
    }
    nearest
}

pub(super) fn water_at(habitat: &Habitat, p: [i64; 3]) -> bool {
    water_surface(habitat, p).is_some()
}

fn water_surface(habitat: &Habitat, p: [i64; 3]) -> Option<f64> {
    let _scope = terra_core::profiling::ScopeTimer::new("creatures.needs.water_surface");
    let material = habitat.block(p)?;
    let fluid = habitat
        .chunks
        .fluid_cell_at(habitat.terrain, habitat.journal, p, material);
    (fluid.amount > 0).then_some(p[1] as f64 + fluid.height())
}

pub(super) fn find_water(animal: &Creature, habitat: &Habitat, radius: i64) -> Option<NeedGoal> {
    let _scope = terra_core::profiling::ScopeTimer::new("creatures.needs.find_water");
    let origin = animal.position.map(|v| v.floor() as i64);
    // Include water one full block below the bank: at most 192 cell probes.
    for r in 1..=radius {
        for [dx, dz] in [
            [1, 0],
            [-1, 0],
            [0, 1],
            [0, -1],
            [1, 1],
            [-1, 1],
            [1, -1],
            [-1, -1],
        ] {
            for dy in [-2, -1, 0, 1] {
                let p = [origin[0] + dx * r, origin[1] + dy, origin[2] + dz * r];
                if !water_at(habitat, p) {
                    continue;
                }
                let center = [p[0] as f64 + 0.5, animal.position[1], p[2] as f64 + 0.5];
                let vx = center[0] - animal.position[0];
                let vz = center[2] - animal.position[2];
                let length = vx.hypot(vz).max(0.1);
                let size = animal.size();
                let bank_distance = (size[0].max(size[2]) * 0.5 + 0.75).max(1.6);
                let shore = [
                    center[0] - vx / length * bank_distance,
                    center[1],
                    center[2] - vz / length * bank_distance,
                ];
                if habitat
                    .ground_sized(shore, size, f64::from(animal.yaw))
                    .is_some()
                {
                    return Some(NeedGoal {
                        kind: NeedKind::Water,
                        position: p,
                    });
                }
            }
        }
    }
    None
}

pub(super) fn update(animal: &mut Creature, habitat: &Habitat, dt: f32, search: bool) -> Forage {
    if animal.kind.is_aquatic() {
        return Forage::None;
    }
    let moving = animal.velocity[0].hypot(animal.velocity[2]).min(3.0);
    animal.needs.food = (animal.needs.food - dt * (0.012 + moving * 0.003)).max(0.0);
    animal.needs.water = (animal.needs.water - dt * (0.018 + moving * 0.006)).max(0.0);
    animal.needs.search_in = (animal.needs.search_in - dt).max(0.0);
    if animal.kind.food() > 0.0 && animal.needs.food == 0.0
        || animal.kind.water() > 0.0 && animal.needs.water == 0.0
    {
        animal.health = (animal.health - dt * 0.025).max(0.0);
    }
    if animal.health <= 0.0 {
        animal.behavior = Behavior::Dead;
        animal.velocity = [0.0; 3];
        animal.needs.goal = None;
        return Forage::None;
    }
    if animal.kind == Kind::Bear
        && (matches!(
            animal.behavior,
            Behavior::Hunt | Behavior::Attack | Behavior::Climb
        ) || animal.needs.goal.is_some_and(|g| g.kind == NeedKind::Tree))
    {
        return Forage::None;
    }
    if animal.behavior == Behavior::Flee {
        animal.needs.goal = None;
        return Forage::None;
    }
    if let Some(goal) = animal.needs.goal {
        let target = [
            goal.position[0] as f64 + 0.5,
            if goal.kind == NeedKind::Food {
                goal.position[1] as f64 + 1.0
            } else {
                water_surface(habitat, goal.position).unwrap_or(goal.position[1] as f64)
            },
            goal.position[2] as f64 + 0.5,
        ];
        let prop = (goal.kind == NeedKind::Food)
            .then(|| food_at(habitat, animal.kind, goal.position))
            .flatten();
        let available = match goal.kind {
            NeedKind::Water => water_at(habitat, goal.position),
            NeedKind::Food => {
                prop.is_some() || grazable_ground(animal.kind, habitat, goal.position)
            }
            NeedKind::Tree
            | NeedKind::Shelter
            | NeedKind::AttackAim
            | NeedKind::Territory
            | NeedKind::AirborneAttackAim => false,
        };
        if !available
            || animal.needs.search_in <= 0.0
            || (animal.position[1] - target[1]).abs() > 1.7
        {
            animal.needs.goal = None;
            animal.needs.search_in = 3.0;
            animal.timer = 0.0;
            return Forage::None;
        }
        let dx = animal.position[0] - target[0];
        let dz = animal.position[2] - target[2];
        let reach: f64 = if goal.kind == NeedKind::Water {
            (animal.kind.size()[0].max(animal.kind.size()[2]) * 0.5 + 0.8).max(1.9)
        } else {
            1.15
        };
        let action = if goal.kind == NeedKind::Water {
            Behavior::Drink
        } else {
            Behavior::Graze
        };
        if dx.hypot(dz) <= reach {
            if animal.behavior != action {
                animal.behavior = action;
                animal.timer = 2.4;
            } else if animal.timer <= 0.0 {
                animal.needs.goal = None;
                animal.needs.search_in = 5.0;
                animal.behavior = Behavior::Rest;
                animal.timer = 2.0;
                if goal.kind == NeedKind::Water {
                    animal.needs.water = (animal.needs.water + 8.0).min(animal.kind.water());
                } else if let Some(prop) = prop {
                    return Forage::Prop {
                        section: prop.section,
                        id: prop.id,
                    };
                } else if grazable_ground(animal.kind, habitat, goal.position) {
                    animal.needs.food = (animal.needs.food + 5.).min(animal.kind.food());
                }
            }
        } else {
            animal.behavior = if goal.kind == NeedKind::Water {
                Behavior::SeekWater
            } else {
                Behavior::SeekFood
            };
            animal.yaw = (dx.atan2(dz)) as f32;
            // Give local obstacle steering time to move around the blocking voxel.
            if animal.timer > 0.0 && animal.timer < 0.7 {
                animal.yaw = (animal.yaw + 1.1).rem_euclid(std::f32::consts::TAU);
            }
        }
        return Forage::None;
    }
    if search && animal.needs.search_in <= 0.0 {
        animal.needs.search_in = 4.0 + (animal.id % 40) as f32 * 0.1;
        let hungry = animal.kind != Kind::Bear && animal.needs.food < animal.kind.food() * 0.85;
        let thirsty = animal.needs.water < animal.kind.water() * 0.85;
        let goal = if thirsty {
            find_water(animal, habitat, 6)
                .or_else(|| hungry.then(|| find_food(animal, habitat)).flatten())
        } else if hungry {
            find_food(animal, habitat)
        } else if animal.needs.water < animal.kind.water() * 0.995 {
            find_water(animal, habitat, 2)
        } else {
            None
        };
        if let Some(goal) = goal {
            animal.needs.goal = Some(goal);
            animal.needs.search_in = 15.0;
            animal.timer = 3.0;
        }
    }
    Forage::None
}

pub fn apply_forage(
    effect: Forage,
    animal: &mut Creature,
    chunks: &mut ChunkStore,
    journal: &mut EditJournal,
    rules: terra_core::GameRules,
) -> Result<bool, String> {
    if !rules.mob_griefing {
        return Ok(false);
    }
    if let Forage::Pollinate { section, id } = effect {
        if animal.kind != Kind::Bee {
            return Ok(false);
        }
        let flower = chunks
            .loaded
            .get(&section)
            .and_then(|section| {
                section
                    .props
                    .iter()
                    .find(|p| p.id == id && p.kind == PropKind::Flower && !p.removed)
            })
            .copied();
        let Some(flower) = flower.filter(|p| {
            !journal.prop_removed(p.section, p.id)
                && crate::props::placement::supported(p, |p| chunks.block_at(p)) == Some(true)
                && distance(p.position, animal.position) < 2.25
        }) else {
            return Ok(false);
        };
        return crate::farming::pollinate_near(chunks, journal, flower.position);
    }
    let Forage::Prop { section, id } = effect else {
        return Ok(false);
    };
    let Some(prop) = chunks
        .loaded
        .get(&section)
        .and_then(|section| {
            section
                .props
                .iter()
                .find(|prop| prop.id == id && edible(animal.kind, prop))
        })
        .copied()
    else {
        return Ok(false);
    };
    let support = support(&prop);
    if prop.kind.crop().is_some() {
        let Some(mut state) = journal
            .crop_at(support)
            .copied()
            .filter(|state| state.mature())
        else {
            return Ok(false);
        };
        state.progress_seconds = 0.0;
        crate::farming::commit_crop_state(chunks, journal, support, state)?;
    } else {
        if !commit_prop_removal(chunks, journal, &PropRemoveRequest { section, id })? {
            return Ok(false);
        }
        let delay = 120.0 + (id.0 % 61) as f32;
        // Growing grass is tracked by the soil growth system, not the generic
        // forage timer (which intentionally rejects these transient props).
        if !prop.growing_grass()
            && let Err(error) = journal.set_forage_regrowth(crate::farming::ForageRegrowth {
                prop,
                remaining_seconds: delay,
            })
        {
            tracing::warn!(%error, "Cannot schedule grazed plant regrowth");
        }
    }
    animal.needs.food = (animal.needs.food + 6.0).min(animal.kind.food());
    if animal.needs.water > animal.kind.water() * 0.3 {
        animal.health = (animal.health + 0.5).min(animal.kind.health());
    }
    Ok(true)
}
