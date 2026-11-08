use super::*;

#[test]
fn train_grates_preserve_real_collision_holes_when_rotated_and_combined_with_solid_slabs() {
    let position = [-17, 15, 31];
    let properties = BlockPropertiesCatalog::default();
    let mut cases = Vec::new();
    for material in 288..=293 {
        let poses = if material == 290 {
            24
        } else if material == 289 {
            10
        } else {
            4
        };
        cases.extend((0..poses).map(|facing| (material, facing)));
    }
    for (first, second) in [(82, 289), (289, 82)] {
        for facing in [0, 4, 5, 6, 7, 8] {
            cases.push((
                first,
                terra_core::construction::merged_slab(first, facing, second).unwrap(),
            ));
        }
    }
    for (material, facing) in cases {
        let mut journal = EditJournal::default();
        journal.record(position, material).unwrap();
        journal
            .set_object_state(
                position,
                Some(BlockObjectState {
                    facing,
                    ..Default::default()
                }),
            )
            .unwrap();
        let mut picking = Vec::new();
        let mut collision = Vec::new();
        let block_at = |p| Some(if p == position { material } else { 0 });
        assert!(visit_boxes_with(
            &journal,
            &properties,
            position,
            block_at,
            |bounds| picking.push(bounds)
        ));
        assert!(visit_collision_boxes_with(
            &journal,
            &properties,
            position,
            block_at,
            |bounds| collision.push(bounds)
        ));
        assert_eq!(picking, collision);
        let expected = terra_core::locomotive_parts::grate_boxes(material, facing).unwrap();
        assert_eq!(picking.len(), expected.len());
        for (actual, expected) in picking.iter().zip(expected) {
            for (axis, coordinate) in position.iter().enumerate() {
                assert!(
                    (actual.minimum[axis] - *coordinate as f64 - expected.minimum[axis]).abs()
                        < 1e-9
                );
                assert!(
                    (actual.maximum[axis] - *coordinate as f64 - expected.maximum[axis]).abs()
                        < 1e-9
                );
            }
        }
        if matches!(material, 288 | 289) && facing == 0 {
            let air =
                std::array::from_fn(|axis| position[axis] as f64 + [0.125, 0.25, 0.125][axis]);
            assert!(
                picking
                    .iter()
                    .all(|bounds| !bounds.intersects(air, air.map(|v| v + 0.01))),
                "a ray/body must be able to pass through a real grate opening"
            );
        }
    }
}

#[test]
fn funnel_legs_exist_only_above_a_loaded_full_floor_without_removing_its_neck() {
    let position = [8, 3, 8];
    let properties = BlockPropertiesCatalog::default();
    for facing in 0..4 {
        let mut journal = EditJournal::default();
        journal.record(position, 153).unwrap();
        journal
            .set_object_state(
                position,
                Some(crate::edits::BlockObjectState {
                    facing,
                    ..Default::default()
                }),
            )
            .unwrap();
        for floor in [None, Some(0), Some(148), Some(3)] {
            let mut boxes = Vec::new();
            assert!(visit_boxes_with(
                &journal,
                &properties,
                position,
                |p| {
                    if p == position {
                        Some(153)
                    } else if p == [8, 2, 8] {
                        floor
                    } else {
                        Some(0)
                    }
                },
                |b| boxes.push(b)
            ));
            assert!(boxes.iter().any(|b| (b.minimum[1] - 2.98).abs() < 0.00001));
            assert_eq!(
                boxes.iter().any(|b| b.minimum[1] <= 2.695),
                floor == Some(3)
            );
        }
    }
}
use crate::{edits::BlockObjectState, streaming::LoadedSection};
use terra_voxel::{BlockSection, global_to_section};

