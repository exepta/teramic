use super::*;
#[test]
fn every_pot_content_roundtrips_at_negative_section_edges_without_extra_plant_props() {
    for position in [[-1, 15, -16], [-16, -1, -17], [16, 16, 16]] {
        for facing in 0..terra_core::flower_pot::STATES {
            let state = BlockObjectState {
                facing,
                ..Default::default()
            };
            assert!(state.valid_for(position, terra_core::flower_pot::MATERIAL));
            let mut journal = EditJournal::default();
            journal
                .record(position, terra_core::flower_pot::MATERIAL)
                .unwrap();
            journal.set_object_state(position, Some(state)).unwrap();
            let owner = global_to_section(position).0;
            let delta = journal.section_delta(owner);
            assert!(delta.placed_props.is_empty());
            let mut restored = EditJournal::default();
            restored.import_delta(delta.clone()).unwrap();
            assert_eq!(restored.object_state(position), Some(state));
            assert_eq!(restored.section_delta(owner), delta);
            restored.record(position, 0).unwrap();
            assert_eq!(restored.object_state(position), None);
        }
    }
    assert!(
        !BlockObjectState {
            facing: terra_core::flower_pot::STATES,
            ..Default::default()
        }
        .valid_for([0; 3], terra_core::flower_pot::MATERIAL)
    );
}
