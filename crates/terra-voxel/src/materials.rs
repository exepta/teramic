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
    WoodenGear = 141,
    StoneGear = 142,
    IronGear = 143,
    WoodenShaft = 144,
    StoneShaft = 145,
    IronShaft = 146,
    WaterWheel = 147,
    Conveyor = 148,
    WoodenLargeGear = 149,
    StoneLargeGear = 150,
    IronLargeGear = 151,
    ItemHopper = 152,
    ItemImporter = 153,
    GearboxHorizontal = 154,
    GearboxVertical = 155,
    Mixer = 156,
    FilteredFunnel = 157,
    BeltMerger = 158,
    FilteredTunnel = 159,
    BeltDistributor = 160,
    Cauldron = 161,
    Campfire = 162,
    WoodPipe = 163,
    StonePipe = 164,
    IronPipe = 165,
    Valve = 166,
    Faucet = 167,
    Pump = 168,
    Basin = 169,
    HoldBlock = 170,
    FluidInfuser = 171,
    GlassPipe = 172,
    Bergfried = 173,
    CastleStoneWall = 174,
    BergfriedWall = 175,
    CastleCrackedWall = 176,
    CastleOvergrownWall = 177,
    CastleMossWall = 178,
    CastleIronWall = 179,
    CastleBattlementStraight = 180,
    CastleBattlementCorner = 181,
    CastleBattlementPillar = 182,
    CastleArchLeft = 183,
    CastleArchRight = 184,
    CastleArrowSlit = 185,
    CastleWindow = 186,
    CastleSandstoneWall = 187,
    CastleDesertWall = 188,
    CastleSnowWall = 189,
    CastleBlackstoneWall = 190,
    CastleRoyalWall = 191,
    CastleTimberWall = 192,
    Lantern = 193,
    Chain = 194,
    AppleLeaves = 195,
    RailStraight = 196,
    RailCurveSmall = 197,
    RailCurveLarge = 198,
    RailSlope = 199,
    RailSlopeEntry = 200,
    RailSlopeExit = 201,
    RailSwitchLeft = 202,
    RailSwitchRight = 203,
    RailCrossing = 204,
    RailBuffer = 205,
    Ballast = 206,
    BallastStairs = 207,
    BallastSlab = 208,
    RailSwitchStand = 209,
    TrainChassis = 210,
    TrainBogie = 211,
    TrainFrame = 212,
    TrainCoupler = 213,
    TrainCab = 214,
    TrainSeat = 215,
    TrainCargo = 216,
    TrainAssemblyStation = 217,
    // F54_enum_START
    TrainFuelEngine = 218,
    TrainBoilerStraight = 219,
    TrainBoilerCurved = 220,
    TrainBoilerEnd = 221,
    TrainSmokebox = 222,
    TrainSmokeboxDoor = 223,
    TrainChimney = 224,
    TrainSteamDome = 225,
    TrainSandDome = 226,
    TrainBrassPipeStraight = 227,
    TrainBrassPipeBend = 228,
    TrainSteamValve = 229,
    TrainPressureGauge = 230,
    TrainPressureUnit = 231,
    TrainWheelLarge = 232,
    TrainWheelMedium = 233,
    TrainWheelSmall = 234,
    TrainAxle = 235,
    TrainConnectingRodLong = 236,
    TrainConnectingRodShort = 237,
    TrainRodJoint = 238,
    TrainPistonRod = 239,
    TrainCrosshead = 240,
    TrainMainFrame = 241,
    TrainRedFrame = 242,
    TrainBufferBeam = 243,
    TrainBuffer = 244,
    TrainCabWallSide = 245,
    TrainCabWallFront = 246,
    TrainCabWindow = 247,
    TrainCabDoor = 248,
    TrainRoofCurved = 249,
    TrainRoofFlat = 250,
    TrainRoofVent = 251,
    TrainTenderWall = 252,
    TrainTenderFront = 253,
    TrainWaterTank = 254,
    TrainCoalLoad = 255,
    TrainLadderSide = 256,
    TrainLadderFront = 257,
    TrainRailing = 258,
    TrainHandrail = 259,
    TrainHandrailPost = 260,
    TrainRivetPlate = 261,
    TrainBolt = 262,
    TrainHatch = 263,
    TrainHeadlightFront = 264,
    TrainHeadlightSide = 265,
    TrainNumberPlate = 266,
    TrainToolbox = 267,
    TrainWoodenPlank = 268,
    TrainCoalPiece = 269,
    TrainBlackIronBlock = 270,
    TrainBlackIronSlab = 271,
    TrainBlackIronStairs = 272,
    TrainBlackIronInnerCorner = 273,
    TrainBlackIronOuterCorner = 274,
    TrainBlackIronSlope = 275,
    TrainDarkSteelBlock = 276,
    TrainDarkSteelSlab = 277,
    TrainDarkSteelStairs = 278,
    TrainDarkSteelInnerCorner = 279,
    TrainDarkSteelOuterCorner = 280,
    TrainDarkSteelSlope = 281,
    TrainSheetMetalBlock = 282,
    TrainSheetMetalSlab = 283,
    TrainSheetMetalStairs = 284,
    TrainSheetMetalInnerCorner = 285,
    TrainSheetMetalOuterCorner = 286,
    TrainSheetMetalSlope = 287,
    TrainGrateBlock = 288,
    TrainGrateSlab = 289,
    TrainGrateStairs = 290,
    TrainGrateInnerCorner = 291,
    TrainGrateOuterCorner = 292,
    TrainGrateSlope = 293,
    TrainCeilingLight = 294,
    // F54_enum_END
    RailTurntable = 295,
    TrainRailBuilder = 296,
    LumberjackStation = 297,
    SmithStation = 298,
    MarketStation = 299,
    ArmyStation = 300,
    RailwayStation = 301,
    MineStation = 302,
    ButcherStation = 303,
    BishopStation = 304,
    FarmStation = 305,
    // F35: append-only persisted material identifiers.
    SpiritBlossomGrass = 306,
    SpiritBlossomDirt = 307,
    SpiritStone = 308,
    BlossomPetalBlock = 309,
    PinkBlossomLeaves = 310,
    WhiteBlossomLeaves = 311,
    LavenderBlossomLeaves = 312,
    SpiritBlossomLog = 313,
    SpiritBlossomPlanks = 314,
    GlowingLanternStone = 315,
    GlowingSpiritFlowers = 316,
    BlossomBush = 317,
    SpiritLanternPost = 318,
    LotusFlower = 319,
    LilyPad = 320,
    HangingBlossomLantern = 321,
    PetalGroundCover = 322,
    SpiritShrineStone = 323,
    BlueClay = 324,
    RedClay = 325,
    GreenClay = 326,
    YellowClay = 327,
    WhiteClay = 328,
    BlackClay = 329,
    GrayClay = 330,
    DarkGrayClay = 331,
    PurpleClay = 332,
    PinkClay = 333,
    LimeClay = 334,
    TurquoiseClay = 335,
    OrangeClay = 336,
    GraveStone = 337,
    DarkGraveStone = 338,
    CrackedGraveStone = 339,
    OrnateGraveStone = 340,
    MossGraveStone = 341,
    OvergrownGraveStone = 342,
    CryptBrick = 343,
    GraveStoneSlab = 344,
    OrnateGraveSlab = 345,
    MosaicGraveSlab = 346,
    CrackedGraveSlab = 347,
    InscribedGraveSlab = 348,
    CryptColumn = 349,
    OrnateCryptColumn = 350,
    CryptCapital = 351,
    CryptColumnBase = 352,
    CryptArch = 353,
    MossyCryptArch = 354,
    GraveReliefWall = 355,
    SkullRelief = 356,
    CrossRelief = 357,
    CryptWallNiche = 358,
    CryptHeraldicBanner = 359,
    CryptTornBanner = 360,
    CryptCurtain = 361,
    GravestoneSmall = 362,
    GravestoneMedium = 363,
    GravestoneLarge = 364,
    GraveCross = 365,
    BorderedGrave = 366,
    SarcophagusClosed = 367,
    SarcophagusOpen = 368,
    GraveLanternPost = 369,
    CryptWallTorch = 370,
    CryptCandles = 371,
    CryptChandelier = 372,
    SoulBrazier = 373,
    SkeletonRemains = 374,
    BonePile = 375,
    GraveUrn = 376,
    OrnateCryptChest = 377,
    CryptBooks = 378,
    GraveSkull = 379,
    CryptChain = 380,
    DeadGrass = 381,
    ThornBush = 382,
    HangingIvy = 383,
    GraveMoss = 384,
    DarkMushrooms = 385,
    GlowingPurpleMushrooms = 386,
    BlossomingVines = 387,
    CryptPressurePlate = 388,
    CryptLever = 389,
    CryptWallSwitch = 390,
    CryptPortcullis = 391,
    LockedCryptGate = 392,
    CryptSpikeTrap = 393,
    CryptArrowTrap = 394,
    SoulCrystalSmall = 395,
    SoulCrystalLarge = 396,
    ActiveRuneBlock = 397,
    InactiveRuneBlock = 398,
    MysticMistBlock = 399,
    BloodGrass = 400,
    BloodDirt = 401,
    BloodStone = 402,
    VeinedStone = 403,
    BloodwoodPlanks = 404,
    BloodwoodLog = 405,
    BloodLeaves = 406,
    CrimsonMycelium = 407,
    BloodMoss = 408,
    BloodCrystalBlock = 409,
    BloodFungalBlock = 410,
    BloodGlowMushroomBlock = 411,
    BloodMushrooms = 412,
    BloodThornBush = 413,
    BloodFlower = 414,
    HangingBloodVines = 415,
    BloodRoots = 416,
    BloodBones = 417,
    BloodFern = 418,
    GiantBloodMushroom = 419,
    BloodRuinedArch = 420,
    BloodCrystalPillar = 421,
    Cactus = 422,
    MeadowDaisies = 423,
    Lupine = 424,
    ForestFern = 425,
    FloweringShrub = 426,
    AlpineHeather = 427,
    DesertShrub = 428,
    ShoreReeds = 429,
    FogBlock = 430,
    Asphalt = 431,
    AsphaltCover = 432,
    AsphaltSlope = 433,
    AsphaltEntry = 434,
    AsphaltExit = 435,
    RoadCurb = 436,
    WetTar = 437,
    DriedTar = 438,
    Bile = 439,
    BileBlock = 440,
    SidewalkBlock = 441,
    Sidewalk = 442,
    SidewalkLeft = 443,
    SidewalkRight = 444,
    SidewalkArcLeft = 445,
    SidewalkArcRight = 446,
    SidewalkLowered = 447,
    StrawBlock = 448,
    DarkDragonEgg = 449,
    EarthDragonEgg = 450,
    WaterDragonEgg = 451,
    AmberGeode = 452,
    TideGeode = 453,
    SaltCaveStone = 454,
    WetSaltStone = 455,
    SaltDustFloor = 456,
    DeepSaltStone = 457,
    TranslucentSaltBlock = 458,
    SaltCrystalCluster = 459,
    SaltStalactite = 460,
    SaltStalagmite = 461,
    SaltRubble = 462,
    SaltPillarFragment = 463,
    NestStraw = 464,
    WetCaveStone = 465,
    CaveCalcite = 466,
    CavePebbles = 467,
    CaveCrystalCluster = 468,
    MossyCaveStone = 469,
    CaveMud = 470,
    LushCaveStone = 471,
    OvergrownStone = 472,
    RootedCaveDirt = 473,
    CaveGrass = 474,
    CaveMoss = 475,
    GlowMoss = 476,
    ClearCaveIce = 477,
    PackedCaveIce = 478,
    FrostedCaveIce = 479,
    FrozenStone = 480,
    SnowDustStone = 481,
    BlueCrystalBlock = 482,
    GlowVine = 483,
    FrostVine = 484,
    FrostCluster = 485,
    GlacialCrystalCluster = 486,
    CaveWildflowers = 487,
}

