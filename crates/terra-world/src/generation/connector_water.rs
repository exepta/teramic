//! Conservative aquifer pockets in existing connectors. Never creates tunnels.
use super::{TerraGenerator, seed::layer_seed};
use crate::water::WaterColumn;
use std::{
    collections::{HashMap, VecDeque},
    sync::{Arc, Mutex, OnceLock},
};

const SIDE: usize = 32;
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct ConnectorWater {
    pub water: WaterColumn,
    pub flow: [f32; 2],
}
type Pocket = [Option<ConnectorWater>; SIDE * SIDE];
#[derive(Default)]
pub(super) struct ConnectorWaterCache(Mutex<HashMap<[i64; 2], Arc<OnceLock<Pocket>>>>);

impl TerraGenerator {
    pub(super) fn connector_water_at(&self, x: f64, z: f64) -> Option<ConnectorWater> {
        if !self.config().water
            || !self.config().cave_biomes.enabled
            || !self.config().cave_biomes.connectors.inherit_biomes
        {
            return None;
        }
        let p = [x.floor() as i64, z.floor() as i64];
        let cell = p.map(|v| v.div_euclid(SIDE as i64));
        let cached = self
            .connector_water
            .0
            .lock()
            .ok()
            .map(|mut cache| {
                if cache.len() >= 256 && !cache.contains_key(&cell) {
                    cache.clear();
                }
                cache.entry(cell).or_default().clone()
            })
            .unwrap_or_default();
        let pocket = cached.get_or_init(|| self.connector_pocket(cell));
        pocket[p[0].rem_euclid(32) as usize + SIDE * p[1].rem_euclid(32) as usize]
    }

