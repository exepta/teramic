use super::{TerraGenerator, seed::layer_seed};
use crate::water::MIN_WATERFALL_DROP;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

const SPACING: f64 = 512.0;
const START: f64 = -192.0;
const STEP: f64 = 8.0;
const POINTS: usize = 49;
const BANK_REACH: f64 = 64.0;
// Includes the widest terminal pool and its immediate bank. Sample once per
// cached path, not in each voxel/column query.
const SUPPORT_REACH: i32 = 20;

#[cfg(test)]
mod brook_tests;
mod erosion;

#[derive(Default)]
pub(super) struct RiverCache(Mutex<HashMap<[i64; 3], Option<Arc<RiverPath>>>>);
struct RiverPath {
    points: [[f64; 3]; POINTS],
    ground: [f64; POINTS],
    extensions: [f64; 2],
    width: f64,
    phase: f64,
    branch: Option<RiverBranch>,
    brook: bool,
}

#[derive(Clone, Copy)]
struct RiverBranch {
    start: f64,
    end: f64,
    offset: f64,
}

impl RiverBranch {
    fn sample(self, t: f64) -> Option<[f64; 2]> {
        if !(self.start..=self.end).contains(&t) {
            return None;
        }
        let phase = (t - self.start) / (self.end - self.start) * std::f64::consts::PI;
        Some([
            self.offset * phase.sin().powi(2),
            self.offset * (phase * 2.0).sin() * std::f64::consts::PI / (self.end - self.start),
        ])
    }
}

impl RiverPath {
    fn level(&self, index: usize, along: f64) -> f64 {
        let upper = self.points[index][1];
        let lower = self.points[(index + 1).min(POINTS - 1)][1];
        if upper - lower >= MIN_WATERFALL_DROP {
            // The cliff is on an integer world edge, not a staircase at every path sample.
            if along < 0.5 { upper } else { lower }
        } else {
            upper + (lower - upper) * along
        }
    }

    fn bank_level(&self, index: usize, t: f64, level: f64, lateral: f64) -> f64 {
        let mut bank = level;
        for i in index.saturating_sub(6)..=(index + 6).min(POINTS - 2) {
            let drop = self.points[i][1] - self.points[i + 1][1];
            if drop < MIN_WATERFALL_DROP {
                continue;
            }
            let edge = (i as f64 + 0.5) * STEP;
            let reach = (8.0 + drop * 0.65).clamp(12.0, 36.0);
            let erosion = (lateral * 0.11 + self.phase).sin() * reach * 0.12
                + (lateral * 0.27 - self.phase * 0.7).sin() * 1.1;
            let abrupt = if t >= edge { 1.0 } else { 0.0 };
            let eroded = super::biomes::smoothstep(edge - reach, edge + reach, t + erosion);
            // Cancel the hydraulic jump only for the exterior valley profile.
            // The actual wet channel keeps its full, grid-aligned waterfall lip.
            bank += drop * (abrupt - eroded);
        }
        bank
    }

