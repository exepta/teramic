use super::*;
use crate::protocol::trains::{TrainAction, TrainOutcome};
use terra_core::{
    ToolKind, WoodSpecies, train_parts,
    trains::{TrainError, TrainPart, Vehicle, VehicleKind},
};
#[path = "server_train_clearance_tests.rs"]
mod clearance;
#[path = "server_train_sort_tests.rs"]
mod sorting;
fn train_fixture() -> (App, Peer, u64, u16, u16) {
    let (mut app, peer, _) = fixture(ResourcePropKind::SmallStones, false);
    app.world_mut()
        .get_mut::<Transform>(peer.body)
        .unwrap()
        .translation = Vec3::new(6.5, 6.0, 6.5);
    let mut parts = Vec::new();
    for z in 0..=6 {
        parts.push(TrainPart {
            offset: [0, 1, z],
            material: if z == 2 {
                train_parts::CHASSIS
            } else {
                train_parts::FRAME
            },
            facing: 0,
            wood: WoodSpecies::Birch,
        });
    }
    for (offset, material) in [
        ([0, 0, 1], train_parts::BOGIE),
        ([0, 0, 5], train_parts::BOGIE),
        ([0, 1, -1], train_parts::COUPLER),
        ([0, 1, 7], train_parts::COUPLER),
        ([0, 2, 3], train_parts::CAB),
        ([0, 2, 4], train_parts::CARGO),
    ] {
        parts.push(TrainPart {
            offset,
            material,
            facing: 0,
            wood: WoodSpecies::Birch,
        });
    }
    let seat = parts
        .iter()
        .position(|part| part.material == train_parts::CAB)
        .unwrap() as u16;
    let cargo = parts
        .iter()
        .position(|part| part.material == train_parts::CARGO)
        .unwrap() as u16;
    let vehicle = Vehicle {
        name: String::new(),
        id: 1,
        owner: peer.uuid,
        kind: VehicleKind::Locomotive,
        anchor: [4, 5, 3],
        parts,
        cargo: std::collections::BTreeMap::from([(
            cargo,
            Inventory {
                slots: vec![ItemStack {
                    item: ItemKind::Block(3),
                    count: 42,
                }],
                ..Default::default()
            },
        )]),
        blocks: Default::default(),
        payloads: Default::default(),
        couplings: [None, None],
        revision: 0,

        motion: Default::default(),
        burn_seconds: 0.0,
        steam: Default::default(),
    };
    let id = app
        .world_mut()
        .resource_mut::<EditJournal>()
        .gameplay_mut()
        .trains
        .insert(vehicle)
        .unwrap();
    (app, peer, id, seat, cargo)
}
fn run(
    app: &mut App,
    peer: &Peer,
    action: TrainAction,
    seats: &mut crate::server::trains::SeatedPlayers,
) -> Result<TrainOutcome, TrainError> {
    let mut system = SystemState::<ControlWorld>::new(app.world_mut());
    let mut world = system.get_mut(app.world_mut()).unwrap();
    crate::server::trains::apply(action, peer, &mut world, seats)
}
#[test]
fn leave_overflow_keeps_both_owned_seat_binding_and_pose_unchanged() {
    let (mut app, peer, id, seat, _) = train_fixture();
    let mut seats = crate::server::trains::SeatedPlayers::default();
    assert!(matches!(
        run(
            &mut app,
            &peer,
            TrainAction::Seat {
                vehicle: id,
                part: seat
            },
            &mut seats
        ),
        Ok(TrainOutcome::Seated(Some(_)))
    ));
    let previous_pose = seats.pose(peer.uuid);
    let binding = seats.binding(peer.uuid);
    {
        let mut journal = app.world_mut().resource_mut::<EditJournal>();
        journal.gameplay_mut().revision = u64::MAX;
        journal.gameplay_mut().trains.revision = u64::MAX;
    }
    let before = app.world().resource::<EditJournal>().gameplay().clone();
    assert_eq!(
        run(&mut app, &peer, TrainAction::Leave, &mut seats),
        Err(TrainError::Overflow)
    );
    assert_eq!(seats.pose(peer.uuid), previous_pose);
    assert_eq!(seats.binding(peer.uuid), binding);
    assert_eq!(app.world().resource::<EditJournal>().gameplay(), &before);
}

