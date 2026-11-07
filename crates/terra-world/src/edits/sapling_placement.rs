//! A tiny planting transaction stages only the affected sparse prop sections.
use super::EditJournal;
use crate::props::{PropId, WorldProp};
use std::collections::{BTreeMap, BTreeSet};
use terra_voxel::SectionPos;

impl EditJournal {
    pub(crate) fn sapling_prop_fits(
        &self,
        prop: &WorldProp,
        soft: &BTreeSet<(SectionPos, PropId)>,
    ) -> bool {
        if !prop.valid() || prop.removed || self.prop_removed(prop.section, prop.id) {
            return false;
        }
        let mut counts = BTreeMap::<_, usize>::from([(prop.section, 0)]);
        for &(owner, id) in soft {
            if !self.prop_removed(owner, id) {
                *counts.entry(owner).or_default() += 1;
            }
        }
        counts.into_iter().all(|(owner, removed)| {
            let section = self.sections.get(&owner);
            let placed = section.map_or(0, |s| s.placed_props.len());
            let tombstones = section.map_or(0, |s| s.removed_props.len());
            let insert = usize::from(owner == prop.section);
            placed + insert <= 256
                && placed + tombstones + removed + insert <= 3840
                && self
                    .prop_epoch(owner)
                    .checked_add((removed + insert) as u64)
                    .is_some()
                && (owner != prop.section
                    || !section.is_some_and(|s| s.placed_props.contains_key(&prop.id)))
        })
    }
    pub(crate) fn place_sapling_prop(
        &mut self,
        prop: WorldProp,
        soft: &BTreeSet<(SectionPos, PropId)>,
    ) -> bool {
        if !self.sapling_prop_fits(&prop, soft) {
            return false;
        }
        let mut staged = BTreeMap::new();
        for owner in std::iter::once(prop.section).chain(soft.iter().map(|(p, _)| *p)) {
            staged
                .entry(owner)
                .or_insert_with(|| self.sections.get(&owner).cloned().unwrap_or_default());
        }
        for &(owner, id) in soft {
            let section = staged.get_mut(&owner).expect("captured prop section");
            if section.removed_props.insert(id) {
                section.prop_epoch += 1;
            }
        }
        let section = staged
            .get_mut(&prop.section)
            .expect("captured sapling section");
        section.placed_props.insert(prop.id, prop);
        section.prop_epoch += 1;
        self.sections.extend(staged);
        true
    }
}