#[test]
fn unsupported_legacy_conveyors_have_no_invisible_frame_or_axle_boxes() {
    let origin = [-16, 0, -16];
    let floor = [-16, -1, -16];
    for facing in 0..4 {
        for support in [Some(0), Some(27), None, Some(4)] {
            let (mut chunks, mut journal, properties) = empty_world(origin);
            put(&mut chunks, &mut journal, origin, 148);
            if let Some(material) = support {
                put(&mut chunks, &mut journal, floor, material);
            } else {
                chunks.loaded.remove(&global_to_section(floor).0);
            }
            let raw =
                terra_core::kinetic::dyed_belt_facing(facing, terra_core::beds::DyeColor::Blue);
            journal
                .set_object_state(
                    origin,
                    Some(BlockObjectState {
                        facing: raw,
                        ..Default::default()
                    }),
                )
                .unwrap();
            let mut picking = Vec::new();
            let mut collision = Vec::new();
            assert!(visit_boxes(&chunks, &journal, &properties, origin, |b| {
                picking.push(b)
            }));
            assert!(visit_collision_boxes(
                &chunks,
                &journal,
                &properties,
                origin,
                |b| collision.push(b)
            ));
            assert_eq!(picking, collision);
            if support == Some(4) {
                let expected: Vec<_> = properties
                    .shape(148)
                    .iter()
                    .map(|source| {
                        let mut bounds = source.rotated_y(facing);
                        for (axis, value) in origin.into_iter().enumerate() {
                            bounds.minimum[axis] += value as f64;
                            bounds.maximum[axis] += value as f64;
                        }
                        bounds
                    })
                    .collect();
                assert_eq!(collision.len(), expected.len());
                for (actual, expected) in collision.iter().zip(&expected) {
                    for axis in 0..3 {
                        assert!((actual.minimum[axis] - expected.minimum[axis]).abs() < 1e-9);
                        assert!((actual.maximum[axis] - expected.maximum[axis]).abs() < 1e-9);
                    }
                }
            } else {
                assert_eq!(collision.len(), 1);
                assert!(
                    (collision[0].maximum[1] - f64::from(terra_core::kinetic::CONVEYOR_TOP)).abs()
                        < 1e-6
                );
                for local in [[0.08, 0.2, 0.5], [0.01, 0.5, 0.5]] {
                    let hidden = point(origin, local, facing, 1.0);
                    assert!(!collision.iter().any(|b| {
                        b.intersects(hidden.map(|v| v - 0.005), hidden.map(|v| v + 0.005))
                    }));
                }
            }
        }
    }
}

#[test]
fn belt_ports_keep_their_open_passage_and_negative_section_foot_overhang_in_all_modes() {
    let origin = [-16, 0, -16];
    for material in [152, 153] {
        for facing in 0..4 {
            for supported in [false, true] {
                let (mut chunks, mut journal, properties) = empty_world(origin);
                if supported {
                    put(
                        &mut chunks,
                        &mut journal,
                        [origin[0], origin[1] - 1, origin[2]],
                        3,
                    );
                }
                let forward = terra_core::kinetic::forward(facing);
                let peer = std::array::from_fn(|axis| {
                    origin[axis] + forward[axis] * if material == 152 { -1 } else { 1 }
                });
                put(&mut chunks, &mut journal, peer, 29);
                put(&mut chunks, &mut journal, origin, material);
                let mut geometric = Vec::new();
                for mode in [0, 4] {
                    let state = BlockObjectState {
                        facing: facing + mode,
                        peer: Some(peer),
                        ..Default::default()
                    };
                    journal.set_object_state(origin, Some(state)).unwrap();
                    let mut actual = Vec::new();
                    assert!(visit_boxes(&chunks, &journal, &properties, origin, |b| {
                        actual.push(b)
                    }));
                    let has_feet = material == 152 || supported;
                    assert_eq!(
                        actual.iter().any(|b| (b.minimum[1] + 0.375).abs() < 1e-6),
                        has_feet
                    );
                    if mode == 0 {
                        geometric = actual.clone();
                    } else {
                        assert_eq!(actual, geometric);
                    }
                    let gap = point(origin, [0.5, -0.22, 0.5], facing, 1.0);
                    assert!(
                        !actual
                            .iter()
                            .any(|b| b.intersects(gap.map(|v| v - 0.10), gap.map(|v| v + 0.10)))
                    );
                    let foot = point(
                        origin,
                        if material == 152 {
                            [0.13, -0.34, 0.12]
                        } else {
                            [0.14, -0.34, 0.14]
                        },
                        facing,
                        1.0,
                    );
                    let eye = [foot[0], foot[1], foot[2] - 0.06];
                    if !supported {
                        let hit =
                            raycast(&chunks, &journal, &properties, eye, [0.0, 0.0, 1.0], 0.15);
                        assert_eq!(
                            hit.is_some_and(|hit| hit.position == origin),
                            has_feet,
                            "only real unsupported tunnel feet remain pickable below the negative-Y seam"
                        );
                    }
                    let mut proposed = Vec::new();
                    visit_proposed_boxes(
                        &chunks,
                        &journal,
                        &properties,
                        origin,
                        material,
                        state,
                        |b| proposed.push(b),
                    );
                    assert_eq!(
                        proposed, actual,
                        "clearance uses the same mode-independent shell"
                    );
                }
            }
        }
    }
}

