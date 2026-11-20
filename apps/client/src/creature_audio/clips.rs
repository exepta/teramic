use bevy::audio::{AudioSource, Decodable, Source};
use std::{io::Cursor, sync::Arc};

pub const RAIN_CLIP: usize = 73;
pub const RAIN_DETAIL_CLIP: usize = 74;
pub const SWIM_CLIPS: [usize; 4] = [
    PATHS.len() + 5,
    PATHS.len() + 6,
    PATHS.len() + 7,
    PATHS.len() + 8,
];
pub const SPIDER_STEP_CLIP: usize = PATHS.len() + 9;
pub const SPIDER_IDLE_CLIP: usize = PATHS.len() + 10;
pub const SPIDER_HISS_CLIP: usize = PATHS.len() + 11;
pub const CHEST_OPEN_CLIP: usize = PATHS.len() + 12;
pub const CHEST_CLOSE_CLIP: usize = PATHS.len() + 13;
pub const STALKER_BREATH_CLIP: usize = PATHS.len() + 14;
pub const STALKER_SCRATCH_CLIP: usize = PATHS.len() + 15;
pub const STALKER_SCREAM_CLIP: usize = PATHS.len() + 16;
pub const STALKER_FAST_BREATH_CLIP: usize = PATHS.len() + 17;
pub const WET_CONTACT_CLIPS: [usize; 2] = [78, 79];
pub const SQUID_JET_CLIP: usize = PATHS.len() + 1;
pub const JELLY_PULSE_CLIP: usize = PATHS.len() + 2;
pub const SHARK_STROKE_CLIP: usize = PATHS.len() + 3;
pub const INK_JET_CLIP: usize = PATHS.len() + 4;
pub const DOOR_OPEN_CLIP: usize = 80;
pub const DOOR_CLOSE_CLIP: usize = 81;
pub const COUGH_CLIPS: [usize; 3] = [82, 84, 85];
pub const FURNACE_CLIP: usize = 83;
pub const STALKER_AIRWAY_CLIP: usize = 86;
pub const PATHS: [&str; 91] = [
    "animals/sheep-bleat.wav",
    "animals/sheep-bleat-2.wav",
    "animals/cow-moo.wav",
    "animals/cow-moo-2.wav",
    "animals/pig-grunt.wav",
    "animals/pig-grunt-2.wav",
    "animals/eat.wav",
    "animals/step.wav",
    "animals/sheep-shake.wav",
    "animals/swim.wav",
    "animals/drink.wav",
    "environment/birds-soft.wav",
    "environment/crickets.wav",
    "environment/cave.wav",
    "footsteps/detail/grass.wav",
    "footsteps/detail/grass-2.wav",
    "footsteps/detail/sand.wav",
    "footsteps/detail/sand-2.wav",
    "footsteps/detail/stone.wav",
    "footsteps/detail/stone-2.wav",
    "footsteps/detail/dirt.wav",
    "footsteps/detail/dirt-2.wav",
    "footsteps/detail/snow.wav",
    "footsteps/detail/snow-2.wav",
    "footsteps/detail/wood.wav",
    "footsteps/detail/wood-2.wav",
    "animals/bee-flight.wav",
    "animals/bee-land.wav",
    "animals/bee-enter.wav",
    "animals/bee-leave.wav",
    "animals/splash.wav",
    "animals/swim-stroke.wav",
    "environment/torch-fire.wav",
    "animals/sleep-breath.wav",
    "footsteps/detail/grass-3.wav",
    "footsteps/detail/grass-4.wav",
    "footsteps/detail/grass-5.wav",
    "footsteps/detail/sand-3.wav",
    "footsteps/detail/sand-4.wav",
    "footsteps/detail/sand-5.wav",
    "footsteps/detail/sand-6.wav",
    "footsteps/detail/stone-3.wav",
    "footsteps/detail/stone-4.wav",
    "footsteps/detail/stone-5.wav",
    "footsteps/detail/stone-6.wav",
    "footsteps/detail/dirt-3.wav",
    "footsteps/detail/dirt-4.wav",
    "footsteps/detail/dirt-5.wav",
    "footsteps/detail/dirt-6.wav",
    "footsteps/detail/snow-3.wav",
    "footsteps/detail/snow-4.wav",
    "footsteps/detail/snow-5.wav",
    "footsteps/detail/snow-6.wav",
    "footsteps/detail/wood-3.wav",
    "footsteps/detail/wood-4.wav",
    "footsteps/detail/wood-5.wav",
    "footsteps/detail/gravel.wav",
    "footsteps/detail/gravel-2.wav",
    "footsteps/detail/gravel-3.wav",
    "footsteps/detail/gravel-4.wav",
    "footsteps/detail/gravel-5.wav",
    "footsteps/detail/gravel-6.wav",
    "footsteps/detail/gravel-7.wav",
    "footsteps/detail/gravel-8.wav",
    "footsteps/detail/gravel-9.wav",
    "footsteps/detail/gravel-10.wav",
    "footsteps/detail/glass.wav",
    "footsteps/detail/glass-2.wav",
    "footsteps/detail/glass-3.wav",
    "footsteps/detail/glass-4.wav",
    "footsteps/detail/glass-5.wav",
    "footsteps/detail/leaves.wav",
    "footsteps/detail/leaves-2.wav",
    "environment/rain.wav",
    "environment/rain-detail.wav",
    "animals/swim-stroke-2.wav",
    "animals/swim-stroke-3.wav",
    "animals/swim-stroke-4.wav",
    "footsteps/wet-film-1.wav",
    "footsteps/wet-film-2.wav",
    "objects/door-open.wav",
    "objects/door-close.wav",
    "player/cough.wav",
    "objects/furnace-fire.wav",
    "player/cough-2.wav",
    "player/cough-3.wav",
    "animals/stalker-airway.wav",
    "environment/wind-ambience.wav",
    "environment/crows.wav",
    "environment/ocean-surf.wav",
    "environment/woodland-birds.wav",
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Surface {
    Grass,
    Sand,
    Stone,
    Dirt,
    Snow,
    Wood,
    Gravel,
    Glass,
    Leaves,
}
impl Surface {
    pub(super) fn material(raw: u16) -> Self {
        if raw == terra_core::rails::TURNTABLE {
            return Self::Stone;
        }
        use terra_voxel::MaterialId::*;
        match terra_voxel::MaterialId::from_raw(raw) {
            Some(material)
                if terra_core::textiles::is_wool(material as u16)
                    || terra_core::textiles::is_carpet(material as u16) =>
            {
                Self::Dirt
            }
            Some(material) if terra_core::locomotive_parts::is_part(material as u16) => {
                if terra_core::locomotive_parts::supports_wood(material as u16) {
                    Self::Wood
                } else {
                    Self::Stone
                }
            }
            Some(LumberjackStation | MarketStation | ButcherStation | FarmStation) => Self::Wood,
            Some(SmithStation | ArmyStation | RailwayStation | MineStation | BishopStation) => {
                Self::Stone
            }
            Some(Grass | SpiritBlossomGrass | BloodGrass | BloodMoss) => Self::Grass,
            Some(DeadGrass) => Self::Grass,
            Some(
                BlueClay | RedClay | GreenClay | YellowClay | WhiteClay | BlackClay | GrayClay
                | DarkGrayClay | PurpleClay | PinkClay | LimeClay | TurquoiseClay | OrangeClay
                | CryptHeraldicBanner | CryptTornBanner | CryptCurtain | CryptCandles | CryptBooks,
            ) => Self::Dirt,
            Some(OrnateCryptChest | CryptWallTorch) => Self::Wood,
            Some(
                ThornBush
                | HangingIvy
                | GraveMoss
                | DarkMushrooms
                | GlowingPurpleMushrooms
                | BlossomingVines,
            ) => Self::Leaves,
            Some(SoulCrystalSmall | SoulCrystalLarge | BloodCrystalBlock | BloodCrystalPillar) => {
                Self::Glass
            }
            Some(Sand) => Self::Sand,
            Some(
                Dirt
                | Farmland
                | Path
                | Clay
                | Mud
                | MushroomStem
                | PurpleMushroomCap
                | RedMushroomCap
                | YellowMushroomCap
                | GreenMushroomCap
                | BlueMushroomCap
                | WhiteMushroomCap
                | Conveyor
                | TrainSeat
                | SpiritBlossomDirt
                | BloodDirt
                | CrimsonMycelium
                | BloodFungalBlock
                | BloodGlowMushroomBlock
                | GiantBloodMushroom
                | WetTar
                | Bile
                | StrawBlock,
            ) => Self::Dirt,
            Some(Snow | SnowLayer | CastleSnowWall) => Self::Snow,
            Some(material) if terra_core::rails::is_rail(material as u16) => Self::Wood,
            Some(
                Wood
                | WeatheredWood
                | OakPlanks
                | BirchPlanks
                | SprucePlanks
                | Workbench
                | Chest
                | SpruceLog
                | Torch
                | OakDoor
                | BirchDoor
                | SpruceDoor
                | OakWindow
                | BirchWindow
                | SpruceWindow
                | OakBed
                | BirchBed
                | SpruceBed
                | HandCrank
                | CherryLog
                | CherryPlanks
                | CherryDoor
                | CherryWindow
                | WoodenGear
                | WoodenShaft
                | WaterWheel
                | WoodenLargeGear
                | ItemHopper
                | ItemImporter
                | GearboxHorizontal
                | GearboxVertical
                | Mixer
                | FilteredFunnel
                | BeltMerger
                | FilteredTunnel
                | BeltDistributor
                | Campfire
                | WoodPipe
                | Basin
                | HoldBlock
                | CastleTimberWall
                | TrainCargo
                | TrainAssemblyStation
                | SpiritBlossomLog
                | SpiritBlossomPlanks
                | HangingBlossomLantern
                | BloodwoodLog
                | BloodwoodPlanks
                | BloodRoots
                | Cactus,
            ) => Self::Wood,
            Some(Gravel | Ballast | BallastStairs | BallastSlab) => Self::Gravel,
            Some(Glass | GlassPipe | Ice | SaltCrystal | IceCrystal) => Self::Glass,
            Some(
                Leaves
                | AutumnLeaves
                | SilverLeaves
                | CherryLeaves
                | WhiteCherryLeaves
                | AppleLeaves
                | Seagrass
                | CaveVine
                | CaveMushroom
                | BlossomPetalBlock
                | PinkBlossomLeaves
                | WhiteBlossomLeaves
                | LavenderBlossomLeaves
                | GlowingSpiritFlowers
                | BlossomBush
                | LotusFlower
                | LilyPad
                | PetalGroundCover
                | BloodLeaves
                | BloodMushrooms
                | BloodThornBush
                | BloodFlower
                | HangingBloodVines
                | BloodFern
                | MeadowDaisies
                | Lupine
                | ForestFern
                | FloweringShrub
                | AlpineHeather
                | DesertShrub
                | ShoreReeds,
            ) => Self::Leaves,
            Some(CaveRoot) => Self::Wood,
            Some(DripstoneTip | terra_voxel::MaterialId::FlowerPot) => Self::Stone,
            Some(
                Air
                | Stone
                | DriedTar
                | Sandstone
                | SmoothStone
                | MossyStone
                | SolidWater
                | FluidWater
                | SaltOre
                | IronOre
                | CoalOre
                | GoldOre
                | DiamondOre
                | RubyOre
                | Dripstone
                | SaltFormation
                | DripstoneFormation
                | Bedrock
                | StructureStone
                | CoralRed
                | CoralBlue
                | CoralYellow
                | CoralPurple
                | Furnace
                | ChimneyStraight
                | ChimneyElbow
                | ChimneyHorizontal
                | ChimneyBendUp
                | ChimneyBendDown
                | Crusher
                | StoneGear
                | IronGear
                | StoneShaft
                | IronShaft
                | StoneLargeGear
                | IronLargeGear
                | Cauldron
                | FluidInfuser
                | Lantern
                | Chain
                | RailSwitchStand
                | Bergfried
                | CastleStoneWall
                | BergfriedWall
                | CastleCrackedWall
                | CastleOvergrownWall
                | CastleMossWall
                | CastleIronWall
                | CastleBattlementStraight
                | CastleBattlementCorner
                | CastleBattlementPillar
                | CastleArchLeft
                | CastleArchRight
                | CastleArrowSlit
                | CastleWindow
                | CastleSandstoneWall
                | CastleDesertWall
                | CastleBlackstoneWall
                | CastleRoyalWall,
            )
            | Some(terra_voxel::MaterialId::SaltStone | terra_voxel::MaterialId::PinkSaltStone)
            | None => Self::Stone,
            Some(material) => terra_core::construction::base_material(material as u16)
                .map_or(Self::Stone, Self::material),
        }
    }

    fn clips(self) -> &'static [usize] {
        match self {
            Self::Grass => &[14, 15, 34, 35, 36],
            Self::Sand => &[16, 17, 37, 38, 39, 40],
            Self::Stone => &[18, 19, 41, 42, 43, 44],
            Self::Dirt => &[20, 21, 45, 46, 47, 48],
            Self::Snow => &[22, 23, 49, 50, 51, 52],
            Self::Wood => &[24, 25, 53, 54, 55],
            Self::Gravel => &[56, 57, 58, 59, 60, 61, 62, 63, 64, 65],
            Self::Glass => &[66, 67, 68, 69, 70],
            Self::Leaves => &[71, 72],
        }
    }
}

