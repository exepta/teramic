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
