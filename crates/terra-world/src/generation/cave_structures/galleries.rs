//! An elongated gallery may occupy both halves of a region pair. The two halves
//! choose the same owner, axis and extent without inspecting generated chunks.
use super::{CaveStructure, TerraGenerator, layer_seed, smoothstep};
use crate::generation::{CaveBiome, CaveKind, caves::CaveRegion};

#[derive(Clone, Copy, Debug, PartialEq)]
pub(in crate::generation) struct Gallery {
    pub half_length: f64,
    pub angle: f64,
    pub phase: f64,
    layout: Layout,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Layout {
    Arcade,
    TwinVault,
    Amphitheatre,
    Rift,
}

impl Gallery {
    pub fn radial(self, u: f64, v: f64, width: f64) -> f64 {
        let along = u / self.half_length;
        let cross = v - self.bend(u, width);
        // A series of vaulted halls widens into pools then narrows into a
        // passage. Adjacent halls share an open floor at the constriction.
        let vault =
            0.78 + 0.18 * (u * 0.061 + self.phase).cos() + 0.08 * (u * 0.103 - self.phase).sin();
        let corridor = |narrowing: f64| {
            (along.powi(4) + (narrowing * cross / (width * vault)).abs().powi(3)).cbrt()
        };
        // A parallel side vault opens twice into the waterside walk.
        let side = ((along - self.phase.cos() * 0.27) / 0.51)
            .hypot((cross / width - (0.67 + 0.18 * (u * 0.071 + self.phase).cos())) / 0.29);
        match self.layout {
            Layout::Arcade => corridor(1.).min(side),
            Layout::TwinVault => {
                // Two vaulted aisles open into one another at either end.
                let fork = (1. - (along / 0.76).powi(2)).max(0.);
                let aisle = (cross / width - 0.68 * fork).abs();
                corridor(1.26).min((along.powi(4) + (aisle / 0.38).powi(3)).cbrt())
            }
            Layout::Amphitheatre => {
                // Three unequal bowls are linked by a narrow waterside passage.
                let bowl = |offset: f64, length: f64, span: f64| {
                    ((along - offset) / length).hypot((cross / width) / span)
                };
                corridor(1.48)
                    .min(bowl(-0.50, 0.46, 0.77))
                    .min(bowl(0.05, 0.39, 1.01))
                    .min(bowl(0.53, 0.40, 0.62))
            }
            Layout::Rift => {
                let crack = cross / width + along * 0.24;
                (along.powi(4) + (crack / (vault * 0.68)).abs().powi(3))
                    .cbrt()
                    .min(((along - 0.21) / 0.27).hypot((crack - 0.28) / 0.57))
            }
        }
    }

    pub fn bend(self, u: f64, width: f64) -> f64 {
        width * (0.18 * (u * 0.030 + self.phase).sin() + 0.07 * (u * 0.062 - self.phase).sin())
    }

    /// Elliptical bedrock islands in the broad pools. The channel remains open
    /// along the other bank, with a dry crown for a salt pillar or crystal fan.
    pub fn island(self, u: f64, v: f64, width: f64) -> f64 {
        let spacing = (self.half_length * 0.89).clamp(44., 96.);
        let dx = (u + 0.76 * spacing).rem_euclid(spacing) - 0.5 * spacing;
        let across = v - self.bend(u, width) - width * 0.075 * (self.phase + u * 0.035).cos();
        (dx / (width * 0.18)).hypot(across / (width * 0.11))
            + 0.08 * (across * 0.79 + dx * 0.46).sin()
    }

