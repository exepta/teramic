use super::*;
use terra_core::{WorldConfig, WorldSeed};

fn climate(terrain: &TerraGenerator, temperature: f64, humidity: f64, ocean: f64) -> BiomeSample {
    let mut biome = terrain.climate_biome_at(0.5, 0.5);
    biome.temperature = temperature;
    biome.humidity = humidity;
    biome.weights = super::super::biomes::BiomeWeights {
        ocean,
        forest: 1.0 - ocean,
        plains: 0.0,
        desert: 0.0,
        highland: 0.0,
        mushroom_park: 0.0,
        cliffted_desert: 0.0,
        cherry_forest: 0.0,
        spirit_blossom: 0.0,
        blood_forest: 0.0,
        ashlands: 0.0,
        savanna: 0.0,
        swamp: 0.0,
        petrified_forest: 0.0,
        crystal_highlands: 0.0,
        misty_coast: 0.0,
    };
    biome
}

fn legacy_reference(terrain: &TerraGenerator, x: f64, z: f64, biome: BiomeSample) -> OceanHabitat {
    let sea = smoothstep(0.28, 0.72, biome.weights.ocean);
    let district = terrain.hills.get([x / 240.0 + 84.3, z / 240.0 - 61.7]);
    let cold = 1.0 - smoothstep(0.23, 0.39, biome.temperature);
    let reef = (1.0 - cold)
        * smoothstep(0.42, 0.65, biome.temperature)
        * smoothstep(-0.28, 0.18, district);
    let colony =
        (0.5 + terrain.valleys.get([x / 21.0 + 37.9, z / 21.0 - 104.1]) * 0.75).clamp(0.0, 1.0);
    let channel = terrain
        .valleys
        .get([x / 73.0 - 51.4, z / 73.0 + 29.3])
        .abs();
    OceanHabitat {
        weights: [reef, (1.0 - cold - reef).max(0.0), cold].map(|weight| (weight * sea) as f32),
        colony: colony as f32,
        cover: (smoothstep(0.035, 0.13, channel) * smoothstep(0.20, 0.65, colony)) as f32,
        rock: (smoothstep(
            -0.12,
            0.38,
            terrain.hills.get([x / 33.0 - 14.7, z / 33.0 + 71.3]),
        ) * smoothstep(0.025, 0.10, channel)) as f32,
        ..Default::default()
    }
}

#[test]
fn revisions_zero_through_three_keep_exact_legacy_habitats_and_relief() {
    for revision in 0..=3 {
        let terrain = TerraGenerator::new(WorldConfig {
            landscape_revision: revision,
            seed: WorldSeed(71),
            ..Default::default()
        })
        .unwrap();
        for (temperature, humidity, ocean) in
            [(0.12, 0.90, 1.0), (0.41, 0.95, 0.55), (0.85, 0.85, 1.0)]
        {
            let biome = climate(&terrain, temperature, humidity, ocean);
            for [x, z] in [
                [-4096.5, 15.999],
                [-16.001, -0.5],
                [0.5, 16.001],
                [345.5, -212.5],
            ] {
                let expected = legacy_reference(&terrain, x, z, biome);
                assert_eq!(terrain.ocean_with_biome(x, z, biome), expected);
                for depth in [3.5, 5.0, 9.0, 11.0, 18.0] {
                    let height = terrain.config().sea_level - depth;
                    let shelf = f64::from(expected.rock)
                        * f64::from(expected.weights[0])
                        * smoothstep(3.0, 10.0, depth);
                    assert_eq!(
                        terrain.ocean_relief(x, z, biome, height),
                        height + shelf * (depth - 2.5).clamp(0.0, 10.0)
                    );
                }
                assert_eq!(expected.swamp, 0.0);
                assert_eq!(expected.clay_depth, 0.0);
                assert!(expected.substrate.is_none());
            }
        }
    }
}

