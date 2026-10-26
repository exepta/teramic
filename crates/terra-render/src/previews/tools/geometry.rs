use super::palette::{LEATHER, LEATHER_LIGHT, WOOD, WOOD_DARK, WOOD_LIGHT};
use super::{Builder, Palette, Shape, ToolMaterial};
use bevy::prelude::*;

fn part(b: &mut Builder, center: [f32; 3], size: [f32; 3], color: [f32; 3]) -> Result<(), String> {
    b.bevel_box(center, size, color, Quat::IDENTITY)
}

fn rivet(b: &mut Builder, x: f32, y: f32, z: f32, p: &Palette) -> Result<(), String> {
    part(b, [x, y, z], [0.018, 0.018, 0.012], p.inlay)
}

pub(super) fn handle(b: &mut Builder, shape: Shape, p: &Palette) -> Result<(), String> {
    let sword = shape == Shape::Sword;
    let top = if sword {
        0.32
    } else if shape == Shape::Stick {
        0.90
    } else {
        0.91
    };
    let bottom = if shape == Shape::Stick { 0.08 } else { 0.04 };
    part(
        b,
        [0.5, (top + bottom) * 0.5, 0.5],
        [0.065, top - bottom, 0.070],
        WOOD,
    )?;
    let segments = if sword { 4 } else { 11 };
    for row in 0..segments {
        let y = bottom + (row as f32 + 0.5) * (top - bottom) / segments as f32;
        let light = if row % 3 == 0 { WOOD_DARK } else { WOOD_LIGHT };
        for side in [-1.0, 1.0] {
            b.voxel(
                [0.5 + side * 0.018, y, 0.536],
                [0.008, (top - bottom) / segments as f32 * 0.77, 0.007],
                light,
            )?;
            b.voxel(
                [0.5 - side * 0.019, y, 0.464],
                [0.006, (top - bottom) / segments as f32 * 0.66, 0.007],
                light,
            )?;
        }
    }
    if shape == Shape::Stick {
        return Ok(());
    }
    // Separate pitched leather strips remain dimensional on both sides of the haft.
    let wraps = if sword { 5 } else { 8 };
    for row in 0..wraps {
        let y = 0.095 + row as f32 * if sword { 0.035 } else { 0.060 };
        b.bevel_box(
            [0.5, y, 0.5],
            [0.083, 0.023, 0.087],
            if row % 3 == 0 { LEATHER_LIGHT } else { LEATHER },
            Quat::from_rotation_z(-0.10),
        )?;
        b.voxel(
            [0.502, y - 0.004, 0.547],
            [0.034, 0.006, 0.008],
            LEATHER_LIGHT,
        )?;
    }
    part(b, [0.5, 0.031, 0.5], [0.108, 0.062, 0.108], p.fitting_dark)?;
    part(b, [0.5, 0.031, 0.559], [0.062, 0.036, 0.018], p.fitting)?;
    part(b, [0.5, 0.031, 0.441], [0.062, 0.036, 0.018], p.fitting)?;
    for z in [0.429, 0.571] {
        rivet(b, 0.5, 0.031, z, p)?;
    }
    if !sword {
        for y in [0.59, 0.72] {
            part(b, [0.5, y, 0.5], [0.089, 0.034, 0.092], p.fitting_dark)?;
            for z in [0.45, 0.55] {
                rivet(b, 0.5, y, z, p)?;
            }
        }
    }
    Ok(())
}

