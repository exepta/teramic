use super::*;
use crate::{
    protocol::npc_work::{WorkAction, WorkError},
    server::npc_work,
};
use terra_core::{
    creatures::{Creature, CreatureBehavior, CreatureKind, CreatureNeeds},
    npc_jobs::{WorkArea, WorkKind},
};

#[path = "server_npc_remote_tests.rs"]
mod remote_tests;

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
fn own_keep_can_inspect_a_remote_craftsman_bag_and_revokes_it_when_the_worker_leaves_the_realm() {
    use terra_core::{
        building_jobs::SettlementBuilding,
        kingdom::{ClaimActor, ClaimKind},
    };
    let (mut app, peer, old_npc, storage) = worker_fixture();
    apply(&mut app, &peer, assign(old_npc, storage)).unwrap();
    let npc = terra_core::creatures::villagers::id([0, 0], 25).unwrap();
    let kingdom = {
        let mut journal = app.world_mut().resource_mut::<EditJournal>();
        let game = journal.gameplay_mut();
        let kingdom = game
            .kingdoms
            .found(peer.uuid, [8, 4, 8], "Remote craftsmen")
            .unwrap();
        for x in 1..=2 {
            game.kingdoms
                .claim(
                    kingdom,
                    [x, 0],
                    ClaimKind::RealmTerritory,
                    ClaimActor::Player(peer.uuid),
                )
                .unwrap();
        }
        game.jobs.orders.clear();
        game.jobs.workers.clear();
        let mut settlement = SettlementBuilding::new(peer.uuid);
        settlement.craftsmen.push(npc);
        game.buildings.settlements.insert(kingdom, settlement);
        let mut bag = Inventory::default();
        assert_eq!(bag.insert(ItemKind::Block(3), 3), 0);
        game.buildings.inventories.insert(npc, bag);
        let mut creature = game.creatures.animals.remove(&old_npc).unwrap();
        game.creatures.owners.remove(&old_npc);
        game.creatures.owners.insert(npc, peer.uuid);
        creature.id = npc;
        creature.variant = terra_core::creatures::villagers::CRAFTSMAN;
        creature.position = [40.5, 4.0, 6.5];
        game.creatures.animals.insert(npc, creature);
        assert!(game.valid());
        kingdom
    };
    apply(&mut app, &peer, WorkAction::InspectNpc { npc }).unwrap();
    let snapshot = private_snapshot(&app, &peer, Some(npc), [5.5, 4.0, 6.5]);
    assert_eq!(
        snapshot.detail.unwrap().inventory.count(ItemKind::Block(3)),
        3
    );
    assert_eq!(snapshot.workers[0].carried_slots, 1);
    app.world_mut()
        .resource_mut::<EditJournal>()
        .gameplay_mut()
        .kingdoms
        .release(peer.uuid, kingdom, [2, 0])
        .unwrap();
    assert!(
        private_snapshot(&app, &peer, Some(npc), [5.5, 4.0, 6.5])
            .detail
            .is_none()
    );
    assert!(
        private_snapshot(&app, &peer, None, [5.5, 4.0, 6.5])
            .detail
            .is_none(),
        "the ordinary worker list never carries private bags"
    );
}

#[test]
fn lumberjack_assignment_requests_a_station_before_changing_profession_or_inventing_supplies() {
    let (mut app, peer, npc, storage) = worker_fixture();
    let before = app
        .world()
        .resource::<EditJournal>()
        .gameplay()
        .creatures
        .animals[&npc]
        .clone();
    let mut action = assign(npc, storage);
    if let WorkAction::Assign { kind, .. } = &mut action {
        *kind = WorkKind::Lumberjack;
    }
    apply(&mut app, &peer, action).unwrap();
    let game = app.world().resource::<EditJournal>().gameplay();
    let worker = &game.jobs.workers[&npc];
    assert_eq!(
        worker.status,
        terra_core::npc_jobs::WorkStatus::Waiting(terra_core::npc_jobs::WaitReason::MissingStation)
    );
    assert_eq!(worker.inventory.occupied_slots(), 0);
    assert_eq!(game.creatures.animals[&npc].variant, before.variant);
    assert_eq!(game.creatures.animals[&npc].sex(), before.sex());
    assert_eq!(game.creatures.owners[&npc], peer.uuid);
}

