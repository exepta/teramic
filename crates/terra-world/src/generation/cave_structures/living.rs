//! Column-local ledges, bridges, ice curtains and rooted life in R12 galleries.
use super::{CaveEcology, CaveStructure, MaterialId, layer_seed};

#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub(super) struct LivingColumn {
    pub bridge: Option<[f32; 2]>,
    shoots: [u8; 2],
    ornaments: [Option<MaterialId>; 2],
}

impl LivingColumn {
    pub(super) fn prepare(room: CaveStructure, x: f64, z: f64) -> Self {
        use MaterialId as M;
        if !room.living_gallery() || !room.active() {
            return Self::default();
        }
        let ice = room.kind == CaveEcology::Waterfall;
        let (u, v) = room.organic_axes(x, z);
        let fraction = v / room.radius;
        let phase = f64::from(room.seed % 628) * 0.01;
        let center_line =
            room.radius * 0.18 + fraction * 4.4 + (fraction * 5.0 + phase).sin() * 1.8;
        let half_width = 1.7 + fraction.abs() * 3.4;
        let across = (u - center_line).abs() / half_width;
        let wing = (fraction.abs() / 0.59).min(1.0);
        let crown =
            room.center[1] - room.height * 0.72 + 8.2 + if ice { room.cascade_step() } else { 0.0 };
        let bridge = (across < 1.0
            && fraction.abs() < 0.61
            && crown < room.ceiling - 4.0
            && crown > room.floor + 1.0)
            .then(|| {
                let top = crown + 0.68 - wing * wing * 2.65 - across * across * 0.58;
                let bottom = top - 1.45 - wing.powi(3) * 7.6;
                [bottom as f32, top as f32]
            });

        let ix = x.floor() as i64;
        let iz = z.floor() as i64;
        let hash = |a: i64, b: i64, salt: u64| {
            layer_seed(
                u64::from(room.seed)
                    ^ (a as u64).wrapping_mul(0x9e3779b97f4a7c15)
                    ^ (b as u64).wrapping_mul(0xbf58476d1ce4e5b9),
                salt,
            )
        };
        let ticket = hash(ix, iz, 537);
        // A colony shares a centre and habit. The main stem and its shorter
        // daughters taper toward the edge, with room between successive colonies.
        let colony = |size: i64, salt: u64| {
            let cx = ix.div_euclid(size);
            let cz = iz.div_euclid(size);
            let h = hash(cx, cz, salt);
            let margin = 2;
            let inside = (size - margin * 2) as u32;
            let dx = ix.rem_euclid(size) - margin - i64::from(h % inside);
            let dz = iz.rem_euclid(size) - margin - i64::from((h >> 9) % inside);
            (h, (dx as f64).hypot(dz as f64))
        };
        let (floor_hash, floor_dist) = colony(8, 538);
        let (roof_hash, roof_dist) = colony(6, 539);
        let max_height = ((room.ceiling - room.floor) * 0.44).clamp(0.0, 10.0) as u8;
        let length = |h: u32, d: f64, width: f64| {
            let height = (4.0 + f64::from(h % 7)) * (1.0 - d / width).max(0.0);
            (height.round() as u8).min(max_height)
        };
        let dry = room.water.is_none();
        let mut shoots = [0, 0];
        let mut ornaments = [None, None];
        if ice {
            if dry && !floor_hash.is_multiple_of(3) {
                shoots[0] = length(floor_hash, floor_dist, 2.4);
            }
            if dry
                && !room.open_to_sky()
                && floor_dist < 1.5
                && floor_hash.is_multiple_of(4)
                && room.ceiling - room.floor < 17.0
            {
                // Frozen upwellings meet the ceiling on sheltered shelves.
                shoots[0] = (room.ceiling - room.floor).ceil() as u8;
            }
            if !room.open_to_sky() && !roof_hash.is_multiple_of(5) {
                shoots[1] = length(roof_hash, roof_dist, 2.65);
            }
            if dry && shoots[0] == 0 {
                ornaments[0] = match ticket % 71 {
                    0..=2 => Some(M::FrostCluster),
                    3 if floor_dist < 3.4 => Some(M::GlacialCrystalCluster),
                    _ => None,
                };
            }
            if shoots[1] == 0 && ticket.is_multiple_of(47) {
                ornaments[1] = Some(M::FrostVine);
            }
        } else {
            if !room.open_to_sky() && roof_dist < 2.6 && ticket.is_multiple_of(4) {
                shoots[1] = length(roof_hash, roof_dist, 3.4).max(1);
                ornaments[1] = Some(if roof_hash.is_multiple_of(6) {
                    M::CaveRoot
                } else {
                    M::CaveVine
                });
            } else if roof_dist < 2.8 && ticket.is_multiple_of(17) {
                ornaments[1] = Some(M::GlowVine);
            }
            if dry || bridge.is_some() {
                // Gold mushrooms follow damp seams in colonies interspersed with ferns.
                let fruit = !floor_hash.is_multiple_of(3)
                    && floor_dist < 2.45
                    && (floor_dist < 0.6 || ticket.is_multiple_of(3));
                ornaments[0] = if fruit {
                    Some(M::CaveMushroom)
                } else {
                    match ticket % 65 {
                        0..=3 => Some(M::ForestFern),
                        4 if floor_dist < 3.0 => Some(M::FloweringShrub),
                        8..=13 if floor_dist < 3.1 => Some(M::CaveWildflowers),
                        14 if room.floor < crown - 5.0 => Some(M::ShoreReeds),
                        _ => None,
                    }
                };
            }
        }
        Self {
            bridge,
            shoots,
            ornaments,
        }
    }

