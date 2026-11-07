use super::*;
use crate::{
    props::{PropKind, WorldProp},
    streaming::{LoadedSection, PropRemoveRequest, commit_prop_removal},
};
use std::time::Duration;
use terra_voxel::{BlockSection, SectionPos};

const SUPPORT: [i64; 3] = [-1, 15, -1];

#[derive(Resource, Default)]
struct FarmChanges(Vec<(bool, bool)>);

fn observe_farm_changes(
    chunks: Res<ChunkStore>,
    journal: Res<EditJournal>,
    mut changes: ResMut<FarmChanges>,
) {
    changes.0.push((chunks.is_changed(), journal.is_changed()));
}

fn track_farm_changes(app: &mut App) {
    app.init_resource::<FarmChanges>()
        .init_resource::<CropReconciliations>()
        .add_systems(Update, observe_farm_changes.after(tick));
}

#[test]
fn progress_only_ticks_persist_crop_state_without_mutating_loaded_chunks_or_reflowing_props() {
    let mut app = scene();
    sow(&mut app, CropKind::Wheat);
    let mut state = *app
        .world()
        .resource::<EditJournal>()
        .crop_at(SUPPORT)
        .unwrap();
    state.moisture = 1.0;
    state.growth_duration_seconds = 3600.0;
    set_state(&mut app, state);
    track_farm_changes(&mut app);
    app.update();
    let before = *app
        .world()
        .resource::<EditJournal>()
        .crop_at(SUPPORT)
        .unwrap();
    let owner = global_to_section(SUPPORT).0;
    let epoch = app.world().resource::<EditJournal>().prop_epoch(owner);
    app.update();
    assert_eq!(
        app.world().resource::<FarmChanges>().0.last(),
        Some(&(false, true))
    );
    let journal = app.world().resource::<EditJournal>();
    assert!(journal.crop_at(SUPPORT).unwrap().progress_seconds > before.progress_seconds);
    assert_eq!(journal.prop_epoch(owner), epoch);
    assert_eq!(app.world().resource::<CropReconciliations>().0, 0);
    assert_eq!(
        app.world().resource::<ChunkStore>().loaded[&owner].props[0].variant,
        0
    );
}

#[test]
fn sixty_four_synchronized_crop_stages_reconcile_once_per_owner_and_publish_all_new_variants() {
    let mut app = scene();
    let positions: Vec<_> = (-8..=-1)
        .flat_map(|z| (-8..=-1).map(move |x| [x, 15, z]))
        .collect();
    let owner = global_to_section(SUPPORT).0;
    app.world_mut()
        .resource_scope(|world, mut journal: Mut<EditJournal>| {
            let mut chunks = world.resource_mut::<ChunkStore>();
            for support in &positions {
                let (_, local) = global_to_section(*support);
                chunks
                    .loaded
                    .get_mut(&owner)
                    .unwrap()
                    .data
                    .set_block(local, 2)
                    .unwrap();
                assert!(
                    plant_crop(
                        &mut chunks,
                        &mut journal,
                        &Default::default(),
                        prop(CropKind::Wheat, *support, 77)
                    )
                    .unwrap()
                );
                let mut state = *journal.crop_at(*support).unwrap();
                state.growth_duration_seconds = 700.0;
                state.progress_seconds = 99.9;
                state.moisture = 1.0;
                assert_eq!(state.stage(), 0);
                commit_crop_state(&mut chunks, &mut journal, *support, state).unwrap();
            }
        });
    let epoch = app.world().resource::<EditJournal>().prop_epoch(owner);
    let identities: std::collections::BTreeSet<_> = app.world().resource::<ChunkStore>().loaded
        [&owner]
        .props
        .iter()
        .map(|p| p.id)
        .collect();
    assert_eq!(identities.len(), 64);
    track_farm_changes(&mut app);
    app.update();
    assert_eq!(app.world().resource::<CropReconciliations>().0, 1);
    let journal = app.world().resource::<EditJournal>();
    assert_eq!(journal.prop_epoch(owner), epoch + 64);
    assert!(
        positions
            .iter()
            .all(|p| journal.crop_at(*p).unwrap().stage() == 1)
    );
    let props = &app.world().resource::<ChunkStore>().loaded[&owner].props;
    assert_eq!(props.len(), 64);
    assert!(props.iter().all(|p| p.variant == 1));
    assert_eq!(
        props
            .iter()
            .map(|p| p.id)
            .collect::<std::collections::BTreeSet<_>>(),
        identities
    );
    app.update();
    assert_eq!(app.world().resource::<CropReconciliations>().0, 1);
    assert_eq!(
        app.world().resource::<FarmChanges>().0.last(),
        Some(&(false, true))
    );
}

