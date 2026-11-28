use super::*;
use terra_core::rails::Piece;

const TRACKS: [(i64, i64, M, u8); 6] = [
    (0, -9, M::RailStraight, 0),
    (0, -5, M::RailSwitchRight, 0),
    (0, 3, M::RailStraight, 0),
    (0, 7, M::RailStraight, 0),
    (0, 11, M::RailBuffer, 0),
    (5, -1, M::RailStraight, 1),
];

pub(super) fn build(f: &mut Frame<'_>, fit_ground: bool) {
    // Leave the tramway level. At the downhill end the landing becomes a
    // trestle on timber piles, with a pedestrian staircase alongside the track.
    for z in -10..=14 {
        let width = match z {
            i64::MIN..=-3 if fit_ground => 1,
            -2..=-1 if fit_ground => 3,
            ..=-1 => 7,
            _ => 4,
        };
        for x in -width..=width {
            if z >= 0 {
                f.column(x, z, f.floor(x, z).min(0) - 1, 0, M::HighlandCliffStone);
                f.column(x, z, 1, 5, M::Air);
            } else if fit_ground {
                deck(f, x, z, 3, x.abs() == width && z % 3 == 0);
            } else if f.floor(x, z) < -1 {
                f.column(x, z, 0, 0, M::SprucePlanks);
                f.column(x, z, 1, 6, M::Air);
                if x.abs() == width && z % 3 == 0 {
                    f.column(x, z, f.floor(x, z), -1, M::SpruceLog);
                }
            } else {
                f.terrace(
                    x,
                    z,
                    0,
                    if x.abs() < 2 {
                        M::CrystalGravel
                    } else {
                        M::HighlandCliffStone
                    },
                );
            }
        }
        if z < -2 && z % 2 == 0 {
            for x in [-width, width] {
                if !fit_ground || f.floor(x, z) < 0 {
                    f.put(x, 1, z, M::SpruceFence);
                }
            }
        }
    }
    f.approach(if fit_ground { 1 } else { 2 }, 0, -11);
    // The adit has a blasted crown, two ore seams and a working side drift.
    // Its incline and back are native rock; only exposed excavation faces
    // receive fresh mineral staining, while the mountain above is untouched.
    for z in 0..=14 {
        for x in -3i64..=3 {
            let ceiling = if x.abs() == 3 {
                5
            } else {
                6 + ((z + 2) / 4) % 2
            };
            f.column(x, z, 1, ceiling - 1, M::Air);
            f.put(
                x,
                ceiling,
                z,
                if z % 5 == 2 {
                    M::CrystalStone
                } else {
                    M::HighlandCliffStone
                },
            );
        }
        for side in [-1, 1] {
            for y in 1..=4 {
                let seam = (z + side * 2).rem_euclid(8);
                f.put(
                    side * 4,
                    y,
                    z,
                    if y == 1 + seam / 3 {
                        if side < 0 {
                            M::CrystalOre
                        } else {
                            M::AuriciteOre
                        }
                    } else {
                        M::HighlandCliffStone
                    },
                );
            }
        }
    }
    // The side working sits beneath verified cover and opens off the haulage
    // gallery. Short posts support the ore pocket and its sample bench.
    if (-10..=-4).all(|x| (5..=9).all(|z| f.floor(x, z) > 7)) {
        for x in -10..=-3 {
            for z in 5..=9 {
                f.put(x, 0, z, M::CrystalGravel);
                f.column(x, z, 1, if z == 5 || z == 9 { 3 } else { 4 }, M::Air);
            }
        }
        for x in [-9, -6] {
            for z in [5, 9] {
                f.column(x, z, 1, 3, M::SpruceLog);
            }
            for z in 5..=9 {
                f.put(x, 4, z, M::WeatheredWood);
            }
        }
        f.put(-10, 2, 7, M::MixedCrystalVein);
        f.put(-9, 1, 8, M::ExcavationCrate);
        f.put(-9, 2, 8, M::MineToolRack);
        f.put(-6, 3, 6, M::CrystalLantern);
    }
    if !fit_ground {
        for x in 3..=9 {
            for z in -2..=0 {
                f.terrace(x, z, 0, M::CrystalGravel);
            }
        }
    } else {
        for (x, z, mat, yaw) in TRACKS.into_iter().filter(|&(_, z, _, _)| z < 0) {
            let local = Piece::new([x, 1, z], mat as u16, yaw).unwrap();
            for [x, y, z] in crate::rails::cells(local) {
                if z < 0 && y == 1 {
                    deck(f, x, z, 3, z % 3 == 0);
                }
            }
        }
        for (x, z, clearance) in [
            (-6, -8, 4),
            (6, -3, 4),
            (7, -3, 2),
            (-5, -3, 2),
            (-5, -4, 2),
            (-5, -6, 2),
        ] {
            deck(f, x, z, clearance, true);
        }
        for z in -7..=-4 {
            for x in 4..=7 {
                if x == 4 || x == 7 {
                    deck(f, x, z, 4, true);
                } else {
                    f.column(x, z, 2, 4, M::Air);
                }
            }
        }
    }
    for z in [0, 4, 8, 12] {
        for x in [-3, 3] {
            f.put(x, 1, z, M::CrystalAshlar);
            f.column(x, z, 2, 4, M::SpruceLog);
            f.put(x - x.signum(), 4, z, M::SprucePlanks);
        }
        for x in -4..=4 {
            f.put(x, 5, z, M::WeatheredWood);
        }
        f.put(-2, 4, z, M::CrystalLantern);
    }
    for x in [-3, 3] {
        f.put(x, 4, -1, M::AmethystCapital);
        f.oriented(x, 3, -1, M::CrystalPennant, 0);
    }
    for (x, z) in [(-6, -8), (6, -3)] {
        f.column(x, z, 1, 3, M::SpruceFence);
        f.put(x, 4, z, M::CrystalLantern);
    }
    // These are genuine track modules. Each owned voxel slice acquires its
    // persistent rail anchor so the player can continue or dismantle the line.
    for (x, z, material, yaw) in TRACKS {
        rail(f, x, z, material, yaw);
    }
    f.put(7, 1, -3, M::CrystalMineCart);
    f.put(-5, 1, -3, M::ExcavationCrate);
    f.put(-5, 2, -3, M::MineToolRack);
    f.chest(-5, 1, -4, M::AuriciteOre, 3);
    f.put(2, 1, 11, M::MixedCrystalVein);
    // Sorting shelves, pegged posts and paired sample bins beside the siding.
    for z in -7..=-4 {
        for x in [4, 7] {
            f.put(x, 1, z, M::SpruceLog);
        }
        for x in 4..=7 {
            f.put(x, 2, z, M::WeatheredWood);
        }
    }
    for (x, z, ore) in [
        (4, -5, M::AuriciteDeposit),
        (6, -6, M::MixedCrystalVein),
        (7, -4, M::CrystalShardCluster),
    ] {
        f.put(x, 3, z, ore);
    }
    f.put(-2, 1, 13, M::CrystalCrown);
    f.put(3, 2, 10, M::MineToolRack);
    f.put(-5, 1, -6, M::ExcavationCrate);
    f.put(-5, 2, -6, M::CrystalLantern);
}

fn deck(f: &mut Frame<'_>, x: i64, z: i64, clearance: i64, post: bool) {
    let ground = f.floor(x, z);
    if ground < -1 {
        f.put(x, 0, z, M::SprucePlanks);
        if post {
            f.column(x, z, ground + 1, -1, M::SpruceLog);
        }
    } else {
        f.put(x, 0, z, M::CrystalGravel);
    }
    // Keep the native slope above the mouth. Only clear the usable height.
    f.column(x, z, 1, clearance, M::Air);
}

fn rail(f: &mut Frame<'_>, x: i64, z: i64, mat: M, local_yaw: u8) {
    let piece = Piece::new(
        f.site.world(x, 1, z),
        mat as u16,
        (local_yaw + 4 - f.site.turn) % 4,
    )
    .unwrap();
    for cell in crate::rails::cells(piece) {
        f.blocks.insert(cell, mat as u16);
        f.objects.push(StructureObject {
            position: cell,
            material: mat as u16,
            state: Some(crate::rails::object_state(piece, cell)),
            loot: None,
            crop: None,
        });
    }
}
