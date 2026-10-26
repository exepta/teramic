//! Exact natural-tree attribution and bounded authoritative felling batches.
use crate::{
    PreparedColumn,
    edits::EditJournal,
    streaming::{BlockEditRequest, ChunkStore, commit_block_edit},
};
use bevy::prelude::Resource;
use std::{
    collections::{HashMap, VecDeque},
    sync::Arc,
};
use terra_core::{ItemKind, ItemStack, PlayerUuid, block_properties::BlockPropertiesCatalog};
use terra_voxel::{SectionPos, global_to_section};

pub const MAX_WOOD: usize = 4096;
pub const MAX_LEAVES: usize = 16384;
pub const MAX_JOBS: usize = 8;
pub const WOOD_BUDGET: usize = 32;
pub const LEAF_BUDGET: usize = 64;
pub const LEAF_DELAY: f64 = 1.5;
pub const WOOD_SECONDS: f32 = 0.1;

mod collapse;
pub use collapse::{Collapse, CollapseSection};

pub(crate) struct Shape {
    pub origin: [i64; 3],
    pub wood_material: u16,
    pub leaves_material: u16,
    pub wood: Vec<[i64; 3]>,
    pub leaves: Vec<[i64; 3]>,
}

/// Every unedited wood cell attributed to a generated tree opts into felling.
pub fn eligible(chunks: &ChunkStore, journal: &EditJournal, foot: [i64; 3]) -> bool {
    journal.block(foot).is_none()
        && chunks
            .block_at(foot)
            .is_some_and(|m| matches!(m, 6 | 10 | 36 | 134))
        && chunks
            .prepared_column([foot[0].div_euclid(16), foot[2].div_euclid(16)])
            .is_some_and(|column| column.felling_foot(foot))
}

/// Dedicated servers need not normally simulate a tree's upper canopy. A valid
/// base interaction requests only this prepared shape through normal streaming.
pub fn request_missing(chunks: &mut ChunkStore, journal: &EditJournal, foot: [i64; 3]) {
    if !eligible(chunks, journal, foot) {
        return;
    }
    let Some(column) = chunks.prepared_column([foot[0].div_euclid(16), foot[2].div_euclid(16)])
    else {
        return;
    };
    let Some(shape) = column.felling_shape(foot) else {
        return;
    };
    let mut positions: Vec<_> = shape.wood.iter().map(|p| global_to_section(*p).0).collect();
    // A crown can reach a neighbouring column before its wood does. One crown
    // section provides that column's immutable attribution, not its whole height.
    let mut columns = std::collections::HashSet::new();
    for cell in &shape.leaves {
        let position = global_to_section(*cell).0;
        if columns.insert([position.x, position.z])
            && chunks.prepared_column([position.x, position.z]).is_none()
        {
            positions.push(position);
        }
    }
    positions.sort_unstable_by_key(|p| (p.x, p.y, p.z));
    positions.dedup();
    if positions.len() <= 48 {
        chunks.request_sections(positions);
    }
}

#[derive(Clone, Debug)]
pub struct Plan {
    origin: [i64; 3],
    foot: [i64; 3],
    material: u16,
    leaves_material: u16,
    wood: Vec<[i64; 3]>,
    leaves: Vec<[i64; 3]>,
    revisions: Vec<(SectionPos, u64)>,
}

impl Plan {
    /// Reads immutable prepared primitives; never samples terrain noise or flood-fills wood.
    pub fn prepare(chunks: &ChunkStore, journal: &EditJournal, foot: [i64; 3]) -> Option<Self> {
        if !eligible(chunks, journal, foot) {
            return None;
        }
        let coordinate = [foot[0].div_euclid(16), foot[2].div_euclid(16)];
        let first = chunks.prepared_column(coordinate)?;
        let shape = first.felling_shape(foot)?;
        if first.felling_owner(foot, shape.wood_material) != Some(shape.origin) {
            return None;
        }
        let mut columns: HashMap<[i64; 2], Arc<PreparedColumn>> =
            HashMap::from([(coordinate, first)]);
        let mut owner = |cell: [i64; 3], material: u16| -> Option<Option<[i64; 3]>> {
            let [x, _, z] = cell;
            let column = [x.div_euclid(16), z.div_euclid(16)];
            if let std::collections::hash_map::Entry::Vacant(entry) = columns.entry(column) {
                entry.insert(chunks.prepared_column(column)?);
            }
            Some(columns.get(&column)?.felling_owner(cell, material))
        };
        let mut wood = Vec::new();
        for cell in shape.wood {
            if journal.block(cell).is_some()
                || owner(cell, shape.wood_material)? != Some(shape.origin)
            {
                continue;
            }
            // A whole-tree timer cannot silently omit an unloaded upper trunk.
            if chunks.block_at(cell)? == shape.wood_material {
                wood.push(cell);
            }
        }
        if wood.is_empty() || !wood.contains(&foot) {
            return None;
        }
        let mut leaves = Vec::new();
        for cell in shape.leaves {
            if journal.block(cell).is_none()
                && owner(cell, shape.leaves_material)? == Some(shape.origin)
                && chunks
                    .block_at(cell)
                    .is_none_or(|m| m == shape.leaves_material)
            {
                leaves.push(cell);
            }
        }
        let mut revisions: Vec<_> = wood.iter().map(|p| global_to_section(*p).0).collect();
        revisions.sort_unstable_by_key(|p| (p.x, p.y, p.z));
        revisions.dedup();
        let revisions = revisions
            .into_iter()
            .map(|p| (p, journal.revision(p)))
            .collect();
        Some(Self {
            origin: shape.origin,
            foot,
            material: shape.wood_material,
            leaves_material: shape.leaves_material,
            wood,
            leaves,
            revisions,
        })
    }

