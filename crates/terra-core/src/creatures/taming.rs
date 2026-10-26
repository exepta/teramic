//! Persistent trust complements the existing UUID ownership map; spawn IDs never change.
use super::{AnimalProduct, Creature, CreatureKind as Kind, CreaturePopulation};
use crate::{CropItem, ItemKind, PlayerUuid, ResourcePropKind};
use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CompanionMode {
    #[default]
    Follow,
    Wait,
    /// Appended to preserve the historical Follow/Wait save discriminants.
    None,
}
impl CompanionMode {
    pub fn label(self) -> &'static str {
        match self {
            Self::Follow => "Folgt",
            Self::Wait => "Wartet",
            Self::None => "Frei",
        }
    }
}

/// Only this explicit command map authorizes movement; historical bond modes
/// remain readable but never silently reactivate following after an upgrade.
pub fn order(state: &crate::GameplayState, id: u64, kind: Kind) -> CompanionMode {
    state
        .pet_orders
        .get(&id)
        .copied()
        .unwrap_or(if kind.is_spider() {
            CompanionMode::None
        } else {
            CompanionMode::Wait
        })
}

pub fn command(
    state: &mut crate::GameplayState,
    id: u64,
    player: PlayerUuid,
    mode: CompanionMode,
) -> bool {
    if state.creatures.owners.get(&id) != Some(&player)
        || state
            .creatures
            .animals
            .get(&id)
            .is_none_or(|a| a.health <= 0.0 || !tameable(a.kind))
        || state.pet_orders.len() >= super::MAX_CREATURES && !state.pet_orders.contains_key(&id)
    {
        return false;
    }
    if state.pet_orders.insert(id, mode) != Some(mode) {
        state.revision = state.revision.wrapping_add(1);
    }
    true
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct AnimalBond {
    pub player: PlayerUuid,
    pub trust: u8,
    pub mode: CompanionMode,
    pub last_fed_age: Option<f64>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpiderDen {
    pub anchor: [i64; 3],
    pub web_id: u64,
}
impl SpiderDen {
    pub fn valid(self) -> bool {
        self.web_id != 0 && self.anchor.iter().all(|v| v.unsigned_abs() <= 999_000)
    }
}
impl AnimalBond {
    pub fn calmed(self, age: f64) -> bool {
        self.valid()
            && self.trust < 100
            && age.is_finite()
            && self
                .last_fed_age
                .is_some_and(|last| (0.0..8.0).contains(&(age - last)))
    }
    pub fn owned(player: PlayerUuid) -> Self {
        Self {
            player,
            trust: 100,
            mode: CompanionMode::Wait,
            last_fed_age: None,
        }
    }
    pub fn valid(self) -> bool {
        !self.player.is_nil()
            && (1..=100).contains(&self.trust)
            && (self.trust == 100 || self.mode == CompanionMode::Wait)
            && self.last_fed_age.is_none_or(|v| v.is_finite() && v >= 0.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct CompanionView {
    pub id: u64,
    pub owner: Option<PlayerUuid>,
    pub player: PlayerUuid,
    pub trust: u8,
    pub mode: CompanionMode,
}
impl CompanionView {
    pub fn valid(self) -> bool {
        self.id != 0
            && AnimalBond {
                player: self.player,
                trust: self.trust,
                mode: self.mode,
                last_fed_age: None,
            }
            .valid()
            && self
                .owner
                .is_none_or(|owner| !owner.is_nil() && owner == self.player)
            && (self.owner.is_some() == (self.trust == 100))
    }
}

pub fn tameable(kind: Kind) -> bool {
    matches!(kind, Kind::Sheep | Kind::Cow | Kind::Pig | Kind::Horse) || kind.is_spider()
}

/// This is also the HUD's source of truth for species-specific food.
pub fn food_gain(kind: Kind, item: ItemKind) -> Option<u8> {
    let accepted = match kind {
        Kind::Sheep | Kind::Cow => item == ItemKind::Crop(CropItem::Wheat),
        Kind::Horse => matches!(
            item,
            ItemKind::Crop(CropItem::Wheat | CropItem::Corn | CropItem::Lettuce)
        ),
        Kind::Pig => matches!(
            item,
            ItemKind::Crop(CropItem::Corn | CropItem::Lettuce)
                | ItemKind::Prop(ResourcePropKind::Mushroom)
        ),
        kind if kind.is_spider() => matches!(
            item,
            ItemKind::Animal(
                AnimalProduct::Beef
                    | AnimalProduct::Pork
                    | AnimalProduct::Mutton
                    | AnimalProduct::Fish
                    | AnimalProduct::Clownfish
            )
        ),
        _ => false,
    };
    accepted.then_some(if kind == Kind::Horse || kind.is_spider() {
        20
    } else {
        25
    })
}

/// Pure validation before authoritative inventory consumption; no partial mutation on rejection.
pub fn offered(
    animal: &Creature,
    population: &CreaturePopulation,
    previous: Option<AnimalBond>,
    player: PlayerUuid,
    food: ItemKind,
) -> Option<AnimalBond> {
    let gain = food_gain(animal.kind, food)?;
    if player.is_nil()
        || !animal.age.is_finite()
        || animal.age < 0.0
        || previous.is_some_and(|bond| !bond.valid())
        || animal.health <= 0.0
        || population
            .owners
            .get(&animal.id)
            .is_some_and(|owner| *owner != player)
    {
        return None;
    }
    let mut bond = previous
        .filter(|bond| bond.player == player)
        .unwrap_or(AnimalBond {
            player,
            trust: 0,
            mode: CompanionMode::Wait,
            last_fed_age: None,
        });
    if population.owners.get(&animal.id) == Some(&player) {
        bond.trust = 100;
    }
    if bond
        .last_fed_age
        .is_some_and(|last| animal.age - last < 1.25)
        || bond.trust == 100
            && animal.health >= animal.kind.health()
            && animal.needs.food >= animal.kind.food()
    {
        return None;
    }
    bond.trust = bond.trust.saturating_add(gain).min(100);
    if bond.trust == 100 && !population.owners.contains_key(&animal.id) {
        bond.mode = CompanionMode::Wait;
    }
    bond.last_fed_age = Some(animal.age);
    Some(bond)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::creatures::{CreatureBehavior, CreatureNeeds, spawning::next_command_id};

    pub fn animal(kind: Kind) -> Creature {
        Creature {
            id: next_command_id(&CreaturePopulation::default()).unwrap(),
            kind,
            home: [0, 0],
            position: [8.0, 3.0, 8.0],
            velocity: [0.0; 3],
            yaw: 0.0,
            health: kind.health(),
            behavior: CreatureBehavior::Rest,
            timer: 0.0,
            age: 0.0,
            variant: 0,
            needs: CreatureNeeds::full(kind),
        }
    }

    #[test]
    fn only_explicit_owner_commands_activate_follow_and_legacy_modes_remain_inactive() {
        let owner = PlayerUuid::from_u128(120);
        let mut state = crate::GameplayState::default();
        let animal = animal(Kind::Cow);
        let id = animal.id;
        state.creatures.animals.insert(id, animal);
        state.creatures.owners.insert(id, owner);
        state.animal_bonds.insert(
            id,
            AnimalBond {
                mode: CompanionMode::Follow,
                ..AnimalBond::owned(owner)
            },
        );
        assert_eq!(order(&state, id, Kind::Cow), CompanionMode::Wait);
        assert_eq!(order(&state, id, Kind::MediumSpider), CompanionMode::None);
        assert!(!command(
            &mut state,
            id,
            PlayerUuid::from_u128(121),
            CompanionMode::Follow
        ));
        assert!(state.pet_orders.is_empty());
        for mode in [
            CompanionMode::Follow,
            CompanionMode::Wait,
            CompanionMode::None,
        ] {
            assert!(command(&mut state, id, owner, mode));
            assert_eq!(order(&state, id, Kind::Cow), mode);
            assert!(state.valid());
        }
        assert_eq!(
            state.animal_bonds[&id].mode,
            CompanionMode::Follow,
            "the historical field is not repurposed"
        );
        let saved = serde_json::to_value(&state).unwrap();
        let decoded: crate::GameplayState = serde_json::from_value(saved.clone()).unwrap();
        assert_eq!(decoded.pet_orders, state.pet_orders);
        let mut legacy = saved;
        legacy.as_object_mut().unwrap().remove("pet_orders");
        let legacy: crate::GameplayState = serde_json::from_value(legacy).unwrap();
        assert!(legacy.pet_orders.is_empty());
        assert_eq!(order(&legacy, id, Kind::Cow), CompanionMode::Wait);
        state.creatures.animals.get_mut(&id).unwrap().health = 0.0;
        assert!(!command(&mut state, id, owner, CompanionMode::Follow));
        state.creatures.owners.remove(&id);
        assert!(!state.valid(), "orphaned commands fail closed");
    }

    #[test]
    fn every_requested_species_accepts_only_its_food_and_builds_persistent_uuid_trust() {
        let player = PlayerUuid::new_v4();
        for (kind, food) in [
            (Kind::Sheep, ItemKind::Crop(CropItem::Wheat)),
            (Kind::Cow, ItemKind::Crop(CropItem::Wheat)),
            (Kind::Pig, ItemKind::Crop(CropItem::Corn)),
            (Kind::Horse, ItemKind::Crop(CropItem::Lettuce)),
            (Kind::SmallSpider, ItemKind::Animal(AnimalProduct::Fish)),
            (Kind::MediumSpider, ItemKind::Animal(AnimalProduct::Pork)),
            (Kind::LargeSpider, ItemKind::Animal(AnimalProduct::Beef)),
        ] {
            let mut a = animal(kind);
            let mut population = CreaturePopulation::default();
            population.animals.insert(a.id, a.clone());
            assert!(offered(&a, &population, None, player, ItemKind::Stick).is_none());
            let mut previous = None;
            for _ in 0..5 {
                let bond = offered(&a, &population, previous, player, food).unwrap();
                assert!(bond.valid());
                assert!(offered(&a, &population, Some(bond), player, food).is_none());
                previous = Some(bond);
                a.age += 1.25;
                if bond.trust == 100 {
                    break;
                }
            }
            let bond = previous.unwrap();
            assert_eq!(bond.trust, 100, "{kind:?}");
            assert_eq!(bond.mode, CompanionMode::Wait);
            population.owners.insert(a.id, player);
            assert_eq!(population.natural_count(), 0);
            assert!(offered(&a, &population, Some(bond), PlayerUuid::new_v4(), food).is_none());
            let view = CompanionView {
                id: a.id,
                owner: Some(player),
                player,
                trust: bond.trust,
                mode: bond.mode,
            };
            assert!(view.valid());
            assert_eq!(
                serde_json::from_str::<AnimalBond>(&serde_json::to_string(&bond).unwrap()).unwrap(),
                bond
            );
        }
    }

    #[test]
    fn malformed_or_owner_inconsistent_bonds_fail_closed() {
        let a = animal(Kind::Cow);
        let player = PlayerUuid::new_v4();
        let mut state = crate::GameplayState::default();
        state.creatures.animals.insert(a.id, a.clone());
        let mut bond = AnimalBond {
            player,
            trust: 25,
            mode: CompanionMode::Wait,
            last_fed_age: Some(0.0),
        };
        state.animal_bonds.insert(a.id, bond);
        assert!(state.valid());
        bond.trust = 100;
        state.animal_bonds.insert(a.id, bond);
        assert!(!state.valid());
        state.creatures.owners.insert(a.id, player);
        assert!(state.valid());
        state.animal_bonds.get_mut(&a.id).unwrap().last_fed_age = Some(f64::NAN);
        assert!(!state.valid());
        assert!(
            !CompanionView {
                id: a.id,
                owner: Some(PlayerUuid::new_v4()),
                player,
                trust: 100,
                mode: CompanionMode::Follow
            }
            .valid()
        );
    }

    #[test]
    fn accepted_food_calms_partial_trust_for_eight_simulated_seconds_and_roundtrips() {
        let bond = AnimalBond {
            player: PlayerUuid::new_v4(),
            trust: 20,
            mode: CompanionMode::Wait,
            last_fed_age: Some(3.0),
        };
        assert!(bond.calmed(3.0));
        assert!(bond.calmed(10.99));
        assert!(!bond.calmed(11.0));
        assert!(!bond.calmed(2.99));
        let saved: AnimalBond =
            serde_json::from_str(&serde_json::to_string(&bond).unwrap()).unwrap();
        assert!(saved.calmed(7.0));
        assert!(!AnimalBond { trust: 100, ..bond }.calmed(7.0));
    }
}
