use super::*;
use crate::{
    edits::BlockObjectState,
    streaming::{BlockEditRequest, LoadedSection, commit_block_edit},
};
use terra_core::{WorldConfig, plumbing};
use terra_voxel::{BlockSection, global_to_section};

const PIPES: [u16; 4] = [
    plumbing::WOOD_PIPE,
    plumbing::STONE_PIPE,
    plumbing::IRON_PIPE,
    plumbing::GLASS_PIPE,
];

#[test]
fn replacing_decorative_water_with_any_pipe_preserves_its_liquid_outside_the_natural_table() {
    for material in PIPES {
        let p = [-1, 8, -17];
        let (terrain, mut chunks, mut journal) = fixture(&[p]);
        put(&mut chunks, &mut journal, p, 14);
        assert_eq!(chunks.fluid_cell_at(&terrain, &journal, p, 14).amount, 0);
        pipe(&mut chunks, &mut journal, p, material, 4);
        assert_eq!(
            chunks.fluid_cell_at(&terrain, &journal, p, material),
            FluidCell {
                amount: 8,
                source: true,
                ..Default::default()
            }
        );
        put(&mut chunks, &mut journal, p, 0);
        assert_eq!(chunks.fluid_cell_at(&terrain, &journal, p, 0).amount, 8);
    }
}

fn fixture(positions: &[[i64; 3]]) -> (TerraGenerator, ChunkStore, EditJournal) {
    let terrain = TerraGenerator::new(WorldConfig {
        base_height: -100.,
        sea_level: -200.,
        rivers: false,
        caves: false,
        trees: false,
        biomes: false,
        ..Default::default()
    })
    .unwrap();
    let mut chunks = ChunkStore::default();
    for &p in positions {
        let owner = global_to_section(p).0;
        chunks.loaded.entry(owner).or_insert_with(|| LoadedSection {
            data: BlockSection::from_blocks(owner, 0, vec![3; 4096]).unwrap(),
            mesh: None,
            mesh_epoch: 0,
            water_epoch: 0,
            water: None,
            props: vec![],
        });
    }
    (terrain, chunks, EditJournal::default())
}

fn put(chunks: &mut ChunkStore, journal: &mut EditJournal, p: [i64; 3], material: u16) {
    let expected_material = chunks.block_at(p).unwrap();
    assert!(
        commit_block_edit(
            chunks,
            journal,
            &BlockEditRequest {
                position: p,
                expected_material,
                material,
            },
            &Default::default(),
        )
        .unwrap()
    );
}

fn pipe(
    chunks: &mut ChunkStore,
    journal: &mut EditJournal,
    p: [i64; 3],
    material: u16,
    facing: u8,
) {
    put(chunks, journal, p, material);
    journal
        .set_object_state(
            p,
            Some(BlockObjectState {
                facing,
                ..Default::default()
            }),
        )
        .unwrap();
}

#[test]
fn isolated_sources_keep_exact_water_and_source_flags_through_every_pipe_axis_and_removal() {
    for p in [[7, 8, 5], [-1, 8, -17]] {
        for material in PIPES {
            for facing in 0..6 {
                for amount in [3, 8] {
                    let (terrain, mut chunks, mut journal) = fixture(&[p]);
                    put(&mut chunks, &mut journal, p, 27);
                    let water = FluidCell {
                        amount,
                        source: true,
                        flow: [0; 2],
                    };
                    journal.set_fluid(p, water).unwrap();
                    let owner = global_to_section(p).0;
                    let fluid_epoch = journal.fluid_epoch(owner);
                    pipe(&mut chunks, &mut journal, p, material, facing);
                    assert_eq!(journal.fluid(p), Some(water));
                    assert_eq!(journal.fluid_epoch(owner), fluid_epoch);
                    assert_eq!(chunks.fluid_cell_at(&terrain, &journal, p, material), water);
                    let epochs = (journal.fluid_epoch(owner), journal.water_mesh_epoch(owner));
                    for _ in 0..32 {
                        assert!(
                            !simulate_fluid_step(&terrain, &chunks, &mut journal, 256)
                                .unwrap()
                                .changed
                        );
                    }
                    assert_eq!(
                        (journal.fluid_epoch(owner), journal.water_mesh_epoch(owner)),
                        epochs
                    );
                    put(&mut chunks, &mut journal, p, 0);
                    assert_eq!(
                        journal.fluid(p),
                        Some(water),
                        "removing a pipe leaves its surrounding water"
                    );
                    assert_eq!(journal.fluid_epoch(owner), fluid_epoch);
                    for _ in 0..16 {
                        assert!(
                            !simulate_fluid_step(&terrain, &chunks, &mut journal, 256)
                                .unwrap()
                                .changed
                        );
                    }
                    assert_eq!(chunks.fluid_cell_at(&terrain, &journal, p, 0), water);
                }
            }
        }
    }
}

