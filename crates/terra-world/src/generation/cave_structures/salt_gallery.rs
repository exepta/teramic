//! Cross-sections of salt galleries. All horizontal shape work is prepared once per column.
use super::{CaveEcology, CaveStructure, MaterialId, layer_seed, smoothstep};

const SURFACE_FLARE: f64 = 3.0;

#[derive(Default, Debug, Clone, Copy, PartialEq)]
pub(super) struct SaltGallery {
    pub(super) bridge: Option<[f64; 2]>,
    karst_pillar: Option<[f32; 3]>,
    skylight: f64,
    lip: f64,
    bedding: f64,
    pub(super) window: (f64, f64),
    pub(super) sky_coupling: f32,
    floor_ornament: Option<(MaterialId, u8)>,
    ceiling_length: u8,
}

impl SaltGallery {
    pub(super) fn column(room: CaveStructure, x: f64, z: f64, daylight: bool) -> Self {
        let (u, v) = room.organic_axes(x, z);
        let phase = f64::from(room.seed % 628) * 0.01;
        let water = room.center[1] - room.height * 0.72;
        // A buttressed stone bridge links opposite terraces. The underside rises
        // above the water at midspan and thickens where it meets the banks.
        let bridge_center =
            room.radius * (0.06 + phase.sin() * 0.18) + (v * 0.073 + phase).sin() * 1.6;
        let shoulder = (v / (room.radius * 0.53)).abs().min(1.0);
        let width = 1.8 + shoulder * 2.2 + (v * 0.16).cos() * 0.45;
        let cross = room.gallery.map_or(v, |g| v - g.bend(u, room.radius));
        let sideways = room.gallery.map_or((u - bridge_center).abs(), |g| {
            (g.span(u) - cross * 0.18 - (v * 0.10).sin() * 1.7).abs()
        });
        let shoulder = if room.gallery.is_some() {
            (cross.abs() / (room.radius * 0.61)).min(1.)
        } else {
            shoulder
        };
        let bridge = if sideways < width && cross.abs() < room.radius * 0.72 {
            let crown = water + (room.height * 0.31).clamp(6.0, 13.0) * (1.0 - shoulder * shoulder);
            let top = crown + 2.0 - (sideways / width).powi(3) * 0.55;
            let bottom = crown - 0.7 - shoulder.powi(3) * 5.5 + (sideways / width).powi(2) * 0.5;
            (bottom < room.ceiling && top > room.floor && top < room.ceiling - 3.0)
                .then_some([bottom, top])
        } else {
            None
        };
        // A gallery has one fracture in its roof. Its location belongs to the
        // whole room, including sections on the far side of the region seam.
        let window = room
            .gallery
            .map_or((room.radius * 0.32, -room.radius * 0.22), |g| {
                let along = room.radius * (0.20 + phase.sin() * 0.16);
                (along, g.bend(along, room.radius) - room.radius * 0.30)
            });
        let along = u - window.0;
        let (along, across) = if room.gallery.is_some() {
            // Follow a short curved joint, with broken rather than parallel lips.
            let bend = ((along * 0.09 + phase).sin() - phase.sin()) * 1.8;
            (along * (0.72 + phase.cos() * 0.09), v - window.1 - bend)
        } else {
            (along * 0.8, v - window.1)
        };
        let skylight_distance = along.hypot(across);
        let outline = if room.gallery.is_some() {
            let theta = across.atan2(along);
            1.0 + 0.12 * (theta * 3. + phase).cos() + 0.09 * (theta * 5. - phase).sin()
        } else {
            1.0
        };
        let skylight = if daylight {
            room.salt_window_radius() * outline - skylight_distance
        } else {
            0.0
        };
        let (mut floor_ornament, _) = ornament(x, z, room.seed, false);
        if let Some(gallery) = room.gallery {
            let island = gallery.island(u, v, room.radius);
            if island < 0.46 {
                let height = (5.3 - island * 7.5 + (x * 4.2 + z * 2.7).sin()).round() as u8;
                floor_ornament = Some((MaterialId::SaltStalagmite, height.max(1)));
            } else if island < 0.76 && (x.floor() as i64 + 2 * z.floor() as i64).rem_euclid(3) == 0
            {
                floor_ornament = Some((MaterialId::SaltCrystalCluster, 1));
            }
        }
        let (_, mut ceiling_length) = ornament(x, z, room.seed, true);
        if room.revision >= 11 {
            let calcite = room.kind == CaveEcology::Dripstone;
            let fan = |ceiling| {
                if room.revision >= 13 {
                    mineral_seep(x, z, room.seed, ceiling, calcite)
                } else {
                    crystal_fan(x, z, room.seed, ceiling, calcite)
                }
            };
            if let Some(height) = fan(false) {
                floor_ornament = Some((MaterialId::SaltStalagmite, height));
            }
            if let Some(height) = fan(true) {
                ceiling_length = height;
            }
        }
        let karst_pillar = room
            .gallery
            .filter(|_| room.kind == CaveEcology::Dripstone)
            .and_then(|gallery| {
                // Isolated mineral buttresses join the roof to the margin of
                // a pool. Their fluted waist stays narrow enough to walk around.
                let along = gallery.span(u - gallery.half_length * 0.23) * 0.85;
                let bank = cross - room.radius * (0.31 + 0.12 * (u * 0.05 + phase).sin());
                let theta = bank.atan2(along);
                let distance = along.hypot(bank) / (1. + 0.08 * (theta * 5. + phase).cos());
                (distance < 5.).then(|| {
                    [
                        distance as f32,
                        room.floor as f32,
                        (1. / (room.ceiling - room.floor)) as f32,
                    ]
                })
            });
        Self {
            bridge,
            karst_pillar,
            window,
            skylight,
            lip: if daylight && room.gallery.is_some() {
                1.3 + 1.1 * ((u - window.0) * 0.078 + phase).sin()
                    + 0.9 * ((v - window.1) * 0.11 - phase).cos()
            } else {
                0.
            },
            bedding: if room.gallery.is_some() {
                4.3 * (u * 0.061 + phase).cos() + 2.7 * (v * 0.12 + phase + u * 0.034).sin()
            } else {
                0.
            },
            sky_coupling: if daylight {
                (1.0 - smoothstep(room.radius * 0.14, room.radius * 1.55, skylight_distance)) as f32
            } else {
                0.0
            },
            floor_ornament,
            ceiling_length,
        }
    }

