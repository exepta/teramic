use super::*;
use crate::protocol::session::{
    ClientControl, EditAction, EditCommand, EditOutcome, InventorySource,
};
use terra_core::{
    GameMode, Inventory, ItemKind, PlayerLocation, PlayerProfile, WoodSpecies,
    containers::ChestMerge,
};
use terra_world::edits::{BlockObjectState, EditJournal};

fn look(app: &App, target: [f32; 3]) -> [f32; 2] {
    let state = app
        .world()
        .resource::<client::ClientSession>()
        .confirmed_state()
        .unwrap();
    let eye =
        Vec3::from_array(state.position) + Vec3::Y * terra_physics::character::PLAYER_EYE_OFFSET;
    let direction = (Vec3::from_array(target) - eye).normalize();
    [(-direction.x).atan2(-direction.z), direction.y.asin()]
}

fn wait_stage(
    host: &mut App,
    clients: &mut [&mut App],
    stage: &str,
    condition: impl Fn(&App, &[&mut App]) -> bool,
) {
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        host.update();
        for replica in clients.iter_mut() {
            replica.update();
        }
        if condition(host, clients) {
            return;
        }
        if Instant::now() >= deadline {
            let states = clients
                .iter()
                .map(|replica| {
                    let session = replica.world().resource::<client::ClientSession>();
                    (
                        session.ready,
                        session.error.clone(),
                        session.last_edit,
                        session.confirmed_state(),
                        session.inventory.clone(),
                        [7, 8, 9, 10].map(|x| {
                            (
                                replica.world().resource::<ChunkStore>().block_at([x, 4, 9]),
                                replica
                                    .world()
                                    .resource::<EditJournal>()
                                    .object_state([x, 4, 9]),
                            )
                        }),
                    )
                })
                .collect::<Vec<_>>();
            panic!("Chest UDP stage {stage} timed out; client states: {states:#?}");
        }
        for replica in clients.iter() {
            assert!(
                replica
                    .world()
                    .resource::<client::ClientSession>()
                    .error
                    .is_none(),
                "Chest UDP stage {stage}: {:?}",
                replica.world().resource::<client::ClientSession>().error
            );
        }
        std::thread::sleep(Duration::from_millis(1));
    }
}

