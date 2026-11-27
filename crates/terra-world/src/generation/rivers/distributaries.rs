//! Distributaries belong to the cached catchment, not to individual chunks.
use super::{DrainageFrame, POINTS, RiverPath, TerraGenerator, network::*};
use crate::generation::{biomes::smoothstep, oases::BANK_REACH};

const MAX_DISTRIBUTARIES: usize = 2;
const MAX_CANDIDATES: usize = 8;
const MIN_LAND_REACH: f64 = 128.0;

#[derive(Clone, Copy)]
struct Outlet {
    point: [f64; 2],
    level: f64,
    sea: bool,
}

impl TerraGenerator {
    pub(super) fn regional_distributaries(
        &self,
        network: &mut RiverNetwork,
        probes: &[([f64; 2], f64); 49],
    ) {
        if self.config().river_terrain_revision < 2 || !self.config().rivers {
            return;
        }
        let main = &network.main;
        let trunk_mouth = world(main, POINTS - 1);
        let mut outlets: Vec<_> = probes
            .iter()
            .filter(|(point, height)| {
                *height < self.config().sea_level - 1.0
                    && distance(*point, trunk_mouth) >= 180.0
                    && self.climate_biome_at(point[0], point[1]).weights.ocean >= 0.35
            })
            .map(|&(point, _)| Outlet {
                point,
                level: self.config().sea_level,
                sea: true,
            })
            .collect();
        if let Some(lake) = main.lake {
            // Different shores of the existing validated reservoir are real
            // destinations; an offshoot never rejoins an arbitrary trunk point.
            for direction in [[-1.0, 0.0], [1.0, 0.0], [0.0, -1.0], [0.0, 1.0]] {
                let point = [
                    lake.center[0] + direction[0] * lake.radius * 0.55,
                    lake.center[1] + direction[1] * lake.radius * 0.55,
                ];
                if distance(point, trunk_mouth) >= 24.0
                    && lake.sample(point[0], point[1]).distance < lake.radius * 0.8
                {
                    outlets.push(Outlet {
                        point,
                        level: lake.level,
                        sea: false,
                    });
                }
            }
        }
        let mut candidates = Vec::new();
        for split in (4..=36).step_by(4) {
            let source = world(main, split);
            if main.ground[split] <= self.config().sea_level + 2.0
                || self.exposed_land_reach(main, split, None).0 < MIN_LAND_REACH
            {
                continue;
            }
            let source_along = main.points[split][2];
            for (outlet_index, outlet) in outlets.iter().enumerate() {
                let [cross, along] = main.frame.project(outlet.point[0], outlet.point[1]);
                let separation = cross - main.points[split][0];
                let length = distance(source, outlet.point);
                if along < source_along + 128.0
                    || !(240.0..=1700.0).contains(&length)
                    || main.points[split][1] < outlet.level
                    || (outlet.sea && separation.abs() < 128.0)
                {
                    continue;
                }
                let check = [
                    source[0] + (outlet.point[0] - source[0]) * MIN_LAND_REACH / length,
                    source[1] + (outlet.point[1] - source[1]) * MIN_LAND_REACH / length,
                ];
                if self.watershed_height(check[0], check[1]) <= self.config().sea_level + 1.0 {
                    continue;
                }
                // Prefer clear forks over a second almost-parallel centreline.
                let score = separation.abs() / length + (1.0 - length / 2400.0) * 0.15;
                candidates.push((score, split, outlet_index));
            }
        }
        candidates
            .sort_unstable_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(&b.1)).then(a.2.cmp(&b.2)));
        for (_, split, outlet_index) in candidates.into_iter().take(MAX_CANDIDATES) {
            let outlet = outlets[outlet_index];
            if network
                .distributaries
                .iter()
                .any(|other| distance(world(&other.path, POINTS - 1), outlet.point) < 32.0)
            {
                continue;
            }
            let Some(path) = self.distributary_path(&network.main, split, outlet) else {
                continue;
            };
            let (land_reach, separate_land_channel) =
                self.exposed_land_reach(&path, 0, Some(&network.main));
            if land_reach < MIN_LAND_REACH || !separate_land_channel {
                continue;
            }
            if network
                .tributaries
                .iter()
                .any(|tributary| crosses_course(&path, &tributary.path, false))
                || network
                    .distributaries
                    .iter()
                    .any(|other| crosses_course(&path, &other.path, true))
                || (outlet.sea && !self.connected_ocean_outlet(outlet.point))
            {
                continue;
            }
            network.distributaries.push(Distributary {
                path,
                split_index: split,
                reaches_sea: outlet.sea,
            });
            if network.distributaries.len() == MAX_DISTRIBUTARIES {
                break;
            }
        }
    }

    fn exposed_land_reach(
        &self,
        path: &RiverPath,
        from: usize,
        other: Option<&RiverPath>,
    ) -> (f64, bool) {
        let start = from as f64 * path.step;
        let end = (POINTS - 1) as f64 * path.step;
        let mut length = 0.0;
        let mut previous = world(path, from);
        let mut separated = other.is_none();
        for step in 0..=((end - start) / 16.0).ceil() as usize {
            let along = (start + step as f64 * 16.0).min(end);
            let at = along / path.step;
            let a = (at.floor() as usize).min(POINTS - 1);
            let b = (a + 1).min(POINTS - 1);
            let point = path.frame.world(
                path.points[a][0] + (path.points[b][0] - path.points[a][0]) * at.fract(),
                along,
            );
            let ground = self.watershed_height(point[0], point[1]);
            let Some(sample) = self.sample_drainage(path, point[0], point[1], None) else {
                break;
            };
            if ground <= self.config().sea_level + 1.0
                || sample.covered
                || sample.roof_blend > 0.05
                || path.lake.is_some_and(|lake| {
                    lake.sample(point[0], point[1]).distance <= lake.radius + BANK_REACH
                })
            {
                break;
            }
            length += distance(previous, point);
            previous = point;
            separated |= other.is_some_and(|other| {
                (0..POINTS - 1).all(|i| {
                    segment_distance(point, world(other, i), world(other, i + 1))
                        > path.width + other.width + 12.0
                })
            });
        }
        (length, separated)
    }

    fn distributary_path(
        &self,
        main: &RiverPath,
        split: usize,
        outlet: Outlet,
    ) -> Option<RiverPath> {
        let source = world(main, split);
        let length = distance(source, outlet.point);
        let frame = DrainageFrame {
            origin: source,
            downstream: [
                (outlet.point[0] - source[0]) / length,
                (outlet.point[1] - source[1]) / length,
            ],
        };
        let step = length / (POINTS - 1) as f64;
        let side = (main.frame.project(outlet.point[0], outlet.point[1])[0]
            - main.points[split][0])
            .signum();
        let bend = side * if outlet.sea { 24.0 } else { 88.0 };
        let mut points = std::array::from_fn(|i| {
            let t = i as f64 / (POINTS - 1) as f64;
            [
                bend * (t * std::f64::consts::PI).sin().powi(2),
                0.0,
                t * length,
            ]
        });
        if outlet.sea
            && main.lake.is_some_and(|lake| {
                points.iter().any(|point| {
                    let [x, z] = frame.world(point[0], point[2]);
                    lake.sample(x, z).distance <= lake.radius + BANK_REACH + 32.0
                })
            })
        {
            // An ocean offshoot cannot descend below an intermediate lake and
            // then climb back to its reservoir through the shoreline blend.
            return None;
        }
        let width = (main.width * 0.65).clamp(1.6, 3.8);
        let mut ground = [0.0; POINTS];
        let mut caps = [0.0; POINTS];
        for i in 0..POINTS {
            let point = frame.world(points[i][0], points[i][2]);
            ground[i] = self.watershed_height(point[0], point[1]);
            let mut support = f64::INFINITY;
            for along in [-0.5, -0.25, 0.0, 0.25, 0.5] {
                let at = (i as f64 + along).clamp(0.0, (POINTS - 1) as f64);
                let a = at.floor() as usize;
                let b = (a + 1).min(POINTS - 1);
                let cross = points[a][0] + (points[b][0] - points[a][0]) * at.fract();
                for side in [-1.0, -0.5, 0.0, 0.5, 1.0] {
                    let reach = if self.config().river_terrain_revision >= 5 {
                        12.0
                    } else {
                        width * 1.4 + 2.0
                    };
                    let [x, z] = frame.world(cross + side * reach, at * step);
                    support = support.min(self.watershed_height(x, z));
                }
            }
            let reservoir = main.lake.is_some_and(|lake| {
                lake.sample(point[0], point[1]).distance <= lake.radius + BANK_REACH
            });
            caps[i] = (support - 2.5).max(self.config().sea_level);
            if reservoir {
                caps[i] = caps[i].max(outlet.level);
            }
            if caps[i] < outlet.level || (i == 0 && caps[i] < main.points[split][1]) {
                return None;
            }
        }
        let mut previous = main.points[split][1];
        for i in 0..POINTS {
            let t = i as f64 / (POINTS - 1) as f64;
            let grade = 1.0 - smoothstep(0.0, 0.90, t);
            previous = previous
                .min(caps[i])
                .min(outlet.level + (main.points[split][1] - outlet.level) * grade);
            points[i][1] = previous;
        }
        points[0][1] = main.points[split][1];
        let path = RiverPath {
            points,
            ground,
            extensions: [0.0; 2],
            width,
            phase: main.phase + split as f64 * 0.73,
            branch: None,
            brook: true,
            step,
            frame,
            lake: main.lake,
        };
        // The joint and the receiving lake/sea may overlap. Everywhere else,
        // this is an independent route, not a bow that silently rejoins its trunk.
        if crosses_course(&path, main, true) {
            return None;
        }
        Some(path)
    }
}

