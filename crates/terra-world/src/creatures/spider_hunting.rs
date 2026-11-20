//! One bounded spatial index per server tick, shared by all hungry spiders.
use super::{Behavior, Creature, CreaturePopulation, Habitat, Kind, distance, spiders};
use std::collections::BTreeMap;
use terra_core::{
    GameplayState, WorldDrop,
    creatures::{NeedGoal, NeedKind, intentions::IntentReason, taming::food_gain},
};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Meal {
    Creature(u64),
    Drop(u64),
}
#[derive(Clone, Copy)]
struct Target {
    meal: Meal,
    position: [f64; 3],
    insect: bool,
}
#[derive(Default)]
pub struct FoodIndex(BTreeMap<[i64; 3], Vec<Target>>);
fn bin(p: [f64; 3]) -> [i64; 3] {
    p.map(|v| (v / 20.0).floor() as i64)
}
impl FoodIndex {
    pub fn new<'a>(
        animals: impl Iterator<Item = &'a Creature>,
        drops: impl Iterator<Item = &'a WorldDrop>,
    ) -> Self {
        let mut index = Self::default();
        for animal in animals.filter(|a| {
            a.health > 0.0
                && matches!(
                    a.kind,
                    Kind::Bee
                        | Kind::Butterfly
                        | Kind::Fish
                        | Kind::Cow
                        | Kind::Sheep
                        | Kind::Pig
                        | Kind::Horse
                )
        }) {
            let mut position = animal.position;
            position[1] += animal.size()[1] * 0.5;
            index.0.entry(bin(position)).or_default().push(Target {
                meal: Meal::Creature(animal.id),
                position,
                insect: matches!(animal.kind, Kind::Bee | Kind::Butterfly),
            });
        }
        for drop in drops.filter(|drop| food_gain(Kind::MediumSpider, drop.stack.item).is_some()) {
            index.0.entry(bin(drop.position)).or_default().push(Target {
                meal: Meal::Drop(drop.id),
                position: drop.position,
                insect: true,
            });
        }
        index
    }
    fn nearest(
        &self,
        a: &Creature,
        habitat: &Habitat,
        den: [f64; 3],
        search: bool,
    ) -> Option<Target> {
        let center = terra_core::creatures::spiders::visual_center(a);
        let prior = a
            .needs
            .goal
            .filter(|goal| goal.kind == NeedKind::Food)
            .map(|goal| goal.position.map(|v| v as f64 + 0.5));
        let probe = if search { center } else { prior? };
        let cell = bin(probe);
        let mut best = if search { 400.0 } else { 3.0_f64.powi(2) };
        let mut result = None;
        for dx in -1..=1 {
            for dy in -1..=1 {
                for dz in -1..=1 {
                    if let Some(targets) = self.0.get(&[cell[0] + dx, cell[1] + dy, cell[2] + dz]) {
                        for target in targets.iter().take(32) {
                            let d = distance(probe, target.position);
                            if d < best
                                && distance(center, target.position) <= 400.0
                                && distance(den, target.position) <= 400.0
                                && (a.kind != Kind::SmallSpider || target.insect)
                                && super::bears::visible(habitat, center, target.position)
                            {
                                result = Some(*target);
                                best = d;
                            }
                        }
                    }
                }
            }
        }
        result
    }
}

pub fn step(
    a: &mut Creature,
    habitat: &Habitat,
    index: &FoodIndex,
    den: [i64; 3],
    dt: f32,
    search: bool,
    navigation: &mut super::navigation::Navigation,
) -> Option<Option<Meal>> {
    step_with_access(
        a,
        habitat,
        (index, false, false),
        den,
        dt,
        search,
        navigation,
    )
}

pub fn step_owned(
    a: &mut Creature,
    habitat: &Habitat,
    index: &FoodIndex,
    den: [i64; 3],
    dt: f32,
    search: bool,
    navigation: &mut super::navigation::Navigation,
) -> Option<Option<Meal>> {
    step_with_access(
        a,
        habitat,
        (index, true, false),
        den,
        dt,
        search,
        navigation,
    )
}

pub fn cycle_step(
    a: &mut Creature,
    habitat: &Habitat,
    access: (&FoodIndex, bool),
    den: [i64; 3],
    dt: f32,
    search: bool,
    navigation: &mut super::navigation::Navigation,
) -> Option<Option<Meal>> {
    step_with_access(
        a,
        habitat,
        (access.0, access.1, true),
        den,
        dt,
        search,
        navigation,
    )
}

