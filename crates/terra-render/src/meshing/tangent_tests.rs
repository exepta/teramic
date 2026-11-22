use super::*;

fn triangle(tangents: bool) -> PrimitiveTemplate {
    PrimitiveTemplate {
        positions: vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
        normals: vec![[0.0, 0.0, 1.0]; 3],
        tangents: if tangents {
            vec![[1.0, 0.0, 0.0, 1.0]; 3]
        } else {
            Vec::new()
        },
        uvs: vec![[0.0, 0.0], [1.0, 0.0], [0.0, 1.0]],
        indices: vec![0, 1, 2],
        material: 0,
    }
}

fn assert_frame(tangent: [f32; 4], normal: [f32; 3]) {
    let direction = Vec3::from_array([tangent[0], tangent[1], tangent[2]]);
    assert!(direction.is_finite());
    assert!((direction.length() - 1.0).abs() < 0.0001);
    assert!(direction.dot(Vec3::from_array(normal)).abs() < 0.0001);
    assert_eq!(tangent[3].abs(), 1.0);
}

#[test]
fn mixed_preview_keeps_indexed_vertices_after_the_last_authored_tangent() {
    let batch = MeshBatch {
        positions: vec![[0.; 3]; 6],
        normals: vec![[0., 1., 0.]; 6],
        uvs: vec![[0.; 2]; 6],
        tangents: vec![[0., 0., 1., -1.]; 3],
        colors: vec![[0.5; 4]; 3],
        indices: vec![0, 1, 2, 3, 4, 5],
        ..Default::default()
    };
    let mesh = crate::previews::batch_mesh(&batch);
    assert!(mesh.attributes().all(|(_, attribute)| attribute.len() == 6));
    let Some(bevy::mesh::VertexAttributeValues::Float32x4(tangents)) =
        mesh.attribute(bevy::mesh::Mesh::ATTRIBUTE_TANGENT)
    else {
        panic!("missing tangents")
    };
    assert_eq!(&tangents[..3], &batch.tangents);
    for &frame in &tangents[3..] {
        assert_frame(frame, [0., 1., 0.]);
    }
}

#[test]
fn authored_tangents_rotate_and_mixed_material_batches_keep_every_vertex_aligned() {
    let mut batches = BTreeMap::new();
    append(
        &mut batches,
        &triangle(false),
        Vec3::ZERO,
        Quat::IDENTITY,
        None,
        None,
    )
    .unwrap();
    assert!(batches[&0].tangents.is_empty());
    let rotation = Quat::from_rotation_y(std::f32::consts::FRAC_PI_2);
    append(&mut batches, &triangle(true), Vec3::X, rotation, None, None).unwrap();
    append(
        &mut batches,
        &triangle(false),
        Vec3::Y,
        Quat::IDENTITY,
        None,
        None,
    )
    .unwrap();
    let batch = &batches[&0];
    assert_eq!(batch.tangents.len(), batch.positions.len());
    assert_eq!(batch.positions.len(), 9);
    for (&tangent, &normal) in batch.tangents.iter().zip(&batch.normals) {
        assert_frame(tangent, normal);
    }
    for tangent in &batch.tangents[3..6] {
        assert!(
            (Vec3::from_array([tangent[0], tangent[1], tangent[2]]) - rotation * Vec3::X).length()
                < 0.0001
        );
        assert_eq!(tangent[3], 1.0);
    }
}

#[test]
fn connected_geometry_rebuilds_tangent_frames_from_its_mirrored_uvs() {
    let mut primitive = triangle(true);
    primitive.uvs[1][0] = -1.0;
    let mut batch = MeshBatch::default();
    append_connected(&mut batch, &primitive, Vec3::ZERO, 63, 0.0, None, false).unwrap();
    assert_eq!(batch.tangents.len(), 3);
    for (&tangent, &normal) in batch.tangents.iter().zip(&batch.normals) {
        assert_frame(tangent, normal);
        assert_eq!(tangent, [-1.0, 0.0, 0.0, -1.0]);
    }
}

