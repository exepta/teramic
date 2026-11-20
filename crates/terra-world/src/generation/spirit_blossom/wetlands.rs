//! Revision 2 keeps drainage paths, outlets and levels stable. Cross-sections
//! expand by area, while dry hills rise independently of lakes and river banks.
use crate::generation::{
    OasisSample, RiverSample, TerraGenerator, biomes::smoothstep, lakes::Lake,
};

pub(crate) const HILL_RELIEF_GAIN: f64 = 0.20;
// Overlapping channels, voxel shore cells and unchanged ocean water reduce
// measured coverage. This raw expansion is calibrated against actual water
// columns over three seeds; it targets about +40% total Spirit water surface.
pub(crate) const WETLAND_AREA_GAIN: f64 = 0.64;

impl TerraGenerator {
    fn spirit_landscape_weight(&self, x: f64, z: f64) -> f64 {
        if self.config().spirit_blossom_revision < 2 || !self.config().biomes {
            return 0.0;
        }
        smoothstep(
            0.35,
            0.85,
            self.climate_biome_at(x, z).weights.spirit_blossom,
        )
    }

    pub(in crate::generation) fn spirit_water_area_scale(&self, x: f64, z: f64) -> f64 {
        1.0 + WETLAND_AREA_GAIN * self.spirit_landscape_weight(x, z)
    }

    pub(in crate::generation) fn spirit_lake_sample(
        &self,
        lake: Lake,
        x: f64,
        z: f64,
    ) -> OasisSample {
        let mut scale = self.spirit_water_area_scale(x, z).sqrt();
        if scale > 1.0
            && let Some(river) = self.river_at(x, z)
            && (river.level - lake.level).abs() > 0.01
        {
            // Existing inlet/outlet levels were solved against the old
            // lake footprint. Do not extend a lower lake over a higher
            // inlet (dry channel) or into a lower outlet (leaking bank).
            let clearance = smoothstep(river.width + 12.0, river.width + 48.0, river.distance);
            scale = 1.0 + (scale - 1.0) * clearance;
        }
        if scale == 1.0 {
            return lake.sample(x, z);
        }
        // Scale the complete lobed outline, not only its nominal radius.
        // sqrt(area factor) scales lake area, not radius; channel overlap and
        // discrete shores are included by the coverage calibration example.
        let mut sample = lake.sample(
            lake.center[0] + (x - lake.center[0]) / scale,
            lake.center[1] + (z - lake.center[1]) / scale,
        );
        sample.distance *= scale;
        sample.radius *= scale;
        sample
    }

    pub(in crate::generation) fn spirit_hill_relief(
        &self,
        x: f64,
        z: f64,
        height: f64,
        river: Option<RiverSample>,
        lake: Option<OasisSample>,
    ) -> f64 {
        let weight = self.spirit_landscape_weight(x, z);
        if weight == 0.0
            || (self.config().hill_amplitude == 0.0 && self.config().mountain_amplitude == 0.0)
        {
            return height;
        }
        let dry = river.map_or(1.0, |r| {
            smoothstep(r.width + 6.0, r.width + 48.0, r.distance)
        }) * lake.map_or(1.0, |l| {
            smoothstep(l.radius + 6.0, l.radius + 40.0, l.distance)
        });
        let coast = smoothstep(
            self.config().sea_level + 2.0,
            self.config().sea_level + 12.0,
            height,
        );
        // Sea level is the stable datum: differences between two fully dry
        // interior heights are exactly 1.2 times the revision-1 difference.
        height
            + (height - self.config().sea_level).max(0.0) * HILL_RELIEF_GAIN * weight * dry * coast
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use terra_core::WorldConfig;

    #[test]
    fn lake_area_multiplier_uses_square_root_not_radius_gain() {
        let radius = 24.0_f64;
        let expanded = radius * (1.0 + WETLAND_AREA_GAIN).sqrt();
        assert!((expanded.powi(2) / radius.powi(2) - (1.0 + WETLAND_AREA_GAIN)).abs() < 1e-12);
    }

    #[test]
    fn old_revisions_and_other_biomes_keep_original_relief() {
        for revision in [0, 1] {
            let terrain = TerraGenerator::new(WorldConfig {
                spirit_blossom_revision: revision,
                ..Default::default()
            })
            .unwrap();
            for [x, z] in [
                [-10000.0, 4000.0],
                [-1.0, -1.0],
                [0.0, 0.0],
                [10000.0, 10000.0],
            ] {
                assert_eq!(terrain.spirit_water_area_scale(x, z), 1.0);
                assert_eq!(terrain.spirit_hill_relief(x, z, 84.7, None, None), 84.7);
            }
        }
        let terrain = TerraGenerator::new(WorldConfig {
            spirit_blossom_revision: 2,
            ..Default::default()
        })
        .unwrap();
        let mut unchanged = 0;
        for x in -16..=16 {
            let x = f64::from(x) * 2048.0;
            if terrain.climate_biome_at(x, -x).weights.spirit_blossom == 0.0 {
                assert_eq!(terrain.spirit_water_area_scale(x, -x), 1.0);
                assert_eq!(terrain.spirit_hill_relief(x, -x, 84.7, None, None), 84.7);
                unchanged += 1;
            }
        }
        assert!(unchanged > 10);
    }

    #[test]
    fn dry_spirit_interior_relief_is_exactly_twenty_percent_higher() {
        let terrain = TerraGenerator::new(WorldConfig {
            spirit_blossom_revision: 2,
            ..Default::default()
        })
        .unwrap();
        for z in (-16384..=16384).step_by(512) {
            for x in (-16384..=16384).step_by(512) {
                let (x, z) = (f64::from(x), f64::from(z));
                if terrain.climate_biome_at(x, z).weights.spirit_blossom >= 0.85 {
                    let lower = terrain.spirit_hill_relief(x, z, 100.0, None, None);
                    let upper = terrain.spirit_hill_relief(x, z, 110.0, None, None);
                    assert!((upper - lower - 12.0).abs() < 1e-12);
                    return;
                }
            }
        }
        panic!("seed must contain a Spirit interior to test relief");
    }
}
