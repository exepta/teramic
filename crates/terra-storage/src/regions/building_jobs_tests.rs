use super::*;
use terra_core::{
    Inventory, ItemKind, ItemStack,
    building_jobs::{
        BuildArea, BuildCell, BuildPhase, BuildStatus, BuildStep, BuildingJob, HOUSE_TEMPLATE,
        SettlementBuilding,
    },
    kingdom::KingdomId,
};

fn root(label: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "terra-building-{label}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ))
}

fn founded() -> (EditJournal, KingdomId, u64) {
    let mut journal = EditJournal::default();
    let owner = PlayerUuid::from_u128(8024);
    let keep = [8, 2, 8];
    journal.record(keep, terra_core::castle::BERGFRIED).unwrap();
    journal
        .set_object_state(keep, Some(terra_world::edits::BlockObjectState::default()))
        .unwrap();
    let mut realms = journal.gameplay().kingdoms.clone();
    let kingdom = realms
        .found(owner, keep, "Persistent construction")
        .unwrap();
    let positions =
        std::array::from_fn(|i| [2.5 + (i % 3) as f64 * 2.0, 2.0, 2.5 + (i / 3) as f64 * 2.0]);
    journal
        .gameplay_mut()
        .commit_founding_with_starters(realms, kingdom, positions)
        .unwrap();
    let npc = journal.gameplay().kingdom_starters.grants[&kingdom].ids[8];
    (journal, kingdom, npc)
}

fn construction() -> EditJournal {
    let (mut journal, kingdom, npc) = founded();
    let owner = PlayerUuid::from_u128(8024);
    let storage = [2, 2, 2];
    journal.record(storage, 29).unwrap();
    let mut chest = Inventory::default();
    assert_eq!(chest.insert(ItemKind::Block(3), 50), 0);
    journal.set_container(storage, chest).unwrap();
    let mut settlement = SettlementBuilding::new(owner);
    settlement.storage = Some(BuildArea::new([2, 2, 2], [4, 3, 4]).unwrap());
    settlement.residential = Some(BuildArea::new([9, 2, 9], [14, 10, 14]).unwrap());
    settlement.craftsmen.push(npc);
    let step = |phase, position, material| BuildStep {
        phase,
        cells: vec![BuildCell {
            position,
            material,
            facing: 0,
            peer: None,
            secondary: false,
        }],
        costs: vec![ItemStack {
            item: ItemKind::Block(material),
            count: 1,
        }],
    };
    let job = BuildingJob {
        id: 1,
        kingdom,
        owner,
        origin: [10, 2, 10],
        facing: 1,
        template: HOUSE_TEMPLATE.into(),
        free_resources: false,
        free_from_step: 0,
        workers: vec![npc],
        steps: vec![
            step(BuildPhase::Foundation, [10, 2, 10], 3),
            step(BuildPhase::Floor, [10, 3, 10], 6),
            step(BuildPhase::Walls, [10, 4, 10], 3),
        ],
        next_step: 1,
        work_seconds: 600.25,
        status: BuildStatus::Building(BuildPhase::Floor),
        enabled: true,
        revision: 7,
    };
    journal.record([10, 2, 10], 3).unwrap();
    let buildings = &mut journal.gameplay_mut().buildings;
    buildings.settlements.insert(kingdom, settlement);
    buildings.next_id = 2;
    buildings.revision = 12;
    buildings.jobs.insert(1, job);
    let mut carried = Inventory::default();
    assert_eq!(carried.insert(ItemKind::Block(3), 7), 0);
    assert_eq!(carried.insert(ItemKind::Block(6), 9), 0);
    buildings.inventories.insert(npc, carried);
    journal
        .gameplay_mut()
        .creatures
        .animals
        .get_mut(&npc)
        .unwrap()
        .variant = terra_core::creatures::villagers::CRAFTSMAN;
    assert!(journal.gameplay().valid());
    journal
}

