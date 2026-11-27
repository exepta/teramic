use crate::meshing::builder::MeshBatch;

/// Use the same active texture-channel switches as Bevy's forward and prepass shaders.
pub(super) fn uses_primary_uv_only(material: &bevy::pbr::StandardMaterial) -> bool {
    use bevy::pbr::StandardMaterialKey as K;
    !K::from(material).intersects(
        K::BASE_COLOR_UV
            | K::EMISSIVE_UV
            | K::METALLIC_ROUGHNESS_UV
            | K::OCCLUSION_UV
            | K::SPECULAR_TRANSMISSION_UV
            | K::THICKNESS_UV
            | K::DIFFUSE_TRANSMISSION_UV
            | K::NORMAL_MAP_UV
            | K::ANISOTROPY_UV
            | K::CLEARCOAT_UV
            | K::CLEARCOAT_ROUGHNESS_UV
            | K::CLEARCOAT_NORMAL_UV
            | K::SPECULAR_UV
            | K::SPECULAR_TINT_UV,
    )
}

/// In a crown, only negative exposure selects submerged motion. Nonnegative
/// terrain weather payloads follow the same leaf shader path as absent UV1.
/// Chunk leaves have no lightmap; fading clones the original material bindings.
pub(crate) fn omit_unused_exposure(batch: &mut MeshBatch) {
    if !batch.shadow_only
        && batch.rooted_wind.is_empty()
        && batch.surface_water.iter().all(|uv| uv[1] >= 0.0)
    {
        batch.surface_water = Vec::new();
    }
}

pub(crate) fn enabled() -> bool {
    static ENABLED: std::sync::LazyLock<bool> = std::sync::LazyLock::new(|| {
        !(std::env::var("TERRA_BENCH_UNPACKED_FOLIAGE").is_ok_and(|v| v == "1")
            && std::env::args().any(|v| v.starts_with("--smoke-test")))
    });
    *ENABLED
}

/// Run on the meshing worker. Preserve positions/UVs/indices and hard normals;
/// crown colors occupy 0..2 linear RGB, decoded by the existing leaf shader.
pub(crate) fn pack(batch: &mut MeshBatch) {
    batch.finish_tangents();
    batch.pack_unit_tangents();
    batch.packed_normals = std::mem::take(&mut batch.normals)
        .into_iter()
        .map(normal)
        .collect();
    batch.packed_colors = std::mem::take(&mut batch.colors)
        .into_iter()
        .map(color)
        .collect();
    // In-place collection can retain the old float allocation capacity.
    // Release that slack before ready jobs wait on upload budgets.
    batch.packed_normals.shrink_to_fit();
    batch.packed_colors.shrink_to_fit();
}

pub(crate) fn normal(n: [f32; 3]) -> [i8; 4] {
    [
        (n[0].clamp(-1.0, 1.0) * 127.0).round() as i8,
        (n[1].clamp(-1.0, 1.0) * 127.0).round() as i8,
        (n[2].clamp(-1.0, 1.0) * 127.0).round() as i8,
        0,
    ]
}

pub(crate) fn color(c: [f32; 4]) -> [u8; 4] {
    [
        (c[0].clamp(0.0, 2.0) * 127.5).round() as u8,
        (c[1].clamp(0.0, 2.0) * 127.5).round() as u8,
        (c[2].clamp(0.0, 2.0) * 127.5).round() as u8,
        (c[3].clamp(0.0, 1.0) * 255.0).round() as u8,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn dry_crown_uv_omission_keeps_underwater_rooted_shadow_and_authored_bindings() {
        use bevy::{mesh::UvChannel, pbr::StandardMaterial};
        assert!(uses_primary_uv_only(&StandardMaterial::default()));
        for material in [
            StandardMaterial {
                base_color_channel: UvChannel::Uv1,
                ..Default::default()
            },
            StandardMaterial {
                normal_map_channel: UvChannel::Uv1,
                ..Default::default()
            },
            StandardMaterial {
                emissive_channel: UvChannel::Uv1,
                ..Default::default()
            },
            StandardMaterial {
                occlusion_channel: UvChannel::Uv1,
                ..Default::default()
            },
            StandardMaterial {
                metallic_roughness_channel: UvChannel::Uv1,
                ..Default::default()
            },
        ] {
            assert!(!uses_primary_uv_only(&material));
        }
        for (exposure, rooted, shadow, removed) in [
            (0.0, false, false, true),
            (1.8, false, false, true),
            (-1.0, false, false, false),
            (0.5, true, false, false),
            (0.5, false, true, false),
        ] {
            let mut batch = MeshBatch {
                surface_water: vec![[4.0, 0.0], [4096.0, exposure]],
                rooted_wind: if rooted { vec![[0.2, 1.0]; 2] } else { vec![] },
                shadow_only: shadow,
                ..Default::default()
            };
            omit_unused_exposure(&mut batch);
            assert_eq!(batch.surface_water.is_empty(), removed);
        }
    }

    #[test]
    fn packed_crowns_keep_geometry_normals_and_hdr_tints_with_bounded_error() {
        let mut batch = MeshBatch {
            positions: vec![[1.0, 2.0, 3.0]; 3],
            normals: vec![[0.0, 1.0, 0.0]; 3],
            uvs: vec![[4.0, 5.0]; 3],
            colors: vec![[1.06, 0.55, 0.38, 1.0]; 3],
            indices: vec![0, 1, 2],
            ..Default::default()
        };
        pack(&mut batch);
        assert_eq!(batch.positions, vec![[1.0, 2.0, 3.0]; 3]);
        assert_eq!(batch.uvs, vec![[4.0, 5.0]; 3]);
        assert_eq!(batch.indices, [0, 1, 2]);
        assert_eq!(batch.packed_normals, vec![[0, 127, 0, 0]; 3]);
        for (value, original) in batch.packed_colors[0][..3].iter().zip([1.06, 0.55, 0.38]) {
            assert!((f32::from(*value) / 127.5 - original).abs() <= 1.0 / 255.0);
        }
        assert_eq!(batch.packed_colors[0][3], 255);
        assert!(batch.normals.is_empty() && batch.colors.is_empty());
        assert_eq!(batch.packed_normals.capacity(), batch.packed_normals.len());
        assert_eq!(batch.packed_colors.capacity(), batch.packed_colors.len());
    }
}
