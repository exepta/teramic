//! Real reliable-channel assembly, private cargo, seating and late-join replication.
use super::*;
use crate::protocol::trains::{TrainAction, TrainOutcome};
use terra_core::{
    Inventory, train_parts,
    trains::{TrainError, VehicleKind},
};

fn put(host: &mut App, position: [i64; 3], material: u16) {
    host.world_mut()
        .resource_scope(|world, mut journal: Mut<EditJournal>| {
            let mut chunks = world.resource_mut::<ChunkStore>();
            let expected = chunks.block_at(position).unwrap();
            if expected != material {
                assert!(
                    commit_block_edit(
                        &mut chunks,
                        &mut journal,
                        &BlockEditRequest {
                            position,
                            expected_material: expected,
                            material
                        },
                        &Default::default()
                    )
                    .unwrap()
                );
            }
        });
}
fn send(app: &mut App, action: TrainAction) -> u64 {
    let request = app
        .world_mut()
        .resource_mut::<client::ClientSession>()
        .train_request(action)
        .unwrap();
    let id = request.0.request;
    app.world_mut().write_message(request);
    id
}
fn response(host: &mut App, owner: &mut App, watcher: &mut App, id: u64) -> TrainOutcome {
    wait_phase(
        "train command response",
        host,
        &mut [owner, watcher],
        |_, clients| {
            clients[0]
                .world()
                .resource::<client::ClientSession>()
                .last_train_result
                .as_ref()
                .is_some_and(|result| result.request == id)
        },
    );
    owner
        .world()
        .resource::<client::ClientSession>()
        .last_train_result
        .as_ref()
        .unwrap()
        .outcome
        .clone()
}

