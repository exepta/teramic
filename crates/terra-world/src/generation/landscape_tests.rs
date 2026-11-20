use super::landforms::terrace;
use super::*;
use terra_core::WorldConfig;
use terra_voxel::{MaterialId, SectionPos};

#[test]
fn revision_one_heights_match_the_pre_f40_executable_snapshot() {
    let terrain = TerraGenerator::new(WorldConfig {
        landscape_revision: 1,
        blood_forest_revision: 0,
        rivers: false,
        ..Default::default()
    })
    .unwrap();
    // Captured from the unchanged terrain_regions executable before F40 on
    // 2026-10-26. Its formatter prints nine decimal places; do not regenerate
    // this contract from the new implementation.
    for ([x, z], expected) in [
        ([0.0, 0.0], 98.791733705),
        ([160.0, -96.0], 109.377985624),
        ([512.0, -512.0], 114.740061717),
        ([-256.0, 256.0], 110.090154128),
        ([1024.0, 768.0], 104.354917520),
    ] {
        let actual = terrain.surface_height(x, z);
        assert!(
            (actual - expected).abs() < 1e-9,
            "legacy revision1 changed at {x},{z}: {actual:.12} != {expected:.12}"
        );
    }
}

#[test]
fn terraces_have_shelves_and_cliffs_without_density_jumps() {
    for level in -12..=12 {
        let h = f64::from(level) * 4.0;
        assert!((terrace(h - 1e-7, 4.0, 0.0) - terrace(h + 1e-7, 4.0, 0.0)).abs() < 1e-5);
        assert_eq!(terrace(h + 0.4, 4.0, 0.0), terrace(h + 1.2, 4.0, 0.0));
        assert!(terrace(h + 2.6, 4.0, 0.0) - terrace(h + 1.4, 4.0, 0.0) > 3.0);
    }
    // This fixture records revision one's deliberately frequent small terraces.
    // Revision two also has intentionally smooth macro-regions (tested below).
    let a = TerraGenerator::new(WorldConfig {
        landscape_revision: 1,
        ..Default::default()
    })
    .unwrap();
    let b = TerraGenerator::new(WorldConfig {
        terrace_strength: 0.0,
        landscape_revision: 1,
        ..Default::default()
    })
    .unwrap();
    let mut changed = 0;
    for z in (-128..=128).step_by(8) {
        for x in (-128..=128).step_by(8) {
            let delta = a.surface_height(f64::from(x), f64::from(z))
                - b.surface_height(f64::from(x), f64::from(z));
            assert!(delta.abs() <= 6.0);
            changed += usize::from(delta.abs() > 0.5);
        }
    }
    assert!(changed > 200);
}

#[test]
fn revision_two_terraces_shape_plateaus_but_leave_plains_relatively_smooth() {
    let a = TerraGenerator::new(WorldConfig {
        landscape_revision: 2,
        water: false,
        rivers: false,
        biomes: false,
        caves: false,
        trees: false,
        ..Default::default()
    })
    .unwrap();
    let b = TerraGenerator::new(WorldConfig {
        terrace_strength: 0.0,
        ..a.config().clone()
    })
    .unwrap();
    let mut plateaus = 0;
    let mut changed = 0;
    let mut plains = 0;
    for z in (-2048..=2048).step_by(64) {
        for x in (-2048..=2048).step_by(64) {
            let [x, z] = [f64::from(x) + 0.5, f64::from(z) + 0.5];
            let region = a.landscape_region(x, z);
            let delta = (a.surface_height(x, z) - b.surface_height(x, z)).abs();
            assert!(
                delta <= 6.0,
                "terracing must not create unbounded elevation changes"
            );
            if region.weights[2] > 0.99 {
                plateaus += 1;
                changed += usize::from(delta > 0.5);
            }
            if region.weights[0] > 0.99 {
                plains += 1;
                assert!(
                    delta < 0.5,
                    "plains should not inherit strong plateau terraces"
                );
            }
        }
    }
    assert!(
        plateaus > 20 && changed > 10 && plains > 20,
        "plateaus={plateaus} changed={changed} plains={plains}"
    );
}

