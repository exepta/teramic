use super::*;
use bevy::time::TimeUpdateStrategy;
use std::time::Duration;
use terra_core::{
    PlayerUuid, WoodSpecies, train_parts,
    trains::{Vehicle, VehicleKind},
};

fn vehicle() -> Vehicle {
    let parts = [
        ([0, 0, 0], train_parts::CHASSIS),
        ([0, -1, 0], train_parts::BOGIE),
        ([0, -1, 1], train_parts::BOGIE),
        ([0, 1, 1], train_parts::FRAME),
        ([0, 1, 0], train_parts::SEAT),
        ([0, 0, -1], train_parts::COUPLER),
    ]
    .into_iter()
    .map(|(offset, material)| TrainPart {
        offset,
        material,
        facing: 0,
        wood: WoodSpecies::Spruce,
    })
    .collect();
    let vehicle = Vehicle {
        name: String::new(),
        id: 1,
        owner: PlayerUuid::from_u128(210_901),
        kind: VehicleKind::Passenger,
        anchor: [-16, 4, -16],
        parts,
        cargo: Default::default(),
        blocks: Default::default(),
        payloads: Default::default(),
        couplings: [None; 2],
        revision: 1,

        motion: Default::default(),
        burn_seconds: 0.0,
    };
    assert!(vehicle.valid());
    vehicle
}

fn app(view: VehicleView) -> App {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, TransformPlugin))
        .add_plugins(terra_core::TerraCorePlugin(Default::default()))
        .add_plugins(crate::TerraPhysicsPlugin)
        .insert_resource(WorldAccess::Replica { ready: true })
        .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f64(
            1.0 / 60.0,
        )));
    app.world_mut()
        .resource_mut::<TrainCollisionViews>()
        .set_views(&[view]);
    app.finish();
    app.cleanup();
    app
}

fn wait_active(app: &mut App, id: u64) {
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    while app
        .world()
        .resource::<TrainCollisionState>()
        .entries
        .get(&id)
        .is_none_or(|entry| !entry.active)
    {
        app.update();
        assert!(
            std::time::Instant::now() < deadline,
            "train collider {id} did not become active"
        );
        std::thread::sleep(Duration::from_millis(1));
    }
}

#[test]
fn geometry_cache_ignores_cargo_revision_owner_and_wood_but_guards_new_positions_until_physics() {
    let view = vehicle().view();
    let mut app = app(view.clone());
    wait_active(&mut app, view.id);
    let entity = app.world().resource::<TrainCollisionState>().entries[&view.id]
        .entity
        .unwrap();
    let mut metadata = view.clone();
    metadata.revision += 1;
    metadata.owner = PlayerUuid::from_u128(210_902);
    for part in &mut metadata.parts {
        part.wood = WoodSpecies::Cherry;
    }
    assert!(
        !app.world_mut()
            .resource_mut::<TrainCollisionViews>()
            .set_views(&[metadata.clone()])
    );
    app.update();
    assert_eq!(
        app.world().resource::<TrainCollisionState>().entries[&view.id].entity,
        Some(entity)
    );
    metadata.anchor[0] += 16;
    assert!(
        app.world_mut()
            .resource_mut::<TrainCollisionViews>()
            .set_views(&[metadata])
    );
    let world = app.world();
    let low = Vec3::new(-1.0, 3.0, -17.0);
    let high = Vec3::new(2.0, 7.0, -13.0);
    assert!(!world.resource::<TrainCollisionState>().ready(
        low,
        high,
        world.resource::<EditJournal>(),
        world.resource::<TrainCollisionViews>(),
        Some(WorldAccess::Replica { ready: true })
    ));
    app.update();
    assert!(
        !app.world().resource::<TrainCollisionState>().entries[&view.id].active,
        "a queued collider cannot become ready before fixed physics publication"
    );
    wait_active(&mut app, view.id);
    assert_ne!(
        app.world().resource::<TrainCollisionState>().entries[&view.id].entity,
        Some(entity)
    );
}

