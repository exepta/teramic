//! Half-float tints preserve HDR RGB and semantic alpha without Unorm clamping.
use super::builder::MeshBatch;
use bevy::{
    asset::RenderAssetUsages,
    mesh::{AttributeQuantization, PrimitiveTopology, VertexAttributeValues},
    prelude::Mesh,
};

fn enabled_for(smoke_test: bool, setting: Option<&str>) -> bool {
    !(smoke_test && setting == Some("0"))
}

fn enabled() -> bool {
    static ENABLED: std::sync::LazyLock<bool> = std::sync::LazyLock::new(|| {
        enabled_for(
            std::env::args().any(|arg| arg.starts_with("--smoke-test")),
            std::env::var("TERRA_HALF_TINTS").ok().as_deref(),
        )
    });
    *ENABLED
}

fn safe_colors(colors: &[[f32; 4]]) -> bool {
    colors
        .iter()
        .flatten()
        .all(|value| value.is_finite() && value.abs() <= 65_504.0)
}

#[cfg(test)]
fn compact_tints(mesh: &mut Mesh) {
    if !enabled() {
        return;
    }
    let Ok(VertexAttributeValues::Float32x4(colors)) = mesh.try_attribute(Mesh::ATTRIBUTE_COLOR)
    else {
        return;
    };
    if !safe_colors(colors) {
        return;
    }
    // Float16 vertex input widens to the same shader vec4, including water flags.
    let _ = mesh.quantize_colors(AttributeQuantization::Float16);
}

