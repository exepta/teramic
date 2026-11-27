//! Petrified trunks and their exposed roots occupy the high shoulders. Dry
//! shrubs recur along the foot of each shelf. Rare excavations expose the
//! fossil bed and a staircase descends from the ruins into a buried sanctuary.
use super::{Cell, Habitat, TILE_SIZE, Tile, relic_trees::TreeForm, ticket};
use crate::{edits::BlockObjectState, generation::structures::StructureObject};
use std::collections::BTreeMap;
use terra_voxel::MaterialId as M;

type Blocks = BTreeMap<Cell, u16>;

pub(super) fn build(
    habitat: &mut Habitat<'_>,
    tile: Tile,
    blocks: &mut Blocks,
    objects: &mut Vec<StructureObject>,
) {
    let seed = habitat.terrain.config().seed.0;
    let key = ticket(seed, tile, 9261);
    let center = tile.map(|n| n * TILE_SIZE + 16);
    let weight = habitat
        .terrain
        .climate_biome_at(center[0] as f64, center[1] as f64)
        .weights
        .petrified_forest;
    if weight < 0.48 {
        return;
    }
    let Some((ground, column)) = habitat.ground(center) else {
        return;
    };
    if column.biome.weights.petrified_forest < 0.52 || habitat.reserved(center, 14) {
        return;
    }
    // Neighbouring dig sites cannot share an excavation or stone staircase.
    let district = tile.map(|v| v.div_euclid(4));
    let site = ticket(seed, district, 9262);
    let landmark = tile[0].rem_euclid(4) == i64::from(site % 4)
        && tile[1].rem_euclid(4) == i64::from((site >> 4) % 4)
        && habitat.terrain.config().structures.ruins
        && habitat.terrain.config().structures_revision > 0;
    if landmark && column.slope < 0.7 {
        let frame = Frame {
            center,
            base: ground,
            turn: key % 4,
        };
        if let Some(survey) = survey(habitat, frame) {
            if site & 0x100 == 0 && habitat.terrain.config().caves {
                buried_temple(blocks, frame, &survey);
                if habitat.terrain.config().ecology_revision >= 20 {
                    temple_forecourt(habitat, blocks, frame, &survey, key, objects);
                }
            } else {
                excavation(habitat, blocks, frame, &survey, key);
            }
            return;
        }
    }
    let origin = [
        center[0] + i64::from((key >> 9) % 11) - 5,
        center[1] + i64::from((key >> 15) % 11) - 5,
    ];
    let mut root_flank = None;
    if key % 5 <= 2
        && let Some((base, col)) = habitat.ground(origin)
        && col.biome.weights.petrified_forest > 0.54
    {
        let radius = 2.3 + f64::from((key >> 4) % 28) * 0.1;
        let height = 4. + f64::from((key >> 16) % 11);
        root_flank = Some([origin[0], origin[1] - radius.ceil() as i64]);
        if habitat.terrain.config().ecology_revision >= 20 && !key.is_multiple_of(4) {
            TreeForm {
                root: [origin[0], base, origin[1]],
                height: height + 8.,
                radius: radius * 0.72,
                wood: M::PetrifiedLog,
                seed: key,
            }
            .grow(habitat, blocks);
        } else {
            trunk(blocks, origin, base, key, [radius, height], |point| {
                bedrock_level(habitat, point)
            });
        }
    }
    if habitat.terrain.config().ecology_revision >= 20 {
        // Fungal colonies follow the sheltered sides of fossil roots and shale shelves.
        // Each pocket shares its cap colour and leaves a corridor to the next one.
        for group in 0..2 {
            let h = ticket(seed, tile, 9331 + group);
            let pocket = [
                center[0] - 10 + i64::from(h % 19),
                center[1] - 10 + i64::from((h >> 8) % 19),
            ];
            let anchor = if group == 0 {
                root_flank.unwrap_or(pocket)
            } else {
                [
                    pocket,
                    [pocket[0] + 3, pocket[1]],
                    [pocket[0], pocket[1] - 3],
                ]
                .into_iter()
                .min_by_key(|&p| habitat.ground(p).map_or(i64::MAX, |(y, _)| y))
                .unwrap_or(pocket)
            };
            for offset in 0..10 {
                let h = ticket(seed ^ u64::from(h), tile, 9360 + offset);
                let p = [
                    anchor[0] + i64::from(h % 7) - 3,
                    anchor[1] + i64::from((h >> 8) % 4) - 2,
                ];
                let Some((y, col)) = habitat.ground(p) else {
                    continue;
                };
                if col.biome.weights.petrified_forest < 0.58 {
                    continue;
                }
                let m = match h % 8 {
                    0 => M::SmallDryShrub,
                    1..=2 => M::PetrifiedRubble,
                    _ => {
                        if group == 0 {
                            M::DarkMushrooms
                        } else if key.is_multiple_of(4) {
                            M::GlowingPurpleMushrooms
                        } else {
                            M::BloodMushrooms
                        }
                    }
                };
                blocks.entry([p[0], y + 1, p[1]]).or_insert(m as u16);
            }
        }
    }
    // Each deposit has its own centre, radial falloff and associated species.
    // Tiles between these groups remain mostly exposed dusty stone.
    let clusters = 1 + key % 3;
    for cluster in 0..clusters {
        let h = ticket(seed, tile, 9270 + u64::from(cluster));
        let anchor = [
            center[0] - 10 + i64::from(h % 21),
            center[1] - 10 + i64::from((h >> 8) % 21),
        ];
        let family = (h >> 16) % 5;
        for i in 0..(3 + (h >> 20) % 7) {
            let h = ticket(seed ^ u64::from(h), tile, 9290 + u64::from(i));
            let p = [
                anchor[0] + i64::from(h % 9) - 4,
                anchor[1] + i64::from((h >> 8) % 9) - 4,
            ];
            let Some((base, col)) = habitat.ground(p) else {
                continue;
            };
            if col.biome.weights.petrified_forest < 0.5 {
                continue;
            }
            let offset = [p[0], base + 1, p[1]];
            let prop = match (family, i, h % 4) {
                (0, 0, _) => M::FossilBones,
                (1, 0, _) if habitat.terrain.config().ecology_revision >= 20 => M::MineralStrata,
                (1, 0..=2, _) => M::PetrifiedMineralDeposit,
                (0 | 1, _, _) => M::PetrifiedRubble,
                (2, 0, _) => M::BrokenPetrifiedStump,
                (2, _, 0) => M::ShatteredStoneRoot,
                (2, _, _) => M::PetrifiedDeadRoot,
                (_, 0..=1, _) => M::LargeDryShrub,
                (_, _, 0) => M::PetrifiedRubble,
                _ => M::SmallDryShrub,
            };
            blocks.entry(offset).or_insert(prop as u16);
        }
    }
}

