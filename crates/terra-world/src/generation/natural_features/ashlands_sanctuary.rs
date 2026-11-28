//! A breached basalt sill feeds an incandescent cascade beside a mineral
//! spring. The cold and molten basins have separate stone walls and floors.
use super::{Cell, Habitat, TILE_SIZE, Tile, relic_trees::TreeForm, ticket};
use crate::volcano::geyser::{SUBMERGED_ECOLOGY_REVISION, VENT_DEPTH};
use crate::water::falls::{CALM_SOURCE_REACH, Fall};
use std::collections::{BTreeMap, BTreeSet};
use terra_voxel::MaterialId as M;

const TERRAIN_FITTED_REVISION: u8 = 25;
const SPRING_X: std::ops::RangeInclusive<i64> = -10..=0;
const SPRING_Z: std::ops::RangeInclusive<i64> = -4..=6;
const JOINED_SPILLWAY_REVISION: u8 = 26;
const FIRST_SPILLWAY_CELL: i64 = 2;

type Blocks = BTreeMap<Cell, u16>;
struct Site {
    center: Tile,
    base: i64,
    turn: u32,
    ground: [[i64; 31]; 31],
    lava: [[i64; 31]; 31],
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
    fn lava_top(&self, x: i64, z: i64) -> i64 {
        self.lava[(z + 15) as usize][(x + 15) as usize]
    }
    fn put(&self, b: &mut Blocks, x: i64, y: i64, z: i64, m: M) {
        b.insert(self.point(x, y, z), m as u16);
    }
    fn foundation(&self, b: &mut Blocks, x: i64, z: i64, level: i64, mat: M) {
        for y in (self.floor(x, z).min(level) - 1)..=level {
            self.put(b, x, y, z, mat);
        }
    }
    fn cut(&self, b: &mut Blocks, x: i64, z: i64, level: i64, mat: M) {
        self.foundation(b, x, z, level, mat);
        for y in level + 1..=self.floor(x, z) + 1 {
            self.put(b, x, y, z, M::Air);
        }
    }
}

pub(super) fn may_have_spring(terrain: &crate::TerraGenerator, tile: Tile, p: Tile) -> bool {
    if terrain.config().ecology_revision < TERRAIN_FITTED_REVISION
        || !terrain.config().water
        || !terrain.config().biomes
    {
        return false;
    }
    let Some(key) = site_key(terrain.config().seed.0, tile) else {
        return false;
    };
    let [u, v] = std::array::from_fn(|i| p[i] - (tile[i] * TILE_SIZE + 16));
    let [x, z] = match (key >> 20) % 4 {
        1 => [v, -u],
        2 => [-u, -v],
        3 => [-v, u],
        _ => [u, v],
    };
    SPRING_X.contains(&x) && SPRING_Z.contains(&z)
}

fn site_key(seed: u64, tile: Tile) -> Option<u32> {
    let region = tile.map(|n| n.div_euclid(3));
    let key = ticket(seed, region, 8422);
    (tile[0].rem_euclid(3) == i64::from(key % 3)
        && tile[1].rem_euclid(3) == i64::from((key >> 8) % 3))
    .then_some(key)
}

