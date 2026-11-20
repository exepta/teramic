use super::*;

#[test]
fn kinetic_builtin_collision_matches_authored_models_and_vertical_axes() {
    for material in 141..=151 {
        let boxes = builtin_boxes(material);
        assert!(!boxes.is_empty(), "missing kinetic collider {material}");
        assert!(boxes.iter().all(|b| b.valid_for(material)));
        if material == 148 {
            assert!(
                (boxes.iter().map(|b| b.maximum[1]).fold(0.0, f64::max)
                    - f64::from(crate::kinetic::CONVEYOR_TOP))
                .abs()
                    < 0.001
            );
        } else {
            for facing in 0..6 {
                assert!(
                    boxes
                        .iter()
                        .all(|b| b.rotated_axis(facing).valid_for(material))
                );
            }
        }
    }
    let axle = ShapeBox {
        minimum: [0.0, 0.4, 0.4],
        maximum: [1.0, 0.6, 0.6],
        hinged: false,
    };
    for facing in [4, 5] {
        let vertical = axle.rotated_axis(facing);
        assert_eq!(vertical.minimum[1], 0.0);
        assert_eq!(vertical.maximum[1], 1.0);
        assert!((vertical.maximum[0] - vertical.minimum[0] - 0.2).abs() < 0.0001);
    }
}

#[test]
fn framed_openings_keep_fixed_frames_and_explicit_hinged_collision_parts() {
    for material in 65..=70 {
        let boxes = builtin_boxes(material);
        assert!(!boxes.is_empty(), "opening {material}");
        assert!(boxes.iter().all(|shape| shape.valid_for(material)));
        assert_eq!(
            boxes.iter().filter(|shape| !shape.hinged).count(),
            if material <= 67 { 3 } else { 4 }
        );
        assert!(boxes.iter().any(|shape| shape.hinged));
        let height = if material <= 67 { 2.0 } else { 1.0 };
        assert!(boxes.iter().any(|shape| shape.maximum[1] == height));
        assert!(boxes.iter().all(|shape| shape.maximum[1] <= height));
        for shape in boxes.iter().filter(|shape| shape.hinged) {
            assert!(shape.minimum[0] >= 0.15 - 1.0e-8);
            assert!(shape.maximum[0] <= if material <= 67 { 0.87 } else { 0.85 } + 1.0e-8);
            assert!(shape.minimum[2] >= 0.45 && shape.maximum[2] <= 0.55);
        }
    }
}

#[test]
fn furnace_has_two_cell_height_and_hollow_chimneys_are_sub_cell_shapes() {
    let furnace = builtin_boxes(59);
    assert!(!furnace.is_empty());
    assert!(furnace.iter().all(|shape| shape.valid_for(59)));
    assert!(furnace.iter().any(|shape| shape.maximum[1] == 2.0));
    assert!(furnace.iter().any(|shape| !shape.valid()));
    for material in 60..=64 {
        let boxes = builtin_boxes(material);
        assert!(!boxes.is_empty());
        assert!(boxes.iter().all(ShapeBox::valid));
    }
    let straight = builtin_boxes(60);
    assert!(
        !straight
            .iter()
            .any(|shape| { shape.intersects([0.49, 0.49, 0.49], [0.51, 0.51, 0.51]) })
    );
}

#[test]
fn wall_torch_poses_stay_in_their_cell_and_vectors_preserve_length() {
    for facing in 4..=7 {
        let foot = torch_point(facing, [0.5, 0.0, 0.5]);
        let flame = torch_point(facing, [0.5, 0.9, 0.5]);
        let normal = torch_normal(facing);
        assert!(flame.iter().all(|v| (0.0..=1.0).contains(v)));
        assert!(foot.iter().all(|v| (0.0..=1.0).contains(v)));
        assert!(
            (0..3)
                .map(|i| (flame[i] - foot[i]) * normal[i])
                .sum::<f64>()
                > 0.3
        );
        let up = torch_vector(facing, [0.0, 1.0, 0.0]);
        assert!((up.iter().map(|v| v * v).sum::<f64>() - 1.0).abs() < 1e-12);
        for shape in builtin_boxes(40) {
            for corner in 0..8 {
                let point = torch_point(
                    facing,
                    std::array::from_fn(|axis| {
                        if corner & (1 << axis) == 0 {
                            shape.minimum[axis]
                        } else {
                            shape.maximum[axis]
                        }
                    }),
                );
                assert!(
                    point.iter().all(|value| (0.0..=1.0).contains(value)),
                    "wall-torch collision must not enter its supporting voxel: facing={facing}, point={point:?}"
                );
            }
        }
    }
    assert_eq!(torch_point(0, [0.3, 0.7, 0.4]), [0.3, 0.7, 0.4]);
    assert!(!builtin_boxes(40).is_empty());
}

