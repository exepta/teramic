use super::*;

#[test]
fn closed_fence_ring_only_wets_the_inward_regions_including_its_corners() {
    for origin in [[15, 8, 15], [-1, -16, -1]] {
        for material in [90, 92] {
            for facing in 0..4 {
                let (terrain, mut chunks, mut journal) = fixture(origin);
                for x in -1..=1 {
                    for z in -1..=1 {
                        if x != 0 || z != 0 {
                            put(
                                &mut chunks,
                                &mut journal,
                                [origin[0] + x, origin[1], origin[2] + z],
                                material,
                                facing,
                            );
                        }
                    }
                }
                put(&mut chunks, &mut journal, origin, 27, 0);
                advance(&terrain, &chunks, &mut journal, 64);
                for x in -1..=1 {
                    for z in -1..=1 {
                        for u in [0.2, 0.8] {
                            for v in [0.2, 0.8] {
                                let point = [
                                    origin[0] as f64 + f64::from(x) + u,
                                    origin[1] as f64 + 0.2,
                                    origin[2] as f64 + f64::from(z) + v,
                                ];
                                let expected = point[0] > origin[0] as f64 - 0.5
                                    && point[0] < origin[0] as f64 + 1.5
                                    && point[2] > origin[2] as f64 - 0.5
                                    && point[2] < origin[2] as f64 + 1.5;
                                assert_eq!(
                                    chunks
                                        .water_column_at_point(&terrain, &journal, point)
                                        .is_some(),
                                    expected,
                                    "material={material} facing={facing} point={point:?}"
                                );
                                assert_eq!(
                                    super::super::fluid_motion_at(
                                        &terrain, &chunks, &journal, point
                                    )
                                    .immersion
                                        > 0.,
                                    expected,
                                    "motion material={material} facing={facing} point={point:?}"
                                );
                            }
                        }
                    }
                }
                let restored = reload(&journal);
                for x in [origin[0] as f64 - 0.8, origin[0] as f64 - 0.2] {
                    let point = [x, origin[1] as f64 + 0.2, origin[2] as f64 + 0.2];
                    assert_eq!(
                        chunks.water_column_at_point(&terrain, &journal, point),
                        chunks.water_column_at_point(&terrain, &restored, point)
                    );
                }
            }
        }
    }
}
use crate::{edits::BlockObjectState, streaming::LoadedSection};
use std::collections::BTreeMap;
use terra_voxel::BlockSection;