#[test]
fn generic_crank_collision_and_picking_share_the_centered_mount_pose_in_six_axes() {
    let origin = [-16, 4, -16];
    for facing in 0..6 {
        for distance in [1, 2] {
            let axis = terra_core::kinetic::axis(facing);
            let peer = std::array::from_fn(|i| origin[i] - axis[i] * distance);
            let state = BlockObjectState {
                facing,
                peer: Some(peer),
                ..Default::default()
            };
            let (mut chunks, mut journal, properties) = empty_world(origin);
            put(&mut chunks, &mut journal, origin, 100);
            journal.set_object_state(origin, Some(state)).unwrap();
            let mut actual = Vec::new();
            assert!(visit_boxes(
                &chunks,
                &journal,
                &properties,
                origin,
                |bounds| actual.push(bounds)
            ));
            let offset = [0.0, 0.045_f32, -0.015_f32];
            let expected: Vec<_> = properties
                .shape(100)
                .iter()
                .map(|source| {
                    let mut local = *source;
                    for (i, displacement) in offset.into_iter().enumerate() {
                        local.minimum[i] += f64::from(displacement);
                        local.maximum[i] += f64::from(displacement);
                    }
                    let mut world = local.rotated_axis(facing);
                    for (i, coordinate) in origin.into_iter().enumerate() {
                        world.minimum[i] += coordinate as f64;
                        world.maximum[i] += coordinate as f64;
                    }
                    world
                })
                .collect();
            assert_eq!(actual, expected);
            let mut proposed = Vec::new();
            assert!(visit_proposed_boxes(
                &chunks,
                &journal,
                &properties,
                origin,
                100,
                state,
                |bounds| proposed.push(bounds)
            ));
            assert_eq!(proposed, actual);
            let eye = std::array::from_fn(|i| {
                (actual[0].minimum[i] + actual[0].maximum[i]) * 0.5 + axis[i] as f64 * 2.0
            });
            let direction = axis.map(|value| -(value as f64));
            let distance = actual
                .iter()
                .filter_map(|bounds| bounds.ray(eye, direction, 3.0))
                .map(|(distance, _)| distance)
                .min_by(f64::total_cmp)
                .unwrap();
            let hit = raycast(&chunks, &journal, &properties, eye, direction, 3.0).unwrap();
            assert_eq!(hit.position, origin);
            assert!((hit.distance - distance).abs() < 1.0e-8);
        }
    }
}

#[test]
fn chimney_roof_seal_collision_matches_the_partial_slab_height() {
    let origin = [-16, 15, -16];
    for pose in [0, 4] {
        let (mut chunks, mut journal, properties) = empty_world(origin);
        put(&mut chunks, &mut journal, origin, 60);
        for offset in [[-1, 0, 0], [1, 0, 0], [0, 0, -1]] {
            let neighbor = std::array::from_fn(|axis| origin[axis] + offset[axis]);
            put(&mut chunks, &mut journal, neighbor, 85);
            journal
                .set_object_state(
                    neighbor,
                    Some(BlockObjectState {
                        facing: pose,
                        ..Default::default()
                    }),
                )
                .unwrap();
        }
        let mut boxes = Vec::new();
        assert!(visit_boxes(
            &chunks,
            &journal,
            &properties,
            origin,
            |bounds| boxes.push(bounds)
        ));
        for (height, filled) in [(0.25, pose == 0), (0.75, pose == 4)] {
            let point = [
                origin[0] as f64 + 0.1,
                origin[1] as f64 + height,
                origin[2] as f64 + 0.5,
            ];
            assert_eq!(
                boxes
                    .iter()
                    .any(|bounds| bounds.intersects(point, point.map(|value| value + 0.001))),
                filled
            );
        }
    }
}

