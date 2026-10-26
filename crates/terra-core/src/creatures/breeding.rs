//! Bounded, persistent breeding state; existing natural/command animals remain adults.
use super::{Creature, CreatureKind, CreaturePopulation, MAX_CREATURES, taming};
use crate::{GameplayState, ItemKind, PlayerUuid};
use serde::{Deserialize, Serialize};

pub const GROWTH_SECONDS: f64 = 1_200.0;
pub const COOLDOWN_SECONDS: f64 = 300.0;
pub const WILLING_SECONDS: f64 = 60.0;
pub const OWNER_LIMIT: usize = 128;
pub const LOCAL_LIMIT: usize = 32;
pub const LOCAL_RADIUS: f64 = 32.0;
pub const PARTNER_RADIUS: f64 = 5.0;
const BIRTH_NAMESPACE: u64 = 1 << 60;

#[derive(Debug, Default, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct AnimalFamily {
    pub parents: Option<[u64; 2]>,
    /// Feeding time continues even when collision pauses biological growth.
    pub active_age: f64,
    /// Timers use the creature's active simulation age, including after reload.
    pub ready_age: f64,
    pub willing_until_age: f64,
}
impl AnimalFamily {
    pub fn valid(self, id: u64) -> bool {
        [self.active_age, self.ready_age, self.willing_until_age]
            .iter()
            .all(|v| v.is_finite() && (0.0..=1.0e12).contains(v))
            && self.parents.is_none_or(|[a, b]| {
                a != 0
                    && b != 0
                    && a != b
                    && a != id
                    && b != id
                    && (!is_birth_id(a) || a < id)
                    && (!is_birth_id(b) || b < id)
            })
    }
    pub fn willing(self, animal: &Creature) -> bool {
        !animal.juvenile()
            && animal.health >= animal.kind.health() * 0.5
            && animal.needs.food >= animal.kind.food() * 0.5
            && self.ready_age <= animal.age
            && self.willing_until_age > animal.age
    }
    pub fn after_birth(&mut self, animal: &Creature) {
        self.willing_until_age = 0.0;
        self.ready_age = animal.age + COOLDOWN_SECONDS;
    }
    pub fn advance_active_age(&mut self, previous_age: f64, elapsed: f64) {
        if previous_age.is_finite() && elapsed.is_finite() && (0.0..=0.25).contains(&elapsed) {
            self.active_age = self.active_age.max(previous_age) + elapsed;
        }
    }
}