    pub fn bed(self, room: CaveStructure, u: f64, v: f64, rim: f64, rough: f64) -> f64 {
        let base = room.center[1] - room.height * 0.72;
        let cross = v - self.bend(u, room.radius);
        let shore = cross.abs() - room.radius * (0.19 + 0.08 * (u * 0.052 + self.phase).cos());
        let shallow = smoothstep(-5., 1.5, shore);
        // Broad asymmetric benches and short ramps along strike. Only the
        // base of each riser is stepped, leaving a traversable bank beside it.
        let bench = (shore - 3.).max(0.) * (0.16 + 0.04 * (u * 0.057 + self.phase).sin());
        let ramp = (bench / 2.3).floor() * 1.65 + bench.rem_euclid(2.3) * 0.29;
        let margin = smoothstep(2., 12., shore);
        let side_ledge = smoothstep(0.28, 0.62, cross / room.radius)
            * (3. + 2. * (u * 0.034 - self.phase).cos());
        let island = self.island(u, v, room.radius);
        let crown = if island < 1.14 {
            1.7 - (island * 2.7).floor() * 1.45
        } else {
            -4.6
        };
        let bluff = (0.5 + 0.5 * (u * 0.095 + self.phase + cross * 0.051).sin()).powi(3);
        let buttress = (3. + 8. * bluff) * smoothstep(0.47, 0.83, (cross / room.radius).abs());
        // Resistant beds break into irregular high ledges instead of a bank
        // climbing one voxel every step all the way to the vaulted ceiling.
        let upper_bank = rim * (0.88 + 0.22 * bluff);
        let ledge = 5.2 + 2.7 * bluff;
        let step = (upper_bank / ledge).floor();
        let run = (upper_bank / ledge).fract();
        let cliff = (step + 0.18 * run + 0.82 * smoothstep(0.40, 0.59, run)) * ledge;
        let landform = match self.layout {
            Layout::Arcade => 0.,
            Layout::TwinVault => {
                5. * smoothstep(0.08, 0.32, cross / room.radius)
                    * (1. - smoothstep(0.32, 0.60, cross / room.radius))
            }
            Layout::Amphitheatre => (shore.max(0.) / 6.).floor().min(4.) * 1.4,
            Layout::Rift => -3. * (1. - smoothstep(0.06, 0.26, (cross / room.radius).abs())),
        };
        base + landform
            + crown.max(-4.6 + shallow * 6. + margin * (ramp + side_ledge))
            + cliff
            + buttress
            + rough * 0.24
    }

    pub fn span(self, u: f64) -> f64 {
        let period = (self.half_length * 0.68).clamp(34., 80.);
        let offset = period * 0.18 * self.phase.sin();
        ((u - offset + period * 0.5).rem_euclid(period)) - period * 0.5
    }
}

pub(in crate::generation) type GalleryRegion = (CaveRegion, [i64; 2], Option<Gallery>);

impl TerraGenerator {
    pub(in crate::generation) fn gallery_region(&self, cell: [i64; 2]) -> GalleryRegion {
        if self.config().cave_structure_revision < 10 {
            return (self.cave_region(cell), cell, None);
        }
        if let Some(result) = self
            .cave_portals
            .3
            .lock()
            .ok()
            .and_then(|cache| cache.get(&cell).copied())
        {
            return result;
        }
        let result = self.plan_gallery(cell);
        if let Ok(mut cache) = self.cave_portals.3.lock() {
            if cache.len() >= 2048 {
                cache.clear();
            }
            cache.insert(cell, result);
        }
        result
    }

