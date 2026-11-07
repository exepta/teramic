//! A bounded proof distinguishes tiny closed pools from replenishing open water.
use crate::{
    TerraGenerator,
    edits::EditJournal,
    streaming::ChunkStore,
    water::{FluidCell, space::FluidSpace},
};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use terra_core::plumbing::{Cell, DIRECTIONS};
use terra_voxel::global_to_section;

pub const REPLENISHING_BODY_CELLS: usize = 27;

#[derive(Debug, Clone, PartialEq)]
pub(super) enum Body {
    Dry,
    Replenishing,
    Finite(Vec<(Cell, FluidCell)>),
}

/// Fewer than 27 connected wet cells must be fully known and isolated before
/// pumping may debit them. Unknown boundaries cannot prove a closed pool.
pub(super) fn classify(
    chunks: &ChunkStore,
    journal: &EditJournal,
    terrain: &TerraGenerator,
    source: Cell,
) -> Body {
    let Some(block) = chunks.block_at(source) else {
        return Body::Dry;
    };
    let first = chunks.fluid_cell_at(terrain, journal, source, block);
    if first.amount == 0 {
        return Body::Dry;
    }
    let mut seen = BTreeSet::from([source]);
    let mut queue = VecDeque::from([(source, first)]);
    let mut finite = vec![];
    while let Some((cell, water)) = queue.pop_front() {
        finite.push((cell, water));
        let space = FluidSpace::at(journal, cell, |p| chunks.block_at(p));
        for direction in DIRECTIONS {
            let adjacent: Cell = std::array::from_fn(|axis| cell[axis] + direction[axis]);
            if seen.contains(&adjacent) {
                continue;
            }
            let Some(block) = chunks.block_at(adjacent) else {
                if space.connected(
                    water,
                    &FluidSpace { solids: vec![] },
                    FluidCell {
                        amount: 8,
                        ..Default::default()
                    },
                    direction,
                ) {
                    return Body::Replenishing;
                }
                continue;
            };
            if !crate::water::can_hold_fluid(block) {
                continue;
            }
            let other = chunks.fluid_cell_at(terrain, journal, adjacent, block);
            if other.amount == 0 {
                continue;
            }
            let neighbor = FluidSpace::at(journal, adjacent, |p| chunks.block_at(p));
            if !space.connected(water, &neighbor, other, direction) {
                continue;
            }
            seen.insert(adjacent);
            if seen.len() >= REPLENISHING_BODY_CELLS {
                return Body::Replenishing;
            }
            queue.push_back((adjacent, other));
        }
    }
    Body::Finite(finite)
}