// Shelves can overhang a roof fall. Sample the same density used to generate
// voxels so every root and every object is seated on an intact surface.
fn bedrock_level(h: &mut Habitat<'_>, point: Tile) -> Option<i64> {
    let col = h.column(point);
    let nominal = (col.height - 0.5).floor() as i64;
    (nominal - 12..=nominal).rev().find(|y| {
        h.terrain
            .cave_density_with_column(
                [
                    point[0] as f64 + 0.5,
                    *y as f64 + 0.5,
                    point[1] as f64 + 0.5,
                ],
                col,
            )
            .0
            <= 0.
    })
}

fn trunk(
    blocks: &mut Blocks,
    p: Tile,
    base: i64,
    key: u32,
    [radius, height]: [f64; 2],
    mut ground: impl FnMut(Tile) -> Option<i64>,
) {
    let angle = f64::from(key % 360).to_radians();
    let (sin, cos) = angle.sin_cos();
    for dz in -10i64..=10 {
        for dx in -10i64..=10 {
            let point = [p[0] + dx, p[1] + dz];
            let local_x = dx as f64;
            let local_z = dz as f64;
            let r = local_x.hypot(local_z);
            if r > radius + 5. {
                continue;
            }
            let theta = local_z.atan2(local_x) + angle;
            let root = (theta * 6. + (r * 0.44).sin()).cos().max(0.).powi(4);
            let edge = radius + (theta * 7.).cos() * 0.34;
            if r > edge + root * 4.6 {
                continue;
            }
            let Some(floor) = ground(point) else {
                continue;
            };
            let ledge = height + (local_x * cos + local_z * sin) * 0.45;
            let outer = r >= edge - 0.75;
            let top = if r <= edge {
                base + (ledge + if outer { (theta * 9.).sin() * 0.8 } else { 0. }).round() as i64
            } else {
                let fade = (1. - (r - edge) / 4.7).max(0.);
                floor + (height * 0.44 * fade * fade * root).ceil() as i64
            };
            for y in floor + 1..=top {
                let mat = if y == top && !outer {
                    let ring = (r * 0.96 + 0.18 * (theta * 3.).sin()).floor() as u32;
                    if ring.is_multiple_of(2) {
                        M::PetrifiedHeartwood
                    } else {
                        M::PetrifiedSapwood
                    }
                } else {
                    M::PetrifiedLog
                };
                blocks.insert([point[0], y, point[1]], mat as u16);
            }
        }
    }
}

