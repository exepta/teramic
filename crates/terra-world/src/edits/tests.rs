use super::*;
use terra_voxel::SectionPos;

#[test]
fn all_opening_species_keep_their_pose_and_pair_when_the_same_block_is_recorded_again() {
    let primary = [-16, 15, -16];
    let upper = [-16, 16, -16];
    for material in (65..=70).chain([139, 140]) {
        for facing in 0..8 {
            let mut journal = EditJournal::default();
            let mut cells = vec![(
                primary,
                BlockObjectState {
                    facing,
                    peer: crate::objects::is_door(material).then_some(upper),
                    ..BlockObjectState::default()
                },
            )];
            if crate::objects::is_door(material) {
                cells.push((
                    upper,
                    BlockObjectState {
                        facing,
                        peer: Some(primary),
                        secondary: true,
                        ..BlockObjectState::default()
                    },
                ));
            }
            for &(position, state) in &cells {
                journal.record(position, material).unwrap();
                journal.set_object_state(position, Some(state)).unwrap();
            }
            let mut replica = EditJournal::default();
            for &(position, state) in &cells {
                journal.record(position, material).unwrap();
                assert_eq!(journal.object_state(position), Some(state));
                replica
                    .import_delta(journal.section_delta(terra_voxel::global_to_section(position).0))
                    .unwrap();
                assert_eq!(replica.object_state(position), Some(state));
            }
            for &(position, _) in &cells {
                journal.record(position, 3).unwrap();
                assert_eq!(journal.object_state(position), None);
            }
        }
    }
}

#[test]
fn object_rotation_and_pair_state_replicate_and_invalidate_same_material_meshes() {
    let p = [-1, 16, 0];
    let peer = [0, 16, 0];
    let mut host = EditJournal::default();
    host.record(p, 28).unwrap();
    host.record(peer, 28).unwrap();
    let mut client = EditJournal::default();
    let owner = terra_voxel::global_to_section(p).0;
    client.import_delta(host.section_delta(owner)).unwrap();
    let previous = client.mesh_epoch(owner);
    let state = BlockObjectState {
        rail_curve: None,
        wood: terra_core::WoodSpecies::Birch,
        facing: 0,
        peer: Some(peer),
        secondary: false,
        liquid: None,
    };
    host.set_object_state(p, Some(state)).unwrap();
    client.import_delta(host.section_delta(owner)).unwrap();
    assert_eq!(client.object_state(p), Some(state));
    assert!(client.mesh_epoch(owner) > previous);
    assert!(
        host.set_object_state(
            p,
            Some(BlockObjectState {
                peer: Some([-1, 17, 0]),
                ..state
            })
        )
        .is_err()
    );
    host.record(p, 0).unwrap();
    assert_eq!(host.object_state(p), None);
    for (yaw, right) in [
        (0.0, [1, 0, 0]),
        (std::f32::consts::FRAC_PI_2, [0, 0, -1]),
        (std::f32::consts::PI, [-1, 0, 0]),
        (-std::f32::consts::FRAC_PI_2, [0, 0, 1]),
    ] {
        assert_eq!(BlockObjectState::from_yaw(yaw).right(), right);
    }
}
#[test]
fn boundary_edits_invalidate_face_neighbors_and_shared_water_corners() {
    let mut journal = EditJournal::default();
    let affected = journal.record([-1, 0, 15], 0).unwrap();
    assert!(affected.contains(&SectionPos::new(-1, 0, 0)));
    assert!(affected.contains(&SectionPos::new(0, 0, 0)));
    assert!(affected.contains(&SectionPos::new(-1, -1, 0)));
    assert!(affected.contains(&SectionPos::new(-1, 0, 1)));
    assert!(affected.contains(&SectionPos::new(0, 0, 1)));
    assert_eq!(affected.len(), 8);
    assert_eq!(journal.revision(SectionPos::new(-1, 0, 0)), 1);
    assert_eq!(journal.revision(SectionPos::new(0, 0, 0)), 0);
    assert_eq!(journal.mesh_epoch(SectionPos::new(0, 0, 0)), 1);
    assert_eq!(
        journal
            .snapshot(SectionPos::new(0, 0, 0))
            .unwrap()
            .get(&[-1, 0, 15]),
        Some(&0)
    );
    assert_eq!(journal.edited_blocks(), 1);
}
#[test]
fn interior_edits_do_not_invalidate_unrelated_sections() {
    let mut journal = EditJournal::default();
    assert_eq!(
        journal.record([5, 5, 5], 3).unwrap(),
        vec![SectionPos::default()]
    );
    assert!(
        journal
            .snapshot(SectionPos::new(3, 0, 0))
            .unwrap()
            .is_empty()
    );
    assert_eq!(journal.mesh_epoch(SectionPos::new(1, 0, 0)), 0);
    assert_eq!(
        journal
            .snapshot(SectionPos::new(0, 1, 0))
            .unwrap()
            .get(&[5, 5, 5]),
        Some(&3)
    );
    assert_eq!(
        journal
            .snapshot(SectionPos::new(0, -1, 0))
            .unwrap()
            .get(&[5, 5, 5]),
        Some(&3)
    );
}

