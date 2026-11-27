//! A breached basalt sill feeds an incandescent cascade beside a mineral
//! spring. The cold and molten basins have separate stone walls and floors.
use super::{Cell, Habitat, TILE_SIZE, Tile, relic_trees::TreeForm, ticket};
use std::collections::BTreeMap;
use terra_voxel::MaterialId as M;

type Blocks = BTreeMap<Cell, u16>;
struct Site {
    center: Tile,
    base: i64,
    turn: u32,
    ground: [[i64; 31]; 31],
}
impl Site {
    fn point(&self, x: i64, y: i64, z: i64) -> Cell {
        let (u, v) = match self.turn {
            1 => (-z, x),
            2 => (-x, -z),
            3 => (z, -x),
            _ => (x, z),
        };
        [self.center[0] + u, self.base + y, self.center[1] + v]
    }
    fn floor(&self, x: i64, z: i64) -> i64 {
        self.ground[(z + 15) as usize][(x + 15) as usize]
    }
    fn put(&self, b: &mut Blocks, x: i64, y: i64, z: i64, m: M) {
        b.insert(self.point(x, y, z), m as u16);
    }
    fn cut(&self, b: &mut Blocks, x: i64, z: i64, level: i64, mat: M) {
        let ground = self.floor(x, z);
        for y in (ground.min(level) - 1)..=level {
            self.put(b, x, y, z, mat);
        }
        for y in level + 1..=ground + 1 {
            self.put(b, x, y, z, M::Air);
        }
    }
}

