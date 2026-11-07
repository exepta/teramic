use super::{
    DrainageFrame, HydrologyRegion, POINTS, RiverPath, RiverSample, TerraGenerator, layer_seed,
};
use crate::generation::{OasisSample, biomes::smoothstep, oases::BANK_REACH};
use std::{collections::VecDeque, sync::Arc};

const REGION: f64 = 2048.0;
const MARGIN: f64 = 352.0;

impl TerraGenerator {
    pub fn hydrology_region_at(&self, x: f64, z: f64) -> Option<HydrologyRegion> {
        if !self.connected_hydrology() || !self.config().water || !x.is_finite() || !z.is_finite() {
            return None;
        }
        let region = region_at(x, z);
        let path = self.connected_path(region)?;
        let course = path.points.map(|p| {
            let [x, z] = path.frame.world(p[0], p[2]);
            [x, p[1], z]
        });
        let mouth = course[POINTS - 1];
        Some(HydrologyRegion {
            region,
            source: course[0],
            mouth,
            course_length: course
                .windows(2)
                .map(|pair| (pair[1][0] - pair[0][0]).hypot(pair[1][2] - pair[0][2]))
                .sum(),
            reaches_sea: mouth[1] == self.config().sea_level,
            course,
            lake: path
                .lake
                .map(|lake| lake.sample(lake.center[0], lake.center[1])),
            tributaries: path
                .tributaries
                .iter()
                .map(|tributary| {
                    let course = tributary.path.points.map(|p| {
                        let [x, z] = tributary.path.frame.world(p[0], p[2]);
                        [x, p[1], z]
                    });
                    super::HydrologyTributary {
                        source: course[0],
                        mouth: course[POINTS - 1],
                        course,
                        accumulated_runoff: tributary.runoff,
                    }
                })
                .collect(),
            distributaries: path
                .distributaries
                .iter()
                .map(|branch| {
                    let course = branch.path.points.map(|p| {
                        let [x, z] = branch.path.frame.world(p[0], p[2]);
                        [x, p[1], z]
                    });
                    super::HydrologyDistributary {
                        split_index: branch.split_index,
                        source: course[0],
                        mouth: course[POINTS - 1],
                        course,
                        reaches_sea: branch.reaches_sea,
                    }
                })
                .collect(),
        })
    }

    pub(super) fn connected_river_at(&self, x: f64, z: f64) -> Option<RiverSample> {
        let path = self.connected_path(region_at(x, z))?;
        let main = self.sample_drainage(&path, x, z, None).map(|mut sample| {
            if !path.tributaries.is_empty() {
                let along = path.frame.project(x, z)[1];
                let accumulated: f64 = path
                    .tributaries
                    .iter()
                    .map(|tributary| {
                        let point = tributary.path.points[POINTS - 1];
                        let mouth = tributary.path.frame.world(point[0], point[2]);
                        let confluence = path.frame.project(mouth[0], mouth[1])[1];
                        tributary.runoff * smoothstep(confluence - 48.0, confluence + 48.0, along)
                    })
                    .sum();
                // Additional drainage widens the same real channel gradually;
                // no new water source or independent simulation is introduced.
                sample.width += (accumulated / 50_176.0).sqrt().min(4.0) * 0.35;
            }
            sample
        });
        if path.tributaries.is_empty() && path.distributaries.is_empty() {
            return main;
        }
        let tributary = path
            .tributaries
            .iter()
            .filter_map(|tributary| self.sample_tributary(&tributary.path, x, z))
            .min_by(|a, b| (a.distance / a.width).total_cmp(&(b.distance / b.width)));
        let mut sample = select_network_sample(main, tributary);
        for branch in &path.distributaries {
            sample = select_network_sample(sample, self.sample_tributary(&branch.path, x, z));
        }
        sample
    }

    pub(in crate::generation) fn connected_lake_at(&self, x: f64, z: f64) -> Option<OasisSample> {
        let lake = self.connected_path(region_at(x, z))?.lake?;
        if (x - lake.center[0]).abs().max((z - lake.center[1]).abs())
            > lake.radius * 1.9 + BANK_REACH
        {
            return None;
        }
        let sample = lake.sample(x, z);
        (sample.distance < sample.radius + BANK_REACH).then_some(sample)
    }

