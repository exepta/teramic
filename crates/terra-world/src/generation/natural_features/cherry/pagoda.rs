//! A small hillside sanctuary. A district selects at most one site; all
//! footprint, tree and foundation checks finish before a single voxel is placed.
use super::{Cell, Habitat, M, StructureObject, Tile, register_pose, ticket};
use std::collections::BTreeMap;

fn candidate(seed: u64, district: Tile) -> (u32, Tile) {
    let priority = ticket(seed, district, 7851);
    (
        priority,
        [
            district[0] * 4 + i64::from(priority % 4),
            district[1] * 4 + i64::from((priority >> 4) % 4),
        ],
    )
}

pub(super) fn chosen(seed: u64, tile: Tile) -> bool {
    let district = tile.map(|n| n.div_euclid(4));
    let own = candidate(seed, district);
    if own.1 != tile {
        return false;
    }
    // Suppress abutting district winners closer than four tiles, including
    // across negative-coordinate boundaries. The rank tie-break is stable.
    for dz in -1..=1 {
        for dx in -1..=1 {
            let other = candidate(seed, [district[0] + dx, district[1] + dz]);
            if other < own && (other.1[0] - tile[0]).pow(2) + (other.1[1] - tile[1]).pow(2) < 16 {
                return false;
            }
        }
    }
    true
}

fn world(center: Tile, x: i64, z: i64, facing: usize) -> Tile {
    let [dx, dz] = [[x, z], [-z, x], [-x, -z], [z, -x]][facing];
    [center[0] + dx, center[1] + dz]
}

const STAIRS: [(i64, i64); 10] = [
    (2, 1),
    (2, 0),
    (2, -1),
    (1, -1),
    (0, -1),
    (0, 0),
    (0, 1),
    (-1, 1),
    (-2, 1),
    (-2, 0),
];

struct Sanctuary {
    // Local x/z coordinates with Y measured from the finished base floor.
    blocks: BTreeMap<Cell, M>,
}
impl Sanctuary {
    fn put(&mut self, x: i64, y: i64, z: i64, material: M) {
        self.blocks.insert([x, y, z], material);
    }
    fn roof(&mut self, inner: i64, outer: i64, eave: i64) {
        // Corner tails and the crown sit one step above the sweeping middle eaves.
        for z in -outer..=outer {
            for x in -outer..=outer {
                let ring = x.abs().max(z.abs());
                if ring < inner {
                    continue;
                }
                let corner = i64::from(x.abs().min(z.abs()) >= outer - 1);
                let rise = (outer - ring + 1) / 2;
                let y = eave + corner + rise;
                if ring == outer {
                    self.put(x, y - 1, z, M::CherryPlanks);
                }
                self.put(x, y, z, M::BlueSlateShingles);
            }
        }
    }
    fn new() -> Self {
        let mut s = Self {
            blocks: BTreeMap::new(),
        };
        for z in -4i64..=4 {
            for x in -4i64..=4 {
                s.put(
                    x,
                    0,
                    z,
                    if x.abs().max(z.abs()) == 4 {
                        M::BlossomStone
                    } else {
                        M::CherryPlanks
                    },
                );
                if x.abs().max(z.abs()) == 4 && !(z == -4 && x.abs() <= 1) {
                    s.put(x, 1, z, M::CherryFence);
                }
            }
        }
        for x in [-3, 3] {
            for z in [-3, 0, 3] {
                for y in 1..=4 {
                    s.put(x, y, z, M::CherryLog);
                }
            }
        }
        for x in -3i64..=3 {
            // A high sill and a recessed altar leave the front and sides open.
            s.put(x, 1, 3, M::CherryPlanks);
            s.put(x, 4, -3, M::CherryPlanks);
            s.put(x, 4, 3, M::CherryPlanks);
        }
        for z in -3..=3 {
            for x in [-3, 3] {
                s.put(x, 4, z, M::CherryPlanks);
            }
        }
        s.put(0, 2, 3, M::SakuraFoxStatue);
        for x in [-2, 2] {
            s.put(x, 3, -3, M::Lantern);
            s.put(x, 3, 3, M::SakuraSilkBanner);
        }
        s.roof(3, 6, 5);
        // The enclosed upper floor is five metres wide with a central stairwell.
        for z in -2..=2 {
            for x in -2..=2 {
                if !STAIRS[4..].contains(&(x, z)) {
                    s.put(x, 5, z, M::CherryPlanks);
                }
            }
        }
        for (i, (x, z)) in STAIRS.into_iter().enumerate() {
            s.put(
                x,
                1 + (i / 2) as i64,
                z,
                if i % 2 == 0 {
                    M::CherrySlab
                } else {
                    M::CherryPlanks
                },
            );
        }
        for x in [-2, 2] {
            for z in [-2, 2] {
                for y in 6..=9 {
                    s.put(x, y, z, M::CherryLog);
                }
            }
        }
        for i in -2..=2 {
            for edge in [-2, 2] {
                s.put(i, 9, edge, M::CherryPlanks);
                s.put(edge, 9, i, M::CherryPlanks);
            }
        }
        for i in -1..=1 {
            s.put(i, 6, 2, M::CherryFence);
            s.put(i, 6, -2, M::CherryFence);
        }
        for x in [-1, 1] {
            s.put(x, 8, -2, M::Lantern);
        }
        s.roof(1, 4, 10);
        for y in 10..=13 {
            s.put(0, y, 0, M::CherryLog);
        }
        s.roof(0, 2, 13);
        s.put(0, 15, 0, M::MossyBlossomStone);
        s.put(0, 16, 0, M::SakuraGardenLantern);
        s
    }
}

