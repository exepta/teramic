//! Coherent cave rooms, sealed terraces and rooted decoration in the existing density field.
use super::{CaveKind, TerraGenerator, biomes::smoothstep, seed::layer_seed};
use crate::water::WaterColumn;
use terra_voxel::MaterialId;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum CaveEcology {
    #[default]
    None,
    Stone,
    Salt,
    Dripstone,
    Lush,
    Waterfall,
}

#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct CaveStructure {
    pub kind: CaveEcology,
    pub center: [f64; 3],
    pub radius: f64,
    pub height: f64,
    pub floor: f64,
    pub ceiling: f64,
    pub radial: f64,
    pub water: Option<WaterColumn>,
    pub dampness: f32,
    /// Revision-4 cascades use the authority's fluid cells, never curtain meshes.
    pub dynamic_water: bool,
    connector_flow: [f32; 2],
    seed: u32,
    revision: u8,
    salt_allowed: bool,
}

#[cfg(test)]
mod tests;

impl CaveStructure {
    pub(super) fn room_feature(self, p: [f64; 3], density: f64, lush: f32) -> Option<MaterialId> {
        if !self.active() {
            return None;
        }
        let legacy = self;
        match self.kind {
            CaveEcology::Salt if self.revision >= 4 => legacy
                .material(p, density)
                .filter(|m| *m == MaterialId::SaltCrystal),
            CaveEcology::Dripstone => legacy
                .material(p, density)
                .filter(|m| *m == MaterialId::DripstoneFormation),
            CaveEcology::Lush if lush > 0.8 => legacy
                .material(p, density)
                .filter(|m| matches!(m, MaterialId::Wood | MaterialId::Leaves)),
            _ => None,
        }
    }
    pub fn active(self) -> bool {
        self.kind != CaveEcology::None && self.radial < 1.0 && self.ceiling - self.floor > 3.0
    }
    pub fn open_to_sky(self) -> bool {
        self.kind == CaveEcology::Lush && self.seed.is_multiple_of(3) && self.radial < 0.14
    }

    pub fn river_flow(self, x: f64) -> [f32; 2] {
        if self.dynamic_water && self.active() {
            return [-0.45, 0.0];
        }
        if !self.active() {
            return self.connector_flow;
        }
        if self.revision < 2
            || !self.active()
            || self.water.is_none()
            || !matches!(self.kind, CaveEcology::Salt | CaveEcology::Stone)
        {
            return [0.0; 2];
        }
        let u = x - self.center[0];
        let slope = (u * 0.055 + f64::from(self.seed % 19)).cos() * 0.44 + (u * 0.12).cos() * 0.24;
        [1.0, slope as f32]
    }

    fn radial_at(self, x: f64, z: f64) -> f64 {
        if self.revision >= 3 {
            let phase = f64::from(self.seed % 628) * 0.01;
            let dx = x - self.center[0];
            let dz = z - self.center[2];
            let u = (dx * phase.cos() + dz * phase.sin()) / self.radius;
            let v = (dz * phase.cos() - dx * phase.sin()) / self.radius;
            let aspect = 0.48 + f64::from((self.seed >> 8) % 40) * 0.01;
            let lobe = if self.seed & 1 == 0 {
                ((u + 0.27).powi(2) / 0.62_f64.powi(2) + (v / aspect).powi(2))
                    .sqrt()
                    .min(
                        ((u - 0.34).powi(2) / 0.52_f64.powi(2)
                            + ((v - 0.14) / (aspect * 0.77)).powi(2))
                        .sqrt(),
                    )
            } else {
                (u * u + (v / aspect).powi(2)).sqrt()
            };
            return (lobe
                * (1.0 + (u * 7.0 + phase).sin() * 0.13 + (v * 9.0 - phase).sin() * 0.09))
                .max((u * u + v * v).sqrt());
        }
        let u = (x - self.center[0]) / self.radius;
        let v = (z - self.center[2]) / (self.radius * 0.88);
        let radial = (u * u + v * v).sqrt();
        if self.revision >= 2 {
            radial
                * (1.0
                    + ((x - self.center[0]) * 0.044).sin() * 0.075
                    + ((z - self.center[2]) * 0.057).sin() * 0.055)
        } else {
            radial
        }
    }

