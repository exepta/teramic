//! Validate an entire ordered tree patch before changing either replica mirror.
use super::*;
use std::collections::{BTreeMap, BTreeSet, HashSet};
use terra_core::{TreeRecord, TreeStage, trees::TreeGeometryKey};

type Geometry = (Vec<[i8; 3]>, Vec<[i8; 3]>);
struct Prepared {
    metadata: TreeRecord,
    geometry: Option<Geometry>,
}
fn compatible(old: &TreeRecord, new: &TreeRecord) -> bool {
    old.root == new.root
        && old.species == new.species
        && old.seed == new.seed
        && old.planted_by == new.planted_by
        && (old.stage == new.stage
            || old.stage == TreeStage::Generated && new.stage == TreeStage::GeneratedFelled
            || old.stage == TreeStage::Mature && new.stage == TreeStage::Felled)
}
fn valid_metadata(tree: &TreeRecord) -> bool {
    tree.id != 0
        && tree.root.iter().all(|v| v.unsigned_abs() <= 1_000_000)
        && tree.planted_by.is_none_or(|p| !p.is_nil())
        && tree.growth_seconds.is_finite()
        && (0.0..=86_400.0).contains(&tree.growth_seconds)
        && tree
            .fruit
            .is_none_or(|f| tree.species == terra_core::TreeSpecies::Apple && f.valid())
        && (tree.stage.is_mature() || tree.fruit.is_none())
        && (tree.stage != TreeStage::Sapling || tree.planted_by.is_some())
        && tree.bounds.is_none_or(|b| {
            (0..3).all(|a| {
                b[0][a] <= b[1][a] && b[0][a].unsigned_abs() <= 64 && b[1][a].unsigned_abs() <= 64
            })
        })
}
fn retained(old: &[[i8; 3]], removed: Vec<[i8; 3]>) -> Result<Vec<[i8; 3]>, String> {
    let count = removed.len();
    let removed = removed.into_iter().collect::<HashSet<_>>();
    if removed.len() != count {
        return Err("Duplicate tree cell removal".into());
    }
    let remaining = old
        .iter()
        .filter(|cell| !removed.contains(*cell))
        .copied()
        .collect::<Vec<_>>();
    if old.len() - remaining.len() != count {
        return Err("Unknown tree cell removal".into());
    }
    Ok(remaining)
}
fn key(prepared: &Prepared, old: Option<&TreeRecord>) -> TreeGeometryKey {
    let mut key = prepared.metadata.geometry_key();
    if let Some((wood, leaves)) = &prepared.geometry {
        key.wood = wood.len();
        key.leaves = leaves.len();
    } else if let Some(old) = old {
        key.wood = old.wood.len();
        key.leaves = old.leaves.len();
    }
    key
}
fn visual_sections(
    chunks: &ChunkStore,
    before: Option<TreeGeometryKey>,
    after: Option<TreeGeometryKey>,
) -> BTreeSet<terra_voxel::SectionPos> {
    let mut sections = BTreeSet::new();
    if before == after {
        return sections;
    }
    for key in before
        .into_iter()
        .chain(after)
        .filter(|key| key.stage.is_generated())
    {
        let Some(bounds) = key.bounds else {
            continue;
        };
        let low = [0, 1, 2].map(|a| (key.root[a] + i64::from(bounds[0][a])).div_euclid(16));
        let high = [0, 1, 2].map(|a| (key.root[a] + i64::from(bounds[1][a])).div_euclid(16));
        for x in low[0]..=high[0] {
            for y in low[1]..=high[1] {
                for z in low[2]..=high[2] {
                    let section = terra_voxel::SectionPos::new(x, y, z);
                    if chunks.loaded.contains_key(&section) {
                        sections.insert(section);
                    }
                }
            }
        }
    }
    sections
}
fn preflight_visuals(
    chunks: &ChunkStore,
    journal: &EditJournal,
    pairs: &[(Option<TreeGeometryKey>, Option<TreeGeometryKey>)],
) -> Result<bool, String> {
    let mut increments = BTreeMap::<_, u64>::new();
    for &(before, after) in pairs {
        for section in visual_sections(chunks, before, after) {
            *increments.entry(section).or_default() += 1;
        }
    }
    for (&section, &count) in &increments {
        journal
            .mesh_epoch(section)
            .checked_add(count)
            .ok_or("Tree mesh revision exhausted")?;
    }
    Ok(!increments.is_empty())
}
fn stamp(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf29ce484222325, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(0x100000001b3)
    })
}