pub(super) fn sword(b: &mut Builder, p: &Palette) -> Result<(), String> {
    let sections = [
        (0.335, 0.074, 0.028),
        (0.40, 0.079, 0.034),
        (0.53, 0.076, 0.035),
        (0.67, 0.072, 0.033),
        (0.81, 0.067, 0.030),
        (0.925, 0.049, 0.024),
        (0.978, 0.021, 0.012),
        (1.0, 0.002, 0.002),
    ];
    let ring = |y: f32, width: f32, depth: f32| {
        let edge = 0.005_f32.min(depth * 0.35);
        [
            (-width, -edge),
            (-width * 0.55, -depth),
            (width * 0.55, -depth),
            (width, -edge),
            (width, edge),
            (width * 0.55, depth),
            (-width * 0.55, depth),
            (-width, edge),
        ]
        .map(|(x, z)| Vec3::new(0.5 + x, y, 0.5 + z))
    };
    for (row, pair) in sections.windows(2).enumerate() {
        let a = ring(pair[0].0, pair[0].1, pair[0].2);
        let c = ring(pair[1].0, pair[1].1, pair[1].2);
        for side in 0..8 {
            let next = (side + 1) % 8;
            let color = match side {
                0 | 2 | 4 | 6 => p.edge,
                1 | 5 if row % 3 == 0 => p.light,
                1 | 5 => p.body,
                _ => p.dark,
            };
            b.quad([a[next], a[side], c[side], c[next]], color)?;
        }
    }
    for (entry, top) in [(sections[0], false), (sections[7], true)] {
        let points = ring(entry.0, entry.1, entry.2);
        let center = Vec3::new(0.5, entry.0, 0.5);
        for side in 0..8 {
            let next = (side + 1) % 8;
            b.triangle(
                if top {
                    [center, points[next], points[side]]
                } else {
                    [center, points[side], points[next]]
                },
                p.edge,
            )?;
        }
    }
    // Stepped crossguard with raised socket and an inset material-specific stud.
    part(b, [0.5, 0.308, 0.5], [0.31, 0.054, 0.105], p.fitting_dark)?;
    for side in [-1.0, 1.0] {
        part(
            b,
            [0.5 + side * 0.165, 0.315 + side * 0.008, 0.5],
            [0.073, 0.073, 0.108],
            p.fitting,
        )?;
        part(
            b,
            [0.5 + side * 0.136, 0.344, 0.5],
            [0.086, 0.026, 0.094],
            p.light,
        )?;
        for z in [0.44, 0.56] {
            rivet(b, 0.5 + side * 0.17, 0.315 + side * 0.008, z, p)?;
        }
    }
    part(b, [0.5, 0.322, 0.5], [0.125, 0.105, 0.125], p.fitting)?;
    for z in [0.427, 0.573] {
        part(b, [0.5, 0.322, z], [0.055, 0.055, 0.024], p.fitting_dark)?;
        part(
            b,
            [0.5, 0.322, z + (z - 0.5).signum() * 0.014],
            [0.031, 0.031, 0.012],
            p.inlay,
        )?;
    }
    for side in [-1.0, 1.0] {
        for row in 0..5 {
            let y = 0.44 + row as f32 * 0.094;
            let z = 0.5 + side * (0.035 - row as f32 * 0.00175);
            if p.kind == ToolMaterial::Wood {
                part(
                    b,
                    [0.48 + (row % 2) as f32 * 0.025, y, z],
                    [0.008, 0.068, 0.005],
                    p.dark,
                )?;
                part(b, [0.515, y + 0.018, z], [0.011, 0.035, 0.005], p.light)?;
            } else if p.kind == ToolMaterial::Stone {
                b.bevel_box(
                    [0.484 + (row % 2) as f32 * 0.024, y, z],
                    [0.052, 0.074, 0.009],
                    if row % 2 == 0 { p.dark } else { p.light },
                    Quat::from_rotation_z(0.12 - row as f32 * 0.05),
                )?;
            } else if p.crystal() {
                crystal_seam(
                    b,
                    [0.48, y - 0.025, z],
                    [0.512, y + 0.004, z + side * 0.002],
                    0.004,
                    p.edge,
                )?;
                crystal_seam(
                    b,
                    [0.512, y + 0.004, z + side * 0.002],
                    [0.499, y + 0.035, z],
                    0.003,
                    p.light,
                )?;
            } else {
                part(b, [0.5, y, z], [0.011, 0.070, 0.005], p.light)?;
            }
        }
    }
    Ok(())
}

