use crate::generation::biomes::BiomeSample;
use terra_voxel::MaterialId;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum TreeSpecies {
    Green,
    Autumn,
    Birch,
    Spruce,
    Palm,
    Mushroom(u8),
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
            Self::Spruce => MaterialId::SpruceLog,
            Self::Mushroom(_) => MaterialId::MushroomStem,
            _ => MaterialId::Wood,
        }
    }
    pub fn leaves(self) -> MaterialId {
        match self {
            Self::Green | Self::Spruce | Self::Palm => MaterialId::Leaves,
            Self::Autumn => MaterialId::AutumnLeaves,
            Self::Birch => MaterialId::SilverLeaves,
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
            Self::Palm => (8.0, 14.0),
            Self::Mushroom(_) => (6.0, 14.0),
        }
    }
    pub fn in_biome(seed: u64, biome: BiomeSample) -> Self {
        if f64::from(super::random(seed, 221)) < biome.weights.mushroom_park {
            return Self::Mushroom((super::random(seed, 222) * 6.0) as u8);
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
