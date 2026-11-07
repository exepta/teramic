//! Rail modules have one persistent anchor and disjoint, owned voxel slices.
//!
//! Object metadata is the saved source of truth. The route graph is derived
//! from complete modules, including journal sections that are not loaded.
use crate::edits::{BlockObjectState, EditJournal};
use std::collections::BTreeMap;
use terra_core::{block_shapes::ShapeBox, rails::Piece};

#[cfg(feature = "bevy")]
pub(crate) mod placement;
#[cfg(feature = "bevy")]
pub use placement::{
    break_layout, commit, environment, placement, plan, proposal, support_edit_allowed,
    toggle_switch_plan,
};

pub type Cell = [i64; 3];

pub(crate) fn valid_state(cell: Cell, material: u16, state: BlockObjectState) -> bool {
    let Some(anchor) = state.peer else {
        return false;
    };
    if cell
        .iter()
        .chain(anchor.iter())
        .any(|value| value.unsigned_abs() > 1_000_000)
        || state.secondary != (cell != anchor)
    {
        return false;
    }
    let Some(_) = Piece::from_facing(anchor, material, state.facing) else {
        return false;
    };
    let mut offset = [0; 3];
    for axis in 0..3 {
        let Some(value) = cell[axis]
            .checked_sub(anchor[axis])
            .and_then(|value| i32::try_from(value).ok())
        else {
            return false;
        };
        offset[axis] = value;
    }
    terra_core::rails::contains_offset(material, state.facing, offset)
}

/// Decode a saved slice without requiring its anchor section to be loaded.
pub fn piece_at(journal: &EditJournal, cell: Cell) -> Option<Piece> {
    let material = journal.block(cell)?;
    let state = journal.object_state(cell)?;
    if !state.valid_for(cell, material) {
        return None;
    }
    Piece::from_facing(state.peer?, material, state.facing)
}

/// Resolve an owned slice to its canonical, matching anchor metadata.
pub fn owner_at(journal: &EditJournal, cell: Cell) -> Option<Piece> {
    let piece = piece_at(journal, cell)?;
    let state = journal.object_state(piece.anchor)?;
    (journal.block(piece.anchor) == Some(piece.material)
        && state == object_state(piece, piece.anchor))
    .then_some(piece)
}

pub fn object_state(piece: Piece, cell: Cell) -> BlockObjectState {
    BlockObjectState {
        facing: piece.encoded_facing(),
        peer: Some(piece.anchor),
        secondary: cell != piece.anchor,
        ..Default::default()
    }
}

pub fn cells(piece: Piece) -> Vec<Cell> {
    piece
        .footprint()
        .iter()
        .map(|offset| std::array::from_fn(|axis| piece.anchor[axis] + i64::from(offset[axis])))
        .collect()
}

/// Partial imports cannot create traversable routes or a second module item.
pub fn complete(journal: &EditJournal, piece: Piece) -> bool {
    cells(piece).into_iter().all(|cell| {
        journal.block(cell) == Some(piece.material)
            && journal.object_state(cell) == Some(object_state(piece, cell))
    })
}

/// Cell-local authored solids, also usable while the canonical section is absent.
pub fn cell_boxes(cell: Cell, material: u16, state: BlockObjectState) -> &'static [ShapeBox] {
    let Some(anchor) = state.peer else {
        return &[];
    };
    let Some(piece) = Piece::from_facing(anchor, material, state.facing) else {
        return &[];
    };
    let mut offset = [0; 3];
    for axis in 0..3 {
        let Some(value) = cell[axis]
            .checked_sub(anchor[axis])
            .and_then(|value| i32::try_from(value).ok())
        else {
            return &[];
        };
        offset[axis] = value;
    }
    piece.cell_boxes(offset)
}

/// A reusable derived graph. Unrelated terrain edits do not rebuild its geometry.
#[derive(Default)]
pub struct NetworkCache {
    epoch: Option<u64>,
    identity: Option<std::sync::Arc<()>>,
    pieces: BTreeMap<Cell, Piece>,
    network: terra_core::rails::RouteNetwork,
}

impl NetworkCache {
    pub fn refresh(&mut self, journal: &EditJournal) -> Result<bool, terra_core::rails::RailError> {
        let identity = journal.rail_cache_identity();
        if self.epoch == Some(journal.rail_epoch())
            && self
                .identity
                .as_ref()
                .is_some_and(|previous| std::sync::Arc::ptr_eq(previous, &identity))
        {
            return Ok(false);
        }
        let pieces: BTreeMap<_, _> = saved_pieces(journal)
            .take(terra_core::rails::MAX_PIECES + 1)
            .map(|piece| (piece.anchor, piece))
            .collect();
        if self.epoch.is_some() && self.pieces == pieces {
            self.epoch = Some(journal.rail_epoch());
            self.identity = Some(identity);
            return Ok(false);
        }
        self.network = terra_core::rails::RouteNetwork::from_pieces(pieces.values().copied())?;
        self.pieces = pieces;
        self.epoch = Some(journal.rail_epoch());
        self.identity = Some(identity);
        Ok(true)
    }

    pub fn graph(&self) -> &terra_core::rails::RouteNetwork {
        &self.network
    }
}

/// Rebuild persisted routes without depending on streaming interest or load order.
pub fn network(
    journal: &EditJournal,
) -> Result<terra_core::rails::RouteNetwork, terra_core::rails::RailError> {
    let pieces: BTreeMap<_, _> = saved_pieces(journal)
        .take(terra_core::rails::MAX_PIECES + 1)
        .map(|piece| (piece.anchor, piece))
        .collect();
    terra_core::rails::RouteNetwork::from_pieces(pieces.into_values())
}

fn saved_pieces(journal: &EditJournal) -> impl Iterator<Item = Piece> + '_ {
    journal.rail_sections().flat_map(|section| {
        journal
            .object_states_in_section(section)
            .filter_map(|(cell, material, state)| {
                (terra_core::rails::is_rail(material) && !state.secondary)
                    .then(|| owner_at(journal, cell))
                    .flatten()
                    .filter(|piece| complete(journal, *piece))
            })
    })
}

#[cfg(all(test, feature = "bevy"))]
mod tests;
