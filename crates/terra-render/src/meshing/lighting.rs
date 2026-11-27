//! Smooth contact lighting from the mesher's immutable, edit-aware halo.
use super::builder::MeshBatch;
use crate::blocks::TemplateSet;

#[path = "lighting_greedy.rs"]
mod greedy;

fn occupied(bits: &[u64], p: [i32; 3]) -> f32 {
    if p.iter().any(|v| !(-1..=16).contains(v)) {
        return 0.0;
    }
    let [x, y, z] = p.map(|v| (v + 1) as usize);
    let i = x + 18 * (y + 18 * z);
    f32::from(u8::from(
        bits.get(i / 64)
            .is_some_and(|word| word & (1 << (i % 64)) != 0),
    ))
}

fn outside_corner(position: f32, positive: bool) -> i32 {
    // Inset relief must probe the outside logical face, avoiding self-darkening.
    if positive {
        (position - 0.00001).ceil() as i32
    } else {
        (position + 0.00001).floor() as i32 - 1
    }
}

fn corner_shade(
    bits: &[u64],
    corner: [i32; 3],
    axis: usize,
    positive: bool,
    cache: &mut [u8],
) -> f32 {
    let mut lattice = corner;
    lattice[axis] += i32::from(!positive);
    let key = lattice.iter().all(|v| (0..=16).contains(v)).then(|| {
        let [x, y, z] = lattice.map(|v| v as usize);
        ((axis * 2 + usize::from(positive)) * 17 * 17 * 17) + x + 17 * (y + 17 * z)
    });
    let stored = key.and_then(|key| cache.get(key)).copied().unwrap_or(255);
    let occlusion = if stored != 255 {
        f32::from(stored)
    } else {
        let u = (axis + 1) % 3;
        let v = (axis + 2) % 3;
        let mut value = 0.0;
        for du in [-1, 0] {
            for dv in [-1, 0] {
                let mut cell = corner;
                cell[u] += du;
                cell[v] += dv;
                value += occupied(bits, cell);
            }
        }
        if let Some(key) = key.and_then(|key| cache.get_mut(key)) {
            *key = value as u8;
        }
        value
    };
    1.0 - (occlusion * 0.12).min(0.40)
}

fn dominant_axis(n: [f32; 3]) -> usize {
    // Ties select the last matching axis, as with Iterator::max_by.
    let [x, y, z] = n.map(|v| v.abs().to_bits());
    if x > y && x > z {
        0
    } else if y > z {
        1
    } else {
        2
    }
}

#[derive(Clone, Copy)]
struct ShadePatch {
    base: [i32; 3],
    corners: [f32; 4],
}

impl ShadePatch {
    const EMPTY: Self = Self {
        base: [i32::MIN; 3],
        corners: [0.; 4],
    };

    /// Hundreds of relief vertices on a block interpolate the same four samples.
    /// Keep the last footprint per direction so even alternating bevels and caps hit.
    fn sample(
        patches: &mut [Self; 6],
        bits: &[u64],
        p: [f32; 3],
        n: [f32; 3],
        cache: &mut [u8],
    ) -> f32 {
        let axis = dominant_axis(n);
        let positive = n[axis] >= 0.0;
        let u = (axis + 1) % 3;
        let v = (axis + 2) % 3;
        let mut base = p.map(|x| x.floor() as i32);
        base[axis] = outside_corner(p[axis], positive);
        let fu = p[u] - base[u] as f32;
        let fv = p[v] - base[v] as f32;
        if fu == 0.0 && fv == 0.0 {
            return corner_shade(bits, base, axis, positive, cache);
        }
        let patch = &mut patches[axis * 2 + usize::from(positive)];
        if patch.base != base {
            patch.base = base;
            patch.corners = std::array::from_fn(|i| {
                let mut corner = base;
                corner[u] += (i / 2) as i32;
                corner[v] += (i % 2) as i32;
                corner_shade(bits, corner, axis, positive, cache)
            });
        }
        let [a, b, c, d] = patch.corners;
        a * ((1.0 - fu) * (1.0 - fv))
            + b * ((1.0 - fu) * fv)
            + c * (fu * (1.0 - fv))
            + d * (fu * fv)
    }
}

// Greedy refinement samples individual integer grid corners before the final
// color pass. The integer fast path only reads the shared corner lattice.
fn sample(bits: &[u64], p: [f32; 3], n: [f32; 3], cache: &mut [u8]) -> f32 {
    ShadePatch::sample(&mut [ShadePatch::EMPTY; 6], bits, p, n, cache)
}

