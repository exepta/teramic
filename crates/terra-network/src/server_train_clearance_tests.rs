use super::*;
use crate::server::trains::SeatedPlayers;
use terra_core::train_motion::RailClearance;

#[test]
fn clearance_configuration_requires_the_parked_driver_and_keeps_invalid_requests_atomic() {
    let (mut app, peer, id, _, _) = builder_fixture();
    app.world_mut()
        .get_mut::<Transform>(peer.body)
        .unwrap()
        .translation = Vec3::new(6.5, 7.0, 6.5);
    let car = app
        .world()
        .resource::<EditJournal>()
        .gameplay()
        .trains
        .vehicles[&id]
        .clone();
    let seat = car
        .parts
        .iter()
        .position(|part| part.material == train_parts::CAB)
        .unwrap() as u16;
    let mut seats = SeatedPlayers::default();
    let size = RailClearance {
        width: 7,
        height: 11,
    };
    let action = TrainAction::RailClearance { vehicle: id, size };
    assert_eq!(
        run(&mut app, &peer, action.clone(), &mut seats),
        Err(TrainError::OutOfReach)
    );
    assert_eq!(
        app.world()
            .resource::<EditJournal>()
            .gameplay()
            .trains
            .vehicles[&id],
        car
    );
    run(
        &mut app,
        &peer,
        TrainAction::Seat {
            vehicle: id,
            part: seat,
        },
        &mut seats,
    )
    .unwrap();
    for invalid in [
        RailClearance {
            width: 4,
            height: 9,
        },
        RailClearance {
            width: 5,
            height: 33,
        },
    ] {
        let before = app
            .world()
            .resource::<EditJournal>()
            .gameplay()
            .trains
            .clone();
        assert_eq!(
            run(
                &mut app,
                &peer,
                TrainAction::RailClearance {
                    vehicle: id,
                    size: invalid
                },
                &mut seats
            ),
            Err(TrainError::Limit)
        );
        assert_eq!(
            app.world().resource::<EditJournal>().gameplay().trains,
            before
        );
    }
    run(&mut app, &peer, action.clone(), &mut seats).unwrap();
    let car = app
        .world()
        .resource::<EditJournal>()
        .gameplay()
        .trains
        .vehicles[&id]
        .clone();
    assert_eq!(car.motion.rail_clearance, size);
    assert!(car.motion.brake);
    assert_eq!(car.motion.throttle, 0);

    // The configured lateral/vertical limits control real excavation, not just its UI label.
    let inside = [7, 15, 12];
    let outside = [[8, 15, 12], [7, 16, 12]];
    for cell in [inside, outside[0], outside[1]] {
        let (section, local) = terra_voxel::global_to_section(cell);
        app.world_mut()
            .resource_mut::<ChunkStore>()
            .loaded
            .entry(section)
            .or_insert_with(|| LoadedSection {
                data: terra_voxel::BlockSection::empty(section).unwrap(),
                mesh: None,
                mesh_epoch: 0,
                water_epoch: 0,
                water: None,
                props: vec![],
            })
            .data
            .set_block(local, 3)
            .unwrap();
    }
    {
        let mut system = SystemState::<ControlWorld>::new(app.world_mut());
        let mut world = system.get_mut(app.world_mut()).unwrap();
        crate::server::train_builder::clear_front(&car, &peer, &mut world).unwrap();
        assert_eq!(world.chunks.block_at(inside), Some(0));
        for cell in outside {
            assert_eq!(world.chunks.block_at(cell), Some(3));
        }
    }
    app.world_mut()
        .resource_mut::<EditJournal>()
        .gameplay_mut()
        .trains
        .vehicles
        .get_mut(&id)
        .unwrap()
        .motion
        .speed_mps = 2.0;
    assert_eq!(
        run(&mut app, &peer, action, &mut seats),
        Err(TrainError::Busy)
    );
    app.world_mut()
        .resource_mut::<EditJournal>()
        .gameplay_mut()
        .trains
        .vehicles
        .get_mut(&id)
        .unwrap()
        .motion
        .speed_mps = 0.0;
    run(
        &mut app,
        &peer,
        TrainAction::RailClearance {
            vehicle: id,
            size: RailClearance::default(),
        },
        &mut seats,
    )
    .unwrap();
    assert_eq!(
        app.world()
            .resource::<EditJournal>()
            .gameplay()
            .trains
            .vehicles[&id]
            .motion
            .rail_clearance
            .height,
        0
    );
}