#[test]
fn assembled_parts_stop_real_falling_bodies_at_their_authored_surface() {
    let view = vehicle().view();
    let top = view
        .parts
        .iter()
        .flat_map(|part| {
            train_parts::boxes(part.material, part.facing)
                .into_iter()
                .map(move |bounds| (part, bounds))
        })
        .filter(|(part, bounds)| {
            [0, 2].into_iter().all(|axis| {
                f64::from(part.offset[axis]) + bounds.minimum[axis] <= 0.5
                    && f64::from(part.offset[axis]) + bounds.maximum[axis] >= 0.5
            })
        })
        .map(|(part, bounds)| {
            view.anchor[1] as f64
                + terra_core::trains::MODEL_Y_OFFSET
                + f64::from(part.offset[1])
                + bounds.maximum[1]
        })
        .max_by(f64::total_cmp)
        .unwrap() as f32;
    let mut app = app(view.clone());
    wait_active(&mut app, view.id);
    let body = app
        .world_mut()
        .spawn((
            RigidBody::Dynamic,
            Collider::sphere(0.2),
            Transform::from_xyz(
                view.anchor[0] as f32 + 0.5,
                top + 3.0,
                view.anchor[2] as f32 + 0.5,
            ),
        ))
        .id();
    for _ in 0..180 {
        app.update();
    }
    let actual = app.world().get::<Position>(body).unwrap().y;
    assert!(
        (actual - (top + 0.2)).abs() < 0.06,
        "actual={actual}, authored support={top}"
    );
    assert!(app.world().resource::<TrainCollisionState>().entries[&view.id].active);
}

#[test]
fn mounted_top_slab_supports_real_bodies_at_its_captured_surface() {
    let mut car = vehicle();
    let index = car.parts.len() as u16;
    car.parts.push(TrainPart {
        offset: [1, 0, 0],
        material: 82,
        facing: 4,
        wood: WoodSpecies::Birch,
    });
    car.blocks.insert(
        index,
        TrainBlockView {
            boxes: vec![terra_core::construction::slab_bounds(4)],
            ..default()
        },
    );
    assert!(car.valid());
    let top = car.model_origin()[1] as f32 + 1.0;
    let mut app = app(car.view());
    wait_active(&mut app, car.id);
    let body = app
        .world_mut()
        .spawn((
            RigidBody::Dynamic,
            Collider::sphere(0.15),
            Transform::from_xyz(
                car.anchor[0] as f32 + 1.5,
                top + 2.0,
                car.anchor[2] as f32 + 0.5,
            ),
        ))
        .id();
    for _ in 0..150 {
        app.update();
    }
    let actual = app.world().get::<Position>(body).unwrap().y;
    assert!(
        (actual - (top + 0.15)).abs() < 0.06,
        "body={actual}, captured top={top}"
    );
}

#[test]
fn captured_shape_changes_rebuild_collision_while_block_colors_do_not() {
    let mut car = vehicle();
    car.blocks.insert(
        0,
        TrainBlockView {
            boxes: train_parts::boxes(train_parts::CHASSIS, 0),
            ..default()
        },
    );
    let mut views = TrainCollisionViews::default();
    assert!(views.set_views(&[car.view()]));
    car.blocks.get_mut(&0).unwrap().tint = Some([21, 42, 63]);
    assert!(!views.set_views(&[car.view()]));
    car.blocks.get_mut(&0).unwrap().boxes[0].maximum[1] += 0.1;
    assert!(views.set_views(&[car.view()]));
}

#[test]
fn authoritative_spawn_clearance_rejects_intersection_with_saved_assembled_parts() {
    let mut journal = EditJournal::default();
    let vehicle = vehicle();
    journal
        .gameplay_mut()
        .trains
        .insert(vehicle.clone())
        .unwrap();
    let position =
        Vec3::from_array(vehicle.model_origin().map(|value| value as f32)) + Vec3::splat(0.5);
    assert!(!crate::character::space_clear_with_shapes(
        &Default::default(),
        &journal,
        &Default::default(),
        position
    ));
}

