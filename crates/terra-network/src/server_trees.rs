//! Ordered, bounded interest deltas. Stable metadata never copies/scans canopy cells.
use super::*;
use std::collections::{BTreeMap, BTreeSet};
use terra_core::{TreeRecord, TreeStage};

#[derive(Resource, Default)]
pub(super) struct TreeRuntime(HashMap<PlayerId, TreePeer>);
#[derive(Default)]
struct TreePeer {
    serial: u64,
    initialized: bool,
    confirmed: BTreeMap<u64, TreeRecord>,
}

fn metadata(tree: &TreeRecord) -> TreeRecord {
    TreeRecord {
        id: tree.id,
        root: tree.root,
        species: tree.species,
        seed: tree.seed,
        planted_by: tree.planted_by,
        stage: tree.stage,
        growth_seconds: if tree.stage == TreeStage::Sapling {
            let step = tree.species.growth_seconds() / 100.;
            (tree.growth_seconds / step).floor().clamp(0., 100.) * step
        } else {
            tree.growth_seconds
        },
        bounds: tree.bounds,
        wood: Vec::new(),
        leaves: Vec::new(),
        fruit: tree.fruit,
        revision: tree.revision,
    }
}
fn same_metadata(a: &TreeRecord, b: &TreeRecord) -> bool {
    // Progress changes need not change the authority's coarse persistent revision.
    // Conversely an internal revision alone must not defeat percentage buckets.
    a.id == b.id
        && a.root == b.root
        && a.species == b.species
        && a.seed == b.seed
        && a.planted_by == b.planted_by
        && a.stage == b.stage
        && a.growth_seconds == b.growth_seconds
        && a.bounds == b.bounds
        && a.fruit == b.fruit
}
fn compatible_geometry(old: &TreeRecord, new: &TreeRecord) -> bool {
    old.root == new.root
        && old.species == new.species
        && old.seed == new.seed
        && old.planted_by == new.planted_by
        && (old.stage == new.stage
            || old.stage == TreeStage::Generated && new.stage == TreeStage::GeneratedFelled
            || old.stage == TreeStage::Mature && new.stage == TreeStage::Felled)
}
/// Felling retains canonical offset order. Additions/reorders require Full.
fn removed(old: &[[i8; 3]], new: &[[i8; 3]]) -> Option<Vec<[i8; 3]>> {
    let mut next = new.iter().peekable();
    let mut removed = Vec::new();
    for cell in old {
        if next.peek().is_some_and(|&&candidate| candidate == *cell) {
            next.next();
        } else {
            removed.push(*cell);
        }
    }
    next.next().is_none().then_some(removed)
}
fn update(tree: &TreeRecord, meta: TreeRecord, old: Option<&TreeRecord>) -> TreeUpdate {
    if let Some(old) = old {
        if old.geometry_key() == tree.geometry_key() {
            return TreeUpdate {
                tree: meta,
                geometry: TreeGeometryUpdate::Unchanged,
            };
        }
        if compatible_geometry(old, tree)
            && let Some(wood) = removed(&old.wood, &tree.wood)
            && let Some(leaves) = removed(&old.leaves, &tree.leaves)
        {
            return TreeUpdate {
                tree: meta,
                geometry: TreeGeometryUpdate::Removed { wood, leaves },
            };
        }
    }
    let mut full = tree.clone();
    full.growth_seconds = meta.growth_seconds;
    TreeUpdate {
        tree: full,
        geometry: TreeGeometryUpdate::Full,
    }
}
fn cells(update: &TreeUpdate) -> usize {
    update.tree.wood.len()
        + update.tree.leaves.len()
        + match &update.geometry {
            TreeGeometryUpdate::Removed { wood, leaves } => wood.len() + leaves.len(),
            _ => 0,
        }
}
fn next_patch(peer_state: &mut TreePeer, visible: &[&TreeRecord]) -> Option<TreePatch> {
    let revision = peer_state.serial.checked_add(1)?;
    let visible_tree_ids = visible.iter().map(|t| t.id).collect::<BTreeSet<_>>();
    let removals = peer_state
        .confirmed
        .keys()
        .filter(|id| !visible_tree_ids.contains(*id))
        .copied()
        .collect::<Vec<_>>();
    let mut patch = TreePatch {
        revision,
        reset: !peer_state.initialized,
        removals,
        records: Vec::new(),
    };
    // Reserve all container/enum/length varints before adding bounded records.
    let mut payload_byte_count = 64 + patch.removals.len() * 10;
    let mut geometry_cell_count = 0;
    let mut accepted_trees = Vec::new();
    for tree in visible.iter().take(MAX_TREE_INTEREST) {
        let tree_metadata = metadata(tree);
        let previous_tree = peer_state.confirmed.get(&tree.id);
        if previous_tree.is_some_and(|previous_tree| {
            same_metadata(previous_tree, &tree_metadata)
                && previous_tree.geometry_key() == tree.geometry_key()
        }) {
            continue;
        }
        let expected_cells = if previous_tree
            .is_some_and(|previous_tree| previous_tree.geometry_key() == tree.geometry_key())
        {
            0
        } else if let Some(previous_tree) = previous_tree.filter(|previous_tree| {
            compatible_geometry(previous_tree, tree)
                && previous_tree.wood.len() >= tree.wood.len()
                && previous_tree.leaves.len() >= tree.leaves.len()
        }) {
            previous_tree.wood.len() - tree.wood.len() + previous_tree.leaves.len()
                - tree.leaves.len()
        } else {
            tree.wood.len() + tree.leaves.len()
        };
        if expected_cells > MAX_TREE_PATCH_CELLS - geometry_cell_count {
            continue;
        }
        let candidate = update(tree, tree_metadata, previous_tree);
        let candidate_cell_count = cells(&candidate);
        if candidate_cell_count > MAX_TREE_PATCH_CELLS - geometry_cell_count {
            continue;
        }
        let Ok(encoded) = postcard::to_allocvec(&candidate) else {
            continue;
        };
        if encoded.len() > MAX_TREE_PATCH_BYTES.saturating_sub(payload_byte_count) {
            continue;
        }
        geometry_cell_count += candidate_cell_count;
        payload_byte_count += encoded.len();
        accepted_trees.push(*tree);
        patch.records.push(candidate);
    }
    if !patch.reset && patch.removals.is_empty() && patch.records.is_empty() {
        return None;
    }
    // Verify the encoded payload, including a conservative control-enum tag.
    // No unsent candidate enters the per-peer baseline before this succeeds.
    if postcard::to_allocvec(&patch)
        .map_or(true, |payload| payload.len() + 8 > MAX_TREE_PATCH_BYTES)
    {
        return None;
    }
    for tree in accepted_trees {
        let mut snapshot = metadata(tree);
        if let Some(previous_tree) = peer_state
            .confirmed
            .get_mut(&tree.id)
            .filter(|previous_tree| previous_tree.geometry_key() == tree.geometry_key())
        {
            snapshot.wood = std::mem::take(&mut previous_tree.wood);
            snapshot.leaves = std::mem::take(&mut previous_tree.leaves);
        } else {
            snapshot.wood = tree.wood.clone();
            snapshot.leaves = tree.leaves.clone();
        }
        peer_state.confirmed.insert(tree.id, snapshot);
    }
    for id in &patch.removals {
        peer_state.confirmed.remove(id);
    }
    peer_state.serial = revision;
    peer_state.initialized = true;
    Some(patch)
}

