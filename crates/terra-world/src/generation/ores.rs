//! Spatially bucketed deposits: section jobs visit deposits, not every ore at every voxel.
use super::{SurfaceColumn, TerraGenerator};
use terra_core::{OreKind, OreVeinConfig};

const CELL: i64 = 32;
fn mix(mut value: u64) -> u64 {
    value = (value ^ (value >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94d049bb133111eb);
    value ^ (value >> 31)
}
fn hash(seed: u64, point: [i64; 3]) -> u64 {
    point.into_iter().fold(seed, |value, axis| {
        mix(value ^ (axis as u64).wrapping_mul(0x9e3779b97f4a7c15))
    })
}
fn unit(value: u64) -> f64 {
    (value >> 11) as f64 / (1_u64 << 53) as f64
}
#[derive(Clone, Copy)]
struct Deposit {
    ore: OreKind,
    config: OreVeinConfig,
    center: [f64; 3],
    radius: [f64; 3],
    seed: u64,
    cave: bool,
    ceiling: f64,
    layered: bool,
}
impl Deposit {
    fn wall_band(self) -> f64 {
        if self.ore == OreKind::Salt {
            1.4 + (1.0 - self.config.cave_affinity) * 3.0
        } else {
            3.2
        }
    }
    fn inside(self, global: [i64; 3], column: SurfaceColumn) -> bool {
        let point = global.map(|value| value as f64 + 0.5);
        if global[1] as f64 > self.ceiling {
            return false;
        }
        let depth = column.height - point[1];
        if !(self.config.min_depth..=self.config.max_depth).contains(&depth) {
            return false;
        }
        let distance: f64 = (0..3)
            .map(|axis| ((point[axis] - self.center[axis]) / self.radius[axis]).powi(2))
            .sum();
        let boundary = if self.layered {
            0.96 + (point[0] * 0.37 + point[1] * 0.29 + self.seed as u32 as f64 * 0.001).sin()
                * (point[2] * 0.31 - point[1] * 0.19).cos()
                * 0.10
        } else {
            0.84 + unit(hash(self.seed, global)) * 0.32
        };
        if distance > boundary {
            return false;
        }
        true
    }
    fn contains(self, terrain: &TerraGenerator, global: [i64; 3], column: SurfaceColumn) -> bool {
        self.inside(global, column)
            && (!self.cave
                || terrain.config().caves && {
                    let p = global.map(|value| value as f64 + 0.5);
                    let density = if self.layered {
                        terrain.cave_density_with_column(p, column).0
                    } else {
                        terrain.tunnel_density_with_shape(p, &column.cave_shape)
                    };
                    density > -self.wall_band() && density <= 0.0
                })
    }
}
impl TerraGenerator {
    /// A sparse geological patch changes salt abundance, never cave geometry.
    pub fn salt_cave_region(&self, point: [i64; 3]) -> bool {
        if self.config().cave_structure_revision >= 4 {
            return self.cave_biome_at(point.map(|v| v as f64 + 0.5)).weights[1] > 0.35;
        }
        if self.config().geology_revision > 0 {
            return self
                .cave_shape(point[0] as f64 + 0.5, point[2] as f64 + 0.5)
                .salt;
        }
        hash(
            self.config().seed.0 ^ 0x53414c5443415645,
            point.map(|value| value.div_euclid(96)),
        )
        .is_multiple_of(7)
    }
    fn deposits(&self, start: [i64; 3], size: usize, mut visit: impl FnMut(Deposit)) {
        if !self.config().ores.enabled {
            return;
        }
        for ore in OreKind::ALL {
            // Native salt is a skin of the already evaluated chamber field. No
            // dense ten-metre prospect grid or second cave-noise pass is needed.
            let layered = self.config().cave_structure_revision >= 4;
            if ore == OreKind::Salt && self.config().geology_revision > 0 && !layered {
                continue;
            }
            let config = self.config().ores.vein(ore);
            if config.frequency == 0.0 {
                continue;
            }
            let margin = (config.radius * 1.5 + 1.0).ceil() as i64;
            let cell_size = if ore == OreKind::Salt && !layered {
                10
            } else {
                CELL
            };
            let lower = start.map(|value| (value - margin).div_euclid(cell_size));
            let upper = start.map(|value| (value + size as i64 + margin).div_euclid(cell_size));
            for z in lower[2]..=upper[2] {
                for y in lower[1]..=upper[1] {
                    for x in lower[0]..=upper[0] {
                        let cell = [x, y, z];
                        let seed = hash(
                            self.config().seed.0 ^ (u64::from(ore.material()) * 0x71e85a3b),
                            cell,
                        );
                        if unit(seed)
                            >= config.frequency
                                * if layered && ore == OreKind::Salt {
                                    0.18
                                } else {
                                    1.0
                                }
                        {
                            continue;
                        }
                        let center = std::array::from_fn(|axis| {
                            cell[axis] as f64 * cell_size as f64
                                + unit(mix(seed.wrapping_add(axis as u64 + 1))) * cell_size as f64
                        });
                        let cave = (ore == OreKind::Salt && !layered)
                            || unit(mix(seed ^ 0x43415645)) < config.cave_affinity;
                        if ore == OreKind::Salt
                            && !self.salt_cave_region(center.map(|v| v.floor() as i64))
                            && (layered || unit(mix(seed ^ 0x52415245)) > 0.035)
                        {
                            continue;
                        }
                        let size = if layered {
                            0.55 + unit(mix(seed ^ 0x53495a45)) * 0.55
                        } else {
                            0.75 + unit(mix(seed ^ 0x53495a45)) * 0.25
                        };
                        let mut radius = [
                            config.radius * 1.35 * size,
                            config.radius * 0.70 * size,
                            config.radius * size,
                        ];
                        if seed & 1 == 1 {
                            radius.swap(0, 2);
                        }
                        visit(Deposit {
                            layered,
                            ceiling: if self.config().bounded_world {
                                match ore {
                                    OreKind::Coal => 200.0,
                                    OreKind::Iron => 170.0,
                                    OreKind::Gold => 56.0,
                                    OreKind::Diamond => -5.0,
                                    OreKind::Ruby => -32.0,
                                    OreKind::Salt => f64::INFINITY,
                                }
                            } else {
                                f64::INFINITY
                            },
                            ore,
                            config,
                            center,
                            radius,
                            seed,
                            cave,
                        });
                    }
                }
            }
        }
    }
    pub(super) fn stamp_ores(
        &self,
        start: [i64; 3],
        size: usize,
        columns: &[SurfaceColumn],
        blocks: &mut [u16],
    ) {
        let host =
            |m| m == 3 || self.config().cave_structure_revision >= 4 && matches!(m, 13 | 57 | 58);
        if !self.config().ores.enabled || !blocks.iter().any(|&m| host(m)) {
            return;
        }
        let mut cave_cache: Option<Vec<f64>> = None;
        self.deposits(start, size, |deposit| {
            let lower: [usize; 3] = std::array::from_fn(|axis| {
                ((deposit.center[axis] - deposit.radius[axis] * 1.1).floor() as i64 - start[axis])
                    .clamp(0, size as i64) as usize
            });
            let upper: [usize; 3] = std::array::from_fn(|axis| {
                ((deposit.center[axis] + deposit.radius[axis] * 1.1).ceil() as i64 - start[axis])
                    .clamp(0, size as i64) as usize
            });
            for z in lower[2]..upper[2] {
                for y in lower[1]..upper[1] {
                    for x in lower[0]..upper[0] {
                        let index = x + size * (y + size * z);
                        if !host(blocks[index]) {
                            continue;
                        }
                        let global = [
                            start[0] + x as i64,
                            start[1] + y as i64,
                            start[2] + z as i64,
                        ];
                        let column = columns[x + size * z];
                        if !deposit.inside(global, column) {
                            continue;
                        }
                        let cave = if deposit.cave {
                            if !self.config().caves {
                                continue;
                            }
                            let cached = &mut cave_cache
                                .get_or_insert_with(|| vec![f64::NAN; blocks.len()])[index];
                            if cached.is_nan() {
                                let p = global.map(|value| value as f64 + 0.5);
                                *cached = if deposit.layered {
                                    self.cave_density_with_column(p, column).0
                                } else {
                                    self.tunnel_density_with_shape(p, &column.cave_shape)
                                };
                            }
                            *cached > -deposit.wall_band() && *cached <= 0.0
                        } else {
                            true
                        };
                        if cave {
                            blocks[index] = deposit.ore.material();
                        }
                    }
                }
            }
        });
    }
    pub(super) fn ore_at(&self, global: [i64; 3], column: SurfaceColumn) -> Option<OreKind> {
        let mut result = None;
        self.deposits(global, 1, |deposit| {
            if result.is_none() && deposit.contains(self, global, column) {
                result = Some(deposit.ore);
            }
        });
        result
    }
}

#[cfg(test)]
mod tests;
