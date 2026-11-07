use super::*;
use crate::protocol::session::{EditOutcome, MiningIntent, MiningTarget};
use terra_core::{GameMode, ItemKind, ItemStack, ToolKind, rails::Piece};
use terra_world::{edits::EditJournal, rails, streaming::BlockEditRequest};

fn eye(client: &App) -> Vec3 {
    Vec3::from_array(
        client
            .world()
            .resource::<client::ClientSession>()
            .confirmed_state()
            .unwrap()
            .position,
    ) + Vec3::Y * terra_physics::character::PLAYER_EYE_OFFSET
}

fn look_at(origin: Vec3, point: Vec3) -> [f32; 2] {
    let direction = (point - origin).normalize();
    [(-direction.x).atan2(-direction.z), direction.y.asin()]
}

fn picked(host: &App, origin: Vec3, look: [f32; 2]) -> Option<terra_voxel::blocks::BlockHit> {
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
    let direction = Quat::from_euler(EulerRot::YXZ, look[0], look[1], 0.0) * Vec3::NEG_Z;
    let (_, target) = terra_world::targeting::pick_world_for_mining(
        host.world().resource::<ChunkStore>(),
        journal,
        host.world()
            .resource::<terra_core::block_properties::BlockPropertiesCatalog>(),
        origin.to_array().map(f64::from),
        direction.to_array().map(f64::from),
        pose,
    );
    match target {
        Some(terra_world::targeting::WorldTarget::Block(hit)) => Some(hit),
        _ => None,
    }
}

fn visible_secondary(host: &App, client: &App, piece: Piece) -> ([i64; 3], [f32; 2]) {
    let origin = eye(client);
    let mut candidates = rails::cells(piece);
    candidates.sort_by(|first, second| {
        let distance = |cell: [i64; 3]| {
            (Vec3::from_array(cell.map(|value| value as f32 + 0.5)) - origin).length_squared()
        };
        distance(*first)
            .total_cmp(&distance(*second))
            .then(first.cmp(second))
    });
    for cell in candidates.into_iter().filter(|cell| *cell != piece.anchor) {
        for bounds in
            rails::cell_boxes(cell, piece.material, rails::object_state(piece, cell)).iter()
        {
            let center = std::array::from_fn(|axis| {
                cell[axis] as f32 + ((bounds.minimum[axis] + bounds.maximum[axis]) * 0.5) as f32
            });
            let look = look_at(origin, Vec3::from_array(center));
            if picked(host, origin, look)
                .is_some_and(|hit| hit.position == cell && hit.distance <= 4.5)
            {
                return (cell, look);
            }
        }
    }
    panic!("no real secondary rail slice visible: eye={origin:?}, piece={piece:?}");
}

