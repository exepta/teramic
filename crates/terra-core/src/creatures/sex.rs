//! The immutable spawn/birth identity is also the saved sex seed, including old worlds.
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CreatureSex {
    Male,
    Female,
}

impl CreatureSex {
    /// This hash is part of the persistent identity contract; never reroll it on reload.
    pub const fn from_spawn_id(id: u64) -> Self {
        let mut seed = id ^ 0x68d2_90ab_74c1_e53f;
        seed = (seed ^ (seed >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        seed = (seed ^ (seed >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        if (seed ^ (seed >> 31)) & 1 == 0 {
            Self::Male
        } else {
            Self::Female
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Male => "Maennlich",
            Self::Female => "Weiblich",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::creatures::{
        Creature, CreatureBehavior, CreatureKind, CreatureNeeds, CreaturePopulation, bee_id,
        breeding, creature_id, spider_id,
    };

    #[test]
    fn old_and_new_spawn_namespaces_have_stable_sex_at_negative_coordinates() {
        for home in [
            [-31_250, -31_250],
            [-17, -1],
            [-1, 0],
            [0, -1],
            [0, 0],
            [31_250, 31_250],
        ] {
            for slot in 0..4 {
                for (id, kind) in [
                    (creature_id(home, slot).unwrap(), CreatureKind::Cow),
                    (bee_id(home, slot).unwrap(), CreatureKind::Bee),
                    (spider_id(home, slot).unwrap(), CreatureKind::SmallSpider),
                ] {
                    let a = Creature {
                        id,
                        kind,
                        home,
                        position: [home[0] as f64 * 16.0, 3.0, home[1] as f64 * 16.0],
                        velocity: [0.0; 3],
                        yaw: 0.0,
                        health: kind.health(),
                        behavior: CreatureBehavior::Rest,
                        timer: 0.0,
                        age: 0.0,
                        variant: 0,
                        needs: CreatureNeeds::full(kind),
                    };
                    assert!(a.valid());
                    let restored: Creature =
                        serde_json::from_str(&serde_json::to_string(&a).unwrap()).unwrap();
                    assert_eq!(restored.sex(), a.sex());
                    assert_eq!(a.sex(), CreatureSex::from_spawn_id(id));
                }
            }
        }
        for (seed, expected) in [
            (0, CreatureSex::Male),
            (1, CreatureSex::Male),
            ((1 << 60) + 1, CreatureSex::Male),
            ((1 << 61) + 1, CreatureSex::Male),
            ((1 << 62) + 1, CreatureSex::Female),
            ((1 << 62) + 2, CreatureSex::Male),
            ((1 << 62) + 3, CreatureSex::Female),
            (1 << 63, CreatureSex::Male),
            (u64::MAX, CreatureSex::Male),
        ] {
            assert_eq!(
                CreatureSex::from_spawn_id(seed),
                expected,
                "saved seed {seed}"
            );
        }
        let id = breeding::next_birth_id(&CreaturePopulation::default(), 0).unwrap();
        assert_eq!(CreatureSex::from_spawn_id(id), CreatureSex::Male);
    }

    #[test]
    fn both_sexes_are_balanced_without_chunk_or_spawn_order_dependencies() {
        for namespace in [0, 1 << 60, 1 << 61, 1 << 62, 1 << 63] {
            let males = (1..=10_000)
                .filter(|id| CreatureSex::from_spawn_id(namespace + id) == CreatureSex::Male)
                .count();
            assert!(
                (4_600..=5_400).contains(&males),
                "namespace {namespace} males {males}"
            );
        }
    }

    #[test]
    fn movement_growth_and_variant_changes_never_change_sex() {
        let mut a = Creature {
            id: breeding::next_birth_id(&Default::default(), 0).unwrap(),
            kind: CreatureKind::Horse,
            home: [-1, 0],
            position: [-8.0, 3.0, 8.0],
            velocity: [0.0; 3],
            yaw: 0.0,
            health: CreatureKind::Horse.health(),
            behavior: CreatureBehavior::Rest,
            timer: 0.0,
            age: 0.0,
            variant: 0,
            needs: CreatureNeeds::full(CreatureKind::Horse),
        };
        let sex = a.sex();
        a.position = [64.0, -20.0, -128.0];
        a.age = breeding::GROWTH_SECONDS;
        a.variant = 15;
        assert_eq!(a.sex(), sex);
    }
}
