use super::*;
use terra_core::containers::ChestMerge;
type Partner = ([i64; 3], i64, BlockObjectState);
#[cfg(test)]
mod tests;

/// Existing pairs are occupied neighbors, never a reason to reject a new chest.
/// Ambiguous automatic placement stays single; an explicit click can choose one.
pub(super) fn partner(
    chunks: &ChunkStore,
    journal: &EditJournal,
    position: [i64; 3],
    merge: ChestMerge,
) -> Result<Option<Partner>, String> {
    let eligible = |peer: [i64; 3]| {
        if chunks.block_at(peer) != Some(29) {
            return None;
        }
        let state = journal.object_state(peer).unwrap_or_default();
        if state.peer.is_some() || state.secondary || !state.valid_for(peer, 29) {
            return None;
        }
        let direction = std::array::from_fn::<_, 3, _>(|i| peer[i] - position[i]);
        let sign = direction
            .into_iter()
            .zip(state.right())
            .map(|(a, b)| a * b)
            .sum::<i64>();
        (sign.abs() == 1).then_some((peer, sign, state))
    };
    match merge {
        ChestMerge::Separate => Ok(None),
        ChestMerge::Target(peer) => {
            if peer.iter().any(|v| v.unsigned_abs() > 990_000)
                || peer[1] != position[1]
                || peer
                    .into_iter()
                    .zip(position)
                    .map(|(a, b)| a.abs_diff(b))
                    .sum::<u64>()
                    != 1
                || chunks.block_at(peer) != Some(29)
            {
                return Err("Chest connection needs an adjacent loaded chest".into());
            }
            Ok(eligible(peer))
        }
        ChestMerge::Auto => {
            let mut found = None;
            for direction in [[1, 0, 0], [-1, 0, 0], [0, 0, 1], [0, 0, -1]] {
                if let Some(candidate) = eligible(offset(position, direction, 1)) {
                    if found.is_some() {
                        return Ok(None);
                    }
                    found = Some(candidate);
                }
            }
            Ok(found)
        }
    }
}