pub(super) struct StepSound {
    pub sound: usize,
    pub pitch: f32,
    pub level: f32,
}

pub(super) struct ContactVariation {
    seed: u32,
    previous: [usize; 9],
}
impl Default for ContactVariation {
    fn default() -> Self {
        Self {
            seed: 0x64a2_19bf,
            previous: [usize::MAX; 9],
        }
    }
}
impl ContactVariation {
    fn random(&mut self) -> u32 {
        self.seed ^= self.seed << 13;
        self.seed ^= self.seed >> 17;
        self.seed ^= self.seed << 5;
        self.seed
    }

    pub(super) fn next(&mut self, material: u16, alternate: bool) -> StepSound {
        let surface = Surface::material(material);
        let family = surface.clips();
        // Skip the last take, including when returning to this surface after another block.
        let previous = self.previous[surface as usize];
        let mut variant = self.random() as usize % (family.len() - 1);
        if variant >= previous {
            variant += 1;
        }
        self.previous[surface as usize] = variant;
        let pitch = 0.975
            + (self.random() & 255) as f32 / 255.0 * 0.05
            + if alternate { 0.006 } else { -0.006 };
        let level = 0.94 + (self.random() & 255) as f32 / 255.0 * 0.12;
        let surface_level = match surface {
            Surface::Stone | Surface::Glass => 0.86,
            Surface::Wood => 0.92,
            _ => 1.0,
        };
        StepSound {
            sound: family[variant],
            pitch,
            level: level * surface_level,
        }
    }
}
pub type Bank = Arc<Vec<Vec<f32>>>;
pub const THUNDER_CLIP: usize = PATHS.len();