#[test]
fn udp_real_vehicle_assembly_private_cargo_seating_late_join_and_atomic_dismantling() {
    let coal = ItemKind::Mineral(terra_core::MineralItem::Coal);
    let mut host = server();
    let address = host
        .world()
        .resource::<server::ServerRuntime>()
        .address
        .unwrap();
    let mut owner = profile_client(
        address,
        PlayerProfile {
            name: "RailBuilder".into(),
            ..Default::default()
        },
    );
    let mut watcher = profile_client(
        address,
        PlayerProfile {
            name: "TrainObserver".into(),
            ..Default::default()
        },
    );
    wait_phase(
        "train clients ready",
        &mut host,
        &mut [&mut owner, &mut watcher],
        |_, clients| {
            clients.iter().all(|app| {
                app.world().resource::<client::ClientSession>().ready
                    && app
                        .world()
                        .resource::<client::ClientSession>()
                        .confirmed_state()
                        .is_some_and(|state| state.grounded)
            })
        },
    );
    let state = owner
        .world()
        .resource::<client::ClientSession>()
        .confirmed_state()
        .unwrap();
    owner
        .world_mut()
        .spawn(terra_physics::character::body_bundle(Vec3::from_array(
            state.position,
        )));
    let ground =
        (state.position[1] - terra_physics::character::PLAYER_HALF_HEIGHT).floor() as i64 - 1;
    let base = [
        state.position[0].floor() as i64,
        ground + 1,
        state.position[2].floor() as i64 + 3,
    ];
    let at = |x, y, z| [base[0] + x, base[1] + y, base[2] + z];
    let station = at(3, 2, 2);
    let start = at(0, 2, 2);
    let cargo_cell = at(0, 3, 4);
    let chest_cell = at(-1, 2, 2);
    let slab_cell = at(-1, 2, 3);
    let engine_cell = at(1, 2, 3);
    let tint = [31, 92, 211];
    let volume: Vec<_> = (-3..=6)
        .flat_map(|x| (0..=6).flat_map(move |y| (-2..=9).map(move |z| at(x, y, z))))
        .collect();
    wait_phase(
        "train real assembly area streamed",
        &mut host,
        &mut [&mut owner, &mut watcher],
        |host, _| {
            volume.iter().all(|cell| {
                host.world()
                    .resource::<ChunkStore>()
                    .block_at(*cell)
                    .is_some()
            })
        },
    );
    for cell in &volume {
        put(&mut host, *cell, 0);
    }
    floor(
        &mut host,
        (-3..=6).flat_map(|x| (-2..=9).map(move |z| at(x, -1, z))),
    );
    host.world_mut()
        .resource_scope(|world, mut journal: Mut<EditJournal>| {
            let mut chunks = world.resource_mut::<ChunkStore>();
            for piece in [
                Piece::new(base, terra_core::rails::STRAIGHT, 0).unwrap(),
                Piece::new(at(0, 0, 4), terra_core::rails::STRAIGHT, 0).unwrap(),
            ] {
                let plan = rails::plan(&chunks, &journal, piece.anchor, piece.material, 0).unwrap();
                assert!(rails::commit(&mut chunks, &mut journal, &plan).unwrap());
            }
        });
    let mut parts = Vec::new();
    for z in 0..=6 {
        parts.push((
            at(0, 2, z),
            if z == 2 {
                train_parts::CHASSIS
            } else {
                train_parts::FRAME
            },
        ));
    }
    parts.extend([
        (at(0, 1, 1), train_parts::BOGIE),
        (at(0, 1, 5), train_parts::BOGIE),
        (at(0, 2, -1), train_parts::COUPLER),
        (at(0, 2, 7), train_parts::COUPLER),
        (at(0, 3, 3), train_parts::CAB),
        (cargo_cell, train_parts::CARGO),
        (chest_cell, 29),
        (slab_cell, 105),
        (engine_cell, terra_core::locomotive_parts::ENGINE),
    ]);
    for (cell, material) in &parts {
        put(&mut host, *cell, *material);
        host.world_mut()
            .resource_mut::<EditJournal>()
            .set_object_state(
                *cell,
                Some(terra_world::edits::BlockObjectState {
                    wood: WoodSpecies::Birch,
                    ..Default::default()
                }),
            )
            .unwrap();
    }
    {
        let mut journal = host.world_mut().resource_mut::<EditJournal>();
        journal
            .set_object_state(
                slab_cell,
                Some(terra_world::edits::BlockObjectState {
                    facing: 5,
                    wood: WoodSpecies::Cherry,
                    ..Default::default()
                }),
            )
            .unwrap();
        journal.set_tint(slab_cell, Some(tint)).unwrap();
        journal
            .set_container(engine_cell, Inventory::default())
            .unwrap();
        journal
            .set_container(
                chest_cell,
                Inventory {
                    slots: vec![ItemStack {
                        item: ItemKind::Block(13),
                        count: 17,
                    }],
                    revision: 6,
                    ..Default::default()
                },
            )
            .unwrap();
    }
    for y in -1..=1 {
        put(&mut host, at(3, y, 2), 3);
    }
    put(&mut host, station, train_parts::STATION);
    let uuid = owner
        .world()
        .resource::<client::ClientSession>()
        .uuid
        .unwrap();
    {
        let mut journal = host.world_mut().resource_mut::<EditJournal>();
        journal
            .set_container(
                cargo_cell,
                Inventory {
                    slots: vec![ItemStack {
                        item: ItemKind::Block(3),
                        count: 42,
                    }],
                    ..Default::default()
                },
            )
            .unwrap();
        assert!(
            journal
                .gameplay_mut()
                .inventories
                .entry(uuid)
                .or_default()
                .creative_set(
                    0,
                    ItemStack {
                        item: ItemKind::Block(3),
                        count: 5
                    }
                )
        );
        assert!(
            journal
                .gameplay_mut()
                .inventories
                .get_mut(&uuid)
                .unwrap()
                .creative_set(
                    8,
                    ItemStack {
                        item: coal,
                        count: 3
                    },
                )
        );
        journal.gameplay_mut().revision += 1;
    }
    wait_phase(
        "train assembly parts and player stock replicated",
        &mut host,
        &mut [&mut owner, &mut watcher],
        |_, clients| {
            clients.iter().all(|app| {
                app.world().resource::<ChunkStore>().block_at(station) == Some(train_parts::STATION)
            }) && clients[0]
                .world()
                .resource::<client::ClientSession>()
                .inventory
                .count(ItemKind::Block(3))
                == 5
                && clients[0]
                    .world()
                    .resource::<client::ClientSession>()
                    .inventory
                    .count(coal)
                    == 3
        },
    );
    let feet = [
        base[0] as f32 + 5.5,
        (ground + 1) as f32,
        base[2] as f32 + 2.5,
    ];
    teleport(
        "train station stance",
        &mut host,
        &mut owner,
        &mut watcher,
        feet,
    );
    let revision = owner
        .world()
        .resource::<client::ClientSession>()
        .trains
        .revision;
    let id = send(
        &mut owner,
        TrainAction::Assemble {
            station,
            start,
            kind: VehicleKind::Locomotive,
            revision,
        },
    );
    let TrainOutcome::Changed {
        vehicle: Some(vehicle_id),
    } = response(&mut host, &mut owner, &mut watcher, id)
    else {
        panic!("assembly must be accepted");
    };
    wait_phase(
        "train assembled on both peers",
        &mut host,
        &mut [&mut owner, &mut watcher],
        |host, clients| {
            host.world()
                .resource::<EditJournal>()
                .gameplay()
                .trains
                .vehicles
                .contains_key(&vehicle_id)
                && clients.iter().all(|app| {
                    app.world()
                        .resource::<client::ClientSession>()
                        .trains
                        .vehicles
                        .iter()
                        .any(|car| car.id == vehicle_id)
                        && parts.iter().all(|(cell, _)| {
                            app.world().resource::<ChunkStore>().block_at(*cell) == Some(0)
                        })
                })
        },
    );
    assert!(
        watcher
            .world()
            .resource::<client::ClientSession>()
            .train_cargo
            .is_none()
    );
    let rename = send(
        &mut owner,
        TrainAction::Name {
            vehicle: vehicle_id,
            name: "TERAMIC EXPRESS".into(),
            station: Some(station),
        },
    );
    assert!(
        matches!(response(&mut host,&mut owner,&mut watcher,rename),TrainOutcome::Changed{vehicle:Some(id)} if id==vehicle_id)
    );
    wait_phase(
        "train sign replicated to both clients",
        &mut host,
        &mut [&mut owner, &mut watcher],
        |_, clients| {
            clients.iter().all(|client| {
                client
                    .world()
                    .resource::<client::ClientSession>()
                    .trains
                    .vehicles
                    .iter()
                    .any(|car| car.id == vehicle_id && car.name == "TERAMIC EXPRESS")
            })
        },
    );
    let car = host
        .world()
        .resource::<EditJournal>()
        .gameplay()
        .trains
        .vehicles[&vehicle_id]
        .clone();
    assert_eq!(car.owner, uuid);
    assert_eq!(car.parts.len(), parts.len());
    let cargo = car
        .parts
        .iter()
        .position(|part| part.material == train_parts::CARGO)
        .unwrap() as u16;
    let chest = car
        .parts
        .iter()
        .position(|part| part.material == 29)
        .unwrap() as u16;
    let slab = car
        .parts
        .iter()
        .position(|part| part.material == 105)
        .unwrap() as u16;
    assert_eq!(car.cargo[&chest].count(ItemKind::Block(13)), 17);
    for client in [&owner, &watcher] {
        let session = client.world().resource::<client::ClientSession>();
        let view = session
            .trains
            .vehicles
            .iter()
            .find(|view| view.id == vehicle_id)
            .unwrap();
        assert_eq!(view.parts[slab as usize].facing, 5);
        assert_eq!(view.parts[slab as usize].wood, WoodSpecies::Cherry);
        assert_eq!(view.blocks[&slab].tint, Some(tint));
        assert_eq!(view.blocks, car.blocks);
        let public = serde_json::to_value(view).unwrap();
        assert!(public.get("cargo").is_none());
        assert!(public.get("payloads").is_none());
    }
    let seat = car
        .parts
        .iter()
        .position(|part| part.material == train_parts::CAB)
        .unwrap() as u16;
    let engine = car
        .parts
        .iter()
        .position(|part| part.material == 218)
        .unwrap() as u16;
    let forbidden = send(
        &mut watcher,
        TrainAction::OpenCargo {
            vehicle: vehicle_id,
            part: chest,
        },
    );
    wait_phase(
        "foreign train cargo rejected",
        &mut host,
        &mut [&mut owner, &mut watcher],
        |_, clients| {
            clients[1]
                .world()
                .resource::<client::ClientSession>()
                .last_train_result
                .as_ref()
                .is_some_and(|result| result.request == forbidden)
        },
    );
    assert!(matches!(
        watcher
            .world()
            .resource::<client::ClientSession>()
            .last_train_result
            .as_ref()
            .unwrap()
            .outcome,
        TrainOutcome::Rejected(TrainError::NotOwner)
    ));
    let cargo_feet = [
        base[0] as f32 + 2.5,
        (ground + 1) as f32,
        base[2] as f32 + 4.5,
    ];
    teleport(
        "train chest accessible side stance",
        &mut host,
        &mut owner,
        &mut watcher,
        [
            base[0] as f32 - 1.7,
            (ground + 1) as f32,
            base[2] as f32 + 2.5,
        ],
    );
    let open_chest = send(
        &mut owner,
        TrainAction::OpenCargo {
            vehicle: vehicle_id,
            part: chest,
        },
    );
    let chest_outcome = response(&mut host, &mut owner, &mut watcher, open_chest);
    let TrainOutcome::Cargo(chest_snapshot) = chest_outcome else {
        panic!("ordinary mounted chest must open from its accessible side: {chest_outcome:?}");
    };
    assert_eq!(chest_snapshot.inventory.count(ItemKind::Block(13)), 17);
    teleport(
        "train cargo stance",
        &mut host,
        &mut owner,
        &mut watcher,
        cargo_feet,
    );
    let open = send(
        &mut owner,
        TrainAction::OpenCargo {
            vehicle: vehicle_id,
            part: cargo,
        },
    );
    let TrainOutcome::Cargo(snapshot) = response(&mut host, &mut owner, &mut watcher, open) else {
        panic!("owner must open real cargo");
    };
    assert_eq!(snapshot.inventory.count(ItemKind::Block(3)), 42);
    let player_revision = owner
        .world()
        .resource::<client::ClientSession>()
        .inventory
        .revision;
    let transfer = send(
        &mut owner,
        TrainAction::Cargo {
            vehicle: vehicle_id,
            part: cargo,
            revision: snapshot.revision,
            player_revision,
            gesture: terra_core::containers::ContainerGesture::Wheel {
                slot: terra_core::INVENTORY_SLOTS as u8,
                to_cursor: false,
            },
            reserved: [0; terra_core::INVENTORY_SLOTS],
        },
    );
    let TrainOutcome::Cargo(updated) = response(&mut host, &mut owner, &mut watcher, transfer)
    else {
        panic!("cargo gesture must be atomic");
    };
    assert_eq!(updated.inventory.count(ItemKind::Block(3)), 41);
    assert_eq!(
        owner
            .world()
            .resource::<client::ClientSession>()
            .inventory
            .count(ItemKind::Block(3)),
        6
    );
    assert!(
        owner
            .world()
            .resource::<client::ClientSession>()
            .inventory
            .cursor
            .is_none()
    );
    assert!(
        watcher
            .world()
            .resource::<client::ClientSession>()
            .train_cargo
            .is_none()
    );
    teleport(
        "train cab accessible side stance",
        &mut host,
        &mut owner,
        &mut watcher,
        [
            base[0] as f32 - 2.5,
            (ground + 1) as f32,
            base[2] as f32 + 3.5,
        ],
    );
    let board = send(
        &mut owner,
        TrainAction::Seat {
            vehicle: vehicle_id,
            part: seat,
        },
    );
    let board_outcome = response(&mut host, &mut owner, &mut watcher, board);
    assert!(
        matches!(board_outcome,TrainOutcome::Seated(Some(binding)) if binding.vehicle==vehicle_id && binding.part==seat),
        "CAB boarding from its accessible side must succeed: {board_outcome:?}"
    );
    wait_phase(
        "train seat authoritative snapshot",
        &mut host,
        &mut [&mut owner, &mut watcher],
        |_, clients| {
            clients[0]
                .world()
                .resource::<client::ClientSession>()
                .confirmed_state()
                .is_some_and(|state| {
                    state
                        .train_seat
                        .is_some_and(|binding| binding.vehicle == vehicle_id)
                })
        },
    );
    let foreign_drive = send(
        &mut watcher,
        TrainAction::Drive {
            vehicle: vehicle_id,
            throttle: 1,
            brake: false,
        },
    );
    wait_phase(
        "foreign train driving rejected",
        &mut host,
        &mut [&mut owner, &mut watcher],
        |_, clients| {
            clients[1]
                .world()
                .resource::<client::ClientSession>()
                .last_train_result
                .as_ref()
                .is_some_and(|result| result.request == foreign_drive)
        },
    );
    assert!(matches!(
        watcher
            .world()
            .resource::<client::ClientSession>()
            .last_train_result
            .as_ref()
            .unwrap()
            .outcome,
        TrainOutcome::Rejected(TrainError::NotOwner)
    ));
    let drive = send(
        &mut owner,
        TrainAction::Drive {
            vehicle: vehicle_id,
            throttle: 1,
            brake: false,
        },
    );
    assert!(
        matches!(response(&mut host, &mut owner, &mut watcher, drive),
        TrainOutcome::Changed { vehicle: Some(id) } if id == vehicle_id)
    );
    let open_engine = send(
        &mut owner,
        TrainAction::OpenCargo {
            vehicle: vehicle_id,
            part: engine,
        },
    );
    let TrainOutcome::Cargo(empty_engine) =
        response(&mut host, &mut owner, &mut watcher, open_engine)
    else {
        panic!("the bound owner must open the empty engine");
    };
    assert_eq!(empty_engine.inventory.count(coal), 0);
    let player_revision = owner
        .world()
        .resource::<client::ClientSession>()
        .inventory
        .revision;
    let refuel = send(
        &mut owner,
        TrainAction::Cargo {
            vehicle: vehicle_id,
            part: engine,
            revision: empty_engine.revision,
            player_revision,
            gesture: terra_core::containers::ContainerGesture::QuickMove { slot: 8 },
            reserved: [0; terra_core::INVENTORY_SLOTS],
        },
    );
    let TrainOutcome::Cargo(refuelled) = response(&mut host, &mut owner, &mut watcher, refuel)
    else {
        panic!("one player-stack click must transfer actual coal into the engine");
    };
    assert_eq!(refuelled.inventory.count(coal), 3);
    wait_phase(
        "actual rail motion reaches owner and observer",
        &mut host,
        &mut [&mut owner, &mut watcher],
        |_, clients| {
            clients.iter().all(|client| {
                client
                    .world()
                    .resource::<client::ClientSession>()
                    .trains
                    .vehicles
                    .iter()
                    .find(|view| view.id == vehicle_id)
                    .is_some_and(|view| {
                        view.motion.speed_mps > 0.1 && view.motion.wheel_distance_m > 0.01
                    })
            }) && clients[0]
                .world()
                .resource::<client::ClientSession>()
                .train_cargo
                .as_ref()
                .is_some_and(|cargo| {
                    cargo.vehicle == vehicle_id
                        && cargo.part == engine
                        && cargo.inventory.count(coal) == 2
                })
                && clients[0]
                    .world()
                    .resource::<client::ClientSession>()
                    .inventory
                    .count(coal)
                    == 0
        },
    );
    let brake = send(
        &mut owner,
        TrainAction::Drive {
            vehicle: vehicle_id,
            throttle: 0,
            brake: true,
        },
    );
    assert!(
        matches!(response(&mut host, &mut owner, &mut watcher, brake),
        TrainOutcome::Changed { vehicle: Some(id) } if id == vehicle_id)
    );
    wait_phase(
        "rail brake stops the actual moving model",
        &mut host,
        &mut [&mut owner, &mut watcher],
        |host, clients| {
            host.world()
                .resource::<EditJournal>()
                .gameplay()
                .trains
                .vehicles[&vehicle_id]
                .motion
                .speed_mps
                .abs()
                < 0.001
                && clients.iter().all(|client| {
                    client
                        .world()
                        .resource::<client::ClientSession>()
                        .trains
                        .vehicles
                        .iter()
                        .find(|view| view.id == vehicle_id)
                        .is_some_and(|view| {
                            view.motion.speed_mps.abs() < 0.001 && view.motion.brake
                        })
                })
        },
    );
    let driven = &host
        .world()
        .resource::<EditJournal>()
        .gameplay()
        .trains
        .vehicles[&vehicle_id];
    assert!(driven.motion.wheel_distance_m > 0.01);
    assert!(driven.burn_seconds > 0.0);
    assert_eq!(driven.cargo[&engine].count(coal), 2);
    assert!(
        owner
            .world()
            .resource::<client::ClientSession>()
            .train_cargo
            .as_ref()
            .is_some_and(|cargo| cargo.part == engine && cargo.inventory.count(coal) == 2),
        "braking and full public snapshots must preserve the open private fuel panel"
    );
    let restored = driven.disassembly_cells().unwrap();
    let cargo_cell = restored[usize::from(cargo)];
    let chest_cell = restored[usize::from(chest)];
    let slab_cell = restored[usize::from(slab)];
    let engine_cell = restored[usize::from(engine)];
    let parts = restored
        .into_iter()
        .zip(driven.parts.iter().map(|part| part.material))
        .collect::<Vec<_>>();
    assert!(
        watcher
            .world()
            .resource::<client::ClientSession>()
            .train_cargo
            .is_none()
    );
    let leave = send(&mut owner, TrainAction::Leave);
    assert!(matches!(
        response(&mut host, &mut owner, &mut watcher, leave),
        TrainOutcome::Seated(None)
    ));
    wait_phase(
        "train leave confirmed",
        &mut host,
        &mut [&mut owner, &mut watcher],
        |host, clients| {
            clients[0]
                .world()
                .resource::<client::ClientSession>()
                .train_seat
                .is_none()
                && clients[0]
                    .world()
                    .resource::<client::ClientSession>()
                    .trains
                    .revision
                    == host
                        .world()
                        .resource::<EditJournal>()
                        .gameplay()
                        .trains
                        .revision
        },
    );
    let mut late = profile_client(
        address,
        PlayerProfile {
            name: "TrainLateJoin".into(),
            ..Default::default()
        },
    );
    wait_phase(
        "late train public geometry",
        &mut host,
        &mut [&mut owner, &mut watcher, &mut late],
        |_, clients| {
            clients[2].world().resource::<client::ClientSession>().ready
                && clients[2]
                    .world()
                    .resource::<client::ClientSession>()
                    .trains
                    .vehicles
                    .iter()
                    .any(|car| car.id == vehicle_id)
        },
    );
    assert!(
        late.world()
            .resource::<client::ClientSession>()
            .train_cargo
            .is_none()
    );
    {
        let session = late.world().resource::<client::ClientSession>();
        let view = session
            .trains
            .vehicles
            .iter()
            .find(|view| view.id == vehicle_id)
            .unwrap();
        assert_eq!(view.blocks[&slab].tint, Some(tint));
        assert_eq!(view.parts[slab as usize].facing, 5);
    }
    teleport(
        "train dismantling stance",
        &mut host,
        &mut owner,
        &mut watcher,
        feet,
    );
    let revision = owner
        .world()
        .resource::<client::ClientSession>()
        .trains
        .vehicles
        .iter()
        .find(|car| car.id == vehicle_id)
        .unwrap()
        .revision;
    let dismantle = send(
        &mut owner,
        TrainAction::Dismantle {
            station,
            vehicle: vehicle_id,
            revision,
        },
    );
    assert!(
        matches!(response(&mut host,&mut owner,&mut watcher,dismantle),TrainOutcome::Changed{vehicle:Some(id)} if id==vehicle_id)
    );
    wait_phase(
        "train restored on owner observer and late join",
        &mut host,
        &mut [&mut owner, &mut watcher, &mut late],
        |_, clients| {
            clients.iter().all(|app| {
                app.world()
                    .resource::<client::ClientSession>()
                    .trains
                    .vehicles
                    .is_empty()
                    && parts.iter().all(|(cell, material)| {
                        app.world().resource::<ChunkStore>().block_at(*cell) == Some(*material)
                    })
            })
        },
    );
    for client in [&owner, &watcher, &late] {
        let journal = client.world().resource::<EditJournal>();
        assert_eq!(journal.object_state(slab_cell).unwrap().facing, 5);
        assert_eq!(
            journal.object_state(slab_cell).unwrap().wood,
            WoodSpecies::Cherry
        );
        assert_eq!(journal.tint_at(slab_cell), Some(tint));
    }
    assert_eq!(
        host.world()
            .resource::<EditJournal>()
            .container_at(chest_cell)
            .unwrap()
            .count(ItemKind::Block(13)),
        17
    );
    assert_eq!(
        host.world()
            .resource::<EditJournal>()
            .container_at(cargo_cell),
        Some(&updated.inventory)
    );
    assert_eq!(
        host.world()
            .resource::<EditJournal>()
            .container_at(engine_cell)
            .unwrap()
            .count(coal),
        2
    );
    assert!(
        host.world()
            .resource::<EditJournal>()
            .furnace_at(engine_cell)
            .is_some_and(|furnace| furnace.burn_remaining > 0.0)
    );
    assert_eq!(
        host.world()
            .resource::<EditJournal>()
            .gameplay()
            .inventories[&uuid]
            .count(ItemKind::Block(3)),
        6
    );
    assert!(
        host.world()
            .resource::<EditJournal>()
            .gameplay()
            .inventories[&uuid]
            .cursor
            .is_none()
    );
}

