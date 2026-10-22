use super::*;
use protocol::session::*;
use terra_core::{GameMode, ItemKind};
use terra_world::{
    edits::EditJournal,
    streaming::{BlockEditRequest, commit_block_edit},
};

fn tick_for(host: &mut App, clients: &mut [&mut App], duration: Duration) {
    let end = Instant::now() + duration;
    while Instant::now() < end {
        host.update();
        for app in clients.iter_mut() {
            app.update();
        }
        std::thread::sleep(Duration::from_millis(2));
    }
}
fn held(host: &mut App, client: &mut App, intent: MiningIntent, duration: Duration) {
    let end = Instant::now() + duration;
    while Instant::now() < end {
        client
            .world_mut()
            .write_message(client::MiningRequest(intent));
        tick_for(host, &mut [client], Duration::from_millis(70));
    }
}
fn fixture() -> (App, App, [i64; 3], [f32; 2]) {
    let mut host = server();
    host.world_mut()
        .resource_mut::<GameConfig>()
        .server
        .game_mode = GameMode::Survival;
    let address = host
        .world()
        .resource::<server::ServerRuntime>()
        .address
        .unwrap();
    let mut a = profile_client(address, terra_core::PlayerProfile::default());
    wait(&mut host, &mut [&mut a], |_, clients| {
        clients[0]
            .world()
            .resource::<client::ClientSession>()
            .confirmed_state()
            .is_some_and(|s| s.grounded)
    });
    let session = a.world().resource::<client::ClientSession>();
    assert_eq!(session.game_mode, GameMode::Survival);
    let eye = Vec3::from_array(session.confirmed_state().unwrap().position)
        + Vec3::Y * terra_physics::character::PLAYER_EYE_OFFSET;
    let position = [
        eye.x.floor() as i64,
        eye.y.floor() as i64,
        eye.z.floor() as i64 - 3,
    ];
    host.world_mut()
        .resource_scope(|world, mut journal: Mut<EditJournal>| {
            let mut chunks = world.resource_mut::<ChunkStore>();
            let before = chunks.block_at(position).unwrap();
            assert!(
                commit_block_edit(
                    &mut chunks,
                    &mut journal,
                    &BlockEditRequest {
                        position,
                        expected_material: before,
                        material: 3
                    },
                    &Default::default(),
                )
                .unwrap()
            );
        });
    wait(&mut host, &mut [&mut a], |_, clients| {
        clients[0]
            .world()
            .resource::<ChunkStore>()
            .block_at(position)
            == Some(3)
    });
    let direction = (Vec3::from_array(position.map(|p| p as f32 + 0.5)) - eye).normalize();
    let look = [(-direction.x).atan2(-direction.z), direction.y.asin()];
    a.world_mut().resource_mut::<client::ClientSession>().look = look;
    (host, a, position, look)
}

#[test]
fn falling_minerals_use_authoritative_damage_and_replicate_the_existing_drop() {
    let (mut host, mut client, _, _) = fixture();
    tick_for(&mut host, &mut [&mut client], Duration::from_millis(2200));
    let position = client
        .world()
        .resource::<client::ClientSession>()
        .confirmed_state()
        .unwrap()
        .position
        .map(f64::from);
    {
        let mut journal = host.world_mut().resource_mut::<EditJournal>();
        journal.gameplay_mut().drop_one(
            [position[0], position[1] + 3.5, position[2]],
            ItemKind::Block(52),
        );
        let drop = journal.gameplay_mut().drops.last_mut().unwrap();
        drop.velocity = [0.0, -8.0, 0.0];
        drop.rotation = [0.0, 0.0, 0.0, 1.0];
        drop.angular_velocity = [0.0; 3];
        drop.pickup_delay_seconds = 10.0;
    }
    wait(&mut host, &mut [&mut client], |_, clients| {
        clients[0]
            .world()
            .resource::<client::ClientSession>()
            .stats
            .survival
            .last_damage
            == Some(terra_core::DamageCause::FallingMineral)
    });
    let session = client.world().resource::<client::ClientSession>();
    assert!(session.stats.survival.health < 100.0);
    assert!(
        host.world()
            .resource::<EditJournal>()
            .gameplay()
            .drops
            .iter()
            .any(|d| d.stack.item == ItemKind::Block(37))
    );
}

