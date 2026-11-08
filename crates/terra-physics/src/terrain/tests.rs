use super::*;
use bevy::time::TimeUpdateStrategy;
use std::time::{Duration, Instant};
use terra_core::{GameConfig, TerraCorePlugin};
use terra_world::streaming::LoadedSection;

#[test]
fn fluid_source_blocks_never_create_solid_voxel_colliders() {
    let data = BlockSection::from_blocks(
        SectionPos::default(),
        0,
        vec![terra_voxel::MaterialId::FluidWater as u16; terra_voxel::blocks::SECTION_VOLUME],
    )
    .unwrap();
    assert!(build_collider(&data).unwrap().is_none());
}

#[test]
fn mineral_collider_profiles_notice_remote_cuts_and_late_support_sections() {
    let owner = SectionPos::default();
    let mut chunks = ChunkStore::default();
    let mut journal = EditJournal::default();
    chunks.loaded.insert(
        owner,
        LoadedSection {
            data: BlockSection::empty(owner).unwrap(),
            mesh: None,
            mesh_epoch: 0,
            water_epoch: 0,
            water: None,
            props: Vec::new(),
        },
    );
    let initial = profile_snapshot(owner, &chunks, &journal);
    assert!(profile_current(Some(initial), owner, &chunks, &journal));
    journal.record([2, 35, 2], 0).unwrap();
    assert_eq!(journal.mesh_epoch(owner), 0);
    assert!(!profile_current(Some(initial), owner, &chunks, &journal));
    assert!(
        profile_current(None, owner, &chunks, &journal),
        "ordinary blocks do not acquire a vertical invalidation dependency"
    );
    let updated = profile_snapshot(owner, &chunks, &journal);
    let upper = SectionPos::new(0, 1, 0);
    chunks.loaded.insert(
        upper,
        LoadedSection {
            data: BlockSection::empty(upper).unwrap(),
            mesh: None,
            mesh_epoch: 0,
            water_epoch: 0,
            water: None,
            props: Vec::new(),
        },
    );
    assert!(!profile_current(Some(updated), owner, &chunks, &journal));
}

fn app() -> App {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, TransformPlugin))
        .add_plugins(TerraCorePlugin(GameConfig::default()))
        .add_plugins(crate::TerraPhysicsPlugin)
        .insert_resource(ChunkFocus([2.5, 3.0, 2.5]))
        .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f64(
            1.0 / 60.0,
        )));
    let mut data = BlockSection::empty(SectionPos::default()).unwrap();
    data.set_block([2, 0, 2], 3).unwrap();
    app.world_mut().resource_mut::<ChunkStore>().loaded.insert(
        SectionPos::default(),
        LoadedSection {
            data,
            mesh: None,
            mesh_epoch: 0,
            water_epoch: 0,
            water: None,
            props: Vec::new(),
        },
    );
    app.finish();
    app.cleanup();
    app
}

fn settle_colliders(app: &mut App, expected_epoch: u64) {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        app.update();
        if app
            .world()
            .resource::<TerrainCollisionState>()
            .sections
            .get(&SectionPos::default())
            .is_some_and(|section| section.epoch == expected_epoch)
        {
            break;
        }
        assert!(Instant::now() < deadline, "collider generation timed out");
        std::thread::sleep(Duration::from_millis(1));
    }
}

