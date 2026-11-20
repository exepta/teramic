use super::*;
use crate::edits::BlockObjectState;

fn stair(position: [i64; 3]) -> StructureObject {
    StructureObject {
        position,
        material: 74,
        state: Some(BlockObjectState {
            facing: 1,
            ..Default::default()
        }),
        loot: None,
        crop: None,
    }
}

fn source(position: [i64; 3]) -> StructureObject {
    StructureObject {
        position,
        material: 27,
        state: None,
        loot: None,
        crop: None,
    }
}

#[cfg(feature = "bevy")]
#[test]
fn real_seeded_village_farm_sources_settle_and_reload_without_water_or_terrain_epoch_churn() {
    use crate::{
        SectionGenerator, TerraGenerator,
        generation::structures::{BuildingRole, StructureKind},
        streaming::{ChunkStore, LoadedSection},
    };
    let terrain = TerraGenerator::new(terra_core::WorldConfig {
        seed: terra_core::WorldSeed(42042),
        structures_revision: 2,
        landscape_revision: 0,
        geology_revision: 0,
        ocean_revision: 0,
        cave_structure_revision: 0,
        base_height: 16.0,
        sea_level: 0.0,
        hill_amplitude: 0.0,
        mountain_amplitude: 0.0,
        terrace_strength: 0.0,
        caves: false,
        trees: false,
        water: true,
        rivers: false,
        biomes: false,
        ..Default::default()
    })
    .unwrap();
    let farm = (-3..=2)
        .flat_map(|z| (-3..=2).map(move |x| [x, z]))
        .find_map(|region| {
            let site = terrain.structure_site(region)?;
            (site.kind == StructureKind::Village)
                .then(|| {
                    site.buildings
                        .into_iter()
                        .find(|b| b.role == BuildingRole::Farm)
                })
                .flatten()
        })
        .expect("bounded seeded village farm fixture");
    let low = global_to_section(farm.bounds[0].map(|v| v - 2)).0;
    let high = global_to_section(farm.bounds[1].map(|v| v + 2)).0;
    let generator = SectionGenerator::new(terrain.clone());
    let mut chunks = ChunkStore::default();
    let mut objects = Vec::new();
    for z in low.z..=high.z {
        for x in low.x..=high.x {
            let column = generator.prepare_column([x, z], false).unwrap();
            for y in low.y..=high.y {
                let position = SectionPos::new(x, y, z);
                objects.extend(column.structure_objects(position).into_iter().filter(|o| {
                    (o.crop.is_some() || o.material == 27)
                        && (0..3).all(|i| {
                            (farm.bounds[0][i]..=farm.bounds[1][i]).contains(&o.position[i])
                        })
                }));
                let generated = generator
                    .build_prepared(position, 0, &Default::default(), &column)
                    .unwrap();
                chunks.loaded.insert(
                    position,
                    LoadedSection {
                        data: generated.data,
                        mesh: None,
                        mesh_epoch: 0,
                        water_epoch: 0,
                        water: generated.water,
                        props: generated.props,
                    },
                );
            }
        }
    }
    let sources: Vec<_> = objects
        .iter()
        .filter(|o| o.material == 27)
        .map(|o| o.position)
        .collect();
    assert_eq!(sources.len(), 7);
    assert_eq!(objects.iter().filter(|o| o.crop.is_some()).count(), 70);
    let mut journal = EditJournal::default();
    journal.initialize_structures(&objects).unwrap();
    for (&position, section) in &mut chunks.loaded {
        journal.apply_prop_removals(position, &mut section.props);
    }
    assert_eq!(journal.fluid_work_counts().sources, 7);
    for position in &sources {
        assert!(crate::water::fluid_cell_at(&terrain, &journal, *position, 27).source);
    }
    let settled = crate::water::simulate_fluid_step(&terrain, &chunks, &mut journal, 4096).unwrap();
    assert!(settled.visited > 0 && settled.visited <= 49);
    assert!(
        !settled.changed,
        "an enclosed irrigated field starts in a stable fluid state"
    );
    assert_eq!(journal.fluid_work_counts().active, 0);
    assert_eq!(journal.fluid_work_counts().sleeping, 0);
    let saved: Vec<_> = journal
        .changed_sections()
        .map(|p| journal.section_delta(p))
        .collect();
    let epochs: Vec<_> = chunks
        .loaded
        .keys()
        .map(|&p| (p, journal.mesh_epoch(p), journal.water_mesh_epoch(p)))
        .collect();
    for _ in 0..20 {
        let idle = crate::water::simulate_fluid_step(&terrain, &chunks, &mut journal, 256).unwrap();
        assert_eq!(idle.visited, 0);
        assert!(!idle.changed);
    }
    for (p, terrain, water) in epochs {
        assert_eq!(
            (journal.mesh_epoch(p), journal.water_mesh_epoch(p)),
            (terrain, water)
        );
    }
    let mut loaded = EditJournal::default();
    for delta in saved {
        loaded.import_delta(delta).unwrap();
    }
    assert!(loaded.initialize_structures(&objects).unwrap().is_empty());
    for position in &sources {
        assert!(loaded.fluid(*position).unwrap().source);
    }
    for _ in 0..4 {
        crate::water::simulate_fluid_step(&terrain, &chunks, &mut loaded, 4096).unwrap();
    }
    assert_eq!(loaded.fluid_work_counts().active, 0);
    assert_eq!(loaded.fluid_work_counts().sources, 7);
    assert_eq!(loaded.fluid_work_counts().recorded_cells, 7);
}

