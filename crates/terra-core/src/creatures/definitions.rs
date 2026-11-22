//! Shared immutable species data: the dedicated server and client load the same JSON contract.
use super::CreatureKind;
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;

mod spawn;
pub use spawn::{AquaticSpawn, MobSpawnSettings, SpawnHerd, SpawnWater};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MobDefinition {
    pub id: CreatureKind,
    pub title: String,
    #[serde(default)]
    pub variant_titles: Vec<String>,
    pub health: f32,
    pub food: f32,
    pub water: f32,
    pub size: [f64; 3],
    #[serde(default)]
    pub minimum_size: Option<[f64; 3]>,
    pub speed: f64,
    pub group_size: u8,
    pub sleeps: bool,
    #[serde(default)]
    pub combat: Option<MobCombat>,
    #[serde(default)]
    pub spawn: Option<MobSpawnSettings>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MobCombat {
    pub damage: f32,
    pub aggro_radius: f64,
    pub reach: f64,
    pub cooldown: f32,
    #[serde(default)]
    pub poison_level: u8,
    #[serde(default)]
    pub poison_seconds: f32,
}
impl MobCombat {
    pub const MELEE_IMPACT_PHASE: f32 = 0.42;

    /// Presentation and authority share a wind-up, impact, recovery cycle.
    /// `Creature::timer` counts down from the complete configured cooldown.
    pub fn melee_phase(self, remaining: f32) -> f32 {
        (1.0 - remaining / self.cooldown).clamp(0.0, 1.0)
    }
    pub fn melee_impact_remaining(self) -> f32 {
        self.cooldown * (1.0 - Self::MELEE_IMPACT_PHASE)
    }

    /// Advance a strike only while in reach and visible. Contact starts a
    /// fresh wind-up; exactly one tick crosses the impact of each full cycle.
    pub fn melee_contact(self, entering: bool, previous: f32, remaining: &mut f32) -> bool {
        let impact = self.melee_impact_remaining();
        let hit = !entering && previous > impact && *remaining <= impact;
        if entering || *remaining <= 0.0 {
            *remaining = self.cooldown;
        }
        hit
    }

    fn valid(self) -> bool {
        self.damage.is_finite()
            && (0.1..=100.0).contains(&self.damage)
            && self.aggro_radius.is_finite()
            && (1.0..=64.0).contains(&self.aggro_radius)
            && self.reach.is_finite()
            && (0.5..=3.0).contains(&self.reach)
            && self.reach <= self.aggro_radius
            && self.cooldown.is_finite()
            && (0.5..=10.0).contains(&self.cooldown)
            && self.poison_seconds.is_finite()
            && (self.poison_level == 0 && self.poison_seconds == 0.0
                || (1..=3).contains(&self.poison_level)
                    && (0.1..=600.0).contains(&self.poison_seconds))
    }
}
impl MobDefinition {
    pub fn variant_title(&self, variant: u8) -> &str {
        self.variant_titles
            .get(usize::from(variant))
            .map_or(&self.title, String::as_str)
    }
    pub fn valid(&self) -> bool {
        !self.title.trim().is_empty()
            && self.variant_titles.len()
                <= if self.id == CreatureKind::Villager {
                    usize::from(super::villagers::ROLE_COUNT)
                } else {
                    16
                }
            && self
                .variant_titles
                .iter()
                .all(|s| !s.trim().is_empty() && s.len() <= 80)
            && self.title.len() <= 80
            && self.health.is_finite()
            && (0.1..=1000.0).contains(&self.health)
            && [self.food, self.water]
                .iter()
                .all(|v| v.is_finite() && (0.0..=1000.0).contains(v))
            && self
                .size
                .iter()
                .all(|v| v.is_finite() && (0.025..=8.0).contains(v))
            && self.minimum_size.is_none_or(|minimum| {
                minimum
                    .into_iter()
                    .zip(self.size)
                    .all(|(min, max)| min.is_finite() && (0.025..=max).contains(&min))
            })
            && self.speed.is_finite()
            && (0.05..=8.0).contains(&self.speed)
            && (1..=4).contains(&self.group_size)
            && self.combat.is_none_or(MobCombat::valid)
            && self
                .spawn
                .as_ref()
                .is_none_or(|spawn| spawn.valid(self.group_size))
    }
}
pub fn get(kind: CreatureKind) -> &'static MobDefinition {
    static DEFINITIONS: OnceLock<Vec<MobDefinition>> = OnceLock::new();
    &DEFINITIONS.get_or_init(|| {
        let root = crate::asset_directory();
        [
            (
                "sheep",
                include_str!("../../../../assets/terra/mobs/sheep.json"),
            ),
            (
                "cow",
                include_str!("../../../../assets/terra/mobs/cow.json"),
            ),
            (
                "pig",
                include_str!("../../../../assets/terra/mobs/pig.json"),
            ),
            (
                "fish",
                include_str!("../../../../assets/terra/mobs/fish.json"),
            ),
            (
                "bee",
                include_str!("../../../../assets/terra/mobs/bee.json"),
            ),
            (
                "bear",
                include_str!("../../../../assets/terra/mobs/bear.json"),
            ),
            (
                "squid",
                include_str!("../../../../assets/terra/mobs/squid.json"),
            ),
            (
                "moon_jelly",
                include_str!("../../../../assets/terra/mobs/moon_jelly.json"),
            ),
            (
                "fire_jelly",
                include_str!("../../../../assets/terra/mobs/fire_jelly.json"),
            ),
            (
                "void_jelly",
                include_str!("../../../../assets/terra/mobs/void_jelly.json"),
            ),
            (
                "sun_jelly",
                include_str!("../../../../assets/terra/mobs/sun_jelly.json"),
            ),
            (
                "crystal_jelly",
                include_str!("../../../../assets/terra/mobs/crystal_jelly.json"),
            ),
            (
                "reef_shark",
                include_str!("../../../../assets/terra/mobs/reef_shark.json"),
            ),
            (
                "hammerhead_shark",
                include_str!("../../../../assets/terra/mobs/hammerhead_shark.json"),
            ),
            (
                "whale_shark",
                include_str!("../../../../assets/terra/mobs/whale_shark.json"),
            ),
            (
                "horse",
                include_str!("../../../../assets/terra/mobs/horse.json"),
            ),
            (
                "small_spider",
                include_str!("../../../../assets/terra/mobs/small_spider.json"),
            ),
            (
                "medium_spider",
                include_str!("../../../../assets/terra/mobs/medium_spider.json"),
            ),
            (
                "large_spider",
                include_str!("../../../../assets/terra/mobs/large_spider.json"),
            ),
            (
                "stalker",
                include_str!("../../../../assets/terra/mobs/stalker.json"),
            ),
            (
                "villager",
                include_str!("../../../../assets/terra/mobs/villager.json"),
            ),
            (
                "butterfly",
                include_str!("../../../../assets/terra/mobs/butterfly.json"),
            ),
            (
                "spirit_gloom",
                include_str!("../../../../assets/terra/mobs/spirit_gloom.json"),
            ),
            (
                "spirit_fox",
                include_str!("../../../../assets/terra/mobs/spirit_fox.json"),
            ),
            (
                "skeleton",
                include_str!("../../../../assets/terra/mobs/skeleton.json"),
            ),
            (
                "raven",
                include_str!("../../../../assets/terra/mobs/raven.json"),
            ),
            (
                "blood_slime",
                include_str!("../../../../assets/terra/mobs/blood_slime.json"),
            ),
            (
                "blood_stalker",
                include_str!("../../../../assets/terra/mobs/blood_stalker.json"),
            ),
            (
                "blood_wisp",
                include_str!("../../../../assets/terra/mobs/blood_wisp.json"),
            ),
            (
                "thorn_brute",
                include_str!("../../../../assets/terra/mobs/thorn_brute.json"),
            ),
            (
                "sand_worm",
                include_str!("../../../../assets/terra/mobs/sand_worm.json"),
            ),
            (
                "dark_dragon",
                include_str!("../../../../assets/terra/mobs/dark_dragon.json"),
            ),
            (
                "earth_dragon",
                include_str!("../../../../assets/terra/mobs/earth_dragon.json"),
            ),
            (
                "water_dragon",
                include_str!("../../../../assets/terra/mobs/water_dragon.json"),
            ),
        ]
        .into_iter()
        .enumerate()
        .map(|(index, (name, fallback))| {
            let builtin: MobDefinition =
                serde_json::from_str(fallback).unwrap_or_else(|_| MobDefinition {
                    id: CreatureKind::ALL[index],
                    title: name.into(),
                    variant_titles: Vec::new(),
                    health: 1.0,
                    food: 0.0,
                    water: 0.0,
                    size: [0.1; 3],
                    minimum_size: None,
                    speed: 1.0,
                    group_size: 1,
                    sleeps: false,
                    combat: None,
                    spawn: None,
                });
            let path = root.join(format!("terra/mobs/{name}.json"));
            match std::fs::read_to_string(&path) {
                Ok(source) => match serde_json::from_str::<MobDefinition>(&source) {
                    Ok(data) if data.id == CreatureKind::ALL[index] && data.valid() => data,
                    _ => {
                        eprintln!(
                            "Invalid mob definition {}; using builtin values",
                            path.display()
                        );
                        builtin
                    }
                },
                Err(_) => builtin,
            }
        })
        .collect()
    })[kind as usize]
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn all_species_json_is_valid_and_matches_the_stable_kind() {
        for kind in CreatureKind::ALL {
            assert!(get(kind).valid());
            assert_eq!(get(kind).id, kind);
        }
        assert_eq!(get(CreatureKind::Bee).group_size, 4);
        let residents = get(CreatureKind::Villager);
        assert_eq!(
            residents.variant_titles.len(),
            usize::from(super::super::villagers::ROLE_COUNT)
        );
        for role in super::super::villagers::ENGINEER..super::super::villagers::ROLE_COUNT {
            assert_ne!(residents.variant_title(role), residents.title);
        }
    }
}
