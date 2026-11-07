use super::EditJournal;
use terra_core::kinetic::{Cell, KineticState};

impl EditJournal {
    pub fn has_kinetics(&self) -> bool {
        !self.kinetic_sections.is_empty()
    }
    /// Only sections containing recorded mechanical parts, never empty terrain.
    pub fn kinetic_sections(&self) -> impl ExactSizeIterator<Item = terra_voxel::SectionPos> + '_ {
        self.kinetic_sections.keys().copied()
    }
    pub fn has_extended_kinetics(&self) -> bool {
        !self.extended_kinetic_sections.is_empty()
    }
    pub fn extended_kinetics_in_section(
        &self,
        section: terra_voxel::SectionPos,
    ) -> impl Iterator<Item = Cell> + '_ {
        self.extended_kinetic_sections
            .get(&section)
            .into_iter()
            .flat_map(|p| p.iter().copied())
    }
    /// Overhangs reach only the immediate neighboring cells, including across
    /// section boundaries. Empty sections require one index lookup.
    pub fn visit_extended_kinetics_near(&self, position: Cell, mut visit: impl FnMut(Cell)) {
        if !self.has_extended_kinetics() {
            return;
        }
        let low = terra_voxel::global_to_section(position.map(|v| v - 1)).0;
        let high = terra_voxel::global_to_section(position.map(|v| v + 1)).0;
        for x in low.x..=high.x {
            for y in low.y..=high.y {
                for z in low.z..=high.z {
                    for owner in
                        self.extended_kinetics_in_section(terra_voxel::SectionPos { x, y, z })
                    {
                        if (0..3).all(|i| (owner[i] - position[i]).abs() <= 1) {
                            visit(owner);
                        }
                    }
                }
            }
        }
    }

    /// Saved placements are indexed by section so the authority can query only
    /// loaded sections without walking unloaded construction history.
    pub fn kinetics_in_section(
        &self,
        section: terra_voxel::SectionPos,
    ) -> impl Iterator<Item = Cell> + '_ {
        self.kinetic_sections
            .get(&section)
            .into_iter()
            .flat_map(|positions| positions.iter().copied())
    }
    pub fn kinetic_state(&self, position: Cell) -> KineticState {
        self.kinetic_states
            .get(&position)
            .copied()
            .unwrap_or_default()
    }
    pub fn replace_kinetic_states(
        &mut self,
        states: std::collections::BTreeMap<Cell, KineticState>,
    ) {
        self.kinetic_states = states;
    }
    pub fn kinetic_states(&self) -> impl Iterator<Item = (Cell, KineticState)> + '_ {
        self.kinetic_states.iter().map(|(&p, &s)| (p, s))
    }
    /// A generic crank uses the same attachment metadata and validation as the
    /// historical crusher crank, but mounts one cell from a shaft or gear.
    pub fn rotary_crank_mount(&self, anchor: Cell, facing: u8) -> Option<Cell> {
        let material = self.block(anchor)?;
        let state = self.object_state(anchor)?;
        let direction = terra_core::kinetic::axis(facing);
        (terra_core::kinetic::is_crank_mount(material)
            && state.valid_for(anchor, material)
            && terra_core::kinetic::has_shaft_port(material, state.facing, direction))
        .then(|| terra_core::kinetic::offset(anchor, direction))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn saved_cross_section_axes_rebuild_index_without_runtime_power() {
        let mut journal = EditJournal::default();
        for p in [[15, 4, 0], [16, 4, 0], [17, 4, 0]] {
            journal.record(p, 144).unwrap();
            journal
                .set_object_state(p, Some(Default::default()))
                .unwrap();
        }
        let mut restored = EditJournal::default();
        for section in journal.changed_sections() {
            restored
                .import_delta(journal.section_delta(section))
                .unwrap();
        }
        assert_eq!(
            restored
                .kinetics_in_section(terra_voxel::SectionPos::default())
                .collect::<Vec<_>>(),
            [[15, 4, 0]]
        );
        let adjacent = terra_voxel::global_to_section([16, 4, 0]).0;
        assert_eq!(
            restored.kinetics_in_section(adjacent).collect::<Vec<_>>(),
            [[16, 4, 0], [17, 4, 0]]
        );
        assert_eq!(restored.kinetic_state([15, 4, 0]), KineticState::default());
        restored.record([16, 4, 0], 0).unwrap();
        assert_eq!(
            restored.kinetics_in_section(adjacent).collect::<Vec<_>>(),
            [[17, 4, 0]]
        );
        restored.record([17, 4, 0], 0).unwrap();
        assert!(restored.kinetics_in_section(adjacent).next().is_none());
    }

    #[test]
    fn vertical_shaft_accepts_only_a_direct_matching_crank_attachment() {
        let mut journal = EditJournal::default();
        let shaft = [3, 3, 3];
        let crank = [3, 4, 3];
        journal.record(shaft, 144).unwrap();
        journal
            .set_object_state(
                shaft,
                Some(super::super::BlockObjectState {
                    facing: 4,
                    ..Default::default()
                }),
            )
            .unwrap();
        journal.record(crank, 100).unwrap();
        journal
            .set_object_state(
                crank,
                Some(super::super::BlockObjectState {
                    facing: 4,
                    peer: Some(shaft),
                    ..Default::default()
                }),
            )
            .unwrap();
        assert_eq!(journal.crank_anchor(crank), Some(shaft));
        assert_eq!(journal.rotary_crank_mount(shaft, 0), None);
        assert!(
            journal
                .set_object_state(
                    crank,
                    Some(super::super::BlockObjectState {
                        facing: 0,
                        peer: Some(shaft),
                        ..Default::default()
                    })
                )
                .is_err()
        );
    }
}
