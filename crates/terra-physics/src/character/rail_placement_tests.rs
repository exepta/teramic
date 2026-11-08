//! Publishing rails around their builder uses the ordinary shared collision response.
use super::*;
use terra_core::{
    WoodSpecies,
    rails::{self, Piece},
};

#[test]
fn a_seed_or_ramp_published_around_a_standing_character_settles_without_teleport_or_persistent_overlap()
 {
    for (ramp, ceiling, flight) in [
        (false, false, false),
        (true, false, false),
        (true, true, false),
        (true, false, true),
    ] {
        let (mut app, body) = fixture(0.5, false, None);
        if ramp {
            app.world_mut()
                .resource_scope(|world, mut journal: Mut<EditJournal>| {
                    let mut chunks = world.resource_mut::<ChunkStore>();
                    let source = Piece::new([4, 1, 1], rails::STRAIGHT, 0).unwrap();
                    let plan = terra_world::rails::plan(
                        &chunks,
                        &journal,
                        source.anchor,
                        source.material,
                        source.encoded_facing(),
                    )
                    .unwrap();
                    terra_world::rails::commit(&mut chunks, &mut journal, &plan).unwrap();
                    let upper = [4, 1, 12];
                    let (owner, local) = terra_voxel::global_to_section(upper);
                    chunks
                        .loaded
                        .get_mut(&owner)
                        .unwrap()
                        .data
                        .set_block(local, 3)
                        .unwrap();
                    journal.record(upper, 3).unwrap();
                });
        }
        if ceiling {
            shape(
                &mut app,
                Vec3::new(3.0, 3.0, 7.0),
                ShapeBox {
                    minimum: [0.0; 3],
                    maximum: [3.0, 1.0, 3.0],
                    hinged: false,
                },
            );
        }
        if flight {
            app.world_mut().resource_mut::<Drive>().input.flight_speed = Some(12.0);
            app.world_mut()
                .resource_scope(|world, mut journal: Mut<EditJournal>| {
                    let mut chunks = world.resource_mut::<ChunkStore>();
                    let cell = [4, 0, 8];
                    let (owner, local) = terra_voxel::global_to_section(cell);
                    chunks
                        .loaded
                        .get_mut(&owner)
                        .unwrap()
                        .data
                        .set_block(local, 0)
                        .unwrap();
                    journal.record(cell, 0).unwrap();
                });
        }
        let center = Vec3::new(4.5, 1.0 + PLAYER_HALF_HEIGHT, if ramp { 8.5 } else { 4.5 });
        app.world_mut()
            .get_mut::<Transform>(body)
            .unwrap()
            .translation = center;
        app.world_mut().get_mut::<LinearVelocity>(body).unwrap().0 = Vec3::ZERO;
        let eye = center + Vec3::Y * PLAYER_EYE_OFFSET;
        let goal = Vec3::new(
            4.5,
            if ramp { 2.0 } else { 1.0 },
            if ramp { 12.5 } else { 4.9 },
        );
        let direction = (goal - eye).normalize();
        let rotation = Quat::from_rotation_arc(Vec3::NEG_Z, direction);
        let (yaw, pitch, _) = rotation.to_euler(EulerRot::YXZ);
        let (_, target) = terra_world::targeting::pick_world_for_rails(
            app.world().resource::<ChunkStore>(),
            app.world().resource::<EditJournal>(),
            app.world()
                .resource::<terra_core::block_properties::BlockPropertiesCatalog>(),
            eye.to_array().map(f64::from),
            direction.to_array().map(f64::from),
            terra_world::props::PropPose::Rest,
        );
        let Some(terra_world::targeting::WorldTarget::Block(hit)) = target else {
            panic!("actual rail floor ray required")
        };
        assert_eq!(hit.position, if ramp { [4, 1, 12] } else { [4, 0, 4] });
        let plan = if ramp {
            terra_world::rails::connection_aim(
                app.world().resource::<ChunkStore>(),
                app.world().resource::<EditJournal>(),
                rails::Endpoint {
                    anchor: [4, 1, 1],
                    port: 1,
                },
                hit,
                (
                    eye.to_array().map(f64::from),
                    direction.to_array().map(f64::from),
                ),
                [yaw, pitch],
                WoodSpecies::Oak,
            )
        } else {
            terra_world::rails::seed(
                app.world().resource::<ChunkStore>(),
                app.world().resource::<EditJournal>(),
                hit,
                (
                    eye.to_array().map(f64::from),
                    direction.to_array().map(f64::from),
                ),
                [yaw, pitch],
                WoodSpecies::Oak,
            )
        }
        .unwrap();
        let half = Vec3::new(PLAYER_RADIUS, PLAYER_HALF_HEIGHT, PLAYER_RADIUS);
        assert!(
            terra_world::placement::boxes(
                &plan,
                app.world().resource::<ChunkStore>(),
                app.world().resource::<EditJournal>(),
                app.world()
                    .resource::<terra_core::block_properties::BlockPropertiesCatalog>()
            )
            .iter()
            .any(|shape| shape.intersects(
                (center - half).to_array().map(f64::from),
                (center + half).to_array().map(f64::from)
            )),
            "the actor must overlap the new authored geometry"
        );
        app.world_mut()
            .resource_scope(|world, mut journal: Mut<EditJournal>| {
                let mut chunks = world.resource_mut::<ChunkStore>();
                terra_world::rails::commit(&mut chunks, &mut journal, &plan).unwrap();
                for (owner, section) in &mut chunks.loaded {
                    section.mesh_epoch = journal.mesh_epoch(*owner);
                }
            });
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            app.update();
            if app
                .world()
                .resource::<TerrainCollisionState>()
                .ready_for_aabb(
                    center - Vec3::splat(1.0),
                    center + Vec3::splat(1.0),
                    app.world().resource::<ChunkStore>(),
                    app.world().resource::<EditJournal>(),
                )
            {
                break;
            }
            assert!(
                Instant::now() < deadline,
                "new rail collision must finish rebuilding"
            );
            std::thread::sleep(Duration::from_millis(1));
        }
        let mut settled_y = Vec::new();
        for tick in 0..180 {
            let previous = app.world().get::<Transform>(body).unwrap().translation;
            app.update();
            let position = app.world().get::<Transform>(body).unwrap().translation;
            assert!(position.is_finite());
            assert!(
                Vec2::new(position.x - center.x, position.z - center.z).length() < 0.8,
                "ramp={ramp}: no long horizontal depenetration: {center:?} -> {position:?}"
            );
            assert!(
                (position.y - previous.y).abs() < 0.8,
                "ramp={ramp}: no sudden vertical relocation: {previous:?} -> {position:?}"
            );
            if tick >= 150 {
                settled_y.push(position.y);
            }
        }
        let position = app.world().get::<Transform>(body).unwrap().translation;
        if ceiling {
            assert!(
                position.y + PLAYER_HALF_HEIGHT <= 3.02,
                "a rail correction must not move the capsule through a ceiling: {position:?}"
            );
            assert!(
                position.y - PLAYER_HALF_HEIGHT < 1.2,
                "blocked headroom must refuse the support lift"
            );
            continue;
        }
        let support = terra_world::interaction_shapes::raycast_solid(
            app.world().resource::<ChunkStore>(),
            app.world().resource::<EditJournal>(),
            app.world()
                .resource::<terra_core::block_properties::BlockPropertiesCatalog>(),
            [f64::from(position.x), 6.0, f64::from(position.z)],
            [0.0, -1.0, 0.0],
            8.0,
        )
        .unwrap();
        let height = 6.0 - support.distance;
        let feet = f64::from(position.y - PLAYER_HALF_HEIGHT);
        assert!(
            (height - 0.04..=height + 0.08).contains(&feet),
            "ramp={ramp}: capsule must settle on actual support, feet={feet}, support={height}, position={position:?}"
        );
        let lowest = settled_y.iter().copied().fold(f32::INFINITY, f32::min);
        let highest = settled_y.iter().copied().fold(f32::NEG_INFINITY, f32::max);
        assert!(
            highest - lowest < 0.03,
            "ramp={ramp}: no persistent vertical jitter"
        );
        assert_eq!(
            app.world().get::<CharacterGrounded>(body).unwrap().0,
            !flight
        );
    }
}

#[test]
fn ordinary_solid_penetration_does_not_trigger_rail_support_recovery() {
    let (mut app, body) = fixture(1.0, false, None);
    let initial = CharacterState {
        position: Vec3::new(8.5, 1.9, 8.5),
        velocity: Vec3::ZERO,
        grounded: true,
    };
    let mut system = bevy::ecs::system::SystemState::<(
        CharacterSimulation,
        Query<&Collider, With<CharacterBody>>,
    )>::new(app.world_mut());
    let (simulation, bodies) = system.get_mut(app.world_mut()).unwrap();
    let collider = bodies.get(body).unwrap();
    let filter = SpatialQueryFilter::from_excluded_entities([body]);
    assert!(
        !simulation
            .spatial
            .shape_intersections(collider, initial.position, Quat::IDENTITY, &filter)
            .is_empty(),
        "the fixture must intersect an actual ordinary solid"
    );
    let mut state = initial;
    simulation.recover_rail_support(body, collider, &mut state, CharacterInput::default());
    assert_eq!(state.position, initial.position);
    assert_eq!(state.velocity, initial.velocity);
}