#[test]
fn a_completely_piped_source_square_stays_quiet_across_four_sections_and_confirmed_reload() {
    let positions = [[-1, 8, 15], [0, 8, 15], [-1, 8, 16], [0, 8, 16]];
    for facing in [0, 1, 4] {
        let (terrain, mut chunks, mut journal) = fixture(&positions);
        for (p, material) in positions.into_iter().zip(PIPES) {
            put(&mut chunks, &mut journal, p, 27);
            pipe(&mut chunks, &mut journal, p, material, facing);
        }
        let expected = FluidCell {
            amount: 8,
            source: true,
            flow: [0; 2],
        };
        let epochs: Vec<_> = positions
            .into_iter()
            .map(|p| {
                let owner = global_to_section(p).0;
                (
                    owner,
                    journal.fluid_epoch(owner),
                    journal.water_mesh_epoch(owner),
                )
            })
            .collect();
        for _ in 0..64 {
            assert!(
                !simulate_fluid_step(&terrain, &chunks, &mut journal, 256)
                    .unwrap()
                    .changed
            );
        }
        for (owner, fluid, mesh) in epochs {
            assert_eq!(journal.fluid_epoch(owner), fluid);
            assert_eq!(journal.water_mesh_epoch(owner), mesh);
        }
        let mut reloaded = EditJournal::default();
        let mut replica_chunks = ChunkStore::default();
        for owner in journal.changed_sections() {
            replica_chunks
                .apply_confirmed_delta(&mut reloaded, journal.section_delta(owner))
                .unwrap();
        }
        for p in positions {
            assert_eq!(journal.fluid(p), Some(expected));
            assert_eq!(reloaded.fluid(p), Some(expected));
            assert_eq!(reloaded.object_state(p), journal.object_state(p));
        }
        for _ in 0..32 {
            assert!(
                !simulate_fluid_step(&terrain, &chunks, &mut reloaded, 256)
                    .unwrap()
                    .changed
            );
        }
    }
}

#[test]
fn swapping_waterloggable_materials_preserves_the_existing_source() {
    let p = [7, 8, 5];
    let (terrain, mut chunks, mut journal) = fixture(&[p]);
    put(&mut chunks, &mut journal, p, 27);
    let water = journal.fluid(p).unwrap();
    for material in PIPES {
        pipe(&mut chunks, &mut journal, p, material, 4);
        assert_eq!(journal.fluid(p), Some(water));
    }
    for material in [82, 74, 90, 92] {
        put(&mut chunks, &mut journal, p, material);
        assert_eq!(journal.fluid(p), Some(water));
        assert_eq!(chunks.fluid_cell_at(&terrain, &journal, p, material), water);
        assert!(
            chunks
                .water_column_at_point(
                    &terrain,
                    &journal,
                    [p[0] as f64 + 0.1, p[1] as f64 + 0.85, p[2] as f64 + 0.1]
                )
                .is_some()
        );
    }
    for _ in 0..32 {
        simulate_fluid_step(&terrain, &chunks, &mut journal, 256).unwrap();
    }
    assert_eq!(journal.fluid(p), Some(water));
    put(&mut chunks, &mut journal, p, 3);
    assert!(
        journal
            .fluid(p)
            .is_none_or(|water| water.amount == 0 && !water.source)
    );
}

#[test]
fn pipes_do_not_create_a_source_from_finite_flowing_water() {
    let p = [7, 8, 5];
    for material in PIPES {
        for facing in [0, 1, 4] {
            let (terrain, mut chunks, mut journal) = fixture(&[p]);
            put(&mut chunks, &mut journal, p, 0);
            let water = FluidCell {
                amount: 5,
                source: false,
                flow: [1, 0],
            };
            journal.set_fluid(p, water).unwrap();
            pipe(&mut chunks, &mut journal, p, material, facing);
            assert_eq!(journal.fluid(p), Some(water));
            for _ in 0..32 {
                simulate_fluid_step(&terrain, &chunks, &mut journal, 256).unwrap();
            }
            assert!(
                journal
                    .fluid(p)
                    .is_none_or(|water| water.amount == 0 && !water.source)
            );
            assert!(journal.fluid_cells().all(|(_, water)| !water.source));
        }
    }
}
