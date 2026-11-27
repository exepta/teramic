use super::*;

impl Royal<'_, '_> {
    /// Doors, beds, containers and workstations use the regular paired-object path.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn house(
        &mut self,
        cx: i64,
        cz: i64,
        y: i64,
        w: i64,
        d: i64,
        h: i64,
        role: BuildingRole,
        beds: usize,
        levels: usize,
    ) {
        let x = cx - w / 2;
        let z = cz - d / 2;
        let y = self.local_floor(x, z, w, d, y);
        let floors = levels.max(1);
        let roof_y = y + floors as i64 * (h + 1) - 1;
        self.foundation(x, z, w, d, y);
        self.fill(
            [x - 1, y, z - 1],
            [x + w + 1, roof_y + w / 2 + 3, z + d + 1],
            0,
        );
        for floor in 0..floors {
            let fy = y + floor as i64 * (h + 1);
            self.fill([x, fy - 1, z], [x + w, fy - 1, z + d], 16);
            for dx in 0..=w {
                for dz in [0, d] {
                    self.fill([x + dx, fy, z + dz], [x + dx, fy + h - 1, z + dz], 103);
                }
            }
            for dz in 0..=d {
                for dx in [0, w] {
                    self.fill([x + dx, fy, z + dz], [x + dx, fy + h - 1, z + dz], 103);
                }
            }
            for dx in [0, w] {
                for dz in [0, d] {
                    self.fill([x + dx, fy, z + dz], [x + dx, fy + h, z + dz], 36);
                }
            }
            for dz in [0, d] {
                self.fill([x, fy + h - 1, z + dz], [x + w, fy + h - 1, z + dz], 16);
            }
            for dx in [0, w] {
                self.fill([x + dx, fy + h - 1, z], [x + dx, fy + h - 1, z + d], 16);
            }
            for dx in [2, w - 2] {
                for dz in [0, d] {
                    if dx != w / 2 {
                        self.state([x + dx, fy + 2, z + dz], 68, 0);
                    }
                }
            }
            for dz in [3, d - 3] {
                for dx in [0, w] {
                    self.state([x + dx, fy + 2, z + dz], 68, 1);
                }
            }
        }
        // Homes use the warm red/brown town palette; civic roofs keep their blue.
        let roof = if role == BuildingRole::House && !self.winter && !self.outpost {
            let variant = crate::generation::seed::layer_seed(
                u64::from(self.builder.seed) ^ (cx as u64).rotate_left(17) ^ cz as u64,
                0x524f_4f46,
            );
            if variant.is_multiple_of(3) {
                18
            } else {
                terra_core::textiles::dyed(103, DyeColor::Red).unwrap()
            }
        } else {
            self.roof()
        };
        for dx in -1..=w + 1 {
            let rise = (dx + 1).min(w + 1 - dx).max(0);
            self.fill(
                [x + dx, roof_y + rise, z - 1],
                [x + dx, roof_y + rise, z + d + 1],
                roof,
            );
            if (0..=w).contains(&dx) {
                for dz in [0, d] {
                    self.fill(
                        [x + dx, roof_y, z + dz],
                        [x + dx, roof_y + rise - 1, z + dz],
                        103,
                    );
                    if rise > 0 {
                        self.set([x + dx, roof_y + rise - 1, z + dz], 16);
                    }
                }
            }
            if self.winter && dx.rem_euclid(3) != 0 {
                for dz in -1..=d + 1 {
                    self.set([x + dx, roof_y + rise + 1, z + dz], 98);
                }
            }
        }
        let entrance = [cx, y, z + d];
        self.paired(entrance, [cx, y + 1, z + d], 65, 2);
        self.fill([cx - 1, y, z + d + 1], [cx + 1, y + 2, z + d + 3], 0);
        self.fill([cx - 1, y - 1, z + d + 1], [cx + 1, y - 1, z + d + 3], 43);
        self.access(entrance);
        // A clear center aisle and two-cell gaps around each bed avoid furniture traps.
        let candidates = if w <= 9 {
            vec![(2, 2), (w - 2, 2), (2, d - 3), (w - 2, d - 3)]
        } else {
            (2..=d - 3)
                .step_by(3)
                .flat_map(|dz| {
                    (2..=w - 2)
                        .filter(move |dx| {
                            ((*dx - 2).rem_euclid(3) == 0 || *dx == w - 2)
                                && (*dx - w / 2).abs() > 1
                        })
                        .map(move |dx| (dx, dz))
                })
                .collect()
        };
        let capacity = candidates.len();
        if beds > capacity * floors || (beds > 0 && capacity == 0) {
            // A malformed future template rejects its site, never a worker thread.
            self.builder.failed = true;
            return;
        }
        for i in 0..beds {
            let floor = i / capacity;
            let (dx, dz) = candidates[i % capacity];
            self.bed(x + dx, y + floor as i64 * (h + 1), z + dz);
        }
        // Straight stair flights use the reserved central aisle of each barracks.
        for floor in 1..floors {
            for step in 0..=h {
                let sy = y + (floor as i64 - 1) * (h + 1) + step;
                let sz = z + d - 2 - step;
                self.state([cx, sy, sz], 74, 2);
                self.fill([cx, sy + 1, sz], [cx, sy + 3, sz], 0);
            }
        }
        self.chest([x + w - 1, y, z + 1], role);
        self.set([x + 1, y, z + 1], 40);
        if role == BuildingRole::Barracks {
            self.banner(x + 2, y + 3, z + d);
        }
        if role == BuildingRole::Smith {
            self.paired([x + 2, y, z + 2], [x + 2, y + 1, z + 2], 59, 0);
            for py in y + 2..=roof_y + 5 {
                self.state([x + 2, py, z + 2], 60, 0);
            }
            self.paired([x + w - 3, y, z + 2], [x + w - 2, y, z + 2], 28, 0);
        }
        self.summary(
            role,
            entrance,
            [x - 1, y - 2, z - 1],
            [x + w + 1, roof_y + w / 2 + 6, z + d + 3],
        );
    }
    pub(super) fn church(&mut self, x: i64, z: i64, y: i64) {
        let (w, d, h) = (13, 23, 10);
        let left = x - w / 2;
        let north = z - d / 2;
        let y = self.local_floor(left, north, w, d, y);
        self.foundation(left, north, w, d, y);
        self.fill([left, y - 1, north], [left + w, y - 1, north + d], 43);
        self.fill([left, y, north], [left + w, y + h + 10, north + d], 0);
        for dx in 0..=w {
            for dz in [0, d] {
                self.fill(
                    [left + dx, y, north + dz],
                    [left + dx, y + h - 1, north + dz],
                    self.masonry(dx),
                );
            }
        }
        for dz in 0..=d {
            for dx in [0, w] {
                self.fill(
                    [left + dx, y, north + dz],
                    [left + dx, y + h - 1, north + dz],
                    43,
                );
            }
        }
        for dz in [4, 10, 16, 20] {
            for dx in [0, w] {
                for dy in 3..=6 {
                    self.state([left + dx, y + dy, north + dz], castle::WINDOW, 1);
                }
            }
        }
        for dx in -1..=w + 1 {
            let rise = (dx + 1).min(w + 1 - dx);
            self.fill(
                [left + dx, y + h + rise, north - 1],
                [left + dx, y + h + rise, north + d + 1],
                self.roof(),
            );
            for dz in [0, d] {
                self.fill(
                    [left + dx, y + h, north + dz],
                    [left + dx, y + h + rise - 1, north + dz],
                    43,
                );
            }
        }
        let door = [x, y, north + d];
        self.paired(door, [x, y + 1, north + d], 65, 2);
        self.access(door);
        self.fill([x - 1, y, north + d - 1], [x + 1, y + 3, north + d - 1], 0);
        for dz in [5, 8, 11, 14, 17] {
            for dx in [3, 9] {
                self.state([left + dx, y, north + dz], 105, 2);
            }
        }
        self.set([x, y, north + 3], 104);
        self.fill([x - 2, y, north + 2], [x + 2, y, north + 2], 16);
        self.tower(x, north + 3, y, 3, 23, false);
        self.cone(x, north + 3, y + 24, 4);
        self.fill([x, y + 34, north + 3], [x, y + 39, north + 3], 19);
        self.fill([x - 2, y + 37, north + 3], [x + 2, y + 37, north + 3], 19);
        self.banner(x - 3, y + 3, north + d);
        self.chest([left + 2, y, north + 2], BuildingRole::Church);
        self.summary(
            BuildingRole::Church,
            door,
            [left - 1, y - 2, north - 1],
            [left + w + 1, y + 40, north + d + 2],
        );
    }
    pub(super) fn market(&mut self) {
        let y = self.floor(0, 0);
        for x in -5_i64..=5 {
            for z in -5_i64..=5 {
                if x * x + z * z <= 25 {
                    self.set([x, y - 1, z], 43);
                }
            }
        }
        // Central fountain leaves a two-block circular route around it.
        for x in -2_i64..=2 {
            for z in -2_i64..=2 {
                if x.abs() == 2 || z.abs() == 2 {
                    self.set([x, y, z], 85);
                } else {
                    let p = self.point([x, y - 1, z]);
                    self.builder.object(p, 27, None, None);
                }
            }
        }
        self.fill([0, y, 0], [0, y + 3, 0], 43);
        self.set([0, y + 4, 0], 49);
        for (x, z) in [(-9, -1), (9, -1), (-9, 17), (9, 17)] {
            let fy = self.floor(x, z);
            for dx in [-2, 2] {
                for dz in [-2, 2] {
                    self.fill([x + dx, fy, z + dz], [x + dx, fy + 2, z + dz], 90);
                }
            }
            for dx in -2..=2 {
                for dz in -2..=2 {
                    self.set(
                        [x + dx, fy + 3, z + dz],
                        if dx.rem_euclid(2) == 0 { 103 } else { 119 },
                    );
                }
            }
            self.fill([x - 1, fy, z + 1], [x + 1, fy, z + 1], 16);
            self.chest([x, fy, z], BuildingRole::Market);
            self.summary(
                BuildingRole::Market,
                [x, fy, z + 2],
                [x - 2, fy - 1, z - 2],
                [x + 2, fy + 4, z + 2],
            );
        }
    }
    pub(super) fn farm(&mut self, x: i64, z: i64, w: i64, d: i64) {
        let [left, _, north] = self.point([x - w / 2, 0, z - d / 2]);
        if !self.builder.allows_field(left, north, w, d) {
            return;
        }
        if self.adapted() {
            self.terraced_farm(x, z, w, d);
            return;
        }
        let left = x - w / 2;
        let north = z - d / 2;
        let y = self.floor(x, z);
        for dx in 0..=w {
            for dz in 0..=d {
                let p = self.point([left + dx, y - 1, north + dz]);
                let natural = self.builder.ground(p[0], p[2]) - self.origin[1];
                self.fill(
                    [left + dx, natural.min(y - 1) - 1, north + dz],
                    [left + dx, y - 1, north + dz],
                    2,
                );
                self.fill(
                    [left + dx, y, north + dz],
                    [left + dx, (natural + 4).max(y + 3), north + dz],
                    0,
                );
                if dx == 0 || dx == w || dz == 0 || dz == d {
                    self.set([left + dx, y, north + dz], 90);
                } else if dx == w / 2 {
                    self.builder.object(p, 27, None, None);
                } else {
                    self.builder.set(p, 101);
                    self.builder.objects.insert(
                        p,
                        StructureObject {
                            position: p,
                            material: 101,
                            state: None,
                            loot: None,
                            crop: Some(if dx < w / 2 {
                                CropKind::Wheat
                            } else {
                                CropKind::Lettuce
                            }),
                        },
                    );
                }
            }
        }
        self.set([x, y, north + d], 0);
        self.summary(
            BuildingRole::Farm,
            [x, y, north + d],
            [left, y - 2, north],
            [left + w, y + 2, north + d],
        );
    }
    pub(super) fn tent(&mut self, x: i64, z: i64, y: i64, w: i64, d: i64) {
        let y = self.local_floor(x - w / 2, z - d / 2, w, d, y);
        if self.terrain_adapted() {
            self.foundation(x - w / 2, z - d / 2, w, d, y);
            self.fill(
                [x - w / 2, y, z - d / 2],
                [x + w / 2, y + w / 2 + 1, z + d / 2],
                0,
            );
        }
        for dx in -w / 2..=w / 2 {
            let h = w / 2 - dx.abs();
            for dz in -d / 2..=d / 2 {
                self.set(
                    [x + dx, y + h, z + dz],
                    if dz.rem_euclid(3) == 0 {
                        103
                    } else {
                        self.roof()
                    },
                );
            }
        }
        self.fill([x, y, z - d / 2], [x, y + 1, z - d / 2], 0);
        self.fill([x, y, z + d / 2], [x, y + 1, z + d / 2], 0);
        self.chest([x + 1, y, z], BuildingRole::Watch);
        self.summary(
            BuildingRole::Watch,
            [x, y, z + d / 2],
            [x - w / 2, y - 1, z - d / 2],
            [x + w / 2, y + w / 2 + 1, z + d / 2],
        );
    }
    pub(super) fn windmill(&mut self, x: i64, z: i64, y: i64) {
        let [left, _, north] = self.point([x - 8, 0, z - 6]);
        if !self.builder.allows_field(left, north, 16, 12) {
            return;
        }
        self.tower(x, z, y, 4, 11, true);
        self.fill([x, y + 8, z + 4], [x, y + 8, z + 6], 36);
        for step in 1..=7 {
            for width in 0..=1 {
                for (dx, dy) in [
                    (step, width),
                    (-step, -width),
                    (width, -step),
                    (-width, step),
                ] {
                    self.set(
                        [x + dx, y + 8 + dy, z + 6],
                        if width == 0 { 16 } else { 90 },
                    );
                }
            }
        }
        self.summary(
            BuildingRole::Farm,
            [x, y, z + 4],
            [x - 8, y - 2, z - 4],
            [x + 8, y + 21, z + 7],
        );
    }
    pub(super) fn stable(&mut self, x: i64, z: i64, y: i64) {
        let y = self.local_floor(x - 5, z - 5, 10, 10, y);
        if self.terrain_adapted() {
            self.foundation(x - 5, z - 5, 10, 10, y);
            self.fill([x - 5, y, z - 5], [x + 5, y + 8, z + 5], 0);
        }
        for dx in [-5, 5] {
            for dz in [-5, 5] {
                self.fill([x + dx, y, z + dz], [x + dx, y + 4, z + dz], 36);
            }
        }
        for dx in -6_i64..=6 {
            let rise = (6 - dx.abs()) / 2;
            self.fill(
                [x + dx, y + 5 + rise, z - 6],
                [x + dx, y + 5 + rise, z + 6],
                18,
            );
        }
        for dz in -5..=5 {
            for dx in [-5, 0, 5] {
                self.set([x + dx, y, z + dz], 90);
            }
        }
        for dx in -4..=4 {
            self.set([x + dx, y, z - 5], 90);
        }
        for dx in [-3, 2] {
            self.fill(
                [x + dx, y, z - 4],
                [x + dx + 1, y + 1, z - 3],
                terra_core::textiles::dyed(103, DyeColor::Yellow).unwrap(),
            );
        }
        self.summary(
            BuildingRole::Stable,
            [x, y, z + 5],
            [x - 6, y - 1, z - 6],
            [x + 6, y + 9, z + 6],
        );
    }
}