#[test]
fn mineral_chain_cut_replicates_only_detached_cells_and_remains_removed_for_a_late_udp_join() {
    let (mut host, mut client, p, _) = fixture();
    host.world_mut()
        .resource_mut::<GameConfig>()
        .server
        .game_mode = GameMode::Creative;
    host.world_mut()
        .resource_scope(|world, mut journal: Mut<EditJournal>| {
            let mut chunks = world.resource_mut::<ChunkStore>();
            for (dy, material) in [(3, 3), (2, 51), (1, 51), (0, 51)] {
                let position = [p[0], p[1] + dy, p[2]];
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
            }
        });
    wait(&mut host, &mut [&mut client], |_, clients| {
        (0..3).all(|dy| {
            clients[0]
                .world()
                .resource::<ChunkStore>()
                .block_at([p[0], p[1] + dy, p[2]])
                == Some(51)
        })
    });
    let eye = Vec3::from_array(
        client
            .world()
            .resource::<client::ClientSession>()
            .confirmed_state()
            .unwrap()
            .position,
    ) + Vec3::Y * terra_physics::character::PLAYER_EYE_OFFSET;
    let target = Vec3::new(p[0] as f32 + 0.5, p[1] as f32 + 1.5, p[2] as f32 + 0.5);
    let direction = (target - eye).normalize();
    let look = [(-direction.x).atan2(-direction.z), direction.y.asin()];
    assert_eq!(
        edit(
            &mut host,
            &mut client,
            EditAction::Block {
                position: [p[0], p[1] + 1, p[2]],
                expected: 51,
                material: 0
            },
            look
        ),
        EditOutcome::Applied
    );
    let cut = |app: &App| {
        let chunks = app.world().resource::<ChunkStore>();
        chunks.block_at(p) == Some(0)
            && chunks.block_at([p[0], p[1] + 1, p[2]]) == Some(0)
            && chunks.block_at([p[0], p[1] + 2, p[2]]) == Some(51)
    };
    wait(&mut host, &mut [&mut client], |host, clients| {
        cut(host) && cut(clients[0])
    });
    let address = host
        .world()
        .resource::<server::ServerRuntime>()
        .address
        .unwrap();
    let mut late = profile_client(address, terra_core::PlayerProfile::default());
    wait(&mut host, &mut [&mut client, &mut late], |_, clients| {
        clients[1].world().resource::<client::ClientSession>().ready && cut(clients[1])
    });
    assert!(
        host.world()
            .resource::<EditJournal>()
            .gameplay()
            .drops
            .iter()
            .any(|drop| matches!(
                drop.stack.item,
                ItemKind::Block(51) | ItemKind::Mineral(terra_core::MineralItem::SaltCrystal)
            ))
    );
}

#[test]
fn dense_masonry_requires_a_real_held_pickaxe_and_uses_its_server_speed() {
    let (mut host, mut a, position, look) = fixture();
    host.world_mut()
        .resource_scope(|world, mut journal: Mut<EditJournal>| {
            let mut chunks = world.resource_mut::<ChunkStore>();
            commit_block_edit(
                &mut chunks,
                &mut journal,
                &BlockEditRequest {
                    position,
                    expected_material: 3,
                    material: 12,
                },
                &Default::default(),
            )
            .unwrap();
        });
    wait(&mut host, &mut [&mut a], |_, clients| {
        clients[0]
            .world()
            .resource::<ChunkStore>()
            .block_at(position)
            == Some(12)
    });
    let intent = MiningIntent {
        target: Some(MiningTarget::Block {
            position,
            expected: 12,
        }),
        look,
    };
    held(&mut host, &mut a, intent, Duration::from_millis(200));
    assert_eq!(
        host.world().resource::<ChunkStore>().block_at(position),
        Some(12)
    );
    let uuid = a.world().resource::<client::ClientSession>().uuid.unwrap();
    host.world_mut()
        .resource_mut::<EditJournal>()
        .gameplay_mut()
        .inventories
        .entry(uuid)
        .or_default()
        .insert(ItemKind::Tool(terra_core::ToolKind::StonePickaxe), 1);
    // Fixture edits bypass transactions, so mirror the equipped inventory explicitly.
    a.world_mut()
        .resource_mut::<client::ClientSession>()
        .inventory = host
        .world()
        .resource::<EditJournal>()
        .gameplay()
        .inventories[&uuid]
        .clone();
    a.world_mut()
        .resource_mut::<client::SelectedInventorySlot>()
        .0 = Some(0);
    held(&mut host, &mut a, intent, Duration::from_millis(650));
    assert_eq!(
        host.world().resource::<ChunkStore>().block_at(position),
        Some(0)
    );
}

