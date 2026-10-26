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
    let down = anchored(chunks, p, m, -1);
    let up = anchored(chunks, p, m, 1);
    match m {
        50 | 56 => down == Some(false),
        51..=53 => down == Some(false) && up == Some(false),
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
    let Some(material) = chunks.block_at(p).filter(|m| segments::is_segment(*m)) else {
        return Ok(());
    };
    if !unsupported(chunks, p, material) {
        return Ok(());
    }
    if !journal.gameplay().can_drop() {
        chunks.support_queue.push_back(p);
        return Ok(());
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
        if segments::is_crystal(material)
            && let Some(drop) = journal.gameplay_mut().drops.last_mut()
        {
            drop.velocity = [0.0, -1.0, 0.0];
            drop.angular_velocity = [0.0; 3];
            drop.rotation = [0.0, 0.0, 0.0, 1.0];
            drop.pickup_delay_seconds = 1.0;
        }
        after_edit(chunks, journal, p, properties)?;
    }
    Ok(())
}

pub(super) fn loaded(
    chunks: &mut ChunkStore,
    journal: &EditJournal,
    position: terra_voxel::SectionPos,
) {
    let mut work = Vec::new();
    // Old sections may contain foliage selected per voxel, leaving gaps within
    // a hanging chain. Inspect only newly arrived data, never the entire world.
    // A missing neighbouring section remains unknown and is not persisted away.
    if let Some(section) = chunks.loaded.get(&position)
        && let Ok(origin) = position.origin()
    {
        for (index, &material) in section.data.blocks().iter().enumerate() {
            if !matches!(material, 54 | 55) {
                continue;
            }
            let p = [
                origin[0] + (index % 16) as i64,
                origin[1] + (index / 16 % 16) as i64,
                origin[2] + (index / 256) as i64,
            ];
            if !chunks
                .block_at([p[0], p[1] + 1, p[2]])
                .is_some_and(|m| matches!(m, 54 | 55))
                && unsupported(chunks, p, material)
            {
                work.push(p);
            }
        }
        // The ceiling of a previously incomplete chain can arrive last.
        for z in 0..16 {
            for x in 0..16 {
                let p = [origin[0] + x, origin[1] - 1, origin[2] + z];
                if let Some(material) = chunks.block_at(p).filter(|m| matches!(m, 54 | 55))
                    && unsupported(chunks, p, material)
                {
                    work.push(p);
                }
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
                let Some(material) = chunks
                    .block_at(p)
                    .filter(|m| segments::is_segment(*m) || *m == 98)
                else {
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