#[test]
fn unchanged_watered_night_crops_do_not_mark_either_world_resource_changed() {
    let mut app = scene();
    sow(&mut app, CropKind::Wheat);
    let mut state = *app
        .world()
        .resource::<EditJournal>()
        .crop_at(SUPPORT)
        .unwrap();
    state.moisture = 1.0;
    set_state(&mut app, state);
    app.world_mut().resource_mut::<WorldClock>().phase = 0.0;
    track_farm_changes(&mut app);
    app.update();
    app.update();
    assert_eq!(
        app.world().resource::<FarmChanges>().0.last(),
        Some(&(false, false))
    );
    assert_eq!(app.world().resource::<CropReconciliations>().0, 0);
}

fn prop(kind: CropKind, support: [i64; 3], id: u64) -> WorldProp {
    WorldProp {
        id: PropId(id),
        kind: kind.resource_prop().into(),
        position: [
            support[0] as f64 + 0.5,
            support[1] as f64 + 1.0,
            support[2] as f64 + 0.5,
        ],
        rotation: 0.0,
        scale: 1.0,
        variant: 0,
        section: global_to_section(support).0,
        removed: false,
    }
}
fn loaded(position: SectionPos) -> LoadedSection {
    LoadedSection {
        data: BlockSection::empty(position).unwrap(),
        mesh: None,
        mesh_epoch: 0,
        water_epoch: 0,
        water: None,
        props: Vec::new(),
    }
}
fn scene() -> App {
    let config = terra_core::WorldConfig {
        base_height: 15.0,
        biomes: false,
        hill_amplitude: 0.0,
        mountain_amplitude: 0.0,
        caves: false,
        rivers: false,
        ..Default::default()
    };
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .insert_resource(WorldAccess::Server)
        .init_resource::<EditJournal>()
        .init_resource::<ChunkStore>()
        .init_resource::<BlockPropertiesCatalog>()
        .init_resource::<WorldClock>()
        .insert_resource(WorldTerrain(crate::TerraGenerator::new(config).unwrap()))
        .insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
            Duration::from_secs(1),
        ))
        .add_systems(Update, tick);
    app.world_mut()
        .resource_mut::<Time<Virtual>>()
        .set_max_delta(Duration::from_secs(1));
    let mut chunks = app.world_mut().resource_mut::<ChunkStore>();
    let owner = global_to_section(SUPPORT).0;
    let mut lower = loaded(owner);
    lower
        .data
        .set_block([15, 15, 15], MaterialId::Dirt as u16)
        .unwrap();
    lower
        .data
        .set_block([14, 15, 15], MaterialId::FluidWater as u16)
        .unwrap();
    chunks.loaded.insert(owner, lower);
    chunks.loaded.insert(
        owner.offset([0, 1, 0]).unwrap(),
        loaded(owner.offset([0, 1, 0]).unwrap()),
    );
    app
}
fn sow(app: &mut App, kind: CropKind) {
    app.world_mut()
        .resource_scope(|world, mut journal: Mut<EditJournal>| {
            let properties = world.resource::<BlockPropertiesCatalog>().clone();
            let mut chunks = world.resource_mut::<ChunkStore>();
            assert!(
                plant_crop(
                    &mut chunks,
                    &mut journal,
                    &properties,
                    prop(kind, SUPPORT, 77)
                )
                .unwrap()
            );
        });
}
fn set_state(app: &mut App, state: CropState) {
    app.world_mut()
        .resource_scope(|world, mut journal: Mut<EditJournal>| {
            commit_crop_state(
                &mut world.resource_mut::<ChunkStore>(),
                &mut journal,
                SUPPORT,
                state,
            )
            .unwrap();
        });
}