#[test]
fn roof_corner_edits_invalidate_a_recorded_chimney_two_cells_across_the_boundary() {
    for sign in [-1_i64, 1] {
        let mut journal = EditJournal::default();
        let chimney = if sign < 0 {
            [-16, -1, -16]
        } else {
            [-1, -1, -1]
        };
        let roof_corner = [chimney[0] + sign * 2, chimney[1], chimney[2]];
        journal.record(chimney, 60).unwrap();
        let owner = terra_voxel::global_to_section(chimney).0;
        let before = journal.mesh_epoch(owner);
        let affected = journal.record(roof_corner, 77).unwrap();
        assert!(affected.contains(&owner));
        assert!(journal.mesh_epoch(owner) > before);
        assert!(affected.len() <= 8);
        let unrelated = owner.offset([0, 1, 0]).unwrap();
        assert!(!affected.contains(&unrelated));
    }
}

#[test]
fn registered_materials_and_custom_ids_survive_generic_section_delta_reload() {
    let mut host = EditJournal::default();
    for id in 1..=terra_voxel::MaterialId::LAST {
        host.record([-20 + i64::from(id), -1, -1], id).unwrap();
    }
    let mut replica = EditJournal::default();
    for section in host.changed_sections() {
        replica.import_delta(host.section_delta(section)).unwrap();
    }
    for id in 1..=terra_voxel::MaterialId::LAST {
        assert_eq!(replica.block([-20 + i64::from(id), -1, -1]), Some(id));
    }
    // Generic journals must preserve opt-in custom models. Client edit commands
    // have their separate authoritative palette validation before committing.
    let owner = SectionPos::default();
    host.record([1; 3], 65000).unwrap();
    replica.import_delta(host.section_delta(owner)).unwrap();
    assert_eq!(replica.block([1; 3]), Some(65000));
}

#[test]
fn solid_water_is_a_barrier_not_a_fluid_source_even_after_removal() {
    use crate::{
        TerraGenerator,
        water::{FluidCell, fluid_cell_at},
    };
    let terrain = TerraGenerator::new(terra_core::WorldConfig {
        landscape_revision: 3,
        sea_level: 8.0,
        base_height: 16.0,
        hill_amplitude: 0.0,
        mountain_amplitude: 0.0,
        trees: false,
        ..Default::default()
    })
    .unwrap();
    let mut journal = EditJournal::default();
    let p = [-1, 20, -1];
    journal
        .set_fluid(
            p,
            FluidCell {
                amount: 4,
                source: false,
                flow: [1, 0],
            },
        )
        .unwrap();
    journal
        .record(p, terra_voxel::MaterialId::SolidWater as u16)
        .unwrap();
    assert_eq!(
        fluid_cell_at(&terrain, &journal, p, 14),
        FluidCell::default()
    );
    assert_eq!(journal.fluid(p).unwrap_or_default(), FluidCell::default());
    journal.record(p, 0).unwrap();
    assert_eq!(
        fluid_cell_at(&terrain, &journal, p, 0),
        FluidCell::default()
    );
}

