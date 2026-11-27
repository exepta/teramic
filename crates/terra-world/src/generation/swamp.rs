//! Still backwaters interleave with low, winding peat ridges at one water table.
use super::{
    TerraGenerator,
    biomes::{BiomeSample, smoothstep},
};
use noise::NoiseFn;
use terra_voxel::MaterialId as M;

impl TerraGenerator {
    pub(super) fn with_swamp(&self, mut sample: BiomeSample, x: f64, z: f64) -> BiomeSample {
        let scale = self.config().biome_scale.max(400.) * 0.72;
        let wetland = self.valleys.get([x / scale - 61.83, z / scale + 47.32]);
        let humid = smoothstep(0.52, 0.78, sample.humidity);
        let mild = smoothstep(0.27, 0.46, sample.temperature);
        let share = smoothstep(0.04, 0.40, wetland) * humid * mild;
        let w = &mut sample.weights;
        for source in [&mut w.forest, &mut w.plains] {
            let wet = *source * share;
            *source -= wet;
            w.swamp += wet;
        }
        sample
    }

    pub(super) fn swamp_height(&self, x: f64, z: f64, raw: f64, weight: f64) -> f64 {
        if weight <= 0.02 {
            return raw;
        }
        let bend = self.hills.get([x / 89. + 43.9, z / 91. - 29.1]) * 14.;
        let hummocks = self.valleys.get([(x + bend) / 47. + 34.2, z / 41. - 57.6]);
        let ridges = self.hills.get([x / 111. + 61.6, (z + bend) / 87. - 49.2]);
        let tussocks = self.hills.get([x / 15. - 17.1, z / 18. + 29.5]);
        let bed = self.config().sea_level + 0.9 + hummocks * 6.2 + ridges * 3.5 + tussocks * 0.85;
        raw + (bed - raw) * smoothstep(0.02, 0.92, weight)
    }

    pub(super) fn swamp_ground(&self, x: f64, z: f64, submerged: bool, slope: f64) -> M {
        let patch = self.hills.get([x / 14. + 59.8, z / 18. - 42.9]);
        if slope > 1.4 {
            if patch > 0.1 {
                M::MossySwampStone
            } else {
                M::SwampStone
            }
        } else if submerged {
            if patch > 0.38 {
                M::SwampClay
            } else if patch < -0.30 {
                M::StonySwampMud
            } else {
                M::SwampMud
            }
        } else if patch < -0.38 {
            M::Peat
        } else if patch > 0.25 {
            M::MossyPeat
        } else {
            M::SwampTurf
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{SectionGenerator, generation::natural_features::SwampLandmark};
    use terra_core::{BiomeDefinitions, BiomeKind, WorldConfig};

    #[test]
    fn marshes_have_open_backwaters_ruins_and_inhabited_docks() {
        let cfg = WorldConfig {
            biome_definitions: Some(BiomeDefinitions::default()),
            preferred_spawn_biome: Some(BiomeKind::Swamp),
            ..Default::default()
        };
        let terrain = TerraGenerator::new(cfg.clone()).unwrap();
        let spawn = crate::spawn::find_land_spawn(&terrain, &Default::default(), [0, 0])
            .expect("new game has a safe wetland spawn");
        assert!(
            terrain
                .biome_at(spawn.feet()[0], spawn.feet()[2])
                .weights
                .swamp
                > 0.8
        );
        let generator = SectionGenerator::new(terrain.clone());
        let old = TerraGenerator::new(WorldConfig {
            ecology_revision: 17,
            ..cfg
        })
        .unwrap();
        for (tile, kind, count) in [
            ([164, -229], SwampLandmark::WitchesLodge, 2),
            ([0, -358], SwampLandmark::SunkenRuins, 0),
            ([6, -354], SwampLandmark::AbandonedHut, 1),
        ] {
            let site = terrain
                .swamp_site(tile)
                .expect("natural shoreline landmark");
            assert_eq!(site.kind, kind);
            let [x, _, z] = site.floor;
            assert!(terrain.biome_at(x as f64, z as f64).weights.swamp > 0.7);
            assert_eq!(old.biome_at(x as f64, z as f64).weights.swamp, 0.);
            let residents = terrain.swamp_residents(tile);
            assert_eq!(residents.len(), count);
            for ([rx, ry, rz], _) in residents {
                assert_eq!(
                    generator.block_at([rx, ry - 1, rz]).unwrap(),
                    M::WillowPlanks as u16
                );
                assert_eq!(generator.block_at([rx, ry, rz]).unwrap(), 0);
                assert_eq!(generator.block_at([rx, ry + 1, rz]).unwrap(), 0);
            }
            let water = (x - 10..=x + 10).any(|x| {
                (z - 10..=z + 10).any(|z| {
                    terrain
                        .water_column(x as f64 + 0.5, z as f64 + 0.5)
                        .is_some()
                })
            });
            assert!(water);
        }
    }
}