    pub(super) fn window_rim(self) -> bool {
        self.sky_coupling > 0.0 && self.skylight.abs() < 6.0 + self.lip.abs()
    }

    pub(super) fn near_window(self) -> bool {
        self.sky_coupling > 0.0 && self.skylight > -32.0
    }

    pub(super) fn daylight(self) -> bool {
        self.skylight > 0.0
    }

    pub(super) fn open_at_surface(self, flared: bool) -> bool {
        self.daylight() || flared && self.sky_coupling > 0.0 && self.skylight + SURFACE_FLARE > 0.0
    }

    pub(super) fn shaft_density(self, floor: f64, y: f64) -> f64 {
        (y - floor).min(self.skylight)
    }

    /// The chamber opens into a fractured, flared throat. Its undercut and
    /// the surface lip have different footprints, leaving sloping rock ribs.
    pub(super) fn flared_shaft(self, y: f64, ceiling: f64, surface: f64) -> f64 {
        let t = smoothstep(ceiling - 2., surface + 1., y);
        self.skylight + t * SURFACE_FLARE + (1.0 - t) * (1.3 - self.lip * 0.45)
            - self.lip * (std::f64::consts::PI * t).sin()
    }

    pub(super) fn density(self, room_density: f64, y: f64) -> f64 {
        let room_density =
            self.karst_pillar
                .map_or(room_density, |[distance, floor, inverse_span]| {
                    let axial = ((y as f32 - floor) * inverse_span * 2. - 1.).abs().min(1.);
                    let radius = 1.28 + 2.40 * axial.powi(3) + 0.13 * (y as f32 * 1.13).sin();
                    room_density.min(f64::from(distance - radius))
                });
        self.bridge.map_or(room_density, |[underside, top]| {
            room_density.min((underside - y).max(y - top))
        })
    }