/// Take finite quantity from the highest shared layer, avoiding a local suction
/// pit. All source flags in a proven small pool become finite together so a
/// remaining placed source cannot regenerate the removed units.
pub(super) fn take(
    chunks: &ChunkStore,
    journal: &mut EditJournal,
    terrain: &TerraGenerator,
    source: Cell,
    maximum: u8,
) -> Result<u8, String> {
    if maximum == 0 {
        return Ok(0);
    }
    let mut cells = match classify(chunks, journal, terrain, source) {
        Body::Dry => return Ok(0),
        Body::Replenishing => return Ok(maximum),
        Body::Finite(cells) => cells,
    };
    let available: u16 = cells.iter().map(|(_, cell)| u16::from(cell.amount)).sum();
    let taken = maximum.min(available.min(8) as u8);
    for (_, cell) in &mut cells {
        cell.source = false;
        cell.flow = [0; 2];
    }
    for _ in 0..taken {
        let index = cells
            .iter()
            .enumerate()
            .filter(|(_, (_, cell))| cell.amount > 0)
            .max_by_key(|(_, (p, cell))| p[1] * 8 + i64::from(cell.amount))
            .map(|(index, _)| index)
            .unwrap();
        cells[index].1.amount -= 1;
    }
    // Preflight the complete pool before publishing either water or inlet units.
    let mut fluid_epochs = BTreeMap::<_, u64>::new();
    let mut mesh_epochs = BTreeMap::<_, u64>::new();
    for &(cell, water) in &cells {
        if journal.fluid(cell) == Some(water) {
            continue;
        }
        let (owner, local) = global_to_section(cell);
        *fluid_epochs.entry(owner).or_default() += 1;
        let offsets = local.map(|v| {
            if v == 0 {
                -1
            } else if v == 15 {
                1
            } else {
                0
            }
        });
        let mut affected = BTreeSet::from([owner]);
        for x in [0, offsets[0]] {
            for y in [0, offsets[1]] {
                for z in [0, offsets[2]] {
                    affected.insert(owner.offset([x, y, z]).map_err(|error| error.to_string())?);
                }
            }
        }
        for owner in affected {
            *mesh_epochs.entry(owner).or_default() += 1;
        }
    }
    if fluid_epochs
        .iter()
        .any(|(&section, &count)| journal.fluid_epoch(section).checked_add(count).is_none())
        || mesh_epochs.iter().any(|(&section, &count)| {
            journal
                .water_mesh_epoch(section)
                .checked_add(count)
                .is_none()
        })
    {
        return Err("Intake water epoch exhausted".into());
    }
    for (cell, water) in cells {
        journal
            .set_fluid(cell, water)
            .map_err(|error| error.to_string())?;
    }
    Ok(taken)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{edits::BlockObjectState, streaming::LoadedSection};
    use terra_core::plumbing::{self as core, BASIN};
    use terra_voxel::BlockSection;

    fn fixture() -> (ChunkStore, EditJournal, TerraGenerator) {
        let mut chunks = ChunkStore::default();
        for x in -1..=1 {
            for y in -1..=1 {
                for z in -1..=1 {
                    let position = terra_voxel::SectionPos::new(x, y, z);
                    chunks.loaded.insert(
                        position,
                        LoadedSection {
                            data: BlockSection::empty(position).unwrap(),
                            mesh: None,
                            mesh_epoch: 0,
                            water_epoch: 0,
                            water: None,
                            props: vec![],
                        },
                    );
                }
            }
        }
        let terrain = TerraGenerator::new(terra_core::WorldConfig {
            base_height: -100.,
            sea_level: -200.,
            hill_amplitude: 0.,
            mountain_amplitude: 0.,
            caves: false,
            rivers: false,
            biomes: false,
            ..Default::default()
        })
        .unwrap();
        (chunks, EditJournal::default(), terrain)
    }
    fn wet(journal: &mut EditJournal, cell: Cell) {
        journal
            .set_fluid(
                cell,
                FluidCell {
                    amount: 8,
                    source: true,
                    ..Default::default()
                },
            )
            .unwrap();
    }
    fn units(journal: &EditJournal) -> u16 {
        journal
            .fluid_cells()
            .map(|(_, cell)| u16::from(cell.amount))
            .sum()
    }

    #[test]
    fn twenty_six_closed_cells_are_finite_but_twenty_seven_connected_cells_replenish() {
        let (chunks, mut journal, terrain) = fixture();
        let source = [4, 4, 4];
        let pool: Vec<_> = (4..=6)
            .flat_map(|x| (4..=6).flat_map(move |y| (4..=6).map(move |z| [x, y, z])))
            .collect();
        for &p in pool.iter().take(26) {
            wet(&mut journal, p);
        }
        assert!(
            matches!(classify(&chunks,&journal,&terrain,source),Body::Finite(ref cells) if cells.len()==26)
        );
        let before = units(&journal);
        assert_eq!(take(&chunks, &mut journal, &terrain, source, 8).unwrap(), 8);
        assert_eq!(units(&journal), before - 8);
        assert!(journal.fluid_cells().all(|(_, cell)| !cell.source));
        // Refill the fixture and connect its last cell across the old boundary.
        for &p in &pool {
            wet(&mut journal, p);
        }
        assert_eq!(
            classify(&chunks, &journal, &terrain, source),
            Body::Replenishing
        );
        let before: Vec<_> = journal.fluid_cells().collect();
        for _ in 0..20 {
            assert_eq!(take(&chunks, &mut journal, &terrain, source, 8).unwrap(), 8);
        }
        assert_eq!(journal.fluid_cells().collect::<Vec<_>>(), before);
    }

    #[test]
    fn outside_connections_and_unloaded_boundaries_preserve_water_across_sections() {
        let (chunks, mut journal, terrain) = fixture();
        for x in 5..=31 {
            wet(&mut journal, [x, 4, 4]);
        }
        assert_eq!(
            classify(&chunks, &journal, &terrain, [15, 4, 4]),
            Body::Replenishing
        );
        let (mut isolated, mut small, terrain) = fixture();
        wet(&mut small, [15, 4, 4]);
        assert!(matches!(
            classify(&isolated, &small, &terrain, [15, 4, 4]),
            Body::Finite(_)
        ));
        isolated.loaded.remove(&global_to_section([16, 4, 4]).0);
        let before = small.fluid([15, 4, 4]);
        assert_eq!(
            classify(&isolated, &small, &terrain, [15, 4, 4]),
            Body::Replenishing
        );
        assert_eq!(
            take(&isolated, &mut small, &terrain, [15, 4, 4], 8).unwrap(),
            8
        );
        assert_eq!(small.fluid([15, 4, 4]), before);
    }

    #[test]
    fn tiny_pool_can_be_completely_drained_without_regenerating_or_using_tank_water() {
        let (mut chunks, mut journal, terrain) = fixture();
        let source = [4, 4, 4];
        wet(&mut journal, source);
        wet(&mut journal, [5, 4, 4]);
        let tank = [6, 4, 4];
        let (owner, local) = global_to_section(tank);
        chunks
            .loaded
            .get_mut(&owner)
            .unwrap()
            .data
            .set_block(local, BASIN)
            .unwrap();
        journal.record(tank, BASIN).unwrap();
        journal
            .set_object_state(tank, Some(BlockObjectState::default()))
            .unwrap();
        journal
            .set_plumbing(
                tank,
                super::super::PlumbingState {
                    water: FluidCell {
                        amount: 8,
                        ..Default::default()
                    },
                    ..Default::default()
                },
            )
            .unwrap();
        assert_eq!(take(&chunks, &mut journal, &terrain, source, 8).unwrap(), 8);
        let remaining = journal
            .fluid_cells()
            .find(|(_, cell)| cell.amount > 0)
            .unwrap()
            .0;
        assert_eq!(
            take(&chunks, &mut journal, &terrain, remaining, 8).unwrap(),
            8
        );
        assert_eq!(units(&journal), 0);
        assert_eq!(take(&chunks, &mut journal, &terrain, source, 8).unwrap(), 0);
        assert_eq!(
            journal.plumbing_at(tank).unwrap().water.amount,
            core::CELL_CAPACITY
        );
    }
    #[test]
    fn real_small_pool_with_all_source_cells_waterlogged_by_pipes_depletes_without_regenerating() {
        let (mut chunks, mut journal, terrain) = fixture();
        let positions = [[4, 4, 4], [5, 4, 4], [4, 4, 5], [5, 4, 5]];
        for x in 3..=6 {
            for y in 3..=5 {
                for z in 3..=6 {
                    let p = [x, y, z];
                    let (owner, local) = global_to_section(p);
                    chunks
                        .loaded
                        .get_mut(&owner)
                        .unwrap()
                        .data
                        .set_block(local, 3)
                        .unwrap();
                }
            }
        }
        let pipes = [
            core::WOOD_PIPE,
            core::STONE_PIPE,
            core::IRON_PIPE,
            core::GLASS_PIPE,
        ];
        for (p, material) in positions.into_iter().zip(pipes) {
            journal.record(p, 27).unwrap();
            journal.record(p, material).unwrap();
            journal
                .set_object_state(
                    p,
                    Some(BlockObjectState {
                        facing: 4,
                        ..Default::default()
                    }),
                )
                .unwrap();
            let (owner, local) = global_to_section(p);
            chunks
                .loaded
                .get_mut(&owner)
                .unwrap()
                .data
                .set_block(local, material)
                .unwrap();
            assert!(journal.fluid(p).unwrap().source);
        }
        assert!(
            matches!(classify(&chunks, &journal, &terrain, positions[0]), Body::Finite(ref cells) if cells.len() == 4)
        );
        assert_eq!(units(&journal), 32);
        assert_eq!(
            take(&chunks, &mut journal, &terrain, positions[0], 8).unwrap(),
            8
        );
        assert_eq!(units(&journal), 24);
        for p in positions {
            assert_eq!(
                journal.fluid(p),
                Some(FluidCell {
                    amount: 6,
                    source: false,
                    flow: [0; 2]
                })
            );
        }
        for _ in 0..32 {
            crate::water::simulate_fluid_step(&terrain, &chunks, &mut journal, 256).unwrap();
            assert!(journal.fluid_cells().all(|(_, water)| !water.source));
            assert!(units(&journal) <= 24);
        }
        assert_eq!(units(&journal), 0);
    }
}
