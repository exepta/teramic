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

pub const MAX_WOOD: usize = terra_core::trees::MAX_TREE_WOOD;
pub const MAX_LEAVES: usize = terra_core::trees::MAX_TREE_LEAVES;
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

/// Saved exact offsets avoid rebuilding procedural geometry in mining previews.
/// Wide generated trunks expose several cells in their lowest three levels;
/// legacy compact trees retain their original root-only interaction.
fn planted_tree(journal: &EditJournal, foot: [i64; 3]) -> Option<&terra_core::TreeRecord> {
    journal.gameplay().trees.trees.values().find(|tree| {
        tree.planted_by.is_some()
            && tree.stage.is_mature()
            && if tree.stage.is_generated() {
                tree.offset(foot).is_some_and(|offset| {
                    (0..=2).contains(&offset[1])
                        && tree.could_own(foot)
                        && tree.wood.contains(&offset)
                })
            } else {
                tree.root == foot && tree.wood.contains(&[0; 3])
            }
    })
}

/// Natural wood and attributed lower planted trunks opt into whole-tree felling.
pub fn eligible(chunks: &ChunkStore, journal: &EditJournal, foot: [i64; 3]) -> bool {
    if let Some(t) = planted_tree(journal, foot) {
        return chunks.block_at(foot) == Some(t.species.wood());
    }
    journal.block(foot).is_none()
        && chunks
            .block_at(foot)
            .is_some_and(|m| matches!(m, 6 | 10 | 36 | 134 | 313))
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
    if let Some(t) = planted_tree(journal, foot) {
        let sections: std::collections::BTreeSet<_> = t
            .wood
            .iter()
            .chain(&t.leaves)
            .map(|p| global_to_section(t.global(*p)).0)
            .collect();
        chunks.request_sections(sections);
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
    planted: Option<u64>,
    species: Option<terra_core::TreeSpecies>,
    origin: [i64; 3],
    foot: [i64; 3],
    material: u16,
    leaves_material: u16,
    wood: Vec<[i64; 3]>,
    leaves: Vec<[i64; 3]>,
    revisions: Vec<(SectionPos, u64)>,
    /// Immutable affiliation survives eviction; permissions remain live registry state.
    domains: Vec<terra_core::kingdom::SettlementDomain>,
}

impl Plan {
    /// Exact saved attribution for productive NPCs; never guesses connected wooden buildings.
    pub(crate) fn worker_task(&self) -> Option<terra_core::npc_jobs::TreeWork> {
        let task = terra_core::npc_jobs::TreeWork {
            root: self.origin,
            foot: self.foot,
            species: self.species?,
            planted: self.planted,
            wood: self.wood.clone(),
            leaves: self.leaves.clone(),
            drops: Vec::new(),
            stage: terra_core::npc_jobs::TreeWorkStage::Cutting,
            sapling_loaded: false,
            sapling_dropped: false,
            started: false,
        };
        task.valid().then_some(task)
    }
    /// Reads immutable prepared primitives; never samples terrain noise or flood-fills wood.
    pub fn prepare(chunks: &ChunkStore, journal: &EditJournal, foot: [i64; 3]) -> Option<Self> {
        if !eligible(chunks, journal, foot) {
            return None;
        }
        if let Some(tree) = planted_tree(journal, foot) {
            return Self::planted(chunks, journal, tree, foot);
        }
        let coordinate = [foot[0].div_euclid(16), foot[2].div_euclid(16)];
        let first = chunks.prepared_column(coordinate)?;
        let shape = first.felling_shape(foot)?;
        if first.felling_owner(foot, shape.wood_material) != Some(shape.origin) {
            return None;
        }
        let tree_species = first
            .felling_species(shape.origin)
            .or_else(|| species(shape.wood_material, shape.leaves_material));
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
        let low: [i64; 2] = [0, 2].map(|axis| {
            wood.iter()
                .chain(&leaves)
                .map(|p| p[axis])
                .min()
                .unwrap()
                .saturating_sub(1)
        });
        let high: [i64; 2] = [0, 2].map(|axis| {
            wood.iter()
                .chain(&leaves)
                .map(|p| p[axis])
                .max()
                .unwrap()
                .saturating_add(1)
        });
        let mut domains = std::collections::BTreeMap::new();
        for site in chunks.village_settlements() {
            let domain = site.domain();
            if [0, 2].into_iter().enumerate().all(|(i, axis)| {
                domain.bounds[0][axis] <= high[i] && domain.bounds[1][axis] >= low[i]
            }) {
                domains.entry(domain.id).or_insert(domain);
            }
        }
        Some(Self {
            planted: None,
            species: tree_species,
            domains: domains.into_values().collect(),
            origin: shape.origin,
            foot,
            material: shape.wood_material,
            leaves_material: shape.leaves_material,
            wood,
            leaves,
            revisions,
        })
    }

    fn planted(
        chunks: &ChunkStore,
        journal: &EditJournal,
        tree: &terra_core::TreeRecord,
        foot: [i64; 3],
    ) -> Option<Self> {
        let mut wood = Vec::new();
        for offset in &tree.wood {
            let p = tree.global(*offset);
            if chunks.block_at(p)? == tree.species.wood() {
                wood.push(p);
            }
        }
        if !wood.contains(&foot) {
            return None;
        }
        wood.sort_unstable_by_key(|p| (std::cmp::Reverse(p[1]), p[2], p[0]));
        let leaves: Vec<_> = tree
            .leaves
            .iter()
            .map(|p| tree.global(*p))
            .filter(|p| journal.block(*p) == Some(tree.species.leaves()))
            .collect();
        let sections: std::collections::BTreeSet<_> =
            wood.iter().map(|p| global_to_section(*p).0).collect();
        let revisions = sections
            .into_iter()
            .map(|p| (p, journal.revision(p)))
            .collect();
        let domains = chunks
            .village_settlements()
            .filter_map(|site| {
                let d = site.domain();
                wood.iter()
                    .chain(&leaves)
                    .any(|p| {
                        [0, 2].into_iter().all(|axis| {
                            p[axis] >= d.bounds[0][axis].saturating_sub(1)
                                && p[axis] <= d.bounds[1][axis].saturating_add(1)
                        })
                    })
                    .then_some(d)
            })
            .collect();
        Some(Self {
            planted: Some(tree.id),
            species: Some(tree.species),
            origin: tree.root,
            foot,
            material: tree.species.wood(),
            leaves_material: tree.species.leaves(),
            wood,
            leaves,
            revisions,
            domains,
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
    pub fn allowed_for(
        &self,
        player: PlayerUuid,
        chunks: &ChunkStore,
        journal: &EditJournal,
    ) -> bool {
        self.wood
            .iter()
            .chain(&self.leaves)
            .all(|&p| break_allowed(chunks, journal, player, p, &self.domains))
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

/// A root-only NPC target avoids several workers reserving separate branches of one tree.
pub(crate) fn worker_root(chunks: &ChunkStore, journal: &EditJournal, cell: [i64; 3]) -> bool {
    if let Some(tree) = planted_tree(journal, cell) {
        return tree.root == cell;
    }
    eligible(chunks, journal, cell)
        && chunks
            .prepared_column([cell[0].div_euclid(16), cell[2].div_euclid(16)])
            .is_some_and(|column| {
                column.felling_owner(cell, chunks.block_at(cell).unwrap_or(0)) == Some(cell)
            })
}

/// One attributed commit and its real item drop form one worker operation. Capacity
/// is checked before removal, and saved progress is advanced only after that commit.
pub(crate) fn worker_cut_one(
    chunks: &mut ChunkStore,
    journal: &mut EditJournal,
    properties: &BlockPropertiesCatalog,
    owner: PlayerUuid,
    task: &mut terra_core::npc_jobs::TreeWork,
    wood: bool,
) -> Result<bool, terra_core::npc_jobs::WaitReason> {
    use terra_core::npc_jobs::WaitReason;
    let Some(&cell) = (if wood { &task.wood } else { &task.leaves }).last() else {
        return Ok(false);
    };
    let Some(material) = chunks.block_at(cell) else {
        return Err(WaitReason::Unloaded);
    };
    if !break_allowed(chunks, journal, owner, cell, &[]) {
        return Err(WaitReason::Protected);
    }
    // The saved plan's cell list is exact; using the actual material also keeps
    // autumn/white crowns and historic wood skins instead of canonical species
    // materials. Every player edit remains excluded by remove_owned below.
    let correct_kind = if wood {
        matches!(material, 6 | 10 | 36 | 134 | 313)
    } else {
        matches!(material, 7..=9 | 132 | 133 | 195 | 310..=312)
    };
    let sapling = !wood && crate::saplings::leaf_drop_roll(cell);
    let extra = usize::from(wood || sapling);
    if journal.gameplay().drops.len() + extra + 6 > terra_core::MAX_WORLD_DROPS
        || journal
            .gameplay()
            .next_drop
            .checked_add((extra + 6) as u64)
            .is_none()
    {
        return Err(WaitReason::StorageFull);
    }
    let removed = correct_kind
        && remove_owned(chunks, journal, properties, cell, material, task.planted)
            .map_err(|_| WaitReason::Protected)?;
    if wood {
        task.wood.pop();
    } else {
        task.leaves.pop();
    }
    if removed && (wood || sapling) {
        let at = task.foot.map(|v| v as f64 + 0.5);
        let game = journal.gameplay_mut();
        let item = if wood {
            ItemKind::Block(material)
        } else {
            ItemKind::Prop(task.species.sapling())
        };
        if !game.drop_one(at, item) {
            return Err(WaitReason::StorageFull);
        }
        task.drops.push(game.next_drop);
        if sapling {
            // Retained serialized legacy flag; it no longer grants a free seed
            // on the first log or suppresses any later independent leaf roll.
            task.sapling_dropped = true;
        }
    }
    Ok(removed)
}

/// Gold/ruby skip each third log only in the timer, never in removal or drops.
pub fn duration(wood: usize, tool: Option<ItemKind>) -> f32 {
    use terra_core::ToolKind;
    let (seconds, skip_thirds) = match tool.map(ItemKind::preview_item) {
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
        tool.map(ItemKind::preview_item),
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
    planted: Option<u64>,
    species: Option<terra_core::TreeSpecies>,
    origin: [i64; 3],
    drop_position: [f64; 3],
    material: u16,
    leaves_material: u16,
    wood: VecDeque<[i64; 3]>,
    leaves: VecDeque<[i64; 3]>,
    owner: PlayerUuid,
    leaves_at: Option<f64>,
    drops: bool,
    domains: Vec<terra_core::kingdom::SettlementDomain>,
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
            planted: plan.planted,
            species: plan.species,
            origin: plan.origin,
            drop_position: plan.foot.map(|v| v as f64 + 0.5),
            material: plan.material,
            leaves_material: plan.leaves_material,
            wood: plan.wood.into(),
            leaves: plan.leaves.into(),
            owner,
            leaves_at: None,
            drops,
            domains: plan.domains,
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
                if !job.wood.is_empty()
                    && wood_left > 0
                    && batch.drops + usize::from(job.drops) <= 4
                {
                    // Existing block commits may also detach up to six torches.
                    // Stop before edits if the resource journal cannot retain drops.
                    let count = job.wood.len().min(wood_left);
                    if journal.gameplay().drops.len() + 2 + count * 6 > terra_core::MAX_WORLD_DROPS
                        || journal
                            .gameplay()
                            .next_drop
                            .checked_add((2 + count * 6) as u64)
                            .is_none()
                    {
                        return Ok(());
                    }
                    let mut removed = 0;
                    for _ in 0..count {
                        let cell = *job.wood.front().expect("bounded nonempty wood batch");
                        if break_allowed(chunks, journal, job.owner, cell, &job.domains)
                            && remove_owned(
                                chunks,
                                journal,
                                properties,
                                cell,
                                job.material,
                                job.planted,
                            )?
                        {
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
                    if now >= ready && batch.drops + usize::from(job.drops) <= 4 {
                        // At most one real sapling stack per bounded decay batch;
                        // attached objects can additionally detach up to six drops
                        // per committed cell, just as during trunk removal.
                        let count = job.leaves.len().min(leaves_left);
                        if journal.gameplay().drops.len() + 1 + count * 6
                            > terra_core::MAX_WORLD_DROPS
                            || journal
                                .gameplay()
                                .next_drop
                                .checked_add((1 + count * 6) as u64)
                                .is_none()
                        {
                            return Ok(());
                        }
                        let mut saplings = 0;
                        for _ in 0..count {
                            let cell = *job.leaves.front().expect("bounded nonempty leaf batch");
                            if break_allowed(chunks, journal, job.owner, cell, &job.domains)
                                && remove_owned(
                                    chunks,
                                    journal,
                                    properties,
                                    cell,
                                    job.leaves_material,
                                    job.planted,
                                )?
                            {
                                batch.leaves += 1;
                                if job.drops
                                    && job.species.is_some()
                                    && crate::saplings::leaf_drop_roll(cell)
                                {
                                    saplings += 1;
                                }
                            }
                            job.leaves.pop_front();
                            leaves_left -= 1;
                        }
                        if saplings > 0
                            && let Some(species) = job.species
                        {
                            if !journal.gameplay_mut().drop_stack(
                                job.drop_position,
                                ItemStack {
                                    item: ItemKind::Prop(species.sapling()),
                                    count: saplings,
                                },
                                [0.0, 1.8, 0.0],
                                0.35,
                            ) {
                                return Err("Tree leaf drop capacity changed during decay".into());
                            }
                            batch.drops += 1;
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

/// Breaking a natural support can detach decoration in a neighboring claim.
/// Initial attribution and every later batch use the same live footprint check.
fn break_allowed(
    chunks: &ChunkStore,
    journal: &EditJournal,
    actor: PlayerUuid,
    cell: [i64; 3],
    domains: &[terra_core::kingdom::SettlementDomain],
) -> bool {
    if !crate::rails::support_edit_allowed(chunks, journal, cell, 0) {
        return false;
    }
    let allowed = |p| {
        let registry = &journal.gameplay().kingdoms;
        if let Some(domain) = domains.iter().find(|domain| domain.contains(p)) {
            return registry.can_in_domain(
                actor,
                terra_core::kingdom::KingdomPermission::Break,
                p,
                domain,
            );
        }
        crate::kingdom_access::can_use(
            &journal.gameplay().kingdoms,
            chunks,
            actor,
            terra_core::kingdom::KingdomPermission::Break,
            p,
        )
    };
    allowed(cell)
        && crate::lanterns::dependents(chunks, journal, cell)
            .into_iter()
            .all(allowed)
        && [
            [-1, 0, 0],
            [1, 0, 0],
            [0, -1, 0],
            [0, 1, 0],
            [0, 0, -1],
            [0, 0, 1],
        ]
        .into_iter()
        .all(|offset| {
            let neighbor = std::array::from_fn(|axis| cell[axis].saturating_add(offset[axis]));
            chunks
                .block_at(neighbor)
                .or_else(|| journal.block(neighbor))
                != Some(40)
                || journal.object_state(neighbor).is_none_or(|state| {
                    crate::torches::support(neighbor, state.facing) != Some(cell)
                        || allowed(neighbor)
                })
        })
}

fn species(wood: u16, leaves: u16) -> Option<terra_core::TreeSpecies> {
    use terra_core::TreeSpecies;
    Some(match (wood, leaves) {
        (6, 195) => TreeSpecies::Apple,
        (6, 7 | 8) => TreeSpecies::Oak,
        (10, 9) => TreeSpecies::Birch,
        (36, 7) => TreeSpecies::Spruce,
        (134, 132 | 133) => TreeSpecies::Cherry,
        _ => return None,
    })
}
fn remove_owned(
    chunks: &mut ChunkStore,
    journal: &mut EditJournal,
    properties: &BlockPropertiesCatalog,
    cell: [i64; 3],
    material: u16,
    planted: Option<u64>,
) -> Result<bool, String> {
    let Some(id) = planted else {
        return remove_natural(chunks, journal, properties, cell, material);
    };
    if !journal
        .gameplay()
        .trees
        .get(id)
        .is_some_and(|tree| tree.owns(cell, material))
        || journal.block(cell) != Some(material)
    {
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
    chunks.invalidate_sections(journal.record(cell, 0).map_err(|e| e.to_string())?);
    Ok(true)
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
