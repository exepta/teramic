//! Coherent cave rooms, sealed terraces and rooted decoration in the existing density field.
use super::{CaveKind, TerraGenerator, biomes::smoothstep, seed::layer_seed};
use crate::water::WaterColumn;
mod bedrock;
mod galleries;
mod living;
mod salt_gallery;
use galleries::Gallery;
pub(super) use galleries::GalleryRegion;
use salt_gallery::SaltGallery;
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
    /// Cascades and revision-5 streams use authoritative fluid cells, never curtain meshes.
    pub dynamic_water: bool,
    salt_gallery: SaltGallery,
    living: living::LivingColumn,
    gallery: Option<Gallery>,
    connector_flow: [f32; 2],
    seed: u32,
    revision: u8,
    salt_allowed: bool,
    suppressed: bool,
}

#[cfg(test)]
mod tests;

impl CaveStructure {
    pub(super) fn room_feature(self, p: [f64; 3], density: f64, lush: f32) -> Option<MaterialId> {
        if !self.active() {
            return None;
        }
        if self.living_gallery() {
            let vegetation = self.living.feature(self, p[1]);
            if vegetation.is_some() || self.kind == CaveEcology::Waterfall {
                return vegetation;
            }
            return (lush > 0.6 && self.water.is_none())
                .then(|| self.material(p, density))
                .flatten()
                .filter(|m| matches!(m, MaterialId::Wood | MaterialId::Leaves));
        }
        let legacy = self;
        match self.kind {
            CaveEcology::Dripstone if self.revision >= 11 => {
                self.salt_gallery.decoration(self, p[1])
            }
            CaveEcology::Salt if self.revision >= 9 => self.salt_gallery.decoration(self, p[1]),
            CaveEcology::Salt if self.revision >= 4 => legacy
                .material(p, density)
                .filter(|m| *m == MaterialId::SaltCrystal),
            CaveEcology::Dripstone if self.revision >= 10 => self
                .material(p, density)
                .filter(|m| matches!(m, MaterialId::DripstoneFormation | MaterialId::DripstoneTip)),
            CaveEcology::Waterfall if self.revision >= 10 && self.water.is_none() => self
                .material(p, density)
                .filter(|m| *m == MaterialId::IceCrystal),
            CaveEcology::Dripstone => legacy
                .material(p, density)
                .filter(|m| *m == MaterialId::DripstoneFormation),
            CaveEcology::Lush if lush > 0.8 => legacy
                .material(p, density)
                .filter(|m| matches!(m, MaterialId::Wood | MaterialId::Leaves)),
            _ => None,
        }
    }
    pub(super) fn salt_gallery_shell(self, y: f64) -> bool {
        self.revision >= 9
            && self.kind == CaveEcology::Salt
            && !self.suppressed
            && self.radial < 1.04
            && ((self.floor.min(self.ceiling) - 4.0..=self.floor.max(self.ceiling) + 4.0)
                .contains(&y)
                || self.salt_gallery.window_rim() && y >= self.ceiling)
    }
    pub(super) fn dripstone_shell(self, y: f64) -> bool {
        self.revision >= 11
            && self.kind == CaveEcology::Dripstone
            && !self.suppressed
            && self.radial < 1.05
            && ((self.floor.min(self.ceiling) - 7.0..=self.floor.max(self.ceiling) + 4.0)
                .contains(&y)
                || self.salt_gallery.window_rim() && y >= self.ceiling)
    }
    pub(super) fn salt_gallery_rock(self, y: f64, patch: f64, cover: f64) -> MaterialId {
        self.salt_gallery.rock(self, y, patch, cover)
    }
    pub(super) fn salt_window_wall(self, y: f64) -> bool {
        self.gallery.is_some() && y > self.ceiling - 4.0 && self.salt_gallery.window_rim()
    }
    /// Half-length and world-space strike of a multi-hall cave.
    pub fn gallery_axis(self) -> Option<(f64, [f64; 2])> {
        self.gallery
            .map(|g| (g.half_length, [g.angle.cos(), g.angle.sin()]))
    }
    /// Sample the authored outline of this single hall. No neighbouring region,
    /// climate, hydrology or terrain noise needs to be regenerated for interest.
    #[cfg(feature = "bevy")]
    pub(crate) fn gallery_shell_span(self, sx: i64, sz: i64) -> Option<[i64; 2]> {
        let mut span: Option<[f64; 2]> = None;
        for z in [0., 8., 16.] {
            for x in [0., 8., 16.] {
                let x = (sx * 16) as f64 + x;
                let z = (sz * 16) as f64 + z;
                if self.radial_at(x, z) > 1.10 {
                    continue;
                }
                let [floor, roof] = self.geometry(x, z);
                let bounds = span.get_or_insert([floor.min(roof), floor.max(roof)]);
                bounds[0] = bounds[0].min(floor.min(roof));
                bounds[1] = bounds[1].max(floor.max(roof));
            }
        }
        span.map(|[lo, hi]| {
            [
                ((lo - 8.).floor() as i64).div_euclid(16),
                ((hi + 8.).ceil() as i64).div_euclid(16),
            ]
        })
    }
    pub fn salt_window_radius(self) -> f64 {
        if self.gallery.is_some() {
            (self.radius * 0.13).clamp(4., 8.)
        } else {
            (self.radius * 0.16).clamp(4., 9.)
        }
    }
    pub fn salt_pool_at(self, y: f64) -> bool {
        self.revision >= 9
            && self.kind == CaveEcology::Salt
            && self.active()
            && self
                .water
                .is_some_and(|water| (water.bed - 1.0..=water.level + 0.5).contains(&y))
    }

