use crate::{TerraGenerator, edits::EditJournal, streaming::ChunkStore};

/// Bodies inherit forty percent of the local current, not its full travel speed.
pub const CURRENT_RESPONSE: f32 = 0.4;
pub const CURRENT_SPEED: f32 = 2.4;

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
    let cell = chunks.fluid_cell_at(terrain, journal, global, block);
    if super::space::partial(block)
        && !super::space::FluidSpace::at(journal, global, |p| chunks.block_at(p)).contains(
            cell,
            std::array::from_fn(|i| position[i] - global[i] as f64),
        )
    {
        return FluidMotion::default();
    }
    let surface = global[1] as f64 + cell.height();
    if cell.amount == 0 || position[1] >= surface {
        return falling_curtain(terrain, position, block, |x, z| {
            chunks.surface_column_at(x, z)
        });
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
    let horizontal = if length > 0.0 {
        flow.map(|component| component / length * CURRENT_SPEED)
    } else {
        [0.0; 2]
    };
    let below = super::fluid::adjacent(global, [0, -1, 0]);
    let falling_cell = !cell.source
        && cell.flow == [0, 0]
        && below.is_some_and(|below| {
            chunks.block_at(below).is_some_and(|block| {
                super::can_hold_fluid(block)
                    && super::fluid_cell_at(terrain, journal, below, block).amount
                        < super::FLUID_CAPACITY
            })
        });
    FluidMotion {
        velocity: [
            horizontal[0],
            if falling_cell
                || river.is_some_and(|river| river.cascade_height >= super::MIN_WATERFALL_DROP)
            {
                -3.2
            } else {
                0.0
            },
            horizontal[1],
        ],
        immersion,
    }
}