pub fn feeding_age(animal: &Creature, family: Option<&AnimalFamily>) -> f64 {
    if is_birth_id(animal.id) {
        family.map_or(animal.age, |family| animal.age.max(family.active_age))
    } else {
        animal.age
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct FamilyView {
    pub id: u64,
    pub parents: Option<[u64; 2]>,
    pub growth: f32,
    pub cooldown_seconds: f32,
    pub willing: bool,
}
impl FamilyView {
    pub fn new(animal: &Creature, family: AnimalFamily) -> Self {
        Self {
            id: animal.id,
            parents: family.parents,
            growth: animal.growth(),
            cooldown_seconds: (family.ready_age - animal.age).clamp(0.0, COOLDOWN_SECONDS) as f32,
            willing: family.willing(animal),
        }
    }
    pub fn valid(self) -> bool {
        self.id != 0
            && self.growth.is_finite()
            && (0.0..=1.0).contains(&self.growth)
            && self.cooldown_seconds.is_finite()
            && (0.0..=COOLDOWN_SECONDS as f32).contains(&self.cooldown_seconds)
            && AnimalFamily {
                parents: self.parents,
                ..Default::default()
            }
            .valid(self.id)
            && (!self.willing || self.growth == 1.0 && self.cooldown_seconds == 0.0)
    }
}

pub fn is_birth_id(id: u64) -> bool {
    (BIRTH_NAMESPACE + 1..=BIRTH_NAMESPACE + u64::from(u32::MAX)).contains(&id)
}
pub fn birth_serial(id: u64) -> Option<u32> {
    is_birth_id(id).then(|| (id - BIRTH_NAMESPACE) as u32)
}
pub fn next_birth_id(population: &CreaturePopulation, serial: u32) -> Option<u64> {
    if population.animals.len() >= MAX_CREATURES {
        return None;
    }
    let last = population
        .animals
        .range(BIRTH_NAMESPACE + 1..=BIRTH_NAMESPACE + u64::from(u32::MAX))
        .next_back()
        .map_or(BIRTH_NAMESPACE, |(id, _)| *id)
        .max(BIRTH_NAMESPACE + u64::from(serial));
    let next = last.checked_add(1)?;
    is_birth_id(next).then_some(next)
}

/// An extra feed, after ownership was established, explicitly enables one mating attempt.
pub fn can_offer(
    state: &GameplayState,
    animal: &Creature,
    player: PlayerUuid,
    food: ItemKind,
) -> bool {
    !player.is_nil()
        && state.creatures.owners.get(&animal.id) == Some(&player)
        && taming::food_gain(animal.kind, food).is_some()
        && !animal.juvenile()
        && animal.health >= animal.kind.health() * 0.5
        && animal.needs.food >= animal.kind.food() * 0.5
        && !matches!(
            animal.behavior,
            super::CreatureBehavior::Dead | super::CreatureBehavior::Sleep
        )
        && state.animal_families.get(&animal.id).is_none_or(|family| {
            family.ready_age <= animal.age && family.willing_until_age <= animal.age
        })
        && state.animal_bonds.get(&animal.id).is_none_or(|bond| {
            bond.last_fed_age.is_none_or(|age| {
                feeding_age(animal, state.animal_families.get(&animal.id)) - age >= 1.25
            })
        })
}

pub fn species(kind: CreatureKind) -> bool {
    taming::tameable(kind) || kind == CreatureKind::Bee
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::creatures::{CreatureBehavior, CreatureNeeds, creature_id};

    pub(super) fn animal() -> Creature {
        Creature {
            id: creature_id([-1, 2], 0).unwrap(),
            kind: CreatureKind::Cow,
            home: [-1, 2],
            position: [-8.0, 3.0, 40.0],
            velocity: [0.0; 3],
            yaw: 0.0,
            health: CreatureKind::Cow.health(),
            behavior: CreatureBehavior::Rest,
            timer: 0.0,
            age: 0.0,
            variant: 0,
            needs: CreatureNeeds::full(CreatureKind::Cow),
        }
    }

    #[test]
    fn new_namespace_grows_without_reclassifying_existing_age_zero_animals() {
        let mut a = animal();
        let adult_size = a.size();
        assert!(!a.juvenile());
        a.id = next_birth_id(&CreaturePopulation::default(), 0).unwrap();
        assert!(a.valid());
        assert!(a.juvenile());
        assert_eq!(a.growth(), 0.0);
        assert_eq!(a.size(), adult_size.map(|v| v * 0.45));
        a.age = GROWTH_SECONDS * 0.5;
        assert_eq!(a.growth(), 0.5);
        assert!(a.size()[1] > adult_size[1] * 0.45 && a.size()[1] < adult_size[1]);
        a.age = GROWTH_SECONDS;
        assert!(!a.juvenile());
        assert_eq!(a.size(), adult_size);
        assert_eq!(a.product(), Some(super::super::AnimalProduct::Beef));
        assert!(!is_birth_id(
            super::super::spawning::next_command_id(&CreaturePopulation::default()).unwrap()
        ));
    }

    #[test]
    fn only_extra_owner_feeds_make_mature_healthy_animals_willing() {
        let player = PlayerUuid::new_v4();
        let other = PlayerUuid::new_v4();
        let mut state = GameplayState::default();
        let mut a = animal();
        let food = ItemKind::Crop(crate::CropItem::Wheat);
        state.creatures.animals.insert(a.id, a.clone());
        assert!(!can_offer(&state, &a, player, food));
        state.creatures.owners.insert(a.id, player);
        assert!(can_offer(&state, &a, player, food));
        assert!(!can_offer(&state, &a, other, food));
        assert!(!can_offer(&state, &a, player, ItemKind::Stick));
        let mut family = AnimalFamily {
            willing_until_age: WILLING_SECONDS,
            ..Default::default()
        };
        state.animal_families.insert(a.id, family);
        assert!(family.willing(&a));
        assert!(!can_offer(&state, &a, player, food));
        family.after_birth(&a);
        state.animal_families.insert(a.id, family);
        assert!(!family.willing(&a));
        assert!(!can_offer(&state, &a, player, food));
        a.age = COOLDOWN_SECONDS;
        assert!(can_offer(&state, &a, player, food));
        a.id = BIRTH_NAMESPACE + 1;
        a.age = 1.0;
        state.creatures.owners.insert(a.id, player);
        assert!(!can_offer(&state, &a, player, food));
    }

    #[test]
    fn family_metadata_and_replica_reject_corrupt_timers_and_parent_cycles() {
        let a = animal();
        assert!(
            !AnimalFamily {
                ready_age: f64::NAN,
                ..Default::default()
            }
            .valid(a.id)
        );
        assert!(
            !AnimalFamily {
                parents: Some([a.id, 5]),
                ..Default::default()
            }
            .valid(a.id)
        );
        let view = FamilyView::new(&a, AnimalFamily::default());
        assert!(view.valid());
        assert!(
            !AnimalFamily {
                active_age: f64::NAN,
                ..Default::default()
            }
            .valid(a.id)
        );
        assert!(
            !FamilyView {
                growth: f32::NAN,
                ..view
            }
            .valid()
        );
        assert!(
            !FamilyView {
                cooldown_seconds: -1.0,
                ..view
            }
            .valid()
        );
    }

    #[test]
    fn paused_growth_does_not_freeze_feeding_cooldown_before_or_after_adulthood() {
        let player = PlayerUuid::new_v4();
        let mut state = GameplayState::default();
        let mut a = animal();
        a.id = next_birth_id(&state.creatures, 0).unwrap();
        a.needs.food -= 2.0;
        state.creatures.owners.insert(a.id, player);
        let mut family = AnimalFamily {
            parents: Some([4, 5]),
            ready_age: GROWTH_SECONDS,
            ..Default::default()
        };
        let mut bond = taming::AnimalBond::owned(player);
        bond.last_fed_age = Some(0.0);
        for _ in 0..6 {
            family.advance_active_age(0.0, 0.25);
        }
        assert_eq!(a.age, 0.0);
        assert_eq!(feeding_age(&a, Some(&family)), 1.5);
        let food = ItemKind::Crop(crate::CropItem::Wheat);
        let mut offered = a.clone();
        offered.age = feeding_age(&a, Some(&family));
        let fed = taming::offered(&offered, &state.creatures, Some(bond), player, food).unwrap();
        assert!(taming::offered(&offered, &state.creatures, Some(fed), player, food).is_none());
        assert_eq!(fed.trust, 100);
        assert_eq!(fed.player, player);
        a.age = GROWTH_SECONDS;
        family.active_age = GROWTH_SECONDS + 2.0;
        let mut fed = fed;
        fed.last_fed_age = Some(GROWTH_SECONDS + 0.5);
        state.animal_families.insert(a.id, family);
        state.animal_bonds.insert(a.id, fed);
        assert!(can_offer(&state, &a, player, food));
        offered.age = feeding_age(&a, Some(&family));
        assert!(taming::offered(&offered, &state.creatures, Some(fed), player, food).is_some());
        family.active_age = GROWTH_SECONDS + 1.0;
        state.animal_families.insert(a.id, family);
        assert!(!can_offer(&state, &a, player, food));
    }
}