#[test]
fn generated_channel_sources_are_real_revisioned_once_and_only_their_bounded_neighborhood_wakes() {
    let objects: Vec<_> = (-4..=2).map(|z| source([-1, 15, z])).collect();
    let mut journal = EditJournal::default();
    journal.initialize_structures(&objects).unwrap();
    let expected = crate::water::FluidCell {
        amount: 8,
        source: true,
        flow: [0; 2],
    };
    for object in &objects {
        assert_eq!(journal.fluid(object.position), Some(expected));
    }
    assert_eq!(journal.fluid_cells().count(), 7);
    assert!(journal.fluid_active.len() <= 49);
    assert!(!journal.fluid_active.is_empty());
    let owners: BTreeSet<_> = objects
        .iter()
        .map(|o| global_to_section(o.position).0)
        .collect();
    let saved: Vec<_> = owners
        .iter()
        .map(|&owner| journal.section_delta(owner))
        .collect();
    for &owner in &owners {
        assert_eq!(journal.mesh_epoch(owner), 0);
        assert_eq!(journal.water_mesh_epoch(owner), 1);
    }
    let active = journal.fluid_active.len();
    assert!(journal.initialize_structures(&objects).unwrap().is_empty());
    assert_eq!(journal.fluid_active.len(), active);
    assert_eq!(
        owners
            .iter()
            .map(|&owner| journal.section_delta(owner))
            .collect::<Vec<_>>(),
        saved
    );
    #[cfg(feature = "bevy")]
    assert!(journal.grass_candidates.is_empty());
}

#[test]
fn missing_old_generated_source_is_repaired_without_terrain_edits_but_player_tombstones_and_explicit_fluid_are_kept()
 {
    let positions = [[-1, 15, -1], [-2, 15, -1], [-3, 15, -1], [-4, 15, -1]];
    let owner = global_to_section(positions[0]).0;
    let mut journal = EditJournal::default();
    for position in positions {
        journal
            .register_generated_block(position, 27, None)
            .unwrap();
    }
    journal.record(positions[1], 0).unwrap();
    let flowing = crate::water::FluidCell {
        amount: 3,
        flow: [1, 0],
        source: false,
    };
    journal.set_fluid(positions[2], flowing).unwrap();
    journal
        .sections
        .get_mut(&owner)
        .unwrap()
        .fluids
        .insert(global_to_section(positions[3]).1, Default::default());
    let epoch = journal.mesh_epoch(owner);
    let revision = journal.revision(owner);
    let fluid_epoch = journal.fluid_epoch(owner);
    let objects: Vec<_> = positions.into_iter().map(source).collect();
    assert!(
        journal.initialize_structures(&objects).unwrap().is_empty(),
        "source-only repair never requests terrain remeshing"
    );
    assert!(journal.fluid(positions[0]).unwrap().source);
    assert_eq!(journal.block(positions[1]), Some(0));
    assert_eq!(journal.fluid(positions[1]), None);
    assert_eq!(journal.fluid(positions[2]), Some(flowing));
    assert_eq!(journal.fluid(positions[3]), Some(Default::default()));
    assert_eq!(journal.mesh_epoch(owner), epoch);
    assert_eq!(journal.revision(owner), revision);
    assert_eq!(journal.fluid_epoch(owner), fluid_epoch + 1);
    let delta = journal.section_delta(owner);
    assert!(journal.initialize_structures(&objects).unwrap().is_empty());
    assert_eq!(journal.section_delta(owner), delta);
}