#[derive(Clone, Copy)]
struct Frame {
    center: Tile,
    base: i64,
    turn: u32,
}
impl Frame {
    fn at(self, x: i64, y: i64, z: i64) -> Cell {
        let (u, v) = match self.turn {
            1 => (-z, x),
            2 => (-x, -z),
            3 => (z, -x),
            _ => (x, z),
        };
        [self.center[0] + u, self.base + y, self.center[1] + v]
    }
    fn set(self, b: &mut Blocks, x: i64, y: i64, z: i64, m: M) {
        b.insert(self.at(x, y, z), m as u16);
    }
}

// Survey before stamping anything. Individual columns can have a stream or
// cliff even when the site's centre is dry. The stored elevations are reused
// when seating walls, uncovering bones and opening the descending walkway.
fn survey(h: &mut Habitat<'_>, f: Frame) -> Option<[[i64; 25]; 25]> {
    let mut ground = [[0; 25]; 25];
    for z in -12..=12 {
        for x in -12..=12 {
            let [px, _, pz] = f.at(x, 0, z);
            let c = h.column([px, pz]);
            let level = bedrock_level(h, [px, pz])?;
            if c.biome.weights.petrified_forest < 0.52
                || c.water_level.is_some()
                || (level - f.base).abs() > 5
            {
                return None;
            }
            ground[(z + 12) as usize][(x + 12) as usize] = level - f.base;
        }
    }
    Some(ground)
}

fn dig_level(x: i64, z: i64, natural: i64) -> i64 {
    if !(-9..=9).contains(&x) || !(-7..=7).contains(&z) {
        return natural;
    }
    // Leave the headland intact behind the pit. Each stepped bench exposes
    // another layer of shale, with an accessible shallow end near the shelter.
    if (-1..=8).contains(&x) && (-4..=3).contains(&z) {
        if (1..=6).contains(&x) && (-2..=1).contains(&z) {
            -2
        } else {
            -1
        }
    } else if z >= 5 || x > 7 {
        natural.clamp(-1, 2)
    } else {
        0
    }
}

