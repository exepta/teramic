use super::{MIN_WATERFALL_DROP, WaterPatch};
use crate::SurfaceColumn;
use terra_voxel::SectionPos;

const EDGES: [[usize; 2]; 4] = [[1, 0], [3, 2], [0, 3], [2, 1]];
const NORMALS: [[f32; 3]; 4] = [
    [1.0, 0.0, 0.0],
    [-1.0, 0.0, 0.0],
    [0.0, 0.0, 1.0],
    [0.0, 0.0, -1.0],
];

/// Corners use only shared padded columns. Falling curtains belong to the lower
/// cell and use world-aligned height bands, including at vertical section seams.
pub(crate) fn build_channel_patch(
    position: SectionPos,
    origin: [i64; 3],
    columns: &[SurfaceColumn],
    padded: &[u16],
) -> Option<WaterPatch> {
    let mut patch = WaterPatch {
        positions: Vec::new(),
        normals: Vec::new(),
        flow: Vec::new(),
        depth_shore: Vec::new(),
        surface_effects: Vec::new(),
        wave_weights: Vec::new(),
        indices: Vec::new(),
    };
    for z in 0..16 {
        for x in 0..16 {
            let column = columns[x + 1 + 18 * (z + 1)];
            let Some(level) = column.water_level else {
                continue;
            };
            let neighbors = [(x, z + 1), (x + 2, z + 1), (x + 1, z), (x + 1, z + 2)];
            let corners = [(x, z), (x, z + 1), (x + 1, z + 1), (x + 1, z)];
            let samples = corners.map(|(cx, cz)| corner(columns, cx, cz, level));
            let points = std::array::from_fn(|i| {
                [
                    corners[i].0 as f32,
                    (samples[i].height - origin[1] as f64) as f32,
                    corners[i].1 as f32,
                ]
            });
            if (level.floor() as i64).div_euclid(16) == position.y {
                let iy = (level - origin[1] as f64).floor() as usize + 1;
                let index = x + 1 + 18 * (iy + 18 * (z + 1));
                if super::can_hold_fluid(padded[index]) && !super::space::partial(padded[index]) {
                    quad(
                        &mut patch,
                        points,
                        samples.map(|s| [s.depth, s.shore]),
                        surface_effects(columns, neighbors, level),
                        column.water_flow,
                    );
                }
            }
            for (side, (nx, nz)) in neighbors.into_iter().enumerate() {
                if column.cave.dynamic_water && level < column.cave.ceiling {
                    continue;
                }
                let Some(upstream) = columns[nx + 18 * nz].water_level else {
                    continue;
                };
                if upstream - level < MIN_WATERFALL_DROP {
                    continue;
                }
                let edge = EDGES[side];
                let bottom = edge.map(|i| samples[i].height);
                let top = edge.map(|i| {
                    let (cx, cz) = corners[i];
                    corner(columns, cx, cz, upstream).height
                });
                let edge_points = edge.map(|i| points[i]);
                waterfall(
                    &mut patch,
                    origin,
                    edge_points,
                    bottom,
                    top,
                    NORMALS[side],
                    column.water_flow,
                );
            }
        }
    }
    super::geometry::assign_wave_weights(&mut patch, origin, columns, None);
    (!patch.indices.is_empty()).then_some(patch)
}

#[derive(Clone, Copy)]
struct Corner {
    height: f64,
    depth: f32,
    shore: f32,
}

fn corner(columns: &[SurfaceColumn], x: usize, z: usize, reference: f64) -> Corner {
    let adjacent = [
        columns[x + 18 * z],
        columns[x + 1 + 18 * z],
        columns[x + 18 * (z + 1)],
        columns[x + 1 + 18 * (z + 1)],
    ]
    .map(|mut column| {
        if reference < column.water_bed()
            && let Some(water) = column.cave.water
        {
            column.water_level = Some(water.level);
            column.height = water.bed - 0.01;
            column.river_cavity = None;
        }
        column
    });
    let mut levels = [f64::INFINITY; 4];
    let mut wet = 0;
    let mut dry_bed = f64::INFINITY;
    for column in adjacent {
        if let Some(level) = column.water_level {
            levels[wet] = level;
            wet += 1;
        } else {
            dry_bed = dry_bed.min(bed(column));
        }
    }
    levels[..wet].sort_by(f64::total_cmp);
    // Group by actual discontinuities. Comparing each sample with the requesting
    // cell would give different corners when several gentle slopes meet.
    let mut start = 0;
    let mut end = wet;
    for i in 1..wet {
        if levels[i] - levels[i - 1] >= MIN_WATERFALL_DROP {
            if reference < levels[i] {
                end = i;
                break;
            }
            start = i;
        }
    }
    let count = end - start;
    let mean = if count == 0 {
        reference
    } else {
        levels[start..end].iter().sum::<f64>() / count as f64
    };
    // A dry low neighbor is a drainage edge. Meet its solid top instead of
    // leaving a suspended horizontal rim. Raised banks retain the shared level.
    let height = if adjacent
        .iter()
        .any(|c| c.cave.dynamic_water && reference < c.cave.ceiling)
    {
        mean
    } else {
        mean.min(dry_bed)
    };
    let depth = adjacent
        .into_iter()
        .filter(|c| {
            c.water_level
                .is_some_and(|l| count > 0 && l >= levels[start] && l <= levels[end - 1])
        })
        .map(|c| (height - bed(c)).max(0.0))
        .sum::<f64>()
        / count.max(1) as f64;
    Corner {
        height,
        depth: depth as f32,
        shore: 1.0 - wet as f32 * 0.25,
    }
}

