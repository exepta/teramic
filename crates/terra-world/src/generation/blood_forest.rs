//! Globally addressed crimson ecology. Saved revision zero is an exact pass-through.
use super::{SurfaceColumn, TerraGenerator, biomes::smoothstep, seed::layer_seed};
use noise::NoiseFn;
use terra_voxel::MaterialId;

impl TerraGenerator {
    /// Material-only blend: coherent multi-meter patches grow through the biome
    /// transition. Global coordinates keep neighboring/reloaded columns identical.
    pub(super) fn blood_surface_material(
        &self,
        [x, z]: [f64; 2],
        weight: f64,
        exposure: f64,
        underlying: MaterialId,
    ) -> Option<MaterialId> {
        if self.config().blood_forest_revision == 0 || weight <= 0.15 {
            return None;
        }
        // Sparse woodland fringes must not punch red holes through coherent
        // dunes, snow contours or marine sediment. Those surfaces only become
        // eligible once Blood Forest dominates, with a separate smooth onset.
        let woodland = matches!(
            underlying,
            MaterialId::Grass
                | MaterialId::Dirt
                | MaterialId::MossyStone
                | MaterialId::SpiritBlossomGrass
                | MaterialId::SpiritBlossomDirt
                | MaterialId::BloodGrass
                | MaterialId::BloodDirt
                | MaterialId::BloodMoss
                | MaterialId::CrimsonMycelium
        );
        let coverage = if woodland {
            smoothstep(0.15, 0.85, weight)
        } else {
            smoothstep(0.5, 1.0, weight)
        };
        if coverage == 0.0 {
            return None;
        }
        if coverage < 1.0 {
            // Separate from the moss/mycelium field: neither per-voxel hashes
            // nor chunk-local coordinates, so edges form patches rather than noise.
            let patch = self.hills.get([x * 0.055 + 183.4, z * 0.055 - 97.2]) * 0.5 + 0.5;
            if patch >= coverage {
                return None;
            }
        }
        let ground_patch = self.hills.get([x * 0.035 + 17.0, z * 0.035 - 9.0]);
        Some(if exposure > 0.6 {
            MaterialId::BloodStone
        } else if ground_patch > 0.25 {
            MaterialId::BloodMoss
        } else if ground_patch < -0.25 {
            MaterialId::CrimsonMycelium
        } else {
            MaterialId::BloodDirt
        })
    }