#[test]
fn modern_warm_cold_and_swamp_floors_are_coherent_and_include_connected_clay() {
    let terrain = TerraGenerator::new(WorldConfig {
        landscape_revision: 4,
        sea_level: 22.0,
        biome_definitions: Some(terra_core::BiomeDefinitions::default()),
        seed: WorldSeed(42042),
        ..Default::default()
    })
    .unwrap();
    for (temperature, humidity, sea, expected) in [
        (0.9, 0.25, 1.0, OceanArea::CoralReef),
        (0.1, 0.80, 1.0, OceanArea::ColdOcean),
        (0.9, 0.95, 0.6, OceanArea::Swamp),
    ] {
        let biome = climate(&terrain, temperature, humidity, sea);
        let mut floors = [0usize; 5];
        let mut clay = 0;
        let mut connected_clay = 0;
        let mut transitions = 0;
        let mut dominant = 0;
        for z in -64..64 {
            for x in -64..64 {
                let (x, z) = (f64::from(x) + 0.5, f64::from(z) + 0.5);
                let habitat = terrain.ocean_with_biome(x, z, biome);
                let next = terrain.ocean_with_biome(x + 1.0, z, biome);
                let floor = habitat.floor_material().unwrap();
                floors[match floor {
                    MaterialId::Sand => 0,
                    MaterialId::Gravel => 1,
                    MaterialId::Mud => 2,
                    MaterialId::Dirt => 3,
                    _ => 4,
                }] += 1;
                transitions += usize::from(next.floor_material() != Some(floor));
                dominant += usize::from(habitat.area() == Some(expected));
                if habitat.clay_depth > 0.0 {
                    clay += 1;
                    connected_clay += usize::from(
                        next.clay_depth > 0.0
                            || terrain.ocean_with_biome(x, z + 1.0, biome).clay_depth > 0.0,
                    );
                }
                assert_eq!(habitat, terrain.ocean_with_biome(x, z, biome));
                assert!(habitat.max_vegetation_depth > 40.0);
                assert_eq!(habitat.ice_depth, 0);
            }
        }
        assert!(dominant > 100, "{expected:?}: dominant={dominant}");
        assert!(clay > 25 && clay < 8192, "{expected:?}: clay={clay}");
        assert!(
            connected_clay * 10 >= clay * 8,
            "clay deposits must be connected, not voxel speckles"
        );
        assert!(
            transitions < 4096,
            "substrate changed at {transitions} of 16384 neighboring columns"
        );
        match expected {
            OceanArea::CoralReef => assert!(floors[0] > floors[1] + floors[2] + floors[3]),
            OceanArea::ColdOcean => {
                assert!(floors[1] + floors[2] > floors[0] + floors[3] + floors[4])
            }
            OceanArea::Swamp => assert!(floors[2] + floors[3] > floors[0] + floors[1]),
            OceanArea::SeagrassForest => unreachable!(),
        }
    }
}

#[test]
fn modern_ocean_blends_are_continuous_at_negative_section_edges() {
    let terrain = TerraGenerator::new(WorldConfig {
        landscape_revision: 4,
        sea_level: 22.0,
        biome_definitions: Some(terra_core::BiomeDefinitions::default()),
        ..Default::default()
    })
    .unwrap();
    let biome = climate(&terrain, 0.57, 0.73, 0.60);
    for [x, z] in [[-4096.0, -16.0], [-16.0, 0.0], [0.0, -16.0], [16.0, 4096.0]] {
        let a = terrain.ocean_with_biome(x - 0.0001, z - 0.0001, biome);
        let b = terrain.ocean_with_biome(x + 0.0001, z + 0.0001, biome);
        for (a, b) in a
            .weights
            .into_iter()
            .chain([a.cover, a.rock, a.colony, a.swamp, a.clay_depth])
            .zip(
                b.weights
                    .into_iter()
                    .chain([b.cover, b.rock, b.colony, b.swamp, b.clay_depth]),
            )
        {
            assert!((a - b).abs() < 0.002);
        }
    }
}

#[test]
fn clay_overrides_modern_marine_floor_only_for_the_deposit_thickness() {
    let terrain = TerraGenerator::new(WorldConfig {
        landscape_revision: 4,
        sea_level: 22.0,
        biome_definitions: Some(terra_core::BiomeDefinitions::default()),
        ..Default::default()
    })
    .unwrap();
    let mut column = terrain.surface_column(0.5, 0.5);
    column.height = -40.0;
    column.water_level = Some(terrain.config().sea_level);
    column.river_cavity = None;
    column.ocean = terrain.ocean_with_biome(0.5, 0.5, climate(&terrain, 0.1, 0.8, 1.0));
    column.clay_depth = 1.25;
    assert_eq!(terrain.column_material(column, 0.5, -0.5), MaterialId::Clay);
    assert_eq!(terrain.column_material(column, 1.5, -1.5), MaterialId::Clay);
    assert_ne!(terrain.column_material(column, 2.5, -2.5), MaterialId::Clay);
    assert_eq!(terrain.column_material(column, 0.5, 0.5), MaterialId::Air);
    column.clay_depth = 0.0;
    assert_eq!(
        terrain.column_material(column, 0.5, -0.5),
        column.ocean.floor_material().unwrap()
    );
}
