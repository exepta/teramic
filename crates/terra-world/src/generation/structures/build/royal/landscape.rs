//! Revision-seven earthworks retain the hillside instead of cutting field pits
//! or ending the courtyard platform in a vertical dirt cylinder.
use super::*;

impl Royal<'_, '_> {
    fn natural_floor(&self, x: i64, z: i64) -> i64 {
        let point = self.point([x, 0, z]);
        self.builder.ground(point[0], point[2]) - self.origin[1]
    }
    pub(super) fn exterior_floor(&self, x: i64, z: i64) -> i64 {
        self.blend_floor(x, z, self.natural_floor(x, z))
    }
    fn blend_floor(&self, x: i64, z: i64, natural: i64) -> i64 {
        let radius = if self.outpost { 31.0 } else { 72.0 };
        let distance = (x as f64).hypot(z as f64);
        if distance <= radius {
            return self.floor(x, z);
        }
        let mut t = ((distance - radius) / 24.0).clamp(0.0, 1.0);
        if !self.outpost {
            // The hillside field is a second natural-height anchor: blending
            // towards it avoids excavating a trench around its untouched soil.
            let field_distance = ((x + 68).abs() - 10).max(0) as f64;
            let field_distance = field_distance.hypot(((z - 78).abs() - 10).max(0) as f64);
            let from_wall = distance - radius;
            t = t.max(from_wall / (from_wall + field_distance));
        }
        let edge_x = (x as f64 * radius / distance).round() as i64;
        let edge_z = (z as f64 * radius / distance).round() as i64;
        let edge = self.floor(edge_x, edge_z);
        ((1.0 - t) * edge as f64 + t * natural as f64).round() as i64
    }
    pub(super) fn blend_exterior(&mut self) {
        let radius = if self.outpost { 31_i64 } else { 72_i64 };
        // radius72+24 stays inside the regional [96,288] plan lookup envelope.
        for z in -radius - 24..=radius + 24 {
            for x in -radius - 24..=radius + 24 {
                let r2 = x * x + z * z;
                if r2 <= radius * radius || r2 >= (radius + 24) * (radius + 24) {
                    continue;
                }
                let natural = self.natural_floor(x, z);
                let target = self.blend_floor(x, z, natural);
                if target == natural {
                    continue;
                }
                // A shallow continuous soil cap is sufficient for earthworks;
                // only the joining road needs the expensive actual-cave scan.
                let support = natural.min(target - 1);
                self.fill([x, support - 2, z], [x, target - 1, z], 2);
                self.set([x, target - 1, z], if self.winter { 5 } else { 1 });
                self.fill([x, target, z], [x, (natural + 7).max(target + 3), z], 0);
            }
        }
    }
    pub(super) fn terraced_farm(&mut self, cx: i64, cz: i64, w: i64, d: i64) {
        let left = cx - w / 2;
        let north = cz - d / 2;
        let hillside = !self.outpost && cx * cx + cz * cz > 72 * 72;
        let mut heights = Vec::with_capacity(((w + 1) * (d + 1)) as usize);
        for dz in 0..=d {
            for dx in 0..=w {
                let x = left + dx;
                let z = north + dz;
                heights.push(if hillside {
                    self.natural_floor(x, z)
                } else {
                    self.floor(x, z)
                });
            }
        }
        let at = |x: i64, z: i64| heights[(x + z * (w + 1)) as usize];
        let mut low = i64::MAX;
        let mut high = i64::MIN;
        for dz in 0..=d {
            for dx in 0..=w {
                let x = left + dx;
                let z = north + dz;
                let y = at(dx, dz);
                low = low.min(y);
                high = high.max(y);
                self.fill([x, y - 2, z], [x, y - 1, z], 2);
                self.fill([x, y, z], [x, y + 3, z], 0);
                if dx == 0 || dx == w || dz == 0 || dz == d {
                    self.set([x, y - 1, z], if self.winter { 5 } else { 1 });
                    self.set([x, y, z], 90);
                    continue;
                }
                // Only local troughs hold source water: a sloping open irrigation
                // channel would escape downhill into the courtyard.
                let contained = dx == w / 2
                    && [[dx - 1, dz], [dx + 1, dz], [dx, dz - 1], [dx, dz + 1]]
                        .into_iter()
                        .all(|[x, z]| at(x, z) >= y);
                let position = self.point([x, y - 1, z]);
                if contained {
                    self.builder.object(position, 27, None, None);
                } else {
                    self.builder.set(position, 101);
                    self.builder.objects.insert(
                        position,
                        StructureObject {
                            position,
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
        let entrance = [cx, at(w / 2, d), north + d];
        self.set(entrance, 0);
        self.summary(
            BuildingRole::Farm,
            entrance,
            [left, low - 2, north],
            [left + w, high + 4, north + d],
        );
    }
}
