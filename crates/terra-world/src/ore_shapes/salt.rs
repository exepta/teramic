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
    let count = match (tall, detail) {
        (true, _) if material == 37 => 2,
        (true, OreDetail::Near) => 5,
        (true, OreDetail::Middle) => 3,
        (true, OreDetail::Far) => 2,
        (false, OreDetail::Near) => 3,
        (false, OreDetail::Middle) => 2,
        (false, OreDetail::Far) => 1,
    };
    let centers = [
        [0.52, 0.52],
        [0.28, 0.34],
        [0.75, 0.64],
        [0.56, 0.22],
        [0.29, 0.72],
    ];
    for (index, center) in centers[..count].iter().enumerate() {
        let random = hash(position, side, seed.wrapping_add(index as u64 * 71));
        let h = height
            * if index == 0 {
                1.0
            } else {
                0.32 + sample(random, 8) * 0.47
            };
        let radius = if index == 0 {
            if material == 37 {
                (0.24 + height * 0.015).min(0.44)
            } else {
                0.13
            }
        } else {
            0.065 + sample(random, 24) * 0.035
        };
        let angle = sample(random, 32) * std::f32::consts::TAU;
        let lean = [
            (sample(random, 40) - 0.5) * 0.07,
            (sample(random, 48) - 0.5) * 0.07,
        ];
        let ring = |i: usize, upper: bool| {
            let a = angle + i as f32 * std::f32::consts::FRAC_PI_2;
            let r = radius
                * if upper {
                    if material == 37 { 0.38 } else { 0.73 }
                } else {
                    1.0
                };
            [
                center[0] + a.cos() * r + if upper { lean[0] } else { 0.0 },
                center[1] + a.sin() * r + if upper { lean[1] } else { 0.0 },
                if upper {
                    h * if material == 37 { 0.55 } else { 0.90 }
                } else if tall {
                    -0.075
                } else {
                    -0.02
                },
            ]
        };
        let tip = [center[0] + lean[0], center[1] + lean[1], h];
        for i in 0..4 {
            let a = ring(i, false);
            let b = ring((i + 1) % 4, false);
            let c = ring(i, true);
            let d = ring((i + 1) % 4, true);
            let triangles = if tall {
                [[a, b, c], [c, b, d], [c, d, tip]]
            } else {
                [[a, b, tip]; 3]
            };
            for triangle in &triangles[..if tall { 3 } else { 1 }] {
                let mut triangle = triangle.map(|v| point(side, v));
                if side.is_multiple_of(2) {
                    triangle.swap(1, 2);
                }
                visit(triangle);
            }
        }
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
            assert_eq!(triangles, 24);
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
                assert_eq!(near.len(), 60);
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