#[test]
fn real_udp_shift_chest_intent_places_separate_then_targeted_pair_and_replicates_without_losing_inventory()
 {
    let mut host = server();
    host.world_mut()
        .resource_mut::<GameConfig>()
        .server
        .game_mode = GameMode::Survival;
    host.world_mut()
        .resource_mut::<GameConfig>()
        .server
        .natural_mob_limit = Some(0);
    let owner = PlayerProfile::default();
    let observer = PlayerProfile::default();
    let observer_uuid = observer.uuid;
    let item = ItemKind::for_wood(29, WoodSpecies::Birch);
    let old = [7, 4, 9];
    {
        let mut journal = host.world_mut().resource_mut::<EditJournal>();
        for x in 5..=13 {
            for z in 6..=14 {
                for y in 3..=8 {
                    journal
                        .record([x, y, z], if y == 3 { 3 } else { 0 })
                        .unwrap();
                }
            }
        }
        journal.record(old, 29).unwrap();
        journal
            .set_object_state(
                old,
                Some(BlockObjectState {
                    wood: WoodSpecies::Spruce,
                    ..Default::default()
                }),
            )
            .unwrap();
        let mut contents = Inventory::default();
        contents.insert(ItemKind::Block(3), 37);
        journal.set_container(old, contents).unwrap();
        for (profile, position) in [(&owner, [8.5, 4.92, 12.5]), (&observer, [11.5, 4.92, 9.5])] {
            journal
                .update_player_location(
                    profile.uuid,
                    PlayerLocation {
                        position,
                        look: [0.; 2],
                        flying: false,
                    },
                )
                .unwrap();
        }
        let mut bag = Inventory::default();
        bag.insert(item, 3);
        journal
            .gameplay_mut()
            .inventories
            .insert(owner.uuid, bag.clone());
        journal
            .gameplay_mut()
            .inventories
            .insert(observer.uuid, bag);
    }
    let address = host
        .world()
        .resource::<server::ServerRuntime>()
        .address
        .unwrap();
    let mut a = profile_client(address, owner.clone());
    let mut b = profile_client(address, observer);
    wait_stage(
        &mut host,
        &mut [&mut a, &mut b],
        "welcome/terrain/inventory",
        |_, clients| {
            clients.iter().all(|replica| {
                let session = replica.world().resource::<client::ClientSession>();
                session.ready
                    && session
                        .confirmed_state()
                        .is_some_and(|state| state.grounded)
                    && replica.world().resource::<ChunkStore>().block_at([8, 4, 9]) == Some(0)
                    && replica
                        .world()
                        .resource::<EditJournal>()
                        .object_state(old)
                        .is_some()
            })
        },
    );
    // Ready/terrain acknowledgements can precede the private inventory snapshot.
    // A production edit must carry the actual latest source-inventory revision.
    wait_stage(
        &mut host,
        &mut [&mut a, &mut b],
        "private source inventory",
        |_, clients| {
            clients.iter().all(|replica| {
                replica
                    .world()
                    .resource::<client::ClientSession>()
                    .inventory
                    .count(item)
                    == 3
            })
        },
    );
    for replica in [&mut a, &mut b] {
        replica
            .world_mut()
            .resource_mut::<client::SelectedInventorySlot>()
            .0 = Some(0);
    }
    // The first player looks along -Z, so the intentionally separate chest has
    // facing 0. The second real player approaches its actual +X side; their own
    // facing differs, and the resulting pair must inherit the existing facing.
    for (actor, p, target, merge) in [
        (0, [8, 4, 9], [8.5, 4., 9.5], ChestMerge::Separate),
        (1, [9, 4, 9], [8.5, 4.5, 9.5], ChestMerge::Target([8, 4, 9])),
    ] {
        let replica = if actor == 0 { &mut a } else { &mut b };
        let aim = look(replica, target);
        replica
            .world_mut()
            .resource_mut::<client::ClientSession>()
            .look = aim;
        // Exercise the production client sender, not a handcrafted wire command.
        replica
            .world_mut()
            .write_message(client::ChestPlaceRequest {
                position: p,
                expected: 0,
                merge,
            });
        let stage = format!("player {actor} place: {merge:?}");
        wait_stage(&mut host, &mut [&mut a, &mut b], &stage, |_, clients| {
            if let Some((received, outcome)) = clients[actor]
                .world()
                .resource::<client::ClientSession>()
                .last_edit
                && received == 1
            {
                assert_eq!(
                    outcome,
                    EditOutcome::Applied,
                    "Chest UDP stage {stage}: real authority rejected placement"
                );
            }
            clients[actor]
                .world()
                .resource::<client::ClientSession>()
                .last_edit
                == Some((1, EditOutcome::Applied))
                && clients.iter().all(|replica| {
                    replica.world().resource::<ChunkStore>().block_at(p) == Some(29)
                        && replica
                            .world()
                            .resource::<EditJournal>()
                            .object_state(p)
                            .is_some_and(|state| {
                                state.peer
                                    == match merge {
                                        ChestMerge::Target(peer) => Some(peer),
                                        _ => None,
                                    }
                            })
                })
                && clients[0]
                    .world()
                    .resource::<client::ClientSession>()
                    .inventory
                    .count(item)
                    == 2
                && clients[1]
                    .world()
                    .resource::<client::ClientSession>()
                    .inventory
                    .count(item)
                    == if actor == 0 { 3 } else { 2 }
        });
    }
    for replica in [&a, &b] {
        let journal = replica.world().resource::<EditJournal>();
        assert_eq!(journal.object_state(old).unwrap().peer, None);
        assert_eq!(journal.object_state([8, 4, 9]).unwrap().facing, 0);
        assert_eq!(journal.object_state([9, 4, 9]).unwrap().facing, 0);
        assert_eq!(
            journal.object_state([8, 4, 9]).unwrap().peer,
            Some([9, 4, 9])
        );
        assert_eq!(
            journal.object_state([9, 4, 9]).unwrap().wood,
            WoodSpecies::Birch
        );
        // Chest contents are private and are not leaked by terrain replication.
        assert!(journal.container_at(old).is_none());
    }
    let old_inventory = host
        .world()
        .resource::<EditJournal>()
        .container_at(old)
        .unwrap()
        .clone();
    assert_eq!(old_inventory.count(ItemKind::Block(3)), 37);
    let aim = look(&b, [9.5, 4.5, 9.5]);
    let session = b.world().resource::<client::ClientSession>();
    let source_revision = session.inventory.revision;
    let revision = b
        .world()
        .resource::<EditJournal>()
        .revision(terra_voxel::global_to_section([10, 4, 9]).0);
    send(
        &mut b,
        ClientControl::PlaceChest {
            command: EditCommand {
                request: 2,
                revision,
                look: aim,
                source: Some(InventorySource {
                    slot: 0,
                    revision: source_revision,
                }),
                action: EditAction::Block {
                    position: [10, 4, 9],
                    expected: 0,
                    material: 29,
                },
            },
            // The actual click hits the pair at X=9, not this older free chest.
            merge: ChestMerge::Target(old),
        },
    );
    wait_stage(
        &mut host,
        &mut [&mut a, &mut b],
        "forged target rejection",
        |_, clients| {
            clients[1]
                .world()
                .resource::<client::ClientSession>()
                .last_edit
                == Some((2, EditOutcome::Invalid))
        },
    );
    let journal = host.world().resource::<EditJournal>();
    assert_eq!(journal.container_at(old), Some(&old_inventory));
    assert_eq!(journal.gameplay().inventories[&owner.uuid].count(item), 2);
    assert_eq!(
        journal.gameplay().inventories[&observer_uuid].count(item),
        2
    );
    assert_eq!(
        host.world().resource::<ChunkStore>().block_at([10, 4, 9]),
        Some(0)
    );
    assert_eq!(
        journal.object_state([9, 4, 9]).unwrap().peer,
        Some([8, 4, 9])
    );
}