#[test]
fn bounded_full_train_registry_public_packet_stays_below_two_mebibytes() {
    use std::collections::BTreeMap;
    use terra_core::trains::{SeatBinding, TrainBlockView, TrainPart, TrainSnapshot, VehicleView};
    let parts: Vec<_> = (0..16)
        .flat_map(|x| {
            (0..16).map(move |z| TrainPart {
                offset: [x, 0, z],
                material: match (x, z) {
                    (0, 0) => train_parts::CHASSIS,
                    (0, 1 | 2) => train_parts::BOGIE,
                    (0, 3) => train_parts::COUPLER,
                    _ => train_parts::CAB,
                },
                facing: 0,
                wood: WoodSpecies::Cherry,
            })
        })
        .collect();
    let blocks: BTreeMap<_, _> = (0..256)
        .map(|index| {
            (
                index,
                TrainBlockView {
                    boxes: vec![
                        terra_core::block_shapes::ShapeBox {
                            minimum: [0.0; 3],
                            maximum: [1.0; 3],
                            hinged: false,
                        };
                        4
                    ],
                    tint: Some([255; 3]),
                    heraldry: Some(terra_core::kingdom::KingdomHeraldry {
                        color: [255; 3],
                        icon: terra_core::kingdom::KingdomIcon::Catalog(u64::MAX),
                    }),
                    ..Default::default()
                },
            )
        })
        .collect();
    let vehicles = (1..=16)
        .map(|id| VehicleView {
            name: String::new(),
            id,
            owner: terra_core::PlayerUuid::from_u128(1),
            kind: VehicleKind::Locomotive,
            anchor: [id as i64 * 100, 8, 0],
            parts: parts.clone(),
            blocks: blocks.clone(),
            couplings: [None; 2],
            revision: u64::MAX,
            mass_kg: 999_999_999,

            motion: Default::default(),
        })
        .collect();
    let seats = (0..1024)
        .map(|index| SeatBinding {
            player: terra_core::PlayerUuid::from_u128(index + 1),
            vehicle: index as u64 / 64 + 1,
            part: (index as u16 % 64) + 4,
        })
        .collect();
    let snapshot = TrainSnapshot {
        revision: u64::MAX,
        vehicles,
        seats,
    };
    assert!(snapshot.valid());
    let bytes = postcard::to_allocvec(&snapshot).unwrap();
    assert!(bytes.len() < 2 * 1024 * 1024, "{} bytes", bytes.len());
    assert_eq!(
        postcard::from_bytes::<TrainSnapshot>(&bytes).unwrap(),
        snapshot
    );
}

