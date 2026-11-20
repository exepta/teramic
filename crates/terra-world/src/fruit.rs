//! Deterministic fruit identities and pure harvest transactions, shared by replicas.
use crate::props::PropId;
#[cfg(any(test, feature = "bevy"))]
use terra_core::TreeStage;
use terra_core::{FruitPhase, TreeRecord, TreeSpecies};
#[cfg(feature = "bevy")]
mod runtime;
#[cfg(feature = "bevy")]
pub use runtime::FruitProps;
#[cfg(feature = "bevy")]
pub(crate) use runtime::tick;

/// Calendar-derived stages; picked slots never refill inside the same ripe cycle.
pub fn advance(
    tree: &mut TreeRecord,
    clock: terra_core::WorldClock,
    seasons: terra_core::SeasonConfig,
) -> bool {
    use terra_core::Season;
    if !tree.stage.is_mature() || tree.species != TreeSpecies::Apple || tree.revision == u64::MAX {
        return false;
    }
    let Some(mut fruit) = tree.fruit else {
        return false;
    };
    if fruit.revision == u64::MAX || fruit.last_harvest_day.is_some_and(|day| clock.day < day) {
        return false;
    }
    let sample = seasons.sample(clock);
    let (phase, cycle) = if seasons.enabled {
        let phase = match sample.season {
            Season::Spring => FruitPhase::Blossom,
            Season::Summer if sample.progress < 0.4 => FruitPhase::Unripe,
            Season::Summer | Season::Autumn => FruitPhase::Ripe,
            Season::Winter => FruitPhase::Dormant,
        };
        (
            phase,
            sample
                .year
                .saturating_mul(seasons.lengths().into_iter().map(u64::from).sum()),
        )
    } else {
        (
            match clock.day % 4 {
                0 => FruitPhase::Blossom,
                1 => FruitPhase::Unripe,
                _ => FruitPhase::Ripe,
            },
            clock.day / 4 * 4,
        )
    };
    let before = fruit;
    if fruit.phase != phase || fruit.cycle_day != cycle {
        fruit.phase = phase;
        fruit.cycle_day = cycle;
        fruit.picked = 0;
        fruit.remaining = if phase == FruitPhase::Ripe {
            FRUIT_SLOTS
        } else {
            0
        };
    }
    if fruit == before {
        return false;
    }
    fruit.revision += 1;
    tree.fruit = Some(fruit);
    tree.revision += 1;
    true
}