#[test]
fn ore_mining_requires_the_right_level_and_drops_resources_not_host_blocks() {
    use terra_core::{MineralItem, ToolKind};
    for (material, insufficient, correct, resource) in [
        (
            31,
            ToolKind::WoodPickaxe,
            ToolKind::StonePickaxe,
            MineralItem::RawIron,
        ),
        (
            34,
            ToolKind::StonePickaxe,
            ToolKind::IronPickaxe,
            MineralItem::Diamond,
        ),
    ] {
        let (mut host, mut a, position, look) = fixture();
        host.world_mut()
            .resource_scope(|world, mut journal: Mut<EditJournal>| {
                let mut chunks = world.resource_mut::<ChunkStore>();
                commit_block_edit(
                    &mut chunks,
                    &mut journal,
                    &BlockEditRequest {
                        position,
                        expected_material: 3,
                        material,
                    },
                    &Default::default(),
                )
                .unwrap();
            });
        wait(&mut host, &mut [&mut a], |_, clients| {
            clients[0]
                .world()
                .resource::<ChunkStore>()
                .block_at(position)
                == Some(material)
        });
        let uuid = a.world().resource::<client::ClientSession>().uuid.unwrap();
        let intent = MiningIntent {
            target: Some(MiningTarget::Block {
                position,
                expected: material,
            }),
            look,
        };
        for (tool, duration) in [(insufficient, 150), (correct, 900)] {
            let mut inventory = terra_core::Inventory::default();
            inventory.insert(ItemKind::Tool(tool), 1);
            host.world_mut()
                .resource_mut::<EditJournal>()
                .gameplay_mut()
                .inventories
                .insert(uuid, inventory.clone());
            a.world_mut()
                .resource_mut::<client::ClientSession>()
                .inventory = inventory;
            a.world_mut()
                .resource_mut::<client::SelectedInventorySlot>()
                .0 = Some(0);
            held(&mut host, &mut a, intent, Duration::from_millis(duration));
            assert_eq!(
                host.world().resource::<ChunkStore>().block_at(position),
                Some(if tool == correct { 0 } else { material })
            );
        }
        let journal = host.world().resource::<EditJournal>();
        let loose: u16 = journal
            .gameplay()
            .drops
            .iter()
            .filter(|drop| drop.stack.item == ItemKind::Mineral(resource))
            .map(|drop| drop.stack.count)
            .sum();
        let picked_up = journal.gameplay().inventories[&uuid].count(ItemKind::Mineral(resource));
        assert_eq!(loose + picked_up, 1);
        assert_eq!(
            journal.gameplay().inventories[&uuid].count(ItemKind::Block(material)),
            0
        );
        assert!(
            !journal
                .gameplay()
                .drops
                .iter()
                .any(|drop| drop.stack.item == ItemKind::Block(material))
        );
        assert_eq!(journal.block(position), Some(0));
    }
}

#[test]
fn survival_is_timed_cancellable_and_direct_removal_cannot_bypass_authority() {
    let (mut host, mut a, position, look) = fixture();
    let section = terra_voxel::global_to_section(position).0;
    let revision = a.world().resource::<EditJournal>().revision(section);
    let command = a
        .world_mut()
        .resource_mut::<client::ClientSession>()
        .edit_command(
            revision,
            EditAction::Block {
                position,
                expected: 3,
                material: 0,
            },
        )
        .unwrap();
    send(&mut a, ClientControl::Edit(command));
    wait(&mut host, &mut [&mut a], |_, clients| {
        clients[0]
            .world()
            .resource::<client::ClientSession>()
            .last_edit
            .is_some()
    });
    assert_eq!(
        a.world()
            .resource::<client::ClientSession>()
            .last_edit
            .unwrap()
            .1,
        EditOutcome::Invalid
    );
    assert_eq!(
        host.world().resource::<ChunkStore>().block_at(position),
        Some(3)
    );
    let intent = MiningIntent {
        target: Some(MiningTarget::Block {
            position,
            expected: 3,
        }),
        look,
    };
    held(&mut host, &mut a, intent, Duration::from_millis(450));
    assert!(
        a.world()
            .resource::<client::ClientSession>()
            .mining
            .is_some_and(|m| m.progress > 0.0 && m.progress < 1.0)
    );
    a.world_mut()
        .write_message(client::MiningRequest(MiningIntent { target: None, look }));
    tick_for(&mut host, &mut [&mut a], Duration::from_millis(900));
    assert_eq!(
        host.world().resource::<ChunkStore>().block_at(position),
        Some(3)
    );
    assert!(
        a.world()
            .resource::<client::ClientSession>()
            .mining
            .is_none()
    );
    // A renewed hold starts from zero rather than preserving the canceled timer.
    held(&mut host, &mut a, intent, Duration::from_millis(450));
    assert_eq!(
        host.world().resource::<ChunkStore>().block_at(position),
        Some(3)
    );
    // Losing the keepalive also cancels, even when the transport remains connected.
    tick_for(&mut host, &mut [&mut a], Duration::from_millis(450));
    assert!(
        a.world()
            .resource::<client::ClientSession>()
            .mining
            .is_none()
    );
    held(&mut host, &mut a, intent, Duration::from_millis(1350));
    wait(&mut host, &mut [&mut a], |_, clients| {
        clients[0]
            .world()
            .resource::<ChunkStore>()
            .block_at(position)
            == Some(0)
    });
    assert_eq!(
        host.world()
            .resource::<EditJournal>()
            .gameplay()
            .drops
            .len(),
        1
    );
    assert_eq!(
        host.world().resource::<EditJournal>().gameplay().drops[0]
            .stack
            .item,
        ItemKind::Block(3)
    );
    held(&mut host, &mut a, intent, Duration::from_millis(100));
    assert_eq!(
        host.world()
            .resource::<EditJournal>()
            .gameplay()
            .drops
            .len(),
        1
    );
    assert_eq!(
        host.world().resource::<EditJournal>().revision(section),
        revision + 1
    );
}

