use super::*;
use client::{ConfirmedEdit, RailPlacementAction, RailPlacementRequest, RailPlacementSent};

#[derive(Resource, Default)]
struct Receipts {
    sent: Vec<RailPlacementSent>,
    confirmed: Vec<ConfirmedEdit>,
}
fn capture(
    mut receipts: ResMut<Receipts>,
    mut sent: MessageReader<RailPlacementSent>,
    mut confirmed: MessageReader<ConfirmedEdit>,
) {
    receipts.sent.extend(sent.read().copied());
    receipts.confirmed.extend(confirmed.read().copied());
}
fn applied(app: &App, token: u64) -> bool {
    let receipts = app.world().resource::<Receipts>();
    let Some(sent) = receipts.sent.iter().find(|sent| sent.token == token) else {
        return false;
    };
    let Some(confirmed) = receipts
        .confirmed
        .iter()
        .find(|confirmed| confirmed.request == sent.request && confirmed.action == sent.action)
    else {
        return false;
    };
    assert_eq!(
        confirmed.outcome,
        EditOutcome::Applied,
        "token {token}, request {}",
        sent.request
    );
    true
}
fn clear_air(host: &mut App, cells: impl IntoIterator<Item = [i64; 3]>) {
    host.world_mut()
        .resource_scope(|world, mut journal: Mut<EditJournal>| {
            let mut chunks = world.resource_mut::<ChunkStore>();
            for position in cells {
                let expected = chunks
                    .block_at(position)
                    .expect("fixture cells must already be loaded");
                if expected != 0 {
                    assert!(
                        commit_block_edit(
                            &mut chunks,
                            &mut journal,
                            &BlockEditRequest {
                                position,
                                expected_material: expected,
                                material: 0
                            },
                            &Default::default()
                        )
                        .unwrap()
                    );
                }
            }
        });
}