    fn waterfall_shelf(self, u: f64, v: f64) -> [f64; 2] {
        let lip = u + (v * 0.13).sin() * 1.6 + (v * 0.31).sin() * 0.7;
        let channel = v + f64::from(self.seed % 11) - 5.0;
        let outlet = (1.0 - smoothstep(2.6, 5.0, (channel + 7.0).abs()))
            .max(1.0 - smoothstep(3.4, 6.0, (channel - 10.0).abs()));
        let level = if lip < 3.0 { 0.0 } else { 8.0 };
        // Rounded rock shoulders seal the upper basin; only notches have a sheer fall.
        let bank = smoothstep(-4.0, 10.0, lip) * 8.0
            + (1.0 - smoothstep(0.0, 6.0, (lip - 3.0).abs())) * 10.0;
        [level, bank * (1.0 - outlet) + level * outlet]
    }

    fn geometry(self, x: f64, z: f64) -> [f64; 2] {
        let r = self.radial_at(x, z);
        let u = x - self.center[0];
        let v = z - self.center[2];
        let base = self.center[1] - self.height * 0.72;
        let phase = if self.revision >= 3 {
            f64::from(self.seed % 628) * 0.01
        } else {
            0.0
        };
        let rough =
            (u * 0.19 + v * 0.11 + phase).sin() * 0.72 + (v * 0.27 - u * 0.13 - phase).sin() * 0.50;
        let rim = smoothstep(0.62, 1.02, r) * self.height * 1.55;
        let basin_r = if self.kind == CaveEcology::Lush {
            0.33
        } else {
            0.61
        };
        let basin =
            if self.revision >= 2 && matches!(self.kind, CaveEcology::Salt | CaveEcology::Stone) {
                // A meandering river with locally wider pools, bounded by dry shelves.
                let center =
                    (u * 0.055 + f64::from(self.seed % 19)).sin() * 8.0 + (u * 0.12).sin() * 2.0;
                let width = 4.5 + (u * 0.07).sin() * 1.8;
                1.0 - smoothstep(width, width + 3.0, (v - center).abs())
            } else {
                1.0 - smoothstep(basin_r - 0.09, basin_r + 0.09, r)
            };
        let shelf = if self.kind == CaveEcology::Waterfall {
            self.waterfall_shelf(u, v)[1]
        } else {
            0.0
        };
        let terraces = if self.revision >= 2 && self.kind == CaveEcology::Salt {
            smoothstep(0.24, 0.52, r) * 2.5 * (1.0 - basin)
        } else {
            0.0
        };
        let relief = if self.revision >= 3 {
            ((u * 0.055 + phase).sin() * (v * 0.061 - phase).cos())
                * (1.5 + f64::from((self.seed >> 10) % 7))
                * (1.0 - basin * 0.9)
        } else {
            0.0
        };
        let floor = base + rim + rough - basin * 3.5 + shelf + terraces + relief;
        let ceiling = self.center[1]
            + self.height * (1.0 - r * r).max(0.0).sqrt() * 0.84
            + rough * 0.45
            + if self.revision >= 3 {
                (u * 0.07 + phase).cos() * (v * 0.09).sin() * self.height * 0.22
            } else {
                0.0
            };
        if self.revision >= 4 {
            let shelf = match self.kind {
                CaveEcology::Salt => {
                    let wave = (u * 0.042 + phase).sin() * 3.0 + (v * 0.051 - phase).cos() * 2.0;
                    let terraces = ((r * 5.0 + wave * 0.12).max(0.0)).floor() * 1.8;
                    base + rim - basin * 2.5 + terraces + rough * 0.3
                }
                CaveEcology::Lush => {
                    let islands = (u * 0.082 + phase).sin() * (v * 0.067).cos();
                    base + rim - basin * 2.0 + (islands * 3.0).floor() * 1.4 + rough * 0.4
                }
                CaveEcology::Waterfall => self.cascade_floor(x, z),
                _ => floor,
            };
            return [shelf.max(-118.5), ceiling];
        }
        [floor.max(-118.5), ceiling]
    }

