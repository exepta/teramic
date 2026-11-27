use super::*;

pub(super) fn build(f: &mut Frame<'_>) {
    f.approach(4, 2, -4);
    // The treasury is cut into a ridge. Distinct mineral strata crop out
    // above its arch and continue along both sides of the vaulted chamber.
    for z in -4_i64..=15 {
        for x in -13_i64..=13 {
            let side = x.abs();
            if z < 1 && side > 9 {
                continue;
            }
            if z < 1 {
                f.terrace(
                    x,
                    z,
                    2,
                    if (x + z).rem_euclid(5) == 0 {
                        M::CrystalAshlar
                    } else {
                        M::PolishedHighlandRock
                    },
                );
            }
            if z >= 1 {
                // Leave the undisturbed roof, flanks and rear as real terrain.
                // The very short porch is the only piece permitted at daylight.
                if side <= 6 && z <= 13 {
                    let ceiling = 10 - (side - 2).max(0) / 2;
                    f.column(x, z, f.floor(x, z).min(1) - 1, 1, M::HighlandCliffStone);
                    f.put(
                        x,
                        2,
                        z,
                        if x.abs() == 4 || (z == 4 || z == 11) && x.abs() < 4 {
                            M::GildedRunestone
                        } else {
                            M::PolishedHighlandRock
                        },
                    );
                    f.column(x, z, 3, ceiling - 1, M::Air);
                    f.put(x, ceiling, z, M::CrystalAshlar);
                }
            }
        }
    }
    // Two square pilasters, each crowned by several facets, frame a recessed
    // gold and amethyst archivolt. The central opening remains five blocks wide.
    for x in [-6, 6] {
        for z in [-1, 0] {
            f.put(x, 3, z, M::AmethystCapital);
            f.put(x, 4, z, M::CrystalAshlar);
            f.column(x, z, 5, 8, M::CrystalAshlar);
            f.put(x, 9, z, M::AmethystCapital);
            f.put(x, 10, z, M::CrystalCrown);
        }
        f.put(x, 3, -2, M::CrystalLantern);
        f.oriented(x, 8, -2, M::CrystalPennant, 0);
    }
    for x in -5_i64..=5 {
        let arch = 10 - (x.abs() - 1).max(0);
        for z in [0, 1] {
            if x.abs() >= 4 {
                f.column(x, z, 3, arch - 1, M::CrystalAshlar);
            }
            f.put(
                x,
                arch,
                z,
                if x.abs() == 3 || x == 0 {
                    M::GildedRunestone
                } else {
                    M::CrystalAshlar
                },
            );
            f.put(x, arch + 1, z, M::CrystalAshlar);
            f.put(x, arch + 2, z, M::PolishedHighlandRock);
        }
    }
    f.put(0, 10, -1, M::VaultKeystone);
    f.oriented(0, 11, -1, M::VaultSigil, 0);
    for z in [4, 8, 12] {
        for x in [-5, 5] {
            f.put(x, 3, z, M::AmethystCapital);
            f.column(x, z, 4, 6, M::CrystalAshlar);
            f.put(x, 7, z, M::AmethystCapital);
            if z == 4 {
                f.put(x - x.signum(), 6, z, M::CrystalLantern);
            }
        }
    }
    for x in -2..=2 {
        for z in 9..=12 {
            f.put(x, 3, z, M::CrystalAshlar);
            if x == 0 || z == 11 {
                f.put(x, 4, z, M::GildedRunestone);
            }
        }
    }
    // A double-terminated crystal in a gold armillary, a fan-shaped votive
    // and a runic pillar each illuminate their own side of the sanctuary.
    f.put(0, 5, 11, M::AltarCrystalFoot);
    f.put(0, 6, 11, M::AltarCrystalHeart);
    f.put(0, 7, 11, M::AltarCrystalCrown);
    f.put(-2, 5, 11, M::AmethystCapital);
    f.put(-2, 6, 11, M::TempleCrystalCluster);
    f.prism(2, 5, 11, 3);
    f.oriented(0, 4, 10, M::VaultSigil, 0);
    for x in [-8, 8] {
        f.terrace(x, -3, 2, M::CrystalAshlar);
        f.prism(x, 3, -3, if x < 0 { 3 } else { 4 });
    }
    f.chest(-3, 3, 10, M::AuriciteOre, 5);
    f.chest(3, 3, 10, M::PolishedHighlandRock, 6);
    f.put(-7, 3, -3, M::AlpineCrystalBloom);
}
