use super::*;
use terra_voxel::global_to_section;

#[test]
fn tint_only_changes_never_invalidate_voxels_water_or_collision_and_replicate_exactly() {
    let mut host = EditJournal::default();
    let position = [-17, 15, 16];
    let owner = global_to_section(position).0;
    host.record(position, 103).unwrap();
    let mut replica = EditJournal::default();
    replica.import_delta(host.section_delta(owner)).unwrap();
    let mesh = host.mesh_epoch(owner);
    let water = host.water_mesh_epoch(owner);
    let revision = host.revision(owner);
    let appearance = host.appearance_epoch(owner);
    let replica_mesh = replica.mesh_epoch(owner);
    let replica_water = replica.water_mesh_epoch(owner);
    assert!(host.set_tint(position, Some([17, 201, 63])).unwrap());
    assert!(!host.set_tint(position, Some([17, 201, 63])).unwrap());
    assert_eq!(host.revision(owner), revision + 1);
    assert_eq!(host.appearance_epoch(owner), appearance + 1);
    assert_eq!(host.mesh_epoch(owner), mesh);
    assert_eq!(host.water_mesh_epoch(owner), water);
    assert!(
        replica
            .import_delta(host.section_delta(owner))
            .unwrap()
            .is_empty()
    );
    assert_eq!(replica.tint_at(position), Some([17, 201, 63]));
    assert_eq!(replica.mesh_epoch(owner), replica_mesh);
    assert_eq!(replica.water_mesh_epoch(owner), replica_water);
    assert_eq!(replica.section_delta(owner), host.section_delta(owner));
    host.record(position, 0).unwrap();
    assert!(host.tint_at(position).is_none());
    host.record(position, 103).unwrap();
    assert!(host.tint_at(position).is_none());
}

#[test]
fn malformed_or_exhausted_tint_updates_do_not_mutate_state() {
    let mut journal = EditJournal::default();
    let position = [15, 16, -1];
    let owner = global_to_section(position).0;
    journal.record(position, 103).unwrap();
    journal.set_tint(position, Some([1, 2, 3])).unwrap();
    let before = journal.section_delta(owner);
    let mut duplicate = before.clone();
    duplicate.tints.push(duplicate.tints[0]);
    assert!(journal.import_delta(duplicate).is_err());
    assert_eq!(journal.section_delta(owner), before);
    let mut orphan = before.clone();
    orphan.blocks.clear();
    assert!(journal.import_delta(orphan).is_err());
    assert_eq!(journal.section_delta(owner), before);
    let mut exhausted = before.clone();
    exhausted.revision = u64::MAX;
    journal.import_delta(exhausted).unwrap();
    let before = journal.section_delta(owner);
    assert!(journal.set_tint(position, Some([4, 5, 6])).is_err());
    assert_eq!(journal.section_delta(owner), before);
    assert!(journal.set_tint([1, 2, 3], Some([1, 2, 3])).is_err());
}

#[test]
fn mixed_slab_rgb_metadata_is_valid_for_either_material_order_and_survives_replication() {
    for (primary, secondary) in [(85, 105), (105, 85)] {
        for facing in [0, 4, 5, 6, 7, 8] {
            let position = [-17, 15, 16];
            let owner = global_to_section(position).0;
            let mut journal = EditJournal::default();
            journal.record(position, primary).unwrap();
            let merged = terra_core::construction::merged_slab(primary, facing, secondary).unwrap();
            journal
                .set_object_state(
                    position,
                    Some(super::BlockObjectState {
                        facing: merged,
                        ..Default::default()
                    }),
                )
                .unwrap();
            journal.set_tint(position, Some([31, 92, 211])).unwrap();
            let delta = journal.section_delta(owner);
            let mut replica = EditJournal::default();
            replica.import_delta(delta.clone()).unwrap();
            assert_eq!(replica.section_delta(owner), delta);
            journal.record(position, primary).unwrap();
            assert_eq!(journal.tint_at(position), Some([31, 92, 211]));
        }
    }
}

#[test]
fn object_pose_changes_preserve_dyeable_rgb_but_clear_final_nonfabric_and_replicate() {
    for primary in [85, 105] {
        let position = [-17, 15, 16];
        let owner = global_to_section(position).0;
        let rgb = [31, 92, 211];
        let mut journal = EditJournal::default();
        journal.record(position, primary).unwrap();
        for facing in [0, 4, 5, 6, 7, 8] {
            let merged = terra_core::construction::merged_slab(primary, facing, 105).unwrap();
            journal
                .set_object_state(
                    position,
                    Some(super::BlockObjectState {
                        facing: merged,
                        ..Default::default()
                    }),
                )
                .unwrap();
            if journal.tint_at(position).is_none() {
                journal.set_tint(position, Some(rgb)).unwrap();
            }
            assert_eq!(journal.tint_at(position), Some(rgb));
        }
        let appearance = journal.appearance_epoch(owner);
        journal
            .set_object_state(position, Some(Default::default()))
            .unwrap();
        assert_eq!(
            journal.tint_at(position),
            if primary == 105 { Some(rgb) } else { None }
        );
        assert_eq!(
            journal.appearance_epoch(owner),
            appearance + u64::from(primary == 85)
        );
        let delta = journal.section_delta(owner);
        let mut replica = EditJournal::default();
        replica.import_delta(delta.clone()).unwrap();
        assert_eq!(replica.section_delta(owner), delta);
    }
}

#[test]
fn object_pose_tint_cleanup_preflights_appearance_overflow_without_mutation() {
    let position = [-17, 15, 16];
    let owner = global_to_section(position).0;
    let mut journal = EditJournal::default();
    journal.record(position, 85).unwrap();
    journal
        .set_object_state(
            position,
            Some(super::BlockObjectState {
                facing: terra_core::construction::merged_slab(85, 0, 105).unwrap(),
                ..Default::default()
            }),
        )
        .unwrap();
    journal.set_tint(position, Some([31, 92, 211])).unwrap();
    let mut delta = journal.section_delta(owner);
    delta.appearance_epoch = u64::MAX;
    journal.import_delta(delta).unwrap();
    let before = journal.section_delta(owner);
    assert!(
        journal
            .set_object_state(position, Some(Default::default()))
            .is_err()
    );
    assert_eq!(journal.section_delta(owner), before);
}