#[test]
fn id25_keeps_town_storage_roster_carried_materials_and_partial_house_without_offline_work() {
    let root = root("roundtrip");
    fs::create_dir_all(&root).unwrap();
    let world = WorldId(802401);
    let mut journal = construction();
    journal
        .gameplay_mut()
        .buildings
        .settlements
        .values_mut()
        .next()
        .unwrap()
        .selected_template = "spruce-cottage-v1".into();
    save_edits(&root, world, &journal).unwrap();
    assert_eq!(
        read(&root.join(INDEX)).unwrap().get(12..16),
        Some(b"ID26".as_slice())
    );
    let before = journal.gameplay().buildings.clone();
    let npc = before.jobs[&1].workers[0];
    for _ in 0..3 {
        let loaded = load_edits(&root, world).unwrap();
        assert_eq!(loaded.gameplay(), journal.gameplay());
        assert_eq!(loaded.gameplay().buildings, before);
        assert_eq!(loaded.gameplay().buildings.jobs[&1].next_step, 1);
        assert_eq!(loaded.gameplay().buildings.jobs[&1].work_seconds, 600.25);
        assert_eq!(
            loaded
                .gameplay()
                .buildings
                .inventory(npc)
                .unwrap()
                .count(ItemKind::Block(3)),
            7
        );
        assert_eq!(
            loaded
                .gameplay()
                .buildings
                .inventory(npc)
                .unwrap()
                .count(ItemKind::Block(6)),
            9
        );
        assert_eq!(loaded.block([10, 2, 10]), Some(3));
        assert_eq!(
            loaded.block([10, 3, 10]),
            None,
            "load must never place the next scheduled block"
        );
        save_edits(&root, world, &loaded).unwrap();
    }
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn frozen_id23_loads_empty_construction_without_inventing_materials_or_craftsmen() {
    let root = root("legacy23");
    fs::create_dir_all(&root).unwrap();
    let world = WorldId(802402);
    let (journal, kingdom, _) = founded();
    save_edits(&root, world, &journal).unwrap();
    assert_eq!(
        read(&root.join(INDEX)).unwrap().get(12..16),
        Some(b"ID23".as_slice())
    );
    let loaded = load_edits(&root, world).unwrap();
    assert_eq!(loaded.gameplay().buildings, Default::default());
    assert_eq!(
        loaded.gameplay().kingdoms.get(kingdom),
        journal.gameplay().kingdoms.get(kingdom)
    );
    assert_eq!(
        loaded.gameplay().kingdom_starters,
        journal.gameplay().kingdom_starters
    );
    assert_eq!(loaded.gameplay().creatures, journal.gameplay().creatures);
    save_edits(&root, world, &loaded).unwrap();
    assert_eq!(
        read(&root.join(INDEX)).unwrap().get(12..16),
        Some(b"ID23".as_slice())
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn corrupt_id25_progress_inventory_or_roster_is_rejected_before_any_world_load() {
    let root = root("invalid24");
    fs::create_dir_all(&root).unwrap();
    let world = WorldId(802403);
    save_edits(&root, world, &construction()).unwrap();
    let original = read(&root.join(INDEX)).unwrap();
    for case in 0..4 {
        let mut index: BuildingIndex = decode(b"ID26", &original).unwrap();
        let npc = index.buildings.jobs[&1].workers[0];
        match case {
            0 => {
                let job = index.buildings.jobs.get_mut(&1).unwrap();
                job.next_step = job.steps.len() + 1;
            }
            1 => {
                index.buildings.jobs.get_mut(&1).unwrap().work_seconds = f64::NAN;
            }
            2 => {
                index.buildings.inventories.get_mut(&npc).unwrap().slots[0].count = u16::MAX;
            }
            _ => {
                let settlement = index.buildings.settlements.values_mut().next().unwrap();
                settlement.craftsmen.push(npc);
            }
        }
        fs::write(root.join(INDEX), encode(b"ID26", &index).unwrap()).unwrap();
        assert!(load_edits(&root, world).is_err(), "case{case}");
        assert!(read_building_index(&read(&root.join(INDEX)).unwrap()).is_err());
    }
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn frozen_id24_migrates_the_old_house_and_supplies_and_defaults_only_the_new_selection() {
    use super::building_legacy::{BuildingIndex24, BuildingRegistry24, SettlementBuilding24};
    let root = root("legacy24");
    fs::create_dir_all(&root).unwrap();
    let world = WorldId(802404);
    let journal = construction();
    save_edits(&root, world, &journal).unwrap();
    let current = read_building_index(&read(&root.join(INDEX)).unwrap()).unwrap();
    let old = BuildingIndex24 {
        prior: current.prior,
        buildings: BuildingRegistry24 {
            revision: current.buildings.revision,
            next_id: current.buildings.next_id,
            settlements: current
                .buildings
                .settlements
                .into_iter()
                .map(|(id, value)| {
                    (
                        id,
                        SettlementBuilding24 {
                            owner: value.owner,
                            storage: value.storage,
                            residential: value.residential,
                            craftsmen: value.craftsmen,
                        },
                    )
                })
                .collect(),
            jobs: current
                .buildings
                .jobs
                .into_iter()
                .map(|(id, job)| (id, job.into()))
                .collect(),
            inventories: current.buildings.inventories,
        },
    };
    fs::write(root.join(INDEX), encode(b"ID24", &old).unwrap()).unwrap();
    let loaded = load_edits(&root, world).unwrap();
    assert_eq!(loaded.gameplay(), journal.gameplay());
    assert!(
        loaded
            .gameplay()
            .buildings
            .settlements
            .values()
            .all(|site| site.selected_template == HOUSE_TEMPLATE)
    );
    save_edits(&root, world, &loaded).unwrap();
    assert_eq!(
        read(&root.join(INDEX)).unwrap().get(12..16),
        Some(b"ID26".as_slice())
    );
    assert_eq!(
        load_edits(&root, world).unwrap().gameplay(),
        journal.gameplay()
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn saved_resolved_job_and_automatic_choice_survive_an_unavailable_template_asset() {
    let root = root("removed-template");
    fs::create_dir_all(&root).unwrap();
    let world = WorldId(802405);
    let mut journal = construction();
    let buildings = &mut journal.gameplay_mut().buildings;
    buildings.jobs.get_mut(&1).unwrap().template = "removed-residential-v1".into();
    buildings
        .settlements
        .values_mut()
        .next()
        .unwrap()
        .selected_template = "removed-residential-v1".into();
    assert!(journal.gameplay().valid());
    save_edits(&root, world, &journal).unwrap();
    let loaded = load_edits(&root, world).unwrap();
    assert_eq!(loaded.gameplay(), journal.gameplay());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn frozen_id25_migrates_selected_templates_without_granting_creative_funding() {
    use super::building_legacy::{BuildingIndex25, BuildingRegistry25};
    let root = root("legacy25");
    fs::create_dir_all(&root).unwrap();
    let world = WorldId(802405);
    let mut journal = construction();
    for site in journal.gameplay_mut().buildings.settlements.values_mut() {
        site.selected_template = "spruce-cottage-v1".into();
    }
    save_edits(&root, world, &journal).unwrap();
    let current = read_building_index(&read(&root.join(INDEX)).unwrap()).unwrap();
    let old = BuildingIndex25 {
        prior: current.prior,
        buildings: BuildingRegistry25 {
            revision: current.buildings.revision,
            next_id: current.buildings.next_id,
            settlements: current.buildings.settlements,
            jobs: current
                .buildings
                .jobs
                .into_iter()
                .map(|(id, job)| (id, job.into()))
                .collect(),
            inventories: current.buildings.inventories,
        },
    };
    fs::write(root.join(INDEX), encode(b"ID25", &old).unwrap()).unwrap();
    let loaded = load_edits(&root, world).unwrap();
    assert_eq!(loaded.gameplay(), journal.gameplay());
    assert!(
        loaded
            .gameplay()
            .buildings
            .jobs
            .values()
            .all(|j| !j.free_resources && j.free_from_step == 0)
    );
    save_edits(&root, world, &loaded).unwrap();
    assert_eq!(
        read(&root.join(INDEX)).unwrap().get(12..16),
        Some(b"ID26".as_slice())
    );
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn id26_preserves_the_creative_funding_breakpoint_real_bags_and_authoritative_saved_mode() {
    let root = root("creative26");
    fs::create_dir_all(&root).unwrap();
    let world = WorldId(802406);
    let mut journal = construction();
    let owner = journal.gameplay().buildings.jobs[&1].owner;
    journal
        .gameplay_mut()
        .player_stats
        .entry(owner)
        .or_default()
        .game_mode = Some(terra_core::GameMode::Creative);
    let job = journal.gameplay_mut().buildings.jobs.get_mut(&1).unwrap();
    job.free_resources = true;
    job.free_from_step = job.next_step;
    save_edits(&root, world, &journal).unwrap();
    let loaded = load_edits(&root, world).unwrap();
    assert_eq!(loaded.gameplay(), journal.gameplay());
    assert_eq!(loaded.gameplay().buildings.jobs[&1].free_from_step, 1);
    assert!(loaded.gameplay().buildings.jobs[&1].free_resources);
    fs::remove_dir_all(root).unwrap();
}