#[test]
fn confirmed_edits_refresh_physical_collision_while_render_remesh_stays_pending() {
    let mut app = app();
    let owner = SectionPos::default();
    settle_colliders(&mut app, 0);
    let mut source = EditJournal::default();
    source.record([2, 1, 2], 3).unwrap();
    app.world_mut()
        .resource_scope(|world, mut journal: Mut<EditJournal>| {
            world
                .resource_mut::<ChunkStore>()
                .apply_confirmed_delta(&mut journal, source.section_delta(owner))
                .unwrap();
        });
    {
        let world = app.world();
        assert_eq!(world.resource::<ChunkStore>().loaded[&owner].mesh_epoch, 0);
        assert!(
            !world.resource::<TerrainCollisionState>().ready_for_aabb(
                Vec3::new(2.1, 1.1, 2.1),
                Vec3::new(2.9, 1.9, 2.9),
                world.resource::<ChunkStore>(),
                world.resource::<EditJournal>(),
            ),
            "the previous collider cannot approve movement through the new block"
        );
    }
    settle_colliders(&mut app, 1);
    assert_eq!(
        app.world().resource::<ChunkStore>().loaded[&owner].mesh_epoch,
        0
    );
    let body = app
        .world_mut()
        .spawn((
            RigidBody::Dynamic,
            Collider::sphere(0.25),
            Transform::from_xyz(2.5, 4.0, 2.5),
        ))
        .id();
    for _ in 0..120 {
        app.update();
    }
    let height = app.world().get::<Position>(body).unwrap().y;
    assert!(
        (height - 2.25).abs() < 0.06,
        "confirmed upper block height={height}"
    );
    source.record([2, 1, 2], 0).unwrap();
    app.world_mut()
        .resource_scope(|world, mut journal: Mut<EditJournal>| {
            world
                .resource_mut::<ChunkStore>()
                .apply_confirmed_delta(&mut journal, source.section_delta(owner))
                .unwrap();
        });
    settle_colliders(&mut app, 2);
    for _ in 0..120 {
        app.update();
    }
    let height = app.world().get::<Position>(body).unwrap().y;
    assert!(
        (height - 1.25).abs() < 0.06,
        "confirmed removal height={height}"
    );
    assert_eq!(
        app.world().resource::<ChunkStore>().loaded[&owner].mesh_epoch,
        0
    );
}

#[test]
fn local_journal_edits_supply_collision_without_waiting_for_a_loaded_payload_replacement() {
    let mut app = app();
    settle_colliders(&mut app, 0);
    app.world_mut()
        .resource_mut::<EditJournal>()
        .record([2, 1, 2], 3)
        .unwrap();
    assert_eq!(
        app.world().resource::<ChunkStore>().block_at([2, 1, 2]),
        Some(0)
    );
    let snapshot = collision_snapshot(
        SectionPos::default(),
        app.world().resource::<ChunkStore>(),
        app.world().resource::<EditJournal>(),
    )
    .unwrap();
    assert_eq!(snapshot.block([2, 1, 2]), Ok(3));
    settle_colliders(&mut app, 1);
    let world = app.world();
    assert!(world.resource::<TerrainCollisionState>().ready_for_aabb(
        Vec3::new(2.1, 1.1, 2.1),
        Vec3::new(2.9, 1.9, 2.9),
        world.resource::<ChunkStore>(),
        world.resource::<EditJournal>(),
    ));
    assert_eq!(
        world.resource::<ChunkStore>().loaded[&SectionPos::default()].mesh_epoch,
        0
    );
}

#[test]
fn forgotten_neighbor_and_vertical_profile_bases_block_collision_publication() {
    let owner = SectionPos::default();
    for offset in [[-1, 0, 0], [0, 2, 0], [1, -2, 1]] {
        let neighbor = owner.offset(offset).unwrap();
        let mut chunks = ChunkStore::default();
        for position in [owner, neighbor] {
            chunks.loaded.insert(
                position,
                LoadedSection {
                    data: BlockSection::empty(position).unwrap(),
                    mesh: None,
                    mesh_epoch: 0,
                    water_epoch: 0,
                    water: None,
                    props: Vec::new(),
                },
            );
        }
        let mut source = EditJournal::default();
        let mut replica = EditJournal::default();
        let position = neighbor.global_block([15, 1, 2]).unwrap();
        source.record(position, 3).unwrap();
        chunks
            .apply_confirmed_delta(&mut replica, source.section_delta(neighbor))
            .unwrap();
        assert!(collision_inputs_known(owner, &chunks));
        let mut forgotten = source.section_delta(neighbor);
        forgotten.revision += 1;
        forgotten.blocks.clear();
        chunks
            .apply_confirmed_delta(&mut replica, forgotten)
            .unwrap();
        assert!(chunks.voxel_data_known(owner));
        assert!(chunks.voxel_base_pending(neighbor));
        assert!(
            !collision_inputs_known(owner, &chunks),
            "unknown halo {offset:?}"
        );
        assert!(collision_snapshot(owner, &chunks, &replica).is_err());
        let mut restored = replica.section_delta(neighbor);
        restored.revision += 1;
        restored.blocks.push(([15, 1, 2], 0));
        chunks
            .apply_confirmed_delta(&mut replica, restored)
            .unwrap();
        assert!(collision_inputs_known(owner, &chunks));
        assert!(collision_snapshot(owner, &chunks, &replica).is_ok());
    }
}

