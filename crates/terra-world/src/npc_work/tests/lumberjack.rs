use super::*;
use terra_core::{TreeRecord, TreeSpecies, TreeStage, npc_jobs::TreeWorkStage};

pub(super) fn planted(chunks: &mut ChunkStore, journal: &mut EditJournal, owner: PlayerUuid) {
    let root = [6, 1, 0];
    set(chunks, [6, 0, 0], 2);
    let wood = vec![[0, 0, 0], [0, 1, 0], [0, 2, 0]];
    let leaves = vec![[-1, 2, 0], [1, 2, 0], [0, 3, 0]];
    for (offset, material) in wood
        .iter()
        .map(|p| (*p, 6))
        .chain(leaves.iter().map(|p| (*p, 7)))
    {
        let cell = std::array::from_fn(|axis| root[axis] + i64::from(offset[axis]));
        set(chunks, cell, material);
        journal.record(cell, material).unwrap();
    }
    assert!(journal.gameplay_mut().trees.insert(TreeRecord {
        id: 5019,
        root,
        species: TreeSpecies::Oak,
        seed: 5019,
        planted_by: Some(owner),
        stage: TreeStage::Mature,
        growth_seconds: 900.0,
        bounds: Some([[-1, 0, 0], [1, 3, 0]]),
        wood,
        leaves,
        fruit: None,
        revision: 0,
    }));
    // Replanting consumes real supplies; a three-leaf fixture must not assume
    // that a probabilistic leaf roll is guaranteed to produce a sapling.
    put_tool(journal, ItemKind::Prop(TreeSpecies::Oak.sapling()));
}

pub(super) fn advance(
    chunks: &mut ChunkStore,
    journal: &mut EditJournal,
    terrain: &TerraGenerator,
    runtime: &mut WorkRuntime,
    n: usize,
) {
    for _ in 0..n {
        for drop in &mut journal.gameplay_mut().drops {
            drop.age_seconds += 0.1;
        }
        run(chunks, journal, terrain, runtime, 1);
    }
}

pub(super) fn put_tool(journal: &mut EditJournal, item: ItemKind) {
    let mut chest = journal.container_at([0, 1, 0]).cloned().unwrap_or_default();
    assert_eq!(chest.insert(item, 1), 0);
    journal.set_container([0, 1, 0], chest).unwrap();
}

#[test]
fn lumberjack_fetches_axe_cuts_attributed_tree_collects_real_drops_and_replants_matching_sapling() {
    let (mut chunks, mut journal, terrain, mut runtime, npc) = fixture(WorkKind::Lumberjack);
    planted(&mut chunks, &mut journal, PlayerUuid::from_u128(1));
    put_tool(&mut journal, ItemKind::Tool(ToolKind::WoodAxe));
    advance(&mut chunks, &mut journal, &terrain, &mut runtime, 1);
    assert_eq!(
        journal
            .container_at([0, 1, 0])
            .unwrap()
            .count(ItemKind::Tool(ToolKind::WoodAxe)),
        1,
        "no remote borrowing"
    );
    assert_eq!(chunks.block_at([6, 1, 0]), Some(6));
    advance(&mut chunks, &mut journal, &terrain, &mut runtime, 600);
    assert_eq!(
        journal
            .container_at([0, 1, 0])
            .unwrap()
            .count(ItemKind::Block(6)),
        3
    );
    assert!(journal.gameplay().drops.is_empty());
    let worker = &journal.gameplay().jobs.workers[&npc];
    assert!(worker.tree.is_none(), "{:?}", worker.status);
    assert_eq!(
        worker.inventory.count(ItemKind::WornTool {
            tool: ToolKind::WoodAxe,
            remaining: 61
        }),
        1
    );
    let saplings: Vec<_> = journal
        .gameplay()
        .trees
        .trees
        .values()
        .filter(|tree| tree.stage == TreeStage::Sapling)
        .collect();
    assert_eq!(saplings.len(), 1);
    assert_eq!(saplings[0].species, TreeSpecies::Oak);
    assert_eq!(
        journal
            .container_at([0, 1, 0])
            .unwrap()
            .count(ItemKind::Prop(TreeSpecies::Oak.sapling())),
        [[5, 3, 0], [7, 3, 0], [6, 4, 0]]
            .into_iter()
            .filter(|cell| crate::saplings::leaf_drop_roll(*cell))
            .count() as u16
    );
    assert!(journal.gameplay().valid());
    advance(&mut chunks, &mut journal, &terrain, &mut runtime, 200);
    assert_eq!(
        journal
            .container_at([0, 1, 0])
            .unwrap()
            .count(ItemKind::Block(6)),
        3,
        "no duplicated logs after completion"
    );
    // A profession change returns the same individual axe to the real depot;
    // player harvesting and tree timing must still recognize its native type.
    journal.gameplay_mut().jobs.orders.get_mut(&1).unwrap().kind = WorkKind::Collector;
    advance(&mut chunks, &mut journal, &terrain, &mut runtime, 200);
    let worn = ItemKind::WornTool {
        tool: ToolKind::WoodAxe,
        remaining: 61,
    };
    assert_eq!(journal.container_at([0, 1, 0]).unwrap().count(worn), 1);
    assert_eq!(
        journal.gameplay().jobs.workers[&npc].inventory.count(worn),
        0
    );
    let properties = terra_core::block_properties::BlockProperties {
        required_tool: Some(terra_core::block_properties::ToolClass::Axe),
        mining_level: 1,
        ..Default::default()
    };
    assert!(properties.can_harvest(Some(worn)));
    assert!(crate::tree_felling::is_axe(Some(worn)));
    assert_eq!(
        crate::tree_felling::duration(20, Some(worn)),
        crate::tree_felling::duration(20, Some(ItemKind::Tool(ToolKind::WoodAxe)))
    );
}