    fn connector_pocket(&self, cell: [i64; 2]) -> Pocket {
        let origin = cell.map(|v| v * SIDE as i64);
        let h = layer_seed(
            self.config().seed.0
                ^ (cell[0] as u64).wrapping_mul(7919)
                ^ (cell[1] as u64).wrapping_mul(104729),
            306,
        );
        let level = -24.0 - f64::from(h % 4) * 24.0;
        let center = self.cave_biome_at([origin[0] as f64 + 16.0, level, origin[1] as f64 + 16.0]);
        let mut result = [None; SIDE * SIDE];
        if center.weights[3] + center.weights[2] * 0.35 < 0.3 {
            return result;
        }
        let mut air = [false; SIDE * SIDE];
        let mut beds = [None; SIDE * SIDE];
        for i in 0..SIDE * SIDE {
            let x = (origin[0] + (i % SIDE) as i64) as f64 + 0.5;
            let z = (origin[1] + (i / SIDE) as i64) as f64 + 0.5;
            let biome = self.cave_biome_column(x, z);
            let room = self.cave_room_at(x, z);
            let shape = self.cave_shape(x, z);
            let field = |y: f64| {
                let mut shape = shape;
                let w = biome.weights_at(y);
                shape.tunnel_width *= f64::from(1.0 - w[1] * 0.10 + w[2] * 0.15 + w[3] * 0.08);
                room.carve(
                    y,
                    self.tunnel_density_with_shape([x, y, z], &shape)
                        .min(biome.surface - y - 5.0),
                    biome.surface,
                )
            };
            air[i] = field(level - 0.5) > 0.0;
            if !air[i] || room.active() || biome.surface - level < 8.0 {
                continue;
            }
            for d in 1..=6 {
                if field(level - f64::from(d) - 0.5) <= 0.0 {
                    beds[i] = Some(level - f64::from(d));
                    break;
                }
            }
        }
        let mut seen = [false; SIDE * SIDE];
        for start in 0..SIDE * SIDE {
            if !air[start] || seen[start] {
                continue;
            }
            let mut cells = Vec::new();
            let mut queue = VecDeque::from([start]);
            seen[start] = true;
            let mut sealed = true;
            while let Some(i) = queue.pop_front() {
                cells.push(i);
                let (x, z) = (i % SIDE, i / SIDE);
                // Reject open spill routes, unsupported bottoms, room overlaps
                // and components crossing the bounded search window.
                sealed &= x > 0 && z > 0 && x < SIDE - 1 && z < SIDE - 1 && beds[i].is_some();
                for (dx, dz) in [(-1, 0), (1, 0), (0, -1), (0, 1)] {
                    let (Some(nx), Some(nz)) = (x.checked_add_signed(dx), z.checked_add_signed(dz))
                    else {
                        continue;
                    };
                    if nx >= SIDE || nz >= SIDE {
                        continue;
                    }
                    let next = nx + SIDE * nz;
                    if air[next] && !seen[next] {
                        seen[next] = true;
                        queue.push_back(next);
                    }
                }
            }
            if sealed && (3..=180).contains(&cells.len()) {
                // A wall at the waterline alone is insufficient: reject hidden
                // submerged outlets before admitting water into this component.
                for &i in &cells {
                    for next in [i - 1, i + 1, i - SIDE, i + SIDE] {
                        if air[next] {
                            continue;
                        }
                        let x = (origin[0] + (next % SIDE) as i64) as f64 + 0.5;
                        let z = (origin[1] + (next / SIDE) as i64) as f64 + 0.5;
                        let biome = self.cave_biome_column(x, z);
                        let room = self.cave_room_at(x, z);
                        let shape = self.cave_shape(x, z);
                        let bed = beds[i].unwrap_or(level);
                        for y in bed as i64..level as i64 {
                            let y = y as f64 + 0.5;
                            let mut shape = shape;
                            let w = biome.weights_at(y);
                            shape.tunnel_width *=
                                f64::from(1.0 - w[1] * 0.10 + w[2] * 0.15 + w[3] * 0.08);
                            sealed &= room.carve(
                                y,
                                self.tunnel_density_with_shape([x, y, z], &shape)
                                    .min(biome.surface - y - 5.0),
                                biome.surface,
                            ) <= 0.0;
                        }
                    }
                }
                if !sealed {
                    continue;
                }
                let mut distance = [usize::MAX; SIDE * SIDE];
                let sink = cells.iter().copied().min_by(|a, b| {
                    beds[*a]
                        .unwrap_or(level)
                        .total_cmp(&beds[*b].unwrap_or(level))
                });
                if let Some(sink) = sink {
                    distance[sink] = 0;
                    let mut queue = VecDeque::from([sink]);
                    while let Some(i) = queue.pop_front() {
                        for next in [i - 1, i + 1, i - SIDE, i + SIDE] {
                            if air[next] && distance[next] == usize::MAX {
                                distance[next] = distance[i] + 1;
                                queue.push_back(next);
                            }
                        }
                    }
                }
                let length = cells.iter().map(|i| distance[*i]).max().unwrap_or(0);
                let flowing = length >= 12 && cells.len() >= 24;
                for i in cells {
                    if let Some(bed) = beds[i] {
                        let surface = if flowing {
                            level - 0.35 + 0.35 * distance[i] as f64 / length as f64
                        } else {
                            level
                        };
                        let mut flow = [0.0; 2];
                        if flowing {
                            for next in [i - 1, i + 1, i - SIDE, i + SIDE] {
                                if distance[next] < distance[i] {
                                    flow = [
                                        ((next % SIDE) as f32 - (i % SIDE) as f32) * 0.35,
                                        ((next / SIDE) as f32 - (i / SIDE) as f32) * 0.35,
                                    ];
                                    break;
                                }
                            }
                        }
                        result[i] = Some(ConnectorWater {
                            water: WaterColumn {
                                level: surface,
                                bed,
                                depth: surface - bed,
                            },
                            flow,
                        });
                    }
                }
            }
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::TerrainGenerator;
    #[test]
    fn connector_pools_have_real_bottoms_closed_sides_and_rebuild_identically() {
        let mut cfg = terra_core::WorldConfig::default();
        cfg.cave_biomes.distribution = terra_core::config::CaveDistribution {
            normal_weight: 0.0,
            salt_weight: 0.0,
            lush_weight: 0.0,
            cascade_weight: 1.0,
        };
        let t = TerraGenerator::new(cfg.clone()).unwrap();
        let mut checked = 0;
        'tiles: for z in -8..8 {
            for x in -8..8 {
                let cell = [x, z];
                let pool = t.connector_pocket(cell);
                if !pool.iter().any(Option::is_some) {
                    continue;
                }
                assert_eq!(pool, t.connector_pocket(cell));
                for (i, water) in pool.into_iter().enumerate() {
                    if let Some(sample) = water {
                        let w = sample.water;
                        if sample.flow != [0.0; 2] {
                            let next = (i as isize
                                + (sample.flow[0] / 0.35).round() as isize
                                + SIDE as isize * (sample.flow[1] / 0.35).round() as isize)
                                as usize;
                            let downstream = pool[next].expect("flow must follow connected water");
                            assert!(downstream.water.level < w.level);
                        }
                        let px = x * 32 + (i % 32) as i64;
                        let pz = z * 32 + (i / 32) as i64;
                        let bottom = [px as f64 + 0.5, w.bed - 0.5, pz as f64 + 0.5];
                        assert!(t.sample(bottom).density <= 0.0, "unsupported {bottom:?}");
                        assert_eq!(t.water_columns(bottom[0], bottom[2]).intervals[1], Some(w));
                        assert!(i % 32 > 0 && i % 32 < 31 && i / 32 > 0 && i / 32 < 31);
                        for j in [i - 1, i + 1, i - 32, i + 32] {
                            if pool[j].is_none() {
                                for y in w.bed as i64..w.level.ceil() as i64 {
                                    let p = [
                                        (x * 32 + (j % 32) as i64) as f64 + 0.5,
                                        y as f64 + 0.5,
                                        (z * 32 + (j / 32) as i64) as f64 + 0.5,
                                    ];
                                    assert!(t.sample(p).density <= 0.0, "open spill {p:?}");
                                }
                            }
                        }
                        checked += 1;
                    }
                }
                if checked > 20 {
                    break 'tiles;
                }
            }
        }
        assert!(checked > 0, "no connector pool generated");
    }
}