    fn cascade_stage(self, x: f64, z: f64) -> (f64, f64, f64) {
        let u = x - self.center[0];
        let v = z - self.center[2];
        let bend = (v * 0.085 + f64::from(self.seed % 11)).sin() * 2.0;
        let along = u + bend;
        let step = 6.0 + f64::from(self.seed % 5);
        let stage = if along > self.radius * 0.22 {
            2.0
        } else if along > -self.radius * 0.24 {
            1.0
        } else {
            0.0
        };
        let level = (self.center[1] - self.height * 0.72).floor() + stage * step;
        let cross = v - (u * 0.045 + f64::from(self.seed % 7)).sin() * self.radius * 0.12;
        (along, level, cross)
    }

    fn cascade_floor(self, x: f64, z: f64) -> f64 {
        let (along, level, cross) = self.cascade_stage(x, z);
        let r = self.radial_at(x, z);
        let bank = smoothstep(2.5, self.radius * 0.35, cross.abs()) * 7.0;
        let base = (self.center[1] - self.height * 0.72).floor();
        let shoulder = base
            + (6.0 + f64::from(self.seed % 5))
                * [-self.radius * 0.24, self.radius * 0.22]
                    .into_iter()
                    .map(|edge| smoothstep(edge - 5.0, edge + 5.0, along))
                    .sum::<f64>();
        let side = smoothstep(3.0, self.radius * 0.30, cross.abs());
        level + (shoulder - level) * side - 2.5
            + bank
            + smoothstep(0.65, 1.02, r) * self.height * 1.55
            + (x * 0.21 + z * 0.17).sin() * 0.22
    }

    fn cascade_water(self, x: f64, z: f64) -> Option<WaterColumn> {
        let (along, level, _) = self.cascade_stage(x, z);
        // Dry receiving strips leave room for real falling cells between pools.
        let falling = [-self.radius * 0.24, self.radius * 0.22]
            .iter()
            .any(|edge| along < *edge && along > *edge - 3.0);
        let [floor, ceiling] = self.geometry(x, z);
        let bed = (floor - 0.5).floor() + 1.0;
        (!falling && self.radial_at(x, z) < 0.92 && bed < level && level + 2.0 < ceiling).then_some(
            WaterColumn {
                level,
                bed,
                depth: level - bed,
            },
        )
    }

    /// Dry cells beside source lips seed the existing bounded fluid scheduler.
    pub fn fluid_outlet(self, x: f64, z: f64) -> Option<[i64; 3]> {
        if !self.dynamic_water || !self.active() {
            return None;
        }
        let water = self.water?;
        if self.cascade_water(x - 1.0, z).is_none()
            && self.geometry(x - 1.0, z)[0] < water.level - 4.0
            && self.radial_at(x - 1.0, z) < 0.85
        {
            Some([
                x.floor() as i64 - 1,
                water.level.ceil() as i64 - 1,
                z.floor() as i64,
            ])
        } else {
            None
        }
    }

    pub(super) fn carve(self, y: f64, old: f64, surface: f64) -> f64 {
        if self.open_to_sky() && y > self.floor && y <= surface + 2.0 {
            return old.max((y - self.floor).min((0.14 - self.radial) * self.radius));
        }
        if !self.active() || surface - y < 5.0 {
            return old;
        }
        if (self.floor - 3.0..=self.ceiling + 3.0).contains(&y) {
            (y - self.floor).min(self.ceiling - y)
        } else {
            old
        }
    }

