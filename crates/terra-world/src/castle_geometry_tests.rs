use super::*;
use crate::streaming::LoadedSection;
use terra_voxel::{BlockSection, global_to_section};

fn install(chunks: &mut ChunkStore, journal: &mut EditJournal, position: [i64; 3], material: u16) {
    let (section, local) = global_to_section(position);
    let loaded = chunks
        .loaded
        .entry(section)
        .or_insert_with(|| LoadedSection {
            data: BlockSection::empty(section).unwrap(),
            mesh: None,
            mesh_epoch: 0,
            water_epoch: 0,
            water: None,
            props: Vec::new(),
        });
    loaded.data.set_block(local, material).unwrap();
    journal.record(position, material).unwrap();
    if terra_core::castle::is_castle(material) {
        journal
            .set_object_state(position, Some(Default::default()))
            .unwrap();
    }
}

#[test]
fn two_arch_halves_cross_section_seams_without_an_invisible_solid_opening() {
    use terra_core::castle;
    let mut chunks = ChunkStore::default();
    let mut journal = EditJournal::default();
    let left = [15, 5, 8];
    let right = [16, 5, 8];
    for (position, material) in [
        (left, castle::ARCH_LEFT),
        (right, castle::ARCH_RIGHT),
        ([15, 5, 11], 3),
        ([16, 5, 11], 3),
    ] {
        install(&mut chunks, &mut journal, position, material);
    }
    let properties = BlockPropertiesCatalog::default();
    for (x, expected) in [
        (15.9, [15, 5, 11]),
        (16.1, [16, 5, 11]),
        (15.1, left),
        (16.9, right),
    ] {
        let hit = raycast(
            &chunks,
            &journal,
            &properties,
            [x, 5.45, 6.5],
            [0.0, 0.0, 1.0],
            6.0,
        )
        .unwrap();
        assert_eq!(
            hit.position, expected,
            "picking must use the actual stone opening"
        );
    }
    let mut boxes = Vec::new();
    for position in [left, right] {
        assert!(visit_collision_boxes(
            &chunks,
            &journal,
            &properties,
            position,
            |bounds| boxes.push(bounds)
        ));
    }
    assert!(
        boxes
            .iter()
            .all(|bounds| bounds.minimum[2] == 8.0 && bounds.maximum[2] == 9.0)
    );
    assert!(
        !boxes
            .iter()
            .any(|bounds| bounds.intersects([15.8, 5.20, 8.0], [16.2, 5.65, 9.0]))
    );
    assert!(
        boxes
            .iter()
            .any(|bounds| bounds.intersects([15.05, 5.20, 8.3], [15.15, 5.65, 8.7]))
    );
}

#[test]
fn castle_window_picking_and_collision_use_its_thin_glass_in_every_pose() {
    use terra_core::castle;
    let mut chunks = ChunkStore::default();
    let mut journal = EditJournal::default();
    let position = [8, 8, 8];
    install(&mut chunks, &mut journal, position, castle::WINDOW);
    let properties = BlockPropertiesCatalog::default();
    for facing in 0..24 {
        journal
            .set_object_state(
                position,
                Some(crate::edits::BlockObjectState {
                    facing,
                    ..Default::default()
                }),
            )
            .unwrap();
        let origin = castle::point(facing, [0.35, 0.35, -1.0]);
        let end = castle::point(facing, [0.35, 0.35, 0.0]);
        let direction = std::array::from_fn(|axis| end[axis] - origin[axis]);
        let origin = std::array::from_fn(|axis| origin[axis] + position[axis] as f64);
        let hit = raycast(&chunks, &journal, &properties, origin, direction, 3.0).unwrap();
        assert_eq!(hit.position, position);
        assert!(
            (hit.distance - 1.495).abs() < 0.00001,
            "opaque/full-cell window hit: {facing}"
        );
        let mut distances = Vec::new();
        visit_collision_boxes(&chunks, &journal, &properties, position, |bounds| {
            if let Some((distance, _)) = bounds.ray(origin, direction, 3.0) {
                distances.push(distance);
            }
        });
        let nearest = distances.into_iter().min_by(f64::total_cmp).unwrap();
        assert!((nearest - hit.distance).abs() < 0.00001);
    }
}

#[test]
fn every_castle_pose_survives_repeated_edits_section_replication_and_reverse_wrench_rotation() {
    use crate::edits::BlockObjectState;
    use terra_core::castle;
    let position = [-17, 5, -1];
    for material in castle::BLOCKS {
        assert!(crate::placement::furniture(material));
        assert!(crate::wrench::supported(material));
        for facing in 0..24 {
            let state = BlockObjectState {
                facing,
                ..Default::default()
            };
            assert!(state.valid_for(position, material));
            let mut journal = EditJournal::default();
            let mut chunks = ChunkStore::default();
            install(&mut chunks, &mut journal, position, material);
            journal.set_object_state(position, Some(state)).unwrap();
            journal.record(position, material).unwrap();
            assert_eq!(journal.object_state(position), Some(state));
            let mut replica = EditJournal::default();
            replica
                .import_delta(journal.section_delta(global_to_section(position).0))
                .unwrap();
            assert_eq!(replica.object_state(position), Some(state));
            for reverse in [false, true] {
                let rotated = crate::wrench::propose(&chunks, &journal, position, reverse).unwrap();
                let expected = (facing + if reverse { 23 } else { 1 }) % 24;
                assert_eq!(rotated.states[0].1.unwrap().facing, expected);
                assert!(rotated.edits.iter().all(|edit| edit.expected_material == material && edit.material == material));
            }
        }
        assert!(
            !BlockObjectState {
                facing: 24,
                ..Default::default()
            }
            .valid_for(position, material)
        );
    }
}

