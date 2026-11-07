//! Shared support rules for floor/wall lamps and bounded hanging chain columns.
use crate::{edits::EditJournal, streaming::ChunkStore};
use terra_core::{block_properties::BlockPropertiesCatalog, lantern};

fn anchor_supports(
    properties: &BlockPropertiesCatalog,
    block: u16,
    material: u16,
    facing: u8,
) -> bool {
    let hangs = material == lantern::CHAIN || facing == lantern::HANGING;
    if hangs && block == lantern::CHAIN {
        return true;
    }
    terra_voxel::MaterialId::from_raw(block).is_some_and(terra_voxel::MaterialId::is_solid)
        && (properties.shape(block).is_empty()
            || properties
                .shape(block)
                .iter()
                .any(|shape| (0..3).all(|i| shape.minimum[i] <= 0.01 && shape.maximum[i] >= 0.99))
            || material == lantern::LANTERN
                && facing == lantern::TABLE
                && terra_core::construction::is_fence(block))
}

pub fn supported(
    chunks: &ChunkStore,
    properties: &BlockPropertiesCatalog,
    position: [i64; 3],
    material: u16,
    facing: u8,
) -> bool {
    support_status(chunks, properties, position, material, facing) == Some(true)
}

/// Unloaded support remains unknown, so streaming cannot break an intact chain.
pub fn support_status(
    chunks: &ChunkStore,
    properties: &BlockPropertiesCatalog,
    position: [i64; 3],
    material: u16,
    facing: u8,
) -> Option<bool> {
    let mut anchor = lantern::support(position, material, facing)?;
    let hangs = material == lantern::CHAIN || facing == lantern::HANGING;
    for _ in 0..lantern::MAX_CHAIN {
        let block = chunks.block_at(anchor)?;
        if hangs && block == lantern::CHAIN {
            anchor[1] = anchor[1].checked_add(1)?;
            continue;
        }
        return Some(anchor_supports(properties, block, material, facing));
    }
    Some(false)
}

/// Exactly the pieces a removed mount will release, for authority rights/drop
/// preflight. The actual edit cascade uses the same directional support rule.
pub fn dependents(chunks: &ChunkStore, journal: &EditJournal, anchor: [i64; 3]) -> Vec<[i64; 3]> {
    dependency_walk(chunks, journal, anchor, None)
}
pub fn released_dependents(
    chunks: &ChunkStore,
    journal: &EditJournal,
    anchor: [i64; 3],
    replacement: u16,
    properties: &BlockPropertiesCatalog,
) -> Vec<[i64; 3]> {
    dependency_walk(chunks, journal, anchor, Some((replacement, properties)))
}
fn dependency_walk(
    chunks: &ChunkStore,
    journal: &EditJournal,
    anchor: [i64; 3],
    replacement: Option<(u16, &BlockPropertiesCatalog)>,
) -> Vec<[i64; 3]> {
    let mut result = Vec::new();
    let mut queue = std::collections::VecDeque::from([anchor]);
    while let Some(parent) = queue.pop_front() {
        for offset in [
            [1, 0, 0],
            [-1, 0, 0],
            [0, 1, 0],
            [0, -1, 0],
            [0, 0, 1],
            [0, 0, -1],
        ] {
            let p = std::array::from_fn(|i| parent[i].saturating_add(offset[i]));
            // Rights must cover saved fixtures before a neighboring section
            // streams back in; otherwise reload pruning bypasses that check.
            let Some(material) = chunks
                .block_at(p)
                .or_else(|| journal.block(p))
                .filter(|m| lantern::is_part(*m))
            else {
                continue;
            };
            let facing = journal.object_state(p).unwrap_or_default().facing;
            if lantern::support(p, material, facing) == Some(parent) && !result.contains(&p) {
                if parent == anchor
                    && replacement.is_some_and(|(block, properties)| {
                        anchor_supports(properties, block, material, facing)
                    })
                {
                    continue;
                }
                result.push(p);
                if material == lantern::CHAIN && result.len() <= lantern::MAX_CHAIN {
                    queue.push_back(p);
                }
            }
        }
        if result.len() > lantern::MAX_CHAIN + 6 {
            break;
        }
    }
    result
}

#[cfg(test)]
mod tests;