#[test]
fn authoritative_pickup_replicates_and_survives_uuid_reconnect() {
    let (mut host, mut a, _, _) = fixture();
    let profile = a.world().resource::<terra_core::PlayerProfile>().clone();
    let position = a
        .world()
        .resource::<client::ClientSession>()
        .confirmed_state()
        .unwrap()
        .position
        .map(f64::from);
    let item = ItemKind::Prop(terra_core::ResourcePropKind::Bush);
    host.world_mut()
        .resource_mut::<EditJournal>()
        .gameplay_mut()
        .drop_one(position, item);
    wait(&mut host, &mut [&mut a], |_, clients| {
        clients[0]
            .world()
            .resource::<client::ClientSession>()
            .inventory
            .count(item)
            == 1
    });
    assert!(
        host.world()
            .resource::<EditJournal>()
            .gameplay()
            .drops
            .is_empty()
    );
    let link = a
        .world()
        .resource::<client::ClientSession>()
        .connection
        .unwrap();
    a.world_mut()
        .trigger(lightyear::prelude::Disconnect { entity: link });
    let deadline = Instant::now() + Duration::from_secs(5);
    while !host
        .world()
        .resource::<server::ConnectedProfiles>()
        .0
        .is_empty()
    {
        host.update();
        a.update();
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(2));
    }
    let address = host
        .world()
        .resource::<server::ServerRuntime>()
        .address
        .unwrap();
    let mut rejoined = profile_client(address, profile);
    wait(&mut host, &mut [&mut rejoined], |_, clients| {
        clients[0]
            .world()
            .resource::<client::ClientSession>()
            .inventory
            .count(item)
            == 1
    });
}

fn edit(host: &mut App, a: &mut App, action: EditAction, look: [f32; 2]) -> EditOutcome {
    let revision = match action {
        EditAction::Block { position, .. } => a
            .world()
            .resource::<EditJournal>()
            .revision(terra_voxel::global_to_section(position).0),
        _ => 0,
    };
    let command = {
        let mut session = a.world_mut().resource_mut::<client::ClientSession>();
        session.look = look;
        session.edit_command(revision, action).unwrap()
    };
    send(a, ClientControl::Edit(command));
    wait(host, &mut [a], |_, clients| {
        clients[0]
            .world()
            .resource::<client::ClientSession>()
            .last_edit
            .is_some_and(|(request, _)| request == command.request)
    });
    a.world()
        .resource::<client::ClientSession>()
        .last_edit
        .unwrap()
        .1
}

#[test]
fn selected_slot_placement_consumes_only_that_stack_and_rejects_stale_inventory() {
    let (mut host, mut a, _, look) = fixture();
    let uuid = a.world().resource::<client::ClientSession>().uuid.unwrap();
    let item = ItemKind::Block(2);
    let mut bag = terra_core::Inventory::default();
    assert!(bag.creative_set(0, terra_core::ItemStack { item, count: 7 }));
    assert!(bag.creative_set(8, terra_core::ItemStack { item, count: 2 }));
    {
        let mut journal = host.world_mut().resource_mut::<EditJournal>();
        journal.gameplay_mut().inventories.insert(uuid, bag.clone());
        journal.gameplay_mut().revision += 1;
    }
    wait(&mut host, &mut [&mut a], |_, clients| {
        clients[0]
            .world()
            .resource::<client::ClientSession>()
            .inventory
            == bag
    });
    let eye = Vec3::from_array(
        a.world()
            .resource::<client::ClientSession>()
            .confirmed_state()
            .unwrap()
            .position,
    ) + Vec3::Y * terra_physics::character::PLAYER_EYE_OFFSET;
    let direction = Quat::from_euler(EulerRot::YXZ, look[0], look[1], 0.0) * Vec3::NEG_Z;
    let (hit, _) = terra_world::targeting::pick_world(
        a.world().resource::<ChunkStore>(),
        eye.to_array().map(f64::from),
        direction.to_array().map(f64::from),
    );
    let position = hit.unwrap().adjacent();
    let revision = a
        .world()
        .resource::<EditJournal>()
        .revision(terra_voxel::global_to_section(position).0);
    for (slot, inventory_revision, outcome) in [
        (8, bag.revision - 1, EditOutcome::Conflict),
        (7, bag.revision, EditOutcome::Invalid),
    ] {
        let mut command = a
            .world_mut()
            .resource_mut::<client::ClientSession>()
            .edit_command(
                revision,
                EditAction::Block {
                    position,
                    expected: 0,
                    material: 2,
                },
            )
            .unwrap();
        command.source = Some(InventorySource {
            slot,
            revision: inventory_revision,
        });
        send(&mut a, ClientControl::Edit(command));
        wait(&mut host, &mut [&mut a], |_, clients| {
            clients[0]
                .world()
                .resource::<client::ClientSession>()
                .last_edit
                == Some((command.request, outcome))
        });
        assert_eq!(
            host.world().resource::<ChunkStore>().block_at(position),
            Some(0)
        );
        assert_eq!(
            host.world()
                .resource::<EditJournal>()
                .gameplay()
                .inventories[&uuid],
            bag
        );
    }
    a.world_mut()
        .resource_mut::<client::SelectedInventorySlot>()
        .0 = Some(8);
    a.world_mut().write_message(BlockEditRequest {
        position,
        expected_material: 0,
        material: 2,
    });
    wait(&mut host, &mut [&mut a], |_, clients| {
        clients[0]
            .world()
            .resource::<client::ClientSession>()
            .inventory
            .count(item)
            == 8
    });
    let bag = &a.world().resource::<client::ClientSession>().inventory;
    assert_eq!(bag.slot(0).unwrap().count, 7);
    assert_eq!(bag.slot(8).unwrap().count, 1);
    assert_eq!(
        host.world().resource::<ChunkStore>().block_at(position),
        Some(2)
    );
}