#[test]
fn combined_generated_source_revision_overflow_is_rejected_before_any_batch_write() {
    let owner = SectionPos::new(-1, 0, -1);
    let mut journal = EditJournal::default();
    journal.sections.entry(owner).or_default().fluid_epoch = u64::MAX - 1;
    let before = journal.section_delta(owner);
    assert!(
        journal
            .initialize_structures(&[source([-1, 2, -1]), source([-2, 2, -1])])
            .is_err()
    );
    assert_eq!(journal.section_delta(owner), before);
    assert!(journal.fluid_active.is_empty());
    assert_eq!(journal.water_mesh_epoch(owner), 0);
}

#[test]
fn generated_roof_registration_keeps_sibling_snapshots_and_simulation_asleep() {
    let owner = SectionPos::new(-1, 0, -1);
    let objects: Vec<_> = (0..16)
        .flat_map(|z| (0..16).map(move |x| stair(owner.global_block([x, 15, z]).unwrap())))
        .collect();
    let mut journal = EditJournal::default();
    let affected = journal.initialize_structures(&objects).unwrap();
    assert_eq!(
        journal.revision(owner),
        256,
        "one public revision per canonical object, not two terrain edits"
    );
    assert_eq!(journal.edited_blocks(), 256);
    assert!(affected.contains(&owner.offset([0, 1, 0]).unwrap()));
    for &section in &affected {
        assert_eq!(
            journal.mesh_epoch(section),
            0,
            "already-generated sibling snapshots remain valid"
        );
        assert_eq!(
            journal.water_mesh_epoch(section),
            1,
            "partial-shape water invalidation is coalesced"
        );
    }
    for object in &objects {
        assert_eq!(journal.block(object.position), Some(object.material));
        assert_eq!(journal.object_state(object.position), object.state);
    }
    assert!(journal.fluid_active.is_empty());
    assert!(journal.fluid_queued.is_empty());
    #[cfg(feature = "bevy")]
    {
        assert!(journal.grass_candidates.is_empty());
        assert!(journal.grass_queued.is_empty());
    }
    let delta = journal.section_delta(owner);
    assert!(journal.initialize_structures(&objects).unwrap().is_empty());
    assert_eq!(journal.section_delta(owner), delta);
    for &section in &affected {
        assert_eq!(journal.mesh_epoch(section), 0);
        assert_eq!(journal.water_mesh_epoch(section), 1);
    }
    let mut replica = EditJournal::default();
    replica.import_delta(delta.clone()).unwrap();
    assert_eq!(
        replica.section_delta(owner),
        delta,
        "ordinary persistence/replication retains all orientations"
    );
}