pub(super) fn compact_batch_tints(batch: &mut MeshBatch) {
    if batch.staged.is_some()
        || batch.half_colors.is_some()
        || !batch.packed_colors.is_empty()
        || batch.colors.is_empty()
        || !enabled()
        || !safe_colors(&batch.colors)
    {
        return;
    }
    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::RENDER_WORLD,
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, std::mem::take(&mut batch.colors));
    let _ = mesh.quantize_colors(AttributeQuantization::Float16);
    // A failed quantization leaves this local mesh's original Float32 attribute.
    match mesh
        .remove_attribute(Mesh::ATTRIBUTE_COLOR)
        .expect("Color-only mesh retains its attribute")
    {
        colors @ VertexAttributeValues::Float16x4(_) => batch.half_colors = Some(colors),
        VertexAttributeValues::Float32x4(colors) => batch.colors = colors,
        _ => unreachable!("Float32 tint quantization returned an unrelated vertex format"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::{
        asset::RenderAssetUsages,
        mesh::{
            Indices, MeshAttributeCompressionFlags, MeshVertexAttribute, MeshVertexBufferLayouts,
            PrimitiveTopology,
        },
        render::render_resource::VertexFormat,
    };

    fn fixture(colors: Vec<[f32; 4]>) -> Mesh {
        let count = colors.len();
        Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::RENDER_WORLD,
        )
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, vec![[1.0, 2.0, 3.0]; count])
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, vec![[0.0, 1.0, 0.0]; count])
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, vec![[0.25, 0.75]; count])
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_1, vec![[0.125, -1.0]; count])
        .with_inserted_attribute(Mesh::ATTRIBUTE_TANGENT, vec![[1.0, 0.0, 0.0, -1.0]; count])
        .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, colors)
        .with_inserted_indices(Indices::U16(vec![0, 1, 2]))
    }

    fn unpacked(mesh: &Mesh) -> Vec<[f32; 4]> {
        let Some(VertexAttributeValues::Float16x4(colors)) = mesh.attribute(Mesh::ATTRIBUTE_COLOR)
        else {
            panic!("Expected half-float tint storage");
        };
        colors
            .iter()
            .map(|color| (*color).map(|value| value.to_f32()))
            .collect()
    }

    #[test]
    fn hdr_signs_alpha_flags_and_unrelated_vertex_payloads_are_preserved() {
        let original = vec![
            [1.337, -0.125, 2.375, 4.0],
            [2.85, 0.647, -0.037, 6.0],
            [65_504.0, -65_504.0, 0.0, 5.0],
            [0.75, 1.0, 0.25, 0.0],
            [0.0, 0.5, 1.0, 1.0],
        ];
        let mut mesh = fixture(original.clone());
        let attributes: Vec<_> = [
            Mesh::ATTRIBUTE_POSITION,
            Mesh::ATTRIBUTE_NORMAL,
            Mesh::ATTRIBUTE_TANGENT,
            Mesh::ATTRIBUTE_UV_0,
            Mesh::ATTRIBUTE_UV_1,
        ]
        .into_iter()
        .map(|attribute| (attribute, mesh.attribute(attribute).unwrap().clone()))
        .collect();
        let indices = mesh.indices().unwrap().clone();
        compact_tints(&mut mesh);
        for (original, packed) in original.iter().zip(unpacked(&mesh)) {
            for channel in 0..4 {
                assert!(
                    (original[channel] - packed[channel]).abs() <= 0.001,
                    "channel{channel}: {} became {}",
                    original[channel],
                    packed[channel]
                );
            }
            assert_eq!(original[3], packed[3], "semantic alpha flags are exact");
        }
        for (attribute, values) in attributes {
            assert_eq!(mesh.attribute(attribute), Some(&values));
        }
        assert_eq!(mesh.indices(), Some(&indices));
        let layout = mesh.get_mesh_vertex_buffer_layout(&mut MeshVertexBufferLayouts::default());
        assert_eq!(
            layout.0.get_attribute_compression(),
            MeshAttributeCompressionFlags::empty()
        );
        assert_eq!(
            layout
                .0
                .get_layout(&[Mesh::ATTRIBUTE_COLOR.at_shader_location(5)])
                .unwrap()
                .attributes[0]
                .format,
            VertexFormat::Float16x4
        );
        let (attribute, _) = mesh
            .attributes()
            .find(|(attribute, _)| attribute.id == Mesh::ATTRIBUTE_COLOR.id)
            .unwrap();
        assert_eq!(attribute.format, VertexFormat::Float16x4);
    }

    #[test]
    fn color_payload_halves_from_sixteen_to_eight_bytes_per_vertex() {
        let mut mesh = fixture(vec![[1.25, 0.375, -0.125, 6.0]; 3]);
        let before = mesh.get_vertex_buffer_size();
        assert_eq!(
            mesh.attribute(Mesh::ATTRIBUTE_COLOR)
                .unwrap()
                .get_bytes()
                .len(),
            3 * 16
        );
        compact_tints(&mut mesh);
        assert_eq!(
            mesh.attribute(Mesh::ATTRIBUTE_COLOR)
                .unwrap()
                .get_bytes()
                .len(),
            3 * 8
        );
        assert_eq!(mesh.get_vertex_buffer_size(), before - 3 * 8);
    }

    #[test]
    fn direct_and_evergreen_temperature_tags_stay_within_a_tenth_degree() {
        use crate::weather::storage::snow::encode_temperature;
        let temperatures: Vec<_> = (-1024..=1024).map(|value| value as f32 / 16.0).collect();
        let colors = temperatures
            .iter()
            .flat_map(|&temperature| {
                let tag = encode_temperature(temperature);
                [[0.25, 0.5, 1.25, tag], [0.25, 0.5, 1.25, 0.5 + tag * 0.49]]
            })
            .collect();
        let mut mesh = fixture(colors);
        compact_tints(&mut mesh);
        let packed = unpacked(&mesh);
        for (&temperature, pair) in temperatures.iter().zip(packed.chunks_exact(2)) {
            let direct = pair[0][3];
            assert!(direct > 0.0 && direct < 1.0);
            let evergreen = pair[1][3];
            assert!((0.5..0.999).contains(&evergreen));
            for tag in [direct, (evergreen - 0.5) / 0.49] {
                let decoded = ((tag * 255.0 - 2.0) / 251.0) * 128.0 - 64.0;
                assert!(
                    (decoded - temperature).abs() < 0.1,
                    "temperature{temperature} became{decoded}"
                );
            }
        }
    }

    #[test]
    fn nonfinite_and_out_of_half_range_channels_leave_the_entire_batch_float32() {
        for invalid in [
            f32::NAN,
            f32::INFINITY,
            f32::NEG_INFINITY,
            65_505.0,
            -65_505.0,
            f32::MAX,
        ] {
            for channel in 0..4 {
                let mut original = vec![[0.25, 0.5, 1.25, 6.0]; 3];
                original[1][channel] = invalid;
                let mut mesh = fixture(original.clone());
                compact_tints(&mut mesh);
                let Some(VertexAttributeValues::Float32x4(actual)) =
                    mesh.attribute(Mesh::ATTRIBUTE_COLOR)
                else {
                    panic!("Unsafe tint values must retain Float32 storage");
                };
                for (original, actual) in original.iter().flatten().zip(actual.iter().flatten()) {
                    assert_eq!(original.to_bits(), actual.to_bits());
                }
            }
        }
    }

    #[test]
    fn existing_normalized_and_half_formats_are_idempotent_and_missing_data_is_safe() {
        for (format, colors) in [
            (
                VertexFormat::Unorm8x4,
                VertexAttributeValues::Unorm8x4(vec![[32, 64, 128, 255]; 3]),
            ),
            (
                VertexFormat::Unorm16x4,
                VertexAttributeValues::Unorm16x4(vec![[32, 64, 128, 65_535]; 3]),
            ),
        ] {
            let mut mesh = fixture(vec![[1.0; 4]; 3]);
            mesh.insert_attribute(
                MeshVertexAttribute {
                    format,
                    ..Mesh::ATTRIBUTE_COLOR
                },
                colors.clone(),
            );
            compact_tints(&mut mesh);
            assert_eq!(mesh.attribute(Mesh::ATTRIBUTE_COLOR), Some(&colors));
        }
        let mut mesh = fixture(vec![[1.25, 0.5, -0.25, 4.0]; 3]);
        compact_tints(&mut mesh);
        let colors = mesh.attribute(Mesh::ATTRIBUTE_COLOR).unwrap().clone();
        compact_tints(&mut mesh);
        assert_eq!(mesh.attribute(Mesh::ATTRIBUTE_COLOR), Some(&colors));
        mesh.remove_attribute(Mesh::ATTRIBUTE_COLOR);
        compact_tints(&mut mesh);
        assert!(!mesh.contains_attribute(Mesh::ATTRIBUTE_COLOR));
        let _gpu = mesh.take_gpu_data().unwrap();
        compact_tints(&mut mesh);
        assert!(mesh.try_attribute(Mesh::ATTRIBUTE_COLOR).is_err());
    }

    #[test]
    fn disabling_half_tints_is_only_allowed_for_the_explicit_smoke_control() {
        for setting in [None, Some("0"), Some("1"), Some("false")] {
            assert!(enabled_for(false, setting));
            assert_eq!(enabled_for(true, setting), setting != Some("0"));
        }
    }

    fn worker_fixture() -> MeshBatch {
        MeshBatch {
            material: 3,
            shadow_sources: vec![1, 2],
            positions: vec![[1.0, 2.0, 3.0]; 3],
            normals: vec![[0.0, 1.0, 0.0]; 3],
            tangents: vec![[1.0, 0.0, 0.0, -1.0]; 3],
            uvs: vec![[0.25, 0.75]; 3],
            rooted_wind: vec![[0.125, 0.75]; 3],
            surface_water: vec![[0.5, -1.0]; 3],
            growth_anchors: vec![[0.25, 0.5, 0.75]; 3],
            colors: vec![
                [1.337, -0.125, 2.375, 4.0],
                [2.85, 0.647, -0.037, 6.0],
                [1.25, 0.75, -0.25, 0.5],
            ],
            indices: vec![2, 1, 0],
            ..Default::default()
        }
    }

    #[test]
    fn worker_moves_only_tints_to_the_eight_byte_payload() {
        let mut batch = worker_fixture();
        let original = batch.colors.clone();
        let mut expected = worker_fixture();
        expected.colors.clear();
        compact_batch_tints(&mut batch);
        let packed = batch.half_colors.take().unwrap();
        assert_eq!(batch, expected, "geometry and metadata must not change");
        assert_eq!(packed.get_bytes().len(), original.len() * 8);
        let VertexAttributeValues::Float16x4(colors) = packed else {
            panic!("Worker tint storage must be Float16x4");
        };
        for (original, packed) in original.iter().zip(colors) {
            for channel in 0..4 {
                assert!((original[channel] - packed[channel].to_f32()).abs() <= 0.001);
            }
            assert_eq!(original[3], packed[3].to_f32());
        }
    }

    #[test]
    fn worker_invalid_values_retain_the_original_float_allocation_and_other_payloads() {
        for invalid in [
            f32::NAN,
            f32::INFINITY,
            f32::NEG_INFINITY,
            65_505.0,
            -65_505.0,
        ] {
            for channel in 0..4 {
                let mut batch = worker_fixture();
                batch.colors[1][channel] = invalid;
                let original = batch.colors.clone();
                let allocation = batch.colors.as_ptr();
                compact_batch_tints(&mut batch);
                assert!(batch.half_colors.is_none());
                assert_eq!(batch.colors.as_ptr(), allocation);
                for (original, actual) in
                    original.iter().flatten().zip(batch.colors.iter().flatten())
                {
                    assert_eq!(original.to_bits(), actual.to_bits());
                }
                batch.colors.clear();
                let mut expected = worker_fixture();
                expected.colors.clear();
                assert_eq!(batch, expected);
            }
        }
    }

    #[test]
    fn worker_is_idempotent_and_leaves_existing_packed_tints_untouched() {
        let mut batch = worker_fixture();
        compact_batch_tints(&mut batch);
        let colors = batch.half_colors.as_ref().unwrap().clone();
        let allocation = batch.half_colors.as_ref().unwrap().get_bytes().as_ptr();
        compact_batch_tints(&mut batch);
        assert_eq!(batch.half_colors.as_ref(), Some(&colors));
        assert_eq!(
            batch.half_colors.as_ref().unwrap().get_bytes().as_ptr(),
            allocation
        );
        assert!(batch.colors.is_empty());
        let mut expected = worker_fixture();
        expected.colors.clear();
        expected.half_colors = Some(colors);
        assert_eq!(batch, expected);

        let mut packed = worker_fixture();
        packed.packed_colors = vec![[32, 64, 128, 255]; 3];
        let mut expected = worker_fixture();
        expected.packed_colors = packed.packed_colors.clone();
        compact_batch_tints(&mut packed);
        assert_eq!(packed, expected);
        let mut empty = MeshBatch::default();
        compact_batch_tints(&mut empty);
        assert_eq!(empty, MeshBatch::default());
    }
}