#[test]
fn dyed_beds_share_the_exact_head_and_foot_picking_volumes_in_every_orientation() {
    for material in 71..=73 {
        for facing in 0..4 {
            let origin = [-16, 15, -16];
            let (mut chunks, mut journal, properties) = empty_world(origin);
            let direction = rotate([0.0, 0.0, 1.0], facing).map(|v| v as i64);
            let peer = std::array::from_fn(|axis| origin[axis] + direction[axis]);
            for (cell, other, secondary) in [(origin, peer, false), (peer, origin, true)] {
                put(&mut chunks, &mut journal, cell, material);
                journal
                    .set_object_state(
                        cell,
                        Some(BlockObjectState {
                            facing: terra_core::beds::dyed_facing(
                                facing,
                                terra_core::beds::DyeColor::Orange,
                            ),
                            peer: Some(other),
                            secondary,
                            ..Default::default()
                        }),
                    )
                    .unwrap();
            }
            let mut foot = Vec::new();
            let mut head = Vec::new();
            assert!(visit_boxes(&chunks, &journal, &properties, origin, |b| {
                foot.push(b)
            }));
            assert!(visit_boxes(&chunks, &journal, &properties, peer, |b| head.push(b)));
            assert_eq!(foot, head);
            for z in [0.5, 1.6] {
                let eye = point(origin, [0.5, 2.0, z], facing, 1.0);
                let hit =
                    raycast(&chunks, &journal, &properties, eye, [0.0, -1.0, 0.0], 3.0).unwrap();
                assert!(hit.position == origin || hit.position == peer);
                let distance = foot
                    .iter()
                    .filter_map(|b| b.ray(eye, [0.0, -1.0, 0.0], 3.0))
                    .map(|(distance, _)| distance)
                    .min_by(f64::total_cmp)
                    .unwrap();
                assert!((hit.distance - distance).abs() < 1.0e-8);
            }
        }
    }
}

#[test]
fn construction_collision_rotates_stairs_and_connects_only_present_fence_arms() {
    let origin = [-16, 15, -16];
    for facing in 0..4 {
        let (mut chunks, mut journal, properties) = empty_world(origin);
        put(&mut chunks, &mut journal, origin, 90);
        journal
            .set_object_state(
                origin,
                Some(BlockObjectState {
                    facing,
                    ..Default::default()
                }),
            )
            .unwrap();
        let mut alone = Vec::new();
        visit_boxes(&chunks, &journal, &properties, origin, |shape| {
            alone.push(shape)
        });
        assert_eq!(alone.len(), 3, "isolated fence only has its post");
        let side = terra_core::construction::fence_neighbor_side(0, facing);
        let mut neighbor = origin;
        neighbor[side / 2] += if side.is_multiple_of(2) { -1 } else { 1 };
        put(&mut chunks, &mut journal, neighbor, 90);
        let mut connected = Vec::new();
        visit_boxes(&chunks, &journal, &properties, origin, |shape| {
            connected.push(shape)
        });
        assert_eq!(connected.len(), 5, "one neighboring post adds two rails");
        put(&mut chunks, &mut journal, origin, 74);
        journal
            .set_object_state(
                origin,
                Some(BlockObjectState {
                    facing,
                    ..Default::default()
                }),
            )
            .unwrap();
        let mut stairs = Vec::new();
        visit_boxes(&chunks, &journal, &properties, origin, |shape| {
            stairs.push(shape)
        });
        assert_eq!(stairs.len(), 3);
        assert!((stairs[0].maximum[1] - stairs[0].minimum[1] - 1.0 / 3.0).abs() < 0.0001);
    }
}

#[test]
fn open_doors_have_ground_level_player_clearance_without_auto_climbing() {
    // This conservative swept box includes 2 cm beyond the actual capsule on
    // every side. A fixed Y proves walking needs neither a jump nor auto-climb.
    let half = [0.34, 0.92, 0.34];
    for material in 65..=67 {
        for facing in 0..4 {
            let origin = [-16, 15, -16];
            let peer = [-16, 16, -16];
            let (mut chunks, mut journal, properties) = empty_world(origin);
            for (cell, other, secondary) in [(origin, peer, false), (peer, origin, true)] {
                put(&mut chunks, &mut journal, cell, material);
                journal
                    .set_object_state(
                        cell,
                        Some(BlockObjectState {
                            facing: facing + 4,
                            peer: Some(other),
                            secondary,
                            ..Default::default()
                        }),
                    )
                    .unwrap();
            }
            let mut boxes = Vec::new();
            visit_boxes(&chunks, &journal, &properties, origin, |b| boxes.push(b));
            for step in 0..=100 {
                let center = point(
                    origin,
                    [0.5175, half[1], -0.5 + f64::from(step) * 0.025],
                    facing,
                    1.0,
                );
                let minimum = std::array::from_fn(|axis| center[axis] - half[axis]);
                let maximum = std::array::from_fn(|axis| center[axis] + half[axis]);
                assert!(
                    boxes
                        .iter()
                        .all(|bounds| !bounds.intersects(minimum, maximum)),
                    "Door {material}, facing {facing}, walking step {step} blocks the player"
                );
            }
        }
    }
}

