//! Surface blood shrines share the ruin planner, foundations, loot and tree clearance.
use super::*;
use terra_voxel::MaterialId as M;

pub(super) fn build(b: &mut Builder<'_>, x: i64, y: i64, z: i64) {
    let seed = i64::from(b.seed);
    let chapel = seed & 1 == 0;
    // A raised apse and broad approach connect the monument to the natural grade.
    for dx in -8_i64..=8 {
        let px = x + dx;
        let entry_top = b.ground(px, z - 10) - 1;
        let approach = |offset: i64| {
            if offset < -5 {
                entry_top + ((y - 1 - entry_top) * (offset + 10).clamp(0, 5)).div_euclid(5)
            } else {
                y - 1 + (offset + 5).clamp(0, 3)
            }
        };
        for dz in -10..=8 {
            let pz = z + dz;
            let aisle = dx.abs() <= 2;
            let platform = dx.abs() <= 6 && dz >= -4;
            if !platform && !aisle {
                continue;
            }
            let ground = b.ground(px, pz);
            let top = approach(dz);
            let stone = match (dx * 13 + dz * 31 + seed).rem_euclid(7) {
                0 => M::MossGraveStone,
                1 | 2 => M::GraveStone,
                _ => M::BloodstainedBricks,
            };
            b.fill([px, ground.min(top) - 1, pz], [px, top, pz], stone as u16);
            b.fill([px, top + 1, pz], [px, ground.max(top + 3), pz], 0);
            if top > approach(dz - 1) {
                b.state([px, top, pz], M::StoneStairs as u16, 0);
            } else if top > approach(dz + 1) {
                b.state([px, top, pz], M::StoneStairs as u16, 2);
            }
        }
    }
    let base = y + 3;
    let put = |b: &mut Builder<'_>, dx: i64, dy: i64, dz: i64, m: M| {
        b.set([x + dx, base + dy, z + dz], m as u16)
    };
    // Arches are full-scale masonry; the central opening is five blocks wide.
    for dz in [0, 5] {
        for side in [-1, 1] {
            let px = x + side * 4;
            b.fill(
                [px - 1, base, z + dz - 1],
                [px + 1, base, z + dz + 1],
                M::GraveStone as u16,
            );
            for h in 1..=6 {
                for depth in 0..=1 {
                    put(
                        b,
                        side * 4,
                        h,
                        dz + depth,
                        if h == 1 || h == 6 {
                            M::CarvedBloodStone
                        } else {
                            M::BloodstainedBricks
                        },
                    );
                }
            }
            b.fill(
                [px - 1, base + 6, z + dz],
                [px + 1, base + 6, z + dz + 1],
                M::CarvedBloodStone as u16,
            );
            for (offset, h) in [(3, 7), (2, 8), (1, 9), (0, 9)] {
                b.fill(
                    [x + offset * side, base + h, z + dz],
                    [x + offset * side, base + h + 1, z + dz + 1],
                    if offset == 2 {
                        M::CarvedBloodStone
                    } else {
                        M::GraveStone
                    } as u16,
                );
            }
            put(b, side * 4, 6, dz - 1, M::BloodwoodLog);
            put(b, side * 4, 5, dz - 1, M::TornBloodBanner);
            put(b, side * 4, 4, dz - 1, M::HangingBloodVines);
            put(b, side * 5, 0, dz - 2, M::BloodBrazier);
            put(b, side * 3, 1, dz - 1, M::CryptCandles);
        }
        put(b, 0, 9, dz, M::CarvedBloodStone);
        put(b, 0, 8, dz, M::HangingBloodLantern);
    }
    if chapel {
        for dx in [-5_i64, 5] {
            for dz in 1..=5 {
                put(b, dx, 6, dz, M::BloodwoodLog);
                if (dx + dz + seed).rem_euclid(4) > 0 {
                    put(b, dx, 7, dz, M::BloodwoodPlanks);
                }
            }
        }
        for dx in -4_i64..=4 {
            put(b, dx, 0, 7, M::BloodstainedBricks);
            put(b, dx, 1, 7, M::BloodstainedBricks);
            let h = 5 - dx.abs() / 2;
            for i in 2..=h {
                if dx.abs() >= 3 || i == h {
                    put(b, dx, i, 7, M::GraveStone);
                }
            }
        }
    } else {
        for side in [-1, 1] {
            for dz in 1..=4 {
                put(b, side * 6, 0, dz, M::OvergrownGraveStone);
                if dz != 2 {
                    put(b, side * 6, 1, dz, M::HangingBloodVines);
                }
            }
        }
    }
    for dx in -2..=2 {
        put(b, dx, 0, 5, M::CarvedBloodStone);
    }
    for (dx, dz, m) in [
        (-1, 5, M::CryptCandles),
        (1, 5, M::BloodCrystalGrowth),
        (0, 5, M::GraveCross),
    ] {
        put(b, dx, 1, dz, m);
    }
    let memorials = [
        (-7, -7, M::GraveCross),
        (6, -8, M::GravestoneSmall),
        (-6, -5, M::WeatheredRibcage),
        (8, -2, M::BoneStake),
        (7, 2, M::GravestoneLarge),
        (-7, 5, M::BloodBones),
        (4, -7, M::StandingBloodLantern),
        (-4, -8, M::StandingBloodLantern),
    ];
    for (dx, dz, m) in memorials {
        let py = b.ground(x + dx, z + dz);
        b.set([x + dx, py, z + dz], m as u16);
    }
    // Leave room for the actual silhouettes of the plants and memorials.
    // Sampling across the flanks also breaks up the two rectangular beds at the stairs.
    let mut occupied = memorials.map(|(dx, dz, _)| (dx, dz)).to_vec();
    let mut soil = Vec::new();
    for dx in -8_i64..=8 {
        for dz in -10_i64..=7 {
            if dx.abs() <= 2 || (dz >= -4 && dx.abs() <= 6) {
                continue;
            }
            let priority = layer_seed(b.seed.into(), ((dx + 8) * 18 + dz + 10) as u64);
            soil.push((priority, dx, dz));
        }
    }
    soil.sort_unstable();
    for (priority, dx, dz) in soil {
        if occupied
            .iter()
            .any(|&(px, pz)| (dx - px).pow(2) + (dz - pz).pow(2) <= 5)
        {
            continue;
        }
        let material = match priority % 8 {
            0 | 1 => M::BloodThornBush,
            2 | 3 => M::BloodFlower,
            4 => M::BloodMushrooms,
            5 => M::BloodRoots,
            _ => M::BloodFern,
        };
        let py = b.ground(x + dx, z + dz);
        b.set([x + dx, py, z + dz], material as u16);
        occupied.push((dx, dz));
    }
    for side in [-1, 1] {
        for dz in [0, 2, 4] {
            put(b, side * 6, 0, dz, M::MossGraveStone);
            put(
                b,
                side * 6,
                1,
                dz,
                if dz == 2 {
                    M::BloodThornBush
                } else {
                    M::BloodFlower
                },
            );
        }
    }
    let chest = [x + 2, base, z + 3];
    // A stain of old pitch has settled flush with the stony cemetery path.
    for [dx, dz] in [[-4, -6], [-5, -6], [-5, -7], [-6, -7], [-4, -7]] {
        let py = b.ground(x + dx, z + dz) - 1;
        b.set([x + dx, py, z + dz], M::DriedTar as u16);
    }
    b.object(
        chest,
        29,
        Some(BlockObjectState {
            facing: 2,
            ..Default::default()
        }),
        Some(loot::inventory(BuildingRole::Ruin, b.seed)),
    );
    b.summary.containers.push(chest);
    b.summary.buildings.push(BuildingSummary {
        role: BuildingRole::Ruin,
        entrance: [x, b.ground(x, z - 10), z - 10],
        bounds: [[x - 8, y - 5, z - 10], [x + 8, base + 10, z + 8]],
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use terra_core::{Config, WorldConfig};

    #[test]
    fn natural_blood_sanctuaries_have_walkable_aisles_and_attached_lanterns() {
        let config =
            WorldConfig::from_toml(include_str!("../../../../../../configs/world.toml")).unwrap();
        let terrain = TerraGenerator::new(config).unwrap();
        let voxels = crate::SectionGenerator::new(terrain.clone());
        for (region, [x, base, z], chapel) in [
            ([1, -29], [575, 71, -10944], false),
            ([-27, -21], [-10170, 76, -7872], true),
        ] {
            let site = terrain.structure_site(region).unwrap();
            assert_eq!(site.kind, StructureKind::Ruin);
            assert_eq!(site.containers, [[x + 2, base, z + 3]]);
            assert_eq!(voxels.block_at(site.containers[0]).unwrap(), 29);
            for (dx, dy, dz, material) in [
                (0, 8, 0, M::HangingBloodLantern),
                (0, 9, 0, M::CarvedBloodStone),
                (4, 5, -1, M::TornBloodBanner),
                (4, 6, -1, M::BloodwoodLog),
                (5, 0, -2, M::BloodBrazier),
                (1, 1, 5, M::BloodCrystalGrowth),
                (0, -2, -3, M::StoneStairs),
                (0, -1, -2, M::StoneStairs),
            ] {
                assert_eq!(
                    voxels.block_at([x + dx, base + dy, z + dz]).unwrap(),
                    material as u16
                );
            }
            assert_eq!(
                voxels.block_at([x - 5, base + 6, z + 3]).unwrap() == M::BloodwoodLog as u16,
                chapel
            );
            let mut growing = Vec::new();
            for dx in -8_i64..=8 {
                for dz in -10_i64..=7 {
                    if dx.abs() <= 2 || (dz >= -4 && dx.abs() <= 6) {
                        continue;
                    }
                    let py = (terrain.surface_height((x + dx) as f64 + 0.5, (z + dz) as f64 + 0.5)
                        - 0.5)
                        .floor() as i64
                        + 1;
                    let material = voxels.block_at([x + dx, py, z + dz]).unwrap();
                    if matches!(
                        M::from_raw(material),
                        Some(
                            M::BloodFlower
                                | M::BloodThornBush
                                | M::BloodMushrooms
                                | M::BloodRoots
                                | M::BloodFern
                        )
                    ) {
                        assert!(
                            growing
                                .iter()
                                .all(|&(px, pz): &(i64, i64)| (px - dx).pow(2) + (pz - dz).pow(2)
                                    > 5)
                        );
                        growing.push((dx, dz));
                    }
                }
            }
            assert!(
                growing.len() >= 4,
                "the approach should still have vegetation"
            );
            for dz in -2..=3 {
                for dy in 0..=2 {
                    assert_eq!(
                        voxels.block_at([x, base + dy, z + dz]).unwrap(),
                        0,
                        "blocked aisle z={dz} y={dy}"
                    );
                }
                assert_ne!(voxels.block_at([x, base - 1, z + dz]).unwrap(), 0);
            }
        }
    }
}
