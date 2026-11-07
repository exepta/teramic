use super::*;

fn templates(id: u16, model: Vec<PrimitiveTemplate>) -> TemplateSet {
    let mut templates = TemplateSet::default();
    templates.blocks.insert(
        id,
        BlockTemplate {
            surface: None,
            render_kind: BlockRenderKind::CustomMesh,
            bevel_width: 0.,
            faces: std::array::from_fn(|_| Vec::new()),
            face_materials: [0; 6],
            side_cap: None,
            details: Vec::new(),
            model,
        },
    );
    templates
}

#[test]
fn actual_pipe_keeps_a_round_open_end_opposite_its_neighbor_at_a_negative_section_border() {
    use crate::meshing::{builder::RenderOptions, custom};
    use terra_voxel::{BlockSection, SectionPos, meshing::BlockMesh};
    let source = load_static_scene("terra/blocks/models/wood_pipe.gltf", 163);
    let templates = templates(163, source);
    let position = SectionPos::new(-1, 0, -1);
    let mut section = BlockSection::empty(position).unwrap();
    section.set_block([15, 8, 15], 163).unwrap();
    let snapshot = custom::capture(&section, &templates, &Default::default(), |cell| {
        Some(if cell == [-1, 9, -1] { 164 } else { 0 })
    })
    .unwrap();
    let parts = custom::compose(
        &BlockMesh::default(),
        &snapshot,
        &templates,
        position,
        &RenderOptions {
            water_level: None,
            detail_level: crate::meshing::DetailLevel::Near,
            terrain: None,
            exposure: None,
            tints: Default::default(),
            conditions: Default::default(),
        },
    )
    .unwrap();
    let points: Vec<_> = parts.iter().flat_map(|part| &part.positions).collect();
    assert!(points.len() < 1000);
    assert!(points.iter().any(|point| (point[1] - 9.).abs() < 1e-5));
    assert!(points.iter().any(|point| (point[1] - 8.).abs() < 1e-5));
    assert!(points.iter().all(|point| point[0] >= 15.28
        && point[0] <= 15.72
        && point[2] >= 15.28
        && point[2] <= 15.72));
    assert!(
        parts
            .iter()
            .flat_map(|part| &part.normals)
            .flatten()
            .all(|v| v.is_finite())
    );
}

#[test]
fn all_hydraulic_definitions_have_bounded_physical_hulls_and_real_shared_pbr_models() {
    let library = BlockLibrary::load(&terra_core::asset_directory().join("terra/blocks")).unwrap();
    for (id, path) in [
        (163, "terra/blocks/models/wood_pipe.gltf"),
        (164, "terra/blocks/models/stone_pipe.gltf"),
        (165, "terra/blocks/models/iron_pipe.gltf"),
        (166, "terra/blocks/models/valve.gltf"),
        (167, "terra/blocks/models/faucet.gltf"),
        (168, "terra/blocks/models/pump.gltf"),
        (169, "terra/blocks/models/basin.gltf"),
        (170, "terra/blocks/models/hold_block.gltf"),
        (171, "terra/blocks/models/fluid_infuser.gltf"),
        (172, "terra/blocks/models/glass_pipe.gltf"),
    ] {
        let definition = library.blocks.iter().find(|block| block.id == id).unwrap();
        definition.validate().unwrap();
        assert!(!definition.collision_boxes.is_empty() && definition.collision_boxes.len() <= 12);
        let model = load_static_scene(path, id);
        assert!(model.iter().map(|part| part.positions.len()).sum::<usize>() <= 3000);
        assert!(
            model
                .iter()
                .flat_map(|part| &part.positions)
                .flatten()
                .all(|value| (0.0..=1.0).contains(value))
        );
        assert!(
            model
                .iter()
                .map(|part| part.material)
                .collect::<std::collections::HashSet<_>>()
                .len()
                <= 6
        );
    }
}