#[test]
fn placement_only_consumes_successfully_committed_items_and_rejects_player_overlap() {
    let (mut host, mut a, position, look) = fixture();
    let uuid = a.world().resource::<client::ClientSession>().uuid.unwrap();
    let item = ItemKind::Block(2);
    let mut bag = terra_core::Inventory::default();
    bag.insert(item, 2);
    host.world_mut()
        .resource_mut::<EditJournal>()
        .gameplay_mut()
        .inventories
        .insert(uuid, bag);
    let eye = Vec3::from_array(
        a.world()
            .resource::<client::ClientSession>()
            .confirmed_state()
            .unwrap()
            .position,
    ) + Vec3::Y * terra_physics::character::PLAYER_EYE_OFFSET;
    let direction = Quat::from_euler(EulerRot::YXZ, look[0], look[1], 0.0) * Vec3::NEG_Z;
    let (block, _) = terra_world::targeting::pick_world(
        a.world().resource::<ChunkStore>(),
        eye.to_array().map(f64::from),
        direction.to_array().map(f64::from),
    );
    assert_eq!(block.unwrap().position, position);
    let adjacent = block.unwrap().adjacent();
    let action = EditAction::Block {
        position: adjacent,
        expected: 0,
        material: 2,
    };
    assert_eq!(
        edit(&mut host, &mut a, action, [1.5, 0.0]),
        EditOutcome::Occluded
    );
    assert_eq!(
        host.world()
            .resource::<EditJournal>()
            .gameplay()
            .inventories[&uuid]
            .count(item),
        2
    );
    assert_eq!(edit(&mut host, &mut a, action, look), EditOutcome::Applied);
    wait(&mut host, &mut [&mut a], |_, clients| {
        clients[0]
            .world()
            .resource::<client::ClientSession>()
            .inventory
            .count(item)
            == 1
    });
    assert_eq!(edit(&mut host, &mut a, action, look), EditOutcome::Conflict);
    assert_eq!(
        host.world()
            .resource::<EditJournal>()
            .gameplay()
            .inventories[&uuid]
            .count(item),
        1
    );
    let down = [0.0, -1.5];
    let ray = Quat::from_euler(EulerRot::YXZ, down[0], down[1], 0.0) * Vec3::NEG_Z;
    let (block, _) = terra_world::targeting::pick_world(
        a.world().resource::<ChunkStore>(),
        eye.to_array().map(f64::from),
        ray.to_array().map(f64::from),
    );
    let at_feet = block.unwrap().adjacent();
    assert_eq!(
        edit(
            &mut host,
            &mut a,
            EditAction::Block {
                position: at_feet,
                expected: 0,
                material: 2
            },
            down
        ),
        EditOutcome::Invalid
    );
    assert_eq!(
        host.world()
            .resource::<EditJournal>()
            .gameplay()
            .inventories[&uuid]
            .count(item),
        1
    );
}