/// None means a harmless stale/exact duplicate; Some reports visual invalidation.
pub(super) fn apply(
    patch: TreePatch,
    session: &mut ClientSession,
    journal: &mut EditJournal,
    chunks: &mut ChunkStore,
) -> Result<Option<bool>, String> {
    if session.player.is_none()
        || patch.revision == 0
        || patch.records.len() > MAX_TREE_INTEREST
        || patch.removals.len() > MAX_TREE_INTEREST
    {
        return Err("Invalid tree patch envelope".into());
    }
    let offsets = patch
        .records
        .iter()
        .try_fold(0usize, |sum, update| {
            let removals = match &update.geometry {
                TreeGeometryUpdate::Removed { wood, leaves } => {
                    wood.len().checked_add(leaves.len())
                }
                _ => Some(0),
            }?;
            sum.checked_add(update.tree.wood.len())?
                .checked_add(update.tree.leaves.len())?
                .checked_add(removals)
        })
        .ok_or("Tree patch count overflow")?;
    if offsets > MAX_TREE_PATCH_CELLS {
        return Err("Tree patch cell budget exceeded".into());
    }
    let bytes = postcard::to_allocvec(&patch).map_err(|error| error.to_string())?;
    if bytes.len() + 8 > MAX_TREE_PATCH_BYTES {
        return Err("Tree patch byte budget exceeded".into());
    }
    let signature = stamp(&bytes);
    if patch.revision < session.trees.revision {
        return Ok(None);
    }
    if patch.revision == session.trees.revision {
        return if session.tree_patch_signature == Some(signature) {
            Ok(None)
        } else {
            Err("Conflicting tree patch revision".into())
        };
    }
    if !patch.reset && session.trees.revision.checked_add(1) != Some(patch.revision) {
        return Err("Missing tree patch revision".into());
    }
    let removal_count = patch.removals.len();
    let removals = patch.removals.into_iter().collect::<BTreeSet<_>>();
    if removals.contains(&0) || removals.len() != removal_count {
        return Err("Invalid tree removals".into());
    }
    let mut prepared = BTreeMap::new();
    for update in patch.records {
        let mut tree = update.tree;
        if !valid_metadata(&tree) || removals.contains(&tree.id) || prepared.contains_key(&tree.id)
        {
            return Err("Invalid or duplicated tree metadata".into());
        }
        let old = (!patch.reset).then(|| session.trees.get(tree.id)).flatten();
        if old.is_some_and(|old| tree.revision < old.revision) {
            return Err("Stale tree record revision".into());
        }
        let geometry = match update.geometry {
            TreeGeometryUpdate::Full => {
                if !tree.valid() {
                    return Err("Invalid full tree geometry".into());
                }
                Some((
                    std::mem::take(&mut tree.wood),
                    std::mem::take(&mut tree.leaves),
                ))
            }
            TreeGeometryUpdate::Unchanged => {
                let old = old.ok_or("Missing tree geometry baseline")?;
                if !tree.wood.is_empty()
                    || !tree.leaves.is_empty()
                    || !compatible(old, &tree)
                    || tree.bounds != old.bounds
                {
                    return Err("Changed unchanged tree geometry".into());
                }
                None
            }
            TreeGeometryUpdate::Removed { wood, leaves } => {
                let old = old.ok_or("Missing tree removal baseline")?;
                if !tree.wood.is_empty() || !tree.leaves.is_empty() || !compatible(old, &tree) {
                    return Err("Mismatched tree removal identity".into());
                }
                tree.wood = retained(&old.wood, wood)?;
                tree.leaves = retained(&old.leaves, leaves)?;
                if !tree.valid() {
                    return Err("Invalid retained tree geometry".into());
                }
                Some((
                    std::mem::take(&mut tree.wood),
                    std::mem::take(&mut tree.leaves),
                ))
            }
        };
        if geometry.is_none()
            && journal
                .gameplay()
                .trees
                .get(tree.id)
                .map(TreeRecord::geometry_key)
                != old.map(TreeRecord::geometry_key)
        {
            return Err("Tree mirror lost geometry baseline".into());
        }
        prepared.insert(
            tree.id,
            Prepared {
                metadata: tree,
                geometry,
            },
        );
    }
    let retained_ids = if patch.reset {
        BTreeSet::new()
    } else {
        session
            .trees
            .trees
            .keys()
            .filter(|id| !removals.contains(*id))
            .copied()
            .collect::<BTreeSet<_>>()
    };
    if retained_ids
        .union(&prepared.keys().copied().collect())
        .count()
        > MAX_TREE_INTEREST
    {
        return Err("Tree interest budget exceeded".into());
    }
    let mut affected = removals.clone();
    affected.extend(prepared.keys().copied());
    if patch.reset {
        affected.extend(session.trees.trees.keys().copied());
    }
    let pairs = affected
        .iter()
        .map(|id| {
            let old = session.trees.get(*id);
            let after = prepared.get(id).map(|next| key(next, old)).or_else(|| {
                (!patch.reset && !removals.contains(id))
                    .then(|| old.map(TreeRecord::geometry_key))
                    .flatten()
            });
            (old.map(TreeRecord::geometry_key), after)
        })
        .collect::<Vec<_>>();
    let visuals = preflight_visuals(chunks, journal, &pairs)?;
    // All hook counters are checked together, so no later hook can fail after
    // an earlier record has already changed an invalidation epoch.
    for &(before, after) in &pairs {
        terra_world::saplings::invalidate_tree_visuals(chunks, journal, before, after)?;
    }
    if patch.reset {
        session.trees.trees.clear();
        journal.gameplay_mut().trees.trees.clear();
    }
    for id in removals {
        session.trees.trees.remove(&id);
        journal.gameplay_mut().trees.trees.remove(&id);
    }
    for (id, prepared) in prepared {
        let mut tree = prepared.metadata;
        if let Some((wood, leaves)) = prepared.geometry {
            tree.wood = wood;
            tree.leaves = leaves;
            journal.gameplay_mut().trees.trees.insert(id, tree.clone());
            session.trees.trees.insert(id, tree);
        } else {
            let mirror = journal
                .gameplay_mut()
                .trees
                .trees
                .get_mut(&id)
                .ok_or("Tree mirror baseline disappeared")?;
            let wood = std::mem::take(&mut mirror.wood);
            let leaves = std::mem::take(&mut mirror.leaves);
            *mirror = tree.clone();
            mirror.wood = wood;
            mirror.leaves = leaves;
            let previous = session
                .trees
                .trees
                .get_mut(&id)
                .ok_or("Tree session baseline disappeared")?;
            tree.wood = std::mem::take(&mut previous.wood);
            tree.leaves = std::mem::take(&mut previous.leaves);
            *previous = tree;
        }
    }
    session.trees.revision = patch.revision;
    journal.gameplay_mut().trees.revision = patch.revision;
    session.tree_patch_signature = Some(signature);
    Ok(Some(visuals))
}

