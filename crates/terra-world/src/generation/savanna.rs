//! Warm, seasonally dry grasslands interspersed with isolated table mountains.
use super::{
    TerraGenerator,
    biomes::{BiomeSample, smoothstep},
};
use noise::NoiseFn;
use terra_voxel::MaterialId as M;

impl TerraGenerator {
    pub(super) fn with_savanna(&self, mut sample: BiomeSample, x: f64, z: f64) -> BiomeSample {
        let scale = self.config().biome_scale.max(360.) * 0.81;
        let region = self.hills.get([x / scale + 79.17, z / scale - 31.86]);
        let warm = smoothstep(0.42, 0.62, sample.temperature);
        let grassland = 1. - smoothstep(0.49, 0.72, sample.humidity);
        let fraction = smoothstep(-0.09, 0.30, region) * warm * grassland;
        let w = &mut sample.weights;
        for source in [&mut w.plains, &mut w.forest, &mut w.desert] {
            let cover = *source * fraction;
            *source -= cover;
            w.savanna += cover;
        }
        sample
    }
    pub(super) fn savanna_height(&self, x: f64, z: f64, raw: f64, weight: f64) -> f64 {
        if weight < 0.03 {
            return raw;
        }
        let kopje = self.valleys.get([x / 152. + 64.3, z / 171. - 81.7]);
        let strata = self.hills.get([x / 51. + 27., z / 48. + 19.]) * 0.13 + kopje;
        let terraces = smoothstep(0.21, 0.39, strata) * 7.
            + smoothstep(0.38, 0.46, strata) * 15.
            + smoothstep(0.53, 0.60, strata) * 12.;
        raw + terraces
            * smoothstep(0.03, 0.88, weight)
            * (self.config().mountain_amplitude / 40.).clamp(0., 1.3)
    }
    pub(super) fn savanna_ground(
        &self,
        x: f64,
        z: f64,
        slope: f64,
        damp: f64,
        submerged: bool,
    ) -> M {
        let patch = self.hills.get([x / 39. + 55.7, z / 45. - 31.9]);
        if submerged {
            return if patch > 0.18 {
                M::Gravel
            } else {
                M::SavannaLoam
            };
        }
        if slope > 1.18 {
            return if patch > 0.3 {
                M::OchreStone
            } else if patch < -0.31 {
                M::WeatheredSavannaStone
            } else {
                M::LayeredSavannaRock
            };
        }
        if damp > 0.14 {
            M::SavannaTurf
        } else if patch < -0.46 {
            M::DrySavannaSoil
        } else if patch < 0.13 {
            M::DrySavannaTurf
        } else {
            M::SavannaTurf
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use terra_core::{BiomeKind, WorldConfig};

    #[test]
    fn natural_savanna_has_grassland_trees_and_ground_cover_only_on_new_worlds() {
        let cfg = WorldConfig {
            biome_definitions: Some(terra_core::BiomeDefinitions::default()),
            ..Default::default()
        };
        let terrain = TerraGenerator::new(cfg.clone()).unwrap();
        let old = TerraGenerator::new(WorldConfig {
            ecology_revision: 16,
            ..cfg
        })
        .unwrap();
        let (x, z) = (-96, 1152);
        let column = terrain.surface_column(f64::from(x) + 0.5, f64::from(z) + 0.5);
        assert_eq!(
            column.biome_kind(terrain.config().sea_level),
            BiomeKind::Savanna
        );
        assert_eq!(
            old.biome_at(f64::from(x) + 0.5, f64::from(z) + 0.5)
                .weights
                .savanna,
            0.
        );
        assert!(matches!(
            column.surface_material,
            M::SavannaTurf | M::DrySavannaTurf
        ));
        let cover = terrain.surface_flora([-3, 36]);
        for grass in [M::SavannaShortGrass, M::SavannaTallGrass] {
            assert!(cover.iter().filter(|(_, m)| *m == grass as u16).count() > 40);
        }
        assert!(
            terrain
                .tree_appearance([-96, 0, 1152])
                .tree_roots()
                .any(|(_, look)| matches!(
                    look,
                    super::super::TreeAppearance::Acacia | super::super::TreeAppearance::Baobab
                ))
        );
        let pool = terrain.surface_column(2507., 2115.);
        assert_eq!(
            pool.biome_kind(terrain.config().sea_level),
            BiomeKind::Savanna
        );
        assert!(
            pool.water_level
                .is_some_and(|level| level - pool.height > 3.)
        );
    }
}