#[test]
fn real_pollination_commits_at_most_four_nearby_moist_crops_without_replacing_ids() {
    let mut app = scene();
    let positions = [
        [-1, 15, -1],
        [-2, 15, -1],
        [-3, 15, -1],
        [-1, 15, -2],
        [-2, 15, -2],
    ];
    app.world_mut()
        .resource_scope(|world, mut journal: Mut<EditJournal>| {
            let mut chunks = world.resource_mut::<ChunkStore>();
            for (index, support) in positions.into_iter().enumerate() {
                let (owner, local) = global_to_section(support);
                chunks
                    .loaded
                    .get_mut(&owner)
                    .unwrap()
                    .data
                    .set_block(local, 2)
                    .unwrap();
                assert!(
                    plant_crop(
                        &mut chunks,
                        &mut journal,
                        &Default::default(),
                        prop(CropKind::Lettuce, support, 100 + index as u64)
                    )
                    .unwrap()
                );
            }
            let ids: Vec<_> = positions
                .iter()
                .map(|p| journal.crop_at(*p).unwrap().prop_id)
                .collect();
            assert!(pollinate_near(&mut chunks, &mut journal, [-1.5, 16.0, -1.5]).unwrap());
            assert_eq!(
                positions
                    .iter()
                    .filter(|p| journal.crop_at(**p).unwrap().progress_seconds == 5.0)
                    .count(),
                4
            );
            assert_eq!(
                positions
                    .iter()
                    .map(|p| journal.crop_at(*p).unwrap().prop_id)
                    .collect::<Vec<_>>(),
                ids
            );
            for support in positions {
                let mut state = *journal.crop_at(support).unwrap();
                state.moisture = 0.0;
                commit_crop_state(&mut chunks, &mut journal, support, state).unwrap();
            }
            assert!(!pollinate_near(&mut chunks, &mut journal, [-1.5, 16.0, -1.5]).unwrap());
        });
}

#[test]
fn mixed_adjacent_crops_preserve_legacy_ids_then_replant_with_distinct_new_ids() {
    let mut app = scene();
    let owner = global_to_section(SUPPORT).0;
    let neighbor = [SUPPORT[0] - 1, SUPPORT[1], SUPPORT[2]];
    let mut hash = 0x4352_4f50_534f_5731_u64 ^ CropKind::Wheat as u64;
    for value in SUPPORT {
        hash = (hash ^ value as u64).wrapping_mul(0x100_0000_01b3);
    }
    let legacy = prop(CropKind::Wheat, SUPPORT, hash.max(1));
    let state = CropState::new(CropKind::Wheat, legacy.id.0);
    let mut saved = EditJournal::default();
    saved.record(SUPPORT, FARMLAND).unwrap();
    saved.place_prop(legacy).unwrap();
    saved.set_crop(SUPPORT, state).unwrap();
    let mut restored = EditJournal::default();
    restored.import_delta(saved.section_delta(owner)).unwrap();
    restored.set_crop(SUPPORT, state).unwrap();
    app.insert_resource(restored);
    {
        let mut chunks = app.world_mut().resource_mut::<ChunkStore>();
        let section = chunks.loaded.get_mut(&owner).unwrap();
        section
            .data
            .set_block(global_to_section(SUPPORT).1, FARMLAND)
            .unwrap();
        section
            .data
            .set_block(global_to_section(neighbor).1, MaterialId::Dirt as u16)
            .unwrap();
        section.props.push(legacy);
    }
    app.world_mut()
        .resource_scope(|world, mut journal: Mut<EditJournal>| {
            let properties = world.resource::<BlockPropertiesCatalog>().clone();
            let mut chunks = world.resource_mut::<ChunkStore>();
            assert!(
                plant_crop(
                    &mut chunks,
                    &mut journal,
                    &properties,
                    prop(CropKind::Corn, neighbor, 1)
                )
                .unwrap()
            );
            assert_eq!(*journal.crop_at(SUPPORT).unwrap(), state);
            let adjacent = *journal.crop_at(neighbor).unwrap();
            assert_ne!(adjacent.prop_id, legacy.id.0);
            assert_eq!(journal.section_delta(owner).placed_props.len(), 2);
            assert!(
                commit_prop_removal(
                    &mut chunks,
                    &mut journal,
                    &PropRemoveRequest {
                        section: owner,
                        id: legacy.id
                    }
                )
                .unwrap()
            );
            assert!(
                plant_crop(
                    &mut chunks,
                    &mut journal,
                    &properties,
                    prop(CropKind::Wheat, SUPPORT, 1)
                )
                .unwrap()
            );
            let replanted = journal.crop_at(SUPPORT).unwrap().prop_id;
            assert_eq!(
                replanted,
                crate::farming::crop_id(SUPPORT, CropKind::Wheat).0
            );
            assert_ne!(replanted, legacy.id.0);
            assert_ne!(replanted, adjacent.prop_id);
            assert!(journal.prop_removed(owner, legacy.id));
            assert_eq!(journal.section_delta(owner).placed_props.len(), 2);
            assert_eq!(journal.crops().count(), 2);
        });
}

