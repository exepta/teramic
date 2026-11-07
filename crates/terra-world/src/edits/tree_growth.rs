//! Private, bounded staging for an Air-only tree transaction. Publication moves
//! completed section maps; it never replays thousands of ordinary block edits.
use super::{EditJournal, SectionEdits};
use crate::{props::PropId, streaming::ChunkStore};
use std::collections::{BTreeMap, BTreeSet};
use terra_core::{TreeRecord, TreeStage};
use terra_voxel::{BlockSection, SectionPos, global_to_section};
struct Section {
    stamp: [u64; 4],
    data_revision: u64,
    original_data: BlockSection,
    edits: Option<SectionEdits>,
    data: Option<BlockSection>,
    blocks: usize,
    appearance: bool,
    fluid: bool,
}
pub(crate) struct TreeGrowthBatch {
    original: TreeRecord,
    mature: TreeRecord,
    sections: BTreeMap<SectionPos, Section>,
    pub(crate) cells: Vec<([i64; 3], u16)>,
    pub(crate) cursor: usize,
    kingdom_revision: u64,
    halo: BTreeSet<SectionPos>,
    wake: BTreeSet<[i64; 3]>,
    pub(crate) soft: BTreeSet<(SectionPos, PropId)>,
}
fn stamp(journal: &EditJournal, p: SectionPos) -> [u64; 4] {
    [
        journal.revision(p),
        journal.prop_epoch(p),
        journal.appearance_epoch(p),
        journal.fluid_epoch(p),
    ]
}
impl TreeGrowthBatch {
    pub(crate) fn begin(
        journal: &EditJournal,
        chunks: &ChunkStore,
        id: u64,
        wood: Vec<[i8; 3]>,
        leaves: Vec<[i8; 3]>,
        soft: BTreeSet<(SectionPos, PropId)>,
    ) -> Option<Self> {
        let original = journal.gameplay.trees.get(id)?.clone();
        if original.stage != TreeStage::Sapling
            || original.growth_seconds < original.species.growth_seconds()
            || wood.is_empty()
        {
            return None;
        }
        let mut mature = original.clone();
        mature.stage = TreeStage::Generated;
        mature.wood = wood;
        mature.leaves = leaves;
        mature.refresh_bounds();
        if mature.species == terra_core::TreeSpecies::Apple {
            mature.fruit = Some(terra_core::FruitState::new(
                journal.clock.unwrap_or_default().day,
            ));
        }
        if !mature.valid() {
            return None;
        }
        let mut cells: Vec<_> = mature
            .wood
            .iter()
            .map(|p| (mature.global(*p), mature.species.wood()))
            .chain(
                mature
                    .leaves
                    .iter()
                    .map(|p| (mature.global(*p), mature.species.leaves())),
            )
            .collect();
        cells.sort_unstable_by_key(|(p, _)| {
            let section = global_to_section(*p).0;
            (section.x, section.y, section.z)
        });
        let prop = global_to_section([original.root[0], original.root[1] - 1, original.root[2]]).0;
        let mut owners: BTreeSet<_> = cells.iter().map(|(p, _)| global_to_section(*p).0).collect();
        owners.insert(prop);
        owners.extend(soft.iter().map(|(p, _)| *p));
        if owners.len() > 64 {
            return None;
        }
        let mut sections = BTreeMap::new();
        let mut halo = BTreeSet::new();
        for p in owners {
            let loaded = chunks.loaded.get(&p)?;
            if stamp(journal, p).contains(&u64::MAX)
                || loaded.data.revision() > u64::MAX - cells.len() as u64
            {
                return None;
            }
            sections.insert(
                p,
                Section {
                    stamp: stamp(journal, p),
                    data_revision: loaded.data.revision(),
                    original_data: loaded.data.clone(),
                    edits: None,
                    data: None,
                    blocks: 0,
                    appearance: false,
                    fluid: false,
                },
            );
            for x in -1..=1 {
                for y in -1..=1 {
                    for z in -1..=1 {
                        let p = p.offset([x, y, z]).ok()?;
                        if journal.mesh_epoch(p) == u64::MAX
                            || journal.water_mesh_epoch(p) == u64::MAX
                        {
                            return None;
                        }
                        halo.insert(p);
                    }
                }
            }
        }
        Some(Self {
            wake: BTreeSet::new(),
            soft,
            kingdom_revision: journal.gameplay.kingdoms.revision,
            original,
            mature,
            sections,
            cells,
            cursor: 0,
            halo,
        })
    }
    fn initialize(&mut self, p: SectionPos, journal: &EditJournal) {
        let section = self.sections.get_mut(&p).expect("captured tree section");
        let mut edits = journal.sections.get(&p).cloned().unwrap_or_default();
        let support = global_to_section([
            self.original.root[0],
            self.original.root[1] - 1,
            self.original.root[2],
        ])
        .0;
        if p == support {
            edits.removed_props.insert(PropId(self.original.id));
            edits.placed_props.remove(&PropId(self.original.id));
            edits.prop_epoch += 1;
        }
        for &(owner, id) in &self.soft {
            if owner == p {
                edits.removed_props.insert(id);
                if edits.prop_epoch == section.stamp[1] {
                    edits.prop_epoch += 1;
                }
            }
        }
        section.edits = Some(edits);
        section.data = Some(section.original_data.clone());
    }
    pub(crate) fn requested_sections(&self) -> impl Iterator<Item = SectionPos> + '_ {
        self.sections.keys().copied()
    }
    pub(crate) fn unchanged(&self, journal: &EditJournal, chunks: &ChunkStore) -> bool {
        journal.gameplay.trees.get(self.original.id) == Some(&self.original)
            && journal.gameplay.kingdoms.revision == self.kingdom_revision
            && self.sections.iter().all(|(p, section)| {
                stamp(journal, *p) == section.stamp
                    && chunks.loaded.get(p).is_some_and(|l| {
                        l.data.revision() == section.data_revision
                            && l.data == section.original_data
                    })
            })
    }
    pub(crate) fn advance(
        &mut self,
        journal: &EditJournal,
        chunks: &ChunkStore,
        budget: usize,
    ) -> Result<bool, String> {
        if !self.unchanged(journal, chunks) {
            return Err("Tree growth environment changed".into());
        }
        let actor = self.original.planted_by.ok_or("Missing tree owner")?;
        let end = (self.cursor + budget).min(self.cells.len());
        let mut initialized = 0;
        for index in self.cursor..end {
            let (p, material) = self.cells[index];
            let (owner, local) = global_to_section(p);
            if self.sections[&owner].edits.is_none() {
                if initialized == 1 {
                    return Ok(false);
                }
                self.initialize(owner, journal);
                initialized += 1;
            }
            if chunks.block_at(p) != Some(0)
                || journal.fluid(p).is_some_and(|f| f.amount > 0)
                || journal.object_state(p).is_some()
                || !crate::kingdom_access::can_use(
                    &journal.gameplay.kingdoms,
                    chunks,
                    actor,
                    terra_core::kingdom::KingdomPermission::Build,
                    p,
                )
                || journal.gameplay.kingdoms.active_at(p).is_some()
            {
                return Err("Tree growth is blocked".into());
            }
            let section = self
                .sections
                .get_mut(&owner)
                .expect("captured tree section");
            let edits = section.edits.as_mut().expect("initialized tree journal");
            edits.blocks.insert(local, material);
            edits.tints.remove(&local);
            edits.belts.remove(&local);
            section.appearance |= edits.surface_conditions.remove(&local).is_some();
            section.fluid |= edits.fluids.remove(&local).is_some();
            section
                .data
                .as_mut()
                .expect("initialized tree blocks")
                .set_block(local, material)
                .map_err(|e| e.to_string())?;
            section.blocks += 1;
            self.cursor = index + 1;
            for d in [
                [-1, 0, 0],
                [1, 0, 0],
                [0, -1, 0],
                [0, 1, 0],
                [0, 0, -1],
                [0, 0, 1],
            ] {
                let n = std::array::from_fn(|a| p[a] + d[a]);
                if matches!(chunks.block_at(n), Some(14 | 27))
                    || journal.fluid(n).is_some_and(|f| f.amount > 0)
                {
                    self.wake.insert(n);
                }
            }
        }
        if self.cursor == self.cells.len() {
            if let Some(p) = self
                .sections
                .iter()
                .find_map(|(p, section)| section.edits.is_none().then_some(*p))
            {
                if initialized == 0 {
                    self.initialize(p, journal);
                }
                return Ok(false);
            }
            return Ok(true);
        }
        Ok(false)
    }
    pub(crate) fn publish(
        mut self,
        journal: &mut EditJournal,
        chunks: &mut ChunkStore,
    ) -> Result<bool, String> {
        if self.cursor != self.cells.len()
            || self
                .sections
                .values()
                .any(|section| section.edits.is_none())
            || !self.unchanged(journal, chunks)
            || journal.gameplay.trees.revision > u64::MAX - 2
            || self.original.revision > u64::MAX - 2
        {
            return Ok(false);
        }
        for &p in &self.halo {
            if journal.mesh_epoch(p) == u64::MAX || journal.water_mesh_epoch(p) == u64::MAX {
                return Ok(false);
            }
        }
        self.mature.revision = self.original.revision + 1;
        for (p, mut section) in self.sections {
            let mut edits = section.edits.take().expect("fully prepared tree journal");
            edits.revision += u64::from(section.blocks > 0);
            edits.appearance_epoch += u64::from(section.appearance);
            edits.fluid_epoch += u64::from(section.fluid);
            journal.sections.insert(p, edits);
            let loaded = chunks.loaded.get_mut(&p).expect("revalidated tree section");
            loaded.data = section.data.take().expect("fully prepared tree blocks");
            for prop in &mut loaded.props {
                if journal.prop_removed(p, prop.id) {
                    prop.removed = true;
                }
            }
        }
        for &p in &self.halo {
            *journal.mesh_epochs.entry(p).or_default() += 1;
            *journal.water_mesh_epochs.entry(p).or_default() += 1;
        }
        for p in self.wake {
            journal.wake_fluid_neighbors(p);
        }
        journal
            .gameplay
            .trees
            .trees
            .insert(self.original.id, self.mature);
        journal.gameplay.trees.changed();
        chunks.invalidate_sections(self.halo);
        Ok(true)
    }
}