fn excavation(
    habitat: &mut Habitat<'_>,
    b: &mut Blocks,
    f: Frame,
    surface: &[[i64; 25]; 25],
    key: u32,
) {
    let ground = |x: i64, z: i64| dig_level(x, z, surface[(z + 12) as usize][(x + 12) as usize]);
    for z in -7i64..=7 {
        for x in -9i64..=9 {
            let top = surface[(z + 12) as usize][(x + 12) as usize];
            let bench = ground(x, z);
            for y in bench + 1..=top + 1 {
                b.insert(f.at(x, y, z), 0);
            }
            for y in top.min(bench)..=bench {
                let material = if bench < 0 || (x * 19 + z * 7).rem_euclid(11) < 3 {
                    M::CrackedStoneSoil
                } else {
                    M::DustyGround
                };
                f.set(b, x, y, z, material);
            }
        }
    }
    let [px, _, pz] = f.at(5, 0, 8);
    trunk(
        b,
        [px, pz],
        f.base + ground(5, 8),
        key,
        [5.2, 8.2],
        |[x, z]| {
            let (dx, dz) = (x - f.center[0], z - f.center[1]);
            let (u, v) = match f.turn {
                1 => (dz, -dx),
                2 => (-dx, -dz),
                3 => (-dz, dx),
                _ => (dx, dz),
            };
            if u.abs() > 15 || v.abs() > 15 || (u < 1 && v < 4) {
                return None;
            }
            let y = if u.abs() <= 12 && v.abs() <= 12 {
                ground(u, v)
            } else {
                bedrock_level(habitat, [x, z])? - f.base
            };
            Some(f.base + y)
        },
    );
    let modern = habitat.terrain.config().ecology_revision >= 20;
    if modern {
        // An old tree split at the crown; the broken half retains its rings
        // while the remaining mineralized leader arches over the cut.
        let root = f.at(6, ground(6, 9), 9);
        TreeForm {
            root,
            height: 19.,
            radius: 2.55,
            wood: M::PetrifiedLog,
            seed: key,
        }
        .grow(habitat, b);
    }
    // Barricade and awning are lightly built, leaving a clear view across the cut.
    for x in -8..=-3 {
        for z in -5..=-1 {
            if [-8, -3].contains(&x) && [-5, -1].contains(&z) {
                for y in 1..=3 {
                    f.set(b, x, y, z, M::OakFence);
                }
            }
            f.set(b, x, 4, z, M::ExcavationAwning);
        }
    }
    for x in -7..=-4 {
        f.set(b, x, 1, -4, M::PetrifiedPlanks);
    }
    for (x, y, z, mat) in [
        (-7, 2, -4, M::ExcavationCrate),
        (-4, 2, -4, M::Lantern),
        (-8, 1, 1, M::ExcavationCrate),
        (-7, 1, 2, M::ExcavationCrate),
        (-2, 1, 1, M::ExcavationStandard),
    ] {
        f.set(b, x, y, z, mat);
    }
    // Exposed ribs run along the edge of the fossiliferous seam. The head
    // lies at the sunny end of the bench, above its collapsed lower jaw.
    let along_x = f.turn.is_multiple_of(2);
    for step in 0..=4 {
        let (x, z) = if along_x {
            (1 + step, -3)
        } else {
            (8, step - 4)
        };
        let material = if step == 4 {
            M::ExcavatedSkull
        } else {
            M::ExcavatedRib
        };
        f.set(b, x, ground(x, z) + 1, z, material);
    }
    for (x, z, mat) in [
        (2, 0, M::FossilBones),
        (5, -1, M::FossilBones),
        (0, -4, M::PetrifiedRubble),
        (-7, -7, M::PetrifiedRubble),
        (-8, -6, M::SmallDryShrub),
        (-8, 3, M::PetrifiedRubble),
        (-9, 5, M::SmallDryShrub),
        (-8, 6, M::LargeDryShrub),
        (-6, 7, M::SmallDryShrub),
        (10, -3, M::LargeDryShrub),
        (11, -2, M::PetrifiedRubble),
        (6, -6, M::PetrifiedRubble),
        (3, -5, M::ExcavationStandard),
        (0, -6, M::PetrifiedRubble),
        (1, -6, M::ShatteredStoneRoot),
    ] {
        f.set(b, x, ground(x, z) + 1, z, mat);
    }
    // An exposed mineral seam continues from a high central outcrop through
    // scattered satellite prisms. Every cap rests on the last intact block.
    for (x, z, rise) in [
        (-6, 5, 1),
        (-5, 5, 2),
        (-4, 5, 1),
        (-5, 6, 3),
        (-4, 6, 2),
        (-3, 6, 1),
        (-5, 7, 1),
        (8, -6, 2),
        (9, -6, 1),
        (8, -7, 1),
        (9, -7, 1),
        (-10, -4, 1),
        (-11, -4, 2),
        (-11, -3, 1),
    ] {
        let floor = ground(x, z);
        for dy in 0..=rise {
            f.set(
                b,
                x,
                floor + dy,
                z,
                if dy == rise {
                    M::MineralEncrustedStone
                } else {
                    M::CrackedStoneSoil
                },
            );
        }
        f.set(b, x, floor + rise + 1, z, M::PetrifiedMineralDeposit);
    }
    for (x, z, rise) in [
        (3, -7, 3),
        (6, -8, 1),
        (7, -8, 2),
        (8, -9, 1),
        (4, -7, 1),
        (-9, 8, 2),
        (-10, 8, 1),
    ] {
        let floor = ground(x, z);
        for dy in 1..=rise {
            f.set(
                b,
                x,
                floor + dy,
                z,
                if dy == 2 {
                    M::CarvedTempleStone
                } else {
                    M::WeatheredRuinStone
                },
            );
        }
        if rise == 1 {
            f.set(b, x, floor + 2, z, M::PetrifiedRubble);
        }
    }
    if modern {
        let floor = ground(-8, 0);
        f.set(b, -8, floor + 1, 0, M::ArchaeologyUrn);
        for (x, z) in [(2, -2), (3, -2), (6, 1), (7, 1), (0, -3)] {
            f.set(b, x, ground(x, z), z, M::FossiliferousStone);
        }
        // A shattered colonnade rises beyond the excavation.
        for (x, z, h) in [(-10, 7, 4), (-10, 11, 4), (-2, 10, 3), (2, -8, 3)] {
            let floor = ground(x, z);
            for dy in 1..=h {
                f.set(
                    b,
                    x,
                    floor + dy,
                    z,
                    if dy == 1 || dy == h {
                        M::TempleArchitrave
                    } else {
                        M::WeatheredRuinStone
                    },
                );
            }
        }
        let lintel = ground(-10, 7).max(ground(-10, 11)) + 5;
        for z in 7..=11 {
            if z == 7 || z == 11 {
                for y in ground(-10, z) + 5..lintel {
                    f.set(b, -10, y, z, M::WeatheredRuinStone);
                }
            }
            f.set(b, -10, lintel, z, M::TempleArchitrave);
        }
    }
}