#[test]
fn lumberjack_breaks_real_worn_axe_waits_and_continues_without_losing_partial_tree_or_loot() {
    let (mut chunks, mut journal, terrain, mut runtime, npc) = fixture(WorkKind::Lumberjack);
    planted(&mut chunks, &mut journal, PlayerUuid::from_u128(1));
    put_tool(
        &mut journal,
        ItemKind::WornTool {
            tool: ToolKind::WoodAxe,
            remaining: 1,
        },
    );
    advance(&mut chunks, &mut journal, &terrain, &mut runtime, 300);
    let worker = &journal.gameplay().jobs.workers[&npc];
    assert_eq!(worker.status, WorkStatus::Waiting(WaitReason::MissingTool));
    assert_eq!(worker.tree.as_ref().unwrap().wood.len(), 2);
    assert!(
        !worker
            .inventory
            .slots
            .iter()
            .any(|s| s.valid() && s.item.tool_kind().is_some())
    );
    assert_eq!(
        journal
            .container_at([0, 1, 0])
            .unwrap()
            .count(ItemKind::Block(6)),
        1
    );
    let saved = serde_json::to_string(&journal.gameplay().jobs).unwrap();
    journal.gameplay_mut().jobs = serde_json::from_str(&saved).unwrap();
    runtime = WorkRuntime::default();
    put_tool(&mut journal, ItemKind::Tool(ToolKind::StoneAxe));
    advance(&mut chunks, &mut journal, &terrain, &mut runtime, 600);
    assert!(journal.gameplay().jobs.workers[&npc].tree.is_none());
    assert_eq!(
        journal
            .container_at([0, 1, 0])
            .unwrap()
            .count(ItemKind::Block(6)),
        3
    );
    assert_eq!(
        journal.gameplay().jobs.workers[&npc]
            .inventory
            .count(ItemKind::WornTool {
                tool: ToolKind::StoneAxe,
                remaining: 126
            }),
        1
    );
}

#[test]
fn lumberjack_does_not_cut_wooden_buildings_or_other_players_planted_trees() {
    let (mut chunks, mut journal, terrain, mut runtime, npc) = fixture(WorkKind::Lumberjack);
    set(&mut chunks, [6, 1, 0], 6);
    journal.record([6, 1, 0], 6).unwrap();
    put_tool(&mut journal, ItemKind::Tool(ToolKind::IronAxe));
    advance(&mut chunks, &mut journal, &terrain, &mut runtime, 100);
    assert_eq!(chunks.block_at([6, 1, 0]), Some(6));
    assert_eq!(
        journal.gameplay().jobs.workers[&npc].status,
        WorkStatus::Waiting(WaitReason::NoTargets)
    );
    planted(&mut chunks, &mut journal, PlayerUuid::from_u128(2));
    advance(&mut chunks, &mut journal, &terrain, &mut runtime, 100);
    assert_eq!(chunks.block_at([6, 1, 0]), Some(6));
    assert_eq!(
        journal.gameplay().jobs.workers[&npc].status,
        WorkStatus::Waiting(WaitReason::Protected)
    );
}