impl MaterialId {
    pub const LAST: u16 = Self::CaveWildflowers as u16;
    pub const FOLIAGE: [u16; 10] = [7, 8, 9, 132, 133, 195, 310, 311, 312, 406];
    pub const BLOCK_COUNT: usize = Self::LAST as usize;

    pub const fn identifier(self) -> &'static str {
        match self {
            Self::Asphalt => "teramic::asphalt",
            Self::AsphaltCover => "teramic::asphalt_cover",
            Self::AsphaltSlope => "teramic::asphalt_slope",
            Self::AsphaltEntry => "teramic::asphalt_entry",
            Self::AsphaltExit => "teramic::asphalt_exit",
            Self::RoadCurb => "teramic::road_curb",
            Self::WetTar => "teramic::wet_tar",
            Self::DriedTar => "teramic::dried_tar",
            Self::Bile => "teramic::bile",
            Self::BileBlock => "teramic::bile_block",
            Self::SidewalkBlock => "teramic::sidewalk_block",
            Self::Sidewalk => "teramic::sidewalk",
            Self::SidewalkLeft => "teramic::sidewalk_left",
            Self::SidewalkRight => "teramic::sidewalk_right",
            Self::SidewalkArcLeft => "teramic::sidewalk_arc_left",
            Self::SidewalkArcRight => "teramic::sidewalk_arc_right",
            Self::SidewalkLowered => "teramic::sidewalk_lowered",
            Self::DarkDragonEgg => "teramic::dark_dragon_egg",
            Self::EarthDragonEgg => "teramic::earth_dragon_egg",
            Self::WaterDragonEgg => "teramic::water_dragon_egg",
            Self::AmberGeode => "teramic::amber_geode",
            Self::TideGeode => "teramic::tide_geode",
            Self::SaltCaveStone => "teramic::salt_cave_stone",
            Self::WetSaltStone => "teramic::wet_salt_stone",
            Self::SaltDustFloor => "teramic::salt_dust_floor",
            Self::DeepSaltStone => "teramic::deep_salt_stone",
            Self::TranslucentSaltBlock => "teramic::translucent_salt_block",
            Self::SaltCrystalCluster => "teramic::salt_crystal_cluster",
            Self::SaltStalactite => "teramic::salt_stalactite",
            Self::SaltStalagmite => "teramic::salt_stalagmite",
            Self::SaltRubble => "teramic::salt_rubble",
            Self::SaltPillarFragment => "teramic::salt_pillar_fragment",
            Self::WetCaveStone => "teramic::wet_cave_stone",
            Self::CaveCalcite => "teramic::cave_calcite",
            Self::CavePebbles => "teramic::cave_pebbles",
            Self::CaveCrystalCluster => "teramic::cave_crystal_cluster",
            Self::MossyCaveStone => "teramic::mossy_cave_stone",
            Self::CaveMud => "teramic::cave_mud",
            Self::LushCaveStone => "teramic::lush_cave_stone",
            Self::OvergrownStone => "teramic::overgrown_stone",
            Self::RootedCaveDirt => "teramic::rooted_cave_dirt",
            Self::CaveGrass => "teramic::cave_grass",
            Self::CaveMoss => "teramic::cave_moss",
            Self::GlowMoss => "teramic::glow_moss",
            Self::ClearCaveIce => "teramic::clear_cave_ice",
            Self::PackedCaveIce => "teramic::packed_cave_ice",
            Self::FrostedCaveIce => "teramic::frosted_cave_ice",
            Self::FrozenStone => "teramic::frozen_stone",
            Self::SnowDustStone => "teramic::snow_dust_stone",
            Self::BlueCrystalBlock => "teramic::blue_crystal_block",
            Self::GlowVine => "teramic::glow_vine",
            Self::FrostVine => "teramic::frost_vine",
            Self::FrostCluster => "teramic::frost_cluster",
            Self::GlacialCrystalCluster => "teramic::glacial_crystal_cluster",
            Self::CaveWildflowers => "teramic::cave_wildflowers",
            Self::StrawBlock => "teramic::straw_block",
            Self::NestStraw => "teramic::nest_straw",
            Self::FogBlock => "teramic::fog_block",
            Self::Cactus => "teramic::cactus",
            Self::MeadowDaisies => "teramic::meadow_daisies",
            Self::Lupine => "teramic::lupine",
            Self::ForestFern => "teramic::forest_fern",
            Self::FloweringShrub => "teramic::flowering_shrub",
            Self::AlpineHeather => "teramic::alpine_heather",
            Self::DesertShrub => "teramic::desert_shrub",
            Self::ShoreReeds => "teramic::shore_reeds",
            Self::BloodGrass => "teramic::blood_grass",
            Self::BloodDirt => "teramic::blood_dirt",
            Self::BloodStone => "teramic::blood_stone",
            Self::VeinedStone => "teramic::veined_stone",
            Self::BloodwoodPlanks => "teramic::bloodwood_planks",
            Self::BloodwoodLog => "teramic::bloodwood_log",
            Self::BloodLeaves => "teramic::blood_leaves",
            Self::CrimsonMycelium => "teramic::crimson_mycelium",
            Self::BloodMoss => "teramic::blood_moss",
            Self::BloodCrystalBlock => "teramic::blood_crystal_block",
            Self::BloodFungalBlock => "teramic::blood_fungal_block",
            Self::BloodGlowMushroomBlock => "teramic::blood_glow_mushroom_block",
            Self::BloodMushrooms => "teramic::blood_mushrooms",
            Self::BloodThornBush => "teramic::blood_thorn_bush",
            Self::BloodFlower => "teramic::blood_flower",
            Self::HangingBloodVines => "teramic::hanging_blood_vines",
            Self::BloodRoots => "teramic::blood_roots",
            Self::BloodBones => "teramic::blood_bones",
            Self::BloodFern => "teramic::blood_fern",
            Self::GiantBloodMushroom => "teramic::giant_blood_mushroom",
            Self::BloodRuinedArch => "teramic::blood_ruined_arch",
            Self::BloodCrystalPillar => "teramic::blood_crystal_pillar",
            Self::BlueClay => "teramic::blue_clay",
            Self::RedClay => "teramic::red_clay",
            Self::GreenClay => "teramic::green_clay",
            Self::YellowClay => "teramic::yellow_clay",
            Self::WhiteClay => "teramic::white_clay",
            Self::BlackClay => "teramic::black_clay",
            Self::GrayClay => "teramic::gray_clay",
            Self::DarkGrayClay => "teramic::dark_gray_clay",
            Self::PurpleClay => "teramic::purple_clay",
            Self::PinkClay => "teramic::pink_clay",
            Self::LimeClay => "teramic::lime_clay",
            Self::TurquoiseClay => "teramic::turquoise_clay",
            Self::OrangeClay => "teramic::orange_clay",
            Self::GraveStone => "teramic::grave_stone",
            Self::DarkGraveStone => "teramic::dark_grave_stone",
            Self::CrackedGraveStone => "teramic::cracked_grave_stone",
            Self::OrnateGraveStone => "teramic::ornate_grave_stone",
            Self::MossGraveStone => "teramic::moss_grave_stone",
            Self::OvergrownGraveStone => "teramic::overgrown_grave_stone",
            Self::CryptBrick => "teramic::crypt_brick",
            Self::GraveStoneSlab => "teramic::grave_stone_slab",
            Self::OrnateGraveSlab => "teramic::ornate_grave_slab",
            Self::MosaicGraveSlab => "teramic::mosaic_grave_slab",
            Self::CrackedGraveSlab => "teramic::cracked_grave_slab",
            Self::InscribedGraveSlab => "teramic::inscribed_grave_slab",
            Self::CryptColumn => "teramic::crypt_column",
            Self::OrnateCryptColumn => "teramic::ornate_crypt_column",
            Self::CryptCapital => "teramic::crypt_capital",
            Self::CryptColumnBase => "teramic::crypt_column_base",
            Self::CryptArch => "teramic::crypt_arch",
            Self::MossyCryptArch => "teramic::mossy_crypt_arch",
            Self::GraveReliefWall => "teramic::grave_relief_wall",
            Self::SkullRelief => "teramic::skull_relief",
            Self::CrossRelief => "teramic::cross_relief",
            Self::CryptWallNiche => "teramic::crypt_wall_niche",
            Self::CryptHeraldicBanner => "teramic::crypt_heraldic_banner",
            Self::CryptTornBanner => "teramic::crypt_torn_banner",
            Self::CryptCurtain => "teramic::crypt_curtain",
            Self::GravestoneSmall => "teramic::gravestone_small",
            Self::GravestoneMedium => "teramic::gravestone_medium",
            Self::GravestoneLarge => "teramic::gravestone_large",
            Self::GraveCross => "teramic::grave_cross",
            Self::BorderedGrave => "teramic::bordered_grave",
            Self::SarcophagusClosed => "teramic::sarcophagus_closed",
            Self::SarcophagusOpen => "teramic::sarcophagus_open",
            Self::GraveLanternPost => "teramic::grave_lantern_post",
            Self::CryptWallTorch => "teramic::crypt_wall_torch",
            Self::CryptCandles => "teramic::crypt_candles",
            Self::CryptChandelier => "teramic::crypt_chandelier",
            Self::SoulBrazier => "teramic::soul_brazier",
            Self::SkeletonRemains => "teramic::skeleton_remains",
            Self::BonePile => "teramic::bone_pile",
            Self::GraveUrn => "teramic::grave_urn",
            Self::OrnateCryptChest => "teramic::ornate_crypt_chest",
            Self::CryptBooks => "teramic::crypt_books",
            Self::GraveSkull => "teramic::grave_skull",
            Self::CryptChain => "teramic::crypt_chain",
            Self::DeadGrass => "teramic::dead_grass",
            Self::ThornBush => "teramic::thorn_bush",
            Self::HangingIvy => "teramic::hanging_ivy",
            Self::GraveMoss => "teramic::grave_moss",
            Self::DarkMushrooms => "teramic::dark_mushrooms",
            Self::GlowingPurpleMushrooms => "teramic::glowing_purple_mushrooms",
            Self::BlossomingVines => "teramic::blossoming_vines",
            Self::CryptPressurePlate => "teramic::crypt_pressure_plate",
            Self::CryptLever => "teramic::crypt_lever",
            Self::CryptWallSwitch => "teramic::crypt_wall_switch",
            Self::CryptPortcullis => "teramic::crypt_portcullis",
            Self::LockedCryptGate => "teramic::locked_crypt_gate",
            Self::CryptSpikeTrap => "teramic::crypt_spike_trap",
            Self::CryptArrowTrap => "teramic::crypt_arrow_trap",
            Self::SoulCrystalSmall => "teramic::soul_crystal_small",
            Self::SoulCrystalLarge => "teramic::soul_crystal_large",
            Self::ActiveRuneBlock => "teramic::active_rune_block",
            Self::InactiveRuneBlock => "teramic::inactive_rune_block",
            Self::MysticMistBlock => "teramic::mystic_mist_block",
            Self::SpiritBlossomGrass => "teramic::spirit_blossom_grass",
            Self::SpiritBlossomDirt => "teramic::spirit_blossom_dirt",
            Self::SpiritStone => "teramic::spirit_stone",
            Self::BlossomPetalBlock => "teramic::blossom_petal_block",
            Self::PinkBlossomLeaves => "teramic::pink_blossom_leaves",
            Self::WhiteBlossomLeaves => "teramic::white_blossom_leaves",
            Self::LavenderBlossomLeaves => "teramic::lavender_blossom_leaves",
            Self::SpiritBlossomLog => "teramic::spirit_blossom_log",
            Self::SpiritBlossomPlanks => "teramic::spirit_blossom_planks",
            Self::GlowingLanternStone => "teramic::glowing_lantern_stone",
            Self::GlowingSpiritFlowers => "teramic::glowing_spirit_flowers",
            Self::BlossomBush => "teramic::blossom_bush",
            Self::SpiritLanternPost => "teramic::spirit_lantern_post",
            Self::LotusFlower => "teramic::lotus_flower",
            Self::LilyPad => "teramic::lily_pad",
            Self::HangingBlossomLantern => "teramic::hanging_blossom_lantern",
            Self::PetalGroundCover => "teramic::petal_ground_cover",
            Self::SpiritShrineStone => "teramic::spirit_shrine_stone",
            Self::AppleLeaves => "teramic::apple_leaves",
            Self::RailStraight => "teramic::rail_straight",
            Self::RailCurveSmall => "teramic::rail_curve_small",
            Self::RailCurveLarge => "teramic::rail_curve_large",
            Self::RailSlope => "teramic::rail_slope",
            Self::RailSlopeEntry => "teramic::rail_slope_entry",
            Self::RailSlopeExit => "teramic::rail_slope_exit",
            Self::RailSwitchLeft => "teramic::rail_switch_left",
            Self::RailSwitchRight => "teramic::rail_switch_right",
            Self::RailCrossing => "teramic::rail_crossing",
            Self::RailBuffer => "teramic::rail_buffer",
            Self::Ballast => "teramic::ballast",
            Self::BallastStairs => "teramic::ballast_stairs",
            Self::BallastSlab => "teramic::ballast_slab",
            Self::RailSwitchStand => "teramic::rail_switch_stand",
            Self::TrainChassis => "teramic::train_chassis",
            Self::TrainBogie => "teramic::train_bogie",
            Self::TrainFrame => "teramic::train_frame",
            Self::TrainCoupler => "teramic::train_coupler",
            Self::TrainCab => "teramic::train_cab",
            Self::TrainSeat => "teramic::train_seat",
            Self::TrainCargo => "teramic::train_cargo",
            Self::TrainAssemblyStation => "teramic::train_assembly_station",
            // F54_identifiers_START
            Self::TrainFuelEngine => "teramic::train_fuel_engine",
            Self::TrainBoilerStraight => "teramic::train_boiler_straight",
            Self::TrainBoilerCurved => "teramic::train_boiler_curved",
            Self::TrainBoilerEnd => "teramic::train_boiler_end",
            Self::TrainSmokebox => "teramic::train_smokebox",
            Self::TrainSmokeboxDoor => "teramic::train_smokebox_door",
            Self::TrainChimney => "teramic::train_chimney",
            Self::TrainSteamDome => "teramic::train_steam_dome",
            Self::TrainSandDome => "teramic::train_sand_dome",
            Self::TrainBrassPipeStraight => "teramic::train_brass_pipe_straight",
            Self::TrainBrassPipeBend => "teramic::train_brass_pipe_bend",
            Self::TrainSteamValve => "teramic::train_steam_valve",
            Self::TrainPressureGauge => "teramic::train_pressure_gauge",
            Self::TrainPressureUnit => "teramic::train_pressure_unit",
            Self::TrainWheelLarge => "teramic::train_wheel_large",
            Self::TrainWheelMedium => "teramic::train_wheel_medium",
            Self::TrainWheelSmall => "teramic::train_wheel_small",
            Self::TrainAxle => "teramic::train_axle",
            Self::TrainConnectingRodLong => "teramic::train_connecting_rod_long",
            Self::TrainConnectingRodShort => "teramic::train_connecting_rod_short",
            Self::TrainRodJoint => "teramic::train_rod_joint",
            Self::TrainPistonRod => "teramic::train_piston_rod",
            Self::TrainCrosshead => "teramic::train_crosshead",
            Self::TrainMainFrame => "teramic::train_main_frame",
            Self::TrainRedFrame => "teramic::train_red_frame",
            Self::TrainBufferBeam => "teramic::train_buffer_beam",
            Self::TrainBuffer => "teramic::train_buffer",
            Self::TrainCabWallSide => "teramic::train_cab_wall_side",
            Self::TrainCabWallFront => "teramic::train_cab_wall_front",
            Self::TrainCabWindow => "teramic::train_cab_window",
            Self::TrainCabDoor => "teramic::train_cab_door",
            Self::TrainRoofCurved => "teramic::train_roof_curved",
            Self::TrainRoofFlat => "teramic::train_roof_flat",
            Self::TrainRoofVent => "teramic::train_roof_vent",
            Self::TrainTenderWall => "teramic::train_tender_wall",
            Self::TrainTenderFront => "teramic::train_tender_front",
            Self::TrainWaterTank => "teramic::train_water_tank",
            Self::TrainCoalLoad => "teramic::train_coal_load",
            Self::TrainLadderSide => "teramic::train_ladder_side",
            Self::TrainLadderFront => "teramic::train_ladder_front",
            Self::TrainRailing => "teramic::train_railing",
            Self::TrainHandrail => "teramic::train_handrail",
            Self::TrainHandrailPost => "teramic::train_handrail_post",
            Self::TrainRivetPlate => "teramic::train_rivet_plate",
            Self::TrainBolt => "teramic::train_bolt",
            Self::TrainHatch => "teramic::train_hatch",
            Self::TrainHeadlightFront => "teramic::train_headlight_front",
            Self::TrainHeadlightSide => "teramic::train_headlight_side",
            Self::TrainNumberPlate => "teramic::train_number_plate",
            Self::TrainToolbox => "teramic::train_toolbox",
            Self::TrainWoodenPlank => "teramic::train_wooden_plank",
            Self::TrainCoalPiece => "teramic::train_coal_piece",
            Self::TrainBlackIronBlock => "teramic::train_black_iron_block",
            Self::TrainBlackIronSlab => "teramic::train_black_iron_slab",
            Self::TrainBlackIronStairs => "teramic::train_black_iron_stairs",
            Self::TrainBlackIronInnerCorner => "teramic::train_black_iron_inner_corner",
            Self::TrainBlackIronOuterCorner => "teramic::train_black_iron_outer_corner",
            Self::TrainBlackIronSlope => "teramic::train_black_iron_slope",
            Self::TrainDarkSteelBlock => "teramic::train_dark_steel_block",
            Self::TrainDarkSteelSlab => "teramic::train_dark_steel_slab",
            Self::TrainDarkSteelStairs => "teramic::train_dark_steel_stairs",
            Self::TrainDarkSteelInnerCorner => "teramic::train_dark_steel_inner_corner",
            Self::TrainDarkSteelOuterCorner => "teramic::train_dark_steel_outer_corner",
            Self::TrainDarkSteelSlope => "teramic::train_dark_steel_slope",
            Self::TrainSheetMetalBlock => "teramic::train_sheet_metal_block",
            Self::TrainSheetMetalSlab => "teramic::train_sheet_metal_slab",
            Self::TrainSheetMetalStairs => "teramic::train_sheet_metal_stairs",
            Self::TrainSheetMetalInnerCorner => "teramic::train_sheet_metal_inner_corner",
            Self::TrainSheetMetalOuterCorner => "teramic::train_sheet_metal_outer_corner",
            Self::TrainSheetMetalSlope => "teramic::train_sheet_metal_slope",
            Self::TrainGrateBlock => "teramic::train_grate_block",
            Self::TrainGrateSlab => "teramic::train_grate_slab",
            Self::TrainGrateStairs => "teramic::train_grate_stairs",
            Self::TrainGrateInnerCorner => "teramic::train_grate_inner_corner",
            Self::TrainGrateOuterCorner => "teramic::train_grate_outer_corner",
            Self::TrainGrateSlope => "teramic::train_grate_slope",
            Self::TrainCeilingLight => "teramic::train_ceiling_light",
            // F54_identifiers_END
            Self::RailTurntable => "teramic::rail_turntable",
            Self::TrainRailBuilder => "teramic::train_rail_builder",
            Self::LumberjackStation => "teramic::lumberjack_station",
            Self::SmithStation => "teramic::smith_station",
            Self::MarketStation => "teramic::market_station",
            Self::ArmyStation => "teramic::army_station",
            Self::RailwayStation => "teramic::railway_station",
            Self::MineStation => "teramic::mine_station",
            Self::ButcherStation => "teramic::butcher_station",
            Self::BishopStation => "teramic::bishop_station",
            Self::FarmStation => "teramic::farm_station",
            Self::Lantern => "teramic::lantern",
            Self::Chain => "teramic::chain",
            Self::WoodPipe => "teramic::wood_pipe",
            Self::StonePipe => "teramic::stone_pipe",
            Self::IronPipe => "teramic::iron_pipe",
            Self::Valve => "teramic::valve",
            Self::Faucet => "teramic::faucet",
            Self::Pump => "teramic::pump",
            Self::Basin => "teramic::basin",
            Self::HoldBlock => "teramic::hold_block",
            Self::FluidInfuser => "teramic::fluid_infuser",
            Self::GlassPipe => "teramic::glass_pipe",
            Self::Bergfried => "teramic::bergfried",
            Self::CastleStoneWall => "teramic::castle_stone_wall",
            Self::BergfriedWall => "teramic::bergfried_wall",
            Self::CastleCrackedWall => "teramic::castle_cracked_wall",
            Self::CastleOvergrownWall => "teramic::castle_overgrown_wall",
            Self::CastleMossWall => "teramic::castle_moss_wall",
            Self::CastleIronWall => "teramic::castle_iron_wall",
            Self::CastleBattlementStraight => "teramic::castle_battlement_straight",
            Self::CastleBattlementCorner => "teramic::castle_battlement_corner",
            Self::CastleBattlementPillar => "teramic::castle_battlement_pillar",
            Self::CastleArchLeft => "teramic::castle_arch_left",
            Self::CastleArchRight => "teramic::castle_arch_right",
            Self::CastleArrowSlit => "teramic::castle_arrow_slit",
            Self::CastleWindow => "teramic::castle_window",
            Self::CastleSandstoneWall => "teramic::castle_sandstone_wall",
            Self::CastleDesertWall => "teramic::castle_desert_wall",
            Self::CastleSnowWall => "teramic::castle_snow_wall",
            Self::CastleBlackstoneWall => "teramic::castle_blackstone_wall",
            Self::CastleRoyalWall => "teramic::castle_royal_wall",
            Self::CastleTimberWall => "teramic::castle_timber_wall",
            Self::GearboxHorizontal => "teramic::gearbox_horizontal",
            Self::GearboxVertical => "teramic::gearbox_vertical",
            Self::Mixer => "teramic::mixer",
            Self::FilteredFunnel => "teramic::filtered_funnel",
            Self::BeltMerger => "teramic::belt_merger",
            Self::FilteredTunnel => "teramic::filtered_tunnel",
            Self::BeltDistributor => "teramic::belt_distributor",
            Self::Cauldron => "teramic::cauldron",
            Self::Campfire => "teramic::campfire",
            Self::ItemHopper => "teramic::item_hopper",
            Self::ItemImporter => "teramic::item_importer",
            Self::WoodenGear => "teramic::wooden_gear",
            Self::StoneGear => "teramic::stone_gear",
            Self::IronGear => "teramic::iron_gear",
            Self::WoodenShaft => "teramic::wooden_shaft",
            Self::StoneShaft => "teramic::stone_shaft",
            Self::IronShaft => "teramic::iron_shaft",
            Self::WaterWheel => "teramic::water_wheel",
            Self::Conveyor => "teramic::conveyor",
            Self::WoodenLargeGear => "teramic::wooden_large_gear",
            Self::StoneLargeGear => "teramic::stone_large_gear",
            Self::IronLargeGear => "teramic::iron_large_gear",
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
            141 => Self::WoodenGear,
            142 => Self::StoneGear,
            143 => Self::IronGear,
            144 => Self::WoodenShaft,
            145 => Self::StoneShaft,
            146 => Self::IronShaft,
            147 => Self::WaterWheel,
            148 => Self::Conveyor,
            149 => Self::WoodenLargeGear,
            150 => Self::StoneLargeGear,
            151 => Self::IronLargeGear,
            152 => Self::ItemHopper,
            153 => Self::ItemImporter,
            154 => Self::GearboxHorizontal,
            155 => Self::GearboxVertical,
            156 => Self::Mixer,
            157 => Self::FilteredFunnel,
            158 => Self::BeltMerger,
            159 => Self::FilteredTunnel,
            160 => Self::BeltDistributor,
            161 => Self::Cauldron,
            162 => Self::Campfire,
            163 => Self::WoodPipe,
            164 => Self::StonePipe,
            165 => Self::IronPipe,
            166 => Self::Valve,
            167 => Self::Faucet,
            168 => Self::Pump,
            169 => Self::Basin,
            170 => Self::HoldBlock,
            171 => Self::FluidInfuser,
            172 => Self::GlassPipe,
            173 => Self::Bergfried,
            174 => Self::CastleStoneWall,
            175 => Self::BergfriedWall,
            176 => Self::CastleCrackedWall,
            177 => Self::CastleOvergrownWall,
            178 => Self::CastleMossWall,
            179 => Self::CastleIronWall,
            180 => Self::CastleBattlementStraight,
            181 => Self::CastleBattlementCorner,
            182 => Self::CastleBattlementPillar,
            183 => Self::CastleArchLeft,
            184 => Self::CastleArchRight,
            185 => Self::CastleArrowSlit,
            186 => Self::CastleWindow,
            187 => Self::CastleSandstoneWall,
            188 => Self::CastleDesertWall,
            189 => Self::CastleSnowWall,
            190 => Self::CastleBlackstoneWall,
            191 => Self::CastleRoyalWall,
            192 => Self::CastleTimberWall,
            195 => Self::AppleLeaves,
            196 => Self::RailStraight,
            197 => Self::RailCurveSmall,
            198 => Self::RailCurveLarge,
            199 => Self::RailSlope,
            200 => Self::RailSlopeEntry,
            201 => Self::RailSlopeExit,
            202 => Self::RailSwitchLeft,
            203 => Self::RailSwitchRight,
            204 => Self::RailCrossing,
            205 => Self::RailBuffer,
            206 => Self::Ballast,
            207 => Self::BallastStairs,
            208 => Self::BallastSlab,
            209 => Self::RailSwitchStand,
            210 => Self::TrainChassis,
            211 => Self::TrainBogie,
            212 => Self::TrainFrame,
            213 => Self::TrainCoupler,
            214 => Self::TrainCab,
            215 => Self::TrainSeat,
            216 => Self::TrainCargo,
            217 => Self::TrainAssemblyStation,
            // F54_raw_START
            218 => Self::TrainFuelEngine,
            219 => Self::TrainBoilerStraight,
            220 => Self::TrainBoilerCurved,
            221 => Self::TrainBoilerEnd,
            222 => Self::TrainSmokebox,
            223 => Self::TrainSmokeboxDoor,
            224 => Self::TrainChimney,
            225 => Self::TrainSteamDome,
            226 => Self::TrainSandDome,
            227 => Self::TrainBrassPipeStraight,
            228 => Self::TrainBrassPipeBend,
            229 => Self::TrainSteamValve,
            230 => Self::TrainPressureGauge,
            231 => Self::TrainPressureUnit,
            232 => Self::TrainWheelLarge,
            233 => Self::TrainWheelMedium,
            234 => Self::TrainWheelSmall,
            235 => Self::TrainAxle,
            236 => Self::TrainConnectingRodLong,
            237 => Self::TrainConnectingRodShort,
            238 => Self::TrainRodJoint,
            239 => Self::TrainPistonRod,
            240 => Self::TrainCrosshead,
            241 => Self::TrainMainFrame,
            242 => Self::TrainRedFrame,
            243 => Self::TrainBufferBeam,
            244 => Self::TrainBuffer,
            245 => Self::TrainCabWallSide,
            246 => Self::TrainCabWallFront,
            247 => Self::TrainCabWindow,
            248 => Self::TrainCabDoor,
            249 => Self::TrainRoofCurved,
            250 => Self::TrainRoofFlat,
            251 => Self::TrainRoofVent,
            252 => Self::TrainTenderWall,
            253 => Self::TrainTenderFront,
            254 => Self::TrainWaterTank,
            255 => Self::TrainCoalLoad,
            256 => Self::TrainLadderSide,
            257 => Self::TrainLadderFront,
            258 => Self::TrainRailing,
            259 => Self::TrainHandrail,
            260 => Self::TrainHandrailPost,
            261 => Self::TrainRivetPlate,
            262 => Self::TrainBolt,
            263 => Self::TrainHatch,
            264 => Self::TrainHeadlightFront,
            265 => Self::TrainHeadlightSide,
            266 => Self::TrainNumberPlate,
            267 => Self::TrainToolbox,
            268 => Self::TrainWoodenPlank,
            269 => Self::TrainCoalPiece,
            270 => Self::TrainBlackIronBlock,
            271 => Self::TrainBlackIronSlab,
            272 => Self::TrainBlackIronStairs,
            273 => Self::TrainBlackIronInnerCorner,
            274 => Self::TrainBlackIronOuterCorner,
            275 => Self::TrainBlackIronSlope,
            276 => Self::TrainDarkSteelBlock,
            277 => Self::TrainDarkSteelSlab,
            278 => Self::TrainDarkSteelStairs,
            279 => Self::TrainDarkSteelInnerCorner,
            280 => Self::TrainDarkSteelOuterCorner,
            281 => Self::TrainDarkSteelSlope,
            282 => Self::TrainSheetMetalBlock,
            283 => Self::TrainSheetMetalSlab,
            284 => Self::TrainSheetMetalStairs,
            285 => Self::TrainSheetMetalInnerCorner,
            286 => Self::TrainSheetMetalOuterCorner,
            287 => Self::TrainSheetMetalSlope,
            288 => Self::TrainGrateBlock,
            289 => Self::TrainGrateSlab,
            290 => Self::TrainGrateStairs,
            291 => Self::TrainGrateInnerCorner,
            292 => Self::TrainGrateOuterCorner,
            293 => Self::TrainGrateSlope,
            294 => Self::TrainCeilingLight,
            // F54_raw_END
            295 => Self::RailTurntable,
            296 => Self::TrainRailBuilder,
            297 => Self::LumberjackStation,
            298 => Self::SmithStation,
            299 => Self::MarketStation,
            300 => Self::ArmyStation,
            301 => Self::RailwayStation,
            302 => Self::MineStation,
            303 => Self::ButcherStation,
            304 => Self::BishopStation,
            305 => Self::FarmStation,
            306 => Self::SpiritBlossomGrass,
            307 => Self::SpiritBlossomDirt,
            308 => Self::SpiritStone,
            309 => Self::BlossomPetalBlock,
            310 => Self::PinkBlossomLeaves,
            311 => Self::WhiteBlossomLeaves,
            312 => Self::LavenderBlossomLeaves,
            313 => Self::SpiritBlossomLog,
            314 => Self::SpiritBlossomPlanks,
            315 => Self::GlowingLanternStone,
            316 => Self::GlowingSpiritFlowers,
            317 => Self::BlossomBush,
            318 => Self::SpiritLanternPost,
            319 => Self::LotusFlower,
            320 => Self::LilyPad,
            321 => Self::HangingBlossomLantern,
            322 => Self::PetalGroundCover,
            323 => Self::SpiritShrineStone,
            324 => Self::BlueClay,
            325 => Self::RedClay,
            326 => Self::GreenClay,
            327 => Self::YellowClay,
            328 => Self::WhiteClay,
            329 => Self::BlackClay,
            330 => Self::GrayClay,
            331 => Self::DarkGrayClay,
            332 => Self::PurpleClay,
            333 => Self::PinkClay,
            334 => Self::LimeClay,
            335 => Self::TurquoiseClay,
            336 => Self::OrangeClay,
            337 => Self::GraveStone,
            338 => Self::DarkGraveStone,
            339 => Self::CrackedGraveStone,
            340 => Self::OrnateGraveStone,
            341 => Self::MossGraveStone,
            342 => Self::OvergrownGraveStone,
            343 => Self::CryptBrick,
            344 => Self::GraveStoneSlab,
            345 => Self::OrnateGraveSlab,
            346 => Self::MosaicGraveSlab,
            347 => Self::CrackedGraveSlab,
            348 => Self::InscribedGraveSlab,
            349 => Self::CryptColumn,
            350 => Self::OrnateCryptColumn,
            351 => Self::CryptCapital,
            352 => Self::CryptColumnBase,
            353 => Self::CryptArch,
            354 => Self::MossyCryptArch,
            355 => Self::GraveReliefWall,
            356 => Self::SkullRelief,
            357 => Self::CrossRelief,
            358 => Self::CryptWallNiche,
            359 => Self::CryptHeraldicBanner,
            360 => Self::CryptTornBanner,
            361 => Self::CryptCurtain,
            362 => Self::GravestoneSmall,
            363 => Self::GravestoneMedium,
            364 => Self::GravestoneLarge,
            365 => Self::GraveCross,
            366 => Self::BorderedGrave,
            367 => Self::SarcophagusClosed,
            368 => Self::SarcophagusOpen,
            369 => Self::GraveLanternPost,
            370 => Self::CryptWallTorch,
            371 => Self::CryptCandles,
            372 => Self::CryptChandelier,
            373 => Self::SoulBrazier,
            374 => Self::SkeletonRemains,
            375 => Self::BonePile,
            376 => Self::GraveUrn,
            377 => Self::OrnateCryptChest,
            378 => Self::CryptBooks,
            379 => Self::GraveSkull,
            380 => Self::CryptChain,
            381 => Self::DeadGrass,
            382 => Self::ThornBush,
            383 => Self::HangingIvy,
            384 => Self::GraveMoss,
            385 => Self::DarkMushrooms,
            386 => Self::GlowingPurpleMushrooms,
            387 => Self::BlossomingVines,
            388 => Self::CryptPressurePlate,
            389 => Self::CryptLever,
            390 => Self::CryptWallSwitch,
            391 => Self::CryptPortcullis,
            392 => Self::LockedCryptGate,
            393 => Self::CryptSpikeTrap,
            394 => Self::CryptArrowTrap,
            395 => Self::SoulCrystalSmall,
            396 => Self::SoulCrystalLarge,
            397 => Self::ActiveRuneBlock,
            398 => Self::InactiveRuneBlock,
            399 => Self::MysticMistBlock,
            400 => Self::BloodGrass,
            401 => Self::BloodDirt,
            402 => Self::BloodStone,
            403 => Self::VeinedStone,
            404 => Self::BloodwoodPlanks,
            405 => Self::BloodwoodLog,
            406 => Self::BloodLeaves,
            407 => Self::CrimsonMycelium,
            408 => Self::BloodMoss,
            409 => Self::BloodCrystalBlock,
            410 => Self::BloodFungalBlock,
            411 => Self::BloodGlowMushroomBlock,
            412 => Self::BloodMushrooms,
            413 => Self::BloodThornBush,
            414 => Self::BloodFlower,
            415 => Self::HangingBloodVines,
            416 => Self::BloodRoots,
            417 => Self::BloodBones,
            418 => Self::BloodFern,
            419 => Self::GiantBloodMushroom,
            420 => Self::BloodRuinedArch,
            421 => Self::BloodCrystalPillar,
            422 => Self::Cactus,
            423 => Self::MeadowDaisies,
            424 => Self::Lupine,
            425 => Self::ForestFern,
            426 => Self::FloweringShrub,
            427 => Self::AlpineHeather,
            428 => Self::DesertShrub,
            429 => Self::ShoreReeds,
            430 => Self::FogBlock,
            431 => Self::Asphalt,
            432 => Self::AsphaltCover,
            433 => Self::AsphaltSlope,
            434 => Self::AsphaltEntry,
            435 => Self::AsphaltExit,
            436 => Self::RoadCurb,
            437 => Self::WetTar,
            438 => Self::DriedTar,
            439 => Self::Bile,
            440 => Self::BileBlock,
            441 => Self::SidewalkBlock,
            442 => Self::Sidewalk,
            443 => Self::SidewalkLeft,
            444 => Self::SidewalkRight,
            445 => Self::SidewalkArcLeft,
            446 => Self::SidewalkArcRight,
            447 => Self::SidewalkLowered,
            448 => Self::StrawBlock,
            449 => Self::DarkDragonEgg,
            450 => Self::EarthDragonEgg,
            451 => Self::WaterDragonEgg,
            452 => Self::AmberGeode,
            453 => Self::TideGeode,
            454 => Self::SaltCaveStone,
            455 => Self::WetSaltStone,
            456 => Self::SaltDustFloor,
            457 => Self::DeepSaltStone,
            458 => Self::TranslucentSaltBlock,
            459 => Self::SaltCrystalCluster,
            460 => Self::SaltStalactite,
            461 => Self::SaltStalagmite,
            462 => Self::SaltRubble,
            463 => Self::SaltPillarFragment,
            464 => Self::NestStraw,
            465 => Self::WetCaveStone,
            466 => Self::CaveCalcite,
            467 => Self::CavePebbles,
            468 => Self::CaveCrystalCluster,
            469 => Self::MossyCaveStone,
            470 => Self::CaveMud,
            471 => Self::LushCaveStone,
            472 => Self::OvergrownStone,
            473 => Self::RootedCaveDirt,
            474 => Self::CaveGrass,
            475 => Self::CaveMoss,
            476 => Self::GlowMoss,
            477 => Self::ClearCaveIce,
            478 => Self::PackedCaveIce,
            479 => Self::FrostedCaveIce,
            480 => Self::FrozenStone,
            481 => Self::SnowDustStone,
            482 => Self::BlueCrystalBlock,
            483 => Self::GlowVine,
            484 => Self::FrostVine,
            485 => Self::FrostCluster,
            486 => Self::GlacialCrystalCluster,
            487 => Self::CaveWildflowers,

            193 => Self::Lantern,
            194 => Self::Chain,
            _ => return None,
        })
    }

    /// Fluid anchors and torches remain selectable without blocking movement.
    pub const fn is_solid(self) -> bool {
        if matches!(self as u16, 359..=361 | 370..=372 | 374..=375 | 378..=387 | 399 | 412..=419 | 423..=429 | 483..=485 | 487)
        {
            return false;
        }
        !matches!(
            self,
            Self::Air
                | Self::NestStraw
                | Self::Bile
                | Self::FluidWater
                | Self::Torch
                | Self::Lantern
                | Self::Chain
                | Self::Seagrass
                | Self::CaveVine
                | Self::CaveRoot
                | Self::CaveMushroom
                | Self::GlowingSpiritFlowers
                | Self::BlossomBush
                | Self::LotusFlower
                | Self::LilyPad
                | Self::HangingBlossomLantern
                | Self::PetalGroundCover
        )
    }

    pub const fn is_opaque(self) -> bool {
        if matches!(self, Self::BileBlock | Self::StrawBlock) {
            return true;
        }
        if matches!(self as u16, 324..=343 | 397..=398 | 400..=405 | 407..=411 | 422 | 430..=431 | 440 | 454..=457 | 465..=466 | 469..=476 | 478..=482)
        {
            return true;
        }
        if matches!(
            self,
            Self::Ballast
                | Self::SpiritBlossomGrass
                | Self::SpiritBlossomDirt
                | Self::SpiritStone
                | Self::BlossomPetalBlock
                | Self::SpiritBlossomLog
                | Self::SpiritBlossomPlanks
                | Self::GlowingLanternStone
        ) {
            return true;
        }
        if (self as u16) >= 141 && (self as u16) <= Self::LAST {
            return false;
        }
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
                | Self::AppleLeaves
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
            Self::CaveGrass
                | Self::CaveMoss
                | Self::RootedCaveDirt
                | Self::Grass
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
                | Self::SpiritBlossomGrass
                | Self::SpiritBlossomDirt
                | Self::SpiritStone
                | Self::BloodGrass
                | Self::BloodDirt
                | Self::BloodStone
                | Self::CrimsonMycelium
                | Self::BloodMoss
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
        if matches!(self, Self::StrawBlock | Self::NestStraw) {
            return true;
        }
        if matches!(self as u16, 404..=406 | 412..=416 | 418..=419 | 422..=429) {
            return true;
        }
        if matches!(self as u16, 359..=361 | 377..=378 | 381..=387 | 392 | 309..=314 | 316..=317 | 319..=322 | 74..=76 | 82..=84 | 90..=92 | 103..=130 | 134..=140)
        {
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
                | Self::AppleLeaves
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
    fn tar_coatings_append_after_roads_and_keep_partial_shape_neighbors_visible() {
        assert_eq!(MaterialId::RoadCurb as u16, 436);
        for (id, material, identifier) in [
            (437, MaterialId::WetTar, "teramic::wet_tar"),
            (438, MaterialId::DriedTar, "teramic::dried_tar"),
        ] {
            assert_eq!(material as u16, id);
            assert_eq!(MaterialId::from_raw(id), Some(material));
            assert_eq!(material.identifier(), identifier);
            assert!(material.is_solid());
            assert!(!material.is_opaque());
        }
    }

    #[test]
    fn regional_flora_appends_ids_and_cactus_uses_real_solid_voxels() {
        assert_eq!(MaterialId::BloodCrystalPillar as u16, 421);
        assert_eq!(MaterialId::Cactus as u16, 422);
        assert!(MaterialId::Cactus.is_solid() && MaterialId::Cactus.is_opaque());
        for id in 423..=429 {
            let material = MaterialId::from_raw(id).unwrap();
            assert!(!material.is_solid() && !material.is_opaque());
            assert!(material.flammable());
            assert!(material.identifier().starts_with("teramic::"));
        }
    }
    #[test]
    fn ballast_and_switch_stand_append_ids_without_changing_old_rail_codes() {
        for (id, material, identifier, opaque) in [
            (205, MaterialId::RailBuffer, "teramic::rail_buffer", false),
            (206, MaterialId::Ballast, "teramic::ballast", true),
            (
                207,
                MaterialId::BallastStairs,
                "teramic::ballast_stairs",
                false,
            ),
            (208, MaterialId::BallastSlab, "teramic::ballast_slab", false),
            (
                209,
                MaterialId::RailSwitchStand,
                "teramic::rail_switch_stand",
                false,
            ),
        ] {
            assert_eq!(material as u16, id);
            assert_eq!(MaterialId::from_raw(id), Some(material));
            assert_eq!(material.identifier(), identifier);
            assert!(material.is_solid());
            assert_eq!(material.is_opaque(), opaque);
            assert!(!material.flammable());
        }
        assert_eq!(MaterialId::LAST, 487);
    }

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

#[cfg(test)]
mod spirit_blossom_tests {
    use super::MaterialId;

    #[test]
    fn spirit_materials_append_stable_ids_and_distinguish_ground_foliage_and_decorations() {
        assert_eq!(MaterialId::FarmStation as u16, 305);
        for raw in 306..=323 {
            let material = MaterialId::from_raw(raw).unwrap();
            assert_eq!(material as u16, raw);
            assert!(material.identifier().starts_with("teramic::"));
        }
        for raw in [306, 307, 308] {
            assert!(MaterialId::from_raw(raw).unwrap().is_natural_ground());
        }
        for raw in [310, 311, 312] {
            assert!(MaterialId::FOLIAGE.contains(&raw));
            assert!(!MaterialId::from_raw(raw).unwrap().is_opaque());
        }
        for raw in [316, 317, 319, 320, 321, 322] {
            assert!(!MaterialId::from_raw(raw).unwrap().is_solid());
        }
        for raw in [306, 307, 308, 309, 313, 314, 315] {
            assert!(MaterialId::from_raw(raw).unwrap().is_opaque());
        }
    }
}

#[cfg(test)]
mod blood_forest_tests {
    use super::MaterialId;

    #[test]
    fn bloodwood_and_plants_burn_but_stone_crystals_and_bones_do_not() {
        for material in [
            MaterialId::BloodwoodPlanks,
            MaterialId::BloodwoodLog,
            MaterialId::BloodLeaves,
            MaterialId::BloodMushrooms,
            MaterialId::BloodThornBush,
            MaterialId::BloodFlower,
            MaterialId::HangingBloodVines,
            MaterialId::BloodRoots,
            MaterialId::BloodFern,
            MaterialId::GiantBloodMushroom,
        ] {
            assert!(material.flammable(), "{material:?}");
        }
        for material in [
            MaterialId::BloodStone,
            MaterialId::BloodCrystalBlock,
            MaterialId::BloodBones,
            MaterialId::BloodRuinedArch,
            MaterialId::BloodCrystalPillar,
        ] {
            assert!(!material.flammable(), "{material:?}");
        }
    }
}

#[cfg(test)]
mod cemetery_tests {
    use super::MaterialId;

    #[test]
    fn cemetery_ids_preserve_terrain_seams_and_nonblocking_decoration() {
        assert_eq!(MaterialId::SpiritShrineStone as u16, 323);
        assert_eq!(MaterialId::BlueClay as u16, 324);
        assert_eq!(MaterialId::MysticMistBlock as u16, 399);
        let mut names = std::collections::HashSet::new();
        for id in 324..=399 {
            let material = MaterialId::from_raw(id).unwrap();
            assert!(names.insert(material.identifier()));
            assert_eq!(
                material.is_opaque(),
                matches!(id, 324..=343 | 397..=398 | 400..=405 | 407..=411 | 422 | 430)
            );
            assert_eq!(
                material.is_solid(),
                !matches!(id, 359..=361 | 370..=372 | 374..=375 | 378..=387 | 399 | 412..=419 | 423..=429 | 483..=485 | 487)
            );
        }
    }
}
