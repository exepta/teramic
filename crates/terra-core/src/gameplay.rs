//! Small authoritative inventories and persistent loose resources.
use crate::PlayerUuid;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashSet};

mod inventory_ops;

/// Nine hotbar slots and two complete nine-slot backpack rows.
pub const INVENTORY_SLOTS: usize = 27;
pub const STACK_LIMIT: u16 = 128;
pub const DROP_LIFETIME_SECONDS: f32 = 300.0;
pub const MAX_WORLD_DROPS: usize = 2048;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GameMode {
    #[default]
    Survival,
    Creative,
    Spectator,
}
impl GameMode {
    pub fn label(self) -> &'static str {
        match self {
            Self::Survival => "Survival",
            Self::Creative => "Creative",
            Self::Spectator => "Spectator",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResourcePropKind {
    Grass,
    Flower,
    Bush,
    Fern,
    Mushroom,
    SmallPlant,
    BeeNest,
    Seagrass,
    SeaCucumber,
    BranchingCoral,
    TubeCoral,
    PlateCoral,
    SeaAnemone,
    SmallStones,
    Wheat,
    Corn,
    Lettuce,
    Scarecrow,
    SpiderWeb,
    OakSapling,
    SpruceSapling,
    BirchSapling,
    CherrySapling,
    AppleSapling,
    /// Internal derived fruit target; never an inventory/placement item.
    AppleFruit,
}
impl ResourcePropKind {
    pub const ALL: [Self; 24] = [
        Self::Grass,
        Self::Flower,
        Self::Bush,
        Self::Fern,
        Self::Mushroom,
        Self::SmallPlant,
        Self::BeeNest,
        Self::Seagrass,
        Self::SeaCucumber,
        Self::BranchingCoral,
        Self::TubeCoral,
        Self::PlateCoral,
        Self::SeaAnemone,
        Self::SmallStones,
        Self::Wheat,
        Self::Corn,
        Self::Lettuce,
        Self::Scarecrow,
        Self::SpiderWeb,
        Self::OakSapling,
        Self::SpruceSapling,
        Self::BirchSapling,
        Self::CherrySapling,
        Self::AppleSapling,
    ];
    pub fn slug(self) -> &'static str {
        match self {
            Self::Grass => "grass_tuft",
            Self::Flower => "flower",
            Self::Bush => "bush",
            Self::Fern => "fern",
            Self::Mushroom => "mushroom",
            Self::SmallPlant => "small_plant",
            Self::BeeNest => "bee_nest",
            Self::Seagrass => "seagrass",
            Self::SeaCucumber => "sea_cucumber",
            Self::BranchingCoral => "branching_coral",
            Self::TubeCoral => "tube_coral",
            Self::PlateCoral => "plate_coral",
            Self::SeaAnemone => "sea_anemone",
            Self::SmallStones => "small_stones",
            Self::Wheat => "wheat_crop",
            Self::Corn => "corn_crop",
            Self::Lettuce => "lettuce_crop",
            Self::Scarecrow => "scarecrow",
            Self::SpiderWeb => "spider_web",
            Self::OakSapling => "oak_sapling",
            Self::SpruceSapling => "spruce_sapling",
            Self::BirchSapling => "birch_sapling",
            Self::CherrySapling => "cherry_sapling",
            Self::AppleSapling => "apple_sapling",
            Self::AppleFruit => "apple_fruit",
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::Grass => "Grass",
            Self::Flower => "Flower",
            Self::Bush => "Bush",
            Self::Fern => "Fern",
            Self::Mushroom => "Mushroom",
            Self::SmallPlant => "Small plant",
            Self::BeeNest => "Bee nest",
            Self::Seagrass => "Seagrass",
            Self::SeaCucumber => "Sea cucumber",
            Self::BranchingCoral => "Branching coral",
            Self::TubeCoral => "Tube coral",
            Self::PlateCoral => "Plate coral",
            Self::SeaAnemone => "Sea anemone",
            Self::SmallStones => "Small stones",
            Self::Wheat => "Wheat",
            Self::Corn => "Corn",
            Self::Lettuce => "Lettuce",
            Self::Scarecrow => "Scarecrow",
            Self::SpiderWeb => "Spider web",
            Self::OakSapling => "Oak sapling",
            Self::SpruceSapling => "Spruce sapling",
            Self::BirchSapling => "Birch sapling",
            Self::CherrySapling => "Cherry sapling",
            Self::AppleSapling => "Apple sapling",
            Self::AppleFruit => "Apple fruit",
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ItemKind {
    Block(u16),
    Prop(ResourcePropKind),
    Tool(ToolKind),
    Stick,
    WoodenBlock {
        material: u16,
        wood: WoodSpecies,
    },
    Mineral(crate::MineralItem),
    Animal(crate::creatures::AnimalProduct),
    Bellows(BellowsKind),
    Dye(crate::beds::DyeColor),
    DyedBed {
        material: u16,
        color: crate::beds::DyeColor,
    },
    Armor(crate::ArmorItem),
    Accessory(crate::AccessoryKind),
    Cape(crate::CapeItem),
    Dust(crate::DustKind),
    Flower(crate::beds::DyeColor),
    Crop(crate::CropItem),
    TintedBlock {
        material: u16,
        wood: WoodSpecies,
        color: [u8; 3],
    },
    GlassBottle,
    WaterBottle,
    Potion(u16),
    /// Water belongs to this vessel across inventories, drops and saved worlds.
    FilledWateringCan {
        tool: ToolKind,
        units: u8,
    },
    /// A snapshot of its maker's heraldry, retained through trade, drops and saves.
    /// Appended to preserve the binary identities of existing item variants.
    HeraldicCape {
        cape: crate::CapeItem,
        heraldry: crate::kingdom::KingdomHeraldry,
    },
    Apple,
    AppleCore,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BellowsKind {
    Stone,
    Iron,
    Gold,
    Ruby,
}
impl BellowsKind {
    pub const fn slug(self) -> &'static str {
        match self {
            Self::Stone => "stone_bellows",
            Self::Iron => "iron_bellows",
            Self::Gold => "gold_bellows",
            Self::Ruby => "ruby_bellows",
        }
    }
    pub const fn speed_multiplier(self) -> f32 {
        match self {
            Self::Stone | Self::Gold => 1.10,
            Self::Iron => 1.20,
            Self::Ruby => 1.15,
        }
    }
    pub const fn double_chance(self) -> f32 {
        match self {
            Self::Gold => 0.05,
            Self::Ruby => 0.075,
            _ => 0.0,
        }
    }
}

#[derive(
    Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
#[serde(rename_all = "snake_case")]
pub enum WoodSpecies {
    #[default]
    Oak,
    Birch,
    Spruce,
    Cherry,
}
impl WoodSpecies {
    pub fn tint(self) -> [f32; 4] {
        match self {
            Self::Oak => [1.0, 1.0, 1.0, 1.0],
            // Compensate the warm oak grain in linear space without new materials.
            Self::Birch => [1.35, 3.6, 16.0, 1.0],
            Self::Spruce => [0.57, 0.63, 0.64, 1.0],
            Self::Cherry => [1.22, 0.82, 1.1, 1.0],
        }
    }
    pub fn slug(self) -> &'static str {
        match self {
            Self::Oak => "oak",
            Self::Birch => "birch",
            Self::Spruce => "spruce",
            Self::Cherry => "cherry",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolKind {
    WoodPickaxe,
    StonePickaxe,
    StoneAxe,
    StoneShovel,
    IronPickaxe,
    WoodAxe,
    WoodShovel,
    WoodSword,
    StoneSword,
    IronSword,
    IronAxe,
    GoldSword,
    GoldAxe,
    GoldPickaxe,
    DiamondSword,
    DiamondAxe,
    DiamondPickaxe,
    RubySword,
    RubyAxe,
    RubyPickaxe,
    WoodWateringCan,
    IronWateringCan,
    GoldWateringCan,
    DiamondWateringCan,
    RubyWateringCan,
    WoodHoe,
    IronHoe,
    GoldHoe,
    DiamondHoe,
    RubyHoe,
    WoodScythe,
    IronScythe,
    GoldScythe,
    DiamondScythe,
    RubyScythe,
    Bucket,
    WaterBucket,
    Wrench,
    WoodSpoon,
    StoneSpoon,
    GoldSpoon,
}
impl ToolKind {
    pub const ALL: [Self; 41] = [
        Self::WoodPickaxe,
        Self::StonePickaxe,
        Self::StoneAxe,
        Self::StoneShovel,
        Self::IronPickaxe,
        Self::WoodAxe,
        Self::WoodShovel,
        Self::WoodSword,
        Self::StoneSword,
        Self::IronSword,
        Self::IronAxe,
        Self::GoldSword,
        Self::GoldAxe,
        Self::GoldPickaxe,
        Self::DiamondSword,
        Self::DiamondAxe,
        Self::DiamondPickaxe,
        Self::RubySword,
        Self::RubyAxe,
        Self::RubyPickaxe,
        Self::WoodWateringCan,
        Self::IronWateringCan,
        Self::GoldWateringCan,
        Self::DiamondWateringCan,
        Self::RubyWateringCan,
        Self::WoodHoe,
        Self::IronHoe,
        Self::GoldHoe,
        Self::DiamondHoe,
        Self::RubyHoe,
        Self::WoodScythe,
        Self::IronScythe,
        Self::GoldScythe,
        Self::DiamondScythe,
        Self::RubyScythe,
        Self::Bucket,
        Self::WaterBucket,
        Self::Wrench,
        Self::WoodSpoon,
        Self::StoneSpoon,
        Self::GoldSpoon,
    ];
    pub fn slug(self) -> &'static str {
        match self {
            Self::WoodPickaxe => "wood_pickaxe",
            Self::StonePickaxe => "stone_pickaxe",
            Self::StoneAxe => "stone_axe",
            Self::StoneShovel => "stone_shovel",
            Self::IronPickaxe => "iron_pickaxe",
            Self::WoodAxe => "wood_axe",
            Self::WoodShovel => "wood_shovel",
            Self::WoodSword => "wood_sword",
            Self::StoneSword => "stone_sword",
            Self::IronSword => "iron_sword",
            Self::IronAxe => "iron_axe",
            Self::GoldSword => "gold_sword",
            Self::GoldAxe => "gold_axe",
            Self::GoldPickaxe => "gold_pickaxe",
            Self::DiamondSword => "diamond_sword",
            Self::DiamondAxe => "diamond_axe",
            Self::DiamondPickaxe => "diamond_pickaxe",
            Self::RubySword => "ruby_sword",
            Self::RubyAxe => "ruby_axe",
            Self::RubyPickaxe => "ruby_pickaxe",
            Self::WoodWateringCan => "wood_watering_can",
            Self::IronWateringCan => "iron_watering_can",
            Self::GoldWateringCan => "gold_watering_can",
            Self::DiamondWateringCan => "diamond_watering_can",
            Self::RubyWateringCan => "ruby_watering_can",
            Self::WoodHoe => "wood_hoe",
            Self::IronHoe => "iron_hoe",
            Self::GoldHoe => "gold_hoe",
            Self::DiamondHoe => "diamond_hoe",
            Self::RubyHoe => "ruby_hoe",
            Self::WoodScythe => "wood_scythe",
            Self::IronScythe => "iron_scythe",
            Self::GoldScythe => "gold_scythe",
            Self::DiamondScythe => "diamond_scythe",
            Self::RubyScythe => "ruby_scythe",
            Self::Bucket => "bucket",
            Self::WaterBucket => "water_bucket",
            Self::Wrench => "wrench",
            Self::WoodSpoon => "wood_spoon",
            Self::StoneSpoon => "stone_spoon",
            Self::GoldSpoon => "gold_spoon",
        }
    }
    pub const fn class(self) -> Option<crate::block_properties::ToolClass> {
        use crate::block_properties::ToolClass;
        match self {
            Self::WoodPickaxe
            | Self::StonePickaxe
            | Self::IronPickaxe
            | Self::GoldPickaxe
            | Self::DiamondPickaxe
            | Self::RubyPickaxe => Some(ToolClass::Pickaxe),
            Self::WoodAxe
            | Self::StoneAxe
            | Self::IronAxe
            | Self::GoldAxe
            | Self::DiamondAxe
            | Self::RubyAxe => Some(ToolClass::Axe),
            Self::WoodShovel | Self::StoneShovel => Some(ToolClass::Shovel),
            _ => None,
        }
    }
    pub const fn mining_level(self) -> u8 {
        match self {
            Self::WoodPickaxe | Self::WoodAxe | Self::WoodShovel | Self::WoodSword => 1,
            Self::StonePickaxe | Self::StoneAxe | Self::StoneShovel | Self::StoneSword => 2,
            Self::IronPickaxe
            | Self::IronAxe
            | Self::IronSword
            | Self::GoldPickaxe
            | Self::GoldAxe
            | Self::GoldSword => 3,
            Self::DiamondPickaxe | Self::DiamondAxe | Self::DiamondSword => 4,
            Self::RubyPickaxe | Self::RubyAxe | Self::RubySword => 5,
            _ => self.farm_tier(),
        }
    }
    pub const fn efficiency(self) -> f32 {
        match self {
            Self::WoodPickaxe | Self::WoodAxe | Self::WoodShovel | Self::WoodSword => 2.0,
            Self::StonePickaxe | Self::StoneAxe | Self::StoneShovel | Self::StoneSword => 4.0,
            Self::IronPickaxe | Self::IronAxe | Self::IronSword => 6.0,
            Self::GoldPickaxe | Self::GoldAxe | Self::GoldSword => 10.0,
            Self::DiamondPickaxe | Self::DiamondAxe | Self::DiamondSword => 8.0,
            Self::RubyPickaxe | Self::RubyAxe | Self::RubySword => 12.0,
            _ => 1.0,
        }
    }
    pub const fn attack_damage(self) -> f32 {
        match self {
            Self::WoodSword => 3.0,
            Self::IronSword | Self::GoldSword | Self::DiamondAxe => 6.0,
            Self::DiamondSword => 8.0,
            Self::RubySword => 9.0,
            Self::RubyAxe => 7.0,
            _ => 5.0,
        }
    }
    pub fn speed(self, material: u16) -> f32 {
        use crate::block_properties::ToolClass;
        match (self.class(), material) {
            (Some(ToolClass::Pickaxe), 3 | 11..=13 | 30..=35 | 37..=39 | 43)
            | (Some(ToolClass::Axe), 6 | 10 | 16..=18 | 28..=29 | 36 | 134..=140)
            | (Some(ToolClass::Shovel), 1..=2 | 4..=5 | 19 | 42 | 48 | 101..=102) => {
                self.efficiency()
            }
            _ => 1.0,
        }
    }
    pub const fn farm_kind(self) -> Option<crate::farming::FarmToolKind> {
        use crate::farming::FarmToolKind;
        match self {
            Self::WoodWateringCan
            | Self::IronWateringCan
            | Self::GoldWateringCan
            | Self::DiamondWateringCan
            | Self::RubyWateringCan => Some(FarmToolKind::WateringCan),
            Self::WoodHoe | Self::IronHoe | Self::GoldHoe | Self::DiamondHoe | Self::RubyHoe => {
                Some(FarmToolKind::Hoe)
            }
            Self::WoodScythe
            | Self::IronScythe
            | Self::GoldScythe
            | Self::DiamondScythe
            | Self::RubyScythe => Some(FarmToolKind::Scythe),
            _ => None,
        }
    }
    pub const fn farm_tier(self) -> u8 {
        match self {
            Self::WoodWateringCan | Self::WoodHoe | Self::WoodScythe => 1,
            Self::IronWateringCan | Self::IronHoe | Self::IronScythe => 2,
            Self::GoldWateringCan | Self::GoldHoe | Self::GoldScythe => 3,
            Self::DiamondWateringCan | Self::DiamondHoe | Self::DiamondScythe => 4,
            Self::RubyWateringCan | Self::RubyHoe | Self::RubyScythe => 5,
            _ => 0,
        }
    }
    pub const fn farm_radius(self) -> i64 {
        match self.farm_kind() {
            Some(crate::farming::FarmToolKind::Scythe) if self.farm_tier() >= 4 => 2,
            Some(crate::farming::FarmToolKind::Scythe) => 1,
            _ => 0,
        }
    }
    pub const fn watering_capacity(self) -> u8 {
        if matches!(
            self.farm_kind(),
            Some(crate::farming::FarmToolKind::WateringCan)
        ) {
            4 + self.farm_tier() * 4
        } else {
            0
        }
    }
    pub const fn farm_cooldown(self) -> f32 {
        match self.farm_tier() {
            1 => 0.65,
            2 => 0.55,
            3 => 0.40,
            4 => 0.45,
            _ => 0.35,
        }
    }
}
impl ItemKind {
    pub const fn tool_kind(self) -> Option<ToolKind> {
        match self {
            Self::Tool(tool) | Self::FilledWateringCan { tool, .. } => Some(tool),
            _ => None,
        }
    }
    pub const fn watering_units(self) -> u8 {
        match self {
            Self::FilledWateringCan { units, .. } => units,
            _ => 0,
        }
    }
    pub fn with_watering_units(self, units: u8) -> Option<Self> {
        let tool = self.tool_kind()?;
        if tool.watering_capacity() == 0 || units > tool.watering_capacity() {
            return None;
        }
        Some(if units == 0 {
            Self::Tool(tool)
        } else {
            Self::FilledWateringCan { tool, units }
        })
    }
    pub const fn preview_item(self) -> Self {
        match self {
            Self::FilledWateringCan { tool, .. } => Self::Tool(tool),
            _ => self,
        }
    }
    pub const fn max_stack(self) -> u16 {
        match self {
            Self::FilledWateringCan { .. } => 1,
            Self::Armor(_)
            | Self::Accessory(_)
            | Self::Cape(_)
            | Self::HeraldicCape { .. }
            | Self::Tool(
                ToolKind::Bucket
                | ToolKind::WaterBucket
                | ToolKind::Wrench
                | ToolKind::WoodSpoon
                | ToolKind::StoneSpoon
                | ToolKind::GoldSpoon,
            ) => 1,
            Self::GlassBottle | Self::WaterBottle | Self::Potion(_) => 16,
            _ => STACK_LIMIT,
        }
    }
    pub fn block_material(self) -> Option<u16> {
        match self {
            Self::Block(material)
            | Self::WoodenBlock { material, .. }
            | Self::TintedBlock { material, .. }
            | Self::DyedBed { material, .. } => Some(material),
            Self::Prop(ResourcePropKind::Seagrass) => Some(50),
            Self::Mineral(crate::MineralItem::SaltCrystal) => Some(51),
            _ => None,
        }
    }
    pub fn wood_species(self) -> Option<WoodSpecies> {
        match self {
            Self::Block(6 | 16 | 28 | 29 | 141 | 144 | 148 | 149 | 196 | 202 | 203) => {
                Some(WoodSpecies::Oak)
            }
            Self::Block(10 | 17) => Some(WoodSpecies::Birch),
            Self::Block(18 | 36 | 169) => Some(WoodSpecies::Spruce),
            Self::Block(214..=217 | 267 | 268) => Some(WoodSpecies::Spruce),
            Self::Block(134 | 135) => Some(WoodSpecies::Cherry),
            Self::WoodenBlock { wood, .. }
            | Self::TintedBlock {
                material: 148,
                wood,
                ..
            } => Some(wood),
            _ => None,
        }
    }
    pub const fn supports_wood(material: u16) -> bool {
        matches!(
            material,
            28 | 29 | 141 | 144 | 148 | 149 | 169 | 196 | 202 | 203 | 214..=217 | 267 | 268
        )
    }
    pub fn for_wood(material: u16, wood: WoodSpecies) -> Self {
        if wood == Self::Block(material).wood_species().unwrap_or_default()
            || !Self::supports_wood(material)
        {
            Self::Block(material)
        } else {
            Self::WoodenBlock { material, wood }
        }
    }
    pub const EXTRA: [Self; 193] = [
        Self::GlassBottle,
        Self::WaterBottle,
        Self::Potion(1),
        Self::Potion(2),
        Self::Potion(3),
        Self::Potion(4),
        Self::Potion(5),
        Self::Potion(6),
        Self::Potion(7),
        Self::Potion(8),
        Self::Potion(9),
        Self::Potion(10),
        Self::Potion(11),
        Self::Potion(12),
        Self::Tool(ToolKind::WoodSpoon),
        Self::Tool(ToolKind::StoneSpoon),
        Self::Tool(ToolKind::GoldSpoon),
        Self::Tool(ToolKind::Wrench),
        Self::Tool(ToolKind::Bucket),
        Self::Tool(ToolKind::WaterBucket),
        Self::Tool(ToolKind::WoodWateringCan),
        Self::Tool(ToolKind::IronWateringCan),
        Self::Tool(ToolKind::GoldWateringCan),
        Self::Tool(ToolKind::DiamondWateringCan),
        Self::Tool(ToolKind::RubyWateringCan),
        Self::Tool(ToolKind::WoodHoe),
        Self::Tool(ToolKind::IronHoe),
        Self::Tool(ToolKind::GoldHoe),
        Self::Tool(ToolKind::DiamondHoe),
        Self::Tool(ToolKind::RubyHoe),
        Self::Tool(ToolKind::WoodScythe),
        Self::Tool(ToolKind::IronScythe),
        Self::Tool(ToolKind::GoldScythe),
        Self::Tool(ToolKind::DiamondScythe),
        Self::Tool(ToolKind::RubyScythe),
        Self::Crop(crate::CropItem::WheatSeeds),
        Self::Crop(crate::CropItem::CornSeeds),
        Self::Crop(crate::CropItem::LettuceSeeds),
        Self::Crop(crate::CropItem::Wheat),
        Self::Crop(crate::CropItem::Corn),
        Self::Crop(crate::CropItem::Lettuce),
        Self::Crop(crate::CropItem::Bread),
        Self::Tool(ToolKind::IronSword),
        Self::Tool(ToolKind::IronAxe),
        Self::Tool(ToolKind::GoldSword),
        Self::Tool(ToolKind::GoldAxe),
        Self::Tool(ToolKind::GoldPickaxe),
        Self::Tool(ToolKind::DiamondSword),
        Self::Tool(ToolKind::DiamondAxe),
        Self::Tool(ToolKind::DiamondPickaxe),
        Self::Tool(ToolKind::RubySword),
        Self::Tool(ToolKind::RubyAxe),
        Self::Tool(ToolKind::RubyPickaxe),
        Self::Dye(crate::beds::DyeColor::Black),
        Self::Dye(crate::beds::DyeColor::Lime),
        Self::Dye(crate::beds::DyeColor::Pink),
        Self::Dye(crate::beds::DyeColor::Gold),
        Self::Dye(crate::beds::DyeColor::Silver),
        Self::Dye(crate::beds::DyeColor::LightBlue),
        Self::Flower(crate::beds::DyeColor::Red),
        Self::Flower(crate::beds::DyeColor::Yellow),
        Self::Flower(crate::beds::DyeColor::White),
        Self::Flower(crate::beds::DyeColor::Blue),
        Self::Flower(crate::beds::DyeColor::Purple),
        Self::Flower(crate::beds::DyeColor::Orange),
        Self::Flower(crate::beds::DyeColor::Green),
        Self::Flower(crate::beds::DyeColor::Pink),
        Self::Dye(crate::beds::DyeColor::White),
        Self::Cape(crate::CapeItem {
            material: crate::ArmorMaterial::Leather,
            color: crate::beds::DyeColor::White,
        }),
        Self::Cape(crate::CapeItem {
            material: crate::ArmorMaterial::Iron,
            color: crate::beds::DyeColor::White,
        }),
        Self::Cape(crate::CapeItem {
            material: crate::ArmorMaterial::Gold,
            color: crate::beds::DyeColor::White,
        }),
        Self::Cape(crate::CapeItem {
            material: crate::ArmorMaterial::Diamond,
            color: crate::beds::DyeColor::White,
        }),
        Self::Cape(crate::CapeItem {
            material: crate::ArmorMaterial::Ruby,
            color: crate::beds::DyeColor::White,
        }),
        Self::Dust(crate::DustKind::Iron),
        Self::Dust(crate::DustKind::Gold),
        Self::Dust(crate::DustKind::Coal),
        Self::Dust(crate::DustKind::Diamond),
        Self::Dust(crate::DustKind::Salt),
        Self::Dust(crate::DustKind::Ruby),
        Self::Dye(crate::beds::DyeColor::Red),
        Self::Dye(crate::beds::DyeColor::Yellow),
        Self::Dye(crate::beds::DyeColor::Blue),
        Self::Dye(crate::beds::DyeColor::Orange),
        Self::Dye(crate::beds::DyeColor::Green),
        Self::Dye(crate::beds::DyeColor::Purple),
        Self::Animal(crate::creatures::AnimalProduct::Mutton),
        Self::Animal(crate::creatures::AnimalProduct::Beef),
        Self::Animal(crate::creatures::AnimalProduct::Pork),
        Self::Animal(crate::creatures::AnimalProduct::Fish),
        Self::Animal(crate::creatures::AnimalProduct::Wool),
        Self::Stick,
        Self::Tool(ToolKind::WoodPickaxe),
        Self::Tool(ToolKind::StonePickaxe),
        Self::Tool(ToolKind::StoneAxe),
        Self::Tool(ToolKind::StoneShovel),
        Self::Tool(ToolKind::IronPickaxe),
        Self::Mineral(crate::MineralItem::SaltCrystal),
        Self::Mineral(crate::MineralItem::RawIron),
        Self::Mineral(crate::MineralItem::Coal),
        Self::Mineral(crate::MineralItem::RawGold),
        Self::Mineral(crate::MineralItem::Diamond),
        Self::Mineral(crate::MineralItem::Ruby),
        Self::Mineral(crate::MineralItem::IronIngot),
        Self::Mineral(crate::MineralItem::GoldIngot),
        Self::Animal(crate::creatures::AnimalProduct::CookedBeef),
        Self::Animal(crate::creatures::AnimalProduct::CookedPork),
        Self::Animal(crate::creatures::AnimalProduct::CookedMutton),
        Self::Animal(crate::creatures::AnimalProduct::CookedFish),
        Self::Animal(crate::creatures::AnimalProduct::Clownfish),
        Self::Animal(crate::creatures::AnimalProduct::CookedClownfish),
        Self::Animal(crate::creatures::AnimalProduct::Leather),
        Self::Animal(crate::creatures::AnimalProduct::Honey),
        Self::Tool(ToolKind::WoodAxe),
        Self::Tool(ToolKind::WoodShovel),
        Self::Tool(ToolKind::WoodSword),
        Self::Tool(ToolKind::StoneSword),
        Self::Bellows(BellowsKind::Stone),
        Self::Bellows(BellowsKind::Iron),
        Self::Bellows(BellowsKind::Gold),
        Self::Bellows(BellowsKind::Ruby),
        Self::Armor(crate::ArmorItem {
            material: crate::ArmorMaterial::Leather,
            piece: crate::ArmorPiece::Helmet,
        }),
        Self::Armor(crate::ArmorItem {
            material: crate::ArmorMaterial::Leather,
            piece: crate::ArmorPiece::Chestplate,
        }),
        Self::Armor(crate::ArmorItem {
            material: crate::ArmorMaterial::Leather,
            piece: crate::ArmorPiece::Gloves,
        }),
        Self::Armor(crate::ArmorItem {
            material: crate::ArmorMaterial::Leather,
            piece: crate::ArmorPiece::Pants,
        }),
        Self::Armor(crate::ArmorItem {
            material: crate::ArmorMaterial::Leather,
            piece: crate::ArmorPiece::Boots,
        }),
        Self::Armor(crate::ArmorItem {
            material: crate::ArmorMaterial::Iron,
            piece: crate::ArmorPiece::Helmet,
        }),
        Self::Armor(crate::ArmorItem {
            material: crate::ArmorMaterial::Iron,
            piece: crate::ArmorPiece::Chestplate,
        }),
        Self::Armor(crate::ArmorItem {
            material: crate::ArmorMaterial::Iron,
            piece: crate::ArmorPiece::Gloves,
        }),
        Self::Armor(crate::ArmorItem {
            material: crate::ArmorMaterial::Iron,
            piece: crate::ArmorPiece::Pants,
        }),
        Self::Armor(crate::ArmorItem {
            material: crate::ArmorMaterial::Iron,
            piece: crate::ArmorPiece::Boots,
        }),
        Self::Armor(crate::ArmorItem {
            material: crate::ArmorMaterial::Gold,
            piece: crate::ArmorPiece::Helmet,
        }),
        Self::Armor(crate::ArmorItem {
            material: crate::ArmorMaterial::Gold,
            piece: crate::ArmorPiece::Chestplate,
        }),
        Self::Armor(crate::ArmorItem {
            material: crate::ArmorMaterial::Gold,
            piece: crate::ArmorPiece::Gloves,
        }),
        Self::Armor(crate::ArmorItem {
            material: crate::ArmorMaterial::Gold,
            piece: crate::ArmorPiece::Pants,
        }),
        Self::Armor(crate::ArmorItem {
            material: crate::ArmorMaterial::Gold,
            piece: crate::ArmorPiece::Boots,
        }),
        Self::Armor(crate::ArmorItem {
            material: crate::ArmorMaterial::Diamond,
            piece: crate::ArmorPiece::Helmet,
        }),
        Self::Armor(crate::ArmorItem {
            material: crate::ArmorMaterial::Diamond,
            piece: crate::ArmorPiece::Chestplate,
        }),
        Self::Armor(crate::ArmorItem {
            material: crate::ArmorMaterial::Diamond,
            piece: crate::ArmorPiece::Gloves,
        }),
        Self::Armor(crate::ArmorItem {
            material: crate::ArmorMaterial::Diamond,
            piece: crate::ArmorPiece::Pants,
        }),
        Self::Armor(crate::ArmorItem {
            material: crate::ArmorMaterial::Diamond,
            piece: crate::ArmorPiece::Boots,
        }),
        Self::Armor(crate::ArmorItem {
            material: crate::ArmorMaterial::Ruby,
            piece: crate::ArmorPiece::Helmet,
        }),
        Self::Armor(crate::ArmorItem {
            material: crate::ArmorMaterial::Ruby,
            piece: crate::ArmorPiece::Chestplate,
        }),
        Self::Armor(crate::ArmorItem {
            material: crate::ArmorMaterial::Ruby,
            piece: crate::ArmorPiece::Gloves,
        }),
        Self::Armor(crate::ArmorItem {
            material: crate::ArmorMaterial::Ruby,
            piece: crate::ArmorPiece::Pants,
        }),
        Self::Armor(crate::ArmorItem {
            material: crate::ArmorMaterial::Ruby,
            piece: crate::ArmorPiece::Boots,
        }),
        Self::Accessory(crate::AccessoryKind::Earrings),
        Self::Accessory(crate::AccessoryKind::Necklace),
        Self::Accessory(crate::AccessoryKind::Belt),
        Self::Accessory(crate::AccessoryKind::Socks),
        Self::WoodenBlock {
            material: 141,
            wood: WoodSpecies::Birch,
        },
        Self::WoodenBlock {
            material: 141,
            wood: WoodSpecies::Spruce,
        },
        Self::WoodenBlock {
            material: 141,
            wood: WoodSpecies::Cherry,
        },
        Self::WoodenBlock {
            material: 144,
            wood: WoodSpecies::Birch,
        },
        Self::WoodenBlock {
            material: 144,
            wood: WoodSpecies::Spruce,
        },
        Self::WoodenBlock {
            material: 144,
            wood: WoodSpecies::Cherry,
        },
        Self::WoodenBlock {
            material: 148,
            wood: WoodSpecies::Birch,
        },
        Self::WoodenBlock {
            material: 148,
            wood: WoodSpecies::Spruce,
        },
        Self::WoodenBlock {
            material: 148,
            wood: WoodSpecies::Cherry,
        },
        Self::WoodenBlock {
            material: 149,
            wood: WoodSpecies::Birch,
        },
        Self::WoodenBlock {
            material: 149,
            wood: WoodSpecies::Spruce,
        },
        Self::WoodenBlock {
            material: 149,
            wood: WoodSpecies::Cherry,
        },
        Self::WoodenBlock {
            material: 169,
            wood: WoodSpecies::Oak,
        },
        Self::WoodenBlock {
            material: 169,
            wood: WoodSpecies::Birch,
        },
        Self::WoodenBlock {
            material: 169,
            wood: WoodSpecies::Cherry,
        },
        Self::Apple,
        Self::AppleCore,
        Self::WoodenBlock {
            material: 196,
            wood: WoodSpecies::Birch,
        },
        Self::WoodenBlock {
            material: 196,
            wood: WoodSpecies::Spruce,
        },
        Self::WoodenBlock {
            material: 196,
            wood: WoodSpecies::Cherry,
        },
        Self::WoodenBlock {
            material: 202,
            wood: WoodSpecies::Birch,
        },
        Self::WoodenBlock {
            material: 202,
            wood: WoodSpecies::Spruce,
        },
        Self::WoodenBlock {
            material: 202,
            wood: WoodSpecies::Cherry,
        },
        Self::WoodenBlock {
            material: 203,
            wood: WoodSpecies::Birch,
        },
        Self::WoodenBlock {
            material: 203,
            wood: WoodSpecies::Spruce,
        },
        Self::WoodenBlock {
            material: 203,
            wood: WoodSpecies::Cherry,
        },
        Self::WoodenBlock {
            material: 214,
            wood: WoodSpecies::Oak,
        },
        Self::WoodenBlock {
            material: 214,
            wood: WoodSpecies::Birch,
        },
        Self::WoodenBlock {
            material: 214,
            wood: WoodSpecies::Cherry,
        },
        Self::WoodenBlock {
            material: 215,
            wood: WoodSpecies::Oak,
        },
        Self::WoodenBlock {
            material: 215,
            wood: WoodSpecies::Birch,
        },
        Self::WoodenBlock {
            material: 215,
            wood: WoodSpecies::Cherry,
        },
        Self::WoodenBlock {
            material: 216,
            wood: WoodSpecies::Oak,
        },
        Self::WoodenBlock {
            material: 216,
            wood: WoodSpecies::Birch,
        },
        Self::WoodenBlock {
            material: 216,
            wood: WoodSpecies::Cherry,
        },
        Self::WoodenBlock {
            material: 217,
            wood: WoodSpecies::Oak,
        },
        Self::WoodenBlock {
            material: 217,
            wood: WoodSpecies::Birch,
        },
        Self::WoodenBlock {
            material: 217,
            wood: WoodSpecies::Cherry,
        },
        Self::WoodenBlock {
            material: 267,
            wood: WoodSpecies::Oak,
        },
        Self::WoodenBlock {
            material: 267,
            wood: WoodSpecies::Birch,
        },
        Self::WoodenBlock {
            material: 267,
            wood: WoodSpecies::Cherry,
        },
        Self::WoodenBlock {
            material: 268,
            wood: WoodSpecies::Oak,
        },
        Self::WoodenBlock {
            material: 268,
            wood: WoodSpecies::Birch,
        },
        Self::WoodenBlock {
            material: 268,
            wood: WoodSpecies::Cherry,
        },
    ];
    pub fn can_harvest(self, material: u16) -> bool {
        if let Some(ore) = crate::OreKind::from_material(material) {
            let level = match self {
                Self::Tool(tool)
                    if tool.class() == Some(crate::block_properties::ToolClass::Pickaxe) =>
                {
                    tool.mining_level()
                }
                _ => 0,
            };
            return level >= ore.mining_level();
        }
        !matches!(material, 12..=13)
            || matches!(self, Self::Tool(tool) if tool.class() == Some(crate::block_properties::ToolClass::Pickaxe))
    }
    pub fn identifier(self) -> String {
        match self {
            Self::HeraldicCape { cape, heraldry } => format!(
                "{}#heraldry={:02X}{:02X}{:02X}:{}",
                Self::Cape(cape.undyed()).identifier(),
                heraldry.color[0],
                heraldry.color[1],
                heraldry.color[2],
                heraldry.icon.key()
            ),
            Self::FilledWateringCan { tool, units } => {
                format!("teramic::{}#water={units}", tool.slug())
            }
            Self::Apple => "teramic::apple".into(),
            Self::AppleCore => "teramic::apple_core".into(),
            Self::GlassBottle => "teramic::glass_bottle".into(),
            Self::WaterBottle => "teramic::water_bottle".into(),
            Self::Potion(id) => crate::brewing::recipe(id).map_or_else(
                || format!("teramic::invalid_potion_{id}"),
                |recipe| recipe.key.clone(),
            ),
            Self::Cape(cape) => {
                let id = format!("teramic::{}_cape", cape.material.slug());
                if cape.color == crate::beds::DyeColor::White {
                    id
                } else {
                    format!("{id}#{}", cape.color.code())
                }
            }
            Self::Dust(kind) => format!("teramic::{}", kind.slug()),
            Self::Crop(kind) => format!("teramic::{}", kind.slug()),
            Self::Flower(color) => format!("teramic::{}_flower", color.slug()),
            Self::Dye(crate::beds::DyeColor::Rgb(rgb)) => format!(
                "teramic::mixed_dye#{}",
                crate::beds::DyeColor::Rgb(rgb).code()
            ),
            Self::Dye(color) => format!("teramic::{}_dye", color.slug()),
            Self::DyedBed { material, color } => {
                format!("{}#{}", Self::Block(material).identifier(), color.code())
            }
            Self::TintedBlock {
                material,
                wood,
                color,
            } => format!(
                "{}#{}",
                Self::for_wood(material, wood).identifier(),
                crate::beds::DyeColor::Rgb(color).code()
            ),
            Self::WoodenBlock { material, wood } => {
                format!("{}#{}", Self::Block(material).identifier(), wood.slug())
            }
            Self::Tool(tool) => format!("teramic::{}", tool.slug()),
            Self::Stick => "teramic::stick".into(),
            Self::Mineral(mineral) => format!("teramic::{}", mineral.slug()),
            Self::Animal(product) => format!("teramic::{}", product.slug()),
            Self::Bellows(kind) => format!("teramic::{}", kind.slug()),
            Self::Armor(armor) => format!("teramic::{}", armor.slug()),
            Self::Accessory(kind) => format!("teramic::{}", kind.slug()),
            Self::Block(id) => terra_voxel::MaterialId::from_raw(id).map_or_else(
                || format!("teramic::custom_{id}"),
                |m| m.identifier().to_owned(),
            ),
            Self::Prop(kind) => format!("teramic::{}", kind.slug()),
        }
    }
    pub fn valid(self) -> bool {
        match self {
            Self::HeraldicCape { cape, heraldry } => {
                cape.color == crate::beds::DyeColor::Rgb(heraldry.color) && heraldry.icon.valid()
            }
            Self::FilledWateringCan { tool, units } => {
                units > 0 && units <= tool.watering_capacity()
            }
            Self::Potion(id) => crate::brewing::recipe(id).is_some(),
            Self::Prop(ResourcePropKind::AppleFruit) => false,
            Self::GlassBottle | Self::WaterBottle | Self::Apple | Self::AppleCore => true,
            Self::Dye(_) => true,
            Self::Armor(_) | Self::Accessory(_) | Self::Cape(_) | Self::Dust(_) | Self::Crop(_) => {
                true
            }
            Self::Flower(color) => crate::beds::DyeColor::FLOWER_COLORS.contains(&color),
            Self::DyedBed { material, .. } => crate::beds::is_bed(material),
            Self::TintedBlock { material, wood, .. } => {
                crate::tints::supports(material) && (material == 148 || wood == WoodSpecies::Oak)
            }
            Self::WoodenBlock { material, wood } => {
                Self::supports_wood(material)
                    && wood != Self::Block(material).wood_species().unwrap_or_default()
            }
            Self::Block(id) => terra_voxel::MaterialId::from_raw(id)
                .is_some_and(|material| material != terra_voxel::MaterialId::Air),
            Self::Prop(_)
            | Self::Tool(_)
            | Self::Stick
            | Self::Mineral(_)
            | Self::Animal(_)
            | Self::Bellows(_) => true,
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ItemStack {
    pub item: ItemKind,
    pub count: u16,
}
impl ItemStack {
    pub const EMPTY: Self = Self {
        item: ItemKind::Block(0),
        count: 0,
    };
    pub fn valid(self) -> bool {
        self.item.valid() && (1..=self.item.max_stack()).contains(&self.count)
    }
}
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Inventory {
    pub slots: Vec<ItemStack>,
    #[serde(default)]
    pub cursor: Option<ItemStack>,
    #[serde(default)]
    pub revision: u64,
}
impl Inventory {
    /// Older saves had up to four backpack rows. They remain readable until
    /// the owner joins and the server can recover excess items at their feet.
    pub fn valid_saved_player(&self) -> bool {
        self.slots.len() <= 45
            && self.cursor.is_none_or(ItemStack::valid)
            && self
                .slots
                .iter()
                .all(|s| *s == ItemStack::EMPTY || s.valid())
    }
    pub fn valid(&self) -> bool {
        self.slots.len() <= INVENTORY_SLOTS
            && self.cursor.is_none_or(ItemStack::valid)
            && self
                .slots
                .iter()
                .all(|s| *s == ItemStack::EMPTY || s.valid())
    }
    pub fn slot(&self, index: usize) -> Option<ItemStack> {
        self.slots.get(index).copied().filter(|s| s.valid())
    }
    pub fn occupied_slots(&self) -> usize {
        self.slots.iter().filter(|s| s.valid()).count()
    }
    pub fn has_capacity(&self, item: ItemKind) -> bool {
        self.occupied_slots() < INVENTORY_SLOTS
            || self
                .slots
                .iter()
                .any(|s| s.item == item && s.count < item.max_stack())
    }
    fn changed(&mut self) {
        self.revision = self.revision.saturating_add(1);
    }
    fn set(&mut self, index: usize, stack: ItemStack) {
        self.slots
            .resize(self.slots.len().max(index + 1), ItemStack::EMPTY);
        self.slots[index] = stack;
    }
    pub fn creative_set(&mut self, slot: usize, stack: ItemStack) -> bool {
        if slot >= INVENTORY_SLOTS || !stack.valid() || self.slot(slot) == Some(stack) {
            return false;
        }
        self.set(slot, stack);
        self.changed();
        true
    }
    /// Transform one selected item, retaining the rest of its legacy stack atomically.
    pub fn transform_one(&mut self, slot: usize, replacement: ItemKind) -> bool {
        if slot >= INVENTORY_SLOTS {
            return false;
        }
        let Some(source) = self.slot(slot) else {
            return false;
        };
        if !replacement.valid() || source.item == replacement || self.revision == u64::MAX {
            return false;
        }
        let mut next = self.clone();
        next.set(
            slot,
            ItemStack {
                item: replacement,
                count: 1,
            },
        );
        if source.count > 1 && next.insert(source.item, source.count - 1) != 0 {
            return false;
        }
        next.revision = self.revision + 1;
        *self = next;
        true
    }
    /// Stable slot indices: merge equal items, or swap complete unlike stacks.
    pub fn move_slot(&mut self, from: usize, to: usize, count: u16) -> bool {
        if from == to || from >= INVENTORY_SLOTS || to >= INVENTORY_SLOTS || count == 0 {
            return false;
        }
        let Some(mut source) = self.slot(from) else {
            return false;
        };
        if count > source.count {
            return false;
        }
        let target = self.slot(to);
        if let Some(target) = target
            && target.item != source.item
        {
            if count != source.count {
                return false;
            }
            self.set(from, target);
            self.set(to, source);
        } else {
            let existing = target.map_or(0, |s| s.count);
            let moved = count.min(source.item.max_stack() - existing);
            if moved == 0 {
                return false;
            }
            source.count -= moved;
            self.set(
                to,
                ItemStack {
                    item: source.item,
                    count: existing + moved,
                },
            );
            self.set(
                from,
                if source.count == 0 {
                    ItemStack::EMPTY
                } else {
                    source
                },
            );
        }
        self.changed();
        true
    }
    pub fn count(&self, item: ItemKind) -> u16 {
        self.slots
            .iter()
            .filter(|s| s.item == item)
            .map(|s| s.count)
            .sum()
    }
    /// Returns the unaccepted amount; full inventories never destroy resources.
    pub fn insert(&mut self, item: ItemKind, mut count: u16) -> u16 {
        if !item.valid() {
            return count;
        }
        let original = count;
        for stack in self
            .slots
            .iter_mut()
            .filter(|s| s.item == item && s.valid())
        {
            let amount = count.min(item.max_stack() - stack.count);
            stack.count += amount;
            count -= amount;
        }
        for slot in 0..INVENTORY_SLOTS {
            if count == 0 {
                break;
            }
            if self.slot(slot).is_some() {
                continue;
            }
            let amount = count.min(item.max_stack());
            self.set(
                slot,
                ItemStack {
                    item,
                    count: amount,
                },
            );
            count -= amount;
        }
        if count != original {
            self.changed();
        }
        count
    }
    pub fn take_one(&mut self, item: ItemKind) -> bool {
        let Some(index) = self
            .slots
            .iter()
            .position(|s| s.item == item && s.count > 0)
        else {
            return false;
        };
        self.take_from(index, item)
    }
    pub fn take_from(&mut self, index: usize, item: ItemKind) -> bool {
        if self.slot(index).is_none_or(|stack| stack.item != item) {
            return false;
        }
        self.slots[index].count -= 1;
        if self.slots[index].count == 0 {
            self.slots[index] = ItemStack::EMPTY;
        }
        self.changed();
        true
    }
    pub fn craft(&mut self, slots: [Option<u8>; 4]) -> bool {
        crate::crafting::RecipeBook::builtins().craft_cells(
            self,
            slots,
            crate::crafting::CraftDestination::Inventory,
        )
    }
}

/// Each recipe cell consumes one resource, including repeated references to a stack.
pub fn crafting_preview(bag: &Inventory, cells: [Option<u8>; 4]) -> Option<ItemStack> {
    let book = crate::crafting::RecipeBook::builtins();
    let (key, _) = book.grid_match(bag, cells)?;
    Some(book.get(key)?.output)
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorldDrop {
    pub id: u64,
    pub position: [f64; 3],
    pub stack: ItemStack,
    #[serde(default)]
    pub velocity: [f32; 3],
    #[serde(default)]
    pub age_seconds: f32,
    /// Quaternion in XYZW order, shared by snapshots and the item renderer.
    #[serde(default = "identity_rotation")]
    pub rotation: [f32; 4],
    #[serde(default)]
    pub angular_velocity: [f32; 3],
    #[serde(default)]
    pub grounded: bool,
    #[serde(default)]
    pub pickup_delay_seconds: f32,
}
fn identity_rotation() -> [f32; 4] {
    [0.0, 0.0, 0.0, 1.0]
}
impl WorldDrop {
    pub fn valid(&self) -> bool {
        self.id != 0
            && self.stack.valid()
            && self
                .velocity
                .iter()
                .all(|v| v.is_finite() && v.abs() <= 100.0)
            && self.age_seconds.is_finite()
            && self.rotation.iter().all(|v| v.is_finite())
            && (self.rotation.iter().map(|v| v * v).sum::<f32>() - 1.0).abs() < 0.01
            && self
                .angular_velocity
                .iter()
                .all(|v| v.is_finite() && v.abs() <= 100.0)
            && self.pickup_delay_seconds.is_finite()
            && (0.0..=10.0).contains(&self.pickup_delay_seconds)
            && (0.0..=DROP_LIFETIME_SECONDS).contains(&self.age_seconds)
            && self
                .position
                .iter()
                .all(|v| v.is_finite() && v.abs() <= 1_000_000.0)
    }
}
#[derive(Debug, Default, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PlayerStats {
    pub game_mode: Option<GameMode>,
    pub blocks_broken: u64,
    pub blocks_placed: u64,
    pub props_harvested: u64,
    pub deaths: u64,
    pub survival: crate::SurvivalVitals,
    pub equipment: crate::EquipmentSlots,
}

#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct GameplayState {
    pub trains: crate::trains::TrainRegistry,
    pub trees: crate::trees::TreeRegistry,
    pub kingdoms: crate::kingdom::KingdomRegistry,
    pub creatures: crate::creatures::CreaturePopulation,
    pub animal_bonds: BTreeMap<u64, crate::creatures::taming::AnimalBond>,
    pub pet_orders: BTreeMap<u64, crate::creatures::taming::CompanionMode>,
    pub animal_families: BTreeMap<u64, crate::creatures::breeding::AnimalFamily>,
    pub next_animal_birth: u32,
    pub spider_dens: BTreeMap<u64, crate::creatures::taming::SpiderDen>,
    pub inventories: BTreeMap<PlayerUuid, Inventory>,
    pub player_stats: BTreeMap<PlayerUuid, PlayerStats>,
    pub drops: Vec<WorldDrop>,
    pub next_drop: u64,
    pub revision: u64,
}
impl GameplayState {
    pub fn valid(&self) -> bool {
        let mut ids = HashSet::new();
        self.trains.valid()
            && self.trees.valid()
            && self.kingdoms.valid()
            && self.creatures.valid()
            && self.pet_orders.len() <= crate::creatures::MAX_CREATURES
            && self.pet_orders.keys().all(|id| {
                self.creatures.owners.contains_key(id)
                    && self
                        .creatures
                        .animals
                        .get(id)
                        .is_some_and(|a| crate::creatures::taming::tameable(a.kind))
            })
            && self.animal_families.len() <= crate::creatures::MAX_CREATURES
            && self.animal_families.iter().all(|(id, family)| {
                family.valid(*id)
                    && self.creatures.animals.get(id).is_some_and(|animal| {
                        crate::creatures::breeding::species(animal.kind)
                            && family.parents.is_none_or(|parents| {
                                crate::creatures::breeding::is_birth_id(*id)
                                    && parents.into_iter().all(|parent| {
                                        self.creatures
                                            .animals
                                            .get(&parent)
                                            .is_some_and(|a| a.kind == animal.kind)
                                    })
                            })
                    })
            })
            && self.creatures.animals.values().all(|animal| {
                !crate::creatures::breeding::is_birth_id(animal.id)
                    || crate::creatures::breeding::birth_serial(animal.id)
                        .is_some_and(|serial| serial <= self.next_animal_birth)
                        && self
                            .animal_families
                            .get(&animal.id)
                            .is_some_and(|family| family.parents.is_some())
                        && (self.creatures.owners.contains_key(&animal.id)
                            || animal.kind == crate::creatures::CreatureKind::Bee
                                && self.creatures.ecology.bees.contains_key(&animal.id))
            })
            && self.spider_dens.len() <= crate::creatures::MAX_CREATURES
            && self.spider_dens.iter().all(|(id, den)| {
                den.valid()
                    && self
                        .creatures
                        .animals
                        .get(id)
                        .is_some_and(|a| a.kind.is_spider())
            })
            && self.animal_bonds.len() <= crate::creatures::MAX_CREATURES
            && self.animal_bonds.iter().all(|(id, bond)| {
                bond.valid()
                    && self
                        .creatures
                        .animals
                        .get(id)
                        .is_some_and(|a| crate::creatures::taming::tameable(a.kind))
                    && if bond.trust == 100 {
                        self.creatures.owners.get(id) == Some(&bond.player)
                    } else {
                        !self.creatures.owners.contains_key(id)
                    }
            })
            && self.inventories.len() <= 4096
            && self.player_stats.len() <= 4096
            && self.player_stats.iter().all(|(id, stats)| {
                !id.is_nil() && stats.survival.valid() && stats.equipment.valid()
            })
            && self
                .inventories
                .iter()
                .all(|(id, bag)| !id.is_nil() && bag.valid_saved_player())
            && self.drops.len() <= MAX_WORLD_DROPS
            && self
                .drops
                .iter()
                .all(|d| d.valid() && d.id <= self.next_drop && ids.insert(d.id))
    }
    pub fn can_drop(&self) -> bool {
        self.drops.len() < MAX_WORLD_DROPS && self.next_drop < u64::MAX
    }
    /// Recover old inventory rows atomically, merging into the 27 accessible
    /// slots first and spawning only the remainder at the resolved join position.
    pub fn recover_player_inventory(&mut self, player: PlayerUuid, position: [f64; 3]) -> bool {
        let Some(original) = self.inventories.get(&player) else {
            return true;
        };
        if !original.valid_saved_player() {
            return false;
        }
        if original.slots.len() <= INVENTORY_SLOTS {
            return true;
        }
        let mut inventory = original.clone();
        let excess = inventory.slots.split_off(INVENTORY_SLOTS);
        let mut remaining = Vec::new();
        for stack in excess.into_iter().filter(|stack| stack.valid()) {
            let count = inventory.insert(stack.item, stack.count);
            if count > 0 {
                remaining.push(ItemStack { count, ..stack });
            }
        }
        if self.drops.len() + remaining.len() > MAX_WORLD_DROPS
            || self.next_drop.checked_add(remaining.len() as u64).is_none()
            || position
                .iter()
                .any(|v| !v.is_finite() || v.abs() > 1_000_000.0)
        {
            return false;
        }
        inventory.revision = original.revision.saturating_add(1);
        self.inventories.insert(player, inventory);
        for stack in remaining {
            let accepted = self.drop_stack(position, stack, [0.0, 1.8, 0.0], 0.35);
            debug_assert!(accepted);
        }
        self.revision = self.revision.saturating_add(1);
        true
    }
    /// Transfer every owned stack to the death position as one preflighted
    /// operation. A full world-drop budget leaves ownership intact for retry.
    pub fn drop_death_inventory(
        &mut self,
        player: PlayerUuid,
        equipment: &mut crate::EquipmentSlots,
        position: [f64; 3],
    ) -> bool {
        let bag = self.inventories.get(&player).cloned().unwrap_or_default();
        if player.is_nil()
            || !bag.valid_saved_player()
            || !equipment.valid()
            || position
                .iter()
                .any(|v| !v.is_finite() || v.abs() > 1_000_000.0)
        {
            return false;
        }
        let stacks: Vec<_> = bag
            .slots
            .iter()
            .copied()
            .filter(|stack| stack.valid())
            .chain(bag.cursor)
            .chain(equipment.slots.into_iter().filter(|stack| stack.valid()))
            .collect();
        if stacks.is_empty() {
            return true;
        }
        let Some(revision) = bag.revision.checked_add(1) else {
            return false;
        };
        if (!self.inventories.contains_key(&player) && self.inventories.len() >= 4096)
            || self.drops.len() + stacks.len() > MAX_WORLD_DROPS
            || self.next_drop.checked_add(stacks.len() as u64).is_none()
            || self.revision.checked_add(stacks.len() as u64 + 1).is_none()
        {
            return false;
        }
        self.inventories.insert(
            player,
            Inventory {
                revision,
                ..Default::default()
            },
        );
        *equipment = Default::default();
        if let Some(stats) = self.player_stats.get_mut(&player) {
            stats.equipment = *equipment;
        }
        for stack in stacks {
            let accepted = self.drop_stack(position, stack, [0.0, 1.8, 0.0], 2.0);
            debug_assert!(accepted, "death drop was preflighted");
        }
        self.revision += 1;
        true
    }

    pub fn drop_one(&mut self, position: [f64; 3], item: ItemKind) -> bool {
        self.drop_stack(
            position,
            ItemStack { item, count: 1 },
            [0.0, 1.8, 0.0],
            0.35,
        )
    }
    pub fn drop_stack(
        &mut self,
        position: [f64; 3],
        stack: ItemStack,
        velocity: [f32; 3],
        pickup_delay_seconds: f32,
    ) -> bool {
        if !self.can_drop()
            || !stack.valid()
            || velocity.iter().any(|v| !v.is_finite() || v.abs() > 100.0)
            || !pickup_delay_seconds.is_finite()
            || !(0.0..=10.0).contains(&pickup_delay_seconds)
            || position
                .iter()
                .any(|v| !v.is_finite() || v.abs() > 1_000_000.0)
        {
            return false;
        }
        self.next_drop += 1;
        self.drops.push(WorldDrop {
            id: self.next_drop,
            position,
            stack,
            velocity,
            age_seconds: 0.0,
            rotation: identity_rotation(),
            angular_velocity: [2.4, (self.next_drop % 11) as f32 * 0.7 + 0.4, -1.7],
            grounded: false,
            pickup_delay_seconds,
        });
        self.revision = self.revision.saturating_add(1);
        true
    }
    pub fn pickup(&mut self, player: PlayerUuid, id: u64) -> bool {
        if player.is_nil()
            || (!self.inventories.contains_key(&player) && self.inventories.len() >= 4096)
        {
            return false;
        }
        let Some(index) = self.drops.iter().position(|d| d.id == id) else {
            return false;
        };
        let stack = self.drops[index].stack;
        let remaining = self
            .inventories
            .entry(player)
            .or_default()
            .insert(stack.item, stack.count);
        if remaining == stack.count {
            return false;
        }
        if remaining == 0 {
            self.drops.remove(index);
        } else {
            self.drops[index].stack.count = remaining;
        }
        self.revision = self.revision.saturating_add(1);
        true
    }
    pub fn consume(&mut self, player: PlayerUuid, item: ItemKind) -> bool {
        self.consume_from(player, item, None)
    }
    pub fn consume_from(&mut self, player: PlayerUuid, item: ItemKind, slot: Option<u8>) -> bool {
        if self
            .inventories
            .get_mut(&player)
            .is_some_and(|bag| match slot {
                Some(slot) => bag.take_from(usize::from(slot), item),
                None => bag.take_one(item),
            })
        {
            self.revision = self.revision.saturating_add(1);
            true
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn placeable_salt_segments_do_not_shadow_the_existing_harvest_resource_identifier() {
        assert_ne!(
            super::ItemKind::Block(51).identifier(),
            super::ItemKind::Mineral(crate::MineralItem::SaltCrystal).identifier()
        );
    }
    #[test]
    fn wool_block_does_not_shadow_the_existing_animal_wool_identifier_in_the_item_catalog() {
        let animal = super::ItemKind::Animal(crate::creatures::AnimalProduct::Wool);
        let block = super::ItemKind::Block(103);
        assert_eq!(animal.identifier(), "teramic::wool");
        assert_eq!(block.identifier(), "teramic::wool_block");
        assert_ne!(animal.identifier(), block.identifier());
        for (identifier, expected) in [("teramic::wool", animal), ("teramic::wool_block", block)] {
            let matches: Vec<_> = (1..=terra_voxel::MaterialId::LAST)
                .map(super::ItemKind::Block)
                .chain(super::ItemKind::EXTRA)
                .filter(|item| item.identifier() == identifier)
                .collect();
            assert_eq!(matches, vec![expected]);
        }
    }
    use super::*;
    #[test]
    fn slot_indices_survive_moves_merges_empty_cells_and_legacy_toml() {
        let mut bag: Inventory =
            toml::from_str("slots = [{item = {block = 3}, count = 64}]").unwrap();
        assert_eq!(bag.revision, 0);
        assert!(bag.move_slot(0, 8, 32));
        assert_eq!(bag.slot(8).unwrap().count, 32);
        assert!(bag.move_slot(0, 8, 32));
        assert!(bag.slot(0).is_none());
        assert_eq!(bag.slot(8).unwrap().count, 64);
        assert_eq!(bag.insert(ItemKind::Block(3), 100), 0);
        assert_eq!(bag.slot(8).unwrap().count, 128);
        assert_eq!(bag.slot(0).unwrap().count, 36);
        assert!(bag.move_slot(0, 26, 36));
        let encoded = toml::to_string(&bag).unwrap();
        assert_eq!(toml::from_str::<Inventory>(&encoded).unwrap(), bag);
        assert!(bag.valid());
        assert!(!bag.move_slot(26, INVENTORY_SLOTS, 1));
        assert!(!bag.move_slot(26, 0, 37));
    }

    #[test]
    fn crafting_consumes_actual_resources_and_is_atomic_when_full() {
        let mut bag = Inventory::default();
        bag.insert(ItemKind::Block(6), 2);
        assert_eq!(
            crafting_preview(&bag, [Some(0), None, None, None])
                .unwrap()
                .count,
            4
        );
        assert!(bag.craft([Some(0), None, None, None]));
        assert_eq!(bag.count(ItemKind::Block(6)), 1);
        assert_eq!(bag.count(ItemKind::Block(16)), 4);
        let mut sand = Inventory::default();
        sand.insert(ItemKind::Block(4), 3);
        assert!(crafting_preview(&sand, [Some(0); 4]).is_none());
        sand.insert(ItemKind::Block(4), 1);
        assert!(sand.craft([Some(0); 4]));
        assert_eq!(sand.count(ItemKind::Block(4)), 0);
        assert_eq!(sand.count(ItemKind::Block(11)), 1);
        let mut full = Inventory::default();
        for slot in 0..INVENTORY_SLOTS {
            full.creative_set(
                slot,
                ItemStack {
                    item: ItemKind::Block(3),
                    count: 128,
                },
            );
        }
        full.creative_set(
            0,
            ItemStack {
                item: ItemKind::Block(6),
                count: 2,
            },
        );
        let before = full.clone();
        assert!(!full.craft([Some(0), None, None, None]));
        assert_eq!(full, before);
    }

    #[test]
    fn old_drop_snapshots_get_stationary_unexpired_defaults() {
        let drop: WorldDrop =
            toml::from_str("id=1\nposition=[1.0,2.0,3.0]\n[stack]\nitem={block=3}\ncount=1")
                .unwrap();
        assert_eq!(drop.velocity, [0.0; 3]);
        assert_eq!(drop.age_seconds, 0.0);
        assert_eq!(drop.rotation, [0.0, 0.0, 0.0, 1.0]);
        assert_eq!(drop.angular_velocity, [0.0; 3]);
        assert_eq!(drop.pickup_delay_seconds, 0.0);
        assert!(drop.valid());
    }
    #[test]
    fn all_registered_block_items_roundtrip_without_changing_ids() {
        let mut state = GameplayState::default();
        for id in 1..=terra_voxel::MaterialId::LAST {
            let uuid = PlayerUuid::from_u128(u128::from((id - 1) / INVENTORY_SLOTS as u16) + 1);
            assert!(state.drop_one([1.0, 2.0, 3.0], ItemKind::Block(id)));
            assert!(state.pickup(uuid, u64::from(id)));
        }
        assert!(state.valid());
        assert!(!ItemKind::Block(0).valid());
        assert!(!ItemKind::Block(terra_voxel::MaterialId::LAST + 1).valid());
        let encoded = toml::to_string(&state).unwrap();
        let restored: GameplayState = toml::from_str(&encoded).unwrap();
        assert_eq!(state, restored);
    }
    #[test]
    fn wooden_mechanics_keep_canonical_oak_and_persistent_species_variants() {
        let mut identifiers = std::collections::BTreeSet::new();
        for material in [141, 144, 148, 149, 196, 202, 203] {
            assert!(ItemKind::supports_wood(material));
            for wood in [
                WoodSpecies::Oak,
                WoodSpecies::Birch,
                WoodSpecies::Spruce,
                WoodSpecies::Cherry,
            ] {
                let item = ItemKind::for_wood(material, wood);
                assert!(item.valid());
                assert_eq!(item.block_material(), Some(material));
                assert_eq!(item.wood_species(), Some(wood));
                assert!(identifiers.insert(item.identifier()));
                if wood == WoodSpecies::Oak {
                    assert_eq!(item, ItemKind::Block(material));
                    assert!(!ItemKind::WoodenBlock { material, wood }.valid());
                } else {
                    assert_eq!(
                        ItemKind::EXTRA
                            .iter()
                            .filter(|entry| **entry == item)
                            .count(),
                        1
                    );
                    assert_eq!(
                        item.identifier(),
                        format!("{}#{}", ItemKind::Block(material).identifier(), wood.slug())
                    );
                }
                let stack = ItemStack { item, count: 128 };
                assert_eq!(
                    toml::from_str::<ItemStack>(&toml::to_string(&stack).unwrap()).unwrap(),
                    stack
                );
            }
        }
        for material in [
            4, 142, 143, 145, 146, 147, 150, 151, 152, 153, 197, 198, 199, 200, 201, 204, 205,
        ] {
            assert!(!ItemKind::supports_wood(material));
            assert!(
                !ItemKind::WoodenBlock {
                    material,
                    wood: WoodSpecies::Birch
                }
                .valid()
            );
            assert_eq!(
                ItemKind::for_wood(material, WoodSpecies::Birch),
                ItemKind::Block(material)
            );
        }
    }
    #[test]
    fn basin_defaults_to_spruce_and_keeps_all_other_species_distinct() {
        assert_eq!(
            ItemKind::Block(169).wood_species(),
            Some(WoodSpecies::Spruce)
        );
        for wood in [
            WoodSpecies::Oak,
            WoodSpecies::Birch,
            WoodSpecies::Spruce,
            WoodSpecies::Cherry,
        ] {
            let item = ItemKind::for_wood(169, wood);
            assert!(item.valid());
            assert_eq!(item.wood_species(), Some(wood));
            assert_eq!(item == ItemKind::Block(169), wood == WoodSpecies::Spruce);
            assert_eq!(
                toml::from_str::<ItemStack>(
                    &toml::to_string(&ItemStack { item, count: 1 }).unwrap()
                )
                .unwrap()
                .item,
                item
            );
            if wood != WoodSpecies::Spruce {
                assert_eq!(
                    ItemKind::EXTRA
                        .iter()
                        .filter(|&&entry| entry == item)
                        .count(),
                    1
                );
            }
        }
        assert!(
            !ItemKind::WoodenBlock {
                material: 169,
                wood: WoodSpecies::Spruce
            }
            .valid()
        );
    }
    #[test]
    fn full_inventory_keeps_drop_and_partial_pickup_keeps_remainder() {
        let uuid = PlayerUuid::new_v4();
        let item = ItemKind::Block(3);
        let mut state = GameplayState::default();
        let mut bag = Inventory::default();
        let capacity = INVENTORY_SLOTS as u16 * STACK_LIMIT;
        assert_eq!(bag.insert(item, capacity), 0);
        state.inventories.insert(uuid, bag);
        assert!(state.drop_one([0.0; 3], item));
        let id = state.drops[0].id;
        assert!(!state.pickup(uuid, id));
        assert!(state.consume(uuid, item));
        state.drops[0].stack.count = 2;
        assert!(state.pickup(uuid, id));
        assert_eq!(state.drops[0].stack.count, 1);
        assert_eq!(state.inventories[&uuid].count(item), capacity);
        assert!(!state.pickup(uuid, id));
        assert!(state.valid());
    }
    #[test]
    fn duplicate_pickup_cannot_duplicate_resources() {
        let uuid = PlayerUuid::new_v4();
        let mut state = GameplayState::default();
        let item = ItemKind::Prop(ResourcePropKind::Flower);
        assert!(state.drop_one([2.0, 3.0, 4.0], item));
        assert!(state.pickup(uuid, 1));
        assert!(!state.pickup(uuid, 1));
        assert_eq!(state.inventories[&uuid].count(item), 1);
        assert!(state.drop_one([2.0, 3.0, 4.0], item));
        assert_eq!(state.drops[0].id, 2);
    }
}

#[cfg(test)]
mod death_inventory_tests {
    use super::*;
    #[test]
    fn death_transfer_conserves_cursor_inventory_and_equipment_without_duplication() {
        let player = PlayerUuid::new_v4();
        let mut state = GameplayState::default();
        let mut bag = Inventory::default();
        bag.insert(ItemKind::Block(3), 17);
        bag.cursor = Some(ItemStack {
            item: ItemKind::Block(2),
            count: 4,
        });
        let before_revision = bag.revision;
        state.inventories.insert(player, bag);
        let mut equipment = crate::EquipmentSlots::default();
        let armor = ItemKind::Armor(crate::ArmorItem {
            material: crate::ArmorMaterial::Iron,
            piece: crate::ArmorPiece::Chestplate,
        });
        equipment.set(crate::EquipmentSlot::Chestplate, Some(armor));
        assert!(state.drop_death_inventory(player, &mut equipment, [2.5, 3.0, 4.5]));
        assert_eq!(state.inventories[&player].occupied_slots(), 0);
        assert!(state.inventories[&player].cursor.is_none());
        assert!(state.inventories[&player].revision > before_revision);
        assert_eq!(equipment, crate::EquipmentSlots::default());
        assert_eq!(
            state
                .drops
                .iter()
                .map(|drop| drop.stack.count as u32)
                .sum::<u32>(),
            22
        );
        assert!(state.drops.iter().any(|drop| drop.stack.item == armor));
        assert!(
            state
                .drops
                .iter()
                .all(|drop| drop.position == [2.5, 3.0, 4.5])
        );
        let committed = state.clone();
        assert!(state.drop_death_inventory(player, &mut equipment, [2.5, 3.0, 4.5]));
        assert_eq!(state, committed);
        assert!(state.valid());
    }
    #[test]
    fn exhausted_drop_ids_defer_the_entire_death_transfer_without_losing_items() {
        let player = PlayerUuid::new_v4();
        let mut state = GameplayState::default();
        let mut bag = Inventory::default();
        bag.insert(ItemKind::Block(3), 2);
        bag.cursor = Some(ItemStack {
            item: ItemKind::Block(2),
            count: 1,
        });
        state.inventories.insert(player, bag);
        state.next_drop = u64::MAX - 1;
        let before = state.clone();
        let mut equipment = crate::EquipmentSlots::default();
        assert!(!state.drop_death_inventory(player, &mut equipment, [0.0; 3]));
        assert_eq!(state, before);
        assert_eq!(equipment, crate::EquipmentSlots::default());
    }
}
