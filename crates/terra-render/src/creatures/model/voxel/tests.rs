use super::*;
use bevy::mesh::VertexAttributeValues;

fn positions(mesh: &Mesh) -> &[[f32; 3]] {
    let Some(VertexAttributeValues::Float32x3(points)) = mesh.attribute(Mesh::ATTRIBUTE_POSITION)
    else {
        panic!("positions")
    };
    points
}

fn axis_surface(mesh: &Mesh) {
    axis_surface_named(mesh, "fixture");
}

fn axis_surface_named(mesh: &Mesh, context: &str) {
    let Some(VertexAttributeValues::Float32x3(normals)) = mesh.attribute(Mesh::ATTRIBUTE_NORMAL)
    else {
        panic!("normals")
    };
    let p = positions(mesh);
    for n in normals {
        let n = Vec3::from_array(*n).abs();
        assert!((n.max_element() - 1.0).abs() < 1e-6 && (n.element_sum() - 1.0).abs() < 1e-6);
    }
    let indices = mesh.indices().unwrap().iter().collect::<Vec<_>>();
    let mut faces = std::collections::BTreeSet::new();
    for ids in indices.chunks_exact(3) {
        let [a, b, c] = [ids[0], ids[1], ids[2]].map(|i| Vec3::from_array(p[i]));
        let cross = (b - a).cross(c - a);
        assert!(cross.length_squared() > 1e-18);
        assert!(cross.dot(Vec3::from_array(normals[ids[0]])) > 0.0);
        let mut key = [a, b, c].map(|v| v.to_array().map(f32::to_bits));
        key.sort();
        assert!(
            faces.insert(key),
            "coincident surfaces in {context}: {key:?}"
        );
    }
}

#[test]
fn joined_and_overpainted_boxes_emit_only_one_closed_exterior() {
    let mut builder = Builder::with_cell_size(0.025);
    builder
        .voxel([-0.1, 0.0, 0.0], [0.2, 0.4, 0.4], [0.5; 3])
        .unwrap();
    builder
        .voxel([0.1, 0.0, 0.0], [0.2, 0.4, 0.4], [0.5; 3])
        .unwrap();
    builder.voxel([0.0; 3], [0.1; 3], [0.8; 3]).unwrap();
    let mesh = builder.finish().unwrap();
    assert_eq!(mesh.count_vertices(), 24);
    assert_eq!(mesh.indices().unwrap().len() / 3, 12);
    axis_surface(&mesh);
}

#[test]
fn round_body_becomes_a_stepped_sculpture_with_exact_outer_bounds() {
    let mut builder = Builder::default();
    builder
        .ellipsoid(Vec3::ZERO, Vec3::new(0.3, 0.2, 0.4), [0.7, 0.4, 0.2])
        .unwrap();
    let mesh = builder.finish().unwrap();
    axis_surface(&mesh);
    assert!((48..3_000).contains(&mesh.count_vertices()));
    let p = positions(&mesh);
    for (axis, radius) in [0.3, 0.2, 0.4].into_iter().enumerate() {
        let low = p.iter().map(|p| p[axis]).fold(f32::INFINITY, f32::min);
        let high = p.iter().map(|p| p[axis]).fold(f32::NEG_INFINITY, f32::max);
        assert!((low + radius).abs() < 1e-6 && (high - radius).abs() < 1e-6);
    }
}

#[test]
fn thin_triangular_fins_and_rotated_antlers_keep_local_voxel_faces() {
    let mut fin = Builder::default();
    fin.triangle(
        [
            Vec3::ZERO,
            Vec3::new(0.6, 0.3, 0.0),
            Vec3::new(0.1, 0.7, 0.0),
        ],
        [0.2, 0.6, 0.7],
    )
    .unwrap();
    let mesh = fin.finish().unwrap();
    axis_surface(&mesh);
    assert!(positions(&mesh).iter().any(|p| p[1] > 0.65));
    let mut horn = Builder::default();
    horn.rotated(
        [0.0; 3],
        [0.08, 0.65, 0.09],
        [0.8; 3],
        Quat::from_rotation_z(0.6),
    )
    .unwrap();
    axis_surface(&horn.finish().unwrap());
}

