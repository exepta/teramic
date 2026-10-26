//! Revision-two lateral adits. Planning reads natural columns, never structure sampling.
use super::{BuildingRole, BuildingSummary, REGION_SIZE, TerraGenerator, build::Builder, loot};
use crate::{edits::BlockObjectState, generation::SurfaceColumn};
use std::collections::HashMap;

const MAX_COLUMNS: usize = 1536;
const DIRECTIONS: [[i64; 3]; 4] = [[1, 0, 0], [0, 0, 1], [-1, 0, 0], [0, 0, -1]];
const OFFSETS: [[i64; 2]; 13] = [
    [0, 0],
    [-16, 0],
    [16, 0],
    [0, -16],
    [0, 16],
    [-16, -16],
    [16, -16],
    [-16, 16],
    [16, 16],
    [-32, 0],
    [32, 0],
    [0, -32],
    [0, 32],
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Adit {
    start: [i64; 3],
    inward: [i64; 3],
    mouth: i64,
    length: i64,
}
impl Adit {
    fn end(self) -> i64 {
        self.mouth + self.length
    }
    fn point(self, step: i64, lateral: i64, height: i64) -> [i64; 3] {
        [
            self.start[0] + self.inward[0] * step - self.inward[2] * lateral,
            self.start[1] + height,
            self.start[2] + self.inward[2] * step + self.inward[0] * lateral,
        ]
    }
    fn half_width(self, step: i64) -> i64 {
        if step >= self.end() - 5 { 2 } else { 1 }
    }
    fn bounds(self) -> [[i64; 3]; 2] {
        let points = [-2, self.end() + 1].into_iter().flat_map(|step| {
            [-3, 3].into_iter().flat_map(move |lateral| {
                [-1, 4]
                    .into_iter()
                    .map(move |height| self.point(step, lateral, height))
            })
        });
        points.fold([[i64::MAX; 3], [i64::MIN; 3]], |mut bounds, p| {
            for (axis, value) in p.into_iter().enumerate() {
                bounds[0][axis] = bounds[0][axis].min(value);
                bounds[1][axis] = bounds[1][axis].max(value);
            }
            bounds
        })
    }
}

struct Natural<'a> {
    terrain: &'a TerraGenerator,
    heights: HashMap<[i64; 2], f64>,
    columns: HashMap<[i64; 2], SurfaceColumn>,
}
impl<'a> Natural<'a> {
    fn new(terrain: &'a TerraGenerator) -> Self {
        Self {
            terrain,
            heights: HashMap::new(),
            columns: HashMap::new(),
        }
    }
    fn height(&mut self, p: [i64; 3]) -> f64 {
        *self.heights.entry([p[0], p[2]]).or_insert_with(|| {
            self.terrain
                .surface_height(p[0] as f64 + 0.5, p[2] as f64 + 0.5)
        })
    }
    fn column(&mut self, p: [i64; 3]) -> Option<SurfaceColumn> {
        let key = [p[0], p[2]];
        if let Some(column) = self.columns.get(&key) {
            return Some(*column);
        }
        if self.columns.len() >= MAX_COLUMNS {
            return None;
        }
        let column = self
            .terrain
            .surface_column(p[0] as f64 + 0.5, p[2] as f64 + 0.5);
        self.columns.insert(key, column);
        Some(column)
    }
    fn solid(&self, p: [i64; 3], column: SurfaceColumn, rock: bool) -> bool {
        let sample = self
            .terrain
            .sample_with_column(p.map(|v| v as f64 + 0.5), column);
        sample.density <= 0.0
            && if rock {
                matches!(sample.material, 3 | 11..=13 | 37 | 41 | 43 | 57..=58)
            } else {
                terra_voxel::MaterialId::from_raw(sample.material)
                    .is_some_and(terra_voxel::MaterialId::is_solid)
            }
    }
    fn dry(&self, adit: Adit, column: SurfaceColumn) -> bool {
        column
            .water_columns()
            .intervals
            .into_iter()
            .flatten()
            .all(|water| {
                water.level <= adit.start[1] as f64 || water.bed >= adit.start[1] as f64 + 4.0
            })
    }
    fn covered(&mut self, adit: Adit) -> bool {
        let outside = adit.point(-3, 0, 0);
        let Some(column) = self.column(outside) else {
            return false;
        };
        if !self.dry(adit, column)
            || !self.solid(adit.point(-3, 0, -2), column, false)
            || (0..=2).any(|height| self.solid(adit.point(-3, 0, height), column, false))
        {
            return false;
        }
        // A supported horizontal approach can cut into the slope, but may not
        // bridge an unseen shaft or turn into a descending surface excavation.
        for step in -2..adit.mouth {
            for lateral in -2..=2 {
                let p = adit.point(step, lateral, -2);
                let Some(column) = self.column(p) else {
                    return false;
                };
                if !self.dry(adit, column) || !self.solid(p, column, false) {
                    return false;
                }
            }
        }
        for step in adit.mouth..=adit.end() + 1 {
            let width = adit.half_width(step);
            for lateral in -width - 1..=width + 1 {
                let p = adit.point(step, lateral, -2);
                let Some(column) = self.column(p) else {
                    return false;
                };
                if !self.dry(adit, column)
                    || !self.solid(p, column, true)
                    || !self.solid(adit.point(step, lateral, 4), column, false)
                    || !self.solid(adit.point(step, lateral, 1), column, true)
                {
                    return false;
                }
                if step >= adit.mouth + 4
                    && (column.height < adit.start[1] as f64 + 8.0
                        || !self.solid(adit.point(step, lateral, 4), column, true)
                        || !self.solid(adit.point(step, lateral, 6), column, true))
                {
                    return false;
                }
            }
        }
        true
    }
}