#[test]
fn sowing_tills_once_and_owns_one_centered_crop_across_negative_section_boundary() {
    for kind in CropKind::ALL {
        let mut app = scene();
        sow(&mut app, kind);
        let journal = app.world().resource::<EditJournal>();
        assert_eq!(journal.block(SUPPORT), Some(FARMLAND));
        let saved = journal
            .section_delta(global_to_section(SUPPORT).0)
            .placed_props;
        assert_eq!(saved.len(), 1);
        assert_eq!(placement::support(&saved[0]), SUPPORT);
        assert_eq!(saved[0].position, [-0.5, 16.0, -0.5]);
        assert_eq!(saved[0].variant, 0);
        assert!(saved[0].valid());
        assert_eq!(journal.crop_at(SUPPORT).unwrap().kind, kind);
        app.world_mut()
            .resource_scope(|world, mut journal: Mut<EditJournal>| {
                let properties = world.resource::<BlockPropertiesCatalog>().clone();
                assert!(
                    !plant_crop(
                        &mut world.resource_mut::<ChunkStore>(),
                        &mut journal,
                        &properties,
                        prop(kind, SUPPORT, 78)
                    )
                    .unwrap()
                );
            });
    }
}

#[test]
fn lettuce_plants_on_unjournaled_prepared_farmland_and_replants_without_rebuilding_soil() {
    let mut app = scene();
    let owner = global_to_section(SUPPORT).0;
    app.world_mut()
        .resource_mut::<ChunkStore>()
        .loaded
        .get_mut(&owner)
        .unwrap()
        .data
        .set_block([15, 15, 15], FARMLAND)
        .unwrap();
    assert_eq!(app.world().resource::<EditJournal>().block(SUPPORT), None);
    sow(&mut app, CropKind::Lettuce);
    let journal = app.world().resource::<EditJournal>();
    assert_eq!(journal.block(SUPPORT), Some(FARMLAND));
    let mesh_epoch = journal.mesh_epoch(owner);
    let revision = journal.revision(owner);
    let crop = *journal.crop_at(SUPPORT).unwrap();
    let initial = journal.section_delta(owner);
    assert_eq!(initial.placed_props.len(), 1);
    assert_eq!(placement::support(&initial.placed_props[0]), SUPPORT);
    assert_eq!(initial.placed_props[0].position, [-0.5, 16.0, -0.5]);
    app.world_mut()
        .resource_scope(|world, mut journal: Mut<EditJournal>| {
            assert!(
                commit_prop_removal(
                    &mut world.resource_mut::<ChunkStore>(),
                    &mut journal,
                    &PropRemoveRequest {
                        section: owner,
                        id: PropId(crop.prop_id)
                    }
                )
                .unwrap()
            );
        });
    assert!(
        app.world()
            .resource::<EditJournal>()
            .crop_at(SUPPORT)
            .is_none()
    );
    sow(&mut app, CropKind::Lettuce);
    let journal = app.world().resource::<EditJournal>();
    assert_eq!(journal.mesh_epoch(owner), mesh_epoch);
    assert_eq!(journal.revision(owner), revision);
    assert_eq!(journal.crop_at(SUPPORT).unwrap().prop_id, crop.prop_id);
    assert_eq!(journal.section_delta(owner).placed_props.len(), 1);
    let mut replica = EditJournal::default();
    replica.import_delta(journal.section_delta(owner)).unwrap();
    assert_eq!(replica.block(SUPPORT), Some(FARMLAND));
}

#[test]
fn lettuce_never_replaces_existing_decor_or_seeds_through_missing_headroom() {
    for blocker in [PropKind::Grass, PropKind::Flower, PropKind::Mushroom] {
        let mut app = scene();
        let owner = global_to_section(SUPPORT).0;
        let mut decoration = prop(CropKind::Lettuce, SUPPORT, 99);
        decoration.kind = blocker;
        app.world_mut()
            .resource_mut::<ChunkStore>()
            .loaded
            .get_mut(&owner)
            .unwrap()
            .props
            .push(decoration);
        app.world_mut()
            .resource_scope(|world, mut journal: Mut<EditJournal>| {
                let properties = world.resource::<BlockPropertiesCatalog>().clone();
                assert!(
                    !plant_crop(
                        &mut world.resource_mut::<ChunkStore>(),
                        &mut journal,
                        &properties,
                        prop(CropKind::Lettuce, SUPPORT, 77)
                    )
                    .unwrap()
                );
                assert!(journal.crop_at(SUPPORT).is_none());
                assert_eq!(journal.block(SUPPORT), None);
                assert_eq!(journal.prop_epoch(owner), 0);
            });
    }
    let mut app = scene();
    let owner = global_to_section(SUPPORT).0;
    app.world_mut()
        .resource_mut::<ChunkStore>()
        .loaded
        .remove(&owner.offset([0, 1, 0]).unwrap());
    app.world_mut()
        .resource_scope(|world, mut journal: Mut<EditJournal>| {
            let properties = world.resource::<BlockPropertiesCatalog>().clone();
            assert!(
                !plant_crop(
                    &mut world.resource_mut::<ChunkStore>(),
                    &mut journal,
                    &properties,
                    prop(CropKind::Lettuce, SUPPORT, 77)
                )
                .unwrap()
            );
            assert_eq!(journal.revision(owner), 0);
            assert!(journal.crop_at(SUPPORT).is_none());
        });
}