#[test]
fn cube_and_static_model_ids_place_and_replicate_but_water_requires_displacement() {
    let (mut host, mut a, position, look) = fixture();
    host.world_mut()
        .resource_mut::<GameConfig>()
        .server
        .game_mode = GameMode::Creative;
    let state = a
        .world()
        .resource::<client::ClientSession>()
        .confirmed_state()
        .unwrap();
    let eye =
        Vec3::from_array(state.position) + Vec3::Y * terra_physics::character::PLAYER_EYE_OFFSET;
    let direction = Quat::from_euler(EulerRot::YXZ, look[0], look[1], 0.0) * Vec3::NEG_Z;
    let (hit, _) = terra_world::targeting::pick_world(
        a.world().resource::<ChunkStore>(),
        eye.to_array().map(f64::from),
        direction.to_array().map(f64::from),
    );
    assert_eq!(hit.unwrap().position, position);
    let adjacent = hit.unwrap().adjacent();
    // Furniture now requires floor support for its entire horizontal footprint.
    host.world_mut()
        .resource_scope(|world, mut journal: Mut<EditJournal>| {
            let mut chunks = world.resource_mut::<ChunkStore>();
            for dx in -1..=1 {
                for dz in -1..=1 {
                    let below = [adjacent[0] + dx, adjacent[1] - 1, adjacent[2] + dz];
                    let expected = chunks.block_at(below).unwrap();
                    if expected == 0 {
                        commit_block_edit(
                            &mut chunks,
                            &mut journal,
                            &BlockEditRequest {
                                position: below,
                                expected_material: 0,
                                material: 3,
                            },
                            &Default::default(),
                        )
                        .unwrap();
                    }
                }
            }
        });
    // Thin rooted segments have separate water/ceiling-support rules.
    for material in 11..=49 {
        // Each sample sends placement and removal. Pace the real UDP fixture
        // below the server's ten-edits-per-second sustained rate.
        tick_for(&mut host, &mut [&mut a], Duration::from_millis(220));
        assert_eq!(
            edit(
                &mut host,
                &mut a,
                EditAction::Block {
                    position: adjacent,
                    expected: 0,
                    material
                },
                look
            ),
            EditOutcome::Applied
        );
        wait(&mut host, &mut [&mut a], |_, clients| {
            clients[0]
                .world()
                .resource::<ChunkStore>()
                .block_at(adjacent)
                == Some(material)
        });
        let removed_material = if material == terra_voxel::MaterialId::FluidWater as u16 {
            assert_eq!(
                edit(
                    &mut host,
                    &mut a,
                    EditAction::Block {
                        position: adjacent,
                        expected: material,
                        material: 0,
                    },
                    look
                ),
                EditOutcome::Invalid
            );
            assert_eq!(
                edit(
                    &mut host,
                    &mut a,
                    EditAction::Block {
                        position: adjacent,
                        expected: material,
                        material: 3,
                    },
                    look
                ),
                EditOutcome::Applied
            );
            wait(&mut host, &mut [&mut a], |_, clients| {
                clients[0]
                    .world()
                    .resource::<ChunkStore>()
                    .block_at(adjacent)
                    == Some(3)
            });
            3
        } else {
            material
        };
        let removal_look = if matches!(material, 28..=29) {
            let eye = Vec3::from_array(
                a.world()
                    .resource::<client::ClientSession>()
                    .confirmed_state()
                    .unwrap()
                    .position,
            ) + Vec3::Y * terra_physics::character::PLAYER_EYE_OFFSET;
            // The original cube-center ray can legitimately pass below a
            // worktop. Aim at an authored surface, not the old full-cell box.
            let target = Vec3::new(
                adjacent[0] as f32 + 0.5,
                adjacent[1] as f32 + if material == 28 { 0.70 } else { 0.40 },
                adjacent[2] as f32 + 0.5,
            );
            let direction = (target - eye).normalize();
            [(-direction.x).atan2(-direction.z), direction.y.asin()]
        } else {
            look
        };
        assert_eq!(
            edit(
                &mut host,
                &mut a,
                EditAction::Block {
                    position: adjacent,
                    expected: removed_material,
                    material: 0
                },
                removal_look
            ),
            EditOutcome::Applied
        );
        wait(&mut host, &mut [&mut a], |_, clients| {
            clients[0]
                .world()
                .resource::<ChunkStore>()
                .block_at(adjacent)
                == Some(0)
        });
    }
    assert_eq!(
        edit(
            &mut host,
            &mut a,
            EditAction::Block {
                position: adjacent,
                expected: 0,
                material: terra_voxel::MaterialId::LAST + 1
            },
            look
        ),
        EditOutcome::Invalid
    );
    assert_eq!(
        host.world().resource::<ChunkStore>().block_at(adjacent),
        Some(0)
    );
}