#[test]
fn ground_contact_and_original_rgb_are_preserved() {
    let mut foot = Builder::default();
    foot.rounded(
        [0.0, -0.488, -0.035],
        [0.31, 0.165, 0.38],
        0.035,
        [0.56, 0.51, 0.40],
    )
    .unwrap();
    let mesh = foot.finish().unwrap();
    let low = positions(&mesh)
        .iter()
        .map(|p| p[1])
        .fold(f32::INFINITY, f32::min);
    assert!((low - (-0.488 - 0.165 * 0.5)).abs() < 1e-6);
    let Some(VertexAttributeValues::Float32x4(colors)) = mesh.attribute(Mesh::ATTRIBUTE_COLOR)
    else {
        panic!("colors")
    };
    let expected = Color::srgb(0.56, 0.51, 0.40).to_linear().to_f32_array();
    assert!(
        colors
            .iter()
            .all(|c| c.iter().zip(expected).all(|(a, b)| (*a - b).abs() < 1e-7))
    );
}

#[test]
fn all_sexes_and_new_variant_rigs_keep_axis_faces_and_original_joint_pivots() {
    use crate::creatures::model::{Kind, Part, parts_variant, villager_variant};
    use terra_core::creatures::{CreatureSex, villagers::ROLE_COUNT};
    for sex in [CreatureSex::Male, CreatureSex::Female] {
        for role in 0..ROLE_COUNT {
            let parts = parts_variant(Kind::Villager, villager_variant(role, sex)).unwrap();
            let head = parts
                .iter()
                .find(|(part, _, _)| matches!(part, Part::Head))
                .unwrap();
            assert_eq!(head.1, Vec3::new(0.0, 1.435, -0.015));
            for (part, _, mesh) in parts {
                axis_surface_named(&mesh, &format!("Villager/{sex:?}/{role}/{part:?}"));
            }
        }
    }
    for (kind, variants) in [
        (Kind::Skeleton, 3),
        (Kind::Butterfly, 7),
        (Kind::SpiritGloom, terra_core::creatures::gloom::COUNT),
    ] {
        for variant in 0..variants {
            let parts = parts_variant(kind, variant).unwrap();
            assert!(parts.iter().any(|(part, _, _)| matches!(part, Part::Body)));
            for (part, _, mesh) in parts {
                axis_surface_named(&mesh, &format!("{kind:?}/{variant}/{part:?}"));
            }
        }
    }
}

#[test]
fn clipped_roundoff_cell_cannot_emit_coincident_caps_or_hide_the_real_surface() {
    let cells = [
        ([0, 0, 0], [0.5_f32.to_bits(); 3]),
        ([1, 0, 0], [0.5_f32.to_bits(); 3]),
        ([2, 1, 0], [0.5_f32.to_bits(); 3]),
    ]
    .into_iter()
    .collect();
    let mesh = surface::mesh(&cells, Vec3::ZERO, 0.1, Vec3::new(0.2, 0.2, 0.1)).unwrap();
    axis_surface(&mesh);
    assert_eq!(mesh.count_vertices(), 24);
}

#[test]
fn every_military_emblem_preserves_pixel_area_without_internal_row_faces() {
    use crate::creatures::model::soldier;
    use terra_core::kingdom::{KingdomHeraldry, KingdomIcon};

    for icon in KingdomIcon::catalog() {
        let mask = icon.glyph_mask().unwrap();
        let mesh = soldier::tabard(KingdomHeraldry { icon, ..default() }).unwrap();
        axis_surface_named(&mesh, &format!("military emblem {icon:?}"));
        let p = positions(&mesh);
        let indices = mesh.indices().unwrap().iter().collect::<Vec<_>>();
        let mut glyph_area = 0.0;
        let mut volume = 0.0;
        for triangle in indices.chunks_exact(3) {
            let [a, b, c] = [triangle[0], triangle[1], triangle[2]].map(|i| Vec3::from_array(p[i]));
            let cross = (b - a).cross(c - a);
            if a.z.abs() > 0.255 && cross.z > 0.0 {
                glyph_area += cross.z * 0.5;
            }
            volume += a.dot(b.cross(c)) / 6.0;
        }
        let pixels = mask.iter().map(|row| row.count_ones()).sum::<u32>() as f32;
        assert!((glyph_area - 2.0 * pixels * 0.006 * 0.006).abs() < 1e-6);
        let cloth_volume = 2.0 * (0.275 * 0.43 + 0.23 * 0.035) * 0.018;
        let relief_volume = 2.0 * pixels * 0.006 * 0.006 * 0.004;
        assert!((volume - cloth_volume - relief_volume).abs() < 1e-6);
    }
}
