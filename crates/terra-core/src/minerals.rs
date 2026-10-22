//! Stable mineral identities and saved, validated generation controls.
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OreKind {
    Salt,
    Iron,
    Coal,
    Gold,
    Diamond,
    Ruby,
}
impl OreKind {
    pub const ALL: [Self; 6] = [
        Self::Salt,
        Self::Iron,
        Self::Coal,
        Self::Gold,
        Self::Diamond,
        Self::Ruby,
    ];
    pub const fn material(self) -> u16 {
        30 + self as u16
    }
    pub const fn from_material(material: u16) -> Option<Self> {
        match material {
            30 => Some(Self::Salt),
            31 => Some(Self::Iron),
            32 => Some(Self::Coal),
            33 => Some(Self::Gold),
            34 => Some(Self::Diamond),
            35 => Some(Self::Ruby),
            _ => None,
        }
    }
    pub const fn color(self) -> [f32; 4] {
        match self {
            Self::Salt => [0.98, 0.94, 0.87, 1.0],
            Self::Iron => [0.76, 0.69, 0.63, 1.0],
            Self::Coal => [0.035, 0.045, 0.055, 1.0],
            Self::Gold => [0.95, 0.61, 0.075, 1.0],
            Self::Diamond => [0.10, 0.78, 0.98, 1.0],
            Self::Ruby => [0.92, 0.025, 0.055, 1.0],
        }
    }
    pub const fn resource(self) -> MineralItem {
        match self {
            Self::Salt => MineralItem::SaltCrystal,
            Self::Iron => MineralItem::RawIron,
            Self::Coal => MineralItem::Coal,
            Self::Gold => MineralItem::RawGold,
            Self::Diamond => MineralItem::Diamond,
            Self::Ruby => MineralItem::Ruby,
        }
    }
    pub const fn mining_level(self) -> u8 {
        match self {
            Self::Salt | Self::Coal => 1,
            Self::Iron | Self::Gold => 2,
            Self::Diamond | Self::Ruby => 3,
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MineralItem {
    SaltCrystal,
    RawIron,
    Coal,
    RawGold,
    Diamond,
    Ruby,
    IronIngot,
    GoldIngot,
}
impl MineralItem {
    pub const ALL: [Self; 8] = [
        Self::SaltCrystal,
        Self::RawIron,
        Self::Coal,
        Self::RawGold,
        Self::Diamond,
        Self::Ruby,
        Self::IronIngot,
        Self::GoldIngot,
    ];
    pub const fn slug(self) -> &'static str {
        match self {
            Self::SaltCrystal => "salt_crystal",
            Self::RawIron => "raw_iron",
            Self::Coal => "coal",
            Self::RawGold => "raw_gold",
            Self::Diamond => "diamond",
            Self::Ruby => "ruby",
            Self::IronIngot => "iron_ingot",
            Self::GoldIngot => "gold_ingot",
        }
    }
    pub const fn ore(self) -> OreKind {
        match self {
            Self::SaltCrystal => OreKind::Salt,
            Self::RawIron | Self::IronIngot => OreKind::Iron,
            Self::Coal => OreKind::Coal,
            Self::RawGold | Self::GoldIngot => OreKind::Gold,
            Self::Diamond => OreKind::Diamond,
            Self::Ruby => OreKind::Ruby,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct OreVeinConfig {
    pub min_depth: f64,
    pub max_depth: f64,
    /// Probability per prospect cell (10^3 salt, 32^3 other minerals).
    pub frequency: f64,
    pub radius: f64,
    /// Fraction restricted to cave walls. Salt is always wall-bound; higher
    /// affinity tightens its coating instead of allowing buried salt veins.
    pub cave_affinity: f64,
}
impl Default for OreVeinConfig {
    fn default() -> Self {
        Self {
            min_depth: 5.0,
            max_depth: 160.0,
            frequency: 0.5,
            radius: 3.0,
            cave_affinity: 0.3,
        }
    }
}
impl OreVeinConfig {
    pub fn valid(self) -> bool {
        self.min_depth.is_finite()
            && self.max_depth.is_finite()
            && self.min_depth >= 3.0
            && self.max_depth > self.min_depth
            && self.max_depth <= 1024.0
            && self.frequency.is_finite()
            && (0.0..=1.0).contains(&self.frequency)
            && self.radius.is_finite()
            && (0.5..=8.0).contains(&self.radius)
            && self.cave_affinity.is_finite()
            && (0.0..=1.0).contains(&self.cave_affinity)
    }
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct OreGeneration {
    pub enabled: bool,
    /// Feature stamp, independent of the unchanged landscape generator version.
    #[serde(default)]
    pub revision: u8,
    pub salt: OreVeinConfig,
    pub iron: OreVeinConfig,
    pub coal: OreVeinConfig,
    pub gold: OreVeinConfig,
    pub diamond: OreVeinConfig,
    pub ruby: OreVeinConfig,
}
impl Default for OreGeneration {
    fn default() -> Self {
        let vein = |min_depth, max_depth, frequency, radius, cave_affinity| OreVeinConfig {
            min_depth,
            max_depth,
            frequency,
            radius,
            cave_affinity,
        };
        Self {
            enabled: true,
            revision: 1,
            salt: vein(6.0, 160.0, 0.90, 6.0, 0.92),
            iron: vein(8.0, 240.0, 0.85, 4.2, 0.28),
            coal: vein(5.0, 180.0, 0.95, 4.8, 0.18),
            gold: vein(32.0, 320.0, 0.42, 3.0, 0.40),
            diamond: vein(70.0, 400.0, 0.20, 2.4, 0.55),
            ruby: vein(55.0, 360.0, 0.25, 2.8, 0.60),
        }
    }
}
impl OreGeneration {
    pub fn legacy() -> Self {
        Self {
            enabled: false,
            revision: 0,
            ..Self::default()
        }
    }
    pub fn vein(&self, ore: OreKind) -> OreVeinConfig {
        match ore {
            OreKind::Salt => self.salt,
            OreKind::Iron => self.iron,
            OreKind::Coal => self.coal,
            OreKind::Gold => self.gold,
            OreKind::Diamond => self.diamond,
            OreKind::Ruby => self.ruby,
        }
    }
    pub fn valid(&self) -> bool {
        self.revision <= 1
            && (!self.enabled || self.revision == 1)
            && OreKind::ALL.iter().all(|&ore| self.vein(ore).valid())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Config, ItemKind, ToolKind, WorldConfig};
    #[test]
    fn old_worlds_remain_disabled_and_new_worlds_save_a_native_ore_stamp() {
        let old = WorldConfig::from_toml("seed = 42").unwrap();
        assert!(!old.ores.enabled);
        assert_eq!(old.ores.revision, 0);
        let new = WorldConfig::default();
        assert!(new.ores.enabled);
        assert_eq!(new.ores.revision, 1);
        assert_eq!(
            WorldConfig::from_toml(&toml::to_string(&new).unwrap()).unwrap(),
            new
        );
        assert!(WorldConfig::from_toml("[ores]\nenabled = true").is_err());
        let prefix = "[ores]\nenabled = true\nrevision = 1\n[ores.iron]\n";
        assert!(WorldConfig::from_toml(&format!("{prefix}radius = 4.0")).is_ok());
        for invalid in [
            "radius = 8.1",
            "frequency = nan",
            "min_depth = -1.0",
            "cave_affinity = 2.0",
        ] {
            assert!(WorldConfig::from_toml(&format!("{prefix}{invalid}")).is_err());
        }
    }
    #[test]
    fn ore_ids_resources_and_tool_progression_are_stable() {
        for (index, ore) in OreKind::ALL.into_iter().enumerate() {
            assert_eq!(ore.material(), 30 + index as u16);
            assert_eq!(OreKind::from_material(ore.material()), Some(ore));
            assert!(ItemKind::Mineral(ore.resource()).valid());
            assert!(!ItemKind::Stick.can_harvest(ore.material()));
            assert!(ItemKind::Tool(ToolKind::IronPickaxe).can_harvest(ore.material()));
            assert_eq!(
                ItemKind::Tool(ToolKind::StonePickaxe).can_harvest(ore.material()),
                ore.mining_level() <= 2
            );
        }
        assert_eq!(
            serde_json::to_string(&ItemKind::Block(29)).unwrap(),
            "{\"block\":29}"
        );
        assert_eq!(
            serde_json::to_string(&ItemKind::Tool(ToolKind::StonePickaxe)).unwrap(),
            "{\"tool\":\"stone_pickaxe\"}"
        );
    }
    #[test]
    fn real_json_furnace_refining_and_iron_pickaxe_consume_resources_atomically() {
        use crate::crafting::{CraftDestination, CraftStation, RecipeBook, recipe_key};
        use crate::furnace::{self, FurnaceState, SmeltingRecipe};
        use crate::{Inventory, ItemStack};
        let recipe: SmeltingRecipe =
            serde_json::from_str(include_str!("../../../assets/smelting/iron.json")).unwrap();
        assert_eq!(furnace::recipe_for(recipe.input), Some(&recipe));
        assert_eq!(
            recipe.output.item,
            ItemKind::Mineral(MineralItem::IronIngot)
        );
        assert_eq!(recipe.output.count, 1);
        let mut furnace = Inventory {
            slots: vec![
                ItemStack {
                    item: recipe.input,
                    count: 3,
                },
                ItemStack {
                    item: ItemKind::Mineral(MineralItem::Coal),
                    count: 1,
                },
            ],
            ..Default::default()
        };
        let mut state = FurnaceState::default();
        for _ in 0..(recipe.seconds * 3.0).ceil() as usize {
            assert!(furnace::advance(&mut state, &mut furnace, 1.0, 0));
        }
        assert_eq!(state.completed, 3);
        assert_eq!(furnace.slot(furnace::INPUT), None);
        assert_eq!(furnace.slot(furnace::FUEL), None);
        assert_eq!(state.burn_total, 80.0);
        assert_eq!(state.burn_remaining, 80.0 - recipe.seconds * 3.0);
        let mut inventory = Inventory {
            slots: vec![
                furnace.slot(furnace::OUTPUT).unwrap(),
                ItemStack {
                    item: ItemKind::Stick,
                    count: 2,
                },
            ],
            ..Default::default()
        };
        let book = RecipeBook::builtins();
        assert!(book.get(recipe_key("teramic::refine_iron")).is_none());
        assert_eq!(
            inventory.count(ItemKind::Mineral(MineralItem::IronIngot)),
            3
        );
        let before = inventory.clone();
        assert!(
            book.craft(
                &mut inventory,
                recipe_key("teramic::iron_pickaxe"),
                2,
                CraftStation::Workbench,
                CraftDestination::Inventory,
            )
            .is_err()
        );
        assert_eq!(inventory, before);
        book.craft(
            &mut inventory,
            recipe_key("teramic::iron_pickaxe"),
            1,
            CraftStation::Workbench,
            CraftDestination::Inventory,
        )
        .unwrap();
        assert_eq!(inventory.count(ItemKind::Tool(ToolKind::IronPickaxe)), 1);
        assert_eq!(inventory.count(ItemKind::Mineral(MineralItem::RawIron)), 0);
        assert_eq!(inventory.count(ItemKind::Stick), 0);
    }
}
