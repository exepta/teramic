use super::*;
use crate::protocol::npc_work::{WorkAction, WorkError};
use terra_core::{
    creatures::{Creature, CreatureBehavior, CreatureKind, CreatureNeeds},
    npc_jobs::{WorkArea, WorkKind},
};
use terra_world::edits::EditJournal;

fn command(
    host: &mut App,
    clients: &mut [&mut App],
    actor: usize,
    action: WorkAction,
) -> Option<WorkError> {
    let request = clients[actor]
        .world_mut()
        .resource_mut::<client::ClientSession>()
        .make_work_request(action)
        .unwrap();
    let id = request.0.request;
    clients[actor].world_mut().write_message(request);
    wait(host, clients, |_, clients| {
        clients[actor]
            .world()
            .resource::<client::ClientSession>()
            .last_work_result
            .as_ref()
            .is_some_and(|result| result.request == id)
    });
    clients[actor]
        .world()
        .resource::<client::ClientSession>()
        .last_work_result
        .as_ref()
        .unwrap()
        .error
        .clone()
}

#[test]
fn work_orders_replicate_over_udp_and_foreign_clients_cannot_reassign_or_pause_workers() {
    let mut host = server();
    let address = host
        .world()
        .resource::<server::ServerRuntime>()
        .address
        .unwrap();
    let owner = terra_core::PlayerProfile {
        name: "Worker owner".into(),
        ..Default::default()
    };
    let visitor = terra_core::PlayerProfile {
        name: "Worker visitor".into(),
        ..Default::default()
    };
    let mut a = profile_client(address, owner.clone());
    let mut b = profile_client(address, visitor);
    wait(&mut host, &mut [&mut a, &mut b], |_, clients| {
        clients.iter().all(|app| {
            let session = app.world().resource::<client::ClientSession>();
            session.ready && session.confirmed_state().is_some()
        })
    });
    let position = a
        .world()
        .resource::<client::ClientSession>()
        .confirmed_state()
        .unwrap()
        .position
        .map(f64::from);
    let npc = 780058;
    let cell = position.map(|v| v.floor() as i64);
    let storage = [cell[0] + 4, cell[1], cell[2] + 2];
    let (section, local) = terra_voxel::global_to_section(storage);
    host.world_mut()
        .resource_mut::<ChunkStore>()
        .loaded
        .get_mut(&section)
        .unwrap()
        .data
        .set_block(local, 29)
        .unwrap();
    {
        let mut journal = host.world_mut().resource_mut::<EditJournal>();
        journal.record(storage, 29).unwrap();
        journal
            .set_container(storage, terra_core::Inventory::default())
            .unwrap();
        journal.gameplay_mut().creatures.animals.insert(
            npc,
            Creature {
                id: npc,
                home: [0, 0],
                kind: CreatureKind::Villager,
                position: [position[0] + 2.0, position[1], position[2]],
                velocity: [0.0; 3],
                yaw: 0.0,
                health: CreatureKind::Villager.health(),
                behavior: CreatureBehavior::Rest,
                timer: 0.0,
                age: 0.0,
                variant: terra_core::creatures::villagers::UNEMPLOYED,
                needs: CreatureNeeds::full(CreatureKind::Villager),
            },
        );
    }
    assert_eq!(
        command(&mut host, &mut [&mut a, &mut b], 0, WorkAction::Inspect),
        None
    );
    wait(&mut host, &mut [&mut a, &mut b], |_, clients| {
        clients[0]
            .world()
            .resource::<client::ClientSession>()
            .work
            .workers
            .iter()
            .any(|worker| worker.npc == npc)
    });
    let action = WorkAction::Assign {
        npc,
        kind: WorkKind::Collector,
        area: WorkArea::new(
            [cell[0], cell[1], cell[2]],
            [cell[0] + 6, cell[1] + 1, cell[2] + 6],
        )
        .unwrap(),
        storage,
        priority: 8,
    };
    assert_eq!(
        command(&mut host, &mut [&mut a, &mut b], 0, action.clone()),
        None
    );
    wait(&mut host, &mut [&mut a, &mut b], |_, clients| {
        clients[0]
            .world()
            .resource::<client::ClientSession>()
            .work
            .workers
            .iter()
            .any(|worker| worker.npc == npc && worker.order.is_some())
    });
    let order = a
        .world()
        .resource::<client::ClientSession>()
        .work
        .workers
        .iter()
        .find(|worker| worker.npc == npc)
        .unwrap()
        .order
        .clone()
        .unwrap();
    assert_eq!(order.owner, owner.uuid);
    assert_eq!(
        command(&mut host, &mut [&mut a, &mut b], 1, action),
        Some(WorkError::Protected)
    );
    assert_eq!(
        command(
            &mut host,
            &mut [&mut a, &mut b],
            1,
            WorkAction::SetEnabled {
                id: order.id,
                revision: order.revision,
                enabled: false
            }
        ),
        Some(WorkError::Protected)
    );
    assert_eq!(
        command(
            &mut host,
            &mut [&mut a, &mut b],
            0,
            WorkAction::SetEnabled {
                id: order.id,
                revision: order.revision,
                enabled: false
            }
        ),
        None
    );
    wait(&mut host, &mut [&mut a, &mut b], |_, clients| {
        clients[0]
            .world()
            .resource::<client::ClientSession>()
            .work
            .workers
            .iter()
            .any(|worker| {
                worker.npc == npc && worker.order.as_ref().is_some_and(|order| !order.enabled)
            })
    });
    assert!(
        !host
            .world()
            .resource::<EditJournal>()
            .gameplay()
            .jobs
            .orders[&order.id]
            .enabled
    );
}