pub(super) fn socket(b: &mut Builder, p: &Palette, y: f32) -> Result<(), String> {
    part(b, [0.5, y, 0.5], [0.13, 0.18, 0.146], p.fitting_dark)?;
    for side in [-1.0, 1.0] {
        part(
            b,
            [0.5, y, 0.5 + side * 0.079],
            [0.092, 0.13, 0.022],
            p.fitting,
        )?;
        part(
            b,
            [0.5, y, 0.5 + side * 0.094],
            [0.036, 0.049, 0.014],
            p.inlay,
        )?;
        for offset in [-0.046, 0.046] {
            rivet(b, 0.5, y + offset, 0.5 + side * 0.098, p)?;
        }
        if p.primitive() {
            for direction in [-1.0, 1.0] {
                b.bevel_box(
                    [0.5, y, 0.5 + side * 0.111],
                    [0.24, 0.028, 0.019],
                    LEATHER_LIGHT,
                    Quat::from_rotation_z(direction * 0.66),
                )?;
            }
        }
    }
    Ok(())
}

pub(super) fn pickaxe(b: &mut Builder, p: &Palette) -> Result<(), String> {
    let left = [
        [0.066, 0.72],
        [0.13, 0.856],
        [0.258, 0.944],
        [0.42, 1.0],
        [0.5, 0.98],
        [0.5, 0.854],
        [0.37, 0.865],
        [0.25, 0.835],
        [0.15, 0.776],
    ];
    for side in [false, true] {
        let shape = left.map(|[x, y]| [if side { 1.0 - x } else { x }, y]);
        extrude(b, &shape, 0.134, p.body, p.light, p.edge, true)?;
    }
    socket(b, p, 0.885)
}

pub(super) fn axe(b: &mut Builder, p: &Palette) -> Result<(), String> {
    let wing = [
        [0.46, 0.737],
        [0.39, 0.728],
        [0.285, 0.635],
        [0.185, 0.61],
        [0.143, 0.693],
        [0.125, 0.85],
        [0.168, 1.0],
        [0.265, 0.967],
        [0.384, 0.91],
        [0.46, 0.91],
    ];
    for side in [false, true] {
        let shape = wing.map(|[x, y]| [if side { 1.0 - x } else { x }, y]);
        extrude(b, &shape, 0.144, p.body, p.dark, p.edge, true)?;
    }
    socket(b, p, 0.825)
}

pub(super) fn shovel(b: &mut Builder, p: &Palette) -> Result<(), String> {
    let blade = [
        [0.407, 0.732],
        [0.35, 0.82],
        [0.35, 0.94],
        [0.397, 0.99],
        [0.5, 1.0],
        [0.603, 0.99],
        [0.65, 0.94],
        [0.65, 0.82],
        [0.593, 0.732],
    ];
    extrude(b, &blade, 0.09, p.body, p.light, p.edge, false)?;
    for side in [-1.0, 1.0] {
        part(
            b,
            [0.5, 0.847, 0.5 + side * 0.052],
            [0.034, 0.223, 0.022],
            p.light,
        )?;
        for x in [0.407, 0.593] {
            part(
                b,
                [x, 0.883, 0.5 + side * 0.049],
                [0.012, 0.122, 0.012],
                p.dark,
            )?;
        }
    }
    socket(b, p, 0.746)
}

fn crystal_seam(
    b: &mut Builder,
    start: [f32; 3],
    end: [f32; 3],
    width: f32,
    color: [f32; 3],
) -> Result<(), String> {
    let a = Vec3::from_array(start);
    let c = Vec3::from_array(end);
    let delta = c - a;
    b.bevel_box(
        ((a + c) * 0.5).to_array(),
        [width, delta.length(), 0.003],
        color,
        Quat::from_rotation_z(-delta.x.atan2(delta.y)),
    )
}

