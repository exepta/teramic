//! Revision-gated Spirit Blossom ecology. Every decision uses global coordinates;
//! section ordering, meshing and clients cannot change the generated cells.
use super::{SurfaceColumn, TerraGenerator, biomes::smoothstep, seed::layer_seed};
use noise::NoiseFn;
use terra_voxel::MaterialId;
mod wetlands;

impl TerraGenerator {
    /// Broad mossy shelves preserve the existing river/lake erosion afterwards.
    pub(super) fn regional_base_height(
        &self,
        x: f64,
        z: f64,
        height: f64,
        climate: super::biomes::BiomeSample,
    ) -> f64 {
        let park_enabled =
            self.config().ecology_revision >= 12 && self.config().landscape_revision >= 9;
        if !self.config().biomes || (self.config().spirit_blossom_revision == 0 && !park_enabled) {
            return height;
        }
        let weights = climate.weights;
        let height = self.marine_height(x, z, climate, height);
        let height = self.cherry_terraces(x, z, height, weights.cherry_forest);
        let weight = weights.spirit_blossom;
        let spirit_height = if self.config().spirit_blossom_revision == 0 || weight <= 0.0 {
            height
        } else if self.config().landscape_revision >= 8 {
            let district = self.hills.get([x / 875.0 + 6.7, z / 875.0 - 4.1]);
            let fraction = if self.config().ecology_revision >= 15
                && self.config().spirit_blossom_revision >= 2
            {
                // Broad meadow treads alternate with exposed lilac risers. Hydrology
                // carves the stream bed and pond shore after this terrain stage.
                0.30 + smoothstep(-0.48, 0.49, district) * 0.68
            } else {
                smoothstep(0.12, 0.58, district) * 0.45
            };
            height + (terraced_height(height, weight) - height) * fraction
        } else {
            terraced_height(height, weight)
        };
        let height_with_desert = self.desert_height(
            x,
            z,
            self.highland_height(x, z, spirit_height, weights.highland),
            weights.desert,
        );
        let height_with_plains = self.plains_height(x, z, height_with_desert, weights.plains);
        if park_enabled && weights.mushroom_park > 0.10 {
            // The park's offset shelf contours add continuously to those from
            // the neighbouring Spirit region. Existing rivers cut through later.
            let offset = self.hills.get([x / 116.0, z / 116.0]) * 3.5;
            let shifted = height + offset;
            height_with_plains + (terraced_height(shifted, weights.mushroom_park) - shifted)
        } else {
            height_with_plains
        }
    }

    pub(super) fn spirit_decoration(
        &self,
        [x, y, z]: [f64; 3],
        column: SurfaceColumn,
    ) -> Option<MaterialId> {
        if self.config().spirit_blossom_revision == 0 || column.biome.weights.spirit_blossom < 0.3 {
            return None;
        }
        // Reserved walkable portal/river passages stay clear, including open mouths.
        if column
            .river_cavity
            .or(column.cave_cavity)
            .is_some_and(|[floor, ceiling]| y > floor && y < ceiling)
        {
            return None;
        }
        let ground = (column.height - 0.5).floor() as i64;
        let yy = y.floor() as i64;
        let water = column
            .water_level
            .filter(|level| *level > column.height + 0.5);
        // Revision six groups land flora in shared bounded patches. Keep the
        // existing water-surface lilies; they are not dry-ground decorations.
        if self.config().ecology_revision >= 6 && water.is_none() {
            return None;
        }
        let expected = water.map_or(ground + 1, |level| level.ceil() as i64);
        if yy != expected || column.slope > 1.8 {
            return None;
        }
        let ticket = layer_seed(
            self.config().seed.0
                ^ (x.floor() as i64 as u64).wrapping_mul(0x9e3779b97f4a7c15)
                ^ (z.floor() as i64 as u64).wrapping_mul(0xbf58476d1ce4e5b9),
            736,
        );
        decoration(ticket, column.biome.weights.spirit_blossom, water.is_some())
    }
}

/// Smooth five-metre moss shelves; zero weight is an exact legacy pass-through.
pub(super) fn terraced_height(height: f64, weight: f64) -> f64 {
    if weight <= 0.0 {
        return height;
    }
    let step = 5.0;
    let terrace = (height / step).floor() * step
        + smoothstep(0.66, 1.0, (height / step).rem_euclid(1.0)) * step;
    height + (terrace - height) * smoothstep(0.1, 0.9, weight) * 0.82
}