#[test]
fn opening_frames_stay_fixed_while_all_species_leaves_clear_the_passage() {
    for material in 65..=70 {
        for facing in 0..4 {
            let origin = [-16, 15, -16];
            let (mut chunks, mut journal, properties) = empty_world(origin);
            put(&mut chunks, &mut journal, origin, material);
            let peer = (material <= 67).then_some([origin[0], origin[1] + 1, origin[2]]);
            let closed = BlockObjectState {
                facing,
                peer,
                ..Default::default()
            };
            journal.set_object_state(origin, Some(closed)).unwrap();
            if let Some(peer) = peer {
                put(&mut chunks, &mut journal, peer, material);
                journal
                    .set_object_state(
                        peer,
                        Some(BlockObjectState {
                            peer: Some(origin),
                            secondary: true,
                            ..closed
                        }),
                    )
                    .unwrap();
            }
            let mut closed_boxes = Vec::new();
            visit_boxes(&chunks, &journal, &properties, origin, |b| {
                closed_boxes.push(b)
            });
            let eye = point(origin, [0.5, 0.5, 0.95], facing, 1.0);
            let direction = rotate([0.0, 0.0, -1.0], facing);
            assert!(raycast(&chunks, &journal, &properties, eye, direction, 1.5).is_some());
            let opened = BlockObjectState {
                facing: facing + 4,
                ..closed
            };
            journal.set_object_state(origin, Some(opened)).unwrap();
            if let Some(peer) = peer {
                journal
                    .set_object_state(
                        peer,
                        Some(BlockObjectState {
                            peer: Some(origin),
                            secondary: true,
                            ..opened
                        }),
                    )
                    .unwrap();
            }
            let mut open_boxes = Vec::new();
            visit_boxes(&chunks, &journal, &properties, origin, |b| {
                open_boxes.push(b)
            });
            assert_eq!(closed_boxes.len(), open_boxes.len());
            for ((source, a), b) in properties
                .shape(material)
                .iter()
                .zip(&closed_boxes)
                .zip(&open_boxes)
            {
                if source.hinged {
                    assert_ne!(a, b);
                } else {
                    assert_eq!(a, b);
                }
            }
            assert!(
                raycast(&chunks, &journal, &properties, eye, direction, 1.5).is_none(),
                "open {material}, facing {facing} must leave the center clear"
            );
            if let Some(peer) = peer {
                let mut upper = Vec::new();
                visit_boxes(&chunks, &journal, &properties, peer, |b| upper.push(b));
                assert_eq!(
                    upper, open_boxes,
                    "upper section must resolve the same hinge"
                );
            }
            let eye = point(origin, [0.5, 0.5, 1.10], facing, 1.0);
            let direction = rotate([-1.0, 0.0, 0.0], facing);
            assert!(
                raycast(&chunks, &journal, &properties, eye, direction, 0.8).is_some(),
                "open leaf tip outside its cell must remain selectable"
            );
        }
    }
}