pub(super) fn legacy(
    trees: terra_core::TreeRegistry,
    session: &mut ClientSession,
    journal: &mut EditJournal,
    chunks: &mut ChunkStore,
) -> Result<Option<bool>, String> {
    if session.player.is_none() || trees.trees.len() > MAX_TREE_INTEREST || !trees.valid() {
        return Err("Invalid tree snapshot".into());
    }
    if trees.revision < session.trees.revision {
        return Ok(None);
    }
    if trees.revision == session.trees.revision {
        return if trees == session.trees {
            Ok(None)
        } else {
            Err("Conflicting tree snapshot revision".into())
        };
    }
    let ids = trees
        .trees
        .keys()
        .chain(session.trees.trees.keys())
        .copied()
        .collect::<BTreeSet<_>>();
    let pairs = ids
        .into_iter()
        .map(|id| {
            (
                session.trees.get(id).map(TreeRecord::geometry_key),
                trees.get(id).map(TreeRecord::geometry_key),
            )
        })
        .collect::<Vec<_>>();
    let visuals = preflight_visuals(chunks, journal, &pairs)?;
    for (before, after) in pairs {
        terra_world::saplings::invalidate_tree_visuals(chunks, journal, before, after)?;
    }
    journal.gameplay_mut().trees = trees.clone();
    session.trees = trees;
    session.tree_patch_signature = None;
    Ok(Some(visuals))
}

#[cfg(test)]
#[path = "client_tree_patch_tests.rs"]
mod tests;
