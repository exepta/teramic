use crate::{edits::EditJournal, streaming::ChunkStore};
use std::{
    collections::{BTreeSet, HashMap, VecDeque},
    sync::Arc,
};
use terra_core::{TreeRecord, trees::TreeGeometryKey};
use terra_voxel::SectionPos;
#[derive(Default)]
pub(crate) struct AppearanceCache {
    entries: HashMap<u64, (TreeGeometryKey, Arc<TreeRecord>)>,
    order: VecDeque<u64>,
}
impl EditJournal {
    /// Cheap keys ignore fruit/progress changes. Geometry is copied once per
    /// actual attribution change, then shared by all section workers.
    pub fn planted_appearance_records(&self, section: SectionPos) -> Vec<Arc<TreeRecord>> {
        let Ok(origin) = section.origin() else {
            return vec![];
        };
        let records = self.gameplay().trees.trees.values().filter(|tree| {
            tree.stage.is_generated()
                && tree.bounds.is_some_and(|b| {
                    (0..3).all(|a| {
                        tree.root[a] + i64::from(b[1][a]) >= origin[a] - 1
                            && tree.root[a] + i64::from(b[0][a]) <= origin[a] + 16
                    })
                })
        });
        let Ok(mut cache) = self.planted_appearances.lock() else {
            return records.map(|tree| Arc::new(tree.clone())).collect();
        };
        records
            .map(|tree| {
                let key = tree.geometry_key();
                if let Some((old, record)) = cache.entries.get(&tree.id)
                    && *old == key
                {
                    return record.clone();
                }
                let record = Arc::new(tree.clone());
                if !cache.entries.contains_key(&tree.id) {
                    if cache.entries.len() >= 128
                        && let Some(id) = cache.order.pop_front()
                    {
                        cache.entries.remove(&id);
                    }
                    cache.order.push_back(tree.id);
                }
                cache.entries.insert(tree.id, (key, record.clone()));
                record
            })
            .collect()
    }
}
/// All counters are preflighted before any side effect. Copy-only geometry keys
/// make ordinary fruit/progress messages constant-cost per record.
pub fn invalidate_tree_visuals(
    chunks: &mut ChunkStore,
    journal: &mut EditJournal,
    before: Option<TreeGeometryKey>,
    after: Option<TreeGeometryKey>,
) -> Result<(), String> {
    if before == after {
        return Ok(());
    }
    let mut sections = BTreeSet::new();
    for key in before
        .into_iter()
        .chain(after)
        .filter(|k| k.stage.is_generated())
    {
        let Some(b) = key.bounds else { continue };
        let low = [0, 1, 2].map(|a| (key.root[a] + i64::from(b[0][a])).div_euclid(16));
        let high = [0, 1, 2].map(|a| (key.root[a] + i64::from(b[1][a])).div_euclid(16));
        for x in low[0]..=high[0] {
            for y in low[1]..=high[1] {
                for z in low[2]..=high[2] {
                    let p = SectionPos::new(x, y, z);
                    if chunks.loaded.contains_key(&p) {
                        sections.insert(p);
                    }
                }
            }
        }
    }
    journal
        .invalidate_structure_meshes(&sections)
        .map_err(|e| e.to_string())?;
    chunks.invalidate_sections(sections);
    Ok(())
}