    pub(super) fn blood_decoration(
        &self,
        [x, y, z]: [f64; 3],
        column: SurfaceColumn,
    ) -> Option<MaterialId> {
        if self.config().ecology_revision >= 5 {
            // New worlds use cached habitat-aware groups in the section pipeline.
            return None;
        }
        let weight = column.biome.weights.blood_forest;
        if self.config().blood_forest_revision == 0 || weight < 0.25 || column.slope > 1.6 {
            return None;
        }
        if column
            .water_level
            .is_some_and(|level| level > column.height + 0.5)
            || column
                .river_cavity
                .or(column.cave_cavity)
                .is_some_and(|[low, high]| y > low && y < high)
            || y.floor() as i64 != (column.height - 0.5).floor() as i64 + 1
        {
            return None;
        }
        let seed = layer_seed(
            self.config().seed.0
                ^ (x.floor() as i64 as u64).wrapping_mul(0x9e3779b97f4a7c15)
                ^ (z.floor() as i64 as u64).wrapping_mul(0xbf58476d1ce4e5b9),
            914,
        );
        if f64::from(seed >> 16) / 65536.0 >= smoothstep(0.25, 0.85, weight) {
            return None;
        }
        match seed % 512 {
            0 => Some(MaterialId::BloodRuinedArch),
            1..=2 => Some(MaterialId::BloodCrystalPillar),
            3..=5 => Some(MaterialId::GiantBloodMushroom),
            6..=8 => Some(MaterialId::BloodBones),
            9..=18 => Some(MaterialId::BloodRoots),
            19..=31 => Some(MaterialId::BloodThornBush),
            32..=48 => Some(MaterialId::BloodFlower),
            49..=65 => Some(MaterialId::BloodMushrooms),
            66..=88 => Some(MaterialId::BloodFern),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use terra_core::{Config, WorldConfig};
    #[test]
    fn blood_revision_preserves_old_worlds_and_is_part_of_generation_identity() {
        let old = WorldConfig::from_toml("seed=42").unwrap();
        assert_eq!(old.blood_forest_revision, 0);
        let mut new = old.clone();
        new.blood_forest_revision = 1;
        assert!(!old.same_terrain(&new));
        let old = TerraGenerator::new(old).unwrap();
        for x in [-8192., -1., 0., 1024., 8192.] {
            assert_eq!(old.biome_at(x, -x).weights.blood_forest, 0.);
        }
    }
    #[test]
    fn blood_regions_are_reproducible_and_boundaries_remain_continuous() {
        let world = TerraGenerator::new(WorldConfig::default()).unwrap();
        let other = TerraGenerator::new(WorldConfig::default()).unwrap();
        let mut maximum: f64 = 0.;
        for x in (-10000..10000).step_by(320) {
            for z in (-10000..10000).step_by(320) {
                let a = world.biome_at(x as f64, z as f64);
                let b = world.biome_at(x as f64 + 1., z as f64);
                assert_eq!(a, other.biome_at(x as f64, z as f64));
                assert!((a.weights.values().iter().sum::<f64>() - 1.).abs() < 1e-9);
                assert!((a.weights.blood_forest - b.weights.blood_forest).abs() < 0.03);
                maximum = maximum.max(a.weights.blood_forest);
            }
        }
        assert!(maximum > 0.65, "no Blood Forest interior: {maximum}");
    }

    #[test]
    fn material_transition_grows_in_coherent_seeded_patches_without_a_half_weight_cliff() {
        let world = TerraGenerator::new(WorldConfig::default()).unwrap();
        let restored = TerraGenerator::new(world.config().clone()).unwrap();
        let legacy = TerraGenerator::new(WorldConfig {
            blood_forest_revision: 0,
            ..world.config().clone()
        })
        .unwrap();
        let weights = [0.0, 0.4, 0.5, 0.6, 1.0];
        let mut coverage = [0; 5];
        let mut neighbors = 0;
        let mut changes_at_half = 0;
        let mut cells = 0;
        for z in (-256..256).step_by(4) {
            for x in (-256..256).step_by(4) {
                let point = [f64::from(x) + 0.5, f64::from(z) + 0.5];
                cells += 1;
                let mut selected = false;
                for (index, weight) in weights.into_iter().enumerate() {
                    let material =
                        world.blood_surface_material(point, weight, 0.0, MaterialId::Grass);
                    assert_ne!(material, Some(MaterialId::BloodGrass));
                    assert_eq!(
                        material,
                        restored.blood_surface_material(point, weight, 0.0, MaterialId::Grass)
                    );
                    // A growing biome must not remove an already selected patch.
                    assert!(!selected || material.is_some());
                    selected = material.is_some();
                    coverage[index] += usize::from(selected);
                }
                assert!(
                    legacy
                        .blood_surface_material(point, 1.0, 0.0, MaterialId::Grass)
                        .is_none()
                );
                let here = world
                    .blood_surface_material(point, 0.5, 0.0, MaterialId::Grass)
                    .is_some();
                let next = world
                    .blood_surface_material([point[0] + 1.0, point[1]], 0.5, 0.0, MaterialId::Grass)
                    .is_some();
                neighbors += usize::from(here == next);
                changes_at_half += usize::from(
                    world
                        .blood_surface_material(point, 0.499, 0.0, MaterialId::Grass)
                        .is_some()
                        != world
                            .blood_surface_material(point, 0.501, 0.0, MaterialId::Grass)
                            .is_some(),
                );
            }
        }
        assert_eq!(coverage[0], 0);
        assert_eq!(coverage[4], cells);
        assert!(
            coverage.windows(2).all(|pair| pair[0] < pair[1]),
            "{coverage:?}"
        );
        assert!(
            neighbors > cells * 4 / 5,
            "patches must span multiple blocks"
        );
        assert!(changes_at_half < cells / 20, "half-weight material cliff");
        // Reverse query order crosses negative section boundaries without state.
        for x in [-32.5, -16.5, -0.5, 0.5, 15.5, 16.5].into_iter().rev() {
            for weight in weights.into_iter().rev() {
                assert_eq!(
                    world.blood_surface_material([x, -16.5], weight, 0.0, MaterialId::Grass),
                    restored.blood_surface_material([x, -16.5], weight, 0.0, MaterialId::Grass)
                );
            }
        }
    }

    #[test]
    fn minority_blood_forest_preserves_sand_snow_and_marine_surfaces() {
        let world = TerraGenerator::new(WorldConfig::default()).unwrap();
        for surface in [
            MaterialId::Sand,
            MaterialId::Snow,
            MaterialId::Clay,
            MaterialId::Stone,
        ] {
            let mut mixed = 0;
            let mut cells = 0;
            for z in (-64..64).step_by(4) {
                for x in (-64..64).step_by(4) {
                    let point = [f64::from(x) + 0.5, f64::from(z) + 0.5];
                    for weight in [0.0, 0.25, 0.49, 0.5] {
                        assert!(
                            world
                                .blood_surface_material(point, weight, 0.0, surface)
                                .is_none(),
                            "minority Blood Forest replaced {surface:?} at {point:?}"
                        );
                    }
                    assert!(
                        world
                            .blood_surface_material(point, 1.0, 0.0, surface)
                            .is_some()
                    );
                    mixed += usize::from(
                        world
                            .blood_surface_material(point, 0.75, 0.0, surface)
                            .is_some(),
                    );
                    cells += 1;
                }
            }
            assert!(
                mixed > cells / 5 && mixed < cells * 4 / 5,
                "majority transition must still blend"
            );
        }
    }

    #[test]
    fn soil_follows_selected_surface_instead_of_biome_majority() {
        let world = TerraGenerator::new(WorldConfig {
            biomes: false,
            water: false,
            caves: false,
            trees: false,
            ..Default::default()
        })
        .unwrap();
        let mut base = world.surface_column(-16.5, -32.5);
        base.height = 20.2;
        base.soil_depth = 5.0;
        for weight in [0.2, 0.8] {
            base.biome.weights.blood_forest = weight;
            base.biome.weights.forest = 1.0 - weight;
            for (surface, dirt, stone) in [
                (MaterialId::Grass, MaterialId::Dirt, MaterialId::Stone),
                (
                    MaterialId::BloodDirt,
                    MaterialId::BloodDirt,
                    MaterialId::BloodStone,
                ),
                (
                    MaterialId::BloodMoss,
                    MaterialId::BloodDirt,
                    MaterialId::BloodStone,
                ),
                (
                    MaterialId::CrimsonMycelium,
                    MaterialId::BloodDirt,
                    MaterialId::BloodStone,
                ),
                (
                    MaterialId::BloodStone,
                    MaterialId::BloodStone,
                    MaterialId::BloodStone,
                ),
            ] {
                let column = SurfaceColumn {
                    surface_material: surface,
                    ..base
                };
                let top = (column.height - 0.5).floor() + 0.5;
                assert_eq!(
                    world.column_material(column, column.height - top, -0.5),
                    surface
                );
                for depth in [1.7, 2.7, 3.7] {
                    assert_eq!(world.column_material(column, depth, -depth), dirt);
                }
                assert_eq!(world.column_material(column, 8.0, -8.0), stone);
                assert_eq!(world.column_material(column, -0.5, 0.5), MaterialId::Air);
            }
        }
    }
}
