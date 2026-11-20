//! Bounded revision-five drainage atop the existing cached catchment course.
use super::{DrainageFrame, POINTS, RiverPath, TerraGenerator};
use crate::water::MIN_WATERFALL_DROP;
use std::ops::Deref;

const GRID: usize = 7;
const CELLS: usize = GRID * GRID;
const CELL_SPACING: f64 = (2048.0 - 352.0 * 2.0) / 6.0;
const MAX_TRIBUTARIES: usize = 3;

pub(super) struct RiverNetwork {
    pub main: RiverPath,
    pub tributaries: Vec<Tributary>,
    pub distributaries: Vec<Distributary>,
}

impl Deref for RiverNetwork {
    type Target = RiverPath;
    fn deref(&self) -> &Self::Target {
        &self.main
    }
}

pub(super) struct Tributary {
    pub path: RiverPath,
    pub runoff: f64,
}

pub(super) struct Distributary {
    pub path: RiverPath,
    pub split_index: usize,
    pub reaches_sea: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Receiver {
    Cell(usize),
    Trunk(usize),
}

struct Drainage {
    receiver: [Option<Receiver>; CELLS],
    accumulated: [f64; CELLS],
}

impl Drainage {
    fn build(
        heights: [f64; CELLS],
        rainfall: [f64; CELLS],
        outlets: [Option<(usize, f64)>; CELLS],
    ) -> Self {
        let receiver = std::array::from_fn(|index| {
            let mut best = outlets[index].map(|(trunk, slope)| (Receiver::Trunk(trunk), slope));
            let [x, z] = [index % GRID, index / GRID];
            for dz in -1..=1 {
                for dx in -1..=1 {
                    let [nx, nz] = [x as i32 + dx, z as i32 + dz];
                    if (dx == 0 && dz == 0)
                        || !(0..GRID as i32).contains(&nx)
                        || !(0..GRID as i32).contains(&nz)
                    {
                        continue;
                    }
                    let next = nx as usize + nz as usize * GRID;
                    let drop = heights[index] - heights[next];
                    if drop <= 1e-7 {
                        continue;
                    }
                    let slope = drop / (f64::from(dx * dx + dz * dz).sqrt() * CELL_SPACING);
                    if best.is_none_or(|(_, old)| slope > old) {
                        best = Some((Receiver::Cell(next), slope));
                    }
                }
            }
            best.map(|(receiver, _)| receiver)
        });
        let mut order: [usize; CELLS] = std::array::from_fn(|i| i);
        order.sort_unstable_by(|&a, &b| heights[b].total_cmp(&heights[a]).then(a.cmp(&b)));
        let mut accumulated = rainfall;
        for index in order {
            if let Some(Receiver::Cell(next)) = receiver[index] {
                accumulated[next] += accumulated[index];
            }
        }
        Self {
            receiver,
            accumulated,
        }
    }

