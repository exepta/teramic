//! Submerged reef grotto. Its open arches and bone cradle sit on a real ocean
//! shelf, with the same continuous salt water inside and outside the nest.
use super::*;

pub(super) fn shelf(
    t: &TerraGenerator,
    middle: [i64; 2],
    seed: u32,
) -> Option<([i64; 2], [i64; 2], i64)> {
    let sea = t.config().sea_level;
    let bed = |x: i64, z: i64| {
        t.natural_surface_water(x as f64 + 0.5, z as f64 + 0.5)
            .filter(|w| (w.level - sea).abs() < 0.25)
            .map(|w| w.bed as i64)
    };
    let mut best = None;
    let mut score = i64::MAX;
    for dz in (-72_i64..=72).step_by(24) {
        for dx in (-72_i64..=72).step_by(24) {
            let [x, z] = [middle[0] + dx, middle[1] + dz];
            let Some(center) = bed(x, z) else { continue };
            if center as f64 > sea - 16. {
                continue;
            }
            let Some(heights) = [(-9, -9), (9, -9), (-9, 9), (9, 9)]
                .map(|(u, v)| bed(x + u, z + v))
                .into_iter()
                .collect::<Option<Vec<_>>>()
            else {
                continue;
            };
            let floor = heights.iter().copied().max().unwrap().max(center) + 2;
            if floor as f64 + 16. >= sea {
                continue;
            }
            for i in 0..4 {
                let dir = [[1, 0], [0, 1], [-1, 0], [0, -1]][(i + seed as usize) % 4];
                let Some(outer) = bed(x + dir[0] * 28, z + dir[1] * 28) else {
                    continue;
                };
                if outer > floor + 3 {
                    continue;
                }
                let rank = (floor - heights.iter().copied().min().unwrap()).abs() * 12
                    + dx.abs() / 8
                    + dz.abs() / 8
                    + (outer - floor).abs();
                if rank < score {
                    score = rank;
                    best = Some(([x, z], dir, floor));
                }
            }
        }
    }
    best
}

pub(super) fn build(b: &mut Builder<'_>, [fx, fz]: [i64; 2]) {
    let [cx, y, cz] = b.summary.center;
    let at = |u: i64, h: i64, v: i64| [cx + u * fz + v * fx, y + h, cz - u * fx + v * fz];
    for v in -24_i64..=35 {
        for u in -23_i64..=23 {
            let radius = (u as f64 / 22.).hypot((v as f64 + 2.) / 23.);
            let entrance = v > 5 && u.abs() < 13 && (u.abs() < 9 || v < 28);
            if radius > 1.13 && !entrance {
                continue;
            }
            let [x, _, z] = at(u, 0, v);
            let bed =
                (b.terrain.surface_height(x as f64 + 0.5, z as f64 + 0.5) - 0.5).floor() as i64 + 1;
            let n = layer_seed(u64::from(b.seed) ^ (u as u64).rotate_left(23), v as u64);
            let rim = radius > 0.76 && !entrance;
            let footing = if rim {
                ((1.08 - radius).max(0.) * 9.).round() as i64
            } else {
                -1
            };
            for h in (bed - y - 1).max(-14)..=footing {
                b.set(
                    at(u, h, v),
                    if h == footing && !rim {
                        M::Gravel
                    } else if (h + v.div_euclid(5)).rem_euclid(6) == 0 {
                        M::MossyStone
                    } else {
                        M::WetSaltStone
                    } as u16,
                );
            }
            for h in (footing + 1)..=(bed - y + 1).max(2) {
                b.set(at(u, h, v), M::Air as u16);
            }
            if rim {
                let cap = (3.
                    + 8. * (1. - ((radius - 0.95) * 5.).abs()).max(0.)
                    + 2.2 * (v as f64 * 0.24).sin())
                .round() as i64;
                for h in footing + 1..=cap {
                    b.set(
                        at(u, h, v),
                        if h == cap || (u + v + h).rem_euclid(13) == 0 {
                            M::MossyStone
                        } else {
                            M::WetSaltStone
                        } as u16,
                    );
                }
                if n.is_multiple_of(11) {
                    b.set(at(u, cap + 1, v), M::RibbonKelp as u16);
                }
            } else if radius > 0.48 && n.is_multiple_of(11) {
                let coral = [
                    M::LuminousCoral,
                    M::StaghornCoral,
                    M::CoralBush,
                    M::TideGeode,
                    M::TableCoral,
                ];
                b.set(
                    at(u, footing + 1, v),
                    coral[(n as usize / 11) % coral.len()] as u16,
                );
            }
        }
    }
    // Two ribbed stone arches mark the mouth. A broken crown admits shafts of daylight.
    for (v, r) in [(13_i64, 14_i64), (24, 11)] {
        for u in -r..=r {
            let crown = (15. * (1. - (u as f64 / r as f64).powi(2)).max(0.).sqrt()).round() as i64;
            for z in v - 2..=v + 2 {
                let lower = if u.abs() >= r - 2 { -3 } else { crown - 2 };
                for h in lower..=crown {
                    b.set(
                        at(u, h, z),
                        if h == crown {
                            M::MossyStone
                        } else {
                            M::DeepSaltStone
                        } as u16,
                    );
                }
            }
            if u % 6 == 0 {
                b.set(at(u, crown - 3, v), M::LuminousCoral as u16);
            }
        }
    }
    // Colonies rise in overlapping shelves around the nursery. Their exposed edges
    // carry fan and pipe corals; the central swimming lane stays open.
    for (u, v, base, fan) in [
        (11, 3, M::CoralPurple, M::OrganPipeCoral),
        (-12, 1, M::CoralBlue, M::CoralBush),
        (-7, 13, M::CoralRed, M::TableCoral),
        (9, 18, M::CoralPurple, M::StaghornCoral),
        (-12, -11, M::CoralYellow, M::BrainCoral),
        (7, -14, M::CoralBlue, M::LuminousCoral),
    ] {
        for (du, dv, height) in [
            (0, 0, 3),
            (1, 0, 2),
            (-1, 0, 2),
            (0, 1, 2),
            (-1, -1, 1),
            (2, 1, 1),
            (1, -2, 1),
        ] {
            for h in 0..height {
                let m = if h == height - 1 { base } else { M::MossyStone };
                b.set(at(u + du, h, v + dv), m as u16);
            }
            b.set(
                at(u + du, height, v + dv),
                if height == 2 { M::TableCoral } else { fan } as u16,
            );
        }
        b.set(at(u - 2, 0, v + 1), M::TideGeode as u16);
    }
    for side in [-1, 1] {
        for (v, height) in [(-11, 5), (-7, 6), (-3, 4)] {
            for h in 0..height {
                b.set(at(side * (11 - h / 2), h, v), M::SaltStone as u16);
            }
        }
        b.set(at(side * 9, 0, -8), M::TideGeode as u16);
    }
    for (u, v) in [(-8, -9), (8, -7)] {
        b.set(at(u, 0, v), M::WaterDragonEgg as u16);
    }
}