#[test]
fn growth_stages_replicate_without_terrain_rebuilds_or_per_tick_prop_epochs() {
    let mut app = scene();
    sow(&mut app, CropKind::Wheat);
    let owner = global_to_section(SUPPORT).0;
    let journal = app.world().resource::<EditJournal>();
    let mesh = journal.mesh_epoch(owner);
    let initial_epoch = journal.prop_epoch(owner);
    let mut state = *journal.crop_at(SUPPORT).unwrap();
    state.progress_seconds = 1.0;
    set_state(&mut app, state);
    assert_eq!(
        app.world().resource::<EditJournal>().prop_epoch(owner),
        initial_epoch
    );
    for stage in 1..=7 {
        state.progress_seconds = CropKind::Wheat.definition().growth_seconds * stage as f32 / 7.0
            + if stage == 7 { 0.0 } else { 0.001 };
        set_state(&mut app, state);
        let journal = app.world().resource::<EditJournal>();
        assert_eq!(journal.mesh_epoch(owner), mesh);
        let delta = journal.section_delta(owner);
        assert_eq!(delta.placed_props[0].variant, stage);
        let mut replica = EditJournal::default();
        replica.import_delta(delta).unwrap();
        let mut props = Vec::new();
        replica.apply_prop_removals(owner, &mut props);
        assert_eq!(props.len(), 1);
        assert_eq!(props[0].variant, stage);
        assert_eq!(
            props[0].id,
            crate::farming::crop_id(SUPPORT, CropKind::Wheat)
        );
    }
    assert_eq!(
        app.world().resource::<EditJournal>().prop_epoch(owner),
        initial_epoch + 7
    );
}

#[test]
fn real_tick_ripens_in_light_and_water_but_pauses_for_missing_headroom_and_unloaded_owner() {
    let mut app = scene();
    sow(&mut app, CropKind::Corn);
    let mut state = *app
        .world()
        .resource::<EditJournal>()
        .crop_at(SUPPORT)
        .unwrap();
    state.progress_seconds = state.kind.definition().growth_seconds - 0.1;
    state.moisture = 1.0;
    set_state(&mut app, state);
    let owner = global_to_section(SUPPORT).0;
    let upper = owner.offset([0, 1, 0]).unwrap();
    let unloaded = app
        .world_mut()
        .resource_mut::<ChunkStore>()
        .loaded
        .remove(&upper)
        .unwrap();
    for _ in 0..5 {
        app.update();
    }
    assert_eq!(
        *app.world()
            .resource::<EditJournal>()
            .crop_at(SUPPORT)
            .unwrap(),
        state
    );
    app.world_mut()
        .resource_mut::<ChunkStore>()
        .loaded
        .insert(upper, unloaded);
    for _ in 0..3 {
        app.update();
    }
    assert!(
        app.world()
            .resource::<EditJournal>()
            .crop_at(SUPPORT)
            .unwrap()
            .mature()
    );
    state.progress_seconds = 0.0;
    set_state(&mut app, state);
    let unloaded = app
        .world_mut()
        .resource_mut::<ChunkStore>()
        .loaded
        .remove(&owner)
        .unwrap();
    for _ in 0..15 {
        app.update();
    }
    assert_eq!(
        *app.world()
            .resource::<EditJournal>()
            .crop_at(SUPPORT)
            .unwrap(),
        state
    );
    app.world_mut()
        .resource_mut::<ChunkStore>()
        .loaded
        .insert(owner, unloaded);
    app.update();
    assert!(
        app.world()
            .resource::<EditJournal>()
            .crop_at(SUPPORT)
            .unwrap()
            .progress_seconds
            < 1.01
    );
}