    fn route(&self, start: usize) -> Option<(Vec<usize>, usize)> {
        let mut route = Vec::new();
        let mut current = start;
        for _ in 0..CELLS {
            route.push(current);
            match self.receiver[current]? {
                Receiver::Cell(next) => current = next,
                Receiver::Trunk(trunk) => return Some((route, trunk)),
            }
        }
        None
    }
}

impl TerraGenerator {
    pub(super) fn regional_network(
        &self,
        main: RiverPath,
        probes: &[([f64; 2], f64); CELLS],
    ) -> RiverNetwork {
        let mut network = RiverNetwork {
            main,
            tributaries: Vec::new(),
            distributaries: Vec::new(),
        };
        if (self.config().landscape_revision < 5 && self.config().river_terrain_revision < 3)
            || !self.config().rivers
        {
            self.regional_distributaries(&mut network, probes);
            return network;
        }
        let main = &network.main;
        let outlets = std::array::from_fn(|index| {
            let (point, height) = probes[index];
            (6..POINTS - 6)
                .filter(|&trunk| {
                    ((trunk - 3)..=(trunk + 3))
                        .all(|i| main.points[i][1] - main.points[i + 1][1] < MIN_WATERFALL_DROP)
                })
                .filter_map(|trunk| {
                    let target = main
                        .frame
                        .world(main.points[trunk][0], main.points[trunk][2]);
                    let distance = (point[0] - target[0]).hypot(point[1] - target[1]);
                    let drop = height - main.points[trunk][1];
                    (distance <= 340.0 && drop > 1.0).then_some((trunk, drop / distance.max(1.0)))
                })
                .max_by(|a, b| a.1.total_cmp(&b.1).then(b.0.cmp(&a.0)))
        });
        let rainfall = std::array::from_fn(|i| {
            let point = probes[i].0;
            let humidity = self.climate_biome_at(point[0], point[1]).humidity;
            CELL_SPACING.powi(2) * (0.35 + humidity.clamp(0.0, 1.0) * 0.65)
        });
        let drainage = Drainage::build(probes.map(|probe| probe.1), rainfall, outlets);
        let mut candidates: Vec<_> = (0..CELLS)
            .filter_map(|source| {
                let (route, trunk) = drainage.route(source)?;
                let mouth = main
                    .frame
                    .world(main.points[trunk][0], main.points[trunk][2]);
                let length = (probes[source].0[0] - mouth[0]).hypot(probes[source].0[1] - mouth[1]);
                let runoff = drainage.accumulated[source];
                (length >= 240.0 && runoff >= CELL_SPACING.powi(2) * 0.55).then_some((
                    length * runoff.sqrt(),
                    source,
                    route,
                    trunk,
                    runoff,
                ))
            })
            .collect();
        candidates.sort_unstable_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(&b.1)));
        for (_, _, route, trunk, runoff) in candidates.into_iter().take(10) {
            let Some(path) = self.tributary_path(main, probes, &route, trunk, runoff) else {
                continue;
            };
            if network
                .tributaries
                .iter()
                .any(|other| paths_touch(&path, &other.path))
            {
                continue;
            }
            network.tributaries.push(Tributary { path, runoff });
            if network.tributaries.len() == MAX_TRIBUTARIES {
                break;
            }
        }
        self.regional_distributaries(&mut network, probes);
        network
    }

    fn tributary_path(
        &self,
        main: &RiverPath,
        probes: &[([f64; 2], f64); CELLS],
        route: &[usize],
        trunk: usize,
        runoff: f64,
    ) -> Option<RiverPath> {
        let source = probes[*route.first()?].0;
        let mouth = main
            .frame
            .world(main.points[trunk][0], main.points[trunk][2]);
        let length = (mouth[0] - source[0]).hypot(mouth[1] - source[1]);
        let frame = DrainageFrame {
            origin: source,
            downstream: [
                (mouth[0] - source[0]) / length,
                (mouth[1] - source[1]) / length,
            ],
        };
        let mut anchors = Vec::with_capacity(route.len() + 1);
        for &node in route {
            let [cross, along] = frame.project(probes[node].0[0], probes[node].0[1]);
            if anchors
                .last()
                .is_some_and(|last: &[f64; 2]| along <= last[1] + 0.1)
            {
                return None;
            }
            if !(0.0..length).contains(&along) {
                if anchors.is_empty() && along.abs() < 1e-6 {
                    anchors.push([cross, 0.0]);
                    continue;
                }
                return None;
            }
            anchors.push([cross, along]);
        }
        anchors.push([0.0, length]);
        if anchors
            .windows(2)
            .any(|pair| ((pair[1][0] - pair[0][0]) / (pair[1][1] - pair[0][1])).abs() > 4.0)
        {
            return None;
        }
        let step = length / (POINTS - 1) as f64;
        let mut points = std::array::from_fn(|i| {
            let along = i as f64 * step;
            let segment = anchors
                .windows(2)
                .find(|pair| along <= pair[1][1])
                .unwrap_or(&anchors[anchors.len() - 2..]);
            let t = ((along - segment[0][1]) / (segment[1][1] - segment[0][1])).clamp(0.0, 1.0);
            [
                segment[0][0] + (segment[1][0] - segment[0][0]) * t,
                0.0,
                along,
            ]
        });
        let join = self.sample_drainage(main, mouth[0], mouth[1], None)?;
        let mut ground = [0.0; POINTS];
        let mut caps = [0.0; POINTS];
        for i in 0..POINTS {
            let world = frame.world(points[i][0], points[i][2]);
            ground[i] = self.base_height_with_biome(
                world[0],
                world[1],
                self.climate_biome_at(world[0], world[1]),
            );
            let mut support = ground[i];
            let a = points[i.saturating_sub(1)];
            let b = points[(i + 1).min(POINTS - 1)];
            let slope = (b[0] - a[0]) / (b[2] - a[2]);
            let normal = 1.0 / slope.hypot(1.0);
            let reach = if self.config().river_terrain_revision >= 5 {
                12.0
            } else {
                10.0
            };
            for offset in [-reach, -reach * 0.5, reach * 0.5, reach] {
                let [x, z] = frame.world(
                    points[i][0] + offset * normal,
                    points[i][2] - offset * slope * normal,
                );
                support =
                    support.min(self.base_height_with_biome(x, z, self.climate_biome_at(x, z)));
            }
            caps[i] = support - 1.6;
            if i < POINTS - 5 && caps[i] < join.level {
                return None;
            }
        }
        let source_level = caps[0];
        if source_level < join.level + 2.0 {
            return None;
        }
        let mut previous = source_level;
        for i in 0..POINTS {
            let t = i as f64 / (POINTS - 1) as f64;
            let grade = 1.0 - crate::generation::biomes::smoothstep(0.0, 0.80, t);
            previous = previous
                .min(caps[i].max(join.level))
                .min(join.level + (source_level - join.level) * grade);
            points[i][1] = previous;
        }
        Some(RiverPath {
            points,
            ground,
            extensions: [0.0; 2],
            width: (1.0 + (runoff / CELL_SPACING.powi(2)).sqrt() * 0.35).min(3.2),
            phase: main.phase + trunk as f64 * 0.73,
            branch: None,
            brook: true,
            step,
            frame,
            lake: main.lake,
        })
    }

    pub(super) fn sample_tributary(
        &self,
        path: &RiverPath,
        x: f64,
        z: f64,
    ) -> Option<super::RiverSample> {
        let mut sample = self.sample_drainage(path, x, z, None)?;
        if self.config().river_terrain_revision >= 5 {
            return Some(sample);
        }
        let along = path.frame.project(x, z)[1];
        let end_distance = along - along.clamp(0.0, (POINTS - 1) as f64 * path.step);
        let normal = (f64::from(sample.flow[0]) * path.frame.downstream[0]
            + f64::from(sample.flow[1]) * path.frame.downstream[1])
            .abs()
            .clamp(0.0, 1.0);
        // DrainageFrame projects onto the overall course, not its local tangent.
        // Correct only lateral distance; end caps retain their longitudinal reach.
        let lateral_squared = (sample.distance.powi(2) - end_distance.powi(2)).max(0.0);
        sample.distance = (lateral_squared * normal.powi(2) + end_distance.powi(2)).sqrt();
        // A diagonal centreline must include its nearest voxel centre and leave
        // at least half a metre of water after the shared rounded-bed conversion.
        sample.width = sample.width.max(1.5);
        sample.depth = sample.depth.max(1.35);
        Some(sample)
    }
}

fn paths_touch(a: &RiverPath, b: &RiverPath) -> bool {
    a.points.iter().any(|p| {
        let p = a.frame.world(p[0], p[2]);
        b.points.iter().any(|q| {
            let q = b.frame.world(q[0], q[2]);
            (p[0] - q[0]).hypot(p[1] - q[1]) < 32.0
        })
    })
}

#[cfg(test)]
#[path = "network_tests.rs"]
mod tests;
