//! Shared meter-cell geometry and support rules for plants and mineral growth.
use terra_voxel::MaterialId;

pub const MAX_CHAIN: i64 = 32;

pub const fn is_segment(material: u16) -> bool {
    matches!(material, 50..=56)
}
pub const fn is_crystal(material: u16) -> bool {
    matches!(material, 51..=53)
}

#[derive(Debug, Clone, Copy, Default)]
pub struct Profile {
    pub below: u8,
    pub above: u8,
    pub hanging: bool,
}

pub fn profile(
    position: [i64; 3],
    material: u16,
    mut block: impl FnMut([i64; 3]) -> Option<u16>,
) -> Profile {
    let mut counts = [0; 2];
    let mut anchors = [false; 2];
    for (i, direction) in [-1, 1].into_iter().enumerate() {
        for distance in 1..=MAX_CHAIN {
            let p = [position[0], position[1] + distance * direction, position[2]];
            match block(p) {
                Some(m) if m == material => counts[i] += 1,
                Some(m) => {
                    anchors[i] =
                        !is_segment(m) && MaterialId::from_raw(m).is_some_and(MaterialId::is_solid);
                    break;
                }
                None => break,
            }
        }
    }
    Profile {
        below: counts[0],
        above: counts[1],
        hanging: matches!(material, 54 | 55) || (!anchors[0] && anchors[1]),
    }
}

pub fn extension(
    position: [i64; 3],
    material: u16,
    mut block: impl FnMut([i64; 3]) -> Option<u16>,
) -> Option<[i64; 3]> {
    if !is_crystal(material) || block(position) != Some(material) {
        return None;
    }
    let profile = profile(position, material, &mut block);
    if i64::from(profile.below) + i64::from(profile.above) + 1 >= MAX_CHAIN {
        return None;
    }
    let direction = if profile.hanging { -1 } else { 1 };
    let root_offset = if profile.hanging {
        i64::from(profile.above) + 1
    } else {
        -i64::from(profile.below) - 1
    };
    let root = [
        position[0],
        position[1].checked_add(root_offset)?,
        position[2],
    ];
    if block(root)
        .is_none_or(|m| is_segment(m) || !MaterialId::from_raw(m).is_some_and(MaterialId::is_solid))
    {
        return None;
    }
    let length = i64::from(if profile.hanging {
        profile.below
    } else {
        profile.above
    }) + 1;
    let tip = [
        position[0],
        position[1].checked_add(direction * length)?,
        position[2],
    ];
    matches!(block(tip), Some(0 | 27)).then_some(tip)
}

fn seed([x, _, z]: [i64; 3]) -> u64 {
    let a =
        (x as u64).wrapping_mul(0x9e3779b97f4a7c15) ^ (z as u64).wrapping_mul(0xbf58476d1ce4e5b9);
    (a ^ (a >> 30)).wrapping_mul(0x94d049bb133111eb)
}

pub fn water_weight(profile: Profile, y: f32) -> f32 {
    let height = f32::from(profile.below) + y;
    (height / (f32::from(profile.below) + f32::from(profile.above) + 1.0))
        .clamp(0.0, 1.0)
        .powi(2)
        * -6.5
}

pub fn radius(profile: Profile, y: f32) -> f32 {
    let total = f32::from(profile.below) + f32::from(profile.above) + 1.0;
    let distance = f32::from(if profile.hanging {
        profile.above
    } else {
        profile.below
    }) + if profile.hanging { 1.0 - y } else { y };
    // Thick mineral shafts, with a short bevel only on the last cell.
    // World-chain distance keeps both sides of section boundaries identical.
    let progress = (distance / total).clamp(0.0, 1.0);
    let shaft = 0.46 - (progress * 4.0).floor() * 0.025;
    let bevel = ((distance - (total - 0.28)) / 0.28).clamp(0.0, 1.0);
    shaft * (1.0 - bevel * 0.25)
}

pub fn displace(p: [f64; 3], weight: f32, time: f32) -> [f64; 3] {
    let d = crate::props::wind_displacement(p, time, [1.0, weight], [0.86, 0.51, 1.0, 0.0]);
    [p[0] + d[0], p[1], p[2] + d[1]]
}