#[test]
fn prop_removal_has_its_own_revision_and_is_idempotent() {
    use crate::props::PropId;
    let mut journal = EditJournal::default();
    let owner = SectionPos::new(-1, 0, -1);
    assert!(journal.remove_prop(owner, PropId(42)).unwrap());
    assert!(!journal.remove_prop(owner, PropId(42)).unwrap());
    assert_eq!(journal.prop_epoch(owner), 1);
    assert_eq!(journal.mesh_epoch(owner), 0);
    assert_eq!(journal.revision(owner), 0);
    assert_eq!(journal.removed_props(), 1);
    assert_eq!(journal.edited_blocks(), 0);
    assert!(!journal.prop_removed(owner, PropId(43)));
    assert!(!journal.prop_removed(SectionPos::default(), PropId(42)));
}

#[test]
fn confirmed_deltas_preserve_authority_counters_and_negative_boundary_invalidation() {
    use crate::{environment::SurfaceCondition, props::PropId};
    let mut host = EditJournal::default();
    let position = [-1, 0, 15];
    let owner = SectionPos::new(-1, 0, 0);
    host.record(position, 0).unwrap();
    host.remove_prop(owner, PropId(17)).unwrap();
    host.set_surface_condition(position, SurfaceCondition::new(0.8, 0.2).unwrap())
        .unwrap();
    let delta = host.section_delta(owner);
    let mut replica = EditJournal::default();
    let affected = replica.import_delta(delta.clone()).unwrap();
    assert_eq!(affected.len(), 8);
    assert_eq!(replica.section_delta(owner), delta);
    assert!(replica.prop_removed(owner, PropId(17)));
    let epoch = replica.mesh_epoch(owner);
    assert!(replica.import_delta(delta.clone()).unwrap().is_empty());
    assert_eq!(replica.mesh_epoch(owner), epoch);
    host.record([-2, 0, 15], 3).unwrap();
    let latest = host.section_delta(owner);
    replica.import_delta(latest.clone()).unwrap();
    assert!(replica.import_delta(delta).unwrap().is_empty());
    assert_eq!(replica.section_delta(owner), latest);
}

#[cfg(feature = "bevy")]
#[test]
fn replica_imports_preserve_deltas_and_epochs_without_accumulating_simulation_work() {
    use crate::{environment::SurfaceCondition, props::PropId, water::FluidCell};
    use terra_voxel::{MaterialId, global_to_section};

    let mut host = EditJournal::default();
    let mut replica = EditJournal::default();
    let mut restored = EditJournal::default();
    let mut chunks = crate::streaming::ChunkStore::default();
    for cycle in 0..100 {
        let position = [-1 - cycle * 16, -1, -1 - cycle * 16];
        let owner = global_to_section(position).0;
        for material in [MaterialId::FluidWater as u16, MaterialId::Stone as u16, 0] {
            host.record(position, material).unwrap();
            if material == MaterialId::FluidWater as u16 {
                host.set_fluid(
                    position,
                    FluidCell {
                        amount: 5,
                        source: true,
                        flow: [1, 0],
                    },
                )
                .unwrap();
            }
            host.set_surface_condition(position, SurfaceCondition::new(0.7, 0.0).unwrap())
                .unwrap();
            host.remove_prop(owner, PropId(cycle as u64 + 1)).unwrap();
            let delta = host.section_delta(owner);
            chunks
                .apply_confirmed_delta(&mut replica, delta.clone())
                .unwrap();
            restored.import_delta(delta.clone()).unwrap();
            assert_eq!(replica.section_delta(owner), delta);
            assert_eq!(replica.section_delta(owner), restored.section_delta(owner));
            assert_eq!(replica.fluid(position), host.fluid(position));
            for dz in 0..=1 {
                for dy in 0..=1 {
                    for dx in 0..=1 {
                        let neighbor = owner.offset([dx, dy, dz]).unwrap();
                        assert_eq!(replica.mesh_epoch(neighbor), restored.mesh_epoch(neighbor));
                        assert_eq!(
                            replica.water_mesh_epoch(neighbor),
                            restored.water_mesh_epoch(neighbor)
                        );
                    }
                }
            }
            let epoch = replica.mesh_epoch(owner);
            let water_epoch = replica.water_mesh_epoch(owner);
            chunks.apply_confirmed_delta(&mut replica, delta).unwrap();
            assert_eq!(replica.mesh_epoch(owner), epoch);
            assert_eq!(replica.water_mesh_epoch(owner), water_epoch);
            assert!(replica.fluid_active.is_empty(), "cycle {cycle}");
            assert!(replica.fluid_queued.is_empty());
            assert!(replica.grass_candidates.is_empty());
            assert!(replica.grass_queued.is_empty());
            assert!(
                !restored.fluid_active.is_empty(),
                "authoritative reload must still wake fluids"
            );
            assert!(!restored.grass_candidates.is_empty());
            restored.take_fluid_work(usize::MAX);
            restored.grass_candidates.clear();
            restored.grass_queued.clear();
        }
    }
}

