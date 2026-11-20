use super::FluidCell;
use crate::{TerraGenerator, edits::EditJournal, streaming::ChunkStore};

/// Bodies inherit forty percent of the local current, not its full travel speed.
pub const CURRENT_RESPONSE: f32 = 0.4;
pub const CURRENT_SPEED: f32 = 2.4;
pub const RIVER_CURRENT_SCALE: f32 = 1.2;

#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct FluidMotion {
    pub velocity: [f32; 3],
    pub immersion: f32,
}

/// Sample actual loaded liquid occupancy, including edited barriers and sources.
/// Cameras and visual wave displacement never participate in gameplay forces.
pub fn fluid_motion_at(
    terrain: &TerraGenerator,
    chunks: &ChunkStore,
    journal: &EditJournal,
    position: [f64; 3],
) -> FluidMotion {
    let _scope = terra_core::profiling::ScopeTimer::new("water.motion");
    if position
        .iter()
        .any(|v| !v.is_finite() || v.abs() > 1_000_000.0)
    {
        return FluidMotion::default();
    }
    let global = position.map(|v| v.floor() as i64);
    let Some(block) = chunks.block_at(global) else {
        return FluidMotion::default();
    };
    if block == terra_core::plumbing::BASIN {
        return crate::plumbing::basin_water_column(chunks, journal, position).map_or_else(
            FluidMotion::default,
            |water| FluidMotion {
                velocity: [0.0; 3],
                immersion: ((water.level - position[1]) / 0.35).clamp(0.0, 1.0) as f32,
            },
        );
    }
    let cell = chunks.fluid_cell_at(terrain, journal, global, block);
    if (super::space::partial(block) || journal.has_extended_kinetics())
        && !super::space::FluidSpace::wet_at(
            journal,
            global,
            |p| chunks.block_at(p),
            |p, material| chunks.raw_fluid_cell_at(terrain, journal, p, material),
        )
        .contains(
            cell,
            std::array::from_fn(|i| position[i] - global[i] as f64),
        )
    {
        return FluidMotion::default();
    }
    let Some(surface) = super::flowing_surface_height(chunks, terrain, journal, position) else {
        return FluidMotion::default();
    };
    if cell.amount == 0 || position[1] >= surface {
        return FluidMotion::default();
    }
    let immersion = ((surface - position[1]) / 0.35).clamp(0.0, 1.0) as f32;
    let surface_water = chunks
        .surface_column_at(global[0], global[2])
        .is_none_or(|column| position[1] >= column.water_bed());
    let river = surface_water
        .then(|| terrain.river_at(position[0], position[2]))
        .flatten()
        .filter(|river| {
            river.distance <= river.width + 1.0
                && river.level > terrain.config().sea_level
                && (global[1] as f64) < river.level + 0.01
        });
    let flow = if cell.flow != [0, 0] {
        cell.flow.map(f32::from)
    } else {
        river.map_or_else(
            || {
                chunks
                    .surface_column_at(global[0], global[2])
                    .filter(|column| {
                        column.cave.water.is_some_and(|water| {
                            position[1] >= water.bed && position[1] < water.level
                        })
                    })
                    .map_or([0.0; 2], |column| column.cave.river_flow(position[0]))
            },
            |river| river.flow,
        )
    };
    let length = flow[0].hypot(flow[1]);
    let river_current = river.is_some()
        || chunks
            .surface_column_at(global[0], global[2])
            .is_some_and(|column| {
                column
                    .cave
                    .water
                    .is_some_and(|water| position[1] >= water.bed && position[1] < water.level)
                    && column.cave.river_flow(position[0]) != [0.0; 2]
            });
    let horizontal = if length > 0.0 {
        let speed = CURRENT_SPEED
            * if river_current {
                RIVER_CURRENT_SCALE
            } else {
                1.0
            };
        flow.map(|component| component / length * speed)
    } else {
        [0.0; 2]
    };
    let falling_cell = falling_fluid_at(terrain, chunks, journal, global);
    FluidMotion {
        velocity: [
            horizontal[0],
            if falling_cell { -3.2 } else { 0.0 },
            horizontal[1],
        ],
        immersion,
    }
}

