use super::*;

impl Royal<'_, '_> {
    pub(super) fn outer_wall(&mut self, radius: i64, height: i64) {
        for z in -radius..=radius {
            for x in -radius..=radius {
                let rr = x * x + z * z;
                if rr > radius * radius
                    || rr < (radius - 3) * (radius - 3)
                    || (z > radius - 5 && x.abs() <= 5)
                {
                    continue;
                }
                let y = self.floor(x, z);
                for dy in 0..height {
                    self.set([x, y + dy, z], self.masonry(dy));
                }
                if rr >= (radius - 1) * (radius - 1) && (x + z).rem_euclid(4) < 2 {
                    self.set([x, y + height, z], castle::BATTLEMENT_STRAIGHT);
                }
                if rr < (radius - 1) * (radius - 1) {
                    self.set([x, y + height - 1, z], 43);
                }
                if (x + z).rem_euclid(17) == 0 && rr >= (radius - 1) * (radius - 1) {
                    self.banner(x, y + height - 3, z);
                }
            }
        }
    }
    pub(super) fn tower(&mut self, cx: i64, cz: i64, y: i64, r: i64, height: i64, roof: bool) {
        if self.terrain_adapted() {
            self.bounded_foundation(cx - r, cz - r, 2 * r, 2 * r, y);
        }
        for dz in -r..=r {
            for dx in -r..=r {
                let rr = dx * dx + dz * dz;
                if rr > r * r {
                    continue;
                }
                let position = self.point([cx + dx, 0, cz + dz]);
                let natural = self.builder.ground(position[0], position[2]) - self.origin[1];
                if !self.terrain_adapted() {
                    self.fill(
                        [cx + dx, natural.min(y - 1) - 2, cz + dz],
                        [cx + dx, y - 1, cz + dz],
                        43,
                    );
                }
                self.fill(
                    [cx + dx, y, cz + dz],
                    [cx + dx, (y + height + 7).max(natural + 5), cz + dz],
                    0,
                );
                if rr >= (r - 1) * (r - 1) {
                    for level in 0..height {
                        self.set([cx + dx, y + level, cz + dz], self.masonry(level));
                    }
                }
                for level in [y - 1, y + height - 1] {
                    self.set([cx + dx, level, cz + dz], 43);
                }
                if rr >= (r - 1) * (r - 1) && (dx + dz).rem_euclid(3) != 0 {
                    self.set([cx + dx, y + height, cz + dz], castle::BATTLEMENT_PILLAR);
                }
            }
        }
        for (dx, dz, facing) in [(0, r, 2), (0, -r, 0), (r, 0, 1), (-r, 0, 3)] {
            self.fill([cx + dx, y, cz + dz], [cx + dx, y + 1, cz + dz], 0);
            let inner = [cx + dx - dx.signum(), y, cz + dz - dz.signum()];
            if self.adapted() {
                // The circular wall's real jambs sit one cell inside its tip.
                self.fill(inner, [inner[0], y + 1, inner[2]], 0);
                self.fill([cx + dx, y, cz + dz], [cx + dx, y + 2, cz + dz], 0);
                self.paired(inner, [inner[0], y + 1, inner[2]], 65, facing);
            } else {
                self.fill(inner, [inner[0], y + 2, inner[2]], 0);
                self.paired([cx + dx, y, cz + dz], [cx + dx, y + 1, cz + dz], 65, facing);
            }
        }
        let inner = (r - 2).max(1);
        let mut route = Vec::new();
        for x in -inner..inner {
            route.push((x, -inner, 1));
        }
        for z in -inner..inner {
            route.push((inner, z, 0));
        }
        for x in (-inner + 1..=inner).rev() {
            route.push((x, inner, 3));
        }
        for z in (-inner + 1..=inner).rev() {
            route.push((-inner, z, 2));
        }
        for level in 0..height {
            let (dx, dz, facing) = route[level as usize % route.len()];
            self.fill(
                [cx + dx, y + level + 1, cz + dz],
                [cx + dx, y + level + 3, cz + dz],
                0,
            );
            self.state([cx + dx, y + level, cz + dz], 77, facing);
        }
        for (dx, dz, facing) in [(0, -r, 0), (r, 0, 1), (0, r, 2), (-r, 0, 3)] {
            for level in [4, height - 4] {
                self.state([cx + dx, y + level, cz + dz], castle::ARROW_SLIT, facing);
            }
        }
        if roof {
            for (dx, dz) in [(-r, 0), (r, 0), (0, -r), (0, r)] {
                self.fill(
                    [cx + dx, y + height, cz + dz],
                    [cx + dx, y + height + 3, cz + dz],
                    36,
                );
            }
            self.cone(cx, cz, y + height + 4, r + 1);
        }
        self.banner(cx, y + height - 3, cz + r);
        self.summary(
            BuildingRole::Watch,
            [cx, y, cz + if self.adapted() { r - 1 } else { r }],
            [cx - r, y - 2, cz - r],
            [cx + r, y + height + r + 6, cz + r],
        );
    }
    pub(super) fn cone(&mut self, x: i64, z: i64, y: i64, r: i64) {
        let roof = self.roof();
        for level in 0..=r {
            let width = r - level;
            for dz in -width..=width {
                for dx in -width..=width {
                    let rr = dx * dx + dz * dz;
                    if rr <= width * width + width
                        && (level == 0 || rr >= (width - 1).max(0).pow(2))
                    {
                        self.set([x + dx, y + level, z + dz], roof);
                        if self.winter && (dx + dz).rem_euclid(4) != 0 {
                            self.set([x + dx, y + level + 1, z + dz], 98);
                        }
                    }
                }
            }
        }
        self.fill([x, y + r + 1, z], [x, y + r + 4, z], 36);
    }
    pub(super) fn gate(&mut self, x: i64, z: i64, y: i64, r: i64) {
        // Excavate before placing the arch: terrain above a sunken entrance must
        // disappear, while the actual masonry/raised portcullis must remain.
        let Some(approach) = self.gate_approach(x, z, y) else {
            self.builder.failed = true;
            return;
        };
        let height = if self.outpost { 7 } else { 12 };
        for dx in [-r - 3, r + 3] {
            self.tower(x + dx, z, y, r, height, true);
        }
        self.fill(
            [x - r - 3, y + height - 3, z - 2],
            [x + r + 3, y + height - 1, z + 2],
            43,
        );
        self.fill([x - 3, y, z - 3], [x + 3, y + 5, z + 3], 0);
        for dx in -3_i64..=3 {
            let top = if dx.abs() == 3 {
                3
            } else if dx.abs() == 2 {
                4
            } else {
                5
            };
            self.fill(
                [x + dx, y + top, z - 2],
                [x + dx, y + height - 4, z + 2],
                43,
            );
        }
        // Open portcullis remains above the head clearance, never a sealed entrance.
        for dx in (-3..=3).step_by(2) {
            self.fill([x + dx, y + 6, z], [x + dx, y + 8, z], 19);
        }
        for bx in [-r - 3, r + 3] {
            self.banner(x + bx, y + 4, z + r);
        }
        for &(dz, sy) in approach
            .iter()
            .filter(|(dz, _)| *dz >= 4 && (*dz - 4).rem_euclid(6) == 0)
        {
            for dx in [-4, 4] {
                let position = self.point([x + dx, 0, z + dz]);
                let natural = self.builder.ground(position[0], position[2]) - self.origin[1];
                self.fill(
                    [x + dx, natural.min(sy - 1) - 1, z + dz],
                    [x + dx, sy, z + dz],
                    43,
                );
                self.set([x + dx, sy + 1, z + dz], 90);
            }
        }
        self.summary(
            BuildingRole::Gate,
            [x, y, z],
            [x - r * 2 - 3, y - 4, z - r],
            [
                x + r * 2 + 3,
                y + height + r + 6,
                z + approach.last().unwrap().0,
            ],
        );
    }
    fn gate_approach(&mut self, x: i64, z: i64, y: i64) -> Option<Vec<(i64, i64)>> {
        // Royal sites remain inside their region's existing [96,288] lookup
        // envelope. The small outpost has more room for a gentle joining road.
        let length = if self.outpost { 32 } else { 22 };
        let ground = |dx, dz| self.natural_walk_height(x + dx, z + dz);
        let mut previous = y;
        let mut profile = Vec::with_capacity((length + 5) as usize);
        for dz in -4..=length {
            let natural = ground(0, dz)?;
            let sy = if dz <= 4 {
                y
            } else {
                natural.clamp(previous - 1, previous + 1)
            };
            profile.push((dz, sy));
            previous = sy;
        }
        // Never leave a vertical face at the end of an otherwise walkable road.
        let endpoint_top = previous.max(profile[profile.len() - 2].1);
        if previous != ground(0, length)? || endpoint_top.abs_diff(ground(0, length + 1)?) > 1 {
            return None;
        }
        let mut previous = y;
        for &(dz, sy) in &profile {
            let deck_top = sy.max(previous);
            for dx in -3..=3 {
                let position = self.point([x + dx, 0, z + dz]);
                let natural = self.builder.ground(position[0], position[2]) - self.origin[1];
                let support = self.natural_walk_height(x + dx, z + dz)?;
                if self.terrain_adapted()
                    && ((deck_top - support).abs() > 3 || (natural - deck_top).abs() > 3)
                {
                    return None;
                }
                self.fill(
                    [x + dx, support.min(sy - 1) - 2, z + dz],
                    [x + dx, deck_top - 1, z + dz],
                    43,
                );
                self.fill(
                    [x + dx, deck_top, z + dz],
                    [x + dx, (natural + 7).max(deck_top + 3), z + dz],
                    0,
                );
                if sy == previous {
                    self.set([x + dx, sy - 1, z + dz], 43);
                } else {
                    self.state(
                        [x + dx, deck_top - 1, z + dz],
                        77,
                        if sy > previous { 0 } else { 2 },
                    );
                }
            }
            previous = sy;
        }
        Some(profile)
    }
    pub(super) fn inner_ward(&mut self) {
        let y = self.floor(0, -35);
        let min_z = -63;
        let max_z = -15;
        for x in -26_i64..=26 {
            for z in [min_z, max_z] {
                if z == max_z && x.abs() <= 4 {
                    continue;
                }
                let y = if self.terrain_adapted() {
                    self.floor(x, z)
                } else {
                    y
                };
                self.fill([x, y, z], [x, y + 6, z], 43);
                if x.rem_euclid(3) != 0 {
                    self.set([x, y + 7, z], castle::BATTLEMENT_STRAIGHT);
                }
            }
        }
        for z in min_z..=max_z {
            for x in [-26, 26] {
                let y = if self.terrain_adapted() {
                    self.floor(x, z)
                } else {
                    y
                };
                self.fill([x, y, z], [x, y + 6, z], 43);
                if z.rem_euclid(3) != 0 {
                    self.set([x, y + 7, z], castle::BATTLEMENT_STRAIGHT);
                }
            }
        }
        for x in [-25, 25] {
            for z in [-61, -17] {
                let base = if self.terrain_adapted() {
                    self.floor(x, z)
                } else {
                    y
                };
                self.tower(x, z, base, 4, 12, true);
            }
        }
        self.keep(0, -40, y);
        for dx in [-7, 7] {
            self.banner(dx, y + 4, max_z);
        }
        if !self.terrain_adapted() {
            self.fill([-4, y, -16], [4, y + 4, -14], 0);
        }
        if self.winter {
            for x in [-10, 10] {
                self.tower(x, -42, y + 24, 3, 10, true);
            }
        }
    }
    fn keep(&mut self, x: i64, z: i64, y: i64) {
        let (rx, rz, height) = (13, 11, 24);
        let y = self.local_floor(x - rx - 4, z - rz - 4, 2 * rx + 8, 2 * rz + 8, y);
        if self.terrain_adapted() {
            self.bounded_foundation(x - rx, z - rz, 2 * rx, 2 * rz, y);
        }
        self.fill([x - rx, y, z - rz], [x + rx, y + height + 4, z + rz], 0);
        for level in 0..height {
            let material = self.masonry(level);
            for dx in -rx..=rx {
                for dz in [-rz, rz] {
                    self.set([x + dx, y + level, z + dz], material);
                }
            }
            for dz in -rz..=rz {
                for dx in [-rx, rx] {
                    self.set([x + dx, y + level, z + dz], material);
                }
            }
        }
        for level in [0, 6, 12, 18, 24] {
            self.fill(
                [x - rx, y + level - 1, z - rz],
                [x + rx, y + level - 1, z + rz],
                if level == 0 { 43 } else { 16 },
            );
        }
        for dx in (-rx + 2..rx).step_by(5) {
            for level in [3, 9, 15, 21] {
                for dz in [-rz, rz] {
                    self.state([x + dx, y + level, z + dz], castle::WINDOW, 0);
                }
            }
        }
        for dz in (-rz + 2..rz).step_by(5) {
            for level in [3, 9, 15, 21] {
                for dx in [-rx, rx] {
                    self.state([x + dx, y + level, z + dz], castle::WINDOW, 1);
                }
            }
        }
        if self.adapted() {
            self.fill([x - 1, y, z + rz], [x, y + 1, z + rz], 0);
            for dx in [-1, 0] {
                self.paired([x + dx, y, z + rz], [x + dx, y + 1, z + rz], 65, 2);
            }
        } else {
            self.fill([x - 2, y, z + rz], [x + 2, y + 3, z + rz], 0);
            for (a, b) in [(x - 1, x), (x + 1, x + 2)] {
                self.paired([a, y, z + rz], [a, y + 1, z + rz], 65, 2);
                self.set([b, y + 2, z + rz], 0);
            }
        }
        // Three straight flights reach all four furnished keep storeys.
        for floor in 0..3 {
            for step in 0..6 {
                let sy = y + floor * 6 + step;
                let sz = z + rz - 3 - step;
                for dx in -rx + 6..=-rx + 7 {
                    self.state([x + dx, sy, sz], 77, 2);
                    self.fill([x + dx, sy + 1, sz], [x + dx, sy + 3, sz], 0);
                }
            }
        }
        for xoff in [-12, 12] {
            for zoff in [-10, 10] {
                self.tower(x + xoff, z + zoff, y, 4, 30, true);
            }
        }
        self.fill(
            [x - 11, y + 24, z - 9],
            [x + 11, y + 24, z + 9],
            self.roof(),
        );
        for level in 1..=9 {
            for dz in -9 + level..=9 - level {
                for dx in -11..=11 {
                    self.set([x + dx, y + 24 + level, z + dz], self.roof());
                }
            }
        }
        if self.terrain_adapted() {
            self.natural_access([x, y, z + rz]);
        }
        self.banner(x, y + 15, z + rz);
        self.set([x, y, z - 7], 104);
        self.fill([x - 2, y, z - 8], [x + 2, y, z - 8], 105);
        self.chest([x + 7, y, z - 6], BuildingRole::Keep);
        self.summary(
            BuildingRole::Keep,
            [x, y, z + rz],
            [x - rx - 4, y - 2, z - rz - 4],
            [x + rx + 4, y + 40, z + rz + 4],
        );
    }
}