fn embedded_bytes(uri: &str) -> Vec<u8> {
    let encoded = uri
        .strip_prefix("data:application/octet-stream;base64,")
        .unwrap();
    let alphabet = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut bytes = Vec::new();
    let mut bits = 0_u32;
    let mut count = 0;
    for value in encoded.bytes().take_while(|value| *value != b'=') {
        let digit = alphabet
            .iter()
            .position(|candidate| *candidate == value)
            .unwrap() as u32;
        bits = (bits << 6) | digit;
        count += 6;
        if count >= 8 {
            count -= 8;
            bytes.push((bits >> count) as u8);
            bits &= (1 << count) - 1;
        }
    }
    bytes
}

#[test]
fn authored_interaction_boxes_bound_every_exported_render_triangle_and_keep_all_extents() {
    for (material, source) in [
        (
            28,
            include_str!("../../../../assets/terra/blocks/models/workbench.gltf"),
        ),
        (
            29,
            include_str!("../../../../assets/terra/blocks/models/chest.gltf"),
        ),
    ] {
        let model: serde_json::Value = serde_json::from_str(source).unwrap();
        let bytes = embedded_bytes(model["buffers"][0]["uri"].as_str().unwrap());
        assert_eq!(
            bytes.len(),
            model["buffers"][0]["byteLength"].as_u64().unwrap() as usize
        );
        let boxes = builtin_boxes(material);
        assert!(!boxes.is_empty());
        assert!(boxes.iter().all(ShapeBox::valid));
        // A chamfer keeps the same AABB but no longer has 24 vertices / 12
        // triangles. Inspect referenced triangles and every collider extremum,
        // without assuming the exporter stores a sequence of uncut cuboids.
        let mut reached = vec![[false; 6]; boxes.len()];
        for primitive in model["meshes"][0]["primitives"].as_array().unwrap() {
            let accessor =
                &model["accessors"][primitive["attributes"]["POSITION"].as_u64().unwrap() as usize];
            assert_eq!(accessor["componentType"], 5126);
            assert_eq!(accessor["type"], "VEC3");
            let view = &model["bufferViews"][accessor["bufferView"].as_u64().unwrap() as usize];
            let offset = view["byteOffset"].as_u64().unwrap_or(0) as usize
                + accessor["byteOffset"].as_u64().unwrap_or(0) as usize;
            let count = accessor["count"].as_u64().unwrap() as usize;
            assert!(count > 0);
            let positions: Vec<[f64; 3]> = bytes[offset..offset + count * 12]
                .chunks_exact(12)
                .map(|vertex| {
                    std::array::from_fn(|axis| {
                        f64::from(f32::from_le_bytes(
                            vertex[axis * 4..axis * 4 + 4].try_into().unwrap(),
                        ))
                    })
                })
                .collect();
            let accessor = &model["accessors"][primitive["indices"].as_u64().unwrap() as usize];
            assert_eq!(accessor["componentType"], 5125);
            let view = &model["bufferViews"][accessor["bufferView"].as_u64().unwrap() as usize];
            let offset = view["byteOffset"].as_u64().unwrap_or(0) as usize;
            let indices_count = accessor["count"].as_u64().unwrap() as usize;
            assert_eq!(indices_count % 3, 0);
            for triangle in bytes[offset..offset + indices_count * 4].chunks_exact(12) {
                let indices: [usize; 3] = std::array::from_fn(|axis| {
                    u32::from_le_bytes(triangle[axis * 4..axis * 4 + 4].try_into().unwrap())
                        as usize
                });
                assert!(indices.iter().all(|index| *index < positions.len()));
                let triangle = indices.map(|index| positions[index]);
                let mut bounded = false;
                for (index, shape) in boxes.iter().enumerate() {
                    if !triangle.iter().all(|point| {
                        (0..3).all(|axis| {
                            point[axis] >= shape.minimum[axis] - 1.0e-6
                                && point[axis] <= shape.maximum[axis] + 1.0e-6
                        })
                    }) {
                        continue;
                    }
                    bounded = true;
                    for point in triangle {
                        for axis in 0..3 {
                            reached[index][axis * 2] |=
                                (point[axis] - shape.minimum[axis]).abs() < 1.0e-6;
                            reached[index][axis * 2 + 1] |=
                                (point[axis] - shape.maximum[axis]).abs() < 1.0e-6;
                        }
                    }
                }
                assert!(
                    bounded,
                    "Model {material} triangle outside its interaction hull: {triangle:?}"
                );
                let [a, b, c] = triangle;
                let ab: [f64; 3] = std::array::from_fn(|axis| b[axis] - a[axis]);
                let ac: [f64; 3] = std::array::from_fn(|axis| c[axis] - a[axis]);
                let cross = [
                    ab[1] * ac[2] - ab[2] * ac[1],
                    ab[2] * ac[0] - ab[0] * ac[2],
                    ab[0] * ac[1] - ab[1] * ac[0],
                ];
                assert!(cross.iter().map(|v| v * v).sum::<f64>() > 0.0);
            }
        }
        for (shape, reached) in boxes.iter().zip(reached) {
            assert!(
                reached.into_iter().all(|hit| hit),
                "Model {material} no longer reaches every interaction-box extremum: {shape:?}"
            );
        }
    }
}