#[test]
fn crop_removal_releases_state_and_covering_plant_retires_without_orphan_record() {
    for position in [SUPPORT, [-1, 17, -1]] {
        let mut app = scene();
        sow(&mut app, CropKind::Corn);
        app.world_mut()
            .resource_scope(|world, mut journal: Mut<EditJournal>| {
                let properties = world.resource::<BlockPropertiesCatalog>().clone();
                let mut chunks = world.resource_mut::<ChunkStore>();
                let expected = chunks.block_at(position).unwrap();
                assert!(
                    commit_block_edit(
                        &mut chunks,
                        &mut journal,
                        &BlockEditRequest {
                            position,
                            expected_material: expected,
                            material: MaterialId::Stone as u16
                        },
                        &properties
                    )
                    .unwrap()
                );
                assert!(journal.crop_at(SUPPORT).is_none());
                assert_eq!(journal.crops().count(), 0);
                assert!(journal.prop_removed(
                    global_to_section(SUPPORT).0,
                    crate::farming::crop_id(SUPPORT, CropKind::Corn)
                ));
            });
    }
}

#[test]
fn grazed_foraging_plant_restores_same_identity_without_multiplication_or_terrain_work() {
    let mut app = scene();
    let owner = global_to_section(SUPPORT).0;
    let mut original = prop(CropKind::Wheat, SUPPORT, 77);
    original.kind = PropKind::Grass;
    app.world_mut()
        .resource_mut::<ChunkStore>()
        .loaded
        .get_mut(&owner)
        .unwrap()
        .props
        .push(original);
    app.world_mut()
        .resource_scope(|world, mut journal: Mut<EditJournal>| {
            assert!(
                commit_prop_removal(
                    &mut world.resource_mut::<ChunkStore>(),
                    &mut journal,
                    &PropRemoveRequest {
                        section: owner,
                        id: original.id
                    }
                )
                .unwrap()
            );
            journal
                .set_forage_regrowth(ForageRegrowth {
                    prop: original,
                    remaining_seconds: 1.0,
                })
                .unwrap();
        });
    let epoch = app.world().resource::<EditJournal>().mesh_epoch(owner);
    for _ in 0..5 {
        app.update();
    }
    let journal = app.world().resource::<EditJournal>();
    assert_eq!(journal.forage_regrowth().count(), 0);
    assert!(!journal.prop_removed(owner, original.id));
    assert_eq!(journal.mesh_epoch(owner), epoch);
    let section = &app.world().resource::<ChunkStore>().loaded[&owner];
    assert_eq!(
        section
            .props
            .iter()
            .filter(|prop| prop.id == original.id && !prop.removed)
            .count(),
        1
    );
    let mut replica = EditJournal::default();
    replica.import_delta(journal.section_delta(owner)).unwrap();
    let mut regenerated = vec![original];
    replica.apply_prop_removals(owner, &mut regenerated);
    replica.apply_prop_removals(owner, &mut regenerated);
    assert_eq!(regenerated.len(), 1);
}

#[test]
fn five_thousand_replants_reuse_three_identities_and_never_exhaust_a_section() {
    let mut app = scene();
    let owner = global_to_section(SUPPORT).0;
    app.world_mut()
        .resource_scope(|world, mut journal: Mut<EditJournal>| {
            let properties = world.resource::<BlockPropertiesCatalog>().clone();
            let mut chunks = world.resource_mut::<ChunkStore>();
            for cycle in 0..5000 {
                let kind = CropKind::ALL[cycle % 3];
                assert!(
                    plant_crop(
                        &mut chunks,
                        &mut journal,
                        &properties,
                        prop(kind, SUPPORT, cycle as u64 + 1)
                    )
                    .unwrap()
                );
                let id = crate::farming::crop_id(SUPPORT, kind);
                assert_eq!(journal.crop_at(SUPPORT).unwrap().prop_id, id.0);
                assert_eq!(
                    chunks.loaded[&owner]
                        .props
                        .iter()
                        .filter(|prop| !prop.removed)
                        .count(),
                    1
                );
                assert!(
                    commit_prop_removal(
                        &mut chunks,
                        &mut journal,
                        &PropRemoveRequest { section: owner, id }
                    )
                    .unwrap()
                );
                assert!(journal.crop_at(SUPPORT).is_none());
                assert!(journal.section_delta(owner).removed_props.len() <= 3);
                assert!(chunks.loaded[&owner].props.len() <= 3);
                assert!(journal.section_delta(owner).placed_props.is_empty());
            }
            assert_eq!(journal.removed_props(), 3);
            assert_eq!(chunks.loaded[&owner].props.len(), 3);
            assert_eq!(journal.crops().count(), 0);
        });
}