#[test]
fn all_pipe_flanges_meet_the_round_body_without_changing_their_outer_bounds() {
    use bevy::math::Vec3;
    for (path, id) in [
        ("terra/blocks/models/wood_pipe.gltf", 163),
        ("terra/blocks/models/stone_pipe.gltf", 164),
        ("terra/blocks/models/iron_pipe.gltf", 165),
        ("terra/blocks/models/glass_pipe.gltf", 172),
    ] {
        let parts = load_static_scene(path, id);
        for side in 0..6 {
            let axis = side / 2;
            let ends = if side % 2 == 0 {
                [0.01, 0.08]
            } else {
                [0.92, 0.99]
            };
            let mut inner = Vec::new();
            let mut outer = Vec::new();
            for (point, normal) in parts
                .iter()
                .flat_map(|part| part.positions.iter().zip(&part.normals))
            {
                if normal[axis].abs() > 1e-5
                    || !ends.into_iter().any(|end| (point[axis] - end).abs() < 1e-5)
                {
                    continue;
                }
                let mut radial = Vec3::from_array(*point) - Vec3::splat(0.5);
                radial[axis] = 0.;
                let dot = radial.dot(Vec3::from_array(*normal));
                if dot < -0.1 {
                    inner.push(radial.length());
                }
                if dot > 0.1 {
                    outer.push(radial.length());
                }
            }
            assert!(
                !inner.is_empty() && !outer.is_empty(),
                "pipe {id}, mouth {side}"
            );
            assert!(
                inner
                    .into_iter()
                    .all(|r| (0.149999..=0.150001).contains(&r)),
                "gap at pipe {id}, mouth {side}"
            );
            assert!(
                outer.into_iter().all(|r| (r - 0.215).abs() < 1e-6),
                "outer fitting changed for pipe {id}"
            );
        }
    }
}

#[test]
fn pipe_support_collars_stay_on_straight_arms_with_authored_uv_tangent_frames() {
    use bevy::math::{Vec2, Vec3};
    for (name, path, id) in [
        ("wood_pipe", "terra/blocks/models/wood_pipe.gltf", 163),
        ("stone_pipe", "terra/blocks/models/stone_pipe.gltf", 164),
        ("iron_pipe", "terra/blocks/models/iron_pipe.gltf", 165),
        ("glass_pipe", "terra/blocks/models/glass_pipe.gltf", 172),
    ] {
        let parts = load_static_scene(path, id);
        for axis in 0..3 {
            let mut ends = Vec::new();
            for (point, normal) in parts
                .iter()
                .flat_map(|part| part.positions.iter().zip(&part.normals))
            {
                let mut radial = Vec3::from_array(*point) - Vec3::splat(0.5);
                radial[axis] = 0.;
                if normal[axis].abs() < 1e-5 && (radial.length() - 0.163).abs() < 1e-6 {
                    assert!(
                        [0.20, 0.25, 0.75, 0.80]
                            .into_iter()
                            .any(|end| (point[axis] - end).abs() < 1e-6),
                        "collar intrudes into bend: {name}, axis {axis}, {point:?}"
                    );
                    ends.push(point[axis]);
                }
            }
            for end in [0.20, 0.25, 0.75, 0.80] {
                assert!(
                    ends.iter()
                        .any(|&coordinate| (coordinate - end).abs() < 1e-6)
                );
            }
        }
        for part in parts {
            assert_eq!(
                part.positions.len(),
                part.tangents.len(),
                "{name} fitting tangents"
            );
            for face in part.indices.chunks_exact(3) {
                let points = [face[0], face[1], face[2]]
                    .map(|i| Vec3::from_array(part.positions[i as usize]));
                let uv =
                    [face[0], face[1], face[2]].map(|i| Vec2::from_array(part.uvs[i as usize]));
                let [a, b] = [uv[1] - uv[0], uv[2] - uv[0]];
                let determinant = a.perp_dot(b);
                assert!(determinant.abs() > 1e-10, "collapsed fitting UV: {name}");
                let [p, q] = [points[1] - points[0], points[2] - points[0]];
                let du = (p * b.y - q * a.y) / determinant;
                let dv = (q * a.x - p * b.x) / determinant;
                for &index in face {
                    let n = Vec3::from_array(part.normals[index as usize]);
                    let t = part.tangents[index as usize];
                    let tangent = Vec3::new(t[0], t[1], t[2]);
                    assert!(tangent.dot(du.normalize()) > 0.999);
                    assert!(n.cross(tangent).dot(dv) * t[3] > 0.);
                }
            }
        }
    }
}

#[test]
fn glass_pipe_has_a_transparent_hollow_shell_and_opaque_technic_flanges() {
    let path = terra_core::asset_directory().join("terra/blocks/models/glass_pipe.gltf");
    let model: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    let materials = model["materials"].as_array().unwrap();
    let glass = materials
        .iter()
        .find(|material| material["name"] == "pipe_glass")
        .unwrap();
    assert_eq!(glass["alphaMode"], "BLEND");
    assert!(
        glass["pbrMetallicRoughness"]["baseColorFactor"][3]
            .as_f64()
            .unwrap()
            < 0.3
    );
    assert_eq!(
        glass["doubleSided"], false,
        "inner walls already have their own inward faces"
    );
    for name in ["brass_flanges", "iron_bolts"] {
        let trim = materials
            .iter()
            .find(|material| material["name"] == name)
            .unwrap();
        assert_eq!(trim["pbrMetallicRoughness"]["baseColorFactor"][3], 1.0);
    }
    assert_eq!(
        terra_voxel::MaterialId::from_raw(172),
        Some(terra_voxel::MaterialId::GlassPipe)
    );
    assert!(!terra_voxel::MaterialId::GlassPipe.is_opaque());
}

