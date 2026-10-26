use super::*;
use terra_core::CropKind;

const PATH: u16 = 102;
const WOOL: u16 = 103;
mod village;

pub(super) struct Builder<'a> {
    pub(super) terrain: &'a TerraGenerator,
    pub(super) summary: SiteSummary,
    patches: BTreeMap<SectionPos, Box<[u16; 4096]>>,
    objects: BTreeMap<[i64; 3], StructureObject>,
    writes: usize,
    failed: bool,
    pub(super) seed: u32,
}
impl Builder<'_> {
    pub(super) fn set(&mut self, p: [i64; 3], material: u16) {
        if p[1] < -120 || p[1] > 350 || self.writes > MAX_SITE_CELLS * 6 {
            self.failed = true;
            return;
        }
        let (owner, local) = global_to_section(p);
        if self.patches.len() >= MAX_SITE_SECTIONS && !self.patches.contains_key(&owner) {
            self.failed = true;
            return;
        }
        self.patches
            .entry(owner)
            .or_insert_with(|| Box::new([u16::MAX; 4096]))
            [local[0] + 16 * (local[1] + 16 * local[2])] = material;
        self.writes += 1;
        self.objects.remove(&p);
    }
    pub(super) fn fill(&mut self, min: [i64; 3], max: [i64; 3], material: u16) {
        for z in min[2]..=max[2] {
            for y in min[1]..=max[1] {
                for x in min[0]..=max[0] {
                    self.set([x, y, z], material);
                }
            }
        }
    }
    pub(super) fn object(
        &mut self,
        p: [i64; 3],
        material: u16,
        state: Option<BlockObjectState>,
        loot: Option<Inventory>,
    ) {
        self.set(p, material);
        self.objects.insert(
            p,
            StructureObject {
                position: p,
                material,
                state,
                loot,
                crop: None,
            },
        );
    }
    pub(super) fn state(&mut self, p: [i64; 3], material: u16, facing: u8) {
        self.object(
            p,
            material,
            Some(BlockObjectState {
                facing,
                ..Default::default()
            }),
            None,
        );
    }
    pub(super) fn ground(&self, x: i64, z: i64) -> i64 {
        (self.terrain.surface_height(x as f64 + 0.5, z as f64 + 0.5) - 0.5).floor() as i64 + 1
    }
    fn fit(&self, x: i64, z: i64, w: i64, d: i64) -> Option<i64> {
        let mut low = i64::MAX;
        let mut high = i64::MIN;
        let mut supports = Vec::with_capacity(((w + 1) * (d + 1)) as usize);
        for dz in 0..=d {
            for dx in 0..=w {
                let px = x + dx;
                let pz = z + dz;
                let column = self
                    .terrain
                    .surface_column(px as f64 + 0.5, pz as f64 + 0.5);
                let y = (column.height - 0.5).floor() as i64 + 1;
                low = low.min(y);
                high = high.max(y);
                if column
                    .water_level
                    .is_some_and(|level| level > y as f64 + 0.2)
                {
                    return None;
                }
                supports.push((px, pz, column));
            }
        }
        (high - low
            <= if self.summary.style == VillageStyle::Highland {
                12
            } else {
                8
            }
            && (-90..=310).contains(&high)
            && supports
                .into_iter()
                .all(|(px, pz, column)| foundation_supported(self.terrain, px, pz, high, column)))
        .then_some(high)
    }
    fn palette(&self) -> (u16, u16, u16, u16, u16, u16, u16) {
        match self.summary.style {
            VillageStyle::Desert => (11, 11, 19, 65, 68, 74, 90),
            VillageStyle::Highland => (3, WOOL, 18, 67, 70, 76, 92),
            VillageStyle::Temperate => (43, WOOL, 16, 65, 68, 74, 90),
        }
    }
    fn building(
        &mut self,
        x: i64,
        z: i64,
        w: i64,
        d: i64,
        h: i64,
        role: BuildingRole,
    ) -> Option<[i64; 3]> {
        let y = self.fit(x, z, w, d)?;
        let (stone, wall, roof, door, window, stairs, fence) = self.palette();
        self.fill([x - 1, y, z - 1], [x + w + 1, y + h + 9, z + d + 1], 0);
        for dz in 0..=d {
            for dx in 0..=w {
                let natural = self.ground(x + dx, z + dz);
                self.fill(
                    [x + dx, natural.min(y - 1) - 1, z + dz],
                    [x + dx, y - 1, z + dz],
                    stone,
                );
            }
        }
        self.fill(
            [x, y - 1, z],
            [x + w, y - 1, z + d],
            if self.summary.style == VillageStyle::Desert {
                11
            } else {
                roof
            },
        );
        self.fill([x, y, z], [x + w, y + h - 1, z + d], wall);
        self.fill([x + 1, y, z + 1], [x + w - 1, y + h - 1, z + d - 1], 0);
        if self.summary.style != VillageStyle::Desert {
            for dx in [0, w] {
                for dz in [0, d] {
                    self.fill([x + dx, y, z + dz], [x + dx, y + h, z + dz], roof);
                }
            }
            self.fill([x, y + h - 1, z], [x + w, y + h - 1, z], roof);
            self.fill([x, y + h - 1, z + d], [x + w, y + h - 1, z + d], roof);
            let center = w / 2;
            for dx in -1..=w + 1 {
                let rise = (center - (dx - center).abs() + 1).max(0);
                let ry = y + h + rise;
                self.fill([x + dx, ry, z - 1], [x + dx, ry, z + d + 1], roof);
                if (0..=w).contains(&dx) && rise > 0 {
                    for edge in [z, z + d] {
                        self.fill([x + dx, y + h, edge], [x + dx, ry - 1, edge], wall);
                    }
                }
                if dx < center {
                    for dz in -1..=d + 1 {
                        self.state([x + dx, ry, z + dz], stairs, 1);
                    }
                } else if dx > center {
                    for dz in -1..=d + 1 {
                        self.state([x + dx, ry, z + dz], stairs, 3);
                    }
                }
            }
        } else {
            self.fill([x - 1, y + h, z - 1], [x + w + 1, y + h, z + d + 1], 11);
            for dx in -1..=w + 1 {
                for dz in [-1, d + 1] {
                    if (dx + self.seed as i64).rem_euclid(3) != 0 {
                        self.set([x + dx, y + h + 1, z + dz], 11);
                    }
                }
            }
            for dz in 0..=d {
                for dx in [-1, w + 1] {
                    if dz % 3 != 0 {
                        self.set([x + dx, y + h + 1, z + dz], 11);
                    }
                }
            }
        }
        let entrance = [x + w / 2, y, z];
        self.object(
            entrance,
            door,
            Some(BlockObjectState {
                peer: Some([entrance[0], y + 1, z]),
                ..Default::default()
            }),
            None,
        );
        self.object(
            [entrance[0], y + 1, z],
            door,
            Some(BlockObjectState {
                peer: Some(entrance),
                secondary: true,
                ..Default::default()
            }),
            None,
        );
        for dx in [2, w - 2] {
            if dx == w / 2 {
                continue;
            }
            self.state([x + dx, y + 2, z], window, 0);
            self.state([x + dx, y + 2, z + d], window, 0);
        }
        for dz in [2, d - 2] {
            self.state([x, y + 2, z + dz], window, 1);
            self.state([x + w, y + 2, z + dz], window, 1);
        }
        if matches!(
            role,
            BuildingRole::House | BuildingRole::Medic | BuildingRole::Church
        ) {
            // White wool furnishings share the ordinary shaped-block pipeline.
            for dx in 2..=w - 2 {
                self.state([x + dx, y, z + d - 1], 105, 0);
            }
            if role == BuildingRole::Church {
                self.fill([x + 3, y, z + d - 3], [x + w - 3, y + 1, z + d - 2], WOOL);
                self.state([x + w / 2, y, z + d - 4], 104, 0);
            }
        }
        let chest = [x + w - 2, y, z + d - 2];
        self.object(
            chest,
            29,
            Some(BlockObjectState::default()),
            Some(loot::inventory(
                role,
                self.seed ^ (x as u32).rotate_left(9) ^ z as u32,
            )),
        );
        self.summary.containers.push(chest);
        for p in [[x + 1, y, z + 1], [x + w - 1, y, z + 1]] {
            self.set(p, 40);
        }
        if matches!(role, BuildingRole::House | BuildingRole::Medic) {
            let bed = [x + 2, y, z + d - 3];
            let head = [bed[0], y, bed[2] + 1];
            let material = if self.summary.style == VillageStyle::Highland {
                73
            } else {
                71
            };
            for (p, peer, secondary) in [(bed, head, false), (head, bed, true)] {
                self.object(
                    p,
                    material,
                    Some(BlockObjectState {
                        peer: Some(peer),
                        secondary,
                        ..Default::default()
                    }),
                    None,
                );
            }
        }
        if role == BuildingRole::Smith {
            let furnace = [x + 2, y, z + d - 2];
            self.object(
                furnace,
                59,
                Some(BlockObjectState {
                    peer: Some([furnace[0], y + 1, furnace[2]]),
                    ..Default::default()
                }),
                None,
            );
            self.object(
                [furnace[0], y + 1, furnace[2]],
                59,
                Some(BlockObjectState {
                    peer: Some(furnace),
                    secondary: true,
                    ..Default::default()
                }),
                None,
            );
            for chimney_y in y + 2..=y + h + 7 {
                self.state([furnace[0], chimney_y, furnace[2]], 60, 0);
            }
            let bench = [x + w / 2, y, z + d - 2];
            let other = [bench[0] + 1, y, bench[2]];
            self.object(
                bench,
                28,
                Some(BlockObjectState {
                    peer: Some(other),
                    ..Default::default()
                }),
                None,
            );
            self.object(
                other,
                28,
                Some(BlockObjectState {
                    peer: Some(bench),
                    secondary: true,
                    ..Default::default()
                }),
                None,
            );
            self.fill([x - 3, y - 1, z + 1], [x - 1, y - 1, z + d - 1], stone);
            self.fill([x - 3, y, z + 1], [x - 1, y + 2, z + d - 1], 0);
            self.fill([x - 3, y + 3, z + 1], [x - 1, y + 3, z + d - 1], roof);
            self.set([x - 3, y, z + 1], fence);
        }
        if matches!(role, BuildingRole::Church | BuildingRole::Watch) {
            let tx = x + w - 3;
            let tz = z + d - 3;
            let top = y + h + 7;
            self.fill([tx, y + h, tz], [tx + 3, top, tz + 3], stone);
            self.fill([tx + 1, y + h, tz + 1], [tx + 2, top, tz + 2], 0);
            self.fill([tx, top + 1, tz], [tx + 3, top + 1, tz + 3], roof);
            for (p, facing) in [
                ([tx + 1, top - 1, tz], 0),
                ([tx + 1, top - 1, tz + 3], 0),
                ([tx, top - 1, tz + 1], 1),
                ([tx + 3, top - 1, tz + 1], 1),
            ] {
                self.state(p, window, facing);
            }
            if self.summary.style == VillageStyle::Desert {
                self.fill([tx, top + 2, tz], [tx + 3, top + 2, tz + 3], 19);
                self.fill([tx + 1, top + 3, tz + 1], [tx + 2, top + 3, tz + 2], 19);
            } else {
                self.fill([tx + 1, top + 2, tz], [tx + 2, top + 2, tz + 3], roof);
            }
            if role == BuildingRole::Church {
                self.fill([tx + 1, top + 3, tz + 1], [tx + 1, top + 6, tz + 1], roof);
                self.fill([tx, top + 5, tz + 1], [tx + 2, top + 5, tz + 1], roof);
            }
            // Climbable voxel stair spiral to an accessible lookout, not a sealed prop tower.
            for level in 0..h + 6 {
                let cycle = level % 4;
                let (dx, dz, facing) = match cycle {
                    0 => (0, 0, 0),
                    1 => (1, 0, 1),
                    2 => (1, 1, 2),
                    _ => (0, 1, 3),
                };
                self.state([tx + 1 + dx, y + level, tz + 1 + dz], 77, facing);
            }
            self.fill([tx + 1, y, tz - 1], [tx + 2, y + 2, tz], 0);
        }
        if role == BuildingRole::Medic {
            for dx in -1..=1 {
                self.set([x + w / 2 + dx, y + h - 2, z - 1], 19);
            }
            self.fill(
                [x + w / 2, y + h - 3, z - 1],
                [x + w / 2, y + h - 1, z - 1],
                19,
            );
        }
        self.summary.buildings.push(BuildingSummary {
            role,
            entrance,
            bounds: [[x - 1, y - 2, z - 1], [x + w + 1, y + h + 14, z + d + 1]],
        });
        Some(entrance)
    }
    fn road(&mut self, from: [i64; 3], to: [i64; 3]) -> Option<()> {
        let steps = (from[0] - to[0]).abs().max((from[2] - to[2]).abs()).max(1);
        if from[1].abs_diff(to[1]) > steps as u64 {
            return None;
        }
        let side = if from[0].abs_diff(to[0]) >= from[2].abs_diff(to[2]) {
            [0, 1]
        } else {
            [1, 0]
        };
        let mut previous = from[1];
        for step in 0..=steps {
            let x = from[0] + (to[0] - from[0]) * step / steps;
            let z = from[2] + (to[2] - from[2]) * step / steps;
            let natural = self.ground(x, z);
            let water = self.terrain.water_column(x as f64 + 0.5, z as f64 + 0.5);
            let bridge = water.is_some_and(|w| w.level > natural as f64);
            let desired = if bridge {
                water.unwrap().level.ceil() as i64 + 1
            } else {
                natural
            };
            let remaining = steps - step;
            let desired = desired.clamp(to[1] - remaining, to[1] + remaining);
            let y = desired.clamp(previous - 1, previous + 1);
            for dx in -1..=1 {
                for dz in -1..=1 {
                    if !bridge {
                        self.fill(
                            [x + dx, natural.min(y - 1) - 1, z + dz],
                            [x + dx, y - 2, z + dz],
                            3,
                        );
                    }
                    self.set([x + dx, y - 1, z + dz], if bridge { 16 } else { PATH });
                    self.fill([x + dx, y, z + dz], [x + dx, y + 3, z + dz], 0);
                }
            }
            if bridge {
                for offset in [-2, 2] {
                    let sx = x + side[0] * offset;
                    let sz = z + side[1] * offset;
                    self.set([sx, y, sz], 90);
                    self.fill([sx, natural, sz], [sx, y - 1, sz], 6);
                }
                if !self
                    .summary
                    .buildings
                    .iter()
                    .any(|b| b.role == BuildingRole::Bridge)
                {
                    self.summary.buildings.push(BuildingSummary {
                        role: BuildingRole::Bridge,
                        entrance: [x, y, z],
                        bounds: [
                            [x - 1 - side[0], y - 8, z - 1 - side[1]],
                            [x + 1 + side[0], y + 1, z + 1 + side[1]],
                        ],
                    });
                }
            }
            previous = y;
        }
        Some(())
    }
    fn farm(&mut self, x: i64, z: i64) -> Option<()> {
        let y = self.fit(x, z, 12, 8)?;
        self.fill([x - 1, y, z - 1], [x + 13, y + 4, z + 9], 0);
        for dz in 0..=8 {
            for dx in 0..=12 {
                let base = self.ground(x + dx, z + dz);
                self.fill(
                    [x + dx, base.min(y - 1) - 1, z + dz],
                    [x + dx, y - 1, z + dz],
                    2,
                );
                if dx == 0 || dx == 12 || dz == 0 || dz == 8 {
                    self.set([x + dx, y, z + dz], 90);
                    continue;
                }
                let p = [x + dx, y - 1, z + dz];
                if dx == 6 {
                    self.object(p, 27, None, None);
                } else {
                    self.set(p, 101);
                    let kind = match dx / 4 {
                        0 => CropKind::Wheat,
                        1 => CropKind::Corn,
                        _ => CropKind::Lettuce,
                    };
                    self.objects.insert(
                        p,
                        StructureObject {
                            position: p,
                            material: 101,
                            state: None,
                            loot: None,
                            crop: Some(kind),
                        },
                    );
                }
            }
        }
        self.set([x + 6, y, z], 0);
        self.summary.buildings.push(BuildingSummary {
            role: BuildingRole::Farm,
            entrance: [x + 6, y, z],
            bounds: [[x - 1, y - 2, z - 1], [x + 13, y + 4, z + 9]],
        });
        Some(())
    }
    fn ruin(&mut self, x: i64, z: i64) -> Option<()> {
        let detailed = self.terrain.config().structures_revision >= 3;
        let y = self.fit(x, z, 14, 12)?;
        self.fill([x - 1, y, z - 1], [x + 15, y + 12, z + 13], 0);
        for dx in 0..=14 {
            for dz in 0..=12 {
                let base = self.ground(x + dx, z + dz);
                self.fill(
                    [x + dx, base.min(y - 1) - 1, z + dz],
                    [x + dx, y - 1, z + dz],
                    43,
                );
                if dx == 0 || dx == 14 || dz == 0 || dz == 12 {
                    let h = 1 + ((dx * 7 + dz * 13 + self.seed as i64).rem_euclid(5));
                    self.fill(
                        [x + dx, y, z + dz],
                        [x + dx, y + h, z + dz],
                        if (dx + dz) % 3 == 0 { 13 } else { 43 },
                    );
                }
            }
        }
        self.fill([x + 6, y, z], [x + 8, y + 3, z], 0);
        for dx in [2, 12] {
            for dz in [2, 10] {
                self.fill([x + dx, y, z + dz], [x + dx, y + 6, z + dz], 43);
                self.fill(
                    [x + dx - 1, y + 6, z + dz - 1],
                    [x + dx + 1, y + 6, z + dz + 1],
                    12,
                );
            }
        }
        if detailed {
            // Broken coping and railings share the normal construction meshes.
            // Keep the entrance and the central route to the chest unobstructed.
            for dx in [2, 3, 4, 10, 11, 12] {
                let h = 1 + (dx * 7 + self.seed as i64).rem_euclid(5);
                self.state([x + dx, y + h + 1, z], 85, 0);
            }
            for dz in [4, 5, 8, 9] {
                self.state([x + 2, y, z + dz], 93, 0);
                self.state([x + 12, y, z + dz], 93, 0);
            }
            for dx in 5..=9 {
                self.state([x + dx, y, z + 8], 77, 0);
                self.state([x + dx, y, z + 9], 85, 4);
            }
            for (dx, dz, facing) in [(3, 3, 1), (11, 3, 3), (3, 9, 2), (11, 9, 0)] {
                self.state([x + dx, y, z + dz], 77, facing);
            }
        }
        let chest = [x + 7, y, z + 10];
        self.object(
            chest,
            29,
            Some(BlockObjectState {
                facing: if detailed { 2 } else { 0 },
                ..Default::default()
            }),
            Some(loot::inventory(BuildingRole::Ruin, self.seed)),
        );
        self.summary.containers.push(chest);
        self.summary.buildings.push(BuildingSummary {
            role: BuildingRole::Ruin,
            entrance: [x + 7, y, z],
            bounds: [[x - 1, y - 2, z - 1], [x + 15, y + 10, z + 13]],
        });
        Some(())
    }
    fn mine(&mut self, x: i64, z: i64) -> Option<()> {
        let entrance = self.building(x, z, 8, 7, 4, BuildingRole::Mine)?;
        let y = entrance[1];
        let cx = x + 4;
        for step in 0..=28 {
            let cy = y - step / 2;
            let cz = z + step;
            self.fill([cx - 2, cy - 1, cz], [cx + 2, cy - 1, cz], 43);
            self.fill([cx - 1, cy, cz], [cx + 1, cy + 3, cz], 0);
            if step % 4 == 0 {
                self.fill([cx - 2, cy, cz], [cx - 2, cy + 3, cz], 16);
                self.fill([cx + 2, cy, cz], [cx + 2, cy + 3, cz], 16);
                self.fill([cx - 2, cy + 3, cz], [cx + 2, cy + 3, cz], 16);
                self.set([cx + 2, cy + 1, cz - 1], 40);
            }
            if step % 2 == 1 {
                for dx in -1..=1 {
                    self.state([cx + dx, cy - 1, cz], 77, 0);
                }
            }
        }
        let cy = y - 14;
        let cz = z + 29;
        self.fill([cx - 8, cy - 1, cz], [cx + 8, cy - 1, cz + 12], 43);
        self.fill([cx - 7, cy, cz], [cx + 7, cy + 5, cz + 11], 0);
        for dx in [-5, 0, 5] {
            self.fill([cx + dx, cy, cz + 5], [cx + dx, cy + 5, cz + 5], 16);
        }
        let chest = [cx + 5, cy, cz + 9];
        self.object(
            chest,
            29,
            Some(BlockObjectState::default()),
            Some(loot::inventory(BuildingRole::Mine, self.seed)),
        );
        self.summary.containers.push(chest);
        for dx in [-8, 8] {
            for dz in 1..11 {
                if dz % 3 != 0 {
                    self.set(
                        [cx + dx, cy + 1, cz + dz],
                        if dz % 2 == 0 { 31 } else { 32 },
                    );
                }
            }
        }
        self.summary.buildings.push(BuildingSummary {
            role: BuildingRole::Mine,
            entrance,
            bounds: [[cx - 8, cy - 2, z], [cx + 8, y + 8, cz + 12]],
        });
        Some(())
    }
    fn finish(mut self) -> Option<Site> {
        if self.failed || self.writes > MAX_SITE_CELLS * 6 {
            return None;
        }
        let mut sections = BTreeMap::new();
        let mut count = 0;
        let mut bounds = [[i64::MAX; 3], [i64::MIN; 3]];
        for (owner, raw) in self.patches {
            let cells: Vec<_> = raw
                .iter()
                .enumerate()
                .filter_map(|(i, &m)| (m != u16::MAX).then_some((i as u16, m)))
                .collect();
            count += cells.len();
            let origin = owner.origin().ok()?;
            for &(i, _) in &cells {
                let i = i64::from(i);
                let p = [
                    origin[0] + i % 16,
                    origin[1] + i / 16 % 16,
                    origin[2] + i / 256,
                ];
                for (a, value) in p.into_iter().enumerate() {
                    bounds[0][a] = bounds[0][a].min(value);
                    bounds[1][a] = bounds[1][a].max(value);
                }
            }
            sections.insert(
                owner,
                StructurePatch {
                    cells,
                    objects: Vec::new(),
                },
            );
        }
        if count > MAX_SITE_CELLS || sections.len() > MAX_SITE_SECTIONS || sections.is_empty() {
            return None;
        }
        self.summary.containers.retain(|p| {
            self.objects
                .get(p)
                .is_some_and(|o| o.material == 29 && o.loot.is_some())
        });
        for (_, object) in self.objects {
            if object
                .state
                .is_some_and(|s| !s.valid_for(object.position, object.material))
            {
                return None;
            }
            sections
                .get_mut(&global_to_section(object.position).0)?
                .objects
                .push(object);
        }
        self.summary.bounds = bounds;
        let settlement = super::settlement::describe(&self.summary, &sections, self.seed);
        Some(Site {
            summary: self.summary,
            sections,
            settlement,
        })
    }
}

