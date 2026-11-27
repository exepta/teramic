use super::*;
use crate::creatures::model::{Part, parts_variant};
use bevy::mesh::VertexAttributeValues;
use terra_core::creatures::CreatureKind as Kind;

fn points(mesh: &Mesh) -> &[[f32; 3]] {
    let Some(VertexAttributeValues::Float32x3(values)) = mesh.attribute(Mesh::ATTRIBUTE_POSITION)
    else {
        panic!("positions");
    };
    values
}

fn ray_hits_before(mesh: &Mesh, origin: Vec3, direction: Vec3, distance: f32) -> bool {
    ray_hits_before_filter(mesh, origin, direction, distance, |_| true)
}

fn ray_hits_before_filter(
    mesh: &Mesh,
    origin: Vec3,
    direction: Vec3,
    distance: f32,
    include_vertex: impl Fn(usize) -> bool,
) -> bool {
    let p = points(mesh);
    let ids: Vec<_> = mesh.indices().unwrap().iter().collect();
    ids.chunks_exact(3)
        .filter(|i| include_vertex(i[0]))
        .any(|i| {
            let [a, b, c] = [i[0], i[1], i[2]].map(|n| Vec3::from_array(p[n]));
            let e1 = b - a;
            let e2 = c - a;
            let cross = direction.cross(e2);
            let determinant = e1.dot(cross);
            if determinant.abs() < 1e-10 {
                return false;
            }
            let delta = origin - a;
            let u = delta.dot(cross) / determinant;
            let q = delta.cross(e1);
            let v = direction.dot(q) / determinant;
            let t = e2.dot(q) / determinant;
            u >= -1e-5 && v >= -1e-5 && u + v <= 1.00001 && t > 0.0 && t < distance - 1e-5
        })
}

#[test]
fn pupils_survive_head_meshing_and_closed_lids_occlude_their_entire_front() {
    for kind in [Kind::Sheep, Kind::Cow, Kind::Pig, Kind::Bear, Kind::Horse] {
        for variant in 0..if kind == Kind::Horse { 16 } else { 1 } {
            let parts = parts_variant(kind, variant).unwrap();
            let (_, head_pivot, head) = parts
                .iter()
                .find(|(part, _, _)| matches!(part, Part::Head))
                .unwrap();
            let (_, lid_pivot, lids) = parts
                .iter()
                .find(|(part, _, _)| matches!(part, Part::Eyelids))
                .unwrap();
            assert_eq!(head_pivot, lid_pivot);
            let p = points(head);
            let Some(VertexAttributeValues::Float32x3(normals)) =
                head.attribute(Mesh::ATTRIBUTE_NORMAL)
            else {
                panic!("normals");
            };
            let Some(VertexAttributeValues::Float32x4(colors)) =
                head.attribute(Mesh::ATTRIBUTE_COLOR)
            else {
                panic!("colors");
            };
            let pupil = Color::srgb(PUPIL[0], PUPIL[1], PUPIL[2])
                .to_linear()
                .to_f32_array();
            let white = Color::srgb(WHITE[0], WHITE[1], WHITE[2])
                .to_linear()
                .to_f32_array();
            assert!(
                colors.contains(&white),
                "{kind:?}/{variant}: sclera survived"
            );
            let mut area = [0.0_f32; 2];
            for start in (0..p.len()).step_by(4) {
                if colors[start] != pupil {
                    continue;
                }
                let outward = if matches!(kind, Kind::Horse | Kind::Sheep | Kind::Cow | Kind::Pig) {
                    Vec3::X * p[start][0].signum()
                } else {
                    Vec3::NEG_Z
                };
                if Vec3::from_array(normals[start]).dot(outward) < 0.99 {
                    continue;
                }
                let quad = &p[start..start + 4];
                let target = quad.iter().map(|p| Vec3::from_array(*p)).sum::<Vec3>() * 0.25;
                let origin = target + outward * 0.5;
                assert!(
                    !ray_hits_before(head, origin, -outward, 0.5),
                    "{kind:?}/{variant}: pupil occluded at {target:?}"
                );
                assert!(
                    ray_hits_before(lids, origin, -outward, 0.5),
                    "{kind:?}/{variant}: lid fails to cover pupil at {target:?}"
                );
                if matches!(kind, Kind::Sheep | Kind::Cow | Kind::Pig) {
                    // The skull must lie directly behind the pupil. A nearby
                    // ear can otherwise lift the eye clean off the sloping face.
                    assert!(
                        ray_hits_before_filter(head, target, -outward, 0.015, |i| colors[i]
                            != pupil),
                        "{kind:?}: eye lacks a socket behind {target:?}"
                    );
                }
                area[usize::from(target.x > 0.0)] += (Vec3::from_array(quad[1])
                    - Vec3::from_array(quad[0]))
                .cross(Vec3::from_array(quad[2]) - Vec3::from_array(quad[0]))
                .length();
            }
            // A one-pixel glint must not replace the dark 6x8-pixel pupil.
            for area in area {
                assert!(area > 0.0025, "{kind:?}/{variant}: pupil area {area}");
            }
        }
    }
}

