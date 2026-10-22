use crate::meshing::builder::MeshBatch;

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
