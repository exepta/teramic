//! Rail modules have one persistent anchor and disjoint, owned voxel slices.
//!
//! Object metadata is the saved source of truth. The route graph is derived
//! from complete modules, including journal sections that are not loaded.
use crate::edits::{BlockObjectState, EditJournal};
use std::collections::BTreeMap;
use std::sync::Arc;
use terra_core::{block_shapes::ShapeBox, rails::Piece};

#[cfg(feature = "bevy")]
mod clearance;
#[cfg(feature = "bevy")]
pub(crate) mod placement;
#[cfg(feature = "bevy")]
pub use clearance::{MAX_CLEARED_PROPS, PropClearance, PropSource, clearance};
#[cfg(feature = "bevy")]
pub use placement::{
    break_layout, commit, connection, connection_sites, connection_target, cost, environment,
    placement, placement_sites, plan, proposal, proposal_connection, select_endpoint, set_wood,
    support_edit_allowed, toggle_switch_plan,
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
    let Some(piece) = piece_from_state(cell, material, state) else {
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
    piece.contains_offset(offset)
}

/// Decode the geometry carried by every owned slice, including free curves.
pub fn piece_from_state(cell: Cell, material: u16, state: BlockObjectState) -> Option<Piece> {
    let anchor = state.peer.unwrap_or(cell);
    let piece = Piece::from_facing(anchor, material, state.facing)?;
    if let Some(curve) = state.rail_curve {
        piece.with_curve(curve)
    } else {
        Some(piece)
    }
}

/// Decode a saved slice without requiring its anchor section to be loaded.
pub fn piece_at(journal: &EditJournal, cell: Cell) -> Option<Piece> {
    let material = journal.block(cell)?;
    let state = journal.object_state(cell)?;
    if !state.valid_for(cell, material) {
        return None;
    }
    piece_from_state(cell, material, state)
}

/// Resolve an owned slice to its canonical, matching anchor metadata.
pub fn owner_at(journal: &EditJournal, cell: Cell) -> Option<Piece> {
    let piece = piece_at(journal, cell)?;
    let state = journal.object_state(piece.anchor)?;
    (journal.block(piece.anchor) == Some(piece.material)
        && state == object_state_with_wood(piece, piece.anchor, state.wood))
    .then_some(piece)
}

pub fn object_state(piece: Piece, cell: Cell) -> BlockObjectState {
    object_state_with_wood(piece, cell, terra_core::WoodSpecies::Oak)
}

pub fn object_state_with_wood(
    piece: Piece,
    cell: Cell,
    wood: terra_core::WoodSpecies,
) -> BlockObjectState {
    BlockObjectState {
        facing: piece.encoded_facing(),
        peer: Some(piece.anchor),
        secondary: cell != piece.anchor,
        wood,
        rail_curve: piece.curve,
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
    let Some(anchor_state) = journal.object_state(piece.anchor) else {
        return false;
    };
    cells(piece).into_iter().all(|cell| {
        journal.block(cell) == Some(piece.material)
            && journal.object_state(cell)
                == Some(object_state_with_wood(piece, cell, anchor_state.wood))
    })
}

/// Cell-local authored solids, also usable while the canonical section is absent.
pub fn cell_boxes(cell: Cell, material: u16, state: BlockObjectState) -> Arc<[ShapeBox]> {
    if state.peer.is_none() {
        return Arc::default();
    }
    let Some(piece) = piece_from_state(cell, material, state) else {
        return Arc::default();
    };
    let mut offset = [0; 3];
    for axis in 0..3 {
        let Some(value) = cell[axis]
            .checked_sub(piece.anchor[axis])
            .and_then(|value| i32::try_from(value).ok())
        else {
            return Arc::default();
        };
        offset[axis] = value;
    }
    piece.cell_boxes(offset)
}

/// Construction and dismantling use the same four-metre item unit.
pub fn piece_cost(piece: Piece) -> u16 {
    let length: f64 = piece.paths().iter().map(|path| path.length()).sum();
    (length / 4.0).ceil().clamp(1.0, f64::from(u16::MAX)) as u16
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