    pub(super) fn material(self, [x, y, z]: [f64; 3], density: f64) -> Option<MaterialId> {
        if !self.active() {
            return None;
        }
        let floor = (self.floor - 0.5).floor() as i64;
        let ceiling = (self.ceiling - 0.5).ceil() as i64;
        let iy = y.floor() as i64;
        let inside = y > self.floor && y < self.ceiling;
        if density <= 0.0 && (y - self.floor).abs().min((y - self.ceiling).abs()) < 2.4 {
            return Some(match self.kind {
                CaveEcology::Salt
                    if self.revision < 2
                        || self.salt_allowed
                            && ((x * 0.16 + z * 0.09).sin() + (z * 0.21 - y * 0.13).sin())
                                > 0.35 =>
                {
                    MaterialId::SaltOre
                }
                CaveEcology::Dripstone => MaterialId::Dripstone,
                CaveEcology::Lush if y < self.floor => {
                    if iy == floor && self.water.is_none() {
                        MaterialId::Grass
                    } else {
                        MaterialId::Dirt
                    }
                }
                CaveEcology::Waterfall => MaterialId::MossyStone,
                _ => MaterialId::Stone,
            });
        }
        if !inside || density <= 0.0 {
            return None;
        }
        if self.kind == CaveEcology::Salt && self.revision >= 2 && !self.salt_allowed {
            return None;
        }
        if matches!(
            self.kind,
            CaveEcology::Salt | CaveEcology::Dripstone | CaveEcology::Waterfall
        ) {
            let spacing = if self.kind == CaveEcology::Dripstone {
                11.0
            } else {
                8.0
            };
            let cx = (x / spacing).floor() as i64;
            let cz = (z / spacing).floor() as i64;
            let ticket = layer_seed(
                u64::from(self.seed)
                    ^ (cx as u64).wrapping_mul(0x9e3779b97f4a7c15)
                    ^ (cz as u64).wrapping_mul(0xbf58476d1ce4e5b9),
                280,
            );
            let center = [
                cx as f64 * spacing + 2.5 + f64::from(ticket % 3),
                cz as f64 * spacing + 2.5 + f64::from((ticket >> 4) % 3),
            ];
            let distance = (x - center[0]).hypot(z - center[1]);
            let span = ceiling - floor - 1;
            if self.kind == CaveEcology::Dripstone && ticket.is_multiple_of(5) && self.radial < 0.85
            {
                let t = ((y - self.floor) / (self.ceiling - self.floor)).clamp(0.0, 1.0);
                let radius = 0.95 + (t - 0.5).abs() * 2.3 + (y * 0.7).sin() * 0.10;
                if distance < radius {
                    return Some(MaterialId::DripstoneFormation);
                }
            }
            if self.revision >= 4
                && self.kind == CaveEcology::Salt
                && ticket.is_multiple_of(29)
                && self.radial < 0.8
                && span > 8
                && span < 28
                && distance < 1.3
            {
                return Some(MaterialId::SaltCrystal);
            }
            if ticket % 4 < if self.revision >= 2 { 1 } else { 2 } && self.radial < 0.87 {
                let length = if self.kind == CaveEcology::Salt {
                    3 + i64::from((ticket >> 8) % 8)
                } else {
                    5 + i64::from((ticket >> 8) % 18)
                }
                .min((span - 2).max(0));
                let hanging = ticket & 8 == 0;
                let distance_from_root = if hanging {
                    ceiling - 1 - iy
                } else {
                    iy - floor - 1
                };
                let (length, width) = if self.revision >= 2 {
                    let cell = layer_seed(
                        u64::from(ticket)
                            ^ (x.floor() as i64 as u64).wrapping_mul(719)
                            ^ (z.floor() as i64 as u64).wrapping_mul(313),
                        286,
                    );
                    // Staggered shafts form a compact mineral cluster, not a
                    // repeated single spike or a solid wall of crystals.
                    (
                        (length - i64::from(cell % 4)).max(2).min(length),
                        if distance_from_root < 2 { 1.6 } else { 1.15 },
                    )
                } else {
                    (length, if distance_from_root < 2 { 1.25 } else { 0.46 })
                };
                if (0..length).contains(&distance_from_root) && distance < width {
                    return Some(match self.kind {
                        CaveEcology::Salt => MaterialId::SaltCrystal,
                        CaveEcology::Waterfall => MaterialId::IceCrystal,
                        _ => MaterialId::DripstoneTip,
                    });
                }
            }
        }
        if self.kind == CaveEcology::Lush {
            let cx = (x / 13.0).floor() as i64;
            let cz = (z / 13.0).floor() as i64;
            let ticket = layer_seed(
                u64::from(self.seed)
                    ^ (cx as u64).wrapping_mul(79_129)
                    ^ (cz as u64).wrapping_mul(97_973),
                281,
            );
            let tree = [cx as f64 * 13.0 + 6.5, cz as f64 * 13.0 + 6.5];
            let tree_floor = (self.geometry(tree[0], tree[1])[0] - 0.5).floor() + 1.0;
            let tx = x - tree[0];
            let tz = z - tree[1];
            let ty = y - tree_floor;
            if ticket.is_multiple_of(7)
                && self.radial_at(tree[0], tree[1]) < 0.77
                && self.water.is_none()
            {
                let height = 3.5 + f64::from((ticket >> 8) % 3) * 0.5;
                if (0.0..height).contains(&ty) && tx.abs() < 0.5 && tz.abs() < 0.5 {
                    return Some(MaterialId::Wood);
                }
                if (1.8..height + 1.5).contains(&ty)
                    && (tx / 2.1).powi(2) + (tz / 1.8).powi(2) + ((ty - height) / 1.9).powi(2) < 1.0
                {
                    return Some(MaterialId::Leaves);
                }
            }
            let cell = layer_seed(
                u64::from(self.seed)
                    ^ (x.floor() as i64 as u64).wrapping_mul(719)
                    ^ (z.floor() as i64 as u64).wrapping_mul(313),
                282,
            );
            if !self.open_to_sky()
                && cell.is_multiple_of(37)
                && iy < ceiling
                && iy >= ceiling - 2 - i64::from(cell % 5)
            {
                return Some(MaterialId::CaveVine);
            }
            if !self.open_to_sky()
                && cell.is_multiple_of(109)
                && iy < ceiling
                && iy >= ceiling - 2 - i64::from(cell % 4)
            {
                return Some(MaterialId::CaveRoot);
            }
            if self.water.is_none() && iy == floor + 1 && cell.is_multiple_of(23) {
                return Some(MaterialId::CaveMushroom);
            }
        }
        None
    }