#[test]
fn generated_metadata_and_tombstones_are_registered_once_without_refilling_contents() {
    let chest = [-2, 2, -3];
    let workbench = [-3, 2, -3];
    let furnace = [-4, 2, -3];
    let upper = [-4, 3, -3];
    let field = [-6, 2, -3];
    let mut inventory = terra_core::Inventory::default();
    assert_eq!(inventory.insert(terra_core::ItemKind::Block(1), 3), 0);
    let mut objects = vec![
        StructureObject {
            position: chest,
            material: 29,
            state: Some(Default::default()),
            loot: Some(inventory.clone()),
            crop: None,
        },
        StructureObject {
            position: workbench,
            material: 28,
            state: Some(Default::default()),
            loot: None,
            crop: None,
        },
        StructureObject {
            position: furnace,
            material: 59,
            state: Some(BlockObjectState {
                peer: Some(upper),
                ..Default::default()
            }),
            loot: None,
            crop: None,
        },
        StructureObject {
            position: upper,
            material: 59,
            state: Some(BlockObjectState {
                peer: Some(furnace),
                secondary: true,
                ..Default::default()
            }),
            loot: None,
            crop: None,
        },
        StructureObject {
            position: field,
            material: terra_core::farming::FARMLAND,
            state: None,
            loot: None,
            crop: Some(terra_core::CropKind::Wheat),
        },
    ];
    let mut journal = EditJournal::default();
    journal.initialize_structures(&objects).unwrap();
    let owner = global_to_section(chest).0;
    assert_eq!(journal.revision(owner), 5);
    assert_eq!(journal.container_at(chest), Some(&inventory));
    assert_eq!(journal.furniture_orders().count(), 2);
    assert_eq!(journal.furnace_at(furnace).unwrap().outlet, [-4, 4, -3]);
    assert!(journal.crop_at(field).unwrap().mature());
    let prop_id = crate::props::PropId(journal.crop_at(field).unwrap().prop_id);
    assert!(journal.placed_props(owner).any(|prop| prop.id == prop_id));
    journal.set_container(chest, Default::default()).unwrap();
    journal.remove_prop(owner, prop_id).unwrap();
    journal.remove_crop(field);
    let removed = stair([-8, 2, -3]);
    journal.record(removed.position, 0).unwrap();
    objects.push(removed.clone());
    let delta = journal.section_delta(owner);
    let revision = journal.gameplay().revision;
    assert!(journal.initialize_structures(&objects).unwrap().is_empty());
    assert_eq!(journal.section_delta(owner), delta);
    assert_eq!(journal.gameplay().revision, revision);
    assert_eq!(journal.container_at(chest), Some(&Default::default()));
    assert_eq!(journal.block(removed.position), Some(0));
    assert!(journal.crop_at(field).is_none());
    assert!(journal.prop_removed(owner, prop_id));
}

#[test]
fn invalid_late_metadata_or_overflow_cannot_partially_register_a_structure_batch() {
    let valid = stair([0, 15, 0]);
    let mut invalid = stair([1, 15, 0]);
    invalid.state.as_mut().unwrap().facing = u8::MAX;
    let mut journal = EditJournal::default();
    assert!(
        journal
            .initialize_structures(&[valid.clone(), invalid])
            .is_err()
    );
    assert_eq!(journal.edited_blocks(), 0);
    assert_eq!(journal.revision(global_to_section(valid.position).0), 0);
    assert_eq!(
        journal.water_mesh_epoch(global_to_section(valid.position).0),
        0
    );
    assert!(journal.fluid_active.is_empty());
    assert!(journal.furniture_orders().next().is_none());

    let overflow = global_to_section(valid.position)
        .0
        .offset([0, 1, 0])
        .unwrap();
    journal.water_mesh_epochs.insert(overflow, u64::MAX);
    assert!(journal.initialize_structures(&[valid]).is_err());
    assert_eq!(journal.edited_blocks(), 0);
    assert_eq!(journal.water_mesh_epoch(overflow), u64::MAX);
    assert_eq!(journal.water_mesh_epoch(SectionPos::new(0, 0, 0)), 0);
}

#[test]
fn batch_preflight_checks_combined_crop_epochs_before_registering_farmland() {
    let owner = SectionPos::new(-1, 0, -1);
    let objects: Vec<_> = [0, 1]
        .into_iter()
        .map(|x| StructureObject {
            position: owner.global_block([x, 2, 3]).unwrap(),
            material: terra_core::farming::FARMLAND,
            state: None,
            loot: None,
            crop: Some(terra_core::CropKind::Lettuce),
        })
        .collect();
    let mut journal = EditJournal::default();
    journal.sections.entry(owner).or_default().prop_epoch = u64::MAX - 1;
    let before = journal.section_delta(owner);
    assert!(journal.initialize_structures(&objects).is_err());
    assert_eq!(journal.section_delta(owner), before);
    assert!(journal.crops().next().is_none());
    assert_eq!(journal.water_mesh_epoch(owner), 0);
}