    fn cascade_height(&self, index: usize, t: f64, level: f64, lateral: f64) -> f64 {
        let mut height = 0.0_f64;
        for i in index.saturating_sub(4)..=index.min(POINTS - 2) {
            let edge = (i as f64 + 0.5) * STEP;
            if t < edge || self.points[i][1] - self.points[i + 1][1] < MIN_WATERFALL_DROP {
                continue;
            }
            // Keep the full plunge shaft, then close its roof over several
            // sections instead of ending the high chamber on a flat rock wall.
            let erosion = (lateral * 0.37 + self.phase).sin() * 2.0;
            let taper = 1.0 - super::biomes::smoothstep(6.0, 28.0, t - edge + erosion);
            height = height.max((self.points[i][1] - level).max(0.0) * taper);
        }
        height
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RiverSample {
    pub level: f64,
    pub distance: f64,
    pub width: f64,
    pub flow: [f32; 2],
    /// Centre depth varies along the river: riffles alternate with deeper pools.
    pub depth: f64,
    /// A deeply buried reach retains its terrain roof and carves a real passage.
    pub covered: bool,
    /// Space above the lower pool needed to join a covered cascading reach.
    pub cascade_height: f64,
    /// Gradually restores the exterior terrain around eroded tunnel entrances.
    pub roof_blend: f64,
    roof_detail: f64,
    bed_detail: f64,
    /// Open fork islands are low alluvial banks, never a slice of a mountain.
    pub(super) island_height: Option<f64>,
    pub(super) bank_reach: f64,
    pub(super) bank_level: f64,
    pub(super) bank_erosion: f64,
    /// Local cataract amphitheatre, absent from the saved legacy terrain contract.
    pub(super) fall_erosion: f64,
}

impl RiverSample {
    pub(crate) fn bed_height(self) -> f64 {
        let across = self.distance / self.width;
        let detail = self.bed_detail * (1.0 - across.min(1.0)).powi(2);
        self.level - self.depth + (self.depth + 0.45) * across * across + detail
    }

    pub(crate) fn cavity(self) -> Option<[f64; 2]> {
        if self.roof_blend <= 0.0 || self.distance >= self.width + 1.5 {
            return None;
        }
        let floor = self.bed_height();
        let arch = (1.0 - (self.distance / (self.width + 1.5)).powi(2))
            .max(0.0)
            .sqrt();
        let ceiling = self.level + self.cascade_height + 1.4 + (5.4 + self.roof_detail) * arch;
        (ceiling > floor).then_some([floor, ceiling])
    }

    pub fn water_level(self, sea_level: f64) -> f64 {
        // The wider influence is for carving/blending land, not for filling it.
        // Otherwise an unrelated low slope becomes an unsupported elevated lake.
        if self.distance <= self.width + 1.0 {
            self.level
        } else {
            sea_level
        }
    }
}

impl TerraGenerator {
    /// Small bounded immutable drainage paths, shared by all vertical jobs and clones.
    pub fn river_at(&self, x: f64, z: f64) -> Option<RiverSample> {
        if !self.config().water || !x.is_finite() || !z.is_finite() {
            return None;
        }
        let main = self
            .config()
            .rivers
            .then(|| self.drainage_at(x, z, false))
            .flatten();
        if main.is_some_and(|r| r.distance <= r.width + 1.0) {
            return main;
        }
        let brook = (self.config().ecology_revision > 0)
            .then(|| self.drainage_at(x, z, true))
            .flatten();
        brook.or(main)
    }

    fn drainage_at(&self, x: f64, z: f64, brook: bool) -> Option<RiverSample> {
        let cell = [
            ((x + SPACING * 0.5) / SPACING).floor() as i64,
            ((z + SPACING * 0.5) / SPACING).floor() as i64,
        ];
        let local_x = x - cell[0] as f64 * SPACING;
        if !(if brook { -156.0..=-8.0 } else { -96.0..=224.0 }).contains(&local_x) {
            return None;
        }
        let key = [cell[0], cell[1], i64::from(brook)];
        let cached = self
            .rivers
            .0
            .lock()
            .ok()
            .and_then(|cache| cache.get(&key).cloned());
        let path = cached.unwrap_or_else(|| {
            let path = self
                .drainage_enabled(cell, brook)
                .then(|| Arc::new(self.build_drainage(cell, brook)));
            if let Ok(mut cache) = self.rivers.0.lock() {
                if cache.len() >= 256 {
                    cache.clear();
                }
                cache.entry(key).or_insert_with(|| path.clone()).clone()
            } else {
                path
            }
        })?;
        let length = (POINTS - 1) as f64 * STEP;
        let t = (z - path.points[0][2]).clamp(-path.extensions[0], length + path.extensions[1]);
        let core_t = t.clamp(0.0, length);
        let index = (core_t / STEP).floor() as usize;
        let next = (index + 1).min(POINTS - 1);
        let a = path.points[index];
        let b = path.points[next];
        let along = (core_t / STEP).fract();
        let dx = if t >= length {
            (path.points[POINTS - 1][0] - path.points[POINTS - 2][0]) / STEP
        } else {
            (b[0] - a[0]) / STEP
        };
        let center_x = a[0] + (b[0] - a[0]) * along + dx * (t - core_t);
        let center_z = path.points[0][2] + t;
        let mut lateral = x - center_x;
        let mut distance = lateral.hypot(z - center_z);
        let pool = super::biomes::smoothstep(POINTS as f64 * STEP - 40.0, length, core_t);
        let mut width = path.width
            * (1.0
                + 0.26 * (core_t * 0.024 + path.phase).sin()
                + 0.12 * (core_t * 0.067 + path.phase * 0.4).sin())
            + pool * if path.brook { 1.2 } else { 4.0 };
        let mut flow_dx = dx;
        let mut branch_depth = 1.0;
        let mut island_height = None;
        if let Some([offset, derivative]) = path.branch.and_then(|branch| branch.sample(core_t)) {
            let branch_width = width * (1.0 - (offset.abs() / 22.0).min(1.0) * 0.36);
            let branch_lateral = x - (center_x + offset);
            let branch_distance = branch_lateral.hypot(z - center_z);
            let between = lateral * branch_lateral <= 0.0;
            if between && offset.abs() > 1.0 {
                let t = (lateral / offset).clamp(0.0, 1.0);
                island_height = Some(0.8 + (t * std::f64::consts::PI).sin() * 1.7);
            }
            if branch_distance / branch_width < distance / width {
                distance = branch_distance;
                width = branch_width;
                lateral = branch_lateral;
                flow_dx += derivative;
                branch_depth = 0.76;
            }
        }
        let bank_reach = if path.brook { 12.0 } else { BANK_REACH };
        (distance <= width + bank_reach).then(|| {
            let length = flow_dx.hypot(1.0);
            let level = path.level(index, along);
            let bank_level = path.bank_level(index, core_t, level, lateral);
            let ground = path.ground[index] + (path.ground[next] - path.ground[index]) * along;
            let erosion = (core_t * 0.075 + lateral * 0.055 + path.phase).sin() * 1.6
                + (lateral * 0.09 + path.phase * 1.7).sin() * 0.5;
            let mut roof_blend =
                super::biomes::smoothstep(8.0, 23.0, ground - bank_level + erosion);
            let fall_erosion = if self.config().river_terrain_revision > 0 && !path.brook {
                path.fall_erosion(index, core_t, lateral, width)
            } else {
                0.0
            };
            if fall_erosion > 0.0 {
                roof_blend *= 1.0 - fall_erosion;
            }
            let depth = if path.brook {
                0.8 + 0.5 * (0.5 + 0.5 * (core_t * 0.055 + path.phase).sin())
            } else {
                (1.15
                    + 2.3 * (0.5 + 0.5 * (core_t * 0.033 + path.phase).sin()).powi(2)
                    + 1.6 * (0.5 + 0.5 * (core_t * 0.071 + path.phase * 0.4).sin()))
                    * branch_depth
            };
            RiverSample {
                level,
                distance,
                width,
                flow: [(flow_dx / length) as f32, (1.0 / length) as f32],
                depth,
                covered: roof_blend >= 1.0,
                roof_blend,
                roof_detail: (core_t * 0.11 + lateral * 0.31 + path.phase).sin() * 0.9
                    + (core_t * 0.27 - lateral * 0.53).sin() * 0.35,
                bed_detail: (core_t * 0.21 + lateral * 0.75 + path.phase).sin() * 0.24,
                island_height,
                bank_reach: (SPACING * 0.5 - (center_z - cell[1] as f64 * SPACING).abs() - width)
                    .clamp(1.0, bank_reach),
                bank_level,
                bank_erosion: (core_t * 0.13 + lateral * 0.23 + path.phase).sin() * 0.9
                    + (core_t * 0.047 - lateral * 0.11 + path.phase * 0.6).sin() * 1.3,
                cascade_height: path.cascade_height(index, core_t, level, lateral),
                fall_erosion,
            }
        })
    }

    fn river_enabled(&self, cell: [i64; 2]) -> bool {
        let x = cell[0] as f64 * SPACING + 64.0;
        let z = cell[1] as f64 * SPACING;
        let desert = [-160.0, 0.0, 160.0]
            .into_iter()
            .map(|offset| self.climate_biome_at(x, z + offset).aridity())
            .sum::<f64>()
            / 3.0;
        if desert < 0.40 {
            return true;
        }
        let seed = layer_seed(
            self.config().seed.0
                ^ (cell[0] as u64).wrapping_mul(0x9e3779b97f4a7c15)
                ^ (cell[1] as u64).wrapping_mul(0xbf58476d1ce4e5b9),
            97,
        );
        // Select whole drainage paths, so rarity never cuts a wet reach at a biome edge.
        seed.is_multiple_of(7)
    }

    fn drainage_enabled(&self, cell: [i64; 2], brook: bool) -> bool {
        if brook && !self.config().biomes {
            return false;
        }
        if !brook {
            return self.river_enabled(cell);
        }
        // Select whole courses from the same climate field as the trees. Never
        // clip a flowing reach at an individual voxel/biome boundary.
        [-128.0, 0.0, 128.0].into_iter().any(|dz| {
            let biome = self.climate_biome_at(
                cell[0] as f64 * SPACING - 80.0,
                cell[1] as f64 * SPACING + dz,
            );
            let w = biome.weights;
            w.forest + w.cherry_forest + w.highland > 0.45 && biome.aridity() < 0.35
        })
    }

    #[cfg(test)]
    fn build_river(&self, cell: [i64; 2]) -> RiverPath {
        self.build_drainage(cell, false)
    }

    fn build_drainage(&self, cell: [i64; 2], brook: bool) -> RiverPath {
        let seed = layer_seed(
            self.config().seed.0
                ^ (cell[0] as u64).wrapping_mul(0x9e3779b97f4a7c15)
                ^ (cell[1] as u64).wrapping_mul(0xbf58476d1ce4e5b9),
            71,
        );
        let phase = f64::from(seed % 1024) / 1024.0 * std::f64::consts::TAU;
        let offset = if brook {
            -100.0 + f64::from(seed % 40)
        } else {
            40.0 + f64::from(seed % 45)
        };
        let branch = (!brook && seed.is_multiple_of(3)).then(|| {
            let start = f64::from(6 + ((seed >> 4) % 16)) * STEP;
            RiverBranch {
                start,
                end: start + f64::from(10 + ((seed >> 9) % 5)) * STEP,
                offset: (16.0 + f64::from((seed >> 14) % 7))
                    * if seed & (1 << 20) == 0 { -1.0 } else { 1.0 },
            }
        });
        let mut previous = f64::INFINITY;
        let mut points = std::array::from_fn(|index| {
            let z = cell[1] as f64 * SPACING + START + index as f64 * STEP;
            let x = cell[0] as f64 * SPACING
                + offset
                + (index as f64 * STEP * 0.014 + phase).sin() * 22.0
                + (index as f64 * STEP * 0.032 + phase * 0.7).sin() * 8.0;
            let end = index == 0 || index == POINTS - 1;
            let longitudinal = if end && !brook {
                SUPPORT_REACH
            } else {
                STEP as i32 / 2
            };
            let mut spill = f64::INFINITY;
            let support = if brook {
                4
            } else if branch.is_some_and(|branch| {
                (branch.start - STEP..=branch.end + STEP).contains(&(index as f64 * STEP))
            }) {
                32
            } else {
                SUPPORT_REACH
            };
            for dz in (-longitudinal..=longitudinal).step_by(4) {
                for dx in (-support..=support).step_by(4) {
                    let sx = x + f64::from(dx);
                    let sz = z + f64::from(dz);
                    spill = spill.min(self.base_height_with_biome(
                        sx,
                        sz,
                        self.climate_biome_at(sx, sz),
                    ));
                }
            }
            // A centre-line sample alone can put the river above a lower bank.
            // Respect the surrounding spill elevation, including rounded end caps.
            // The margin covers voxel quantization and the space between samples.
            // Retain fractional heights: small descents become continuous flowing reaches.
            previous =
                previous.min((spill - if brook { 0.8 } else { 1.0 }).max(self.config().sea_level));
            [x, previous, z]
        });
        let ground = points
            .map(|p| self.base_height_with_biome(p[0], p[2], self.climate_biome_at(p[0], p[2])));
        // A substantial natural descent can drain through one rock step and a
        // covered lower reach. Only lower the downstream level, never perch water
        // above its original bank support. Limit this to one <=44m cataract per path.
        for index in 0..POINTS - 1 {
            let outlet = points[(index + 8).min(POINTS - 1)][1];
            if !brook && (30.0..=44.0).contains(&(points[index][1] - outlet)) {
                for point in &mut points[index + 1..] {
                    point[1] = point[1].min(outlet);
                }
                break;
            }
        }
        let extensions = std::array::from_fn(|end| {
            let index = if end == 0 { 0 } else { POINTS - 1 };
            if brook || ground[index] - points[index][1] < 14.0 {
                return 0.0;
            }
            let direction = if end == 0 { -1.0 } else { 1.0 };
            let other = if end == 0 { 1 } else { POINTS - 2 };
            let dx = (points[index][0] - points[other][0]) / STEP;
            let mut extent = 0.0;
            for step in 1..=5 {
                let z = points[index][2] + direction * f64::from(step) * STEP;
                let x = points[index][0] + dx * f64::from(step) * STEP;
                let roof = self.base_height_with_biome(x, z, self.climate_biome_at(x, z));
                let supported = [-16.0, 0.0, 16.0].into_iter().all(|offset| {
                    self.base_height_with_biome(x + offset, z, self.climate_biome_at(x + offset, z))
                        > points[index][1] + 1.0
                });
                if roof < points[index][1] + 10.0 || !supported {
                    break;
                }
                extent = f64::from(step) * STEP;
            }
            extent
        });
        RiverPath {
            points,
            ground,
            extensions,
            width: if brook {
                1.15 + f64::from(seed % 5) * 0.14
            } else {
                4.0 + f64::from(seed % 4)
            },
            phase,
            branch,
            brook,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_fork_islands_are_low_banks_instead_of_mountain_pillars() {
        let terrain = river_terrain();
        let mut checked = 0;
        for cz in -8..=8 {
            for cx in -8..=8 {
                if !terrain.river_enabled([cx, cz]) {
                    continue;
                }
                let path = terrain.build_river([cx, cz]);
                let Some(branch) = path.branch else { continue };
                let t = (branch.start + branch.end) * 0.5;
                let Some([offset, _]) = branch.sample(t) else {
                    continue;
                };
                let index = (t / STEP).floor() as usize;
                let a = path.points[index];
                let b = path.points[(index + 1).min(POINTS - 1)];
                let x = a[0] + (b[0] - a[0]) * (t / STEP).fract() + offset * 0.5;
                let z = path.points[0][2] + t;
                let Some(river) = terrain.river_at(x, z) else {
                    continue;
                };
                if river.roof_blend == 0.0 && river.distance > river.width {
                    assert!(terrain.surface_height(x, z) <= river.level + 2.5 + 1e-9);
                    checked += 1;
                }
            }
        }
        assert!(
            checked > 3,
            "actual open fork islands must be sampled: {checked}"
        );
    }

    #[test]
    fn widened_river_banks_join_unchanged_terrain_at_drainage_cell_edges() {
        let terrain = river_terrain();
        for cz in -3..=3 {
            for cx in -3..=3 {
                for offset in [-80.0, 0.0, 80.0, 160.0] {
                    let x = cx as f64 * SPACING + offset;
                    let z = cz as f64 * SPACING + SPACING * 0.5;
                    let a = terrain.surface_height(x, z - 1e-5);
                    let b = terrain.surface_height(x, z + 1e-5);
                    assert!((a - b).abs() < 0.001, "river cell edge {x},{z}: {a}, {b}");
                }
            }
        }
    }
    use terra_core::WorldConfig;

    fn river_terrain() -> TerraGenerator {
        TerraGenerator::new(WorldConfig {
            rivers: true,
            caves: false,
            trees: false,
            ..Default::default()
        })
        .unwrap()
    }

    fn centre_at(path: &RiverPath, t: f64) -> [f64; 2] {
        let index = (t / STEP).floor() as usize;
        let next = (index + 1).min(POINTS - 1);
        let along = (t / STEP).fract();
        [
            path.points[index][0] + (path.points[next][0] - path.points[index][0]) * along,
            path.points[0][2] + t,
        ]
    }

    #[test]
    fn bed_depth_and_width_form_pools_and_riffles_without_uphill_water() {
        let terrain = river_terrain();
        for cell in [[0, 0], [-1, -1], [2, -3]] {
            let path = terrain.build_river(cell);
            let mut depths = [f64::INFINITY, f64::NEG_INFINITY];
            let mut widths = depths;
            let mut previous = f64::INFINITY;
            for t in 0..=(POINTS - 1) * STEP as usize {
                let [x, z] = centre_at(&path, t as f64);
                let river = terrain.river_at(x, z).unwrap();
                let water = terrain.water_column(x, z).unwrap();
                assert!(water.bed < water.level);
                assert!(river.level <= previous + 1e-9);
                previous = river.level;
                depths[0] = depths[0].min(river.depth);
                depths[1] = depths[1].max(river.depth);
                widths[0] = widths[0].min(river.width);
                widths[1] = widths[1].max(river.width);
            }
            assert!(depths[1] - depths[0] > 2.0, "no varied bed in {cell:?}");
            assert!(widths[1] - widths[0] > 2.0, "no varied width in {cell:?}");
        }
    }

    #[test]
    fn side_channels_split_and_rejoin_with_a_real_dry_island() {
        let terrain = river_terrain();
        let replay = river_terrain();
        let mut found = 0;
        let mut negative = false;
        for cell in (-3..=3).flat_map(|z| (-3..=3).map(move |x| [x, z])) {
            let path = terrain.build_river(cell);
            let Some(branch) = path.branch else {
                continue;
            };
            for t in [branch.start, branch.end] {
                let [offset, derivative] = branch.sample(t).unwrap();
                assert!(offset.abs() < 1e-10 && derivative.abs() < 1e-10);
            }
            let t = (branch.start + branch.end) * 0.5;
            let [x, z] = centre_at(&path, t);
            let [offset, _] = branch.sample(t).unwrap();
            let main = terrain.water_column(x, z).unwrap();
            let side = terrain.water_column(x + offset, z).unwrap();
            assert_eq!(main.level, side.level);
            assert_eq!(
                terrain.river_at(x + offset, z),
                replay.river_at(x + offset, z)
            );
            if terrain.water_column(x + offset * 0.5, z).is_some() {
                continue;
            }
            // Both complete paths remain wet, not merely two disconnected pools.
            for step in branch.start as i64..=branch.end as i64 {
                let [x, z] = centre_at(&path, step as f64);
                let [offset, _] = branch.sample(step as f64).unwrap();
                assert!(terrain.water_column(x, z).is_some());
                assert!(terrain.water_column(x + offset, z).is_some());
            }
            eprintln!(
                "river bifurcation fixture: main=({x:.1},{z:.1}) side=({:.1},{z:.1}) level={:.3}",
                x + offset,
                main.level
            );
            found += 1;
            negative |= cell[0] < 0 || cell[1] < 0;
            if found >= 3 && negative {
                break;
            }
        }
        assert!(found >= 3 && negative, "missing connected river islands");
    }

    #[test]
    fn portal_roofs_close_gradually_with_asymmetric_eroded_edges() {
        let terrain = river_terrain();
        let mut portals = 0;
        for cell in (-2..=2).flat_map(|z| (-2..=2).map(move |x| [x, z])) {
            let path = terrain.build_river(cell);
            for (index, pair) in path.points.windows(2).enumerate() {
                if pair[0][1] - pair[1][1] >= MIN_WATERFALL_DROP {
                    continue;
                }
                let mut transition = Vec::new();
                for step in 0..STEP as usize {
                    let t = index as f64 * STEP + step as f64;
                    let [x, z] = centre_at(&path, t);
                    let river = terrain.river_at(x, z).unwrap();
                    if !(0.05..0.95).contains(&river.roof_blend) {
                        continue;
                    }
                    let column = terrain.surface_column(x, z);
                    let raw = terrain.base_height_with_biome(x, z, terrain.climate_biome_at(x, z));
                    if raw <= river.level + 0.5 {
                        continue;
                    }
                    assert!(column.river_cavity.is_some());
                    assert!(column.height < raw);
                    let water = terrain.water_column(x, z).unwrap();
                    assert!(water.bed < water.level);
                    let left = terrain.river_at(x - 2.0, z).unwrap();
                    let right = terrain.river_at(x + 2.0, z).unwrap();
                    transition.push((x, z, (left.roof_blend - right.roof_blend).abs()));
                }
                if transition.len() >= 4 && transition.iter().any(|p| p.2 > 0.05) {
                    let (x, z, _) = transition[transition.len() / 2];
                    eprintln!("eroded river portal fixture: x={x:.1} z={z:.1}");
                    portals += 1;
                    break;
                }
            }
            if portals >= 3 {
                break;
            }
        }
        assert!(portals >= 3, "portal transitions must span several columns");
    }

    #[test]
    fn eroded_portal_roofs_remain_face_connected_to_their_banks() {
        use std::collections::VecDeque;

        for caves in [false, true] {
            let terrain = TerraGenerator::new(WorldConfig {
                rivers: true,
                caves,
                trees: false,
                ..Default::default()
            })
            .unwrap();
            let fixtures: Vec<_> = [
                [-481.5, -640.5], // Native portal screenshot.
                [551.5, -1112.5], // Eroded exterior/arch intersection island.
                [55.5, -1081.5],  // Cave noise cutting off the arch underside.
            ]
            .into_iter()
            .chain(
                (-2..=2)
                    .flat_map(|z| (-2..=2).map(move |x| [x, z]))
                    .filter_map(|cell| {
                        let path = terrain.build_river(cell);
                        (0..(POINTS - 1) * STEP as usize)
                            .map(|step| centre_at(&path, step as f64))
                            .find(|[x, z]| {
                                terrain.river_at(*x, *z).is_some_and(|river| {
                                    let height = terrain.surface_height(*x, *z);
                                    (0.15..0.95).contains(&river.roof_blend)
                                        && river.cavity().is_some_and(|c| height > c[1])
                                })
                            })
                    })
                    .take(8),
            )
            .collect();
            assert_eq!(fixtures.len(), 11);
            for [cx, cz] in fixtures {
                const SIDE: usize = 48;
                let origin = [cx.floor() as i64 - 24, cz.floor() as i64 - 24];
                let columns: Vec<_> = (0..SIDE)
                    .flat_map(|z| (0..SIDE).map(move |x| [x, z]))
                    .map(|[x, z]| {
                        terrain.surface_column(
                            (origin[0] + x as i64) as f64 + 0.5,
                            (origin[1] + z as i64) as f64 + 0.5,
                        )
                    })
                    .collect();
                let bottom = columns
                    .iter()
                    .map(|c| c.river_cavity.map_or(c.height, |v| v[0]).floor() as i64)
                    .min()
                    .unwrap()
                    - 3;
                let top = columns
                    .iter()
                    .map(|c| c.height.ceil() as i64)
                    .max()
                    .unwrap()
                    + 1;
                let height = (top - bottom + 1) as usize;
                let index = |x, y, z| (y * SIDE + z) * SIDE + x;
                let mut solid = vec![false; SIDE * SIDE * height];
                let mut connected = solid.clone();
                let mut queue = VecDeque::new();
                for y in 0..height {
                    for z in 0..SIDE {
                        for x in 0..SIDE {
                            let p = [
                                (origin[0] + x as i64) as f64 + 0.5,
                                (bottom + y as i64) as f64 + 0.5,
                                (origin[1] + z as i64) as f64 + 0.5,
                            ];
                            let id = index(x, y, z);
                            let column = columns[z * SIDE + x];
                            solid[id] = terrain.sample_with_column(p, column).density <= 0.0;
                            let ground = column
                                .river_cavity
                                .map_or(column.height, |cavity| column.height.min(cavity[0]));
                            // Only actual bed/bank terrain anchors a roof. A roof
                            // touching the sampled volume boundary proves nothing.
                            if solid[id] && p[1] <= ground {
                                connected[id] = true;
                                queue.push_back([x, y, z]);
                            }
                        }
                    }
                }
                while let Some([x, y, z]) = queue.pop_front() {
                    for [dx, dy, dz] in [
                        [-1, 0, 0],
                        [1, 0, 0],
                        [0, -1, 0],
                        [0, 1, 0],
                        [0, 0, -1],
                        [0, 0, 1],
                    ] {
                        let Some(nx) = x.checked_add_signed(dx) else {
                            continue;
                        };
                        let Some(ny) = y.checked_add_signed(dy) else {
                            continue;
                        };
                        let Some(nz) = z.checked_add_signed(dz) else {
                            continue;
                        };
                        if nx >= SIDE || ny >= height || nz >= SIDE {
                            continue;
                        }
                        let id = index(nx, ny, nz);
                        if solid[id] && !connected[id] {
                            connected[id] = true;
                            queue.push_back([nx, ny, nz]);
                        }
                    }
                }
                let mut retained_roof_blocks = 0;
                for y in 0..height {
                    for z in 8..SIDE - 8 {
                        for x in 8..SIDE - 8 {
                            let column = columns[z * SIDE + x];
                            if column
                                .river_cavity
                                .is_some_and(|c| bottom as f64 + y as f64 + 0.5 >= c[1])
                            {
                                let id = index(x, y, z);
                                retained_roof_blocks += usize::from(solid[id]);
                                assert!(
                                    !solid[id] || connected[id],
                                    "floating river roof: caves={caves} position={:?} column={column:?}",
                                    [
                                        origin[0] + x as i64,
                                        bottom + y as i64,
                                        origin[1] + z as i64
                                    ]
                                );
                            }
                        }
                    }
                }
                assert!(
                    retained_roof_blocks >= 16,
                    "portal fixture must retain a real supported roof: caves={caves}, centre={cx},{cz}, blocks={retained_roof_blocks}"
                );
            }
        }
    }

    #[test]
    fn small_drops_slope_continuously_and_only_five_meter_cliffs_fall() {
        for drop in [0.5, 1.0, 4.999, 5.0, 8.0] {
            let path = RiverPath {
                points: std::array::from_fn(|i| {
                    [
                        0.0,
                        if i == 0 { 20.0 } else { 20.0 - drop },
                        i as f64 * STEP,
                    ]
                }),
                ground: [20.0; POINTS],
                extensions: [0.0; 2],
                width: 5.0,
                phase: 0.0,
                branch: None,
                brook: false,
            };
            if drop < MIN_WATERFALL_DROP {
                assert_eq!(path.level(0, 0.25), 20.0 - drop * 0.25);
                assert!((path.level(0, 0.5 - 0.00001) - path.level(0, 0.5)).abs() < 0.001);
            } else {
                assert_eq!(path.level(0, 0.5 - 0.00001), 20.0);
                assert_eq!(path.level(0, 0.5), 20.0 - drop);
            }
        }
    }

    #[test]
    fn hydraulic_falls_keep_their_full_height_while_exterior_banks_are_continuous() {
        let path = RiverPath {
            points: std::array::from_fn(|i| {
                [0.0, if i == 0 { 46.0 } else { 10.0 }, i as f64 * STEP]
            }),
            ground: [58.0; POINTS],
            extensions: [0.0; 2],
            width: 5.0,
            phase: 0.3,
            branch: None,
            brook: false,
        };
        let edge = STEP * 0.5;
        for lateral in [-45.0, -17.0, 0.0, 19.0, 43.0] {
            let before = path.bank_level(0, edge - 1e-6, path.level(0, 0.5 - 1e-6 / STEP), lateral);
            let after = path.bank_level(0, edge + 1e-6, path.level(0, 0.5 + 1e-6 / STEP), lateral);
            assert!(
                (before - after).abs() < 1e-4,
                "planar bank jump at lateral {lateral}: {before} -> {after}"
            );
        }
        assert_eq!(path.level(0, 0.499), 46.0);
        assert_eq!(path.level(0, 0.5), 10.0);
        assert_ne!(
            path.bank_level(0, edge, 10.0, -19.0),
            path.bank_level(0, edge, 10.0, 19.0)
        );
    }

    #[test]
    fn real_waterfalls_do_not_project_a_flat_cliff_across_the_entire_valley() {
        let terrain = river_terrain();
        let mut checked = 0;
        let mut negatives = 0;
        for cell in (-8..=8).flat_map(|z| (-8..=8).map(move |x| [x, z])) {
            if !terrain.river_enabled(cell) {
                continue;
            }
            let path = terrain.build_river(cell);
            for (index, pair) in path.points.windows(2).enumerate() {
                if pair[0][1] - pair[1][1] < MIN_WATERFALL_DROP {
                    continue;
                }
                let t = (index as f64 + 0.5) * STEP;
                let [center, z] = centre_at(&path, t);
                for offset in [-42.0, -28.0, 28.0, 42.0] {
                    let x = center + offset;
                    let Some(river) = terrain.river_at(x, z) else {
                        continue;
                    };
                    if river.distance < river.width + 12.0 || river.island_height.is_some() {
                        continue;
                    }
                    let before = terrain.surface_height(x, z - 1e-6);
                    let after = terrain.surface_height(x, z + 1e-6);
                    assert!(
                        (before - after).abs() < 1e-3,
                        "wide planar fall wall at {x},{z}: {before} -> {after}"
                    );
                    checked += 1;
                    negatives += usize::from(x < 0.0 || z < 0.0);
                }
            }
        }
        assert!(
            checked > 20 && negatives > 10,
            "must inspect actual positive and negative river cliffs: {checked}/{negatives}"
        );
    }

    #[test]
    fn cataract_roofs_taper_downstream_instead_of_ending_on_a_vertical_wall() {
        let path = RiverPath {
            points: std::array::from_fn(|i| {
                [0.0, if i == 0 { 46.0 } else { 10.0 }, i as f64 * STEP]
            }),
            ground: [58.0; POINTS],
            extensions: [0.0; 2],
            width: 5.0,
            phase: 0.0,
            branch: None,
            brook: false,
        };
        assert_eq!(path.cascade_height(0, 4.5, 10.0, 0.0), 36.0);
        let middle = path.cascade_height(2, 16.0, 10.0, 0.0);
        assert!(middle > 0.0 && middle < 36.0);
        assert_eq!(path.cascade_height(4, 32.0, 10.0, 0.0), 0.0);
        assert_ne!(
            path.cascade_height(2, 20.0, 10.0, -2.0),
            path.cascade_height(2, 20.0, 10.0, 2.0)
        );
    }

    #[test]
    fn river_valleys_only_carve_land_and_blend_back_into_the_original_terrain() {
        let terrain = TerraGenerator::new(WorldConfig {
            rivers: true,
            ..Default::default()
        })
        .unwrap();
        for cell in [[0, 0], [-1, -1]] {
            for point in terrain.build_river(cell).points.iter().step_by(4) {
                for offset in -48..=48 {
                    let x = point[0] + f64::from(offset);
                    let z = point[2];
                    let raw = terrain.base_height_with_biome(x, z, terrain.climate_biome_at(x, z));
                    assert!(
                        terrain.surface_height(x, z) <= raw + 1e-9,
                        "raised bank at {x}, {z}"
                    );
                    if terrain.river_at(x, z).is_none() {
                        assert_eq!(terrain.surface_height(x, z), raw);
                    }
                }
            }
        }
    }
    #[test]
    fn paths_are_deterministic_downhill_and_legacy_worlds_are_unchanged() {
        let legacy = TerraGenerator::new(WorldConfig::default()).unwrap();
        assert!(legacy.river_at(64.0, 24.0).is_none());
        let config = WorldConfig {
            rivers: true,
            ..Default::default()
        };
        let a = TerraGenerator::new(config.clone()).unwrap();
        let b = TerraGenerator::new(config).unwrap();
        let mut drops = 0;
        let mut largest_drop = 0.0_f64;
        for cell in (-8..=8).flat_map(|z| (-8..=8).map(move |x| [x, z])) {
            if !a.river_enabled(cell) {
                continue;
            }
            let path = a.build_river(cell);
            for pair in path.points.windows(2) {
                assert!(pair[1][1] <= pair[0][1]);
                drops += usize::from(pair[0][1] - pair[1][1] >= MIN_WATERFALL_DROP);
                largest_drop = largest_drop.max(pair[0][1] - pair[1][1]);
            }
            for point in path.points {
                let query = [point[0], point[2] + 0.01];
                let river = a.river_at(query[0], query[1]).unwrap();
                assert_eq!(Some(river), b.river_at(query[0], query[1]));
                assert!(a.water_column(query[0], query[1]).unwrap().bed < river.level);
                assert!(a.water_column(query[0], query[1]).is_some());
            }
        }
        assert!(
            drops > 0,
            "The default seed must contain actual waterfalls (largest drop {largest_drop})"
        );
    }

    #[test]
    fn desert_rarity_selects_whole_paths_deterministically() {
        let a = TerraGenerator::new(WorldConfig {
            rivers: true,
            ..Default::default()
        })
        .unwrap();
        let b = TerraGenerator::new(a.config().clone()).unwrap();
        let mut dry_paths = 0;
        let mut wet_paths = 0;
        for cell in (-24..=24).flat_map(|z| (-24..=24).map(move |x| [x, z])) {
            assert_eq!(a.river_enabled(cell), b.river_enabled(cell));
            let x = cell[0] as f64 * SPACING + 64.0;
            let z = cell[1] as f64 * SPACING;
            let desert = [-160.0, 0.0, 160.0]
                .into_iter()
                .map(|dz| a.biome_at(x, z + dz).aridity())
                .sum::<f64>()
                / 3.0;
            if desert < 0.4 {
                assert!(a.river_enabled(cell));
                continue;
            }
            dry_paths += 1;
            wet_paths += usize::from(a.river_enabled(cell));
            if !a.river_enabled(cell) {
                for point in a.build_river(cell).points.iter().step_by(8) {
                    assert!(a.river_at(point[0], point[2]).is_none());
                }
            }
        }
        assert!(dry_paths > 100);
        assert!((0.06..0.25).contains(&(wet_paths as f64 / dry_paths as f64)));
    }

    #[test]
    fn river_water_has_supported_banks_including_negative_and_terminal_edges() {
        for seed in [42042, 7, 98173] {
            let terrain = TerraGenerator::new(WorldConfig {
                seed: terra_core::WorldSeed(seed),
                rivers: true,
                ..Default::default()
            })
            .unwrap();
            for cell in [[0, 0], [-1, -1], [0, 1]] {
                let path = terrain.build_river(cell);
                for point in path.points.iter().step_by(3) {
                    for dz in [-31.5, -7.5, -0.5, 3.5, 31.5] {
                        for dx in (-48..=48).step_by(2) {
                            let x = (point[0] + f64::from(dx)).floor() + 0.5;
                            let z = (point[2] + dz).floor() + 0.5;
                            let Some(water) = terrain.water_column(x, z) else {
                                continue;
                            };
                            for [nx, nz] in [[x - 1.0, z], [x + 1.0, z], [x, z - 1.0], [x, z + 1.0]]
                            {
                                if terrain.water_column(nx, nz).is_none() {
                                    let bank = (terrain.surface_height(nx, nz) - 0.5).floor() + 1.0;
                                    assert!(
                                        bank >= water.level - 0.5,
                                        "unsupported shore seed={seed} at {x},{z}: level={} bank={bank}",
                                        water.level
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn covered_rivers_keep_solid_roofs_and_real_water_in_headless_and_render_sections() {
        let terrain = TerraGenerator::new(WorldConfig {
            rivers: true,
            caves: false,
            trees: false,
            ..Default::default()
        })
        .unwrap();
        let generator = crate::SectionGenerator::new(terrain.clone());
        let mut found = 0;
        for cell in (-3..=3).flat_map(|z| (-3..=3).map(move |x| [x, z])) {
            let path = terrain.build_river(cell);
            let candidate = (0..(POINTS - 1) * STEP as usize).step_by(2).find_map(|t| {
                let [x, z] = centre_at(&path, t as f64);
                let x = x.floor() as i64;
                let z = z.floor() as i64;
                let column = terrain.surface_column(x as f64 + 0.5, z as f64 + 0.5);
                let cavity = column.river_cavity?;
                let river = terrain.river_at(x as f64 + 0.5, z as f64 + 0.5)?;
                (river.covered && column.height > cavity[1] + 2.0).then_some((x, z, column, cavity))
            });
            let Some((x, z, column, [floor, ceiling])) = candidate else {
                continue;
            };
            found += 1;
            let sample = terrain.river_at(x as f64 + 0.5, z as f64 + 0.5).unwrap();
            let water = terrain
                .water_column(x as f64 + 0.5, z as f64 + 0.5)
                .unwrap();
            assert_eq!(water.bed, column.water_bed());
            assert_eq!(column.water_level, Some(water.level));
            assert_eq!(
                column.height,
                terrain.base_height_with_biome(
                    x as f64 + 0.5,
                    z as f64 + 0.5,
                    terrain.climate_biome_at(x as f64 + 0.5, z as f64 + 0.5)
                )
            );
            let inside = [x, sample.level.ceil() as i64 + 1, z];
            let roof = [x, ceiling.ceil() as i64 + 1, z];
            let ground = [x, floor.floor() as i64 - 1, z];
            assert_eq!(generator.block_at(inside).unwrap(), 0);
            assert_ne!(generator.block_at(roof).unwrap(), 0);
            assert_ne!(generator.block_at(ground).unwrap(), 0);
            let position = terra_voxel::global_to_section([x, water.level.floor() as i64, z]).0;
            let rendered = generator
                .build(position, 0, &Default::default(), true)
                .unwrap();
            let headless = generator
                .build(position, 0, &Default::default(), false)
                .unwrap();
            assert_eq!(rendered.data, headless.data);
            assert!(rendered.water.is_some());
            assert!(headless.water.is_none());
            assert_eq!(rendered.props, headless.props);
            let [low, high] = rendered.surface_sections.unwrap();
            assert!(low <= position.y && high >= roof[1].div_euclid(16));
            eprintln!(
                "river tunnel fixture: x={x} z={z} level={} roof={ceiling}",
                water.level
            );
            if found >= 3 {
                break;
            }
        }
        assert!(
            found >= 3,
            "fixtures must include covered rivers at multiple coordinates"
        );
    }

    #[test]
    fn buried_terminal_galleries_extend_the_wet_channel_without_cutting_the_roof() {
        let terrain = TerraGenerator::new(WorldConfig {
            rivers: true,
            caves: false,
            trees: false,
            ..Default::default()
        })
        .unwrap();
        let generator = crate::SectionGenerator::new(terrain.clone());
        let mut tested = 0;
        for cell in [[0, 0], [-1, -1], [-2, -1], [1, 1]] {
            let path = terrain.build_river(cell);
            for end in 0..2 {
                let extent = path.extensions[end];
                if extent < STEP {
                    continue;
                }
                let i = if end == 0 { 0 } else { POINTS - 1 };
                let other = if end == 0 { 1 } else { POINTS - 2 };
                let dx = (path.points[i][0] - path.points[other][0]) / STEP;
                let direction = if end == 0 { -1.0 } else { 1.0 };
                for step in 0..=extent as i64 {
                    let x = (path.points[i][0] + dx * step as f64).floor() as i64;
                    let z = (path.points[i][2] + direction * step as f64).floor() as i64;
                    let water = terrain
                        .water_column(x as f64 + 0.5, z as f64 + 0.5)
                        .unwrap();
                    assert!((water.level - path.points[i][1]).abs() < 1.0);
                    assert_eq!(
                        generator
                            .block_at([x, water.level.ceil() as i64 + 1, z])
                            .unwrap(),
                        0
                    );
                }
                tested += 1;
            }
        }
        assert!(
            tested > 0,
            "at least one covered terminal must continue into a wet gallery"
        );
        let legacy = TerraGenerator::new(WorldConfig::default()).unwrap();
        assert!(legacy.surface_column(64.5, 192.5).river_cavity.is_none());
    }

    #[test]
    fn generated_long_cataract_has_an_open_shaft_and_water_in_every_vertical_section() {
        let terrain = TerraGenerator::new(WorldConfig {
            rivers: true,
            trees: false,
            caves: false,
            ..Default::default()
        })
        .unwrap();
        let candidate = (-12..=12)
            .flat_map(|z| (-12..=12).map(move |x| [x, z]))
            .find_map(|cell| {
                terrain
                    .build_river(cell)
                    .points
                    .windows(2)
                    .find_map(|pair| {
                        (pair[0][1] - pair[1][1] >= 30.0).then_some((pair[0], pair[1]))
                    })
            });
        let (upper, lower) = candidate.expect("the default seed must produce a genuine >=30m fall");
        let x = ((upper[0] + lower[0]) * 0.5).floor() as i64;
        let z = (upper[2] + STEP * 0.5).floor() as i64;
        let generator = crate::SectionGenerator::new(terrain.clone());
        let downstream = terrain
            .water_column(x as f64 + 0.5, z as f64 + 0.5)
            .unwrap();
        let upstream = terrain
            .water_column(x as f64 + 0.5, z as f64 - 0.5)
            .unwrap();
        assert!(upstream.level - downstream.level >= 30.0);
        for y in downstream.level.ceil() as i64..=upstream.level.floor() as i64 + 1 {
            assert_eq!(
                generator.block_at([x, y, z]).unwrap(),
                0,
                "blocked fall shaft at {x},{y},{z}"
            );
        }
        let low = (downstream.level.floor() as i64).div_euclid(16);
        let high = (upstream.level.floor() as i64).div_euclid(16);
        let mut wakes = Vec::new();
        #[cfg(feature = "bevy")]
        let mut chunks = crate::streaming::ChunkStore::default();
        for dz in -1..=1 {
            for dx in -1..=1 {
                for sy in low..=high {
                    let owner = terra_voxel::SectionPos::new(
                        x.div_euclid(16) + dx,
                        sy,
                        z.div_euclid(16) + dz,
                    );
                    let section = generator
                        .build(owner, 0, &Default::default(), true)
                        .unwrap();
                    wakes.extend(section.fluid_wake.iter().copied());
                    if let Some(water) = &section.water {
                        assert!(
                            water.surface_effects.iter().all(|e| e[3] != 1.0),
                            "reservoirs must never fabricate a falling sheet"
                        );
                    }
                    #[cfg(feature = "bevy")]
                    chunks.loaded.insert(
                        owner,
                        crate::streaming::LoadedSection {
                            data: section.data,
                            mesh: None,
                            mesh_epoch: 0,
                            water_epoch: 0,
                            water: section.water,
                            props: section.props,
                        },
                    );
                }
            }
        }
        let outlet = [x, upstream.level.ceil() as i64 - 1, z];
        let outlet_owner = terra_voxel::global_to_section([x, outlet[1], z - 1]).0;
        assert_eq!(
            generator
                .build(outlet_owner, 0, &Default::default(), true)
                .unwrap()
                .fluid_wake,
            generator
                .build(outlet_owner, 0, &Default::default(), false)
                .unwrap()
                .fluid_wake,
            "headless and meshed generation must activate identical source lips"
        );
        assert!(
            wakes.contains(&outlet),
            "a real open source lip must seed the gravity scheduler: {outlet:?}"
        );
        #[cfg(feature = "bevy")]
        {
            let mut journal = crate::edits::EditJournal::default();
            let middle = [
                x as f64 + 0.5,
                (upstream.level + downstream.level) * 0.5,
                z as f64 + 0.5,
            ];
            assert_eq!(
                crate::water::fluid_motion_at(&terrain, &chunks, &journal, middle),
                crate::water::FluidMotion::default(),
                "dry air has no projected waterfall force"
            );
            for p in wakes {
                journal.wake_fluid_neighbors(p);
            }
            let shaft = downstream.level.ceil() as i64..upstream.level.floor() as i64;
            for _ in 0..240 {
                crate::water::simulate_fluid_step(&terrain, &chunks, &mut journal, 1024).unwrap();
                if shaft
                    .clone()
                    .all(|y| journal.fluid([x, y, z]).is_some_and(|c| c.amount > 0))
                {
                    break;
                }
            }
            assert!(
                shaft
                    .clone()
                    .all(|y| journal.fluid([x, y, z]).is_some_and(|c| c.amount > 0)),
                "all vertical sections must contain real persistent nonsource flow"
            );
            let motion = crate::water::fluid_motion_at(&terrain, &chunks, &journal, middle);
            assert!(motion.immersion > 0.0 && motion.velocity[1] < 0.0);
            let mut restored = crate::edits::EditJournal::default();
            for owner in journal.changed_sections() {
                restored.import_delta(journal.section_delta(owner)).unwrap();
            }
            for y in shaft {
                assert_eq!(restored.fluid([x, y, z]), journal.fluid([x, y, z]));
            }
        }
    }
}