#[test]
fn rejected_crop_capacity_does_not_till_soil_or_advance_any_terrain_revision() {
    let mut app = scene();
    let owner = global_to_section(SUPPORT).0;
    app.world_mut()
        .resource_scope(|world, mut journal: Mut<EditJournal>| {
            for index in 0..256 {
                let mut filler = prop(
                    CropKind::Wheat,
                    [-16 + index % 16, 0, -16 + index / 16],
                    index as u64 + 100,
                );
                filler.kind = PropKind::Flower;
                assert!(journal.place_prop(filler).unwrap());
            }
            let properties = world.resource::<BlockPropertiesCatalog>().clone();
            let epoch = journal.prop_epoch(owner);
            let mut chunks = world.resource_mut::<ChunkStore>();
            assert!(
                !plant_crop(
                    &mut chunks,
                    &mut journal,
                    &properties,
                    prop(CropKind::Wheat, SUPPORT, 77)
                )
                .unwrap()
            );
            assert_eq!(chunks.block_at(SUPPORT), Some(MaterialId::Dirt as u16));
            assert_eq!(journal.block(SUPPORT), None);
            assert_eq!(journal.revision(owner), 0);
            assert_eq!(journal.mesh_epoch(owner), 0);
            assert_eq!(journal.prop_epoch(owner), epoch);
            assert_eq!(journal.crops().count(), 0);
        });
}

fn roof(app: &mut App, material: u16) {
    let position = [-1, 18, -1];
    let (owner, local) = global_to_section(position);
    app.world_mut()
        .resource_mut::<ChunkStore>()
        .loaded
        .get_mut(&owner)
        .unwrap()
        .data
        .set_block(local, material)
        .unwrap();
}

#[test]
fn glass_roofs_transmit_daylight_but_only_open_sky_provides_rainwater() {
    for material in [15, 80, 88, 96] {
        let mut app = scene();
        roof(&mut app, material);
        let exposure = sky(
            app.world().resource::<ChunkStore>(),
            app.world().resource::<WorldTerrain>(),
            SUPPORT,
        );
        assert_eq!(
            exposure,
            SkyExposure {
                light: 0.9,
                rain: 0.0
            }
        );
        sow(&mut app, CropKind::Wheat);
        for _ in 0..4 {
            app.update();
        }
        assert!(
            app.world()
                .resource::<EditJournal>()
                .crop_at(SUPPORT)
                .unwrap()
                .progress_seconds
                > 0.0,
            "Irrigated crop must grow under glass material {material}"
        );
    }
    let mut dark = scene();
    roof(&mut dark, MaterialId::Stone as u16);
    sow(&mut dark, CropKind::Wheat);
    for _ in 0..4 {
        dark.update();
    }
    assert_eq!(
        dark.world()
            .resource::<EditJournal>()
            .crop_at(SUPPORT)
            .unwrap()
            .progress_seconds,
        0.0
    );

    for (material, receives_rain) in [(0, true), (15, false), (3, false)] {
        let mut app = scene();
        roof(&mut app, material);
        app.world_mut()
            .resource_mut::<ChunkStore>()
            .loaded
            .get_mut(&global_to_section(SUPPORT).0)
            .unwrap()
            .data
            .set_block([14, 15, 15], 0)
            .unwrap();
        let mut weather = WorldWeather::new(1);
        weather.pattern = terra_core::WeatherPattern::Rain;
        weather.from_intensity = 1.0;
        weather.target_intensity = 1.0;
        app.insert_resource(weather);
        sow(&mut app, CropKind::Wheat);
        let mut state = *app
            .world()
            .resource::<EditJournal>()
            .crop_at(SUPPORT)
            .unwrap();
        state.moisture = 0.0;
        set_state(&mut app, state);
        for _ in 0..6 {
            app.update();
        }
        let state = app
            .world()
            .resource::<EditJournal>()
            .crop_at(SUPPORT)
            .unwrap();
        assert_eq!(
            state.moisture > 0.0,
            receives_rain,
            "Roof material {material}"
        );
        assert_eq!(
            state.progress_seconds > 0.0,
            receives_rain,
            "Only exposed, rain-watered crops may grow here: {material}"
        );
    }
}
#[test]
fn watering_can_ray_uses_negative_coordinates_and_stops_at_walls_and_unloaded_sections() {
    let owner = terra_voxel::SectionPos::new(-1, 0, -1);
    let mut data = terra_voxel::BlockSection::empty(owner).unwrap();
    data.set_block([12, 3, 12], 27).unwrap();
    let mut chunks = ChunkStore::default();
    chunks.loaded.insert(
        owner,
        crate::streaming::LoadedSection {
            data,
            mesh: None,
            mesh_epoch: 0,
            water_epoch: 0,
            water: None,
            props: Vec::new(),
        },
    );
    let mut journal = EditJournal::default();
    assert_eq!(
        water_target(&chunks, &journal, [-3.5, 3.5, -1.5], [0.0, 0.0, -1.0]),
        Some([-4, 3, -4])
    );
    // Finite runoff is not a refill source. Explicit source overrides remain usable.
    chunks
        .loaded
        .get_mut(&owner)
        .unwrap()
        .data
        .set_block([12, 3, 12], 0)
        .unwrap();
    journal
        .set_fluid(
            [-4, 3, -4],
            crate::water::FluidCell {
                amount: 4,
                flow: [0; 2],
                source: false,
            },
        )
        .unwrap();
    assert_eq!(
        water_target(&chunks, &journal, [-3.5, 3.5, -1.5], [0.0, 0.0, -1.0]),
        None
    );
    journal
        .set_fluid(
            [-4, 3, -4],
            crate::water::FluidCell {
                amount: 4,
                flow: [0; 2],
                source: true,
            },
        )
        .unwrap();
    assert_eq!(
        water_target(&chunks, &journal, [-3.5, 3.5, -1.5], [0.0, 0.0, -1.0]),
        Some([-4, 3, -4])
    );
    journal
        .set_fluid([-4, 3, -4], crate::water::FluidCell::default())
        .unwrap();
    assert_eq!(
        water_target(&chunks, &journal, [-3.5, 3.5, -1.5], [0.0, 0.0, -1.0]),
        None
    );
    chunks
        .loaded
        .get_mut(&owner)
        .unwrap()
        .data
        .set_block([12, 3, 12], 27)
        .unwrap();
    journal
        .set_fluid(
            [-4, 3, -4],
            crate::water::FluidCell {
                amount: 8,
                flow: [0; 2],
                source: true,
            },
        )
        .unwrap();
    assert_eq!(
        water_target(&chunks, &journal, [-3.5, 3.5, -1.5], [0.0, 0.0, -1.0]),
        Some([-4, 3, -4])
    );
    chunks
        .loaded
        .get_mut(&owner)
        .unwrap()
        .data
        .set_block([12, 3, 14], 3)
        .unwrap();
    assert_eq!(
        water_target(&chunks, &journal, [-3.5, 3.5, -1.5], [0.0, 0.0, -1.0]),
        None
    );
    assert_eq!(
        water_target(&chunks, &journal, [-3.5, 3.5, 1.5], [0.0, 0.0, -1.0]),
        None
    );
    assert_eq!(
        water_target(&chunks, &journal, [f64::NAN; 3], [0.0, 0.0, -1.0]),
        None
    );
}

