//! The gateway and well sanctuary share masonry, but have distinct silhouettes.
use super::{BlockObjectState, Builder, BuildingRole, BuildingSummary, loot};
use terra_voxel::MaterialId as M;

pub(super) fn ruin(b: &mut Builder<'_>, x: i64, y: i64, z: i64) {
    let shrine = b.seed & 1 == 0;
    for dx in -7_i64..=7 {
        for dz in -6_i64..=6 {
            let [px, pz] = [x + dx, z + dz];
            let base = b.ground(px, pz);
            let apron = dx.abs() <= 2 || (dx.abs() <= 5 && dz.abs() <= 4);
            if !apron {
                continue;
            }
            b.fill(
                [px, base.min(y - 1) - 1, pz],
                [px, y - 1, pz],
                M::Sandstone as u16,
            );
            b.fill([px, y, pz], [px, (base + 2).max(y + 7), pz], 0);
            if dx.abs() <= 1 && dz.abs() <= 4 {
                b.set([px, y - 1, pz], M::OasisMosaic as u16);
            }
            if dx.abs() == 5 && dz.abs() <= 3 && (dz + dx).rem_euclid(4) != 1 {
                b.set([px, y, pz], M::BandedSandstone as u16);
            }
        }
    }
    for dz in [-3_i64, 3] {
        for dx in [-3_i64, 3] {
            let top = if !shrine && dx == -3 && dz == 3 { 2 } else { 5 };
            b.fill(
                [x + dx, y, z + dz],
                [x + dx, y + top, z + dz],
                M::Sandstone as u16,
            );
            for dy in [0, top] {
                b.set([x + dx, y + dy, z + dz], M::CarvedSunstone as u16);
            }
        }
        if dz == -3 || shrine {
            for dx in -2_i64..=2 {
                let spring = 6 - dx.abs() / 2;
                b.set([x + dx, y + spring, z + dz], M::CarvedSunstone as u16);
                b.set([x + dx, y + spring + 1, z + dz], M::Sandstone as u16);
            }
            for dx in [-2_i64, 2] {
                b.set([x + dx, y + 4, z + dz], M::SunBanner as u16);
            }
        }
    }
    if shrine {
        for dx in [-3, 3] {
            b.fill(
                [x + dx, y + 5, z - 2],
                [x + dx, y + 5, z + 2],
                M::CarvedSunstone as u16,
            );
        }
        // Stepped, partly open cupola above the processional aisle.
        for dx in -2_i64..=2 {
            for dz in -2_i64..=2 {
                let ring = dx.abs().max(dz.abs());
                if ring > 0 {
                    b.set([x + dx, y + 8 - ring, z + dz], M::BandedSandstone as u16);
                }
            }
        }
        b.set([x, y, z], M::CarvedSunstone as u16);
        b.set([x, y + 1, z], M::PaintedAmphora as u16);
    }
    b.set([x + 4, y, z - 2], M::PaintedAmphora as u16);
    b.set([x - 4, y, z + 1], M::CavePebbles as u16);
    let chest = [x + 4, y, z + 2];
    b.object(
        chest,
        M::Chest as u16,
        Some(BlockObjectState {
            facing: 2,
            ..Default::default()
        }),
        Some(loot::inventory(BuildingRole::Ruin, b.seed)),
    );
    b.summary.containers.push(chest);
    b.summary.buildings.push(BuildingSummary {
        role: BuildingRole::Ruin,
        entrance: [x, y, z - 6],
        bounds: [[x - 7, y - 2, z - 6], [x + 7, y + 8, z + 6]],
    });
}