/// Gravity flow is actual connected liquid, never a procedural cliff projection.
pub fn falling_fluid_at(
    terrain: &TerraGenerator,
    chunks: &ChunkStore,
    journal: &EditJournal,
    global: [i64; 3],
) -> bool {
    let Some(block) = chunks.block_at(global) else {
        return false;
    };
    let cell = chunks.fluid_cell_at(terrain, journal, global, block);
    if cell.amount == 0 || cell.source || cell.flow != [0, 0] {
        return false;
    }
    let Some(down) = super::fluid::adjacent(global, [0, -1, 0]) else {
        return false;
    };
    let Some(below_block) = chunks.block_at(down) else {
        return false;
    };
    if !super::can_hold_fluid(below_block) {
        return false;
    }
    let current = super::space::FluidSpace::at(journal, global, |p| chunks.block_at(p));
    let below = super::space::FluidSpace::at(journal, down, |p| chunks.block_at(p));
    if !current.connected(
        cell,
        &below,
        FluidCell {
            amount: 8,
            ..Default::default()
        },
        [0, -1, 0],
    ) {
        return false;
    }
    if chunks
        .fluid_cell_at(terrain, journal, down, below_block)
        .amount
        < super::FLUID_CAPACITY
    {
        return true;
    }
    let Some(above) = super::fluid::adjacent(global, [0, 1, 0]) else {
        return false;
    };
    let Some(above_block) = chunks.block_at(above) else {
        return false;
    };
    let supplied = chunks.fluid_cell_at(terrain, journal, above, above_block);
    let connected = supplied.amount > 0
        && current.connected(
            FluidCell { amount: 8, ..cell },
            &super::space::FluidSpace::at(journal, above, |p| chunks.block_at(p)),
            supplied,
            [0, 1, 0],
        );
    if cell.amount < 8 && connected {
        return true;
    }
    connected
        && [[1, 0, 0], [-1, 0, 0], [0, 0, 1], [0, 0, -1]]
            .into_iter()
            .any(|direction| {
                exposed_falling_edge(terrain, chunks, journal, global, &current, cell, direction)
            })
}