    /// The generated pool or water spilling from one of its terraces. Surface
    /// rivers above the vault keep their own biome and light response.
    pub fn pool_ecology_at(self, y: f64) -> Option<CaveEcology> {
        if self.salt_pool_at(y) {
            Some(CaveEcology::Salt)
        } else if self.living_gallery()
            && self.active()
            && (self.floor - 1.0..self.ceiling).contains(&y)
        {
            Some(self.kind)
        } else {
            None
        }
    }

    pub fn active(self) -> bool {
        !self.suppressed
            && self.kind != CaveEcology::None
            && self.radial < 1.0
            && self.ceiling - self.floor > 3.0
    }
    pub fn open_to_sky(self) -> bool {
        !self.suppressed
            && (self.salt_gallery.open_at_surface(self.gallery.is_some())
                || self.kind == CaveEcology::Lush
                    && self.seed.is_multiple_of(3)
                    && self.radial < 0.14)
    }

    /// Include the walls and floor that a camera above a natural shaft can see
    /// obliquely. This uses the footprint of the actual window of this gallery.
    pub(super) fn surface_view_floor(self) -> Option<f64> {
        (self.active()
            && (self.open_to_sky()
                || self.salt_gallery.near_window()
                || self.kind == CaveEcology::Lush
                    && self.seed.is_multiple_of(3)
                    && self.radial < 0.45))
            .then_some(self.floor)
    }

    /// Natural high window seen from this chamber. Its contribution fades into
    /// the side galleries and goes to zero below the gallery floor.
    pub fn skylight(self, y: f64) -> Option<([f64; 2], f32, f32)> {
        if self.living_gallery()
            && self.kind == CaveEcology::Lush
            && self.seed.is_multiple_of(3)
            && self.active()
            && (self.floor - 1.0..self.ceiling).contains(&y)
        {
            return Some((
                [self.center[0], self.center[2]],
                (1.0 - self.radial * 0.83) as f32,
                (self.radius * 0.10).clamp(2.3, 7.0) as f32,
            ));
        }
        self.salt_skylight(y).map(|(xz, coupling)| {
            let flare = if self.gallery.is_some() { 2.5 } else { 0. };
            (xz, coupling, (self.salt_window_radius() + flare) as f32)
        })
    }

    pub fn salt_skylight(self, y: f64) -> Option<([f64; 2], f32)> {
        let coupling = self.salt_gallery.sky_coupling;
        if coupling <= 0.0 || !(self.floor - 0.5..=self.ceiling + 0.5).contains(&y) {
            return None;
        }
        let (s, c) = self.axis_angle().sin_cos();
        let (u, v) = self.salt_gallery.window;
        Some((
            [
                self.center[0] + u * c - v * s,
                self.center[2] + u * s + v * c,
            ],
            coupling,
        ))
    }

