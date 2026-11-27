//! Face-local relief for cut stone, inlays, exposed mineral pockets and joinery.
use super::super::{PrimitiveTemplate, definitions::CubeSurface};
use super::patch::FacePatch as Face;
use bevy::math::Vec2;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum CraftedForm {
    Masonry,
    Temple,
    RootStone,
    CrystalRock,
    Planks,
    Shingles,
    Textile,
    BoundStraw,
}

pub(super) fn classify(name: &str, surface: Option<CubeSurface>) -> Option<CraftedForm> {
    use CraftedForm::*;
    let name = name.to_ascii_lowercase().replace(['_', '-'], " ");
    Some(
        if name.contains("root bound") || name.contains("rooted") || name.contains("root soil") {
            RootStone
        } else if [
            "carved",
            "temple",
            "ornate",
            "runestone",
            "rune ",
            "mosaic",
            "keystone",
        ]
        .iter()
        .any(|s| name.contains(s))
        {
            Temple
        } else if ["shingles", "roof slate", "roof tiles"]
            .iter()
            .any(|s| name.contains(s))
        {
            Shingles
        } else if ["planks", "timber"].iter().any(|s| name.contains(s)) {
            Planks
        } else if matches!(surface, Some(CubeSurface::Wool)) || name.contains("wool") {
            Textile
        } else if name.contains("hay bale") || name.contains("straw block") {
            BoundStraw
        } else if [
            "masonry",
            "brick",
            "ashlar",
            "ruin stone",
            "grave stone",
            "structure stone",
            "polished",
            "smooth stone",
            "plaster",
            "lighthouse stripe",
        ]
        .iter()
        .any(|s| name.contains(s))
        {
            Masonry
        } else if (name.contains("crystal") && !name.contains("gravel"))
            || name.ends_with(" ore")
            || name.ends_with(" spire")
            || name.contains("mineral")
        {
            CrystalRock
        } else {
            return None;
        },
    )
}

/// A complete 1 m block already exists behind this geometry. Parts start on
/// its face and rise outward. Nothing here replaces or cuts a block face.
pub(super) fn face(form: CraftedForm, side: usize, variant: usize) -> Vec<PrimitiveTemplate> {
    let mut face = Face::new(side, variant);
    let phase = (face.variant + side) % 4;
    match form {
        CraftedForm::Masonry => courses(&mut face, phase),
        CraftedForm::Temple => frieze(&mut face, phase),
        CraftedForm::RootStone => roots(&mut face, phase),
        CraftedForm::CrystalRock => crystal_outcrop(&mut face, phase),
        CraftedForm::Planks => joinery(&mut face, phase, false),
        CraftedForm::Shingles => joinery(&mut face, phase, true),
        CraftedForm::Textile => weave(&mut face, phase),
        CraftedForm::BoundStraw => straw(&mut face, phase),
    }
    face.meshes
}

const ACCENT: usize = 7;
type Point = [f32; 3];

fn rim<const N: usize>(face: &mut Face, slot: usize, foot: &[Point; N], top: &[Point; N]) {
    for i in 0..N {
        let j = (i + 1) % N;
        face.wound_polygon(slot, [foot[i], foot[j], top[j], top[i]]);
    }
}

/// An attached plate, with two chamfered corners and a visible shoulder.
/// Height is measured outward here; FacePatch uses signed inward depth.
fn plate(face: &mut Face, bounds: [f32; 4], height: f32, chip: f32) {
    let [x, y, r, t] = bounds;
    let cut = chip.min((r - x) * 0.24).min((t - y) * 0.42);
    let plan = [
        [x, y + 0.45 * cut],
        [x + 0.70 * cut, y],
        [r - cut, y],
        [r, y + cut],
        [r, t],
        [x, t],
    ];
    let center = Vec2::new((x + r) * 0.5, (y + t) * 0.5);
    let base = plan.map(|[u, v]| [u, v, 0.0]);
    let top = plan.map(|p| {
        let q = Vec2::from_array(p);
        let inset = q.lerp(center, 0.10);
        [inset.x, inset.y, -height]
    });
    rim(face, face.side, &base, &top);
    face.wound_polygon(face.side, top);
}

fn courses(face: &mut Face, phase: usize) {
    let joint = [0.46, 0.54, 0.42, 0.51][phase];
    for (row, (y, t)) in [(0.025, joint - 0.016), (joint + 0.015, 0.973)]
        .into_iter()
        .enumerate()
    {
        let cuts = if (phase + row) % 2 == 0 {
            [0.29 + 0.019 * phase as f32, 0.66, 0.975]
        } else {
            [0.41 + 0.03 * phase as f32, 0.975, 0.975]
        };
        let mut x = 0.026;
        for (i, r) in cuts.into_iter().enumerate() {
            if r <= x {
                continue;
            }
            let height = [0.021, 0.046, 0.034, 0.052][(phase + row + i) % 4];
            let cut = [0.034, 0.067, 0.049, 0.078][(phase + row * 2 + i) % 4];
            plate(face, [x, y, r, t], height, cut);
            x = r + 0.023;
        }
    }
}