    fn plan_gallery(&self, cell: [i64; 2]) -> GalleryRegion {
        let original = self.cave_region(cell);
        let group = cell.map(|c| c.div_euclid(2));
        let h = layer_seed(
            self.config().seed.0
                ^ (group[0] as u64).wrapping_mul(0x9e3779b97f4a7c15)
                ^ (group[1] as u64).wrapping_mul(0xbf58476d1ce4e5b9),
            447,
        );
        let axis = usize::from(h & 1 != 0);
        let mut other = cell;
        other[axis] ^= 1;
        let peer = self.cave_region(other);
        let karst = |r: CaveRegion| {
            self.config().cave_structure_revision >= 11
                && r.ecology == CaveBiome::Normal
                && r.kind == CaveKind::Dripstone
        };
        let priority = |r: CaveRegion| (r.salt, karst(r));
        let choose_long = original.presence > 0.
            && peer.presence > 0.
            && if self.config().cave_structure_revision >= 13 {
                h % 7 < 2
            } else {
                original.salt || peer.salt || karst(original) || karst(peer) || h % 7 < 2
            }
            && (original.ecology == peer.ecology
                || matches!(
                    (original.ecology, peer.ecology),
                    (
                        super::super::CaveBiome::Normal,
                        super::super::CaveBiome::Salt
                    ) | (
                        super::super::CaveBiome::Salt,
                        super::super::CaveBiome::Normal
                    )
                ));
        let (mut region, owner) = if choose_long
            && (priority(peer) > priority(original)
                || priority(peer) == priority(original) && other < cell)
        {
            (peer, other)
        } else {
            (original, cell)
        };
        if region.radius == 0. || !choose_long && !region.salt && !karst(region) {
            return (original, cell, None);
        }
        let phase = f64::from((h >> 8) % 628) * 0.01;
        let axis_angle = if axis == 0 {
            0.
        } else {
            std::f64::consts::FRAC_PI_2
        };
        let angle = if choose_long {
            axis_angle + phase.sin() * 0.085
        } else {
            phase
        };
        let half_length = if karst(region) && choose_long {
            143. + f64::from((h >> 13) % 33)
        } else if karst(region) {
            (region.radius * 1.65).clamp(67., 82.)
        } else if choose_long {
            105. + f64::from((h >> 13) % 63)
        } else {
            (region.radius * 1.35).clamp(46., 78.)
        };
        let mut horizontal = cell.map(|p| (p * 192 + 96) as f64);
        if choose_long {
            horizontal[axis] = (group[axis] * 384 + 192) as f64;
            region.surface = original.surface.min(peer.surface);
        }
        region.center[0] = horizontal[0];
        region.center[2] = horizontal[1];
        region.radius = (region.radius * 0.82 + 12.).clamp(27., 61.);
        region.height = (region.height * (1.10 + 0.21 * phase.cos().abs())).min(57.);
        if karst(region) {
            region.radius = (region.radius + 11.).clamp(45., 65.);
            region.height = (region.height + 5.).clamp(28., 55.);
        }
        (
            region,
            owner,
            Some(Gallery {
                half_length,
                angle,
                phase,
                layout: if self.config().cave_structure_revision >= 13
                    && (karst(region) || region.salt)
                {
                    [
                        Layout::Arcade,
                        Layout::TwinVault,
                        Layout::Amphitheatre,
                        Layout::Rift,
                    ][((h >> 21) % 4) as usize]
                } else {
                    Layout::Arcade
                },
            }),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use terra_core::WorldConfig;

    #[test]
    fn mineral_galleries_are_rarer_and_have_four_different_floor_plans() {
        let survey = |revision| {
            let terrain = TerraGenerator::new(WorldConfig {
                cave_structure_revision: revision,
                ..Default::default()
            })
            .unwrap();
            let mut counts = [0; 2];
            let mut layouts = [false; 4];
            for x in -20..20 {
                for z in -20..20 {
                    let (r, _, gallery) = terrain.gallery_region([x, z]);
                    if r.presence <= 0. {
                        continue;
                    }
                    if r.salt || r.ecology == CaveBiome::Normal && r.kind == CaveKind::Dripstone {
                        counts[usize::from(r.salt)] += 1;
                        if let Some(g) = gallery {
                            layouts[g.layout as usize] = true;
                        }
                    }
                }
            }
            (counts, layouts)
        };
        let (old, _) = survey(12);
        let (new, layouts) = survey(13);
        eprintln!("mineral gallery regions in 1600 cells, dripstone/salt: {old:?} -> {new:?}");
        assert!(
            new.into_iter()
                .zip(old)
                .all(|(current, old)| current > 0 && current * 3 < old * 2)
        );
        assert_eq!(layouts, [true; 4]);
    }
}