    pub(super) fn prop(
        self,
        position: [i64; 3],
        section: terra_voxel::SectionPos,
    ) -> Option<crate::props::WorldProp> {
        if self.revision >= 3
            || self.kind != CaveEcology::Lush
            || !self.active()
            || self.water.is_some()
        {
            return None;
        }
        let id = u64::from(self.seed)
            ^ (position[0] as u64).wrapping_mul(0x9e3779b97f4a7c15)
            ^ (position[1] as u64).wrapping_mul(0x94d049bb133111eb)
            ^ (position[2] as u64).wrapping_mul(0xbf58476d1ce4e5b9);
        let hash = layer_seed(id, 283);
        if hash % 100 >= 15 {
            return None;
        }
        use crate::props::PropKind;
        let kind = match hash % 5 {
            0 => PropKind::Bush,
            1 => PropKind::Fern,
            2 => PropKind::Mushroom,
            _ => PropKind::Grass,
        };
        Some(crate::props::WorldProp {
            id: crate::props::PropId(id),
            kind,
            position: [
                position[0] as f64 + 0.5,
                position[1] as f64 + 1.0,
                position[2] as f64 + 0.5,
            ],
            rotation: (hash % 628) as f32 * 0.01,
            scale: 0.5 + (hash % 40) as f32 * 0.01,
            variant: (hash % 8) as u8,
            section,
            removed: false,
        })
    }
}

impl TerraGenerator {
    pub fn cave_structure_at(&self, x: f64, z: f64) -> CaveStructure {
        let mut room = self.cave_room_at(x, z);
        if self.config().cave_structure_revision >= 3 && self.config().caves && !room.active() {
            if let Some(sample) = self.connector_water_at(x, z) {
                room.water = Some(sample.water);
                room.connector_flow = sample.flow;
            } else {
                room.water = None;
            }
        }
        room
    }