    pub fn wood_count(&self) -> usize {
        self.wood.len()
    }

    pub fn foot(&self) -> [i64; 3] {
        self.foot
    }

    pub fn wood_positions(&self) -> impl Iterator<Item = [i64; 3]> + '_ {
        self.wood.iter().copied()
    }

    pub fn duration(&self) -> f32 {
        self.wood.len() as f32 * WOOD_SECONDS
    }

    pub fn duration_with_tool(&self, tool: Option<ItemKind>) -> f32 {
        duration(self.wood_count(), tool)
    }

    pub fn collapse(&self, direction: [f32; 2]) -> Collapse {
        Collapse::from_plan(self, direction)
    }

    /// Cheap per-heartbeat authority check; block edits invalidate entire plans.
    pub fn unchanged(&self, chunks: &ChunkStore, journal: &EditJournal) -> bool {
        self.revisions.iter().all(|&(section, revision)| {
            chunks.loaded.contains_key(&section) && journal.revision(section) == revision
        }) && eligible(chunks, journal, self.foot)
    }
}

/// Gold/ruby skip each third log only in the timer, never in removal or drops.
pub fn duration(wood: usize, tool: Option<ItemKind>) -> f32 {
    use terra_core::ToolKind;
    let (seconds, skip_thirds) = match tool {
        Some(ItemKind::Tool(ToolKind::StoneAxe)) => (0.0925, false),
        Some(ItemKind::Tool(ToolKind::IronAxe)) => (0.08, false),
        Some(ItemKind::Tool(ToolKind::GoldAxe)) => (0.09, true),
        Some(ItemKind::Tool(ToolKind::DiamondAxe)) => (0.05, false),
        Some(ItemKind::Tool(ToolKind::RubyAxe)) => (0.05, true),
        _ => (WOOD_SECONDS, false),
    };
    let timed = wood - if skip_thirds { wood / 3 } else { 0 };
    timed as f32 * seconds
}

pub fn is_axe(tool: Option<ItemKind>) -> bool {
    use terra_core::ToolKind;
    matches!(
        tool,
        Some(ItemKind::Tool(
            ToolKind::WoodAxe
                | ToolKind::StoneAxe
                | ToolKind::IronAxe
                | ToolKind::GoldAxe
                | ToolKind::DiamondAxe
                | ToolKind::RubyAxe
        ))
    )
}

struct Job {
    origin: [i64; 3],
    drop_position: [f64; 3],
    material: u16,
    leaves_material: u16,
    wood: VecDeque<[i64; 3]>,
    leaves: VecDeque<[i64; 3]>,
    owner: PlayerUuid,
    leaves_at: Option<f64>,
    drops: bool,
}

#[derive(Default, Resource)]
pub struct Queue(VecDeque<Job>);

#[derive(Default, Debug, Clone, Copy)]
pub struct Batch {
    pub wood: usize,
    pub leaves: usize,
    pub drops: usize,
}

impl Queue {
    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn enqueue(&mut self, plan: Plan, owner: PlayerUuid) -> bool {
        self.enqueue_with_drops(plan, owner, true)
    }

    pub fn enqueue_with_drops(&mut self, plan: Plan, owner: PlayerUuid, drops: bool) -> bool {
        if self.0.len() >= MAX_JOBS
            || owner.is_nil()
            || self.0.iter().any(|job| job.origin == plan.origin)
        {
            return false;
        }
        self.0.push_back(Job {
            origin: plan.origin,
            drop_position: plan.foot.map(|v| v as f64 + 0.5),
            material: plan.material,
            leaves_material: plan.leaves_material,
            wood: plan.wood.into(),
            leaves: plan.leaves.into(),
            owner,
            leaves_at: None,
            drops,
        });
        true
    }