#[test]
fn lumberjack_missing_sapling_keeps_completed_task_and_reports_supply_or_planting_problem() {
    let (mut chunks, mut journal, terrain, mut runtime, npc) = fixture(WorkKind::Lumberjack);
    planted(&mut chunks, &mut journal, PlayerUuid::from_u128(1));
    put_tool(&mut journal, ItemKind::Tool(ToolKind::IronAxe));
    let item = ItemKind::Prop(TreeSpecies::Oak.sapling());
    let mut withdrawn = Inventory::default();
    let mut reached = false;
    for _ in 0..600 {
        // Simulate another user withdrawing the actual delivered sapling. The
        // worker can reach its depot and borrow in the same simulation tick,
        // so an intermediate Planting/unborrowed state is not guaranteed visible.
        let mut chest = journal.container_at([0, 1, 0]).cloned().unwrap();
        if chest.take_one(item) {
            assert_eq!(withdrawn.insert(item, 1), 0);
            journal.set_container([0, 1, 0], chest).unwrap();
        }
        advance(&mut chunks, &mut journal, &terrain, &mut runtime, 1);
        if journal
            .gameplay()
            .jobs
            .workers
            .get(&npc)
            .and_then(|worker| worker.tree.as_ref())
            .is_some_and(|task| task.stage == TreeWorkStage::Planting && !task.sapling_loaded)
            && journal.gameplay().jobs.workers[&npc].status
                == WorkStatus::Waiting(WaitReason::MissingMaterial)
        {
            reached = true;
            break;
        }
    }
    assert!(
        reached,
        "status={:?}, tree={:?}",
        journal.gameplay().jobs.workers[&npc].status,
        journal.gameplay().jobs.workers[&npc].tree
    );
    assert!(withdrawn.count(item) >= 1);
    advance(&mut chunks, &mut journal, &terrain, &mut runtime, 200);
    assert_eq!(
        journal.gameplay().jobs.workers[&npc].status,
        WorkStatus::Waiting(WaitReason::MissingMaterial)
    );
    assert!(withdrawn.take_one(item));
    put_tool(&mut journal, item);
    set(&mut chunks, [6, 0, 0], 3);
    advance(&mut chunks, &mut journal, &terrain, &mut runtime, 200);
    assert_eq!(
        journal.gameplay().jobs.workers[&npc].status,
        WorkStatus::Waiting(WaitReason::NoPlantingSite)
    );
    assert_eq!(
        journal.gameplay().jobs.workers[&npc].inventory.count(item),
        1,
        "blocked planting never consumes seed"
    );
}