    pub(super) fn rock(self, room: CaveStructure, y: f64, patch: f64, cover: f64) -> MaterialId {
        let near_floor = self.bridge.is_some_and(|[_, top]| (top - y).abs() < 1.55)
            || (room.floor - y).abs() < 1.5;
        let below_water = room.water.is_some_and(|water| y < water.level + 1.0);
        if below_water && near_floor {
            return MaterialId::WetSaltStone;
        }
        if cover > 0.0 && near_floor && patch > -0.24 {
            return MaterialId::SaltDustFloor;
        }
        if room.gallery.is_some() {
            // Frost settles on shelves. Within the cliff it follows fractures
            // and short tilted seams, with darker host rock in recessed beds.
            let bed = y + self.bedding;
            let joint = (bed * 0.37 + patch * 3.6).cos();
            if cover > 0. && patch + 0.09 * joint > 0.56 - cover * 0.32 {
                return if patch > 0.74 && joint > 0.35 {
                    MaterialId::TranslucentSaltBlock
                } else if joint > 0.85 && patch > 0.48 {
                    MaterialId::SaltFormation
                } else {
                    MaterialId::SaltStone
                };
            }
            if cover > 0. && patch > 0.22 && joint < -0.82 {
                return MaterialId::PinkSaltStone;
            }
            return if patch < -0.40 && (y < room.floor - 1. || joint < 0.4) {
                MaterialId::DeepSaltStone
            } else {
                MaterialId::SaltCaveStone
            };
        }
        let stratum = (y * 0.34 + f64::from(room.seed % 43) + patch * 2.7).sin();
        if cover > 0.0 && patch > 0.47 - cover * 0.48 {
            if stratum > 0.82 {
                return MaterialId::SaltFormation;
            }
            if patch > 0.89 - cover * 0.44 && stratum < -0.74 {
                return MaterialId::TranslucentSaltBlock;
            }
            return MaterialId::SaltStone;
        }
        // Keep a bedding seam thin, with exposed rock between successive layers.
        let seam = (y * 0.16 + patch * 0.36 + f64::from(room.seed % 11)).rem_euclid(1.0);
        if cover > 0.0 && seam < 0.12 && patch > 0.20 - cover * 0.5 {
            MaterialId::PinkSaltStone
        } else if patch < -0.23 || y < room.floor - 1.0 {
            MaterialId::DeepSaltStone
        } else {
            MaterialId::SaltCaveStone
        }
    }

    pub(super) fn decoration(self, room: CaveStructure, y: f64) -> Option<MaterialId> {
        let mut floor = room.floor;
        let mut ceiling = room.ceiling;
        if let Some([underside, top]) = self.bridge {
            if y > top {
                floor = top;
            } else if y < underside {
                ceiling = underside;
            } else {
                return None;
            }
        }
        let calcite = room.kind == CaveEcology::Dripstone;
        let iy = y.floor() as i64;
        let up = iy - (floor - 0.5).floor() as i64 - 1;
        let down = (ceiling - 0.5).ceil() as i64 - 1 - iy;
        if (!self.daylight() || ceiling < room.ceiling)
            && (0..i64::from(self.ceiling_length)).contains(&down)
            && up > 4
        {
            if room.revision >= 10 && self.ceiling_length > 1 {
                return Some(if calcite {
                    MaterialId::DripstoneTip
                } else {
                    MaterialId::SaltCrystal
                });
            }
            if calcite {
                return Some(MaterialId::DripstoneTip);
            }
            return Some(if down + 1 == i64::from(self.ceiling_length) {
                MaterialId::SaltStalactite
            } else {
                MaterialId::TranslucentSaltBlock
            });
        }
        let (tip, height) = self.floor_ornament?;
        if (0..i64::from(height)).contains(&up)
            && down > 3
            && room.water.is_none_or(|water| floor >= water.level - 0.5)
        {
            if calcite {
                return Some(match tip {
                    MaterialId::SaltRubble => MaterialId::CavePebbles,
                    MaterialId::SaltCrystalCluster if height == 1 => MaterialId::CaveCrystalCluster,
                    _ => MaterialId::DripstoneTip,
                });
            }
            if room.revision >= 10 && tip == MaterialId::SaltStalagmite && height > 1 {
                return Some(MaterialId::SaltCrystal);
            }
            return Some(if up + 1 == i64::from(height) {
                tip
            } else if tip == MaterialId::SaltPillarFragment {
                MaterialId::PinkSaltStone
            } else {
                MaterialId::SaltFormation
            });
        }
        None
    }
}

