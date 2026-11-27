//! Snowline, glacial relief and exposed surfaces of the modern alpine regions.
use super::{TerraGenerator, biomes::smoothstep};
use noise::NoiseFn;
use terra_voxel::MaterialId;

pub(super) const SNOWLINE: f64 = 110.0;

impl TerraGenerator {
    pub(super) fn highland_overhaul(&self) -> bool {
        self.config().ecology_revision >= 15 && self.config().landscape_revision >= 9
    }

    /// The uncarved climate field forms a mountain belt. Surface classification
    /// reads the final eroded height afterwards, including deep river cuts.
    pub(super) fn highland_height(&self, x: f64, z: f64, base: f64, weight: f64) -> f64 {
        if !self.highland_overhaul() || weight <= 0.12 {
            return base;
        }
        let warp = self.valleys.get([x / 310.0 + 7.7, z / 280.0 - 41.2]);
        let ridge = 1.0
            - self
                .mountains
                .get([x / 170.0 + warp * 0.45, z / 210.0 - 3.4])
                .abs();
        let cirque = self.hills.get([x / 108.0 - 29.4, z / 145.0 + 16.2]);
        let ramp = smoothstep(0.12, 0.93, weight);
        let strength = (self.config().mountain_amplitude / 40.0).clamp(0.0, 1.5);
        let crest = smoothstep(0.52, 0.98, ridge);
        let shelf = smoothstep(0.04, 0.22, cirque) * 9.0 + smoothstep(0.27, 0.42, cirque) * 12.0;
        let alpine = 126.0 + 0.22 * (base - 100.0) + crest * (20.0 + 29.0 * cirque) + shelf;
        base + (alpine - base) * ramp * strength
    }