#[test]
fn all_eye_orientations_keep_more_pupil_than_glint_and_the_same_lid_outline() {
    for side in [-1.0, 0.0, 1.0] {
        let spec = Eye {
            center: Vec3::ZERO,
            width: 0.12,
            side,
            skin: [0.4, 0.2, 0.1],
        };
        let open = spec.mesh(false).unwrap();
        let closed = spec.mesh(true).unwrap();
        assert!(open.count_vertices() < 400 && closed.count_vertices() < 400);
        let facing = if side == 0.0 {
            Vec3::NEG_Z
        } else {
            Vec3::X * side
        };
        for vertex in points(&open) {
            let p = Vec3::from_array(*vertex);
            assert!(p.dot(facing) <= 1e-7);
        }
        for vertex in points(&closed) {
            assert!(Vec3::from_array(*vertex).dot(facing) >= 0.0099);
        }
        let Some(VertexAttributeValues::Float32x3(normals)) =
            open.attribute(Mesh::ATTRIBUTE_NORMAL)
        else {
            panic!("normals");
        };
        for n in normals {
            let n = Vec3::from_array(*n).abs();
            assert!(
                (n.max_element() - 1.0).abs() < 1e-6 && (n.element_sum() - 1.0).abs() < 1e-6,
                "axis normal {n:?}"
            );
        }
        let p = points(&open);
        let ids: Vec<_> = open.indices().unwrap().iter().collect();
        for i in ids.chunks_exact(3) {
            let [a, b, c] = [i[0], i[1], i[2]].map(|i| Vec3::from_array(p[i]));
            let geometric = (b - a).cross(c - a).normalize();
            assert!(
                geometric.abs_diff_eq(Vec3::from_array(normals[i[0]]), 1e-6),
                "winding and normal disagree for eye side {side}"
            );
        }
    }
}

#[test]
fn eye_follows_actual_voxel_surface_while_distant_muzzle_cannot_push_it_out() {
    let mut head = crate::creatures::model::voxel::Builder::with_cell_size(0.04);
    head.voxel([0.0, 0.12, -0.20], [0.3, 0.14, 0.18], [0.4; 3])
        .unwrap();
    head.voxel([0.0, -0.2, -0.4], [0.3, 0.14, 0.25], [0.4; 3])
        .unwrap();
    let mesh = head.finish().unwrap();
    let spec = Eye {
        center: Vec3::new(0.0, 0.12, -0.25),
        width: 0.10,
        side: 0.0,
        skin: [0.4; 3],
    };
    let placed = outside_surface(&mesh, spec);
    assert!(
        placed.center.z < -0.29 && placed.center.z > -0.35,
        "socket surface, not distant muzzle: {:?}",
        placed.center
    );
    assert_eq!(placed.center.x, spec.center.x);
    assert_eq!(placed.center.y, spec.center.y);
    let origin = placed.center + Vec3::NEG_Z;
    assert!(!ray_hits_before(&mesh, origin, Vec3::Z, 1.0));
    assert!(ray_hits_before(
        &placed.mesh(true).unwrap(),
        origin,
        Vec3::Z,
        1.0
    ));
}

#[test]
fn peaceful_eyes_use_a_small_warm_catchlight_instead_of_a_white_staring_frame() {
    let mesh = Eye {
        center: Vec3::ZERO,
        width: 0.12,
        side: 0.0,
        skin: [0.4, 0.2, 0.1],
    }
    .mesh(false)
    .unwrap();
    let Some(VertexAttributeValues::Float32x3(normals)) = mesh.attribute(Mesh::ATTRIBUTE_NORMAL)
    else {
        panic!("normals")
    };
    let Some(VertexAttributeValues::Float32x4(colors)) = mesh.attribute(Mesh::ATTRIBUTE_COLOR)
    else {
        panic!("colors")
    };
    let positions = points(&mesh);
    let dark = Color::srgb(PUPIL[0], PUPIL[1], PUPIL[2])
        .to_linear()
        .to_f32_array();
    let white = Color::srgb(WHITE[0], WHITE[1], WHITE[2])
        .to_linear()
        .to_f32_array();
    let mut pupil_area = 0.0;
    let mut white_area = 0.0;
    for first in (0..positions.len()).step_by(4) {
        if normals[first][2] > -0.99 {
            continue;
        }
        let area = (Vec3::from_array(positions[first + 1]) - Vec3::from_array(positions[first]))
            .cross(Vec3::from_array(positions[first + 2]) - Vec3::from_array(positions[first]))
            .length();
        if colors[first] == dark {
            pupil_area += area;
        }
        if colors[first] == white {
            white_area += area;
        }
    }
    assert!(white_area > 0.0 && pupil_area > white_area * 10.0);
}