fn buried_temple(b: &mut Blocks, f: Frame, surface: &[[i64; 25]; 25]) {
    // A tall rock-cut nave with a rough vaulted ceiling. Its side aisles wrap
    // around free-standing columns to the elevated fossil altar.
    for z in -11i64..=5 {
        for x in -10i64..=10 {
            if x.abs() + (-8 - z).max(0) > 11 {
                continue;
            }
            let floor = -11 + (-z - 5).clamp(0, 3);
            let ceiling = (-2 - x.abs() / 4).min(surface[(z + 12) as usize][(x + 12) as usize] - 2);
            f.set(b, x, floor, z, M::WeatheredRuinStone);
            for y in floor + 1..=ceiling {
                b.insert(f.at(x, y, z), 0);
            }
            if x.abs() == 9 || z == -10 {
                for y in floor + 1..=ceiling {
                    f.set(
                        b,
                        x,
                        y,
                        z,
                        if y == floor + 3 && x % 3 == 0 {
                            M::CarvedTempleStone
                        } else {
                            M::WeatheredRuinStone
                        },
                    );
                }
            }
        }
    }
    for x in [-6, 6] {
        for z in [-7, -2, 3] {
            let foot = -11 + (-z - 5i64).clamp(0, 3);
            for y in foot + 1..=-4 {
                f.set(
                    b,
                    x,
                    y,
                    z,
                    if y == foot + 3 {
                        M::CarvedTempleStone
                    } else {
                        M::WeatheredRuinStone
                    },
                );
            }
            for dx in -1..=1 {
                for dz in -1..=1 {
                    f.set(b, x + dx, foot + 1, z + dz, M::WeatheredRuinStone);
                    f.set(b, x + dx, -4, z + dz, M::WeatheredRuinStone);
                }
            }
        }
    }
    // Entrance stairs climb towards the actual sky, with a daylight well above
    // their upper flight. Continue the last step to local ground if it slopes.
    let exit = surface[24][12];
    for z in -4i64..=12 {
        for x in -2..=2 {
            let ground = surface[(z + 12) as usize][(x + 12) as usize];
            let step = -11 + (z + 4) * (11 + exit) / 16;
            let head = if z >= 6 {
                ground + 1
            } else {
                (step + 4).min(ground + 1)
            };
            for y in step + 1..=head {
                b.insert(f.at(x, y, z), 0);
            }
            f.set(b, x, step, z, M::WeatheredRuinStone);
        }
    }
    // A roof fall admits sunlight into the side aisle. Strands of
    // mineralized wood remain fused to the lip and trail down the wall.
    for z in -3i64..=1 {
        for x in 2i64..=5 {
            if (x - 4).abs() + (z + 1).abs() > 3 {
                continue;
            }
            let top = surface[(z + 12) as usize][(x + 12) as usize];
            for y in -4..=top + 1 {
                b.insert(f.at(x, y, z), 0);
            }
        }
    }
    for (x, z, length) in [(8, 0, 4), (8, -1, 3), (7, 0, 2), (3, -7, 2)] {
        for y in -2 - length..=-2 {
            f.set(b, x, y, z, M::PetrifiedLog);
        }
    }
    // Paired raised plinths and a broken lintel frame the fossil altar.
    for x in [-3, 3] {
        for y in -7..=-3 {
            f.set(
                b,
                x,
                y,
                -9,
                if y == -5 {
                    M::CarvedTempleStone
                } else {
                    M::WeatheredRuinStone
                },
            );
        }
    }
    for x in -3..=3 {
        f.set(b, x, -3, -9, M::WeatheredRuinStone);
    }
    for (x, z, rise) in [
        (-8, 2, 2),
        (-8, 1, 1),
        (-7, 2, 1),
        (8, -4, 2),
        (7, -4, 1),
        (8, -3, 1),
    ] {
        for y in -10..=-11 + rise {
            f.set(b, x, y, z, M::MineralEncrustedStone);
        }
        f.set(b, x, -10 + rise, z, M::PetrifiedMineralDeposit);
    }
    for (x, z) in [(4, 5), (-4, 3), (-8, -4), (8, 3)] {
        f.set(b, x, -10, z, M::PetrifiedRubble);
    }
    for x in -2..=2 {
        f.set(b, x, -7, -9, M::CarvedTempleStone);
    }
    f.set(b, 1, -6, -9, M::ExcavatedSkull);
    f.set(b, 0, -6, -9, M::FossilBones);
    for x in [-4, 4] {
        f.set(b, x, -8, -7, M::CarvedTempleStone);
        f.set(b, x, -7, -7, M::Lantern);
        f.set(b, x, -10, 1, M::PetrifiedMineralDeposit);
    }
    f.set(b, -5, -10, 0, M::ShatteredStoneRoot);
    f.set(b, 7, -10, 1, M::FossilBones);
    f.set(b, 5, -7, -9, M::CryptTornBanner);
}