#[test]
fn udp_separate_switch_and_real_stand_replicate_toggle_and_return_one_switch_after_timed_mining() {
    let (mut host, mut first, mut second) = pair();
    let state = first
        .world()
        .resource::<client::ClientSession>()
        .confirmed_state()
        .unwrap();
    let ground_y =
        (state.position[1] - terra_physics::character::PLAYER_HALF_HEIGHT).floor() as i64 - 1;
    let anchor = [
        state.position[0].floor() as i64,
        ground_y + 1,
        state.position[2].floor() as i64 + 3,
    ];
    let piece = Piece::new(anchor, terra_core::rails::SWITCH_RIGHT, 0).unwrap();
    let cells = rails::cells(piece);
    let minimum: [i64; 3] =
        std::array::from_fn(|axis| cells.iter().map(|cell| cell[axis]).min().unwrap());
    let maximum: [i64; 3] =
        std::array::from_fn(|axis| cells.iter().map(|cell| cell[axis]).max().unwrap());
    // Build a real stone foundation and clear its generated ground cover with
    // ordinary world edits. Never fake rail cells or bypass their placement API.
    wait(&mut host, &mut [&mut first, &mut second], |host, _| {
        let chunks = host.world().resource::<ChunkStore>();
        cells.iter().all(|cell| chunks.block_at(*cell).is_some())
            && (minimum[0] - 1..=maximum[0] + 1).all(|x| {
                (minimum[2] - 2..=maximum[2] + 1)
                    .all(|z| chunks.block_at([x, ground_y, z]).is_some())
            })
    });
    host.world_mut()
        .resource_scope(|world, mut journal: Mut<EditJournal>| {
            let mut chunks = world.resource_mut::<ChunkStore>();
            for x in minimum[0] - 1..=maximum[0] + 1 {
                for z in minimum[2] - 2..=maximum[2] + 1 {
                    let position = [x, ground_y, z];
                    let expected = chunks.block_at(position).unwrap();
                    if expected != 3 {
                        assert!(
                            terra_world::streaming::commit_block_edit(
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
            }
        });
    let uuid = first
        .world()
        .resource::<client::ClientSession>()
        .uuid
        .unwrap();
    let item = ItemKind::Block(piece.material);
    let cost = rails::piece_cost(piece);
    {
        let mut journal = host.world_mut().resource_mut::<EditJournal>();
        let inventory = journal.gameplay_mut().inventories.entry(uuid).or_default();
        assert!(inventory.creative_set(
            8,
            ItemStack {
                item,
                count: cost + 1
            }
        ));
        assert!(inventory.creative_set(
            6,
            ItemStack {
                item: ItemKind::Block(terra_core::rails::SWITCH_STAND),
                count: 1
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
    wait(&mut host, &mut [&mut first, &mut second], |_, clients| {
        clients[0]
            .world()
            .resource::<client::ClientSession>()
            .inventory
            .count(item)
            == cost + 1
            && clients.iter().all(|client| {
                client
                    .world()
                    .resource::<ChunkStore>()
                    .block_at([anchor[0], ground_y, anchor[2]])
                    == Some(3)
            })
    });
    let origin = eye(&first);
    let target = Vec3::new(
        anchor[0] as f32 + 0.5,
        anchor[1] as f32,
        anchor[2] as f32 + 0.5,
    );
    let look = look_at(origin, target);
    let hit = picked(&host, origin, look).expect("the prepared supporting floor must be visible");
    assert_eq!(hit.position, [anchor[0], ground_y, anchor[2]]);
    assert_eq!(hit.normal, [0, 1, 0]);
    let direction = Quat::from_euler(EulerRot::YXZ, look[0], look[1], 0.0) * Vec3::NEG_Z;
    let preview = rails::placement(
        host.world().resource::<ChunkStore>(),
        host.world().resource::<EditJournal>(),
        hit,
        piece.material,
        (
            origin.to_array().map(f64::from),
            direction.to_array().map(f64::from),
        ),
        look,
    )
    .unwrap();
    assert!(
        preview
            .states
            .iter()
            .any(|(cell, state)| *cell == anchor && state.is_some_and(|state| !state.secondary))
    );
    first
        .world_mut()
        .resource_mut::<client::ClientSession>()
        .look = look;
    first
        .world_mut()
        .resource_mut::<client::SelectedInventorySlot>()
        .0 = Some(8);
    first.world_mut().write_message(BlockEditRequest {
        position: anchor,
        expected_material: 0,
        material: piece.material,
    });
    wait(&mut host, &mut [&mut first, &mut second], |_, clients| {
        clients[0]
            .world()
            .resource::<client::ClientSession>()
            .last_edit
            .is_some()
    });
    assert_eq!(
        first
            .world()
            .resource::<client::ClientSession>()
            .last_edit
            .unwrap()
            .1,
        EditOutcome::Applied
    );
    wait(
        &mut host,
        &mut [&mut first, &mut second],
        |host, clients| {
            rails::complete(host.world().resource::<EditJournal>(), piece)
                && clients
                    .iter()
                    .all(|client| rails::complete(client.world().resource::<EditJournal>(), piece))
                && clients[0]
                    .world()
                    .resource::<client::ClientSession>()
                    .inventory
                    .count(item)
                    == 1
        },
    );
    assert_eq!(
        host.world()
            .resource::<EditJournal>()
            .gameplay()
            .inventories[&uuid]
            .count(item),
        1
    );
    let stand = rails::stand::positions(piece)[0];
    assert!(!cells.contains(&stand));
    assert_eq!(
        host.world()
            .resource::<ChunkStore>()
            .block_at([stand[0], ground_y, stand[2]]),
        Some(3)
    );
    let origin = eye(&first);
    // Aim at the near corner of the genuine supporting face, not through the switch.
    let direction = Vec3::new(
        stand[0] as f32 + 0.5,
        anchor[1] as f32,
        stand[2] as f32 + 0.5,
    ) - origin;
    let near = Vec3::new(
        stand[0] as f32 + if direction.x > 0.0 { 0.1 } else { 0.9 },
        anchor[1] as f32,
        stand[2] as f32 + if direction.z > 0.0 { 0.1 } else { 0.9 },
    );
    let stand_look = look_at(origin, near);
    let support_hit =
        picked(&host, origin, stand_look).expect("the real stand floor must be visible");
    assert_eq!(support_hit.position, [stand[0], ground_y, stand[2]]);
    assert_eq!(support_hit.normal, [0, 1, 0]);
    assert!(support_hit.distance <= 4.5);
    let previous = first
        .world()
        .resource::<client::ClientSession>()
        .last_edit
        .unwrap()
        .0;
    first
        .world_mut()
        .resource_mut::<client::ClientSession>()
        .look = stand_look;
    first
        .world_mut()
        .resource_mut::<client::SelectedInventorySlot>()
        .0 = Some(6);
    first.world_mut().write_message(BlockEditRequest {
        position: stand,
        expected_material: 0,
        material: terra_core::rails::SWITCH_STAND,
    });
    wait(&mut host, &mut [&mut first, &mut second], |_, clients| {
        clients[0]
            .world()
            .resource::<client::ClientSession>()
            .last_edit
            .is_some_and(|(sequence, _)| sequence != previous)
    });
    assert_eq!(
        first
            .world()
            .resource::<client::ClientSession>()
            .last_edit
            .unwrap()
            .1,
        EditOutcome::Applied
    );
    wait(
        &mut host,
        &mut [&mut first, &mut second],
        |host, clients| {
            rails::stand::switch_at(host.world().resource::<EditJournal>(), stand) == Some(piece)
                && clients.iter().all(|client| {
                    rails::stand::switch_at(client.world().resource::<EditJournal>(), stand)
                        == Some(piece)
                })
                && clients[0]
                    .world()
                    .resource::<client::ClientSession>()
                    .inventory
                    .count(ItemKind::Block(terra_core::rails::SWITCH_STAND))
                    == 0
        },
    );
    let token = host
        .world()
        .resource::<server::ServerRuntime>()
        .issue_token(620)
        .unwrap();
    let mut late = client(token);
    wait(
        &mut host,
        &mut [&mut first, &mut second, &mut late],
        |_, clients| {
            clients[2].world().resource::<client::ClientSession>().ready
                && rails::complete(clients[2].world().resource::<EditJournal>(), piece)
        },
    );
    let graph = rails::network(host.world().resource::<EditJournal>()).unwrap();
    for client in [&first, &second, &late] {
        assert_eq!(
            rails::network(client.world().resource::<EditJournal>()).unwrap(),
            graph
        );
        for cell in &cells {
            assert_eq!(
                client.world().resource::<EditJournal>().object_state(*cell),
                Some(rails::object_state(piece, *cell))
            );
        }
    }
    let stand_state = host
        .world()
        .resource::<EditJournal>()
        .object_state(stand)
        .unwrap();
    let origin = eye(&first);
    let switch_look = terra_core::block_shapes::builtin_boxes(terra_core::rails::SWITCH_STAND)
        .iter()
        .filter_map(|shape| {
            let shape = shape.rotated_y(stand_state.facing);
            let center = Vec3::from_array(std::array::from_fn(|axis| {
                stand[axis] as f32 + ((shape.minimum[axis] + shape.maximum[axis]) * 0.5) as f32
            }));
            let look = look_at(origin, center);
            picked(&host, origin, look)
                .filter(|hit| hit.position == stand && hit.distance <= 4.5)
                .map(|_| look)
        })
        .next()
        .expect("the actual stand grip must be visible within interaction reach");
    first
        .world_mut()
        .resource_mut::<client::ClientSession>()
        .look = switch_look;
    let previous = first
        .world()
        .resource::<client::ClientSession>()
        .last_edit
        .unwrap()
        .0;
    first
        .world_mut()
        .write_message(client::ObjectInteractionRequest {
            position: stand,
            expected: terra_core::rails::SWITCH_STAND,
            facing: stand_state.facing,
            closing: false,
        });
    let switched = Piece::new(anchor, piece.material, 4).unwrap();
    wait(
        &mut host,
        &mut [&mut first, &mut second, &mut late],
        |_, clients| {
            clients[0]
                .world()
                .resource::<client::ClientSession>()
                .last_edit
                .is_some_and(|(sequence, _)| sequence != previous)
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
    wait(
        &mut host,
        &mut [&mut first, &mut second, &mut late],
        |host, clients| {
            rails::complete(host.world().resource::<EditJournal>(), switched)
                && clients.iter().all(|client| {
                    rails::complete(client.world().resource::<EditJournal>(), switched)
                })
        },
    );
    assert_eq!(
        host.world()
            .resource::<EditJournal>()
            .gameplay()
            .inventories[&uuid]
            .count(item),
        1
    );
    let (secondary, mining_look) = visible_secondary(&host, &first, switched);
    first
        .world_mut()
        .resource_mut::<client::ClientSession>()
        .look = mining_look;
    first
        .world_mut()
        .resource_mut::<client::SelectedInventorySlot>()
        .0 = Some(7);
    let next_drop = host.world().resource::<EditJournal>().gameplay().next_drop;
    let intent = MiningIntent {
        target: Some(MiningTarget::Block {
            position: secondary,
            expected: piece.material,
        }),
        look: mining_look,
        fell_tree: false,
    };
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
        second.update();
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
            "secondary rail mining timed out: last_edit={:?}, progress={:?}, target={:?}",
            session.last_edit,
            session.mining,
            picked(&host, eye(&first), mining_look)
        );
        std::thread::sleep(Duration::from_millis(1));
    }
    assert!(
        progress_seen,
        "Survival removal must use the timed mining path"
    );
    first
        .world_mut()
        .write_message(client::MiningRequest(MiningIntent {
            target: None,
            ..intent
        }));
    let gameplay = host.world().resource::<EditJournal>().gameplay();
    assert_eq!(
        gameplay.next_drop,
        next_drop + 1,
        "the whole turnout returns exactly one separate switch item"
    );
    let dropped: Vec<_> = gameplay
        .drops
        .iter()
        .filter(|drop| drop.stack.item == item)
        .collect();
    assert_eq!(dropped.len(), 1);
    assert_eq!(dropped[0].stack.count, cost);
    assert_eq!(gameplay.player_stats[&uuid].blocks_placed, 2);
    assert_eq!(gameplay.player_stats[&uuid].blocks_broken, 1);
    wait(
        &mut host,
        &mut [&mut first, &mut second, &mut late],
        |host, clients| {
            std::iter::once(host)
                .chain(clients.iter().map(|client| &**client))
                .all(|app| {
                    let journal = app.world().resource::<EditJournal>();
                    cells.iter().all(|cell| {
                        journal.block(*cell) == Some(0) && journal.object_state(*cell).is_none()
                    }) && rails::network(journal).unwrap().is_empty()
                })
        },
    );
}
