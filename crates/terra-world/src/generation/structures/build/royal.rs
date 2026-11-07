//! Revision-six authored strongholds: all geometry becomes ordinary streamed voxels.
use super::*;
use terra_core::{beds::DyeColor, castle};

mod buildings;
mod fortifications;
mod landscape;
#[cfg(test)]
mod tests;
mod vegetation;

/// Only existing settlement slots can become strongholds; other seeded kinds stay put.
pub(in crate::generation::structures) fn selected(seed: u32) -> bool {
    seed % 10 != 9 && seed % 9 <= 3 && !(seed >> 8).is_multiple_of(4)
}

pub(super) fn plan(terrain: &TerraGenerator, region: [i64; 2], seed: u32) -> Option<Site> {
    let outpost = (seed >> 8) % 4 == 1;
    let settings = terrain.config().structures;
    let enabled = if terrain.config().structures_revision >= 7 {
        if outpost {
            settings.outposts
        } else {
            settings.kingdoms
        }
    } else {
        settings.villages
    };
    if !enabled {
        return None;
    }
    let x = region[0] * REGION_SIZE + 192;
    let z = region[1] * REGION_SIZE + 192;
    let biome = terrain.biome_at(x as f64, z as f64);
    if biome.weights.ocean > 0.25 || biome.weights.desert + biome.weights.cliffted_desert > 0.5 {
        return None;
    }
    let y = (terrain.surface_height(x as f64 + 0.5, z as f64 + 0.5) - 0.5).floor() as i64 + 1;
    let winter = !outpost
        && (biome.weights.highland > 0.18 || y as f64 >= terrain.config().snow_height - 12.0);
    let half = if outpost { 31 } else { 73 };
    let mut low = y;
    let mut high = y;
    for dz in [-half, 0, half] {
        for dx in [-half, 0, half] {
            let column = terrain.surface_column((x + dx) as f64 + 0.5, (z + dz) as f64 + 0.5);
            let h = (column.height - 0.5).floor() as i64 + 1;
            if column.water_level.is_some_and(|w| w > h as f64 + 0.1) {
                return None;
            }
            low = low.min(h);
            high = high.max(h);
        }
    }
    if high - low
        > if winter {
            56
        } else if outpost {
            18
        } else {
            30
        }
        || !(-80..=280).contains(&y)
    {
        return None;
    }
    let mut builder = Builder {
        terrain,
        summary: SiteSummary {
            id: (u64::from(seed) << 32) ^ region[0] as u64 ^ (region[1] as u64).rotate_left(17),
            kind: if outpost {
                StructureKind::Outpost
            } else {
                StructureKind::Kingdom
            },
            style: if winter {
                VillageStyle::Highland
            } else {
                VillageStyle::Temperate
            },
            center: [x, y, z],
            bounds: [[0; 3]; 2],
            buildings: Vec::new(),
            containers: Vec::new(),
        },
        patches: BTreeMap::new(),
        objects: BTreeMap::new(),
        writes: 0,
        failed: false,
        seed,
    };
    let mut royal = Royal {
        builder: &mut builder,
        origin: [x, y, z],
        winter,
        outpost,
    };
    royal.terrain();
    if outpost {
        royal.outpost();
    } else {
        royal.kingdom();
    }
    royal.plant_courtyard_trees();
    let marker = if outpost {
        [0, 3, -14]
    } else {
        [0, royal.floor(0, -40), -47]
    };
    royal.set(marker, castle::BERGFRIED);
    royal.builder.summary.center = royal.point(marker);
    let site = builder.finish()?;
    if terrain.config().structures_revision >= 9
        && !super::super::distribution::within_royal_region(region, site.summary.bounds)
    {
        return None;
    }
    Some(site)
}