// The open upper stair passes beneath a stepped sandstone arch. Both jambs
// start on surveyed rock, framed by fossil panels and weathered turquoise inlay.
fn temple_forecourt(
    h: &mut Habitat<'_>,
    b: &mut Blocks,
    f: Frame,
    surface: &[[i64; 25]; 25],
    key: u32,
    objects: &mut Vec<StructureObject>,
) {
    let ground = |x: i64, z: i64| surface[(z + 12) as usize][(x + 12) as usize];
    let level = ground(0, 10);
    // The front of the hall was cut into a fossil-bearing rock shelf. Keep
    // the descending stair clear underneath its vaulted overhang. The rear
    // of the ridge falls back to the original hillside before the light well.
    for z in 2_i64..=9 {
        for x in -9_i64..=9 {
            let natural = ground(x, z);
            let breadth = (10 - x.abs()).clamp(0, 5);
            let ridge = (z - 1).min(5) + breadth - 1;
            let top = level + ridge;
            let ceiling = if x.abs() <= 2 {
                // Match the height of the doorway at its axis and leave
                // four blocks above the actual descending stair.
                let stair = -11 + (z + 4) * (11 + ground(0, 12)) / 16;
                (stair + 5).max(level + 2 + (z - 3).max(0) / 2)
            } else {
                natural
            };
            for y in ceiling..=top {
                let joint = (x * 11 + y * 13 + z * 23).rem_euclid(19);
                let material = if y == top && joint < 4 {
                    M::RootBoundStone
                } else if y == top && joint == 8 {
                    M::TurquoiseVeinedStone
                } else if joint == 11 && x.abs() > 3 {
                    M::FossiliferousStone
                } else if z == 9 && x.abs() <= 5 {
                    M::WeatheredRuinStone
                } else {
                    M::CrackedStoneSoil
                };
                f.set(b, x, y, z, material);
            }
        }
    }
    // A fossil root crosses the crown of the portal and sinks into the
    // stone flank, leaving the upper edge broken against the sky.
    for x in -7_i64..=6 {
        let z = 6 + (x + 7) / 5;
        let y = level + (z - 1).min(5) + (10 - x.abs()).clamp(0, 5) - 1;
        f.set(b, x, y, z, M::RootBoundStone);
        f.set(b, x, y + 1, z, M::PetrifiedLog);
        if (-5..=2).contains(&x) {
            f.set(b, x, y + 1, z + 1, M::RootBoundStone);
        }
    }
    for z in [8, 10] {
        for x in [-4, 4] {
            for y in ground(x, z).min(level)..=level + 5 {
                f.set(
                    b,
                    x,
                    y,
                    z,
                    if y == level + 1 || y == level + 5 {
                        M::TempleArchitrave
                    } else {
                        M::WeatheredRuinStone
                    },
                );
                if y <= level + 1 {
                    f.set(b, x + x.signum(), y, z, M::FossiliferousStone);
                }
            }
        }
        for x in -4_i64..=4 {
            let y = level + 5 + (4 - x.abs()).min(2);
            f.set(b, x, y, z, M::WeatheredRuinStone);
            f.set(b, x, y + 1, z, M::TempleArchitrave);
            if x.abs() > 2 {
                f.set(b, x, y - 1, z, M::WeatheredRuinStone);
            }
        }
    }
    for x in [-4, 4] {
        f.set(b, x, level + 4, 11, M::TemplePennant);
        objects.push(StructureObject {
            position: f.at(x, level + 4, 11),
            material: M::TemplePennant as u16,
            state: Some(BlockObjectState {
                facing: ((4 - f.turn) % 4) as u8,
                ..Default::default()
            }),
            loot: None,
            crop: None,
        });
        let g = ground(x, 12);
        f.set(b, x, g + 1, 12, M::TempleArchitrave);
        f.set(b, x, g + 2, 12, M::AshenBrazier);
        // The lower aisle repeats the same painted lintels across its columns.
        for z in [-9, -8] {
            f.set(b, x, -4, z, M::TempleArchitrave);
        }
    }
    // Splayed buttresses frame the entrance. Their turquoise impost
    // course carries across the arch to a pale carved keystone.
    for side in [-1_i64, 1] {
        for x in 5..=8 {
            let u = x * side;
            let z = 9 + (x - 5) / 2;
            let floor = ground(u, z);
            let height = level + 7 - (x - 5) * 2;
            for y in floor..=height {
                let material = if y == height || y == level + 1 {
                    M::TempleArchitrave
                } else if (x + y).rem_euclid(5) == 0 {
                    M::FossiliferousStone
                } else {
                    M::WeatheredRuinStone
                };
                f.set(b, u, y, z, material);
            }
        }
        // A freestanding, capped pilaster at each corner of the approach.
        let x = 8 * side;
        let g = ground(x, 12);
        for y in g..=g + 4 {
            f.set(
                b,
                x,
                y,
                12,
                if y == g + 3 {
                    M::TempleArchitrave
                } else {
                    M::CarvedTempleStone
                },
            );
        }
        for dx in -1..=1 {
            f.set(b, x + dx, g + 5, 12, M::WeatheredRuinStone);
        }
    }
    f.set(b, 0, level + 7, 10, M::CarvedTempleStone);
    for x in -1..=1 {
        f.set(b, x, level + 9, 10, M::TempleArchitrave);
    }

    // Lichen, scree and yellow grasses indicate the old approach from the
    // valley. The central line of the path stays clear for players on foot.
    for z in 11..=15 {
        for x in -11_i64..=11 {
            let [wx, _, wz] = f.at(x, 0, z);
            let Some(y) = bedrock_level(h, [wx, wz]) else {
                continue;
            };
            let y = y - f.base;
            let k = ticket(u64::from(key), [wx, wz], 9401);
            if z >= 13 && x.abs() <= 3 {
                f.set(
                    b,
                    x,
                    y,
                    z,
                    if k.is_multiple_of(9) {
                        M::FossiliferousStone
                    } else if k.is_multiple_of(4) {
                        M::WeatheredRuinStone
                    } else {
                        M::CrackedStoneSoil
                    },
                );
            } else if x.abs() >= 4 && k.is_multiple_of(4) && !b.contains_key(&f.at(x, y + 1, z)) {
                let plant = match (x.abs() + z).rem_euclid(9) {
                    0 => M::SmallDryShrub,
                    1..=3 => M::PetrifiedRubble,
                    4 => M::MineralStrata,
                    _ => M::DarkMushrooms,
                };
                f.set(b, x, y + 1, z, plant);
            }
        }
    }
    let root = f.at(-8, ground(-8, 4), 4);
    TreeForm {
        root,
        height: 16.,
        radius: 2.2,
        wood: M::PetrifiedLog,
        seed: key,
    }
    .grow(h, b);
    for (x, z) in [(7, 6), (8, 7), (8, 8), (-7, 10)] {
        let y = ground(x, z);
        f.set(b, x, y, z, M::TurquoiseVeinedStone);
        f.set(
            b,
            x,
            y + 1,
            z,
            if x % 2 == 0 {
                M::PetrifiedMineralDeposit
            } else {
                M::MineralStrata
            },
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{SectionGenerator, TerrainGenerator};
    use terra_core::WorldConfig;

    #[test]
    fn abandoned_sites_and_the_fossil_bed_are_present_in_the_generated_world() {
        let terrain = crate::TerraGenerator::new(WorldConfig::default()).unwrap();
        let generator = SectionGenerator::new(terrain.clone());
        let old = crate::TerraGenerator::new(WorldConfig {
            ecology_revision: 18,
            ..Default::default()
        })
        .unwrap();
        for (tile, marker) in [
            ([-21, 21], M::ExcavationCrate),
            ([-21, 12], M::ShatteredStoneRoot),
        ] {
            let [x, z] = tile.map(|n| n * TILE_SIZE + 16);
            let column = terrain.surface_column(x as f64 + 0.5, z as f64 + 0.5);
            assert!(column.biome.weights.petrified_forest > 0.52);
            assert_eq!(
                old.biome_at(x as f64, z as f64).weights.petrified_forest,
                0.
            );
            let patch = terrain.natural_patch(tile);
            let p = patch
                .cells
                .iter()
                .find(|(_, m)| *m == marker as u16)
                .expect("generated landmark marker")
                .0;
            let (section, local) = terra_voxel::global_to_section(p);
            let built = generator
                .build(section, 0, &Default::default(), true)
                .unwrap();
            let [lo, hi] = built.surface_sections.unwrap();
            assert!((lo..=hi).contains(&section.y));
            assert_eq!(built.data.block(local).unwrap(), marker as u16);
            assert_eq!(generator.block_at(p).unwrap(), marker as u16);
            assert!(
                patch
                    .cells
                    .iter()
                    .any(|(_, m)| *m == M::CarvedTempleStone as u16)
            );
            assert!(patch.cells.iter().any(|(_, m)| *m == M::FossilBones as u16));
            if marker == M::ExcavationCrate {
                for required in [
                    M::PetrifiedLog,
                    M::PetrifiedSapwood,
                    M::PetrifiedHeartwood,
                    M::PetrifiedMineralDeposit,
                    M::ExcavatedSkull,
                    M::ExcavatedRib,
                ] {
                    assert!(
                        patch.cells.iter().any(|(_, mat)| *mat == required as u16),
                        "{required:?}"
                    );
                }
                for &([px, py, pz], id) in &patch.cells {
                    if matches!(id, 710..=717 | 719 | 722..=723) {
                        let below = generator.block_at([px, py - 1, pz]).unwrap();
                        assert!(
                            M::from_raw(below).is_some_and(M::is_solid),
                            "unsupported fossil site material {id} at {:?}",
                            [px, py, pz]
                        );
                    }
                }
            }
            if marker == M::ShatteredStoneRoot {
                assert!((p[1] as f64) < column.height - 6.);
                let air = [p[0], p[1] + 1, p[2]];
                assert_eq!(generator.block_at(air).unwrap(), 0);
                let sample = terrain.sample(air.map(|n| n as f64 + 0.5));
                assert!(sample.density > 0. && sample.material == 0);
            }
        }
    }
}

#[cfg(test)]
#[test]
fn petrified_fungal_colonies_recur_in_groups_on_actual_surface_sections() {
    use crate::SectionGenerator;
    let terrain = crate::TerraGenerator::new(Default::default()).unwrap();
    let sections = SectionGenerator::new(terrain.clone());
    for tx in -24..-18 {
        for tz in 13..23 {
            let patch = terrain.natural_patch([tx, tz]);
            let fungi: Vec<_> = patch
                .cells
                .iter()
                .filter(|(_, id)| {
                    matches!(
                        M::from_raw(*id),
                        Some(M::DarkMushrooms | M::GlowingPurpleMushrooms | M::BloodMushrooms)
                    )
                })
                .collect();
            if fungi.len() < 4 {
                continue;
            }
            assert!(fungi.iter().any(|(a, _)| {
                fungi
                    .iter()
                    .any(|(b, _)| a != b && (a[0] - b[0]).abs() + (a[2] - b[2]).abs() < 5)
            }));
            let (section, local) = terra_voxel::global_to_section(fungi[0].0);
            let built = sections
                .build(section, 0, &Default::default(), true)
                .unwrap();
            assert_eq!(built.data.block(local).unwrap(), fungi[0].1);
            assert!(
                built
                    .props
                    .iter()
                    .all(|p| p.kind != crate::props::PropKind::Flower
                        || terrain
                            .surface_biome(&terrain.surface_column(p.position[0], p.position[2]))
                            != crate::generation::biomes::BiomeKind::PetrifiedForest)
            );
            return;
        }
    }
    panic!("no supported fungal colony");
}
