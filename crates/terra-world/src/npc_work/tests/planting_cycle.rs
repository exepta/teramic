use super::*;
use terra_core::{TreeSpecies, TreeStage};
use terra_voxel::{BlockSection, SectionPos};
const TABLE: [i64; 3] = [0, 1, 2];

fn orchard(chunks: &mut ChunkStore) {
    for x in -8..=24 {
        for z in -12..=12 {
            set(chunks, [x, 0, z], 2);
        }
    }
    for x in -1..=1 {
        for y in 0..=3 {
            for z in -1..=1 {
                let p = SectionPos::new(x, y, z);
                chunks.loaded.entry(p).or_insert_with(|| LoadedSection {
                    data: BlockSection::empty(p).unwrap(),
                    mesh: None,
                    mesh_epoch: 0,
                    water_epoch: 0,
                    water: None,
                    props: Vec::new(),
                });
            }
        }
    }
}
fn supplies(journal: &mut EditJournal, species: TreeSpecies, count: u16) {
    let mut stock = Inventory::default();
    assert_eq!(stock.insert(ItemKind::Prop(species.sapling()), count), 0);
    assert_eq!(stock.insert(ItemKind::Tool(ToolKind::RubyAxe), 1), 0);
    journal.set_container(TABLE, stock).unwrap();
}
fn first_plant(
    chunks: &mut ChunkStore,
    journal: &mut EditJournal,
    terrain: &TerraGenerator,
    runtime: &mut WorkRuntime,
) -> u64 {
    for _ in 0..600 {
        lumberjack::advance(chunks, journal, terrain, runtime, 1);
        if let Some(tree) = journal
            .gameplay()
            .trees
            .trees
            .values()
            .find(|tree| tree.stage == TreeStage::Sapling)
        {
            return tree.id;
        }
    }
    panic!(
        "lumberjack failed to plant from real table supplies: {:?}",
        journal.gameplay().jobs.workers
    );
}
#[test]
fn empty_area_is_planted_from_each_real_sapling_species_without_a_mature_tree() {
    for species in TreeSpecies::ALL {
        let (mut chunks, mut journal, terrain, mut runtime, npc) = fixture(WorkKind::Lumberjack);
        orchard(&mut chunks);
        supplies(&mut journal, species, 3);
        let start = journal.gameplay().creatures.animals[&npc].position;
        run(&mut chunks, &mut journal, &terrain, &mut runtime, 1);
        assert_eq!(
            journal
                .container_at(TABLE)
                .unwrap()
                .count(ItemKind::Prop(species.sapling())),
            3,
            "no remote supply withdrawal"
        );
        let id = first_plant(&mut chunks, &mut journal, &terrain, &mut runtime);
        let tree = journal.gameplay().trees.get(id).unwrap();
        assert_eq!(tree.species, species);
        assert_eq!(tree.planted_by, Some(PlayerUuid::from_u128(1)));
        assert_eq!(tree.growth_seconds, 0.0);
        assert!(tree.wood.is_empty() && tree.leaves.is_empty());
        assert_eq!(
            journal
                .container_at(TABLE)
                .unwrap()
                .count(ItemKind::Prop(species.sapling())),
            2
        );
        assert!(
            crate::creatures::distance(start, journal.gameplay().creatures.animals[&npc].position)
                > 0.1
        );
        lumberjack::advance(&mut chunks, &mut journal, &terrain, &mut runtime, 200);
        assert_eq!(
            journal
                .gameplay()
                .trees
                .trees
                .values()
                .filter(|tree| tree.stage == TreeStage::Sapling)
                .count(),
            1,
            "forestry spacing leaves room for the mature crown"
        );
        assert_eq!(
            journal
                .container_at(TABLE)
                .unwrap()
                .count(ItemKind::Prop(species.sapling())),
            2
        );
        assert!(journal.gameplay().valid());
    }
}
#[test]
fn planted_spruce_grows_is_harvested_and_replanted_with_exact_real_seed_and_log_accounting() {
    let (mut chunks, mut journal, terrain, mut runtime, npc) = fixture(WorkKind::Lumberjack);
    orchard(&mut chunks);
    supplies(&mut journal, TreeSpecies::Spruce, 2);
    let first = first_plant(&mut chunks, &mut journal, &terrain, &mut runtime);
    // Advance only the saved growth clock; use the actual growth transaction,
    // exact generated tree geometry, felling attribution and physical drops.
    journal
        .gameplay_mut()
        .trees
        .get_mut(first)
        .unwrap()
        .growth_seconds = TreeSpecies::Spruce.growth_seconds();
    assert!(crate::saplings::grow(&mut chunks, &mut journal, first).unwrap());
    let mature = journal.gameplay().trees.get(first).unwrap().clone();
    assert_eq!(mature.stage, TreeStage::Generated);
    let rolls = mature
        .leaves
        .iter()
        .filter(|p| crate::saplings::leaf_drop_roll(mature.global(**p)))
        .count() as u16;
    let mut replanted = false;
    for _ in 0..6000 {
        lumberjack::advance(&mut chunks, &mut journal, &terrain, &mut runtime, 1);
        replanted = journal.gameplay().trees.get(first).unwrap().stage
            == TreeStage::GeneratedFelled
            && journal.gameplay().trees.trees.values().any(|tree| {
                tree.stage == TreeStage::Sapling
                    && tree.id != first
                    && tree.species == TreeSpecies::Spruce
            });
        if replanted {
            break;
        }
    }
    assert!(replanted, "{:?}", journal.gameplay().jobs.workers[&npc]);
    assert_eq!(
        journal
            .container_at([0, 1, 0])
            .unwrap()
            .count(ItemKind::Block(36)),
        mature.wood.len() as u16
    );
    let item = ItemKind::Prop(TreeSpecies::Spruce.sapling());
    let stored = journal.container_at(TABLE).unwrap().count(item)
        + journal.container_at([0, 1, 0]).unwrap().count(item)
        + journal.gameplay().jobs.workers[&npc].inventory.count(item);
    let dropped: u16 = journal
        .gameplay()
        .drops
        .iter()
        .filter(|drop| drop.stack.item == item)
        .map(|drop| drop.stack.count)
        .sum();
    assert_eq!(
        stored + dropped,
        rolls,
        "two planted seeds were consumed, every extra seed came from a real leaf roll"
    );
    assert!(journal.gameplay().valid());
}
#[test]
fn unloaded_or_protected_empty_sites_do_not_consume_seed_and_resume_when_loaded() {
    let (mut chunks, mut journal, terrain, mut runtime, npc) = fixture(WorkKind::Lumberjack);
    orchard(&mut chunks);
    supplies(&mut journal, TreeSpecies::Spruce, 2);
    journal.gameplay_mut().jobs.orders.get_mut(&1).unwrap().area =
        WorkArea::new([20, 1, 0], [23, 2, 2]).unwrap();
    let section = SectionPos::new(1, 0, 0);
    let unloaded = chunks.loaded.remove(&section).unwrap();
    run(&mut chunks, &mut journal, &terrain, &mut runtime, 200);
    assert_eq!(
        journal.gameplay().jobs.workers[&npc].status,
        WorkStatus::Waiting(WaitReason::Unloaded)
    );
    assert_eq!(
        journal
            .container_at(TABLE)
            .unwrap()
            .count(ItemKind::Prop(TreeSpecies::Spruce.sapling())),
        2
    );
    assert!(journal.gameplay().trees.trees.is_empty());
    chunks.loaded.insert(section, unloaded);
    first_plant(&mut chunks, &mut journal, &terrain, &mut runtime);
    assert_eq!(
        journal
            .container_at(TABLE)
            .unwrap()
            .count(ItemKind::Prop(TreeSpecies::Spruce.sapling())),
        1
    );
    let (mut chunks, mut journal, terrain, mut runtime, npc) = fixture(WorkKind::Lumberjack);
    orchard(&mut chunks);
    supplies(&mut journal, TreeSpecies::Oak, 2);
    journal
        .gameplay_mut()
        .kingdoms
        .found(PlayerUuid::from_u128(2), [6, 1, 0], "Foreign")
        .unwrap();
    run(&mut chunks, &mut journal, &terrain, &mut runtime, 100);
    assert_eq!(
        journal.gameplay().jobs.workers[&npc].status,
        WorkStatus::Waiting(WaitReason::Protected)
    );
    assert_eq!(
        journal
            .container_at(TABLE)
            .unwrap()
            .count(ItemKind::Prop(TreeSpecies::Oak.sapling())),
        2
    );
    assert!(journal.gameplay().trees.trees.is_empty());
}
#[test]
fn planting_task_restores_without_withdrawing_or_consuming_a_second_seed() {
    let (mut chunks, mut journal, terrain, mut runtime, npc) = fixture(WorkKind::Lumberjack);
    orchard(&mut chunks);
    supplies(&mut journal, TreeSpecies::Birch, 3);
    let item = ItemKind::Prop(TreeSpecies::Birch.sapling());
    let mut carrying = false;
    for _ in 0..200 {
        run(&mut chunks, &mut journal, &terrain, &mut runtime, 1);
        carrying = journal
            .gameplay()
            .jobs
            .workers
            .get(&npc)
            .is_some_and(|worker| worker.tree.is_some() && worker.inventory.count(item) == 1)
            && journal.gameplay().trees.trees.is_empty();
        if carrying {
            break;
        }
    }
    assert!(carrying);
    let saved = serde_json::to_vec(&journal.gameplay().jobs).unwrap();
    journal.gameplay_mut().jobs = serde_json::from_slice(&saved).unwrap();
    runtime = WorkRuntime::default();
    first_plant(&mut chunks, &mut journal, &terrain, &mut runtime);
    assert_eq!(journal.container_at(TABLE).unwrap().count(item), 2);
    assert_eq!(
        journal.gameplay().jobs.workers[&npc].inventory.count(item),
        0
    );
    assert_eq!(journal.gameplay().trees.trees.len(), 1);
}