fn find(terrain: &TerraGenerator, center: [i64; 2], seed: u32) -> Option<Adit> {
    let region = center.map(|v| v.div_euclid(REGION_SIZE) * REGION_SIZE);
    let mut natural = Natural::new(terrain);
    for index in 0..OFFSETS.len() {
        let offset = OFFSETS[(index + seed as usize % OFFSETS.len()) % OFFSETS.len()];
        for direction in 0..4 {
            let inward = DIRECTIONS[(direction + (seed >> 7) as usize) % 4];
            let mut start = [center[0] + offset[0], 0, center[1] + offset[1]];
            let outside = [start[0] - inward[0] * 3, 0, start[2] - inward[2] * 3];
            start[1] = (natural.height(outside) - 0.5).floor() as i64 + 1;
            if !(-90..=300).contains(&start[1]) {
                continue;
            }
            let mut adit = Adit {
                start,
                inward,
                mouth: 0,
                length: 20 + i64::from((seed >> 12) % 7),
            };
            let early = natural.height(adit.point(12, 0, 0));
            let deep = natural.height(adit.point(28, 0, 0));
            if early < start[1] as f64 + 5.0 || deep < start[1] as f64 + 12.0 {
                continue;
            }
            for mouth in 3..=12 {
                adit.mouth = mouth;
                if natural.height(adit.point(mouth, 0, 0)) < start[1] as f64 + 5.0 {
                    continue;
                }
                let bounds = adit.bounds();
                if [0, 2].into_iter().enumerate().any(|(i, axis)| {
                    bounds[0][axis] < region[i] + 96 || bounds[1][axis] > region[i] + 288
                }) {
                    continue;
                }
                if natural.covered(adit) {
                    return Some(adit);
                }
            }
        }
    }
    None
}

pub(super) fn build(builder: &mut Builder<'_>, x: i64, z: i64) -> Option<()> {
    let adit = find(builder.terrain, [x, z], builder.seed)?;
    let entrance = adit.point(adit.mouth, 0, 0);
    for step in -2..=adit.end() {
        let width = adit.half_width(step);
        for lateral in -width - 1..=width + 1 {
            builder.set(adit.point(step, lateral, -1), 43);
        }
        for lateral in -width..=width {
            for height in 0..=3 {
                builder.set(adit.point(step, lateral, height), 0);
            }
        }
        if step >= adit.mouth && ((step - adit.mouth) as u64).is_multiple_of(4)
            || step == adit.mouth - 1
        {
            for lateral in [-width - 1, width + 1] {
                for height in 0..=3 {
                    builder.set(adit.point(step, lateral, height), 6);
                }
            }
            for lateral in -width - 1..=width + 1 {
                builder.set(adit.point(step, lateral, 3), 16);
            }
        }
        if step >= adit.mouth && (step - adit.mouth) % 8 == 3 {
            let lateral = if (builder.seed & 1) == 0 {
                width
            } else {
                -width
            };
            let normal = [
                -adit.inward[2] * -lateral.signum(),
                0,
                adit.inward[0] * -lateral.signum(),
            ];
            let facing = match normal {
                [1, 0, 0] => 4,
                [-1, 0, 0] => 5,
                [0, 0, 1] => 6,
                _ => 7,
            };
            builder.state(adit.point(step, lateral, 1), 40, facing);
        }
    }
    let chest = adit.point(adit.end() - 2, 1, 0);
    let facing = chest_facing(adit.inward, builder.terrain.config().structures_revision);
    builder.object(
        chest,
        29,
        Some(BlockObjectState {
            facing,
            ..Default::default()
        }),
        Some(loot::inventory(BuildingRole::Mine, builder.seed)),
    );
    builder.summary.containers.push(chest);
    builder.summary.center = entrance;
    builder.summary.buildings.push(BuildingSummary {
        role: BuildingRole::Mine,
        entrance,
        bounds: adit.bounds(),
    });
    Some(())
}

fn chest_facing(inward: [i64; 3], revision: u8) -> u8 {
    let facing = match inward {
        [0, 0, 1] => 0,
        [1, 0, 0] => 1,
        [0, 0, -1] => 2,
        _ => 3,
    };
    (facing + if revision >= 3 { 2 } else { 0 }) % 4
}

#[cfg(test)]
mod tests;
