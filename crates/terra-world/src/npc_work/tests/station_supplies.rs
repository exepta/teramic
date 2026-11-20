use super::*;
use crate::{SectionGenerator, tree_felling};
use std::sync::Arc;

fn natural_spruce(
    chunks: &mut ChunkStore,
    terrain: &TerraGenerator,
    root: [i64; 3],
) -> tree_felling::Shape {
    let generator = SectionGenerator::new(terrain.clone());
    let coordinate = [root[0].div_euclid(16), root[2].div_euclid(16)];
    let column = generator
        .prepare_column(coordinate, false)
        .unwrap()
        .with_spruce_for_test(root, 901);
    let shape = column.felling_shape(root).unwrap();
    for (cell, material) in shape
        .wood
        .iter()
        .map(|cell| (*cell, shape.wood_material))
        .chain(
            shape
                .leaves
                .iter()
                .map(|cell| (*cell, shape.leaves_material)),
        )
    {
        set(chunks, cell, material);
        let coordinate = [cell[0].div_euclid(16), cell[2].div_euclid(16)];
        if chunks.prepared_column(coordinate).is_none() {
            let column = generator
                .prepare_column(coordinate, false)
                .unwrap()
                .with_spruce_for_test(root, 901);
            chunks.set_prepared_column_for_test(coordinate, Some(Arc::new(column)));
        }
    }
    shape
}

#[test]
fn ruby_axe_and_spruce_saplings_in_the_table_supply_a_terraced_natural_tree_with_separate_output_chest()
 {
    let (mut chunks, mut journal, terrain, mut runtime, npc) = fixture(WorkKind::Lumberjack);
    // The worker starts on the lower terrace; the tree is on the next real step.
    for x in -8..=14 {
        for z in -9..=9 {
            set(&mut chunks, [x, 0, z], 2);
            if x >= 4 {
                set(&mut chunks, [x, 1, z], 2);
            }
        }
    }
    let root = [6, 2, 0];
    let shape = natural_spruce(&mut chunks, &terrain, root);
    assert_eq!(shape.wood_material, 36);
    assert!(
        shape.wood.iter().any(|cell| cell[1] >= 16),
        "actual authored conifer crosses sections"
    );
    let table = [0, 1, 2];
    let mut stock = Inventory::default();
    assert_eq!(stock.insert(ItemKind::Tool(ToolKind::RubyAxe), 1), 0);
    assert_eq!(
        stock.insert(
            ItemKind::Prop(terra_core::TreeSpecies::Spruce.sapling()),
            128
        ),
        0
    );
    journal.set_container(table, stock).unwrap();
    let start = journal.gameplay().creatures.animals[&npc].position;
    let mut moved = false;
    for _ in 0..2200 {
        for drop in &mut journal.gameplay_mut().drops {
            drop.age_seconds += 0.1;
        }
        run(&mut chunks, &mut journal, &terrain, &mut runtime, 1);
        moved |=
            crate::creatures::distance(start, journal.gameplay().creatures.animals[&npc].position)
                > 1.0;
        if journal
            .gameplay()
            .trees
            .trees
            .values()
            .any(|tree| tree.stage == terra_core::TreeStage::Sapling)
        {
            break;
        }
    }
    let worker = &journal.gameplay().jobs.workers[&npc];
    assert!(moved);
    assert_eq!(
        journal.gameplay().creatures.animals[&npc].variant,
        villagers::LUMBERJACK
    );
    assert!(
        shape
            .wood
            .iter()
            .all(|cell| chunks.block_at(*cell) != Some(36)),
        "{:?} {:?}",
        worker.status,
        worker.target
    );
    assert_eq!(
        journal
            .container_at([0, 1, 0])
            .unwrap()
            .count(ItemKind::Block(36)),
        shape.wood.len() as u16
    );
    assert_eq!(
        journal
            .container_at(table)
            .unwrap()
            .count(ItemKind::Tool(ToolKind::RubyAxe)),
        0
    );
    assert!(
        journal
            .gameplay()
            .trees
            .trees
            .values()
            .any(|tree| tree.stage == terra_core::TreeStage::Sapling
                && tree.species == terra_core::TreeSpecies::Spruce),
        "{:?}",
        worker.status
    );
    let seed = ItemKind::Prop(terra_core::TreeSpecies::Spruce.sapling());
    let remaining = journal.container_at(table).unwrap().count(seed)
        + journal.container_at([0, 1, 0]).unwrap().count(seed)
        + worker.inventory.count(seed);
    let harvested = shape
        .leaves
        .iter()
        .filter(|cell| crate::saplings::leaf_drop_roll(**cell))
        .count() as u16;
    assert_eq!(
        remaining,
        128 + harvested - 1,
        "exactly one real sapling was planted"
    );
    assert!(journal.gameplay().valid());
}

#[test]
fn station_supplies_are_not_borrowed_remotely() {
    let (mut chunks, mut journal, terrain, mut runtime, npc) = fixture(WorkKind::Lumberjack);
    super::lumberjack::planted(&mut chunks, &mut journal, PlayerUuid::from_u128(1));
    let table = [0, 1, 2];
    let mut stock = Inventory::default();
    stock.insert(ItemKind::Tool(ToolKind::RubyAxe), 1);
    journal.set_container(table, stock.clone()).unwrap();
    run(&mut chunks, &mut journal, &terrain, &mut runtime, 1);
    assert_eq!(journal.container_at(table), Some(&stock));
    assert_eq!(
        journal.gameplay().jobs.workers[&npc]
            .inventory
            .occupied_slots(),
        0
    );
    run(&mut chunks, &mut journal, &terrain, &mut runtime, 100);
    assert_eq!(
        journal
            .container_at(table)
            .unwrap()
            .count(ItemKind::Tool(ToolKind::RubyAxe)),
        0
    );
    assert!(
        journal.gameplay().jobs.workers[&npc]
            .inventory
            .slots
            .iter()
            .any(|stack| stack.item.tool_kind() == Some(ToolKind::RubyAxe))
    );
}