    pub(in crate::generation) fn connected_lake_conflicts(
        &self,
        center: [f64; 2],
        reach: f64,
    ) -> bool {
        self.connected_path(region_at(center[0], center[1]))
            .and_then(|path| path.lake)
            .is_some_and(|lake| {
                let sample = lake.sample(center[0], center[1]);
                // Covers the anisotropy of the smaller outer lobes as well.
                sample.distance < sample.radius + BANK_REACH + reach * 2.4
            })
    }

    fn connected_path(&self, cell: [i64; 2]) -> Option<Arc<super::network::RiverNetwork>> {
        if self.config().hill_amplitude == 0.0 && self.config().mountain_amplitude == 0.0 {
            return None;
        }
        // A shared initialization cell also deduplicates simultaneous vertical
        // section jobs. Expensive terrain probes happen once per catchment.
        let entry = if let Ok(mut cache) = self.rivers.1.lock() {
            if !cache.contains_key(&cell) && cache.len() >= 256 {
                if self.config().landscape_revision >= 5
                    || self.config().river_terrain_revision >= 2
                {
                    // Retain hot neighbours and in-flight OnceLocks. Evict one
                    // completed distant catchment, never the entire working set.
                    let distant = completed_cache_victim(&cache, cell);
                    if let Some(distant) = distant {
                        cache.remove(&distant);
                    } else {
                        drop(cache);
                        return self.build_connected_path(cell).map(Arc::new);
                    }
                } else {
                    cache.clear();
                }
            }
            Arc::clone(cache.entry(cell).or_default())
        } else {
            return self.build_connected_path(cell).map(Arc::new);
        };
        entry
            .get_or_init(|| self.build_connected_path(cell).map(Arc::new))
            .clone()
    }

    pub(super) fn brook_conflicts_with_region(&self, brook: &RiverPath) -> bool {
        let points = brook.points.map(|p| brook.frame.world(p[0], p[2]));
        let reach = brook.width * 1.38 + 1.2 + 12.0 + 2.0;
        let bounds = course_bounds(&points, reach);
        let lower = region_at(bounds[0][0], bounds[0][1]);
        let upper = region_at(bounds[1][0], bounds[1][1]);
        debug_assert!(upper[0] - lower[0] <= 1 && upper[1] - lower[1] <= 1);
        for z in lower[1]..=upper[1] {
            for x in lower[0]..=upper[0] {
                let Some(regional) = self.connected_path([x, z]) else {
                    continue;
                };
                if regional.lake.is_some_and(|lake| {
                    let reach = lake.radius * 1.9 + BANK_REACH;
                    overlaps(
                        bounds,
                        [
                            [lake.center[0] - reach, lake.center[1] - reach],
                            [lake.center[0] + reach, lake.center[1] + reach],
                        ],
                    )
                }) {
                    return true;
                }
                if !self.config().rivers {
                    continue;
                }
                for course in std::iter::once(&regional.main)
                    .chain(regional.tributaries.iter().map(|tributary| &tributary.path))
                    .chain(regional.distributaries.iter().map(|branch| &branch.path))
                {
                    let regional_points = course.points.map(|p| course.frame.world(p[0], p[2]));
                    let regional_reach = course.width * 1.38 + 7.0 + 36.0;
                    if !overlaps(bounds, course_bounds(&regional_points, regional_reach)) {
                        continue;
                    }
                    // Both immutable courses have 48 segments. AABB rejection is
                    // deliberately conservative, including every dry-bank apron.
                    // This runs once before caching the complete brook, not per voxel.
                    for pair in points.windows(2) {
                        let brook_bounds = segment_bounds(pair[0], pair[1], reach);
                        if regional_points.windows(2).any(|other| {
                            overlaps(
                                brook_bounds,
                                segment_bounds(other[0], other[1], regional_reach),
                            )
                        }) {
                            return true;
                        }
                    }
                }
            }
        }
        false
    }

