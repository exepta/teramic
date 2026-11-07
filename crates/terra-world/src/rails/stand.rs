//! A nearby stand controls one complete switch; no persistent remote link exists.
use super::{Cell, cells, complete, owner_at};
use crate::edits::EditJournal;
use std::collections::BTreeMap;
use terra_core::rails::{Kind, Piece, SWITCH_STAND};

/// Three adjacent cells on the outside of the throat's former lever housing.
pub fn positions(piece: Piece) -> Vec<Cell> {
    if !piece.kind().is_some_and(Kind::is_switch) {
        return Vec::new();
    }
    let x = if piece.material == terra_core::rails::SWITCH_LEFT {
        3
    } else {
        -3
    };
    (0..=2)
        .map(|z| {
            let [x, z] = match piece.facing {
                0 => [x, z],
                1 => [z, -x],
                2 => [-x, -z],
                _ => [-z, x],
            };
            [piece.anchor[0] + x, piece.anchor[1], piece.anchor[2] + z]
        })
        .collect()
}

/// Ambiguous, stale, missing or incomplete neighboring switches fail closed.
pub fn switch_at(journal: &EditJournal, stand: Cell) -> Option<Piece> {
    if journal.block(stand) != Some(SWITCH_STAND) {
        return None;
    }
    let mut matches = BTreeMap::new();
    for material in [
        terra_core::rails::SWITCH_LEFT,
        terra_core::rails::SWITCH_RIGHT,
    ] {
        for facing in 0..4 {
            let prototype = Piece::new([0; 3], material, facing)?;
            for offset in positions(prototype) {
                let anchor = std::array::from_fn(|axis| stand[axis] - offset[axis]);
                if let Some(piece) = owner_at(journal, anchor)
                    && piece.material == material
                    && piece.facing == facing
                    && complete(journal, piece)
                    && !cells(piece).contains(&stand)
                {
                    matches.insert(piece.anchor, piece);
                }
            }
        }
    }
    (matches.len() == 1).then(|| *matches.values().next().expect("one local switch"))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn add(journal: &mut EditJournal, piece: Piece) {
        for cell in cells(piece) {
            journal.record(cell, piece.material).unwrap();
            journal
                .set_object_state(cell, Some(super::super::object_state(piece, cell)))
                .unwrap();
        }
    }
    #[test]
    fn local_stands_survive_reload_and_switch_changes_but_reject_remote_partial_or_ambiguous_links()
    {
        for material in [
            terra_core::rails::SWITCH_LEFT,
            terra_core::rails::SWITCH_RIGHT,
        ] {
            for facing in 0..4 {
                let piece = Piece::new([-17, 16, -17], material, facing).unwrap();
                let stand = positions(piece)[1];
                assert!(!cells(piece).contains(&stand));
                let mut journal = EditJournal::default();
                add(&mut journal, piece);
                journal.record(stand, SWITCH_STAND).unwrap();
                assert_eq!(switch_at(&journal, stand), Some(piece));
                let owners: std::collections::BTreeSet<_> = cells(piece)
                    .into_iter()
                    .chain([stand])
                    .map(|cell| terra_voxel::global_to_section(cell).0)
                    .collect();
                let mut replica = EditJournal::default();
                for owner in owners {
                    replica.import_delta(journal.section_delta(owner)).unwrap();
                }
                assert_eq!(switch_at(&replica, stand), Some(piece));
                let switched = Piece::new(piece.anchor, material, facing | 4).unwrap();
                add(&mut replica, switched);
                assert_eq!(switch_at(&replica, stand), Some(switched));
                replica.record(piece.anchor, 0).unwrap();
                assert_eq!(switch_at(&replica, stand), None);
                let remote = [stand[0] + 20, stand[1], stand[2]];
                journal.record(remote, SWITCH_STAND).unwrap();
                assert_eq!(switch_at(&journal, remote), None);
            }
        }
        let mut journal = EditJournal::default();
        let left = Piece::new([0; 3], terra_core::rails::SWITCH_LEFT, 0).unwrap();
        let right = Piece::new([6, 0, 0], terra_core::rails::SWITCH_RIGHT, 0).unwrap();
        add(&mut journal, left);
        add(&mut journal, right);
        let shared = positions(left)[1];
        assert!(positions(right).contains(&shared));
        journal.record(shared, SWITCH_STAND).unwrap();
        assert_eq!(switch_at(&journal, shared), None);
    }
}