fn step_with_access(
    a: &mut Creature,
    habitat: &Habitat,
    access: (&FoodIndex, bool, bool),
    den: [i64; 3],
    dt: f32,
    search: bool,
    navigation: &mut super::navigation::Navigation,
) -> Option<Option<Meal>> {
    let (index, owned, cycle) = access;
    if !dt.is_finite()
        || dt <= 0.0
        || dt > 0.25
        || a.health <= 0.0
        || !a.kind.is_spider()
        || !cycle && a.needs.food > 40.0
        || a.behavior == Behavior::Descend
    {
        return None;
    }
    let target = index.nearest(a, habitat, den.map(|v| v as f64 + 0.5), search);
    let Some(target) = target else {
        if a.needs.goal.is_some_and(|goal| goal.kind == NeedKind::Food) {
            a.needs.goal = None;
            a.behavior = Behavior::Crawl;
        }
        return None;
    };
    a.age += f64::from(dt);
    a.timer = (a.timer - dt).max(0.0);
    a.needs.food = (a.needs.food - dt * 0.016).max(0.0);
    a.needs.water = 0.0;
    if a.needs.food == 0.0 {
        a.health = (a.health - dt * 0.025).max(0.0);
        if a.health == 0.0 {
            a.behavior = Behavior::Dead;
            return Some(None);
        }
    }
    if spiders::submersion_damage(a, habitat, dt) {
        return Some(None);
    }
    a.needs.search_in = (a.needs.search_in - dt).max(0.0);
    if cycle && search {
        a.needs.search_in = 1.0;
    }
    a.needs.goal = Some(NeedGoal {
        kind: NeedKind::Food,
        position: target.position.map(|v| v.floor() as i64),
    });
    navigation.intend(a, IntentReason::Food, Some(target.position));
    let reach = (1.0 + a.kind.size()[2] * 0.5) * a.growth_scale();
    if distance(
        terra_core::creatures::spiders::visual_center(a),
        target.position,
    ) <= reach.powi(2)
    {
        if !matches!(a.behavior, Behavior::Climb | Behavior::Ceiling) {
            a.behavior = Behavior::Attack;
        }
        a.velocity = [0.0; 3];
        return Some(if a.timer <= 0.0 {
            a.timer = terra_core::creatures::definitions::get(a.kind)
                .combat
                .map_or(1.0, |combat| combat.cooldown);
            Some(target.meal)
        } else {
            None
        });
    }
    spiders::hunt_surface(a, habitat, target.position, dt, search, navigation, owned);
    if !matches!(
        a.behavior,
        Behavior::Climb | Behavior::Ceiling | Behavior::Descend
    ) {
        a.behavior = Behavior::Hunt;
    }
    Some(None)
}

pub fn eat(
    population: &mut CreaturePopulation,
    gameplay: &mut GameplayState,
    habitat: &Habitat,
    id: u64,
    meal: Meal,
) -> bool {
    let Some(spider) = population.animals.get(&id) else {
        return false;
    };
    if !spider.kind.is_spider() || spider.health <= 0.0 {
        return false;
    }
    let center = terra_core::creatures::spiders::visual_center(spider);
    let reach = (1.0 + spider.kind.size()[2] * 0.5) * spider.growth_scale();
    let eaten = match meal {
        Meal::Creature(prey) => {
            if population.owners.contains_key(&prey) {
                return false;
            }
            let Some(target) = population.animals.get(&prey) else {
                return false;
            };
            let mut point = target.position;
            point[1] += target.size()[1] * 0.5;
            if target.health <= 0.0
                || !matches!(
                    target.kind,
                    Kind::Bee
                        | Kind::Butterfly
                        | Kind::Fish
                        | Kind::Cow
                        | Kind::Sheep
                        | Kind::Pig
                        | Kind::Horse
                )
                || spider.kind == Kind::SmallSpider
                    && !matches!(target.kind, Kind::Bee | Kind::Butterfly)
                || distance(center, point) > reach.powi(2)
                || !super::bears::visible(habitat, center, point)
            {
                return false;
            }
            let damage = terra_core::creatures::definitions::get(spider.kind)
                .combat
                .map_or(1.0, |combat| combat.damage)
                * spider.growth_scale().powi(2) as f32;
            let target = population.animals.get_mut(&prey).expect("validated prey");
            target.health = (target.health - damage).max(0.0);
            if target.health == 0.0 {
                target.behavior = Behavior::Dead;
                true
            } else {
                target.behavior = Behavior::Flee;
                target.timer = 3.0;
                false
            }
        }
        Meal::Drop(drop) => {
            let Some(index) = gameplay.drops.iter().position(|d| {
                d.id == drop
                    && food_gain(spider.kind, d.stack.item).is_some()
                    && distance(center, d.position) <= reach.powi(2)
                    && super::bears::visible(habitat, center, d.position)
            }) else {
                return false;
            };
            gameplay.drops[index].stack.count -= 1;
            if gameplay.drops[index].stack.count == 0 {
                gameplay.drops.remove(index);
            }
            true
        }
    };
    if eaten && let Some(spider) = population.animals.get_mut(&id) {
        spider.needs.food = (spider.needs.food + 15.0).min(spider.kind.food());
        spider.health = (spider.health + 1.0).min(spider.kind.health());
        spider.needs.goal = None;
        spider.needs.search_in = 2.0;
        gameplay.revision = gameplay.revision.wrapping_add(1);
    }
    eaten
}