#[test]
fn distant_unsupported_extension_does_not_stop_nearby_excavation_or_consume_cargo() {
    use crate::server::{train_builder, trains};
    use lightyear::prelude::{Connected, PeerId, RemoteId};
    use terra_core::train_motion::StopReason;

    let (mut app, mut peer, id, cargo, _) = builder_fixture();
    peer.mode = Some(GameMode::Survival);
    app.world_mut()
        .get_mut::<Transform>(peer.body)
        .unwrap()
        .translation = Vec3::new(6.5, 7.0, 6.5);
    app.world_mut()
        .resource_scope(|world, mut journal: Mut<EditJournal>| {
            let mut chunks = world.resource_mut::<ChunkStore>();
            for x in -2..=14 {
                for z in 23..=32 {
                    for y in 3..=14 {
                        let (section, local) = terra_voxel::global_to_section([x, y, z]);
                        chunks
                            .loaded
                            .entry(section)
                            .or_insert_with(|| LoadedSection {
                                data: terra_voxel::BlockSection::empty(section).unwrap(),
                                mesh: None,
                                mesh_epoch: 0,
                                water_epoch: 0,
                                water: None,
                                props: vec![],
                            })
                            .data
                            .set_block(local, 0)
                            .unwrap();
                    }
                }
            }
            // The track continues beyond the hill, but the next unbuilt segment
            // has no foundation. That distant gap must not veto the local scoop.
            for z in [7, 11, 15, 19] {
                let plan = terra_world::rails::plan(
                    &chunks,
                    &journal,
                    [4, 5, z],
                    terra_core::rails::STRAIGHT,
                    0,
                )
                .unwrap();
                terra_world::rails::commit(&mut chunks, &mut journal, &plan).unwrap();
            }
            for x in 2..=6 {
                for y in 6..=13 {
                    for z in 12..=20 {
                        let (section, local) = terra_voxel::global_to_section([x, y, z]);
                        chunks
                            .loaded
                            .get_mut(&section)
                            .unwrap()
                            .data
                            .set_block(local, 3)
                            .unwrap();
                    }
                }
            }
            let car = journal.gameplay_mut().trains.vehicles.get_mut(&id).unwrap();
            car.motion.cursor = None;
            car.motion.rail_clearance.height = 9;
            let engine = car.parts.len() as u16;
            car.parts.push(TrainPart {
                offset: [0, 2, 1],
                material: 218,
                facing: 2,
                wood: WoodSpecies::Oak,
            });
            car.cargo.insert(
                engine,
                Inventory {
                    slots: vec![ItemStack {
                        item: ItemKind::Block(18),
                        count: 2,
                    }],
                    ..Default::default()
                },
            );
            assert!(car.valid());
        });
    let initial_track_count = terra_world::rails::network(app.world().resource::<EditJournal>())
        .unwrap()
        .len();
    let before_cargo = app
        .world()
        .resource::<EditJournal>()
        .gameplay()
        .trains
        .vehicles[&id]
        .cargo[&cargo]
        .clone();
    let seat = app
        .world()
        .resource::<EditJournal>()
        .gameplay()
        .trains
        .vehicles[&id]
        .parts
        .iter()
        .position(|part| part.material == train_parts::CAB)
        .unwrap() as u16;
    let mut seats = SeatedPlayers::default();
    run(
        &mut app,
        &peer,
        TrainAction::Seat {
            vehicle: id,
            part: seat,
        },
        &mut seats,
    )
    .unwrap();
    run(
        &mut app,
        &peer,
        TrainAction::RailBuilder {
            vehicle: id,
            active: true,
        },
        &mut seats,
    )
    .unwrap();
    run(
        &mut app,
        &peer,
        TrainAction::Drive {
            vehicle: id,
            throttle: 1,
            brake: false,
        },
        &mut seats,
    )
    .expect("the nearby hill can be excavated despite the distant unsupported extension");
    assert_eq!(
        app.world().resource::<ChunkStore>().block_at([4, 8, 12]),
        Some(0)
    );
    let connection = app
        .world_mut()
        .spawn((RemoteId(PeerId::Netcode(1)), peer))
        .id();
    app.world_mut().entity_mut(connection).insert(Connected);
    let mut time = Time::<Fixed>::from_hz(60.0);
    time.advance_by(std::time::Duration::from_secs_f64(1.0 / 60.0));
    app.insert_resource(time)
        .insert_resource(seats)
        .init_resource::<trains::TrainRuntime>()
        .init_resource::<crate::server::train_damage::Impacts>()
        .add_systems(Update, (train_builder::tick, trains::drive).chain());
    for _ in 0..180 {
        app.update();
    }
    {
        let journal = app.world().resource::<EditJournal>();
        let car = &journal.gameplay().trains.vehicles[&id];
        assert!(car.motion.wheel_distance_m > 2.0, "{:?}", car.motion);
        assert!(!car.motion.brake, "{:?}", car.motion);
        assert_eq!(car.cargo[&cargo], before_cargo);
        assert_eq!(
            terra_world::rails::network(journal).unwrap().len(),
            initial_track_count
        );
    }
    // Once the unsupported extension reaches the actual work range, fail closed
    // and brake before the physical train reaches the end of the existing track.
    for _ in 0..600 {
        app.update();
        let journal = app.world().resource::<EditJournal>();
        let car = &journal.gameplay().trains.vehicles[&id];
        if car.motion.brake && car.motion.stopped() {
            break;
        }
    }
    let journal = app.world().resource::<EditJournal>();
    let car = &journal.gameplay().trains.vehicles[&id];
    assert!(car.motion.brake && car.motion.stopped(), "{:?}", car.motion);
    assert_eq!(car.motion.stop_reason, StopReason::BuilderBlocked);
    assert!(car.model_bounds()[1][2] < 23.0, "{:?}", car.model_bounds());
    assert_eq!(car.cargo[&cargo], before_cargo);
    assert_eq!(
        terra_world::rails::network(journal).unwrap().len(),
        initial_track_count
    );
    assert_eq!(
        app.world().resource::<ChunkStore>().block_at([4, 4, 24]),
        Some(0)
    );
}