/// A spray of neighbouring prisms shares one water seep. Peak and shoulder
/// heights come from that seep, so a 3x3 fan survives section boundaries.
fn crystal_fan(x: f64, z: f64, seed: u32, ceiling: bool, calcite: bool) -> Option<u8> {
    let spacing = if calcite { 12 } else { 14 };
    let x = x.floor() as i64;
    let z = z.floor() as i64;
    let ticket = layer_seed(
        u64::from(seed)
            ^ (x.div_euclid(spacing) as u64).wrapping_mul(0x9e3779b97f4a7c15)
            ^ (z.div_euclid(spacing) as u64).wrapping_mul(0xbf58476d1ce4e5b9),
        if ceiling { 457 } else { 461 },
    );
    if ticket.is_multiple_of(4) {
        return None;
    }
    let dx = x.rem_euclid(spacing) - 2 - i64::from((ticket >> 3) % (spacing as u32 - 4));
    let dz = z.rem_euclid(spacing) - 2 - i64::from((ticket >> 12) % (spacing as u32 - 4));
    if dx.abs().max(dz.abs()) > 1 {
        return None;
    }
    let peak = if calcite {
        7 + i64::from(ticket >> 20) % 11
    } else {
        5 + i64::from(ticket >> 20) % 6
    };
    let setback = 2 * dx.abs() + 2 * dz.abs() + i64::from(dx + dz > 0);
    Some((peak - setback).max(2) as u8)
}

/// Taper neighbouring columns outwards from the same deposition site. Calcite
/// follows an elongated joint; halite builds overlapping, stepped shoulders.
fn mineral_seep(x: f64, z: f64, seed: u32, ceiling: bool, calcite: bool) -> Option<u8> {
    let spacing = if calcite { 16 } else { 18 };
    let [x, z] = [x.floor() as i64, z.floor() as i64];
    let hash = layer_seed(
        u64::from(seed)
            ^ (x.div_euclid(spacing) as u64).wrapping_mul(0x9e3779b97f4a7c15)
            ^ (z.div_euclid(spacing) as u64).wrapping_mul(0xbf58476d1ce4e5b9),
        if ceiling { 533 } else { 537 },
    );
    if hash.is_multiple_of(5) {
        return None;
    }
    let center = |bits| 4 + i64::from((hash >> bits) % (spacing as u32 - 8));
    let dx = (x.rem_euclid(spacing) - center(3)) as f64;
    let dz = (z.rem_euclid(spacing) - center(9)) as f64;
    if dx.abs().max(dz.abs()) > 4. {
        return None;
    }
    let (sin, cos) = (f64::from(hash % 628) * 0.01).sin_cos();
    let u = dx * cos + dz * sin;
    let v = dz * cos - dx * sin;
    let spread = 0.78 + f64::from((hash >> 17) % 5) * 0.09;
    let main = (u / (2.65 * spread)).hypot(v / (2.05 * spread));
    let shoulder = ((u - 1.55) / (2.05 * spread)).hypot((v + 0.65) / (1.56 * spread));
    let peak = if calcite {
        7. + f64::from((hash >> 21) % 10)
    } else {
        5. + f64::from((hash >> 21) % 8)
    };
    let length = peak * (1. - main).max(0.).powf(0.82);
    let lobe = peak * 0.63 * (1. - shoulder).max(0.).powf(0.80);
    let height = length.max(lobe);
    (height > 0.7).then(|| height.round().max(1.) as u8)
}