    pub(super) fn highland_surface(&self, x: f64, z: f64, height: f64, slope: f64) -> MaterialId {
        let drift = self.hills.get([x / 29.0 + 48.2, z / 35.0 - 15.7]);
        if slope > 1.0 + drift * 0.35 {
            MaterialId::AlpineGranite
        } else if height < 146.0 + drift * 12.0 && drift > 0.12 {
            MaterialId::SnowCoveredSod
        } else if height > 161.0 && drift < -0.20 {
            MaterialId::IcySnow
        } else if slope > 0.36 || drift < -0.15 {
            MaterialId::Firn
        } else {
            MaterialId::Snow
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generation::biomes::BiomeSample;
    use terra_core::WorldConfig;
    use terra_voxel::MaterialId as M;

    #[test]
    fn carved_highland_floor_changes_ecology_only_above_the_saved_snowline() {
        let modern = TerraGenerator::new(WorldConfig {
            ecology_revision: 15,
            ..Default::default()
        })
        .unwrap();
        let legacy = TerraGenerator::new(WorldConfig {
            ecology_revision: 14,
            ..Default::default()
        })
        .unwrap();
        let mut candidate: BiomeSample = modern.climate_biome_at(0., 0.);
        candidate.weights = super::super::biomes::BiomeWeights {
            ocean: 0.,
            plains: 0.,
            forest: 0.,
            desert: 0.,
            highland: 1.,
            mushroom_park: 0.,
            cliffted_desert: 0.,
            cherry_forest: 0.,
            spirit_blossom: 0.,
            blood_forest: 0.,
            ashlands: 0.,
            savanna: 0.0,
            swamp: 0.0,
            petrified_forest: 0.0,
            crystal_highlands: 0.0,
            misty_coast: 0.0,
        };
        for y in [-30.0, 90.0, 109.999, 110.0] {
            let adjusted = modern.surface_ecology(candidate, y);
            assert_eq!(adjusted.weights.highland, 0.0);
            assert!((adjusted.weights.values().iter().sum::<f64>() - 1.0).abs() < 1e-10);
            assert_eq!(legacy.surface_ecology(candidate, y), candidate);
        }
        assert_eq!(
            modern.surface_ecology(candidate, 130.).weights.highland,
            1.0
        );
        let modern_base = modern.highland_height(73., -128., 68., 1.);
        assert!(modern_base > SNOWLINE);
        assert_eq!(legacy.highland_height(73., -128., 68., 1.), 68.);
        assert_eq!(
            modern.highland_surface(73., -128., 161., 2.),
            M::AlpineGranite
        );
    }

    fn shipped_terrain() -> TerraGenerator {
        use terra_core::BiomeDefinitions;
        TerraGenerator::new(WorldConfig {
            biome_definitions: Some(
                BiomeDefinitions::load_directory(terra_core::asset_directory().join("biome"))
                    .unwrap(),
            ),
            ..Default::default()
        })
        .unwrap()
    }

    #[test]
    fn alpine_lake_has_solid_ice_next_to_open_water() {
        let terrain = shipped_terrain();
        let ice = [-5204, 127, -5636];
        let x = ice[0] as f64 + 0.5;
        let z = ice[2] as f64 + 0.5;
        let shore = terrain.surface_column(x, z);
        assert!(shore.biome.weights.highland > 0.5);
        assert_eq!(shore.ice_bottom(), Some(ice[1] as f64));
        let generator = crate::SectionGenerator::new(terrain.clone());
        assert_eq!(generator.block_at(ice).unwrap(), M::Ice as u16);
        let floor = [ice[0], shore.water_bed() as i64 - 1, ice[2]];
        assert_eq!(shore.snow_cover, 0.0);
        assert!(matches!(
            M::from_raw(generator.block_at(floor).unwrap()),
            Some(M::Gravel | M::AlpineGranite | M::Clay | M::Stone)
        ));
        assert!([-8., 0., 8.].into_iter().any(|dx| {
            [-8., 0., 8.].into_iter().any(|dz| {
                let lake = terrain.surface_column(x + dx, z + dz);
                lake.water_level == shore.water_level && lake.ice_bottom().is_none()
            })
        }));
    }

    #[test]
    fn a_submerged_lake_floor_stays_sediment_above_the_configured_snowline() {
        let mut config = shipped_terrain().config().clone();
        config.snow_height = config.sea_level + 0.1;
        let terrain = TerraGenerator::new(config).unwrap();
        // This regional basin has a sediment bed above the test snowline.
        let [x, z] = [-1280, -1504];
        let column = terrain.surface_column(x as f64 + 0.5, z as f64 + 0.5);
        let lake = terrain.lake_at(x as f64 + 0.5, z as f64 + 0.5).unwrap();
        assert!(lake.distance < lake.radius);
        assert!(column.height > terrain.config().snow_height);
        assert!(column.water_level.unwrap() > column.water_bed());
        assert_eq!(column.snow_cover, 0.0);
        let floor = [x, column.water_bed() as i64 - 1, z];
        let generator = crate::SectionGenerator::new(terrain);
        assert!(matches!(
            M::from_raw(generator.block_at(floor).unwrap()),
            Some(M::Sand | M::Gravel | M::Clay | M::Stone)
        ));
    }

    #[test]
    fn alpine_props_are_real_editable_cells_on_deterministic_snowy_slopes() {
        use crate::{SectionGenerator, generation::TreeAppearance};
        use std::collections::BTreeSet;
        use terra_voxel::global_to_section;
        let terrain = shipped_terrain();
        let generator = SectionGenerator::new(terrain.clone());
        let mut props = BTreeSet::new();
        let tile = [-121, -13];
        // The sheltered shrubs and exposed scree occupy separate patches
        // on this slope. Sample both sides of their shared tile boundary.
        for (p, material) in [tile, [tile[0], tile[1] - 1]]
            .into_iter()
            .flat_map(|region| terrain.surface_flora(region))
        {
            if !(587..=595).contains(&material) || !props.insert(material) {
                continue;
            }
            let (section, local) = global_to_section(p);
            assert_eq!(
                generator.generate(section).unwrap().block(local).unwrap(),
                material
            );
            let column = terrain.surface_column(p[0] as f64 + 0.5, p[2] as f64 + 0.5);
            assert!(column.height >= SNOWLINE && column.biome.weights.highland > 0.5);
        }
        assert_eq!(props.len(), 9, "alpine prop types: {props:?}");
        let mut canopy = Vec::new();
        for dx in [0, 16] {
            for dz in [0, 16] {
                let trees = terrain.tree_appearance([tile[0] * 32 + dx, 128, tile[1] * 32 + dz]);
                canopy.extend(trees.tree_roots().map(|(_, kind)| kind));
            }
        }
        assert!(canopy.contains(&TreeAppearance::Birch));
        assert!(canopy.contains(&TreeAppearance::Spruce));
    }
}