#[test]
fn horse_and_train_seat_transitions_are_exclusive_without_mutating_either_owner() {
    use terra_core::creatures::{Creature, CreatureBehavior, CreatureKind, CreatureNeeds};
    let (mut app, mut peer, id, seat, _) = train_fixture();
    peer.held_slot = None;
    let horse = Creature {
        id: (1 << 62) + 210_719,
        home: [0, 0],
        kind: CreatureKind::Horse,
        position: [6.5, 4.0, 6.5],
        velocity: [0.0; 3],
        yaw: 0.0,
        health: 20.0,
        behavior: CreatureBehavior::Rest,
        timer: 0.0,
        age: 0.0,
        variant: 0,
        needs: CreatureNeeds::full(CreatureKind::Horse),
    };
    {
        let mut journal = app.world_mut().resource_mut::<EditJournal>();
        journal
            .gameplay_mut()
            .creatures
            .animals
            .insert(horse.id, horse.clone());
        journal
            .gameplay_mut()
            .creatures
            .owners
            .insert(horse.id, peer.uuid);
    }
    let mut seats = crate::server::trains::SeatedPlayers::default();
    let mut system = SystemState::<ControlWorld>::new(app.world_mut());
    {
        let mut world = system.get_mut(app.world_mut()).unwrap();
        assert!(
            world
                .mounts
                .mount(&horse, peer.uuid, world.journal.gameplay())
        );
        let before = world.journal.gameplay().clone();
        assert_eq!(
            crate::server::trains::apply(
                TrainAction::Seat {
                    vehicle: id,
                    part: seat
                },
                &peer,
                &mut world,
                &mut seats
            ),
            Err(TrainError::Busy)
        );
        assert_eq!(world.journal.gameplay(), &before);
        assert_eq!(world.mounts.active[&peer.uuid].horse, horse);
        assert!(seats.binding(peer.uuid).is_none());
        world.mounts.release(peer.uuid);
        assert!(matches!(
            crate::server::trains::apply(
                TrainAction::Seat {
                    vehicle: id,
                    part: seat
                },
                &peer,
                &mut world,
                &mut seats
            ),
            Ok(TrainOutcome::Seated(Some(_)))
        ));
        let before = world.journal.gameplay().clone();
        let pose = seats.pose(peer.uuid);
        assert!(crate::server::trains::horse_mount_blocked(
            &peer, &world, &seats, horse.id, true
        ));
        assert_eq!(world.journal.gameplay(), &before);
        assert_eq!(seats.pose(peer.uuid), pose);
        assert!(world.mounts.active.is_empty());
        assert_eq!(
            crate::server::trains::apply(TrainAction::Leave, &peer, &mut world, &mut seats),
            Ok(TrainOutcome::Seated(None))
        );
        assert!(seats.has_exit(peer.uuid));
        let before = world.journal.gameplay().clone();
        assert!(crate::server::trains::horse_mount_blocked(
            &peer, &world, &seats, horse.id, true
        ));
        assert_eq!(world.journal.gameplay(), &before);
        assert!(world.mounts.active.is_empty());
    }
}
#[test]
fn cargo_gesture_is_owner_checked_and_stale_retry_cannot_duplicate_the_owned_stack() {
    let (mut app, mut peer, id, _, cargo) = train_fixture();
    let mut seats = crate::server::trains::SeatedPlayers::default();
    let owner = peer.uuid;
    peer.uuid = PlayerUuid::from_u128(99);
    assert_eq!(
        run(
            &mut app,
            &peer,
            TrainAction::OpenCargo {
                vehicle: id,
                part: cargo
            },
            &mut seats
        ),
        Err(TrainError::NotOwner)
    );
    peer.uuid = owner;
    let car_revision = app
        .world()
        .resource::<EditJournal>()
        .gameplay()
        .trains
        .vehicles[&id]
        .revision;
    let player_revision =
        app.world().resource::<EditJournal>().gameplay().inventories[&owner].revision;
    let action = TrainAction::Cargo {
        vehicle: id,
        part: cargo,
        revision: car_revision,
        player_revision,
        gesture: terra_core::containers::ContainerGesture::Wheel {
            slot: terra_core::INVENTORY_SLOTS as u8,
            to_cursor: false,
        },
        reserved: [0; terra_core::INVENTORY_SLOTS],
    };
    let result = run(&mut app, &peer, action.clone(), &mut seats).unwrap();
    assert!(
        matches!(result,TrainOutcome::Cargo(snapshot) if snapshot.inventory.count(ItemKind::Block(3))==41)
    );
    let before = app.world().resource::<EditJournal>().gameplay().clone();
    assert_eq!(before.inventories[&owner].count(ItemKind::Block(3)), 1);
    assert!(before.inventories[&owner].cursor.is_none());
    assert_eq!(
        run(&mut app, &peer, action, &mut seats),
        Err(TrainError::Stale)
    );
    assert_eq!(app.world().resource::<EditJournal>().gameplay(), &before);
}

#[test]
fn a_bound_owner_can_fuel_and_use_distant_car_storage_but_foreign_and_unseated_packets_cannot() {
    let (mut app, mut peer, id, seat, cargo) = train_fixture();
    let owner = peer.uuid;
    let section = terra_voxel::global_to_section([4, 5, 21]).0;
    app.world_mut().resource_mut::<ChunkStore>().loaded.insert(
        section,
        LoadedSection {
            data: terra_voxel::BlockSection::empty(section).unwrap(),
            mesh: None,
            mesh_epoch: 0,
            water_epoch: 0,
            water: None,
            props: Vec::new(),
        },
    );
    let (engine, chest) = {
        let mut journal = app.world_mut().resource_mut::<EditJournal>();
        let car = journal.gameplay_mut().trains.vehicles.get_mut(&id).unwrap();
        for part in &mut car.parts {
            if part.material == train_parts::COUPLER && part.offset[2] == 7 {
                part.offset[2] = 19;
            } else if part.material == train_parts::BOGIE && part.offset[2] == 5 {
                part.offset[2] = 16;
            } else if part.material == train_parts::CARGO {
                part.offset[2] = 18;
            }
        }
        for z in 7..=18 {
            car.parts.push(TrainPart {
                offset: [0, 1, z],
                material: train_parts::FRAME,
                facing: 0,
                wood: WoodSpecies::Birch,
            });
        }
        let engine = car.parts.len() as u16;
        car.parts.push(TrainPart {
            offset: [0, 2, 16],
            material: 218,
            facing: 0,
            wood: WoodSpecies::Birch,
        });
        let chest = car.parts.len() as u16;
        car.parts.push(TrainPart {
            offset: [0, 2, 17],
            material: 29,
            facing: 0,
            wood: WoodSpecies::Birch,
        });
        car.blocks.insert(
            chest,
            terra_core::trains::TrainBlockView {
                boxes: vec![terra_core::block_shapes::ShapeBox {
                    minimum: [0.0; 3],
                    maximum: [1.0; 3],
                    hinged: false,
                }],
                ..Default::default()
            },
        );
        for (part, item) in [(engine, ItemKind::Block(16)), (chest, ItemKind::Block(13))] {
            car.cargo.insert(
                part,
                Inventory {
                    slots: vec![ItemStack { item, count: 3 }],
                    ..Default::default()
                },
            );
        }
        assert!(car.valid());
        (engine, chest)
    };
    let mut seats = crate::server::trains::SeatedPlayers::default();
    for part in [engine, chest, cargo] {
        assert_eq!(
            run(
                &mut app,
                &peer,
                TrainAction::OpenCargo { vehicle: id, part },
                &mut seats
            ),
            Err(TrainError::OutOfReach)
        );
    }
    assert!(matches!(
        run(
            &mut app,
            &peer,
            TrainAction::Seat {
                vehicle: id,
                part: seat
            },
            &mut seats
        ),
        Ok(TrainOutcome::Seated(Some(_)))
    ));
    for part in [engine, chest, cargo] {
        let TrainOutcome::Cargo(snapshot) = run(
            &mut app,
            &peer,
            TrainAction::OpenCargo { vehicle: id, part },
            &mut seats,
        )
        .unwrap() else {
            panic!("the bound owner must open every storage part in this car");
        };
        let player_revision =
            app.world().resource::<EditJournal>().gameplay().inventories[&owner].revision;
        let action = TrainAction::Cargo {
            vehicle: id,
            part,
            revision: snapshot.revision,
            player_revision,
            gesture: terra_core::containers::ContainerGesture::Wheel {
                slot: terra_core::INVENTORY_SLOTS as u8,
                to_cursor: false,
            },
            reserved: [0; terra_core::INVENTORY_SLOTS],
        };
        peer.uuid = PlayerUuid::from_u128(99);
        let before = app.world().resource::<EditJournal>().gameplay().clone();
        assert_eq!(
            run(
                &mut app,
                &peer,
                TrainAction::OpenCargo { vehicle: id, part },
                &mut seats
            ),
            Err(TrainError::NotOwner)
        );
        assert_eq!(
            run(&mut app, &peer, action.clone(), &mut seats),
            Err(TrainError::NotOwner)
        );
        assert_eq!(app.world().resource::<EditJournal>().gameplay(), &before);
        peer.uuid = owner;
        let result = run(&mut app, &peer, action, &mut seats).unwrap();
        let TrainOutcome::Cargo(updated) = result else {
            panic!("remote owner cargo gesture must work");
        };
        assert_eq!(
            updated.inventory.slots[0].count,
            snapshot.inventory.slots[0].count - 1
        );
        if part == engine {
            let player_revision =
                app.world().resource::<EditJournal>().gameplay().inventories[&owner].revision;
            let refill = TrainAction::Cargo {
                vehicle: id,
                part,
                revision: updated.revision,
                player_revision,
                gesture: terra_core::containers::ContainerGesture::Wheel {
                    slot: terra_core::INVENTORY_SLOTS as u8,
                    to_cursor: true,
                },
                reserved: [0; terra_core::INVENTORY_SLOTS],
            };
            let TrainOutcome::Cargo(refilled) = run(&mut app, &peer, refill, &mut seats).unwrap()
            else {
                panic!("the bound owner must be able to fuel the distant engine");
            };
            assert_eq!(
                refilled.inventory.slots[0].count,
                snapshot.inventory.slots[0].count
            );
        }
    }
}