    fn build_connected_path(&self, cell: [i64; 2]) -> Option<super::network::RiverNetwork> {
        let seed = layer_seed(
            self.config().seed.0
                ^ (cell[0] as u64).wrapping_mul(0x9e3779b97f4a7c15)
                ^ (cell[1] as u64).wrapping_mul(0xbf58476d1ce4e5b9),
            171,
        );
        let origin = [cell[0] as f64 * REGION, cell[1] as f64 * REGION];
        let probes: [([f64; 2], f64); 49] = std::array::from_fn(|index| {
            let point = [
                origin[0] + MARGIN + (index % 7) as f64 * (REGION - 2.0 * MARGIN) / 6.0,
                origin[1] + MARGIN + (index / 7) as f64 * (REGION - 2.0 * MARGIN) / 6.0,
            ];
            (
                point,
                self.base_height_with_biome(
                    point[0],
                    point[1],
                    self.climate_biome_at(point[0], point[1]),
                ),
            )
        });
        let mut coastal: Vec<_> = probes
            .iter()
            .filter(|(point, height)| {
                *height < self.config().sea_level - 1.0
                    && self.climate_biome_at(point[0], point[1]).weights.ocean >= 0.35
            })
            .collect();
        // Prefer a real shore rather than extending the visible course far
        // across a deep ocean bed. Connectivity is checked in bounded world space.
        coastal.sort_unstable_by(|a, b| b.1.total_cmp(&a.1));
        let coastal = coastal
            .into_iter()
            .take(3)
            .find(|(point, _)| self.connected_ocean_outlet(*point));
        let (mouth, mouth_height, coast, inland_lake) = if let Some(&(point, height)) = coastal {
            (point, height, true, None)
        } else {
            let mut inland: Vec<_> = probes
                .iter()
                .filter(|(_, height)| *height > self.config().sea_level + 5.0)
                .collect();
            inland.sort_unstable_by(|a, b| a.1.total_cmp(&b.1));
            let (point, height, lake) =
                inland.into_iter().take(3).find_map(|&(point, height)| {
                    self.build_regional_lake(point, seed)
                        .map(|lake| (point, height, lake))
                })?;
            (point, height, false, Some(lake))
        };
        let aridity = self.climate_biome_at(mouth[0], mouth[1]).aridity();
        if aridity > 0.55 && !seed.is_multiple_of(7) {
            return None;
        }
        let &(source, _) = probes
            .iter()
            .filter(|(point, height)| {
                distance(*point, mouth) >= 800.0
                    && *height > mouth_height + 3.0
                    && *height > self.config().sea_level + 6.0
            })
            .max_by(|a, b| {
                let rank = |p: &([f64; 2], f64)| {
                    distance(p.0, mouth) + (p.1 - mouth_height).min(80.0) * 2.0
                };
                rank(a).total_cmp(&rank(b))
            })?;
        let length = distance(source, mouth);
        let step = length / (POINTS - 1) as f64;
        let frame = DrainageFrame {
            origin: source,
            downstream: [
                (mouth[0] - source[0]) / length,
                (mouth[1] - source[1]) / length,
            ],
        };
        let mut lake = if coast {
            let mut candidates: Vec<_> = probes
                .iter()
                .filter(|(point, height)| {
                    let [cross, along] = frame.project(point[0], point[1]);
                    (length * 0.30..length * 0.78).contains(&along)
                        && cross.abs() < 400.0
                        && *height > self.config().sea_level + 6.0
                })
                .collect();
            candidates.sort_unstable_by(|a, b| a.1.total_cmp(&b.1));
            candidates
                .into_iter()
                .take(3)
                .find_map(|(point, _)| self.build_regional_lake(*point, seed))
        } else {
            inland_lake
        };
        // The interior margin contains even the rare widest multi-lobe lake.
        // Thus a column consults one catchment, not an unbounded neighbour graph.
        lake = lake.filter(|lake| {
            let reach = lake.radius * 1.9 + BANK_REACH;
            lake.center
                .into_iter()
                .zip(origin)
                .all(|(c, o)| c - reach > o && c + reach < o + REGION)
        });
        let lake_anchor = lake.map(|lake| frame.project(lake.center[0], lake.center[1]));
        let phase = f64::from(seed % 1024) * std::f64::consts::TAU / 1024.0;
        let points: [[f64; 3]; POINTS] = std::array::from_fn(|index| {
            let t = index as f64 / (POINTS - 1) as f64;
            let z = t * length;
            let (bend, meander) = lake_anchor.map_or((0.0, 1.0), |[cross, along]| {
                let anchor = (along / length).clamp(0.001, 0.999);
                let weight = if t <= anchor {
                    smoothstep(0.0, anchor, t)
                } else {
                    1.0 - smoothstep(anchor, 1.0, t)
                };
                (cross * weight, 1.0 - (-((t - anchor) / 0.14).powi(2)).exp())
            });
            let x = bend
                + (z * 0.009 + phase).sin()
                    * (t * std::f64::consts::PI).sin()
                    * (22.0 + f64::from((seed >> 12) % 30))
                    * meander;
            [x, 0.0, z]
        });
        let mut ground = [0.0; POINTS];
        let mut caps = [0.0; POINTS];
        for (index, point) in points.iter().enumerate() {
            let world = frame.world(point[0], point[2]);
            ground[index] = self.base_height_with_biome(
                world[0],
                world[1],
                self.climate_biome_at(world[0], world[1]),
            );
            let mut support = f64::INFINITY;
            for along in [-0.5, -0.25, 0.0, 0.25, 0.5] {
                let at = (index as f64 + along).clamp(0.0, (POINTS - 1) as f64);
                let a = at.floor() as usize;
                let b = (a + 1).min(POINTS - 1);
                let center_x = points[a][0] + (points[b][0] - points[a][0]) * at.fract();
                for cross in (-24..=24).step_by(3) {
                    let [x, z] = frame.world(center_x + f64::from(cross), at * step);
                    support =
                        support.min(self.base_height_with_biome(x, z, self.climate_biome_at(x, z)));
                }
            }
            caps[index] = (support - 3.0).max(self.config().sea_level);
        }
        if let Some(ref mut lake) = lake {
            let upstream = points
                .iter()
                .zip(caps)
                .take_while(|(p, _)| {
                    let [x, z] = frame.world(p[0], p[2]);
                    let sample = lake.sample(x, z);
                    sample.distance > sample.radius + BANK_REACH
                })
                .map(|(_, cap)| cap)
                .fold(f64::INFINITY, f64::min);
            lake.level = lake.level.min(upstream);
        }
        lake = lake.filter(|lake| {
            let height = self.base_height_with_biome(
                lake.center[0],
                lake.center[1],
                self.climate_biome_at(lake.center[0], lake.center[1]),
            );
            lake.level > self.config().sea_level + 1.0 && height - lake.level < 24.0
        });
        if !coast && lake.is_none() {
            return None;
        }
        let mut previous = f64::INFINITY;
        let points = std::array::from_fn(|index| {
            let mut point = points[index];
            let [x, z] = frame.world(point[0], point[2]);
            let level = lake
                .filter(|lake| {
                    let sample = lake.sample(x, z);
                    sample.distance <= sample.radius + BANK_REACH
                })
                .map_or(caps[index], |lake| lake.level);
            previous = previous.min(level);
            point[1] = previous;
            point
        });
        let main = RiverPath {
            points,
            ground,
            extensions: [0.0; 2],
            width: 3.0 + f64::from((seed >> 5) % 6) * 0.65,
            phase,
            branch: None,
            brook: false,
            step,
            frame,
            lake,
        };
        Some(self.regional_network(main, &probes))
    }

