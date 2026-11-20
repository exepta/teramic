//! Revision-seven long catchments. Local streams remain independently cached;
//! a column consults one additional immutable macro basin, never a world graph.
use super::network::{Distributary, RiverNetwork, Tributary};
use super::{DrainageFrame, POINTS, RiverPath, TerraGenerator, layer_seed};
use crate::generation::{biomes::smoothstep, lakes::Lake};
use std::sync::Arc;

const BASIN: f64 = 32_768.0;
const CACHE: usize = 16;

pub(super) struct MacroRoute {
    pub source: [f64; 2],
    pub mouth: [f64; 2],
    pub coast: bool,
    pub lake: Option<Lake>,
}

fn key(x: f64, z: f64) -> [i64; 2] {
    [(x / BASIN).floor() as i64, (z / BASIN).floor() as i64]
}

impl TerraGenerator {
    /// The largest basins need a route search, rather than choosing one nearby
    /// sea probe first and discarding every other outlet in the catchment.
    pub(super) fn long_macro_endpoints(
        &self,
        probes: &[([f64; 2], f64); 49],
        seed: u32,
    ) -> Option<MacroRoute> {
        const MINIMUM: f64 = 17_000.0;
        let sea = self.config().sea_level;
        let mut coastal = Vec::new();
        let mut inland = Vec::new();
        for &(mouth, height) in probes {
            let coast = height < sea - 1.0
                && self.climate_biome_at(mouth[0], mouth[1]).weights.ocean >= 0.35;
            if !coast && height <= sea + 5.0 {
                continue;
            }
            for &(source, source_height) in probes {
                let length = distance(source, mouth);
                if length < MINIMUM || source_height < (height + 3.0).max(sea + 6.0) {
                    continue;
                }
                // Sixteen cheap probes discard obvious sea crossings before
                // the dense, metre-scale shoreline/channel validation below.
                let mut dry_distance = length;
                let mut valid = true;
                for i in 1..=16 {
                    let t = i as f64 / 16.0;
                    let p = [
                        source[0] + (mouth[0] - source[0]) * t,
                        source[1] + (mouth[1] - source[1]) * t,
                    ];
                    if self.terrain_base_height(p[0], p[1]) < sea - 0.5 {
                        dry_distance = length * (i - 1) as f64 / 16.0;
                        valid = coast && dry_distance >= MINIMUM - length / 16.0;
                        break;
                    }
                }
                if valid {
                    let candidate = (source, mouth, dry_distance);
                    if coast {
                        coastal.push(candidate);
                    } else {
                        inland.push(candidate);
                    }
                }
            }
        }
        coastal.sort_unstable_by(|a, b| b.2.total_cmp(&a.2));
        for (source, sea_probe, _) in coastal.into_iter().take(24) {
            let Some(mouth) = self.macro_shore(source, sea_probe) else {
                continue;
            };
            if distance(source, mouth) >= MINIMUM && self.connected_ocean_outlet(sea_probe) {
                return Some(MacroRoute {
                    source,
                    mouth,
                    coast: true,
                    lake: None,
                });
            }
        }
        inland.sort_unstable_by(|a, b| b.2.total_cmp(&a.2));
        for (source, mouth, _) in inland.into_iter().take(24) {
            if !self.dry_macro_line(source, mouth) {
                continue;
            }
            if let Some(lake) = self.build_regional_lake(mouth, seed) {
                return Some(MacroRoute {
                    source,
                    mouth,
                    coast: false,
                    lake: Some(lake),
                });
            }
        }
        None
    }