/// Closed, finite triangles used by both render composition and exact picking.
pub fn visit(
    material: u16,
    position: [i64; 3],
    profile: Profile,
    coarse: bool,
    mut emit: impl FnMut([[f32; 3]; 3], [f32; 3]),
) {
    let h = seed(position);
    if is_crystal(material) {
        let sides = 8;
        let angle = (h & 255) as f32 * std::f32::consts::TAU / 255.0;
        let ring = |y: f32, i: usize| {
            // Bevelled square salt / ice prisms; limestone has uneven ribs.
            let bevel = if material == 53 { 0.46 } else { 0.28 };
            let corners = [
                [1.0, bevel],
                [bevel, 1.0],
                [-bevel, 1.0],
                [-1.0, bevel],
                [-1.0, -bevel],
                [-bevel, -1.0],
                [bevel, -1.0],
                [1.0, -bevel],
            ];
            let [u, v] = corners[i];
            let r = radius(profile, y) / (1.0_f32 + bevel * bevel).sqrt()
                * if material == 52 && i.is_multiple_of(3) {
                    0.83
                } else {
                    1.0
                };
            [
                0.5 + (u * angle.cos() - v * angle.sin()) * r,
                y,
                0.5 + (u * angle.sin() + v * angle.cos()) * r,
            ]
        };
        let terminal = if profile.hanging {
            profile.below == 0
        } else {
            profile.above == 0
        };
        let split = if profile.hanging { 0.45 } else { 0.55 };
        let levels: &[f32] = if coarse {
            &[0.0, 1.0]
        } else if terminal {
            &[0.0, 0.28, split, 0.72, 1.0]
        } else {
            &[0.0, 0.28, 0.72, 1.0]
        };
        for i in 0..sides {
            let n = (i + 1) % sides;
            let shade = 0.86 + (i % 3) as f32 * 0.055;
            let color = match material {
                51 => [shade, shade * 0.91, shade * 0.94],
                52 => [shade, shade * 0.94, shade * 0.85],
                _ => [shade * 0.87, shade * 0.96, shade],
            };
            for ys in levels.windows(2) {
                let a = ring(ys[0], i);
                let b = ring(ys[0], n);
                let c = ring(ys[1], n);
                let d = ring(ys[1], i);
                emit([a, d, b], color);
                emit([d, c, b], color);
            }
            if profile.below == 0 {
                emit([[0.5, 0.0, 0.5], ring(0.0, i), ring(0.0, n)], color);
            }
            if profile.above == 0 {
                emit([[0.5, 1.0, 0.5], ring(1.0, n), ring(1.0, i)], color);
            }
        }
        return;
    }
    if material == 50 {
        let phase = (h & 255) as f32 * 0.0245;
        for stalk in 0..if coarse { 1 } else { 2 } {
            let a = phase + stalk as f32 * 2.39996;
            let center = [0.5 + a.cos() * 0.10, 0.5, 0.5 + a.sin() * 0.10];
            voxel(center, [0.045, 1.0, 0.045], [0.63, 0.88, 0.45], &mut emit);
            for leaf in 0..if coarse { 2 } else { 3 } {
                let y = 0.13 + leaf as f32 * if coarse { 0.45 } else { 0.32 };
                let a = a + if (leaf as i64 + position[1]).rem_euclid(2) == 0 {
                    0.0
                } else {
                    std::f32::consts::PI
                };
                let width = 0.065 + ((h >> (leaf * 3)) & 3) as f32 * 0.01;
                let root = [center[0], y, center[2]];
                let tip = [
                    center[0] + a.cos() * 0.24,
                    y + 0.16,
                    center[2] + a.sin() * 0.24,
                ];
                rod(root, tip, width, [0.79, 1.0, 0.62], &mut emit);
            }
        }
    } else if matches!(material, 54 | 55) {
        let root = material == 55;
        let color = if root {
            [0.47, 0.28, 0.15]
        } else {
            [0.34, 0.62, 0.20]
        };
        let phase = (h & 255) as f32 * 0.0245;
        let axis = |y: f32| {
            let t = position[1] as f32 + y;
            [
                0.5 + (t * 0.8 + phase).sin() * 0.13,
                y,
                0.5 + (t * 0.65 + phase).cos() * 0.13,
            ]
        };
        rod(
            axis(0.0),
            axis(0.5),
            if root { 0.22 } else { 0.055 },
            color,
            &mut emit,
        );
        rod(
            axis(0.5),
            axis(1.0),
            if root { 0.22 } else { 0.055 },
            color,
            &mut emit,
        );
        for i in 0..if coarse { 2 } else { 5 } {
            let a = i as f32 * 2.39996 + (h & 255) as f32 * 0.1;
            let y = 0.12 + i as f32 * 0.17;
            if root {
                rod(
                    [0.5, y + 0.14, 0.5],
                    [0.5 + a.cos() * 0.29, y, 0.5 + a.sin() * 0.29],
                    0.045,
                    color,
                    &mut emit,
                );
            } else {
                voxel(
                    [axis(y)[0] + a.cos() * 0.20, y, axis(y)[2] + a.sin() * 0.20],
                    [0.22, 0.10, 0.16],
                    [0.50, 0.85, 0.31],
                    &mut emit,
                );
            }
        }
    } else if material == 56 {
        voxel(
            [0.5, 0.19, 0.5],
            [0.085, 0.38, 0.085],
            [0.86, 0.70, 0.38],
            &mut emit,
        );
        voxel(
            [0.5, 0.36, 0.5],
            [0.40, 0.07, 0.34],
            [1.0, 0.57, 0.18],
            &mut emit,
        );
        voxel(
            [0.5, 0.42, 0.5],
            [0.30, 0.065, 0.27],
            [1.0, 0.80, 0.41],
            &mut emit,
        );
        voxel(
            [0.5, 0.47, 0.5],
            [0.16, 0.04, 0.15],
            [1.0, 0.87, 0.58],
            &mut emit,
        );
    }
}