#[test]
fn an_exposed_cab_profile_is_reachable_above_its_side_panel_but_never_through_a_world_wall() {
    let (mut app, peer, id, seat, _) = train_fixture();
    let slab = {
        let mut journal = app.world_mut().resource_mut::<EditJournal>();
        let car = journal.gameplay_mut().trains.vehicles.get_mut(&id).unwrap();
        let index = car.parts.len() as u16;
        car.parts.push(TrainPart {
            offset: [-1, 1, 3],
            material: 105,
            facing: 5,
            wood: WoodSpecies::Birch,
        });
        car.blocks.insert(
            index,
            terra_core::trains::TrainBlockView {
                boxes: vec![terra_core::construction::slab_bounds(5)],
                ..Default::default()
            },
        );
        assert!(car.valid());
        index
    };
    app.world_mut()
        .get_mut::<Transform>(peer.body)
        .unwrap()
        .translation = Vec3::new(1.5, 4.9, 6.5);
    let eye = [1.5_f64, 5.62, 6.5];
    {
        let journal = app.world().resource::<EditJournal>();
        let car = &journal.gameplay().trains.vehicles[&id];
        let old_target = car
            .world_cell(usize::from(seat))
            .unwrap()
            .map(|v| v as f64 + 0.5);
        let delta: [f64; 3] = std::array::from_fn(|i| old_target[i] - eye[i]);
        let distance = delta.iter().map(|v| v * v).sum::<f64>().sqrt();
        let hit = terra_world::trains::raycast(journal, eye, delta.map(|v| v / distance), distance)
            .unwrap();
        assert_eq!(
            hit.part, slab,
            "the former quantized owner center is hidden by the side panel"
        );
    }
    let mut seats = crate::server::trains::SeatedPlayers::default();
    assert!(matches!(
        run(
            &mut app,
            &peer,
            TrainAction::Seat {
                vehicle: id,
                part: seat
            },
            &mut seats
        ),
        Ok(TrainOutcome::Seated(Some(_)))
    ));
    for y in 5..=8 {
        for z in 5..=7 {
            app.world_mut()
                .resource_mut::<ChunkStore>()
                .loaded
                .get_mut(&terra_voxel::global_to_section([2, y, z]).0)
                .unwrap()
                .data
                .set_block([2, y as usize, z as usize], 3)
                .unwrap();
            app.world_mut()
                .resource_mut::<EditJournal>()
                .record([2, y, z], 3)
                .unwrap();
        }
    }
    let before = app.world().resource::<EditJournal>().gameplay().clone();
    let mut outside = crate::server::trains::SeatedPlayers::default();
    assert_eq!(
        run(
            &mut app,
            &peer,
            TrainAction::Seat {
                vehicle: id,
                part: seat
            },
            &mut outside
        ),
        Err(TrainError::OutOfReach)
    );
    assert_eq!(app.world().resource::<EditJournal>().gameplay(), &before);
}

