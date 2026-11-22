use super::{BlockEditRequest, ChunkStore, WorldAccess, editing::commit_segment_edit};
use crate::{edits::EditJournal, segments};
use bevy::prelude::*;
use terra_core::{ItemKind, block_properties::BlockPropertiesCatalog};
use terra_voxel::MaterialId;

fn anchored(chunks: &ChunkStore, p: [i64; 3], material: u16, direction: i64) -> Option<bool> {
    for step in 1..=segments::MAX_CHAIN + 1 {
        let point = [p[0], p[1] + step * direction, p[2]];
        let m = chunks.block_at(point)?;
        if m == material {
            continue;
        }
        return Some(
            MaterialId::from_raw(m)
                .is_some_and(|m| m.is_solid() && !segments::is_segment(m as u16)),
        );
    }
    Some(false)
}

fn unsupported(chunks: &ChunkStore, p: [i64; 3], m: u16) -> bool {
    if matches!(m, 54 | 55) {
        return segments::hanging_support(p, |p| chunks.block_at(p)) == Some(false);
    }
    if segments::is_crystal(m) {
        return segments::mineral_support(p, m, |p| chunks.block_at(p)) == Some(false);
    }
    if let Some(dy) = segments::fixed_mineral_root(m) {
        return chunks
            .block_at([p[0], p[1] + dy, p[2]])
            .is_some_and(|m| !segments::root_block(m));
    }
    match m {
        50 | 56 => anchored(chunks, p, m, -1) == Some(false),
        _ => false,
    }
}

pub(super) fn after_edit(
    chunks: &mut ChunkStore,
    journal: &mut EditJournal,
    p: [i64; 3],
    properties: &BlockPropertiesCatalog,
) -> Result<(), String> {
    for direction in [-1, 1] {
        let q = [p[0], p[1] + direction, p[2]];
        prune(chunks, journal, q, properties)?;
    }
    Ok(())
}

fn prune(
    chunks: &mut ChunkStore,
    journal: &mut EditJournal,
    p: [i64; 3],
    properties: &BlockPropertiesCatalog,
) -> Result<(), String> {
    if let Some(material) = chunks
        .block_at(p)
        .filter(|m| terra_core::lantern::is_part(*m))
    {
        let facing = journal.object_state(p).unwrap_or_default().facing;
        if crate::lanterns::support_status(chunks, properties, p, material, facing) != Some(false) {
            return Ok(());
        }
        let required =
            crate::lanterns::released_dependents(chunks, journal, p, 0, properties).len() + 1;
        if journal.gameplay().drops.len().saturating_add(required) > terra_core::MAX_WORLD_DROPS
            || journal
                .gameplay()
                .next_drop
                .checked_add(required as u64)
                .is_none()
        {
            chunks.support_queue.push_back(p);
            return Ok(());
        }
        if super::commit_block_edit(
            chunks,
            journal,
            &BlockEditRequest {
                position: p,
                expected_material: material,
                material: 0,
            },
            properties,
        )? {
            journal
                .gameplay_mut()
                .drop_one(p.map(|v| v as f64 + 0.5), ItemKind::Block(material));
        }
        return Ok(());
    }
    if chunks.block_at(p) == Some(98) {
        let below = [p[0], p[1] - 1, p[2]];
        if chunks
            .block_at(below)
            .is_some_and(|m| !MaterialId::from_raw(m).is_some_and(MaterialId::is_solid))
        {
            commit_segment_edit(
                chunks,
                journal,
                &BlockEditRequest {
                    position: p,
                    expected_material: 98,
                    material: 0,
                },
                properties,
            )?;
        }
        return Ok(());
    }
    let Some(material) = chunks
        .block_at(p)
        .filter(|m| segments::is_segment(*m) || segments::fixed_mineral_root(*m).is_some())
    else {
        return Ok(());
    };
    if !unsupported(chunks, p, material) {
        return Ok(());
    }
    if !journal.gameplay().can_drop() {
        chunks.support_queue.push_back(p);
        return Ok(());
    }
    if segments::is_crystal(material) {
        return detach_mineral(chunks, journal, p, material, properties);
    }
    if commit_segment_edit(
        chunks,
        journal,
        &BlockEditRequest {
            position: p,
            expected_material: material,
            material: 0,
        },
        properties,
    )? {
        journal
            .gameplay_mut()
            .drop_one(p.map(|v| v as f64 + 0.5), ItemKind::Block(material));
        after_edit(chunks, journal, p, properties)?;
    }
    Ok(())
}