    pub fn advance(
        &mut self,
        chunks: &mut ChunkStore,
        journal: &mut EditJournal,
        properties: &BlockPropertiesCatalog,
        now: f64,
    ) -> Result<Batch, String> {
        let mut batch = Batch::default();
        if !now.is_finite() {
            return Ok(batch);
        }
        let mut wood_left = WOOD_BUDGET;
        let mut leaves_left = LEAF_BUDGET;
        for _ in 0..self.0.len() {
            let Some(mut job) = self.0.pop_front() else {
                break;
            };
            let result: Result<(), String> = (|| {
                if !job.wood.is_empty() && wood_left > 0 && batch.drops < 4 {
                    // Existing block commits may also detach up to six torches.
                    // Stop before edits if the resource journal cannot retain drops.
                    let count = job.wood.len().min(wood_left);
                    if journal.gameplay().drops.len() + 1 + count * 6 > terra_core::MAX_WORLD_DROPS
                        || journal
                            .gameplay()
                            .next_drop
                            .checked_add((1 + count * 6) as u64)
                            .is_none()
                    {
                        return Ok(());
                    }
                    let mut removed = 0;
                    for _ in 0..count {
                        let cell = *job.wood.front().expect("bounded nonempty wood batch");
                        if remove_natural(chunks, journal, properties, cell, job.material)? {
                            removed += 1;
                        }
                        job.wood.pop_front();
                        wood_left -= 1;
                    }
                    if removed > 0 {
                        let state = journal.gameplay_mut();
                        if job.drops
                            && !state.drop_stack(
                                job.drop_position,
                                ItemStack {
                                    item: ItemKind::Block(job.material),
                                    count: removed as u16,
                                },
                                [0.0, 1.8, 0.0],
                                0.35,
                            )
                        {
                            return Err("Natural wood drop capacity changed during felling".into());
                        }
                        let stats = state.player_stats.entry(job.owner).or_default();
                        stats.blocks_broken = stats.blocks_broken.saturating_add(removed as u64);
                        batch.wood += removed;
                        batch.drops += usize::from(job.drops);
                    }
                }
                if job.wood.is_empty() {
                    let ready = *job.leaves_at.get_or_insert(now + LEAF_DELAY);
                    if now >= ready {
                        for _ in 0..job.leaves.len().min(leaves_left) {
                            let cell = *job.leaves.front().expect("bounded nonempty leaf batch");
                            if remove_natural(
                                chunks,
                                journal,
                                properties,
                                cell,
                                job.leaves_material,
                            )? {
                                batch.leaves += 1;
                            }
                            job.leaves.pop_front();
                            leaves_left -= 1;
                        }
                    }
                }
                Ok(())
            })();
            if !job.wood.is_empty() || !job.leaves.is_empty() {
                self.0.push_back(job);
            }
            result?;
        }
        Ok(batch)
    }

    /// Graceful world exit drains the same bounded commits before taking a save snapshot.
    /// Crash recovery deliberately relies on already committed ordinary section edits.
    pub fn finish(
        &mut self,
        chunks: &mut ChunkStore,
        journal: &mut EditJournal,
        properties: &BlockPropertiesCatalog,
    ) -> Result<(), String> {
        let limit =
            MAX_JOBS * (MAX_WOOD.div_ceil(WOOD_BUDGET) + MAX_LEAVES.div_ceil(LEAF_BUDGET) + 2);
        for _ in 0..limit {
            if self.is_empty() {
                return Ok(());
            }
            let pending: usize = self
                .0
                .iter()
                .map(|job| job.wood.len() + job.leaves.len())
                .sum();
            self.advance(chunks, journal, properties, f64::MAX)?;
            let remaining: usize = self
                .0
                .iter()
                .map(|job| job.wood.len() + job.leaves.len())
                .sum();
            if remaining == pending {
                return Err("Tree felling paused because the world-drop budget is full".into());
            }
        }
        Err("Tree felling exceeded its bounded final-save drain".into())
    }
}

fn remove_natural(
    chunks: &mut ChunkStore,
    journal: &mut EditJournal,
    properties: &BlockPropertiesCatalog,
    cell: [i64; 3],
    material: u16,
) -> Result<bool, String> {
    // Even replacing the same log/leaf material creates an override: never
    // consume it after the player has built or replanted on this position.
    if journal.block(cell).is_some() {
        return Ok(false);
    }
    if let Some(current) = chunks.block_at(cell) {
        if current != material {
            return Ok(false);
        }
        return commit_block_edit(
            chunks,
            journal,
            &BlockEditRequest {
                position: cell,
                expected_material: material,
                material: 0,
            },
            properties,
        );
    }
    chunks.invalidate_sections(journal.record(cell, 0).map_err(|error| error.to_string())?);
    Ok(true)
}

#[cfg(test)]
mod tests;