#[test]
fn real_coal_can_be_clicked_onto_the_cursor_then_deposited_into_an_owned_engine_atomically() {
    use terra_core::containers::ContainerGesture;
    let (mut app, peer, id, seat, _) = train_fixture();
    let coal = ItemKind::Mineral(terra_core::MineralItem::Coal);
    assert!(terra_core::furnace::fuel_seconds(coal).is_some());
    let engine = {
        let mut journal = app.world_mut().resource_mut::<EditJournal>();
        let car = journal.gameplay_mut().trains.vehicles.get_mut(&id).unwrap();
        let engine = car.parts.len() as u16;
        car.parts.push(TrainPart {
            offset: [1, 1, 3],
            material: 218,
            facing: 0,
            wood: WoodSpecies::Birch,
        });
        car.cargo.insert(engine, Inventory::default());
        assert!(car.valid());
        assert!(
            journal
                .gameplay_mut()
                .inventories
                .get_mut(&peer.uuid)
                .unwrap()
                .creative_set(
                    8,
                    ItemStack {
                        item: coal,
                        count: 12
                    }
                )
        );
        engine
    };
    let mut seats = crate::server::trains::SeatedPlayers::default();
    assert!(matches!(
        run(
            &mut app,
            &peer,
            TrainAction::Seat {
                vehicle: id,
                part: seat
            },
            &mut seats
        ),
        Ok(TrainOutcome::Seated(Some(_)))
    ));
    let TrainOutcome::Cargo(snapshot) = run(
        &mut app,
        &peer,
        TrainAction::OpenCargo {
            vehicle: id,
            part: engine,
        },
        &mut seats,
    )
    .unwrap() else {
        panic!("the owner must open the fuel inventory");
    };
    let player_revision =
        app.world().resource::<EditJournal>().gameplay().inventories[&peer.uuid].revision;
    let pickup = TrainAction::Cargo {
        vehicle: id,
        part: engine,
        revision: snapshot.revision,
        player_revision,
        gesture: ContainerGesture::Click {
            slot: 8,
            secondary: false,
        },
        reserved: [0; terra_core::INVENTORY_SLOTS],
    };
    let TrainOutcome::Cargo(picked) = run(&mut app, &peer, pickup, &mut seats).unwrap() else {
        panic!("clicking real coal must pick up the stack");
    };
    assert_eq!(
        app.world().resource::<EditJournal>().gameplay().inventories[&peer.uuid].cursor,
        Some(ItemStack {
            item: coal,
            count: 12
        })
    );
    assert_eq!(picked.inventory.count(coal), 0);
    let player_revision =
        app.world().resource::<EditJournal>().gameplay().inventories[&peer.uuid].revision;
    let deposit = TrainAction::Cargo {
        vehicle: id,
        part: engine,
        revision: picked.revision,
        player_revision,
        gesture: ContainerGesture::Click {
            slot: terra_core::INVENTORY_SLOTS as u8,
            secondary: false,
        },
        reserved: [0; terra_core::INVENTORY_SLOTS],
    };
    let TrainOutcome::Cargo(filled) = run(&mut app, &peer, deposit.clone(), &mut seats).unwrap()
    else {
        panic!("clicking the engine must deposit the held coal");
    };
    assert_eq!(filled.inventory.count(coal), 12);
    assert!(
        app.world().resource::<EditJournal>().gameplay().inventories[&peer.uuid]
            .cursor
            .is_none()
    );
    assert_eq!(
        app.world().resource::<EditJournal>().gameplay().inventories[&peer.uuid].count(coal),
        0
    );
    let before = app.world().resource::<EditJournal>().gameplay().clone();
    assert_eq!(
        run(&mut app, &peer, deposit, &mut seats),
        Err(TrainError::Stale)
    );
    assert_eq!(app.world().resource::<EditJournal>().gameplay(), &before);
    {
        let mut journal = app.world_mut().resource_mut::<EditJournal>();
        let bag = journal
            .gameplay_mut()
            .inventories
            .get_mut(&peer.uuid)
            .unwrap();
        bag.cursor = Some(ItemStack {
            item: ItemKind::Block(3),
            count: 1,
        });
        bag.revision += 1;
    }
    let before = app.world().resource::<EditJournal>().gameplay().clone();
    let invalid = TrainAction::Cargo {
        vehicle: id,
        part: engine,
        revision: filled.revision,
        player_revision: before.inventories[&peer.uuid].revision,
        gesture: ContainerGesture::Click {
            slot: terra_core::INVENTORY_SLOTS as u8,
            secondary: false,
        },
        reserved: [0; terra_core::INVENTORY_SLOTS],
    };
    assert_eq!(
        run(&mut app, &peer, invalid, &mut seats),
        Err(TrainError::Invalid)
    );
    assert_eq!(app.world().resource::<EditJournal>().gameplay(), &before);
}

#[test]
fn train_names_require_ownership_and_reject_controls_or_oversized_text_atomically() {
    let (mut app, peer, id, seat, _) = train_fixture();
    let mut seats = crate::server::trains::SeatedPlayers::default();
    assert!(
        run(
            &mut app,
            &peer,
            TrainAction::Seat {
                vehicle: id,
                part: seat
            },
            &mut seats
        )
        .is_ok()
    );
    for name in ["a".repeat(25), "bad\nname".into(), "\u{0000}".into()] {
        let before = app.world().resource::<EditJournal>().gameplay().clone();
        assert_eq!(
            run(
                &mut app,
                &peer,
                TrainAction::Name {
                    vehicle: id,
                    name,
                    station: None
                },
                &mut seats
            ),
            Err(TrainError::Invalid)
        );
        assert_eq!(app.world().resource::<EditJournal>().gameplay(), &before);
    }
    assert!(
        run(
            &mut app,
            &peer,
            TrainAction::Name {
                vehicle: id,
                name: "Lok Köln".into(),
                station: None
            },
            &mut seats
        )
        .is_ok()
    );
    assert_eq!(
        app.world()
            .resource::<EditJournal>()
            .gameplay()
            .trains
            .vehicles[&id]
            .name,
        "Lok Köln"
    );
    let mut foreign = peer;
    foreign.uuid = terra_core::PlayerUuid::from_u128(999_219);
    assert_eq!(
        run(
            &mut app,
            &foreign,
            TrainAction::Name {
                vehicle: id,
                name: "stolen".into(),
                station: None
            },
            &mut seats
        ),
        Err(TrainError::NotOwner)
    );
}

