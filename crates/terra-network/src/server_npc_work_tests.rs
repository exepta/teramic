use super::*;
use crate::{
    protocol::npc_work::{WorkAction, WorkError},
    server::npc_work,
};
use terra_core::{
    creatures::{Creature, CreatureBehavior, CreatureKind, CreatureNeeds},
    npc_jobs::{WorkArea, WorkKind},
};

fn worker_fixture() -> (App, Peer, u64, [i64; 3]) {
    let (mut app, peer, _) = fixture(ResourcePropKind::SmallStones, false);
    let npc = 7001;
    let storage = [8, 4, 8];
    let (owner, local) = global_to_section(storage);
    app.world_mut()
        .resource_mut::<ChunkStore>()
        .loaded
        .get_mut(&owner)
        .unwrap()
        .data
        .set_block(local, 29)
        .unwrap();
    let mut journal = app.world_mut().resource_mut::<EditJournal>();
    journal.record(storage, 29).unwrap();
    journal
        .set_container(storage, Inventory::default())
        .unwrap();
    journal.gameplay_mut().creatures.animals.insert(
        npc,
        Creature {
            id: npc,
            kind: CreatureKind::Villager,
            home: [0, 0],
            position: [5.5, 4.0, 6.5],
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
    (app, peer, npc, storage)
}

fn assign(npc: u64, storage: [i64; 3]) -> WorkAction {
    WorkAction::Assign {
        npc,
        kind: WorkKind::Collector,
        area: WorkArea::new([2, 4, 2], [12, 5, 12]).unwrap(),
        storage,
        priority: 7,
    }
}

fn apply(app: &mut App, peer: &Peer, action: WorkAction) -> Result<(), WorkError> {
    let mut state = SystemState::<ControlWorld>::new(app.world_mut());
    let mut world = state.get_mut(app.world_mut()).unwrap();
    npc_work::apply(action, peer, &mut world)
}

#[test]
fn npc_work_returned_worn_axes_keep_authoritative_player_mining_speed() {
    let (mut app, _peer, _npc, storage) = worker_fixture();
    let mut state = SystemState::<ControlWorld>::new(app.world_mut());
    let world = state.get_mut(app.world_mut()).unwrap();
    let target = crate::protocol::session::MiningTarget::Block {
        position: storage,
        expected: 6,
    };
    for tool in [
        terra_core::ToolKind::WoodAxe,
        terra_core::ToolKind::StoneAxe,
        terra_core::ToolKind::IronAxe,
        terra_core::ToolKind::GoldAxe,
        terra_core::ToolKind::DiamondAxe,
        terra_core::ToolKind::RubyAxe,
    ] {
        let worn = Some(ItemKind::WornTool { tool, remaining: 1 });
        let fresh = Some(ItemKind::Tool(tool));
        let worn_duration = crate::server::gameplay::tool_duration(target, worn, &world, None);
        assert_eq!(
            worn_duration,
            crate::server::gameplay::tool_duration(target, fresh, &world, None),
            "{tool:?}"
        );
        assert!(worn_duration < crate::server::gameplay::tool_duration(target, None, &world, None));
    }
}

#[test]
fn npc_work_assignment_is_authoritative_owned_and_preserves_carried_items_on_pause() {
    let (mut app, peer, npc, storage) = worker_fixture();
    apply(&mut app, &peer, assign(npc, storage)).unwrap();
    let jobs = &app.world().resource::<EditJournal>().gameplay().jobs;
    let order = jobs.orders.values().next().unwrap().clone();
    assert_eq!(order.npc, npc);
    assert_eq!(order.priority, 7);
    assert!(jobs.valid());
    app.world_mut()
        .resource_mut::<EditJournal>()
        .gameplay_mut()
        .jobs
        .workers
        .get_mut(&npc)
        .unwrap()
        .insert(ItemKind::Block(3), 17);
    apply(
        &mut app,
        &peer,
        WorkAction::SetEnabled {
            id: order.id,
            revision: order.revision,
            enabled: false,
        },
    )
    .unwrap();
    let jobs = &app.world().resource::<EditJournal>().gameplay().jobs;
    assert_eq!(jobs.workers[&npc].inventory.count(ItemKind::Block(3)), 17);
    assert!(!jobs.orders[&order.id].enabled);
    let before = jobs.clone();
    assert_eq!(
        apply(
            &mut app,
            &peer,
            WorkAction::SetEnabled {
                id: order.id,
                revision: order.revision,
                enabled: true
            }
        ),
        Err(WorkError::Stale)
    );
    assert_eq!(
        app.world().resource::<EditJournal>().gameplay().jobs,
        before
    );
    let mut foreign = peer;
    foreign.uuid = PlayerUuid::new_v4();
    assert_eq!(
        apply(&mut app, &foreign, assign(npc, storage)),
        Err(WorkError::Protected)
    );
    assert_eq!(
        app.world().resource::<EditJournal>().gameplay().jobs,
        before
    );
}

#[test]
fn npc_work_rejects_forged_areas_stores_and_missing_permissions_without_mutation() {
    let (mut app, mut peer, npc, storage) = worker_fixture();
    let before = app.world().resource::<EditJournal>().gameplay().clone();
    let mut invalid = assign(npc, storage);
    if let WorkAction::Assign { area, .. } = &mut invalid {
        area.max[0] += 64;
    }
    assert_eq!(apply(&mut app, &peer, invalid), Err(WorkError::Invalid));
    assert_eq!(
        apply(&mut app, &peer, assign(npc, [9, 4, 8])),
        Err(WorkError::Storage)
    );
    peer.permissions.build = false;
    assert_eq!(
        apply(&mut app, &peer, assign(npc, storage)),
        Err(WorkError::Protected)
    );
    assert_eq!(app.world().resource::<EditJournal>().gameplay(), &before);
}