#[test]
fn assembled_bogie_wheels_contact_actual_steel_height_and_both_gauge_centers() {
    let steel_top = terra_core::rails::DECK_HEIGHT + terra_core::rails::RAIL_HEIGHT;
    for facing in 0..4 {
        let boxes = train_parts::boxes(train_parts::BOGIE, facing);
        let wheel_bottom = boxes
            .iter()
            .map(|bounds| bounds.minimum[1])
            .min_by(f64::total_cmp)
            .unwrap();
        assert!((1.0 + terra_core::trains::MODEL_Y_OFFSET + wheel_bottom - steel_top).abs() < 1e-8);
        let lateral = if facing % 2 == 0 { 0 } else { 2 };
        for center in [-0.5, 1.5] {
            assert!(
                boxes.iter().any(|bounds| {
                    (bounds.minimum[1] - wheel_bottom).abs() < 1e-8
                        && bounds.minimum[lateral] <= center
                        && bounds.maximum[lateral] >= center
                }),
                "wheel contact at gauge {center}, facing {facing}"
            );
        }
    }
}

fn loaded_cells(app: &mut App, view: &VehicleView, restore: bool) {
    use terra_voxel::{BlockSection, global_to_section};
    use terra_world::streaming::LoadedSection;
    app.world_mut()
        .resource_scope(|world, mut journal: Mut<EditJournal>| {
            let mut chunks = world.resource_mut::<terra_world::streaming::ChunkStore>();
            for (index, part) in view.parts.iter().enumerate() {
                let cell = view.part_cell(index).unwrap();
                let (owner, local) = global_to_section(cell);
                let section = chunks.loaded.entry(owner).or_insert_with(|| LoadedSection {
                    data: BlockSection::empty(owner).unwrap(),
                    mesh: None,
                    mesh_epoch: 0,
                    water_epoch: 0,
                    water: None,
                    props: Vec::new(),
                });
                if restore {
                    section.data.set_block(local, part.material).unwrap();
                    journal.record(cell, part.material).unwrap();
                }
            }
        });
    app.insert_resource(terra_world::streaming::ChunkFocus(
        view.anchor.map(|value| value as f64 + 0.5),
    ));
}

#[test]
fn dismantled_collision_waits_for_restored_parts_and_ready_terrain_before_retiring() {
    let view = vehicle().view();
    let mut app = app(view.clone());
    loaded_cells(&mut app, &view, false);
    wait_active(&mut app, view.id);
    let previous = app.world().resource::<TrainCollisionState>().entries[&view.id]
        .entity
        .unwrap();
    app.world_mut()
        .resource_mut::<TrainCollisionViews>()
        .set_views(&[]);
    app.update();
    let entry = &app.world().resource::<TrainCollisionState>().entries[&view.id];
    assert!(entry.retiring);
    assert_eq!(
        entry.entity,
        Some(previous),
        "a snapshot removal cannot turn an unreceived voxel restoration into air"
    );
    loaded_cells(&mut app, &view, true);
    app.update();
    assert!(
        app.world()
            .resource::<TrainCollisionState>()
            .entries
            .contains_key(&view.id),
        "confirmed voxel values alone do not replace their pending physical terrain collider"
    );
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    while app
        .world()
        .resource::<TrainCollisionState>()
        .entries
        .contains_key(&view.id)
    {
        app.update();
        assert!(
            std::time::Instant::now() < deadline,
            "restored terrain collision did not become ready"
        );
        std::thread::sleep(Duration::from_millis(1));
    }
    app.update();
    assert!(app.world().get_entity(previous).is_err());
    assert_eq!(
        app.world_mut()
            .query::<&TrainCollider>()
            .iter(app.world())
            .count(),
        0
    );
    let world = app.world();
    for index in 0..view.parts.len() {
        let cell = view.part_cell(index).unwrap();
        let center = Vec3::from_array(cell.map(|value| value as f32 + 0.5));
        assert!(
            world
                .resource::<crate::TerrainCollisionState>()
                .ready_for_aabb(
                    center - Vec3::splat(0.1),
                    center + Vec3::splat(0.1),
                    world.resource::<terra_world::streaming::ChunkStore>(),
                    world.resource::<EditJournal>(),
                )
        );
    }
}

