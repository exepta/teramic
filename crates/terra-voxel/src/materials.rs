use serde::{Deserialize, Serialize};

#[repr(u16)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MaterialId {
    Air = 0,
    Grass = 1,
    Dirt = 2,
    Stone = 3,
    Sand = 4,
    Snow = 5,
    Wood = 6,
    Leaves = 7,
    AutumnLeaves = 8,
    SilverLeaves = 9,
    WeatheredWood = 10,
    Sandstone = 11,
    SmoothStone = 12,
    MossyStone = 13,
    SolidWater = 14,
    Glass = 15,
    OakPlanks = 16,
    BirchPlanks = 17,
    SprucePlanks = 18,
    Clay = 19,
    MushroomStem = 20,
    PurpleMushroomCap = 21,
    RedMushroomCap = 22,
    YellowMushroomCap = 23,
    GreenMushroomCap = 24,
    BlueMushroomCap = 25,
    WhiteMushroomCap = 26,
    FluidWater = 27,
    Workbench = 28,
    Chest = 29,
    SaltOre = 30,
    IronOre = 31,
    CoalOre = 32,
    GoldOre = 33,
    DiamondOre = 34,
    RubyOre = 35,
    SpruceLog = 36,
    Dripstone = 37,
    SaltFormation = 38,
    DripstoneFormation = 39,
    Torch = 40,
    Bedrock = 41,
    Gravel = 42,
    StructureStone = 43,
    CoralRed = 44,
    CoralBlue = 45,
    CoralYellow = 46,
    CoralPurple = 47,
    Mud = 48,
    Ice = 49,
    Seagrass = 50,
    SaltCrystal = 51,
    DripstoneTip = 52,
    IceCrystal = 53,
    CaveVine = 54,
    CaveRoot = 55,
    CaveMushroom = 56,
    SaltStone = 57,
    PinkSaltStone = 58,
    Furnace = 59,
    ChimneyStraight = 60,
    ChimneyElbow = 61,
    ChimneyHorizontal = 62,
    ChimneyBendUp = 63,
    ChimneyBendDown = 64,
    OakDoor = 65,
    BirchDoor = 66,
    SpruceDoor = 67,
    OakWindow = 68,
    BirchWindow = 69,
    SpruceWindow = 70,
    OakBed = 71,
    BirchBed = 72,
    SpruceBed = 73,
    OakStairs = 74,
    BirchStairs = 75,
    SpruceStairs = 76,
    StoneStairs = 77,
    DirtStairs = 78,
    FineSandStairs = 79,
    GlassStairs = 80,
    IceStairs = 81,
    OakSlab = 82,
    BirchSlab = 83,
    SpruceSlab = 84,
    StoneSlab = 85,
    DirtSlab = 86,
    FineSandSlab = 87,
    GlassSlab = 88,
    IceSlab = 89,
    OakFence = 90,
    BirchFence = 91,
    SpruceFence = 92,
    StoneFence = 93,
    DirtFence = 94,
    FineSandFence = 95,
    GlassFence = 96,
    IceFence = 97,
    SnowLayer = 98,
    Crusher = 99,
    HandCrank = 100,
    Farmland = 101,
    Path = 102,
    Wool = 103,
    WoolStairs = 104,
    WoolSlab = 105,
    WhiteCarpet = 106,
    RedCarpet = 107,
    YellowCarpet = 108,
    BlueCarpet = 109,
    OrangeCarpet = 110,
    GreenCarpet = 111,
    PurpleCarpet = 112,
    BlackCarpet = 113,
    LimeCarpet = 114,
    PinkCarpet = 115,
    GoldCarpet = 116,
    SilverCarpet = 117,
    LightBlueCarpet = 118,
    RedWool = 119,
    YellowWool = 120,
    BlueWool = 121,
    OrangeWool = 122,
    GreenWool = 123,
    PurpleWool = 124,
    BlackWool = 125,
    LimeWool = 126,
    PinkWool = 127,
    GoldWool = 128,
    SilverWool = 129,
    LightBlueWool = 130,
    FlowerPot = 131,
    CherryLeaves = 132,
    WhiteCherryLeaves = 133,
    CherryLog = 134,
    CherryPlanks = 135,
    CherryStairs = 136,
    CherrySlab = 137,
    CherryFence = 138,
    CherryDoor = 139,
    CherryWindow = 140,
}

