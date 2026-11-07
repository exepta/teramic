use super::*;

fn face(axis: usize, height: f32) -> ([[f32; 3]; 4], [f32; 3], [usize; 2]) {
    let tangents: Vec<_> = (0..3).filter(|&i| i != axis).collect();
    let mut normal = [0.; 3];
    normal[axis] = 1.;
    let mut points = [[0., 0.], [0., 1.], [1., 1.], [1., 0.]].map(|v| {
        let mut p = [0.; 3];
        p[axis] = height;
        p[tangents[0]] = v[0];
        p[tangents[1]] = v[1];
        p
    });
    if axis != 1 {
        points.reverse();
    }
    (points, normal, [tangents[0], tangents[1]])
}

fn area(polygons: &[Vec<[f32; 3]>], tangents: [usize; 2]) -> f64 {
    polygons
        .iter()
        .map(|polygon| {
            let p = |i: usize| {
                DVec2::new(
                    f64::from(polygon[i][tangents[0]]),
                    f64::from(polygon[i][tangents[1]]),
                )
            };
            (1..polygon.len() - 1)
                .map(|i| (p(i) - p(0)).perp_dot(p(i + 1) - p(0)).abs() * 0.5)
                .sum::<f64>()
        })
        .sum()
}

fn coverage(polygons: &[Vec<[f32; 3]>], tangents: [usize; 2], point: DVec2) -> usize {
    polygons
        .iter()
        .filter(|polygon| {
            let p = |i: usize| {
                DVec2::new(
                    f64::from(polygon[i][tangents[0]]),
                    f64::from(polygon[i][tangents[1]]),
                )
            };
            let sides: Vec<_> = (0..polygon.len())
                .map(|i| (p((i + 1) % polygon.len()) - p(i)).perp_dot(point - p(i)))
                .collect();
            sides.iter().all(|d| *d >= -1e-8) || sides.iter().all(|d| *d <= 1e-8)
        })
        .count()
}

#[test]
fn all_straight_rotations_follow_the_round_wall_and_keep_collision_box_corners_wet() {
    for face_axis in 0..3 {
        let (face, normal, tangents) = face(face_axis, 0.5);
        for pipe_axis in 0..3 {
            let clipped = fragments(
                vec![face.to_vec()],
                face,
                normal,
                [0.; 3],
                3 << (pipe_axis * 2),
                true,
            );
            let expected = if face_axis == pipe_axis {
                1. - SIDES as f64 * 0.5 * RADIUS.powi(2) * (TAU / SIDES as f64).sin()
            } else {
                1. - (0.3 + 2. * 0.07 * (0.43 - 0.3) + 2. * 0.05 * (0.326 - 0.3))
            };
            assert!(
                (area(&clipped, tangents) - expected).abs() < 1e-5,
                "face={face_axis} pipe={pipe_axis} area={} expected={expected}",
                area(&clipped, tangents)
            );
            if pipe_axis == face_axis {
                assert_eq!(
                    coverage(&clipped, tangents, DVec2::new(0.637, 0.641)),
                    1,
                    "the square collision corner must remain covered by lake water"
                );
                assert_eq!(coverage(&clipped, tangents, DVec2::new(0.501, 0.503)), 0);
            }
        }
    }
}

#[test]
fn support_ring_cuts_follow_the_authored_straight_arms_instead_of_the_old_bend_overlap() {
    let (face, normal, tangents) = face(1, 0.5);
    let clipped = fragments(vec![face.to_vec()], face, normal, [0.; 3], 3, true);
    for x in [0.225, 0.775] {
        assert_eq!(coverage(&clipped, tangents, DVec2::new(x, 0.66)), 0);
    }
    for x in [0.315, 0.685] {
        assert_eq!(coverage(&clipped, tangents, DVec2::new(x, 0.66)), 1);
    }
}

#[test]
fn a_dry_open_mouth_removes_only_its_annular_wall_without_a_square_or_bore_hole() {
    for axis in 0..3 {
        for plane in [0., 1.] {
            let (face, normal, tangents) = face(axis, plane);
            let clipped = fragments(
                vec![face.to_vec()],
                face,
                normal,
                [0.; 3],
                3 << (axis * 2),
                false,
            );
            let ring_area =
                SIDES as f64 * 0.5 * (RADIUS.powi(2) - BORE.powi(2)) * (TAU / SIDES as f64).sin();
            assert!((area(&clipped, tangents) - (1. - ring_area)).abs() < 1e-5);
            assert_eq!(
                coverage(&clipped, tangents, DVec2::new(0.501, 0.503)),
                1,
                "water must remain visible through the open intake bore"
            );
            assert_eq!(coverage(&clipped, tangents, DVec2::new(0.637, 0.641)), 1);
            assert_eq!(coverage(&clipped, tangents, DVec2::new(0.634, 0.502)), 0);
        }
    }
}

#[test]
fn all_bends_and_manifolds_have_single_wound_finite_faces_at_positive_and_negative_origins() {
    for mask in 1u8..64 {
        if mask.count_ones() < 2 {
            continue;
        }
        for axis in 0..3 {
            for plane in [0., 0.045, 0.125, 0.3, 0.5, 0.7, 0.955, 1.] {
                for offset in [[0.; 3], [-16.; 3], [15.; 3]] {
                    let (mut face, normal, tangents) = face(axis, plane);
                    for p in &mut face {
                        for i in 0..3 {
                            p[i] += offset[i];
                        }
                    }
                    let clipped = fragments(vec![face.to_vec()], face, normal, offset, mask, true);
                    let a = area(&clipped, tangents);
                    assert!(
                        a > 0. && a <= 1.00001,
                        "mask={mask} axis={axis} plane={plane} offset={offset:?} area={a}"
                    );
                    for polygon in &clipped {
                        assert!(polygon.iter().flatten().all(|v| v.is_finite()));
                        let anchor = DVec3::from_array(polygon[0].map(f64::from));
                        for i in 1..polygon.len() - 1 {
                            let b = DVec3::from_array(polygon[i].map(f64::from));
                            let c = DVec3::from_array(polygon[i + 1].map(f64::from));
                            assert!(
                                (b - anchor)
                                    .cross(c - anchor)
                                    .dot(DVec3::from_array(normal.map(f64::from)))
                                    > 0.,
                                "empty or reversed triangle mask={mask} axis={axis} plane={plane} offset={offset:?} {polygon:?}"
                            );
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn faceted_manifold_dents_remain_wet_instead_of_inheriting_a_convex_hull_gap() {
    let (face, normal, tangents) = face(1, 0.57982);
    let clipped = fragments(vec![face.to_vec()], face, normal, [0.; 3], 25, true);
    assert_eq!(
        coverage(&clipped, tangents, DVec2::new(0.6181625, 0.6748)),
        1
    );
    assert_eq!(coverage(&clipped, tangents, DVec2::new(0.501, 0.503)), 0);
}