    pub(super) fn connected_ocean_outlet(&self, mouth: [f64; 2]) -> bool {
        let wet = |point: [f64; 2]| {
            let biome = self.climate_biome_at(point[0], point[1]);
            let height = self.base_height_with_biome(point[0], point[1], biome);
            (height < self.config().sea_level - 1.0).then_some((biome.weights.ocean, height))
        };
        let mut seen = [false; 81];
        let mut queue = VecDeque::from([[0_i32, 0_i32]]);
        seen[40] = true;
        while let Some([x, z]) = queue.pop_front() {
            let point = [
                mouth[0] + f64::from(x) * 16.0,
                mouth[1] + f64::from(z) * 16.0,
            ];
            let Some((ocean, height)) = wet(point) else {
                continue;
            };
            if x * x + z * z >= 9 && ocean >= 0.55 && height < self.config().sea_level - 3.0 {
                return true;
            }
            for [dx, dz] in [[-1, 0], [1, 0], [0, -1], [0, 1]] {
                let [nx, nz] = [x + dx, z + dz];
                if !(-4..=4).contains(&nx) || !(-4..=4).contains(&nz) {
                    continue;
                }
                let index = (nx + 4 + (nz + 4) * 9) as usize;
                if seen[index] {
                    continue;
                }
                // Every connecting metre remains below the sea reservoir, so a
                // narrow land barrier cannot masquerade as a connected outlet.
                if (1..=16).all(|step| {
                    wet([
                        point[0] + f64::from(dx * step),
                        point[1] + f64::from(dz * step),
                    ])
                    .is_some()
                }) {
                    seen[index] = true;
                    queue.push_back([nx, nz]);
                }
            }
        }
        false
    }
}

