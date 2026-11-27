//! Shared meter-cell geometry and support rules for plants and mineral growth.
use terra_voxel::MaterialId;

pub(crate) mod eelgrass;
mod halite;
mod mineral_fan;
mod root;

pub const MAX_CHAIN: i64 = 32;

pub const fn is_segment(material: u16) -> bool {
    matches!(material, 50..=56 | 460)
}
pub const fn is_crystal(material: u16) -> bool {
    matches!(material, 51..=53 | 460)
}

/// Hanging plants require an unbroken chain to a real ceiling. Unknown sections
/// are not missing support and must never cause persisted plant removal.
pub fn hanging_support(
    position: [i64; 3],
    mut block: impl FnMut([i64; 3]) -> Option<u16>,
) -> Option<bool> {
    for distance in 1..=MAX_CHAIN + 1 {
        let p = [position[0], position[1].checked_add(distance)?, position[2]];
        let material = block(p)?;
        if matches!(material, 54 | 55) {
            continue;
        }
        return Some(crate::props::placement::ground_support(material));
    }
    Some(false)
}

/// Mineral needles can grow from either surface. A gap in the same-material
/// run separates two formations; rock beyond that gap cannot support either.
pub fn mineral_support(
    position: [i64; 3],
    material: u16,
    mut block: impl FnMut([i64; 3]) -> Option<u16>,
) -> Option<bool> {
    let mut unknown = false;
    for direction in [-1, 1] {
        if material == 460 && direction < 0 {
            continue;
        }
        for distance in 1..=MAX_CHAIN + 1 {
            let p = [
                position[0],
                position[1].checked_add(direction * distance)?,
                position[2],
            ];
            let Some(m) = block(p) else {
                unknown = true;
                break;
            };
            if m == material {
                continue;
            }
            if root_block(m) {
                return Some(true);
            }
            break;
        }
    }
    (!unknown).then_some(false)
}

pub const fn fixed_mineral_root(material: u16) -> Option<i64> {
    match material {
        460 | 483 | 484 => Some(1),
        459 | 461..=463 | 467 | 468 | 485..=487 => Some(-1),
        _ => None,
    }
}

pub fn root_block(m: u16) -> bool {
    !is_segment(m)
        && fixed_mineral_root(m).is_none()
        && MaterialId::from_raw(m).is_some_and(MaterialId::is_solid)
}

#[derive(Debug, Clone, Copy, Default)]
pub struct Profile {
    pub below: u8,
    pub above: u8,
    pub hanging: bool,
    pub joined: bool,
    /// Occupied neighbouring shafts at the lower and upper section of this cell.
    /// Clockwise bits start at +X and alternate faces and diagonals.
    pub fan: [u8; 2],
}