#[test]
fn prop_and_appearance_only_deltas_do_not_invalidate_terrain_meshes() {
    use crate::{environment::SurfaceCondition, props::PropId};
    let owner = SectionPos::new(-1, -1, -1);
    let mut host = EditJournal::default();
    host.remove_prop(owner, PropId(99)).unwrap();
    host.set_surface_condition([-1, -1, -1], SurfaceCondition::new(1.0, 0.0).unwrap())
        .unwrap();
    let mut replica = EditJournal::default();
    assert!(
        replica
            .import_delta(host.section_delta(owner))
            .unwrap()
            .is_empty()
    );
    assert_eq!(replica.mesh_epoch(owner), 0);
    assert_eq!(replica.prop_epoch(owner), 1);
    assert_eq!(replica.appearance_epoch(owner), 1);
}

#[test]
fn malformed_confirmed_deltas_are_rejected_without_mutation() {
    let owner = SectionPos::new(-1, 0, 0);
    let mut replica = EditJournal::default();
    let invalid = SectionDelta {
        position: owner,
        revision: 1,
        blocks: vec![([16, 0, 0], 3)],
        ..Default::default()
    };
    assert!(replica.import_delta(invalid).is_err());
    assert_eq!(replica.edited_blocks(), 0);
    assert_eq!(replica.revision(owner), 0);
    assert_eq!(replica.mesh_epoch(owner), 0);
}

#[test]
fn fluid_delta_replays_removals_without_touching_terrain_or_props() {
    use crate::water::FluidCell;
    let mut host = EditJournal::default();
    let cell = FluidCell {
        amount: 6,
        flow: [1, 0],
        source: false,
    };
    let position = [-1, 0, -1];
    let owner = SectionPos::new(-1, 0, -1);
    host.set_fluid(position, cell).unwrap();
    let first = host.section_delta(owner);
    let mut replica = EditJournal::default();
    assert!(replica.import_delta(first.clone()).unwrap().is_empty());
    assert_eq!(replica.fluid(position), Some(cell));
    assert_eq!(replica.mesh_epoch(owner), 0);
    assert_eq!(replica.prop_epoch(owner), 0);
    assert_eq!(replica.water_mesh_epoch(SectionPos::default()), 1);
    let water_epoch = replica.water_mesh_epoch(owner);
    replica.import_delta(first.clone()).unwrap();
    assert_eq!(replica.water_mesh_epoch(owner), water_epoch);
    host.set_fluid(position, FluidCell::default()).unwrap();
    let latest = host.section_delta(owner);
    assert!(latest.fluids.is_empty());
    replica.import_delta(latest.clone()).unwrap();
    assert_eq!(replica.fluid(position), None);
    assert_eq!(replica.fluid_epoch(owner), 2);
    replica.import_delta(first).unwrap();
    assert_eq!(replica.section_delta(owner), latest);
}

#[test]
fn invalid_fluid_cells_and_duplicate_delta_cells_are_rejected_before_mutation() {
    use crate::water::FluidCell;
    let mut journal = EditJournal::default();
    let bad = FluidCell {
        amount: 9,
        ..Default::default()
    };
    assert!(journal.set_fluid([0; 3], bad).is_err());
    assert_eq!(journal.fluid_cells().count(), 0);
    assert_eq!(journal.water_mesh_epoch(SectionPos::default()), 0);
    for fluids in [
        vec![([0; 3], bad)],
        vec![([0; 3], FluidCell::default())],
        vec![
            (
                [0; 3],
                FluidCell {
                    amount: 8,
                    ..Default::default()
                }
            );
            2
        ],
    ] {
        let delta = SectionDelta {
            fluid_epoch: 1,
            fluids,
            ..Default::default()
        };
        assert!(journal.import_delta(delta).is_err());
        assert_eq!(journal.changed_sections().count(), 0);
    }
}
