//! Limestone field structures, timber framing and sail mills in the Plains.
use super::{BlockObjectState, Builder, BuildingRole, BuildingSummary, loot};
use terra_voxel::MaterialId as M;

pub(super) fn active(b: &Builder<'_>, x: i64, z: i64) -> bool {
    b.terrain.config().ecology_revision >= 13
        && b.terrain.config().biomes
        && b.terrain
            .biome_at(x as f64 + 0.5, z as f64 + 0.5)
            .weights
            .plains
            > 0.53
}

pub(super) fn arcade(b: &mut Builder<'_>, x: i64, y: i64, z: i64) {
    let stone = |dx: i64, dy: i64, dz: i64| match (dx * 19 + dy * 7 + dz * 3).rem_euclid(11) {
        0..=1 => M::MossyStone,
        2..=4 => M::StructureStone,
        _ => M::MeadowLimestone,
    } as u16;
    // Broken cloister walls overlook a central through-path. Each arch springs from actual ribs.
    for dx in -7_i64..=7 {
        for dz in -6_i64..=6 {
            let px = x + dx;
            let pz = z + dz;
            let base = b.ground(px, pz);
            let aisle = dx.abs() <= 2;
            let outside = dx.abs() == 6 || dz.abs() == 4;
            if !outside && !aisle {
                continue;
            }
            b.fill(
                [px, base.min(y) - 2, pz],
                [px, y - 1, pz],
                if aisle && dz.abs() > 4 {
                    M::Path as u16
                } else {
                    stone(dx, 0, dz)
                },
            );
            b.fill([px, y, pz], [px, (base + 2).max(y + 4), pz], 0);
            if outside {
                // Side bays are seven metres across. The detached north-west pier is weathered.
                let across = if dx.abs() == 6 {
                    dz.abs()
                } else {
                    dx.abs() * 4 / 6
                };
                let cap = if dx.abs() == 6 { 6 } else { 8 };
                let height = if across >= 4 {
                    0
                } else {
                    cap - [0, 0, 1, 3][across as usize]
                };
                let top = if dz < 0 && dx < -1 {
                    (dx + 11).clamp(1, 6)
                } else {
                    cap + 1
                };
                if height <= top {
                    for dy in height..=top {
                        b.set([px, y + dy, pz], stone(dx, dy, dz));
                    }
                }
            }
        }
    }
    for [dx, dz] in [[-6, -4], [6, -4], [-6, 4], [6, 4]] {
        for u in -1_i64..=1 {
            for v in -1_i64..=1 {
                b.set([x + dx + u, y, z + dz + v], stone(dx + u, 0, dz + v));
            }
        }
    }
    // Existing ruin loot and central access remain available at the new landmark.
    let chest = [x + 3, y, z + 2];
    b.set([chest[0], y - 1, chest[2]], M::MeadowLimestone as u16);
    b.object(
        chest,
        M::Chest as u16,
        Some(BlockObjectState::default()),
        Some(loot::inventory(BuildingRole::Ruin, b.seed)),
    );
    b.summary.containers.push(chest);
    b.summary.buildings.push(BuildingSummary {
        role: BuildingRole::Ruin,
        entrance: [x, y, z - 6],
        bounds: [[x - 7, y - 2, z - 6], [x + 7, y + 9, z + 6]],
    });
}

/// Search just outside the field and all reserved plots. A mill may be absent on steep farms.
pub(super) fn mill_by_field(
    b: &mut Builder<'_>,
    farm: [i64; 2],
    overlaps: impl Fn([[i64; 2]; 2]) -> bool,
) {
    let [fx, fz] = farm;
    for [x, z] in [
        [fx + 6, fz + 17],
        [fx - 10, fz + 4],
        [fx + 23, fz + 4],
        [fx + 6, fz - 13],
    ] {
        let bounds = [[x - 8, z - 6], [x + 8, z + 5]];
        if overlaps(bounds) {
            continue;
        }
        let Some(y) = b.fit(x - 4, z - 4, 8, 8) else {
            continue;
        };
        mill(b, x, y, z);
        break;
    }
}