#[cfg(feature = "bevy")]
#[test]
fn only_selected_loaded_meshes_are_invalidated_and_real_edits_still_wake_simulation() {
    let object = stair([-1, 15, -1]);
    let owner = global_to_section(object.position).0;
    let mut journal = EditJournal::default();
    let affected = journal
        .initialize_structures(std::slice::from_ref(&object))
        .unwrap();
    let loaded_neighbor = owner.offset([1, 1, 1]).unwrap();
    assert!(affected.contains(&loaded_neighbor));
    journal
        .invalidate_structure_meshes(&BTreeSet::from([loaded_neighbor]))
        .unwrap();
    assert_eq!(journal.mesh_epoch(loaded_neighbor), 1);
    assert_eq!(journal.mesh_epoch(owner), 0);
    assert_eq!(journal.revision(owner), 1);
    assert!(journal.fluid_active.is_empty());
    journal.record(object.position, 0).unwrap();
    assert_eq!(journal.mesh_epoch(owner), 1);
    assert_eq!(journal.mesh_epoch(loaded_neighbor), 2);
    assert_eq!(journal.revision(owner), 2);
    assert!(!journal.fluid_active.is_empty());
    assert!(!journal.grass_candidates.is_empty());
}

#[test]
fn generated_ornate_crypt_loot_initializes_persists_and_never_refills_after_loading() {
    let position = [-17, -1, 31];
    let mut loot = terra_core::Inventory::default();
    loot.insert(
        terra_core::ItemKind::Mineral(terra_core::MineralItem::RawIron),
        7,
    );
    let object = StructureObject {
        position,
        material: terra_core::grave_blocks::ORNATE_CHEST,
        state: Some(BlockObjectState {
            facing: 2,
            ..Default::default()
        }),
        loot: Some(loot.clone()),
        crop: None,
    };
    let objects = [object];
    let mut journal = EditJournal::default();
    assert!(!journal.initialize_structures(&objects).unwrap().is_empty());
    assert_eq!(journal.container_at(position), Some(&loot));
    let owner = global_to_section(position).0;
    let delta = serde_json::to_string(&journal.section_delta(owner)).unwrap();
    let containers = serde_json::to_string(&journal.containers().collect::<Vec<_>>()).unwrap();
    let mut loaded = EditJournal::default();
    loaded
        .import_delta(serde_json::from_str(&delta).unwrap())
        .unwrap();
    let contents: Vec<([i64; 3], terra_core::Inventory)> =
        serde_json::from_str(&containers).unwrap();
    for (position, inventory) in contents {
        loaded.set_container(position, inventory).unwrap();
    }
    assert_eq!(
        loaded.block(position),
        Some(terra_core::grave_blocks::ORNATE_CHEST)
    );
    assert_eq!(loaded.object_state(position).unwrap().facing, 2);
    assert_eq!(loaded.container_at(position), Some(&loot));
    assert!(loaded.initialize_structures(&objects).unwrap().is_empty());
    loaded
        .set_container(position, terra_core::Inventory::default())
        .unwrap();
    let containers = serde_json::to_string(&loaded.containers().collect::<Vec<_>>()).unwrap();
    let mut looted = EditJournal::default();
    looted
        .import_delta(serde_json::from_str(&delta).unwrap())
        .unwrap();
    let contents: Vec<([i64; 3], terra_core::Inventory)> =
        serde_json::from_str(&containers).unwrap();
    for (position, inventory) in contents {
        looted.set_container(position, inventory).unwrap();
    }
    assert!(looted.initialize_structures(&objects).unwrap().is_empty());
    assert_eq!(
        looted.container_at(position),
        Some(&terra_core::Inventory::default())
    );
}