pub(super) fn broadcast(
    journal: Res<EditJournal>,
    profiles: Res<ConnectedProfiles>,
    bodies: Query<&Transform, With<terra_physics::character::CharacterBody>>,
    mut runtime: ResMut<TreeRuntime>,
    mut peers: Query<(&Peer, &mut MessageSender<ServerControl>), With<Connected>>,
) {
    runtime.0.retain(|id, _| profiles.0.contains_key(id));
    for (peer, mut sender) in &mut peers {
        if !peer.accepted {
            continue;
        }
        let Ok(body) = bodies.get(peer.body) else {
            continue;
        };
        let p = body.translation.to_array().map(f64::from);
        let range = f64::from(peer.view.max(2)) * 16. + 32.;
        let mut visible = journal
            .gameplay()
            .trees
            .trees
            .values()
            .filter(|tree| {
                !matches!(tree.stage, TreeStage::Felled | TreeStage::GeneratedFelled)
                    || !tree.wood.is_empty()
                    || !tree.leaves.is_empty()
            })
            .filter_map(|tree| {
                let dx = tree.root[0] as f64 - p[0];
                let dz = tree.root[2] as f64 - p[2];
                let distance = dx * dx + dz * dz;
                (distance <= range * range && (tree.root[1] as f64 - p[1]).abs() < 96.)
                    .then_some((distance, tree))
            })
            .collect::<Vec<_>>();
        visible.sort_unstable_by(|a, b| {
            (a.1.stage != TreeStage::Sapling)
                .cmp(&(b.1.stage != TreeStage::Sapling))
                .then(a.0.total_cmp(&b.0))
                .then(a.1.id.cmp(&b.1.id))
        });
        let visible = visible
            .into_iter()
            .take(MAX_TREE_INTEREST)
            .map(|(_, tree)| tree)
            .collect::<Vec<_>>();
        if let Some(patch) = next_patch(runtime.0.entry(peer.id).or_default(), &visible) {
            sender.send::<ControlChannel>(ServerControl::TreeUpdates(patch));
        }
    }
}

#[cfg(test)]
#[path = "server_tree_patch_tests.rs"]
mod tests;
