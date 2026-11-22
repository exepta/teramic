//! Exposed cleaved crystals. All detail levels keep the same principal crystal
//! and face planes, so switching LOD cannot move a glint or change the pick surface.
use super::{OreDetail, OreKind, hash, point, sample};

struct Habit {
    sides: usize,
    height: f32,
    width: f32,
    aspect: f32,
    shoulder: f32,
    table: f32,
    irregular: f32,
}

impl Habit {
    fn for_ore(ore: OreKind) -> Self {
        let (sides, height, width, aspect, shoulder, table, irregular) = match ore {
            // Coal cleaves into thin, fractured shelves. Native hematite grows in
            // flat steel-grey blades; gold is a much rounder, lumpy intergrowth.
            OreKind::Coal => (6, 0.43, 0.23, 0.78, 0.46, 0.72, 0.23),
            OreKind::Iron => (5, 0.73, 0.20, 0.57, 0.72, 0.81, 0.14),
            OreKind::Gold => (7, 0.56, 0.22, 0.87, 0.48, 0.60, 0.22),
            OreKind::Diamond => (8, 0.78, 0.22, 0.91, 0.54, 0.44, 0.04),
            OreKind::Ruby => (6, 0.91, 0.185, 0.94, 0.76, 0.64, 0.045),
            OreKind::Salt => (4, 1.0, 0.16, 0.97, 0.87, 0.89, 0.035),
        };
        Self {
            sides,
            height,
            width,
            aspect,
            shoulder,
            table,
            irregular,
        }
    }
}

pub(super) fn deposit(
    ore: OreKind,
    pos: [i64; 3],
    side: usize,
    lod: OreDetail,
    mut visit: impl FnMut([[f32; 3]; 3], f32),
) {
    if side >= 6 {
        return;
    }
    let h = Habit::for_ore(ore);
    let seed = hash(pos, side, 0x4745_4f44 ^ u64::from(ore.material()));
    let angle = sample(seed, 0) * std::f32::consts::TAU;
    let center = [
        0.45 + 0.10 * sample(seed, 8),
        0.43 + 0.14 * sample(seed, 16),
    ];
    let height = h.height * (0.58 + 0.40 * sample(seed, 24)) * if side == 3 { 1.0 } else { 0.45 };
    let count = match lod {
        OreDetail::Near => 3,
        OreDetail::Middle => 2,
        OreDetail::Far => 1,
    };
    for i in 0..count {
        let random = hash(pos, side, seed.wrapping_add(i as u64 * 109));
        let direction = angle + [0., 2.2, -1.3][i];
        let off = [direction.cos(), direction.sin()];
        let length = [0., 0.21, 0.22][i];
        let root = [center[0] + off[0] * length, center[1] + off[1] * length];
        let rise = height * [1., 0.66, 0.36][i];
        let radius = h.width * [1., 0.70, 0.50][i];
        let sides = [h.sides, 5, 3][i];
        let lean = [0.052, 0.105, 0.071][i];
        let turn = angle + sample(random, 32) * 1.2;
        let vertex = |j: usize, level: f32, scale: f32| {
            let phi = j as f32 * std::f32::consts::TAU / sides as f32;
            let edge = 1. + h.irregular * (phi * 2. + turn).sin();
            let (x, z) = (
                radius * edge * scale * phi.cos(),
                radius * h.aspect * edge * scale * phi.sin(),
            );
            let (s, c) = turn.sin_cos();
            let taper = if ore == OreKind::Diamond { 0.64 } else { 0.82 };
            let base = if level == 0. { taper } else { 1. };
            point(
                side,
                [
                    root[0] + (c * x - s * z) * base + off[0] * level * lean,
                    root[1] + (s * x + c * z) * base + off[1] * level * lean,
                    if level == 0. {
                        -0.075
                    } else {
                        rise * level + x * (0.14 + 0.22 * sample(random, 48))
                    },
                ],
            )
        };
        let mut emit = |mut tri: [[f32; 3]; 3], tone: f32| {
            if side.is_multiple_of(2) {
                tri.swap(1, 2);
            }
            visit(tri, tone)
        };
        for j in 0..sides {
            let k = (j + 1) % sides;
            let a = vertex(j, 0., 1.);
            let b = vertex(k, 0., 1.);
            let c = vertex(j, 1., h.table);
            let d = vertex(k, 1., h.table);
            let glint = 0.78 + 0.19 * (turn + j as f32 * 1.83).cos();
            if i == 0 {
                let m = vertex(j, h.shoulder, 1.);
                let n = vertex(k, h.shoulder, 1.);
                emit([a, b, m], glint);
                emit([m, b, n], glint);
                emit([m, n, c], glint + 0.22);
                emit([c, n, d], glint + 0.22);
            } else {
                emit([a, b, c], glint);
                emit([c, b, d], glint);
            }
            if j > 0 && j + 1 < sides {
                emit([vertex(0, 1., h.table), c, d], 1.14);
            }
        }
    }
}
