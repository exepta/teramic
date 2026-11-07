use super::*;
use crate::edits::EditJournal;
use terra_core::WorldConfig;
use terra_voxel::{MaterialId, SectionPos};

#[test]
fn section_generation_matches_global_blocks_in_negative_regions_and_any_order() {
    let generator = SectionGenerator::new(TerraGenerator::new(WorldConfig::default()).unwrap());
    let p = SectionPos::new(-1, -1, -1);
    let first = generator.generate(p).unwrap();
    generator.generate(SectionPos::new(1, 0, 1)).unwrap();
    assert_eq!(first, generator.generate(p).unwrap());
    assert_eq!(
        first.block([15; 3]).unwrap(),
        generator.block_at([-1; 3]).unwrap()
    );
    let right = generator.generate(SectionPos::new(0, -1, -1)).unwrap();
    for z in 0..16 {
        for y in 0..16 {
            assert_eq!(
                right.block([0, y, z]).unwrap(),
                generator
                    .block_at([0, -16 + y as i64, -16 + z as i64])
                    .unwrap()
            );
        }
    }
    assert!(generator.generate(SectionPos::new(i64::MAX, 0, 0)).is_err());
    assert!(generator.block_at([i64::MAX; 3]).is_err());
}
#[test]
fn trees_and_subsurface_layers_are_deterministic_across_sections() {
    let terrain = TerraGenerator::new(WorldConfig {
        landscape_revision: 3,
        sea_level: 8.0,
        base_height: 12.0,
        hill_amplitude: 0.0,
        mountain_amplitude: 0.0,
        caves: false,
        biomes: false,
        ..Default::default()
    })
    .unwrap();
    let generator = SectionGenerator::new(terrain);
    assert_eq!(
        generator.block_at([1, 11, 1]).unwrap(),
        MaterialId::Grass as u16
    );
    assert_eq!(
        generator.block_at([1, 10, 1]).unwrap(),
        MaterialId::Dirt as u16
    );
    assert_eq!(
        generator.block_at([1, 0, 1]).unwrap(),
        MaterialId::Stone as u16
    );
    let mut wood = 0;
    let mut leaves = 0;
    for z in -4..=4 {
        for x in -4..=4 {
            let chunk = generator.generate(SectionPos::new(x, 0, z)).unwrap();
            wood += chunk
                .blocks()
                .iter()
                .filter(|&&m| {
                    matches!(
                        MaterialId::from_raw(m),
                        Some(MaterialId::Wood | MaterialId::WeatheredWood | MaterialId::SpruceLog)
                    )
                })
                .count();
            let canopy = generator.generate(SectionPos::new(x, 1, z)).unwrap();
            leaves += canopy
                .blocks()
                .iter()
                .filter(|&&m| {
                    [
                        MaterialId::Leaves as u16,
                        MaterialId::AutumnLeaves as u16,
                        MaterialId::SilverLeaves as u16,
                    ]
                    .contains(&m)
                })
                .count();
        }
    }
    assert!(wood > 0 && leaves > 0);
}
#[test]
fn journal_overrides_rebuild_data_and_padded_meshes() {
    let generator = SectionGenerator::new(
        TerraGenerator::new(WorldConfig {
            trees: false,
            caves: false,
            ..Default::default()
        })
        .unwrap(),
    );
    let mut journal = EditJournal::default();
    journal.record([15, 0, 0], 0).unwrap();
    let p = SectionPos::default();
    let snapshot = journal.snapshot(p).unwrap();
    let result = generator
        .build(p, journal.revision(p), &snapshot, true)
        .unwrap();
    assert_eq!(result.data.block([15, 0, 0]).unwrap(), 0);
    assert_eq!(result.data.revision(), 1);
    assert!(result.mesh.is_some());
    assert_eq!(
        journal
            .snapshot(SectionPos::new(1, 0, 0))
            .unwrap()
            .get(&[15, 0, 0]),
        Some(&0)
    );
}
#[test]
fn bounded_world_has_two_foundation_layers_a_solid_cave_floor_and_no_outside_blocks() {
    let generator = SectionGenerator::new(TerraGenerator::new(WorldConfig::default()).unwrap());
    for y in [-129, -128, -127, -126, -121, 356, 357] {
        let global = [11, y, -23];
        let (section, local) = terra_voxel::global_to_section(global);
        let sample = generator.block_at(global).unwrap();
        assert_eq!(
            generator.generate(section).unwrap().block(local).unwrap(),
            sample
        );
        match y {
            -128 | -127 => assert_eq!(sample, MaterialId::Bedrock as u16),
            -126 | -121 => assert_ne!(sample, 0),
            -129 | 357 => assert_eq!(sample, 0),
            _ => {}
        }
    }
}