pub(super) fn build(
    h: &mut Habitat<'_>,
    tile: Tile,
    b: &mut Blocks,
    objects: &mut Vec<crate::generation::structures::StructureObject>,
) -> bool {
    let seed = h.terrain.config().seed.0;
    let region = tile.map(|n| n.div_euclid(3));
    let key = ticket(seed, region, 8422);
    if tile[0].rem_euclid(3) != i64::from(key % 3)
        || tile[1].rem_euclid(3) != i64::from((key >> 8) % 3)
    {
        return false;
    }
    let center = tile.map(|n| n * TILE_SIZE + 16);
    let Some((base, col)) = h.ground(center) else {
        return false;
    };
    if col.biome.weights.ashlands < 0.68 || h.reserved(center, 15) || col.slope > 0.85 {
        return false;
    }
    let mut s = Site {
        center,
        base,
        turn: (key >> 20) % 4,
        ground: [[0; 31]; 31],
    };
    for z in -15..=15 {
        for x in -15..=15 {
            let [px, _, pz] = s.point(x, 0, z);
            let c = h.column([px, pz]);
            let floor = (c.height - 0.5).floor() as i64;
            if c.biome.weights.ashlands < 0.44
                || c.water_level.is_some()
                || (floor - base).abs() > 11
                || c.cave_cavity.is_some_and(|[_, top]| top > c.height - 2.0)
            {
                return false;
            }
            s.ground[(z + 15) as usize][(x + 15) as usize] = floor - base;
        }
    }
    let has_water = h.terrain.config().water;
    // Native pond shoreline with a crescent of black glass. The hot outlet
    // stands on a shallow island just above the waterline so its jet is visible.
    for z in -7_i64..=8 {
        for x in -11_i64..=3 {
            let r = ((x + 4) as f64 / 6.4).powi(2) + ((z - 1) as f64 / 5.8).powi(2);
            if r > 1.28 {
                continue;
            }
            let surface = -1;
            let floor = surface
                - if r < 0.50 {
                    3
                } else if r < 0.83 {
                    2
                } else if r < 1. {
                    1
                } else {
                    0
                };
            s.cut(
                b,
                x,
                z,
                floor,
                if r < 0.83 {
                    M::TurquoiseVeinedStone
                } else {
                    M::VolcanicObsidian
                },
            );
            for y in floor + 1..=surface {
                s.put(
                    b,
                    x,
                    y,
                    z,
                    if !has_water {
                        M::MineralEncrustedStone
                    } else if y == surface && x.rem_euclid(2) == 0 && z.rem_euclid(2) == 0 {
                        M::FluidWater
                    } else {
                        M::Air
                    },
                );
            }
        }
    }
    if has_water {
        for y in -4..=-1 {
            s.put(b, -5, y, 3, M::VolcanicObsidian);
        }
        s.put(b, -5, 0, 3, M::GeyserVent);
    }
    // A basalt amphitheatre is stratified into three narrowing terraces.
    // The fall lips overlap the lower liquid by one column, keeping the
    // exposed vertical sheets connected at the landings.
    for z in -6_i64..=9 {
        for x in 2_i64..=15 {
            let bank = (z - 1).abs();
            let rough = (x * 17 + z * 11).rem_euclid(7);
            let sill = match x {
                2..=4 => 0,
                5..=7 => 3,
                8..=10 => 6,
                _ => 9,
            };
            let edge = (7 - bank).clamp(0, 4);
            let high = if edge < 3 {
                (sill - 2 * (3 - edge)).max(-1)
            } else {
                sill
            };
            if bank > 5 && rough > 3 {
                continue;
            }
            let old = s.floor(x, z);
            for y in old.min(-3)..=high {
                let mat = if y == high && bank > 2 && rough < 2 {
                    M::VolcanicObsidian
                } else if (y + 2).rem_euclid(5) == 0 {
                    M::DarkVolcanicStone
                } else {
                    M::ColumnarBasalt
                };
                s.put(b, x, y, z, mat);
            }
            for y in high + 1..=old + 1 {
                s.put(b, x, y, z, M::Air);
            }
            if bank <= 2 && x == 15 {
                // The spring emerges through a broken obsidian headwall.
                for y in old.min(sill - 2)..=sill + 1 + i64::from(bank != 0) {
                    s.put(b, x, y, z, M::VolcanicObsidian);
                }
            } else if (-1..=1).contains(&(z - 1)) {
                s.put(b, x, high - 1, z, M::VolcanicObsidian);
                for y in high..=sill {
                    s.put(b, x, y, z, M::Lava);
                }
                if matches!(x, 4 | 7 | 10) {
                    for y in sill..=sill + 3 {
                        s.put(b, x, y, z, M::Lava);
                    }
                }
            } else if bank == 2 {
                s.put(
                    b,
                    x,
                    high + 1,
                    z,
                    if rough < 2 {
                        M::LavaCrust
                    } else {
                        M::ColumnarBasalt
                    },
                );
            } else if high > 1 && rough == 0 {
                s.put(b, x, high + 1, z, M::EmberCrystals);
            }
        }
    }
    // A one-metre sinter divide prevents the lake from touching the lava.
    for z in -1..=3 {
        for y in s.floor(1, z).min(-4)..=0 {
            s.put(b, 1, y, z, M::VolcanicObsidian);
        }
    }
    if h.terrain.config().structures.ruins && h.terrain.config().structures_revision > 0 {
        shrine(&s, b);
    }
    if h.terrain.config().trees {
        let root = s.point(-9, s.floor(-9, 8), 8);
        TreeForm {
            root,
            radius: 2.2,
            height: 14.5,
            wood: M::CharredLog,
            seed: key,
        }
        .grow(h, b);
    }
    for (x, z, mat) in [
        (-10, 6, M::CinderRoot),
        (-11, 7, M::EmberBloom),
        (-7, 8, M::AshMushrooms),
        (-2, 8, M::FallenCharredLog),
        (-1, 8, M::EmberBloom),
        (-8, -3, M::EmberCrystals),
        (-11, 2, M::BurntGrass),
        (-12, 2, M::BurntGrass),
        (1, 8, M::EmberCrystals),
        (-2, 10, M::FossilBones),
    ] {
        if !b.contains_key(&s.point(x, s.floor(x, z) + 1, z)) {
            s.put(b, x, s.floor(x, z) + 1, z, mat);
        }
    }
    // Like village wells these outlets enter the realm fluid journal when
    // their section streams in. Sources stay removable and survive reloads.
    objects.extend(
        b.iter()
            .filter(|(_, raw)| **raw == M::FluidWater as u16)
            .map(
                |(&position, &material)| crate::generation::structures::StructureObject {
                    position,
                    material,
                    state: None,
                    loot: None,
                    crop: None,
                },
            ),
    );
    true
}