#[test]
fn removed_vehicle_collision_does_not_leak_when_all_former_sections_are_unloaded() {
    let view = vehicle().view();
    let mut app = app(view.clone());
    loaded_cells(&mut app, &view, false);
    wait_active(&mut app, view.id);
    let previous = app.world().resource::<TrainCollisionState>().entries[&view.id]
        .entity
        .unwrap();
    app.world_mut()
        .resource_mut::<TrainCollisionViews>()
        .set_views(&[]);
    app.world_mut()
        .resource_mut::<terra_world::streaming::ChunkStore>()
        .loaded
        .clear();
    app.update();
    assert!(
        app.world()
            .resource::<TrainCollisionState>()
            .entries
            .is_empty()
    );
    assert!(app.world().get_entity(previous).is_err());
}

#[test]
fn new_replica_identity_cannot_reuse_an_old_collider_with_the_same_revision_and_vehicle_id() {
    let view = vehicle().view();
    let mut app = app(view.clone());
    wait_active(&mut app, view.id);
    let mut next = view.clone();
    next.anchor[0] += 32;
    let mut fresh = TrainCollisionViews::default();
    fresh.set_views(&[next.clone()]);
    assert_eq!(
        fresh.epoch,
        app.world().resource::<TrainCollisionViews>().epoch
    );
    app.insert_resource(fresh);
    app.update();
    assert_eq!(
        app.world().resource::<TrainCollisionState>().entries[&view.id].anchor,
        next.anchor
    );
    assert!(!app.world().resource::<TrainCollisionState>().entries[&view.id].active);
}

fn receive_views(app: &mut App, vehicles: &[VehicleView]) {
    app.world_mut()
        .resource_scope(|world, journal: Mut<EditJournal>| {
            world
                .resource_mut::<TrainCollisionViews>()
                .set_confirmed_views(vehicles, &journal)
                .unwrap();
        });
}

#[test]
fn moved_dismantling_waits_for_current_cells_even_when_final_pose_and_removal_arrive_together() {
    let original = vehicle().view();
    let mut app = app(original.clone());
    loaded_cells(&mut app, &original, false);
    wait_active(&mut app, original.id);
    let mut moved = original.clone();
    moved.motion.translation = [48.0, 0.0, 16.0];
    moved.motion.pivot = [0.5, 0.0, 0.5];
    moved.motion.yaw = std::f64::consts::FRAC_PI_2;
    let cells = moved.disassembly_cells().unwrap();
    let mut restored = moved.clone();
    restored.anchor = cells[0];
    restored.motion = Default::default();
    for (part, cell) in restored.parts.iter_mut().zip(&cells) {
        part.offset = std::array::from_fn(|axis| (cell[axis] - restored.anchor[axis]) as i16);
        part.facing = (part.facing + 1) % 4;
    }
    loaded_cells(&mut app, &restored, false);
    app.world_mut()
        .resource_mut::<TrainCollisionViews>()
        .set_motions(&[terra_core::train_motion::TrainMotionFrame {
            id: moved.id,
            revision: moved.revision,
            motion: moved.motion,
        }])
        .unwrap();
    receive_views(&mut app, &[]);
    app.update();
    let entry = &app.world().resource::<TrainCollisionState>().entries[&original.id];
    assert!(entry.retiring);
    assert_eq!(entry.restore_cells, cells);
    assert_eq!(entry.restore_turns, 1);
    assert_eq!(entry.pose, moved.model_transform());
    assert!(
        app.world()
            .get::<Position>(entry.entity.unwrap())
            .unwrap()
            .0
            .distance(Vec3::from_array(entry.pose.0.map(|value| value as f32)))
            < 1e-4
    );
    let mut source = EditJournal::default();
    let mut sections = BTreeSet::new();
    for (cell, part) in cells.iter().zip(&restored.parts) {
        source.record(*cell, part.material).unwrap();
        sections.insert(terra_voxel::global_to_section(*cell).0);
    }
    for section in sections {
        publish_current(&mut app, &source, section);
    }
    wait_retired(&mut app, original.id);
    assert!(original.parts.iter().enumerate().all(|(index, _)| {
        app.world()
            .resource::<ChunkStore>()
            .block_at(original.part_cell(index).unwrap())
            == Some(0)
    }));
}

