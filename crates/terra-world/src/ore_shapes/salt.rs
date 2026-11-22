use super::{OreDetail, hash, point, sample};

pub const SALT_REACH: i64 = 10;
pub const MINERAL_REACH: i64 = 14;
const FLOOR_REACH: i64 = 4;

/// Sparse clusters leave walking space between deposits. The same query is used
/// on generation workers and by interaction; unknown cells stop growth.
pub fn salt_growth_height(
    position: [i64; 3],
    side: usize,
    block_at: impl FnMut([i64; 3]) -> Option<u16>,
) -> f32 {
    mineral_growth_height(30, position, side, block_at)
}

pub fn mineral_growth_height(
    material: u16,
    position: [i64; 3],
    side: usize,
    mut block_at: impl FnMut([i64; 3]) -> Option<u16>,
) -> f32 {
    if !matches!(material, 30 | 37) || !matches!(side, 2 | 3) {
        return 0.0;
    }
    let seed = hash(position, side, 0x7361_6c74_6772_6f77);
    let spacing = if material == 37 { 24 } else { 8 };
    if !seed.is_multiple_of(spacing) {
        return 0.0;
    }
    let direction = if side == 2 { -1 } else { 1 };
    let wanted = if side == 2 {
        2.0 + sample(seed, 16) * if material == 37 { 12.0 } else { 8.0 }
    } else {
        2.0 + sample(seed, 16) * 1.85
    };
    let mut free = 0;
    let mut opposing = false;
    for step in 1..=MINERAL_REACH {
        let Some(y) = position[1].checked_add(step * direction) else {
            break;
        };
        let p = [position[0], y, position[2]];
        match block_at(p) {
            Some(0 | 27) => free += 1,
            other => {
                opposing = matches!(other, Some(30 | 37))
                    && hash(p, side ^ 1, 0x7361_6c74_6772_6f77)
                        .is_multiple_of(if other == Some(37) { 24 } else { 8 });
                break;
            }
        }
    }
    let space = free as f32 * if opposing { 0.5 } else { 1.0 } - 0.04;
    if space < 2.0 { 0.0 } else { wanted.min(space) }
}

pub(super) fn visit_salt(
    position: [i64; 3],
    side: usize,
    detail: OreDetail,
    growth: f32,
    mut visit: impl FnMut([[f32; 3]; 3]),
) {
    visit_growth(30, position, side, detail, growth, &mut visit);
}

pub(super) fn visit_growth(
    material: u16,
    position: [i64; 3],
    side: usize,
    detail: OreDetail,
    growth: f32,
    mut visit: impl FnMut([[f32; 3]; 3]),
) {
    if side >= 6 {
        return;
    }
    let seed = hash(position, side, 0x7361_6c74_6661_6365);
    let tall = matches!(side, 2 | 3) && growth >= 2.0;
    if material == 37 && !tall && !seed.is_multiple_of(4) {
        return;
    }
    let height = if tall {
        growth.min(if side == 2 {
            if material == 37 {
                14.0
            } else {
                SALT_REACH as f32
            }
        } else {
            FLOOR_REACH as f32
        })
    } else if matches!(side, 2 | 3) {
        0.12 + sample(seed, 16) * 0.08
    } else {
        0.05 + sample(seed, 16) * 0.07
    };
    if material == 30 {
        visit_halite(position, side, detail, height, tall, &mut visit);
        return;
    }
    visit_calcite(position, side, detail, height, tall, visit);
}