// Attached faceted ridge. The base follows the block face along both edges.
// A bevel remains visible in profile even when the textured block behind it
// has the same material as the ridge.
fn strand(face: &mut Face, slot: usize, path: &[[f32; 3]], height: f32) {
    let section = |i: usize| {
        let [u, v, width] = path[i];
        let a = path[i.saturating_sub(1)];
        let b = path[(i + 1).min(path.len() - 1)];
        let tangent = Vec2::new(b[0] - a[0], b[1] - a[1]).normalize();
        let right = Vec2::new(tangent.y, -tangent.x) * width;
        let h = height.min(width * 1.35);
        [(1.0, 0.0), (0.40, -h * 0.88), (-0.23, -h), (-1.0, 0.0)]
            .map(|(k, d)| [u + right.x * k, v + right.y * k, d])
    };
    let mut prev = section(0);
    face.wound_polygon(slot, prev);
    for k in 1..path.len() {
        let next = section(k);
        for i in 0..3 {
            face.wound_polygon(slot, [prev[i], next[i], next[i + 1], prev[i + 1]]);
        }
        prev = next;
    }
    face.wound_polygon(slot, [prev[3], prev[2], prev[1], prev[0]]);
}

fn roots(face: &mut Face, phase: usize) {
    // A few chips remain visible between the petrified ropes. The central
    // color mesh contributes the original stone and every unadorned area.
    plate(face, [0.060, 0.061, 0.38, 0.27], 0.021, 0.058);
    plate(face, [0.60, 0.64, 0.924, 0.92], 0.025, 0.076);
    if face.side == 2 {
        return;
    }
    let rotate = |[mut u, mut v, w]: [f32; 3]| {
        for _ in 0..face.variant {
            (u, v) = (1.0 - v, u);
        }
        [u, v, w]
    };
    let main = [
        [0.28, 0.982, 0.043],
        [0.28, 0.85, 0.049],
        [0.39, 0.70, 0.056],
        [0.52, 0.57, 0.069],
        [0.54, 0.43, 0.058],
        [0.72, 0.35, 0.061],
        [0.86, 0.39, 0.053],
        [0.982, 0.39, 0.047],
    ]
    .map(rotate);
    let other = [
        [0.67, 0.018, 0.040],
        [0.67, 0.16, 0.049],
        [0.55, 0.28, 0.043],
        [0.43, 0.39, 0.052],
        [0.42, 0.52, 0.050],
        [0.29, 0.64, 0.041],
        [0.14, 0.68, 0.048],
        [0.018, 0.68, 0.044],
    ]
    .map(rotate);
    if face.side == 3 {
        let branch = [
            [0.54, 0.44, 0.032],
            [0.67, 0.52, 0.025],
            [0.77, 0.61, 0.012],
        ]
        .map(rotate);
        strand(face, ACCENT, &other, 0.044);
        strand(face, ACCENT, &main, 0.069);
        strand(face, ACCENT, &branch, 0.034);
        return;
    }
    // The widths and edge positions correspond on the top and adjoining
    // sides. Leave a narrow border for a directly adjoining block or overlay.
    for ([x, z, r], direction) in [
        (main[0], 1.),
        (main[7], -1.),
        (other[0], -1.),
        (other[7], 1.),
    ] {
        let (side, u) = if x < 0.02 {
            (0, z)
        } else if x > 0.98 {
            (1, z)
        } else if z < 0.02 {
            (4, x)
        } else {
            (5, x)
        };
        if side != face.side {
            continue;
        }
        let inward = if u < 0.5 { 1.0 } else { -1.0 };
        let leg = [
            [u, 0.982, r],
            [u, 0.86, r * 0.96],
            [u + 0.1 * inward, 0.71, r * 1.16],
            [u + 0.21 * inward, 0.53, r],
            [u + 0.17 * inward, 0.33, r * 0.84],
            [u + 0.30 * inward, 0.15, r * 0.62],
            [u + 0.34 * inward, 0.057, r * 0.22],
        ];
        let twig = [
            [u + 0.20 * inward, 0.55, r * 0.70],
            [u + 0.20 * inward + 0.16 * direction, 0.46, r * 0.60],
            [u + 0.20 * inward + 0.26 * direction, 0.31, r * 0.30],
            [u + 0.20 * inward + 0.31 * direction, 0.23, r * 0.17],
        ];
        strand(face, ACCENT, &leg, 0.062 + 0.002 * phase as f32);
        strand(face, ACCENT, &twig, 0.030);
    }
}

