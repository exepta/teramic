use super::*;
use crate::protocol::session::{EditOutcome, MiningIntent, MiningTarget};
use terra_core::{
    GameMode, ItemKind, ItemStack, PlayerProfile, ToolKind, WoodSpecies,
    rails::{Endpoint, Piece},
};
use terra_world::{
    edits::EditJournal,
    rails,
    streaming::{BlockEditRequest, commit_block_edit},
};

#[path = "railway_context_tests.rs"]
mod context;

#[derive(Resource, Clone, Copy)]
struct FixtureGeometry {
    original: Piece,
    curve: Option<Piece>,
}

fn wait_phase(
    label: &str,
    host: &mut App,
    clients: &mut [&mut App],
    condition: impl Fn(&App, &[&mut App]) -> bool,
) {
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        host.update();
        for client in clients.iter_mut() {
            client.update();
            let session = client.world().resource::<client::ClientSession>();
            assert!(
                session.error.is_none(),
                "rail phase {label}: {:?}",
                session.error
            );
        }
        if condition(host, clients) {
            return;
        }
        if Instant::now() >= deadline {
            let chunks = host.world().resource::<ChunkStore>();
            let journal = host.world().resource::<EditJournal>();
            eprintln!(
                "rail phase={label}; server_error={:?}; console={:?}; loaded={}; modules={}",
                host.world().resource::<server::ServerRuntime>().error,
                host.world().resource::<server::ServerConsole>().output,
                chunks.loaded.len(),
                journal.rail_module_count()
            );
            if let Some(geometry) = host.world().get_resource::<FixtureGeometry>() {
                for piece in std::iter::once(geometry.original).chain(geometry.curve) {
                    let mut materials = std::collections::BTreeMap::new();
                    let mut missing = Vec::new();
                    let mut occupied = Vec::new();
                    let mut missing_columns = Vec::new();
                    for cell in rails::cells(piece) {
                        let material = chunks.block_at(cell);
                        *materials.entry(material).or_insert(0_usize) += 1;
                        if material.is_none() && missing.len() < 12 {
                            missing.push(cell);
                        }
                        if material
                            .is_some_and(|material| material != 0 && material != piece.material)
                            && occupied.len() < 12
                        {
                            occupied.push((cell, material));
                        }
                        if chunks
                            .prepared_column([cell[0].div_euclid(16), cell[2].div_euclid(16)])
                            .is_none()
                            && missing_columns.len() < 12
                        {
                            missing_columns.push(cell);
                        }
                    }
                    eprintln!(
                        "piece={piece:?}; complete={}; materials={materials:?}; missing={missing:?}; occupied={occupied:?}; missing_columns={missing_columns:?}",
                        rails::complete(journal, piece)
                    );
                }
            }
            for (index, client) in clients.iter().enumerate() {
                let session = client.world().resource::<client::ClientSession>();
                eprintln!(
                    "client{index}: ready={} teleporting={} state={:?} selected={:?} slot8={:?} last_edit={:?} modules={} loaded={}",
                    session.ready,
                    session.teleporting,
                    session.confirmed_state().map(|state| (
                        state.generation,
                        state.position,
                        state.grounded
                    )),
                    client.world().resource::<client::SelectedInventorySlot>().0,
                    session.inventory.slot(8),
                    session.last_edit,
                    client.world().resource::<EditJournal>().rail_module_count(),
                    client.world().resource::<ChunkStore>().loaded.len()
                );
            }
            panic!("rail network condition timed out in phase: {label}");
        }
        std::thread::sleep(Duration::from_millis(1));
    }
}