fn publish_current(app: &mut App, source: &EditJournal, section: terra_voxel::SectionPos) {
    app.world_mut()
        .resource_scope(|world, mut journal: Mut<EditJournal>| {
            world
                .resource_mut::<ChunkStore>()
                .apply_confirmed_delta(&mut journal, source.section_delta(section))
                .unwrap();
        });
}
fn wait_retired(app: &mut App, id: u64) {
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    while app
        .world()
        .resource::<TrainCollisionState>()
        .entries
        .contains_key(&id)
    {
        app.update();
        assert!(
            std::time::Instant::now() < deadline,
            "causal train retirement timed out"
        );
        std::thread::sleep(Duration::from_millis(1));
    }
}

#[test]
fn coalesced_restoration_followed_by_mining_or_replacement_retires_without_old_materials() {
    for changed in [false, true] {
        let view = vehicle().view();
        let mut app = app(view.clone());
        loaded_cells(&mut app, &view, false);
        wait_active(&mut app, view.id);
        receive_views(&mut app, &[]);
        app.update();
        assert!(app.world().resource::<TrainCollisionState>().entries[&view.id].retiring);
        let mut source = EditJournal::default();
        let mut sections = BTreeSet::new();
        for (index, part) in view.parts.iter().enumerate() {
            let cell = view.part_cell(index).unwrap();
            sections.insert(terra_voxel::global_to_section(cell).0);
            source.record(cell, part.material).unwrap();
            source
                .record(cell, if changed && index == 0 { 3 } else { 0 })
                .unwrap();
        }
        let sections: Vec<_> = sections.into_iter().collect();
        assert!(sections.len() > 1);
        publish_current(&mut app, &source, sections[0]);
        app.update();
        assert!(
            app.world()
                .resource::<TrainCollisionState>()
                .entries
                .contains_key(&view.id),
            "every loaded owner needs its causal section checkpoint"
        );
        for section in sections.into_iter().skip(1) {
            publish_current(&mut app, &source, section);
        }
        wait_retired(&mut app, view.id);
        assert!(
            view.parts.iter().enumerate().all(|(index, part)| {
                app.world()
                    .resource::<ChunkStore>()
                    .block_at(view.part_cell(index).unwrap())
                    != Some(part.material)
            }),
            "the original restored parts were never visible to this replica"
        );
        assert!(
            app.world()
                .resource::<TrainCollisionViews>()
                .retirements
                .is_empty()
        );
    }
}

#[test]
fn rapid_identical_remontage_adopts_the_already_active_collider_under_the_new_vehicle_id() {
    let view = vehicle().view();
    let mut app = app(view.clone());
    loaded_cells(&mut app, &view, false);
    wait_active(&mut app, view.id);
    let entity = app.world().resource::<TrainCollisionState>().entries[&view.id]
        .entity
        .unwrap();
    assert!(app.world().resource::<TrainCollisionState>().entries[&view.id].active);
    let mut next = view.clone();
    next.id += 1;
    receive_views(&mut app, &[next.clone()]);
    app.update();
    let state = app.world().resource::<TrainCollisionState>();
    assert_eq!(state.entries.len(), 1);
    assert!(!state.entries.contains_key(&view.id));
    assert_eq!(state.entries[&next.id].entity, Some(entity));
    assert!(state.entries[&next.id].active);
    assert_eq!(app.world().get::<TrainCollider>(entity).unwrap().0, next.id);
}

#[test]
fn coalesced_remontage_with_different_parts_hands_off_only_after_new_geometry_is_active() {
    let view = vehicle().view();
    let mut app = app(view.clone());
    loaded_cells(&mut app, &view, false);
    wait_active(&mut app, view.id);
    let previous = app.world().resource::<TrainCollisionState>().entries[&view.id]
        .entity
        .unwrap();
    let mut next = view.clone();
    next.id += 1;
    next.parts
        .iter_mut()
        .find(|part| part.material == train_parts::FRAME)
        .unwrap()
        .offset = [0, 2, 0];
    assert!(next.valid());
    receive_views(&mut app, &[next.clone()]);
    let mut source = EditJournal::default();
    let mut sections = BTreeSet::new();
    for index in 0..view.parts.len() {
        let cell = view.part_cell(index).unwrap();
        source.record(cell, 0).unwrap();
        sections.insert(terra_voxel::global_to_section(cell).0);
    }
    for section in sections {
        publish_current(&mut app, &source, section);
    }
    app.update();
    assert!(
        app.world()
            .resource::<TrainCollisionState>()
            .entries
            .contains_key(&view.id)
    );
    assert!(!app.world().resource::<TrainCollisionState>().entries[&next.id].active);
    wait_retired(&mut app, view.id);
    assert!(app.world().resource::<TrainCollisionState>().entries[&next.id].active);
    assert!(app.world().get_entity(previous).is_err());
}

