use super::*;
use crate::{
    TerraGenerator,
    edits::EditJournal,
    streaming::{ChunkStore, LoadedSection},
};
use terra_core::{PlayerUuid, WorldConfig};
use terra_voxel::{BlockSection, SectionPos};

fn fixture(negative: bool) -> (TerraGenerator, ChunkStore, EditJournal, [i64; 3], [f64; 3]) {
    let terrain = TerraGenerator::new(WorldConfig {
        landscape_revision: 3,
        water: false,
        trees: false,
        biomes: false,
        rivers: false,
        caves: false,
        base_height: 3.,
        hill_amplitude: 0.,
        mountain_amplitude: 0.,
        ..Default::default()
    })
    .unwrap();
    let section = if negative {
        SectionPos::new(-1, 0, -1)
    } else {
        SectionPos::default()
    };
    let mut data = BlockSection::empty(section).unwrap();
    for x in 0..16 {
        for z in 0..16 {
            for y in 0..3 {
                data.set_block([x, y, z], 3).unwrap();
            }
        }
    }
    let mut chunks = ChunkStore::default();
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
    let offset = if negative { -16 } else { 0 };
    (
        terrain,
        chunks,
        EditJournal::default(),
        [8 + offset, 3, 8 + offset],
        [8.5 + offset as f64, 3., 12.5 + offset as f64],
    )
}

#[test]
fn starter_positions_are_distinct_dry_loaded_reachable_and_work_for_negative_coordinates() {
    for negative in [false, true] {
        let (terrain, chunks, journal, keep, feet) = fixture(negative);
        let owner = PlayerUuid::from_u128(80);
        let habitat = Habitat {
            terrain: &terrain,
            chunks: &chunks,
            journal: &journal,
        };
        let players = [[
            [feet[0] - 0.3, 3., feet[2] - 0.3],
            [feet[0] + 0.3, 4.8, feet[2] + 0.3],
        ]];
        let result = plan(&habitat, keep, owner, feet, &players).unwrap();
        assert_eq!(result.len(), STARTER_COUNT);
        for (i, position) in result.iter().enumerate() {
            assert!(habitat.clear(*position, CreatureKind::Villager.size(), false));
            assert_eq!(
                habitat.ground(*position, CreatureKind::Villager),
                Some(*position)
            );
            assert_eq!(
                claim_chunk(position.map(|v| v.floor() as i64)),
                claim_chunk(keep)
            );
            assert!(!overlaps(
                bounds(*position, CreatureKind::Villager.size()),
                players[0]
            ));
            assert!(
                result[..i]
                    .iter()
                    .all(|other| (position[0] - other[0]).hypot(position[2] - other[2]) >= 1.15)
            );
        }
    }
}

#[test]
fn unloaded_water_edited_floors_and_enclosed_seed_never_produce_a_partial_plan() {
    for blocked in 0..5 {
        let (terrain, mut chunks, mut journal, keep, feet) = fixture(false);
        match blocked {
            0 => chunks.loaded.clear(),
            1 => {
                let data = &mut chunks.loaded.get_mut(&SectionPos::default()).unwrap().data;
                for x in 0..16 {
                    for z in 0..16 {
                        data.set_block([x, 3, z], terra_voxel::MaterialId::FluidWater as u16)
                            .unwrap();
                    }
                }
            }
            2 => {
                for x in 0..16 {
                    for z in 0..16 {
                        journal.record([x, 2, z], 3).unwrap();
                    }
                }
            }
            3 => {
                let data = &mut chunks.loaded.get_mut(&SectionPos::default()).unwrap().data;
                for x in 7..=9 {
                    for z in 11..=13 {
                        if x != 8 || z != 12 {
                            for y in 3..8 {
                                data.set_block([x, y, z], 3).unwrap();
                            }
                        }
                    }
                }
            }
            _ => {
                for x in 0..16 {
                    for z in 0..16 {
                        journal
                            .set_fluid(
                                [x, 3, z],
                                crate::water::FluidCell {
                                    amount: 8,
                                    flow: [0; 2],
                                    source: true,
                                },
                            )
                            .unwrap();
                    }
                }
            }
        }
        let before = journal.gameplay().clone();
        assert_eq!(
            plan(
                &Habitat {
                    terrain: &terrain,
                    chunks: &chunks,
                    journal: &journal
                },
                keep,
                PlayerUuid::from_u128(80),
                feet,
                &[]
            ),
            Err(KingdomError::StarterAreaUnavailable),
            "case {blocked}"
        );
        assert_eq!(journal.gameplay(), &before);
    }
}

#[test]
fn a_founder_cannot_use_a_foreign_claim_as_the_reachable_seed() {
    let (terrain, chunks, mut journal, keep, feet) = fixture(false);
    journal
        .gameplay_mut()
        .kingdoms
        .found(PlayerUuid::from_u128(99), [8, 3, 8], "Foreign Realm")
        .unwrap();
    assert_eq!(
        plan(
            &Habitat {
                terrain: &terrain,
                chunks: &chunks,
                journal: &journal
            },
            keep,
            PlayerUuid::from_u128(80),
            feet,
            &[]
        ),
        Err(KingdomError::StarterAreaUnavailable)
    );
}

#[test]
fn a_safe_built_founder_floor_can_reach_unmodified_outdoor_spawn_ground() {
    let (terrain, chunks, mut journal, keep, feet) = fixture(false);
    let support = [feet[0].floor() as i64, 2, feet[2].floor() as i64];
    journal.record(support, 3).unwrap();
    let habitat = Habitat {
        terrain: &terrain,
        chunks: &chunks,
        journal: &journal,
    };
    assert!(!terrain_floor(&habitat, feet));
    let positions = plan(&habitat, keep, PlayerUuid::from_u128(80), feet, &[]).unwrap();
    assert!(
        positions
            .into_iter()
            .all(|position| terrain_floor(&habitat, position))
    );
}
