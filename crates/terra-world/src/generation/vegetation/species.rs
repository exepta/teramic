use crate::generation::biomes::BiomeSample;
use terra_voxel::MaterialId;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum TreeSpecies {
    Green,
    Autumn,
    Birch,
    Spruce,
    Palm,
    Cherry(bool),
    SpiritBlossom(u8),
    Bloodwood,
    Mushroom(u8),
    Fir,
    Apple,
}

impl TreeSpecies {
    #[cfg(test)]
    pub fn from_seed(seed: u64) -> Self {
        match (seed >> 32) % 10 {
            0..=4 => Self::Green,
            5..=7 => Self::Autumn,
            _ => Self::Birch,
        }
    }
    pub fn wood(self) -> MaterialId {
        match self {
            Self::Birch => MaterialId::WeatheredWood,
            Self::Spruce | Self::Fir => MaterialId::SpruceLog,
            Self::Cherry(_) => MaterialId::CherryLog,
            Self::SpiritBlossom(_) => MaterialId::SpiritBlossomLog,
            Self::Bloodwood => MaterialId::BloodwoodLog,
            Self::Mushroom(_) => MaterialId::MushroomStem,
            _ => MaterialId::Wood,
        }
    }
    pub fn leaves(self) -> MaterialId {
        match self {
            Self::Green | Self::Spruce | Self::Fir | Self::Palm => MaterialId::Leaves,
            Self::Autumn => MaterialId::AutumnLeaves,
            Self::Birch => MaterialId::SilverLeaves,
            Self::Apple => MaterialId::AppleLeaves,
            Self::Bloodwood => MaterialId::BloodLeaves,
            Self::SpiritBlossom(variant) => [
                MaterialId::PinkBlossomLeaves,
                MaterialId::WhiteBlossomLeaves,
                MaterialId::LavenderBlossomLeaves,
            ][usize::from(variant % 3)],
            Self::Cherry(false) => MaterialId::CherryLeaves,
            Self::Cherry(true) => MaterialId::WhiteCherryLeaves,
            Self::Mushroom(variant) => [
                MaterialId::PurpleMushroomCap,
                MaterialId::RedMushroomCap,
                MaterialId::YellowMushroomCap,
                MaterialId::GreenMushroomCap,
                MaterialId::BlueMushroomCap,
                MaterialId::WhiteMushroomCap,
            ][usize::from(variant % 6)],
        }
    }
    pub fn height_range(self) -> (f32, f32) {
        match self {
            Self::Green => (10.0, 20.0),
            Self::Autumn => (9.0, 18.0),
            Self::Birch => (14.0, 22.0),
            Self::Spruce => (21.0, 33.6),
            Self::Fir => (22.0, 31.0),
            Self::Palm => (8.0, 14.0),
            Self::Cherry(_) => (9.0, 17.0),
            Self::SpiritBlossom(_) => (13.0, 23.0),
            Self::Bloodwood => (14.0, 25.0),
            Self::Mushroom(_) => (6.0, 14.0),
            Self::Apple => (7.0, 10.0),
        }
    }
    pub fn in_biome(seed: u64, biome: BiomeSample, landscape_revision: u8) -> Self {
        if f64::from(super::random(seed, 910)) < (biome.weights.blood_forest * 1.7).min(1.0) {
            return Self::Bloodwood;
        }
        if f64::from(super::random(seed, 734)) < (biome.weights.spirit_blossom * 1.6).min(1.0) {
            return Self::SpiritBlossom((super::random(seed, 735) * 3.0) as u8);
        }
        if f64::from(super::random(seed, 271)) < (biome.weights.cherry_forest * 1.7).min(1.0) {
            return Self::Cherry(super::random(seed, 272) < 0.24);
        }
        if f64::from(super::random(seed, 221)) < biome.weights.mushroom_park {
            return Self::Mushroom((super::random(seed, 222) * 6.0) as u8);
        }
        if landscape_revision >= 4 {
            let fir =
                (biome.weights.highland * 0.34 + biome.weights.forest * 0.27).clamp(0.0, 0.45);
            if f64::from(super::random(seed, 274)) < fir {
                return Self::Fir;
            }
        }
        let spruce = (biome.weights.highland * 0.85
            + biome.weights.forest * (0.5 - biome.temperature).max(0.0) * 1.8)
            .clamp(0.0, 0.92);
        if f64::from(super::random(seed, 223)) < spruce {
            return Self::Spruce;
        }
        let green = 0.35 + biome.weights.forest * 0.30;
        let autumn = (0.15 + biome.temperature * 0.20) * (1.0 - biome.weights.highland * 0.4);
        let value = f64::from(super::random(seed, 220));
        if value < green {
            Self::Green
        } else if value < green + autumn {
            Self::Autumn
        } else {
            Self::Birch
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generation::biomes::BiomeWeights;

    #[test]
    fn firs_join_forests_and_highlands_only_in_new_landscapes() {
        for (forest, highland) in [(1.0, 0.0), (0.0, 1.0), (0.0, 0.0)] {
            let biome = BiomeSample {
                temperature: 0.25,
                humidity: 0.7,
                weights: BiomeWeights {
                    ocean: 0.0,
                    plains: 1.0 - forest - highland,
                    forest,
                    desert: 0.0,
                    highland,
                    mushroom_park: 0.0,
                    cliffted_desert: 0.0,
                    cherry_forest: 0.0,
                    spirit_blossom: 0.0,
                    blood_forest: 0.0,
                },
            };
            let mut firs = 0;
            let mut spruces = 0;
            for seed in 0..1024 {
                let seed = super::super::mix(seed);
                let legacy = TreeSpecies::in_biome(seed, biome, 3);
                assert_ne!(legacy, TreeSpecies::Fir);
                for revision in 0..3 {
                    assert_eq!(TreeSpecies::in_biome(seed, biome, revision), legacy);
                }
                let current = TreeSpecies::in_biome(seed, biome, 4);
                if current == TreeSpecies::Fir {
                    firs += 1;
                } else {
                    assert_eq!(current, legacy);
                }
                spruces += usize::from(current == TreeSpecies::Spruce);
            }
            if forest + highland > 0.0 {
                assert!(firs > 180 && firs < 450, "fir count {firs}");
                assert!(spruces > 100, "mixed conifers, spruce count {spruces}");
            } else {
                assert_eq!(firs, 0);
            }
        }
    }
}