#[test]
fn final_worker_packing_preserves_tangents_and_fills_later_procedural_vertices() {
    let mut batch = MeshBatch {
        positions: vec![[0.0; 3]; 4],
        normals: vec![[0.0, 1.0, 0.0]; 4],
        tangents: vec![[1.0, 0.0, 0.0, 1.0]; 3],
        indices: vec![0, 1, 2],
        ..Default::default()
    };
    crate::foliage::packing::pack(&mut batch);
    batch.compact_indices();
    assert!(batch.normals.is_empty());
    assert_eq!(batch.tangents, vec![[1.0, 0.0, 0.0, 1.0]; 4]);
    assert_eq!(batch.compact_indices, [0, 1, 2]);
}

#[test]
fn static_worker_frames_reduce_twenty_bytes_per_vertex_and_preserve_handedness() {
    let directions = [
        Vec3::X,
        Vec3::NEG_X,
        Vec3::Y,
        Vec3::NEG_Y,
        Vec3::Z,
        Vec3::NEG_Z,
        Vec3::new(0.23, -0.77, 0.44).normalize(),
        Vec3::new(-0.49, 0.31, -0.82).normalize(),
    ];
    let normals: Vec<_> = directions
        .into_iter()
        .flat_map(|n| [n.to_array(); 2])
        .collect();
    let tangents: Vec<_> = directions
        .into_iter()
        .flat_map(|n| [-1.0, 1.0].map(|sign| crate::blocks::tangent_frame(Vec3::X, n, sign)))
        .collect();
    let count = normals.len();
    let mut batch = MeshBatch {
        positions: (0..count).map(|i| [i as f32, -2.0, 3.0]).collect(),
        normals: normals.clone(),
        tangents: tangents.clone(),
        uvs: vec![[0.25, 0.75]; count],
        surface_water: vec![[0.4, 0.8]; count],
        packed_colors: vec![[33, 66, 99, 255]; count],
        indices: vec![0, 1, 2, 0, 2, 3],
        ..Default::default()
    };
    let positions = batch.positions.clone();
    let before = crate::upload_budget::batch_bytes(&batch);
    batch.pack_static_frame();
    assert_eq!(
        before - crate::upload_budget::batch_bytes(&batch),
        count * 20
    );
    assert_eq!(batch.positions, positions);
    assert_eq!(batch.uvs, vec![[0.25, 0.75]; count]);
    assert_eq!(batch.surface_water, vec![[0.4, 0.8]; count]);
    assert_eq!(batch.packed_colors, vec![[33, 66, 99, 255]; count]);
    assert_eq!(batch.indices, [0, 1, 2, 0, 2, 3]);
    assert!(batch.normals.is_empty() && batch.tangents.is_empty());
    assert_eq!(batch.packed_normals.capacity(), count);
    assert_eq!(batch.packed_tangents.capacity(), count);
    for i in 0..count {
        let decode = |packed: [i8; 4]| {
            Vec3::new(
                f32::from(packed[0]),
                f32::from(packed[1]),
                f32::from(packed[2]),
            ) / 127.0
        };
        let normal = decode(batch.packed_normals[i]).normalize();
        let tangent = decode(batch.packed_tangents[i]).normalize();
        assert!(normal.distance(Vec3::from_array(normals[i])) < 0.009);
        assert!(
            tangent.distance(Vec3::from_array([
                tangents[i][0],
                tangents[i][1],
                tangents[i][2]
            ])) < 0.009
        );
        assert!(normal.dot(tangent).abs() < 0.013);
        assert_eq!(
            f32::from(batch.packed_tangents[i][3]) / 127.0,
            tangents[i][3]
        );
    }
    let packed_normals = batch.packed_normals.clone();
    let packed_tangents = batch.packed_tangents.clone();
    batch.pack_static_frame();
    assert_eq!(batch.packed_normals, packed_normals);
    assert_eq!(batch.packed_tangents, packed_tangents);
}

