use super::*;
use terra_voxel::BlockSection;
use terra_world::streaming::LoadedSection;

#[derive(Resource)]
struct Fixture {
    definitions: BTreeMap<u16, BlockLight>,
    focus: Vec3,
    reach: f32,
    tick: bool,
}

fn update(
    mut cache: ResMut<Lamps>,
    chunks: Res<ChunkStore>,
    journal: Res<EditJournal>,
    messages: Messages,
    fixture: Res<Fixture>,
) {
    assert!(messages.available());
    let urgent = collect(&mut cache, &chunks, &messages);
    if fixture.tick || urgent {
        refresh(
            &mut cache,
            &chunks,
            &journal,
            &fixture.definitions,
            fixture.focus,
            fixture.reach,
        );
    }
}

fn app() -> App {
    let mut app = App::new();
    app.init_resource::<Lamps>()
        .init_resource::<ChunkStore>()
        .init_resource::<EditJournal>()
        .add_message::<SectionPublished>()
        .add_message::<SectionRemoved>()
        .add_message::<SectionEditApplied>()
        .insert_resource(Fixture {
            definitions: crate::blocks::definitions::BlockLibrary::load(
                &terra_core::asset_directory().join("terra/blocks"),
            )
            .unwrap()
            .blocks
            .into_iter()
            .filter_map(|block| block.light.map(|light| (block.id, light)))
            .collect(),
            focus: Vec3::ZERO,
            reach: 192.0,
            tick: true,
        })
        .add_systems(Update, update);
    app
}

fn section(world: &mut World, position: SectionPos, blocks: &[([usize; 3], u16)]) {
    let mut data = BlockSection::empty(position).unwrap();
    for &(local, material) in blocks {
        data.set_block(local, material).unwrap();
    }
    world.resource_mut::<ChunkStore>().loaded.insert(
        position,
        LoadedSection {
            data,
            mesh: None,
            mesh_epoch: 0,
            water_epoch: 0,
            water: None,
            props: Vec::new(),
        },
    );
}

fn positions(cache: &Lamps) -> Vec<[i64; 3]> {
    let mut positions: Vec<_> = cache
        .emitters
        .iter()
        .map(|(position, _)| *position)
        .collect();
    positions.sort_unstable();
    positions
}

fn assert_reference(world: &World) {
    let fixture = world.resource::<Fixture>();
    let mut reference = Lamps::default();
    loop {
        discover(
            &mut reference,
            world.resource::<ChunkStore>(),
            world.resource::<EditJournal>(),
            &fixture.definitions,
            fixture.focus,
            fixture.reach,
        );
        if !reference.dirty {
            break;
        }
    }
    let actual = world.resource::<Lamps>();
    assert_eq!(positions(actual), positions(&reference));
    for (position, light) in &actual.emitters {
        assert!(
            reference
                .emitters
                .iter()
                .any(|(other, expected)| other == position && same_light(light, expected))
        );
    }
}

#[test]
fn metadata_only_replica_updates_never_seed_or_rediscover_the_loaded_world() {
    let mut app = app();
    section(
        app.world_mut(),
        SectionPos::new(-1, 0, -1),
        &[([15, 3, 15], 40)],
    );
    app.update();
    assert_reference(app.world());
    let initial = {
        let cache = app.world().resource::<Lamps>();
        (cache.updates.seeds, cache.updates.source_scans)
    };
    for _ in 0..128 {
        app.world_mut()
            .resource_mut::<EditJournal>()
            .gameplay_mut()
            .revision += 1;
        app.world_mut()
            .resource_mut::<ChunkStore>()
            .loaded
            .values_mut()
            .next()
            .unwrap()
            .water_epoch += 1;
        app.update();
    }
    let cache = app.world().resource::<Lamps>();
    assert_eq!((cache.updates.seeds, cache.updates.source_scans), initial);
    assert!(cache.updates.pending.is_empty());
    assert_eq!(positions(cache), [[-1, 3, -1]]);
}

#[test]
fn message_cursors_drain_between_timer_ticks_and_raw_publications_replace_same_epoch_data() {
    let mut app = app();
    app.update();
    app.world_mut().resource_mut::<Fixture>().tick = false;
    let owner = SectionPos::new(-1, 0, -1);
    section(app.world_mut(), owner, &[([15, 3, 15], 40)]);
    app.world_mut()
        .write_message(SectionPublished { position: owner });
    for _ in 0..5 {
        app.update();
    }
    assert!(
        app.world()
            .resource::<Lamps>()
            .updates
            .pending
            .contains_key(&owner)
    );
    assert!(app.world().resource::<Lamps>().emitters.is_empty());
    app.world_mut().resource_mut::<Fixture>().tick = true;
    app.update();
    assert_reference(app.world());
    assert_eq!(positions(app.world().resource::<Lamps>()), [[-1, 3, -1]]);

    // A generation replacement can have the same raw revision, not the same cells.
    section(app.world_mut(), owner, &[([14, 3, 15], 40)]);
    app.world_mut()
        .write_message(SectionPublished { position: owner });
    app.update();
    assert_reference(app.world());
    assert_eq!(positions(app.world().resource::<Lamps>()), [[-2, 3, -1]]);
}

