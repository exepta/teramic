//! Material and decoration rules shared by all surfaces of the existing cave field.
use super::{CaveBiomeSample, SurfaceColumn, TerraGenerator, seed::layer_seed};
use noise::NoiseFn;
use terra_voxel::{MaterialId, SectionPos};

impl TerraGenerator {
    pub fn cave_connector_at(&self, p: [f64; 3]) -> bool {
        let c = self.surface_column(p[0], p[2]);
        c.height - p[1] > 5.0 && self.cave_density_with_column(p, c).0 > 0.0 && !room_at(c, p[1])
    }

    pub fn cave_ecology_sample(&self, p: [f64; 3], c: SurfaceColumn) -> CaveBiomeSample {
        CaveBiomeSample::from_weights(
            self.cave_ecology_weights(p, c),
            c.cave_biome.sample.region_id,
            c.cave_biome.sample.boundary_distance,
        )
    }

    #[inline]
    fn cave_ecology_weights(&self, p: [f64; 3], c: SurfaceColumn) -> [f32; 4] {
        let cfg = &self.config().cave_biomes.connectors;
        if !room_at(c, p[1]) && !cfg.inherit_biomes {
            return [1.0, 0.0, 0.0, 0.0];
        }
        let w = c.cave_biome.weights_at(p[1]);
        if !room_at(c, p[1]) && !cfg.blend_biomes {
            let mut weights = [0.0; 4];
            let mut primary = 0;
            for i in 1..4 {
                if w[i] > w[primary] {
                    primary = i;
                }
            }
            weights[primary] = 1.0;
            weights
        } else {
            w
        }
    }

