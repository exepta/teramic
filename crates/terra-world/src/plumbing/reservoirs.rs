use crate::{edits::EditJournal, streaming::ChunkStore};
use std::collections::{BTreeSet, VecDeque};
use terra_core::plumbing::{BASIN, BasinShape, Cell, DIRECTIONS, MAX_BASIN};

/// Only complete, loaded rectangular reservoirs remove their interior walls.
pub fn basin_shape(chunks: &ChunkStore, journal: &EditJournal, start: Cell) -> Option<BasinShape> {
    if chunks.block_at(start) != Some(BASIN) {
        return None;
    }
    let mut cells = BTreeSet::from([start]);
    let mut queue = VecDeque::from([start]);
    while let Some(cell) = queue.pop_front() {
        for direction in DIRECTIONS {
            let next = std::array::from_fn(|axis| cell[axis] + direction[axis]);
            let material = chunks.block_at(next)?;
            if material == BASIN
                && crate::kingdom_access::can_transport(chunks, journal, cell, next)
                && cells.insert(next)
            {
                if cells.len() > MAX_BASIN {
                    return None;
                }
                queue.push_back(next);
            }
        }
    }
    let low = std::array::from_fn(|axis| cells.iter().map(|cell| cell[axis]).min().unwrap());
    let high: Cell = std::array::from_fn(|axis| cells.iter().map(|cell| cell[axis]).max().unwrap());
    let size: [u8; 3] =
        std::array::from_fn(|axis| u8::try_from(high[axis] - low[axis] + 1).unwrap_or(255));
    let shape = BasinShape { origin: low, size };
    (shape.valid()
        && shape.cells().count() == cells.len()
        && shape.cells().all(|cell| {
            cells.contains(&cell)
                && journal.object_state(cell).is_some()
                && DIRECTIONS.into_iter().all(|direction| {
                    let next = std::array::from_fn(|axis| cell[axis] + direction[axis]);
                    !shape.contains(next)
                        || crate::kingdom_access::can_transport(chunks, journal, cell, next)
                })
        }))
    .then_some(shape)
}