#[test]
fn negative_boundary_neighbor_load_edit_and_unload_refresh_exposure_before_async_meshes() {
    let mut app = app();
    let owner = SectionPos::new(-1, 0, -1);
    section(
        app.world_mut(),
        owner,
        &[
            ([15, 3, 14], 99),
            ([14, 3, 14], 3),
            ([15, 2, 14], 3),
            ([15, 4, 14], 3),
            ([15, 3, 13], 3),
            ([15, 3, 15], 3),
        ],
    );
    app.update();
    assert!(app.world().resource::<Lamps>().emitters.is_empty());
    let neighbor = SectionPos::new(0, 0, -1);
    section(app.world_mut(), neighbor, &[]);
    app.world_mut()
        .write_message(SectionPublished { position: neighbor });
    app.update();
    assert_reference(app.world());
    assert_eq!(positions(app.world().resource::<Lamps>()), [[-1, 3, -2]]);
    let own_epoch = app.world().resource::<Lamps>().sections[&owner].epoch;
    let own_mesh_epoch = app.world().resource::<EditJournal>().mesh_epoch(owner);
    let own_raw_revision = app.world().resource::<ChunkStore>().loaded[&owner]
        .data
        .revision();

    app.world_mut().resource_mut::<Fixture>().tick = false;
    app.world_mut()
        .resource_mut::<EditJournal>()
        .record([0, 3, -2], 3)
        .unwrap();
    app.world_mut()
        .write_message(SectionEditApplied { position: neighbor });
    app.update();
    assert_reference(app.world());
    assert!(app.world().resource::<Lamps>().emitters.is_empty());
    assert_eq!(
        app.world().resource::<Lamps>().sections[&owner].epoch,
        own_epoch + 1
    );
    assert_eq!(
        app.world().resource::<EditJournal>().mesh_epoch(owner),
        own_mesh_epoch + 1
    );
    assert_eq!(
        app.world().resource::<ChunkStore>().loaded[&owner]
            .data
            .revision(),
        own_raw_revision
    );
    assert_eq!(
        app.world().resource::<ChunkStore>().block_at([0, 3, -2]),
        Some(0)
    );
    app.world_mut()
        .resource_mut::<EditJournal>()
        .record([0, 3, -2], 0)
        .unwrap();
    app.world_mut()
        .write_message(SectionEditApplied { position: neighbor });
    app.update();
    assert_reference(app.world());
    assert_eq!(positions(app.world().resource::<Lamps>()), [[-1, 3, -2]]);

    app.world_mut()
        .resource_mut::<ChunkStore>()
        .loaded
        .remove(&neighbor);
    app.world_mut()
        .write_message(SectionRemoved { position: neighbor });
    app.update();
    assert_reference(app.world());
    assert!(app.world().resource::<Lamps>().emitters.is_empty());
    app.world_mut()
        .resource_mut::<ChunkStore>()
        .loaded
        .remove(&owner);
    app.world_mut()
        .write_message(SectionRemoved { position: owner });
    app.update();
    assert!(app.world().resource::<Lamps>().sections.is_empty());
}

#[test]
fn scarecrow_prop_edits_and_definition_changes_update_the_existing_source_cache() {
    use terra_world::props::{PropId, PropKind, WorldProp};
    let mut app = app();
    let owner = SectionPos::new(-1, 0, -1);
    section(app.world_mut(), owner, &[([2, 3, 4], 40)]);
    app.update();
    let prop = WorldProp {
        id: PropId(17),
        kind: PropKind::Scarecrow,
        position: [-11.5, 3.0, -11.5],
        rotation: 0.0,
        scale: 1.0,
        variant: 0,
        section: owner,
        removed: false,
    };
    assert!(
        app.world_mut()
            .resource_mut::<EditJournal>()
            .place_prop(prop)
            .unwrap()
    );
    app.world_mut()
        .write_message(SectionEditApplied { position: owner });
    app.update();
    assert_reference(app.world());
    assert_eq!(app.world().resource::<Lamps>().emitters.len(), 2);
    app.world_mut()
        .resource_mut::<EditJournal>()
        .remove_prop(owner, prop.id)
        .unwrap();
    app.world_mut()
        .write_message(SectionEditApplied { position: owner });
    app.update();
    assert_reference(app.world());
    assert_eq!(app.world().resource::<Lamps>().emitters.len(), 1);
    app.world_mut()
        .resource_mut::<Fixture>()
        .definitions
        .get_mut(&40)
        .unwrap()
        .intensity += 17.0;
    app.update();
    assert_reference(app.world());
    let expected = app.world().resource::<Fixture>().definitions[&40].intensity;
    assert_eq!(
        app.world().resource::<Lamps>().emitters[0].1.intensity,
        expected
    );
}