#[test]
fn natural_water_columns_are_valid_fill_targets_without_recorded_source_blocks() {
    use crate::{SectionGenerator, streaming::LoadedSection};
    use std::sync::Arc;
    use terra_voxel::BlockSection;
    let terrain = crate::TerraGenerator::new(terra_core::WorldConfig {
        base_height: 3.,
        sea_level: 8.,
        hill_amplitude: 0.,
        mountain_amplitude: 0.,
        biomes: false,
        caves: false,
        rivers: false,
        trees: false,
        landscape_revision: 0,
        structures_revision: 0,
        ..Default::default()
    })
    .unwrap();
    let generator = SectionGenerator::new(terrain);
    let column = Arc::new(generator.prepare_column([0, 0], true).unwrap());
    let owner = SectionPos::default();
    let mut chunks = ChunkStore::default();
    chunks.loaded.insert(
        owner,
        LoadedSection {
            data: BlockSection::empty(owner).unwrap(),
            mesh: None,
            mesh_epoch: 0,
            water_epoch: 0,
            water: None,
            props: vec![],
        },
    );
    chunks.set_prepared_column_for_test([0, 0], Some(column));
    let mut journal = EditJournal::default();
    let origin = [6.5, 10.5, 4.5];
    let direction = [0., -1., 0.];
    assert_eq!(
        water_target(&chunks, &journal, origin, direction),
        Some([6, 7, 4])
    );
    assert!(journal.fluid([6, 7, 4]).is_none());
    journal.record([6, 7, 4], 0).unwrap();
    journal
        .set_fluid([6, 7, 4], crate::water::FluidCell::default())
        .unwrap();
    assert_eq!(
        water_target(&chunks, &journal, origin, direction),
        Some([6, 6, 4]),
        "explicitly emptied top source cannot be silently reselected"
    );
    chunks
        .loaded
        .get_mut(&owner)
        .unwrap()
        .data
        .set_block([6, 8, 4], 3)
        .unwrap();
    assert_eq!(water_target(&chunks, &journal, origin, direction), None);
    chunks.loaded.clear();
    assert_eq!(water_target(&chunks, &journal, origin, direction), None);
}
