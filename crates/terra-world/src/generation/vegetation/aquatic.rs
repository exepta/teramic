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
        if !(0.65..=40.0).contains(&depth)
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
        let light = (1.0 - ((depth - 16.0) / 24.0).clamp(0.0, 1.0)) as f32;
        let shelf = habitat.rock * (1.0 - (column.slope as f32 / 3.5).clamp(0.0, 0.75));
        let coral = reef * shelf * habitat.cover * light;
        let meadow = forest * habitat.cover * light * if soft { 1.0 } else { 0.30 };
        let density = if marine > 0.1 {
            (0.025 + coral * 0.47 + meadow * 0.48 + cold * habitat.cover * 0.12).min(0.52)
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
        } else if reef > 0.35 && habitat.cover > 0.25 && shelf > 0.25 && choice < 0.18 {
            PropKind::SeaAnemone
        } else if marine > 0.2 && soft && habitat.cover < 0.60 && choice > 0.91 {
            PropKind::SeaCucumber
        } else if (habitat.cover < 0.1 && marine > 0.1) || choice > 0.85 || depth > 29.0 {
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
        let terrain = TerraGenerator::new(Default::default()).unwrap();
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
        let terrain = TerraGenerator::new(Default::default()).unwrap();
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
}