#[test]
fn lumberjack_uses_natural_generator_attribution_in_negative_sections() {
    let (_, mut journal, terrain, mut runtime, npc) = fixture(WorkKind::Lumberjack);
    let generator = crate::SectionGenerator::new(
        TerraGenerator::new(WorldConfig {
            seed: terra_core::WorldSeed(42042),
            landscape_revision: 3,
            sea_level: 8.0,
            base_height: 12.0,
            hill_amplitude: 0.0,
            mountain_amplitude: 0.0,
            caves: false,
            cave_structure_revision: 0,
            rivers: false,
            water: false,
            biomes: false,
            trees: true,
            tree_density: 1.0,
            ..Default::default()
        })
        .unwrap(),
    );
    let mut shape = None;
    'found: for z in -2..=0 {
        for x in -2..=0 {
            let column = generator.prepare_column([x, z], false).unwrap();
            for lz in 0..16 {
                for lx in 0..16 {
                    let cell = [x * 16 + lx, 12, z * 16 + lz];
                    if let Some(tree) = column.felling_shape(cell)
                        && column.felling_owner(tree.origin, tree.wood_material)
                            == Some(tree.origin)
                    {
                        shape = Some(tree);
                        break 'found;
                    }
                }
            }
        }
    }
    let shape = shape.expect("dense seed has a natural attributed tree");
    let root = shape.origin;
    assert!(root[0] < 0 && root[2] < 0);
    // Generated trunks can be wider than the old fixed 3.5m offset. Put both
    // worker and depot outside the actual lower trunk, never inside solid wood.
    let western_trunk = shape
        .wood
        .iter()
        .chain(&shape.leaves)
        .filter(|cell| cell[1] <= root[1] + 3)
        .map(|cell| cell[0])
        .min()
        .unwrap();
    let worker_x = western_trunk - 2;
    let storage = [western_trunk - 5, root[1], root[2]];
    let mut chunks = ChunkStore::default();
    for x in storage[0] - 3..=root[0] + 9 {
        for z in root[2] - 9..=root[2] + 9 {
            for y in root[1] - 1..=root[1] + 4 {
                set(&mut chunks, [x, y, z], if y < root[1] { 2 } else { 0 });
            }
        }
    }
    // Capsule shape bounds also contain theoretical roots below the surface.
    // Real section generation stamps wood only into air, preserving its terrain.
    // Build the same flat ground before stamping this isolated attributed tree.
    for &cell in shape
        .wood
        .iter()
        .chain(&shape.leaves)
        .filter(|cell| cell[1] < root[1])
    {
        set(&mut chunks, cell, 2);
    }
    for (cell, material) in shape
        .wood
        .iter()
        .map(|p| (*p, shape.wood_material))
        .chain(shape.leaves.iter().map(|p| (*p, shape.leaves_material)))
    {
        if chunks.block_at(cell).is_none_or(|material| material == 0) {
            set(&mut chunks, cell, material);
        }
        let column = [cell[0].div_euclid(16), cell[2].div_euclid(16)];
        if chunks.prepared_column(column).is_none() {
            chunks.set_prepared_column_for_test(
                column,
                Some(std::sync::Arc::new(
                    generator.prepare_column(column, false).unwrap(),
                )),
            );
        }
    }
    set(&mut chunks, storage, 29);
    journal.record(storage, 29).unwrap();
    let mut chest = Inventory::default();
    assert_eq!(chest.insert(ItemKind::Tool(ToolKind::RubyAxe), 4), 0);
    journal.set_container(storage, chest).unwrap();
    let station = [storage[0], storage[1], storage[2] + 2];
    set(
        &mut chunks,
        station,
        terra_core::profession_stations::LUMBERJACK,
    );
    journal
        .record(station, terra_core::profession_stations::LUMBERJACK)
        .unwrap();
    let order = journal.gameplay_mut().jobs.orders.get_mut(&1).unwrap();
    order.area = WorkArea::new(root, root).unwrap();
    order.storage = storage;
    journal
        .gameplay_mut()
        .creatures
        .animals
        .get_mut(&npc)
        .unwrap()
        .position = [worker_x as f64 + 0.5, root[1] as f64, root[2] as f64 + 0.5];
    assert_eq!(chunks.block_at([worker_x, root[1], root[2]]), Some(0));
    assert_eq!(chunks.block_at([worker_x, root[1] + 1, root[2]]), Some(0));
    let plan = crate::tree_felling::Plan::prepare(&chunks, &journal, root).unwrap();
    let support = [root[0], root[1] - 1, root[2]];
    assert_eq!(
        chunks.block_at(support),
        Some(2),
        "tree stamping preserves planting soil"
    );
    assert!(
        plan.wood_positions().all(|cell| cell[1] >= root[1]),
        "solid terrain is never attributed as generated wood"
    );
    let expected = plan.wood_count() as u16;
    let species = plan.worker_task().unwrap().species;
    let mut chest = journal.container_at(storage).cloned().unwrap();
    assert_eq!(chest.insert(ItemKind::Prop(species.sapling()), 1), 0);
    journal.set_container(storage, chest).unwrap();
    for _ in 0..12_000 {
        advance(&mut chunks, &mut journal, &terrain, &mut runtime, 1);
        if journal
            .container_at(storage)
            .unwrap()
            .count(ItemKind::Block(shape.wood_material))
            == expected
            && journal
                .gameplay()
                .jobs
                .workers
                .get(&npc)
                .is_some_and(|worker| worker.tree.is_none())
        {
            break;
        }
    }
    assert_eq!(
        journal
            .container_at(storage)
            .unwrap()
            .count(ItemKind::Block(shape.wood_material)),
        expected,
        "status={:?}, position={:?}, target={:?}, pending={:?}",
        journal.gameplay().jobs.workers[&npc].status,
        journal.gameplay().creatures.animals[&npc].position,
        journal.gameplay().jobs.workers[&npc].target,
        journal.gameplay().jobs.workers[&npc]
            .tree
            .as_ref()
            .map(|tree| (
                tree.root,
                tree.foot,
                tree.stage,
                tree.wood.len(),
                tree.leaves.len(),
                tree.drops.len()
            ))
    );
    assert!(
        journal.gameplay().jobs.workers[&npc].tree.is_none(),
        "{:?}",
        journal.gameplay().jobs.workers[&npc].status
    );
    assert_eq!(
        chunks.block_at(support),
        Some(2),
        "felling preserves the original planting soil"
    );
    assert!(
        journal
            .gameplay()
            .trees
            .trees
            .values()
            .any(|tree| tree.stage == TreeStage::Sapling
                && tree.root == root
                && tree.species == species)
    );
    assert!(journal.gameplay().valid());
}