impl MaterialId {
    pub const LAST: u16 = Self::CherryWindow as u16;
    pub const FOLIAGE: [u16; 5] = [7, 8, 9, 132, 133];
    pub const BLOCK_COUNT: usize = Self::LAST as usize;

    pub const fn identifier(self) -> &'static str {
        match self {
            Self::CherryLog => "teramic::cherry_log",
            Self::CherryPlanks => "teramic::cherry_planks",
            Self::CherryStairs => "teramic::cherry_stairs",
            Self::CherrySlab => "teramic::cherry_slab",
            Self::CherryFence => "teramic::cherry_fence",
            Self::CherryDoor => "teramic::cherry_door",
            Self::CherryWindow => "teramic::cherry_window",
            Self::CherryLeaves => "teramic::cherry_leaves",
            Self::WhiteCherryLeaves => "teramic::white_cherry_leaves",
            Self::Path => "teramic::path",
            Self::Wool => "teramic::wool_block",
            Self::WoolStairs => "teramic::wool_stairs",
            Self::WoolSlab => "teramic::wool_slab",
            Self::WhiteCarpet => "teramic::carpet",
            Self::RedCarpet => "teramic::red_carpet",
            Self::YellowCarpet => "teramic::yellow_carpet",
            Self::BlueCarpet => "teramic::blue_carpet",
            Self::OrangeCarpet => "teramic::orange_carpet",
            Self::GreenCarpet => "teramic::green_carpet",
            Self::PurpleCarpet => "teramic::purple_carpet",
            Self::BlackCarpet => "teramic::black_carpet",
            Self::LimeCarpet => "teramic::lime_carpet",
            Self::PinkCarpet => "teramic::pink_carpet",
            Self::GoldCarpet => "teramic::gold_carpet",
            Self::SilverCarpet => "teramic::silver_carpet",
            Self::LightBlueCarpet => "teramic::light_blue_carpet",
            Self::RedWool => "teramic::red_wool",
            Self::YellowWool => "teramic::yellow_wool",
            Self::BlueWool => "teramic::blue_wool",
            Self::OrangeWool => "teramic::orange_wool",
            Self::GreenWool => "teramic::green_wool",
            Self::PurpleWool => "teramic::purple_wool",
            Self::BlackWool => "teramic::black_wool",
            Self::LimeWool => "teramic::lime_wool",
            Self::PinkWool => "teramic::pink_wool",
            Self::GoldWool => "teramic::gold_wool",
            Self::SilverWool => "teramic::silver_wool",
            Self::LightBlueWool => "teramic::light_blue_wool",
            Self::FlowerPot => "teramic::flower_pot",
            Self::Farmland => "teramic::farmland",
            Self::Crusher => "teramic::crusher",
            Self::HandCrank => "teramic::hand_crank",
            Self::SnowLayer => "teramic::snow_layer",
            Self::OakStairs => "teramic::oak_stairs",
            Self::BirchStairs => "teramic::birch_stairs",
            Self::SpruceStairs => "teramic::spruce_stairs",
            Self::StoneStairs => "teramic::stone_stairs",
            Self::DirtStairs => "teramic::dirt_stairs",
            Self::FineSandStairs => "teramic::fine_sand_stairs",
            Self::GlassStairs => "teramic::glass_stairs",
            Self::IceStairs => "teramic::ice_stairs",
            Self::OakSlab => "teramic::oak_slab",
            Self::BirchSlab => "teramic::birch_slab",
            Self::SpruceSlab => "teramic::spruce_slab",
            Self::StoneSlab => "teramic::stone_slab",
            Self::DirtSlab => "teramic::dirt_slab",
            Self::FineSandSlab => "teramic::fine_sand_slab",
            Self::GlassSlab => "teramic::glass_slab",
            Self::IceSlab => "teramic::ice_slab",
            Self::OakFence => "teramic::oak_fence",
            Self::BirchFence => "teramic::birch_fence",
            Self::SpruceFence => "teramic::spruce_fence",
            Self::StoneFence => "teramic::stone_fence",
            Self::DirtFence => "teramic::dirt_fence",
            Self::FineSandFence => "teramic::fine_sand_fence",
            Self::GlassFence => "teramic::glass_fence",
            Self::IceFence => "teramic::ice_fence",
            Self::OakBed => "teramic::oak_bed",
            Self::BirchBed => "teramic::birch_bed",
            Self::SpruceBed => "teramic::spruce_bed",
            Self::OakDoor => "teramic::oak_door",
            Self::BirchDoor => "teramic::birch_door",
            Self::SpruceDoor => "teramic::spruce_door",
            Self::OakWindow => "teramic::oak_window",
            Self::BirchWindow => "teramic::birch_window",
            Self::SpruceWindow => "teramic::spruce_window",
            Self::Furnace => "teramic::furnace",
            Self::ChimneyStraight => "teramic::chimney_straight",
            Self::ChimneyElbow => "teramic::chimney_elbow",
            Self::ChimneyHorizontal => "teramic::chimney_horizontal",
            Self::ChimneyBendUp => "teramic::chimney_bend_up",
            Self::ChimneyBendDown => "teramic::chimney_bend_down",
            Self::SaltStone => "teramic::salt_stone",
            Self::PinkSaltStone => "teramic::pink_salt_stone",
            Self::Seagrass => "teramic::seagrass_block",
            Self::SaltCrystal => "teramic::salt_crystal_block",
            Self::DripstoneTip => "teramic::dripstone_tip",
            Self::IceCrystal => "teramic::ice_crystal",
            Self::CaveVine => "teramic::cave_vine",
            Self::CaveRoot => "teramic::cave_root",
            Self::CaveMushroom => "teramic::cave_mushroom",
            Self::Ice => "teramic::ice",
            Self::CoralRed => "teramic::coral_red",
            Self::CoralBlue => "teramic::coral_blue",
            Self::CoralYellow => "teramic::coral_yellow",
            Self::CoralPurple => "teramic::coral_purple",
            Self::Mud => "teramic::mud",
            Self::Gravel => "teramic::gravel",
            Self::StructureStone => "teramic::structure_stone",
            Self::Bedrock => "teramic::bedrock",
            Self::Torch => "teramic::torch",
            Self::SpruceLog => "teramic::spruce_log",
            Self::Dripstone => "teramic::dripstone",
            Self::SaltFormation => "teramic::salt_formation",
            Self::DripstoneFormation => "teramic::dripstone_formation",
            Self::SaltOre => "teramic::salt_ore",
            Self::IronOre => "teramic::iron_ore",
            Self::CoalOre => "teramic::coal_ore",
            Self::GoldOre => "teramic::gold_ore",
            Self::DiamondOre => "teramic::diamond_ore",
            Self::RubyOre => "teramic::ruby_ore",
            Self::Workbench => "teramic::workbench",
            Self::Chest => "teramic::chest",
            Self::Air => "teramic::air",
            Self::Grass => "teramic::grass",
            Self::Dirt => "teramic::dirt",
            Self::Stone => "teramic::stone",
            Self::Sand => "teramic::sand",
            Self::Snow => "teramic::snow",
            Self::Wood => "teramic::wood",
            Self::Leaves => "teramic::leaves",
            Self::AutumnLeaves => "teramic::autumn_leaves",
            Self::SilverLeaves => "teramic::silver_leaves",
            Self::WeatheredWood => "teramic::weathered_wood",
            Self::Sandstone => "teramic::sandstone",
            Self::SmoothStone => "teramic::smooth_stone",
            Self::MossyStone => "teramic::mossy_stone",
            Self::SolidWater => "teramic::solid_water",
            Self::Glass => "teramic::glass",
            Self::OakPlanks => "teramic::oak_planks",
            Self::BirchPlanks => "teramic::birch_planks",
            Self::SprucePlanks => "teramic::spruce_planks",
            Self::Clay => "teramic::clay",
            Self::MushroomStem => "teramic::mushroom_stem",
            Self::PurpleMushroomCap => "teramic::purple_mushroom_cap",
            Self::RedMushroomCap => "teramic::red_mushroom_cap",
            Self::YellowMushroomCap => "teramic::yellow_mushroom_cap",
            Self::GreenMushroomCap => "teramic::green_mushroom_cap",
            Self::BlueMushroomCap => "teramic::blue_mushroom_cap",
            Self::WhiteMushroomCap => "teramic::white_mushroom_cap",
            Self::FluidWater => "teramic::fluid_water",
        }
    }

    pub const fn from_raw(id: u16) -> Option<Self> {
        Some(match id {
            0 => Self::Air,
            1 => Self::Grass,
            2 => Self::Dirt,
            3 => Self::Stone,
            4 => Self::Sand,
            5 => Self::Snow,
            6 => Self::Wood,
            7 => Self::Leaves,
            8 => Self::AutumnLeaves,
            9 => Self::SilverLeaves,
            10 => Self::WeatheredWood,
            11 => Self::Sandstone,
            12 => Self::SmoothStone,
            13 => Self::MossyStone,
            14 => Self::SolidWater,
            15 => Self::Glass,
            16 => Self::OakPlanks,
            17 => Self::BirchPlanks,
            18 => Self::SprucePlanks,
            19 => Self::Clay,
            20 => Self::MushroomStem,
            21 => Self::PurpleMushroomCap,
            22 => Self::RedMushroomCap,
            23 => Self::YellowMushroomCap,
            24 => Self::GreenMushroomCap,
            25 => Self::BlueMushroomCap,
            26 => Self::WhiteMushroomCap,
            27 => Self::FluidWater,
            28 => Self::Workbench,
            29 => Self::Chest,
            30 => Self::SaltOre,
            31 => Self::IronOre,
            32 => Self::CoalOre,
            33 => Self::GoldOre,
            34 => Self::DiamondOre,
            35 => Self::RubyOre,
            36 => Self::SpruceLog,
            37 => Self::Dripstone,
            38 => Self::SaltFormation,
            39 => Self::DripstoneFormation,
            40 => Self::Torch,
            41 => Self::Bedrock,
            42 => Self::Gravel,
            43 => Self::StructureStone,
            44 => Self::CoralRed,
            45 => Self::CoralBlue,
            46 => Self::CoralYellow,
            47 => Self::CoralPurple,
            48 => Self::Mud,
            49 => Self::Ice,
            50 => Self::Seagrass,
            51 => Self::SaltCrystal,
            52 => Self::DripstoneTip,
            53 => Self::IceCrystal,
            54 => Self::CaveVine,
            55 => Self::CaveRoot,
            56 => Self::CaveMushroom,
            57 => Self::SaltStone,
            58 => Self::PinkSaltStone,
            59 => Self::Furnace,
            60 => Self::ChimneyStraight,
            61 => Self::ChimneyElbow,
            62 => Self::ChimneyHorizontal,
            63 => Self::ChimneyBendUp,
            64 => Self::ChimneyBendDown,
            65 => Self::OakDoor,
            66 => Self::BirchDoor,
            67 => Self::SpruceDoor,
            68 => Self::OakWindow,
            69 => Self::BirchWindow,
            70 => Self::SpruceWindow,
            71 => Self::OakBed,
            72 => Self::BirchBed,
            73 => Self::SpruceBed,
            74 => Self::OakStairs,
            75 => Self::BirchStairs,
            76 => Self::SpruceStairs,
            77 => Self::StoneStairs,
            78 => Self::DirtStairs,
            79 => Self::FineSandStairs,
            80 => Self::GlassStairs,
            81 => Self::IceStairs,
            82 => Self::OakSlab,
            83 => Self::BirchSlab,
            84 => Self::SpruceSlab,
            85 => Self::StoneSlab,
            86 => Self::DirtSlab,
            87 => Self::FineSandSlab,
            88 => Self::GlassSlab,
            89 => Self::IceSlab,
            90 => Self::OakFence,
            91 => Self::BirchFence,
            92 => Self::SpruceFence,
            93 => Self::StoneFence,
            94 => Self::DirtFence,
            95 => Self::FineSandFence,
            96 => Self::GlassFence,
            97 => Self::IceFence,
            98 => Self::SnowLayer,
            99 => Self::Crusher,
            100 => Self::HandCrank,
            101 => Self::Farmland,
            102 => Self::Path,
            103 => Self::Wool,
            104 => Self::WoolStairs,
            105 => Self::WoolSlab,
            106 => Self::WhiteCarpet,
            107 => Self::RedCarpet,
            108 => Self::YellowCarpet,
            109 => Self::BlueCarpet,
            110 => Self::OrangeCarpet,
            111 => Self::GreenCarpet,
            112 => Self::PurpleCarpet,
            113 => Self::BlackCarpet,
            114 => Self::LimeCarpet,
            115 => Self::PinkCarpet,
            116 => Self::GoldCarpet,
            117 => Self::SilverCarpet,
            118 => Self::LightBlueCarpet,
            119 => Self::RedWool,
            120 => Self::YellowWool,
            121 => Self::BlueWool,
            122 => Self::OrangeWool,
            123 => Self::GreenWool,
            124 => Self::PurpleWool,
            125 => Self::BlackWool,
            126 => Self::LimeWool,
            127 => Self::PinkWool,
            128 => Self::GoldWool,
            129 => Self::SilverWool,
            130 => Self::LightBlueWool,
            131 => Self::FlowerPot,
            132 => Self::CherryLeaves,
            133 => Self::WhiteCherryLeaves,
            134 => Self::CherryLog,
            135 => Self::CherryPlanks,
            136 => Self::CherryStairs,
            137 => Self::CherrySlab,
            138 => Self::CherryFence,
            139 => Self::CherryDoor,
            140 => Self::CherryWindow,
            _ => return None,
        })
    }

    /// Fluid anchors and torches remain selectable without blocking movement.
    pub const fn is_solid(self) -> bool {
        !matches!(
            self,
            Self::Air
                | Self::FluidWater
                | Self::Torch
                | Self::Seagrass
                | Self::CaveVine
                | Self::CaveRoot
                | Self::CaveMushroom
        )
    }

    pub const fn is_opaque(self) -> bool {
        if (self as u16) >= 74 && (self as u16) <= 98
            || matches!(self, Self::Path | Self::WoolStairs | Self::WoolSlab)
            || (self as u16) >= 106 && (self as u16) <= 118
            || matches!(
                self,
                Self::FlowerPot
                    | Self::CherryStairs
                    | Self::CherrySlab
                    | Self::CherryFence
                    | Self::CherryDoor
                    | Self::CherryWindow
            )
        {
            return false;
        }
        !matches!(
            self,
            Self::Air
                | Self::Torch
                | Self::Workbench
                | Self::Chest
                | Self::Furnace
                | Self::Crusher
                | Self::HandCrank
                | Self::ChimneyStraight
                | Self::ChimneyElbow
                | Self::ChimneyHorizontal
                | Self::ChimneyBendUp
                | Self::ChimneyBendDown
                | Self::OakDoor
                | Self::BirchDoor
                | Self::SpruceDoor
                | Self::OakWindow
                | Self::BirchWindow
                | Self::SpruceWindow
                | Self::OakBed
                | Self::BirchBed
                | Self::SpruceBed
                | Self::Leaves
                | Self::AutumnLeaves
                | Self::SilverLeaves
                | Self::CherryLeaves
                | Self::WhiteCherryLeaves
                | Self::SolidWater
                | Self::Glass
                | Self::FluidWater
                | Self::Ice
                | Self::Seagrass
                | Self::SaltCrystal
                | Self::DripstoneTip
                | Self::IceCrystal
                | Self::CaveVine
                | Self::CaveRoot
                | Self::CaveMushroom
        )
    }

    pub const fn is_natural_ground(self) -> bool {
        matches!(
            self,
            Self::Grass
                | Self::Dirt
                | Self::Farmland
                | Self::Path
                | Self::Stone
                | Self::Sand
                | Self::Snow
                | Self::Sandstone
                | Self::MossyStone
                | Self::Clay
                | Self::Gravel
                | Self::Mud
        )
    }

    pub const fn supports_water_plants(self) -> bool {
        self.is_natural_ground()
            || matches!(
                self,
                Self::CoralRed | Self::CoralBlue | Self::CoralYellow | Self::CoralPurple
            )
    }

    pub const fn flammable(self) -> bool {
        if matches!(self as u16, 74..=76 | 82..=84 | 90..=92 | 103..=130 | 134..=140) {
            return true;
        }
        matches!(
            self,
            Self::Grass
                | Self::SpruceLog
                | Self::Workbench
                | Self::Chest
                | Self::Wood
                | Self::OakBed
                | Self::BirchBed
                | Self::SpruceBed
                | Self::WeatheredWood
                | Self::Leaves
                | Self::AutumnLeaves
                | Self::SilverLeaves
                | Self::CherryLeaves
                | Self::WhiteCherryLeaves
                | Self::OakPlanks
                | Self::BirchPlanks
                | Self::SprucePlanks
                | Self::MushroomStem
                | Self::PurpleMushroomCap
                | Self::RedMushroomCap
                | Self::YellowMushroomCap
                | Self::GreenMushroomCap
                | Self::BlueMushroomCap
                | Self::WhiteMushroomCap
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn village_materials_append_without_changing_old_ids_or_shape_occlusion() {
        assert_eq!(MaterialId::Farmland as u16, 101);
        for (id, material) in [
            MaterialId::Path,
            MaterialId::Wool,
            MaterialId::WoolStairs,
            MaterialId::WoolSlab,
        ]
        .into_iter()
        .enumerate()
        {
            assert_eq!(material as usize, 102 + id);
            assert_eq!(MaterialId::from_raw(material as u16), Some(material));
            assert!(material.is_solid());
        }
        assert!(MaterialId::Wool.is_opaque());
        assert!(!MaterialId::WoolSlab.is_opaque());
        assert!(!MaterialId::WoolStairs.is_opaque());
        assert!(!MaterialId::Path.is_opaque());
        assert!(MaterialId::Path.is_natural_ground());
        assert!(MaterialId::Wool.flammable());
    }
    #[test]
    fn reef_cubes_append_ids_without_changing_existing_materials() {
        for (offset, material) in [
            MaterialId::CoralRed,
            MaterialId::CoralBlue,
            MaterialId::CoralYellow,
            MaterialId::CoralPurple,
            MaterialId::Mud,
        ]
        .into_iter()
        .enumerate()
        {
            assert_eq!(material as usize, 44 + offset);
            assert_eq!(MaterialId::from_raw(material as u16), Some(material));
            assert!(material.is_solid() && material.is_opaque());
            assert!(!material.flammable());
            assert!(material.identifier().starts_with("teramic::"));
        }
        assert_eq!(MaterialId::StructureStone as u16, 43);
        assert!(MaterialId::Mud.is_natural_ground());
        assert!(!MaterialId::CoralRed.is_natural_ground());
    }
    #[test]
    fn appended_materials_keep_legacy_ids_and_distinguish_visibility_from_collision() {
        assert_eq!(MaterialId::Bedrock as u16, 41);
        assert_eq!(MaterialId::Gravel as u16, 42);
        assert_eq!(MaterialId::StructureStone as u16, 43);
        for (id, material) in [
            MaterialId::Air,
            MaterialId::Grass,
            MaterialId::Dirt,
            MaterialId::Stone,
            MaterialId::Sand,
            MaterialId::Snow,
            MaterialId::Wood,
            MaterialId::Leaves,
            MaterialId::AutumnLeaves,
            MaterialId::SilverLeaves,
            MaterialId::WeatheredWood,
            MaterialId::Sandstone,
            MaterialId::SmoothStone,
            MaterialId::MossyStone,
            MaterialId::SolidWater,
            MaterialId::Glass,
            MaterialId::OakPlanks,
            MaterialId::BirchPlanks,
            MaterialId::SprucePlanks,
            MaterialId::Clay,
        ]
        .into_iter()
        .enumerate()
        {
            assert_eq!(material as u16, id as u16);
        }
        for id in 0..=MaterialId::LAST {
            assert_eq!(MaterialId::from_raw(id).unwrap() as u16, id);
        }
        assert!(MaterialId::from_raw(MaterialId::LAST + 1).is_none());
        for material in [MaterialId::Glass, MaterialId::SolidWater] {
            assert!(material.is_solid());
            assert!(!material.is_opaque());
            assert!(!material.flammable());
            assert!(!material.is_natural_ground());
        }
    }

    #[test]
    fn mushroom_blocks_are_solid_harvestable_structures_not_spawn_ground() {
        for (index, material) in [
            MaterialId::MushroomStem,
            MaterialId::PurpleMushroomCap,
            MaterialId::RedMushroomCap,
            MaterialId::YellowMushroomCap,
            MaterialId::GreenMushroomCap,
            MaterialId::BlueMushroomCap,
            MaterialId::WhiteMushroomCap,
        ]
        .into_iter()
        .enumerate()
        {
            assert_eq!(material as u16, 20 + index as u16);
            assert!(material.is_solid());
            assert!(material.is_opaque());
            assert!(material.flammable());
            assert!(!material.is_natural_ground());
        }
    }

    #[test]
    fn fluid_source_appends_id_without_changing_solid_water() {
        assert_eq!(MaterialId::WhiteMushroomCap as u16, 26);
        assert_eq!(MaterialId::FluidWater as u16, 27);
        assert_eq!(MaterialId::FluidWater.identifier(), "teramic::fluid_water");
        assert!(!MaterialId::FluidWater.is_solid());
        assert!(!MaterialId::FluidWater.is_opaque());
        assert!(!MaterialId::FluidWater.flammable());
        assert!(MaterialId::SolidWater.is_solid());
    }
}