#[cfg(any(feature = "bevy", test))]
pub(super) fn shared_corner_height(
    columns: &[SurfaceColumn],
    x: usize,
    z: usize,
    reference: f64,
) -> f64 {
    corner(columns, x, z, reference).height
}

pub(super) fn shared_wave_weight(columns: &[SurfaceColumn], x: usize, z: usize) -> f32 {
    let mut levels = [f64::INFINITY; 4];
    let mut count = 0;
    for column in [
        columns[x + 18 * z],
        columns[x + 1 + 18 * z],
        columns[x + 18 * (z + 1)],
        columns[x + 1 + 18 * (z + 1)],
    ] {
        if let Some(level) = column.water_level {
            levels[count] = level;
            count += 1;
        }
    }
    if count == 0 {
        return 0.0;
    }
    levels[..count].sort_by(f64::total_cmp);
    // Both sides of the discontinuity request precisely the same zero weight.
    // The anchored curtain therefore meets the upper and lower surface exactly.
    if levels[..count]
        .windows(2)
        .any(|pair| pair[1] - pair[0] >= MIN_WATERFALL_DROP)
    {
        return 0.0;
    }
    let depth = corner(columns, x, z, levels[0]).depth;
    let t = ((depth - 0.05) / 0.90).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn bed(column: SurfaceColumn) -> f64 {
    column.water_bed()
}

fn surface_effects(
    columns: &[SurfaceColumn],
    neighbors: [(usize, usize); 4],
    level: f64,
) -> [[f32; 4]; 4] {
    let mut strongest: (f64, usize) = (0.0, 0);
    for (side, (x, z)) in neighbors.into_iter().enumerate() {
        if let Some(neighbor) = columns[x + 18 * z].water_level {
            let delta = neighbor - level;
            if delta.abs() >= MIN_WATERFALL_DROP && delta.abs() > strongest.0.abs() {
                strongest = (delta, side);
            }
        }
    }
    let (delta, side) = strongest;
    if delta == 0.0 {
        return [[0.0; 4]; 4];
    }
    std::array::from_fn(|i| {
        let distance = if EDGES[side].contains(&i) { 0.0 } else { 1.0 };
        [
            distance,
            0.0,
            delta.abs() as f32,
            if delta > 0.0 { 2.0 } else { 3.0 },
        ]
    })
}

fn waterfall(
    patch: &mut WaterPatch,
    origin: [i64; 3],
    edge: [[f32; 3]; 2],
    bottom: [f64; 2],
    top: [f64; 2],
    normal: [f32; 3],
    flow: [f32; 2],
) {
    let section_bottom = origin[1] as f64;
    let section_top = section_bottom + 16.0;
    let low = bottom[0].min(bottom[1]).max(section_bottom);
    let high = top[0].max(top[1]).min(section_top);
    if high <= low {
        return;
    }
    let sample = |i: usize, y: f64| {
        let height = (top[i] - bottom[i]).max(0.001);
        let y = y.clamp(bottom[i], top[i]);
        let progress = ((top[i] - y) / height) as f32;
        let across =
            origin[0] as f64 + f64::from(edge[i][0]) + origin[2] as f64 + f64::from(edge[i][2]);
        // Broad streams project from the cliff and fold into narrower runnels.
        // The zero-ended envelope keeps the exact shared crest and impact edge.
        let corrugation = 0.56
            + 0.18 * (across * 1.73).sin() as f32
            + 0.06 * (across * 3.61 + y * 0.91).sin() as f32;
        let offset = 0.06 + 0.15 * progress + (progress * std::f32::consts::PI).sin() * corrugation;
        (
            [
                edge[i][0] + normal[0] * offset,
                (y - section_bottom) as f32,
                edge[i][2] + normal[2] * offset,
            ],
            [
                (top[i] - y) as f32,
                (y - bottom[i]) as f32,
                height as f32,
                1.0,
            ],
        )
    };
    let first_band = low.floor() as i64;
    let last_band = high.ceil() as i64;
    for band in first_band..last_band {
        let y0 = (band as f64).max(low);
        let y1 = ((band + 1) as f64).min(high);
        let (a, ae) = sample(0, y0);
        let (b, be) = sample(1, y0);
        let (c, ce) = sample(1, y1);
        let (d, de) = sample(0, y1);
        clipped_quad(
            patch,
            [a, b, c, d],
            [[0.35, 1.0]; 4],
            [ae, be, ce, de],
            flow,
        );
    }
    if top[0].max(top[1]) >= section_bottom && top[0].min(top[1]) <= section_top {
        let (a, ae) = sample(0, top[0]);
        let (b, be) = sample(1, top[1]);
        let rim = std::array::from_fn::<_, 2, _>(|i| {
            [edge[i][0], (top[i] - section_bottom) as f32, edge[i][2]]
        });
        clipped_quad(
            patch,
            [rim[0], a, b, rim[1]],
            [[0.35, 0.5]; 4],
            [ae, ae, be, be],
            flow,
        );
    }
}

fn clipped_quad(
    patch: &mut WaterPatch,
    points: [[f32; 3]; 4],
    depth: [[f32; 2]; 4],
    effects: [[f32; 4]; 4],
    flow: [f32; 2],
) {
    if points.iter().all(|p| (0.0..=16.0).contains(&p[1])) {
        quad(patch, points, depth, effects, flow);
        return;
    }
    // A sloping lip/foot can cross the section plane partway along one edge.
    // Clip its polygon and attributes together; clamping each vertex would fold it.
    let mut vertices = [([0.0; 3], [0.0; 2], [0.0; 4]); 8];
    for i in 0..4 {
        vertices[i] = (points[i], depth[i], effects[i]);
    }
    let mut count = 4;
    for (plane, above) in [(0.0, true), (16.0, false)] {
        let mut output = [([0.0; 3], [0.0; 2], [0.0; 4]); 8];
        let mut length = 0;
        for i in 0..count {
            let a = vertices[(i + count - 1) % count];
            let b = vertices[i];
            let inside_a = if above {
                a.0[1] >= plane
            } else {
                a.0[1] <= plane
            };
            let inside_b = if above {
                b.0[1] >= plane
            } else {
                b.0[1] <= plane
            };
            if inside_a != inside_b {
                let t = (plane - a.0[1]) / (b.0[1] - a.0[1]);
                let mut point = std::array::from_fn(|j| a.0[j] + t * (b.0[j] - a.0[j]));
                point[1] = plane;
                output[length] = (
                    point,
                    std::array::from_fn(|j| a.1[j] + t * (b.1[j] - a.1[j])),
                    std::array::from_fn(|j| a.2[j] + t * (b.2[j] - a.2[j])),
                );
                length += 1;
            }
            if inside_b {
                output[length] = b;
                length += 1;
            }
        }
        vertices = output;
        count = length;
    }
    for i in 1..count.saturating_sub(1) {
        let triangle = [vertices[0], vertices[i], vertices[i + 1], vertices[i + 1]];
        quad(
            patch,
            triangle.map(|v| v.0),
            triangle.map(|v| v.1),
            triangle.map(|v| v.2),
            flow,
        );
    }
}

fn quad(
    patch: &mut WaterPatch,
    points: [[f32; 3]; 4],
    depth: [[f32; 2]; 4],
    effects: [[f32; 4]; 4],
    flow: [f32; 2],
) {
    let cross = |a: usize, b: usize, c: usize| {
        let ab: [f32; 3] = std::array::from_fn(|i| points[b][i] - points[a][i]);
        let ac: [f32; 3] = std::array::from_fn(|i| points[c][i] - points[a][i]);
        [
            ab[1] * ac[2] - ab[2] * ac[1],
            ab[2] * ac[0] - ab[0] * ac[2],
            ab[0] * ac[1] - ab[1] * ac[0],
        ]
    };
    let a = cross(0, 1, 2);
    let b = cross(0, 2, 3);
    let sum: [f32; 3] = std::array::from_fn(|i| a[i] + b[i]);
    let length = sum.iter().map(|v| v * v).sum::<f32>().sqrt();
    if length <= f32::EPSILON {
        return;
    }
    let normal = sum.map(|v| v / length);
    // At most sixteen height bands per side in a 16m section; all details stay batched.
    let base = patch.positions.len() as u32;
    patch.positions.extend(points);
    patch.normals.extend([normal; 4]);
    patch.depth_shore.extend(depth);
    patch.surface_effects.extend(effects);
    patch.flow.extend([flow; 4]);
    for (triangle, area) in [([0, 1, 2], a), ([0, 2, 3], b)] {
        if area.iter().map(|v| v * v).sum::<f32>() > f32::EPSILON {
            patch.indices.extend(triangle.map(|i| base + i));
        }
    }
}