#[test]
fn turntable_action_rotates_the_owned_vehicle_and_track_atomically() {
    let (mut app, mut peer, id, seat, _) = train_fixture();
    // Construct the platform before publishing the fixture's assembled car.
    let car = app
        .world_mut()
        .resource_mut::<EditJournal>()
        .gameplay_mut()
        .trains
        .vehicles
        .remove(&id)
        .unwrap();
    app.world_mut()
        .resource_scope(|world, mut journal: Mut<EditJournal>| {
            let mut chunks = world.resource_mut::<ChunkStore>();
            for x in -10..=18 {
                for y in 3..=10 {
                    for z in -8..=20 {
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
                                props: Vec::new(),
                            })
                            .data
                            .set_block(local, if y == 3 { 3 } else { 0 })
                            .unwrap();
                    }
                }
            }
            let plan = terra_world::rails::plan(
                &chunks,
                &journal,
                [4, 4, 6],
                terra_core::rails::TURNTABLE,
                0,
            )
            .unwrap();
            terra_world::rails::commit(&mut chunks, &mut journal, &plan).unwrap();
        });
    let station = [18, 5, 6];
    {
        let mut chunks = app.world_mut().resource_mut::<ChunkStore>();
        let (owner, local) = terra_voxel::global_to_section(station);
        chunks
            .loaded
            .get_mut(&owner)
            .unwrap()
            .data
            .set_block(local, train_parts::STATION)
            .unwrap();
    }
    app.world_mut()
        .resource_mut::<EditJournal>()
        .record(station, train_parts::STATION)
        .unwrap();
    app.world_mut()
        .get_mut::<Transform>(peer.body)
        .unwrap()
        .translation = Vec3::new(18.5, 6.0, 6.5);
    let mut empty_seats = crate::server::trains::SeatedPlayers::default();
    for clockwise in [true, false] {
        assert_eq!(
            run(
                &mut app,
                &peer,
                TrainAction::RotateEmpty {
                    station,
                    anchor: [4, 4, 6],
                    clockwise
                },
                &mut empty_seats
            ),
            Ok(TrainOutcome::Changed { vehicle: None })
        );
    }
    assert_eq!(
        terra_world::rails::owner_at(app.world().resource::<EditJournal>(), [4, 4, 6])
            .unwrap()
            .facing,
        0
    );
    app.world_mut()
        .get_mut::<Transform>(peer.body)
        .unwrap()
        .translation = Vec3::new(6.5, 6.0, 6.5);
    app.world_mut()
        .resource_mut::<EditJournal>()
        .gameplay_mut()
        .trains
        .vehicles
        .insert(id, car);
    let mut seats = crate::server::trains::SeatedPlayers::default();
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
    let (position, _) = seats.pose(peer.uuid).unwrap();
    app.world_mut()
        .get_mut::<Transform>(peer.body)
        .unwrap()
        .translation = position;
    let before = app
        .world()
        .resource::<EditJournal>()
        .gameplay()
        .trains
        .vehicles[&id]
        .clone();
    let owner = peer.uuid;
    peer.uuid = PlayerUuid::from_u128(220_999);
    assert_eq!(
        run(
            &mut app,
            &peer,
            TrainAction::Turntable {
                vehicle: id,
                clockwise: true,
                station: None
            },
            &mut seats
        ),
        Err(TrainError::NotOwner)
    );
    peer.uuid = owner;
    assert_eq!(
        app.world()
            .resource::<EditJournal>()
            .gameplay()
            .trains
            .vehicles[&id],
        before
    );
    assert_eq!(
        run(
            &mut app,
            &peer,
            TrainAction::Turntable {
                vehicle: id,
                clockwise: true,
                station: None
            },
            &mut seats
        ),
        Ok(TrainOutcome::Changed { vehicle: Some(id) })
    );
    let journal = app.world().resource::<EditJournal>();
    let turned = &journal.gameplay().trains.vehicles[&id];
    assert!((turned.motion.yaw - std::f64::consts::FRAC_PI_2).abs() < 1e-6);
    assert_eq!(
        terra_world::rails::owner_at(journal, [4, 4, 6])
            .unwrap()
            .facing,
        1
    );
    assert_eq!(turned.cargo, before.cargo);
    let mut car = turned.clone();
    car.motion.speed_mps = 0.5;
    app.world_mut()
        .resource_mut::<EditJournal>()
        .gameplay_mut()
        .trains
        .vehicles
        .insert(id, car);
    let epoch = app.world().resource::<EditJournal>().rail_epoch();
    assert!(
        run(
            &mut app,
            &peer,
            TrainAction::Turntable {
                vehicle: id,
                clockwise: false,
                station: None
            },
            &mut seats
        )
        .is_err()
    );
    assert_eq!(app.world().resource::<EditJournal>().rail_epoch(), epoch);
}

#[test]
fn steam_water_transfer_and_gear_limits_are_owner_authorized_atomic_and_directional() {
    let (mut app, mut peer, id, seat, _) = train_fixture();
    {
        let mut journal = app.world_mut().resource_mut::<EditJournal>();
        let game = journal.gameplay_mut();
        game.inventories.entry(peer.uuid).or_default().creative_set(
            0,
            ItemStack {
                item: ItemKind::Tool(ToolKind::WaterBucket),
                count: 1,
            },
        );
        let car = game.trains.vehicles.get_mut(&id).unwrap();
        let engine = car.parts.len() as u16;
        car.parts.push(TrainPart {
            offset: [0, 2, 5],
            material: 218,
            facing: 0,
            wood: Default::default(),
        });
        car.cargo.insert(engine, Inventory::default());
        car.parts.push(TrainPart {
            offset: [0, 2, 2],
            material: 254,
            facing: 0,
            wood: Default::default(),
        });
        for z in 0..6 {
            car.parts.push(TrainPart {
                offset: [1, 1, z],
                material: 219,
                facing: 0,
                wood: Default::default(),
            });
        }
        for (offset, material) in [([1, 2, 0], 229), ([1, 2, 1], 230)] {
            car.parts.push(TrainPart {
                offset,
                material,
                facing: 0,
                wood: Default::default(),
            });
        }
        assert!(car.valid());
    }
    let mut seats = crate::server::trains::SeatedPlayers::default();
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
    let revision =
        app.world().resource::<EditJournal>().gameplay().inventories[&peer.uuid].revision;
    let command = TrainAction::FillWater {
        vehicle: id,
        player_revision: revision,
    };
    let owner = peer.uuid;
    peer.uuid = PlayerUuid::from_u128(999);
    let before = app.world().resource::<EditJournal>().gameplay().clone();
    assert_eq!(
        run(&mut app, &peer, command.clone(), &mut seats),
        Err(TrainError::NotOwner)
    );
    assert_eq!(app.world().resource::<EditJournal>().gameplay(), &before);
    peer.uuid = owner;
    run(&mut app, &peer, command.clone(), &mut seats).unwrap();
    let game = app.world().resource::<EditJournal>().gameplay();
    assert_eq!(game.trains.vehicles[&id].steam.water, 1000.0);
    assert_eq!(
        game.inventories[&owner].count(ItemKind::Tool(ToolKind::WaterBucket)),
        0
    );
    assert_eq!(
        game.inventories[&owner].count(ItemKind::Tool(ToolKind::Bucket)),
        1
    );
    assert_eq!(
        run(&mut app, &peer, command, &mut seats),
        Err(TrainError::Stale)
    );
    // This cab faces opposite the build axis: negative world throttle is forward.
    assert_eq!(
        run(
            &mut app,
            &peer,
            TrainAction::Drive {
                vehicle: id,
                throttle: -6,
                brake: false
            },
            &mut seats
        ),
        Err(TrainError::GearLocked)
    );
    app.world_mut()
        .resource_mut::<EditJournal>()
        .gameplay_mut()
        .trains
        .vehicles
        .get_mut(&id)
        .unwrap()
        .steam
        .pressure_bar = 9.0;
    run(
        &mut app,
        &peer,
        TrainAction::Drive {
            vehicle: id,
            throttle: -6,
            brake: false,
        },
        &mut seats,
    )
    .unwrap();
    assert_eq!(
        run(
            &mut app,
            &peer,
            TrainAction::Drive {
                vehicle: id,
                throttle: 3,
                brake: false
            },
            &mut seats
        ),
        Err(TrainError::GearLocked)
    );
    run(
        &mut app,
        &peer,
        TrainAction::Drive {
            vehicle: id,
            throttle: 2,
            brake: true,
        },
        &mut seats,
    )
    .unwrap();
    peer.mode = Some(GameMode::Creative);
    app.world_mut()
        .resource_mut::<EditJournal>()
        .gameplay_mut()
        .trains
        .vehicles
        .get_mut(&id)
        .unwrap()
        .steam
        .water = 7900.0;
    let inventory = app.world().resource::<EditJournal>().gameplay().inventories[&owner].clone();
    run(
        &mut app,
        &peer,
        TrainAction::FillWater {
            vehicle: id,
            player_revision: inventory.revision,
        },
        &mut seats,
    )
    .unwrap();
    let game = app.world().resource::<EditJournal>().gameplay();
    assert_eq!(
        game.trains.vehicles[&id].steam.water,
        terra_core::train_power::WATER_CAPACITY
    );
    assert_eq!(game.inventories[&owner], inventory);
}