fn shrine(s: &Site, b: &mut Blocks) {
    for z in -14_i64..=-5 {
        for x in -10_i64..=1 {
            let step = (-z - 5).clamp(0, 4);
            s.cut(b, x, z, step, M::ScoriaBricks);
        }
    }
    for (x, z, height) in [
        (-9, -12, 4),
        (-6, -12, 7),
        (-1, -12, 7),
        (1, -10, 4),
        (-10, -6, 2),
        (0, -6, 2),
    ] {
        let foot = (-z - 5_i64).clamp(0, 4);
        for y in foot + 1..=foot + height {
            s.put(
                b,
                x,
                y,
                z,
                if y == 7 && height > 4 {
                    M::PyricRuneStone
                } else {
                    M::ScoriaBricks
                },
            );
        }
        if height >= 4 {
            for dx in -1..=1 {
                s.put(b, x + dx, foot + height, z, M::ScoriaBricks);
            }
        }
    }
    for x in -6_i64..=-1 {
        let y = 11 + (x + 6).min(-1 - x).min(2);
        s.put(b, x, y, -12, M::ScoriaBricks);
        s.put(b, x, y + 1, -12, M::ScoriaBricks);
    }
    // Cloth hangs from each shoulder of the arch, framing the rune.
    for x in [-5, -2] {
        s.put(b, x, 11, -12, M::TornBloodBanner);
    }
    for x in [-9, -1] {
        for y in 2..=3 {
            s.put(b, x, y, -7, M::ScoriaBricks);
        }
        s.put(b, x, 4, -7, M::AshenBrazier);
    }
    for y in 5..=8 {
        s.put(
            b,
            -4,
            y,
            -14,
            if y == 7 {
                M::PyricRuneStone
            } else {
                M::VolcanicObsidian
            },
        );
    }
    s.put(b, -3, 5, -13, M::EmberCrystals);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{SectionGenerator, TerraGenerator};
    use terra_core::WorldConfig;

    #[test]
    fn spring_initializes_persistent_water_sources_and_the_fall_stays_joined() {
        let t = TerraGenerator::new(WorldConfig::default()).unwrap();
        let g = SectionGenerator::new(t.clone());
        let tile = [74, -146];
        let patch = t.natural_patch(tile);
        let vent = [2389, 30, -4659];
        assert_eq!(g.block_at(vent).unwrap(), M::GeyserVent as u16);
        assert_eq!(g.block_at([vent[0], vent[1] + 1, vent[2]]).unwrap(), 0);
        assert!(patch.objects.len() >= 12);
        for source in &patch.objects {
            assert_eq!(source.material, M::FluidWater as u16);
            assert_eq!(patch.block_at(source.position), Some(source.material));
        }
        // This rotated cascade spans three connected drops of molten rock.
        for (x, y) in [(2380, 30), (2377, 33), (2374, 36)] {
            for dy in 0..=3 {
                assert_eq!(g.block_at([x, y + dy, -4657]).unwrap(), M::Lava as u16);
            }
        }
        for z in -4658..=-4656 {
            assert_eq!(
                g.block_at([2369, 40, z]).unwrap(),
                M::VolcanicObsidian as u16
            );
            assert_eq!(g.block_at([2370, 39, z]).unwrap(), M::Lava as u16);
            assert_eq!(
                g.block_at([2370, 38, z]).unwrap(),
                M::VolcanicObsidian as u16
            );
        }
        for id in [
            M::PyricRuneStone,
            M::AshenBrazier,
            M::ScoriaBricks,
            M::CharredLog,
        ] {
            assert!(patch.cells.iter().any(|(_, m)| *m == id as u16), "{id:?}");
        }
        let mut old = t.config().clone();
        old.ecology_revision = 19;
        assert!(
            !TerraGenerator::new(old)
                .unwrap()
                .surface_flora(tile)
                .iter()
                .any(|(_, m)| *m == M::GeyserVent as u16)
        );
        let mut dry = t.config().clone();
        dry.water = false;
        let dry_patch = TerraGenerator::new(dry).unwrap().natural_patch(tile);
        assert!(
            dry_patch
                .objects
                .iter()
                .all(|s| s.material != M::FluidWater as u16)
        );
        assert!(
            dry_patch
                .cells
                .iter()
                .all(|(_, m)| *m != M::GeyserVent as u16)
        );
    }
}
