use super::*;
use crate::{
    edits::BlockObjectState,
    streaming::{BlockEditRequest, LoadedSection, commit_block_edit},
};
use terra_core::{
    ItemKind,
    lantern::{CHAIN, LANTERN},
};
use terra_voxel::{BlockSection, SectionPos, blocks::BlockHit, global_to_section};

fn fixture() -> (ChunkStore, EditJournal) {
    let mut chunks = ChunkStore::default();
    for x in -1..=0 {
        for z in -1..=0 {
            for y in 0..=1 {
                let position = SectionPos::new(x, y, z);
                chunks.loaded.insert(
                    position,
                    LoadedSection {
                        data: BlockSection::empty(position).unwrap(),
                        mesh: None,
                        mesh_epoch: 0,
                        water_epoch: 0,
                        water: None,
                        props: Vec::new(),
                    },
                );
            }
        }
    }
    (chunks, EditJournal::default())
}
fn put(
    chunks: &mut ChunkStore,
    journal: &mut EditJournal,
    p: [i64; 3],
    material: u16,
    state: Option<u8>,
) {
    let (owner, local) = global_to_section(p);
    chunks
        .loaded
        .get_mut(&owner)
        .unwrap()
        .data
        .set_block(local, material)
        .unwrap();
    journal.record(p, material).unwrap();
    if let Some(facing) = state {
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
fn mount(
    chunks: &mut ChunkStore,
    journal: &mut EditJournal,
    anchor: [i64; 3],
    normal: [i32; 3],
    material: u16,
) -> [i64; 3] {
    let plan = crate::placement::propose(
        chunks,
        journal,
        BlockHit {
            position: anchor,
            normal,
            distance: 1.,
        },
        ItemKind::Block(material),
        ([0.; 3], [0., 0., -1.]),
        [0.; 2],
    )
    .unwrap();
    crate::placement::environment(&plan, chunks, journal, true).unwrap();
    let p = plan.edits[0].position;
    assert!(commit_block_edit(chunks, journal, &plan.edits[0], &Default::default()).unwrap());
    for (p, state) in plan.states {
        journal.set_object_state(p, state).unwrap();
    }
    p
}

#[test]
fn all_lantern_mounts_have_matching_preview_pick_geometry_persistent_poses_and_support_drops() {
    for anchor in [[7, 7, 7], [-1, 17, -1]] {
        for normal in [
            [0, 1, 0],
            [1, 0, 0],
            [-1, 0, 0],
            [0, 0, 1],
            [0, 0, -1],
            [0, -1, 0],
        ] {
            let (mut chunks, mut journal) = fixture();
            put(&mut chunks, &mut journal, anchor, 3, None);
            let p = mount(&mut chunks, &mut journal, anchor, normal, LANTERN);
            let facing = lantern::facing(LANTERN, normal).unwrap();
            assert_eq!(journal.object_state(p).unwrap().facing, facing);
            let mut actual = Vec::new();
            assert!(crate::interaction_shapes::visit_boxes(
                &chunks,
                &journal,
                &Default::default(),
                p,
                |b| actual.push(b)
            ));
            let expected = lantern::boxes(LANTERN, facing)
                .into_iter()
                .map(|mut b| {
                    for (i, value) in p.into_iter().enumerate() {
                        b.minimum[i] += value as f64;
                        b.maximum[i] += value as f64;
                    }
                    b
                })
                .collect::<Vec<_>>();
            assert_eq!(actual, expected);
            let mut replica = EditJournal::default();
            for owner in journal.changed_sections() {
                replica.import_delta(journal.section_delta(owner)).unwrap();
            }
            assert_eq!(replica.object_state(p), journal.object_state(p));
            assert!(
                commit_block_edit(
                    &mut chunks,
                    &mut journal,
                    &BlockEditRequest {
                        position: anchor,
                        expected_material: 3,
                        material: 0
                    },
                    &Default::default()
                )
                .unwrap()
            );
            assert_eq!(chunks.block_at(p), Some(0));
            assert!(journal.object_state(p).is_none());
            assert_eq!(journal.gameplay().drops.len(), 1);
            assert_eq!(
                journal.gameplay().drops[0].stack.item,
                ItemKind::Block(LANTERN)
            );
        }
    }
}

#[test]
fn anchored_chain_crosses_section_border_and_releases_every_link_and_lamp_once() {
    let (mut chunks, mut journal) = fixture();
    let anchor = [-1, 19, -1];
    put(&mut chunks, &mut journal, anchor, 36, None);
    let mut end = anchor;
    for _ in 0..6 {
        end = mount(&mut chunks, &mut journal, end, [0, -1, 0], CHAIN);
    }
    let lamp = mount(&mut chunks, &mut journal, end, [0, -1, 0], LANTERN);
    assert_eq!(dependents(&chunks, &journal, anchor).len(), 7);
    let upper = global_to_section(anchor).0;
    let data = chunks.loaded.remove(&upper).unwrap();
    assert_eq!(
        support_status(
            &chunks,
            &Default::default(),
            lamp,
            LANTERN,
            lantern::HANGING
        ),
        None
    );
    chunks.loaded.insert(upper, data);
    assert!(supported(
        &chunks,
        &Default::default(),
        lamp,
        LANTERN,
        lantern::HANGING
    ));
    assert!(
        commit_block_edit(
            &mut chunks,
            &mut journal,
            &BlockEditRequest {
                position: anchor,
                expected_material: 36,
                material: 0
            },
            &Default::default()
        )
        .unwrap()
    );
    for y in lamp[1]..anchor[1] {
        assert_eq!(chunks.block_at([-1, y, -1]), Some(0));
    }
    assert_eq!(
        journal
            .gameplay()
            .drops
            .iter()
            .filter(|d| d.stack.item == ItemKind::Block(CHAIN))
            .map(|d| u32::from(d.stack.count))
            .sum::<u32>(),
        6
    );
    assert_eq!(
        journal
            .gameplay()
            .drops
            .iter()
            .filter(|d| d.stack.item == ItemKind::Block(LANTERN))
            .map(|d| u32::from(d.stack.count))
            .sum::<u32>(),
        1
    );
    assert!(
        !commit_block_edit(
            &mut chunks,
            &mut journal,
            &BlockEditRequest {
                position: anchor,
                expected_material: 36,
                material: 0
            },
            &Default::default()
        )
        .unwrap()
    );
}

#[test]
fn lamps_reject_water_and_wash_away_once_while_metal_chain_keeps_water() {
    let terrain = crate::TerraGenerator::new(terra_core::WorldConfig {
        base_height: -100.,
        sea_level: -200.,
        rivers: false,
        ..Default::default()
    })
    .unwrap();
    let (mut chunks, mut journal) = fixture();
    put(&mut chunks, &mut journal, [7, 9, 7], 3, None);
    let chain = mount(&mut chunks, &mut journal, [7, 9, 7], [0, -1, 0], CHAIN);
    let water = crate::water::FluidCell {
        amount: 8,
        source: true,
        ..Default::default()
    };
    journal.set_fluid(chain, water).unwrap();
    assert!(crate::water::can_hold_fluid(CHAIN));
    let lamp = mount(&mut chunks, &mut journal, chain, [0, -1, 0], LANTERN);
    journal.set_fluid(lamp, water).unwrap();
    let step = crate::water::simulate_fluid_step(&terrain, &chunks, &mut journal, 4096).unwrap();
    assert!(step.flooded_torches.contains(&lamp));
    assert!(!step.flooded_torches.contains(&chain));
    crate::torches::wash_away(&mut chunks, &mut journal, &step.flooded_torches).unwrap();
    crate::torches::wash_away(&mut chunks, &mut journal, &step.flooded_torches).unwrap();
    assert_eq!(chunks.block_at(chain), Some(CHAIN));
    assert_eq!(chunks.block_at(lamp), Some(0));
    assert_eq!(journal.gameplay().drops.len(), 1);
    assert!(journal.fluid(lamp).unwrap().amount > 0);
    assert!(
        commit_block_edit(
            &mut chunks,
            &mut journal,
            &BlockEditRequest {
                position: lamp,
                expected_material: 0,
                material: LANTERN
            },
            &Default::default()
        )
        .is_err()
    );
}

#[test]
fn lanterns_fit_real_fence_post_tops_and_chains_cannot_float_or_extend_upward() {
    let (mut chunks, mut journal) = fixture();
    put(&mut chunks, &mut journal, [7, 6, 7], 90, Some(0));
    let lamp = mount(&mut chunks, &mut journal, [7, 6, 7], [0, 1, 0], LANTERN);
    assert_eq!(journal.object_state(lamp).unwrap().facing, lantern::TABLE);
    assert!(!supported(
        &chunks,
        &Default::default(),
        [3, 6, 3],
        CHAIN,
        0
    ));
    assert_eq!(lantern::facing(CHAIN, [0, 1, 0]), None);
}

#[test]
fn support_removal_preflights_all_chain_drops_and_drop_id_overflow_before_any_mutation() {
    let (mut chunks, mut journal) = fixture();
    let anchor = [7, 10, 7];
    put(&mut chunks, &mut journal, anchor, 36, None);
    let first = mount(&mut chunks, &mut journal, anchor, [0, -1, 0], CHAIN);
    let second = mount(&mut chunks, &mut journal, first, [0, -1, 0], CHAIN);
    let lamp = mount(&mut chunks, &mut journal, second, [0, -1, 0], LANTERN);
    let properties = BlockPropertiesCatalog::default();
    assert!(released_dependents(&chunks, &journal, anchor, 3, &properties).is_empty());
    assert_eq!(
        released_dependents(&chunks, &journal, anchor, 0, &properties).len(),
        3
    );
    let request = BlockEditRequest {
        position: anchor,
        expected_material: 36,
        material: 0,
    };
    for overflow in [false, true] {
        journal.gameplay_mut().drops.clear();
        journal.gameplay_mut().next_drop = 0;
        if overflow {
            journal.gameplay_mut().next_drop = u64::MAX - 2;
        } else {
            for _ in 0..terra_core::MAX_WORLD_DROPS - 2 {
                assert!(journal.gameplay_mut().drop_one([0.; 3], ItemKind::Stick));
            }
        }
        let revision = journal.revision(global_to_section(anchor).0);
        assert!(commit_block_edit(&mut chunks, &mut journal, &request, &properties).is_err());
        assert_eq!(journal.revision(global_to_section(anchor).0), revision);
        assert_eq!(chunks.block_at(anchor), Some(36));
        assert_eq!(chunks.block_at(first), Some(CHAIN));
        assert_eq!(chunks.block_at(second), Some(CHAIN));
        assert_eq!(chunks.block_at(lamp), Some(LANTERN));
    }
}

#[test]
fn unloaded_saved_wall_lantern_stays_in_the_cross_claim_permission_footprint() {
    let (mut chunks, mut journal) = fixture();
    let anchor = [-1, 18, -1];
    put(&mut chunks, &mut journal, anchor, 3, None);
    let lamp = mount(&mut chunks, &mut journal, anchor, [1, 0, 0], LANTERN);
    let owner = terra_core::PlayerUuid::from_u128(901);
    let actor = terra_core::PlayerUuid::from_u128(902);
    journal
        .gameplay_mut()
        .kingdoms
        .found(owner, [0, 20, -1], "Protected lamp")
        .unwrap();
    chunks.loaded.remove(&global_to_section(lamp).0).unwrap();
    assert_eq!(chunks.block_at(lamp), None);
    assert_eq!(journal.block(lamp), Some(LANTERN));
    assert_eq!(dependents(&chunks, &journal, anchor), vec![lamp]);
    let allowed = |p| {
        crate::kingdom_access::can_use(
            &journal.gameplay().kingdoms,
            &chunks,
            actor,
            terra_core::kingdom::KingdomPermission::Break,
            p,
        )
    };
    assert!(allowed(anchor));
    assert!(
        !dependents(&chunks, &journal, anchor)
            .into_iter()
            .all(allowed)
    );
    assert_eq!(
        released_dependents(&chunks, &journal, anchor, 0, &Default::default()),
        vec![lamp]
    );
    assert!(released_dependents(&chunks, &journal, anchor, 3, &Default::default()).is_empty());
}