/// Remove a contiguous unsupported shaft, publishing one physical drop for it.
/// Both exposed ends still reconcile other plants or minerals that they supported.
fn detach_mineral(
    chunks: &mut ChunkStore,
    journal: &mut EditJournal,
    [x, y, z]: [i64; 3],
    material: u16,
    properties: &BlockPropertiesCatalog,
) -> Result<(), String> {
    let mut low = y;
    while y - low < segments::MAX_CHAIN - 1 && chunks.block_at([x, low - 1, z]) == Some(material) {
        low -= 1;
    }
    let state = journal.gameplay_mut();
    let origin = [x as f64 + 0.5, low as f64 + 0.5, z as f64 + 0.5];
    // Reserve the one drop slot before block removals can release attached objects.
    if !state.drop_one(origin, ItemKind::Block(material)) {
        chunks.support_queue.push_back([x, y, z]);
        return Ok(());
    }
    let slot = state.drops.len() - 1;
    let mut count = 0;
    let mut pending_error = None;
    while count < segments::MAX_CHAIN {
        let point = [x, low + count, z];
        if chunks.block_at(point) != Some(material) {
            break;
        }
        match commit_segment_edit(
            chunks,
            journal,
            &BlockEditRequest {
                position: point,
                expected_material: material,
                material: 0,
            },
            properties,
        ) {
            Ok(true) => count += 1,
            Ok(false) => break,
            Err(error) => {
                pending_error = Some(error);
                break;
            }
        }
    }
    let state = journal.gameplay_mut();
    if count == 0 {
        state.drops.remove(slot);
    } else {
        let drop = &mut state.drops[slot];
        drop.position[1] = low as f64 + count as f64 * 0.5;
        drop.stack.count = count as u16;
        drop.rotation = terra_core::WorldDrop::MINERAL_TIP_DOWN;
        drop.angular_velocity = [0.; 3];
        drop.velocity = [0., -1., 0.];
        drop.pickup_delay_seconds = 1.;
        chunks
            .support_queue
            .extend([[x, low - 1, z], [x, low + count, z]]);
    }
    pending_error.map_or(Ok(()), Err)
}