#[test]
fn udp_creative_workshop_builds_the_real_template_with_empty_inventory_on_both_clients() {
    let mut host = server();
    let address = host
        .world()
        .resource::<server::ServerRuntime>()
        .address
        .unwrap();
    let mut owner = profile_client(
        address,
        PlayerProfile {
            name: "AutoBuilder".into(),
            ..Default::default()
        },
    );
    let mut watcher = profile_client(
        address,
        PlayerProfile {
            name: "BuildObserver".into(),
            ..Default::default()
        },
    );
    wait_phase(
        "auto-build clients ready",
        &mut host,
        &mut [&mut owner, &mut watcher],
        |_, clients| {
            clients.iter().all(|app| {
                app.world().resource::<client::ClientSession>().ready
                    && app
                        .world()
                        .resource::<client::ClientSession>()
                        .confirmed_state()
                        .is_some_and(|state| state.grounded)
            })
        },
    );
    let state = owner
        .world()
        .resource::<client::ClientSession>()
        .confirmed_state()
        .unwrap();
    let ground =
        (state.position[1] - terra_physics::character::PLAYER_HALF_HEIGHT).floor() as i64 - 1;
    let base = [
        state.position[0].floor() as i64,
        ground + 1,
        state.position[2].floor() as i64 + 3,
    ];
    let at = |x, y, z| [base[0] + x, base[1] + y, base[2] + z];
    let station = at(3, 2, 2);
    let chassis = at(0, 2, 2);
    let volume: Vec<_> = (-3..=6)
        .flat_map(|x| (0..=7).flat_map(move |y| (-2..=10).map(move |z| at(x, y, z))))
        .collect();
    wait_phase(
        "auto-build area loaded",
        &mut host,
        &mut [&mut owner, &mut watcher],
        |host, _| {
            volume.iter().all(|cell| {
                host.world()
                    .resource::<ChunkStore>()
                    .block_at(*cell)
                    .is_some()
            })
        },
    );
    for cell in volume {
        put(&mut host, cell, 0);
    }
    floor(
        &mut host,
        (-3..=6).flat_map(|x| (-2..=10).map(move |z| at(x, -1, z))),
    );
    host.world_mut()
        .resource_scope(|world, mut journal: Mut<EditJournal>| {
            let mut chunks = world.resource_mut::<ChunkStore>();
            for piece in [
                Piece::new(base, terra_core::rails::STRAIGHT, 0).unwrap(),
                Piece::new(at(0, 0, 4), terra_core::rails::STRAIGHT, 0).unwrap(),
            ] {
                let plan = rails::plan(&chunks, &journal, piece.anchor, piece.material, 0).unwrap();
                rails::commit(&mut chunks, &mut journal, &plan).unwrap();
            }
        });
    for y in 0..=1 {
        put(&mut host, at(3, y, 2), 3);
    }
    put(&mut host, station, train_parts::STATION);
    let player_revision = owner
        .world()
        .resource::<client::ClientSession>()
        .inventory
        .revision;
    let request = send(
        &mut owner,
        TrainAction::Build {
            station,
            chassis,
            facing: 0,
            kind: VehicleKind::Locomotive,
            wheelbase: 4,
            player_revision,
        },
    );
    assert_eq!(
        response(&mut host, &mut owner, &mut watcher, request),
        TrainOutcome::Changed { vehicle: None }
    );
    let guide =
        terra_core::train_blueprint::building_template(chassis, 0, VehicleKind::Locomotive, 4);
    wait_phase(
        "auto-built template replicated",
        &mut host,
        &mut [&mut owner, &mut watcher],
        |_, clients| {
            clients.iter().all(|app| {
                guide.iter().all(|part| {
                    app.world().resource::<ChunkStore>().block_at(part.cell) == Some(part.material)
                })
            })
        },
    );
    assert!(
        owner
            .world()
            .resource::<client::ClientSession>()
            .inventory
            .slots
            .iter()
            .all(|stack| stack.count == 0)
    );
    let player_revision = owner
        .world()
        .resource::<client::ClientSession>()
        .inventory
        .revision;
    let again = send(
        &mut owner,
        TrainAction::Build {
            station,
            chassis,
            facing: 0,
            kind: VehicleKind::Locomotive,
            wheelbase: 4,
            player_revision,
        },
    );
    assert_eq!(
        response(&mut host, &mut owner, &mut watcher, again),
        TrainOutcome::Changed { vehicle: None }
    );
}