fn dry_bed(h: &mut Habitat<'_>, p: Tile) -> Option<i64> {
    let c = h.column(p);
    if c.water_level.is_some()
        || c.biome.weights.cherry_forest < 0.52
        || c.cave.open_to_sky()
        || c.cave_cavity.is_some()
        || c.river_cavity.is_some()
        || h.reserved(p, 2)
    {
        return None;
    }
    h.ground(p).map(|(bed, _)| bed)
}

pub(super) fn build(
    h: &mut Habitat<'_>,
    center: Tile,
    cells: &mut BTreeMap<Cell, u16>,
    objects: &mut Vec<StructureObject>,
) -> bool {
    // Test additional clearings inside the owner tile. The longest stair
    // extends ten metres, so each entire sanctuary stays within that tile.
    [[0, 0], [-4, -4], [4, 4], [-4, 4], [4, -4]]
        .into_iter()
        .any(|[dx, dz]| try_site(h, [center[0] + dx, center[1] + dz], cells, objects))
}

fn try_site(
    h: &mut Habitat<'_>,
    center: Tile,
    cells: &mut BTreeMap<Cell, u16>,
    objects: &mut Vec<StructureObject>,
) -> bool {
    let mut floor = i64::MIN;
    let mut low = i64::MAX;
    let mut beds = BTreeMap::new();
    for z in -6i64..=6 {
        for x in -6i64..=6 {
            let p = [center[0] + x, center[1] + z];
            let Some(bed) = dry_bed(h, p) else {
                return false;
            };
            beds.insert(p, bed);
            if x.abs().max(z.abs()) <= 4 {
                low = low.min(bed);
                floor = floor.max(bed);
                if h.root_distance_squared(p) <= 1 {
                    return false;
                }
            }
        }
    }
    if !(1..=4).contains(&(floor - low)) {
        return false;
    }
    let mut facings = Vec::new();
    for facing in 0..4 {
        let p = world(center, 0, -10, facing);
        if let Some(bed) = dry_bed(h, p)
            && (0..=3).contains(&(floor - bed))
        {
            facings.push((floor - bed, facing));
        }
    }
    facings.sort_by_key(|(rise, _)| std::cmp::Reverse(*rise));
    for (_, facing) in facings {
        let mut site = Sanctuary::new();
        let mut valid = true;
        for x in -1..=1 {
            let mut steps = Vec::new();
            for z in -10..=-4 {
                let p = world(center, x, z, facing);
                let Some(bed) = dry_bed(h, p) else {
                    valid = false;
                    break;
                };
                steps.push((z, bed));
            }
            if !valid {
                break;
            }
            let entrance = steps[0].1;
            if !(0..=3).contains(&(floor - entrance)) {
                valid = false;
                break;
            }
            for (z, bed) in steps {
                let top = (2 * (floor + 1)).min(2 * (entrance + 1) + z + 10);
                let tread = (top - 1).div_euclid(2);
                if bed > tread {
                    valid = false;
                    break;
                }
                for y in bed + 1..tread {
                    site.put(x, y - floor, z, M::MossyBlossomStone);
                }
                site.put(
                    x,
                    tread - floor,
                    z,
                    if top.rem_euclid(2) == 0 {
                        M::BlossomStone
                    } else {
                        M::CherrySlab
                    },
                );
            }
        }
        if !valid {
            continue;
        }
        for z in -4..=4 {
            for x in -4..=4 {
                let p = world(center, x, z, facing);
                for y in beds[&p] + 1..floor {
                    site.put(
                        x,
                        y - floor,
                        z,
                        if (x + z) % 3 == 0 {
                            M::MossyBlossomStone
                        } else {
                            M::BlossomStone
                        },
                    );
                }
            }
        }
        let occupied = |cell: Cell| {
            h.tree_occupied(cell)
                || cells
                    .get(&cell)
                    .is_some_and(|&id| M::from_raw(id).is_some_and(M::is_solid))
        };
        // Both storeys keep their two-block headroom, including the stairwell.
        if (-4..=4).any(|z| {
            (-4..=4).any(|x| {
                let p = world(center, x, z, facing);
                (floor + 1..=floor + 9).any(|y| occupied([p[0], y, p[1]]))
            })
        }) {
            continue;
        }
        let mut stamped = Vec::with_capacity(site.blocks.len());
        for ([x, y, z], material) in site.blocks {
            let p = world(center, x, z, facing);
            let pos = [p[0], floor + y, p[1]];
            if occupied(pos) {
                valid = false;
                break;
            }
            stamped.push((pos, material));
        }
        if valid {
            for (pos, material) in stamped {
                cells.insert(pos, material as u16);
                register_pose(objects, pos, material, (4 - facing as u8) % 4);
            }
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanctuary_staircase_has_headroom_and_all_three_roofs_turn_up_at_the_corners() {
        let s = Sanctuary::new();
        for (i, (x, z)) in STAIRS.into_iter().enumerate() {
            let y = (i / 2) as i64 + 1;
            assert_eq!(
                s.blocks.get(&[x, y, z]),
                Some(if i % 2 == 0 {
                    &M::CherrySlab
                } else {
                    &M::CherryPlanks
                })
            );
            for above in [1, 2] {
                assert!(
                    !s.blocks.contains_key(&[x, y + above, z]),
                    "stair headroom {i}"
                );
            }
        }
        for (radius, eave) in [(6, 5), (4, 10), (2, 13)] {
            assert_eq!(
                s.blocks.get(&[0, eave, radius]),
                Some(&M::BlueSlateShingles)
            );
            assert_eq!(
                s.blocks.get(&[radius, eave + 1, radius]),
                Some(&M::BlueSlateShingles)
            );
        }
        assert!(!s.blocks.contains_key(&[0, 1, -4]) && !s.blocks.contains_key(&[0, 2, -3]));
    }
}
