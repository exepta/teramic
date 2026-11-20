//! Founding and work discovery through real, authenticated UDP control messages.
use super::*;
use crate::protocol::npc_work::WorkAction;
use terra_core::creatures::{CreatureKind, villagers};

fn roles(host: &App, owner: PlayerUuid) -> std::collections::BTreeMap<u8, usize> {
    let game = host.world().resource::<EditJournal>().gameplay();
    let mut roles = std::collections::BTreeMap::new();
    for npc in game.creatures.animals.values().filter(|npc| {
        npc.kind == CreatureKind::Villager && game.creatures.owners.get(&npc.id) == Some(&owner)
    }) {
        *roles.entry(npc.variant).or_default() += 1;
    }
    roles
}

#[test]
fn kingdom_starters_are_replicated_assignable_and_never_regranted_by_rpc_replay() {
    let mut host = server();
    host.world_mut()
        .resource_mut::<terra_core::WorldClock>()
        .paused = true;
    host.world_mut()
        .resource_mut::<terra_core::WorldClock>()
        .phase = 0.5;
    let address = host
        .world()
        .resource::<server::ServerRuntime>()
        .address
        .unwrap();
    let profile = PlayerProfile::default();
    let mut client = profile_client(address, profile.clone());
    wait(&mut host, &mut [&mut client], |_, clients| {
        let session = clients[0].world().resource::<client::ClientSession>();
        session.ready && session.confirmed_state().is_some_and(|pose| pose.grounded)
    });
    let pose = client
        .world()
        .resource::<client::ClientSession>()
        .confirmed_state()
        .unwrap();
    let position = [
        pose.position[0].floor() as i64,
        (pose.position[1] + terra_physics::character::PLAYER_EYE_OFFSET).floor() as i64,
        pose.position[2].floor() as i64 - 3,
    ];
    edit(&mut host, position, terra_core::castle::BERGFRIED).unwrap();
    aim(&mut host, &mut [&mut client], 0, position);
    let command = client
        .world_mut()
        .resource_mut::<client::ClientSession>()
        .kingdom_request(KingdomAction::Found {
            position,
            name: "Starter cohort".into(),
        })
        .unwrap();
    let replay = command.clone();
    client.world_mut().write_message(command);
    wait(&mut host, &mut [&mut client], |_, clients| {
        clients[0]
            .world()
            .resource::<client::ClientSession>()
            .last_kingdom_result
            .is_some()
    });
    let KingdomOutcome::Founded { id } = client
        .world()
        .resource::<client::ClientSession>()
        .last_kingdom_result
        .as_ref()
        .unwrap()
        .outcome
    else {
        panic!("safe flat founding rejected")
    };
    let expected = [(10, 3), (12, 2), (11, 3), (villagers::UNEMPLOYED, 10)]
        .into_iter()
        .collect::<std::collections::BTreeMap<_, _>>();
    assert_eq!(roles(&host, profile.uuid), expected);
    let receipt = host
        .world()
        .resource::<EditJournal>()
        .gameplay()
        .kingdom_starters
        .grants[&id]
        .clone();
    wait(&mut host, &mut [&mut client], |_, clients| {
        let replica = clients[0].world().resource::<client::ReplicatedCreatures>();
        receipt
            .ids
            .iter()
            .all(|id| replica.creatures.contains_key(id))
    });
    let work = client
        .world_mut()
        .resource_mut::<client::ClientSession>()
        .make_work_request(WorkAction::Inspect)
        .unwrap();
    client.world_mut().write_message(work);
    wait(&mut host, &mut [&mut client], |_, clients| {
        clients[0]
            .world()
            .resource::<client::ClientSession>()
            .work
            .workers
            .iter()
            .filter(|worker| receipt.ids.contains(&worker.npc))
            .count()
            == 10
    });
    let before = client
        .world()
        .resource::<client::ClientSession>()
        .kingdom_response;
    client.world_mut().write_message(replay);
    // The client intentionally ignores results carrying an old nonce. A fresh
    // Inspect on the same ordered channel is the barrier proving replay delivery.
    assert_eq!(
        request(
            &mut host,
            &mut [&mut client],
            0,
            KingdomAction::Inspect { position }
        ),
        KingdomOutcome::Inspected { kingdom: Some(id) }
    );
    assert_eq!(
        client
            .world()
            .resource::<client::ClientSession>()
            .kingdom_response,
        before + 1
    );
    assert_eq!(roles(&host, profile.uuid), expected);
    assert_eq!(
        host.world()
            .resource::<EditJournal>()
            .gameplay()
            .kingdom_starters
            .grants[&id],
        receipt
    );
    assert_eq!(
        request(
            &mut host,
            &mut [&mut client],
            0,
            KingdomAction::Found {
                position,
                name: "Duplicate grant".into()
            }
        ),
        KingdomOutcome::Rejected(KingdomError::AlreadyMember)
    );
    assert_eq!(roles(&host, profile.uuid), expected);
}
