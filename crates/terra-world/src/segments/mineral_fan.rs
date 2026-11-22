//! Grow adjacent shafts into one body. Both owners of a horizontal face
//! derive the same rim from the cells straddling each world-space height.
use super::{Profile, mineral_progress, root_block, shaft_radius};

const AROUND: [[i64; 2]; 8] = [
    [1, 0],
    [1, 1],
    [0, 1],
    [-1, 1],
    [-1, 0],
    [-1, -1],
    [0, -1],
    [1, -1],
];

pub(super) fn connections(
    [x, y, z]: [i64; 3],
    material: u16,
    block: &mut impl FnMut([i64; 3]) -> Option<u16>,
) -> [u8; 2] {
    let solid = |m: Option<u16>| m.is_some_and(|m| m == material || root_block(m));
    let own = [solid(block([x, y - 1, z])), solid(block([x, y + 1, z]))];
    let mut masks = [0; 2];
    if own == [false; 2] {
        return masks;
    }
    for (i, [dx, dz]) in AROUND.into_iter().enumerate() {
        let middle = block([x + dx, y, z + dz]);
        if !solid(middle) {
            continue;
        }
        for (plane, dy) in [-1, 1].into_iter().enumerate() {
            if own[plane] {
                let end = block([x + dx, y + dy, z + dz]);
                if solid(end) && (middle == Some(material) || end == Some(material)) {
                    masks[plane] |= 1 << i;
                }
            }
        }
    }
    masks
}

pub(super) fn visit(
    material: u16,
    profile: Profile,
    seed: u64,
    coarse: bool,
    emit: &mut impl FnMut([[f32; 3]; 3], [f32; 3]),
) {
    let bevel = match material {
        51 => 0.65,
        53 => 0.53,
        _ => 0.34,
    };
    let (s, c) = (((seed & 255) as f32 / 255. - 0.5) * 0.14).sin_cos();
    let cant = [
        ((seed >> 16 & 255) as f32 / 255. - 0.5) * 0.19,
        ((seed >> 28 & 255) as f32 / 255. - 0.5) * 0.19,
    ];
    let outline = |y: f32, i: usize, mask: u8| {
        let face = i & !1;
        let clockwise = i % 2 == 1;
        let sign = if clockwise { 1. } else { -1. };
        let [nx, nz] = AROUND[face].map(|v| v as f32);
        let attached = mask & (1 << face) != 0;
        let adjacent = (face + if clockwise { 2 } else { 6 }) % 8;
        let diagonal = (face + if clockwise { 1 } else { 7 }) % 8;
        let corner = attached && mask & (1 << adjacent) != 0 && mask & (1 << diagonal) != 0;
        let (r, w) = if attached {
            (0.5, if corner { 0.5 } else { 0.36 })
        } else {
            let flute = if material == 52 {
                0.82 + ((seed.rotate_right(i as u32 * 6) & 15) as f32 / 15.) * 0.18
            } else {
                0.94 + ((seed.rotate_right(i as u32 * 6) & 7) as f32 / 7.) * 0.06
            };
            let r = shaft_radius(material, profile, y) * flute;
            (r, r * bevel)
        };
        let (x, z) = (nx * r - nz * w * sign, nz * r + nx * w * sign);
        if attached {
            [0.5 + x, y, 0.5 + z]
        } else {
            let tip = mineral_progress(profile, y).powi(2);
            [
                0.5 + c * x - s * z + cant[0] * tip,
                y,
                0.5 + s * x + c * z + cant[1] * tip,
            ]
        }
    };
    let ring = |y: f32, i| {
        let lo = outline(y, i, profile.fan[0]);
        let hi = outline(y, i, profile.fan[1]);
        std::array::from_fn::<_, 3, _>(|a| lo[a] * (1. - y) + hi[a] * y)
    };
    let levels: &[f32] = if coarse {
        &[0., 1.]
    } else {
        &[0., 0.33, 0.67, 1.]
    };
    // Share each computed ring across adjoining facets and caps. The near
    // outline has at most four rings, so no mesh-time allocations are needed.
    let mut rings = [[[0.; 3]; 8]; 4];
    for (row, &height) in rings.iter_mut().zip(levels) {
        *row = std::array::from_fn(|i| ring(height, i));
    }
    let rings = &rings[..levels.len()];
    let cap = |row: &[[f32; 3]; 8]| {
        std::array::from_fn(|i| row.iter().map(|p| p[i]).sum::<f32>() * 0.125)
    };
    let first = &rings[0];
    let last = &rings[rings.len() - 1];
    let bottom = (profile.below == 0).then(|| cap(first));
    let top = (profile.above == 0).then(|| cap(last));
    for i in 0..8 {
        let j = (i + 1) % 8;
        let shade = 0.74 + ((i + (seed >> 10) as usize) % 5) as f32 * 0.06;
        let color = match material {
            51 => [shade, shade * 0.91, shade * 0.96],
            53 => [shade * 0.83, shade * 0.94, shade],
            _ => [shade, shade * 0.88, shade * 0.76],
        };
        let mut triangle = |a, b, c| {
            if a != b && a != c && b != c {
                emit([a, b, c], color);
            }
        };
        if i % 2 != 0 || profile.fan[0] & profile.fan[1] & (1 << i) == 0 {
            for band in rings.windows(2) {
                let [a, b] = [band[0][i], band[0][j]];
                let [c, d] = [band[1][j], band[1][i]];
                triangle(a, d, b);
                triangle(d, c, b);
            }
        }
        if let Some(middle) = bottom {
            triangle(middle, first[i], first[j]);
        }
        if let Some(middle) = top {
            triangle(middle, last[j], last[i]);
        }
    }
}
