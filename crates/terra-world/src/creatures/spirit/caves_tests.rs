use super::*;
use crate::{
    TerraGenerator,
    edits::EditJournal,
    streaming::{ChunkStore, LoadedSection},
};
use terra_core::WorldConfig;
use terra_voxel::{BlockSection, SectionPos};

fn fixture() -> (TerraGenerator, ChunkStore, EditJournal, [i64; 2]) {
    let terrain = TerraGenerator::new(WorldConfig {
        biomes: false,
        caves: false,
        trees: false,
        water: false,
        base_height: 64.0,
        hill_amplitude: 0.0,
        mountain_amplitude: 0.0,
        ..Default::default()
    })
    .unwrap();
    let home = (-8..=8)
        .map(|x| [x, 0])
        .find(|home| {
            terra_core::creatures::definitions::get(Kind::SpiritGloom)
                .permits_cave_spawn(random(gloom_id(*home).unwrap(), terrain.config().seed.0), 4)
        })
        .unwrap();
    let mut chunks = ChunkStore::default();
    for sx in 0..2 {
        for sz in 0..2 {
            let section = SectionPos::new(home[0] * 2 + sx, 0, home[1] * 2 + sz);
            let mut data = BlockSection::empty(section).unwrap();
            for x in 0..16 {
                for z in 0..16 {
                    for y in [0, 5, 6, 7] {
                        data.set_block([x, y, z], 3).unwrap();
                    }
                }
            }
            chunks.loaded.insert(
                section,
                LoadedSection {
                    data,
                    mesh: None,
                    mesh_epoch: 0,
                    water_epoch: 0,
                    water: None,
                    props: vec![],
                },
            );
        }
    }
    (terrain, chunks, EditJournal::default(), home)
}
#[test]
fn loaded_covered_cave_glooms_are_valid_persistent_and_cannot_respawn_after_death() {
    let (terrain, chunks, journal, home) = fixture();
    let h = Habitat {
        terrain: &terrain,
        chunks: &chunks,
        journal: &journal,
    };
    let mut population = CreaturePopulation::default();
    assert_eq!(spawn(&mut population, &h, home, 1.4, 0), 0);
    assert_eq!(spawn(&mut population, &h, home, 1.4, 1), 1);
    let id = gloom_id(home).unwrap();
    let animal = &population.animals[&id];
    assert_eq!(animal.kind, Kind::SpiritGloom);
    assert!(matches!(animal.variant, gloom::LIGHT | gloom::SHADOW));
    assert!(h.clear(animal.position, animal.size(), false));
    assert!(population.valid());
    assert!(population.explored.is_empty());
    let animal = population.animals.get_mut(&id).unwrap();
    animal.health = 0.0;
    animal.behavior = Behavior::Dead;
    let saved = population.clone();
    assert_eq!(spawn(&mut population, &h, home, 1.4, 1), 0);
    assert_eq!(saved, population);
}
#[test]
fn unknown_or_daylight_caves_do_not_claim_an_identity_or_surface_habitat() {
    let (terrain, mut chunks, journal, home) = fixture();
    let mut population = CreaturePopulation::default();
    for section in chunks.loaded.values_mut() {
        for x in 0..16 {
            for z in 0..16 {
                for y in [5, 6, 7] {
                    section.data.set_block([x, y, z], 0).unwrap();
                }
            }
        }
    }
    let h = Habitat {
        terrain: &terrain,
        chunks: &chunks,
        journal: &journal,
    };
    assert_eq!(spawn(&mut population, &h, home, 1.4, 1), 0);
    chunks.loaded.clear();
    let h = Habitat {
        terrain: &terrain,
        chunks: &chunks,
        journal: &journal,
    };
    assert_eq!(spawn(&mut population, &h, home, 1.4, 1), 0);
    assert!(population.animals.is_empty() && population.explored.is_empty());
}
