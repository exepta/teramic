//! Smooth contact lighting from the mesher's immutable, edit-aware halo.
use super::builder::MeshBatch;
use crate::blocks::TemplateSet;

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

fn sample(bits: &[u64], p: [f32; 3], n: [f32; 3], cache: &mut [u8]) -> f32 {
    let axis = (0..3)
        .max_by(|&a, &b| n[a].abs().total_cmp(&n[b].abs()))
        .unwrap_or(1);
    let u = (axis + 1) % 3;
    let v = (axis + 2) % 3;
    let base = p.map(|x| x.floor() as i32);
    let fu = p[u] - base[u] as f32;
    let fv = p[v] - base[v] as f32;
    let mut result = 0.0;
    for a in 0..2 {
        for b in 0..2 {
            let weight = if a == 0 { 1.0 - fu } else { fu } * if b == 0 { 1.0 - fv } else { fv };
            if weight <= 0.0 {
                continue;
            }
            let mut corner = base;
            corner[u] += a;
            corner[v] += b;
            // Bevel/relief vertices are inset into the owner. Probe the outside
            // logical face, not that owner's solid cell (which would self-darken).
            corner[axis] = if n[axis] >= 0.0 {
                (p[axis] - 0.00001).ceil() as i32
            } else {
                (p[axis] + 0.00001).floor() as i32 - 1
            };
            let mut lattice = corner;
            lattice[axis] += i32::from(n[axis] < 0.0);
            let key = lattice.iter().all(|v| (0..=16).contains(v)).then(|| {
                let [x, y, z] = lattice.map(|v| v as usize);
                ((axis * 2 + usize::from(n[axis] >= 0.0)) * 17 * 17 * 17) + x + 17 * (y + 17 * z)
            });
            let stored = key.and_then(|key| cache.get(key)).copied().unwrap_or(255);
            let occlusion = if stored != 255 {
                f32::from(stored)
            } else {
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
            result += (1.0 - (occlusion * 0.12).min(0.40)) * weight;
        }
    }
    result
}

pub(super) fn apply(batches: &mut [MeshBatch], bits: &[u64], templates: &TemplateSet) {
    if bits.is_empty() {
        return;
    }
    // A scratch lattice shared by every material; each exposed corner is probed once.
    let mut cache = vec![255; 6 * 17 * 17 * 17];
    for batch in batches {
        if batch.shadow_only
            || templates.blocks.iter().any(|(&id, block)| {
                matches!(id, 7..=9 | 14 | 15 | 27..=29 | 40)
                    && block.face_materials.contains(&batch.material)
            })
        {
            continue;
        }
        batch.colors.resize(batch.positions.len(), [1.0; 4]);
        for ((&p, &n), color) in batch
            .positions
            .iter()
            .zip(&batch.normals)
            .zip(&mut batch.colors)
        {
            let shade = sample(bits, p, n, &mut cache);
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