fn world(path: &RiverPath, index: usize) -> [f64; 2] {
    path.frame
        .world(path.points[index][0], path.points[index][2])
}

fn distance(a: [f64; 2], b: [f64; 2]) -> f64 {
    (a[0] - b[0]).hypot(a[1] - b[1])
}

fn segment_distance(point: [f64; 2], a: [f64; 2], b: [f64; 2]) -> f64 {
    let delta = [b[0] - a[0], b[1] - a[1]];
    let along = (((point[0] - a[0]) * delta[0] + (point[1] - a[1]) * delta[1])
        / (delta[0] * delta[0] + delta[1] * delta[1]).max(1e-12))
    .clamp(0.0, 1.0);
    distance(point, [a[0] + delta[0] * along, a[1] + delta[1] * along])
}

fn segment_pair_distance(a: [f64; 2], b: [f64; 2], c: [f64; 2], d: [f64; 2]) -> f64 {
    let cross = |u: [f64; 2], v: [f64; 2]| u[0] * v[1] - u[1] * v[0];
    let ab = [b[0] - a[0], b[1] - a[1]];
    let cd = [d[0] - c[0], d[1] - c[1]];
    let ac = [c[0] - a[0], c[1] - a[1]];
    let determinant = cross(ab, cd);
    if determinant.abs() > 1e-12
        && (0.0..=1.0).contains(&(cross(ac, cd) / determinant))
        && (0.0..=1.0).contains(&(cross(ac, ab) / determinant))
    {
        return 0.0;
    }
    segment_distance(a, c, d)
        .min(segment_distance(b, c, d))
        .min(segment_distance(c, a, b))
        .min(segment_distance(d, a, b))
}

fn crosses_course(path: &RiverPath, other: &RiverPath, shared_joint: bool) -> bool {
    let mut separated = !shared_joint;
    for i in 1..POINTS {
        let previous = world(path, i - 1);
        let point = world(path, i);
        if path.lake.is_some_and(|lake| {
            [previous, point]
                .into_iter()
                .all(|point| lake.sample(point[0], point[1]).distance < lake.radius + BANK_REACH)
        }) {
            continue;
        }
        let touching = (0..POINTS - 1).any(|j| {
            segment_pair_distance(previous, point, world(other, j), world(other, j + 1))
                < path.width + other.width + 8.0
        });
        if touching && separated {
            return true;
        }
        separated |= !touching;
    }
    // A shared fork may open gradually; once separate, it must stay separate.
    !separated
}

#[cfg(test)]
mod tests;