    pub(super) fn cave_ecology_material(
        &self,
        p: [f64; 3],
        c: SurfaceColumn,
        density: f64,
    ) -> Option<MaterialId> {
        let gallery_shell = c.cave.salt_gallery_shell(p[1]);
        let karst_shell = c.cave.dripstone_shell(p[1]);
        let living_shell = c.cave.living_shell(p[1]);
        if !self.config().cave_biomes.enabled
            || p[1] < -119.0
            || (density < -2.2 && !gallery_shell && !karst_shell && !living_shell)
        {
            return None;
        }
        let [x, y, z] = p;
        let w = self.cave_ecology_weights(p, c);
        if density <= 0.0 {
            // Spatially coherent outcrops rather than per-block biome dithering.
            let patch = self
                .valleys
                .get([x * 0.072 + 213.0, y * 0.095, z * 0.072 - 89.0]);
            if living_shell {
                let exposed = density >= -1.0;
                let floor = exposed && self.cave_density_with_column([x, y + 1.0, z], c).0 > 0.0;
                let ceiling =
                    exposed && !floor && self.cave_density_with_column([x, y - 1.0, z], c).0 > 0.0;
                return Some(c.cave.living_rock(y, patch, floor, ceiling));
            }
            if karst_shell {
                let shore = c.cave.center[1] - c.cave.height * 0.72;
                return Some(if y < shore + 1.0 && patch < -0.18 {
                    MaterialId::CaveMud
                } else if y < shore + 1.5 || patch < -0.40 {
                    MaterialId::WetCaveStone
                } else if patch < -0.16 && y < shore + 11.0 {
                    MaterialId::MossyCaveStone
                } else if patch > 0.32 && (y * 0.28 + x * 0.05 + z * 0.03).sin() > 0.08 {
                    MaterialId::CaveCalcite
                } else if patch > 0.11 || (y - c.cave.ceiling).abs() < 1.3 && patch > -0.12 {
                    MaterialId::Dripstone
                } else {
                    MaterialId::WetCaveStone
                });
            }
            if self.config().cave_structure_revision >= 4 {
                let surface = self.config().cave_biomes.salt_surfaces;
                let covered = 1.0 - surface[0] / surface.iter().sum::<f64>();
                if gallery_shell {
                    return Some(c.cave.salt_gallery_rock(y, patch, covered));
                }
                if covered > 0.0 && w[1] > 0.01 && patch > 0.65 - covered * f64::from(w[1]) * 1.4 {
                    return Some(if (y * 0.16 + x * 0.012 + z * 0.02).sin() > 0.55 {
                        MaterialId::PinkSaltStone
                    } else {
                        MaterialId::SaltStone
                    });
                }
            } else if w[1] > 0.01 && patch > 0.67 - f64::from(w[1]) * 1.20 {
                return Some(MaterialId::SaltOre);
            }
            if w[2] + w[3] > 0.01 && patch < -0.68 + f64::from(w[2] * 0.80 + w[3] * 0.45) {
                if self.config().cave_structure_revision >= 4
                    && density > -1.0
                    && w[2] > 0.5
                    && self.cave_density_with_column([x, y + 1.0, z], c).0 > 0.0
                    && !c
                        .water_columns()
                        .intervals
                        .iter()
                        .flatten()
                        .any(|water| y + 1.0 >= water.bed && y + 1.0 < water.level)
                {
                    return Some(MaterialId::Grass);
                }
                return Some(MaterialId::MossyStone);
            }
            if c.cave.kind == super::CaveEcology::Dripstone && room_at(c, y + 1.0) && patch > 0.0 {
                return Some(MaterialId::Dripstone);
            }
            if patch > 0.37 && w[0] + w[2] > 0.3 {
                return Some(MaterialId::Dirt);
            }
            return None;
        }
        if let Some(material) = c.cave.room_feature(p, density, w[2]) {
            let coverage = self.config().cave_biomes.salt_surfaces;
            let cluster = coverage[2] + coverage[3];
            if self.config().cave_structure_revision >= 9 && c.cave.kind == super::CaveEcology::Salt
            {
                return (cluster > 0.0).then_some(material);
            }
            if self.config().cave_structure_revision < 4
                || material != MaterialId::SaltCrystal
                || (w[1] > 0.5
                    && cluster > 0.0
                    && self.valleys.get([x * 0.025, 7.0, z * 0.025])
                        > 0.30 - cluster / coverage.iter().sum::<f64>() * 3.0)
            {
                return Some(material);
            }
        }
        if living_shell
            || gallery_shell
            || self.config().cave_structure_revision >= 11
                && c.cave.kind == super::CaveEcology::Dripstone
                && room_at(c, y)
        {
            return None;
        }
        if !self.config().cave_biomes.connectors.biome_decorations && !room_at(c, y) {
            return None;
        }
        if c.water_columns()
            .intervals
            .iter()
            .flatten()
            .any(|water| y < water.level && y >= water.bed)
        {
            return None;
        }
        let h = ticket(self, p.map(|v| v.floor() as i64));
        let roll = ((h >> 10) & 65535) as f32 / 65535.0;
        let field = |offset: f64| self.cave_density_with_column([x, y + offset, z], c).0;
        let lush = self.config().cave_structure_revision >= 4;
        let patch = if lush {
            (self.valleys.get([x * 0.06, y * 0.025, z * 0.06]) + 0.5).clamp(0.0, 1.0) as f32
                * (self.config().cave_biomes.lush_density / 0.72) as f32
        } else {
            1.0
        };
        if h.is_multiple_of(if lush { 11 } else { 31 })
            && roll < (w[2] * 0.65 + w[3] * 0.15 + w[0] * 0.025) * patch
        {
            let length = 1
                + (h >> 24)
                    % if room_at(c, y) && lush {
                        8
                    } else if room_at(c, y) {
                        4
                    } else {
                        2
                    };
            for d in 1..=length {
                if field(f64::from(d)) <= 0.0 {
                    if field(-3.0) > 0.0 {
                        return Some(if h & 4 == 0 {
                            MaterialId::CaveRoot
                        } else {
                            MaterialId::CaveVine
                        });
                    }
                    break;
                }
            }
        }
        if h.is_multiple_of(89)
            && roll < w[0] * 0.13 + w[2] * 0.7 + w[3] * 0.2
            && field(-1.0) <= 0.0
            && field(2.0) > 0.0
        {
            return Some(MaterialId::CaveMushroom);
        }
        if !h.is_multiple_of(43) {
            return None;
        }
        if lush && room_at(c, y) && w[1] > 0.5 {
            // Salt rooms use coherent multi-column clusters, not uniform shafts.
            return None;
        }
        if !room_at(c, y) && density > 0.75 {
            return None;
        }
        let mineral = if roll < w[1] {
            MaterialId::SaltCrystal
        } else if roll < w[1] + w[3] * 0.45 {
            MaterialId::IceCrystal
        } else if room_at(c, y) && c.cave.kind == super::CaveEcology::Dripstone && roll < 0.85 {
            MaterialId::DripstoneTip
        } else {
            return None;
        };
        let length = if room_at(c, y) {
            2 + (h >> 24) % 7
        } else {
            1 + (h >> 24) % 2
        };
        let direction = if h & 8 == 0 { 1.0 } else { -1.0 };
        // Leave at least three empty cells beyond a decoration. An actual
        // original rock anchor is required; no decoration is its own support.
        for d in 1..=length {
            if field(direction * f64::from(d)) <= 0.0 {
                if field(-direction * f64::from(length + 3 - d)) > 0.0 {
                    return Some(mineral);
                }
                break;
            }
        }
        None
    }

