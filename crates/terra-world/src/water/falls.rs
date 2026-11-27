//! Settled natural discharge. Each span is an immutable generator result,
//! shared by the first mesh, remesh and physics, without a simulation tick.
use super::{FluidCell, FluidSnapshot};
use crate::{SurfaceColumn, edits::EditJournal};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Fall {
    pub bottom: i32,
    pub top: i32,
    pub source_y: i32,
    /// Horizontal displacement from this column back to its feeding source.
    pub source: [i8; 2],
    pub authored: bool,
    /// Exposed crest height in 1/256 of a block. The reservoir can sit between
    /// the coarser eighth-block units used for transport.
    crest_height: u8,
    /// Heights lost at successive channel steps (up to 11 five-bit offsets).
    pub route: u64,
}
impl Fall {
    pub(crate) fn new(bottom: i64, top: i64, source_y: i64, source: [i8; 2]) -> Self {
        Self {
            bottom: bottom as i32,
            top: top as i32,
            source_y: source_y as i32,
            source,
            authored: false,
            crest_height: 224,
            route: 0,
        }
    }
    pub fn contains(self, y: i64) -> bool {
        (i64::from(self.bottom)..=i64::from(self.top)).contains(&y)
    }
    pub(crate) fn fed_by(mut self, level: f64) -> Self {
        let head = (level - f64::from(self.top)).clamp(0., 1.);
        self.crest_height = (224. * head).floor() as u8;
        self
    }
    pub(crate) fn cell(self, y: i64) -> FluidCell {
        if self.authored && self.source == [0; 2] && y == i64::from(self.source_y) {
            return FluidCell {
                amount: 8,
                source: true,
                flow: [0; 2],
            };
        }
        let crest = y == i64::from(self.top);
        FluidCell {
            amount: if crest {
                self.crest_height.div_ceil(32).max(1)
            } else {
                8
            },
            source: false,
            flow: if crest {
                self.source.map(|v| -v.signum())
            } else {
                [0; 2]
            },
        }
    }
    fn supplied(self, p: [i64; 3], mut open: impl FnMut([i64; 3]) -> bool) -> bool {
        let source = [
            p[0] + i64::from(self.source[0]),
            i64::from(self.source_y),
            p[2] + i64::from(self.source[1]),
        ];
        if !open(source) {
            return false;
        }
        let steps = self.source[0]
            .unsigned_abs()
            .max(self.source[1].unsigned_abs());
        let mut high = source[1];
        for i in 1..steps {
            let low = source[1] - ((self.route >> (5 * (i - 1))) & 31) as i64;
            let x = source[0] - i64::from(i) * i64::from(self.source[0].signum());
            let z = source[2] - i64::from(i) * i64::from(self.source[1].signum());
            if !(low..=high).all(|y| open([x, y, z])) {
                return false;
            }
            high = low;
        }
        (p[1]..=i64::from(self.top)).all(|y| open([p[0], y, p[2]]))
    }
    pub(crate) fn live(self, p: [i64; 3], journal: &EditJournal) -> bool {
        self.supplied(p, |q| {
            let edited = journal.block(q);
            let removed_source = self.authored
                && q == [
                    p[0] + i64::from(self.source[0]),
                    i64::from(self.source_y),
                    p[2] + i64::from(self.source[1]),
                ]
                && edited.is_some_and(|m| m != terra_voxel::MaterialId::FluidWater as u16);
            !removed_source
                && edited.is_none_or(super::can_hold_fluid)
                && journal.fluid(q).is_none_or(|c| c.amount > 0)
        })
    }
}

pub(super) fn crest_depth(column: &SurfaceColumn, y: i64, cell: FluidCell) -> Option<f64> {
    if cell.flow == [0; 2] || cell.source {
        return None;
    }
    column
        .falls
        .iter()
        .flatten()
        .find(|f| y == i64::from(f.top) && cell == f.cell(y))
        .map(|f| f64::from(f.crest_height) / 256.)
}

