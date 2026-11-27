//! Both ends of a bucket transfer commit a single normal block edit. A lava
//! block without flow state already represents a full, stationary source.
use super::*;

fn dry_air(chunks: &ChunkStore, journal: &EditJournal, p: [i64; 3]) -> bool {
    p.iter().all(|v| v.unsigned_abs() <= 1_000_000)
        && chunks.block_at(p) == Some(0)
        && !wet(chunks, journal, p)
}

pub fn target(
    chunks: &ChunkStore,
    journal: &EditJournal,
    origin: [f64; 3],
    direction: [f64; 3],
    filled: bool,
) -> Option<[i64; 3]> {
    let hit = terra_voxel::blocks::raycast_blocks(origin, direction, 6., |p| {
        let raw = chunks.block_at(p)?;
        Some(if raw == 0 && wet(chunks, journal, p) {
            M::SolidWater as u16
        } else {
            raw
        })
    })?;
    if filled {
        let destination = std::array::from_fn(|i| hit.position[i] + i64::from(hit.normal[i]));
        dry_air(chunks, journal, destination).then_some(destination)
    } else {
        state(chunks, journal, hit.position)
            .filter(|s| s.source)
            .map(|_| hit.position)
    }
}

pub fn pour(
    chunks: &mut ChunkStore,
    journal: &mut EditJournal,
    properties: &BlockPropertiesCatalog,
    position: [i64; 3],
) -> Result<bool, String> {
    if !dry_air(chunks, journal, position) {
        return Ok(false);
    }
    commit_block_edit(
        chunks,
        journal,
        &BlockEditRequest {
            position,
            expected_material: 0,
            material: LAVA,
        },
        properties,
    )
}

pub fn take_source(
    chunks: &mut ChunkStore,
    journal: &mut EditJournal,
    properties: &BlockPropertiesCatalog,
    position: [i64; 3],
) -> Result<bool, String> {
    if !state(chunks, journal, position).is_some_and(|s| s.source) {
        return Ok(false);
    }
    commit_block_edit(
        chunks,
        journal,
        &BlockEditRequest {
            position,
            expected_material: LAVA,
            material: 0,
        },
        properties,
    )
}
