use super::*;
use std::collections::{HashMap, HashSet, VecDeque};
const RADIUS: i64 = 42;
#[derive(Clone, Copy, PartialEq, Eq)]
struct Surface {
    base: i64,
    stair: Option<u8>,
    bridge: bool,
}
pub(super) struct Routes {
    origin: [i64; 2],
    radius: i64,
    width: usize,
    blocked: Vec<bool>,
    pub(super) walk: HashSet<[i64; 2]>,
    levels: HashMap<[i64; 2], i64>,
    surfaces: HashMap<[i64; 2], Surface>,
}
impl Routes {
    pub(super) fn new(origin: [i64; 2], plots: &[Plot], farm: Option<[[i64; 2]; 2]>) -> Self {
        Self::with_radius(origin, plots, farm, RADIUS)
    }
    pub(super) fn with_radius(
        origin: [i64; 2],
        plots: &[Plot],
        farm: Option<[[i64; 2]; 2]>,
        radius: i64,
    ) -> Self {
        let radius = radius.clamp(RADIUS, 84);
        let width = (radius * 2 + 1) as usize;
        let mut result = Self {
            origin,
            radius,
            width,
            blocked: vec![false; width * width],
            walk: HashSet::new(),
            levels: HashMap::new(),
            surfaces: HashMap::new(),
        };
        for bounds in plots.iter().map(Plot::bounds).chain(farm) {
            for z in bounds[0][1] - 2..=bounds[1][1] + 2 {
                for x in bounds[0][0] - 2..=bounds[1][0] + 2 {
                    if let Some(i) = result.index([x, z]) {
                        result.blocked[i] = true;
                    }
                }
            }
        }
        if let Some(farm) = farm {
            let gate = (farm[0][0] + farm[1][0]) / 2;
            for z in farm[0][1] - 3..=farm[0][1] + 1 {
                for x in gate - 1..=gate + 1 {
                    if let Some(i) = result.index([x, z]) {
                        result.blocked[i] = false;
                    }
                }
            }
        }
        result
    }
    fn index(&self, p: [i64; 2]) -> Option<usize> {
        let x = p[0] - self.origin[0] + self.radius;
        let z = p[1] - self.origin[1] + self.radius;
        ((0..self.width as i64).contains(&x) && (0..self.width as i64).contains(&z))
            .then_some((x + z * self.width as i64) as usize)
    }
    fn point(&self, i: usize) -> [i64; 2] {
        [
            self.origin[0] + (i % self.width) as i64 - self.radius,
            self.origin[1] + (i / self.width) as i64 - self.radius,
        ]
    }
    fn junction(&self, p: [i64; 2]) -> bool {
        let Some(surface) = self.surfaces.get(&p) else {
            return false;
        };
        surface.stair.is_none()
            && (-1..=1).all(|dx| {
                (-1..=1).all(|dz| {
                    self.surfaces
                        .get(&[p[0] + dx, p[1] + dz])
                        .is_none_or(|other| other == surface)
                })
            })
    }
    pub(super) fn connect(&mut self, builder: &mut Builder<'_>, goal: [i64; 3]) -> Option<()> {
        let end = self.index([goal[0], goal[2]])?;
        let mut previous = vec![usize::MAX; self.width * self.width];
        let mut protected = vec![false; self.width * self.width];
        for (&p, surface) in &self.surfaces {
            if surface.stair.is_some() {
                for dx in -1..=1 {
                    for dz in -1..=1 {
                        if let Some(i) = self.index([p[0] + dx, p[1] + dz]) {
                            protected[i] = true;
                        }
                    }
                }
            }
        }
        let mut starts: Vec<_> = self
            .levels
            .keys()
            .copied()
            .filter(|&p| self.junction(p))
            .collect();
        if self.levels.is_empty() {
            starts.push(self.origin);
        }
        starts.sort_unstable();
        let mut queue = VecDeque::new();
        for p in starts {
            let start = self.index(p)?;
            previous[start] = start;
            queue.push_back(start);
        }
        while let Some(i) = queue.pop_front() {
            if i == end {
                break;
            }
            let p = self.point(i);
            for [dx, dz] in [[1, 0], [0, 1], [-1, 0], [0, -1]] {
                let Some(next) = self.index([p[0] + dx, p[1] + dz]) else {
                    continue;
                };
                if previous[next] != usize::MAX
                    || self.blocked[next] && next != end
                    || protected[next]
                {
                    continue;
                }
                previous[next] = i;
                queue.push_back(next);
            }
        }
        if previous[end] == usize::MAX {
            return reject(
                builder.seed,
                goal,
                "unreachable",
                [i64::from(protected[end]), i64::from(self.blocked[end]), 0],
            );
        }
        let mut route = vec![end];
        while previous[*route.last()?] != *route.last()? {
            route.push(previous[*route.last()?]);
        }
        route.reverse();
        let start = self.point(route[0]);
        let mut foot = self
            .levels
            .get(&start)
            .copied()
            .unwrap_or_else(|| builder.ground(start[0], start[1]));
        let mut flat = vec![false; route.len()];
        flat[0] = true;
        for (i, &cell) in route.iter().enumerate() {
            let p = self.point(cell);
            flat[i] |= (-1..=1)
                .any(|dx| (-1..=1).any(|dz| self.surfaces.contains_key(&[p[0] + dx, p[1] + dz])));
            if i > 0 && i + 1 < route.len() {
                let a = self.point(route[i - 1]);
                let b = self.point(route[i + 1]);
                if [p[0] - a[0], p[1] - a[1]] != [b[0] - p[0], b[1] - p[1]] {
                    flat[i - 1..=i + 1].fill(true);
                }
            }
        }
        let mut remaining_grade = vec![0_i64; route.len()];
        for i in (0..route.len() - 1).rev() {
            remaining_grade[i] = remaining_grade[i + 1] + i64::from(!flat[i] && !flat[i + 1]);
        }
        if foot.abs_diff(goal[1]) > remaining_grade[0] as u64 {
            return reject(
                builder.seed,
                goal,
                "grade-capacity",
                [foot, goal[1], remaining_grade[0]],
            );
        }
        for (step, &index) in route.iter().enumerate() {
            let p = self.point(index);
            let previous_point = self.point(route[step.saturating_sub(1)]);
            let next_point = self.point(route[(step + 1).min(route.len() - 1)]);
            let direction = if next_point != p {
                [next_point[0] - p[0], next_point[1] - p[1]]
            } else {
                [p[0] - previous_point[0], p[1] - previous_point[1]]
            };
            let side = if direction[0] != 0 { [0, 1] } else { [1, 0] };
            let natural = builder.ground(p[0], p[1]);
            let water = builder
                .terrain
                .natural_surface_water(p[0] as f64 + 0.5, p[1] as f64 + 0.5);
            let bridge = water.is_some_and(|w| w.level > natural as f64);
            let desired = self.levels.get(&p).copied().unwrap_or_else(|| {
                if bridge {
                    water.unwrap().level.ceil() as i64 + 1
                } else {
                    natural
                }
            });
            let remaining = remaining_grade[step];
            let grade = i64::from(step > 0 && !flat[step - 1] && !flat[step]);
            let height = desired
                .clamp(goal[1] - remaining, goal[1] + remaining)
                .clamp(foot - grade, foot + grade);
            if (height - goal[1]).abs() > remaining {
                return reject(
                    builder.seed,
                    goal,
                    "grade-progress",
                    [height, goal[1], remaining],
                );
            }
            if self.levels.get(&p).is_some_and(|&old| old != height) {
                return reject(builder.seed, goal, "center-level", [p[0], height, p[1]]);
            }
            let slope = height - foot;
            let base = height - 1 + i64::from(slope < 0 && !bridge);
            let ascent = if slope < 0 {
                [previous_point[0] - p[0], previous_point[1] - p[1]]
            } else {
                [p[0] - previous_point[0], p[1] - previous_point[1]]
            };
            let surface = Surface {
                base,
                stair: (slope != 0 && !bridge).then(|| facing(ascent)),
                bridge,
            };
            for lane in -1..=1 {
                let key = [p[0] + side[0] * lane, p[1] + side[1] * lane];
                if self.surfaces.get(&key).is_some_and(|old| *old != surface) {
                    return reject(builder.seed, goal, "lane-profile", [key[0], base, key[1]]);
                }
            }
            let lane_columns = [-1, 0, 1].map(|lane| {
                let key = [p[0] + side[0] * lane, p[1] + side[1] * lane];
                let column = builder
                    .terrain
                    .surface_column(key[0] as f64 + 0.5, key[1] as f64 + 0.5);
                (key, column)
            });
            if bridge
                && !bridge_clearance(
                    base,
                    water.map(|water| water.level),
                    lane_columns.map(|(_, column)| column.water_level),
                )
            {
                return reject(builder.seed, goal, "bridge-water", [p[0], base, p[1]]);
            }
            for (lane, ([x, z], column)) in (-1..=1).zip(lane_columns) {
                self.walk.insert([x, z]);
                if self.surfaces.contains_key(&[x, z]) {
                    continue;
                }
                let lane_ground = (column.height - 0.5).floor() as i64 + 1;
                if !bridge {
                    if column.water_level.is_some_and(|y| y > base as f64 + 0.2) {
                        return reject(builder.seed, goal, "dry-lane-water", [x, base, z]);
                    }
                    if !foundation_supported(builder.terrain, x, z, base + 1, column) {
                        return reject(builder.seed, goal, "foundation", [x, base, z]);
                    }
                    builder.fill([x, lane_ground.min(base) - 1, z], [x, base - 1, z], 2);
                }
                builder.fill([x, base, z], [x, base + 4, z], 0);
                if bridge {
                    builder.set([x, base, z], 16);
                } else if slope != 0 {
                    builder.state([x, base, z], 78, facing(ascent));
                } else if lane != 0 && ((x ^ z ^ i64::from(builder.seed)) & 3) == 0 {
                    builder.state([x, base, z], 86, 0);
                } else {
                    builder.set([x, base, z], PATH);
                }
                self.surfaces.insert([x, z], surface);
            }
            if bridge {
                for offset in [-2, 2] {
                    let x = p[0] + side[0] * offset;
                    let z = p[1] + side[1] * offset;
                    if !self.walk.contains(&[x, z]) {
                        let support = builder.ground(x, z);
                        builder.fill([x, support, z], [x, base, z], 6);
                        builder.set([x, base + 1, z], 90);
                    }
                }
                if !builder
                    .summary
                    .buildings
                    .iter()
                    .any(|b| b.role == BuildingRole::Bridge)
                {
                    builder.summary.buildings.push(BuildingSummary {
                        role: BuildingRole::Bridge,
                        entrance: [p[0], height, p[1]],
                        bounds: [
                            [p[0] - 2, natural, p[1] - 2],
                            [p[0] + 2, height + 1, p[1] + 2],
                        ],
                    });
                }
            }
            self.levels.entry(p).or_insert(height);
            foot = height;
        }
        Some(())
    }
}
pub(super) fn bridge_clearance(base: i64, center: Option<f64>, lanes: [Option<f64>; 3]) -> bool {
    let top = base as f64 + 1.0;
    center
        .into_iter()
        .chain(lanes.into_iter().flatten())
        .all(|water| water < top)
}
#[inline]
fn reject(_seed: u32, _goal: [i64; 3], _reason: &str, _detail: [i64; 3]) -> Option<()> {
    #[cfg(test)]
    if std::env::var_os("TERRA_STRUCTURE_TRACE").is_some() {
        eprintln!("street seed={_seed} goal={_goal:?} reason={_reason} detail={_detail:?}");
    }
    None
}
fn facing([x, z]: [i64; 2]) -> u8 {
    if x > 0 {
        1
    } else if x < 0 {
        3
    } else if z > 0 {
        0
    } else {
        2
    }
}
