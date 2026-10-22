use super::*;
use terra_core::WorldConfig;
use terra_voxel::MaterialId;

#[test]
fn fractional_surface_heights_have_exactly_one_grass_voxel_then_at_least_three_dirt_voxels() {
    let terrain = TerraGenerator::new(WorldConfig {
        biomes: false,
        caves: false,
        trees: false,
        ..Default::default()
    })
    .unwrap();
    let base = terrain.surface_column(-0.5, -0.5);
    for height in [-16.9_f64, -16.5, -16.1, 20.1, 20.49, 20.5, 20.9, 21.0] {
        let column = SurfaceColumn {
            height,
            surface_material: MaterialId::Grass,
            soil_depth: 4.5,
            clay_depth: 0.0,
            sandstone_depth: 0.0,
            moss_entrance: false,
            moss_daylight_floor: f64::INFINITY,
            ..base
        };
        let top = (height - 0.5).floor() + 0.5;
        for layer in 0..=3 {
            let y = top - f64::from(layer);
            let expected = if layer == 0 {
                MaterialId::Grass
            } else {
                MaterialId::Dirt
            };
            assert_eq!(
                terrain.column_material(column, height - y, y - height),
                expected,
                "height={height}, layer={layer}"
            );
        }
    }
}

#[test]
fn grassy_oasis_and_desert_transition_surfaces_keep_dirt_not_sandstone_underneath() {
    let terrain = TerraGenerator::new(WorldConfig::default()).unwrap();
    let base = terrain.surface_column(2304.5, -8191.5);
    assert!(base.sandstone_depth > 4.0);
    let column = SurfaceColumn {
        surface_material: MaterialId::Grass,
        soil_depth: 5.0,
        ..base
    };
    assert_eq!(
        terrain.column_material(column, 0.5, -0.5),
        MaterialId::Grass
    );
    for depth in [2.0, 3.0, 4.0] {
        assert_eq!(
            terrain.column_material(column, depth, -depth),
            MaterialId::Dirt
        );
    }
}

#[test]
fn sandstone_is_stratified_under_desert_sand_and_smooth_stone_is_never_generated() {
    let terrain = TerraGenerator::new(WorldConfig {
        caves: false,
        trees: false,
        ..Default::default()
    })
    .unwrap();
    let mut sandstone = 0;
    let mut sand = 0;
    for z in -12..=12 {
        for x in -12..=12 {
            let x = 2304.5 + f64::from(x);
            let z = -8191.5 + f64::from(z);
            let column = terrain.surface_column(x, z);
            assert!(column.biome.aridity() > 0.8);
            for depth in 0..32 {
                let material = terrain
                    .sample_with_column([x, column.height - f64::from(depth) - 0.5, z], column)
                    .material;
                sandstone += usize::from(material == MaterialId::Sandstone as u16);
                sand += usize::from(material == MaterialId::Sand as u16);
                assert_ne!(material, MaterialId::SmoothStone as u16);
                if f64::from(depth) >= column.sandstone_depth.max(column.soil_depth).ceil() {
                    assert_eq!(material, MaterialId::Stone as u16);
                }
            }
        }
    }
    assert!(sand > 100 && sandstone > 100);
}

#[test]
fn submerged_clay_forms_bounded_deterministic_patches_among_sand() {
    let terrain = TerraGenerator::new(WorldConfig {
        trees: false,
        caves: false,
        ..Default::default()
    })
    .unwrap();
    let mut clay = 0;
    let mut sand = 0;
    for z in -24..=24 {
        for x in -24..=24 {
            let x = -3327.5 + f64::from(x);
            let z = -8191.5 + f64::from(z);
            let column = terrain.surface_column(x, z);
            let y = column.water_bed() - 0.5;
            let sample = terrain.sample_with_column([x, y, z], column);
            assert_eq!(sample, terrain.sample([x, y, z]));
            clay += usize::from(sample.material == MaterialId::Clay as u16);
            sand += usize::from(sample.material == MaterialId::Sand as u16);
            if sample.material == MaterialId::Clay as u16 {
                assert!(column.water_level.is_some());
                assert!(column.clay_depth > 0.0 && column.clay_depth < 4.0);
                assert_ne!(
                    terrain.sample_with_column([x, y - 4.0, z], column).material,
                    MaterialId::Clay as u16
                );
            }
        }
    }
    assert!(clay > 10 && sand > 100, "clay={clay}, sand={sand}");
}

#[test]
fn moss_is_sparse_and_confined_to_damp_daylight_cave_walls() {
    let terrain = TerraGenerator::new(WorldConfig {
        cave_structure_revision: 2,
        trees: false,
        biomes: false,
        rivers: true,
        ..Default::default()
    })
    .unwrap();
    let mut moss = 0;
    let mut stone = 0;
    for [cx, cz] in [[-481.5, -640.5], [551.5, -1112.5], [55.5, -1081.5]] {
        for z in -16..=16 {
            for x in -16..=16 {
                let x = cx + f64::from(x);
                let z = cz + f64::from(z);
                let column = terrain.surface_column(x, z);
                for depth in 0..48 {
                    let y = (column.height - 0.5).floor() + 0.5 - f64::from(depth);
                    let sample = terrain.sample_with_column([x, y, z], column);
                    stone += usize::from(sample.material == MaterialId::Stone as u16);
                    if sample.material == MaterialId::MossyStone as u16 {
                        moss += 1;
                        assert!(sample.density <= 0.0 && sample.density >= -1.25);
                        assert!(column.biome.humidity > 0.45);
                        assert!(column.moss_entrance || y >= column.moss_daylight_floor);
                    }
                    assert!(!matches!(sample.material, 12 | 14..=18));
                }
            }
        }
    }
    assert!(moss > 0 && moss * 20 < stone, "moss={moss}, stone={stone}");
}