#[test]
fn destroying_is_owner_authorized_reach_checked_revision_checked_and_atomic() {
    let (mut app, mut peer, id, _, _) = train_fixture();
    let mut seats = crate::server::trains::SeatedPlayers::default();
    let before = app.world().resource::<EditJournal>().gameplay().clone();
    let revision = before.trains.vehicles[&id].revision;
    assert_eq!(
        run(
            &mut app,
            &peer,
            TrainAction::Destroy {
                vehicle: id,
                revision: revision + 1
            },
            &mut seats
        ),
        Err(TrainError::Stale)
    );
    assert_eq!(*app.world().resource::<EditJournal>().gameplay(), before);
    let owner = peer.uuid;
    peer.uuid = terra_core::PlayerUuid::from_u128(222999);
    assert_eq!(
        run(
            &mut app,
            &peer,
            TrainAction::Destroy {
                vehicle: id,
                revision
            },
            &mut seats
        ),
        Err(TrainError::NotOwner)
    );
    peer.uuid = owner;
    let near = app.world().get::<Transform>(peer.body).unwrap().translation;
    app.world_mut()
        .get_mut::<Transform>(peer.body)
        .unwrap()
        .translation = Vec3::splat(300.0);
    assert_eq!(
        run(
            &mut app,
            &peer,
            TrainAction::Destroy {
                vehicle: id,
                revision
            },
            &mut seats
        ),
        Err(TrainError::OutOfReach)
    );
    assert_eq!(*app.world().resource::<EditJournal>().gameplay(), before);
    app.world_mut()
        .get_mut::<Transform>(peer.body)
        .unwrap()
        .translation = near;
    let edits = app.world().resource::<EditJournal>().edited_blocks();
    assert_eq!(
        run(
            &mut app,
            &peer,
            TrainAction::Destroy {
                vehicle: id,
                revision
            },
            &mut seats
        ),
        Ok(TrainOutcome::Changed { vehicle: None })
    );
    let journal = app.world().resource::<EditJournal>();
    assert!(journal.gameplay().trains.vehicles.is_empty());
    assert_eq!(journal.gameplay().revision, before.revision + 1);
    assert_eq!(
        journal.edited_blocks(),
        edits,
        "destruction must not restore world blocks"
    );
    assert_eq!(journal.gameplay().inventories, before.inventories);
    assert_eq!(
        run(
            &mut app,
            &peer,
            TrainAction::Destroy {
                vehicle: id,
                revision
            },
            &mut seats
        ),
        Err(TrainError::Missing)
    );
}

#[test]
fn capturing_a_seated_player_preserves_the_last_standing_save_position() {
    let (mut app, mut peer, id, seat, _) = train_fixture();
    let mut seats = crate::server::trains::SeatedPlayers::default();
    assert!(matches!(
        run(
            &mut app,
            &peer,
            TrainAction::Seat {
                vehicle: id,
                part: seat
            },
            &mut seats
        ),
        Ok(TrainOutcome::Seated(Some(_)))
    ));
    let last_standing = terra_core::PlayerLocation {
        position: [0.5, 6.0, 0.5],
        look: [0.0; 2],
        flying: false,
    };
    let uuid = peer.uuid;
    app.world_mut()
        .resource_mut::<EditJournal>()
        .update_player_location(uuid, last_standing)
        .unwrap();
    peer.spawned = true;
    peer.accepted = true;
    app.world_mut()
        .get_entity_mut(peer.body)
        .unwrap()
        .insert(crate::server::ServerCharacter {
            link: Entity::PLACEHOLDER,
        });
    app.world_mut().spawn(peer);
    app.insert_resource(seats);
    app.world_mut()
        .run_system_cached(crate::server::locations::capture)
        .unwrap();
    assert_eq!(
        app.world().resource::<EditJournal>().player_location(uuid),
        Some(last_standing)
    );
}

#[test]
fn vehicle_workshop_can_couple_and_release_nearby_owned_cars_without_a_station() {
    use terra_core::trains::CouplerEnd;
    let (mut app, peer, first, _, _) = train_fixture();
    let mut other = app
        .world()
        .resource::<EditJournal>()
        .gameplay()
        .trains
        .vehicles[&first]
        .clone();
    other.anchor[2] += 9;
    let second = app
        .world_mut()
        .resource_mut::<EditJournal>()
        .gameplay_mut()
        .trains
        .insert(other)
        .unwrap();
    let section = terra_voxel::global_to_section([4, 5, 20]).0;
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
            props: Vec::new(),
        });
    app.world_mut()
        .get_mut::<Transform>(peer.body)
        .unwrap()
        .translation = Vec3::new(6.5, 6., 11.5);
    let mut seated = crate::server::trains::SeatedPlayers::default();
    let revision = app
        .world()
        .resource::<EditJournal>()
        .gameplay()
        .trains
        .revision;
    let action = TrainAction::Couple {
        station: None,
        first,
        first_end: CouplerEnd::Front,
        second,
        second_end: CouplerEnd::Rear,
        revision,
    };
    assert!(run(&mut app, &peer, action.clone(), &mut seated).is_ok());
    assert!(
        app.world()
            .resource::<EditJournal>()
            .gameplay()
            .trains
            .vehicles[&first]
            .couplings[0]
            .is_some()
    );
    assert_eq!(
        run(&mut app, &peer, action, &mut seated),
        Err(TrainError::Stale)
    );
    let revision = app
        .world()
        .resource::<EditJournal>()
        .gameplay()
        .trains
        .revision;
    let action = TrainAction::Uncouple {
        station: None,
        vehicle: first,
        end: CouplerEnd::Front,
        revision,
    };
    // Opening an old UI remotely must not authorize a later operation.
    app.world_mut()
        .get_mut::<Transform>(peer.body)
        .unwrap()
        .translation = Vec3::splat(100.);
    assert_eq!(
        run(&mut app, &peer, action.clone(), &mut seated),
        Err(TrainError::OutOfReach)
    );
    app.world_mut()
        .get_mut::<Transform>(peer.body)
        .unwrap()
        .translation = Vec3::new(6.5, 6., 11.5);
    assert!(run(&mut app, &peer, action, &mut seated).is_ok());
    assert!(
        app.world()
            .resource::<EditJournal>()
            .gameplay()
            .trains
            .vehicles[&first]
            .couplings[0]
            .is_none()
    );
}