pub(crate) fn add_cells(
    columns: &[SurfaceColumn],
    origin: [i64; 3],
    cells: &mut FluidSnapshot,
    journal: Option<&EditJournal>,
) {
    for (i, col) in columns.iter().enumerate() {
        for span in col.falls.into_iter().flatten() {
            let x = origin[0] + (i % 18) as i64 - 1;
            let z = origin[2] + (i / 18) as i64 - 1;
            for y in
                i64::from(span.bottom).max(origin[1] - 1)..=i64::from(span.top).min(origin[1] + 16)
            {
                let p = [x, y, z];
                if col.water_columns().at(y).is_some() {
                    continue;
                }
                cells.entry(p).or_insert_with(|| {
                    if journal.is_none_or(|j| span.live(p, j)) {
                        span.cell(y)
                    } else {
                        FluidCell::default()
                    }
                });
            }
        }
    }
}

pub(crate) fn sampled(
    columns: super::WaterColumns,
    p: [i64; 3],
    journal: &EditJournal,
) -> FluidCell {
    if let Some(pool) = columns.at(p[1]) {
        return super::fluid::from_column(Some(pool), p[1]);
    }
    columns
        .falls
        .into_iter()
        .flatten()
        .find(|s| s.contains(p[1]) && s.live(p, journal))
        .map_or_else(FluidCell::default, |s| s.cell(p[1]))
}

#[cfg(feature = "bevy")]
pub(crate) fn near(chunks: &crate::streaming::ChunkStore, owner: terra_voxel::SectionPos) -> bool {
    (-1..=1).any(|dx| (-1..=1).any(|dz| chunks.column_has_falls([owner.x + dx, owner.z + dz])))
}
#[cfg(feature = "bevy")]
pub(crate) fn invalidate(
    chunks: &crate::streaming::ChunkStore,
    journal: &mut EditJournal,
    changed: [i64; 3],
) -> Result<(), terra_voxel::blocks::BlockError> {
    if !near(chunks, terra_voxel::global_to_section(changed).0) {
        return Ok(());
    }
    let [x, _, z] = changed;
    let mut sections = std::collections::BTreeSet::new();
    for offset in -11..=11 {
        for [cx, cz] in [[x + offset, z], [x, z + offset]] {
            let Some(c) = chunks.surface_column_at(cx, cz) else {
                continue;
            };
            for span in c.falls.into_iter().flatten() {
                if !span.supplied([cx, i64::from(span.bottom), cz], |q| q != changed) {
                    let lo = i64::from(span.bottom).div_euclid(16);
                    let hi = i64::from(span.top).div_euclid(16);
                    for sy in lo..=hi {
                        for ox in -1..=1 {
                            for oz in -1..=1 {
                                sections.insert(terra_voxel::SectionPos::new(
                                    (cx + ox).div_euclid(16),
                                    sy,
                                    (cz + oz).div_euclid(16),
                                ));
                            }
                        }
                    }
                }
            }
        }
    }
    journal.invalidate_generated_water_sections(&sections)
}