#[test]
fn filling_models_match_belt_height_and_leave_real_clearance_for_vessels() {
    let hold = load_static_scene("terra/blocks/models/hold_block.gltf", 170);
    let infuser = load_static_scene("terra/blocks/models/fluid_infuser.gltf", 171);
    let top = hold
        .iter()
        .flat_map(|part| &part.positions)
        .map(|p| p[1])
        .fold(0., f32::max);
    assert!((f64::from(top) - terra_core::infusion::HOLD_TOP).abs() < 1e-6);
    let points: Vec<_> = infuser.iter().flat_map(|part| &part.positions).collect();
    for axis in 0..3 {
        let minimum = points.iter().map(|p| p[axis]).fold(f32::INFINITY, f32::min);
        let maximum = points
            .iter()
            .map(|p| p[axis])
            .fold(f32::NEG_INFINITY, f32::max);
        assert!(minimum.abs() < 1e-6 && (maximum - 1.).abs() < 1e-6);
    }
    assert!(
        points
            .iter()
            .any(|p| p[1].abs() < 1e-6 && (p[0] - 0.5).abs() < 0.06 && (p[2] - 0.5).abs() < 0.06)
    );
    let hold_boxes = terra_core::block_shapes::builtin_boxes(170);
    assert!((hold_boxes[0].maximum[1] - terra_core::infusion::HOLD_TOP).abs() < 1e-6);
    assert!(
        terra_core::block_shapes::builtin_boxes(171)
            .iter()
            .all(|bounds| {
                bounds.minimum[1] + terra_core::infusion::INFUSER_OFFSET as f64
                    >= terra_core::infusion::HOLD_TOP + 1.0
            })
    );
    for model in [hold, infuser] {
        for part in model {
            for triangle in part.indices.chunks_exact(3) {
                let [a, b, c] = std::array::from_fn(|i| {
                    bevy::math::Vec3::from_array(part.positions[triangle[i] as usize])
                });
                let normal = bevy::math::Vec3::from_array(part.normals[triangle[0] as usize]);
                assert!((b - a).cross(c - a).dot(normal) > 0.);
            }
        }
    }
}

#[test]
fn actual_valve_ports_remain_cell_centered_in_all_six_axes_and_both_open_states() {
    let model = load_static_scene("terra/blocks/models/valve.gltf", 166);
    let mut authored = terra_core::block_shapes::ShapeBox {
        minimum: [f64::INFINITY; 3],
        maximum: [f64::NEG_INFINITY; 3],
        hinged: false,
    };
    for point in model.iter().flat_map(|part| &part.positions) {
        for (axis, &coordinate) in point.iter().enumerate() {
            authored.minimum[axis] = authored.minimum[axis].min(f64::from(coordinate));
            authored.maximum[axis] = authored.maximum[axis].max(f64::from(coordinate));
        }
    }
    let templates = templates(166, model);
    for facing in (0..6).flat_map(|facing| [facing, facing | 8]) {
        let batches = crate::meshing::custom::model_geometry(
            166,
            terra_world::edits::BlockObjectState {
                facing,
                ..Default::default()
            },
            Default::default(),
            [None; 6],
            [0; 6],
            &templates,
        )
        .unwrap();
        let axis = terra_core::kinetic::axis(facing & 7)
            .iter()
            .position(|value| *value != 0)
            .unwrap();
        let points: Vec<_> = batches.iter().flat_map(|batch| &batch.positions).collect();
        for endpoint in [0., 1.] {
            assert!(
                points
                    .iter()
                    .any(|point| (point[axis] - endpoint).abs() < 1e-5)
            );
        }
        assert!(points.iter().all(|point| {
            point
                .iter()
                .all(|value| *value >= -0.0001 && *value <= 1.0001)
        }));
        if facing & 8 == 0 {
            let expected = authored.rotated_axis(facing);
            for axis in 0..3 {
                let minimum = points
                    .iter()
                    .map(|point| f64::from(point[axis]))
                    .fold(f64::INFINITY, f64::min);
                let maximum = points
                    .iter()
                    .map(|point| f64::from(point[axis]))
                    .fold(f64::NEG_INFINITY, f64::max);
                assert!(
                    (minimum - expected.minimum[axis]).abs() < 1e-5,
                    "valve handle and selection must share the same basis for facing {facing}"
                );
                assert!(
                    (maximum - expected.maximum[axis]).abs() < 1e-5,
                    "valve handle and selection must share the same basis for facing {facing}"
                );
            }
        }
    }
}