#[test]
fn a_craftsman_without_a_project_can_return_stock_through_owned_doors_but_cannot_keep_opening_them_after_delivery()
 {
    use terra_core::building_jobs::{BuildArea, SettlementBuilding};
    use terra_world::{edits::BlockObjectState, npc_work::WorkerDoorRequest};
    let (mut app, peer, npc, storage) = worker_fixture();
    let door = [6, 4, 6];
    let upper = [6, 5, 6];
    for (cell, secondary, other) in [(door, false, upper), (upper, true, door)] {
        let (section, local) = global_to_section(cell);
        app.world_mut()
            .resource_mut::<ChunkStore>()
            .loaded
            .get_mut(&section)
            .unwrap()
            .data
            .set_block(local, 65)
            .unwrap();
        let mut journal = app.world_mut().resource_mut::<EditJournal>();
        journal.record(cell, 65).unwrap();
        journal
            .set_object_state(
                cell,
                Some(BlockObjectState {
                    peer: Some(other),
                    secondary,
                    ..Default::default()
                }),
            )
            .unwrap();
    }
    let kingdom = {
        let mut journal = app.world_mut().resource_mut::<EditJournal>();
        let game = journal.gameplay_mut();
        let kingdom = game
            .kingdoms
            .found(peer.uuid, [12, 4, 12], "Returning stock")
            .unwrap();
        game.creatures.owners.insert(npc, peer.uuid);
        game.creatures.animals.get_mut(&npc).unwrap().variant =
            terra_core::creatures::villagers::CRAFTSMAN;
        let mut site = SettlementBuilding::new(peer.uuid);
        site.storage = Some(BuildArea::new(storage, storage).unwrap());
        site.craftsmen.push(npc);
        game.buildings.settlements.insert(kingdom, site);
        let mut bag = Inventory::default();
        assert_eq!(bag.insert(ItemKind::Block(16), 3), 0);
        game.buildings.inventories.insert(npc, bag);
        kingdom
    };
    let check = |app: &App| {
        npc_work::authorized_worker_doors(
            app.world().resource::<ChunkStore>(),
            app.world().resource::<EditJournal>(),
            std::iter::once(WorkerDoorRequest {
                npc,
                owner: peer.uuid,
                position: door,
            }),
        )
    };
    assert_eq!(check(&app), [door].into());
    let snapshot = private_snapshot(&app, &peer, Some(npc), [5.5, 4.0, 6.5]);
    assert_eq!(
        snapshot.workers[0].status,
        terra_core::npc_jobs::WorkStatus::Delivering
    );
    assert_eq!(
        snapshot
            .detail
            .unwrap()
            .inventory
            .count(ItemKind::Block(16)),
        3
    );
    app.world_mut()
        .resource_mut::<EditJournal>()
        .gameplay_mut()
        .buildings
        .inventories
        .insert(npc, Inventory::default());
    assert!(check(&app).is_empty(), "an empty bag is no return job");
    let mut bag = Inventory::default();
    assert_eq!(bag.insert(ItemKind::Block(16), 3), 0);
    {
        let mut journal = app.world_mut().resource_mut::<EditJournal>();
        journal
            .gameplay_mut()
            .buildings
            .inventories
            .insert(npc, bag);
        journal
            .gameplay_mut()
            .buildings
            .settlements
            .get_mut(&kingdom)
            .unwrap()
            .craftsmen
            .clear();
    }
    assert!(
        check(&app).is_empty(),
        "an unregistered worker cannot use the return exception"
    );
    {
        let mut journal = app.world_mut().resource_mut::<EditJournal>();
        let game = journal.gameplay_mut();
        game.buildings
            .settlements
            .get_mut(&kingdom)
            .unwrap()
            .craftsmen
            .push(npc);
        game.kingdoms = Default::default();
        game.kingdoms
            .found(PlayerUuid::new_v4(), door, "Foreign doorway")
            .unwrap();
    }
    assert!(
        check(&app).is_empty(),
        "returning stock never grants foreign opening rights"
    );
}