#[test]
fn worker_frame_packing_preserves_nonstandard_directions_and_shadow_only_meshes() {
    for normal in [[0.0; 3], [2.0, 0.0, 0.0], [f32::NAN, 0.0, 0.0]] {
        let mut batch = MeshBatch {
            positions: vec![[0.0; 3]],
            normals: vec![normal],
            tangents: vec![[1.0, 0.0, 0.0, 0.5]],
            ..Default::default()
        };
        batch.pack_static_frame();
        assert_eq!(batch.normals[0].map(f32::to_bits), normal.map(f32::to_bits));
        assert_eq!(batch.tangents, [[1.0, 0.0, 0.0, 0.5]]);
        assert!(batch.packed_normals.is_empty() && batch.packed_tangents.is_empty());
    }
    for shadow in [false, true] {
        let mut batch = MeshBatch {
            positions: vec![[0.0; 3]; 4],
            normals: vec![[0.0, 1.0, 0.0]; 4],
            tangents: vec![[1.0, 0.0, 0.0, -1.0]; 3],
            shadow_only: shadow,
            rooted_wind: if shadow {
                Vec::new()
            } else {
                vec![[0.3, 0.8]; 4]
            },
            ..Default::default()
        };
        batch.pack_static_frame();
        if shadow {
            assert_eq!(batch.normals.len(), 4);
            assert_eq!(batch.tangents.len(), 3);
            assert!(batch.packed_normals.is_empty() && batch.packed_tangents.is_empty());
        } else {
            assert!(batch.normals.is_empty() && batch.tangents.is_empty());
            assert_eq!(batch.packed_normals.len(), 4);
            assert_eq!(batch.packed_tangents.len(), 4);
            assert_eq!(batch.rooted_wind, [[0.3, 0.8]; 4]);
        }
    }
    let mut mixed = MeshBatch {
        positions: vec![[0.0; 3]; 4],
        normals: vec![[0.0, 1.0, 0.0]; 4],
        tangents: vec![[1.0, 0.0, 0.0, -1.0]; 3],
        ..Default::default()
    };
    mixed.pack_static_frame();
    assert_eq!(mixed.packed_tangents.len(), 4);
    assert_eq!(mixed.packed_tangents[0], [127, 0, 0, -127]);
    assert_eq!(mixed.packed_tangents[3], [127, 0, 0, 127]);
}

#[test]
fn rooted_grass_frames_pack_signed_directions_without_touching_hdr_colors_or_wind() {
    let normals = [
        Vec3::X,
        Vec3::NEG_X,
        Vec3::Y,
        Vec3::NEG_Y,
        Vec3::Z,
        Vec3::NEG_Z,
        Vec3::new(-0.23, 0.77, -0.44).normalize(),
    ];
    let tangents: Vec<_> = normals
        .iter()
        .enumerate()
        .map(|(index, &normal)| {
            crate::blocks::tangent_frame(Vec3::X, normal, if index % 2 == 0 { -1.0 } else { 1.0 })
        })
        .collect();
    let count = normals.len();
    let colors = vec![[1.25, 0.5, 2.75, 0.375]; count];
    let wind: Vec<_> = (0..count)
        .map(|index| [0.0, index as f32 / count as f32])
        .collect();
    let mut batch = MeshBatch {
        positions: vec![[0.0; 3]; count],
        normals: normals.map(|normal| normal.to_array()).to_vec(),
        tangents: tangents.clone(),
        colors: colors.clone(),
        rooted_wind: wind.clone(),
        uvs: vec![[0.25, 0.75]; count],
        indices: vec![0, 1, 2],
        ..Default::default()
    };
    let bytes = crate::upload_budget::batch_bytes(&batch);
    batch.pack_static_frame();
    assert_eq!(
        bytes - crate::upload_budget::batch_bytes(&batch),
        count * 20
    );
    assert_eq!(batch.colors, colors);
    assert_eq!(batch.rooted_wind, wind);
    assert!(batch.packed_colors.is_empty());
    assert!(batch.normals.is_empty() && batch.tangents.is_empty());
    for (index, normal) in normals.iter().copied().enumerate() {
        let decode = |packed: [i8; 4]| {
            Vec3::new(
                f32::from(packed[0]),
                f32::from(packed[1]),
                f32::from(packed[2]),
            ) / 127.0
        };
        assert!(
            decode(batch.packed_normals[index])
                .normalize()
                .distance(normal)
                < 0.009
        );
        assert!(
            decode(batch.packed_tangents[index])
                .normalize()
                .distance(Vec3::from_array([
                    tangents[index][0],
                    tangents[index][1],
                    tangents[index][2]
                ]))
                < 0.009
        );
        assert_eq!(
            f32::from(batch.packed_tangents[index][3]) / 127.0,
            tangents[index][3]
        );
    }
    let mut mesh = Mesh::new(
        bevy::mesh::PrimitiveTopology::TriangleList,
        bevy::asset::RenderAssetUsages::MAIN_WORLD,
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, batch.positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, batch.uvs)
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_1, batch.rooted_wind)
    .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, batch.colors);
    mesh.insert_attribute(
        bevy::mesh::MeshVertexAttribute {
            format: bevy::render::render_resource::VertexFormat::Snorm8x4,
            ..Mesh::ATTRIBUTE_NORMAL
        },
        bevy::mesh::VertexAttributeValues::Snorm8x4(batch.packed_normals),
    );
    mesh.insert_attribute(
        bevy::mesh::MeshVertexAttribute {
            format: bevy::render::render_resource::VertexFormat::Snorm8x4,
            ..Mesh::ATTRIBUTE_TANGENT
        },
        bevy::mesh::VertexAttributeValues::Snorm8x4(batch.packed_tangents),
    );
    let mut layouts = bevy::mesh::MeshVertexBufferLayouts::default();
    let layout = mesh.get_mesh_vertex_buffer_layout(&mut layouts);
    let buffer = layout
        .0
        .get_layout(&[
            Mesh::ATTRIBUTE_POSITION.at_shader_location(0),
            Mesh::ATTRIBUTE_NORMAL.at_shader_location(1),
            Mesh::ATTRIBUTE_TANGENT.at_shader_location(4),
            Mesh::ATTRIBUTE_UV_0.at_shader_location(2),
            Mesh::ATTRIBUTE_UV_1.at_shader_location(3),
            Mesh::ATTRIBUTE_COLOR.at_shader_location(5),
        ])
        .unwrap();
    assert_eq!(
        buffer.array_stride, 52,
        "HDR colors and float wind remain full precision; only the frame saves 20B/vertex"
    );
    assert!(
        matches!(mesh.attribute(Mesh::ATTRIBUTE_COLOR), Some(bevy::mesh::VertexAttributeValues::Float32x4(actual)) if actual == &colors)
    );
}