pub const FRUIT_SLOTS: u8 = 8;
fn mix(mut v: u64) -> u64 {
    v = (v ^ (v >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    v = (v ^ (v >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    v ^ (v >> 31)
}
pub fn generation_seed(world_seed: u64, root: [i64; 3]) -> u64 {
    mix(world_seed
        ^ (root[0].div_euclid(16) as u64).wrapping_mul(0x9e37_79b9_7f4a_7c15)
        ^ (root[2].div_euclid(16) as u64).wrapping_mul(0xbf58_476d_1ce4_e5b9))
}
pub fn natural_id(world_seed: u64, root: [i64; 3]) -> u64 {
    mix(generation_seed(world_seed, root)
        ^ (root[1] as u64).rotate_left(17)
        ^ 0x4150_504c_4554_5245)
        | 1 << 63
}
pub fn prop_id(tree: u64, slot: u8) -> PropId {
    let mut hash = tree ^ 0x4150_504c_455f_4652;
    hash = (hash ^ (hash >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    hash = (hash ^ (hash >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    PropId(((hash ^ (hash >> 31)) | 1 << 63) & !7 | u64::from(slot & 7))
}
pub fn slot(tree: u64, id: PropId) -> Option<u8> {
    (0..FRUIT_SLOTS).find(|&slot| prop_id(tree, slot) == id)
}

pub fn harvest_state(
    tree: &TreeRecord,
    revision: u64,
    slot: u8,
    day: u64,
) -> Result<TreeRecord, String> {
    let mut next = tree.clone();
    let fruit = next.fruit.as_mut().ok_or("Tree has no fruit")?;
    if !next.stage.is_mature()
        || next.species != TreeSpecies::Apple
        || slot >= FRUIT_SLOTS
        || fruit.phase != FruitPhase::Ripe
        || fruit.revision != revision
        || fruit.remaining == 0
        || fruit.picked & (1 << slot) != 0
        || fruit.remaining != FRUIT_SLOTS - fruit.picked.count_ones() as u8
    {
        return Err("Fruit state changed".into());
    }
    fruit.revision = fruit
        .revision
        .checked_add(1)
        .ok_or("Fruit revision exhausted")?;
    next.revision = next
        .revision
        .checked_add(1)
        .ok_or("Tree revision exhausted")?;
    fruit.remaining -= 1;
    fruit.picked |= 1 << slot;
    fruit.last_harvest_day = Some(day);
    Ok(next)
}

#[cfg(test)]
mod tests {
    use super::*;
    use terra_core::{FruitState, SeasonConfig, WorldClock};
    pub(super) fn tree() -> TreeRecord {
        TreeRecord {
            id: 17,
            root: [8, 1, 8],
            species: TreeSpecies::Apple,
            seed: 17,
            planted_by: None,
            stage: TreeStage::Mature,
            growth_seconds: 0.,
            bounds: None,
            wood: vec![],
            leaves: vec![],
            fruit: Some(FruitState::new(0)),
            revision: 0,
        }
    }
    fn day(day: u64) -> WorldClock {
        WorldClock {
            day,
            ..Default::default()
        }
    }
    #[test]
    fn harvest_is_per_slot_revision_checked_and_never_refills_during_the_same_season() {
        let mut tree = tree();
        let seasons = SeasonConfig::default();
        assert!(advance(&mut tree, day(0), seasons));
        assert_eq!(tree.fruit.unwrap().phase, FruitPhase::Blossom);
        assert!(advance(&mut tree, day(7), seasons));
        assert_eq!(tree.fruit.unwrap().phase, FruitPhase::Unripe);
        assert!(harvest_state(&tree, tree.fruit.unwrap().revision, 0, 7).is_err());
        assert!(advance(&mut tree, day(11), seasons));
        let before = tree.clone();
        tree = harvest_state(&tree, tree.fruit.unwrap().revision, 3, 11).unwrap();
        assert_eq!(tree.fruit.unwrap().remaining, 7);
        assert_eq!(tree.fruit.unwrap().picked, 8);
        assert!(tree.valid());
        assert!(harvest_state(&tree, before.fruit.unwrap().revision, 2, 11).is_err());
        assert!(harvest_state(&tree, tree.fruit.unwrap().revision, 3, 11).is_err());
        assert!(harvest_state(&tree, tree.fruit.unwrap().revision, 8, 11).is_err());
        assert!(!advance(&mut tree, day(20), seasons));
        assert_eq!(tree.fruit.unwrap().remaining, 7);
        assert!(!advance(&mut tree, day(20), seasons));
        assert!(advance(&mut tree, day(21), seasons));
        assert_eq!(tree.fruit.unwrap().phase, FruitPhase::Dormant);
        assert!(advance(&mut tree, day(28), seasons));
        assert_eq!(tree.fruit.unwrap().phase, FruitPhase::Blossom);
        assert!(advance(&mut tree, day(39), seasons));
        assert_eq!(tree.fruit.unwrap().remaining, 8);
        assert_eq!(tree.fruit.unwrap().picked, 0);
    }
    #[test]
    fn no_seasons_uses_saved_four_day_cycles_and_exhausted_revisions_do_not_mutate() {
        let seasons = SeasonConfig {
            enabled: false,
            ..Default::default()
        };
        let mut tree = tree();
        advance(&mut tree, day(2), seasons);
        for slot in 0..8 {
            tree = harvest_state(&tree, tree.fruit.unwrap().revision, slot, 2).unwrap();
        }
        assert_eq!(tree.fruit.unwrap().remaining, 0);
        assert!(!advance(&mut tree, day(3), seasons));
        assert!(harvest_state(&tree, tree.fruit.unwrap().revision, 0, 3).is_err());
        assert!(!advance(&mut tree, day(1), seasons));
        assert!(advance(&mut tree, day(6), seasons));
        assert_eq!(tree.fruit.unwrap().remaining, 8);
        tree.fruit.as_mut().unwrap().revision = u64::MAX;
        let old = tree.clone();
        assert!(harvest_state(&tree, u64::MAX, 0, 6).is_err());
        assert!(!advance(&mut tree, day(8), seasons));
        assert_eq!(tree, old);
        tree.stage = TreeStage::Felled;
        tree.fruit = None;
        assert!(harvest_state(&tree, 0, 0, 9).is_err());
    }
}