#[test]
fn collider_worker_discards_a_replaced_voxel_payload_even_at_the_same_mesh_epoch() {
    let mut app = app();
    let owner = SectionPos::default();
    let previous = Arc::new(
        app.world().resource::<ChunkStore>().loaded[&owner]
            .data
            .clone(),
    );
    let worker_data = previous.clone();
    app.world_mut().resource_mut::<ColliderJobs>().0.insert(
        owner,
        ColliderJob {
            data: previous,
            plain: None,
            profile: None,
            epoch: 0,
            task: AsyncComputeTaskPool::get().spawn(async move { build_collider(&worker_data) }),
        },
    );
    app.world_mut()
        .resource_mut::<ChunkStore>()
        .loaded
        .get_mut(&owner)
        .unwrap()
        .data = BlockSection::empty(owner).unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        app.update();
        let state = app.world().resource::<TerrainCollisionState>();
        if state.completed_jobs > 0 && state.pending_jobs == 0 {
            break;
        }
        assert!(Instant::now() < deadline, "replacement collider timed out");
        std::thread::sleep(Duration::from_millis(1));
    }
    let state = app.world().resource::<TerrainCollisionState>();
    assert_eq!(
        state.completed_jobs, 1,
        "the old payload was discarded, not activated"
    );
    assert_eq!(state.active_colliders, 0);
    assert!(state.sections[&owner].entity.is_none());
}

#[test]
fn logical_voxel_colliders_stop_falls_and_refresh_after_mining_without_renderer() {
    let mut app = app();
    settle_colliders(&mut app, 0);
    let body = app
        .world_mut()
        .spawn((
            RigidBody::Dynamic,
            Collider::sphere(0.25),
            Transform::from_xyz(2.5, 3.0, 2.5),
        ))
        .id();
    for _ in 0..120 {
        app.update();
    }
    let height = app.world().get::<Position>(body).unwrap().y;
    assert!((height - 1.25).abs() < 0.06, "height={height}");
    app.world_mut()
        .resource_mut::<EditJournal>()
        .record([2, 0, 2], 0)
        .unwrap();
    {
        let mut chunks = app.world_mut().resource_mut::<ChunkStore>();
        let section = chunks.loaded.get_mut(&SectionPos::default()).unwrap();
        section.data.set_block([2, 0, 2], 0).unwrap();
        section.mesh_epoch = 1;
    }
    settle_colliders(&mut app, 1);
    for _ in 0..60 {
        app.update();
    }
    assert!(app.world().get::<Position>(body).unwrap().y < 0.0);
    assert_eq!(
        app.world()
            .resource::<TerrainCollisionState>()
            .active_colliders,
        0
    );
}

#[test]
fn eviction_removes_colliders_and_old_jobs_cannot_resurrect_edited_data() {
    let mut app = app();
    settle_colliders(&mut app, 0);
    let owner = SectionPos::default();
    let data = app.world().resource::<ChunkStore>().loaded[&owner]
        .data
        .clone();
    app.world_mut().resource_mut::<ColliderJobs>().0.insert(
        owner,
        ColliderJob {
            data: Arc::new(data.clone()),
            plain: None,
            profile: None,
            epoch: 0,
            task: AsyncComputeTaskPool::get().spawn(async move { build_collider(&data) }),
        },
    );
    app.world_mut()
        .resource_mut::<EditJournal>()
        .record([2, 0, 2], 0)
        .unwrap();
    app.world_mut().resource_mut::<ChunkStore>().loaded.clear();
    for _ in 0..5 {
        app.update();
    }
    assert_eq!(
        app.world()
            .resource::<TerrainCollisionState>()
            .active_colliders,
        0
    );
    assert!(app.world().resource::<ColliderJobs>().0.is_empty());
    assert_eq!(
        app.world_mut()
            .query::<&TerrainCollider>()
            .iter(app.world())
            .count(),
        0
    );
}

#[test]
fn collision_readiness_requires_all_intersected_sections_and_current_epochs() {
    let mut app = app();
    settle_colliders(&mut app, 0);
    let world = app.world();
    let state = world.resource::<TerrainCollisionState>();
    let chunks = world.resource::<ChunkStore>();
    let journal = world.resource::<EditJournal>();
    assert!(state.ready_for_aabb(Vec3::splat(1.0), Vec3::splat(3.0), chunks, journal));
    assert!(!state.ready_for_aabb(Vec3::splat(-0.1), Vec3::splat(3.0), chunks, journal));
}

