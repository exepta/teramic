use super::*;
use crate::edits::EditJournal;
use terra_voxel::{SectionPos, global_to_section};

#[test]
fn negative_surface_state_has_local_snapshot_and_only_owner_appearance_epoch() {
    let mut journal = EditJournal::default();
    let position = [-1, -17, -16];
    let (owner, local) = global_to_section(position);
    assert_eq!(owner, SectionPos::new(-1, -2, -1));
    assert_eq!(local, [15, 15, 0]);
    let condition = SurfaceCondition::new(0.7, 0.25).unwrap();
    assert!(journal.set_surface_condition(position, condition).unwrap());
    assert_eq!(journal.surface_condition(position), condition);
    assert_eq!(
        journal.surface_snapshot(owner).get(&local),
        Some(&condition)
    );
    assert_eq!(journal.appearance_epoch(owner), 1);
    assert_eq!(journal.revision(owner), 0);
    assert_eq!(journal.mesh_epoch(owner), 0);
    assert_eq!(journal.prop_epoch(owner), 0);
    assert_eq!(journal.edited_blocks(), 0);
    assert!(journal.snapshot(owner).unwrap().is_empty());
    for offset in [[1, 0, 0], [0, 1, 0], [0, 0, -1]] {
        let neighbor = owner.offset(offset).unwrap();
        assert_eq!(journal.appearance_epoch(neighbor), 0);
        assert_eq!(journal.mesh_epoch(neighbor), 0);
        assert!(journal.surface_snapshot(neighbor).is_empty());
    }
}

#[test]
fn surface_updates_are_idempotent_and_reset_removes_sparse_entry() {
    let mut journal = EditJournal::default();
    let position = [5, 7, 9];
    let owner = SectionPos::default();
    let wet = SurfaceCondition::new(1.0, 0.0).unwrap();
    assert!(
        !journal
            .set_surface_condition(position, SurfaceCondition::default())
            .unwrap()
    );
    assert_eq!(journal.appearance_epoch(owner), 0);
    assert!(journal.set_surface_condition(position, wet).unwrap());
    let snapshot = journal.surface_snapshot(owner);
    assert!(!journal.set_surface_condition(position, wet).unwrap());
    assert_eq!(journal.appearance_epoch(owner), 1);
    assert!(
        journal
            .set_surface_condition(position, SurfaceCondition::default())
            .unwrap()
    );
    assert_eq!(journal.appearance_epoch(owner), 2);
    assert!(journal.surface_snapshot(owner).is_empty());
    assert_eq!(
        journal.surface_condition(position),
        SurfaceCondition::default()
    );
    assert_eq!(snapshot.get(&[5, 7, 9]), Some(&wet));
    assert!(
        !journal
            .set_surface_condition(position, SurfaceCondition::default())
            .unwrap()
    );
    assert_eq!(journal.appearance_epoch(owner), 2);
}

#[test]
fn invalid_surface_values_are_rejected_without_mutating_existing_state() {
    let position = [2, 3, 4];
    let initial = SurfaceCondition::new(0.4, 0.6).unwrap();
    let mut journal = EditJournal::default();
    journal.set_surface_condition(position, initial).unwrap();
    for invalid in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY, -0.01, 1.01] {
        let wet = SurfaceCondition {
            wetness: invalid,
            charred: 0.0,
            ..Default::default()
        };
        let charred = SurfaceCondition {
            wetness: 0.0,
            charred: invalid,
            ..Default::default()
        };
        assert_eq!(
            SurfaceCondition::new(invalid, 0.0),
            Err(SurfaceConditionError::InvalidWetness)
        );
        assert_eq!(
            SurfaceCondition::new(0.0, invalid),
            Err(SurfaceConditionError::InvalidCharred)
        );
        assert_eq!(
            journal.set_surface_condition(position, wet),
            Err(SurfaceConditionError::InvalidWetness)
        );
        assert_eq!(
            journal.set_surface_condition(position, charred),
            Err(SurfaceConditionError::InvalidCharred)
        );
        assert_eq!(journal.surface_condition(position), initial);
        assert_eq!(journal.appearance_epoch(SectionPos::default()), 1);
    }
    assert!(SurfaceCondition::new(0.0, 1.0).is_ok());
    assert!(SurfaceCondition::new(1.0, 0.0).is_ok());
}

#[test]
fn replacing_block_clears_only_its_condition_and_advances_appearance_when_needed() {
    let mut journal = EditJournal::default();
    let owner = SectionPos::new(-1, 0, 0);
    let position = [-1, 7, 8];
    let nearby = [-2, 7, 8];
    let condition = SurfaceCondition::new(0.3, 0.8).unwrap();
    journal.set_surface_condition(position, condition).unwrap();
    journal.set_surface_condition(nearby, condition).unwrap();
    assert_eq!(journal.appearance_epoch(owner), 2);
    journal.record(position, 0).unwrap();
    assert_eq!(
        journal.surface_condition(position),
        SurfaceCondition::default()
    );
    assert_eq!(journal.surface_condition(nearby), condition);
    assert_eq!(journal.appearance_epoch(owner), 3);
    assert_eq!(journal.appearance_epoch(SectionPos::default()), 0);
    assert_eq!(journal.revision(owner), 1);
    assert_eq!(journal.mesh_epoch(owner), 1);
    assert_eq!(journal.mesh_epoch(SectionPos::default()), 1);
    journal.record(position, 3).unwrap();
    assert_eq!(journal.appearance_epoch(owner), 3);
    assert_eq!(journal.revision(owner), 2);
}

