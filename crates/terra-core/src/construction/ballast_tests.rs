use super::*;

fn contains(boxes: &[ShapeBox], point: [f64; 3]) -> bool {
    boxes.iter().any(|shape| {
        (0..3).all(|axis| point[axis] > shape.minimum[axis] && point[axis] < shape.maximum[axis])
    })
}
fn volume(boxes: &[ShapeBox]) -> f64 {
    boxes
        .iter()
        .map(|shape| {
            (0..3)
                .map(|axis| shape.maximum[axis] - shape.minimum[axis])
                .product::<f64>()
        })
        .sum()
}

#[test]
fn appended_ballast_slabs_preserve_every_historical_mixed_facing() {
    let historical = [82, 83, 84, 85, 86, 87, 88, 89, 105, 137];
    let locomotive_slabs = [271, 277, 283, 289];
    for first in historical
        .into_iter()
        .chain([BALLAST_SLAB])
        .chain(locomotive_slabs)
    {
        for (index, second) in historical.into_iter().enumerate() {
            for (half, facing) in HALF_FACINGS.into_iter().enumerate() {
                let saved = merged_slab(first, facing, second).unwrap();
                assert_eq!(saved, 10 + index as u8 * 6 + half as u8);
                let decoded = slab_halves(first, saved).unwrap();
                assert_eq!(decoded[0], (first, facing));
                assert_eq!(decoded[1].0, second);
            }
        }
        for (half, facing) in HALF_FACINGS.into_iter().enumerate() {
            let saved = merged_slab(first, facing, BALLAST_SLAB).unwrap();
            assert_eq!(saved, 70 + half as u8);
            assert_eq!(slab_halves(first, saved).unwrap()[1].0, BALLAST_SLAB);
            assert!(
                (volume(
                    &slab_parts(first, saved)
                        .into_iter()
                        .map(|(_, shape)| shape)
                        .collect::<Vec<_>>()
                ) - 1.0)
                    .abs()
                    < 1e-10
            );
        }
        for (index, second) in locomotive_slabs.into_iter().enumerate() {
            for (half, facing) in HALF_FACINGS.into_iter().enumerate() {
                let saved = merged_slab(first, facing, second).unwrap();
                assert_eq!(saved, 76 + index as u8 * 6 + half as u8);
                let decoded = slab_halves(first, saved).unwrap();
                assert_eq!(decoded[0], (first, facing));
                assert_eq!(decoded[1].0, second);
                assert!(valid_facing(first, saved));
            }
        }
    }
    assert_eq!(merged_slab(BALLAST_SLAB, 8, BALLAST_SLAB), Some(75));
    assert_eq!(LAST_MIXED_SLAB, 99);
    assert!(valid_facing(BALLAST_SLAB, 76));
    assert!(valid_facing(BALLAST_SLAB, 99));
    assert!(!valid_facing(BALLAST_SLAB, 100));
    assert_eq!(base_material(BALLAST_STAIRS), Some(BALLAST));
    assert_eq!(base_material(BALLAST_SLAB), Some(BALLAST));
    for material in [42, BALLAST, BALLAST_STAIRS, BALLAST_SLAB] {
        assert!(is_ballast(material));
    }
    for material in [3, 43, 85, 196, 209] {
        assert!(!is_ballast(material));
    }
}

#[test]
fn ballast_has_four_real_treads_in_every_pose_and_half_slabs_keep_six_planes() {
    assert_eq!(boxes(BALLAST_STAIRS).len(), 4);
    for facing in 0..24 {
        let profile = placed_boxes(BALLAST_STAIRS, facing, [None; 6]);
        assert_eq!(profile.len(), 4);
        assert!(profile.iter().all(|shape| shape.valid_for(BALLAST_STAIRS)));
        assert!((volume(&profile) - 0.625).abs() < 1e-10);
        for (index, first) in profile.iter().enumerate() {
            assert!(
                profile[index + 1..]
                    .iter()
                    .all(|other| !first.intersects(other.minimum, other.maximum))
            );
        }
    }
    let profile = placed_boxes(BALLAST_STAIRS, 0, [None; 6]);
    for tread in 0..4 {
        let z = (f64::from(tread) + 0.5) / 4.0;
        let top = f64::from(tread + 1) / 4.0;
        assert!(contains(&profile, [0.5, top - 0.01, z]));
        assert!(!contains(&profile, [0.5, top + 0.01, z]));
    }
    for facing in HALF_FACINGS {
        let profile = placed_boxes(BALLAST_SLAB, facing, [None; 6]);
        assert_eq!(profile, vec![slab_bounds(facing)]);
        assert_eq!(volume(&profile), 0.5);
    }
}

#[test]
fn mixed_three_and_four_tread_corners_match_the_actual_profile_union_or_intersection() {
    for (material, neighbor) in [
        (BALLAST_STAIRS, 77),
        (77, BALLAST_STAIRS),
        (BALLAST_STAIRS, BALLAST_STAIRS),
    ] {
        for facing in 0..24 {
            let neighbor_facing = facing / 4 * 4 + (facing + 1) % 4;
            let forward = stair_basis(facing)[2];
            let axis = (0..3).find(|&axis| forward[axis] != 0).unwrap();
            let own = placed_boxes(material, facing, [None; 6]);
            let other = placed_boxes(neighbor, neighbor_facing, [None; 6]);
            for outer in [false, true] {
                let mut neighbors = [None; 6];
                neighbors[axis * 2 + usize::from((forward[axis] > 0) == outer)] =
                    Some((neighbor, neighbor_facing));
                let profile = placed_boxes(material, facing, neighbors);
                assert!(profile.len() <= 144);
                assert!(profile.iter().all(|shape| shape.valid_for(material)));
                for x in 0..12 {
                    for y in 0..12 {
                        for z in 0..12 {
                            let point =
                                [x, y, z].map(|coordinate| (f64::from(coordinate) + 0.5) / 12.0);
                            let first = contains(&own, point);
                            let second = contains(&other, point);
                            assert_eq!(
                                contains(&profile, point),
                                if outer {
                                    first && second
                                } else {
                                    first || second
                                },
                                "{material}/{neighbor}/{facing}/{outer}: {point:?}"
                            );
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn perpendicular_ballast_and_legacy_stairs_union_their_real_treads_without_air_holes() {
    let own = placed_boxes(BALLAST_STAIRS, 0, [None; 6]);
    let other = placed_boxes(77, 8, [None; 6]);
    let profile = placed_boxes(
        BALLAST_STAIRS,
        0,
        [Some((77, 8)), None, None, None, None, None],
    );
    assert!(profile.len() <= 288);
    for x in 0..12 {
        for y in 0..12 {
            for z in 0..12 {
                let point = [x, y, z].map(|coordinate| (f64::from(coordinate) + 0.5) / 12.0);
                assert_eq!(
                    contains(&profile, point),
                    contains(&own, point) || contains(&other, point)
                );
            }
        }
    }
}