#[test]
fn a_source_is_replaced_in_place_and_a_solid_block_cannot_be_overwritten() {
    let (mut host, mut a, support, look) = fixture();
    host.world_mut()
        .resource_mut::<GameConfig>()
        .server
        .game_mode = GameMode::Creative;
    let state = a
        .world()
        .resource::<client::ClientSession>()
        .confirmed_state()
        .unwrap();
    let eye =
        Vec3::from_array(state.position) + Vec3::Y * terra_physics::character::PLAYER_EYE_OFFSET;
    let direction = Quat::from_euler(EulerRot::YXZ, look[0], look[1], 0.0) * Vec3::NEG_Z;
    let hit = terra_world::targeting::pick_world(
        a.world().resource::<ChunkStore>(),
        eye.to_array().map(f64::from),
        direction.to_array().map(f64::from),
    )
    .0
    .unwrap();
    assert_eq!(hit.position, support);
    let position = hit.adjacent();
    for (expected, material) in [(0, 27), (27, 3)] {
        if expected == 27 {
            for mode in [GameMode::Creative, GameMode::Survival] {
                host.world_mut()
                    .resource_mut::<GameConfig>()
                    .server
                    .game_mode = mode;
                assert_eq!(
                    edit(
                        &mut host,
                        &mut a,
                        EditAction::Block {
                            position,
                            expected: 27,
                            material: 0,
                        },
                        look
                    ),
                    EditOutcome::Invalid
                );
                assert_eq!(
                    host.world().resource::<ChunkStore>().block_at(position),
                    Some(27)
                );
            }
            host.world_mut()
                .resource_mut::<GameConfig>()
                .server
                .game_mode = GameMode::Creative;
        }
        assert_eq!(
            edit(
                &mut host,
                &mut a,
                EditAction::Block {
                    position,
                    expected,
                    material
                },
                look
            ),
            EditOutcome::Applied
        );
        wait(&mut host, &mut [&mut a], |_, clients| {
            clients[0]
                .world()
                .resource::<ChunkStore>()
                .block_at(position)
                == Some(material)
        });
    }
    assert!(
        host.world()
            .resource::<EditJournal>()
            .fluid(position)
            .is_none()
    );
    assert!(
        a.world()
            .resource::<EditJournal>()
            .fluid(position)
            .is_none()
    );
    assert_eq!(
        edit(
            &mut host,
            &mut a,
            EditAction::Block {
                position,
                expected: 3,
                material: 2
            },
            look
        ),
        EditOutcome::Invalid
    );
    assert_eq!(
        host.world().resource::<ChunkStore>().block_at(position),
        Some(3)
    );
}

#[test]
fn target_switch_out_of_reach_and_full_drop_budget_preserve_blocks() {
    let (mut host, mut a, position, look) = fixture();
    let first = MiningIntent {
        target: Some(MiningTarget::Block {
            position,
            expected: 3,
        }),
        look,
    };
    held(&mut host, &mut a, first, Duration::from_millis(500));
    let far = MiningIntent {
        target: Some(MiningTarget::Block {
            position: [500, 15, 500],
            expected: 3,
        }),
        look,
    };
    a.world_mut().write_message(client::MiningRequest(far));
    tick_for(&mut host, &mut [&mut a], Duration::from_millis(900));
    assert_eq!(
        host.world().resource::<ChunkStore>().block_at(position),
        Some(3)
    );
    assert!(
        a.world()
            .resource::<client::ClientSession>()
            .mining
            .is_none()
    );
    {
        let mut journal = host.world_mut().resource_mut::<EditJournal>();
        for _ in 0..terra_core::MAX_WORLD_DROPS {
            assert!(
                journal
                    .gameplay_mut()
                    .drop_one([500.0; 3], ItemKind::Block(3))
            );
        }
    }
    held(&mut host, &mut a, first, Duration::from_millis(1400));
    assert_eq!(
        host.world().resource::<ChunkStore>().block_at(position),
        Some(3)
    );
    assert_eq!(
        host.world()
            .resource::<EditJournal>()
            .gameplay()
            .drops
            .len(),
        terra_core::MAX_WORLD_DROPS
    );
}

#[test]
fn nearby_pickup_cannot_reach_through_solid_walls() {
    let (mut host, mut a, _, _) = fixture();
    let body = a
        .world()
        .resource::<client::ClientSession>()
        .confirmed_state()
        .unwrap()
        .position;
    let wall = [
        body[0].floor() as i64 + 1,
        body[1].floor() as i64,
        body[2].floor() as i64,
    ];
    host.world_mut()
        .resource_scope(|world, mut journal: Mut<EditJournal>| {
            let mut chunks = world.resource_mut::<ChunkStore>();
            let before = chunks.block_at(wall).unwrap();
            assert!(
                commit_block_edit(
                    &mut chunks,
                    &mut journal,
                    &BlockEditRequest {
                        position: wall,
                        expected_material: before,
                        material: 3
                    },
                    &Default::default(),
                )
                .unwrap()
            );
            assert!(journal.gameplay_mut().drop_one(
                [
                    f64::from(body[0]) + 1.8,
                    f64::from(body[1]),
                    f64::from(body[2])
                ],
                ItemKind::Block(3)
            ));
        });
    tick_for(&mut host, &mut [&mut a], Duration::from_millis(350));
    assert_eq!(
        host.world()
            .resource::<EditJournal>()
            .gameplay()
            .drops
            .len(),
        1
    );
    host.world_mut()
        .resource_scope(|world, mut journal: Mut<EditJournal>| {
            let mut chunks = world.resource_mut::<ChunkStore>();
            assert!(
                commit_block_edit(
                    &mut chunks,
                    &mut journal,
                    &BlockEditRequest {
                        position: wall,
                        expected_material: 3,
                        material: 0
                    },
                    &Default::default(),
                )
                .unwrap()
            );
        });
    wait(&mut host, &mut [&mut a], |_, clients| {
        clients[0]
            .world()
            .resource::<client::ClientSession>()
            .inventory
            .count(ItemKind::Block(3))
            == 1
    });
}