#[test]
fn long_vehicle_retirement_uses_actual_physics_interest_without_waiting_for_distant_colliders() {
    let mut car = vehicle();
    car.parts
        .retain(|part| part.material != train_parts::FRAME && part.material != train_parts::SEAT);
    for z in 0..=48 {
        car.parts.push(TrainPart {
            offset: [0, 1, z],
            material: train_parts::FRAME,
            facing: 0,
            wood: WoodSpecies::Oak,
        });
    }
    car.parts.push(TrainPart {
        offset: [0, 2, 0],
        material: train_parts::SEAT,
        facing: 0,
        wood: WoodSpecies::Oak,
    });
    assert!(car.valid());
    let view = car.view();
    let mut app = app(view.clone());
    loaded_cells(&mut app, &view, false);
    wait_active(&mut app, view.id);
    let far = terra_voxel::global_to_section(view.part_cell(view.parts.len() - 2).unwrap()).0;
    assert!(
        !app.world()
            .resource::<crate::TerrainCollisionState>()
            .in_dispatch_interest(far)
    );
    receive_views(&mut app, &[]);
    let mut source = EditJournal::default();
    let mut sections = BTreeSet::new();
    for index in 0..view.parts.len() {
        let cell = view.part_cell(index).unwrap();
        source.record(cell, 0).unwrap();
        sections.insert(terra_voxel::global_to_section(cell).0);
    }
    for section in sections {
        publish_current(&mut app, &source, section);
    }
    wait_retired(&mut app, view.id);
    let world = app.world();
    let origin = Vec3::from_array(far.origin().unwrap().map(|v| v as f32));
    assert!(
        !world
            .resource::<crate::TerrainCollisionState>()
            .ready_for_aabb(
                origin + Vec3::splat(0.1),
                origin + Vec3::splat(0.9),
                world.resource::<ChunkStore>(),
                world.resource::<EditJournal>()
            ),
        "character movement still refuses unbuilt distant terrain after retiring the obsolete vehicle"
    );
}

#[test]
fn receipt_checkpoint_buffer_has_a_hard_budget_and_rejects_without_partial_geometry_mutation() {
    let mut views = TrainCollisionViews::default();
    let journal = EditJournal::default();
    let mut view = vehicle().view();
    views.set_views(&[view.clone()]);
    for _ in 0..terra_core::trains::MAX_VEHICLES * 8 {
        view.id += 1;
        views
            .set_confirmed_views(&[view.clone()], &journal)
            .unwrap();
    }
    let before = views.epoch;
    let previous = views.vehicles.clone();
    view.id += 1;
    assert!(views.set_confirmed_views(&[view], &journal).is_err());
    assert_eq!(views.epoch, before);
    assert_eq!(views.vehicles, previous);
}