    pub(super) fn macro_path_at(&self, x: f64, z: f64) -> Option<Arc<RiverNetwork>> {
        if self.config().river_terrain_revision < 7
            || !self.config().rivers
            || self.config().hill_amplitude == 0.0 && self.config().mountain_amplitude == 0.0
        {
            return None;
        }
        let cell = key(x, z);
        let entry = {
            let mut cache = self.rivers.2.lock().unwrap_or_else(|e| e.into_inner());
            if !cache.contains_key(&cell) && cache.len() >= CACHE {
                if let Some(old) = super::connected::completed_cache_victim(&cache, cell) {
                    cache.remove(&old);
                } else {
                    drop(cache);
                    return self.build_macro(cell).map(Arc::new);
                }
            }
            Arc::clone(cache.entry(cell).or_default())
        };
        entry
            .get_or_init(|| self.build_macro(cell).map(Arc::new))
            .clone()
    }

    pub(super) fn macro_lake_at(&self, x: f64, z: f64) -> Option<crate::generation::OasisSample> {
        let network = self.macro_path_at(x, z)?;
        std::iter::once(network.main.lake)
            .chain(network.tributaries.iter().map(|p| p.path.lake))
            .chain(network.distributaries.iter().map(|p| p.path.lake))
            .flatten()
            .map(|lake| self.spirit_lake_sample(lake, x, z))
            .filter(|sample| sample.distance < sample.radius + crate::generation::oases::BANK_REACH)
            .min_by(|a, b| a.distance.total_cmp(&b.distance))
    }

    fn build_macro(&self, cell: [i64; 2]) -> Option<RiverNetwork> {
        let seed = layer_seed(
            self.config().seed.0
                ^ (cell[0] as u64).wrapping_mul(0x9e3779b97f4a7c15)
                ^ (cell[1] as u64).wrapping_mul(0xbf58476d1ce4e5b9),
            719,
        );
        let extent = [8192.0, 16384.0, 30720.0][(seed % 3) as usize];
        let margin = (BASIN - extent) * 0.5;
        self.build_catchment(
            cell.map(|c| c.wrapping_add(0x100000)),
            [
                cell[0] as f64 * BASIN + margin,
                cell[1] as f64 * BASIN + margin,
            ],
            extent,
            true,
        )
    }

    /// Find the first real sea crossing along a candidate route. Coarse basin
    /// probes can lie kilometres offshore; counting that water as river length
    /// would produce a nominal long course with an invisible ocean gap.
    pub(super) fn macro_shore(&self, source: [f64; 2], mouth: [f64; 2]) -> Option<[f64; 2]> {
        let length = distance(source, mouth);
        let steps = (length / 48.0).ceil().clamp(1.0, 1024.0) as usize;
        let mut previous = source;
        for i in 1..=steps {
            let t = i as f64 / steps as f64;
            let p = [
                source[0] + (mouth[0] - source[0]) * t,
                source[1] + (mouth[1] - source[1]) * t,
            ];
            if self.terrain_base_height(p[0], p[1]) < self.config().sea_level - 0.6 {
                if self.climate_biome_at(p[0], p[1]).weights.ocean < 0.45 {
                    return None;
                }
                let mut dry = previous;
                let mut wet = p;
                for _ in 0..12 {
                    let middle = [(dry[0] + wet[0]) * 0.5, (dry[1] + wet[1]) * 0.5];
                    if self.terrain_base_height(middle[0], middle[1])
                        < self.config().sea_level - 0.6
                    {
                        wet = middle;
                    } else {
                        dry = middle;
                    }
                }
                return Some(wet);
            }
            previous = p;
        }
        None
    }

    pub(super) fn dry_macro_line(&self, source: [f64; 2], mouth: [f64; 2]) -> bool {
        let length = distance(source, mouth);
        let steps = (length / 48.0).ceil().clamp(1.0, 1024.0) as usize;
        (0..steps).all(|i| {
            let t = i as f64 / steps as f64;
            if (1.0 - t) * length < 96.0 {
                return true;
            }
            let p = [
                source[0] + (mouth[0] - source[0]) * t,
                source[1] + (mouth[1] - source[1]) * t,
            ];
            self.terrain_base_height(p[0], p[1]) > self.config().sea_level - 0.5
        })
    }