#[test]
fn bounded_discovery_focus_reach_and_reset_do_not_lose_deferred_or_distant_sources() {
    let mut app = app();
    app.world_mut().resource_mut::<Fixture>().reach = 2_000.0;
    for x in -64..=64 {
        section(
            app.world_mut(),
            SectionPos::new(x, 0, 0),
            if x == 0 { &[([2, 3, 4], 40)] } else { &[] },
        );
    }
    app.update();
    assert_eq!(
        app.world().resource::<Lamps>().sections.len(),
        DISCOVERY_BUDGET
    );
    assert_eq!(app.world().resource::<Lamps>().updates.pending.len(), 1);
    app.update();
    assert_reference(app.world());
    assert_eq!(app.world().resource::<Lamps>().sections.len(), 129);

    let distant = SectionPos::new(200, 0, 0);
    section(app.world_mut(), distant, &[([2, 3, 4], 40)]);
    app.world_mut().resource_mut::<Fixture>().reach = 16.0;
    app.world_mut()
        .write_message(SectionPublished { position: distant });
    app.update();
    assert!(
        !app.world()
            .resource::<Lamps>()
            .sections
            .contains_key(&distant)
    );
    app.world_mut().resource_mut::<Fixture>().focus = Vec3::new(3200.0, 0.0, 0.0);
    app.update();
    assert!(positions(app.world().resource::<Lamps>()).contains(&[3202, 3, 4]));

    // This is the same cache reset performed on Replica readiness/identity changes.
    {
        let mut cache = app.world_mut().resource_mut::<Lamps>();
        clear(&mut cache);
    }
    app.world_mut().insert_resource(ChunkStore::default());
    app.world_mut().insert_resource(EditJournal::default());
    app.update();
    assert!(app.world().resource::<Lamps>().sections.is_empty());
    assert!(app.world().resource::<Lamps>().emitters.is_empty());
}

#[test]
fn pending_positions_are_hard_bounded_and_missing_messages_disable_the_fast_path() {
    let mut pending = BTreeMap::new();
    let mut overflow = false;
    for x in 0..=MAX_PENDING {
        queue(
            &mut pending,
            &mut overflow,
            SectionPos::new(x as i64, 0, 0),
            false,
        );
    }
    assert_eq!(pending.len(), MAX_PENDING);
    assert!(overflow);
    queue(&mut pending, &mut overflow, SectionPos::default(), true);
    assert_eq!(pending.len(), MAX_PENDING);
    assert!(pending[&SectionPos::default()]);

    let mut world = World::new();
    world.init_resource::<bevy::ecs::message::Messages<SectionPublished>>();
    world.init_resource::<bevy::ecs::message::Messages<SectionRemoved>>();
    let mut state = bevy::ecs::system::SystemState::<Messages>::new(&mut world);
    assert!(!state.get(&world).unwrap().available());
}

#[test]
fn a_remove_and_reload_in_one_frame_preserves_the_current_section_generation() {
    let mut app = app();
    let owner = SectionPos::new(-1, 0, -1);
    section(app.world_mut(), owner, &[([2, 3, 4], 40)]);
    app.update();
    section(app.world_mut(), owner, &[([3, 3, 4], 40)]);
    app.world_mut()
        .write_message(SectionRemoved { position: owner });
    app.world_mut()
        .write_message(SectionPublished { position: owner });
    app.update();
    assert_reference(app.world());
    assert_eq!(positions(app.world().resource::<Lamps>()), [[-13, 3, -12]]);
}

#[test]
fn actual_selection_clears_lamp_entities_and_pending_work_when_the_replica_resets() {
    let mut app = App::new();
    app.init_resource::<Time>()
        .insert_resource(WorldAccess::Replica { ready: false })
        .init_resource::<ChunkStore>()
        .init_resource::<EditJournal>()
        .init_resource::<BlockAssets>()
        .init_resource::<Lamps>()
        .init_resource::<TorchLightingStatus>()
        .add_message::<SectionPublished>()
        .add_message::<SectionRemoved>()
        .add_message::<SectionEditApplied>()
        .add_systems(Update, super::super::select);
    let light = BlockLight {
        color: [1.0; 3],
        intensity: 30.0,
        range: 4.0,
        falloff: Default::default(),
        flicker: 0.0,
        cave_intensity: 1.0,
        shadows: false,
        offset: None,
    };
    let old = app
        .world_mut()
        .spawn((
            VoxelLamp([-1, 2, -1], light, Vec3::ZERO),
            Transform::default(),
            PointLight::default(),
        ))
        .id();
    app.world_mut()
        .resource_mut::<Lamps>()
        .emitters
        .push(([-1, 2, -1], light));
    app.world_mut()
        .resource_mut::<Lamps>()
        .updates
        .pending
        .insert(SectionPos::new(-1, 0, -1), true);
    app.update();
    assert!(app.world().get_entity(old).is_err());
    let cache = app.world().resource::<Lamps>();
    assert!(
        cache.sections.is_empty() && cache.emitters.is_empty() && cache.updates.pending.is_empty()
    );
    assert!(!cache.updates.initialized);
}