/// Chamfered polygon solids keep the curved, stepped outline and genuine side
/// thickness. Ear clipping avoids the crossing triangles of a concave fan.
pub(super) fn extrude(
    b: &mut Builder,
    outline: &[[f32; 2]],
    depth: f32,
    body: [f32; 3],
    side: [f32; 3],
    bevel: [f32; 3],
    ridged: bool,
) -> Result<(), String> {
    let mut points: Vec<Vec2> = outline.iter().copied().map(Vec2::from_array).collect();
    let area: f32 = points
        .iter()
        .zip(points.iter().cycle().skip(1))
        .take(points.len())
        .map(|(a, c)| a.perp_dot(*c))
        .sum();
    if area < 0.0 {
        points.reverse();
    }
    let center = points.iter().copied().sum::<Vec2>() / points.len() as f32;
    let mut inner: Vec<_> = points
        .iter()
        .map(|p| center + (*p - center) * 0.87)
        .collect();
    let mut triangles = triangulate(&inner)?;
    if ridged {
        // Split one containing triangle rather than fanning the concave head.
        // This gives a shared raised crest and several large, watertight facets.
        let containing = triangles.iter().position(|&[a, c, d]| {
            [(a, c), (c, d), (d, a)]
                .iter()
                .all(|&(x, y)| (inner[y] - inner[x]).perp_dot(center - inner[x]) > 1e-6)
        });
        let index = containing.unwrap_or(0);
        let [a, c, d] = triangles.swap_remove(index);
        let crest = if containing.is_some() {
            center
        } else {
            (inner[a] + inner[c] + inner[d]) / 3.0
        };
        let n = inner.len();
        inner.push(crest);
        triangles.extend([[a, c, n], [c, d, n], [d, a, n]]);
    }
    let at = |p: Vec2, z: f32| Vec3::new(p.x, p.y, 0.5 + z);
    let half = depth * 0.5;
    let span = points
        .iter()
        .map(|p| (p.x - center.x).abs())
        .fold(0.0_f32, f32::max);
    let relief = |index: usize| {
        if !ridged {
            return half;
        }
        if index == points.len() {
            return half * 1.38;
        }
        let crest = (1.0 - (inner[index].x - center.x).abs() / span).clamp(0.0, 1.0);
        half * (0.74 + 0.48 * crest)
    };
    for i in 0..points.len() {
        let n = (i + 1) % points.len();
        b.quad(
            [
                at(points[i], -half * 0.62),
                at(points[n], -half * 0.62),
                at(points[n], half * 0.62),
                at(points[i], half * 0.62),
            ],
            side,
        )?;
        b.quad(
            [
                at(points[n], half * 0.62),
                at(inner[n], relief(n)),
                at(inner[i], relief(i)),
                at(points[i], half * 0.62),
            ],
            bevel,
        )?;
        b.quad(
            [
                at(points[i], -half * 0.62),
                at(inner[i], -relief(i)),
                at(inner[n], -relief(n)),
                at(points[n], -half * 0.62),
            ],
            bevel,
        )?;
    }
    for [a, c, d] in triangles {
        let front = [
            at(inner[a], relief(a)),
            at(inner[c], relief(c)),
            at(inner[d], relief(d)),
        ];
        let normal = (front[1] - front[0]).cross(front[2] - front[0]).normalize();
        let gain = if ridged {
            (0.94 + normal.x * 0.23 + normal.y * 0.15).clamp(0.68, 1.14)
        } else {
            1.0
        };
        let color = body.map(|v| (v * gain).min(1.0));
        b.triangle(front, color)?;
        b.triangle(
            [
                at(inner[d], -relief(d)),
                at(inner[c], -relief(c)),
                at(inner[a], -relief(a)),
            ],
            color,
        )?;
    }
    Ok(())
}

fn triangulate(points: &[Vec2]) -> Result<Vec<[usize; 3]>, String> {
    let mut rest: Vec<_> = (0..points.len()).collect();
    let mut triangles = Vec::with_capacity(points.len().saturating_sub(2));
    while rest.len() > 3 {
        let mut clipped = false;
        for i in 0..rest.len() {
            let a = rest[(i + rest.len() - 1) % rest.len()];
            let c = rest[i];
            let d = rest[(i + 1) % rest.len()];
            if (points[c] - points[a]).perp_dot(points[d] - points[c]) <= 1e-8 {
                continue;
            }
            let inside = rest.iter().any(|&j| {
                if [a, c, d].contains(&j) {
                    return false;
                }
                [(a, c), (c, d), (d, a)]
                    .iter()
                    .all(|&(x, y)| (points[y] - points[x]).perp_dot(points[j] - points[x]) >= -1e-8)
            });
            if inside {
                continue;
            }
            triangles.push([a, c, d]);
            rest.remove(i);
            clipped = true;
            break;
        }
        if !clipped {
            return Err("Invalid tool head polygon".into());
        }
    }
    if rest.len() == 3 {
        triangles.push([rest[0], rest[1], rest[2]]);
    }
    Ok(triangles)
}
