//! Spawn filters reference the existing world biome slugs; they never own habitat state.
use super::{CreatureKind, MobDefinition};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpawnWater {
    Ocean,
    Lake,
    River,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct MobSpawnSettings {
    pub enabled: bool,
    /// Empty means all existing surface biomes.
    pub biomes: Vec<String>,
    pub herd_size: Option<[u8; 2]>,
    pub biome_herds: Vec<SpawnHerd>,
    pub aquatic: Option<AquaticSpawn>,
    /// Per suitable underground habitat cell; None preserves legacy gates.
    pub cave_chance: Option<f64>,
}

impl Default for MobSpawnSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            biomes: Vec::new(),
            herd_size: None,
            biome_herds: Vec::new(),
            aquatic: None,
            cave_chance: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SpawnHerd {
    pub biome: String,
    pub size: [u8; 2],
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct AquaticSpawn {
    pub ocean_weight: f64,
    pub lake_weight: f64,
    pub river_weight: f64,
    /// Contiguous water-column depth, in blocks.
    pub water_depth: [f64; 2],
    /// Feet/base depth beneath the surface. None preserves the existing placement.
    pub depth_below_surface: Option<[f64; 2]>,
}

impl Default for AquaticSpawn {
    fn default() -> Self {
        Self {
            ocean_weight: 0.0,
            lake_weight: 0.0,
            river_weight: 0.0,
            water_depth: [0.0, 2048.0],
            depth_below_surface: None,
        }
    }
}

fn biome_valid(name: &str) -> bool {
    matches!(
        name,
        "ocean"
            | "plains"
            | "forest"
            | "desert"
            | "highland"
            | "mushroom_park"
            | "cliffted_desert"
            | "cherry_forest"
            | "spirit_blossom"
            | "blood_forest"
            | "ashlands"
            | "savanna"
            | "swamp"
            | "petrified_forest"
            | "crystal_highlands"
            | "misty_coast"
    )
}

fn depth_valid([min, max]: [f64; 2]) -> bool {
    min.is_finite() && max.is_finite() && min >= 0.0 && max >= min && max <= 2048.0
}

fn herd_valid([min, max]: [u8; 2], limit: u8) -> bool {
    min > 0 && min <= max && max <= limit
}

impl MobSpawnSettings {
    pub fn valid(&self, limit: u8) -> bool {
        self.biomes.len() <= 12
            && self.biomes.iter().all(|biome| biome_valid(biome))
            && self
                .biomes
                .iter()
                .enumerate()
                .all(|(i, biome)| !self.biomes[..i].contains(biome))
            && self.herd_size.is_none_or(|size| herd_valid(size, limit))
            && self.biome_herds.len() <= 12
            && self.biome_herds.iter().enumerate().all(|(i, herd)| {
                biome_valid(&herd.biome)
                    && herd_valid(herd.size, limit)
                    && !self.biome_herds[..i]
                        .iter()
                        .any(|old| old.biome == herd.biome)
            })
            && self.aquatic.is_none_or(AquaticSpawn::valid)
            && self
                .cave_chance
                .is_none_or(|chance| chance.is_finite() && (0.0..=1.0).contains(&chance))
    }

    fn permits_biome(&self, biome: &str) -> bool {
        self.enabled
            && (self.biomes.is_empty()
                || self
                    .biomes
                    .iter()
                    .any(|name| name == biome || (biome == "cherry_forest" && name == "forest")))
    }
}

impl AquaticSpawn {
    pub fn valid(self) -> bool {
        [self.ocean_weight, self.lake_weight, self.river_weight]
            .into_iter()
            .all(|weight| weight.is_finite() && (0.0..=1000.0).contains(&weight))
            && depth_valid(self.water_depth)
            && self.depth_below_surface.is_none_or(depth_valid)
    }

    pub fn weight(self, water: SpawnWater, depth: f64) -> f64 {
        if !depth.is_finite() || !(self.water_depth[0]..=self.water_depth[1]).contains(&depth) {
            return 0.0;
        }
        match water {
            SpawnWater::Ocean => self.ocean_weight,
            SpawnWater::Lake => self.lake_weight,
            SpawnWater::River => self.river_weight,
        }
    }

    pub fn legacy(kind: CreatureKind) -> Self {
        use CreatureKind::*;
        let ocean_weight = match kind {
            Squid => 13.0,
            MoonJelly | FireJelly | SunJelly | ReefShark => 6.0,
            VoidJelly | CrystalJelly => 5.0,
            HammerheadShark => 4.0,
            WhaleShark => 2.0,
            Fish => 47.0,
            _ => 0.0,
        };
        Self {
            ocean_weight,
            lake_weight: match kind {
                Squid => 1.0,
                Fish => 23.0,
                _ => 0.0,
            },
            river_weight: if kind == Fish { 1.0 } else { 0.0 },
            ..Self::default()
        }
    }
}

impl MobDefinition {
    pub fn permits_cave_spawn(&self, seed: u64, legacy_denominator: u64) -> bool {
        if legacy_denominator == 0 || self.spawn.as_ref().is_some_and(|spawn| !spawn.enabled) {
            return false;
        }
        let Some(chance) = self.spawn.as_ref().and_then(|spawn| spawn.cave_chance) else {
            return seed.is_multiple_of(legacy_denominator);
        };
        // Stratify the existing gate rather than rerolling old eligible IDs.
        // Increasing its chance retains every legacy cell and opens a stable
        // fraction of additional cells. No per-frame randomness or new IDs.
        let bucket = seed % legacy_denominator;
        let fraction = (seed / legacy_denominator % 10_000) as f64 / 10_000.0;
        (bucket as f64 + fraction) < chance * legacy_denominator as f64
    }

    pub fn permits_spawn_biome(&self, biome: &str) -> bool {
        self.spawn
            .as_ref()
            .is_none_or(|spawn| spawn.permits_biome(biome))
    }

    pub fn aquatic_spawn(&self) -> AquaticSpawn {
        self.spawn
            .as_ref()
            .and_then(|spawn| spawn.aquatic)
            .unwrap_or_else(|| AquaticSpawn::legacy(self.id))
    }

    pub fn spawn_group_size(&self, seed: u64, biome: &str, legacy: u8) -> u8 {
        if !self.permits_spawn_biome(biome) {
            return 0;
        }
        let range = self.spawn.as_ref().and_then(|spawn| {
            spawn
                .biome_herds
                .iter()
                .find(|herd| herd.biome == biome)
                .map(|herd| herd.size)
                .or(spawn.herd_size)
        });
        range.map_or(legacy.min(self.group_size), |[min, max]| {
            min + (seed % u64::from(max - min + 1)) as u8
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn legacy_fish() -> MobDefinition {
        let mut json: serde_json::Value =
            serde_json::from_str(include_str!("../../../../../assets/terra/mobs/fish.json"))
                .unwrap();
        json.as_object_mut().unwrap().remove("spawn");
        serde_json::from_value(json).unwrap()
    }

    #[test]
    fn missing_or_empty_spawn_settings_preserve_legacy_defaults() {
        let mut fish = legacy_fish();
        assert!(fish.spawn.is_none() && fish.valid());
        assert_eq!(
            fish.aquatic_spawn(),
            AquaticSpawn::legacy(CreatureKind::Fish)
        );
        for seed in 0..60 {
            for count in 1..=4 {
                assert_eq!(fish.spawn_group_size(seed, "forest", count), count);
            }
        }
        fish.spawn = Some(serde_json::from_str("{}").unwrap());
        assert!(fish.valid());
        assert_eq!(
            fish.aquatic_spawn(),
            AquaticSpawn::legacy(CreatureKind::Fish)
        );
        assert_eq!(fish.spawn_group_size(1, "plains", 3), 3);
    }

    #[test]
    fn invalid_spawn_weights_depths_and_group_bounds_are_rejected() {
        for weight in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, -0.1, 1000.1] {
            assert!(
                !AquaticSpawn {
                    ocean_weight: weight,
                    ..Default::default()
                }
                .valid()
            );
        }
        for range in [
            [-1.0, 2.0],
            [3.0, 2.0],
            [0.0, f64::INFINITY],
            [f64::NAN, 1.0],
            [0.0, 2048.1],
        ] {
            assert!(
                !AquaticSpawn {
                    water_depth: range,
                    ..Default::default()
                }
                .valid()
            );
            assert!(
                !AquaticSpawn {
                    depth_below_surface: Some(range),
                    ..Default::default()
                }
                .valid()
            );
        }
        let mut fish = legacy_fish();
        for range in [[0, 2], [3, 2], [1, 5]] {
            fish.spawn = Some(MobSpawnSettings {
                herd_size: Some(range),
                ..Default::default()
            });
            assert!(!fish.valid());
        }
        assert!(
            !serde_json::from_str::<MobSpawnSettings>(r#"{"biomes":["invented_biome"]}"#)
                .unwrap()
                .valid(4)
        );
        assert!(
            serde_json::from_str::<MobSpawnSettings>(r#"{"aquatic":{"unknown_weight":1}}"#)
                .is_err()
        );
    }

    #[test]
    fn biome_filters_and_herd_overrides_affect_actual_group_counts() {
        let mut fish = legacy_fish();
        fish.spawn = Some(MobSpawnSettings {
            biomes: vec!["forest".into(), "plains".into()],
            herd_size: Some([2, 3]),
            biome_herds: vec![SpawnHerd {
                biome: "forest".into(),
                size: [1, 2],
            }],
            ..Default::default()
        });
        assert!(fish.valid());
        for seed in 0..100 {
            assert_eq!(
                fish.spawn_group_size(seed, "forest", 4),
                1 + (seed % 2) as u8
            );
            assert_eq!(
                fish.spawn_group_size(seed, "plains", 4),
                2 + (seed % 2) as u8
            );
            assert_eq!(fish.spawn_group_size(seed, "ocean", 4), 0);
        }
        fish.spawn.as_mut().unwrap().enabled = false;
        assert_eq!(fish.spawn_group_size(0, "forest", 4), 0);
    }

    #[test]
    fn explicit_builtin_marine_weights_match_the_legacy_contract() {
        for kind in CreatureKind::ALL
            .into_iter()
            .filter(|kind| kind.is_aquatic())
        {
            let definition = super::super::get(kind);
            assert!(
                definition
                    .spawn
                    .as_ref()
                    .and_then(|spawn| spawn.aquatic)
                    .is_some()
            );
            assert_eq!(definition.aquatic_spawn(), AquaticSpawn::legacy(kind));
        }
    }

    #[test]
    fn cave_spawn_chances_are_validated_relative_increases_without_rerolling_old_cells() {
        for (kind, denominator, chance, old_count, new_count) in [
            (CreatureKind::Stalker, 11_u64, 0.1, 10_000, 11_000),
            (CreatureKind::MediumSpider, 5, 0.25, 10_000, 12_500),
            (CreatureKind::LargeSpider, 5, 0.25, 10_000, 12_500),
        ] {
            let definition = super::super::get(kind);
            assert_eq!(definition.spawn.as_ref().unwrap().cave_chance, Some(chance));
            let mut old = 0;
            let mut new = 0;
            for seed in 0..denominator * 10_000 {
                let allowed = definition.permits_cave_spawn(seed, denominator);
                assert_eq!(allowed, definition.permits_cave_spawn(seed, denominator));
                if seed.is_multiple_of(denominator) {
                    old += 1;
                    assert!(allowed, "old eligible cell must not be rerolled");
                }
                new += usize::from(allowed);
            }
            assert_eq!(old, old_count);
            assert_eq!(new, new_count);
        }
        let mut definition = super::super::get(CreatureKind::MediumSpider).clone();
        for chance in [f64::NAN, f64::INFINITY, -0.01, 1.01] {
            definition.spawn.as_mut().unwrap().cave_chance = Some(chance);
            assert!(!definition.valid());
        }
        for chance in [0.0, 1.0] {
            definition.spawn.as_mut().unwrap().cave_chance = Some(chance);
            assert!(definition.valid());
            for seed in 0..100 {
                assert_eq!(definition.permits_cave_spawn(seed, 5), chance == 1.0);
            }
        }
        definition.spawn.as_mut().unwrap().enabled = false;
        assert!(!definition.permits_cave_spawn(0, 5));
        definition.spawn = None;
        for seed in 0..100 {
            assert_eq!(
                definition.permits_cave_spawn(seed, 5),
                seed.is_multiple_of(5)
            );
        }
    }
}