    pub(super) fn density(self, air: f64, y: f64) -> f64 {
        self.bridge.map_or(air, |[base, top]| {
            air.min((f64::from(base) - y).max(y - f64::from(top)))
        })
    }

    pub(super) fn feature(self, room: CaveStructure, y: f64) -> Option<MaterialId> {
        let floor = (room.floor - 0.5).floor() as i64;
        let roof = (room.ceiling - 0.5).ceil() as i64;
        let iy = y.floor() as i64;
        let above = iy - floor;
        let below = roof - iy;
        if room.kind == CaveEcology::Waterfall {
            if (1..=i64::from(self.shoots[0])).contains(&above)
                || (1..=i64::from(self.shoots[1])).contains(&below)
            {
                return Some(MaterialId::IceCrystal);
            }
        } else if (1..=i64::from(self.shoots[1])).contains(&below) {
            return self.ornaments[1];
        }
        if below == 1 {
            return self.ornaments[1];
        }
        let on_bridge = self
            .bridge
            .is_some_and(|[_, top]| iy == (f64::from(top) - 0.5).floor() as i64 + 1);
        if above == 1 && room.water.is_none() || on_bridge {
            self.ornaments[0]
        } else {
            None
        }
    }
}

impl CaveStructure {
    pub(super) fn living_gallery(self) -> bool {
        self.revision >= 12 && matches!(self.kind, CaveEcology::Lush | CaveEcology::Waterfall)
    }

    pub(in crate::generation) fn living_shell(self, y: f64) -> bool {
        self.living_gallery()
            && !self.suppressed
            && self.radial < 1.035
            && (self.floor.min(self.ceiling) - 3.0..=self.floor.max(self.ceiling) + 3.0)
                .contains(&y)
    }

    pub(in crate::generation) fn living_rock(
        self,
        y: f64,
        patch: f64,
        floor: bool,
        ceiling: bool,
    ) -> MaterialId {
        use MaterialId as M;
        let bed = self.center[1] - self.height * 0.72;
        let water = self.water.is_some_and(|w| y < w.level + 0.65);
        if self.kind == CaveEcology::Lush {
            if floor && !water {
                if patch > 0.47 {
                    M::GlowMoss
                } else if patch > -0.29 {
                    M::CaveGrass
                } else {
                    M::CaveMoss
                }
            } else if !water && y > self.floor - 3.0 && y < self.floor && patch > -0.22 {
                M::RootedCaveDirt
            } else if ceiling && patch > 0.07 || patch > 0.33 {
                M::OvergrownStone
            } else if patch > -0.12 {
                M::MossyCaveStone
            } else {
                M::LushCaveStone
            }
        } else if floor && !water {
            if patch < -0.25 {
                M::FrostedCaveIce
            } else {
                M::SnowDustStone
            }
        } else if ceiling && patch > 0.12 {
            M::FrostedCaveIce
        } else if patch > 0.57 && (y * 0.23).sin() > 0.34 {
            M::BlueCrystalBlock
        } else if water && patch > -0.35 || patch > 0.35 && y < bed + 13.0 {
            M::ClearCaveIce
        } else if patch > -0.15 {
            M::PackedCaveIce
        } else {
            M::FrozenStone
        }
    }
}