fn decoration(ticket: u32, weight: f64, water: bool) -> Option<MaterialId> {
    // Coherent biome density fades out; no sharp rectangular decoration border.
    let acceptance = f64::from(ticket >> 16) / 65536.0;
    if acceptance >= smoothstep(0.25, 0.9, weight) {
        return None;
    }
    let choice = ticket & 0xffff;
    if water {
        return match choice % 96 {
            0 => Some(MaterialId::LotusFlower),
            1..=3 => Some(MaterialId::LilyPad),
            _ => None,
        };
    }
    match choice % 512 {
        0 => Some(MaterialId::SpiritShrineStone),
        1..=2 => Some(MaterialId::SpiritLanternPost),
        3..=12 => Some(MaterialId::BlossomBush),
        13..=34 => Some(MaterialId::GlowingSpiritFlowers),
        35..=65 => Some(MaterialId::PetalGroundCover),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use terra_core::{Config, WorldConfig};

    #[test]
    fn saved_worlds_keep_legacy_ecology_and_revision_is_part_of_cache_identity() {
        let old = WorldConfig::from_toml("seed = 42").unwrap();
        assert_eq!(old.spirit_blossom_revision, 0);
        let mut new = old.clone();
        new.spirit_blossom_revision = 1;
        assert!(!old.same_terrain(&new));
        assert!(WorldConfig::from_toml("spirit_blossom_revision = 3").is_err());
        let old = TerraGenerator::new(old).unwrap();
        for x in [-8000.0, -1.0, 0.0, 1850.0, 8192.0] {
            assert_eq!(old.biome_at(x, -x).weights.spirit_blossom, 0.0);
        }
    }

    #[test]
    fn all_land_and_water_decorations_have_reproducible_bounded_density() {
        let mut observed = std::collections::BTreeSet::new();
        for ticket in 0..65536 {
            for water in [false, true] {
                let first = decoration(ticket, 1.0, water);
                assert_eq!(first, decoration(ticket, 1.0, water));
                assert_eq!(decoration(ticket, 0.0, water), None);
                if let Some(material) = first {
                    observed.insert(material as u16);
                }
            }
        }
        assert_eq!(
            observed,
            [316, 317, 318, 319, 320, 322, 323].into_iter().collect()
        );
    }

    #[test]
    fn spirit_ornaments_never_occupy_reserved_walkable_portal_or_river_air() {
        let terrain = TerraGenerator::new(WorldConfig {
            ecology_revision: 5,
            biomes: false,
            caves: false,
            water: false,
            ..Default::default()
        })
        .unwrap();
        let mut column = terrain.surface_column(0.5, 0.5);
        column.height = 10.2;
        column.slope = 0.0;
        column.water_level = None;
        column.river_cavity = None;
        column.cave_cavity = None;
        column.biome.weights.spirit_blossom = 1.0;
        let position = (0..512)
            .map(|x| [f64::from(x) + 0.5, 10.5, -1.5])
            .find(|p| terrain.spirit_decoration(*p, column).is_some())
            .unwrap();
        for river in [false, true] {
            let reserved = SurfaceColumn {
                river_cavity: river.then_some([10.0, 15.0]),
                cave_cavity: (!river).then_some([10.0, 15.0]),
                ..column
            };
            assert_eq!(terrain.spirit_decoration(position, reserved), None);
        }
    }

    #[test]
    fn spirit_regions_are_deterministic_with_continuous_normalized_boundaries() {
        let config = WorldConfig::default();
        let generator = TerraGenerator::new(config.clone()).unwrap();
        let duplicate = TerraGenerator::new(config).unwrap();
        let mut maximum: f64 = 0.0;
        for z in (-10000..=10000).step_by(500) {
            for x in (-10000..=10000).step_by(500) {
                let a = generator.climate_biome_at(x as f64, z as f64);
                assert_eq!(a, duplicate.climate_biome_at(x as f64, z as f64));
                assert!((a.weights.values().iter().sum::<f64>() - 1.0).abs() < 1e-10);
                let b = generator.climate_biome_at(x as f64 + 1.0, z as f64);
                assert!((a.weights.spirit_blossom - b.weights.spirit_blossom).abs() < 0.025);
                maximum = maximum.max(a.weights.spirit_blossom);
            }
        }
        assert!(
            maximum > 0.65,
            "a complete biome interior must be reachable: {maximum}"
        );
    }
}

#[cfg(test)]
mod integration_tests {
    use super::*;
    use crate::{SectionGenerator, generation::vegetation::trees_in_region};
    use terra_core::{Config, WorldConfig};
    use terra_voxel::{SectionPos, global_to_section};

    fn standard_world() -> TerraGenerator {
        TerraGenerator::new(
            WorldConfig::from_toml(include_str!("../../../../configs/world.toml")).unwrap(),
        )
        .unwrap()
    }

    #[test]
    fn actual_negative_spirit_region_contains_ground_trees_and_persistent_section_decoration() {
        let terrain = standard_world();
        let mut center = None;
        'find: for z in (-8192..=8192).step_by(256) {
            for x in (-8192..=-256).step_by(256) {
                if [[0.0, 0.0], [-48.0, -48.0], [48.0, 48.0]]
                    .into_iter()
                    .any(|[dx, dz]| {
                        terrain
                            .climate_biome_at(x as f64 + dx, z as f64 + dz)
                            .weights
                            .spirit_blossom
                            < 0.8
                    })
                {
                    continue;
                }
                let c = terrain.surface_column(x as f64 + 0.5, z as f64 + 0.5);
                if c.surface_material == MaterialId::SpiritBlossomGrass
                    && c.height > terrain.config().sea_level + 4.0
                {
                    center = Some([x, c.height.floor() as i64, z]);
                    break 'find;
                }
            }
        }
        let center = center
            .expect("seed 42042 has an accessible Spirit Blossom interior in negative coordinates");
        eprintln!("Spirit Blossom seed 42042 core: {center:?}");
        let trees = trees_in_region(
            &terrain,
            [center[0] - 64, center[1] - 64, center[2] - 64],
            128,
        );
        let tree = trees
            .iter()
            .find(|tree| tree.wood_material == MaterialId::SpiritBlossomLog)
            .expect("a real spirit region must contain blossom trees");
        let tree_section = global_to_section(tree.origin).0;
        // Ecology six intentionally leaves glades between regional plant
        // groups. Four arbitrary sections no longer guarantee a flower: locate
        // an actual group in this bounded 160m square, then verify its complete
        // generated section rather than synthesizing a decoration fixture.
        let tile = [center[0].div_euclid(32), center[2].div_euclid(32)];
        let decoration = (-2..=2)
            .flat_map(|z| (-2..=2).map(move |x| [tile[0] + x, tile[1] + z]))
            .flat_map(|tile| terrain.surface_flora(tile))
            .find(|(_, material)| matches!(material, 316..=323 | 610..=612))
            .expect("the Spirit interior must contain a real regional decoration group");
        let generator = SectionGenerator::new(terrain);
        let ground = global_to_section(center).0;
        let positions = [
            ground,
            SectionPos::new(ground.x + 1, ground.y, ground.z),
            tree_section,
            SectionPos::new(tree_section.x, tree_section.y + 1, tree_section.z),
            global_to_section(decoration.0).0,
        ];
        let first: Vec<_> = positions
            .into_iter()
            .map(|p| generator.generate(p).unwrap())
            .collect();
        for (p, expected) in positions.into_iter().zip(&first).rev() {
            assert_eq!(&generator.generate(p).unwrap(), expected);
            let meshed = generator.build(p, 0, &Default::default(), true).unwrap();
            assert_eq!(&meshed.data, expected);
            let origin = p.origin().unwrap();
            for local in [[0, 0, 0], [15, 0, 15], [0, 15, 0], [15, 15, 15]] {
                let global = std::array::from_fn(|i| origin[i] + local[i] as i64);
                assert_eq!(
                    expected.block(local).unwrap(),
                    generator.block_at(global).unwrap(),
                    "section seam at {global:?}"
                );
            }
        }
        let cells: Vec<_> = first.iter().flat_map(|s| s.blocks()).copied().collect();
        assert!(cells.contains(&(MaterialId::SpiritBlossomGrass as u16)));
        assert!(cells.contains(&(MaterialId::SpiritBlossomLog as u16)));
        assert!(cells.iter().any(|m| (606..=607).contains(m)));
        assert!(
            cells.iter().any(|m| matches!(m, 316..=323 | 610..=612)),
            "the region must contain actual placed flowers, petals or lanterns"
        );
        let (_, local) = global_to_section(decoration.0);
        assert_eq!(first[4].block(local).unwrap(), decoration.1);
        assert_eq!(generator.block_at(decoration.0).unwrap(), decoration.1);
    }

    #[test]
    fn spirit_terraces_are_continuous_and_do_not_modify_zero_weight_legacy_terrain() {
        for y in -1200..3500 {
            let height = y as f64 / 10.0;
            assert_eq!(terraced_height(height, 0.0), height);
            let a = terraced_height(height, 1.0);
            let b = terraced_height(height + 0.001, 1.0);
            assert!((b - a).abs() < 0.01);
            assert!((a - height).abs() < 5.0);
        }
    }
}
