use super::sections::BlockOverrides;
use super::*;
use terra_core::WorldConfig;
use terra_voxel::SectionPos;

#[test]
fn surface_grid_matches_individual_columns_at_every_cell_including_the_mesh_halo() {
    for config in [
        WorldConfig::default(),
        WorldConfig {
            landscape_revision: 5,
            ecology_revision: 15,
            ..Default::default()
        },
    ] {
        let terrain = TerraGenerator::new(config).unwrap();
        for (start, size) in [([-17, -1], 18), ([113, -161], 16), ([1023, 1023], 18)] {
            let grid = terrain.surface_column_grid(start, size);
            for (i, actual) in grid.iter().enumerate() {
                let x = (start[0] + (i % size) as i64) as f64 + 0.5;
                let z = (start[1] + (i / size) as i64) as f64 + 0.5;
                assert_eq!(*actual, terrain.surface_column(x, z), "({x}, {z})");
            }
        }
    }
}

#[test]
fn prepared_columns_survive_runtime_rule_changes_for_new_sections_and_edits() {
    let original = TerraGenerator::new(WorldConfig::default()).unwrap();
    let generator = SectionGenerator::new(original.clone());
    for coordinate in [[8, 37], [-9, -38]] {
        let prepared = generator.prepare_column(coordinate, true).unwrap();
        let mut changed = original.clone();
        assert!(std::ptr::eq(prepared.config.as_ref(), changed.config()));
        let mut rules = terra_core::WorldRuleSettings::from_world(changed.config());
        rules.game_rules.game_time_speed = 2.0;
        rules.game_rules.sleep_percentage = 50.0;
        rules.seasons.spring_days = 2;
        rules.time.start_hour = 21.0;
        changed.set_world_rules(rules);
        assert_ne!(
            changed.config().time.start_hour,
            original.config().time.start_hour
        );
        assert_eq!(
            prepared.config.time.start_hour,
            original.config().time.start_hour
        );
        let updated = SectionGenerator::new(changed);
        for y in [-2, 4] {
            let p = SectionPos::new(coordinate[0], y, coordinate[1]);
            let edit = [p.x * 16 + 15, p.y * 16 + 15, p.z * 16 + 15];
            for edits in [BlockOverrides::new(), [(edit, 11)].into()] {
                let expected = generator.build_prepared(p, 1, &edits, &prepared).unwrap();
                let actual = updated.build_prepared(p, 1, &edits, &prepared).unwrap();
                assert_eq!(actual.data, expected.data);
                assert_eq!(
                    actual.mesh.unwrap().positions,
                    expected.mesh.unwrap().positions
                );
                assert_eq!(actual.water, expected.water);
                assert_eq!(actual.props, expected.props);
            }
        }
    }
}

#[test]
fn prepared_columns_reject_changed_terrain_or_world_identity() {
    let config = WorldConfig::default();
    let generator = SectionGenerator::new(TerraGenerator::new(config.clone()).unwrap());
    let prepared = generator.prepare_column([-1, -1], true).unwrap();
    for changed in [
        WorldConfig {
            seed: terra_core::WorldSeed(99),
            ..config.clone()
        },
        WorldConfig {
            id: terra_core::WorldId(99),
            ..config.clone()
        },
        WorldConfig {
            sea_level: 10.0,
            ..config.clone()
        },
        WorldConfig {
            tree_density: 0.5,
            ..config
        },
    ] {
        let other = SectionGenerator::new(TerraGenerator::new(changed).unwrap());
        assert!(matches!(
            other.build_prepared(
                SectionPos::new(-1, 0, -1),
                0,
                &Default::default(),
                &prepared
            ),
            Err(SectionGenerationError::ColumnMismatch)
        ));
    }
}