fn ornament(x: f64, z: f64, seed: u32, ceiling: bool) -> (Option<(MaterialId, u8)>, u8) {
    // Each gallery cell has its own offset, footprint and growth habit.
    // Individual shoots in a cluster terminate at staggered heights.
    let spacing = if ceiling { 9 } else { 8 };
    let x = x.floor() as i64;
    let z = z.floor() as i64;
    let cell_seed = layer_seed(
        u64::from(seed)
            ^ (x.div_euclid(spacing) as u64).wrapping_mul(0x9e3779b97f4a7c15)
            ^ (z.div_euclid(spacing) as u64).wrapping_mul(0xbf58476d1ce4e5b9),
        if ceiling { 441 } else { 443 },
    );
    let dx = x.rem_euclid(spacing) - 2 - i64::from((cell_seed >> 4) % 4);
    let dz = z.rem_euclid(spacing) - 2 - i64::from((cell_seed >> 10) % 4);
    let distance = dx * dx + dz * dz;
    if distance > 5 || cell_seed % 7 < 2 {
        return (None, 0);
    }
    let peak = 2 + i64::from((cell_seed >> 17) % 3);
    if ceiling {
        let length = (peak - distance - i64::from(dx.abs() > dz.abs())).max(0) as u8;
        return (None, length);
    }
    let (tip, height) = match (cell_seed >> 21) % 5 {
        0 if distance < 2 => (MaterialId::SaltPillarFragment, (peak - distance * 2) as u8),
        1 if distance < 3 => (
            MaterialId::SaltStalagmite,
            (peak - distance * 2).max(1) as u8,
        ),
        _ if distance == 0 => (MaterialId::SaltCrystalCluster, 1),
        _ if distance <= 2 && (dx + dz) % 2 == 0 => (MaterialId::SaltRubble, 1),
        _ => return (None, 0),
    };
    (Some((tip, height)), 0)
}

pub(super) fn terraces(
    base: f64,
    u: f64,
    v: f64,
    radius: f64,
    phase: f64,
    rim: f64,
    rough: f64,
) -> f64 {
    let river = (u * 0.045 + phase).sin() * radius * 0.13;
    let distance = (v - river).abs();
    // Alternating narrow channels and broad still pools. Low shelves ascend
    // around the rim and stay walkable alongside the water.
    let width = radius * (0.10 + 0.10 * (u / radius * 6.0 + phase).cos().powi(2));
    let bank = distance - width + rough * 0.8;
    let floor =
        -3.4 + smoothstep(-2.5, 2.5, bank) * 4.0 + (bank.max(0.0) / 3.7).floor().min(5.0) * 1.2;
    base + floor + rim + rough * 0.30
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn salt_seep_grows_ten_cells_and_has_shoulders_in_all_nine_columns() {
        for ceiling in [true, false] {
            let fan =
                |x, z| crystal_fan(f64::from(x) + 0.5, f64::from(z) + 0.5, 879, ceiling, false);
            let (x, z) = (-120..0)
                .flat_map(|x| (-120..0).map(move |z| (x, z)))
                .find(|&(x, z)| fan(x, z) == Some(10))
                .expect("ten-cell halite spear");
            for dx in -1..=1 {
                for dz in -1..=1 {
                    assert!(matches!(fan(x + dx, z + dz), Some(4..=10)));
                }
            }
        }
    }
}