pub fn profile(
    position: [i64; 3],
    material: u16,
    mut block: impl FnMut([i64; 3]) -> Option<u16>,
) -> Profile {
    let mut counts = [0; 2];
    let mut anchors = [false; 2];
    for (i, direction) in [-1, 1].into_iter().enumerate() {
        for distance in 1..=MAX_CHAIN + 1 {
            let p = [position[0], position[1] + distance * direction, position[2]];
            match block(p) {
                Some(m) if m == material => counts[i] = (counts[i] + 1).min(MAX_CHAIN as u8),
                Some(m) => {
                    anchors[i] = root_block(m);
                    break;
                }
                None => break,
            }
        }
    }
    Profile {
        below: counts[0],
        above: counts[1],
        joined: matches!(material, 52 | 53) && anchors == [true, true],
        hanging: matches!(material, 54 | 55 | 460) || (!anchors[0] && anchors[1]),
        fan: if is_crystal(material) {
            mineral_fan::connections(position, material, &mut block)
        } else {
            [0; 2]
        },
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
    if !block(root).is_some_and(root_block) {
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

fn mineral_progress(profile: Profile, y: f32) -> f32 {
    let total = f32::from(profile.below) + f32::from(profile.above) + 1.0;
    let height = (f32::from(profile.below) + y) / total;
    if profile.joined {
        (1.0 - (height * 2.0 - 1.0).abs()) * 0.64
    } else if profile.hanging {
        1.0 - height
    } else {
        height
    }
}

/// Conservative horizontal envelope used by the four collision slices.
pub fn radius(material: u16, profile: Profile, y: f32) -> f32 {
    let radius = shaft_radius(material, profile, y)
        + if profile.below > 0 || profile.above > 0 || profile.fan != [0; 2] {
            0.095 * mineral_progress(profile, y).powi(2)
        } else {
            0.
        };
    if profile.fan == [0; 2] {
        return radius;
    }
    let ends = profile.fan.map(|mask| {
        if mask & 0x55 == 0 {
            (radius * 1.05).min(0.5)
        } else {
            0.5
        }
    });
    ends[0] * (1. - y) + ends[1] * y
}

fn shaft_radius(material: u16, profile: Profile, y: f32) -> f32 {
    let t = mineral_progress(profile, y);
    match material {
        // Halite cleaves into blunt, bevelled prisms.
        51 | 460 => {
            let tip = ((t - 0.70) / 0.30).clamp(0., 1.);
            0.035 + 0.427 * (1. - 0.34 * t) * (1. - tip)
        }
        // Dripping calcite accretes in rounded flutes. The rooted end flares
        // into the ceiling, then slowly narrows along the entire pendant.
        52 => 0.018 + 0.46 * (1. - t).powf(if profile.hanging { 0.81 } else { 1.07 }),
        // Frosted shoulders give way to long cleavage faces and an ice point.
        53 => 0.027 + 0.425 * (1.0 - 0.29 * t) * (1.0 - ((t - 0.73) / 0.27).clamp(0., 1.)),
        _ => 0.030 + 0.42 * (1. - t * 0.52) * (1. - ((t - 0.61) / 0.39).clamp(0., 1.)),
    }
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
    // The ceiling-only halite block uses the same connected mineral geometry.
    let (material, profile) = if material == 460 {
        (
            51,
            Profile {
                hanging: true,
                ..profile
            },
        )
    } else {
        (material, profile)
    };
    if is_crystal(material) && (profile.fan != [0; 2] || profile.below > 0 || profile.above > 0) {
        mineral_fan::visit(material, profile, h, coarse, &mut emit);
        return;
    }
    if material == 51 {
        halite::visit(profile, h, coarse, &mut emit);
        return;
    }
    if is_crystal(material) {
        let sides = match material {
            52 => 10,
            _ => 8,
        };
        let angle = (h & 255) as f32 * (std::f32::consts::TAU / 256.);
        let (s, c) = angle.sin_cos();
        let ring = |y: f32, i: usize| {
            let a = std::f32::consts::TAU * i as f32 / sides as f32;
            let (u, v) = match material {
                52 => {
                    let flute = if i.is_multiple_of(2) {
                        1.0
                    } else {
                        0.66 + ((h >> (i + 9)) & 3) as f32 * 0.05
                    };
                    (a.cos() * flute, a.sin() * flute)
                }
                // Unequal bevel widths create a split-crystal glint without
                // changing the section outline between near and distant meshes.
                _ => [
                    (0.82, 0.48),
                    (0.47, 0.91),
                    (-0.44, 0.91),
                    (-0.81, 0.51),
                    (-0.81, -0.45),
                    (-0.42, -0.94),
                    (0.43, -0.94),
                    (0.82, -0.49),
                ][i % 8],
            };
            let r = radius(material, profile, y);
            [0.5 + (c * u - s * v) * r, y, 0.5 + (s * u + c * v) * r]
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
            let shade = 0.74 + ((i + (h >> 12) as usize) % 5) as f32 * 0.061;
            let color = match material {
                52 if i.is_multiple_of(3) => [shade, shade * 0.89, shade * 0.77],
                52 => [shade * 0.95, shade * 0.93, shade * 0.91],
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
        eelgrass::visit(h, profile, coarse, &mut emit);
    } else if material == 55 {
        root::visit(h, position[1], profile, &mut emit);
    } else if material == 54 {
        let color = [0.34, 0.62, 0.20];
        let phase = (h & 255) as f32 * 0.0245;
        let axis = |y: f32| {
            let t = position[1] as f32 + y;
            [
                0.5 + (t * 0.8 + phase).sin() * 0.13,
                y,
                0.5 + (t * 0.65 + phase).cos() * 0.13,
            ]
        };
        rod(axis(0.0), axis(0.5), 0.055, color, &mut emit);
        rod(axis(0.5), axis(1.0), 0.055, color, &mut emit);
        for i in 0..if coarse { 2 } else { 5 } {
            let a = i as f32 * 2.39996 + (h & 255) as f32 * 0.1;
            let y = 0.12 + i as f32 * 0.17;
            voxel(
                [axis(y)[0] + a.cos() * 0.20, y, axis(y)[2] + a.sin() * 0.20],
                [0.22, 0.10, 0.16],
                [0.50, 0.85, 0.31],
                &mut emit,
            );
        }
    } else if material == 56 {
        // A larger crown and two young fruiting bodies share the same patch.
        for (x, z, height, radius) in [
            (0.45, 0.47, 0.77, 0.28),
            (0.72, 0.68, 0.46, 0.18),
            (0.24, 0.76, 0.29, 0.13),
        ] {
            if coarse && height < 0.30 {
                continue;
            }
            let mut bottom = 0.0;
            for (width, depth, top, color) in [
                (0.39, 0.39, 0.68, [0.98, 0.79, 0.44]),
                (1.72, 1.54, 0.74, [1., 0.92, 0.62]),
                (2.0, 1.77, 0.88, [1., 0.57, 0.13]),
                (1.36, 1.20, 1.00, [1., 0.76, 0.25]),
            ] {
                let top = top * height;
                voxel(
                    [x, (bottom + top) * 0.5, z],
                    [width * radius, top - bottom, depth * radius],
                    color,
                    &mut emit,
                );
                bottom = top;
            }
        }
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
        crate::props::PropPose::Seasonal { time: Some(t), .. } => t,
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