#[cfg(feature = "bevy")]
pub(crate) fn interrupted(
    columns: &[SurfaceColumn],
    origin: [i64; 3],
    journal: &EditJournal,
) -> bool {
    columns.iter().enumerate().any(|(i, c)| {
        c.falls.into_iter().flatten().any(|s| {
            let y = i64::from(s.bottom).max(origin[1] - 1);
            y <= i64::from(s.top).min(origin[1] + 16)
                && !s.live(
                    [
                        origin[0] + (i % 18) as i64 - 1,
                        y,
                        origin[2] + (i / 18) as i64 - 1,
                    ],
                    journal,
                )
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn broad_spills_join_the_reservoir_and_slope_downstream_across_section_edges() {
        let terrain = crate::TerraGenerator::new(Default::default()).unwrap();
        let mut base = terrain.surface_column(0.5, 0.5);
        base.height = 3.;
        base.river_cavity = None;
        base.cave.water = None;
        base.cave.dynamic_water = false;
        let padded = vec![0; 18 * 18 * 18];
        for level in [12.5_f64, 12.04, 13.] {
            for (axis, direction) in [(0, 1_i64), (0, -1), (2, 1), (2, -1)] {
                let mut edge_heights = vec![];
                for side in [-1, 0] {
                    let mut section = terra_voxel::SectionPos::default();
                    if axis == 0 {
                        section.x = side
                    } else {
                        section.z = side
                    }
                    let origin = section.origin().unwrap();
                    let columns = (0..18 * 18)
                        .map(|i| {
                            let p = [
                                origin[0] + i as i64 % 18 - 1,
                                0,
                                origin[2] + i as i64 / 18 - 1,
                            ];
                            let from_lip = if direction > 0 { p[axis] } else { -1 - p[axis] };
                            let mut c = base;
                            c.water_level = (from_lip < 0).then_some(level);
                            if from_lip == 0 {
                                let mut inlet = [0; 2];
                                inlet[axis / 2] = -direction as i8;
                                let top = level.ceil() as i64 - 1;
                                c.falls[0] = Some(Fall::new(3, top, top, inlet).fed_by(level));
                            }
                            c
                        })
                        .collect::<Vec<_>>();
                    let water = super::super::build_natural_patch(
                        section, origin, &columns, &padded, 0., true,
                    )
                    .unwrap();
                    let upstream = (direction > 0) == (side == -1);
                    let mut at_lip = false;
                    let mut at_outer_face = false;
                    for (p, n) in water.positions.iter().zip(&water.normals) {
                        if n[1] <= 0. || p[2 - axis] != 8. {
                            continue;
                        }
                        let along = (p[axis] + origin[axis] as f32) * direction as f32;
                        if along == 0. {
                            at_lip = true;
                            edge_heights.push(f64::from(p[1]));
                        } else if !upstream && along == 1. {
                            at_outer_face = true;
                            assert!(
                                p[1] < level as f32,
                                "the outside of the crest must drain downhill"
                            );
                            assert!(
                                n[axis] * direction as f32 > 0.,
                                "the reflected slope faces downhill"
                            );
                        }
                    }
                    assert!(at_lip && (upstream || at_outer_face));
                }
                assert!(
                    edge_heights.iter().all(|h| (h - level).abs() < 1e-5),
                    "{edge_heights:?} {level}"
                );
            }
        }
    }

    #[test]
    fn separate_vertical_workers_join_the_same_loaded_stream_and_respect_an_upstream_dam() {
        let terrain = crate::TerraGenerator::new(Default::default()).unwrap();
        let mut col = terrain.surface_column(-24.5, 2.5);
        col.height = -55.;
        col.water_level = None;
        col.cave.water = None;
        let mut columns = vec![col; 18 * 18];
        let index = 9 + 9 * 18;
        let span = Fall::new(-17, 23, 23, [-1, 0]);
        columns[index].falls[0] = Some(span);
        let mut journal = EditJournal::default();
        let mut vertices = 0;
        for sy in [-2, -1, 0, 1] {
            let owner = terra_voxel::SectionPos::new(-2, sy, 0);
            let origin = owner.origin().unwrap();
            let mut cells = FluidSnapshot::new();
            add_cells(&columns, origin, &mut cells, Some(&journal));
            let mut patch = None;
            super::super::fluid_mesh::append_fluid_patch(
                &mut patch,
                owner,
                &columns,
                &vec![0; 18 * 18 * 18],
                &cells,
                true,
            );
            let mesh =
                patch.expect("the stream is visible before ticking in every vertical section");
            let lowest = i64::from(span.bottom).max(origin[1]);
            let highest = i64::from(span.top).min(origin[1] + 15);
            assert!(
                mesh.positions
                    .iter()
                    .any(|p| p[1] < (lowest - origin[1]) as f32 + 0.05)
            );
            for y in lowest..=highest {
                assert_eq!(cells[&[-24, y, 8]], span.cell(y));
            }
            vertices += mesh.positions.len();
        }
        assert!(vertices > 150 && journal.fluid_cells().count() == 0);
        journal
            .record([-24, 19, 8], terra_voxel::MaterialId::Stone as u16)
            .unwrap();
        assert_eq!(
            sampled(columns[index].water_columns(), [-24, -17, 8], &journal).amount,
            0
        );
        assert_eq!(
            sampled(columns[index].water_columns(), [-24, 22, 8], &journal).amount,
            8
        );
        let mut refreshed = FluidSnapshot::new();
        add_cells(&columns, [-32, -32, 0], &mut refreshed, Some(&journal));
        assert_eq!(refreshed[&[-24, -17, 8]].amount, 0);
    }
}
