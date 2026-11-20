use super::*;

const TABLE: [i64; 3] = [0, 1, 2];

fn table(chunks: &mut ChunkStore, journal: &mut EditJournal, material: u16) {
    set(chunks, TABLE, material);
    journal.record(TABLE, material).unwrap();
}

#[test]
fn lumberjack_demands_a_real_table_visits_it_before_changing_profession_then_works() {
    let (mut chunks, mut journal, terrain, mut runtime, npc) = fixture(WorkKind::Lumberjack);
    table(&mut chunks, &mut journal, 0);
    journal
        .gameplay_mut()
        .creatures
        .animals
        .get_mut(&npc)
        .unwrap()
        .variant = villagers::UNEMPLOYED;
    let sex = journal.gameplay().creatures.animals[&npc].sex();
    let start = journal.gameplay().creatures.animals[&npc].position;
    lumberjack::planted(&mut chunks, &mut journal, PlayerUuid::from_u128(1));
    lumberjack::put_tool(&mut journal, ItemKind::Tool(ToolKind::WoodAxe));
    run(&mut chunks, &mut journal, &terrain, &mut runtime, 100);
    assert_eq!(
        journal.gameplay().jobs.workers[&npc].status,
        WorkStatus::Waiting(WaitReason::MissingStation)
    );
    assert_eq!(
        journal.gameplay().creatures.animals[&npc].variant,
        villagers::UNEMPLOYED
    );
    assert_eq!(journal.gameplay().creatures.animals[&npc].position, start);
    assert_eq!(chunks.block_at([6, 1, 0]), Some(6));
    assert_eq!(
        journal
            .container_at([0, 1, 0])
            .unwrap()
            .count(ItemKind::Tool(ToolKind::WoodAxe)),
        1
    );
    assert!(
        journal.gameplay().jobs.workers[&npc]
            .inventory
            .slots
            .is_empty()
    );

    table(
        &mut chunks,
        &mut journal,
        terra_core::profession_stations::SMITH,
    );
    run(&mut chunks, &mut journal, &terrain, &mut runtime, 40);
    assert_eq!(
        journal.gameplay().jobs.workers[&npc].status,
        WorkStatus::Waiting(WaitReason::MissingStation)
    );
    assert_eq!(
        journal.gameplay().creatures.animals[&npc].variant,
        villagers::UNEMPLOYED
    );

    table(
        &mut chunks,
        &mut journal,
        terra_core::profession_stations::LUMBERJACK,
    );
    let mut changed = false;
    for _ in 0..100 {
        let before = journal.gameplay().creatures.animals[&npc].position;
        run(&mut chunks, &mut journal, &terrain, &mut runtime, 1);
        let actor = &journal.gameplay().creatures.animals[&npc];
        assert!(
            crate::creatures::distance(before, actor.position) < 0.5_f64.powi(2),
            "no teleport to table"
        );
        assert_eq!(actor.sex(), sex);
        if actor.variant == villagers::LUMBERJACK {
            assert!((actor.position[0] - 0.5).hypot(actor.position[2] - 2.5) < 2.1);
            changed = true;
            break;
        }
    }
    assert!(changed, "station must be physically reached");
    lumberjack::advance(&mut chunks, &mut journal, &terrain, &mut runtime, 650);
    assert_eq!(
        journal
            .container_at([0, 1, 0])
            .unwrap()
            .count(ItemKind::Block(6)),
        3
    );
    assert!(
        journal
            .gameplay()
            .trees
            .trees
            .values()
            .any(|tree| tree.stage == terra_core::TreeStage::Sapling)
    );
    assert!(journal.gameplay().jobs.workers[&npc].tree.is_none());
}

#[test]
fn removing_the_table_preserves_partial_tree_and_inventory_through_reload() {
    let (mut chunks, mut journal, terrain, mut runtime, npc) = fixture(WorkKind::Lumberjack);
    lumberjack::planted(&mut chunks, &mut journal, PlayerUuid::from_u128(1));
    lumberjack::put_tool(
        &mut journal,
        ItemKind::WornTool {
            tool: ToolKind::WoodAxe,
            remaining: 1,
        },
    );
    lumberjack::advance(&mut chunks, &mut journal, &terrain, &mut runtime, 350);
    let worker = journal.gameplay().jobs.workers[&npc].clone();
    assert_eq!(worker.status, WorkStatus::Waiting(WaitReason::MissingTool));
    assert_eq!(worker.tree.as_ref().unwrap().wood.len(), 2);
    table(&mut chunks, &mut journal, 0);
    let saved = serde_json::to_string(&journal.gameplay().jobs).unwrap();
    journal.gameplay_mut().jobs = serde_json::from_str(&saved).unwrap();
    runtime = WorkRuntime::default();
    lumberjack::put_tool(&mut journal, ItemKind::Tool(ToolKind::StoneAxe));
    lumberjack::advance(&mut chunks, &mut journal, &terrain, &mut runtime, 100);
    let waiting = &journal.gameplay().jobs.workers[&npc];
    assert_eq!(
        waiting.status,
        WorkStatus::Waiting(WaitReason::MissingStation)
    );
    assert_eq!(waiting.inventory, worker.inventory);
    assert_eq!(waiting.tree, worker.tree);
    assert_eq!(
        journal
            .container_at([0, 1, 0])
            .unwrap()
            .count(ItemKind::Tool(ToolKind::StoneAxe)),
        1
    );
    assert_eq!(
        journal
            .container_at([0, 1, 0])
            .unwrap()
            .count(ItemKind::Block(6)),
        1
    );
    table(
        &mut chunks,
        &mut journal,
        terra_core::profession_stations::LUMBERJACK,
    );
    lumberjack::advance(&mut chunks, &mut journal, &terrain, &mut runtime, 650);
    assert!(journal.gameplay().jobs.workers[&npc].tree.is_none());
    assert_eq!(
        journal
            .container_at([0, 1, 0])
            .unwrap()
            .count(ItemKind::Block(6)),
        3
    );
    assert!(journal.gameplay().valid());
}

#[test]
fn a_table_in_the_work_area_is_found_separately_from_the_supply_chest() {
    let (mut chunks, mut journal, terrain, mut runtime, npc) = fixture(WorkKind::Lumberjack);
    table(&mut chunks, &mut journal, 0);
    let station = [7, 1, 2];
    set(
        &mut chunks,
        station,
        terra_core::profession_stations::LUMBERJACK,
    );
    journal
        .record(station, terra_core::profession_stations::LUMBERJACK)
        .unwrap();
    journal
        .gameplay_mut()
        .creatures
        .animals
        .get_mut(&npc)
        .unwrap()
        .variant = villagers::UNEMPLOYED;
    run(&mut chunks, &mut journal, &terrain, &mut runtime, 1);
    assert_eq!(
        journal.gameplay().creatures.animals[&npc].variant,
        villagers::UNEMPLOYED
    );
    run(&mut chunks, &mut journal, &terrain, &mut runtime, 150);
    assert_eq!(
        journal.gameplay().creatures.animals[&npc].variant,
        villagers::LUMBERJACK
    );
    assert_eq!(
        journal.gameplay().jobs.workers[&npc].status,
        WorkStatus::Waiting(WaitReason::NoTargets)
    );
    assert_eq!(chunks.block_at([0, 1, 0]), Some(29));
}
