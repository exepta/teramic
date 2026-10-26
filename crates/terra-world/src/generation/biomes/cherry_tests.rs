use super::*;
use crate::{TerraGenerator, generation::vegetation::GroundCover, props::PropKind};
use terra_core::{WorldConfig, WorldSeed};
use terra_voxel::{MaterialId, SectionPos};

#[test]
fn cherry_regions_are_seeded_continuous_and_normalized_in_negative_coordinates() {
    for seed in [42042, 7, 99173] {
        let config = WorldConfig {
            seed: WorldSeed(seed),
            ..Default::default()
        };
        let a = TerraGenerator::new(config.clone()).unwrap();
        let b = TerraGenerator::new(config).unwrap();
        let mut maximum = 0.0_f64;
        let mut negative = false;
        for z in (-4096..=4096).step_by(128) {
            for x in (-4096..=4096).step_by(128) {
                let p = a.climate_biome_at(x as f64, z as f64);
                assert_eq!(p, b.climate_biome_at(x as f64, z as f64));
                assert!((p.weights.values().iter().sum::<f64>() - 1.0).abs() < 1e-10);
                maximum = maximum.max(p.weights.cherry_forest);
                negative |= p.weights.cherry_forest > 0.7 && (x < 0 || z < 0);
                if p.weights.cherry_forest > 0.7 {
                    let near = a.climate_biome_at(x as f64 - 0.001, z as f64 + 0.001);
                    assert!((p.weights.cherry_forest - near.weights.cherry_forest).abs() < 0.0001);
                }
            }
        }
        assert!(maximum > 0.8 && negative, "seed {seed} maximum {maximum}");
    }
}

#[test]
fn visible_ocean_requires_actual_marine_water_not_a_climate_weight_on_dry_land() {
    let terrain = TerraGenerator::new(Default::default()).unwrap();
    let mut column = terrain.surface_column(0.0, 0.0);
    column.biome.weights = BiomeWeights {
        ocean: 0.8,
        forest: 0.2,
        plains: 0.0,
        desert: 0.0,
        highland: 0.0,
        mushroom_park: 0.0,
        cliffted_desert: 0.0,
        cherry_forest: 0.0,
    };
    column.height = 40.0;
    column.river_cavity = None;
    column.water_level = None;
    assert_eq!(terrain.surface_biome(&column), BiomeKind::Forest);
    column.water_level = Some(terrain.config().sea_level);
    column.height = 2.0;
    assert_eq!(terrain.surface_biome(&column), BiomeKind::Ocean);
    column.height = 40.0;
    column.river_cavity = Some([2.0, 6.0]);
    assert_eq!(
        terrain.surface_biome(&column),
        BiomeKind::Forest,
        "a buried river under grass is not a surface ocean"
    );
    column.river_cavity = None;
    column.water_level = Some(24.0);
    column.height = 20.0;
    assert_eq!(
        terrain.surface_biome(&column),
        BiomeKind::Forest,
        "an inland lake is not ocean"
    );
}

#[test]
fn cherry_flowers_are_only_pink_or_white_and_survive_the_item_roundtrip() {
    let terrain = TerraGenerator::new(Default::default()).unwrap();
    let mut column = terrain.surface_column(0.0, 0.0);
    column.biome.weights.cherry_forest = 1.0;
    column.biome.weights.ocean = 0.0;
    column.biome.weights.desert = 0.0;
    column.biome.weights.cliffted_desert = 0.0;
    column.water_level = None;
    let mut seen = [0; 2];
    for z in -16..16 {
        for x in -16..16 {
            let props = terrain.ground_props(
                [x, 100, z],
                SectionPos::new(x.div_euclid(16), 6, z.div_euclid(16)),
                MaterialId::Grass as u16,
                GroundCover {
                    grass: 1.0,
                    bushes: 0.0,
                    flowers: 1.0,
                    ferns: 0.0,
                    path_clearance: 1.0,
                },
                column,
            );
            for prop in props
                .into_iter()
                .flatten()
                .filter(|p| p.kind == PropKind::Flower)
            {
                assert!(matches!(prop.variant, 2 | 4));
                seen[usize::from(prop.variant == 4)] += 1;
                let color = terra_core::crusher::flower_color(prop.variant);
                assert!(terra_core::ItemKind::Flower(color).valid());
                assert_eq!(terra_core::crusher::flower_variant(color), prop.variant);
            }
        }
    }
    assert!(seen.iter().all(|&n| n > 8), "{seen:?}");
}
