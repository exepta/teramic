//! Creation receipts, never a population target. Missing or dead starters are not replenished.
use super::{KingdomError, KingdomId, KingdomRegistry, KingdomState, MAX_KINGDOMS, SettlementKind};
use crate::{
    GameplayState, PlayerUuid,
    creatures::{
        Creature, CreatureBehavior, CreatureKind, CreatureNeeds, CreaturePopulation, MAX_CREATURES,
        soldiers::SoldierLoadout, villagers,
    },
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const STARTER_COUNT: usize = 18;
pub const STARTER_ROLES: [u8; STARTER_COUNT] = [
    SoldierLoadout::Sword.role(),
    SoldierLoadout::Sword.role(),
    SoldierLoadout::Sword.role(),
    SoldierLoadout::Bow.role(),
    SoldierLoadout::Bow.role(),
    SoldierLoadout::Spear.role(),
    SoldierLoadout::Spear.role(),
    SoldierLoadout::Spear.role(),
    villagers::UNEMPLOYED,
    villagers::UNEMPLOYED,
    villagers::UNEMPLOYED,
    villagers::UNEMPLOYED,
    villagers::UNEMPLOYED,
    villagers::UNEMPLOYED,
    villagers::UNEMPLOYED,
    villagers::UNEMPLOYED,
    villagers::UNEMPLOYED,
    villagers::UNEMPLOYED,
];
const NAMESPACE: u64 = 1 << 57;

#[cfg(test)]
#[path = "starter_tests.rs"]
mod tests;

/// A kingdom ID is never reused, including dissolved kingdoms. Reserve 32 identities
/// per kingdom without touching natural residents, command spawns or animal births.
pub fn starter_creature_id(kingdom: KingdomId, slot: usize) -> Option<u64> {
    let serial = kingdom
        .0
        .checked_mul(32)?
        .checked_add(u64::try_from(slot).ok()?)?;
    (kingdom.0 != 0 && slot < STARTER_COUNT && serial < NAMESPACE).then_some(NAMESPACE | serial)
}

pub fn is_starter_id(id: u64) -> bool {
    id >> 57 == 1 && (id & (NAMESPACE - 1)) / 32 != 0 && id % 32 < STARTER_COUNT as u64
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KingdomStarterGrant {
    pub founder: PlayerUuid,
    pub ids: [u64; STARTER_COUNT],
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct KingdomStarterRegistry {
    pub grants: BTreeMap<KingdomId, KingdomStarterGrant>,
}

impl KingdomStarterRegistry {
    pub fn valid(&self) -> bool {
        self.grants.len() <= MAX_KINGDOMS
            && self.grants.iter().all(|(kingdom, grant)| {
                !grant.founder.is_nil()
                    && grant
                        .ids
                        .iter()
                        .enumerate()
                        .all(|(slot, id)| starter_creature_id(*kingdom, slot) == Some(*id))
            })
    }

    pub fn valid_for(&self, kingdoms: &KingdomRegistry, population: &CreaturePopulation) -> bool {
        self.valid()
            && self.grants.iter().all(|(id, grant)| {
                kingdoms.get(*id).is_some_and(|kingdom| {
                    kingdom.founder == grant.founder && kingdom.kind == SettlementKind::Kingdom
                })
            })
            && population
                .animals
                .values()
                .filter(|creature| is_starter_id(creature.id))
                .all(|creature| {
                    let kingdom = KingdomId((creature.id & (NAMESPACE - 1)) / 32);
                    self.grants.get(&kingdom).is_some_and(|grant| {
                        creature.kind == CreatureKind::Villager
                            && grant.ids.contains(&creature.id)
                            && population.owners.get(&creature.id) == Some(&grant.founder)
                    })
                })
    }
}

impl GameplayState {
    /// Authority commits only a newly founded kingdom and its fully preflighted cohort.
    /// Inventories are unchanged; the already placed Bergfried paid its normal block cost.
    pub fn commit_founding_with_starters(
        &mut self,
        next: KingdomRegistry,
        id: KingdomId,
        positions: [[f64; 3]; STARTER_COUNT],
    ) -> Result<(), KingdomError> {
        let kingdom = next.get(id).ok_or(KingdomError::UnknownKingdom)?;
        if self.kingdoms.get(id).is_some() || self.kingdom_starters.grants.contains_key(&id) {
            return Err(KingdomError::KeepAlreadyBound);
        }
        if !next.valid()
            || kingdom.kind != SettlementKind::Kingdom
            || kingdom.state != KingdomState::Active
            || next.kingdoms.len() != self.kingdoms.kingdoms.len() + 1
            || next.revision
                != self
                    .kingdoms
                    .revision
                    .checked_add(1)
                    .ok_or(KingdomError::CapacityReached)?
            || !self
                .kingdom_starters
                .valid_for(&self.kingdoms, &self.creatures)
        {
            return Err(KingdomError::InvalidIdentity);
        }
        let revision = self
            .revision
            .checked_add(1)
            .ok_or(KingdomError::CapacityReached)?;
        if self
            .creatures
            .animals
            .len()
            .checked_add(STARTER_COUNT)
            .is_none_or(|n| n > MAX_CREATURES)
            || self.kingdom_starters.grants.len() >= MAX_KINGDOMS
        {
            return Err(KingdomError::CapacityReached);
        }
        let home = super::claim_chunk(kingdom.location);
        let mut ids = [0; STARTER_COUNT];
        let mut creatures = Vec::with_capacity(STARTER_COUNT);
        for (slot, position) in positions.into_iter().enumerate() {
            let creature_id = starter_creature_id(id, slot).ok_or(KingdomError::CapacityReached)?;
            if self.creatures.animals.contains_key(&creature_id)
                || self.creatures.owners.contains_key(&creature_id)
            {
                return Err(KingdomError::CapacityReached);
            }
            if position
                .iter()
                .any(|v| !v.is_finite() || v.abs() > 999_000.)
                || super::claim_chunk(position.map(|v| v.floor() as i64)) != home
                || positions[..slot].iter().any(|other| {
                    (position[0] - other[0]).hypot(position[2] - other[2]) < 0.9
                        && (position[1] - other[1]).abs() < CreatureKind::Villager.size()[1]
                })
            {
                return Err(KingdomError::StarterAreaUnavailable);
            }
            let creature = Creature {
                id: creature_id,
                kind: CreatureKind::Villager,
                home,
                position,
                velocity: [0.; 3],
                yaw: 0.,
                health: CreatureKind::Villager.health(),
                behavior: CreatureBehavior::Rest,
                timer: 0.,
                age: 0.,
                variant: STARTER_ROLES[slot],
                needs: CreatureNeeds::full(CreatureKind::Villager),
            };
            if !creature.valid() {
                return Err(KingdomError::InvalidLocation);
            }
            ids[slot] = creature_id;
            creatures.push(creature);
        }
        let grant = KingdomStarterGrant {
            founder: kingdom.founder,
            ids,
        };
        for creature in creatures {
            self.creatures.owners.insert(creature.id, grant.founder);
            self.creatures.animals.insert(creature.id, creature);
        }
        self.kingdom_starters.grants.insert(id, grant);
        self.kingdoms = next;
        self.revision = revision;
        Ok(())
    }
}