fn crystal_outcrop(face: &mut Face, phase: usize) {
    plate(face, [0.095, 0.11, 0.41, 0.32], 0.029, 0.08);
    plate(face, [0.65, 0.62, 0.94, 0.87], 0.021, 0.07);
    if face.side == 2 || face.side == [0, 4, 1, 5][face.variant] {
        return;
    }
    let crystals = [
        ([0.46, 0.48], [0.39, 0.41], 0.107, 0.236),
        ([0.61, 0.51], [0.74, 0.54], 0.084, 0.168),
        ([0.36, 0.61], [0.27, 0.70], 0.066, 0.128),
    ];
    let count = if face.side == 3 { 3 } else { 1 };
    for (i, (mut base, mut tip, mut width, mut height)) in
        crystals.into_iter().take(count).enumerate()
    {
        if face.side != 3 {
            width *= 0.54 + 0.07 * (phase % 2) as f32;
            height *= 0.40;
        }
        for _ in 0..phase {
            base = [1. - base[1], base[0]];
            tip = [1. - tip[1], tip[0]];
        }
        let base = Vec2::from_array(base);
        let tip = Vec2::from_array(tip);
        let scale = 1.0 - 0.025 * ((face.variant + i) % 3) as f32;
        let ring = |at: Vec2, r: f32, d: f32| {
            [
                (1.0, 0.0),
                (0.48, 0.87),
                (-0.59, 0.79),
                (-1.0, -0.08),
                (-0.39, -0.91),
                (0.56, -0.77),
            ]
            .map(|(x, y)| [at.x + r * x, at.y + r * y, d])
        };
        let foot = ring(base, width * 0.92, 0.0);
        let shoulder = ring(base.lerp(tip, 0.68), width, -height * 0.70 * scale);
        rim(face, ACCENT, &foot, &shoulder);
        for j in 0..6 {
            face.wound_polygon(
                ACCENT,
                [
                    shoulder[j],
                    shoulder[(j + 1) % 6],
                    [tip.x, tip.y, -height * scale],
                ],
            );
        }
    }
    if let Some(gems) = face.meshes.iter_mut().find(|p| p.material == ACCENT) {
        for i in 0..gems.positions.len() {
            let normal = bevy::math::Vec3::from_array(gems.normals[i]);
            let [x, y, z] = gems.positions[i];
            let (axis, uv) = if normal.y.abs() > normal.x.abs().max(normal.z.abs()) {
                (3, [x, z])
            } else if normal.x.abs() > normal.z.abs() {
                (0, [z, y])
            } else {
                (4, [x, y])
            };
            gems.uvs[i] = uv;
            gems.tangents[i] = super::super::projected_tangent(normal, axis);
        }
    }
}

fn frieze(face: &mut Face, phase: usize) {
    if face.side == 2 || face.side == 3 {
        courses(face, phase);
        return;
    }
    plate(face, [0.019, 0.023, 0.981, 0.139], 0.050, 0.035);
    plate(face, [0.035, 0.156, 0.187, 0.799], 0.032, 0.044);
    plate(face, [0.812, 0.156, 0.965, 0.799], 0.032, 0.044);
    plate(face, [0.019, 0.809, 0.981, 0.976], 0.054, 0.035);
    plate(face, [0.176, 0.730, 0.823, 0.789], 0.030, 0.024);
    if face.side == 5 {
        let sweep = [0., 0.029, -0.022, 0.044][phase];
        strand(
            face,
            face.side,
            &[
                [0.47, 0.18, 0.044],
                [0.51, 0.34, 0.036],
                [0.50, 0.51, 0.033],
                [0.53 + sweep, 0.63, 0.023],
                [0.55 + sweep, 0.69, 0.012],
            ],
            0.050,
        );
        strand(
            face,
            face.side,
            &[
                [0.50, 0.38, 0.029],
                [0.36, 0.45, 0.029],
                [0.29, 0.58 + sweep, 0.011],
            ],
            0.040,
        );
        strand(
            face,
            face.side,
            &[
                [0.51, 0.46, 0.028],
                [0.63, 0.51 + sweep, 0.026],
                [0.71, 0.65, 0.012],
            ],
            0.044,
        );
    } else {
        let v = 0.45 + 0.019 * phase as f32;
        let foot = [
            [0.33, v, 0.],
            [0.50, v - 0.15, 0.],
            [0.67, v, 0.],
            [0.50, v + 0.15, 0.],
        ];
        let top = foot.map(|[x, y, _]| [0.50 + (x - 0.50) * 0.78, v + (y - v) * 0.78, -0.034]);
        rim(face, face.side, &foot, &top);
        face.wound_polygon(face.side, top);
    }
}

