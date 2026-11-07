use super::*;
use crate::streaming::LoadedSection;
use terra_core::{Inventory, ItemKind, WoodSpecies};
use terra_voxel::{BlockSection, global_to_section};

fn fixture(anchor: Cell, floor: bool) -> (ChunkStore, EditJournal) {
    let center = global_to_section(anchor).0;
    let mut chunks = ChunkStore::default();
    for x in -1..=1 {
        for y in -1..=1 {
            for z in -1..=1 {
                let position = center.offset([x, y, z]).unwrap();
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
    if floor {
        for x in -4..=4 {
            for z in -4..=4 {
                let (owner, local) = global_to_section(add(anchor, [x, -1, z]));
                chunks
                    .loaded
                    .get_mut(&owner)
                    .unwrap()
                    .data
                    .set_block(local, 3)
                    .unwrap();
            }
        }
    }
    (chunks, EditJournal::default())
}

fn put(
    chunks: &mut ChunkStore,
    journal: &mut EditJournal,
    position: Cell,
    material: u16,
    state: BlockObjectState,
) {
    let (owner, local) = global_to_section(position);
    chunks
        .loaded
        .get_mut(&owner)
        .unwrap()
        .data
        .set_block(local, material)
        .unwrap();
    journal.record(position, material).unwrap();
    journal.set_object_state(position, Some(state)).unwrap();
}

fn pair(
    chunks: &mut ChunkStore,
    journal: &mut EditJournal,
    anchor: Cell,
    material: u16,
    facing: u8,
) -> Cell {
    let state = BlockObjectState {
        facing,
        wood: WoodSpecies::Cherry,
        ..Default::default()
    };
    let direction = if terra_core::beds::is_bed(material) {
        kinetic::forward(facing)
    } else if matches!(material, 28 | 29 | 99) {
        state.right()
    } else {
        [0, 1, 0]
    };
    let peer = add(anchor, direction);
    put(
        chunks,
        journal,
        anchor,
        material,
        BlockObjectState {
            peer: Some(peer),
            ..state
        },
    );
    put(
        chunks,
        journal,
        peer,
        material,
        BlockObjectState {
            peer: Some(anchor),
            secondary: true,
            ..state
        },
    );
    peer
}

fn deltas(journal: &EditJournal) -> Vec<crate::edits::SectionDelta> {
    let sections: BTreeSet<_> = journal.changed_sections().collect();
    sections
        .into_iter()
        .map(|section| journal.section_delta(section))
        .collect()
}

fn rotate(chunks: &mut ChunkStore, journal: &mut EditJournal, position: Cell, reverse: bool) {
    let plan = propose(chunks, journal, position, reverse).unwrap();
    assert!(commit(chunks, journal, &plan).unwrap());
}

#[test]
fn unsupported_blocks_do_not_include_pots_fences_or_ordinary_terrain() {
    for material in [
        0, 1, 2, 3, 4, 40, 90, 95, 98, 102, 103, 131, 138, 161, 162, 169,
    ] {
        assert!(!supported(material));
    }
    for material in [
        28, 29, 59, 60, 65, 68, 71, 74, 82, 99, 100, 104, 105, 136, 137, 139, 140, 141, 147, 148,
        151, 163, 164, 165, 166, 167, 168, 172,
    ] {
        assert!(supported(material));
    }
}

#[test]
fn all_stair_and_rotary_poses_have_a_finite_forward_reverse_cycle() {
    let anchor = [-1, 4, -1];
    for (material, count, floor) in [
        (74, 24, false),
        (144, 6, false),
        (172, 6, false),
        (141, 6, false),
        (147, 4, false),
        (148, 4, true),
    ] {
        let (mut chunks, mut journal) = fixture(anchor, floor);
        for facing in 0..count {
            let original = BlockObjectState {
                facing,
                wood: WoodSpecies::Birch,
                ..Default::default()
            };
            put(&mut chunks, &mut journal, anchor, material, original);
            let plan = propose(&chunks, &journal, anchor, false).unwrap();
            let boxes = placement::boxes(&plan, &chunks, &journal, &Default::default());
            assert!(boxes.iter().all(|bounds| {
                bounds
                    .minimum
                    .into_iter()
                    .chain(bounds.maximum)
                    .all(f64::is_finite)
            }));
            assert!(commit(&mut chunks, &mut journal, &plan).unwrap());
            assert_eq!(
                journal.object_state(anchor).unwrap().facing,
                (facing + 1) % count
            );
            rotate(&mut chunks, &mut journal, anchor, true);
            assert_eq!(journal.object_state(anchor), Some(original));
        }
    }
}

#[test]
fn six_slab_planes_preserve_both_mixed_material_identities_and_reverse_exactly() {
    let anchor = [-1, 4, -1];
    let (mut chunks, mut journal) = fixture(anchor, false);
    for half in [0, 4, 5, 6, 7, 8] {
        for other in construction::SLAB_MATERIALS {
            let original = BlockObjectState {
                facing: construction::merged_slab(82, half, other).unwrap(),
                wood: WoodSpecies::Spruce,
                ..Default::default()
            };
            put(&mut chunks, &mut journal, anchor, 82, original);
            rotate(&mut chunks, &mut journal, anchor, false);
            let halves =
                construction::slab_halves(82, journal.object_state(anchor).unwrap().facing)
                    .unwrap();
            assert_eq!([halves[0].0, halves[1].0], [82, other]);
            rotate(&mut chunks, &mut journal, anchor, true);
            assert_eq!(journal.object_state(anchor), Some(original));
        }
        put(
            &mut chunks,
            &mut journal,
            anchor,
            82,
            BlockObjectState {
                facing: half,
                ..Default::default()
            },
        );
        for _ in 0..6 {
            rotate(&mut chunks, &mut journal, anchor, false);
        }
        assert_eq!(journal.object_state(anchor).unwrap().facing, half);
    }
}

#[test]
fn colored_beds_and_wooden_benches_rotate_the_peer_around_the_fixed_primary() {
    let anchor = [-1, 4, -1];
    for (material, facing) in std::iter::once((28, 0)).chain(
        terra_core::beds::DyeColor::ALL
            .into_iter()
            .map(|color| (71, terra_core::beds::dyed_facing(1, color))),
    ) {
        let (mut chunks, mut journal) = fixture(anchor, true);
        let old_peer = pair(&mut chunks, &mut journal, anchor, material, facing);
        let original = journal.object_state(anchor).unwrap();
        rotate(&mut chunks, &mut journal, old_peer, false);
        let rotated = journal.object_state(anchor).unwrap();
        assert_eq!(rotated.facing / 4, facing / 4);
        assert_eq!(rotated.wood, WoodSpecies::Cherry);
        assert_eq!(
            rotated.peer,
            Some(add(anchor, turn(relative(old_peer, anchor), false)))
        );
        assert_eq!(chunks.block_at(old_peer), Some(0));
        let peer = rotated.peer.unwrap();
        assert_eq!(journal.object_state(peer).unwrap().facing, rotated.facing);
        rotate(&mut chunks, &mut journal, peer, true);
        assert_eq!(journal.object_state(anchor), Some(original));
        assert_eq!(chunks.block_at(old_peer), Some(material));
        assert!(journal.gameplay().drops.is_empty());
    }
}

#[test]
fn rgb_bed_rotations_keep_both_halves_tinted_across_negative_section_boundaries() {
    let anchor = [-1, 4, -1];
    let color = [71, 93, 184];
    for material in [71, 72, 73] {
        let (mut chunks, mut journal) = fixture(anchor, true);
        let original_peer = pair(&mut chunks, &mut journal, anchor, material, 0);
        journal.set_tint(anchor, Some(color)).unwrap();
        journal.set_tint(original_peer, Some(color)).unwrap();
        let original = journal.object_state(anchor).unwrap();
        for _ in 0..4 {
            let old_peer = journal.object_state(anchor).unwrap().peer.unwrap();
            rotate(&mut chunks, &mut journal, old_peer, false);
            let peer = journal.object_state(anchor).unwrap().peer.unwrap();
            assert_eq!(journal.tint_at(anchor), Some(color));
            assert_eq!(journal.tint_at(peer), Some(color));
            assert_eq!(journal.tint_at(old_peer), None);
            assert_eq!(chunks.block_at(old_peer), Some(0));
            assert_eq!(chunks.block_at(peer), Some(material));
            assert_eq!(journal.tints().count(), 2);
            assert!(journal.gameplay().drops.is_empty());
        }
        assert_eq!(journal.object_state(anchor), Some(original));
        assert_eq!(journal.tint_at(original_peer), Some(color));
    }
}

#[test]
fn rgb_bed_rotation_tint_overflow_does_not_publish_any_pose_or_block_changes() {
    let anchor = [-1, 4, -1];
    let (mut chunks, mut journal) = fixture(anchor, true);
    let peer = pair(&mut chunks, &mut journal, anchor, 71, 0);
    let color = [71, 93, 184];
    journal.set_tint(anchor, Some(color)).unwrap();
    journal.set_tint(peer, Some(color)).unwrap();
    let destination = add(anchor, [1, 0, 0]);
    let destination_owner = global_to_section(destination).0;
    assert_ne!(destination_owner, global_to_section(anchor).0);
    assert_ne!(destination_owner, global_to_section(peer).0);
    let mut delta = journal.section_delta(destination_owner);
    delta.appearance_epoch = u64::MAX;
    journal.import_delta(delta).unwrap();
    let before = deltas(&journal);
    let gameplay = journal.gameplay().clone();
    let epochs: Vec<_> = chunks
        .loaded
        .iter()
        .map(|(section, loaded)| {
            (
                *section,
                loaded.data.revision(),
                journal.mesh_epoch(*section),
                journal.water_mesh_epoch(*section),
            )
        })
        .collect();
    let plan = propose(&chunks, &journal, peer, false).unwrap();
    let error = commit(&mut chunks, &mut journal, &plan).unwrap_err();
    assert_eq!(error, "Tint appearance overflow");
    assert_eq!(deltas(&journal), before);
    assert_eq!(journal.gameplay(), &gameplay);
    assert_eq!(journal.tint_at(anchor), Some(color));
    assert_eq!(journal.tint_at(peer), Some(color));
    assert_eq!(journal.tint_at(destination), None);
    assert_eq!(chunks.block_at(peer), Some(71));
    assert_eq!(chunks.block_at(destination), Some(0));
    for (section, blocks, mesh, water) in epochs {
        assert_eq!(chunks.loaded[&section].data.revision(), blocks);
        assert_eq!(journal.mesh_epoch(section), mesh);
        assert_eq!(journal.water_mesh_epoch(section), water);
    }
}

#[test]
fn double_chest_moves_its_full_peer_inventory_without_drops_or_reordering() {
    let anchor = [-1, 4, -1];
    let (mut chunks, mut journal) = fixture(anchor, true);
    let peer = pair(&mut chunks, &mut journal, anchor, 29, 0);
    let mut primary = Inventory::default();
    primary.insert(ItemKind::Block(3), 17);
    let mut secondary = Inventory::default();
    secondary.insert(ItemKind::Block(4), 23);
    journal.set_container(anchor, primary.clone()).unwrap();
    journal.set_container(peer, secondary.clone()).unwrap();
    let order = journal
        .furniture_orders()
        .find(|(position, _)| *position == peer)
        .unwrap()
        .1;
    let revision = journal.gameplay().revision;
    rotate(&mut chunks, &mut journal, peer, false);
    let target = journal.object_state(anchor).unwrap().peer.unwrap();
    assert_eq!(journal.container_at(anchor), Some(&primary));
    assert_eq!(journal.container_at(target), Some(&secondary));
    assert!(journal.container_at(peer).is_none());
    assert_eq!(journal.gameplay().revision, revision + 1);
    assert_eq!(
        journal
            .furniture_orders()
            .find(|(position, _)| *position == target)
            .unwrap()
            .1,
        order
    );
    rotate(&mut chunks, &mut journal, target, true);
    assert_eq!(journal.container_at(peer), Some(&secondary));
    assert!(journal.gameplay().drops.is_empty());
}

#[test]
fn crusher_pair_and_mounted_crank_move_without_resetting_machine_progress() {
    let anchor = [-1, 4, -1];
    let (mut chunks, mut journal) = fixture(anchor, true);
    let peer = pair(&mut chunks, &mut journal, anchor, 99, 0);
    let crank = add(anchor, [2, 0, 0]);
    put(
        &mut chunks,
        &mut journal,
        crank,
        100,
        BlockObjectState {
            peer: Some(anchor),
            ..Default::default()
        },
    );
    let recipe = &terra_core::crusher::recipes()[0];
    let mut inventory = Inventory::default();
    inventory.insert(recipe.input, 3);
    journal.set_container(anchor, inventory.clone()).unwrap();
    let progress = terra_core::crusher::CrusherState {
        progress_seconds: recipe.seconds * 0.5,
        recipe: Some(recipe.id),
        completed: 7,
    };
    journal.set_crusher(anchor, progress.clone()).unwrap();
    rotate(&mut chunks, &mut journal, crank, false);
    assert_eq!(journal.crusher_at(anchor), Some(&progress));
    assert_eq!(journal.container_at(anchor), Some(&inventory));
    assert_eq!(
        journal.crusher_crank_position(anchor),
        Some(add(anchor, [0, 0, -2]))
    );
    assert_eq!(journal.crank_anchor(add(anchor, [0, 0, -2])), Some(anchor));
    assert_eq!(chunks.block_at(peer), Some(0));
    assert_eq!(chunks.block_at(crank), Some(0));
    assert!(journal.gameplay().drops.is_empty());
}

#[test]
fn vertical_pairs_preserve_cells_open_bits_and_furnace_private_progress() {
    let anchor = [-1, 4, -1];
    for material in [59, 65, 66, 67, 139] {
        let (mut chunks, mut journal) = fixture(anchor, true);
        let facing = if material == 59 { 2 } else { 6 };
        let peer = pair(&mut chunks, &mut journal, anchor, material, facing);
        let original = journal.object_state(anchor).unwrap();
        let furnace = terra_core::furnace::FurnaceState {
            burn_remaining: 10.0,
            burn_total: 20.0,
            completed: 13,
            smoke_credit: 0.5,
            ..Default::default()
        };
        if material == 59 {
            journal.set_furnace(anchor, furnace.clone()).unwrap();
        }
        rotate(&mut chunks, &mut journal, peer, false);
        let next = journal.object_state(anchor).unwrap();
        assert_eq!(next.peer, Some(peer));
        assert_eq!(next.facing / 4, facing / 4);
        assert_eq!(chunks.block_at(peer), Some(material));
        if material == 59 {
            assert_eq!(journal.furnace_at(anchor), Some(&furnace));
        }
        rotate(&mut chunks, &mut journal, peer, true);
        assert_eq!(journal.object_state(anchor), Some(original));
    }
}

#[test]
fn two_axial_cranks_follow_a_vertical_axis_flip_without_clobbering_each_other() {
    let anchor = [-1, 4, -1];
    let (mut chunks, mut journal) = fixture(anchor, false);
    put(
        &mut chunks,
        &mut journal,
        anchor,
        144,
        BlockObjectState {
            facing: 4,
            ..Default::default()
        },
    );
    for (facing, wood) in [(4, WoodSpecies::Birch), (5, WoodSpecies::Spruce)] {
        put(
            &mut chunks,
            &mut journal,
            add(anchor, kinetic::axis(facing)),
            100,
            BlockObjectState {
                facing,
                peer: Some(anchor),
                wood,
                ..Default::default()
            },
        );
    }
    rotate(&mut chunks, &mut journal, anchor, false);
    for (facing, wood) in [(5, WoodSpecies::Birch), (4, WoodSpecies::Spruce)] {
        let position = add(anchor, kinetic::axis(facing));
        assert_eq!(journal.crank_anchor(position), Some(anchor));
        assert_eq!(journal.object_state(position).unwrap().wood, wood);
    }
    rotate(&mut chunks, &mut journal, anchor, true);
    assert_eq!(
        journal.object_state(add(anchor, [0, 1, 0])).unwrap().wood,
        WoodSpecies::Birch
    );
}

#[test]
fn chimney_ports_rotate_geometrically_and_preserve_the_elbow_style() {
    for pose in (8..23).chain(40..55) {
        let state = BlockObjectState {
            facing: pose,
            ..Default::default()
        };
        if !state.valid_for([0; 3], 60) {
            continue;
        }
        let next = facing(60, state, false).unwrap();
        let expected = crate::objects::chimney_ports(60, pose)
            .unwrap()
            .map(|port| turn(port, false));
        let actual = crate::objects::chimney_ports(60, next).unwrap();
        assert!(actual == expected || actual == [expected[1], expected[0]]);
        assert_eq!(next >= 32, pose >= 32);
        assert_eq!(
            facing(
                60,
                BlockObjectState {
                    facing: next,
                    ..state
                },
                true
            )
            .unwrap(),
            pose
        );
    }
}

#[test]
fn blocked_or_unloaded_peer_rotations_never_change_journal_contents() {
    let anchor = [-1, 4, -1];
    let (mut chunks, mut journal) = fixture(anchor, true);
    let peer = pair(&mut chunks, &mut journal, anchor, 29, 0);
    let destination = add(anchor, [0, 0, -1]);
    let (owner, local) = global_to_section(destination);
    chunks
        .loaded
        .get_mut(&owner)
        .unwrap()
        .data
        .set_block(local, 3)
        .unwrap();
    let before = deltas(&journal);
    assert!(propose(&chunks, &journal, anchor, false).is_err());
    assert_eq!(deltas(&journal), before);
    chunks
        .loaded
        .get_mut(&owner)
        .unwrap()
        .data
        .set_block(local, 0)
        .unwrap();
    chunks.loaded.remove(&global_to_section(peer).0);
    assert!(propose(&chunks, &journal, anchor, false).is_err());
    assert_eq!(deltas(&journal), before);
    assert!(journal.gameplay().drops.is_empty());
}

#[test]
fn revision_and_container_overflow_reject_the_complete_rotation_before_any_write() {
    let anchor = [-1, 4, -1];
    for section_overflow in [true, false] {
        let (mut chunks, mut journal) = fixture(anchor, true);
        let peer = pair(&mut chunks, &mut journal, anchor, 29, 0);
        let mut inventory = Inventory::default();
        inventory.insert(ItemKind::Block(3), 7);
        journal.set_container(peer, inventory.clone()).unwrap();
        if section_overflow {
            let mut delta = journal.section_delta(global_to_section(anchor).0);
            delta.revision = u64::MAX;
            journal.import_delta(delta).unwrap();
        } else {
            journal.gameplay_mut().revision = u64::MAX;
        }
        let before = deltas(&journal);
        let epochs: Vec<_> = before
            .iter()
            .map(|delta| {
                (
                    delta.position,
                    journal.mesh_epoch(delta.position),
                    journal.water_mesh_epoch(delta.position),
                )
            })
            .collect();
        let plan = propose(&chunks, &journal, anchor, false).unwrap();
        assert!(commit(&mut chunks, &mut journal, &plan).is_err());
        assert_eq!(deltas(&journal), before);
        assert_eq!(journal.container_at(peer), Some(&inventory));
        assert_eq!(chunks.block_at(peer), Some(29));
        assert_eq!(chunks.block_at(add(anchor, [0, 0, -1])), Some(0));
        for (section, mesh, water) in epochs {
            assert_eq!(journal.mesh_epoch(section), mesh);
            assert_eq!(journal.water_mesh_epoch(section), water);
        }
    }
}

#[test]
fn orphan_cranks_and_supported_blocks_above_a_vacated_peer_are_rejected() {
    let anchor = [-1, 4, -1];
    let (mut chunks, mut journal) = fixture(anchor, true);
    put(
        &mut chunks,
        &mut journal,
        anchor,
        100,
        BlockObjectState {
            peer: Some(add(anchor, [-1, 0, 0])),
            ..Default::default()
        },
    );
    assert!(propose(&chunks, &journal, anchor, false).is_err());
    let peer = pair(&mut chunks, &mut journal, anchor, 28, 0);
    put(
        &mut chunks,
        &mut journal,
        add(peer, [0, 1, 0]),
        131,
        BlockObjectState::default(),
    );
    let before = deltas(&journal);
    assert!(propose(&chunks, &journal, anchor, false).is_err());
    assert_eq!(deltas(&journal), before);
}