    pub fn river_flow(self, x: f64) -> [f32; 2] {
        if self.dynamic_water && self.active() {
            if self.revision >= 5 {
                let phase = self.axis_angle();
                return [(-0.45 * phase.cos()) as f32, (-0.45 * phase.sin()) as f32];
            }
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
        if let Some(gallery) = self.gallery {
            let (u, v) = self.organic_axes(x, z);
            return gallery.radial(u, v, self.radius);
        }
        if self.revision >= 5 {
            let (u, v) = self.organic_axes(x, z);
            let u = u / self.radius;
            let v = v / self.radius;
            if self.bedrock_chamber() {
                return self.bedrock_contour(u, v);
            }
            let phase = f64::from(self.seed % 628) * 0.01;
            let aspect = 0.66 + f64::from((self.seed >> 8) % 39) * 0.01;
            let warp_u = u + (v * 4.2 + phase).sin() * 0.055;
            let warp_v = v + (u * 5.1 - phase).sin() * 0.045;
            let angle = warp_v.atan2(warp_u);
            let contour =
                0.92 + (angle * 3.0 + phase).sin() * 0.04 + (angle * 5.0 - phase).cos() * 0.025;
            let main = (warp_u.powi(2) + (warp_v / aspect).powi(2)).sqrt() / contour;
            let lobe = (((warp_u - 0.22).powi(2) / 0.68_f64.powi(2))
                + ((warp_v + 0.14) / (aspect * 0.76)).powi(2))
            .sqrt();
            let organic = if self.seed & 2 == 0 {
                main.min(lobe)
            } else {
                main
            };
            // Descriptors still fit their original region; adjacent region
            // sampling cannot clip a new lobe at a 192 m boundary.
            return organic.max((u * u + v * v).sqrt() / 0.985);
        }
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

    fn axis_angle(self) -> f64 {
        self.gallery
            .map_or(f64::from(self.seed % 628) * 0.01, |g| g.angle)
    }

    fn organic_axes(self, x: f64, z: f64) -> (f64, f64) {
        let phase = self.axis_angle();
        let dx = x - self.center[0];
        let dz = z - self.center[2];
        (
            dx * phase.cos() + dz * phase.sin(),
            dz * phase.cos() - dx * phase.sin(),
        )
    }

    fn channel_mask(self, u: f64, v: f64) -> f64 {
        let phase = f64::from(self.seed % 628) * 0.01;
        let center = (u * 0.045 + phase).sin() * self.radius * 0.12
            + (u * 0.12 - phase).sin() * self.radius * 0.035;
        let width = 1.6
            + f64::from((self.seed >> 12) % 5) * 0.3
            + (-((u / self.radius + 0.25) / 0.18).powi(2)).exp() * 1.8
            + (-((u / self.radius - 0.32) / 0.16).powi(2)).exp() * 1.2;
        1.0 - smoothstep(width, width + 1.5, (v - center).abs())
    }

    fn cascade_edges(self) -> [f64; 2] {
        if self.revision >= 5 {
            [
                -self.radius * (0.18 + f64::from((self.seed >> 3) % 14) * 0.01),
                self.radius * (0.16 + f64::from((self.seed >> 11) % 16) * 0.01),
            ]
        } else {
            [-self.radius * 0.24, self.radius * 0.22]
        }
    }

    fn cascade_step(self) -> f64 {
        if self.living_gallery() && self.kind == CaveEcology::Lush {
            // Two drops share the chamber. Leave headroom above the upper bank.
            return (5.0 + f64::from(self.seed % 4))
                .min((self.height * 0.42).max(3.0))
                .floor();
        }
        if self.revision >= 5 && self.kind != CaveEcology::Waterfall {
            1.5 + f64::from((self.seed >> 6) % 4) * 0.5
        } else {
            6.0 + f64::from(self.seed % 5)
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
        let (u, v) = if self.revision >= 5 {
            self.organic_axes(x, z)
        } else {
            (x - self.center[0], z - self.center[2])
        };
        if self.bedrock_chamber() {
            return self.bedrock_section(x, z, u, v, r);
        }
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
        let basin = if self.revision >= 5 {
            self.channel_mask(u, v)
        } else if self.revision >= 2 && matches!(self.kind, CaveEcology::Salt | CaveEcology::Stone)
        {
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
            let shelf =
                if self.revision >= 9 && self.kind == CaveEcology::Salt && !self.dynamic_water {
                    if let Some(gallery) = self.gallery {
                        gallery.bed(self, u, v, rim, rough)
                    } else {
                        salt_gallery::terraces(base, u, v, self.radius, phase, rim, rough)
                    }
                } else if self.revision >= 11 && self.kind == CaveEcology::Dripstone {
                    let shelf = self
                        .gallery
                        .map_or(floor, |g| g.bed(self, u, v, rim, rough));
                    shelf.max(base - 3.1 + 0.45 * rough)
                } else if self.revision >= 5 && self.dynamic_water {
                    self.cascade_floor(x, z)
                } else {
                    match self.kind {
                        CaveEcology::Salt => {
                            let wave =
                                (u * 0.042 + phase).sin() * 3.0 + (v * 0.051 - phase).cos() * 2.0;
                            let terraces = if self.revision >= 5 {
                                ((u * 0.065 + v * 0.037 + phase).sin() * 1.6
                                    + (v * 0.055 - phase).cos() * 0.8
                                    + r * 1.1)
                                    .floor()
                                    * 1.35
                            } else {
                                ((r * 5.0 + wave * 0.12).max(0.0)).floor() * 1.8
                            };
                            base + rim - basin * 2.5 + terraces + rough * 0.3
                        }
                        CaveEcology::Lush => {
                            let islands = (u * 0.082 + phase).sin() * (v * 0.067).cos();
                            base + rim - basin * 2.0 + (islands * 3.0).floor() * 1.4 + rough * 0.4
                        }
                        CaveEcology::Waterfall => self.cascade_floor(x, z),
                        _ => floor,
                    }
                };
            return [shelf.max(-118.5), ceiling];
        }
        [floor.max(-118.5), ceiling]
    }

    fn cascade_stage(self, x: f64, z: f64) -> (f64, f64, f64) {
        let (u, v) = if self.revision >= 5 {
            self.organic_axes(x, z)
        } else {
            (x - self.center[0], z - self.center[2])
        };
        let bend = (v * 0.085 + f64::from(self.seed % 11)).sin() * 2.0;
        let along = u + bend;
        let step = self.cascade_step();
        let edges = self.cascade_edges();
        let stage = if along > edges[1] {
            2.0
        } else if along > edges[0] {
            1.0
        } else {
            0.0
        };
        let level = (self.center[1] - self.height * 0.72).floor() + stage * step;
        let cross = v
            - (u * 0.045 + f64::from(self.seed % 7)).sin() * self.radius * 0.12
            - if self.revision >= 5 {
                (u * 0.12 - f64::from(self.seed % 628) * 0.01).sin() * self.radius * 0.035
            } else {
                0.0
            };
        (along, level, cross)
    }

    fn cascade_floor(self, x: f64, z: f64) -> f64 {
        let (along, level, cross) = self.cascade_stage(x, z);
        let r = self.radial_at(x, z);
        let width = if self.living_gallery() {
            // Basins flare upstream of each lip. Both bank and water sample this
            // same floor, including the branching water around the garden island.
            let reach = along / self.radius;
            let pool = (1.0 - smoothstep(0.04, 0.48, (reach + 0.24).abs())) * self.radius * 0.12;
            2.7 + self.radius * 0.035 + pool
        } else if self.revision >= 5 {
            2.1 + f64::from((self.seed >> 14) % 4) * 0.35
                + (-((along / self.radius + 0.22) / 0.20).powi(2)).exp() * 1.7
        } else {
            2.5
        };
        let bank = smoothstep(
            width,
            if self.revision >= 5 {
                width + 4.5
            } else {
                self.radius * 0.35
            },
            cross.abs(),
        ) * 7.0;
        let base = (self.center[1] - self.height * 0.72).floor();
        let shoulder = base
            + self.cascade_step()
                * self
                    .cascade_edges()
                    .into_iter()
                    .map(|edge| smoothstep(edge - 5.0, edge + 5.0, along))
                    .sum::<f64>();
        let side = smoothstep(
            if self.revision >= 5 { width } else { 3.0 },
            if self.revision >= 5 {
                width + 4.0
            } else {
                self.radius * 0.30
            },
            cross.abs(),
        );
        let terrace = if self.living_gallery() {
            // Low stone steps beside the water become planted benches or rime
            // shelves. An asymmetrical island splits the lower botanical pool.
            let edge = cross.abs() - width;
            let bench = smoothstep(0.8, 1.6, edge) * 1.1 + smoothstep(4.1, 5.3, edge) * 1.4;
            let island = if self.kind == CaveEcology::Lush {
                let oval = ((along / self.radius + 0.29) / 0.17).hypot((cross + 0.7) / 3.1);
                (1.0 - smoothstep(0.42, 1.0, oval)) * 4.4
            } else {
                0.0
            };
            bench + island
        } else {
            0.0
        };
        level + (shoulder - level) * side - 2.5
            + bank
            + terrace
            + smoothstep(0.65, 1.02, r) * self.height * 1.55
            + (x * 0.21 + z * 0.17).sin() * 0.22
    }

    fn cascade_water(self, x: f64, z: f64) -> Option<WaterColumn> {
        let (along, level, _) = self.cascade_stage(x, z);
        // Dry receiving strips leave room for real falling cells between pools.
        let falling = self.cascade_edges().iter().any(|edge| {
            along < *edge
                && along
                    > *edge
                        - if self.revision >= 5 && self.kind != CaveEcology::Waterfall {
                            1.0
                        } else {
                            3.0
                        }
        });
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
        if self.revision >= 5 {
            let phase = self.axis_angle();
            for [dx, dz] in [[-1, 0], [0, -1], [1, 0], [0, 1]] {
                if -(dx as f64) * phase.cos() - (dz as f64) * phase.sin() < 0.25 {
                    continue;
                }
                if self.cascade_water(x + dx as f64, z + dz as f64).is_none()
                    && self.geometry(x + dx as f64, z + dz as f64)[0] < water.level - 1.5
                    && self.radial_at(x + dx as f64, z + dz as f64) < 0.85
                {
                    return Some([
                        x.floor() as i64 + dx,
                        water.level.ceil() as i64 - 1,
                        z.floor() as i64 + dz,
                    ]);
                }
            }
            return None;
        }
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
        if self.suppressed
            && self.radial < 1.0
            && self.ceiling - self.floor > 3.0
            && surface - y >= 5.0
            && (self.floor - 3.0..=self.ceiling + 3.0).contains(&y)
        {
            // Removing a room must not reopen the connector cells formerly
            // sealed by its floor/ceiling. Keep those guards and close its volume.
            return old.min((y - self.floor).min(self.ceiling - y)).min(-0.5);
        }
        if self.gallery.is_some()
            && self.salt_gallery.sky_coupling > 0.
            && y > self.floor
            && y <= surface + 2.
            && (self.open_to_sky() || self.salt_gallery.window_rim())
        {
            let shaft =
                (y - self.floor).min(self.salt_gallery.flared_shaft(y, self.ceiling, surface));
            let vault = (self.ceiling.min(surface - 5.) - y).min(y - self.floor);
            return self.salt_gallery.density(old.max(shaft).max(vault), y);
        }
        if self.open_to_sky() && y > self.floor && y <= surface + 2.0 {
            if self.salt_gallery.daylight() {
                return self
                    .salt_gallery
                    .density(old.max(self.salt_gallery.shaft_density(self.floor, y)), y);
            }
            return old.max((y - self.floor).min((0.14 - self.radial) * self.radius));
        }
        if !self.active() || surface - y < 5.0 {
            return old;
        }
        if (self.floor - 3.0..=self.ceiling + 3.0).contains(&y) {
            // Where a passage meets a chamber's outer shoulder, blend the two
            // signed fields. Keep the bed below standing water sealed.
            if self.bedrock_chamber() && self.radial > 0.62 {
                return self.bedrock_junction(y, old, (y - self.floor).min(self.ceiling - y));
            }
            let chamber = (y - self.floor).min(self.ceiling - y);
            self.living
                .density(self.salt_gallery.density(chamber, y), y)
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
                if self.revision >= 10 {
                    // A pair of fluted curtains meets only in short cavities.
                    // Off-axis daughter flutes terminate before the main lobe.
                    let edge = distance * 2.4;
                    let down = ceiling - 1 - iy;
                    let up = iy - floor - 1;
                    let pendant = f64::from(7 + ((ticket >> 6) % 13)) - edge;
                    let mound = f64::from(4 + ((ticket >> 12) % 7)) - edge;
                    if distance < 1.65
                        && ((down >= 0 && (down as f64) < pendant)
                            || (up >= 0 && (up as f64) < mound))
                    {
                        return Some(MaterialId::DripstoneTip);
                    }
                } else {
                    let t = ((y - self.floor) / (self.ceiling - self.floor)).clamp(0.0, 1.0);
                    let radius = 0.95 + (t - 0.5).abs() * 2.3 + (y * 0.7).sin() * 0.10;
                    if distance < radius {
                        return Some(MaterialId::DripstoneFormation);
                    }
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
            if ticket.is_multiple_of(7)
                && self.water.is_none()
                && self.radial_at(tree[0], tree[1]) < 0.77
            {
                let tree_floor = (self.geometry(tree[0], tree[1])[0] - 0.5).floor() + 1.0;
                let tx = x - tree[0];
                let tz = z - tree[1];
                let ty = y - tree_floor;
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

    /// Samples the chamber geometry without looking up water in connecting tunnels.
    pub fn cave_room_at(&self, x: f64, z: f64) -> CaveStructure {
        if self.config().cave_structure_revision == 0
            || self.config().geology_revision < 4
            || !self.config().caves
        {
            return CaveStructure::default();
        }
        let cell = [(x / 192.0).floor() as i64, (z / 192.0).floor() as i64];
        let (region, cell, gallery) = self.gallery_region(cell);
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
        let salt_daylight = self.config().cave_structure_revision >= 9
            && (kind == CaveEcology::Salt
                || self.config().cave_structure_revision >= 11 && kind == CaveEcology::Dripstone)
            && seed.is_multiple_of(3)
            && surface > self.config().sea_level + 8.0;
        if salt_daylight {
            center[1] = surface - region.height * 0.84 - 6.0;
        }
        let mut sample = CaveStructure {
            kind,
            center,
            gallery,
            radius: region.radius,
            height: region.height,
            seed,
            revision: self.config().cave_structure_revision,
            salt_allowed,
            suppressed: region.presence <= 0.0,
            dynamic_water: self.config().cave_structure_revision >= 4
                && (kind == CaveEcology::Waterfall
                    || self.config().cave_structure_revision >= 12 && kind == CaveEcology::Lush
                    || self.config().cave_structure_revision >= 5
                        && matches!(
                            kind,
                            CaveEcology::Stone | CaveEcology::Salt | CaveEcology::Lush
                        )
                        && seed.is_multiple_of(4)
                        && (kind != CaveEcology::Salt || gallery.is_none())),
            ..Default::default()
        };
        sample.radial = sample.radial_at(x, z);
        [sample.floor, sample.ceiling] = sample.geometry(x, z);
        if sample.active()
            && (sample.revision >= 9 && kind == CaveEcology::Salt
                || sample.revision >= 11 && kind == CaveEcology::Dripstone)
        {
            sample.salt_gallery = SaltGallery::column(sample, x, z, salt_daylight);
        }
        if sample.suppressed {
            sample.dynamic_water = false;
            return sample;
        }
        sample.dampness = match kind {
            CaveEcology::Salt => 0.55,
            CaveEcology::Waterfall => 0.65,
            CaveEcology::Dripstone if sample.revision >= 11 => 0.42,
            CaveEcology::Lush => 0.35,
            _ => 0.0,
        };
        if sample.dynamic_water {
            sample.water = self
                .config()
                .water
                .then(|| sample.cascade_water(x, z))
                .flatten();
            sample.living = living::LivingColumn::prepare(sample, x, z);
            return sample;
        }
        if self.config().water
            && sample.active()
            && (matches!(
                kind,
                CaveEcology::Salt | CaveEcology::Waterfall | CaveEcology::Lush | CaveEcology::Stone
            ) || sample.revision >= 11 && kind == CaveEcology::Dripstone)
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
        sample.living = living::LivingColumn::prepare(sample, x, z);
        sample
    }
}
