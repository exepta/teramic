//! Water shares section batches but keeps fluid occupancy separate from solid blocks.
#[cfg(feature = "bevy")]
pub mod buckets;
mod channels;
pub(crate) mod fluid;
#[cfg(any(feature = "bevy", test))]
pub(crate) mod fluid_mesh;
mod geometry;
#[cfg(feature = "bevy")]
mod motion;
pub(crate) mod outlets;
#[cfg(feature = "bevy")]
mod power;
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
pub use motion::{
    CURRENT_RESPONSE, CURRENT_SPEED, FluidMotion, RIVER_CURRENT_SCALE, falling_fluid_at,
    fluid_motion_at,
};
pub(crate) use outlets::natural_outlets;
#[cfg(feature = "bevy")]
pub use power::{WaterDriveKind, WheelMotion, wheel_motion_at};
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
    // Enabling rivers does not turn every ocean section into a channel. The
    // shared grid retains shoreline/depth/wave samples without duplicating its
    // corners four times per cell. Inspect the halo too, so incoming channels
    // and local reservoirs keep their original continuous edge geometry.
    let flat_surface = columns.iter().all(|column| {
        column.water_flow == [0.0; 2]
            && column.river_cavity.is_none()
            && column.cave.water.is_none()
            && column.water_level.map_or_else(
                || column.water_bed() >= sea_level,
                |level| level == sea_level,
            )
    });
    let mut patch = if river && !flat_surface {
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

#[cfg(test)]
mod natural_patch_tests {
    use super::*;
    use crate::{TerraGenerator, generation::CaveStructure};
    use terra_core::WorldConfig;
    use terra_voxel::SectionPos;

    fn columns() -> Vec<crate::SurfaceColumn> {
        let terrain = TerraGenerator::new(WorldConfig {
            base_height: 4.0,
            sea_level: 8.0,
            hill_amplitude: 0.0,
            mountain_amplitude: 0.0,
            caves: false,
            trees: false,
            biomes: false,
            rivers: false,
            structures_revision: 0,
            landscape_revision: 0,
            ..Default::default()
        })
        .unwrap();
        let mut surface = terrain.surface_column(-16.5, -0.5);
        surface.height = 4.0;
        surface.water_level = Some(8.0);
        surface.water_flow = [0.0; 2];
        surface.river_cavity = None;
        surface.cave = CaveStructure::default();
        vec![surface; 18 * 18]
    }

    #[test]
    fn river_enabled_flat_ocean_uses_shared_corners_with_existing_shore_and_wave_payloads() {
        let owner = SectionPos::new(-2, 0, -1);
        let origin = owner.origin().unwrap();
        let padded = vec![0; 18 * 18 * 18];
        let mut columns = columns();
        for z in 0..18 {
            for x in 0..18 {
                let column = &mut columns[x + 18 * z];
                column.height = if x < 5 { 9.0 } else { 2.0 + (z % 5) as f64 };
                column.water_level = (column.water_bed() < 8.0).then_some(8.0);
            }
        }
        let patch = build_natural_patch(owner, origin, &columns, &padded, 8.0, true).unwrap();
        let expected = build_water_patch(owner, origin, 8.0, &columns, &padded).unwrap();
        assert_eq!(patch, expected);
        assert_eq!(patch.positions.len(), 17 * 17);
        assert!(patch.depth_shore.iter().any(|sample| sample[1] > 0.0));
        assert!(patch.wave_weights.iter().any(|&weight| weight > 0.9));
        assert!(
            patch
                .indices
                .iter()
                .all(|&index| (index as usize) < patch.positions.len())
        );
        assert!(
            build_natural_patch(
                owner.offset([0, 1, 0]).unwrap(),
                owner.offset([0, 1, 0]).unwrap().origin().unwrap(),
                &columns,
                &padded,
                8.0,
                true
            )
            .is_none()
        );
    }

    #[test]
    fn channel_geometry_is_retained_for_local_levels_flows_cavities_and_dry_drainage_edges() {
        let owner = SectionPos::new(-2, 0, -1);
        let origin = owner.origin().unwrap();
        let padded = vec![0; 18 * 18 * 18];
        for variant in 0..4 {
            let mut columns = columns();
            // A halo-only difference must also prevent an incompatible grid
            // from replacing the incoming channel's shared corner payload.
            let sample = &mut columns[9 * 18];
            match variant {
                0 => sample.water_level = Some(8.5),
                1 => sample.water_flow = [0.15, -0.25],
                2 => sample.river_cavity = Some([-4.0, 6.0]),
                _ => sample.water_level = None,
            }
            let patch = build_natural_patch(owner, origin, &columns, &padded, 8.0, true).unwrap();
            let expected = build_channel_patch(owner, origin, &columns, &padded).unwrap();
            assert_eq!(patch, expected, "variant {variant}");
            assert_eq!(patch.positions.len(), 16 * 16 * 4);
        }
    }

    #[test]
    fn flat_sea_optimization_preserves_independent_cave_reservoirs() {
        let mut columns = columns();
        for column in &mut columns {
            column.cave.floor = -25.0;
            column.cave.ceiling = -12.0;
            column.cave.water = Some(WaterColumn {
                level: -20.0,
                bed: -25.0,
                depth: 5.0,
            });
        }
        let padded = vec![0; 18 * 18 * 18];
        let upper = SectionPos::new(-2, 0, -1);
        let sea = build_natural_patch(upper, upper.origin().unwrap(), &columns, &padded, 8.0, true)
            .unwrap();
        assert_eq!(
            sea,
            build_channel_patch(upper, upper.origin().unwrap(), &columns, &padded).unwrap()
        );
        let lower = SectionPos::new(-2, -2, -1);
        let underground =
            build_natural_patch(lower, lower.origin().unwrap(), &columns, &padded, 8.0, true)
                .unwrap();
        assert_eq!(underground.positions.len(), 16 * 16 * 4);
        assert!(
            underground
                .positions
                .iter()
                .all(|p| p[1] + lower.origin().unwrap()[1] as f32 == -20.0)
        );
        assert!(
            build_natural_patch(
                SectionPos::new(-2, -1, -1),
                [-32, -16, -16],
                &columns,
                &padded,
                8.0,
                true
            )
            .is_none()
        );
    }
}
mod fence;
