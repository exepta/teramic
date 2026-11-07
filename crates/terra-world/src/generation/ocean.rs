//! Continuous marine habitats, sampled once per cached surface column.
use super::{
    TerraGenerator,
    biomes::{BiomeSample, smoothstep},
};
use noise::NoiseFn;
use terra_voxel::MaterialId;

mod modern;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OceanArea {
    CoralReef,
    SeagrassForest,
    ColdOcean,
    Swamp,
}

impl OceanArea {
    pub fn label(self) -> &'static str {
        match self {
            Self::CoralReef => "Coral Reef",
            Self::SeagrassForest => "Seagrass Forest",
            Self::ColdOcean => "Cold Ocean",
            Self::Swamp => "Swamp",
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct OceanHabitat {
    /// Reef, seagrass forest and cold ocean blend continuously at their edges.
    pub weights: [f32; 3],
    pub colony: f32,
    pub cover: f32,
    pub rock: f32,
    /// Revision-four organic coastal/marine habitat, without changing legacy weight slots.
    pub swamp: f32,
    pub coral_density: f32,
    pub seagrass_density: f32,
    pub max_vegetation_depth: f32,
    /// Connected clay deposit thickness; zero retains the underlying substrate.
    pub clay_depth: f32,
    substrate: Option<MaterialId>,
    /// Solid sea ice extends downward from sea level + 1; zero means open water.
    pub ice_depth: u8,
}

impl OceanHabitat {
    pub fn area(self) -> Option<OceanArea> {
        if self.weights.iter().copied().sum::<f32>() < 0.1 {
            return None;
        }
        if self.swamp > self.weights.iter().copied().sum::<f32>() * 0.5 {
            return Some(OceanArea::Swamp);
        }
        let index = (0..3).max_by(|&a, &b| self.weights[a].total_cmp(&self.weights[b]))?;
        Some(
            [
                OceanArea::CoralReef,
                OceanArea::SeagrassForest,
                OceanArea::ColdOcean,
            ][index],
        )
    }

    pub(super) fn floor_material(self) -> Option<MaterialId> {
        if let Some(substrate) = self.substrate {
            Some(substrate)
        } else if self.weights[0] > 0.45 && self.rock > 0.38 {
            Some(if self.rock > 0.54 && self.colony > 0.30 {
                if self.colony > 0.90 {
                    MaterialId::CoralPurple
                } else if self.colony > 0.83 {
                    MaterialId::CoralBlue
                } else if self.colony > 0.76 {
                    MaterialId::CoralRed
                } else {
                    MaterialId::CoralYellow
                }
            } else {
                MaterialId::Stone
            })
        } else if self.weights[2] > 0.55 && self.rock > 0.4 {
            Some(MaterialId::Gravel)
        } else if self.weights[1] > 0.55 && self.colony < 0.24 {
            Some(MaterialId::Mud)
        } else {
            None
        }
    }
}

impl TerraGenerator {
    pub fn ocean_habitat(&self, x: f64, z: f64) -> OceanHabitat {
        self.ocean_with_biome(x, z, self.climate_biome_at(x, z))
    }

    pub(super) fn ocean_with_biome(&self, x: f64, z: f64, biome: BiomeSample) -> OceanHabitat {
        let sea = smoothstep(0.28, 0.72, biome.weights.ocean);
        if !self.config().water || sea == 0.0 {
            return OceanHabitat::default();
        }
        if self.config().landscape_revision >= 4 {
            return self.modern_ocean_habitat(x, z, biome, sea);
        }
        let district = self.hills.get([x / 240.0 + 84.3, z / 240.0 - 61.7]);
        let cold = 1.0 - smoothstep(0.23, 0.39, biome.temperature);
        let reef = (1.0 - cold)
            * smoothstep(0.42, 0.65, biome.temperature)
            * smoothstep(-0.28, 0.18, district);
        let meadow = (1.0 - cold - reef).max(0.0);
        let colony =
            (0.5 + self.valleys.get([x / 21.0 + 37.9, z / 21.0 - 104.1]) * 0.75).clamp(0.0, 1.0);
        let channel = self.valleys.get([x / 73.0 - 51.4, z / 73.0 + 29.3]).abs();
        let cover = smoothstep(0.035, 0.13, channel) * smoothstep(0.20, 0.65, colony);
        let rock = smoothstep(
            -0.12,
            0.38,
            self.hills.get([x / 33.0 - 14.7, z / 33.0 + 71.3]),
        ) * smoothstep(0.025, 0.10, channel);
        // Cold oceans stay open. Ice remains an explicit player-placeable block.
        let ice_depth = 0;
        OceanHabitat {
            weights: [reef, meadow, cold].map(|w| (w * sea) as f32),
            colony: colony as f32,
            cover: cover as f32,
            rock: rock as f32,
            ice_depth,
            ..Default::default()
        }
    }

    pub(super) fn ocean_relief(&self, x: f64, z: f64, biome: BiomeSample, height: f64) -> f64 {
        if self.config().ocean_revision == 0 || height >= self.config().sea_level - 3.0 {
            return height;
        }
        let habitat = self.ocean_with_biome(x, z, biome);
        let depth = self.config().sea_level - height;
        let shelf =
            f64::from(habitat.rock) * f64::from(habitat.weights[0]) * smoothstep(3.0, 10.0, depth);
        // Existing connected seabed becomes stepped reef ledges; no floating cubes.
        height + shelf * (depth - 2.5).clamp(0.0, 10.0)
    }
}

#[cfg(test)]
mod modern_tests;
#[cfg(test)]
mod tests;