/// Calcite grows in lobed curtains; exposed salt cleaves into flat-sided cuboids.
/// Both detail levels retain the same lead silhouette for picking from a distance.
pub(super) fn visit_calcite(
    position: [i64; 3],
    side: usize,
    detail: OreDetail,
    height: f32,
    prominent: bool,
    mut visit: impl FnMut([[f32; 3]; 3]),
) {
    let seed = hash(position, side, 0x4341_4c43_4954_4521);
    let count = if prominent && detail != OreDetail::Far {
        3
    } else {
        1
    };
    let centers = [[0.51, 0.50], [0.25, 0.37], [0.73, 0.65]];
    let sides = 8;
    for (i, &center) in centers.iter().take(count).enumerate() {
        let ticket = hash(position, side, seed.wrapping_add(i as u64 * 71));
        let h = height
            * if i == 0 {
                1.
            } else {
                0.24 + 0.30 * sample(ticket, 8)
            };
        let r = if i == 0 && prominent {
            0.34 + sample(ticket, 16) * 0.06
        } else {
            0.08 + sample(ticket, 16) * 0.045
        };
        let angle = sample(ticket, 32) * std::f32::consts::TAU;
        let lean = (sample(ticket, 40) - 0.5) * 0.06;
        let ring = |j: usize, t: f32, radius: f32| {
            let a = angle + j as f32 * std::f32::consts::TAU / sides as f32;
            let flute = if j.is_multiple_of(2) { 1. } else { 0.78 };
            point(
                side,
                [
                    center[0] + r * radius * flute * a.cos() + lean * t,
                    center[1] + r * radius * flute * a.sin() - lean * t * 0.6,
                    if t == 0. { -0.025 } else { t * h },
                ],
            )
        };
        let mut emit = |mut tri: [[f32; 3]; 3]| {
            if side.is_multiple_of(2) {
                tri.swap(1, 2);
            }
            visit(tri);
        };
        let tip = point(side, [center[0] + lean, center[1] - lean * 0.6, h]);
        for j in 0..sides {
            let k = (j + 1) % sides;
            let a = ring(j, 0., 1.);
            let b = ring(k, 0., 1.);
            let m = ring(j, 0.23, 0.63);
            let n = ring(k, 0.23, 0.63);
            let c = ring(j, 0.70, 0.24);
            let d = ring(k, 0.70, 0.24);
            emit([a, b, m]);
            emit([m, b, n]);
            emit([m, n, c]);
            emit([c, n, d]);
            emit([c, d, tip]);
        }
    }
}

