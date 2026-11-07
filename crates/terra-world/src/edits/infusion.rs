use super::EditJournal;
use terra_core::infusion::HoldVisual;
use terra_voxel::SectionPos;

impl EditJournal {
    pub fn has_holds(&self) -> bool {
        !self.hold_sections.is_empty()
    }
    pub fn holds_in_section(&self, section: SectionPos) -> impl Iterator<Item = [i64; 3]> + '_ {
        self.hold_sections
            .get(&section)
            .into_iter()
            .flat_map(|cells| cells.iter().copied())
    }
    pub fn infusion_view(&self, position: [i64; 3]) -> Option<HoldVisual> {
        self.infusion_views.get(&position).copied()
    }
    pub(crate) fn replace_infusion_views(&mut self, views: impl IntoIterator<Item = HoldVisual>) {
        self.infusion_views = views
            .into_iter()
            .take(terra_core::MAX_WORLD_DROPS)
            .filter(|view| view.valid())
            .map(|view| (view.position, view))
            .collect();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn hold_index_survives_delta_reload_and_views_never_change_stored_revision() {
        let mut journal = EditJournal::default();
        let p = [-17, 4, -1];
        let owner = terra_voxel::global_to_section(p).0;
        journal.record(p, terra_core::infusion::HOLD_BLOCK).unwrap();
        journal
            .set_object_state(p, Some(Default::default()))
            .unwrap();
        assert_eq!(journal.holds_in_section(owner).collect::<Vec<_>>(), vec![p]);
        let delta = journal.section_delta(owner);
        let revision = journal.gameplay().revision;
        journal.replace_infusion_views([HoldVisual {
            position: p,
            ..Default::default()
        }]);
        assert_eq!(journal.gameplay().revision, revision);
        let mut replica = EditJournal::default();
        replica.import_delta(delta).unwrap();
        assert_eq!(replica.holds_in_section(owner).collect::<Vec<_>>(), vec![p]);
        assert!(replica.infusion_view(p).is_none());
        journal.record(p, 3).unwrap();
        assert!(!journal.has_holds());
        assert!(journal.infusion_view(p).is_none());
    }
}