fn builder_fixture() -> (App, Peer, u64, u16, terra_world::rails::NetworkCache) {
    let (mut app, peer, id, _, cargo) = train_fixture();
    app.world_mut()
        .get_mut::<Transform>(peer.body)
        .unwrap()
        .translation = Vec3::new(14.0, 7.0, 14.0);
    app.world_mut()
        .resource_scope(|world, mut journal: Mut<EditJournal>| {
            let mut chunks = world.resource_mut::<ChunkStore>();
            for x in -2..=14 {
                for z in -2..=22 {
                    for y in 3..=12 {
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
                            .set_block(local, if y <= 4 { 3 } else { 0 })
                            .unwrap();
                    }
                }
            }
            journal
                .gameplay_mut()
                .trains
                .vehicles
                .get_mut(&id)
                .unwrap()
                .anchor[1] = 6;
            let track = terra_world::rails::plan(
                &chunks,
                &journal,
                [4, 5, 3],
                terra_core::rails::STRAIGHT,
                0,
            )
            .unwrap();
            terra_world::rails::commit(&mut chunks, &mut journal, &track).unwrap();
            let car = journal.gameplay_mut().trains.vehicles.get_mut(&id).unwrap();
            car.parts.push(TrainPart {
                offset: [0, 1, 8],
                material: train_parts::RAIL_BUILDER,
                facing: 2,
                wood: WoodSpecies::Oak,
            });
            car.motion.cursor = Some(terra_core::train_motion::RailCursor {
                anchor: [4, 5, 3],
                path: 0,
                distance: 2.0,
                forward: true,
            });
            car.cargo.get_mut(&cargo).unwrap().slots = vec![
                ItemStack {
                    item: ItemKind::for_wood(196, WoodSpecies::Cherry),
                    count: 4,
                },
                ItemStack {
                    item: ItemKind::Block(206),
                    count: 64,
                },
            ];
        });
    let mut graph = terra_world::rails::NetworkCache::default();
    graph
        .refresh(app.world().resource::<EditJournal>())
        .unwrap();
    (app, peer, id, cargo, graph)
}
fn build_step(
    app: &mut App,
    peer: &Peer,
    id: u64,
    graph: &terra_world::rails::NetworkCache,
) -> Result<(), TrainError> {
    let car = app
        .world()
        .resource::<EditJournal>()
        .gameplay()
        .trains
        .vehicles[&id]
        .clone();
    let mut state = SystemState::<ControlWorld>::new(app.world_mut());
    let mut world = state.get_mut(app.world_mut()).unwrap();
    crate::server::train_builder::step(&car, peer, &mut world, graph.graph())
}
#[test]
fn rail_builder_survival_consumes_cargo_but_creative_does_not() {
    for creative in [false, true] {
        let (mut app, mut peer, id, cargo, graph) = builder_fixture();
        peer.mode = Some(if creative {
            GameMode::Creative
        } else {
            GameMode::Survival
        });
        let before = app
            .world()
            .resource::<EditJournal>()
            .gameplay()
            .trains
            .vehicles[&id]
            .cargo[&cargo]
            .clone();
        assert_eq!(build_step(&mut app, &peer, id, &graph), Ok(()));
        let journal = app.world().resource::<EditJournal>();
        let bag = &journal.gameplay().trains.vehicles[&id].cargo[&cargo];
        if creative {
            assert_eq!(bag, &before);
        } else {
            assert_eq!(bag.count(ItemKind::for_wood(196, WoodSpecies::Cherry)), 3);
            assert!(bag.count(ItemKind::Block(206)) < 64);
            assert!(!journal.gameplay().drops.is_empty());
        }
        let mut new_graph = terra_world::rails::NetworkCache::default();
        new_graph.refresh(journal).unwrap();
        assert_eq!(new_graph.graph().len(), 2);
        assert!(journal.gameplay().trains.vehicles[&id].valid());
    }
}
#[test]
fn rail_builder_missing_materials_or_a_second_locomotive_changes_nothing() {
    let (mut app, peer, id, cargo, graph) = builder_fixture();
    let obstacle = [4, 8, 12];
    let (section, local) = terra_voxel::global_to_section(obstacle);
    app.world_mut()
        .resource_mut::<ChunkStore>()
        .loaded
        .get_mut(&section)
        .unwrap()
        .data
        .set_block(local, 3)
        .unwrap();
    let drops_before = app
        .world()
        .resource::<EditJournal>()
        .gameplay()
        .drops
        .clone();
    app.world_mut()
        .resource_mut::<EditJournal>()
        .gameplay_mut()
        .trains
        .vehicles
        .get_mut(&id)
        .unwrap()
        .cargo
        .get_mut(&cargo)
        .unwrap()
        .slots[1]
        .count = 0;
    let before = app
        .world()
        .resource::<EditJournal>()
        .gameplay()
        .trains
        .clone();
    let epoch = app.world().resource::<EditJournal>().rail_epoch();
    assert_eq!(
        build_step(&mut app, &peer, id, &graph),
        Err(TrainError::MissingMaterials)
    );
    assert_eq!(
        app.world().resource::<EditJournal>().gameplay().trains,
        before
    );
    assert_eq!(app.world().resource::<EditJournal>().rail_epoch(), epoch);
    assert_eq!(
        app.world().resource::<ChunkStore>().block_at(obstacle),
        Some(3)
    );
    assert_eq!(
        app.world().resource::<EditJournal>().gameplay().drops,
        drops_before
    );
    let mut registry = before.clone();
    let mut extra = registry.vehicles[&id].clone();
    extra.id = 2;
    extra.couplings[0] = Some(terra_core::trains::Coupling {
        vehicle: id,
        end: terra_core::trains::CouplerEnd::Rear,
    });
    registry.vehicles.get_mut(&id).unwrap().couplings[1] = Some(terra_core::trains::Coupling {
        vehicle: 2,
        end: terra_core::trains::CouplerEnd::Front,
    });
    registry.vehicles.insert(2, extra);
    assert_eq!(
        terra_world::rails::builder::validate_consist(&registry, &registry.vehicles[&id]),
        Err(TrainError::Limit)
    );
}

