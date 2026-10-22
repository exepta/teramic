use bevy::audio::{AudioSource, Decodable, Source};
use std::{io::Cursor, sync::Arc};

pub const RAIN_CLIP: usize = 73;
pub const RAIN_DETAIL_CLIP: usize = 74;
pub const SWIM_CLIPS: [usize; 4] = [31, 75, 76, 77];
pub const WET_CONTACT_CLIPS: [usize; 2] = [78, 79];
pub const SQUID_JET_CLIP: usize = PATHS.len() + 1;
pub const JELLY_PULSE_CLIP: usize = PATHS.len() + 2;
pub const SHARK_STROKE_CLIP: usize = PATHS.len() + 3;
pub const INK_JET_CLIP: usize = PATHS.len() + 4;
pub const DOOR_OPEN_CLIP: usize = 80;
pub const DOOR_CLOSE_CLIP: usize = 81;
pub const COUGH_CLIPS: [usize; 3] = [82, 84, 85];
pub const FURNACE_CLIP: usize = 83;
pub const PATHS: [&str; 86] = [
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
        use terra_voxel::MaterialId::*;
        match terra_voxel::MaterialId::from_raw(raw) {
            Some(Grass) => Self::Grass,
            Some(Sand) => Self::Sand,
            Some(
                Dirt | Clay | Mud | MushroomStem | PurpleMushroomCap | RedMushroomCap
                | YellowMushroomCap | GreenMushroomCap | BlueMushroomCap | WhiteMushroomCap,
            ) => Self::Dirt,
            Some(Snow | SnowLayer) => Self::Snow,
            Some(
                Wood | WeatheredWood | OakPlanks | BirchPlanks | SprucePlanks | Workbench | Chest
                | SpruceLog | Torch | OakDoor | BirchDoor | SpruceDoor | OakWindow | BirchWindow
                | SpruceWindow | OakBed | BirchBed | SpruceBed,
            ) => Self::Wood,
            Some(Gravel) => Self::Gravel,
            Some(Glass | Ice | SaltCrystal | IceCrystal) => Self::Glass,
            Some(Leaves | AutumnLeaves | SilverLeaves | Seagrass | CaveVine | CaveMushroom) => {
                Self::Leaves
            }
            Some(CaveRoot) => Self::Wood,
            Some(DripstoneTip) => Self::Stone,
            Some(
                Air | Stone | Sandstone | SmoothStone | MossyStone | SolidWater | FluidWater
                | SaltOre | IronOre | CoalOre | GoldOre | DiamondOre | RubyOre | Dripstone
                | SaltFormation | DripstoneFormation | Bedrock | StructureStone | CoralRed
                | CoralBlue | CoralYellow | CoralPurple | Furnace | ChimneyStraight | ChimneyElbow
                | ChimneyHorizontal | ChimneyBendUp | ChimneyBendDown,
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

pub fn load() -> Result<Bank, String> {
    let root = terra_core::asset_directory().join("audio");
    let mut clips = Vec::new();
    for name in PATHS {
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
        clips.push(clip);
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
        assert_eq!(clips.len(), PATHS.len() + 5);
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
    }
}