#[test]
fn udp_seed_receipt_waits_for_the_real_three_tile_starter_before_stretching_and_late_joining() {
    let mut host = server();
    let address = host
        .world()
        .resource::<server::ServerRuntime>()
        .address
        .unwrap();
    let mut first = profile_client(
        address,
        PlayerProfile {
            name: "RailBuilder".into(),
            ..Default::default()
        },
    );
    first
        .init_resource::<Receipts>()
        .add_systems(Update, capture);
    let mut watcher = profile_client(
        address,
        PlayerProfile {
            name: "SeedWatcher".into(),
            ..Default::default()
        },
    );
    wait_phase(
        "seed clients grounded",
        &mut host,
        &mut [&mut first, &mut watcher],
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
    let state = first
        .world()
        .resource::<client::ClientSession>()
        .confirmed_state()
        .unwrap();
    first
        .world_mut()
        .spawn(terra_physics::character::body_bundle(Vec3::from_array(
            state.position,
        )));
    let ground =
        (state.position[1] - terra_physics::character::PLAYER_HALF_HEIGHT).floor() as i64 - 1;
    let anchor = [
        state.position[0].floor() as i64,
        ground + 1,
        state.position[2].floor() as i64 + 3,
    ];
    let starter = Piece::seed(anchor, 0).unwrap();
    let wood = WoodSpecies::Spruce;
    host.insert_resource(FixtureGeometry {
        original: starter,
        curve: None,
    });
    let end_floor = [anchor[0], ground, anchor[2] + 7];
    let seed_feet = [
        anchor[0] as f32 + 2.5,
        (ground + 1) as f32,
        anchor[2] as f32 - 2.5,
    ];
    let end_feet = [
        anchor[0] as f32 + 2.5,
        (ground + 1) as f32,
        anchor[2] as f32 + 6.0,
    ];
    let air_cells: Vec<_> = (-1..=1)
        .flat_map(|x| (0..=8).map(move |z| [anchor[0] + x, ground + 1, anchor[2] + z]))
        .collect();
    let floors: std::collections::BTreeSet<_> = air_cells
        .iter()
        .map(|cell| [cell[0], ground, cell[2]])
        .chain(
            [seed_feet, end_feet]
                .map(|feet| [feet[0].floor() as i64, ground, feet[2].floor() as i64]),
        )
        .collect();
    wait_phase(
        "seed fixture streaming",
        &mut host,
        &mut [&mut first, &mut watcher],
        |host, _| {
            let chunks = host.world().resource::<ChunkStore>();
            air_cells
                .iter()
                .chain(floors.iter())
                .all(|cell| chunks.block_at(*cell).is_some())
        },
    );
    floor(&mut host, floors);
    clear_air(&mut host, air_cells);
    let uuid = first
        .world()
        .resource::<client::ClientSession>()
        .uuid
        .unwrap();
    let item = ItemKind::for_wood(terra_core::rails::STRAIGHT, wood);
    let stock = 10;
    {
        let mut journal = host.world_mut().resource_mut::<EditJournal>();
        assert!(
            journal
                .gameplay_mut()
                .inventories
                .entry(uuid)
                .or_default()
                .creative_set(8, ItemStack { item, count: stock })
        );
        journal.gameplay_mut().revision += 1;
    }
    host.world_mut()
        .resource_mut::<GameConfig>()
        .server
        .game_mode = GameMode::Survival;
    first
        .world_mut()
        .resource_mut::<client::SelectedInventorySlot>()
        .0 = Some(8);
    wait_phase(
        "seed stock and replica floor",
        &mut host,
        &mut [&mut first, &mut watcher],
        |host, clients| {
            clients[0]
                .world()
                .resource::<client::ClientSession>()
                .inventory
                .count(item)
                == stock
                && clients.iter().all(|app| {
                    app.world()
                        .resource::<ChunkStore>()
                        .block_at([anchor[0], ground, anchor[2]])
                        == Some(3)
                        && app.world().resource::<EditJournal>().revision(
                            terra_voxel::global_to_section([anchor[0], ground, anchor[2]]).0,
                        ) == host.world().resource::<EditJournal>().revision(
                            terra_voxel::global_to_section([anchor[0], ground, anchor[2]]).0,
                        )
                })
        },
    );
    teleport(
        "seed placement stance",
        &mut host,
        &mut first,
        &mut watcher,
        seed_feet,
    );
    let origin = eye(&first);
    let look = look_at(
        origin,
        Vec3::new(
            anchor[0] as f32 + 0.5,
            (ground + 1) as f32,
            anchor[2] as f32 + 0.5,
        ),
    );
    let hit = picked(&host, origin, look).expect("starter floor has a genuine unobstructed ray");
    assert_eq!(hit.position, [anchor[0], ground, anchor[2]]);
    assert_eq!(hit.normal, [0, 1, 0]);
    let initial = rails::seed(
        host.world().resource::<ChunkStore>(),
        host.world().resource::<EditJournal>(),
        hit,
        ray(origin, look),
        look,
        wood,
    )
    .unwrap();
    assert_eq!(initial.edits.len(), 3);
    assert_eq!(rails::cost(&initial), Some(1));
    assert_eq!(
        rails::next_endpoint(&initial),
        Some(Endpoint { anchor, port: 1 })
    );
    let state = initial
        .states
        .iter()
        .find(|(cell, _)| *cell == anchor)
        .unwrap()
        .1
        .unwrap();
    assert_eq!(
        rails::piece_from_state(anchor, starter.material, state),
        Some(starter)
    );
    first
        .world_mut()
        .resource_mut::<client::ClientSession>()
        .look = look;
    let seed_action = RailPlacementAction::Seed {
        position: hit.position,
        expected: 3,
        signature: rails::seed_signature(&initial),
    };
    first.world_mut().write_message(RailPlacementRequest {
        token: 209_001,
        action: seed_action,
    });
    assert!(
        !rails::complete(first.world().resource::<EditJournal>(), starter),
        "sending never publishes speculative rail cells"
    );
    wait_phase(
        "seed exact request applied",
        &mut host,
        &mut [&mut first, &mut watcher],
        |_, clients| applied(clients[0], 209_001),
    );
    wait_phase(
        "seed three tiles replicated",
        &mut host,
        &mut [&mut first, &mut watcher],
        |host, clients| {
            rails::complete(host.world().resource::<EditJournal>(), starter)
                && clients
                    .iter()
                    .all(|app| rails::complete(app.world().resource::<EditJournal>(), starter))
                && clients[0]
                    .world()
                    .resource::<client::ClientSession>()
                    .inventory
                    .count(item)
                    == stock - 1
        },
    );
    let sent = first
        .world()
        .resource::<Receipts>()
        .sent
        .iter()
        .find(|sent| sent.token == 209_001)
        .copied()
        .unwrap();
    assert_eq!(sent.action, seed_action.edit_action());
    let destination = starter.ports()[1].position;
    let start = rails::continuation_endpoint(
        first.world().resource::<EditJournal>(),
        starter,
        destination,
    )
    .expect("only the fully replicated free starter end becomes the next start");
    assert_eq!(start, Endpoint { anchor, port: 1 });
    for app in [&host, &first, &watcher] {
        let journal = app.world().resource::<EditJournal>();
        assert_eq!(journal.rail_module_count(), 1);
        assert_eq!(rails::network(journal).unwrap().len(), 1);
        assert_eq!(rails::owner_at(journal, anchor), Some(starter));
        assert_eq!(rails::cells(starter).len(), 3);
        assert!(
            rails::cells(starter)
                .iter()
                .all(|cell| journal.object_state(*cell)
                    == Some(rails::object_state_with_wood(starter, *cell, wood)))
        );
        assert!(
            !rails::complete(
                journal,
                Piece::new(anchor, terra_core::rails::STRAIGHT, 0).unwrap()
            ),
            "no phantom legacy four-metre piece exists"
        );
    }
    teleport(
        "stretch destination stance",
        &mut host,
        &mut first,
        &mut watcher,
        end_feet,
    );
    let origin = eye(&first);
    let look = look_at(
        origin,
        Vec3::new(
            end_floor[0] as f32 + 0.5,
            (ground + 1) as f32,
            end_floor[2] as f32 + 0.5,
        ),
    );
    let hit = picked(&host, origin, look).expect("stretch destination uses the actual floor ray");
    assert_eq!(hit.position, end_floor);
    assert_eq!(hit.normal, [0, 1, 0]);
    let extension = rails::connection(
        host.world().resource::<ChunkStore>(),
        host.world().resource::<EditJournal>(),
        start,
        hit,
        look,
        wood,
    )
    .unwrap();
    let endpoint = rails::next_endpoint(&extension).unwrap();
    let state = extension
        .states
        .iter()
        .find(|(cell, _)| *cell == endpoint.anchor)
        .unwrap()
        .1
        .unwrap();
    let stretched =
        rails::piece_from_state(endpoint.anchor, terra_core::rails::STRAIGHT, state).unwrap();
    let cost = rails::cost(&extension).unwrap();
    assert!(stretched.paths()[0].length() > 4.0);
    assert!(cost > 1);
    host.world_mut().resource_mut::<FixtureGeometry>().curve = Some(stretched);
    first
        .world_mut()
        .resource_mut::<client::ClientSession>()
        .look = look;
    let connect_action = RailPlacementAction::Connect {
        start,
        position: hit.position,
        expected: 3,
    };
    first.world_mut().write_message(RailPlacementRequest {
        token: 209_002,
        action: connect_action,
    });
    wait_phase(
        "stretch exact request applied",
        &mut host,
        &mut [&mut first, &mut watcher],
        |_, clients| applied(clients[0], 209_002),
    );
    wait_phase(
        "stretch geometry and stock replicated",
        &mut host,
        &mut [&mut first, &mut watcher],
        |host, clients| {
            rails::complete(host.world().resource::<EditJournal>(), stretched)
                && clients
                    .iter()
                    .all(|app| rails::complete(app.world().resource::<EditJournal>(), stretched))
                && clients[0]
                    .world()
                    .resource::<client::ClientSession>()
                    .inventory
                    .count(item)
                    == stock - 1 - cost
        },
    );
    let receipts = first.world().resource::<Receipts>();
    let second = receipts
        .sent
        .iter()
        .find(|sent| sent.token == 209_002)
        .unwrap();
    assert_ne!(second.request, sent.request);
    assert_eq!(second.action, connect_action.edit_action());
    let late_token = host
        .world()
        .resource::<server::ServerRuntime>()
        .issue_token(209_9001)
        .unwrap();
    let mut late = client(late_token);
    wait_phase(
        "seed and stretch late join",
        &mut host,
        &mut [&mut first, &mut watcher, &mut late],
        |_, clients| {
            let app = &clients[2];
            app.world().resource::<client::ClientSession>().ready
                && rails::complete(app.world().resource::<EditJournal>(), starter)
                && rails::complete(app.world().resource::<EditJournal>(), stretched)
        },
    );
    let graph = rails::network(host.world().resource::<EditJournal>()).unwrap();
    assert_eq!(graph.len(), 2);
    for app in [&first, &watcher, &late] {
        let journal = app.world().resource::<EditJournal>();
        assert_eq!(rails::network(journal).unwrap(), graph);
        assert!(
            rails::cells(starter)
                .into_iter()
                .chain(rails::cells(stretched))
                .all(|cell| journal.object_state(cell).unwrap().wood == wood)
        );
        let end = stretched.ports()[1].position;
        assert_eq!(
            rails::continuation_endpoint(journal, stretched, end),
            Some(endpoint)
        );
        assert_eq!(
            rails::continuation_endpoint(journal, starter, destination),
            None
        );
    }
    assert_eq!(
        graph
            .route(Endpoint { anchor, port: 0 }, endpoint, 16)
            .unwrap()
            .legs
            .len(),
        2
    );
}
