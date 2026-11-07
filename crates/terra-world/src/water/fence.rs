//! A fence separates its wet and dry sides inside a voxel as well as between voxels.
use super::FluidCell;
use crate::edits::EditJournal;
use std::collections::{HashSet, VecDeque};
use terra_core::{block_shapes::ShapeBox, construction};

/// Four horizontal regions meet at the post. Connected rails separate these
/// regions all the way to the adjoining post, including below/above the rails.
/// Follow only fence regions; ordinary water is an entry, never a shortcut
/// around a dry bank. No new persistent or replicated fluid format is needed.
pub(super) fn dry_regions(
    journal: &EditJournal,
    position: [i64; 3],
    mut block_at: impl FnMut([i64; 3]) -> Option<u16>,
    mut water_at: impl FnMut([i64; 3], u16) -> FluidCell,
) -> Vec<ShapeBox> {
    // A retained source means this fence was put into an already wet voxel.
    // Preserve that surrounding water even in an isolated one-cell basin.
    // Bank contact supplied from one side remains finite, so it still uses
    // the separated regions below.
    if block_at(position).is_some_and(|material| water_at(position, material).source) {
        return vec![];
    }
    let mut dry = Vec::new();
    for region in 0..4 {
        let mut seen = HashSet::from([(position, region)]);
        let mut queue = VecDeque::from([(position, region)]);
        let mut wet = false;
        while let Some((p, quadrant)) = queue.pop_front() {
            let connections = connected_arms(journal, p, &mut block_at);
            for axis in [0, 2] {
                let bit = if axis == 0 { 1 } else { 2 };
                let high = quadrant & bit != 0;
                // Crossing the middle of X meets the Z rail in this Z half,
                // and conversely for the other axis.
                let across = if axis == 0 {
                    4 + usize::from(quadrant & 2 != 0)
                } else {
                    usize::from(quadrant & 1 != 0)
                };
                if !connections[across] && seen.len() < 128 && seen.insert((p, quadrant ^ bit)) {
                    queue.push_back((p, quadrant ^ bit));
                }
                let mut q = p;
                q[axis] += if high { 1 } else { -1 };
                let Some(material) = block_at(q) else {
                    continue;
                };
                if construction::is_fence(material) {
                    if seen.len() < 128 && seen.insert((q, quadrant ^ bit)) {
                        queue.push_back((q, quadrant ^ bit));
                    }
                } else if super::can_hold_fluid(material) && water_at(q, material).amount > 0 {
                    wet = true;
                    break;
                }
            }
            if wet {
                break;
            }
        }
        if !wet {
            dry.push(ShapeBox {
                minimum: [
                    if region & 1 == 0 { 0. } else { 0.5 },
                    0.,
                    if region & 2 == 0 { 0. } else { 0.5 },
                ],
                maximum: [
                    if region & 1 == 0 { 0.5 } else { 1. },
                    1.,
                    if region & 2 == 0 { 0.5 } else { 1. },
                ],
                hinged: false,
            });
        }
    }
    dry
}

fn connected_arms(
    journal: &EditJournal,
    p: [i64; 3],
    block_at: &mut impl FnMut([i64; 3]) -> Option<u16>,
) -> [bool; 6] {
    let facing = journal.object_state(p).unwrap_or_default().facing;
    let mut connected = [false; 6];
    for arm in 0..4 {
        let side = construction::fence_neighbor_side(arm, facing);
        let mut q = p;
        q[side / 2] += if side.is_multiple_of(2) { -1 } else { 1 };
        connected[side] = construction::connects(block_at(q));
    }
    connected
}