fn recording(root: &std::path::Path, name: &str) -> Result<Vec<f32>, String> {
    let path = root.join(name);
    let data = std::fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    if data.len() > 4_000_000 {
        return Err(format!("Oversized recording: {name}"));
    }
    let audio = AudioSource { bytes: data.into() };
    let decoder = <AudioSource as Decodable>::Decoder::try_from(Cursor::new(audio))
        .map_err(|e| format!("Invalid recording {name}: {e}"))?;
    if decoder.channels().get() != 1 || decoder.sample_rate().get() != 24_000 {
        return Err(format!("Recording must be mono 24000 Hz: {name}"));
    }
    let clip: Vec<f32> = decoder.take(24_000 * 60 + 1).collect();
    if clip.len() < 240
        || clip.len() > 24_000 * 60
        || clip.iter().any(|s| !s.is_finite() || s.abs() > 1.0)
    {
        return Err(format!("Invalid recording samples: {name}"));
    }
    Ok(clip)
}

pub fn load() -> Result<Bank, String> {
    let root = terra_core::asset_directory().join("audio");
    let mut clips = Vec::new();
    for name in PATHS {
        clips.push(recording(&root, name)?);
    }
    clips.push(super::thunder::synthesize());
    for (start, duration, cutoff, gain) in [
        (0.03, 0.62, 1800.0, 0.85),
        (0.31, 0.50, 900.0, 0.65),
        (0.60, 0.66, 1500.0, 0.90),
        (0.05, 0.72, 2200.0, 1.00),
    ] {
        clips.push(aquatic_foley(&clips[9], start, duration, cutoff, gain));
    }
    for (variant, source) in [31, 75, 76, 77].into_iter().enumerate() {
        clips.push(super::spiders::swim(&clips[source], &clips[9], variant));
    }
    clips.push(super::spiders::step(&clips[18], &clips[71]));
    clips.push(super::spiders::idle(&clips[72]));
    clips.push(super::spiders::hiss());
    clips.push(super::chests::foley(
        &clips[DOOR_OPEN_CLIP],
        &clips[53],
        true,
    ));
    clips.push(super::chests::foley(
        &clips[DOOR_CLOSE_CLIP],
        &clips[54],
        false,
    ));
    clips.push(super::stalker::breath(&clips[STALKER_AIRWAY_CLIP], false));
    clips.push(super::stalker::scratch(&clips[53], &clips[72]));
    clips.push(super::stalker::scream());
    clips.push(super::stalker::breath(&clips[STALKER_AIRWAY_CLIP], true));
    let leaves = recording(&root, "environment/leaves.wav")?;
    super::ambience::append(&mut clips, &leaves);
    super::worm::append(&mut clips);
    super::worm::export(&clips)?;
    Ok(Arc::new(clips))
}