pub(super) fn build(
    h: &mut Habitat<'_>,
    tile: Tile,
    b: &mut Blocks,
    objects: &mut Vec<crate::generation::structures::StructureObject>,
    spring: &mut Vec<(Tile, Fall)>,
) -> bool {
    let seed = h.terrain.config().seed.0;
    let Some(key) = site_key(seed, tile) else {
        return false;
    };
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
        lava: [[i64::MIN; 31]; 31],
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
            if let Some(lava) = c.lava_level {
                s.lava[(z + 15) as usize][(x + 15) as usize] = (lava - 0.5).floor() as i64 - base;
            }
        }
    }
    let has_water = h.terrain.config().water;
    let fitted = h.terrain.config().ecology_revision >= TERRAIN_FITTED_REVISION;
    if h.terrain.config().ecology_revision >= JOINED_SPILLWAY_REVISION {
        joined_spillway(&s, b, key);
    } else if fitted {
        fitted_spillway(&s, b, key);
    } else {
        legacy_spring(&s, b, has_water, h.terrain.config().ecology_revision);
        legacy_spillway(&s, b);
    }
    if h.terrain.config().structures.ruins && h.terrain.config().structures_revision > 0 {
        shrine(&s, b, fitted);
    }
    if h.terrain.config().trees {
        let [x, z] = if fitted { [-12, 9] } else { [-9, 8] };
        let root = s.point(x, s.floor(x, z), z);
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
    // Close the basin after putting in the surrounding formations and ruins.
    if fitted {
        fitted_spring(&s, b, has_water, key, spring);
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

fn legacy_spring(s: &Site, b: &mut Blocks, has_water: bool, ecology_revision: u8) {
    // Native pond shoreline with a crescent of black glass around the vent.
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
        if ecology_revision >= SUBMERGED_ECOLOGY_REVISION {
            s.put(b, -5, -VENT_DEPTH, 3, M::GeyserVent);
            // The authored water span fills all the way down to the mouth on
            // first publication. The surface source is removable like the others.
            s.put(b, -5, -1, 3, M::FluidWater);
        } else {
            for y in -4..=-1 {
                s.put(b, -5, y, 3, M::VolcanicObsidian);
            }
            s.put(b, -5, 0, 3, M::GeyserVent);
        }
    }
}

fn legacy_spillway(s: &Site, b: &mut Blocks) {
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
}

fn fitted_spillway(s: &Site, b: &mut Blocks, key: u32) {
    const FIRST: i64 = FIRST_SPILLWAY_CELL;
    const LAST: i64 = 14;
    let bend = if key & 0x40000000 == 0 { -1 } else { 1 };
    let axis = |x| match x {
        5..=7 | 11..=13 => 1 + bend,
        8..=10 => 1 + 2 * bend,
        _ => 1,
    };
    let outlet = (s.floor(FIRST, axis(FIRST)) + 1).clamp(-3, 2);
    let head = (s.floor(LAST, axis(LAST)) + 1).clamp(outlet + 3, outlet + 7);
    let mut level = outlet;
    for x in FIRST..=LAST {
        let center = axis(x);
        let grade = outlet + (head - outlet) * (x - FIRST) / (LAST - FIRST);
        let native = s.floor(x, center) + 1;
        // The longitudinal grade joins the surrounding slopes. Each metre can
        // rise once, even where the native ground is terraced irregularly.
        if x > FIRST {
            level = native
                .clamp(grade - 1, grade + 1)
                .clamp(level, level + 1)
                .min(head);
        }
        for z in center - 4..=center + 4 {
            let distance = (z - center).abs();
            let ground = s.floor(x, z);
            let uneven = (x * 17 + z * 11 + i64::from(key % 11)).rem_euclid(7);
            if distance <= 1 {
                s.cut(b, x, z, level - 1, M::VolcanicObsidian);
                for y in level + 1..=s.lava_top(x, z) {
                    s.put(b, x, y, z, M::Air);
                }
                s.put(b, x, level, z, M::Lava);
                continue;
            }
            let wanted = level + 1 - (distance - 2) - i64::from(distance > 2 && uneven < 3);
            if wanted <= ground {
                continue;
            }
            for y in ground + 1..=wanted {
                let material = if y == wanted && uneven < 2 {
                    M::VolcanicObsidian
                } else if distance == 4 {
                    M::VolcanicGravel
                } else {
                    M::ColumnarBasalt
                };
                s.put(b, x, y, z, material);
            }
        }
    }
    for z in axis(LAST) - 1..=axis(LAST) + 1 {
        for y in s.floor(15, z) + 1..=level + 1 {
            s.put(b, 15, y, z, M::VolcanicObsidian);
        }
    }
}

fn joined_spillway(s: &Site, b: &mut Blocks, key: u32) {
    const LAST: i64 = 14;
    const LENGTH: i64 = LAST - FIRST_SPILLWAY_CELL;
    const DROP: i64 = 2;
    let bend = if key & 0x40000000 == 0 { -1 } else { 1 };
    let axis = |x| match x {
        5..=7 | 11..=13 => 1 + bend,
        8..=10 => 1 + 2 * bend,
        _ => 1,
    };
    let grade = |x| s.floor(x, axis(x)).max(s.lava_top(x, axis(x)));
    let forward = grade(LAST) >= grade(FIRST_SPILLWAY_CELL);
    let x_at = |run| {
        if forward {
            FIRST_SPILLWAY_CELL + run
        } else {
            LAST - run
        }
    };
    let ground_at = |run| grade(x_at(run));
    let head = ground_at(LENGTH);
    // In low relief the plunge pool sits inside a shallow cut. On a hillside
    // both ends meet the native levels and the existing rise supplies the fall.
    let foot = ground_at(0).min(head - DROP);
    let lip = (5_i64..=8)
        .max_by_key(|&run| (ground_at(run) - ground_at(run - 1), -(run - 6).abs()))
        .unwrap();
    let pool =
        ((ground_at(lip - 1) + ground_at(lip) - DROP).div_euclid(2)).clamp(foot, head - DROP);
    let level_at = |run| {
        if run < lip {
            foot + (pool - foot) * run / (lip - 1)
        } else {
            pool + DROP + (head - pool - DROP) * (run - lip) / (LENGTH - lip)
        }
    };
    let width_at = |run: i64| {
        let center = axis(x_at(run));
        let previous = axis(x_at((run - 1).max(0)));
        let extra = i64::from((3..=9).contains(&run));
        (center.min(previous) - extra, center.max(previous) + extra)
    };
    for run in 0..=LENGTH {
        let x = x_at(run);
        let level = level_at(run);
        let (lo, hi) = width_at(run);
        let downstream = (run - 1).max(0);
        let below = level_at(downstream);
        let (out_lo, out_hi) = width_at(downstream);
        for z in lo - 2..=hi + 2 {
            let distance = (lo - z).max(z - hi).max(0);
            let ground = s.floor(x, z);
            let stagger = (x * 17 + z * 11 + i64::from(key % 11)).rem_euclid(7);
            if distance == 0 {
                // A two-block brink carries one continuous sheet down to the
                // lower reach. Single-block grades get a sloping liquid cap.
                let bottom = if level - below >= DROP && (out_lo..=out_hi).contains(&z) {
                    below
                } else {
                    level
                };
                s.cut(b, x, z, bottom - 1, M::DarkVolcanicStone);
                for y in level + 1..=s.lava_top(x, z) {
                    s.put(b, x, y, z, M::Air);
                }
                for y in bottom..=level {
                    s.put(b, x, y, z, M::Lava);
                }
            } else if distance == 1 {
                if ground < level {
                    s.foundation(b, x, z, level, M::DarkVolcanicStone);
                } else if ground > level + 1 && stagger < 4 {
                    s.cut(b, x, z, ground - 1, M::DarkVolcanicStone);
                }
            } else if ground < level - 1 && stagger > 1 {
                // Only the exposed shoulder needs a rubble step. The rest of
                // the outer edge belongs to the original ground profile.
                s.foundation(b, x, z, level - 1, M::VolcanicGravel);
            }
        }
    }
}

fn fitted_spring(
    s: &Site,
    b: &mut Blocks,
    has_water: bool,
    key: u32,
    spring: &mut Vec<(Tile, Fall)>,
) {
    const SURFACE: i64 = -1;
    const MOUTH: Tile = [-5, 3];
    const SIDES: [Tile; 4] = [[-1, 0], [1, 0], [0, -1], [0, 1]];
    let mut bed = BTreeMap::new();
    let rotation = f64::from(key % 19) * 0.41;
    for z in SPRING_Z {
        for x in SPRING_X {
            let uneven = (rotation + x as f64 * 0.72 + z as f64 * 0.94).sin() * 0.075;
            let r = ((x + 5) as f64 / 5.9).powi(2) + ((z - 1) as f64 / 5.4).powi(2) + uneven;
            let depth = match r {
                r if r < 0.55 => 3,
                r if r < 0.79 => 2,
                r if r < 1.0 => 1,
                _ => continue,
            };
            bed.insert([x, z], SURFACE - depth);
        }
    }
    // Every water cell has a solid floor. The ring overlaps the deepest floor
    // at each side face and reaches at least the full source height.
    let is_source = |[x, z]: Tile| x % 2 == 0 && z % 2 == 0 || [x, z] == MOUTH;
    let sources: Vec<_> = bed.keys().copied().filter(|&p| is_source(p)).collect();
    let mut rim = BTreeSet::new();
    for (&[x, z], &floor) in &bed {
        s.foundation(b, x, z, floor, M::TurquoiseVeinedStone);
        if has_water {
            let [px, top, pz] = s.point(x, SURFACE, z);
            let [sx, sz] = sources
                .iter()
                .copied()
                .min_by_key(|&[sx, sz]| (sx - x).pow(2) + (sz - z).pow(2))
                .expect("the vent is a spring source");
            let [sx, _, sz] = s.point(sx, SURFACE, sz);
            debug_assert!((sx - px).abs().max((sz - pz).abs()) <= CALM_SOURCE_REACH);
            spring.push((
                [px, pz],
                Fall::calm_pool(s.base + floor + 1, top, [(sx - px) as i8, (sz - pz) as i8]),
            ));
        }
        let clear_to = SURFACE.max(s.floor(x, z) + 1).max(s.lava_top(x, z));
        for y in floor + 1..=clear_to {
            let material = if !has_water && y <= SURFACE {
                M::MineralEncrustedStone
            } else if has_water && y == SURFACE && is_source([x, z]) {
                M::FluidWater
            } else {
                M::Air
            };
            s.put(b, x, y, z, material);
        }
        for [dx, dz] in SIDES {
            let adjacent = [x + dx, z + dz];
            if !bed.contains_key(&adjacent) {
                rim.insert(adjacent);
            }
        }
    }
    for [x, z] in rim {
        let native = s.floor(x, z);
        let minimum = native.min(-VENT_DEPTH - 1);
        let top = (if x == 1 { 0 } else { SURFACE }).max(s.lava_top(x, z));
        for y in minimum..=top {
            s.put(b, x, y, z, M::VolcanicObsidian);
        }
        // A small rubble apron tapers raised sections into the native bed.
        for [dx, dz] in SIDES {
            let [ax, az] = [x + dx, z + dz];
            if ax >= FIRST_SPILLWAY_CELL || bed.contains_key(&[ax, az]) {
                continue;
            }
            let ground = s.floor(ax, az);
            for y in ground + 1..top.min(SURFACE) {
                b.entry(s.point(ax, y, az))
                    .or_insert(M::VolcanicGravel as u16);
            }
        }
    }
    if has_water {
        s.put(b, MOUTH[0], -VENT_DEPTH, MOUTH[1], M::GeyserVent);
    }
}

fn shrine(s: &Site, b: &mut Blocks, fitted: bool) {
    for z in -14_i64..=-5 {
        for x in -10_i64..=1 {
            let step = (-z - 5).clamp(0, 4);
            // Keep the existing monument supports and central stairs together.
            // The narrower approach leaves the outer slopes in place.
            let outer = !(-9..=0).contains(&x);
            let stair = (-7..=-3).contains(&x);
            let foundation = (-13..=-9).contains(&z) && !outer;
            let footing = matches!(
                (x, z),
                (-9, -12)
                    | (-6, -12)
                    | (-1, -12)
                    | (1, -10)
                    | (-10, -6)
                    | (0, -6)
                    | (-9, -7)
                    | (-1, -7)
            );
            if !fitted || stair || foundation || footing {
                s.cut(b, x, z, step, M::ScoriaBricks);
            }
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
    fn photographed_spillway_reaches_native_ground_and_has_one_connected_fall() {
        let biome_definitions = terra_core::BiomeDefinitions::load_directory(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/biome"),
        )
        .unwrap();
        let mut config = WorldConfig {
            seed: terra_core::WorldSeed(18_235_869_202_272_357_357),
            biome_definitions: Some(biome_definitions),
            ecology_revision: JOINED_SPILLWAY_REVISION,
            ..WorldConfig::default()
        };
        let terrain = TerraGenerator::new(config.clone()).unwrap();
        let generated = SectionGenerator::new(terrain.clone());
        config.ecology_revision = JOINED_SPILLWAY_REVISION - 1;
        let previous = SectionGenerator::new(TerraGenerator::new(config).unwrap());
        let at = |x: i64, y: i64, z: i64| [-9456 + z, 28 + y, -3824 - x];
        assert_eq!(
            terrain.natural_patch([-296, -120]).geyser,
            Some([-9453, 24, -3819])
        );
        assert_eq!(
            previous.block_at(at(15, 4, 1)).unwrap(),
            M::VolcanicObsidian as u16
        );
        assert_eq!(generated.block_at(at(15, 4, 1)).unwrap(), M::Air as u16);
        let caps: Vec<_> = (FIRST_SPILLWAY_CELL..=14)
            .map(|x| {
                (-4..=4)
                    .rev()
                    .find(|&y| {
                        (-2..=4).any(|z| generated.block_at(at(x, y, z)).unwrap() == M::Lava as u16)
                    })
                    .unwrap()
            })
            .collect();
        assert_eq!(caps[0], -2);
        assert_eq!(caps[12], 0);
        let mut falls = 0;
        for (i, caps) in caps.windows(2).enumerate() {
            let x = FIRST_SPILLWAY_CELL + i as i64;
            assert!((0..=2).contains(&(caps[1] - caps[0])));
            if caps[1] - caps[0] == 2 {
                falls += 1;
                assert!((-2..=4).any(|z| {
                    generated.block_at(at(x, caps[0], z)).unwrap() == M::Lava as u16
                        && (caps[0]..=caps[1])
                            .all(|y| generated.block_at(at(x + 1, y, z)).unwrap() == M::Lava as u16)
                        && generated.block_at(at(x, caps[0] + 1, z)).unwrap() == M::Air as u16
                }));
            }
        }
        assert_eq!(falls, 1);
    }

    #[test]
    fn photographed_spring_has_one_flat_surface_before_the_first_fluid_tick() {
        use crate::{
            edits::EditJournal,
            water::{FluidCell, fluid_cell_at},
        };
        use terra_voxel::global_to_section;

        let biome_definitions = terra_core::BiomeDefinitions::load_directory(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/biome"),
        )
        .unwrap();
        let terrain = TerraGenerator::new(WorldConfig {
            seed: terra_core::WorldSeed(18_235_869_202_272_357_357),
            biome_definitions: Some(biome_definitions),
            ecology_revision: TERRAIN_FITTED_REVISION,
            ..WorldConfig::default()
        })
        .unwrap();
        let patch = terrain.natural_patch([-296, -120]);
        let generator = SectionGenerator::new(terrain.clone());
        let top = 27;
        let mut owners = BTreeSet::new();
        let mut journal = EditJournal::default();
        assert!(patch.geothermal_spring.len() > 50);
        assert!(patch.geothermal_spring.len() > 2 * patch.objects.len());
        for &([x, z], span) in &patch.geothermal_spring {
            let p = [x, top, z];
            owners.insert(global_to_section(p).0);
            assert_eq!(i64::from(span.top), top);
            let material = generator.block_at(p).unwrap();
            assert_eq!(
                fluid_cell_at(&terrain, &journal, p, material),
                FluidCell {
                    amount: 8,
                    source: material == M::FluidWater as u16,
                    flow: [0, 0],
                }
            );
        }
        let mut triangles = BTreeMap::<Tile, usize>::new();
        for owner in owners {
            let prepared = generator.prepare_column([owner.x, owner.z], true).unwrap();
            let section = generator
                .build_prepared(owner, 0, &Default::default(), &prepared)
                .unwrap();
            let origin = owner.origin().unwrap();
            let water = section.water.expect("spring mesh before simulation");
            for triangle in water.indices.chunks_exact(3) {
                let vertices =
                    [triangle[0], triangle[1], triangle[2]].map(|i| water.positions[i as usize]);
                if vertices
                    .iter()
                    .all(|v| v[1] + origin[1] as f32 == (top + 1) as f32)
                {
                    let tile = [0, 2].map(|axis| {
                        origin[axis]
                            + vertices
                                .iter()
                                .map(|v| v[axis])
                                .fold(f32::INFINITY, f32::min) as i64
                    });
                    *triangles.entry(tile).or_default() += 1;
                }
            }
        }
        for &(tile, _) in &patch.geothermal_spring {
            assert_eq!(
                triangles.get(&tile),
                Some(&2),
                "open or duplicate surface at {tile:?}"
            );
        }

        let &([x, z], span) = patch
            .geothermal_spring
            .iter()
            .find(|(_, s)| s.source != [0, 0])
            .unwrap();
        let p = [x, top, z];
        let source = [
            x + i64::from(span.source[0]),
            top,
            z + i64::from(span.source[1]),
        ];
        let cell = FluidCell {
            amount: 4,
            flow: [1, 0],
            source: false,
        };
        journal.set_fluid(p, cell).unwrap();
        assert_eq!(fluid_cell_at(&terrain, &journal, p, 0).amount, 8);
        journal.record(p, M::Air as u16).unwrap();
        journal.set_fluid(p, FluidCell::default()).unwrap();
        assert_eq!(fluid_cell_at(&terrain, &journal, p, 0).amount, 0);

        let mut drained = EditJournal::default();
        drained.record(source, M::Air as u16).unwrap();
        assert_eq!(fluid_cell_at(&terrain, &drained, p, 0).amount, 0);
    }

    #[test]
    fn spring_initializes_persistent_water_sources_and_the_fall_stays_joined() {
        let t = TerraGenerator::new(WorldConfig {
            ecology_revision: 24,
            ..WorldConfig::default()
        })
        .unwrap();
        let g = SectionGenerator::new(t.clone());
        let tile = [74, -146];
        let patch = t.natural_patch(tile);
        let vent = [2389, 26, -4659];
        assert_eq!(g.block_at(vent).unwrap(), M::GeyserVent as u16);
        assert_eq!(g.block_at([vent[0], vent[1] + 1, vent[2]]).unwrap(), 0);
        assert_eq!(g.block_at([2389, 29, -4659]).unwrap(), M::FluidWater as u16);
        assert_eq!(g.block_at([2389, 30, -4659]).unwrap(), 0);
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
        old.ecology_revision = SUBMERGED_ECOLOGY_REVISION - 1;
        let legacy = SectionGenerator::new(TerraGenerator::new(old.clone()).unwrap());
        assert_eq!(
            legacy.block_at([2389, 30, -4659]).unwrap(),
            M::GeyserVent as u16
        );
        assert_eq!(
            legacy.block_at([2389, 29, -4659]).unwrap(),
            M::VolcanicObsidian as u16
        );
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

    #[test]
    fn spring_water_stays_sealed_and_the_spillway_preserves_more_native_rock() {
        use crate::water::can_hold_fluid;
        use std::collections::{HashSet, VecDeque};

        let biome_definitions = terra_core::BiomeDefinitions::load_directory(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/biome"),
        )
        .unwrap();
        let cases = [
            (WorldConfig::default(), [74, -146]),
            (
                WorldConfig {
                    seed: terra_core::WorldSeed(10_727_036_740_062_400_187),
                    biome_definitions: Some(biome_definitions.clone()),
                    ..WorldConfig::default()
                },
                [26, 213],
            ),
            (
                WorldConfig {
                    seed: terra_core::WorldSeed(10_727_036_740_062_400_187),
                    biome_definitions: Some(biome_definitions),
                    ..WorldConfig::default()
                },
                [36, 213],
            ),
        ];
        for (config, tile) in cases {
            let mut old = config.clone();
            old.ecology_revision = 24;
            let legacy = TerraGenerator::new(old).unwrap();
            let terrain = TerraGenerator::new(config).unwrap();
            let current = terrain.natural_patch(tile);
            let previous = legacy.natural_patch(tile);
            let vent = current.geyser.expect("geothermal spring");
            assert_eq!(Some(vent), previous.geyser);

            // Flood the real voxel neighbourhood from the loaded water surface.
            // Positive hydraulic head is unavailable, so only faces at or below
            // the crest can participate. The lowest cell is the basalt floor.
            let generator = SectionGenerator::new(terrain.clone());
            let [vx, vy, vz] = vent;
            let crest = vy + VENT_DEPTH - 1;
            let mut seen = HashSet::new();
            let mut queue = VecDeque::from([[vx, crest, vz]]);
            let mut wet = HashSet::new();
            while let Some(p) = queue.pop_front() {
                if !seen.insert(p) {
                    continue;
                }
                let material = generator.block_at(p).unwrap();
                assert_ne!(material, M::Lava as u16, "water touches lava {p:?}");
                if !can_hold_fluid(material) {
                    continue;
                }
                assert!(
                    p[1] > vy && (p[0] - vx).abs() <= 8 && (p[2] - vz).abs() <= 8,
                    "leak in {tile:?} through {p:?}"
                );
                wet.insert(p);
                queue.extend(
                    [
                        [1, 0, 0],
                        [-1, 0, 0],
                        [0, 1, 0],
                        [0, -1, 0],
                        [0, 0, 1],
                        [0, 0, -1],
                    ]
                    .map(|delta| std::array::from_fn(|i| p[i] + delta[i]))
                    .into_iter()
                    .filter(|p| p[1] <= crest),
                );
            }
            assert!(wet.len() > 100, "the spring should retain a covered basin");
            assert!(
                current
                    .objects
                    .iter()
                    .all(|source| wet.contains(&source.position))
            );

            #[cfg(feature = "bevy")]
            if tile == [36, 213] {
                use crate::{
                    edits::EditJournal,
                    streaming::{ChunkStore, LoadedSection},
                    water::simulate_fluid_step,
                };
                use std::sync::Arc;
                use terra_voxel::SectionPos;

                let mut chunks = ChunkStore::default();
                for sx in (vx - 10).div_euclid(16)..=(vx + 10).div_euclid(16) {
                    for sz in (vz - 10).div_euclid(16)..=(vz + 10).div_euclid(16) {
                        let coordinate = [sx, sz];
                        let column = Arc::new(generator.prepare_column(coordinate, false).unwrap());
                        chunks.set_prepared_column_for_test(coordinate, Some(column));
                        for sy in (vy - 3).div_euclid(16)..=(crest + 3).div_euclid(16) {
                            let owner = SectionPos::new(sx, sy, sz);
                            chunks.loaded.insert(
                                owner,
                                LoadedSection {
                                    data: generator.generate(owner).unwrap(),
                                    mesh: None,
                                    mesh_epoch: 0,
                                    water_epoch: 0,
                                    water: None,
                                    props: Vec::new(),
                                },
                            );
                        }
                    }
                }
                let mut journal = EditJournal::default();
                journal.initialize_structures(&current.objects).unwrap();
                let mut simulated = BTreeSet::new();
                for _ in 0..36 {
                    simulate_fluid_step(&terrain, &chunks, &mut journal, 256).unwrap();
                    simulated.extend(
                        journal
                            .fluid_cells()
                            .filter_map(|(p, water)| (water.amount > 0).then_some(p)),
                    );
                }
                assert_eq!(simulated.len(), current.objects.len());
                assert!(current.geothermal_spring.len() > simulated.len());
                assert!(
                    simulated.iter().all(|p| wet.contains(p)),
                    "fluid left the sealed basin: {:?}",
                    simulated.iter().find(|p| !wet.contains(*p))
                );
            }

            let center = tile.map(|v| v * TILE_SIZE + 16);
            let key = ticket(terrain.config().seed.0, tile.map(|v| v.div_euclid(3)), 8422);
            let local = |[x, _, z]: Cell| match (key >> 20) % 4 {
                1 => z - center[1],
                2 => center[0] - x,
                3 => center[1] - z,
                _ => x - center[0],
            };
            let altered_spillway = |patch: &super::super::NaturalPatch| {
                patch.cells.iter().filter(|&&(p, _)| local(p) >= 2).count()
            };
            assert!(altered_spillway(&current) * 2 < altered_spillway(&previous));
            for run in 2..=14 {
                assert!(
                    current
                        .cells
                        .iter()
                        .any(|&(p, raw)| local(p) == run && raw == M::Lava as u16)
                );
            }
        }
    }

    #[cfg(feature = "bevy")]
    #[test]
    fn generated_geyser_has_water_over_the_mouth_on_first_publication_and_drains() {
        use crate::{
            edits::EditJournal,
            streaming::{ChunkStore, LoadedSection},
            volcano::geyser::submerged,
            water::FLUID_CAPACITY,
        };
        let terrain = TerraGenerator::new(WorldConfig::default()).unwrap();
        let generator = SectionGenerator::new(terrain.clone());
        let vent = [2389, 26, -4659];
        let mouth = [2389, 27, -4659];
        let surface = [2389, 29, -4659];
        let owner = terra_voxel::global_to_section(vent).0;
        let coordinate = [owner.x, owner.z];
        let prepared = std::sync::Arc::new(generator.prepare_column(coordinate, false).unwrap());
        let mut chunks = ChunkStore::default();
        let mut journal = EditJournal::default();
        chunks.loaded.insert(
            owner,
            LoadedSection {
                data: generator.generate(owner).unwrap(),
                mesh: None,
                mesh_epoch: 0,
                water_epoch: 0,
                water: None,
                props: Vec::new(),
            },
        );
        chunks.set_prepared_column_for_test(coordinate, Some(prepared.clone()));
        assert!(submerged(&chunks, &journal, &terrain, vent));
        assert_eq!(
            chunks.fluid_cell_at(&terrain, &journal, mouth, 0).amount,
            FLUID_CAPACITY
        );
        assert!(prepared.geothermal_water([2389.5, 29.5, -4658.5]));
        journal
            .initialize_structures(&prepared.structure_objects(owner))
            .unwrap();
        assert!(journal.fluid(surface).is_some_and(|water| water.source));
        assert!(submerged(&chunks, &journal, &terrain, vent));
        journal.record(surface, M::Air as u16).unwrap();
        assert!(!submerged(&chunks, &journal, &terrain, vent));
    }
}
