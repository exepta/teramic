use super::*;
use crate::{
    edits::{BlockObjectState, EditJournal},
    streaming::{BlockEditRequest, LoadedSection, commit_block_edit},
};
use terra_core::{ItemKind, kinetic};
use terra_voxel::{BlockSection, SectionPos, blocks::BlockHit, global_to_section};

const MOUNTS: [([i32; 3], u8); 5] = [
    ([1, 0, 0], 4),
    ([-1, 0, 0], 5),
    ([0, 0, 1], 6),
    ([0, 0, -1], 7),
    ([0, 1, 0], 0),
];

#[test]
fn all_torch_mounts_reject_water_and_flooded_torches_drop_once_without_removing_water() {
    let terrain = crate::TerraGenerator::new(terra_core::WorldConfig {
        sea_level: -100.,
        rivers: false,
        ..Default::default()
    })
    .unwrap();
    for (normal, facing) in MOUNTS {
        let (mut chunks, mut journal) = fixture();
        let anchor = [7, 6, 7];
        put(&mut chunks, &mut journal, anchor, 3);
        let plan = mount_plan(&chunks, &journal, anchor, normal);
        let p = plan.edits[0].position;
        let cell = crate::water::FluidCell {
            amount: 8,
            source: true,
            ..Default::default()
        };
        journal.set_fluid(p, cell).unwrap();
        assert_eq!(
            crate::placement::environment(&plan, &chunks, &journal, true).unwrap_err(),
            "Torches cannot be placed in water"
        );
        assert!(
            commit_block_edit(
                &mut chunks,
                &mut journal,
                &plan.edits[0],
                &Default::default()
            )
            .is_err()
        );
        assert_eq!(chunks.block_at(p), Some(0));
        assert_eq!(journal.fluid(p), Some(cell));
        journal.set_fluid(p, Default::default()).unwrap();
        assert!(
            commit_block_edit(
                &mut chunks,
                &mut journal,
                &plan.edits[0],
                &Default::default()
            )
            .unwrap()
        );
        journal
            .set_object_state(
                p,
                Some(BlockObjectState {
                    facing,
                    ..Default::default()
                }),
            )
            .unwrap();
        journal.set_fluid(p, cell).unwrap();
        let step =
            crate::water::simulate_fluid_step(&terrain, &chunks, &mut journal, 4096).unwrap();
        assert!(step.flooded_torches.contains(&p));
        wash_away(&mut chunks, &mut journal, &step.flooded_torches).unwrap();
        wash_away(&mut chunks, &mut journal, &step.flooded_torches).unwrap();
        assert_eq!(chunks.block_at(p), Some(0));
        assert!(journal.object_state(p).is_none());
        assert_eq!(journal.gameplay().drops.len(), 1);
        assert_eq!(journal.gameplay().drops[0].stack.item, ItemKind::Block(40));
        assert_eq!(journal.fluid(p), Some(cell));
        let mut replica = EditJournal::default();
        for owner in journal.changed_sections() {
            replica.import_delta(journal.section_delta(owner)).unwrap();
        }
        assert_eq!(replica.block(p), Some(0));
        assert_eq!(replica.fluid(p), Some(cell));
    }
}

fn put(chunks: &mut ChunkStore, journal: &mut EditJournal, cell: [i64; 3], material: u16) {
    let (owner, local) = global_to_section(cell);
    chunks
        .loaded
        .get_mut(&owner)
        .unwrap()
        .data
        .set_block(local, material)
        .unwrap();
    journal.record(cell, material).unwrap();
}

fn fixture() -> (ChunkStore, EditJournal) {
    let mut chunks = ChunkStore::default();
    for x in -1..=0 {
        for z in -1..=0 {
            let owner = SectionPos::new(x, 0, z);
            chunks.loaded.insert(
                owner,
                LoadedSection {
                    data: BlockSection::empty(owner).unwrap(),
                    mesh: None,
                    mesh_epoch: 0,
                    water_epoch: 0,
                    water: None,
                    props: vec![],
                },
            );
        }
    }
    let mut journal = EditJournal::default();
    put(
        &mut chunks,
        &mut journal,
        [12, 6, 12],
        kinetic::WOODEN_LARGE_GEAR,
    );
    journal
        .set_object_state([12, 6, 12], Some(BlockObjectState::default()))
        .unwrap();
    assert!(journal.has_extended_kinetics());
    (chunks, journal)
}

#[test]
fn real_lateral_flow_washes_away_a_dry_torch_without_turning_the_flow_into_a_source() {
    let terrain = crate::TerraGenerator::new(terra_core::WorldConfig {
        base_height: -100.,
        sea_level: -200.,
        rivers: false,
        ..Default::default()
    })
    .unwrap();
    let (mut chunks, mut journal) = fixture();
    let torch = [7, 6, 7];
    let source = [6, 6, 7];
    for x in 5..=8 {
        for z in 6..=8 {
            for y in 5..=7 {
                let p = [x, y, z];
                if p != torch && p != source {
                    put(&mut chunks, &mut journal, p, 3);
                }
            }
        }
    }
    let plan = mount_plan(&chunks, &journal, [7, 5, 7], [0, 1, 0]);
    crate::placement::environment(&plan, &chunks, &journal, true).unwrap();
    assert!(
        commit_block_edit(
            &mut chunks,
            &mut journal,
            &plan.edits[0],
            &Default::default()
        )
        .unwrap()
    );
    put(&mut chunks, &mut journal, source, 27);
    for _ in 0..24 {
        let step =
            crate::water::simulate_fluid_step(&terrain, &chunks, &mut journal, 4096).unwrap();
        wash_away(&mut chunks, &mut journal, &step.flooded_torches).unwrap();
    }
    assert_eq!(chunks.block_at(torch), Some(0));
    assert_eq!(journal.gameplay().drops.len(), 1);
    assert!(
        journal
            .fluid(torch)
            .is_some_and(|cell| cell.amount > 0 && !cell.source)
    );
    assert!(journal.fluid(source).unwrap().source);
}