fn voxel(
    center: [f32; 3],
    size: [f32; 3],
    color: [f32; 3],
    emit: &mut impl FnMut([[f32; 3]; 3], [f32; 3]),
) {
    let min = std::array::from_fn::<_, 3, _>(|i| center[i] - size[i] * 0.5);
    let max = std::array::from_fn::<_, 3, _>(|i| center[i] + size[i] * 0.5);
    for axis in 0..3 {
        for side in 0..2 {
            let u = (axis + 1) % 3;
            let v = (axis + 2) % 3;
            let mut corners = [[0.0; 3]; 4];
            for (i, [a, b]) in [[0, 0], [1, 0], [1, 1], [0, 1]].into_iter().enumerate() {
                corners[i][axis] = if side == 0 { min[axis] } else { max[axis] };
                corners[i][u] = if a == 0 { min[u] } else { max[u] };
                corners[i][v] = if b == 0 { min[v] } else { max[v] };
            }
            for indices in [[0, 1, 2], [0, 2, 3]] {
                let mut t = indices.map(|i| corners[i]);
                if side == 0 {
                    t.swap(1, 2);
                }
                emit(t, color);
            }
        }
    }
}

fn rod(
    a: [f32; 3],
    b: [f32; 3],
    width: f32,
    color: [f32; 3],
    emit: &mut impl FnMut([[f32; 3]; 3], [f32; 3]),
) {
    let d = std::array::from_fn::<_, 3, _>(|i| b[i] - a[i]);
    let length = d.iter().map(|x| x * x).sum::<f32>().sqrt().max(0.00001);
    let d = d.map(|v| v / length);
    let u = if d[0].abs() + d[2].abs() < 0.01 {
        [1.0, 0.0, 0.0]
    } else {
        let l = d[0].hypot(d[2]);
        [d[2] / l, 0.0, -d[0] / l]
    };
    let v = [
        d[1] * u[2] - d[2] * u[1],
        d[2] * u[0] - d[0] * u[2],
        d[0] * u[1] - d[1] * u[0],
    ];
    let corners = std::array::from_fn::<_, 8, _>(|i| {
        std::array::from_fn::<_, 3, _>(|axis| {
            (if i & 4 == 0 { a[axis] } else { b[axis] })
                + u[axis] * width * (if i & 1 == 0 { -0.5 } else { 0.5 })
                + v[axis] * width * (if i & 2 == 0 { -0.5 } else { 0.5 })
        })
    });
    for quad in [
        [0, 2, 3, 1],
        [4, 5, 7, 6],
        [0, 1, 5, 4],
        [2, 6, 7, 3],
        [0, 4, 6, 2],
        [1, 3, 7, 5],
    ] {
        emit(
            [corners[quad[0]], corners[quad[1]], corners[quad[2]]],
            color,
        );
        emit(
            [corners[quad[0]], corners[quad[2]], corners[quad[3]]],
            color,
        );
    }
}

#[cfg(feature = "bevy")]
pub(crate) fn raycast(
    chunks: &crate::streaming::ChunkStore,
    position: [i64; 3],
    material: u16,
    origin: [f64; 3],
    direction: [f64; 3],
    reach: f64,
    pose: crate::props::PropPose,
) -> Option<terra_voxel::blocks::BlockHit> {
    let profile = profile(position, material, |p| chunks.block_at(p));
    let mut nearest: Option<terra_voxel::blocks::BlockHit> = None;
    let swept = matches!(
        pose,
        crate::props::PropPose::Swept | crate::props::PropPose::WeatherSwept { .. }
    );
    let time = match pose {
        crate::props::PropPose::Wind(t) | crate::props::PropPose::WeatherWind { time: t, .. } => t,
        _ => 0.0,
    };
    for phase in 0..if swept && material == 50 { 16 } else { 1 } {
        let time = if swept { phase as f32 * 0.37 } else { time };
        visit(material, position, profile, false, |triangle, _| {
            let vertices = triangle.map(|v| {
                let world = std::array::from_fn(|i| position[i] as f64 + f64::from(v[i]));
                if material == 50 && !matches!(pose, crate::props::PropPose::Rest) {
                    displace(world, water_weight(profile, v[1]), time)
                } else {
                    world
                }
            });
            let local =
                vertices.map(|v| std::array::from_fn(|i| (v[i] - position[i] as f64) as f32));
            let local_origin = std::array::from_fn(|i| origin[i] - position[i] as f64);
            if let Some(distance) = crate::ore_shapes::ray_triangle(
                local,
                local_origin,
                direction,
                nearest.map_or(reach, |n| n.distance),
            ) {
                let n = crate::ore_shapes::normal(local);
                let axis = (0..3)
                    .max_by(|&a, &b| n[a].abs().total_cmp(&n[b].abs()))
                    .unwrap_or(1);
                let mut normal = [0; 3];
                normal[axis] = if n[axis] > 0.0 { 1 } else { -1 };
                nearest = Some(terra_voxel::blocks::BlockHit {
                    position,
                    normal,
                    distance,
                });
            }
        });
    }
    nearest
}

#[cfg(test)]
mod tests;
