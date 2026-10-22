//! Command vocabulary and persistent identities, separate from natural habitat slots.
use super::{CreatureKind, CreaturePopulation, MAX_CREATURES};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SpawnMob {
    pub kind: CreatureKind,
    pub variant: u8,
}

pub const MOBS: &[(&str, SpawnMob)] = &[
    ("teramic::sheep", mob(CreatureKind::Sheep, 0)),
    ("teramic::cow", mob(CreatureKind::Cow, 0)),
    ("teramic::pig", mob(CreatureKind::Pig, 0)),
    ("teramic::fish", mob(CreatureKind::Fish, 2)),
    ("teramic::clown_fish", mob(CreatureKind::Fish, 0)),
    ("teramic::bee", mob(CreatureKind::Bee, 0)),
    ("teramic::bear", mob(CreatureKind::Bear, 0)),
    ("teramic::squid", mob(CreatureKind::Squid, 0)),
    ("teramic::moon_jelly", mob(CreatureKind::MoonJelly, 0)),
    ("teramic::fire_jelly", mob(CreatureKind::FireJelly, 0)),
    ("teramic::void_jelly", mob(CreatureKind::VoidJelly, 0)),
    ("teramic::sun_jelly", mob(CreatureKind::SunJelly, 0)),
    ("teramic::crystal_jelly", mob(CreatureKind::CrystalJelly, 0)),
    ("teramic::reef_shark", mob(CreatureKind::ReefShark, 0)),
    (
        "teramic::hammerhead_shark",
        mob(CreatureKind::HammerheadShark, 0),
    ),
    ("teramic::whale_shark", mob(CreatureKind::WhaleShark, 0)),
    ("teramic::horse", mob(CreatureKind::Horse, 0)),
];

const fn mob(kind: CreatureKind, variant: u8) -> SpawnMob {
    SpawnMob { kind, variant }
}

pub fn lookup(id: &str) -> Option<SpawnMob> {
    MOBS.iter()
        .find(|(name, _)| *name == id)
        .map(|(_, mob)| *mob)
}

impl SpawnMob {
    pub fn id(self) -> &'static str {
        MOBS.iter()
            .find(|(_, mob)| *mob == self)
            .map_or("unknown", |(name, _)| name)
    }
}

const COMMAND_NAMESPACE: u64 = 1 << 62;

pub fn is_command_id(id: u64) -> bool {
    (COMMAND_NAMESPACE + 1..=COMMAND_NAMESPACE + u64::from(u32::MAX)).contains(&id)
}

pub fn next_command_id(population: &CreaturePopulation) -> Option<u64> {
    if population.animals.len() >= MAX_CREATURES {
        return None;
    }
    let last = population
        .animals
        .range(COMMAND_NAMESPACE + 1..=COMMAND_NAMESPACE + u64::from(u32::MAX))
        .next_back()
        .map_or(COMMAND_NAMESPACE, |(id, _)| *id);
    let next = last.checked_add(1)?;
    is_command_id(next).then_some(next)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::creatures::{AnimalProduct, Creature, CreatureBehavior, CreatureNeeds};

    #[test]
    fn command_identities_survive_serialization_without_claiming_natural_habitats() {
        let mut population = CreaturePopulation::default();
        for (_, mob) in MOBS {
            let id = next_command_id(&population).unwrap();
            assert_ne!(Some(id), super::super::creature_id([0, 0], 0));
            assert_ne!(Some(id), super::super::bee_id([0, 0], 0));
            let animal = Creature {
                id,
                kind: mob.kind,
                home: [0, 0],
                position: [0.0, 3.0, 0.0],
                velocity: [0.0; 3],
                yaw: 0.0,
                health: mob.kind.health(),
                behavior: CreatureBehavior::Rest,
                timer: 0.0,
                age: 0.0,
                variant: mob.variant,
                needs: CreatureNeeds::full(mob.kind),
            };
            if mob.id() == "teramic::clown_fish" {
                assert_eq!(animal.product(), Some(AnimalProduct::Clownfish));
            }
            assert!(animal.valid());
            population.animals.insert(id, animal);
        }
        assert!(population.valid());
        assert!(population.explored.is_empty());
        let restored: CreaturePopulation =
            serde_json::from_str(&serde_json::to_string(&population).unwrap()).unwrap();
        assert_eq!(restored, population);
        assert_eq!(next_command_id(&restored), next_command_id(&population));
        assert!(
            !restored
                .animals
                .contains_key(&next_command_id(&restored).unwrap())
        );
        assert!(!is_command_id(COMMAND_NAMESPACE));
        assert!(!is_command_id(COMMAND_NAMESPACE + u64::from(u32::MAX) + 1));
        assert!(lookup("other::cow").is_none());
    }
}