#[test]
fn worker_door_authority_rechecks_owner_order_loaded_halves_and_material() {
    use terra_world::{edits::BlockObjectState, npc_work::WorkerDoorRequest};
    let (mut app, peer, npc, storage) = worker_fixture();
    apply(&mut app, &peer, assign(npc, storage)).unwrap();
    let door = [6, 4, 6];
    let upper = [6, 5, 6];
    for (cell, secondary, other) in [(door, false, upper), (upper, true, door)] {
        let (section, local) = global_to_section(cell);
        app.world_mut()
            .resource_mut::<ChunkStore>()
            .loaded
            .get_mut(&section)
            .unwrap()
            .data
            .set_block(local, 65)
            .unwrap();
        let mut journal = app.world_mut().resource_mut::<EditJournal>();
        journal.record(cell, 65).unwrap();
        journal
            .set_object_state(
                cell,
                Some(BlockObjectState {
                    peer: Some(other),
                    secondary,
                    ..Default::default()
                }),
            )
            .unwrap();
    }
    let check = |app: &App, actor| {
        npc_work::authorized_worker_doors(
            app.world().resource::<ChunkStore>(),
            app.world().resource::<EditJournal>(),
            std::iter::once(WorkerDoorRequest {
                npc,
                owner: actor,
                position: door,
            }),
        )
    };
    assert_eq!(check(&app, peer.uuid), [door].into());
    assert!(check(&app, terra_core::PlayerUuid::new_v4()).is_empty());
    app.world_mut()
        .resource_mut::<EditJournal>()
        .gameplay_mut()
        .kingdoms
        .found(terra_core::PlayerUuid::new_v4(), door, "Protected doorway")
        .unwrap();
    assert!(
        check(&app, peer.uuid).is_empty(),
        "ownership of the worker grants no foreign opening rights"
    );
    app.world_mut()
        .resource_mut::<EditJournal>()
        .gameplay_mut()
        .kingdoms = Default::default();
    let order = app
        .world()
        .resource::<EditJournal>()
        .gameplay()
        .jobs
        .orders
        .values()
        .next()
        .unwrap()
        .clone();
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
    assert!(
        check(&app, peer.uuid).is_empty(),
        "paused orders cannot open doors"
    );
    app.world_mut()
        .resource_mut::<EditJournal>()
        .gameplay_mut()
        .jobs
        .orders
        .get_mut(&order.id)
        .unwrap()
        .enabled = true;
    let (section, local) = global_to_section(upper);
    app.world_mut()
        .resource_mut::<ChunkStore>()
        .loaded
        .get_mut(&section)
        .unwrap()
        .data
        .set_block(local, 0)
        .unwrap();
    assert!(
        check(&app, peer.uuid).is_empty(),
        "stale or missing second halves cannot be opened"
    );
    for cell in [door, upper] {
        let (section, local) = global_to_section(cell);
        app.world_mut()
            .resource_mut::<ChunkStore>()
            .loaded
            .get_mut(&section)
            .unwrap()
            .data
            .set_block(local, 248)
            .unwrap();
        app.world_mut()
            .resource_mut::<EditJournal>()
            .record(cell, 248)
            .unwrap();
    }
    assert!(
        check(&app, peer.uuid).is_empty(),
        "iron cab doors are not operated automatically"
    );
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

fn private_snapshot(
    app: &App,
    peer: &Peer,
    npc: Option<u64>,
    position: [f64; 3],
) -> crate::protocol::npc_work::WorkSnapshot {
    npc_work::snapshot(
        peer,
        position,
        app.world().resource::<ChunkStore>(),
        app.world().resource::<EditJournal>(),
        npc,
        app.world().resource::<GameConfig>(),
    )
}

#[test]
fn direct_inspection_of_unassigned_workers_returns_an_authoritative_empty_nine_slot_bag() {
    let (mut app, peer, npc, _storage) = worker_fixture();
    let before = app.world().resource::<EditJournal>().gameplay().clone();
    assert_eq!(
        apply(&mut app, &peer, WorkAction::InspectNpc { npc }),
        Ok(())
    );
    assert_eq!(
        app.world().resource::<EditJournal>().gameplay(),
        &before,
        "inspection never adopts a worker or creates an order"
    );
    let snapshot = private_snapshot(&app, &peer, Some(npc), [5.5, 4.0, 6.5]);
    assert!(snapshot.valid());
    let bag = &snapshot.detail.as_ref().unwrap().inventory;
    assert!(bag.valid() && bag.slots.len() <= terra_core::npc_jobs::WORKER_SLOTS);
    assert_eq!(bag.occupied_slots(), 0);
    assert!(
        private_snapshot(&app, &peer, None, [5.5, 4.0, 6.5])
            .detail
            .is_none(),
        "ordinary work subscriptions disclose no inventory contents"
    );
}

#[test]
fn direct_worker_inventory_is_private_and_rechecks_owner_range_claims_and_build_permission() {
    let (mut app, mut peer, npc, storage) = worker_fixture();
    apply(&mut app, &peer, assign(npc, storage)).unwrap();
    let owner = peer.uuid;
    {
        let mut state = app.world_mut().resource_mut::<EditJournal>();
        state
            .gameplay_mut()
            .jobs
            .workers
            .get_mut(&npc)
            .unwrap()
            .insert(ItemKind::Block(3), 17);
    }
    let snapshot = private_snapshot(&app, &peer, Some(npc), [5.5, 4.0, 6.5]);
    assert!(snapshot.valid());
    assert_eq!(
        snapshot
            .detail
            .as_ref()
            .unwrap()
            .inventory
            .count(ItemKind::Block(3)),
        17
    );
    assert!(
        private_snapshot(&app, &peer, Some(npc), [25.5, 4.0, 6.5])
            .detail
            .is_none(),
        "the existing private subscription loses its inventory out of reach"
    );
    peer.uuid = terra_core::PlayerUuid::new_v4();
    assert_eq!(
        apply(&mut app, &peer, WorkAction::InspectNpc { npc }),
        Err(WorkError::Protected)
    );
    assert!(
        private_snapshot(&app, &peer, Some(npc), [5.5, 4.0, 6.5])
            .detail
            .is_none()
    );
    peer.uuid = owner;
    peer.permissions.build = false;
    assert!(
        private_snapshot(&app, &peer, Some(npc), [5.5, 4.0, 6.5])
            .detail
            .is_none()
    );
    peer.permissions.build = true;
    app.world_mut()
        .resource_mut::<EditJournal>()
        .gameplay_mut()
        .kingdoms
        .found(
            terra_core::PlayerUuid::new_v4(),
            [5, 4, 6],
            "Foreign work claim",
        )
        .unwrap();
    assert!(
        private_snapshot(&app, &peer, Some(npc), [5.5, 4.0, 6.5])
            .detail
            .is_none(),
        "claim changes revoke an already open private subscription"
    );
    assert_eq!(
        app.world()
            .resource::<EditJournal>()
            .gameplay()
            .jobs
            .workers[&npc]
            .inventory
            .count(ItemKind::Block(3)),
        17
    );
}

#[test]
fn direct_worker_selection_survives_the_snapshot_cap_without_bypassing_authority_filters() {
    let (mut app, peer, npc, _) = worker_fixture();
    {
        let mut journal = app.world_mut().resource_mut::<EditJournal>();
        let game = journal.gameplay_mut();
        let selected = game.creatures.animals[&npc].clone();
        for id in 100..240 {
            let mut other = selected.clone();
            other.id = id;
            game.creatures.animals.insert(id, other);
        }
    }
    let ordinary = private_snapshot(&app, &peer, None, [5.5, 4.0, 6.5]);
    assert_eq!(ordinary.workers.len(), 128);
    assert!(!ordinary.workers.iter().any(|worker| worker.npc == npc));
    let direct = private_snapshot(&app, &peer, Some(npc), [5.5, 4.0, 6.5]);
    assert_eq!(direct.workers.len(), 128);
    assert_eq!(direct.workers[0].npc, npc);
    assert_eq!(direct.detail.as_ref().unwrap().npc, npc);
    assert!(
        direct.valid(),
        "the selected lookup is not duplicated by the remaining iterator"
    );
    app.world_mut()
        .resource_mut::<EditJournal>()
        .gameplay_mut()
        .creatures
        .owners
        .insert(npc, terra_core::PlayerUuid::new_v4());
    let foreign = private_snapshot(&app, &peer, Some(npc), [5.5, 4.0, 6.5]);
    assert_eq!(foreign.workers.len(), 128);
    assert!(!foreign.workers.iter().any(|worker| worker.npc == npc));
    assert!(foreign.detail.is_none());
    {
        let mut journal = app.world_mut().resource_mut::<EditJournal>();
        let creatures = &mut journal.gameplay_mut().creatures;
        creatures.owners.remove(&npc);
        creatures.animals.get_mut(&npc).unwrap().position[0] = 5000.0;
    }
    let distant = private_snapshot(&app, &peer, Some(npc), [5.5, 4.0, 6.5]);
    assert!(!distant.workers.iter().any(|worker| worker.npc == npc));
    assert!(distant.detail.is_none());
}

#[test]
fn an_open_private_worker_inventory_is_revoked_when_the_npc_section_unloads() {
    let (mut app, peer, npc, _) = worker_fixture();
    let position = [5.5, 4.0, 6.5];
    assert!(
        private_snapshot(&app, &peer, Some(npc), position)
            .detail
            .is_some()
    );
    let (section, _) = global_to_section([5, 4, 6]);
    app.world_mut()
        .resource_mut::<ChunkStore>()
        .loaded
        .remove(&section);
    let snapshot = private_snapshot(&app, &peer, Some(npc), position);
    assert!(
        snapshot.detail.is_none(),
        "an old subscription cannot infer access from evicted village metadata"
    );
    assert!(snapshot.valid());
}