// Envelope and filter existing CC0 water Foley; these are movements, not invented animal voices.
fn aquatic_foley(source: &[f32], start: f32, duration: f32, cutoff: f32, gain: f32) -> Vec<f32> {
    let begin = (start * 24_000.0) as usize;
    let count = (duration * 24_000.0) as usize;
    let alpha = 1.0 - (-std::f32::consts::TAU * cutoff / 24_000.0).exp();
    let bass_alpha = 1.0 - (-std::f32::consts::TAU * 90.0 / 24_000.0).exp();
    let mut low = 0.0;
    let mut bass = 0.0;
    (0..count)
        .map(|index| {
            let sample = source.get(begin + index).copied().unwrap_or(0.0);
            low += (sample - low) * alpha;
            bass += (low - bass) * bass_alpha;
            let time = index as f32 / 24_000.0;
            let envelope = (time / 0.06).min(1.0) * ((duration - time) / 0.14).clamp(0.0, 1.0);
            ((low - bass) * gain * envelope).clamp(-0.65, 0.65)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn every_block_routes_to_its_physical_surface_family() {
        use Surface::*;
        let mut expected = vec![
            Stone, Grass, Dirt, Stone, Sand, Snow, Wood, Leaves, Leaves, Leaves, Wood, Stone,
            Stone, Stone, Stone, Glass, Wood, Wood, Wood, Dirt, Dirt, Dirt, Dirt, Dirt, Dirt, Dirt,
            Dirt, Stone, Wood, Wood, Stone, Stone, Stone, Stone, Stone, Stone, Wood, Stone, Stone,
            Stone, Wood, Stone, Gravel, Stone, Stone, Stone, Stone, Stone, Dirt, Glass, Leaves,
            Glass, Stone, Glass, Leaves, Wood, Leaves, Stone, Stone, Stone, Stone, Stone, Stone,
            Stone, Stone, Wood, Wood, Wood, Wood, Wood, Wood,
        ];
        expected.extend([Wood; 3]);
        for _ in 0..3 {
            expected.extend([Wood, Wood, Wood, Stone, Dirt, Sand, Glass, Glass]);
        }
        expected.push(Snow);
        expected.push(Stone); // Crusher's stone hoppers and metal drive.
        expected.push(Wood); // Hand crank grip and timber mounting plate.
        expected.push(Dirt); // Cultivated soil retains earth contact sounds.
        expected.extend([Dirt; 4]); // Packed paths and muffled wool, including its shaped forms.
        expected.extend([Dirt; 25]); // All carpet and dyed wool use the same soft textile contacts.
        expected.push(Stone); // The flower pot is hard fired ceramic, not loose raw clay.
        expected.extend([Leaves; 2]);
        expected.extend([Wood; 7]);
        expected.extend([Wood, Stone, Stone, Wood, Stone, Stone, Wood, Dirt]);
        expected.extend([Wood, Stone, Stone]); // Large gears retain each physical material.
        expected.extend([Wood; 4]); // Port and gearbox housings are timber with narrow metal straps.
        expected.push(Wood); // The mixer hopper and frame use the same timber contacts.
        expected.extend([Wood; 2]); // Filter cassette housing and four-way merger frame.
        expected.extend([Wood; 2]); // Filtered tunnel and distributor retain their timber frames.
        expected.extend([Stone, Wood]); // Iron brewing bowl and the campfire's charred logs.
        expected.extend([Wood, Stone, Stone, Stone, Stone, Stone, Wood]); // Pipe bodies, metal fittings, pump and timber basin.
        expected.extend([Wood, Stone]); // Timber Hold Block platform and hard metal Fluid Infuser casing.
        expected.push(Glass); // Transparent glass pipe retains glass contact sounds.
        expected.extend([Stone; 16]); // Founding keep, masonry, battlements, arches, slit/window and warm stone styles.
        expected.push(Snow); // Snow-covered wall has a real snow cap underfoot.
        expected.extend([Stone; 2]); // Blackstone and royal heraldry retain their masonry contact.
        expected.push(Wood); // Timber reinforcement is the same contact family as strapped wooden machinery.
        expected.extend([Stone; 2]); // Iron lantern feet/frame and chain links use the hard metal family.
        expected.push(Leaves); // The apple canopy keeps the same leaf contact sounds.
        expected.extend([Wood; 10]); // Railway modules retain timber sleeper contact sounds.
        expected.extend([Gravel; 3]); // Railway ballast and its shaped embankment blocks.
        expected.push(Stone); // The separate switch stand has an iron base.
        expected.extend([Stone; 5]); // Chassis, wheelsets, frame, coupler and cab floor are metal.
        expected.extend([Dirt, Wood, Wood]); // Upholstered seat, timber cargo and workshop tabletop.
        expected.extend([Stone; terra_core::locomotive_parts::COUNT]);
        expected[267] = Wood;
        expected[268] = Wood;
        expected.push(Stone); // Rotating steel turntable deck.
        expected.push(Stone); // Steel rail builder scoop and frame.
        // Station contacts follow their authored timber counters or masonry/metal bases.
        expected.extend([Wood, Stone, Wood, Stone, Stone, Stone, Wood, Stone, Wood]);
        // Spirit turf, earth, stone, petals, three canopies, timber and ornaments.
        expected.extend([
            Grass, Dirt, Stone, Leaves, Leaves, Leaves, Leaves, Wood, Wood, Stone, Leaves, Leaves,
            Stone, Leaves, Leaves, Wood, Leaves, Stone,
        ]);
        expected.extend([Dirt; 13]); // Colored raw clay.
        expected.extend([Stone; 22]); // Grave masonry through the carved wall niche.
        expected.extend([Dirt; 3]); // Banners and curtain.
        expected.extend([Stone; 8]); // Graves, sarcophagi and the lantern post.
        expected.extend([
            Wood, Dirt, Stone, Stone, Stone, Stone, Stone, Wood, Dirt, Stone, Stone,
        ]);
        expected.push(Grass); // Dry grass, followed by six soft vegetation props.
        expected.extend([Leaves; 6]);
        expected.extend([Stone; 7]); // Plates, gates and decorative mechanisms.
        expected.extend([Glass, Glass, Stone, Stone, Stone]); // Crystals and rune/mist stone.
        // Blood terrain, wood, foliage, fungal props, hard bone/ruins and crystals (400–421).
        expected.extend([
            Grass, Dirt, Stone, Stone, Wood, Wood, Leaves, Dirt, Grass, Glass, Dirt, Dirt, Leaves,
            Leaves, Leaves, Leaves, Wood, Stone, Leaves, Dirt, Stone, Glass,
        ]);
        expected.push(Wood); // Solid cactus stem (422).
        expected.extend([Leaves; 7]); // Soft authored regional flora (423–429).
        expected.push(Stone); // Solid fog stone (430).
        expected.extend([Stone; 6]); // Asphalt surfaces and the stone road curb (431–436).
        expected.extend([Dirt, Stone, Dirt, Stone]); // Wet tar, dry road, liquid Galle and its solid block (437–440).
        expected.extend([Stone; 7]); // Sidewalk block and its six edge profiles (441–447).
        expected.push(Dirt); // Straw muffles footsteps (448).
        assert_eq!(
            expected.len(),
            usize::from(terra_voxel::MaterialId::LAST) + 1
        );
        for (material, expected) in expected.into_iter().enumerate() {
            assert_eq!(
                Surface::material(material as u16),
                expected,
                "material {material}"
            );
        }
        assert_eq!(Surface::material(u16::MAX), Stone);
    }

    #[test]
    fn castle_contacts_use_stone_iron_timber_and_snow_foley_without_visual_finish_leaking_into_sound()
     {
        use terra_core::castle;
        let mut variation = ContactVariation::default();
        for material in castle::BLOCKS {
            let expected = match material {
                castle::TIMBER_WALL => Surface::Wood,
                castle::SNOW_WALL => Surface::Snow,
                _ => Surface::Stone,
            };
            assert_eq!(
                Surface::material(material),
                expected,
                "castle material {material}"
            );
            for _ in 0..12 {
                assert!(
                    expected
                        .clips()
                        .contains(&variation.next(material, false).sound)
                );
            }
        }
        // The existing hard-contact family includes iron drives and fittings;
        // reinforcement must not sound like the banner cloth or climbing ivy.
        assert_eq!(
            Surface::material(castle::IRON_WALL),
            Surface::material(terra_voxel::MaterialId::IronGear as u16)
        );
        assert_eq!(Surface::material(castle::ROYAL_WALL), Surface::Stone);
        assert_eq!(Surface::material(castle::OVERGROWN_WALL), Surface::Stone);
    }

    #[test]
    fn filtered_logistics_keep_timber_contacts_while_iron_drives_remain_hard() {
        use terra_voxel::MaterialId;
        let mut variation = ContactVariation::default();
        for (material, base) in [
            (MaterialId::FilteredTunnel, MaterialId::ItemHopper),
            (MaterialId::BeltDistributor, MaterialId::BeltMerger),
            (MaterialId::HoldBlock, MaterialId::ItemHopper),
        ] {
            assert_eq!(Surface::material(material as u16), Surface::Wood);
            assert_eq!(
                Surface::material(material as u16),
                Surface::material(base as u16)
            );
            for alternate in [false, true] {
                assert!(
                    Surface::Wood
                        .clips()
                        .contains(&variation.next(material as u16, alternate).sound)
                );
            }
        }
        for material in [
            MaterialId::IronGear,
            MaterialId::IronShaft,
            MaterialId::FluidInfuser,
        ] {
            assert_eq!(Surface::material(material as u16), Surface::Stone);
        }
    }

    #[test]
    fn all_textile_colors_keep_soft_wool_contacts_and_fired_pots_keep_hard_contacts() {
        for base in [
            terra_core::textiles::WHITE_WOOL,
            terra_core::textiles::WHITE_CARPET,
        ] {
            for color in terra_core::beds::DyeColor::ALL {
                let material = terra_core::textiles::dyed(base, color).unwrap();
                assert_eq!(Surface::material(material), Surface::material(103));
                assert_ne!(Surface::material(material), Surface::Stone);
                let mut variation = ContactVariation::default();
                for alternate in [false, true] {
                    assert!(
                        Surface::Dirt
                            .clips()
                            .contains(&variation.next(material, alternate).sound)
                    );
                }
            }
        }
        assert_eq!(
            Surface::material(terra_core::flower_pot::MATERIAL),
            Surface::Stone
        );
    }

    #[test]
    fn contact_variation_uses_every_recording_without_repeating_the_previous_take() {
        let mut ids = std::collections::BTreeSet::new();
        for material in [1, 4, 3, 2, 5, 16, 42, 15, 7] {
            let mut variation = ContactVariation::default();
            let mut previous = usize::MAX;
            let mut heard = std::collections::BTreeSet::new();
            for i in 0..256 {
                let take = variation.next(material, i % 2 == 0);
                assert_ne!(take.sound, previous);
                assert!((0.96..=1.04).contains(&take.pitch));
                assert!((0.80..=1.06).contains(&take.level));
                heard.insert(take.sound);
                previous = take.sound;
            }
            assert_eq!(heard.len(), Surface::material(material).clips().len());
            for id in heard {
                assert!(ids.insert(id));
            }
        }
        assert_eq!(ids.len(), 51);
    }
    #[test]
    fn every_shipped_recording_decodes_and_contains_real_sample_energy() {
        let clips = load().unwrap();
        assert_eq!(
            clips.len(),
            PATHS.len() + 18 + super::super::ambience::DERIVED + super::super::worm::COUNT
        );
        for (name, clip) in PATHS.iter().zip(clips.iter()) {
            let rms = (clip.iter().map(|s| s * s).sum::<f32>() / clip.len() as f32).sqrt();
            assert!(rms > 0.001 && rms < 0.5, "{name}: {rms}");
            if name.starts_with("footsteps/detail/") {
                assert!((0.02..0.15).contains(&rms), "{name}: step RMS {rms}");
                assert!(clip.len() <= 24_000, "{name}: contact exceeds one second");
                assert!(
                    clip.iter().all(|s| s.abs() <= 0.721),
                    "{name}: peak headroom"
                );
                assert!(
                    clip[..960.min(clip.len())].iter().any(|s| s.abs() > 0.02),
                    "{name}: delayed contact"
                );
            }
        }
        for index in [
            SQUID_JET_CLIP,
            JELLY_PULSE_CLIP,
            SHARK_STROKE_CLIP,
            INK_JET_CLIP,
        ] {
            let clip = &clips[index];
            assert!((12_000..=18_000).contains(&clip.len()));
            assert!(
                clip.iter()
                    .all(|sample| sample.is_finite() && sample.abs() <= 0.65)
            );
            assert!(clip.iter().any(|sample| sample.abs() > 0.001));
            assert!(clip[0].abs() < 0.001 && clip.last().unwrap().abs() < 0.001);
        }
        for index in [CHEST_OPEN_CLIP, CHEST_CLOSE_CLIP] {
            let clip = &clips[index];
            assert!((11_000..=12_000).contains(&clip.len()));
            assert!(
                clip.iter()
                    .all(|sample| sample.is_finite() && sample.abs() <= 0.60)
            );
            assert!(clip.iter().any(|sample| sample.abs() > 0.001));
            assert_eq!(clip[0], 0.0);
            assert_eq!(*clip.last().unwrap(), 0.0);
        }
        assert_ne!(clips[CHEST_OPEN_CLIP], clips[CHEST_CLOSE_CLIP]);
    }
}
