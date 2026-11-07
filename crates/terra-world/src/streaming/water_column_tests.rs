use super::*;
use crate::{
    edits::{BlockObjectState, EditJournal},
    water::FluidCell,
};
use terra_core::plumbing;
use terra_voxel::{BlockSection, global_to_section};

fn fixture() -> (ChunkStore, EditJournal, TerraGenerator) {
    let mut chunks = ChunkStore::default();
    for x in -1..=0 {
        for z in -1..=0 {
            let position = SectionPos::new(x, 1, z);
            chunks.loaded.insert(
                position,
                LoadedSection {
                    data: BlockSection::from_blocks(position, 0, vec![3; 4096]).unwrap(),
                    mesh: None,
                    mesh_epoch: 0,
                    water_epoch: 0,
                    water: None,
                    props: vec![],
                },
            );
        }
    }
    let terrain = TerraGenerator::new(terra_core::WorldConfig {
        base_height: 16.,
        sea_level: -100.,
        hill_amplitude: 0.,
        mountain_amplitude: 0.,
        rivers: false,
        caves: false,
        biomes: false,
        ..Default::default()
    })
    .unwrap();
    (chunks, EditJournal::default(), terrain)
}
fn put(chunks: &mut ChunkStore, journal: &mut EditJournal, p: [i64; 3], material: u16, facing: u8) {
    let (section, local) = global_to_section(p);
    chunks
        .loaded
        .get_mut(&section)
        .unwrap()
        .data
        .set_block(local, material)
        .unwrap();
    journal.record(p, material).unwrap();
    if material != 0 && material != 27 && material != 3 {
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

#[test]
fn all_four_sides_of_a_waterlogged_vertical_pipe_share_the_actual_pool_column() {
    let (mut chunks, mut journal, terrain) = fixture();
    let p = [-6, 18, -1];
    for x in -10..=-6 {
        for z in -3..=1 {
            for y in 17..=18 {
                put(&mut chunks, &mut journal, [x, y, z], 27, 0);
            }
            put(&mut chunks, &mut journal, [x, 19, z], 0, 0);
        }
    }
    put(&mut chunks, &mut journal, p, plumbing::IRON_PIPE, 4);
    put(
        &mut chunks,
        &mut journal,
        [-6, 19, -1],
        plumbing::IRON_PIPE,
        4,
    );
    put(
        &mut chunks,
        &mut journal,
        [-5, 19, -1],
        plumbing::IRON_PIPE,
        0,
    );
    assert_eq!(
        journal.fluid(p),
        Some(FluidCell {
            amount: 8,
            source: true,
            ..Default::default()
        })
    );
    for [dx, dz] in [[-0.29, 0.], [0.29, 0.], [0., -0.29], [0., 0.29]] {
        let point = [
            p[0] as f64 + 0.5 + dx,
            p[1] as f64 + 0.5,
            p[2] as f64 + 0.5 + dz,
        ];
        let column = chunks
            .water_column_at_point(&terrain, &journal, point)
            .unwrap();
        assert_eq!(
            column.level, 19.,
            "partition produced a false surface at {point:?}"
        );
        assert_eq!(
            column.bed, 17.,
            "partition produced a false bed at {point:?}"
        );
        assert_eq!(column.depth, 2.);
    }
    assert!(
        chunks
            .water_column_at_point(&terrain, &journal, [-5.5, 18.5, -0.5])
            .is_none(),
        "the real pipe solid must still exclude liquid"
    );
}

#[test]
fn a_real_horizontal_shaft_keeps_separate_water_intervals_and_upward_entry_uses_the_lower_one() {
    let (mut chunks, mut journal, terrain) = fixture();
    let p = [-8, 18, -8];
    for y in 17..=19 {
        put(&mut chunks, &mut journal, [p[0], y, p[2]], 27, 0);
    }
    put(&mut chunks, &mut journal, p, 144, 0);
    let space = crate::water::space::FluidSpace::at(&journal, p, |q| chunks.block_at(q));
    let solids: Vec<_> = space
        .solids
        .iter()
        .filter(|bounds| {
            bounds.minimum[0] <= 0.5
                && bounds.maximum[0] > 0.5
                && bounds.minimum[2] <= 0.5
                && bounds.maximum[2] > 0.5
        })
        .collect();
    let lower_ceiling = solids
        .iter()
        .map(|bounds| bounds.minimum[1])
        .fold(f64::INFINITY, f64::min);
    let upper_floor = solids
        .iter()
        .map(|bounds| bounds.maximum[1])
        .fold(f64::NEG_INFINITY, f64::max);
    assert!(lower_ceiling > 0. && upper_floor < 1. && lower_ceiling < upper_floor);
    let x = p[0] as f64 + 0.5;
    let z = p[2] as f64 + 0.5;
    let lower = chunks
        .water_column_at_point(&terrain, &journal, [x, 17.5, z])
        .unwrap();
    assert!(
        (lower.level - (18. + lower_ceiling)).abs() < 1e-8,
        "must enter the lower connected void above, not stop at voxel boundary"
    );
    assert_eq!(lower.bed, 17.);
    let upper = chunks
        .water_column_at_point(&terrain, &journal, [x, 19.5, z])
        .unwrap();
    assert_eq!(upper.level, 20.);
    assert!((upper.bed - (18. + upper_floor)).abs() < 1e-8);
    assert!(
        chunks
            .water_column_at_point(
                &terrain,
                &journal,
                [x, 18. + (lower_ceiling + upper_floor) * 0.5, z]
            )
            .is_none(),
        "real shaft gap must not be merged through solid material"
    );
}
