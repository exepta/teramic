use super::{Builder, Coat};
use bevy::prelude::*;
use std::f32::consts::TAU;

pub(super) type Rgb = [f32; 3];
pub(super) const fn v(x: f32, y: f32, z: f32) -> Vec3 {
    Vec3::new(x, y, z)
}
pub(super) fn tint(rgb: Rgb, shade: f32) -> Rgb {
    rgb.map(|c| (c * shade).min(1.0))
}

#[derive(Clone, Copy)]
pub(super) struct Section {
    pub pos: Vec3,
    pub width: f32,
    pub depth: f32,
}
pub(super) fn sec(pos: Vec3, width: f32, depth: f32) -> Section {
    Section { pos, width, depth }
}

fn axes(axis: Vec3) -> (Vec3, Vec3) {
    let guide = if axis.x.abs() > 0.88 {
        Vec3::Z
    } else {
        Vec3::X
    };
    let u = (guide - axis * guide.dot(axis)).normalize();
    (u, axis.cross(u))
}

/// A swept, closed, flat-shaded solid. The section frame follows the tangent.
pub(super) fn sweep(
    mesh: &mut Builder,
    path: &[Section],
    sides: usize,
    color: Rgb,
) -> Result<(), String> {
    let rings: Vec<Vec<Vec3>> = (0..path.len())
        .map(|i| {
            let a = path[i.saturating_sub(1)].pos;
            let b = path[(i + 1).min(path.len() - 1)].pos;
            let (x, y) = axes((b - a).normalize());
            (0..sides)
                .map(|j| {
                    let (sn, cs) = (TAU * (j as f32 + 0.5) / sides as f32).sin_cos();
                    path[i].pos + x * (cs * path[i].width) + y * (sn * path[i].depth)
                })
                .collect()
        })
        .collect();
    for (k, r) in rings.windows(2).enumerate() {
        for j in 0..sides {
            let n = (j + 1) % sides;
            mesh.quad(
                [r[0][j], r[0][n], r[1][n], r[1][j]],
                tint(color, 0.96 + 0.038 * ((k + j) as f32 * 1.7).cos()),
            )?;
        }
    }
    for (i, reverse) in [(0, true), (rings.len() - 1, false)] {
        for j in 1..sides - 1 {
            let (a, b) = if reverse { (j + 1, j) } else { (j, j + 1) };
            mesh.triangle([rings[i][0], rings[i][a], rings[i][b]], color)?;
        }
    }
    Ok(())
}

pub(super) fn horn(
    mesh: &mut Builder,
    root: Vec3,
    bend: Vec3,
    tip: Vec3,
    width: f32,
    color: Rgb,
) -> Result<(), String> {
    // A narrow closed cap preserves the silhouette at the tip without coincident vertices.
    sweep(
        mesh,
        &[
            sec(root, width, width * 0.82),
            sec(bend, width * 0.60, width * 0.52),
            sec(tip, width * 0.035, width * 0.028),
        ],
        6,
        color,
    )
}

pub(super) fn beam(
    mesh: &mut Builder,
    a: Vec3,
    b: Vec3,
    width: f32,
    end: f32,
    color: Rgb,
) -> Result<(), String> {
    sweep(
        mesh,
        &[sec(a, width, width * 0.88), sec(b, end, end * 0.88)],
        6,
        color,
    )
}

/// Raised overlapping scutes follow the solid between stations. Each scute
/// has a broad shoulder, a ridge and a pointed free end above the next row.
pub(super) fn armoured(
    mesh: &mut Builder,
    path: &[Section],
    plate_count: usize,
    coat: Coat,
) -> Result<(), String> {
    sweep(mesh, path, 10, coat.shadow)?;
    for (i, pair) in path.windows(2).enumerate() {
        let a = pair[0];
        let b = pair[1];
        let axis = (b.pos - a.pos).normalize();
        let (u, v) = axes(axis);
        for j in 0..plate_count {
            let angle = TAU * (j as f32 + 0.5 + (i % 2) as f32 * 0.33) / plate_count as f32;
            let normal = u * angle.cos() + v * angle.sin();
            let under = normal.y + normal.z * 0.64 < -0.31;
            let shade = 0.95 + 0.08 * ((i * 7 + j * 3) as f32).sin();
            let color = tint(
                if under {
                    coat.belly
                } else if (i + 2 * j) % 5 == 0 {
                    coat.ridge
                } else {
                    coat.scale
                },
                shade,
            );
            let point = |t: f32, arc: f32, protrude: f32| {
                let (sn, cs) = (angle + arc * TAU / plate_count as f32).sin_cos();
                let w = a.width + (b.width - a.width) * t;
                let d = a.depth + (b.depth - a.depth) * t;
                a.pos.lerp(b.pos, t)
                    + u * (cs * w * (1.04 + protrude))
                    + v * (sn * d * (1.04 + protrude))
            };
            let verts = [
                point(0.00, -0.44, 0.),
                point(0.0, 0.44, 0.),
                point(0.58, 0.48, 0.016),
                point(0.94, 0.29, 0.02),
                point(1.09, -0.03, 0.012),
                point(0.83, -0.48, 0.012),
            ];
            let peak = point(0.48, 0.03, 0.065);
            for k in 0..6 {
                mesh.triangle(
                    [verts[k], verts[(k + 1) % 6], peak],
                    tint(color, [1.06, 1.01, 0.93, 0.88, 0.98, 1.04][k]),
                )?;
            }
        }
    }
    Ok(())
}

pub(super) fn block(
    mesh: &mut Builder,
    pos: Vec3,
    size: Vec3,
    angle: Quat,
    color: Rgb,
) -> Result<(), String> {
    mesh.bevel_box(pos.to_array(), size.to_array(), color, angle)
}