#[test]
fn survival_prop_mining_creates_one_resource_without_removing_neighbors() {
    let (mut host, mut a, _, _) = fixture();
    wait(&mut host, &mut [&mut a], |_, clients| {
        clients[0]
            .world()
            .resource::<terra_world::streaming::WorldGeneration>()
            .status
            == terra_world::streaming::GenerationStatus::Ready
    });
    let state = a
        .world()
        .resource::<client::ClientSession>()
        .confirmed_state()
        .unwrap();
    let eye =
        Vec3::from_array(state.position) + Vec3::Y * terra_physics::character::PLAYER_EYE_OFFSET;
    let chunks = host.world().resource::<ChunkStore>();
    let (prop, look) = chunks.loaded.values().flat_map(|section| &section.props).filter(|prop| !prop.removed).find_map(|prop| {
        let target = Vec3::from_array(prop.position.map(|v| v as f32)) + Vec3::Y * 0.2;
        let rotation = Transform::from_translation(eye).looking_at(target, Vec3::Y).rotation;
        let (yaw, pitch, _) = rotation.to_euler(EulerRot::YXZ);
        if pitch.abs() > 1.4 { return None; }
        let (_, selected) = terra_world::targeting::pick_world(chunks, eye.to_array().map(f64::from), (rotation * Vec3::NEG_Z).to_array().map(f64::from));
        matches!(selected, Some(terra_world::targeting::WorldTarget::Prop(hit)) if hit.prop.id == prop.id).then_some((*prop, [yaw, pitch]))
    }).expect("reachable generated prop");
    let alive = chunks.loaded[&prop.section]
        .props
        .iter()
        .filter(|p| !p.removed)
        .count();
    let intent = MiningIntent {
        target: Some(MiningTarget::Prop {
            section: prop.section,
            id: prop.id,
        }),
        look,
    };
    held(&mut host, &mut a, intent, Duration::from_millis(400));
    wait(&mut host, &mut [&mut a], |_, clients| {
        clients[0]
            .world()
            .resource::<EditJournal>()
            .prop_removed(prop.section, prop.id)
    });
    assert_eq!(
        host.world().resource::<ChunkStore>().loaded[&prop.section]
            .props
            .iter()
            .filter(|p| !p.removed)
            .count(),
        alive - 1
    );
    let gameplay = host.world().resource::<EditJournal>().gameplay();
    let stored: u16 = gameplay
        .inventories
        .values()
        .flat_map(|bag| &bag.slots)
        .map(|s| s.count)
        .sum();
    let loose: u16 = gameplay.drops.iter().map(|d| d.stack.count).sum();
    assert_eq!(stored + loose, 1);
    assert_eq!(gameplay.next_drop, 1);
}

#[test]
fn legacy_density_packet_cannot_bypass_collectible_prop_occlusion() {
    let (mut host, mut a, _, _) = fixture();
    host.world_mut()
        .resource_mut::<GameConfig>()
        .server
        .game_mode = GameMode::Creative;
    wait(&mut host, &mut [&mut a], |_, clients| {
        clients[0]
            .world()
            .resource::<terra_world::streaming::WorldGeneration>()
            .status
            == terra_world::streaming::GenerationStatus::Ready
    });
    let state = a
        .world()
        .resource::<client::ClientSession>()
        .confirmed_state()
        .unwrap();
    let eye =
        Vec3::from_array(state.position) + Vec3::Y * terra_physics::character::PLAYER_EYE_OFFSET;
    let chunks = host.world().resource::<ChunkStore>();
    let (block, prop, look) = chunks.loaded.values().flat_map(|section| &section.props).filter(|prop| !prop.removed).find_map(|prop| {
        let target = Vec3::from_array(prop.position.map(|v| v as f32)) + Vec3::Y * 0.2;
        let rotation = Transform::from_translation(eye).looking_at(target, Vec3::Y).rotation;
        let (yaw, pitch, _) = rotation.to_euler(EulerRot::YXZ);
        if pitch.abs() > 1.4 { return None; }
        let (block, selected) = terra_world::targeting::pick_world(chunks, eye.to_array().map(f64::from), (rotation * Vec3::NEG_Z).to_array().map(f64::from));
        let block = block?;
        matches!(selected, Some(terra_world::targeting::WorldTarget::Prop(hit)) if hit.prop.id == prop.id).then_some((block, *prop, [yaw, pitch]))
    }).expect("natural prop occluding a solid block");
    let material = chunks.block_at(block.position).unwrap();
    send(&mut a, ClientControl::VegetationDensity(0.0));
    assert_eq!(
        edit(
            &mut host,
            &mut a,
            EditAction::Block {
                position: block.position,
                expected: material,
                material: 0
            },
            look
        ),
        EditOutcome::Occluded
    );
    assert_eq!(
        host.world()
            .resource::<ChunkStore>()
            .block_at(block.position),
        Some(material)
    );
    assert!(
        !host
            .world()
            .resource::<EditJournal>()
            .prop_removed(prop.section, prop.id)
    );
}