fn empty_world(origin: [i64; 3]) -> (ChunkStore, EditJournal, BlockPropertiesCatalog) {
    let mut chunks = ChunkStore::default();
    let center = global_to_section(origin).0;
    for dz in -1..=1 {
        for dy in -1..=1 {
            for dx in -1..=1 {
                let position = center.offset([dx, dy, dz]).unwrap();
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
    (
        chunks,
        EditJournal::default(),
        BlockPropertiesCatalog::default(),
    )
}

#[test]
fn protruding_mineral_facets_pick_the_owner_across_air_and_section_boundaries() {
    let origin = [-16, -1, -16];
    for ore in terra_core::OreKind::ALL {
        let (mut chunks, mut journal, properties) = empty_world(origin);
        put(&mut chunks, &mut journal, origin, ore.material());
        let mut point = [0.0_f64; 3];
        crate::ore_shapes::visit_face(
            ore,
            origin,
            3,
            crate::ore_shapes::OreDetail::Near,
            |triangle| {
                let center = std::array::from_fn(|axis| {
                    triangle.iter().map(|v| f64::from(v[axis])).sum::<f64>() / 3.0
                });
                if center[1] > point[1] {
                    point = center;
                }
            },
        );
        assert!(point[1] > 1.01);
        let eye = std::array::from_fn(|axis| {
            origin[axis] as f64 + point[axis] + if axis == 2 { 1.5 } else { 0.0 }
        });
        let hit = raycast(&chunks, &journal, &properties, eye, [0.0, 0.0, -1.0], 3.0).unwrap();
        assert_eq!(hit.position, origin);
        assert!(hit.distance > 1.0 && hit.distance < 2.0);
        // The carrier alone retains ordinary, closed full-block collision.
        assert!(space_intersects(
            &chunks,
            &journal,
            &properties,
            origin,
            origin.map(|v| v as f64 + 0.25),
            origin.map(|v| v as f64 + 0.75)
        ));
        let mut cover = origin;
        cover[1] += 1;
        put(&mut chunks, &mut journal, cover, 3);
        let covered = raycast(&chunks, &journal, &properties, eye, [0.0, 0.0, -1.0], 3.0).unwrap();
        assert_eq!(
            covered.position, cover,
            "covered crystals must not remain pickable"
        );
    }
}
fn put(chunks: &mut ChunkStore, journal: &mut EditJournal, position: [i64; 3], material: u16) {
    let (section, cell) = global_to_section(position);
    chunks
        .loaded
        .get_mut(&section)
        .unwrap()
        .data
        .set_block(cell, material)
        .unwrap();
    journal.record(position, material).unwrap();
}

#[test]
fn long_salt_tip_picks_its_carrier_and_disappears_when_clearance_is_blocked() {
    for side in [2, 3] {
        let (origin, height) = (-48..48)
            .find_map(|x| {
                let p = [x, 16, -16];
                let h = crate::ore_shapes::salt_growth_height(p, side, |_| Some(0));
                (h > if side == 2 { 8.0 } else { 3.0 }).then_some((p, h))
            })
            .unwrap();
        let (mut chunks, mut journal, properties) = empty_world(origin);
        put(&mut chunks, &mut journal, origin, 30);
        let mut point = [0.0; 3];
        let mut furthest = 0.0_f64;
        crate::ore_shapes::visit_shaded_face_with_growth(
            30,
            origin,
            side,
            crate::ore_shapes::OreDetail::Near,
            height,
            |triangle, _| {
                let center = std::array::from_fn(|axis| {
                    triangle.iter().map(|v| f64::from(v[axis])).sum::<f64>() / 3.0
                });
                let distance = if side == 2 {
                    -center[1]
                } else {
                    center[1] - 1.0
                };
                if distance > furthest {
                    furthest = distance;
                    point = center;
                }
            },
        );
        assert!(furthest > 2.0);
        let eye = std::array::from_fn(|axis| {
            origin[axis] as f64 + point[axis] + if axis == 2 { 1.5 } else { 0.0 }
        });
        let hit = raycast(&chunks, &journal, &properties, eye, [0.0, 0.0, -1.0], 3.0).unwrap();
        assert_eq!(hit.position, origin);
        let blocker = [
            origin[0],
            origin[1] + if side == 2 { -2 } else { 2 },
            origin[2],
        ];
        put(&mut chunks, &mut journal, blocker, 3);
        assert_eq!(
            crate::ore_shapes::salt_growth_height(origin, side, |p| chunks.block_at(p)),
            0.0
        );
        assert!(raycast(&chunks, &journal, &properties, eye, [0.0, 0.0, -1.0], 3.0).is_none());
    }
}
fn furniture(
    chunks: &mut ChunkStore,
    journal: &mut EditJournal,
    origin: [i64; 3],
    material: u16,
    facing: u8,
) -> [i64; 3] {
    let state = BlockObjectState {
        facing,
        ..Default::default()
    };
    let second = std::array::from_fn(|axis| origin[axis] + state.right()[axis]);
    put(chunks, journal, origin, material);
    journal
        .set_object_state(
            origin,
            Some(BlockObjectState {
                peer: (material == 28).then_some(second),
                ..state
            }),
        )
        .unwrap();
    if material == 28 {
        put(chunks, journal, second, material);
        journal
            .set_object_state(
                second,
                Some(BlockObjectState {
                    peer: Some(origin),
                    secondary: true,
                    ..state
                }),
            )
            .unwrap();
    }
    second
}
fn rotate([x, y, z]: [f64; 3], facing: u8) -> [f64; 3] {
    match facing {
        0 => [x, y, z],
        1 => [z, y, -x],
        2 => [-x, y, -z],
        _ => [-z, y, x],
    }
}
fn point(origin: [i64; 3], source: [f64; 3], facing: u8, width: f64) -> [f64; 3] {
    let value = rotate(
        [source[0] * width - 0.5, source[1], source[2] - 0.5],
        facing,
    );
    std::array::from_fn(|axis| {
        origin[axis] as f64 + value[axis] + if axis == 1 { 0.0 } else { 0.5 }
    })
}

#[test]
fn workbench_boxes_share_exact_footprint_for_all_facings_and_negative_section_secondary_cells() {
    for facing in 0..4 {
        let origin = [-16, 4, -16];
        let (mut chunks, mut journal, properties) = empty_world(origin);
        let second = furniture(&mut chunks, &mut journal, origin, 28, facing);
        let mut primary_boxes = Vec::new();
        let mut secondary_boxes = Vec::new();
        assert!(visit_boxes(&chunks, &journal, &properties, origin, |b| {
            primary_boxes.push(b)
        }));
        assert!(visit_boxes(&chunks, &journal, &properties, second, |b| {
            secondary_boxes.push(b)
        }));
        assert_eq!(primary_boxes, secondary_boxes);
        assert_eq!(primary_boxes.len(), properties.shape(28).len());
        for (source, transformed) in properties.shape(28).iter().zip(&primary_boxes) {
            for corner in 0..8 {
                let local = std::array::from_fn(|axis| {
                    if corner & (1 << axis) == 0 {
                        source.minimum[axis]
                    } else {
                        source.maximum[axis]
                    }
                });
                let world = point(origin, local, facing, 2.0);
                for (axis, value) in world.into_iter().enumerate() {
                    assert!(value >= transformed.minimum[axis] - 1.0e-10);
                    assert!(value <= transformed.maximum[axis] + 1.0e-10);
                }
            }
        }
        let origin_ray = point(origin, [0.8, 2.0, 0.5], facing, 2.0);
        let hit = raycast(
            &chunks,
            &journal,
            &properties,
            origin_ray,
            [0.0, -1.0, 0.0],
            4.0,
        )
        .unwrap();
        assert_eq!(hit.position, second, "facing {facing}");
        assert_eq!(hit.normal, [0, 1, 0]);
        assert!(hit.distance > 1.0 && hit.distance < 1.5);
    }
}

#[test]
fn rays_and_aabbs_pass_through_workbench_gaps_but_hit_table_or_a_solid_block_behind() {
    for facing in 0..4 {
        let origin = [4, 4, 4];
        let (mut chunks, mut journal, properties) = empty_world(origin);
        let second = furniture(&mut chunks, &mut journal, origin, 28, facing);
        let ray_origin = point(origin, [0.53, 0.52, 2.5], facing, 2.0);
        let direction = rotate([0.0, 0.0, -1.0], facing);
        assert!(
            raycast(&chunks, &journal, &properties, ray_origin, direction, 4.0).is_none(),
            "facing {facing}"
        );
        let gap = point(origin, [0.53, 0.52, 0.5], facing, 2.0);
        assert!(!space_intersects(
            &chunks,
            &journal,
            &properties,
            second,
            gap.map(|v| v - 0.01),
            gap.map(|v| v + 0.01)
        ));
        let table = point(origin, [0.53, 0.70, 0.5], facing, 2.0);
        assert!(space_intersects(
            &chunks,
            &journal,
            &properties,
            second,
            table.map(|v| v - 0.01),
            table.map(|v| v + 0.01)
        ));
        let table_ray = point(origin, [0.53, 0.70, 2.5], facing, 2.0);
        let table_hit = raycast(&chunks, &journal, &properties, table_ray, direction, 4.0).unwrap();
        assert!(table_hit.distance > 1.5 && table_hit.distance < 1.7);
        let behind = point(origin, [0.53, 0.52, -0.5], facing, 2.0).map(|v| v.floor() as i64);
        put(&mut chunks, &mut journal, behind, 3);
        let hit = raycast(&chunks, &journal, &properties, ray_origin, direction, 4.0).unwrap();
        assert_eq!(hit.position, behind);
        assert_eq!(
            hit.normal,
            rotate([0.0, 0.0, 1.0], facing).map(|v| v as i32)
        );
    }
}

#[test]
fn chest_rounded_lid_and_inset_body_leave_real_air_instead_of_a_full_cell_box() {
    let origin = [4, 4, 4];
    let (mut chunks, mut journal, properties) = empty_world(origin);
    furniture(&mut chunks, &mut journal, origin, 29, 0);
    for (local, expected) in [
        ([0.5, 0.78, 0.15], false),
        ([0.5, 0.4, 0.15], true),
        ([0.02, 0.4, 0.5], false),
    ] {
        let point = point(origin, local, 0, 1.0);
        assert_eq!(
            space_intersects(
                &chunks,
                &journal,
                &properties,
                origin,
                point.map(|v| v - 0.005),
                point.map(|v| v + 0.005)
            ),
            expected
        );
    }
    assert!(
        raycast(
            &chunks,
            &journal,
            &properties,
            point(origin, [0.5, 0.85, 2.0], 0, 1.0),
            [0.0, 0.0, -1.0],
            3.0
        )
        .is_none()
    );
    let body = raycast(
        &chunks,
        &journal,
        &properties,
        point(origin, [0.5, 0.4, 2.0], 0, 1.0),
        [0.0, 0.0, -1.0],
        3.0,
    )
    .unwrap();
    assert_eq!(body.position, origin);
    assert!(body.distance > 1.0 && body.distance < 1.2);
}

#[test]
fn ordinary_cubes_keep_complete_cell_picking_and_volume() {
    let origin = [4, 4, 4];
    let (mut chunks, mut journal, properties) = empty_world(origin);
    put(&mut chunks, &mut journal, origin, 3);
    assert!(!visit_boxes(
        &chunks,
        &journal,
        &properties,
        origin,
        |_| panic!("Cube has no custom boxes")
    ));
    let hit = raycast(
        &chunks,
        &journal,
        &properties,
        [4.02, 4.02, 6.0],
        [0.0, 0.0, -1.0],
        4.0,
    )
    .unwrap();
    assert_eq!(hit.position, origin);
    assert_eq!(hit.distance, 1.0);
    assert_eq!(hit.normal, [0, 0, 1]);
    assert!(space_intersects(
        &chunks,
        &journal,
        &properties,
        origin,
        [4.01; 3],
        [4.02; 3]
    ));
}

#[test]
fn water_wheel_rim_stays_pickable_but_only_its_axle_blocks_bodies() {
    let origin = [-16, 4, -16];
    for facing in 0..6 {
        let (mut chunks, mut journal, properties) = empty_world(origin);
        put(
            &mut chunks,
            &mut journal,
            origin,
            terra_core::kinetic::WATER_WHEEL,
        );
        journal
            .set_object_state(
                origin,
                Some(BlockObjectState {
                    facing,
                    ..Default::default()
                }),
            )
            .unwrap();
        let mut picking = Vec::new();
        let mut collision = Vec::new();
        assert!(visit_boxes(&chunks, &journal, &properties, origin, |b| {
            picking.push(b)
        }));
        assert!(visit_collision_boxes(
            &chunks,
            &journal,
            &properties,
            origin,
            |b| collision.push(b)
        ));
        assert_eq!(picking.len(), properties.shape(147).len());
        assert_eq!(collision.len(), 1);
        for (local, blocked) in [([0.5, 1.5, 0.5], false), ([0.5; 3], true)] {
            let mut probe = ShapeBox {
                minimum: local.map(|v| v - 0.01),
                maximum: local.map(|v| v + 0.01),
                hinged: false,
            }
            .rotated_axis(facing);
            for (axis, value) in origin.iter().enumerate() {
                probe.minimum[axis] += *value as f64;
                probe.maximum[axis] += *value as f64;
            }
            assert!(
                picking
                    .iter()
                    .any(|b| b.intersects(probe.minimum, probe.maximum))
            );
            assert_eq!(
                space_intersects(
                    &chunks,
                    &journal,
                    &properties,
                    origin,
                    probe.minimum,
                    probe.maximum,
                ),
                blocked,
                "facing={facing}, local={local:?}"
            );
            let axis = terra_core::kinetic::axis(facing);
            let eye = std::array::from_fn(|i| {
                (probe.minimum[i] + probe.maximum[i]) * 0.5 + axis[i] as f64 * 2.0
            });
            let hit = raycast(
                &chunks,
                &journal,
                &properties,
                eye,
                axis.map(|v| -(v as f64)),
                3.0,
            )
            .unwrap();
            assert_eq!(
                hit.position, origin,
                "the full wheel must remain selectable"
            );
        }
    }
}
