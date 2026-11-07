use super::*;
use terra_core::WorldConfig;

#[test]
fn main_terrain_defaults_ignore_later_profiles_at_negative_boundaries_and_on_reload() {
    for seed in [7, 42042, 99173] {
        let config = WorldConfig {
            seed: terra_core::WorldSeed(seed),
            ..Default::default()
        };
        assert_eq!(config.landscape_revision, 1);
        assert_eq!(config.sea_level, 8.0);
        assert!(!config.rivers);
        assert!(config.biome_definitions.is_none());
        let reference = TerraGenerator::new(config.clone()).unwrap();
        let mut profiles = terra_core::BiomeDefinitions::default();
        for definition in &mut profiles.entries {
            definition.selection_weight = 1.0;
            definition.terrain.min_height += 100.0;
            definition.terrain.max_height += 100.0;
            definition.terrain.tree_density = 1.0;
            definition.terrain.soil_depth = 31.0;
        }
        profiles.landscapes.warp_strength += 64.0;
        profiles.validate().unwrap();
        let with_unused_profiles = TerraGenerator::new(WorldConfig {
            biome_definitions: Some(profiles),
            ..config.clone()
        })
        .unwrap();
        let restored = TerraGenerator::new(config).unwrap();
        let points = [
            [-4096.5, 768.5],
            [-16.001, -16.001],
            [-16.0, -16.0],
            [-15.999, -15.999],
            [-0.5, -0.5],
            [0.5, 0.5],
            [4096.5, -768.5],
        ];
        for [x, z] in points.into_iter().rev() {
            let expected = reference.surface_column(x, z);
            assert_eq!(with_unused_profiles.surface_column(x, z), expected);
            assert_eq!(restored.surface_column(x, z), expected);
            assert_eq!(
                with_unused_profiles.ground_cover(x, z),
                reference.ground_cover(x, z)
            );
            assert_eq!(
                with_unused_profiles.forest_probability(x, z).to_bits(),
                reference.forest_probability(x, z).to_bits()
            );
            for y in [
                -48.5,
                -16.5,
                -0.5,
                15.5,
                expected.height - 0.5,
                expected.height + 0.5,
            ] {
                let position = [x, y, z];
                assert_eq!(
                    with_unused_profiles.sample_with_column(position, expected),
                    reference.sample_with_column(position, expected)
                );
                assert_eq!(
                    restored.sample_with_column(position, expected),
                    reference.sample_with_column(position, expected)
                );
            }
        }
    }
}

#[test]
fn version_five_reference_regions_match_the_seeded_landscape() {
    let terrain = TerraGenerator::new(WorldConfig {
        landscape_revision: 0,
        ..Default::default()
    })
    .unwrap();
    assert_eq!(
        terrain.version(),
        5,
        "update reference fixtures when changing generator versions"
    );
    // Genuine highland ranges now rise above Y80; ecology follows that altitude
    // instead of labelling the old low foothills as Highland.
    for ([x, z], height, park_influence) in [
        ([0.0, 0.0], 107.676959142, true),
        ([512.0, -512.0], 112.549834159, true),
        ([-256.0, 256.0], 121.332939037, true),
        ([1024.0, 768.0], 118.130066651, false),
    ] {
        assert_eq!(
            terrain.biome_at(x, z).weights.mushroom_park > 0.0,
            park_influence
        );
        assert!(
            (terrain.surface_height(x, z) - height).abs() < 1e-7,
            "{x}, {z}"
        );
    }
}