fn eye(app: &App) -> Vec3 {
    Vec3::from_array(
        app.world()
            .resource::<client::ClientSession>()
            .confirmed_state()
            .unwrap()
            .position,
    ) + Vec3::Y * terra_physics::character::PLAYER_EYE_OFFSET
}
fn look_at(eye: Vec3, target: Vec3) -> [f32; 2] {
    let direction = (target - eye).normalize();
    [(-direction.x).atan2(-direction.z), direction.y.asin()]
}
fn ray(eye: Vec3, look: [f32; 2]) -> ([f64; 3], [f64; 3]) {
    (
        eye.to_array().map(f64::from),
        (Quat::from_euler(EulerRot::YXZ, look[0], look[1], 0.0) * Vec3::NEG_Z)
            .to_array()
            .map(f64::from),
    )
}
fn picked(host: &App, eye: Vec3, look: [f32; 2]) -> Option<terra_voxel::blocks::BlockHit> {
    let journal = host.world().resource::<EditJournal>();
    let config = host.world().resource::<GameConfig>();
    let pose = terra_world::props::PropPose::WeatherSwept {
        frame: terra_world::props::weather_wind_frame(
            host.world().resource::<terra_core::WorldWeather>().wind(),
        ),
    }
    .with_season(
        journal
            .clock
            .unwrap_or_default()
            .season(config.world.seasons),
    );
    let (origin, direction) = ray(eye, look);
    let (_, target) = terra_world::targeting::pick_world_for_mining(
        host.world().resource::<ChunkStore>(),
        journal,
        host.world()
            .resource::<terra_core::block_properties::BlockPropertiesCatalog>(),
        origin,
        direction,
        pose,
    );
    match target {
        Some(terra_world::targeting::WorldTarget::Block(hit)) => Some(hit),
        _ => None,
    }
}
fn teleport(phase: &str, host: &mut App, first: &mut App, watcher: &mut App, feet: [f32; 3]) {
    let generation = first
        .world()
        .resource::<client::ClientSession>()
        .confirmed_state()
        .unwrap()
        .generation;
    assert!(
        host.world_mut()
            .resource_mut::<server::ServerConsole>()
            .submit(format!(
                "tp RailBuilder {} {} {}",
                feet[0], feet[1], feet[2]
            ))
    );
    wait_phase(phase, host, &mut [first, watcher], |_, clients| {
        let session = clients[0].world().resource::<client::ClientSession>();
        session.ready
            && !session.teleporting
            && session.confirmed_state().is_some_and(|state| {
                state.generation > generation
                    && state.grounded
                    && (state.position[0] - feet[0]).abs() < 0.2
                    && (state.position[2] - feet[2]).abs() < 0.2
            })
    });
}
fn floor(host: &mut App, positions: impl IntoIterator<Item = [i64; 3]>) {
    let positions: std::collections::BTreeSet<_> = positions.into_iter().collect();
    host.world_mut()
        .resource_scope(|world, mut journal: Mut<EditJournal>| {
            let mut chunks = world.resource_mut::<ChunkStore>();
            for position in positions {
                let expected = chunks
                    .block_at(position)
                    .expect("fixture support must be genuinely loaded");
                if expected != 3 {
                    assert!(
                        commit_block_edit(
                            &mut chunks,
                            &mut journal,
                            &BlockEditRequest {
                                position,
                                expected_material: expected,
                                material: 3
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
fn udp_free_connection_uses_a_selected_real_port_costs_wood_units_and_replicates_to_late_join_before_secondary_mining()
 {
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
    let mut watcher = profile_client(
        address,
        PlayerProfile {
            name: "RailWatcher".into(),
            ..Default::default()
        },
    );
    wait_phase(
        "initial clients grounded",
        &mut host,
        &mut [&mut first, &mut watcher],
        |_, clients| {
            clients.iter().all(|client| {
                let session = client.world().resource::<client::ClientSession>();
                session.ready
                    && session
                        .confirmed_state()
                        .is_some_and(|state| state.grounded)
            })
        },
    );
    // Minimal transport clients do not install the player presentation plugin.
    // The normal teleport completion/reconciliation systems need a real body,
    // just as the existing headless felling/teleport fixtures do.
    let initial = first
        .world()
        .resource::<client::ClientSession>()
        .confirmed_state()
        .unwrap()
        .position;
    first
        .world_mut()
        .spawn(terra_physics::character::body_bundle(Vec3::from_array(
            initial,
        )));
    let state = first
        .world()
        .resource::<client::ClientSession>()
        .confirmed_state()
        .unwrap();
    let ground =
        (state.position[1] - terra_physics::character::PLAYER_HALF_HEIGHT).floor() as i64 - 1;
    let original = Piece::new(
        [
            state.position[0].floor() as i64,
            ground + 1,
            state.position[2].floor() as i64 + 3,
        ],
        terra_core::rails::STRAIGHT,
        0,
    )
    .unwrap();
    let wood = WoodSpecies::Spruce;
    host.insert_resource(FixtureGeometry {
        original,
        curve: None,
    });
    let start = Endpoint {
        anchor: original.anchor,
        port: 1,
    };
    let end_floor = [original.anchor[0] + 4, ground, original.anchor[2] + 17];
    let source_feet = [
        original.anchor[0] as f32 - 2.5,
        ground as f32 + 1.0,
        original.anchor[2] as f32 + 3.0,
    ];
    let end_feet = [
        end_floor[0] as f32 + 2.5,
        ground as f32 + 1.0,
        end_floor[2] as f32 - 2.5,
    ];
    wait_phase(
        "original support streaming",
        &mut host,
        &mut [&mut first, &mut watcher],
        |host, _| {
            let chunks = host.world().resource::<ChunkStore>();
            rails::cells(original)
                .iter()
                .all(|cell| chunks.block_at(*cell).is_some())
                && original.supports().iter().all(|point| {
                    chunks
                        .block_at(std::array::from_fn(|axis| {
                            original.anchor[axis] + i64::from(point.support[axis])
                        }))
                        .is_some()
                })
        },
    );
    floor(
        &mut host,
        original.supports().iter().map(|point| {
            std::array::from_fn(|axis| original.anchor[axis] + i64::from(point.support[axis]))
        }),
    );
    host.world_mut()
        .resource_scope(|world, mut journal: Mut<EditJournal>| {
            let mut chunks = world.resource_mut::<ChunkStore>();
            let mut plan = rails::plan(
                &chunks,
                &journal,
                original.anchor,
                original.material,
                original.encoded_facing(),
            )
            .unwrap();
            rails::set_wood(&mut plan, wood);
            assert!(rails::commit(&mut chunks, &mut journal, &plan).unwrap());
        });
    let proposed_eye = Vec3::from_array(end_feet)
        + Vec3::Y
            * (terra_physics::character::PLAYER_HALF_HEIGHT
                + terra_physics::character::PLAYER_EYE_OFFSET);
    let endpoint_target = Vec3::new(
        end_floor[0] as f32 + 0.5,
        ground as f32 + 1.0,
        end_floor[2] as f32 + 0.5,
    );
    let look = look_at(proposed_eye, endpoint_target);
    let proposed_hit = terra_voxel::blocks::BlockHit {
        position: end_floor,
        normal: [0, 1, 0],
        distance: 4.0,
    };
    let proposal = rails::proposal_connection(
        host.world().resource::<ChunkStore>(),
        host.world().resource::<EditJournal>(),
        start,
        proposed_hit,
        look,
        wood,
    )
    .unwrap();
    let (anchor, state) = proposal
        .states
        .iter()
        .find_map(|(cell, state)| {
            state
                .filter(|state| !state.secondary)
                .map(|state| (*cell, state))
        })
        .unwrap();
    let curve = rails::piece_from_state(anchor, terra_core::rails::STRAIGHT, state).unwrap();
    host.world_mut().resource_mut::<FixtureGeometry>().curve = Some(curve);
    assert!(curve.curve.is_some());
    assert!(curve.paths()[0].length() > 14.0);
    let cells = rails::cells(curve);
    let cost = rails::piece_cost(curve);
    assert!(cost >= 4);
    wait_phase(
        "curve space and stance streaming",
        &mut host,
        &mut [&mut first, &mut watcher],
        |host, _| {
            let chunks = host.world().resource::<ChunkStore>();
            cells.iter().all(|cell| chunks.block_at(*cell) == Some(0))
                && curve.supports().iter().all(|point| {
                    chunks
                        .block_at(std::array::from_fn(|axis| {
                            curve.anchor[axis] + i64::from(point.support[axis])
                        }))
                        .is_some()
                })
                && [source_feet, end_feet].iter().all(|feet| {
                    chunks
                        .block_at([feet[0].floor() as i64, ground, feet[2].floor() as i64])
                        .is_some()
                })
        },
    );
    let supports = curve.supports();
    floor(
        &mut host,
        supports
            .iter()
            .map(|point| {
                std::array::from_fn(|axis| curve.anchor[axis] + i64::from(point.support[axis]))
            })
            .chain(
                [source_feet, end_feet]
                    .map(|feet| [feet[0].floor() as i64, ground, feet[2].floor() as i64]),
            ),
    );
    teleport(
        "source teleport",
        &mut host,
        &mut first,
        &mut watcher,
        source_feet,
    );
    let source_eye = eye(&first);
    let mut selected = None;
    for cell in rails::cells(original) {
        for bounds in rails::cell_boxes(
            cell,
            original.material,
            rails::object_state_with_wood(original, cell, wood),
        )
        .iter()
        {
            let point = Vec3::from_array(std::array::from_fn(|axis| {
                cell[axis] as f32 + ((bounds.minimum[axis] + bounds.maximum[axis]) * 0.5) as f32
            }));
            let look = look_at(source_eye, point);
            if let Some(hit) = picked(&host, source_eye, look)
                && rails::select_endpoint(
                    host.world().resource::<EditJournal>(),
                    hit,
                    ray(source_eye, look),
                ) == Some(start)
            {
                selected = Some(hit);
                break;
            }
        }
        if selected.is_some() {
            break;
        }
    }
    assert!(
        selected.is_some(),
        "the connection start must first be selected on a real visible port within 4.5m"
    );
    teleport(
        "destination teleport",
        &mut host,
        &mut first,
        &mut watcher,
        end_feet,
    );
    let origin = eye(&first);
    let look = look_at(origin, endpoint_target);
    let hit = picked(&host, origin, look).expect("end foundation needs a real unobstructed ray");
    assert_eq!(hit.position, end_floor);
    assert_eq!(hit.normal, [0, 1, 0]);
    assert!(hit.distance <= 4.5);
    let validated = rails::connection(
        host.world().resource::<ChunkStore>(),
        host.world().resource::<EditJournal>(),
        start,
        hit,
        look,
        wood,
    )
    .unwrap();
    assert_eq!(rails::cost(&validated), Some(cost));
    let terminal = rails::connection_target(&validated)
        .unwrap()
        .position
        .position();
    assert!(
        (0..3)
            .map(|axis| (terminal[axis] - f64::from(origin[axis])).powi(2))
            .sum::<f64>()
            <= 4.5 * 4.5
    );
    let uuid = first
        .world()
        .resource::<client::ClientSession>()
        .uuid
        .unwrap();
    let item = ItemKind::for_wood(terra_core::rails::STRAIGHT, wood);
    {
        let mut journal = host.world_mut().resource_mut::<EditJournal>();
        let inventory = journal.gameplay_mut().inventories.entry(uuid).or_default();
        assert!(inventory.creative_set(
            8,
            ItemStack {
                item,
                count: cost + 3
            }
        ));
        let pickaxe = ItemStack {
            item: ItemKind::Tool(ToolKind::WoodPickaxe),
            count: 1,
        };
        if inventory.slot(7) != Some(pickaxe) {
            assert!(inventory.creative_set(7, pickaxe));
        }
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
    first
        .world_mut()
        .resource_mut::<client::ClientSession>()
        .look = look;
    wait_phase(
        "inventory confirmation",
        &mut host,
        &mut [&mut first, &mut watcher],
        |_, clients| {
            clients[0]
                .world()
                .resource::<client::ClientSession>()
                .inventory
                .count(item)
                == cost + 3
        },
    );
    let previous = first
        .world()
        .resource::<client::ClientSession>()
        .last_edit
        .map(|edit| edit.0);
    first.world_mut().write_message(client::RailConnectRequest {
        start,
        position: hit.position,
        expected: 3,
    });
    wait_phase(
        "connection edit response",
        &mut host,
        &mut [&mut first, &mut watcher],
        |_, clients| {
            clients[0]
                .world()
                .resource::<client::ClientSession>()
                .last_edit
                .is_some_and(|edit| Some(edit.0) != previous)
        },
    );
    assert_eq!(
        first
            .world()
            .resource::<client::ClientSession>()
            .last_edit
            .unwrap()
            .1,
        EditOutcome::Applied
    );
    wait_phase(
        "two-client curve replication and cost",
        &mut host,
        &mut [&mut first, &mut watcher],
        |host, clients| {
            rails::complete(host.world().resource::<EditJournal>(), curve)
                && clients
                    .iter()
                    .all(|client| rails::complete(client.world().resource::<EditJournal>(), curve))
                && clients[0]
                    .world()
                    .resource::<client::ClientSession>()
                    .inventory
                    .count(item)
                    == 3
        },
    );
    let token = host
        .world()
        .resource::<server::ServerRuntime>()
        .issue_token(205620)
        .unwrap();
    let mut late = client(token);
    wait_phase(
        "late-join curve metadata",
        &mut host,
        &mut [&mut first, &mut watcher, &mut late],
        |_, clients| {
            clients[2].world().resource::<client::ClientSession>().ready
                && rails::complete(clients[2].world().resource::<EditJournal>(), curve)
        },
    );
    let graph = rails::network(host.world().resource::<EditJournal>()).unwrap();
    for app in [&first, &watcher, &late] {
        let journal = app.world().resource::<EditJournal>();
        assert_eq!(rails::network(journal).unwrap(), graph);
        assert!(cells.iter().all(|cell| journal.object_state(*cell)
            == Some(rails::object_state_with_wood(curve, *cell, wood))));
    }
    let origin = eye(&first);
    let mut secondary = None;
    for cell in cells.iter().copied().filter(|cell| *cell != curve.anchor) {
        for bounds in rails::cell_boxes(
            cell,
            curve.material,
            rails::object_state_with_wood(curve, cell, wood),
        )
        .iter()
        {
            let point = Vec3::from_array(std::array::from_fn(|axis| {
                cell[axis] as f32 + ((bounds.minimum[axis] + bounds.maximum[axis]) * 0.5) as f32
            }));
            let look = look_at(origin, point);
            if picked(&host, origin, look)
                .is_some_and(|hit| hit.position == cell && hit.distance <= 4.5)
            {
                secondary = Some((cell, look));
                break;
            }
        }
        if secondary.is_some() {
            break;
        }
    }
    let (secondary, look) = secondary
        .expect("a genuine curve slice must remain reachable from the safe endpoint stance");
    first
        .world_mut()
        .resource_mut::<client::SelectedInventorySlot>()
        .0 = Some(7);
    first
        .world_mut()
        .resource_mut::<client::ClientSession>()
        .look = look;
    let intent = MiningIntent {
        target: Some(MiningTarget::Block {
            position: secondary,
            expected: curve.material,
        }),
        look,
        fell_tree: false,
    };
    let next_drop = host.world().resource::<EditJournal>().gameplay().next_drop;
    let deadline = Instant::now() + Duration::from_secs(15);
    let mut heartbeat = Instant::now();
    let mut progress_seen = false;
    loop {
        if Instant::now() >= heartbeat {
            first
                .world_mut()
                .write_message(client::MiningRequest(intent));
            heartbeat = Instant::now() + Duration::from_millis(25);
        }
        host.update();
        first.update();
        watcher.update();
        late.update();
        let session = first.world().resource::<client::ClientSession>();
        assert!(session.error.is_none(), "{:?}", session.error);
        progress_seen |= session
            .mining
            .is_some_and(|progress| progress.progress < 1.0);
        if host.world().resource::<EditJournal>().block(secondary) == Some(0) {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "curve mining stalled: {:?}, {:?}",
            session.last_edit,
            session.mining
        );
        std::thread::sleep(Duration::from_millis(1));
    }
    assert!(progress_seen);
    first
        .world_mut()
        .write_message(client::MiningRequest(MiningIntent {
            target: None,
            ..intent
        }));
    let gameplay = host.world().resource::<EditJournal>().gameplay();
    assert_eq!(gameplay.next_drop, next_drop + 1);
    let dropped = gameplay
        .drops
        .iter()
        .find(|drop| drop.stack.item == item)
        .unwrap();
    assert_eq!(dropped.stack.count, cost);
    assert_eq!(gameplay.inventories[&uuid].count(item), 3);
    wait_phase(
        "removed curve replicated",
        &mut host,
        &mut [&mut first, &mut watcher, &mut late],
        |host, clients| {
            std::iter::once(host)
                .chain(clients.iter().map(|client| &**client))
                .all(|app| {
                    let journal = app.world().resource::<EditJournal>();
                    cells.iter().all(|cell| {
                        journal.block(*cell) == Some(0) && journal.object_state(*cell).is_none()
                    }) && rails::complete(journal, original)
                        && rails::network(journal).unwrap().len() == 1
                })
        },
    );
}
