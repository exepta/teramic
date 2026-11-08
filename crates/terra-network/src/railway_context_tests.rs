use super::*;
use terra_core::rails::BuildChoice;

#[test]
fn udp_context_choice_uses_real_ray_signature_and_material_then_replicates_to_late_join() {
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
            name: "ContextWatcher".into(),
            ..Default::default()
        },
    );
    wait_phase(
        "context clients grounded",
        &mut host,
        &mut [&mut first, &mut watcher],
        |_, clients| {
            clients.iter().all(|client| {
                client.world().resource::<client::ClientSession>().ready
                    && client
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
    let old = Piece::new(
        [
            state.position[0].floor() as i64,
            ground + 1,
            state.position[2].floor() as i64 + 3,
        ],
        terra_core::rails::STRAIGHT,
        0,
    )
    .unwrap();
    let next = Piece::new(
        [old.anchor[0], old.anchor[1], old.anchor[2] + 4],
        terra_core::rails::STRAIGHT,
        0,
    )
    .unwrap();
    let feet = [
        old.anchor[0] as f32 + 3.5,
        (ground + 1) as f32,
        old.anchor[2] as f32 + 4.5,
    ];
    let floors: std::collections::BTreeSet<_> = old
        .supports()
        .iter()
        .map(|point| std::array::from_fn(|axis| old.anchor[axis] + i64::from(point.support[axis])))
        .chain(next.supports().iter().map(|point| {
            std::array::from_fn(|axis| next.anchor[axis] + i64::from(point.support[axis]))
        }))
        .chain([[feet[0].floor() as i64, ground, feet[2].floor() as i64]])
        .collect();
    wait_phase(
        "context terrain loaded",
        &mut host,
        &mut [&mut first, &mut watcher],
        |host, _| {
            let chunks = host.world().resource::<ChunkStore>();
            floors.iter().all(|cell| chunks.block_at(*cell).is_some())
                && rails::cells(old)
                    .into_iter()
                    .chain(rails::cells(next))
                    .all(|cell| chunks.block_at(cell).is_some())
        },
    );
    floor(&mut host, floors);
    let uuid = first
        .world()
        .resource::<client::ClientSession>()
        .uuid
        .unwrap();
    let item = ItemKind::Block(terra_core::rails::STRAIGHT);
    {
        let mut journal = host.world_mut().resource_mut::<EditJournal>();
        assert!(
            journal
                .gameplay_mut()
                .inventories
                .entry(uuid)
                .or_default()
                .creative_set(8, ItemStack { item, count: 4 })
        );
        journal.gameplay_mut().revision += 1;
    }
    host.world_mut()
        .resource_mut::<GameConfig>()
        .server
        .game_mode = GameMode::Survival;
    wait_phase(
        "context inventory and floor",
        &mut host,
        &mut [&mut first, &mut watcher],
        |_, clients| {
            clients[0]
                .world()
                .resource::<client::ClientSession>()
                .inventory
                .count(item)
                == 4
                && clients.iter().all(|client| {
                    client.world().resource::<ChunkStore>().block_at([
                        old.anchor[0],
                        ground,
                        old.anchor[2],
                    ]) == Some(3)
                })
        },
    );
    let look = look_at(
        eye(&first),
        Vec3::new(
            old.anchor[0] as f32 + 0.5,
            old.anchor[1] as f32,
            old.anchor[2] as f32 + 0.5,
        ),
    );
    let hit =
        picked(&host, eye(&first), look).expect("initial supporting floor is genuinely visible");
    assert_eq!(hit.position, [old.anchor[0], ground, old.anchor[2]]);
    first
        .world_mut()
        .resource_mut::<client::ClientSession>()
        .look = look;
    first
        .world_mut()
        .resource_mut::<client::SelectedInventorySlot>()
        .0 = Some(8);
    first.world_mut().write_message(BlockEditRequest {
        position: old.anchor,
        expected_material: 0,
        material: old.material,
    });
    wait_phase(
        "context original track",
        &mut host,
        &mut [&mut first, &mut watcher],
        |host, clients| {
            rails::complete(host.world().resource::<EditJournal>(), old)
                && clients
                    .iter()
                    .all(|client| rails::complete(client.world().resource::<EditJournal>(), old))
                && clients[0]
                    .world()
                    .resource::<client::ClientSession>()
                    .inventory
                    .count(item)
                    == 3
        },
    );
    teleport(
        "context source stance",
        &mut host,
        &mut first,
        &mut watcher,
        feet,
    );
    let origin = eye(&first);
    let mut selection = None;
    for cell in rails::cells(old) {
        for bounds in rails::cell_boxes(cell, old.material, rails::object_state(old, cell)).iter() {
            let point = Vec3::from_array(std::array::from_fn(|axis| {
                cell[axis] as f32 + ((bounds.minimum[axis] + bounds.maximum[axis]) * 0.5) as f32
            }));
            let look = look_at(origin, point);
            let Some(hit) = picked(&host, origin, look) else {
                continue;
            };
            let context = rails::context_choice(
                host.world().resource::<ChunkStore>(),
                host.world().resource::<EditJournal>(),
                hit,
                ray(origin, look),
                look,
                WoodSpecies::Oak,
                BuildChoice::Straight,
            );
            if context.valid
                && context.source
                    == Some(Endpoint {
                        anchor: old.anchor,
                        port: 1,
                    })
                && context.layout.as_ref().is_some_and(|layout| {
                    layout.states.iter().any(|(cell, state)| {
                        *cell == next.anchor && state.is_some_and(|state| !state.secondary)
                    })
                })
            {
                selection = Some((hit, look, context));
                break;
            }
        }
        if selection.is_some() {
            break;
        }
    }
    let (hit, look, selected) =
        selection.expect("a real reachable free end offers the actual context extension");
    assert!(hit.distance <= 4.5);
    let request = client::RailBuildRequest {
        position: hit.position,
        expected: old.material,
        choice: BuildChoice::Straight,
        signature: rails::context_signature(&selected),
    };
    let previous = first
        .world()
        .resource::<client::ClientSession>()
        .last_edit
        .unwrap()
        .0;
    first
        .world_mut()
        .resource_mut::<client::ClientSession>()
        .look = look;
    first.world_mut().write_message(request);
    wait_phase(
        "context request result",
        &mut host,
        &mut [&mut first, &mut watcher],
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
    wait_phase(
        "context extension replicated",
        &mut host,
        &mut [&mut first, &mut watcher],
        |host, clients| {
            rails::complete(host.world().resource::<EditJournal>(), next)
                && clients
                    .iter()
                    .all(|client| rails::complete(client.world().resource::<EditJournal>(), next))
                && clients[0]
                    .world()
                    .resource::<client::ClientSession>()
                    .inventory
                    .count(item)
                    == 2
        },
    );
    let token = host
        .world()
        .resource::<server::ServerRuntime>()
        .issue_token(207114)
        .unwrap();
    let mut late = client(token);
    wait_phase(
        "context late join",
        &mut host,
        &mut [&mut first, &mut watcher, &mut late],
        |_, clients| {
            clients[2].world().resource::<client::ClientSession>().ready
                && rails::complete(clients[2].world().resource::<EditJournal>(), next)
        },
    );
    let graph = rails::network(host.world().resource::<EditJournal>()).unwrap();
    assert_eq!(graph.len(), 2);
    for app in [&first, &watcher, &late] {
        assert_eq!(
            rails::network(app.world().resource::<EditJournal>()).unwrap(),
            graph
        );
    }
    let before = host
        .world()
        .resource::<EditJournal>()
        .gameplay()
        .inventories[&uuid]
        .clone();
    let previous = first
        .world()
        .resource::<client::ClientSession>()
        .last_edit
        .unwrap()
        .0;
    first.world_mut().write_message(request);
    wait_phase(
        "stale context intent rejected",
        &mut host,
        &mut [&mut first, &mut watcher, &mut late],
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
        EditOutcome::Conflict
    );
    assert_eq!(
        host.world()
            .resource::<EditJournal>()
            .gameplay()
            .inventories[&uuid],
        before
    );
    assert_eq!(
        rails::network(host.world().resource::<EditJournal>()).unwrap(),
        graph
    );
}