fn joinery(face: &mut Face, phase: usize, shingles: bool) {
    let rows = 3;
    for row in 0..rows {
        let y = 0.019 + 0.955 * row as f32 / rows as f32;
        let t = 0.011 + 0.955 * (row + 1) as f32 / rows as f32;
        let seam = [0.33, 0.65, 0.51, 0.40][(row + phase) % 4];
        let split = [[0.020, seam - 0.009], [seam + 0.009, 0.980]];
        let whole = [[0.020, 0.980]];
        let boards = if shingles || (row + phase) % 2 == 0 {
            &split[..]
        } else {
            &whole[..]
        };
        for &[x, r] in boards {
            if shingles {
                let foot = [[x, y, 0.0], [r, y, 0.0], [r, t, 0.0], [x, t, 0.0]];
                let h = 0.037 + 0.003 * ((row + phase) % 3) as f32;
                let top = [
                    [x + 0.010, y + 0.010, -h],
                    [r - 0.013, y + 0.010, -h],
                    [r - 0.009, t - 0.007, -0.008],
                    [x + 0.007, t - 0.007, -0.008],
                ];
                rim(face, face.side, &foot, &top);
                face.wound_polygon(face.side, top);
            } else {
                plate(
                    face,
                    [x, y, r, t],
                    0.013 + 0.005 * ((phase + row) % 3) as f32,
                    0.021,
                );
            }
        }
    }
}

fn weave(face: &mut Face, phase: usize) {
    // Each tuft is rooted; the raised warp/weft crowns alternate at crossings.
    let lanes = [0.20, 0.50, 0.80];
    for (n, u) in lanes.into_iter().enumerate() {
        for horizontal in [false, true] {
            let mut last = None;
            for (k, v) in [0.025, 0.20, 0.50, 0.80, 0.975].into_iter().enumerate() {
                let high = ((k + n + phase) % 2 == 0) ^ horizontal;
                let h = if k == 0 || k == 4 {
                    0.0
                } else if high {
                    0.025
                } else {
                    0.007
                };
                let width = [0.061, 0.055, 0.058, 0.064][(phase + n) % 4];
                let at = u + if k % 2 == 0 { 0.011 } else { -0.005 };
                let section = [(width, 0.), (0., -h), (-width, 0.)].map(|(offset, d)| {
                    if horizontal {
                        [v, at + offset, d]
                    } else {
                        [at - offset, v, d]
                    }
                });
                if let Some(prev) = last {
                    let prev: [[f32; 3]; 3] = prev;
                    for j in 0..2 {
                        face.wound_polygon(
                            face.side,
                            [prev[j + 1], section[j + 1], section[j], prev[j]],
                        );
                    }
                }
                last = Some(section);
            }
        }
    }
}

fn straw(face: &mut Face, phase: usize) {
    for i in 0..5 {
        let u = 0.104 + i as f32 * 0.198;
        let wobble = [0.009, -0.014, 0.012, -0.004][(i + phase) % 4];
        strand(
            face,
            face.side,
            &[
                [u, 0.031, 0.046],
                [u + wobble, 0.50, 0.072],
                [u, 0.963, 0.050],
            ],
            0.028,
        );
    }
    for v in [0.28 + 0.006 * phase as f32, 0.72 - 0.006 * phase as f32] {
        strand(
            face,
            face.side,
            &[
                [0.040, v, 0.027],
                [0.50, v + 0.011, 0.031],
                [0.959, v - 0.011, 0.028],
            ],
            0.047,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ornaments_project_from_full_faces_and_never_cross_a_lateral_grid_plane() {
        use CraftedForm::*;
        for form in [
            Masonry,
            Temple,
            RootStone,
            CrystalRock,
            Planks,
            Shingles,
            Textile,
            BoundStraw,
        ] {
            for variant in 0..4 {
                for side in 0..6 {
                    let parts = face(form, side, variant);
                    let axis = side / 2;
                    let mut height = 0.0_f32;
                    for p in parts.iter().flat_map(|mesh| &mesh.positions) {
                        for (i, &v) in p.iter().enumerate() {
                            if i == axis {
                                let outward = if side % 2 == 0 { -v } else { v - 1.0 };
                                assert!((0.0..=0.24).contains(&outward), "{form:?} {side} {p:?}");
                                height = height.max(outward);
                            } else {
                                assert!((0.01..=0.99).contains(&v), "{form:?} {side} {p:?}");
                            }
                        }
                    }
                    assert!(height > 0.007, "{form:?} {side} has no physical relief");
                }
            }
        }
    }
}