fn mount_plan(
    chunks: &ChunkStore,
    journal: &EditJournal,
    anchor: [i64; 3],
    normal: [i32; 3],
) -> crate::placement::Layout {
    let hit = BlockHit {
        position: anchor,
        normal,
        distance: 3.5,
    };
    let ray = (
        std::array::from_fn(|axis| anchor[axis] as f64 + 0.5 + f64::from(normal[axis]) * 4.0),
        normal.map(|value| -f64::from(value)),
    );
    let plan = crate::placement::propose(chunks, journal, hit, ItemKind::Block(40), ray, [0.0; 2])
        .unwrap();
    assert_eq!(
        crate::targeting::placement_facing(chunks, journal, hit, 40, ray),
        facing(normal).unwrap()
    );
    plan
}

#[test]
fn floor_and_all_wall_previews_pass_real_clearance_even_when_mechanics_exist() {
    for anchor in [[7, 6, 7], [-1, 6, -1]] {
        for (normal, expected_facing) in MOUNTS {
            let (mut chunks, mut journal) = fixture();
            put(&mut chunks, &mut journal, anchor, 3);
            let plan = mount_plan(&chunks, &journal, anchor, normal);
            crate::placement::environment(&plan, &chunks, &journal, true).unwrap();
            let position = std::array::from_fn(|axis| anchor[axis] + i64::from(normal[axis]));
            assert_eq!(plan.edits[0].position, position);
            assert_eq!(plan.states[0].1.unwrap().facing, expected_facing);
            assert_eq!(support(position, expected_facing), Some(anchor));
            for bounds in crate::placement::boxes(
                &plan,
                &chunks,
                &journal,
                &BlockPropertiesCatalog::default(),
            ) {
                assert!(
                    !bounds.intersects(anchor.map(|v| v as f64), anchor.map(|v| v as f64 + 1.0)),
                    "the mounted torch must not collide with its supporting face"
                );
            }
        }
    }
}

#[test]
fn unsupported_surfaces_and_ceilings_are_rejected_in_the_same_preview_path() {
    for (normal, _) in MOUNTS {
        let (mut chunks, mut journal) = fixture();
        let anchor = [7, 6, 7];
        put(&mut chunks, &mut journal, anchor, 82);
        let plan = mount_plan(&chunks, &journal, anchor, normal);
        assert!(crate::placement::environment(&plan, &chunks, &journal, false).is_err());
    }
    let (mut chunks, mut journal) = fixture();
    let anchor = [7, 6, 7];
    put(&mut chunks, &mut journal, anchor, 3);
    let hit = BlockHit {
        position: anchor,
        normal: [0, -1, 0],
        distance: 3.5,
    };
    assert_eq!(facing(hit.normal), None);
    assert!(
        crate::placement::propose(
            &chunks,
            &journal,
            hit,
            ItemKind::Block(40),
            ([7.5, 2.5, 7.5], [0.0, 1.0, 0.0]),
            [0.0; 2]
        )
        .is_err()
    );
}

#[test]
fn wall_and_floor_torches_keep_valid_replacements_and_drop_once_if_support_becomes_partial() {
    for (normal, expected_facing) in MOUNTS {
        let (mut chunks, mut journal) = fixture();
        let anchor = [7, 6, 7];
        put(&mut chunks, &mut journal, anchor, 3);
        let plan = mount_plan(&chunks, &journal, anchor, normal);
        let position = plan.edits[0].position;
        let properties = BlockPropertiesCatalog::default();
        assert!(commit_block_edit(&mut chunks, &mut journal, &plan.edits[0], &properties).unwrap());
        journal
            .set_object_state(position, plan.states[0].1)
            .unwrap();
        assert!(supported(&chunks, &properties, position, expected_facing));
        assert!(
            commit_block_edit(
                &mut chunks,
                &mut journal,
                &BlockEditRequest {
                    position: anchor,
                    expected_material: 3,
                    material: 2
                },
                &properties
            )
            .unwrap()
        );
        assert_eq!(chunks.block_at(position), Some(40));
        assert_eq!(
            journal.object_state(position).unwrap().facing,
            expected_facing
        );
        assert!(journal.gameplay().drops.is_empty());
        assert!(
            commit_block_edit(
                &mut chunks,
                &mut journal,
                &BlockEditRequest {
                    position: anchor,
                    expected_material: 2,
                    material: 82
                },
                &properties
            )
            .unwrap()
        );
        assert_eq!(chunks.block_at(position), Some(0));
        assert!(journal.object_state(position).is_none());
        let drops = &journal.gameplay().drops;
        assert_eq!(drops.len(), 1);
        assert_eq!(drops[0].stack.item, ItemKind::Block(40));
        assert_eq!(drops[0].stack.count, 1);
        assert!(
            commit_block_edit(
                &mut chunks,
                &mut journal,
                &BlockEditRequest {
                    position: anchor,
                    expected_material: 82,
                    material: 0
                },
                &properties
            )
            .unwrap()
        );
        assert_eq!(journal.gameplay().drops.len(), 1);
    }
}
