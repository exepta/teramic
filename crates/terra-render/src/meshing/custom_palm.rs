//! Natural palm fronds follow the generated curve across voxel and section boundaries.
use super::MeshBatch;
use bevy::math::Vec3;
use std::collections::BTreeMap;
use terra_world::generation::PalmSpan;

/// A folded spine and paired tapering leaflets form one silhouette. Sampling the
/// same longitudinal knots in every cell makes both sides of every seam coincide.
pub(crate) fn append(
    batches: &mut BTreeMap<usize, MeshBatch>,
    slot: usize,
    origin: [i32; 3],
    spans: &[PalmSpan],
) {
    let batch = batches.entry(slot).or_insert_with(|| MeshBatch {
        material: slot,
        ..Default::default()
    });
    for span in spans {
        let [a, b] = span.endpoints.map(Vec3::from_array);
        let [lo, hi] = span.progress;
        let side = Vec3::new(-(b.z - a.z), 0., b.x - a.x).normalize_or_zero();
        let offset = Vec3::from_array(origin.map(|i| i as f32));
        let knots = (1..28)
            .map(|i| i as f32 / 28.0)
            .filter(|t| *t > lo + 1e-6 && *t < hi - 1e-6)
            .chain([hi]);
        let mut last = lo;
        for t in knots {
            let a_center = offset + a.lerp(b, (last - lo) / (hi - lo));
            let b_center = offset + a.lerp(b, (t - lo) / (hi - lo));
            // The central blade remains broad and continuous beneath the serrated margins.
            let profile = |u: f32| {
                let envelope = (std::f32::consts::PI * u).sin().max(0.).sqrt() * (1. - u * 0.30);
                let tooth = 1. - ((u * 14.).rem_euclid(1.) * 2. - 1.).abs();
                let width = 0.022 * (1. - u) + envelope * (0.43 + 0.72 * tooth);
                (width, 0.05 + 0.45 * envelope)
            };
            let (w0, h0) = profile(last);
            let (w1, h1) = profile(t);
            for sign in [-1.0, 1.0] {
                let ribs = [
                    a_center + Vec3::Y * (h0 * 0.13),
                    b_center + Vec3::Y * (h1 * 0.13),
                    b_center + side * sign * w1 - Vec3::Y * h1,
                    a_center + side * sign * w0 - Vec3::Y * h0,
                ];
                let mut n = (ribs[1] - ribs[0])
                    .cross(ribs[3] - ribs[0])
                    .normalize_or_zero();
                if n.y < 0. {
                    n = -n
                }
                let tint = if sign > 0. {
                    [0.88, 1., 0.67, 1.]
                } else {
                    [0.69, 0.88, 0.56, 1.]
                };
                for face in [1., -1.] {
                    let base = batch.positions.len() as u32;
                    batch.positions.extend(ribs.map(|p| p.to_array()));
                    batch.normals.extend([(n * face).to_array(); 4]);
                    batch.uvs.extend([[last, 0.], [t, 0.], [t, 1.], [last, 1.]]);
                    batch.colors.extend([tint; 4]);
                    if sign * face < 0. {
                        batch
                            .indices
                            .extend([base, base + 1, base + 2, base, base + 2, base + 3])
                    } else {
                        batch
                            .indices
                            .extend([base + 2, base + 1, base, base + 3, base + 2, base])
                    }
                }
            }
            last = t;
        }
    }
}