#[test]
fn castle_prediction_uses_the_shared_upright_placement_rule_on_all_six_support_faces() {
    use terra_core::castle;
    let mut chunks = ChunkStore::default();
    let mut journal = EditJournal::default();
    let support = [8, 8, 8];
    install(&mut chunks, &mut journal, support, 3);
    for normal in [
        [1, 0, 0],
        [-1, 0, 0],
        [0, 1, 0],
        [0, -1, 0],
        [0, 0, 1],
        [0, 0, -1],
    ] {
        let hit = BlockHit {
            position: support,
            normal,
            distance: 1.0,
        };
        let ray = ([8.5, 9.5, 6.0], [0.0, -0.25, 1.0]);
        for material in castle::BLOCKS {
            let facing = crate::targeting::placement_facing(&chunks, &journal, hit, material, ray);
            let plan = crate::placement::propose(
                &chunks,
                &journal,
                hit,
                terra_core::ItemKind::Block(material),
                ray,
                [0.0; 2],
            )
            .unwrap();
            assert_eq!(plan.states[0].0, hit.adjacent());
            assert_eq!(plan.states[0].1.unwrap().facing, facing);
            let basis = terra_core::construction::stair_basis(facing);
            if normal[1] == 0 {
                assert_eq!(basis[1], [0, 1, 0]);
                assert_eq!(basis[2], normal.map(|v| v as i8));
            } else {
                assert_eq!(basis[1], normal.map(|v| v as i8));
            }
        }
    }
}

#[test]
fn extending_castle_wall_edges_preserves_all_explicit_poses_including_arch_halves() {
    use terra_core::castle;
    let mut chunks = ChunkStore::default();
    let mut journal = EditJournal::default();
    let support = [8, 8, 8];
    for clicked in [
        castle::WINDOW,
        castle::ARROW_SLIT,
        castle::BATTLEMENT_STRAIGHT,
        castle::BATTLEMENT_CORNER,
        castle::ARCH_LEFT,
        castle::ARCH_RIGHT,
    ] {
        install(&mut chunks, &mut journal, support, clicked);
        for facing in 0..24 {
            journal
                .set_object_state(
                    support,
                    Some(crate::edits::BlockObjectState {
                        facing,
                        ..Default::default()
                    }),
                )
                .unwrap();
            let basis = terra_core::construction::stair_basis(facing);
            for normal in [
                basis[0],
                basis[0].map(|v| -v),
                basis[1],
                basis[1].map(|v| -v),
            ] {
                let hit = BlockHit {
                    position: support,
                    normal: normal.map(i32::from),
                    distance: 1.0,
                };
                let ray = ([8.5, 9.5, 6.0], [0.0, -0.25, 1.0]);
                for material in [
                    castle::WINDOW,
                    castle::ARCH_LEFT,
                    castle::ARCH_RIGHT,
                    castle::BATTLEMENT_STRAIGHT,
                ] {
                    assert_eq!(
                        crate::targeting::placement_facing(&chunks, &journal, hit, material, ray),
                        facing
                    );
                    let plan = crate::placement::propose(
                        &chunks,
                        &journal,
                        hit,
                        terra_core::ItemKind::Block(material),
                        ray,
                        [0.0; 2],
                    )
                    .unwrap();
                    assert_eq!(plan.states[0].1.unwrap().facing, facing);
                }
            }
        }
    }
}