    pub(super) fn macro_network(
        &self,
        main: RiverPath,
        probes: &[([f64; 2], f64); 49],
        seed: u32,
    ) -> Option<RiverNetwork> {
        // A long course cannot jump over an intervening sea and reappear inland.
        for pair in main.points.windows(2).take(POINTS - 3) {
            let a = main.frame.world(pair[0][0], pair[0][2]);
            let b = main.frame.world(pair[1][0], pair[1][2]);
            if !self.dry_macro_line(a, b) {
                return None;
            }
        }
        let mut network = RiverNetwork {
            main,
            tributaries: Vec::new(),
            distributaries: Vec::new(),
        };
        for split in [12, 26] {
            let p = network.main.points[split];
            let join = network.main.frame.world(p[0], p[2]);
            let mut candidates: Vec<_> = probes
                .iter()
                .filter(|(point, height)| distance(*point, join) > 2500.0 && *height > p[1] + 5.0)
                .collect();
            candidates.sort_unstable_by(|a, b| distance(b.0, join).total_cmp(&distance(a.0, join)));
            let path = candidates
                .into_iter()
                .take(6)
                .find_map(|(source, _)| {
                    self.macro_branch(*source, join, None, p[1], seed ^ split as u32)
                })
                .or_else(|| {
                    // The coarse basin grid is five kilometres apart on a long
                    // river. Its furthest probes frequently lie across another
                    // sea or a lower side valley. Search the actual upstream land
                    // corridor as well, retaining distinct kilometre-long arms.
                    if network.main.step * ((POINTS - 1) as f64) < 16_000.0 {
                        return None;
                    }
                    let mut sources = Vec::new();
                    for upstream in [0.0, 2400.0, 4800.0, 7200.0] {
                        for cross in [
                            -4800.0, -2400.0, -1200.0, -600.0, 600.0, 1200.0, 2400.0, 4800.0,
                        ] {
                            let source = network.main.frame.world(p[0] + cross, p[2] - upstream);
                            if distance(source, join) < 2500.0
                                || source[0] < probes[0].0[0]
                                || source[1] < probes[0].0[1]
                                || source[0] > probes[48].0[0]
                                || source[1] > probes[48].0[1]
                                || self.terrain_base_height(source[0], source[1]) < p[1] + 5.0
                            {
                                continue;
                            }
                            if (0..=16).all(|i| {
                                let t = i as f64 / 16.0;
                                self.terrain_base_height(
                                    source[0] + (join[0] - source[0]) * t,
                                    source[1] + (join[1] - source[1]) * t,
                                ) >= p[1] + 1.0
                            }) {
                                sources.push(source);
                            }
                        }
                    }
                    sources.sort_unstable_by(|a, b| {
                        (distance(*a, join) - 4800.0)
                            .abs()
                            .total_cmp(&(distance(*b, join) - 4800.0).abs())
                    });
                    sources.into_iter().take(16).find_map(|source| {
                        self.macro_branch(source, join, None, p[1], seed ^ split as u32)
                    })
                });
            if let Some(path) = path {
                network.tributaries.push(Tributary {
                    path,
                    runoff: 8_000_000.0,
                });
            }
        }
        // A genuine outgoing fork terminates in its own lower lake, rather than
        // looping back to the parent after a few neighbouring blocks.
        for split in [2, 8, 18] {
            let p = network.main.points[split];
            let source = network.main.frame.world(p[0], p[2]);
            let mut candidates: Vec<_> = probes
                .iter()
                .filter(|(point, height)| {
                    distance(*point, source) > 2500.0
                        && *height > self.config().sea_level + 5.0
                        && *height < p[1] + 18.0
                })
                .collect();
            candidates
                .sort_unstable_by(|a, b| distance(b.0, source).total_cmp(&distance(a.0, source)));
            let branch = candidates.into_iter().take(8).find_map(|(mouth, _)| {
                let mut lake = self.build_regional_lake(*mouth, seed ^ 0x7b91)?;
                lake.level = lake.level.min(p[1] - 1.0);
                if lake.level <= self.config().sea_level + 1.0
                    || self.terrain_base_height(mouth[0], mouth[1]) - lake.level > 24.0
                {
                    return None;
                }
                let mut path =
                    self.macro_branch(source, *mouth, Some(lake), lake.level, seed ^ 0x7b91)?;
                // The split shares the exact parent's level and position.
                if path.points[0][1] + 0.01 < p[1] {
                    return None;
                }
                for point in &mut path.points {
                    point[1] = point[1].min(p[1]);
                }
                path.points[0][1] = p[1];
                Some(Distributary {
                    path,
                    split_index: split,
                    reaches_sea: false,
                })
            });
            if let Some(branch) = branch {
                network.distributaries.push(branch);
                break;
            }
        }
        Some(network)
    }