    pub(super) fn cave_prop(
        &self,
        support: [i64; 3],
        c: SurfaceColumn,
        section: SectionPos,
    ) -> Option<crate::props::WorldProp> {
        if self.config().cave_structure_revision < 3 {
            return c.cave.prop(support, section);
        }
        let h = ticket(self, support);
        let layered = self.config().cave_structure_revision >= 4;
        if h % 10000 >= if layered { 3600 } else { 1200 } {
            return None;
        }
        let p = [
            support[0] as f64 + 0.5,
            support[1] as f64 + 1.5,
            support[2] as f64 + 0.5,
        ];
        if c.height - p[1] < 7.0 || !self.config().cave_biomes.enabled {
            return None;
        }
        if !room_at(c, p[1]) && !self.config().cave_biomes.connectors.biome_decorations {
            return None;
        }
        let w = self.cave_ecology_weights(p, c);
        let vegetation = if layered {
            let patch =
                (self.valleys.get([p[0] * 0.055, 12.0, p[2] * 0.055]) + 0.5).clamp(0.0, 1.0) as f32;
            let water = c
                .cave
                .water
                .map_or(0.0, |v| (1.0 - ((p[1] - v.level) / 6.0).abs()).max(0.0))
                as f32;
            let light = if c.cave.open_to_sky() { 1.0 } else { 0.65 };
            (0.08 + patch * 0.30 + water * 0.08)
                * light
                * self.config().cave_biomes.lush_density as f32
                / 0.72
        } else {
            0.12
        };
        if (h % 10000) as f32 / 10000.0
            >= w[2] * vegetation + w[3] * if layered { 0.04 } else { 0.028 } + w[0] * 0.002
        {
            return None;
        }
        if c.water_columns()
            .intervals
            .iter()
            .flatten()
            .any(|water| (!layered || p[1] >= water.bed) && p[1] < water.level)
        {
            return None;
        }
        if self.cave_density_with_column([p[0], p[1] - 1.0, p[2]], c).0 > 0.0
            || self.cave_density_with_column(p, c).0 <= 0.0
            || self.cave_density_with_column([p[0], p[1] + 1.0, p[2]], c).0 <= 0.0
        {
            return None;
        }
        use crate::props::{PropId, PropKind, WorldProp};
        Some(WorldProp {
            id: PropId(
                self.config().seed.0
                    ^ 0x6361766570726f70
                    ^ (support[0] as u64).wrapping_mul(0x9e3779b97f4a7c15)
                    ^ (support[1] as u64).wrapping_mul(0x94d049bb133111eb)
                    ^ (support[2] as u64).wrapping_mul(0xbf58476d1ce4e5b9),
            ),
            kind: match h % if layered { 9 } else { 5 } {
                0 => PropKind::Mushroom,
                1 | 2 => PropKind::Fern,
                5 => PropKind::Bush,
                6 if self.surface_biome(&c) == super::biomes::BiomeKind::PetrifiedForest => {
                    PropKind::Mushroom
                }
                6 => PropKind::Flower,
                7 => PropKind::SmallPlant,
                _ => PropKind::Grass,
            },
            position: [p[0], support[1] as f64 + 1.0, p[2]],
            rotation: (h % 628) as f32 * 0.01,
            scale: 0.40 + ((h >> 16) % 25) as f32 * 0.01,
            variant: (h % 8) as u8,
            section,
            removed: false,
        })
    }

    pub(super) fn cave_prop_column_candidate(&self, x: i64, z: i64) -> bool {
        ticket(self, [x, 0, z]) % 10000
            < if self.config().cave_structure_revision >= 4 {
                3600
            } else {
                1200
            }
    }
}

fn ticket(terrain: &TerraGenerator, p: [i64; 3]) -> u32 {
    layer_seed(
        terrain.config().seed.0
            ^ (p[0] as u64).wrapping_mul(0x9e3779b97f4a7c15)
            ^ (p[2] as u64).wrapping_mul(0xbf58476d1ce4e5b9),
        304,
    )
}
fn room_at(c: SurfaceColumn, y: f64) -> bool {
    c.cave.active() && (c.cave.floor - 2.5..c.cave.ceiling + 2.5).contains(&y)
}