pub(super) fn plan(terrain: &TerraGenerator, region: [i64; 2], seed: u32) -> Option<Site> {
    if terrain.config().structures_revision >= 2 {
        village::plan(terrain, region, seed)
    } else {
        plan_legacy(terrain, region, seed)
    }
}

pub(super) fn plan_legacy(terrain: &TerraGenerator, region: [i64; 2], seed: u32) -> Option<Site> {
    if seed % 10 == 9 {
        return None;
    }
    let x = region[0] * REGION_SIZE + 192 + i64::from((seed >> 4) % 97) - 48;
    let z = region[1] * REGION_SIZE + 192 + i64::from((seed >> 12) % 97) - 48;
    let biome = terrain.biome_at(x as f64, z as f64);
    if biome.weights.ocean > 0.3 {
        return None;
    }
    let style = if biome.weights.desert + biome.weights.cliffted_desert > 0.4 {
        VillageStyle::Desert
    } else if biome.weights.highland > 0.2 {
        VillageStyle::Highland
    } else {
        VillageStyle::Temperate
    };
    let kind = match seed % 9 {
        0..=3 => StructureKind::Village,
        4..=6 => StructureKind::Ruin,
        _ => StructureKind::Mine,
    };
    if kind == StructureKind::Village
        && style == VillageStyle::Temperate
        && !matches!(
            biome.weights.dominant(),
            super::super::biomes::BiomeKind::Plains | super::super::biomes::BiomeKind::Forest
        )
    {
        return None;
    }
    let y = (terrain.surface_height(x as f64 + 0.5, z as f64 + 0.5) - 0.5).floor() as i64 + 1;
    let mut b = Builder {
        terrain,
        summary: SiteSummary {
            id: (u64::from(seed) << 32) ^ region[0] as u64 ^ (region[1] as u64).rotate_left(17),
            kind,
            style,
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
    match kind {
        StructureKind::Village => {
            let buildings = [
                (-24, -32, 12, 17, 7, BuildingRole::Church),
                (9, -26, 10, 10, 5, BuildingRole::Smith),
                (16, 4, 9, 9, 7, BuildingRole::Watch),
                (-25, 8, 10, 10, 5, BuildingRole::Medic),
                (-8, 20, 9, 8, 4, BuildingRole::House),
                (10, 23, 8, 8, 4, BuildingRole::House),
                (-8, -33, 8, 8, 4, BuildingRole::House),
            ];
            for (dx, dz, w, d, _, _) in buildings {
                let floor = b.fit(x + dx, z + dz, w, d)?;
                let entrance = [x + dx + w / 2, floor, z + dz - 2];
                if dz < 0 {
                    // North-facing doors on the far side must be reached
                    // around the shell, not by a path later buried by walls.
                    let side = x + dx + w + 3;
                    let turn = [side, b.ground(side, z), z];
                    let front = [side, floor, entrance[2]];
                    b.road([x, y, z], turn)?;
                    b.road(turn, front)?;
                    b.road(front, entrance)?;
                } else {
                    b.road([x, y, z], entrance)?;
                }
            }
            for (dx, dz, w, d, h, role) in buildings {
                b.building(x + dx, z + dz, w, d, h, role)?;
            }
            b.farm(x - 38, z - 8)?;
            let farm_y = b
                .summary
                .buildings
                .iter()
                .find(|building| building.role == BuildingRole::Farm)?
                .entrance[1];
            b.road([x, y, z], [x - 32, farm_y, z - 10])?;
            for (dx, dz) in [(-3, -4), (4, 5)] {
                let py = b.ground(x + dx, z + dz);
                b.fill([x + dx, py, z + dz], [x + dx, py + 2, z + dz], 43);
                b.set([x + dx, py + 3, z + dz], 40);
            }
        }
        StructureKind::Ruin => b.ruin(x - 7, z - 6)?,
        StructureKind::Mine => b.mine(x - 4, z - 5)?,
    }
    b.finish()
}

fn foundation_supported(
    terrain: &TerraGenerator,
    x: i64,
    z: i64,
    floor: i64,
    column: super::super::SurfaceColumn,
) -> bool {
    let natural = (column.height - 0.5).floor() as i64 + 1;
    let below = natural.min(floor - 1) - 2;
    // Public sample() consults the structure cache; calling it while its plan
    // is being built would recurse into the same OnceLock.
    let sample =
        terrain.sample_with_column([x as f64 + 0.5, below as f64 + 0.5, z as f64 + 0.5], column);
    !column.cave.open_to_sky()
        && sample.density < 0.0
        && crate::props::placement::ground_support(sample.material)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn real_water_bridges_keep_both_travel_axes_clear_and_place_rails_outside_the_three_wide_deck()
    {
        let terrain = TerraGenerator::new(terra_core::WorldConfig {
            structures_revision: 0,
            biomes: false,
            trees: false,
            caves: false,
            water: true,
            rivers: false,
            landscape_revision: 0,
            geology_revision: 0,
            ocean_revision: 0,
            base_height: 0.0,
            sea_level: 8.0,
            hill_amplitude: 0.0,
            mountain_amplitude: 0.0,
            ..Default::default()
        })
        .unwrap();
        for travel in [[1, 0], [0, 1]] {
            let from = [-8, 9, -16];
            let to = [from[0] + travel[0] * 20, from[1], from[2] + travel[1] * 20];
            let mut builder = Builder {
                terrain: &terrain,
                summary: SiteSummary {
                    id: 1,
                    kind: StructureKind::Village,
                    style: VillageStyle::Temperate,
                    center: from,
                    bounds: [[0; 3]; 2],
                    buildings: Vec::new(),
                    containers: Vec::new(),
                },
                patches: BTreeMap::new(),
                objects: BTreeMap::new(),
                writes: 0,
                failed: false,
                seed: 42,
            };
            assert!(builder.road(from, to).is_some());
            let site = builder.finish().unwrap();
            assert!(
                site.summary
                    .buildings
                    .iter()
                    .any(|b| b.role == BuildingRole::Bridge)
            );
            let side = [travel[1], travel[0]];
            let journal = crate::edits::EditJournal::default();
            for step in 0..=20 {
                let x = from[0] + travel[0] * step;
                let z = from[2] + travel[1] * step;
                let water = terrain
                    .water_column(x as f64 + 0.5, z as f64 + 0.5)
                    .unwrap();
                assert!(
                    water.depth >= 8.0,
                    "this is a real generated water crossing, not a dry-road fixture"
                );
                for lane in -1..=1 {
                    let sx = x + side[0] * lane;
                    let sz = z + side[1] * lane;
                    assert_eq!(site.block_at([sx, 8, sz]), Some(16));
                    assert_eq!(
                        site.block_at([sx, 9, sz]),
                        Some(0),
                        "deck must not contain centerline railing"
                    );
                    assert_eq!(site.block_at([sx, 10, sz]), Some(0));
                    for y in 0..8 {
                        let p = [sx, y, sz];
                        assert!(
                            site.block_at(p).is_none(),
                            "the bridge deck must not stamp a solid underwater causeway"
                        );
                        assert!(
                            crate::water::fluid_cell_at(&terrain, &journal, p, 0).amount > 0,
                            "the existing fluid engine retains the real water passage below every deck lane"
                        );
                    }
                }
                for offset in [-2, 2] {
                    let sx = x + side[0] * offset;
                    let sz = z + side[1] * offset;
                    assert_eq!(site.block_at([sx, 9, sz]), Some(90));
                    assert_eq!(site.block_at([sx, 8, sz]), Some(6));
                    assert_eq!(site.block_at([sx, 0, sz]), Some(6));
                }
            }
        }
    }

    #[test]
    fn foundation_probes_real_cave_skylights_and_ravines_not_just_the_unchanged_heightfield() {
        let terrain = TerraGenerator::new(terra_core::WorldConfig {
            structures_revision: 0,
            biomes: false,
            trees: false,
            caves: false,
            water: false,
            rivers: false,
            landscape_revision: 0,
            geology_revision: 0,
            hill_amplitude: 0.0,
            mountain_amplitude: 0.0,
            ..Default::default()
        })
        .unwrap();
        let mut column = terrain.surface_column(1.5, 1.5);
        let floor = (column.height - 0.5).floor() as i64 + 1;
        assert!(foundation_supported(&terrain, 1, 1, floor, column));
        // This off-center footprint column was missed by the old corner/center
        // probes; the actual CaveStructure CSG opens its roof, not its heightmap.
        column.cave.kind = super::super::super::CaveEcology::Lush;
        column.cave.floor = column.height - 12.0;
        column.cave.ceiling = column.height + 2.0;
        column.cave.radius = 24.0;
        column.cave.radial = 0.0;
        assert!(column.cave.open_to_sky());
        let sample = terrain.sample_with_column([1.5, floor as f64 - 2.5, 1.5], column);
        assert!(
            sample.density > 0.0,
            "the existing cave engine actually carved away the foundation support"
        );
        assert!(!foundation_supported(&terrain, 1, 1, floor, column));
        column.cave = Default::default();
        column.river_cavity = Some([column.height - 12.0, column.height + 2.0]);
        assert!(!foundation_supported(&terrain, 1, 1, floor, column));
        column.river_cavity = None;
        column.cave_cavity = Some([column.height - 12.0, column.height - 4.0]);
        assert!(
            foundation_supported(&terrain, 1, 1, floor, column),
            "a real solid roof above a deeper cave remains a suitable site"
        );
    }

    #[test]
    fn road_doglegs_reject_impossible_endpoint_grades_before_writing_disconnected_steps() {
        let terrain = TerraGenerator::new(terra_core::WorldConfig {
            structures_revision: 0,
            biomes: false,
            trees: false,
            caves: false,
            water: false,
            rivers: false,
            landscape_revision: 0,
            hill_amplitude: 0.0,
            mountain_amplitude: 0.0,
            ..Default::default()
        })
        .unwrap();
        let mut builder = Builder {
            terrain: &terrain,
            summary: SiteSummary {
                id: 1,
                kind: StructureKind::Village,
                style: VillageStyle::Highland,
                center: [0, 16, 0],
                bounds: [[0; 3]; 2],
                buildings: Vec::new(),
                containers: Vec::new(),
            },
            patches: BTreeMap::new(),
            objects: BTreeMap::new(),
            writes: 0,
            failed: false,
            seed: 42,
        };
        assert!(builder.road([0, 16, 0], [4, 24, 0]).is_none());
        assert!(builder.patches.is_empty());
        assert!(builder.road([0, 16, 0], [8, 24, 0]).is_some());
        let site = builder.finish().unwrap();
        assert_eq!(site.block_at([8, 23, 0]), Some(PATH));
        assert_eq!(site.block_at([8, 24, 0]), Some(0));
    }

    #[test]
    fn climate_building_styles_use_real_distinct_roofs_windows_and_reachable_interiors() {
        let terrain = TerraGenerator::new(terra_core::WorldConfig {
            structures_revision: 0,
            trees: false,
            caves: false,
            biomes: false,
            rivers: false,
            water: false,
            landscape_revision: 0,
            hill_amplitude: 0.0,
            mountain_amplitude: 0.0,
            ..Default::default()
        })
        .unwrap();
        for style in [
            VillageStyle::Temperate,
            VillageStyle::Desert,
            VillageStyle::Highland,
        ] {
            let mut builder = Builder {
                terrain: &terrain,
                summary: SiteSummary {
                    id: 1,
                    kind: StructureKind::Village,
                    style,
                    center: [0, 16, 0],
                    bounds: [[0; 3]; 2],
                    buildings: Vec::new(),
                    containers: Vec::new(),
                },
                patches: BTreeMap::new(),
                objects: BTreeMap::new(),
                writes: 0,
                failed: false,
                seed: 42,
            };
            let entrance = builder
                .building(-6, -8, 12, 16, 6, BuildingRole::Church)
                .unwrap();
            let site = builder.finish().unwrap();
            assert_eq!(
                site.block_at([entrance[0], entrance[1] + 2, entrance[2] + 1]),
                Some(0)
            );
            assert_eq!(
                site.block_at([entrance[0], entrance[1] - 1, entrance[2] + 1]),
                Some(if style == VillageStyle::Desert {
                    11
                } else if style == VillageStyle::Highland {
                    18
                } else {
                    16
                })
            );
            let objects: Vec<_> = site
                .sections
                .values()
                .flat_map(|p| p.objects.iter())
                .collect();
            assert!(objects.iter().any(|o| (68..=70).contains(&o.material)));
            assert!(objects.iter().any(|o| (65..=67).contains(&o.material)));
            let roof_stairs = if style == VillageStyle::Highland {
                76
            } else {
                74
            };
            assert_eq!(
                objects.iter().any(|o| o.material == roof_stairs),
                style != VillageStyle::Desert
            );
            assert!(
                site.sections
                    .values()
                    .flat_map(|p| p.cells.iter())
                    .all(|&(_, m)| m != 5 && m != 98),
                "highland roofs accumulate existing weather-driven snow, not permanent snow edits"
            );
            for o in objects {
                assert!(
                    o.state
                        .is_none_or(|state| state.valid_for(o.position, o.material))
                );
            }
        }
    }
}
