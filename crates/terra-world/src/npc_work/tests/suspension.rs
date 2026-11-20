use super::*;
#[test]
fn defending_worker_keeps_orders_inventory_and_world_progress_until_resumed() {
    let (mut chunks, mut journal, terrain, mut runtime, npc) = fixture(WorkKind::Collector);
    assert!(journal.gameplay_mut().drop_stack(
        [7.5, 1.2, 0.5],
        ItemStack {
            item: ItemKind::Block(3),
            count: 7
        },
        [0.0; 3],
        0.0
    ));
    run(&mut chunks, &mut journal, &terrain, &mut runtime, 5);
    let worker = journal.gameplay().jobs.workers[&npc].clone();
    let orders = journal.gameplay().jobs.orders.clone();
    let creature = journal.gameplay().creatures.animals[&npc].clone();
    assert!(runtime.travel.contains_key(&npc));
    runtime.door_requests.insert(
        [5, 1, 0],
        WorkerDoorRequest {
            npc,
            owner: PlayerUuid::from_u128(1),
            position: [5, 1, 0],
        },
    );
    runtime.set_suspended([npc]);
    assert!(!runtime.travel.contains_key(&npc));
    assert!(runtime.door_requests.is_empty());
    run(&mut chunks, &mut journal, &terrain, &mut runtime, 30);
    assert_eq!(journal.gameplay().jobs.workers[&npc], worker);
    assert_eq!(journal.gameplay().jobs.orders, orders);
    let after = &journal.gameplay().creatures.animals[&npc];
    assert_eq!(after.position, creature.position);
    assert_eq!(after.age, creature.age);
    assert_eq!(journal.gameplay().drops.len(), 1);
    assert_eq!(
        journal
            .container_at([0, 1, 0])
            .unwrap()
            .count(ItemKind::Block(3)),
        0
    );
    // A defender can move elsewhere while the preserved job remains suspended.
    journal
        .gameplay_mut()
        .creatures
        .animals
        .get_mut(&npc)
        .unwrap()
        .position = [5.5, 1.0, 2.5];
    runtime.travel.insert(npc, movement::Travel::default());
    runtime.set_suspended([]);
    assert!(!runtime.travel.contains_key(&npc));
    run(&mut chunks, &mut journal, &terrain, &mut runtime, 300);
    assert!(journal.gameplay().drops.is_empty());
    assert_eq!(
        journal
            .container_at([0, 1, 0])
            .unwrap()
            .count(ItemKind::Block(3)),
        7
    );
}