fn put(chunks: &mut ChunkStore, journal: &mut EditJournal, p: [i64; 3], material: u16, facing: u8) {
    let (owner, local) = global_to_section(p);
    chunks
        .loaded
        .get_mut(&owner)
        .unwrap()
        .data
        .set_block(local, material)
        .unwrap();
    journal.record(p, material).unwrap();
    if super::super::space::partial(material) {
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
}

fn fixture(origin: [i64; 3]) -> (TerraGenerator, ChunkStore, EditJournal) {
    let terrain = TerraGenerator::new(terra_core::WorldConfig {
        sea_level: -100.,
        base_height: -50.,
        hill_amplitude: 0.,
        mountain_amplitude: 0.,
        rivers: false,
        caves: false,
        biomes: false,
        trees: false,
        ..Default::default()
    })
    .unwrap();
    let mut chunks = ChunkStore::default();
    for x in origin[0] - 2..=origin[0] + 3 {
        for y in origin[1] - 1..=origin[1] + 1 {
            for z in origin[2] - 2..=origin[2] + 3 {
                let owner = global_to_section([x, y, z]).0;
                chunks.loaded.entry(owner).or_insert_with(|| LoadedSection {
                    data: BlockSection::from_blocks(owner, 0, vec![3; 4096]).unwrap(),
                    mesh: None,
                    mesh_epoch: 0,
                    water_epoch: 0,
                    water: None,
                    props: vec![],
                });
            }
        }
    }
    let mut journal = EditJournal::default();
    for x in origin[0] - 2..=origin[0] + 3 {
        for y in origin[1]..=origin[1] + 1 {
            for z in origin[2] - 2..=origin[2] + 3 {
                put(&mut chunks, &mut journal, [x, y, z], 0, 0);
            }
        }
    }
    (terrain, chunks, journal)
}

fn amount(journal: &EditJournal) -> u64 {
    journal
        .fluid_cells()
        .map(|(_, cell)| u64::from(cell.amount))
        .sum()
}
fn advance(terrain: &TerraGenerator, chunks: &ChunkStore, journal: &mut EditJournal, count: usize) {
    let initial = amount(journal);
    let mut added = 0;
    let mut removed = 0;
    for _ in 0..count {
        let step = simulate_fluid_step(terrain, chunks, journal, 4096).unwrap();
        added += step.sourced + step.replenished;
        removed += step.drained + step.absorbed;
        assert_eq!(amount(journal) + removed, initial + added);
    }
}
fn reload(journal: &EditJournal) -> EditJournal {
    let mut reloaded = EditJournal::default();
    for owner in journal.changed_sections() {
        reloaded.import_delta(journal.section_delta(owner)).unwrap();
    }
    reloaded
}

fn slab_ring(chunks: &mut ChunkStore, journal: &mut EditJournal, origin: [i64; 3]) {
    for x in -1..=2 {
        for z in -1..=2 {
            if x == -1 || x == 2 || z == -1 || z == 2 {
                put(
                    chunks,
                    journal,
                    [origin[0] + x, origin[1], origin[2] + z],
                    82,
                    0,
                );
            }
        }
    }
}

#[test]
fn source_fed_half_height_water_stays_inside_the_solid_half_of_a_slab_ring_across_sections_and_reload()
 {
    for origin in [[15, 16, 15], [-1, -16, -1]] {
        let (terrain, mut chunks, mut journal) = fixture(origin);
        slab_ring(&mut chunks, &mut journal, origin);
        let interior = [
            [origin[0], origin[1], origin[2]],
            [origin[0] + 1, origin[1], origin[2]],
            [origin[0], origin[1], origin[2] + 1],
            [origin[0] + 1, origin[1], origin[2] + 1],
        ];
        for cell in interior {
            journal
                .set_fluid(
                    cell,
                    FluidCell {
                        amount: 4,
                        source: cell == interior[0],
                        flow: [0; 2],
                    },
                )
                .unwrap();
        }
        advance(&terrain, &chunks, &mut journal, 64);
        assert_eq!(amount(&journal), 16);
        for cell in interior {
            assert_eq!(journal.fluid(cell).unwrap().amount, 4);
        }
        assert!(
            journal
                .fluid_cells()
                .all(|(cell, fluid)| interior.contains(&cell) || fluid.amount == 0)
        );
        let before: BTreeMap<_, _> = journal.fluid_cells().collect();
        let mut restored = reload(&journal);
        for cell in interior {
            restored.wake_fluid_neighbors(cell);
        }
        advance(&terrain, &chunks, &mut restored, 64);
        assert_eq!(restored.fluid_cells().collect::<BTreeMap<_, _>>(), before);
        assert_eq!(amount(&restored), 16);
        assert_eq!(
            restored
                .fluid_cells()
                .filter(|(_, fluid)| fluid.source)
                .count(),
            1
        );
    }
}

#[test]
fn a_real_bucket_source_stays_inside_the_slab_boundary() {
    let origin = [15, 16, 15];
    let (terrain, mut chunks, mut journal) = fixture(origin);
    slab_ring(&mut chunks, &mut journal, origin);
    assert!(
        crate::water::buckets::transfer(
            &mut chunks,
            &mut journal,
            &terrain,
            &terra_core::block_properties::BlockPropertiesCatalog::default(),
            origin,
            true
        )
        .unwrap()
    );
    assert_eq!(
        journal.fluid(origin).unwrap(),
        FluidCell {
            amount: 8,
            source: true,
            flow: [0; 2]
        }
    );
    advance(&terrain, &chunks, &mut journal, 64);
    let slab = [origin[0] - 1, origin[1], origin[2]];
    let outside = [origin[0] - 2, origin[1], origin[2]];
    assert!(journal.fluid(outside).is_none_or(|cell| cell.amount == 0));
    let water = chunks.fluid_cell_at(&terrain, &journal, slab, 82);
    let space = super::super::space::FluidSpace::at(&journal, slab, |p| chunks.block_at(p));
    assert!(!space.contains(water, [0.5, 0.25, 0.5]));
    assert!(!space.contains(water, [0.5, 0.75, 0.5]));
    assert_eq!(journal.fluid(origin).unwrap().amount, 4);
    assert_eq!(
        journal
            .fluid_cells()
            .filter(|(_, cell)| cell.source)
            .count(),
        1
    );
}

#[test]
fn wet_source_connected_stair_inner_corner_matches_the_source_height_without_becoming_a_source() {
    for origin in [[15, 16, 15], [-1, -16, -1]] {
        let (terrain, mut chunks, mut journal) = fixture(origin);
        let sources = [origin, [origin[0], origin[1], origin[2] + 1]];
        let stairs = [
            [origin[0] + 1, origin[1], origin[2]],
            [origin[0] + 1, origin[1], origin[2] + 1],
        ];
        for source in sources {
            put(&mut chunks, &mut journal, source, 27, 0);
        }
        put(&mut chunks, &mut journal, stairs[0], 74, 1);
        put(&mut chunks, &mut journal, stairs[1], 74, 0);
        advance(&terrain, &chunks, &mut journal, 64);
        for stair in stairs {
            let water = chunks.fluid_cell_at(&terrain, &journal, stair, 74);
            assert_eq!(water.amount, 8);
            assert!(!water.source);
            let point = [
                stair[0] as f64 + 0.15,
                stair[1] as f64 + 0.85,
                stair[2] as f64 + 0.15,
            ];
            let column = chunks
                .water_column_at_point(&terrain, &journal, point)
                .unwrap();
            assert_eq!(column.level, origin[1] as f64 + 1.0);
        }
        let restored = reload(&journal);
        for stair in stairs {
            assert_eq!(restored.fluid(stair), journal.fluid(stair));
        }
        assert_eq!(
            journal
                .fluid_cells()
                .filter(|(_, cell)| cell.source)
                .count(),
            sources.len()
        );
    }
}

#[test]
fn source_water_fills_connected_fence_gaps_and_the_air_behind_them_at_one_height() {
    for origin in [[15, 16, 15], [-1, -16, -1]] {
        let (terrain, mut chunks, mut journal) = fixture(origin);
        put(&mut chunks, &mut journal, origin, 27, 0);
        let fences = [
            [origin[0] + 1, origin[1], origin[2]],
            [origin[0] + 1, origin[1], origin[2] + 1],
        ];
        for fence in fences {
            put(&mut chunks, &mut journal, fence, 90, 0);
        }
        let behind = [origin[0] + 2, origin[1], origin[2]];
        // This basin includes34 ordinary cells. One source has to pay every
        // height increment before its distant rear surface reaches equilibrium.
        advance(&terrain, &chunks, &mut journal, 320);
        for fence in fences {
            let water = chunks.fluid_cell_at(&terrain, &journal, fence, 90);
            assert_eq!(water.amount, 8);
            assert!(!water.source);
            let space =
                super::super::space::FluidSpace::at(&journal, fence, |p| chunks.block_at(p));
            assert!(space.contains(water, [0.15, 0.8, 0.15]));
            assert!(!space.contains(water, [0.5, 0.8, 0.5]));
        }
        assert_eq!(journal.fluid(behind).unwrap().amount, 8);
        assert!(!journal.fluid(behind).unwrap().source);
        let restored = reload(&journal);
        for fence in fences.into_iter().chain([behind]) {
            assert_eq!(restored.fluid(fence), journal.fluid(fence));
        }
        assert_eq!(
            journal
                .fluid_cells()
                .filter(|(_, cell)| cell.source)
                .count(),
            1
        );
    }
}

#[test]
fn bucket_fence_ring_fills_only_the_interior_and_waterlogged_rails() {
    for bank in [82, 90, 92] {
        let origin = [15, 16, 15];
        let (terrain, mut chunks, mut journal) = fixture(origin);
        for x in -1..=2 {
            for z in -1..=2 {
                if x == -1 || x == 2 || z == -1 || z == 2 {
                    put(
                        &mut chunks,
                        &mut journal,
                        [origin[0] + x, origin[1], origin[2] + z],
                        bank,
                        0,
                    );
                }
            }
        }
        put(&mut chunks, &mut journal, origin, 27, 0);
        advance(&terrain, &chunks, &mut journal, 64);
        for (p, water) in journal.fluid_cells() {
            if p[1] == origin[1]
                && (p[0] < origin[0] - 1
                    || p[0] > origin[0] + 2
                    || p[2] < origin[2] - 1
                    || p[2] > origin[2] + 2)
            {
                assert_eq!(water.amount, 0, "bank {bank} leaked at {p:?}");
            }
        }
        assert_eq!(
            journal.fluid(origin).unwrap().amount,
            if bank == 82 { 4 } else { 8 }
        );
        let inside = [origin[0] + 1, origin[1], origin[2] + 1];
        assert_eq!(
            journal.fluid(inside).unwrap().amount,
            if bank == 82 { 4 } else { 8 }
        );
        let wall = [origin[0] - 1, origin[1], origin[2]];
        if bank != 82 {
            assert!(journal.fluid(wall).is_some_and(|w| w.amount > 0));
        }
    }
}

#[test]
fn already_full_source_square_is_capped_to_the_slab_crest_with_an_exact_ledger() {
    let origin = [15, 16, 15];
    let (terrain, mut chunks, mut journal) = fixture(origin);
    slab_ring(&mut chunks, &mut journal, origin);
    for dx in 0..2 {
        for dz in 0..2 {
            put(
                &mut chunks,
                &mut journal,
                [origin[0] + dx, origin[1], origin[2] + dz],
                27,
                0,
            );
        }
    }
    advance(&terrain, &chunks, &mut journal, 64);
    for dx in 0..2 {
        for dz in 0..2 {
            let cell = journal
                .fluid([origin[0] + dx, origin[1], origin[2] + dz])
                .unwrap();
            assert_eq!(cell.amount, 4);
            assert!(cell.source);
        }
    }
}