#[test]
fn surface_strata_keep_grass_and_dirt_on_steps_and_bury_stone_below_soil() {
    for landscape_revision in [0, 1, 2, 3] {
        let terrain = TerraGenerator::new(WorldConfig {
            landscape_revision,
            rivers: false,
            caves: false,
            trees: false,
            biomes: false,
            ..Default::default()
        })
        .unwrap();
        let mut grass = 0;
        let mut green_slopes = 0;
        let mut heights = [f64::INFINITY, f64::NEG_INFINITY];
        for z in (-384..=384).step_by(4) {
            for x in (-384..=384).step_by(4) {
                let column = terrain.surface_column(f64::from(x) + 0.5, f64::from(z) + 0.5);
                heights[0] = heights[0].min(column.height);
                heights[1] = heights[1].max(column.height);
                let p = [f64::from(x) + 0.5, column.height - 0.5, f64::from(z) + 0.5];
                let sample = terrain.sample_with_column(p, column);
                if column.slope > 1.05 && column.surface_material == MaterialId::Grass {
                    assert_eq!(sample.material, MaterialId::Grass as u16);
                    green_slopes += 1;
                }
                if column.surface_material == MaterialId::Grass {
                    assert_eq!(sample.material, MaterialId::Grass as u16);
                    for depth in [2.0, 3.0, 4.0] {
                        assert_eq!(
                            terrain
                                .sample_with_column([p[0], column.height - depth, p[2]], column)
                                .material,
                            MaterialId::Dirt as u16
                        );
                    }
                    grass += 1;
                }
                assert_eq!(
                    terrain
                        .sample_with_column([p[0], column.height - 12.0, p[2]], column)
                        .material,
                    MaterialId::Stone as u16
                );
            }
        }
        assert!(
            grass > 100 && green_slopes > 100,
            "grass={grass}, slopes={green_slopes}"
        );
        assert!(heights[1] - heights[0] > 30.0, "{heights:?}");
    }
}

#[test]
fn forest_groups_and_clearings_have_continuous_cover() {
    let terrain = TerraGenerator::new(WorldConfig::default()).unwrap();
    let mut sparse = 0;
    let mut dense = 0;
    for z in (-2048..=2048).step_by(32) {
        for x in (-2048..=2048).step_by(32) {
            let cover = terrain.forest_probability(f64::from(x), f64::from(z));
            assert!((0.0..=1.0).contains(&cover));
            sparse += usize::from(cover < 0.04);
            dense += usize::from(cover > 0.6);
            assert!(
                (cover - terrain.forest_probability(f64::from(x) + 0.001, f64::from(z))).abs()
                    < 0.001
            );
        }
    }
    assert!(
        sparse > 100 && dense > 100,
        "sparse={sparse}, dense={dense}"
    );
}

#[test]
fn landscape_sections_match_global_samples_at_positive_and_negative_edges() {
    let terrain = TerraGenerator::new(WorldConfig {
        trees: false,
        ..Default::default()
    })
    .unwrap();
    let generator = SectionGenerator::new(terrain);
    for [x, z] in [[-1_i64, -1], [0, 0], [-33, 48], [512, -512], [-1024, 768]] {
        let h = generator
            .terrain()
            .surface_height(x as f64 + 0.5, z as f64 + 0.5);
        let position = SectionPos::new(
            x.div_euclid(16),
            ((h - 0.5).floor() as i64).div_euclid(16),
            z.div_euclid(16),
        );
        let section = generator
            .build(position, 0, &Default::default(), true)
            .unwrap()
            .data;
        let origin = position.origin().unwrap();
        for ly in [0, 7, 15] {
            for lz in [0, 15] {
                for lx in [0, 15] {
                    let p = [
                        origin[0] + lx as i64,
                        origin[1] + ly as i64,
                        origin[2] + lz as i64,
                    ];
                    assert_eq!(
                        section.block([lx, ly, lz]).unwrap(),
                        generator.block_at(p).unwrap()
                    );
                }
            }
        }
        assert_eq!(section, generator.generate(position).unwrap());
    }
}

#[test]
fn uniform_surface_coverage_never_dithers_materials_per_block() {
    let terrain = TerraGenerator::new(WorldConfig {
        base_height: 9.5,
        sea_level: 8.0,
        landscape_revision: 3,
        hill_amplitude: 0.0,
        mountain_amplitude: 0.0,
        trees: false,
        caves: false,
        ..Default::default()
    })
    .unwrap();
    let mut sand = 0;
    let mut grass = 0;
    for z in -16..16 {
        for x in -16..16 {
            let column = terrain.surface_column(f64::from(x) + 0.5, f64::from(z) + 0.5);
            assert_eq!(column.shore_cover, 0.5);
            assert_eq!(column.snow_cover, 0.0);
            sand += usize::from(column.surface_material == MaterialId::Sand);
            grass += usize::from(column.surface_material == MaterialId::Grass);
        }
    }
    assert_eq!(
        (sand, grass),
        (1024, 0),
        "A flat shore must not be a checkerboard"
    );
    let snowy = TerraGenerator::new(WorldConfig {
        base_height: terrain.config().snow_height
            + (terrain.biome_at(0.0, 0.0).temperature - 0.5) * 16.0,
        ..terrain.config().clone()
    })
    .unwrap();
    let mut snow = 0;
    let mut green = 0;
    for z in -32..32 {
        for x in -32..32 {
            let column = snowy.surface_column(f64::from(x) + 0.5, f64::from(z) + 0.5);
            assert_eq!(column.shore_cover, 0.0);
            snow += usize::from(column.surface_material == MaterialId::Snow);
            green += usize::from(column.surface_material == MaterialId::Grass);
            let next = snowy.surface_column(f64::from(x) + 0.501, f64::from(z) + 0.5);
            assert!((next.snow_cover - column.snow_cover).abs() < 0.001);
        }
    }
    assert_eq!(
        (snow, green),
        (4096, 0),
        "A flat snow contour must stay coherent"
    );
}