fn region_at(x: f64, z: f64) -> [i64; 2] {
    [(x / REGION).floor() as i64, (z / REGION).floor() as i64]
}

pub(super) fn completed_cache_victim(
    cache: &std::collections::HashMap<[i64; 2], super::RegionalPathCell>,
    cell: [i64; 2],
) -> Option<[i64; 2]> {
    cache
        .iter()
        .filter(|(_, entry)| entry.get().is_some())
        .max_by_key(|(key, _)| {
            (
                key[0].abs_diff(cell[0]).max(key[1].abs_diff(cell[1])),
                **key,
            )
        })
        .map(|(key, _)| *key)
}

fn wet_channel(sample: RiverSample) -> bool {
    sample.distance < sample.width && (sample.bed_height() - 0.5).floor() + 1.0 < sample.level
}

pub(super) fn select_network_sample(
    main: Option<RiverSample>,
    tributary: Option<RiverSample>,
) -> Option<RiverSample> {
    // The one-metre water-level influence also includes dry shoreline cells.
    // Only actual wet channel cells may hide an incoming tributary's bed.
    if main.is_some_and(wet_channel) {
        return main;
    }
    let Some(mut tributary) = tributary else {
        return main;
    };
    if let Some(main) = main {
        let blend = 1.0 - smoothstep(main.width + 1.0, main.width + 13.0, main.distance);
        tributary.level += (main.level - tributary.level) * blend;
        tributary.bank_level += (main.bank_level - tributary.bank_level) * blend;
    }
    if wet_channel(tributary) {
        return Some(tributary);
    }
    if main.is_none_or(|main| tributary.distance / tributary.width < main.distance / main.width) {
        Some(tributary)
    } else {
        main
    }
}

fn distance(a: [f64; 2], b: [f64; 2]) -> f64 {
    (a[0] - b[0]).hypot(a[1] - b[1])
}

fn course_bounds(points: &[[f64; 2]; POINTS], reach: f64) -> [[f64; 2]; 2] {
    points.iter().fold(
        [[f64::INFINITY; 2], [f64::NEG_INFINITY; 2]],
        |mut bounds, point| {
            for axis in 0..2 {
                bounds[0][axis] = bounds[0][axis].min(point[axis] - reach);
                bounds[1][axis] = bounds[1][axis].max(point[axis] + reach);
            }
            bounds
        },
    )
}

fn segment_bounds(a: [f64; 2], b: [f64; 2], reach: f64) -> [[f64; 2]; 2] {
    [
        [a[0].min(b[0]) - reach, a[1].min(b[1]) - reach],
        [a[0].max(b[0]) + reach, a[1].max(b[1]) + reach],
    ]
}

fn overlaps(a: [[f64; 2]; 2], b: [[f64; 2]; 2]) -> bool {
    (0..2).all(|axis| a[0][axis] <= b[1][axis] && b[0][axis] <= a[1][axis])
}

#[cfg(test)]
mod tests;
