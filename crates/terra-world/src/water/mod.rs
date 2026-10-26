//! Water shares section batches but keeps fluid occupancy separate from solid blocks.
#[cfg(feature = "bevy")]
pub mod buckets;
mod channels;
pub(crate) mod fluid;
#[cfg(any(feature = "bevy", test))]
mod fluid_mesh;
mod geometry;
#[cfg(feature = "bevy")]
mod motion;
pub(crate) mod outlets;
#[cfg(feature = "bevy")]
mod simulation;
pub(crate) mod space;
pub(crate) use channels::build_channel_patch;
#[cfg(test)]
mod tests;

pub use fluid::{
    FLUID_CAPACITY, FluidCell, FluidError, FluidSnapshot, can_hold_fluid, fluid_cell_at,
};
#[cfg(feature = "bevy")]
pub(crate) use fluid_mesh::{FluidOccluders, append_fluid_patch};
pub use geometry::WaterPatch;
pub(crate) use geometry::build_water_patch;
#[cfg(feature = "bevy")]
pub use motion::{CURRENT_RESPONSE, CURRENT_SPEED, FluidMotion, falling_fluid_at, fluid_motion_at};
pub(crate) use outlets::natural_outlets;
#[cfg(feature = "bevy")]
pub(crate) use simulation::tick as tick_fluids;
#[cfg(feature = "bevy")]
pub use simulation::{FluidDiagnostics, FluidStep, FluidWorkCounts, simulate_fluid_step};

pub const WATER_SURFACE_VERSION: u32 = 1;
pub const MIN_WATERFALL_DROP: f64 = 5.0;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WaterColumn {
    pub level: f64,
    pub bed: f64,
    pub depth: f64,
}

/// Independent bounded reservoirs may coexist above and below solid rock.
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct WaterColumns {
    pub intervals: [Option<WaterColumn>; 3],
}
impl WaterColumns {
    pub fn at(self, y: i64) -> Option<WaterColumn> {
        self.intervals
            .into_iter()
            .flatten()
            .find(|c| y as f64 >= c.bed && (y as f64) < c.level)
    }
    pub fn below(self, y: f64) -> Option<WaterColumn> {
        self.intervals
            .into_iter()
            .flatten()
            .filter(|c| c.bed <= y)
            .min_by(|a, b| (a.level - y).abs().total_cmp(&(b.level - y).abs()))
    }
}

impl crate::TerraGenerator {
    pub fn water_columns(&self, x: f64, z: f64) -> WaterColumns {
        WaterColumns {
            intervals: [
                self.water_column(x, z),
                self.cave_structure_at(x, z).water,
                None,
            ],
        }
    }
    pub fn water_column(&self, x: f64, z: f64) -> Option<WaterColumn> {
        if !self.config().water {
            return None;
        }
        let river = self.river_at(x, z);
        let oasis = self.oasis_at(x, z).or_else(|| self.lake_at(x, z));
        let surface = self.surface_height(x, z);
        let floor = river
            .and_then(|river| river.cavity())
            .map_or(surface, |cavity| surface.min(cavity[0]));
        let bed = (floor - 0.5).floor() + 1.0;
        let level = oasis
            .filter(|sample| sample.distance <= sample.radius)
            .map_or_else(
                || {
                    river.map_or(self.config().sea_level, |river| {
                        river.water_level(self.config().sea_level)
                    })
                },
                |sample| sample.level,
            );
        (bed < level).then_some(WaterColumn {
            level,
            bed,
            depth: level - bed,
        })
    }
}

pub(crate) fn build_natural_patch(
    position: terra_voxel::SectionPos,
    origin: [i64; 3],
    columns: &[crate::SurfaceColumn],
    padded: &[u16],
    sea_level: f64,
    river: bool,
) -> Option<WaterPatch> {
    let mut patch = if river {
        build_channel_patch(position, origin, columns, padded)
    } else {
        build_water_patch(position, origin, sea_level, columns, padded)
    };
    if columns.iter().any(|c| c.cave.water.is_some()) {
        let underground: Vec<_> = columns
            .iter()
            .copied()
            .enumerate()
            .map(|(index, mut c)| {
                c.water_level = c.cave.water.map(|w| w.level);
                c.height = c.cave.water.map_or(c.cave.floor, |w| w.bed - 0.01);
                c.river_cavity = None;
                c.water_flow = if c.cave.active()
                    && c.cave.kind == crate::generation::CaveEcology::Waterfall
                {
                    [-0.4, 0.0]
                } else {
                    c.cave
                        .river_flow(origin[0] as f64 + (index % 18) as f64 - 0.5)
                };
                c
            })
            .collect();
        if let Some(add) = build_channel_patch(position, origin, &underground, padded) {
            if let Some(target) = patch.as_mut() {
                target.append(add);
            } else {
                patch = Some(add);
            }
        }
    }
    patch
}

pub(crate) fn local_water_levels(columns: &[crate::SurfaceColumn], sea_level: f64) -> bool {
    columns
        .iter()
        .any(|column| column.water_level.is_some_and(|level| level != sea_level))
}
