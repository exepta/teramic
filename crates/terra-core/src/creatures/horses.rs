//! Spawn-fixed horse traits derived from the persistent identity, never rerolled on reload.
use super::{Creature, CreatureKind};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HorseTraits {
    pub stamina: u16,
    pub jump_height: f32,
}

impl Creature {
    pub fn horse_traits(&self) -> Option<HorseTraits> {
        if self.kind != CreatureKind::Horse {
            return None;
        }
        let mut hash = self.id.wrapping_mul(0x9e37_79b9_7f4a_7c15);
        hash = (hash ^ (hash >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        hash = (hash ^ (hash >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        hash ^= hash >> 31;
        Some(HorseTraits {
            stamina: 65 + (hash % 136) as u16,
            jump_height: 1.1 + ((hash >> 32) % 1151) as f32 * 0.001,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::creatures::{CreatureBehavior, CreatureNeeds};
    #[test]
    fn every_coat_and_random_trait_survives_save_reload_without_rerolling() {
        let mut capacities = std::collections::BTreeSet::new();
        for id in 1..=512 {
            let animal = Creature {
                id: (1 << 62) + id,
                kind: CreatureKind::Horse,
                home: [0, 0],
                position: [0.0, 1.0, 0.0],
                velocity: [0.0; 3],
                yaw: 0.0,
                health: 20.0,
                behavior: CreatureBehavior::Rest,
                timer: 0.0,
                age: 0.0,
                variant: (id % 16) as u8,
                needs: CreatureNeeds::full(CreatureKind::Horse),
            };
            assert!(animal.valid());
            let traits = animal.horse_traits().unwrap();
            assert!((65..=200).contains(&traits.stamina));
            assert!((1.1..=2.25).contains(&traits.jump_height));
            capacities.insert(traits.stamina);
            let restored: Creature =
                serde_json::from_str(&serde_json::to_string(&animal).unwrap()).unwrap();
            assert_eq!(traits, restored.horse_traits().unwrap());
            assert_eq!(animal.kind.food(), 30.0);
            assert_eq!(animal.kind.water(), 25.0);
        }
        assert!(capacities.len() > 100);
    }
}