    fn macro_branch(
        &self,
        source: [f64; 2],
        mouth: [f64; 2],
        lake: Option<Lake>,
        terminal: f64,
        seed: u32,
    ) -> Option<RiverPath> {
        if !self.dry_macro_line(source, mouth) {
            return None;
        }
        let length = distance(source, mouth);
        let frame = DrainageFrame {
            origin: source,
            downstream: [
                (mouth[0] - source[0]) / length,
                (mouth[1] - source[1]) / length,
            ],
        };
        let step = length / (POINTS - 1) as f64;
        let phase = f64::from(seed % 1024) * std::f64::consts::TAU / 1024.0;
        let mut ground = [0.0; POINTS];
        let mut support = [f64::INFINITY; POINTS];
        let mut points = std::array::from_fn(|i| {
            let t = i as f64 / (POINTS - 1) as f64;
            [
                ((t * std::f64::consts::PI).sin()) * (t * 8.0 + phase).sin() * 64.0,
                0.0,
                t * length,
            ]
        });
        for (i, p) in points.iter().enumerate() {
            let world = frame.world(p[0], p[2]);
            ground[i] = self.terrain_base_height(world[0], world[1]);
            let subdivisions = (step / 24.0).ceil().clamp(4.0, 64.0) as usize;
            for n in 0..=subdivisions {
                let at = (i as f64 + n as f64 / subdivisions as f64 - 0.5)
                    .clamp(0.0, (POINTS - 1) as f64);
                let a = at.floor() as usize;
                let b = (a + 1).min(POINTS - 1);
                let cross = points[a][0] + (points[b][0] - points[a][0]) * at.fract();
                for side in [-12.0, 0.0, 12.0] {
                    let q = frame.world(cross + side, at * step);
                    support[i] = support[i].min(self.terrain_base_height(q[0], q[1]) - 2.0);
                }
            }
            // Keep at least one metre of real bank above the receiving water.
            // The cap's extra metre is bed erosion allowance, not a reason to
            // reject an otherwise supported, shallow valley confluence.
            if support[i] + 1.0 < terminal {
                return None;
            }
        }
        let mut previous = f64::INFINITY;
        for (i, p) in points.iter_mut().enumerate() {
            let t = i as f64 / (POINTS - 1) as f64;
            previous = previous.min(support[i].max(terminal));
            p[1] = terminal + (previous - terminal) * (1.0 - smoothstep(0.0, 1.0, t));
        }
        points[POINTS - 1][1] = terminal;
        Some(RiverPath {
            points,
            ground,
            extensions: [0.0; 2],
            width: 4.2,
            phase,
            branch: None,
            brook: false,
            step,
            frame,
            lake,
        })
    }
}

fn distance(a: [f64; 2], b: [f64; 2]) -> f64 {
    (a[0] - b[0]).hypot(a[1] - b[1])
}

#[cfg(test)]
mod tests;
