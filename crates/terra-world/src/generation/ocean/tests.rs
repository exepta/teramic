use super::*;
use crate::generation::{SectionGenerator, TerrainGenerator};
use terra_core::WorldConfig;
use terra_voxel::{SectionPos, global_to_section};

#[test]
fn habitats_are_continuous_and_have_coherent_clearings_on_both_sides_of_zero() {
    let terrain = TerraGenerator::new(WorldConfig {
        landscape_revision: 3,
        ..Default::default()
    })
    .unwrap();
    let mut biome = terrain.biome_at(0.0, 0.0);
    biome.weights.ocean = 1.0;
    for (temperature, expected) in [
        (0.12, OceanArea::ColdOcean),
        (0.41, OceanArea::SeagrassForest),
        (0.85, OceanArea::CoralReef),
    ] {
        biome.temperature = temperature;
        let mut dominant = 0;
        let mut clear = 0;
        let mut covered = 0;
        for z in (-128..128).step_by(4) {
            for x in (-128..128).step_by(4) {
                let (x, z) = (f64::from(x), f64::from(z));
                let a = terrain.ocean_with_biome(x, z, biome);
                assert_eq!(a, terrain.ocean_with_biome(x, z, biome));
                let b = terrain.ocean_with_biome(x + 0.001, z + 0.001, biome);
                for (a, b) in a
                    .weights
                    .into_iter()
                    .chain([a.cover, a.rock, a.colony])
                    .zip(b.weights.into_iter().chain([b.cover, b.rock, b.colony]))
                {
                    assert!((0.0..=1.0).contains(&a) && (a - b).abs() < 0.002);
                }
                dominant += usize::from(a.area() == Some(expected));
                clear += usize::from(a.cover < 0.01);
                covered += usize::from(a.cover > 0.70);
            }
        }
        assert!(
            dominant > 100 && clear > 100 && covered > 100,
            "{expected:?}: regions {dominant}, openings {clear}, colonies {covered}"
        );
    }
}

pub(super) fn cold_ocean(terrain: &TerraGenerator) -> (i64, i64) {
    for z in (-8192..8192).step_by(128) {
        for x in (-8192..8192).step_by(128) {
            let column = terrain.surface_column(x as f64 + 0.5, z as f64 + 0.5);
            if column.water_level.is_some() && column.ocean.weights[2] > 0.8 {
                return (x, z);
            }
        }
    }
    panic!("Default seed must contain cold ocean water");
}

#[test]
fn legacy_cold_ocean_remains_open_water_in_headless_and_padded_sections() {
    let terrain = TerraGenerator::new(WorldConfig {
        ecology_revision: 14,
        ..Default::default()
    })
    .unwrap();
    let (x, z) = cold_ocean(&terrain);
    let column = terrain.surface_column(x as f64 + 0.5, z as f64 + 0.5);
    assert!(column.ice_bottom().is_none());
    let bottom = column.water_bed().ceil() as i64;
    let top = column.water_level.unwrap().floor() as i64;
    assert!(column.water_bed() <= bottom as f64);
    let generator = SectionGenerator::new(terrain.clone());
    let section = global_to_section([x, top, z]).0;
    let mesh = generator
        .build(section, 0, &Default::default(), true)
        .unwrap();
    let headless = generator
        .build(section, 0, &Default::default(), false)
        .unwrap();
    assert_eq!(mesh.data, headless.data);
    assert_eq!(mesh.props, headless.props);
    for y in bottom..=top {
        assert_ne!(generator.block_at([x, y, z]).unwrap(), 49);
        assert_ne!(
            terrain
                .sample([x as f64 + 0.5, y as f64 + 0.5, z as f64 + 0.5])
                .material,
            49
        );
    }
    assert_eq!(generator.block_at([x, top + 1, z]).unwrap(), 0);
    let edited = generator
        .build(
            section,
            1,
            &[([x, top, z], MaterialId::Ice as u16)].into(),
            true,
        )
        .unwrap();
    assert_eq!(
        edited.data.block(global_to_section([x, top, z]).1).unwrap(),
        MaterialId::Ice as u16
    );
    for neighbour in [
        SectionPos::new(section.x - 1, section.y, section.z),
        SectionPos::new(section.x + 1, section.y, section.z),
    ] {
        let chunk = generator.generate(neighbour).unwrap();
        let origin = neighbour.origin().unwrap();
        for y in 0..16 {
            let global = [origin[0], origin[1] + y, origin[2]];
            assert_eq!(
                chunk.block([0, y as usize, 0]).unwrap(),
                generator.block_at(global).unwrap()
            );
        }
    }
}

#[test]
fn legacy_worlds_do_not_acquire_solid_ice_or_new_seabed() {
    let new = TerraGenerator::new(WorldConfig {
        landscape_revision: 3,
        ..Default::default()
    })
    .unwrap();
    let (x, z) = cold_ocean(&new);
    let mut config = new.config().clone();
    config.ocean_revision = 0;
    let old = TerraGenerator::new(config.clone()).unwrap();
    let a = old.surface_column(x as f64 + 0.5, z as f64 + 0.5);
    assert!(a.ice_bottom().is_none());
    // The extension is ocean-local; inland terrain is numerically unchanged.
    for [x, z] in [[0.5, 0.5], [-6784.0, -8192.0], [2304.0, -8192.0]] {
        assert_eq!(old.surface_height(x, z), new.surface_height(x, z));
    }
}