pub(super) fn visit_halite(
    position: [i64; 3],
    side: usize,
    detail: OreDetail,
    height: f32,
    tall: bool,
    mut visit: impl FnMut([[f32; 3]; 3]),
) {
    let seed = hash(position, side, 0x4841_4c49_5445);
    let centers = [
        [0.51, 0.52],
        [0.28, 0.33],
        [0.75, 0.62],
        [0.59, 0.23],
        [0.29, 0.73],
    ];
    let count = match (tall, detail) {
        (true, OreDetail::Near) => 5,
        (true, OreDetail::Middle) => 3,
        (true, OreDetail::Far) => 2,
        (false, OreDetail::Near) => 3,
        (false, OreDetail::Middle) => 2,
        (false, OreDetail::Far) => 1,
    };
    let orientation = sample(seed, 0) * std::f32::consts::TAU;
    for (i, &root) in centers.iter().take(count).enumerate() {
        let variation = hash(position, side, seed.wrapping_add(i as u64 * 113));
        let h = height
            * if i == 0 {
                1.
            } else {
                0.37 + 0.44 * sample(variation, 8)
            };
        let radius = if i == 0 {
            if tall { 0.21 } else { 0.155 }
        } else {
            0.095 + 0.036 * sample(variation, 16)
        };
        let angle = orientation + (sample(variation, 24) - 0.5) * 0.52;
        let (s, c) = angle.sin_cos();
        let ring = |j: usize, level: f32, width: f32| {
            let [dx, dz] = [[-1., -1.], [1., -1.], [1., 1.], [-1., 1.]][j];
            let x = dx * width * radius;
            let z = dz * width * radius * (0.70 + 0.25 * sample(variation, 32));
            let lean = 0.13 * level;
            point(
                side,
                [
                    root[0] + (root[0] - 0.5) * lean + c * x - s * z,
                    root[1] + (root[1] - 0.5) * lean + s * x + c * z,
                    if level == 0. {
                        -0.055
                    } else {
                        level * h + (dx + dz - 2.) * 0.045 * radius
                    },
                ],
            )
        };
        let mut emit = |mut tri: [[f32; 3]; 3]| {
            if side.is_multiple_of(2) {
                tri.swap(1, 2);
            }
            visit(tri)
        };
        for j in 0..4 {
            let k = (j + 1) % 4;
            let a = ring(j, 0., 0.84);
            let b = ring(k, 0., 0.84);
            let m = ring(j, 0.88, 1.);
            let n = ring(k, 0.88, 1.);
            let c = ring(j, 1., 0.79);
            let d = ring(k, 1., 0.79);
            emit([a, b, m]);
            emit([m, b, n]);
            emit([m, n, c]);
            emit([c, n, d]);
        }
        let cap = [0, 1, 2, 3].map(|j| ring(j, 1., 0.79));
        emit([cap[0], cap[1], cap[2]]);
        emit([cap[0], cap[2], cap[3]]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dripstone_inlays_leave_empty_faces_and_long_roots_stay_in_their_column() {
        let mut decorated = 0;
        for x in -128..128 {
            let p = [x, -32, 16];
            let mut count = 0;
            visit_growth(37, p, 2, OreDetail::Near, 0.0, |_| count += 1);
            decorated += usize::from(count > 0);
            let mut triangles = 0;
            visit_growth(37, p, 2, OreDetail::Far, 14.0, |triangle| {
                triangles += 1;
                for vertex in triangle {
                    assert!((0.0..=1.0).contains(&vertex[0]));
                    assert!((0.0..=1.0).contains(&vertex[2]));
                }
            });
            assert_eq!(triangles, 40);
        }
        assert!((32..96).contains(&decorated));
    }

    #[test]
    fn dripstone_is_sparse_longer_and_blocked_by_unknown_or_solid_cells() {
        let mut drip = 0;
        let mut salt = 0;
        let mut longest = 0.0_f32;
        for x in -512..512 {
            let p = [x, 16, -16];
            salt += usize::from(salt_growth_height(p, 2, |_| Some(0)) > 0.0);
            let h = mineral_growth_height(37, p, 2, |_| Some(0));
            if h > 0.0 {
                drip += 1;
                longest = longest.max(h);
                assert!(h <= 14.0);
                assert_eq!(mineral_growth_height(37, p, 2, |_| None), 0.0);
                assert_eq!(
                    mineral_growth_height(37, p, 2, |q| Some(if q[1] < 14 { 3 } else { 0 })),
                    0.0
                );
            }
        }
        assert!(longest > 12.0 && drip < salt / 2 && drip > 10);
    }

    #[test]
    fn floor_ceiling_lengths_clearance_and_lod_are_bounded() {
        for side in [2, 3] {
            let mut count = 0;
            let mut longest = 0.0_f32;
            for x in -64..64 {
                let p = [x, -16, -17];
                let h = salt_growth_height(p, side, |_| Some(0));
                if h == 0.0 {
                    continue;
                }
                count += 1;
                longest = longest.max(h);
                assert!((2.0..=if side == 2 { 10.0 } else { 4.0 }).contains(&h));
                assert_eq!(salt_growth_height(p, side, |_| None), 0.0);
                assert_eq!(
                    salt_growth_height(p, side, |q| {
                        ((q[1] - p[1]).abs() < 3).then_some(0).or(Some(3))
                    }),
                    0.0
                );
                let mut near = Vec::new();
                visit_salt(p, side, OreDetail::Near, h, |t| near.push(t));
                assert_eq!(near.len(), 5 * 18);
                for lod in [OreDetail::Middle, OreDetail::Far] {
                    visit_salt(p, side, lod, h, |t| assert!(near.contains(&t)));
                }
                for v in near.into_iter().flatten() {
                    assert!(v.iter().all(|value| value.is_finite()));
                    assert!((0.0..=1.0).contains(&v[0]) && (0.0..=1.0).contains(&v[2]));
                    assert!((-10.0..=5.0).contains(&v[1]));
                }
            }
            assert!(count >= 5 && longest > if side == 2 { 9.0 } else { 3.5 });
        }
    }
}