pub(super) fn apply(batches: &mut [MeshBatch], bits: &[u64], templates: &TemplateSet) {
    if bits.is_empty() {
        return;
    }
    // A scratch lattice shared by every material; each exposed corner is probed once.
    let mut cache = vec![255; 6 * 17 * 17 * 17];
    let unshaded =
        [14, 15, 27, 28, 29, 40, terra_world::lava::LAVA].map(|id| templates.blocks.get(&id));
    for batch in batches {
        if batch.shadow_only
            || templates.foliage_material(batch.material)
            || unshaded
                .iter()
                .flatten()
                .any(|block| block.face_materials.contains(&batch.material))
        {
            continue;
        }
        greedy::refine(batch, bits, &mut cache);
        batch.colors.resize(batch.positions.len(), [1.0; 4]);
        let mut patches = [ShadePatch::EMPTY; 6];
        for ((&p, &n), color) in batch
            .positions
            .iter()
            .zip(&batch.normals)
            .zip(&mut batch.colors)
        {
            let shade = ShadePatch::sample(&mut patches, bits, p, n, &mut cache);
            for value in &mut color[..3] {
                *value *= shade;
            }
        }
        // Standard PBR decodes ordinary unorm colors directly. Do not use the
        // foliage's separate 0..2 encoding or clamp HDR furniture/biome tints.
        if batch
            .colors
            .iter()
            .flatten()
            .all(|v| (0.0..=1.0).contains(v))
        {
            batch.packed_colors = std::mem::take(&mut batch.colors)
                .into_iter()
                .map(|color| color.map(|v| (v * 255.0).round() as u8))
                .collect();
            batch.packed_colors.shrink_to_fit();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cluster_materials_retain_linear_tints_until_crown_packing() {
        use crate::blocks::FoliageClusterMaterials;
        let mut templates = TemplateSet::default();
        templates.foliage_clusters.insert(
            7,
            FoliageClusterMaterials {
                leaves: 500,
                needles: Some(501),
            },
        );
        let tint = [0.83, 0.62, 0.54, 0.35];
        for material in [500, 501] {
            let mut batch = MeshBatch {
                material,
                positions: vec![[0., 1., 0.]; 3],
                normals: vec![[0., 1., 0.]; 3],
                colors: vec![tint; 3],
                ..Default::default()
            };
            apply(std::slice::from_mut(&mut batch), &[0; 92], &templates);
            assert_eq!(batch.colors, [tint; 3]);
            assert!(batch.packed_colors.is_empty());
            crate::foliage::packing::pack(&mut batch);
            assert_eq!(
                batch.packed_colors,
                [crate::foliage::packing::color(tint); 3]
            );
        }
    }

    fn uncached_sample(bits: &[u64], p: [f32; 3], n: [f32; 3], cache: &mut [u8]) -> f32 {
        let axis = dominant_axis(n);
        let positive = n[axis] >= 0.0;
        let u = (axis + 1) % 3;
        let v = (axis + 2) % 3;
        let mut base = p.map(|x| x.floor() as i32);
        base[axis] = outside_corner(p[axis], positive);
        let fu = p[u] - base[u] as f32;
        let fv = p[v] - base[v] as f32;
        if fu == 0.0 && fv == 0.0 {
            return corner_shade(bits, base, axis, positive, cache);
        }
        let mut result = 0.0;
        for a in 0..2 {
            for b in 0..2 {
                let weight =
                    if a == 0 { 1.0 - fu } else { fu } * if b == 0 { 1.0 - fv } else { fv };
                if weight > 0.0 {
                    let mut corner = base;
                    corner[u] += a;
                    corner[v] += b;
                    result += corner_shade(bits, corner, axis, positive, cache) * weight;
                }
            }
        }
        result
    }

    #[test]
    fn relief_contact_patches_equal_individual_samples_across_bevels_and_section_edges() {
        let bits: Vec<u64> = (0_u64..92)
            .map(|i| i.wrapping_mul(0xf943_8aec_f014_fb7b))
            .collect();
        let mut patches = [ShadePatch::EMPTY; 6];
        let mut cache = vec![255; 6 * 17 * 17 * 17];
        for face in 0..6 {
            let mut normal = [0.36; 3];
            normal[face / 2] = if face % 2 == 0 { -0.86 } else { 0.86 };
            for x in [-0.18, 0.0, 0.18, 15.5, 16.0, 16.18] {
                for y in [0.0, 0.001, 5.5, 6.0, 15.9, 16.0] {
                    for z in [0.0, 0.03, 5.25, 15.8, 16.0] {
                        let p = [x, y, z];
                        assert_eq!(
                            ShadePatch::sample(&mut patches, &bits, p, normal, &mut cache)
                                .to_bits(),
                            uncached_sample(&bits, p, normal, &mut []).to_bits()
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn empty_and_transparent_halos_are_bright_and_contact_is_smooth() {
        let mut bits = vec![0; terra_voxel::meshing::PADDED_VOLUME.div_ceil(64)];
        assert_eq!(
            sample(&bits, [1.0, 1.0, 1.0], [0.0, 1.0, 0.0], &mut []),
            1.0
        );
        let i = 1 + 18 * (2 + 18);
        bits[i / 64] |= 1 << (i % 64);
        let a = sample(&bits, [1.0, 1.0, 1.0], [0.0, 1.0, 0.0], &mut []);
        let b = sample(&bits, [1.001, 1.0, 1.001], [0.0, 1.0, 0.0], &mut []);
        assert!((0.6..1.0).contains(&a) && (a - b).abs() < 0.001);
    }
    #[test]
    fn halo_lighting_agrees_at_negative_section_boundaries_and_cache_is_exact() {
        let bits = |origin: i32| {
            let mut bits = vec![0_u64; terra_voxel::meshing::PADDED_VOLUME.div_ceil(64)];
            for z in -1..=16 {
                for y in -1..=16 {
                    for x in -1..=16 {
                        if y == 1 && x + origin == -1 && z == 0 {
                            let i = ((x + 1) + 18 * ((y + 1) + 18 * (z + 1))) as usize;
                            bits[i / 64] |= 1 << (i % 64);
                        }
                    }
                }
            }
            bits
        };
        let left = sample(&bits(-16), [16.0, 1.0, 1.0], [0.0, 1.0, 0.0], &mut []);
        let right = sample(&bits(0), [0.0, 1.0, 1.0], [0.0, 1.0, 0.0], &mut []);
        assert_eq!(left, right);
        assert!(left < 1.0);
        let mut cache = vec![255; 6 * 17 * 17 * 17];
        assert_eq!(
            right,
            sample(&bits(0), [0.0, 1.0, 1.0], [0.0, 1.0, 0.0], &mut cache)
        );
    }
}