#[test]
fn glass_and_fixed_water_stop_falling_bodies_like_opaque_building_blocks() {
    for material in [
        terra_voxel::MaterialId::Glass,
        terra_voxel::MaterialId::SolidWater,
    ] {
        let mut app = app();
        app.world_mut()
            .resource_mut::<ChunkStore>()
            .loaded
            .get_mut(&SectionPos::default())
            .unwrap()
            .data
            .set_block([2, 0, 2], material as u16)
            .unwrap();
        settle_colliders(&mut app, 0);
        let body = app
            .world_mut()
            .spawn((
                RigidBody::Dynamic,
                Collider::sphere(0.25),
                Transform::from_xyz(2.5, 3.0, 2.5),
            ))
            .id();
        for _ in 0..120 {
            app.update();
        }
        let height = app.world().get::<Position>(body).unwrap().y;
        assert!((height - 1.25).abs() < 0.06, "{material:?}: {height}");
    }
}

#[test]
fn cube_collision_survives_neighbour_edits_but_not_local_or_model_changes() {
    let mut app = app();
    settle_colliders(&mut app, 0);
    let owner = SectionPos::default();
    app.world_mut()
        .resource_mut::<EditJournal>()
        .record([-1, 2, 2], 3)
        .unwrap();
    {
        let world = app.world();
        assert!(world.resource::<TerrainCollisionState>().ready_for_aabb(
            Vec3::ONE,
            Vec3::splat(3.0),
            world.resource::<ChunkStore>(),
            world.resource::<EditJournal>()
        ));
    }
    app.update();
    assert_eq!(
        app.world()
            .resource::<TerrainCollisionState>()
            .completed_jobs,
        1
    );
    app.world_mut()
        .resource_mut::<EditJournal>()
        .record([-1, 2, 2], terra_core::crusher::MATERIAL)
        .unwrap();
    let world = app.world();
    assert!(!plain_current(
        &world.resource::<TerrainCollisionState>().sections[&owner],
        owner,
        world.resource::<ChunkStore>(),
        world.resource::<EditJournal>()
    ));
    app.world_mut()
        .resource_mut::<EditJournal>()
        .record([2, 0, 2], 0)
        .unwrap();
    let world = app.world();
    assert!(!world.resource::<TerrainCollisionState>().ready_for_aabb(
        Vec3::ONE,
        Vec3::splat(3.0),
        world.resource::<ChunkStore>(),
        world.resource::<EditJournal>()
    ));
}

#[test]
fn rapid_remote_cube_placements_keep_local_movement_ready_until_rebuild() {
    let mut app = app();
    settle_colliders(&mut app, 0);
    // Stay in the same section and deliberately do not run the collider jobs.
    for x in 6..14 {
        app.world_mut()
            .resource_mut::<EditJournal>()
            .record([x, 2, 2], 3)
            .unwrap();
        app.world_mut()
            .resource_mut::<ChunkStore>()
            .loaded
            .get_mut(&SectionPos::default())
            .unwrap()
            .data
            .set_block([x as usize, 2, 2], 3)
            .unwrap();
        let world = app.world();
        let state = world.resource::<TerrainCollisionState>();
        let chunks = world.resource::<ChunkStore>();
        let journal = world.resource::<EditJournal>();
        assert!(state.ready_for_aabb(Vec3::ONE, Vec3::splat(3.0), chunks, journal));
        assert!(!state.ready_for_aabb(
            Vec3::new(x as f32 - 0.5, 1.0, 1.0),
            Vec3::new(x as f32 + 1.5, 3.0, 3.0),
            chunks,
            journal
        ));
    }
    // A confirmed model outside the body still invalidates this conservative path.
    app.world_mut()
        .resource_mut::<EditJournal>()
        .record([12, 2, 2], terra_core::crusher::MATERIAL)
        .unwrap();
    let world = app.world();
    assert!(!world.resource::<TerrainCollisionState>().ready_for_aabb(
        Vec3::ONE,
        Vec3::splat(3.0),
        world.resource::<ChunkStore>(),
        world.resource::<EditJournal>()
    ));
}