#[test]
fn asynchronous_cooks_remain_bounded_and_obsolete_generations_never_publish() {
    let base = vehicle().view();
    let mut app = app(base.clone());
    let mut views: Vec<_> = (0..8)
        .map(|index| {
            let mut view = base.clone();
            view.id += index;
            view.anchor[0] += index as i64 * 16;
            view
        })
        .collect();
    app.world_mut()
        .resource_mut::<TrainCollisionViews>()
        .set_views(&views);
    app.update();
    assert_eq!(
        app.world().resource::<TrainCollisionState>().jobs.len(),
        MAX_COLLIDER_JOBS
    );
    for generation in 1..=12 {
        for view in &mut views {
            view.anchor[0] += 1;
            let mut boxes = train_parts::boxes(train_parts::CHASSIS, 0);
            boxes[0].maximum[1] += f64::from(generation) * 0.005;
            view.blocks.insert(0, TrainBlockView { boxes, ..default() });
            assert!(view.valid());
        }
        app.world_mut()
            .resource_mut::<TrainCollisionViews>()
            .set_views(&views);
        app.update();
        let state = app.world().resource::<TrainCollisionState>();
        assert!(state.jobs.len() <= MAX_COLLIDER_JOBS);
        assert!(
            state
                .entries
                .values()
                .all(|entry| entry.entity.is_none() && !entry.active),
            "a replaced worker generation must never install its obsolete collider"
        );
    }
    for view in &views {
        wait_active(&mut app, view.id);
    }
    let state = app.world().resource::<TrainCollisionState>();
    assert!(state.jobs.is_empty());
    assert_eq!(state.entries.len(), views.len());
    for view in &views {
        let entry = &state.entries[&view.id];
        assert_eq!(entry.anchor, view.anchor);
        assert_eq!(entry.blocks[&0].boxes, view.blocks[&0].boxes);
        let entity = entry.entity.unwrap();
        let actual = app.world().get::<Position>(entity).unwrap().0;
        let expected = Vec3::from_array(view.model_origin().map(|value| value as f32));
        assert_eq!(actual, expected);
        assert_eq!(app.world().get::<TrainCollider>(entity).unwrap().0, view.id);
    }
}

#[test]
fn dismantling_a_pending_worker_cannot_resurrect_a_removed_vehicle() {
    let view = vehicle().view();
    let mut app = app(view.clone());
    app.update();
    assert_eq!(app.world().resource::<TrainCollisionState>().jobs.len(), 1);
    assert!(
        app.world().resource::<TrainCollisionState>().entries[&view.id]
            .entity
            .is_none()
    );
    app.world_mut()
        .resource_mut::<TrainCollisionViews>()
        .set_views(&[]);
    app.update();
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    while !app
        .world()
        .resource::<TrainCollisionState>()
        .jobs
        .is_empty()
    {
        app.update();
        assert!(
            std::time::Instant::now() < deadline,
            "obsolete train worker did not drain"
        );
        std::thread::sleep(Duration::from_millis(1));
    }
    app.update();
    assert!(
        app.world()
            .resource::<TrainCollisionState>()
            .entries
            .is_empty()
    );
    assert_eq!(
        app.world_mut()
            .query::<&TrainCollider>()
            .iter(app.world())
            .count(),
        0
    );
}

#[test]
fn compact_train_motion_moves_the_existing_backend_body_without_recooking() {
    let car = vehicle();
    let view = car.view();
    let mut app = app(view.clone());
    wait_active(&mut app, view.id);
    let entity = app.world().resource::<TrainCollisionState>().entries[&view.id]
        .entity
        .unwrap();
    let epoch = app.world().resource::<TrainCollisionViews>().epoch;
    let motion = terra_core::train_motion::TrainMotion {
        translation: [16.0, 2.0, -8.0],
        yaw: std::f64::consts::FRAC_PI_2,
        ..Default::default()
    };
    app.world_mut()
        .resource_mut::<TrainCollisionViews>()
        .set_motions(&[terra_core::train_motion::TrainMotionFrame {
            id: view.id,
            revision: view.revision,
            motion,
        }])
        .unwrap();
    let mut moved = view;
    moved.motion = motion;
    for _ in 0..3 {
        app.update();
    }
    let state = app.world().resource::<TrainCollisionState>();
    assert_eq!(state.entries[&moved.id].entity, Some(entity));
    assert_eq!(app.world().resource::<TrainCollisionViews>().epoch, epoch);
    assert!(state.jobs.is_empty());
    let expected = moved.model_transform();
    let position = app.world().get::<Position>(entity).unwrap().0;
    let rotation = app.world().get::<Rotation>(entity).unwrap().0;
    assert!((position - Vec3::from_array(expected.0.map(|v| v as f32))).length() < 0.001);
    assert!(
        rotation
            .dot(Quat::from_array(expected.1.map(|v| v as f32)))
            .abs()
            > 0.9999
    );
}