fn mill(b: &mut Builder<'_>, x: i64, y: i64, z: i64) {
    for dy in -1..=14 {
        let radius: i64 = match dy {
            -1..=3 => 4,
            4..=10 => 3,
            11..=12 => 4,
            13 => 3,
            _ => 1,
        };
        for dz in -radius..=radius {
            for dx in -radius..=radius {
                let edge = dx.abs().max(dz.abs());
                if dx.abs() + dz.abs() > radius * 2 - 2 && radius >= 3 {
                    continue;
                }
                let p = [x + dx, y + dy, z + dz];
                if dy == -1 {
                    b.fill(
                        [p[0], b.ground(p[0], p[2]).min(y) - 1, p[2]],
                        p,
                        M::MeadowLimestone as u16,
                    );
                    continue;
                }
                let shell = edge == radius || dx.abs() + dz.abs() >= radius * 2 - 2;
                let material = if dy >= 11 {
                    M::BlueSlateShingles
                } else if !shell || dz == -radius && dx == 0 && dy <= 2 {
                    M::Air
                } else if ((dx == radius || dx == -radius) && dz == 0 || dz == radius && dx == 0)
                    && (dy == 2 || dy == 8)
                {
                    M::Glass
                } else if dy == 4 || dy == 10 || dx.abs() == 2 && dz.abs() == radius {
                    M::Wood
                } else if dy <= 3 {
                    M::MeadowLimestone
                } else {
                    M::WhiteClay
                };
                b.set(p, material as u16);
            }
        }
    }
    b.fill([x, y + 8, z - 5], [x, y + 8, z - 3], M::Wood as u16);
    for (dx, dy) in [(1, 1), (-1, 1), (-1, -1), (1, -1)] {
        for t in 1_i64..=6 {
            let join = [x + t * dx, y + 8 + t * dy, z - 5];
            b.set(join, M::ParkTimber as u16);
            b.set([join[0] - dx, join[1], join[2]], M::ParkTimber as u16);
            if t >= 3 {
                let cloth = if t == 6 { M::BirchPlanks } else { M::Wool };
                b.set([join[0] - dy, join[1] + dx, join[2]], cloth as u16);
                b.set([join[0] - dy, join[1] + dx - dy, join[2]], cloth as u16);
            }
        }
    }
    for (dx, dz, height) in [(-4, 4, 2), (-3, 4, 1), (-4, 5, 1)] {
        let base = b.ground(x + dx, z + dz);
        b.fill(
            [x + dx, base, z + dz],
            [x + dx, base + height - 1, z + dz],
            M::BoundHayBale as u16,
        );
    }
}

/// A covered spring near the village paths, with a stone-lined basin a block deep.
pub(super) fn well(
    b: &mut Builder<'_>,
    center: [i64; 2],
    overlaps: impl Fn([[i64; 2]; 2]) -> bool,
) {
    for [u, v] in [[-8, -7], [8, 7], [8, -7], [-8, 7]] {
        let x = center[0] + u;
        let z = center[1] + v;
        if overlaps([[x - 3, z - 2], [x + 3, z + 2]]) {
            continue;
        }
        let Some(y) = b.fit(x - 2, z - 2, 4, 4) else {
            continue;
        };
        for dz in -1_i64..=1 {
            for dx in -1_i64..=1 {
                let p = [x + dx, y, z + dz];
                b.fill(
                    [p[0], (y - 2).min(b.ground(p[0], p[2]) - 1), p[2]],
                    [p[0], y - 1, p[2]],
                    M::MeadowLimestone as u16,
                );
                if dx != 0 || dz != 0 {
                    b.set(p, M::StructureStone as u16);
                } else if b.terrain.config().water {
                    b.object(p, M::FluidWater as u16, None, None);
                } else {
                    b.set(p, M::Air as u16);
                }
            }
        }
        for dx in [-1, 1] {
            b.fill([x + dx, y + 1, z], [x + dx, y + 3, z], M::Wood as u16);
        }
        b.fill([x - 1, y + 3, z], [x + 1, y + 3, z], M::Wood as u16);
        for dz in -1_i64..=1 {
            let y = y + 5 - dz.abs();
            b.fill(
                [x - 2, y, z + dz],
                [x + 2, y, z + dz],
                M::BlueSlateShingles as u16,
            );
        }
        break;
    }
}