#[test]
fn prepared_columns_reject_every_other_landscape_revision() {
    for revision in 0..=5 {
        let config = WorldConfig {
            landscape_revision: revision,
            ..Default::default()
        };
        let generator = SectionGenerator::new(TerraGenerator::new(config.clone()).unwrap());
        let prepared = generator.prepare_column([-1, -1], true).unwrap();
        for changed in (0..=5).filter(|other| *other != revision) {
            let other = SectionGenerator::new(
                TerraGenerator::new(WorldConfig {
                    landscape_revision: changed,
                    ..config.clone()
                })
                .unwrap(),
            );
            assert!(matches!(
                other.build_prepared(
                    SectionPos::new(-1, 0, -1),
                    0,
                    &Default::default(),
                    &prepared
                ),
                Err(SectionGenerationError::ColumnMismatch)
            ));
        }
    }
}

#[test]
fn prepared_columns_match_uncached_generation_and_never_cache_edits() {
    let generator = SectionGenerator::new(TerraGenerator::new(WorldConfig::default()).unwrap());
    let prepared = generator.prepare_column([-1, -1], true).unwrap();
    for y in -4..5 {
        let p = SectionPos::new(-1, y, -1);
        let plain = generator.build(p, 0, &Default::default(), true).unwrap();
        let cached = generator
            .build_prepared(p, 0, &Default::default(), &prepared)
            .unwrap();
        assert_eq!(plain.data, cached.data);
        assert_eq!(
            plain.mesh.unwrap().positions,
            cached.mesh.unwrap().positions
        );
        assert_eq!(plain.water, cached.water);
    }
    let p = SectionPos::new(-1, 3, -1);
    let edits = [([-1, 63, -1], terra_voxel::MaterialId::SmoothStone as u16)].into();
    assert_eq!(
        generator
            .build_prepared(p, 1, &edits, &prepared)
            .unwrap()
            .data
            .block([15; 3])
            .unwrap(),
        terra_voxel::MaterialId::SmoothStone as u16
    );
    assert_ne!(
        generator
            .build_prepared(p, 0, &Default::default(), &prepared)
            .unwrap()
            .data
            .block([15; 3])
            .unwrap(),
        terra_voxel::MaterialId::SmoothStone as u16
    );
    assert!(matches!(
        generator.build_prepared(SectionPos::default(), 0, &Default::default(), &prepared),
        Err(SectionGenerationError::ColumnMismatch)
    ));
}

#[test]
fn ocean_desert_and_forest_profiles_generate_distinct_wide_regions() {
    let terrain = TerraGenerator::new(WorldConfig {
        landscape_revision: 4,
        blood_forest_revision: 0,
        sea_level: 22.0,
        rivers: true,
        biome_definitions: Some(terra_core::BiomeDefinitions::default()),
        ..Default::default()
    })
    .unwrap();
    let ocean = [-3328.0, -8192.0];
    let mut wet = 0;
    for z in (-256..=256).step_by(16) {
        for x in (-256..=256).step_by(16) {
            wet += usize::from(
                terrain
                    .water_column(ocean[0] + f64::from(x), ocean[1] + f64::from(z))
                    .is_some(),
            );
        }
    }
    assert!(
        wet > 1000,
        "Ocean must be wider than the old local ponds: {wet}"
    );
    let desert = terrain.surface_column(2304.0, -8192.0);
    assert!(desert.biome.aridity() > 0.99);
    assert_eq!(desert.surface_material, terra_voxel::MaterialId::Sand);
    assert_eq!(terrain.forest_probability(2304.0, -8192.0), 0.0);
    let forest = terrain.biome_at(-6784.0, -8192.0);
    assert!(forest.weights.forest > 0.99);
    let mut vacant = 0;
    let mut dense = 0;
    for z in (-128..128).step_by(4) {
        for x in (-128..128).step_by(4) {
            let c = terrain.ground_cover(-6784.0 + f64::from(x), -8192.0 + f64::from(z));
            vacant += usize::from(c.path_clearance == 0.0 || c.grass < 0.02);
            dense += usize::from(c.grass > 0.5);
        }
    }
    assert!(vacant > 100 && dense > 100);
}