#[cfg(feature = "bevy")]
mod requests {
    use super::*;
    use crate::streaming::{ChunkStore, LoadedSection};
    use bevy::prelude::*;
    use terra_voxel::BlockSection;

    fn app() -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .init_resource::<ChunkStore>()
            .init_resource::<EditJournal>()
            .add_message::<SurfaceConditionRequest>()
            .add_systems(Update, apply_surface_conditions);
        app
    }

    fn loaded_section(owner: SectionPos) -> LoadedSection {
        let mut data = BlockSection::empty(owner).unwrap();
        for material in 1..=terra_voxel::MaterialId::LAST {
            data.set_block(
                [
                    usize::from(material) % 16,
                    8,
                    9 + usize::from(material) / 16,
                ],
                material,
            )
            .unwrap();
        }
        LoadedSection {
            data,
            mesh: None,
            mesh_epoch: 17,
            water_epoch: 0,
            water: None,
            props: Vec::new(),
        }
    }

    #[test]
    fn requests_accept_only_loaded_solids_without_changing_voxel_data_or_epochs() {
        let mut app = app();
        let owner = SectionPos::new(-2, -1, 0);
        let section = loaded_section(owner);
        let original_data = section.data.clone();
        app.world_mut()
            .resource_mut::<ChunkStore>()
            .loaded
            .insert(owner, section);
        let condition = SurfaceCondition::new(0.5, 0.75).unwrap();
        for material in 0..=usize::from(terra_voxel::MaterialId::LAST) {
            app.world_mut().write_message(SurfaceConditionRequest {
                position: owner
                    .global_block([material % 16, 8, 9 + material / 16])
                    .unwrap(),
                condition,
            });
        }
        let unloaded = [-33, -8, 9];
        app.world_mut().write_message(SurfaceConditionRequest {
            position: unloaded,
            condition,
        });
        app.update();
        let journal = app.world().resource::<EditJournal>();
        for material in 0..=usize::from(terra_voxel::MaterialId::LAST) {
            let position = owner
                .global_block([material % 16, 8, 9 + material / 16])
                .unwrap();
            assert_eq!(
                journal.surface_condition(position),
                if terra_voxel::MaterialId::from_raw(material as u16)
                    .is_some_and(terra_voxel::MaterialId::is_solid)
                {
                    condition
                } else {
                    SurfaceCondition::default()
                }
            );
        }
        assert_eq!(
            journal.surface_condition(unloaded),
            SurfaceCondition::default()
        );
        assert_eq!(
            journal.appearance_epoch(owner),
            (0..=terra_voxel::MaterialId::LAST)
                .filter(|&id| terra_voxel::MaterialId::from_raw(id)
                    .is_some_and(terra_voxel::MaterialId::is_solid))
                .count() as u64
        );
        assert_eq!(journal.revision(owner), 0);
        assert_eq!(journal.mesh_epoch(owner), 0);
        assert_eq!(journal.prop_epoch(owner), 0);
        let loaded = &app.world().resource::<ChunkStore>().loaded[&owner];
        assert_eq!(loaded.mesh_epoch, 17);
        assert_eq!(loaded.data, original_data);
        assert!(loaded.mesh.is_none());
    }

    #[test]
    fn surface_state_survives_negative_section_eviction_and_reload() {
        let mut app = app();
        let owner = SectionPos::new(-2, -1, -3);
        let position = owner.global_block([1, 8, 9]).unwrap();
        app.world_mut()
            .resource_mut::<ChunkStore>()
            .loaded
            .insert(owner, loaded_section(owner));
        let condition = SurfaceCondition::new(0.5, 1.0).unwrap();
        app.world_mut().write_message(SurfaceConditionRequest {
            position,
            condition,
        });
        app.update();
        app.world_mut()
            .resource_mut::<ChunkStore>()
            .loaded
            .remove(&owner);
        app.world_mut().write_message(SurfaceConditionRequest {
            position,
            condition: SurfaceCondition::default(),
        });
        app.update();
        assert_eq!(
            app.world()
                .resource::<EditJournal>()
                .surface_condition(position),
            condition
        );
        app.world_mut()
            .resource_mut::<ChunkStore>()
            .loaded
            .insert(owner, loaded_section(owner));
        app.world_mut().write_message(SurfaceConditionRequest {
            position,
            condition: SurfaceCondition {
                wetness: f32::NAN,
                charred: 0.0,
                ..Default::default()
            },
        });
        app.update();
        let journal = app.world().resource::<EditJournal>();
        assert_eq!(journal.surface_condition(position), condition);
        assert_eq!(
            journal.surface_snapshot(owner).get(&[1, 8, 9]),
            Some(&condition)
        );
        assert_eq!(journal.appearance_epoch(owner), 1);
        assert_eq!(journal.revision(owner), 0);
    }
}