pub(super) fn loaded(
    chunks: &mut ChunkStore,
    journal: &EditJournal,
    position: terra_voxel::SectionPos,
) {
    let mut work = Vec::new();
    // Old sections may contain foliage selected before the final terrain CSG,
    // or gaps within a hanging chain. Inspect only newly arrived data.
    // A missing neighbouring section remains unknown and is not persisted away.
    if let Some(section) = chunks.loaded.get(&position)
        && let Ok(origin) = position.origin()
    {
        for (index, &material) in section.data.blocks().iter().enumerate() {
            if !matches!(material, 50 | 54 | 55 | 56)
                && segments::fixed_mineral_root(material).is_none()
                && !terra_core::lantern::is_part(material)
            {
                continue;
            }
            let p = [
                origin[0] + (index % 16) as i64,
                origin[1] + (index / 16 % 16) as i64,
                origin[2] + (index / 256) as i64,
            ];
            if terra_core::lantern::is_part(material) {
                work.push(p);
                continue;
            }
            let neighbor = segments::fixed_mineral_root(material)
                .unwrap_or(if matches!(material, 54 | 55) { 1 } else { -1 });
            if !chunks
                .block_at([p[0], p[1] + neighbor, p[2]])
                .is_some_and(|m| m == material || matches!((material, m), (54 | 55, 54 | 55)))
                && unsupported(chunks, p, material)
            {
                work.push(p);
            }
        }
        // The ceiling of a previously incomplete chain can arrive last.
        for z in 0..16 {
            for x in 0..16 {
                let p = [origin[0] + x, origin[1] - 1, origin[2] + z];
                if let Some(material) = chunks
                    .block_at(p)
                    .filter(|m| matches!(m, 54 | 55) || segments::fixed_mineral_root(*m) == Some(1))
                    && unsupported(chunks, p, material)
                {
                    work.push(p);
                }
                // A rooted aquatic chain may have arrived before its floor.
                let p = [origin[0] + x, origin[1] + 16, origin[2] + z];
                if let Some(material) = chunks.block_at(p).filter(|m| {
                    matches!(m, 50 | 56) || segments::fixed_mineral_root(*m) == Some(-1)
                }) && unsupported(chunks, p, material)
                {
                    work.push(p);
                }
            }
        }
    }
    // A wall/ceiling support can stream after its already loaded fixture.
    // Inspect only sparse objects at the six newly published borders.
    for offset in [
        [1, 0, 0],
        [-1, 0, 0],
        [0, 1, 0],
        [0, -1, 0],
        [0, 0, 1],
        [0, 0, -1],
    ] {
        let Ok(neighbor) = position.offset(offset) else {
            continue;
        };
        for (p, material, state) in journal.object_states_in_section(neighbor) {
            if terra_core::lantern::is_part(material)
                && terra_core::lantern::support(p, material, state.facing)
                    .is_some_and(|anchor| terra_voxel::global_to_section(anchor).0 == position)
            {
                work.push(p);
            }
        }
    }
    // Reconcile only chains touched by persisted support loss. A revision in
    // this column is not evidence that every natural formation lost its root.
    for dy in -2..=2 {
        let Ok(edited) = position.offset([0, dy, 0]) else {
            continue;
        };
        let Ok(origin) = edited.origin() else {
            continue;
        };
        for (local, material) in journal.blocks_in_section(edited) {
            if MaterialId::from_raw(material).is_some_and(MaterialId::is_solid) {
                continue;
            }
            let changed = std::array::from_fn::<_, 3, _>(|axis| origin[axis] + local[axis] as i64);
            for direction in [-1, 1] {
                let p = [changed[0], changed[1] + direction, changed[2]];
                let Some(material) = chunks.block_at(p).filter(|m| {
                    segments::is_segment(*m)
                        || segments::fixed_mineral_root(*m).is_some()
                        || *m == 98
                }) else {
                    continue;
                };
                work.push(p);
                for step in 1..=segments::MAX_CHAIN {
                    let next = [p[0], p[1] + step * direction, p[2]];
                    if chunks.block_at(next) != Some(material) {
                        break;
                    }
                    work.push(next);
                }
            }
        }
    }
    work.sort_unstable();
    work.dedup();
    chunks.support_queue.extend(work);
}

#[cfg(test)]
mod tests;

/// Reload reconciliation is bounded; valid natural columns enqueue no work.
pub(super) fn tick(
    access: Res<WorldAccess>,
    mut chunks: ResMut<ChunkStore>,
    mut journal: ResMut<EditJournal>,
    properties: Res<BlockPropertiesCatalog>,
) {
    if !access.authoritative() || chunks.support_queue.is_empty() {
        return;
    }
    for _ in 0..32 {
        let Some(p) = chunks.support_queue.pop_front() else {
            break;
        };
        if let Err(error) = prune(&mut chunks, &mut journal, p, &properties) {
            tracing::error!(%error,"Segment support reconciliation failed");
        }
    }
}