fn falling_curtain<'a>(
    terrain: &TerraGenerator,
    position: [f64; 3],
    block: u16,
    mut prepared: impl FnMut(i64, i64) -> Option<&'a crate::SurfaceColumn>,
) -> FluidMotion {
    let _scope = terra_core::profiling::ScopeTimer::new("water.falling_curtain");
    if !super::can_hold_fluid(block) {
        return FluidMotion::default();
    }
    let [x, y, z] = position;
    let river = terrain
        .river_at(x, z)
        .filter(|river| river.cascade_height >= super::MIN_WATERFALL_DROP);
    let [cx, cz] = [x.floor(), z.floor()];
    if river.is_none() {
        // Revision 3+ selects ecology per 192 m region, whose boundaries align
        // with voxel edges. Older revisions still sample their local climate.
        let cave_kind = (terrain.config().cave_structure_revision >= 3)
            .then(|| prepared(cx as i64, cz as i64))
            .flatten()
            .map_or_else(
                || terrain.cave_structure_at(x, z).kind,
                |column| column.cave.kind,
            );
        if cave_kind != crate::generation::CaveEcology::Waterfall {
            return FluidMotion::default();
        }
    }
    let mut columns = |x: f64, z: f64| {
        prepared(x as i64, z as i64).map_or_else(
            || terrain.water_columns(x + 0.5, z + 0.5),
            |column| column.water_columns(),
        )
    };
    let Some(lower) = columns(cx, cz).below(y) else {
        return FluidMotion::default();
    };
    for [dx, dz] in [[-1.0, 0.0], [1.0, 0.0], [0.0, -1.0], [0.0, 1.0]] {
        let Some(upper) = columns(cx + dx, cz + dz).below(y + 32.0) else {
            continue;
        };
        if upper.level - lower.level < super::MIN_WATERFALL_DROP
            || y < lower.level
            || y > upper.level
        {
            continue;
        }
        let progress = ((upper.level - y) / (upper.level - lower.level)).clamp(0.0, 1.0);
        let distance = if dx < 0.0 {
            x - cx
        } else if dx > 0.0 {
            cx + 1.0 - x
        } else if dz < 0.0 {
            z - cz
        } else {
            cz + 1.0 - z
        };
        // The same zero-ended projection envelope as channels::waterfall.
        let across = if dx != 0.0 {
            cx + if dx > 0.0 { 1.0 } else { 0.0 } + z
        } else {
            x + cz + if dz > 0.0 { 1.0 } else { 0.0 }
        };
        let corrugation =
            0.56 + 0.18 * (across * 1.73).sin() + 0.06 * (across * 3.61 + y * 0.91).sin();
        let projection =
            0.06 + 0.15 * progress + (progress * std::f64::consts::PI).sin() * corrugation;
        let immersion = (1.0 - ((distance - projection).abs() / 0.24)).clamp(0.0, 1.0) as f32;
        if immersion > 0.0 {
            return FluidMotion {
                velocity: [
                    river.map_or(-0.4, |r| r.flow[0]) * CURRENT_SPEED,
                    -3.2,
                    river.map_or(0.0, |r| r.flow[1]) * CURRENT_SPEED,
                ],
                immersion,
            };
        }
    }
    FluidMotion::default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{streaming::LoadedSection, water::FluidCell};
    use std::collections::HashMap;
    use terra_core::WorldConfig;
    use terra_voxel::{BlockSection, MaterialId, SectionPos};

    fn prepared_neighborhood(
        terrain: &TerraGenerator,
        cells: &[[i64; 2]],
    ) -> HashMap<[i64; 2], crate::SurfaceColumn> {
        let mut columns = HashMap::new();
        for &[x, z] in cells {
            for [dx, dz] in [[0, 0], [-1, 0], [1, 0], [0, -1], [0, 1]] {
                let p = [x + dx, z + dz];
                columns.entry(p).or_insert_with(|| {
                    let column = terrain.surface_column(p[0] as f64 + 0.5, p[1] as f64 + 0.5);
                    assert_eq!(
                        column.water_columns(),
                        terrain.water_columns(p[0] as f64 + 0.5, p[1] as f64 + 0.5)
                    );
                    column
                });
            }
        }
        columns
    }

    #[test]
    fn prepared_dry_current_matches_fallback_across_ecology_revisions_and_region_edges() {
        let cells = [
            [-193, -193],
            [-192, -192],
            [-1, -1],
            [0, 0],
            [191, 191],
            [192, 192],
            [16800, -17952],
        ];
        for revision in [0, 2, 3, 4] {
            for rivers in [false, true] {
                let terrain = TerraGenerator::new(WorldConfig {
                    cave_structure_revision: revision,
                    rivers,
                    ..Default::default()
                })
                .unwrap();
                let columns = prepared_neighborhood(&terrain, &cells);
                for [x, z] in cells {
                    for fraction in [0.01, 0.49, 0.99] {
                        let [px, pz] = [x as f64 + fraction, z as f64 + 1.0 - fraction];
                        if revision >= 3 {
                            assert_eq!(
                                columns[&[x, z]].cave.kind,
                                terrain.cave_structure_at(px, pz).kind
                            );
                        }
                        for y in [-100.0, -48.0, 8.0, 80.0, 361.0] {
                            let position = [px, y, pz];
                            let expected = falling_curtain(&terrain, position, 0, |_, _| None);
                            let all =
                                falling_curtain(&terrain, position, 0, |x, z| columns.get(&[x, z]));
                            let local = falling_curtain(&terrain, position, 0, |cx, cz| {
                                ([cx, cz] == [x, z]).then(|| &columns[&[x, z]])
                            });
                            assert_eq!(
                                all, expected,
                                "revision={revision} rivers={rivers} {position:?}"
                            );
                            assert_eq!(local, expected, "missing neighbors at {position:?}");
                            assert_eq!(
                                falling_curtain(&terrain, position, 0, |x, z| columns.get(&[x, z])),
                                all,
                                "prediction replay at {position:?}"
                            );
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn prepared_current_preserves_actual_legacy_waterfall_contact() {
        let terrain = TerraGenerator::new(WorldConfig {
            cave_structure_revision: 2,
            ..Default::default()
        })
        .unwrap();
        let room = terrain.cave_structure_at(-13344.0, -18336.0);
        let [cx, _, cz] = room.center;
        let (x, z, high, low) = (-22..=22)
            .find_map(|dz| {
                (-4..=10).find_map(|dx| {
                    let x = cx + f64::from(dx) + 0.5;
                    let z = cz + f64::from(dz) + 0.5;
                    let high = terrain.cave_structure_at(x, z).water?;
                    let low = terrain.cave_structure_at(x - 1.0, z).water?;
                    (high.level - low.level >= 8.0).then_some((x, z, high, low))
                })
            })
            .expect("legacy waterfall outlet");
        let cell = [x.floor() as i64 - 1, z.floor() as i64];
        let columns = prepared_neighborhood(&terrain, &[cell]);
        let mut contacts = 0;
        for fraction in [0.05, 0.25, 0.5, 0.75, 0.95] {
            let position = [cell[0] as f64 + fraction, (high.level + low.level) * 0.5, z];
            let expected = falling_curtain(&terrain, position, 0, |_, _| None);
            let actual = falling_curtain(&terrain, position, 0, |x, z| columns.get(&[x, z]));
            assert_eq!(actual, expected, "waterfall contact at {position:?}");
            contacts += usize::from(actual.immersion > 0.0);
            assert_eq!(
                falling_curtain(&terrain, position, MaterialId::Stone as u16, |x, z| columns
                    .get(&[x, z])),
                FluidMotion::default()
            );
        }
        assert!(
            contacts > 0,
            "waterfall fixture must include actual current contact"
        );
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