#[test]
fn static_frames_pack_at_every_detail_level_without_changing_composed_geometry() {
    let mut faces = crate::blocks::cube::cube_faces(0.0, [0; 6], None);
    for primitive in faces.iter_mut().flatten() {
        primitive.tangents = primitive
            .normals
            .iter()
            .map(|&normal| crate::blocks::tangent_frame(Vec3::X, Vec3::from_array(normal), -1.0))
            .collect();
    }
    let templates = TemplateSet {
        blocks: std::collections::HashMap::from([(
            3,
            crate::blocks::BlockTemplate {
                surface: None,
                model: Vec::new(),
                render_kind: BlockRenderKind::Cube,
                face_materials: [0; 6],
                side_cap: None,
                bevel_width: 0.0,
                faces,
                details: Vec::new(),
            },
        )]),
        ..Default::default()
    };
    let face = BlockFace {
        edge_mask: 15,
        occupied_neighbor: false,
        minimum: [0, 4, 0],
        width: 3,
        height: 2,
        u_axis: 0,
        v_axis: 2,
        side: 3,
        material: 3,
    };
    for detail_level in [DetailLevel::Near, DetailLevel::Middle, DetailLevel::Far] {
        let options = RenderOptions {
            water_level: None,
            detail_level,
            terrain: None,
            exposure: None,
            tints: Default::default(),
            conditions: Default::default(),
        };
        let mut batches =
            compose_section_with_options(&[face], &templates, SectionPos::default(), &options)
                .unwrap();
        assert!(!batches.is_empty());
        for batch in &mut batches {
            let positions = batch.positions.clone();
            let uvs = batch.uvs.clone();
            let indices = batch.indices.clone();
            assert_eq!(batch.tangents.len(), positions.len());
            batch.pack_static_frame();
            assert_eq!(batch.positions, positions);
            assert_eq!(batch.uvs, uvs);
            assert_eq!(batch.indices, indices);
            assert_eq!(batch.packed_normals.len(), positions.len());
            assert_eq!(batch.packed_tangents.len(), positions.len());
        }
    }
}