#[test]
fn rail_clearer_removes_five_by_nine_ahead_preserves_edges_and_drops_stacks() {
    for creative in [false, true] {
        let (mut app, mut peer, id, _, _) = builder_fixture();
        app.world_mut()
            .resource_mut::<EditJournal>()
            .gameplay_mut()
            .trains
            .vehicles
            .get_mut(&id)
            .unwrap()
            .motion
            .rail_clearance
            .height = 9;
        peer.mode = Some(if creative {
            GameMode::Creative
        } else {
            GameMode::Survival
        });
        // An entity on the route must not veto removal; moving train impact handles damage.
        app.world_mut()
            .get_mut::<Transform>(peer.body)
            .unwrap()
            .translation = Vec3::new(4.5, 6.0, 12.5);
        {
            let mut chunks = app.world_mut().resource_mut::<ChunkStore>();
            for x in 1..=7 {
                for y in 5..=14 {
                    for z in 12..=14 {
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
        }
        let before_bags = app
            .world()
            .resource::<EditJournal>()
            .gameplay()
            .trains
            .vehicles[&id]
            .cargo
            .clone();
        let car = app
            .world()
            .resource::<EditJournal>()
            .gameplay()
            .trains
            .vehicles[&id]
            .clone();
        let mut state = SystemState::<ControlWorld>::new(app.world_mut());
        let mut world = state.get_mut(app.world_mut()).unwrap();
        assert_eq!(
            crate::server::train_builder::clear_front(&car, &peer, &mut world),
            Ok(())
        );
        for x in 1..=7 {
            for y in 5..=14 {
                for z in 12..=14 {
                    let inside = (2..=6).contains(&x) && y < 14 && z < 14;
                    assert_eq!(
                        world.chunks.block_at([x, y, z]),
                        Some(if inside { 0 } else { 3 }),
                        "{x},{y},{z}"
                    );
                }
            }
        }
        assert_eq!(
            world.journal.gameplay().trains.vehicles[&id].cargo,
            before_bags
        );
        let drops = &world.journal.gameplay().drops;
        let stones: u32 = drops
            .iter()
            .filter(|drop| drop.stack.item == ItemKind::Block(3))
            .map(|drop| u32::from(drop.stack.count))
            .sum();
        assert_eq!(stones, if creative { 0 } else { 90 });
        assert!(drops.len() < 10);
    }
}

#[test]
fn rail_clearer_drop_overflow_leaves_the_obstacle_intact() {
    let (mut app, mut peer, id, _, _) = builder_fixture();
    peer.mode = Some(GameMode::Survival);
    let cell = [4, 8, 12];
    let (section, local) = terra_voxel::global_to_section(cell);
    app.world_mut()
        .resource_mut::<ChunkStore>()
        .loaded
        .get_mut(&section)
        .unwrap()
        .data
        .set_block(local, 3)
        .unwrap();
    app.world_mut()
        .resource_mut::<EditJournal>()
        .gameplay_mut()
        .next_drop = u64::MAX;
    let car = app
        .world()
        .resource::<EditJournal>()
        .gameplay()
        .trains
        .vehicles[&id]
        .clone();
    let mut state = SystemState::<ControlWorld>::new(app.world_mut());
    let mut world = state.get_mut(app.world_mut()).unwrap();
    assert_eq!(
        crate::server::train_builder::clear_front(&car, &peer, &mut world),
        Err(TrainError::Overflow)
    );
    assert_eq!(world.chunks.block_at(cell), Some(3));
}

#[test]
fn seated_driver_enables_builder_then_scheduled_drive_clears_and_extends_real_tracks() {
    use crate::server::{train_builder, trains};
    use lightyear::prelude::{Connected, PeerId, RemoteId};

    for creative in [false, true] {
        let (mut app, mut peer, id, cargo, _) = builder_fixture();
        peer.mode = Some(if creative {
            GameMode::Creative
        } else {
            GameMode::Survival
        });
        app.world_mut()
            .get_mut::<Transform>(peer.body)
            .unwrap()
            .translation = Vec3::new(6.5, 7.0, 6.5);
        let obstacle = [4, 8, 12];
        app.world_mut()
            .resource_scope(|world, mut journal: Mut<EditJournal>| {
                let mut chunks = world.resource_mut::<ChunkStore>();
                for x in -2..=14 {
                    for z in 23..=40 {
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
                                .set_block(local, if y <= 4 { 3 } else { 0 })
                                .unwrap();
                        }
                    }
                }
                // Both bogies must rest on a genuine continuous route; the old helper
                // tests supplied a cursor despite having only one short rail section.
                for z in [7, 11] {
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
                let (section, local) = terra_voxel::global_to_section(obstacle);
                chunks
                    .loaded
                    .get_mut(&section)
                    .unwrap()
                    .data
                    .set_block(local, 3)
                    .unwrap();
                let car = journal.gameplay_mut().trains.vehicles.get_mut(&id).unwrap();
                car.motion.cursor = None;
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
        let mut seats = trains::SeatedPlayers::default();
        let enable = TrainAction::RailBuilder {
            vehicle: id,
            active: true,
        };
        assert_eq!(
            run(&mut app, &peer, enable.clone(), &mut seats),
            Err(TrainError::OutOfReach)
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
        run(&mut app, &peer, enable, &mut seats).unwrap();
        assert!(
            app.world()
                .resource::<EditJournal>()
                .gameplay()
                .trains
                .vehicles[&id]
                .motion
                .rail_builder_active
        );
        let before = app
            .world()
            .resource::<EditJournal>()
            .gameplay()
            .trains
            .vehicles[&id]
            .cargo[&cargo]
            .clone();
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
        .unwrap();
        assert_eq!(
            app.world().resource::<ChunkStore>().block_at(obstacle),
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
        for _ in 0..60 {
            app.update();
        }
        let journal = app.world().resource::<EditJournal>();
        let car = &journal.gameplay().trains.vehicles[&id];
        assert!(car.motion.wheel_distance_m > 0.2, "{:?}", car.motion);
        assert!(car.motion.speed_mps > 0.0 && car.motion.speed_mps <= 3.0);
        assert!(!car.motion.brake, "{:?}", car.motion);
        assert!(terra_world::rails::network(journal).unwrap().len() > 3);
        if creative {
            assert_eq!(car.cargo[&cargo], before);
        } else {
            assert!(car.cargo[&cargo].count(ItemKind::for_wood(196, WoodSpecies::Cherry)) < 4);
            assert!(car.cargo[&cargo].count(ItemKind::Block(206)) < 64);
        }
    }
}
