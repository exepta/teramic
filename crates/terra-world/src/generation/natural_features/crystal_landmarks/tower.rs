use super::*;

// Tower coordinates use an eight-sided drum, with its centre at local (0, 3).
fn drum(x: i64, z: i64, r: i64) -> bool {
    x.abs() <= r && z.abs() <= r && x.abs() + z.abs() <= r + (r + 1) / 2
}

pub(super) fn build(f: &mut Frame<'_>) {
    f.approach(4, 2, -4);
    for z in -4..=11 {
        for x in -7_i64..=12 {
            if x.abs() > 5 && !(x >= 4 && (-1..=9).contains(&z)) {
                continue;
            }
            if x < 4 && !drum(x, z - 3, 7) {
                continue;
            }
            f.terrace(x, z, 2, M::CrystalAshlar);
        }
    }
    for y in 3..=17 {
        let ring = matches!(y, 3 | 4 | 11 | 16 | 17);
        let r = if ring { 5 } else { 4 };
        for dz in -r..=r {
            for x in -r..=r {
                if !drum(x, dz, r) {
                    continue;
                }
                let z = 3 + dz;
                let shell = !drum(x, dz, r - 1);
                if !shell {
                    if y <= 5 && !drum(x, dz, 3) {
                        f.put(x, y, z, M::CrystalAshlar);
                    }
                    continue;
                }
                let doorway = dz < 0 && x.abs() <= 1 && y <= 6;
                let tall_window = (7..=9).contains(&y) && (x == 0 || dz == 0);
                let upper_window = (13..=15).contains(&y) && (x.abs() == 1 || dz.abs() == 1);
                let mat = if doorway {
                    M::Air
                } else if tall_window || upper_window {
                    M::Glass
                } else if y == 11 {
                    M::PolishedHighlandRock
                } else if y == 16 && (x + dz).rem_euclid(5) == 0 {
                    M::GildedRunestone
                } else {
                    M::CrystalAshlar
                };
                f.put(x, y, z, mat);
            }
        }
    }
    for z in -3..=1 {
        for x in -1..=1 {
            f.column(x, z, 3, 6, M::Air);
        }
    }
    // Through the entrance, a clockwise stair climbs the inside wall. The last
    // tread breaks through the gallery floor; the telescope remains in the middle.
    let mut steps = Vec::new();
    for z in 1..=6 {
        steps.push((-2, z, 0));
    }
    for x in -1..=2 {
        steps.push((x, 6, 1));
    }
    for z in (0..=5).rev() {
        steps.push((2, z, 2));
    }
    for (i, (x, z, face)) in steps.into_iter().enumerate() {
        let y = 3 + i as i64;
        f.put(x, y - 1, z, M::PolishedHighlandRock);
        f.oriented(x, y, z, M::StoneStairs, face);
    }
    for dz in -5..=5 {
        for x in -5..=5 {
            if !drum(x, dz, 5) {
                continue;
            }
            let z = 3 + dz;
            if x != 2 || !(0..=2).contains(&z) {
                f.put(x, 18, z, M::CrystalAshlar);
            }
            if !drum(x, dz, 4) {
                f.put(x, 19, z, M::StoneFence);
            }
        }
    }
    for (x, dz) in [
        (-3, -3),
        (3, -3),
        (-4, 0),
        (4, 0),
        (-3, 3),
        (3, 3),
        (0, 4),
        (0, -4),
    ] {
        let z = 3 + dz;
        f.put(x, 19, z, M::AmethystCapital);
        f.column(x, z, 20, 22, M::CrystalAshlar);
        f.put(x, 23, z, M::AmethystCapital);
    }
    f.put(0, 19, 3, M::CrystalObservatoryLens);
    f.put(0, 19, 6, M::CrystalLantern);
    for y in 24..=30 {
        let r = match y {
            24 => 6,
            25 => 5,
            26 => 4,
            27 => 3,
            28 => 2,
            _ => 1,
        };
        for dz in -r..=r {
            for x in -r..=r {
                if drum(x, dz, r) && (y == 24 || !drum(x, dz, r - 1)) {
                    f.put(x, y, 3 + dz, M::VioletRoofSlate);
                }
            }
        }
    }
    f.put(0, 31, 3, M::GildedRunestone);
    f.put(0, 32, 3, M::CrystalObservatoryLens);
    // A lit study has a separate gabled roof and a through door into the drum.
    for z in 0..=8 {
        for x in 4..=10 {
            for y in 3..=7 {
                if x == 4 && (2..=4).contains(&z) && y <= 5 {
                    f.put(x, y, z, M::Air);
                    continue;
                }
                if x != 10 && z != 0 && z != 8 {
                    continue;
                }
                let window = ((x == 6 || x == 8) && z == 0 || x == 10 && (z == 2 || z == 5))
                    && (4..=6).contains(&y);
                f.put(x, y, z, if window { M::Glass } else { M::CrystalAshlar });
            }
        }
    }
    for x in 3..=5 {
        f.column(x, 3, 3, 5, M::Air);
    }
    for z in -1_i64..=9 {
        for x in 4..=11 {
            let top = 8 + (4 - (z - 4).abs()).max(0);
            if x == 10 {
                f.column(x, z, 8, top - 1, M::CrystalAshlar);
            }
            f.put(x, top, z, M::VioletRoofSlate);
        }
    }
    f.chest(8, 3, 6, M::GildedRunestone, 2);
    f.put(8, 3, 4, M::SprucePlanks);
    f.put(8, 4, 4, M::CrystalObservatoryLens);
    f.put(7, 5, 7, M::CrystalLantern);
    for (x, y) in [(-3, 6), (3, 6), (-2, 13), (2, 13)] {
        f.oriented(x, y, -1, M::CrystalPennant, 0);
    }
    // A projecting pointed porch gives the door depth even against a cliff.
    for x in [-2_i64, 2] {
        f.put(x, 3, -3, M::AmethystCapital);
        f.column(x, -3, 4, 5, M::CrystalAshlar);
        f.put(x, 6, -3, M::AmethystCapital);
    }
    for x in -2_i64..=2 {
        let y = 8 - x.abs();
        f.put(x, y + 1, -3, M::GildedRunestone);
        f.put(x, y + 2, -3, M::VioletRoofSlate);
    }
    f.oriented(0, 8, -3, M::VaultSigil, 0);
    f.prism(-4, 3, -3, 3);
    f.put(-5, 3, -3, M::FrostedMoss);
    f.put(0, 12, -2, M::VaultSigil);
    f.put(-5, 3, -2, M::AlpineCrystalBloom);
}