fn exposed_falling_edge(
    terrain: &TerraGenerator,
    chunks: &ChunkStore,
    journal: &EditJournal,
    global: [i64; 3],
    current: &super::space::FluidSpace,
    cell: FluidCell,
    direction: [i64; 3],
) -> bool {
    let Some(side) = super::fluid::adjacent(global, direction) else {
        return false;
    };
    let Some(side_block) = chunks
        .block_at(side)
        .filter(|block| super::can_hold_fluid(*block))
    else {
        return false;
    };
    if chunks
        .fluid_cell_at(terrain, journal, side, side_block)
        .amount
        >= cell.amount
    {
        return false;
    }
    let full = FluidCell {
        amount: super::FLUID_CAPACITY,
        ..Default::default()
    };
    let side_space = super::space::FluidSpace::at(journal, side, |p| chunks.block_at(p));
    if !current.connected(cell, &side_space, full, direction) {
        return false;
    }
    let Some(down) = super::fluid::adjacent(side, [0, -1, 0]) else {
        return false;
    };
    let Some(block) = chunks
        .block_at(down)
        .filter(|block| super::can_hold_fluid(*block))
    else {
        return false;
    };
    // Water above a full cell is also normal in a deep, sleeping pool. A
    // supplied full shaft needs a real spill edge with room to fall beside it;
    // solid basin walls and equally filled neighbors are not downward flow.
    chunks.fluid_cell_at(terrain, journal, down, block).amount < super::FLUID_CAPACITY
        && side_space.connected(
            full,
            &super::space::FluidSpace::at(journal, down, |p| chunks.block_at(p)),
            full,
            [0, -1, 0],
        )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{streaming::LoadedSection, water::FluidCell};
    use terra_core::WorldConfig;
    use terra_voxel::{BlockSection, MaterialId, SectionPos};

    #[test]
    fn fractional_vertical_jet_shares_its_rendered_width_and_full_height_with_immersion() {
        let terrain = TerraGenerator::new(WorldConfig {
            sea_level: -100.,
            rivers: false,
            ..Default::default()
        })
        .unwrap();
        let owner = SectionPos::default();
        let mut chunks = ChunkStore::default();
        chunks.loaded.insert(
            owner,
            LoadedSection {
                data: BlockSection::empty(owner).unwrap(),
                mesh: None,
                mesh_epoch: 0,
                water_epoch: 0,
                water: None,
                props: vec![],
            },
        );
        let mut journal = EditJournal::default();
        for y in 4..=6 {
            journal
                .set_fluid(
                    [8, y, 8],
                    FluidCell {
                        amount: 1,
                        source: y == 6,
                        ..Default::default()
                    },
                )
                .unwrap();
        }
        let center = fluid_motion_at(&terrain, &chunks, &journal, [8.5, 5.75, 8.5]);
        assert!(center.immersion > 0. && center.velocity[1] < 0.);
        assert_eq!(
            fluid_motion_at(&terrain, &chunks, &journal, [8.05, 5.75, 8.5]),
            FluidMotion::default(),
            "the dry part outside the narrow stream stays dry"
        );
        assert_eq!(
            super::super::flowing_surface_height(&chunks, &terrain, &journal, [8.5, 5.75, 8.5]),
            Some(6.)
        );
    }

    #[test]
    fn river_current_scale_is_twenty_percent_stronger_without_changing_canal_baseline() {
        assert!((CURRENT_SPEED * RIVER_CURRENT_SCALE - 2.88).abs() < 0.0001);
        assert!((CURRENT_RESPONSE * CURRENT_SPEED - 0.96).abs() < 0.0001);
    }

    #[test]
    fn full_exposed_waterfall_has_real_downward_current_but_a_deep_calm_pool_does_not() {
        let terrain = TerraGenerator::new(WorldConfig {
            water: true,
            rivers: false,
            sea_level: -100.0,
            ..Default::default()
        })
        .unwrap();
        let owner = SectionPos::default();
        let mut data = BlockSection::empty(owner).unwrap();
        for x in 0..16 {
            for z in 0..16 {
                data.set_block([x, 2, z], 3).unwrap();
            }
        }
        for x in 2..=12 {
            for z in 2..=12 {
                if x == 2 || x == 12 || z == 2 || z == 12 {
                    for y in 3..=7 {
                        data.set_block([x, y, z], 3).unwrap();
                    }
                }
            }
        }
        let mut chunks = ChunkStore::default();
        chunks.loaded.insert(
            owner,
            LoadedSection {
                data,
                mesh: None,
                mesh_epoch: 0,
                water_epoch: 0,
                water: None,
                props: vec![],
            },
        );
        let mut journal = EditJournal::default();
        for x in 3..12 {
            for z in 3..12 {
                for y in 3..=7 {
                    journal
                        .set_fluid(
                            [x, y, z],
                            FluidCell {
                                amount: if y == 7 { 4 } else { 8 },
                                ..Default::default()
                            },
                        )
                        .unwrap();
                }
            }
        }
        for y in 3..=11 {
            journal
                .set_fluid(
                    [14, y, 7],
                    FluidCell {
                        amount: 8,
                        source: y == 11,
                        ..Default::default()
                    },
                )
                .unwrap();
        }
        for y in 3..=7 {
            for [x, z] in [[3, 3], [7, 7], [11, 7]] {
                let position = [x as f64 + 0.5, y as f64 + 0.25, z as f64 + 0.5];
                assert!(!falling_fluid_at(&terrain, &chunks, &journal, [x, y, z]));
                assert_eq!(
                    fluid_motion_at(&terrain, &chunks, &journal, position).velocity[1],
                    0.0
                );
            }
        }
        assert!(falling_fluid_at(&terrain, &chunks, &journal, [14, 6, 7]));
        assert_eq!(
            fluid_motion_at(&terrain, &chunks, &journal, [14.5, 6.5, 7.5]).velocity[1],
            -3.2
        );
        // Closing all side spill edges turns this into a full supported column,
        // not a perpetual suction force merely because its source is above.
        for [x, z] in [[13, 7], [15, 7], [14, 6], [14, 8]] {
            for y in 3..=10 {
                chunks
                    .loaded
                    .get_mut(&owner)
                    .unwrap()
                    .data
                    .set_block([x, y, z], 3)
                    .unwrap();
            }
        }
        assert!(!falling_fluid_at(&terrain, &chunks, &journal, [14, 6, 7]));
        journal
            .set_fluid(
                [14, 5, 7],
                FluidCell {
                    amount: 4,
                    ..Default::default()
                },
            )
            .unwrap();
        assert!(falling_fluid_at(&terrain, &chunks, &journal, [14, 6, 7]));
    }

    #[test]
    fn actual_liquid_level_barriers_and_flow_determine_current() {
        let terrain = TerraGenerator::new(WorldConfig {
            water: true,
            rivers: false,
            sea_level: -100.0,
            ..Default::default()
        })
        .unwrap();
        let owner = SectionPos::new(-1, 0, -1);
        let mut chunks = ChunkStore::default();
        chunks.loaded.insert(
            owner,
            LoadedSection {
                data: BlockSection::empty(owner).unwrap(),
                mesh: None,
                mesh_epoch: 0,
                water_epoch: 0,
                water: None,
                props: vec![],
            },
        );
        let mut journal = EditJournal::default();
        let p = [-1, 7, -1];
        journal
            .set_fluid(
                p,
                FluidCell {
                    amount: 4,
                    flow: [1, 1],
                    source: false,
                },
            )
            .unwrap();
        let sample = fluid_motion_at(&terrain, &chunks, &journal, [-0.5, 7.1, -0.5]);
        assert!((sample.velocity[0].hypot(sample.velocity[2]) - CURRENT_SPEED).abs() < 0.0001);
        assert_eq!(sample.immersion, 1.0);
        assert!((CURRENT_RESPONSE * CURRENT_SPEED - 0.96).abs() < 0.0001);
        assert_eq!(
            fluid_motion_at(&terrain, &chunks, &journal, [-0.5, 7.6, -0.5]),
            FluidMotion::default()
        );
        chunks
            .loaded
            .get_mut(&owner)
            .unwrap()
            .data
            .set_block([15, 7, 15], MaterialId::SolidWater as u16)
            .unwrap();
        assert_eq!(
            fluid_motion_at(&terrain, &chunks, &journal, [-0.5, 7.1, -0.5]),
            FluidMotion::default()
        );
        assert_eq!(
            fluid_motion_at(&terrain, &chunks, &journal, [f64::NAN, 7.1, -0.5]),
            FluidMotion::default()
        );
    }
}