struct Royal<'a, 'b> {
    builder: &'a mut Builder<'b>,
    origin: [i64; 3],
    winter: bool,
    outpost: bool,
}
impl Royal<'_, '_> {
    fn adapted(&self) -> bool {
        self.builder.terrain.config().structures_revision >= 7
    }
    fn point(&self, [x, y, z]: [i64; 3]) -> [i64; 3] {
        [self.origin[0] + x, self.origin[1] + y, self.origin[2] + z]
    }
    /// The heightfield can have an open cave beneath it. A joining road needs
    /// actual support and headroom, without recursively consulting its own plan.
    fn natural_walk_height(&self, x: i64, z: i64) -> Option<i64> {
        let position = self.point([x, 0, z]);
        let column = self
            .builder
            .terrain
            .surface_column(position[0] as f64 + 0.5, position[2] as f64 + 0.5);
        let nominal = (column.height - 0.5).floor() as i64 + 1;
        let sample = |y: i64| {
            self.builder.terrain.sample_with_column(
                [
                    position[0] as f64 + 0.5,
                    y as f64 + 0.5,
                    position[2] as f64 + 0.5,
                ],
                column,
            )
        };
        for feet in (nominal - 24..=nominal + 2).rev() {
            if column.water_level.is_some_and(|water| water > feet as f64) {
                continue;
            }
            let below = sample(feet - 1);
            if below.density <= 0.0
                && crate::props::placement::ground_support(below.material)
                && sample(feet).density > 0.0
                && sample(feet + 1).density > 0.0
            {
                return Some(feet - self.origin[1]);
            }
        }
        None
    }
    fn set(&mut self, p: [i64; 3], material: u16) {
        let p = self.point(p);
        self.builder.set(p, material);
    }
    fn fill(&mut self, a: [i64; 3], b: [i64; 3], material: u16) {
        let a = self.point(a);
        let b = self.point(b);
        self.builder.fill(a, b, material);
    }
    fn state(&mut self, p: [i64; 3], material: u16, facing: u8) {
        let p = self.point(p);
        self.builder.state(p, material, facing);
    }
    fn paired(&mut self, a: [i64; 3], b: [i64; 3], material: u16, facing: u8) {
        let a = self.point(a);
        let b = self.point(b);
        for (p, peer, secondary) in [(a, b, false), (b, a, true)] {
            self.builder.object(
                p,
                material,
                Some(BlockObjectState {
                    facing,
                    peer: Some(peer),
                    secondary,
                    ..Default::default()
                }),
                None,
            );
        }
    }
    fn floor(&self, x: i64, z: i64) -> i64 {
        if self.outpost {
            return if x * x + (z + 13) * (z + 13) < 90 {
                3
            } else {
                0
            };
        }
        if self.winter {
            if x.abs() <= 5 && (-28..=0).contains(&z) {
                (-z).min(14)
            } else if z <= -24 {
                14
            } else if z <= -8 {
                7
            } else {
                0
            }
        } else if x.abs() <= 26 && (-64..=-16).contains(&z) {
            5
        } else if x.abs() <= 4 && (-15..=-5).contains(&z) {
            (-z - 4) / 2
        } else {
            0
        }
    }
    fn roof(&self) -> u16 {
        terra_core::textiles::dyed(
            103,
            if self.winter {
                DyeColor::Red
            } else {
                DyeColor::Blue
            },
        )
        .unwrap()
    }
    fn masonry(&self, y: i64) -> u16 {
        if y.rem_euclid(4) == 0 {
            castle::STONE_WALL
        } else {
            43
        }
    }
    fn terrain(&mut self) {
        let radius = if self.outpost { 31 } else { 72 };
        for z in -radius..=radius {
            for x in -radius..=radius {
                if x * x + z * z > radius * radius {
                    continue;
                }
                let floor = self.floor(x, z);
                let p = self.point([x, 0, z]);
                let natural = self.builder.ground(p[0], p[2]) - self.origin[1];
                self.fill(
                    [x, natural.min(floor - 1) - 2, z],
                    [x, floor - 1, z],
                    if self.winter || floor > 0 { 43 } else { 2 },
                );
                self.fill([x, floor, z], [x, (natural + 7).max(floor + 7), z], 0);
                let paving = if self.outpost {
                    if floor > 0 {
                        43
                    } else if (x.abs() <= 2 && (-7..=28).contains(&z))
                        || ((z - 7).abs() <= 1 && (-17..=1).contains(&x))
                        || ((z - 16).abs() <= 1 && (0..=18).contains(&x))
                        || ((z - 18).abs() <= 1 && (-14..=0).contains(&x))
                        || ((z - 1).abs() <= 1 && (0..=14).contains(&x))
                        || ((x - 13).abs() <= 1 && (-5..=1).contains(&z))
                    {
                        PATH
                    } else {
                        1
                    }
                } else if x.abs() <= 5
                    || (z - 16).abs() <= 2
                    || (z - 32).abs() <= 2
                    || (z - 52).abs() <= 2
                {
                    43
                } else if self.winter {
                    5
                } else {
                    1
                };
                self.set([x, floor - 1, z], paving);
            }
        }
        if self.adapted() {
            self.blend_exterior();
        }
        // Continuous, one-block risers keep raised wards accessible on foot.
        if !self.outpost {
            for z in -28..=55 {
                for x in -2..=2 {
                    let y = self.floor(x, z);
                    if self.floor(x, z) != self.floor(x, z + 1) {
                        self.state([x, y - 1, z], 77, 2);
                    } else {
                        self.set([x, y - 1, z], 43);
                    }
                }
            }
        }
    }
    fn foundation(&mut self, x: i64, z: i64, w: i64, d: i64, y: i64) {
        for dz in 0..=d {
            for dx in 0..=w {
                let p = self.point([x + dx, 0, z + dz]);
                let natural = self.builder.ground(p[0], p[2]) - self.origin[1];
                self.fill(
                    [x + dx, natural.min(y - 1) - 2, z + dz],
                    [x + dx, y - 2, z + dz],
                    43,
                );
            }
        }
    }
    fn access(&mut self, door: [i64; 3]) {
        let [x, y, z] = door;
        self.foundation(x - 1, z + 1, 2, 2, y);
        self.fill([x - 1, y - 1, z + 1], [x + 1, y - 1, z + 3], 43);
        self.fill([x - 1, y, z + 1], [x + 1, y + 2, z + 3], 0);
        let end_z = z + 3;
        let direction = if x < 0 { -1 } else { 1 };
        let ground = self.floor(x + direction * 12, end_z);
        for step in 1..=(y - ground).clamp(0, 24) {
            let sx = x + direction * step;
            let sy = y - step;
            self.foundation(sx, end_z, 0, 0, sy + 1);
            self.fill([sx, sy + 1, end_z], [sx, sy + 4, end_z], 0);
            self.state([sx, sy, end_z], 77, if direction < 0 { 1 } else { 3 });
        }
    }
    fn summary(&mut self, role: BuildingRole, entrance: [i64; 3], low: [i64; 3], high: [i64; 3]) {
        let entrance = self.point(entrance);
        let low = self.point(low);
        let high = self.point(high);
        self.builder.summary.buildings.push(BuildingSummary {
            role,
            entrance,
            bounds: [low, high],
        });
    }
    fn chest(&mut self, p: [i64; 3], role: BuildingRole) {
        let p = self.point(p);
        self.builder.object(
            p,
            29,
            Some(BlockObjectState::default()),
            Some(loot::inventory(
                role,
                self.builder.seed ^ (p[0] as u32).rotate_left(8) ^ p[2] as u32,
            )),
        );
        self.builder.summary.containers.push(p);
    }
    fn bed(&mut self, x: i64, y: i64, z: i64) {
        self.paired(
            [x, y, z],
            [x, y, z + 1],
            if self.winter { 73 } else { 71 },
            0,
        );
    }
    fn light(&mut self, x: i64, y: i64, z: i64) {
        self.fill([x, y, z], [x, y + 2, z], 90);
        self.set([x, y + 3, z], 40);
    }
    fn banner(&mut self, x: i64, y: i64, z: i64) {
        self.set([x, y, z], castle::ROYAL_WALL);
    }
    fn kingdom(&mut self) {
        self.outer_wall(72, 8);
        for (x, z) in [
            (-51, -51),
            (0, -71),
            (51, -51),
            (-71, 0),
            (71, 0),
            (-51, 51),
            (51, 51),
        ] {
            let y = self.floor(x, z);
            self.tower(x, z, y, 5, 15, true);
        }
        self.gate(0, 70, 0, 5);
        self.inner_ward();
        let civilian_count = 50 + (self.builder.seed % 31) as usize;
        let military_count = 26 + ((self.builder.seed >> 7) % 39) as usize;
        let mut remaining = civilian_count;
        for z in [8, 24, 40] {
            for x in [-48, -32, -16, 0, 16, 32, 48] {
                if x == 0 || (z == 8 && x == -32) {
                    continue;
                }
                let beds = remaining.min(4);
                remaining -= beds;
                self.house(
                    x,
                    z,
                    self.floor(x, z),
                    9,
                    9,
                    5,
                    BuildingRole::House,
                    beds,
                    0,
                );
            }
        }
        for (x, z) in [(16, 56), (-16, 56), (16, -4)] {
            let beds = remaining.min(4);
            remaining -= beds;
            self.house(
                x,
                z,
                self.floor(x, z),
                9,
                9,
                5,
                BuildingRole::House,
                beds,
                0,
            );
        }
        debug_assert_eq!(remaining, 0);
        let first = military_count.min(32);
        self.house(
            -48,
            -14,
            self.floor(-48, -14),
            15,
            21,
            5,
            BuildingRole::Barracks,
            first,
            2,
        );
        self.house(
            -31,
            3,
            self.floor(-31, 3),
            15,
            17,
            5,
            BuildingRole::Barracks,
            military_count - first,
            2,
        );
        self.house(
            31,
            -4,
            self.floor(31, -4),
            11,
            11,
            5,
            BuildingRole::Smith,
            0,
            0,
        );
        self.church(46, -16, self.floor(46, -16));
        self.market();
        self.farm(-60, 20, 6, 12);
        self.farm(60, 20, 6, 12);
        if self.adapted() {
            self.farm(-68, 78, 20, 20);
        } else {
            self.farm(-58, 74, 20, 20);
        }
        self.windmill(
            -80,
            42,
            if self.adapted() {
                self.exterior_floor(-80, 42)
            } else {
                self.floor(-80, 42)
            },
        );
        for x in [-10, 10] {
            for z in [0, 18, 34, 52] {
                self.light(x, self.floor(x, z), z);
            }
        }
    }
    fn outpost(&mut self) {
        // Irregular-height timber palisade, with a real open gate at the south.
        for z in -30_i64..=30 {
            for x in -30_i64..=30 {
                let r = x * x + z * z;
                if (28 * 28..=30 * 30).contains(&r) && !(z > 24 && x.abs() < 5) {
                    let top = 4 + (x + z).rem_euclid(2);
                    self.fill([x, 0, z], [x, top, z], 36);
                    self.state([x, top + 1, z], 84, 0);
                    if x.rem_euclid(9) == 0 || z.rem_euclid(9) == 0 {
                        self.banner(x, 2, z);
                    }
                }
            }
        }
        self.tower(0, -14, 3, 6, 14, true);
        self.gate(0, 28, 0, 3);
        let soldiers = 4 + ((self.builder.seed >> 7) % 7) as usize;
        self.house(-15, -2, 0, 15, 11, 5, BuildingRole::Barracks, soldiers, 0);
        self.stable(16, 11, 0);
        self.tent(13, -9, 0, 7, 9);
        self.tent(-13, 15, 0, 7, 7);
        self.farm(-21, 15, 7, 12);
        // A shallow staircase rises to the central watchtower's stone plinth.
        for z in -5..=0 {
            let y = (-z + 1) / 2;
            self.fill([-2, y - 1, z], [2, y - 1, z], 43);
            self.fill([-2, y, z], [2, y + 3, z], 0);
        }
        for x in -3_i64..=3 {
            for z in 6_i64..=12 {
                if x * x + (z - 9) * (z - 9) <= 9 {
                    self.set([x, 0, z], 43);
                }
            }
        }
        self.set([0, 0, 9], terra_core::campfire::MATERIAL);
        self.summary(BuildingRole::Market, [0, 0, 12], [-3, -1, 6], [3, 2, 12]);
    }
}
