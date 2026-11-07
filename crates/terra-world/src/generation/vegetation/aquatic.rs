//! Underwater occupants of the existing deterministic ground-prop generator.
use super::{mix, random};
use crate::{
    SurfaceColumn, TerraGenerator,
    props::{PropId, PropKind, WorldProp},
};
use terra_voxel::{MaterialId, SectionPos};

impl TerraGenerator {
    pub(in crate::generation) fn aquatic_prop(
        &self,
        support: [i64; 3],
        section: SectionPos,
        material: u16,
        column: SurfaceColumn,
        depth: f64,
    ) -> Option<WorldProp> {
        let modern =
            self.config().landscape_revision >= 4 && column.ocean.max_vegetation_depth > 0.0;
        let max_depth = if modern && column.ocean.max_vegetation_depth > 0.0 {
            f64::from(column.ocean.max_vegetation_depth)
        } else {
            40.0
        };
        if !(0.65..=max_depth).contains(&depth)
            || !MaterialId::from_raw(material).is_some_and(MaterialId::supports_water_plants)
        {
            return None;
        }
        let mut seed = mix(self.config().seed.0 ^ 0x6171_7561_5072_6f70);
        for coordinate in support {
            seed = mix(seed ^ (coordinate as u64).wrapping_mul(0x9e3779b97f4a7c15));
        }
        let habitat = column.ocean;
        let [reef, forest, cold] = habitat.weights;
        let marine = reef + forest + cold;
        let soft = matches!(
            MaterialId::from_raw(material),
            Some(MaterialId::Sand | MaterialId::Dirt | MaterialId::Clay | MaterialId::Mud)
        );
        let light = if modern {
            // Deep meadows thin out gradually; the configurable marine floor
            // range no longer falls through a hard legacy forty-metre cutoff.
            (1.0 - ((depth / max_depth - 0.45) / 0.55).clamp(0.0, 0.85)) as f32
        } else {
            (1.0 - ((depth - 16.0) / 24.0).clamp(0.0, 1.0)) as f32
        };
        let shelf = habitat.rock * (1.0 - (column.slope as f32 / 3.5).clamp(0.0, 0.75));
        let coral = if modern {
            habitat.coral_density * shelf * habitat.cover * light
        } else {
            reef * shelf * habitat.cover * light
        };
        let meadow = if modern {
            let substrate = match MaterialId::from_raw(material) {
                Some(MaterialId::Gravel) => 0.70,
                Some(MaterialId::Stone) => 0.25,
                _ if soft => 1.0,
                _ => 0.30,
            };
            habitat.seagrass_density * habitat.cover * light * substrate
        } else {
            forest * habitat.cover * light * if soft { 1.0 } else { 0.30 }
        };
        let density = if marine > 0.1 {
            if modern {
                (0.015 + coral + meadow).min(0.60)
            } else {
                (0.025 + coral * 0.47 + meadow * 0.48 + cold * habitat.cover * 0.12).min(0.52)
            }
        } else if depth < 12.0 && soft {
            0.08
        } else {
            0.018
        };
        if random(seed, 1) > density {
            return None;
        }
        let choice = random(seed, 2);
        let kind = if coral > 0.07 && choice < 0.82 {
            // Colonies follow a continuous field; ledges prefer plates, lower pockets tubes.
            if column.slope > 0.65 && habitat.colony > 0.42 {
                PropKind::PlateCoral
            } else if habitat.colony > 0.64 {
                PropKind::TubeCoral
            } else {
                PropKind::BranchingCoral
            }
        } else if reef > 0.35
            && habitat.cover > 0.25
            && shelf > 0.25
            && choice < 0.18
            && (!modern || habitat.coral_density > 0.0)
        {
            PropKind::SeaAnemone
        } else if marine > 0.2 && soft && habitat.cover < 0.60 && choice > 0.91 {
            PropKind::SeaCucumber
        } else if (habitat.cover < 0.1 && marine > 0.1)
            || choice > 0.85
            || (!modern && depth > 29.0)
            || (modern && meadow <= 0.0)
        {
            PropKind::SmallStones
        } else {
            PropKind::Seagrass
        };
        let scale = if kind == PropKind::Seagrass {
            0.35 + forest * habitat.cover * 1.05 + cold * 0.30 + random(seed, 4) * 0.30
        } else {
            0.65 + random(seed, 4) * 0.30
        };
        let mut prop = WorldProp {
            id: PropId(seed),
            kind,
            position: [
                support[0] as f64 + 0.5,
                support[1] as f64 + 1.0,
                support[2] as f64 + 0.5,
            ],
            rotation: random(seed, 3) * std::f32::consts::TAU,
            scale,
            variant: (random(seed, 5) * 8.0) as u8,
            section,
            removed: false,
        };
        let available = column
            .ice_bottom()
            .map_or(depth, |bottom| depth.min(bottom - prop.position[1]))
            - 0.2;
        let height = prop.bounds()[1][1] - prop.position[1];
        if kind == PropKind::Seagrass && height > available {
            prop.scale *= (available / height) as f32;
        }
        (prop.valid() && prop.bounds()[1][1] - prop.position[1] <= available).then_some(prop)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reefs_are_deterministic_rooted_underwater_and_include_all_seven_species() {
        let terrain = TerraGenerator::new(terra_core::WorldConfig {
            landscape_revision: 3,
            ..Default::default()
        })
        .unwrap();
        let mut column = terrain.surface_column(0.5, 0.5);
        column.biome.weights.ocean = 1.0;
        column.biome.temperature = 0.85;
        let mut species = std::collections::BTreeSet::new();
        for z in -96..96 {
            for x in -96..96 {
                column.ocean =
                    terrain.ocean_with_biome(x as f64 + 0.5, z as f64 + 0.5, column.biome);
                column.slope = if z < 0 { 0.8 } else { 0.1 };
                let support = [x, -17, z];
                let section = terra_voxel::global_to_section(support).0;
                let a =
                    terrain.aquatic_prop(support, section, MaterialId::Sand as u16, column, 8.0);
                let b =
                    terrain.aquatic_prop(support, section, MaterialId::Sand as u16, column, 8.0);
                assert_eq!(a, b);
                if let Some(prop) = a {
                    species.insert(prop.kind as u8);
                    assert!(prop.valid() && prop.water_tolerant());
                    assert_eq!(crate::props::placement::support(&prop), support);
                    assert!(prop.bounds()[1][1] < prop.position[1] + 8.0);
                }
                assert!(
                    terrain
                        .aquatic_prop(support, section, 0, column, 8.0)
                        .is_none()
                );
                assert!(
                    terrain
                        .aquatic_prop(support, section, 4, column, 0.1)
                        .is_none()
                );
            }
        }
        assert_eq!(species.len(), 7);
        column.biome.weights.ocean = 0.0;
        column.ocean = Default::default();
        for x in -32..32 {
            let support = [x, -17, 0];
            if let Some(prop) = terrain.aquatic_prop(
                support,
                terra_voxel::global_to_section(support).0,
                4,
                column,
                8.0,
            ) {
                assert!(matches!(
                    prop.kind,
                    PropKind::Seagrass | PropKind::SmallStones
                ));
            }
        }
    }

    #[test]
    fn kelp_forests_have_tall_fronds_and_cold_water_excludes_tropical_corals() {
        let terrain = TerraGenerator::new(terra_core::WorldConfig {
            landscape_revision: 3,
            ..Default::default()
        })
        .unwrap();
        let mut column = terrain.surface_column(0.5, 0.5);
        column.biome.weights.ocean = 1.0;
        column.slope = 0.1;
        column.water_level = Some(8.0);
        let mut tall = 0;
        let mut cold_plants = 0;
        for temperature in [0.12, 0.41] {
            column.biome.temperature = temperature;
            for z in (-96..96).step_by(2) {
                for x in (-96..96).step_by(2) {
                    column.ocean =
                        terrain.ocean_with_biome(x as f64 + 0.5, z as f64 + 0.5, column.biome);
                    let support = [x, -9, z];
                    let section = terra_voxel::global_to_section(support).0;
                    if let Some(prop) = terrain.aquatic_prop(support, section, 4, column, 16.0) {
                        let height = prop.bounds()[1][1] - prop.position[1];
                        if temperature < 0.2 {
                            assert!(matches!(
                                prop.kind,
                                PropKind::Seagrass | PropKind::SmallStones | PropKind::SeaCucumber
                            ));
                            cold_plants += 1;
                        } else if prop.kind == PropKind::Seagrass && height > 3.0 {
                            tall += 1;
                        }
                        if let Some(bottom) = column.ice_bottom() {
                            assert!(prop.bounds()[1][1] < bottom);
                        }
                    }
                }
            }
        }
        assert!(
            tall > 50 && cold_plants > 50,
            "tall={tall}, cold={cold_plants}"
        );
    }

    #[test]
    fn revision_four_seagrass_reaches_deep_warm_cold_and_swamp_floors() {
        let terrain = TerraGenerator::new(terra_core::WorldConfig {
            landscape_revision: 4,
            sea_level: 22.0,
            rivers: true,
            biome_definitions: None,
            ..Default::default()
        })
        .unwrap();
        let mut column = terrain.surface_column(0.5, 0.5);
        column.slope = 0.1;
        column.water_level = Some(terrain.config().sea_level);
        for (temperature, humidity, ocean) in [(0.9, 0.25, 1.0), (0.1, 0.8, 1.0), (0.9, 0.95, 0.6)]
        {
            column.biome.temperature = temperature;
            column.biome.humidity = humidity;
            column.biome.weights.ocean = ocean;
            let mut seagrass = 0;
            let mut coral = 0;
            for z in (-96..96).step_by(2) {
                for x in (-96..96).step_by(2) {
                    column.ocean = terrain.ocean_with_biome(
                        f64::from(x) + 0.5,
                        f64::from(z) + 0.5,
                        column.biome,
                    );
                    let support = [
                        i64::from(x),
                        terrain.config().sea_level.floor() as i64 - 65,
                        i64::from(z),
                    ];
                    let section = terra_voxel::global_to_section(support).0;
                    let material = column.ocean.floor_material().unwrap() as u16;
                    let prop = terrain.aquatic_prop(support, section, material, column, 64.0);
                    assert_eq!(
                        prop,
                        terrain.aquatic_prop(support, section, material, column, 64.0)
                    );
                    if let Some(prop) = prop {
                        seagrass += usize::from(prop.kind == PropKind::Seagrass);
                        coral += usize::from(matches!(
                            prop.kind,
                            PropKind::PlateCoral | PropKind::TubeCoral | PropKind::BranchingCoral
                        ));
                        assert_eq!(crate::props::placement::support(&prop), support);
                        assert!(prop.bounds()[1][1] <= column.water_level.unwrap());
                    }
                    assert!(
                        terrain
                            .aquatic_prop(
                                support,
                                section,
                                material,
                                column,
                                f64::from(column.ocean.max_vegetation_depth) + 1.0
                            )
                            .is_none()
                    );
                }
            }
            assert!(
                seagrass > 50,
                "temperature={temperature} humidity={humidity} seagrass={seagrass}"
            );
            if temperature < 0.2 {
                assert_eq!(coral, 0);
            }
        }
    }

    #[test]
    fn revision_four_preserves_the_freshwater_vegetation_path_exactly() {
        let config = terra_core::WorldConfig {
            landscape_revision: 4,
            sea_level: 22.0,
            rivers: true,
            biome_definitions: Some(terra_core::BiomeDefinitions::default()),
            ..Default::default()
        };
        let modern = TerraGenerator::new(config.clone()).unwrap();
        let legacy = TerraGenerator::new(terra_core::WorldConfig {
            landscape_revision: 3,
            ..config
        })
        .unwrap();
        let mut column = modern.surface_column(0.5, 0.5);
        column.ocean = Default::default();
        column.water_level = Some(modern.config().sea_level);
        let mut seagrass = 0;
        for z in -48..48 {
            for x in -48..48 {
                let support = [x, modern.config().sea_level.floor() as i64 - 9, z];
                let section = terra_voxel::global_to_section(support).0;
                let current =
                    modern.aquatic_prop(support, section, MaterialId::Sand as u16, column, 8.0);
                assert_eq!(
                    current,
                    legacy.aquatic_prop(support, section, MaterialId::Sand as u16, column, 8.0)
                );
                seagrass +=
                    usize::from(current.is_some_and(|prop| prop.kind == PropKind::Seagrass));
            }
        }
        assert!(seagrass > 50);
    }
}