    pub(super) fn cave_room_at(&self, x: f64, z: f64) -> CaveStructure {
        if self.config().cave_structure_revision == 0
            || self.config().geology_revision < 4
            || !self.config().caves
        {
            return CaveStructure::default();
        }
        let cell = [(x / 192.0).floor() as i64, (z / 192.0).floor() as i64];
        let region = self.cave_region(cell);
        if region.kind == CaveKind::Spaghetti {
            return CaveStructure::default();
        }
        let climate = region.climate;
        let surface = region.surface;
        let seed = layer_seed(
            self.config().seed.0
                ^ (cell[0] as u64).wrapping_mul(0x9e3779b97f4a7c15)
                ^ (cell[1] as u64).wrapping_mul(0xbf58476d1ce4e5b9),
            284,
        );
        let salt_allowed = region.salt
            && (self.config().cave_structure_revision >= 3
                || self.climate_biome_at(x, z).weights.cliffted_desert > 0.5);
        let kind = if self.config().cave_structure_revision >= 3 {
            match region.ecology {
                super::CaveBiome::Salt => CaveEcology::Salt,
                super::CaveBiome::Lush => CaveEcology::Lush,
                super::CaveBiome::Cascade => CaveEcology::Waterfall,
                super::CaveBiome::Normal if region.kind == CaveKind::Dripstone => {
                    CaveEcology::Dripstone
                }
                _ => CaveEcology::Stone,
            }
        } else if region.salt && (self.config().cave_structure_revision >= 2 || salt_allowed) {
            CaveEcology::Salt
        } else if region.kind == CaveKind::Dripstone {
            CaveEcology::Dripstone
        } else if surface >= 80.0 && climate.weights.highland > 0.20 {
            CaveEcology::Waterfall
        } else if climate.weights.forest + climate.weights.plains + climate.weights.mushroom_park
            > 0.5
            && surface > self.config().sea_level + 5.0
        {
            CaveEcology::Lush
        } else {
            CaveEcology::Stone
        };
        let mut center = region.center;
        if kind == CaveEcology::Lush {
            center[1] = (surface - region.height - 9.0).max(-65.0);
        }
        if kind == CaveEcology::Waterfall {
            center[1] = (surface - region.height - 18.0).max(-85.0);
        }
        let mut sample = CaveStructure {
            kind,
            center,
            radius: region.radius,
            height: region.height,
            seed,
            revision: self.config().cave_structure_revision,
            salt_allowed,
            dynamic_water: self.config().cave_structure_revision >= 4
                && kind == CaveEcology::Waterfall,
            ..Default::default()
        };
        sample.radial = sample.radial_at(x, z);
        [sample.floor, sample.ceiling] = sample.geometry(x, z);
        sample.dampness = match kind {
            CaveEcology::Salt => 0.55,
            CaveEcology::Waterfall => 0.65,
            CaveEcology::Lush => 0.35,
            _ => 0.0,
        };
        if sample.dynamic_water {
            sample.water = self
                .config()
                .water
                .then(|| sample.cascade_water(x, z))
                .flatten();
            return sample;
        }
        if self.config().water
            && sample.active()
            && matches!(
                kind,
                CaveEcology::Salt | CaveEcology::Waterfall | CaveEcology::Lush | CaveEcology::Stone
            )
        {
            let shelf = if kind == CaveEcology::Waterfall {
                sample.waterfall_shelf(x - center[0], z - center[2])[0]
            } else {
                0.0
            };
            let level = (center[1] - region.height * 0.72 - 0.25).floor() + shelf;
            let bed = (sample.floor - 0.5).floor() + 1.0;
            if bed < level && sample.ceiling > level + 2.0 {
                sample.water = Some(WaterColumn {
                    level,
                    bed,
                    depth: level - bed,
                });
            }
        }
        sample
    }
}