#[test]
fn connected_corner_picking_placement_and_replica_geometry_cross_two_section_seams() {
    use terra_core::castle::{self, connections};
    let mut chunks = ChunkStore::default();
    let mut journal = EditJournal::default();
    let center = [15, 5, 15];
    let east = [16, 5, 15];
    let south = [15, 5, 16];
    for (p, id) in [
        (center, castle::BATTLEMENT_CORNER),
        (east, castle::BATTLEMENT_STRAIGHT),
        (south, castle::BERGFRIED_WALL),
    ] {
        install(&mut chunks, &mut journal, p, id);
    }
    let properties = BlockPropertiesCatalog::default();
    let collect = |chunks: &ChunkStore, journal: &EditJournal| {
        let mut result = Vec::new();
        assert!(visit_collision_boxes(
            chunks,
            journal,
            &properties,
            center,
            |b| result.push(b)
        ));
        result
    };
    let profile = collect(&chunks, &journal);
    let mut expected = connections::profile_boxes(castle::BATTLEMENT_CORNER, 10).to_vec();
    for b in &mut expected {
        for (i, v) in center.into_iter().enumerate() {
            b.minimum[i] += v as f64;
            b.maximum[i] += v as f64;
        }
    }
    assert_eq!(profile, expected);
    let mut proposed = Vec::new();
    visit_placement_boxes(
        &chunks,
        &journal,
        &properties,
        center,
        castle::BATTLEMENT_CORNER,
        0,
        |b| proposed.push(b),
    );
    assert_eq!(proposed, profile);
    let hit = raycast(
        &chunks,
        &journal,
        &properties,
        [15.90, 5.35, 13.0],
        [0.0, 0.0, 1.0],
        4.0,
    )
    .unwrap();
    assert_eq!(hit.position, center, "the derived arm must be pickable");
    assert!((hit.distance - 2.25).abs() < 1e-8);
    let mut replica = EditJournal::default();
    for p in [center, east, south] {
        replica
            .import_delta(journal.section_delta(global_to_section(p).0))
            .unwrap();
    }
    assert_eq!(collect(&chunks, &replica), profile);
    let owner = global_to_section(center).0;
    let epoch = journal.mesh_epoch(owner);
    install(&mut chunks, &mut journal, east, 0);
    assert!(
        journal.mesh_epoch(owner) > epoch,
        "editing across the seam must invalidate the connected neighbor"
    );
    replica
        .import_delta(journal.section_delta(global_to_section(east).0))
        .unwrap();
    let changed = collect(&chunks, &journal);
    assert_eq!(collect(&chunks, &replica), changed);
    assert_ne!(profile, changed);
    assert!(
        !changed
            .iter()
            .any(|b| b.intersects([15.89, 5.3, 15.4], [15.91, 5.4, 15.6])),
        "removed arm must not leave invisible collision"
    );
}

#[test]
fn pillar_stacks_cross_vertical_section_seams_without_interior_crowns_and_rebuild_on_removal() {
    use terra_core::castle::{self, connections};
    let mut chunks = ChunkStore::default();
    let mut journal = EditJournal::default();
    let bottom = [15, 15, 15];
    let middle = [15, 16, 15];
    let top = [15, 17, 15];
    let side = [16, 16, 15];
    for p in [bottom, middle, top, side] {
        install(&mut chunks, &mut journal, p, castle::BATTLEMENT_PILLAR);
    }
    let properties = BlockPropertiesCatalog::default();
    let collect = |journal: &EditJournal, chunks: &ChunkStore, p: [i64; 3]| {
        let mut result = Vec::new();
        assert!(visit_collision_boxes(
            chunks,
            journal,
            &properties,
            p,
            |b| result.push(b)
        ));
        result
    };
    let expected = |p: [i64; 3], key: u8| {
        connections::profile_boxes(castle::BATTLEMENT_PILLAR, key)
            .iter()
            .copied()
            .map(|mut b| {
                for (i, v) in p.into_iter().enumerate() {
                    b.minimum[i] += v as f64;
                    b.maximum[i] += v as f64;
                }
                b
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(
        collect(&journal, &chunks, bottom),
        expected(bottom, connections::STACK_ABOVE)
    );
    assert_eq!(
        collect(&journal, &chunks, middle),
        expected(
            middle,
            connections::STACK_ABOVE | connections::STACK_BELOW | 2
        )
    );
    assert_eq!(collect(&journal, &chunks, top), expected(top, 0));
    let mut ghost = Vec::new();
    visit_placement_boxes(
        &chunks,
        &journal,
        &properties,
        middle,
        castle::BATTLEMENT_PILLAR,
        0,
        |b| ghost.push(b),
    );
    assert_eq!(ghost, collect(&journal, &chunks, middle));
    let mut replica = EditJournal::default();
    for p in [bottom, middle, side] {
        replica
            .import_delta(journal.section_delta(global_to_section(p).0))
            .unwrap();
    }
    assert_eq!(
        collect(&replica, &chunks, middle),
        collect(&journal, &chunks, middle)
    );
    let below_owner = global_to_section(bottom).0;
    let before = journal.mesh_epoch(below_owner);
    install(&mut chunks, &mut journal, middle, 0);
    assert!(journal.mesh_epoch(below_owner) > before);
    replica
        .import_delta(journal.section_delta(global_to_section(middle).0))
        .unwrap();
    assert_eq!(collect(&replica, &chunks, bottom), expected(bottom, 0));
    assert_eq!(
        crate::placement::picked_item(&chunks, &journal, bottom),
        Some(terra_core::ItemKind::Block(castle::BATTLEMENT_PILLAR))
    );
    install(&mut chunks, &mut journal, bottom, castle::BATTLEMENT_CORNER);
    assert_eq!(
        crate::placement::picked_item(&chunks, &journal, bottom),
        Some(terra_core::ItemKind::Block(castle::BATTLEMENT_STRAIGHT))
    );
}
