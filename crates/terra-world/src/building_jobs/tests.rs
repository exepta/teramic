use super::*;
use crate::{
    edits::BlockObjectState,
    streaming::{LoadedSection, commit_object_state},
};
use terra_core::block_properties::BlockPropertiesCatalog;
use terra_core::{
    ItemStack, WorldConfig,
    creatures::{CreatureNeeds, villagers},
};
use terra_voxel::{BlockSection, SectionPos, global_to_section};

#[test]
fn default_house_is_captured_from_the_authored_village_without_free_supplies() {
    let steps = preview_house().unwrap();
    assert!(steps.len() > 100 && steps.len() < MAX_BUILD_STEPS);
    assert!(steps.iter().all(BuildStep::valid));
    assert!(
        steps
            .iter()
            .any(|s| s.cells.len() == 2 && matches!(s.cells[0].material, 65..=67))
    );
    assert!(
        steps
            .iter()
            .any(|s| s.cells.len() == 2 && matches!(s.cells[0].material, 71..=73))
    );
    assert!(steps.iter().all(|s| !s.costs.is_empty()));
}

fn set(chunks: &mut ChunkStore, p: [i64; 3], material: u16) {
    let (section, local) = global_to_section(p);
    chunks
        .loaded
        .get_mut(&section)
        .unwrap()
        .data
        .set_block(local, material)
        .unwrap();
}
fn fixture(
    workers: usize,
) -> (
    ChunkStore,
    EditJournal,
    TerraGenerator,
    KingdomId,
    PlayerUuid,
) {
    let terrain = TerraGenerator::new(WorldConfig {
        base_height: 12.0,
        hill_amplitude: 0.0,
        mountain_amplitude: 0.0,
        water: false,
        trees: false,
        rivers: false,
        caves: false,
        biomes: false,
        ..Default::default()
    })
    .unwrap();
    let mut chunks = ChunkStore::default();
    for x in -1..=1 {
        for y in 0..=2 {
            for z in 0..=2 {
                let section = SectionPos::new(x, y, z);
                let mut data = BlockSection::empty(section).unwrap();
                if y == 0 {
                    for lx in 0..16 {
                        for lz in 0..16 {
                            for ly in 0..12 {
                                data.set_block([lx, ly, lz], if ly == 11 { 1 } else { 2 })
                                    .unwrap();
                            }
                        }
                    }
                }
                chunks.loaded.insert(
                    section,
                    LoadedSection {
                        data,
                        mesh: None,
                        mesh_epoch: 0,
                        water_epoch: 0,
                        water: None,
                        props: Vec::new(),
                    },
                );
            }
        }
    }
    let owner = PlayerUuid::from_u128(238);
    let mut journal = EditJournal::default();
    let realm = journal
        .gameplay_mut()
        .kingdoms
        .found(owner, [15, 12, 31], "Builders")
        .unwrap();
    journal.gameplay_mut().buildings.settlements.insert(
        realm,
        SettlementBuilding {
            owner,
            storage: Some(BuildArea::new([0, 12, 16], [1, 13, 31]).unwrap()),
            residential: None,
            selected_template: default_template_id(),
            craftsmen: Vec::new(),
        },
    );
    for slot in 0..workers {
        let id = villagers::id([0, 1], slot as u16).unwrap();
        journal.gameplay_mut().creatures.animals.insert(
            id,
            Creature {
                id,
                kind: CreatureKind::Villager,
                home: [0, 1],
                position: [1.5, 12.0, 18.5 + slot as f64 * 2.0],
                velocity: [0.0; 3],
                yaw: 0.0,
                health: 20.0,
                behavior: CreatureBehavior::Rest,
                timer: 0.0,
                age: 0.0,
                variant: villagers::CRAFTSMAN,
                needs: CreatureNeeds::full(CreatureKind::Villager),
            },
        );
        journal.gameplay_mut().creatures.owners.insert(id, owner);
        let registry = &mut journal.gameplay_mut().buildings;
        registry
            .settlements
            .get_mut(&realm)
            .unwrap()
            .craftsmen
            .push(id);
        registry.inventories.insert(id, Inventory::default());
    }
    for p in [[0, 12, 16], [1, 12, 16], [0, 12, 31], [1, 12, 31]] {
        set(&mut chunks, p, 29);
        journal.record(p, 29).unwrap();
        journal.set_container(p, Inventory::default()).unwrap();
    }
    (chunks, journal, terrain, realm, owner)
}
fn stock(journal: &mut EditJournal, materials: Vec<ItemStack>) -> BTreeMap<ItemKind, u32> {
    let positions = [[0, 12, 16], [1, 12, 16], [0, 12, 31], [1, 12, 31]];
    let mut bags: [Inventory; 4] = std::array::from_fn(|_| Inventory::default());
    let mut slot = 0;
    let mut counts = BTreeMap::new();
    for stack in materials {
        *counts.entry(stack.item).or_default() += u32::from(stack.count);
        let mut left = stack.count;
        while left > 0 {
            let n = left.min(stack.item.max_stack());
            assert!(slot < 72, "template supplies fit four real chests");
            assert!(bags[slot / 18].creative_set(
                slot % 18,
                ItemStack {
                    item: stack.item,
                    count: n
                }
            ));
            left -= n;
            slot += 1;
        }
    }
    for (p, bag) in positions.into_iter().zip(bags) {
        journal.set_container(p, bag).unwrap();
    }
    counts
}
fn totals(journal: &EditJournal) -> BTreeMap<ItemKind, u32> {
    let mut result = BTreeMap::new();
    for bag in journal
        .containers()
        .map(|(_, b)| b)
        .chain(journal.gameplay().buildings.inventories.values())
    {
        for stack in bag.slots.iter().filter(|s| s.valid()) {
            *result.entry(stack.item).or_default() += u32::from(stack.count);
        }
    }
    result
}
fn doors(chunks: &mut ChunkStore, journal: &mut EditJournal, runtime: &mut BuildRuntime) {
    for request in runtime.drain_door_requests() {
        let Some(state) = journal.object_state(request.position) else {
            continue;
        };
        let Some(peer) = state.peer else {
            continue;
        };
        assert!(build_access(
            chunks,
            journal,
            journal
                .gameplay()
                .buildings
                .jobs
                .values()
                .find(|j| j.owner == request.owner)
                .unwrap()
                .kingdom,
            request.owner,
            request.position
        ));
        for p in [request.position, peer] {
            let old = journal.object_state(p).unwrap();
            commit_object_state(
                chunks,
                journal,
                p,
                Some(BlockObjectState {
                    facing: old.facing % 4 + 4,
                    ..old
                }),
            )
            .unwrap();
        }
    }
}

#[test]
fn full_authored_house_is_walked_built_roofed_furnished_and_scaffold_is_recovered_without_free_materials()
 {
    for facing in 0..4 {
        build_full_house(facing, HOUSE_TEMPLATE, false, false);
    }
}
#[test]
fn spruce_cottage_is_physically_built_in_all_facings_with_exact_paid_material_conservation() {
    for facing in 0..4 {
        build_full_house(facing, "spruce-cottage-v1", false, false);
    }
}
#[test]
fn a_saved_house_finishes_from_resolved_steps_after_its_template_asset_is_removed() {
    build_full_house(0, "spruce-cottage-v1", true, false);
}
#[test]
fn creative_craftsman_physically_finishes_a_full_house_in_all_facings_without_town_storage_or_items()
 {
    for facing in 0..4 {
        build_full_house(facing, "spruce-cottage-v1", false, true);
    }
}
fn build_full_house(facing: u8, template_id: &str, retire_template: bool, creative: bool) {
    build_full_house_with_preparation(
        facing,
        template_id,
        retire_template,
        creative,
        SitePreparation::Untouched,
    );
}

enum SitePreparation {
    Untouched,
    ClayTerrace,
    SavedSoilPad,
    FourMetreTerrace,
}

fn build_full_house_with_preparation(
    facing: u8,
    template_id: &str,
    retire_template: bool,
    creative: bool,
    preparation: SitePreparation,
) {
    let (mut chunks, mut journal, terrain, realm, owner) = fixture(1);
    let mut preserved_ground = Vec::new();
    match preparation {
        SitePreparation::Untouched => {}
        SitePreparation::ClayTerrace => {
            // A real one-block natural clay terrace at the future west wall,
            // plus an explicitly cleared cell beside it.
            set(&mut chunks, [4, 12, 24], 19);
            set(&mut chunks, [3, 11, 22], 0);
            journal.record([3, 11, 22], 0).unwrap();
        }
        SitePreparation::SavedSoilPad => {
            // The real test world's flattened lawn contains persistent grass
            // and dirt edits. Only authored floor/foundation cells may change.
            for x in 2..=13 {
                for z in 17..=29 {
                    for y in 10..=11 {
                        let position = [x, y, z];
                        let material = chunks.block_at(position).unwrap();
                        journal.record(position, material).unwrap();
                    }
                }
            }
            // Keep the grass probe outside the ramp: ordinary scaffold cover
            // correctly converts grass beneath it to dirt via block editing.
            // These untouched cells retain their actual original materials.
            for position in [[2, 11, 26], [3, 10, 22], [13, 10, 28]] {
                preserved_ground.push((position, chunks.block_at(position).unwrap()));
            }
        }
        SitePreparation::FourMetreTerrace => {
            // A four-by-four patch of the world's full four-metre terrace.
            // Its unedited clay is excavated by the actual worker before walls.
            for x in 4..=7 {
                for z in 20..=23 {
                    for y in 12..16 {
                        set(&mut chunks, [x, y, z], 19);
                    }
                }
            }
        }
    }
    // Every orientation has a loaded, owned walking approach beyond its deck.
    for cell in [
        [-1, 1],
        [1, 1],
        [0, 0],
        [0, 2],
        [-1, 0],
        [1, 0],
        [-1, 2],
        [1, 2],
    ] {
        journal
            .gameplay_mut()
            .kingdoms
            .claim(
                realm,
                cell,
                ClaimKind::ProtectedBuilding,
                terra_core::kingdom::ClaimActor::Player(owner),
            )
            .unwrap();
    }
    let job = if retire_template {
        let path =
            std::env::temp_dir().join(format!("terra-retired-cottage-{}.json", std::process::id()));
        let mut source = template_catalog()
            .unwrap()
            .get(template_id)
            .unwrap()
            .clone();
        source.id = "retired-cottage-v1".into();
        std::fs::write(&path, serde_json::to_vec(&source).unwrap()).unwrap();
        let source = terra_core::building_jobs::template::BuildingTemplate::load(&path).unwrap();
        let planned = planning::plan_resolved(
            &chunks,
            &journal,
            &terrain,
            realm,
            owner,
            [8, 12, 24],
            facing,
            &source,
        )
        .unwrap();
        std::fs::remove_file(&path).unwrap();
        assert!(!path.exists());
        assert!(template_catalog().unwrap().get(&planned.template).is_none());
        let bytes = serde_json::to_vec(&planned).unwrap();
        let restored: BuildingJob = serde_json::from_slice(&bytes).unwrap();
        assert!(restored.valid());
        restored
    } else {
        plan_template(
            &chunks,
            &journal,
            &terrain,
            realm,
            owner,
            [8, 12, 24],
            facing,
            template_id,
        )
        .unwrap()
    };
    let mut expected = if creative {
        BTreeMap::new()
    } else {
        stock(&mut journal, job.required_materials())
    };
    if !creative {
        for step in &job.steps {
            for stack in &step.costs {
                *expected.get_mut(&stack.item).unwrap() -= u32::from(stack.count);
            }
        }
        *expected.entry(ItemKind::Block(16)).or_default() += job
            .steps
            .iter()
            .filter(|s| s.phase == BuildPhase::Cleanup)
            .count() as u32;
    } else {
        journal
            .gameplay_mut()
            .buildings
            .settlements
            .get_mut(&realm)
            .unwrap()
            .storage = None;
        journal
            .gameplay_mut()
            .player_stats
            .entry(owner)
            .or_default()
            .game_mode = Some(terra_core::GameMode::Creative);
    }
    let total = job.steps.len();
    for &(position, material) in &preserved_ground {
        assert!(
            job.steps
                .iter()
                .flat_map(|step| &step.cells)
                .all(|cell| cell.position != position),
            "existing soil outside the authored foundation is not excavated"
        );
        if material == terra_voxel::MaterialId::Grass as u16 {
            let above = [position[0], position[1] + 1, position[2]];
            assert!(
                job.steps
                    .iter()
                    .flat_map(|step| &step.cells)
                    .all(|cell| { cell.position != above || cell.material == 0 }),
                "the unchanged grass probe is never covered by scaffolding"
            );
        }
    }
    let id = journal.gameplay_mut().buildings.insert(job).unwrap();
    let mut runtime = BuildRuntime::default();
    if creative {
        runtime.set_creative_owners([owner]);
    }
    let properties = BlockPropertiesCatalog::default();
    let npc = journal.gameplay().buildings.settlements[&realm].craftsmen[0];
    let mut highest = 12.0_f64;
    let mut moved = false;
    let mut last = 0;
    let mut stagnant = 0;
    let mut restored = false;
    for _ in 0..100_000 {
        let old = journal.gameplay().creatures.animals[&npc].position;
        tick(
            &mut chunks,
            &mut journal,
            &terrain,
            &properties,
            0.1,
            &mut runtime,
        );
        doors(&mut chunks, &mut journal, &mut runtime);
        if !restored && journal.gameplay().buildings.jobs[&id].next_step >= total / 2 {
            let before = totals(&journal);
            let bytes = serde_json::to_vec(&journal.gameplay().buildings).unwrap();
            let saved: BuildingRegistry = serde_json::from_slice(&bytes).unwrap();
            assert!(saved.valid());
            journal.gameplay_mut().buildings = saved;
            runtime = BuildRuntime::default();
            if creative {
                runtime.set_creative_owners([owner]);
            }
            restored = true;
            assert_eq!(
                totals(&journal),
                before,
                "reload never spends or creates stock"
            );
        }
        let animal = &journal.gameplay().creatures.animals[&npc];
        assert!(
            (animal.position[0] - old[0]).hypot(animal.position[2] - old[2]) <= 0.5,
            "no teleport"
        );
        highest = highest.max(animal.position[1]);
        moved |= crate::creatures::distance(old, animal.position) > 0.00001;
        let current = &journal.gameplay().buildings.jobs[&id];
        if current.status == BuildStatus::Complete {
            break;
        }
        if current.next_step == last {
            stagnant += 1;
        } else {
            last = current.next_step;
            stagnant = 0;
        }
        assert!(
            stagnant < 2000,
            "{template_id} facing {facing}: stuck step {}/{} {:?}, cell {:?}, npc {:?}, bag {:?}, route {:?}",
            current.next_step,
            total,
            current.status,
            current.steps.get(current.next_step),
            animal.position,
            journal.gameplay().buildings.inventories[&npc],
            runtime.routes.get(&npc)
        );
    }
    let complete = &journal.gameplay().buildings.jobs[&id];
    assert_eq!(
        complete.status,
        BuildStatus::Complete,
        "{}/{} {:?}",
        complete.next_step,
        total,
        complete.status
    );
    assert_eq!(complete.next_step, total);
    assert!(
        moved && highest > 16.0,
        "real scaffold/roof climbing: {highest}"
    );
    let mut final_cells = BTreeMap::new();
    for step in &complete.steps {
        for cell in &step.cells {
            final_cells.insert(cell.position, cell.material);
        }
    }
    // Furnishing may reuse a cleaned working-deck cell (for example a lantern).
    // Verify the final authored material everywhere, including every roof cell.
    for (position, material) in final_cells {
        assert_eq!(
            chunks.block_at(position),
            Some(material),
            "facing{facing} final cell{position:?}"
        );
    }
    expected.retain(|_, n| *n > 0);
    assert_eq!(totals(&journal), expected);
    for (position, material) in preserved_ground {
        assert_eq!(chunks.block_at(position), Some(material));
        assert_eq!(journal.block(position), Some(material));
    }
    assert!(restored && journal.gameplay().buildings.valid());
    eprintln!(
        "{template_id} facing{facing}: Complete, {total} steps, highest walking support{highest}"
    );
}

#[test]
fn one_to_four_onsite_craftsmen_have_exact_real_clock_rates_and_pause_never_spends_or_advances() {
    for crew in 1..=4 {
        let (mut chunks, mut journal, terrain, realm, owner) = fixture(crew);
        let positions = [
            [7.5, 12.0, 24.5],
            [9.5, 12.0, 24.5],
            [8.5, 12.0, 23.5],
            [8.5, 12.0, 25.5],
        ];
        let ids = journal.gameplay().buildings.settlements[&realm]
            .craftsmen
            .clone();
        for (id, p) in ids.iter().zip(positions) {
            journal
                .gameplay_mut()
                .creatures
                .animals
                .get_mut(id)
                .unwrap()
                .position = p;
            let mut worker = terra_core::npc_jobs::WorkerState::default();
            assert_eq!(worker.insert(ItemKind::Block(16), 1), 0);
            journal
                .gameplay_mut()
                .buildings
                .inventories
                .insert(*id, worker.inventory);
        }
        let steps = (0..8)
            .map(|n| BuildStep {
                phase: if n % 2 == 0 {
                    BuildPhase::Walls
                } else {
                    BuildPhase::Cleanup
                },
                cells: vec![BuildCell {
                    position: [8, 12, 24],
                    material: if n % 2 == 0 { 16 } else { 0 },
                    facing: 0,
                    peer: None,
                    secondary: false,
                }],
                costs: if n % 2 == 0 {
                    vec![ItemStack {
                        item: ItemKind::Block(16),
                        count: 1,
                    }]
                } else {
                    Vec::new()
                },
            })
            .collect();
        let job = BuildingJob {
            id: 1,
            kingdom: realm,
            owner,
            origin: [8, 12, 24],
            facing: 0,
            template: HOUSE_TEMPLATE.into(),
            free_resources: false,
            free_from_step: 0,
            workers: ids,
            steps,
            next_step: 0,
            work_seconds: 0.0,
            status: BuildStatus::Queued,
            enabled: true,
            revision: 1,
        };
        let id = journal.gameplay_mut().buildings.insert(job).unwrap();
        let mut runtime = BuildRuntime::default();
        let properties = BlockPropertiesCatalog::default();
        journal
            .gameplay_mut()
            .buildings
            .jobs
            .get_mut(&id)
            .unwrap()
            .enabled = false;
        let before = totals(&journal);
        for _ in 0..20 {
            tick(
                &mut chunks,
                &mut journal,
                &terrain,
                &properties,
                0.1,
                &mut runtime,
            );
        }
        assert_eq!(journal.gameplay().buildings.jobs[&id].work_seconds, 0.0);
        assert_eq!(totals(&journal), before);
        journal
            .gameplay_mut()
            .buildings
            .jobs
            .get_mut(&id)
            .unwrap()
            .enabled = true;
        let mut elapsed = 0.0;
        while journal.gameplay().buildings.jobs[&id].next_step < 8 && elapsed < 1900.0 {
            tick(
                &mut chunks,
                &mut journal,
                &terrain,
                &properties,
                0.1,
                &mut runtime,
            );
            elapsed += 0.1;
        }
        assert!(
            (elapsed - house_seconds(crew).unwrap()).abs() < 0.11,
            "crew {crew}: {elapsed}s, {:?}",
            journal.gameplay().buildings.jobs[&id].status
        );
        assert_eq!(totals(&journal), before);
    }
}

fn simple_job(
    kingdom: KingdomId,
    owner: PlayerUuid,
    workers: Vec<u64>,
    target: [i64; 3],
) -> BuildingJob {
    BuildingJob {
        id: 1,
        kingdom,
        owner,
        origin: target,
        facing: 0,
        template: HOUSE_TEMPLATE.into(),
        free_resources: false,
        free_from_step: 0,
        workers,
        steps: vec![BuildStep {
            phase: BuildPhase::Walls,
            cells: vec![BuildCell {
                position: target,
                material: 16,
                facing: 0,
                peer: None,
                secondary: false,
            }],
            costs: vec![ItemStack {
                item: ItemKind::Block(16),
                count: 1,
            }],
        }],
        next_step: 0,
        work_seconds: 0.0,
        status: BuildStatus::Queued,
        enabled: true,
        revision: 7,
    }
}
fn plank(journal: &mut EditJournal, npc: u64) {
    let mut bag = terra_core::npc_jobs::WorkerState::default();
    assert_eq!(bag.insert(ItemKind::Block(16), 1), 0);
    journal
        .gameplay_mut()
        .buildings
        .inventories
        .insert(npc, bag.inventory);
}
#[test]
fn blocked_target_and_player_body_pause_work_before_earning_time_and_control_revision_stays_stable()
{
    let (mut chunks, mut journal, terrain, realm, owner) = fixture(1);
    let npc = journal.gameplay().buildings.settlements[&realm].craftsmen[0];
    journal
        .gameplay_mut()
        .creatures
        .animals
        .get_mut(&npc)
        .unwrap()
        .position = [7.5, 12.0, 24.5];
    plank(&mut journal, npc);
    let id = journal
        .gameplay_mut()
        .buildings
        .insert(simple_job(realm, owner, vec![npc], [8, 12, 24]))
        .unwrap();
    let mut runtime = BuildRuntime::default();
    runtime.set_player_bodies([[8.5, 12.0, 24.5]]);
    let before = totals(&journal);
    let properties = BlockPropertiesCatalog::default();
    for _ in 0..20 {
        tick(
            &mut chunks,
            &mut journal,
            &terrain,
            &properties,
            0.1,
            &mut runtime,
        );
    }
    assert_eq!(
        journal.gameplay().buildings.jobs[&id].status,
        BuildStatus::Waiting(BuildWait::Blocked)
    );
    assert_eq!(journal.gameplay().buildings.jobs[&id].work_seconds, 0.0);
    assert_eq!(totals(&journal), before);
    runtime.set_player_bodies([]);
    for _ in 0..20 {
        tick(
            &mut chunks,
            &mut journal,
            &terrain,
            &properties,
            0.1,
            &mut runtime,
        );
    }
    assert!(journal.gameplay().buildings.jobs[&id].work_seconds > 1.9);
    assert_eq!(
        journal.gameplay().buildings.jobs[&id].revision,
        7,
        "RPC control version must not race progress snapshots"
    );
    let earned = journal.gameplay().buildings.jobs[&id].work_seconds;
    journal
        .gameplay_mut()
        .buildings
        .jobs
        .get_mut(&id)
        .unwrap()
        .enabled = false;
    for _ in 0..20 {
        tick(
            &mut chunks,
            &mut journal,
            &terrain,
            &properties,
            0.1,
            &mut runtime,
        );
    }
    assert_eq!(journal.gameplay().buildings.jobs[&id].work_seconds, earned);
    assert_eq!(totals(&journal), before);
}
#[test]
fn all_rotations_preserve_ordinary_block_states_and_paired_furniture() {
    for facing in 0..4 {
        let steps = planning::authored([8, 12, 24], facing).unwrap();
        for c in steps.iter().flat_map(|s| &s.cells) {
            if c.peer.is_some() || c.facing != 0 {
                assert!(
                    BlockObjectState {
                        facing: c.facing,
                        peer: c.peer,
                        secondary: c.secondary,
                        ..Default::default()
                    }
                    .valid_for(c.position, c.material),
                    "rotation {facing}: {c:?}"
                );
            }
        }
    }
}
#[test]
fn round_robin_does_not_let_four_jobs_without_workers_starve_another_kingdom() {
    let (mut chunks, mut journal, terrain, first, owner) = fixture(1);
    let npc = journal.gameplay().buildings.settlements[&first].craftsmen[0];
    journal
        .gameplay_mut()
        .buildings
        .settlements
        .get_mut(&first)
        .unwrap()
        .craftsmen
        .clear();
    for _ in 0..4 {
        journal
            .gameplay_mut()
            .buildings
            .insert(simple_job(first, owner, vec![], [8, 12, 24]))
            .unwrap();
    }
    let second_owner = PlayerUuid::from_u128(239);
    let second = journal
        .gameplay_mut()
        .kingdoms
        .found(second_owner, [31, 12, 31], "Second builders")
        .unwrap();
    journal.gameplay_mut().buildings.settlements.insert(
        second,
        SettlementBuilding {
            owner: second_owner,
            storage: Some(BuildArea::new([16, 12, 16], [17, 13, 17]).unwrap()),
            residential: None,
            selected_template: default_template_id(),
            craftsmen: vec![npc],
        },
    );
    journal
        .gameplay_mut()
        .creatures
        .owners
        .insert(npc, second_owner);
    journal
        .gameplay_mut()
        .creatures
        .animals
        .get_mut(&npc)
        .unwrap()
        .position = [23.5, 12.0, 24.5];
    plank(&mut journal, npc);
    set(&mut chunks, [16, 12, 16], 29);
    journal.record([16, 12, 16], 29).unwrap();
    journal
        .set_container([16, 12, 16], Inventory::default())
        .unwrap();
    let id = journal
        .gameplay_mut()
        .buildings
        .insert(simple_job(second, second_owner, vec![npc], [24, 12, 24]))
        .unwrap();
    assert!(journal.gameplay().buildings.valid());
    let mut runtime = BuildRuntime::default();
    let properties = BlockPropertiesCatalog::default();
    for _ in 0..20 {
        tick(
            &mut chunks,
            &mut journal,
            &terrain,
            &properties,
            0.1,
            &mut runtime,
        );
    }
    assert!(journal.gameplay().buildings.jobs[&id].work_seconds > 0.5);
    assert!(
        journal
            .gameplay()
            .buildings
            .jobs
            .iter()
            .filter(|(key, _)| **key != id)
            .all(|(_, j)| j.work_seconds == 0.0)
    );
}
#[test]
fn missing_materials_unloaded_targets_and_outside_claims_never_spend_or_build() {
    let (mut chunks, mut journal, terrain, realm, owner) = fixture(1);
    let npc = journal.gameplay().buildings.settlements[&realm].craftsmen[0];
    journal
        .gameplay_mut()
        .creatures
        .animals
        .get_mut(&npc)
        .unwrap()
        .position = [1.5, 12.0, 17.5];
    let id = journal
        .gameplay_mut()
        .buildings
        .insert(simple_job(realm, owner, vec![npc], [8, 12, 24]))
        .unwrap();
    let before = totals(&journal);
    let mut runtime = BuildRuntime::default();
    let properties = BlockPropertiesCatalog::default();
    for _ in 0..20 {
        tick(
            &mut chunks,
            &mut journal,
            &terrain,
            &properties,
            0.1,
            &mut runtime,
        );
    }
    assert_eq!(journal.gameplay().buildings.jobs[&id].work_seconds, 0.0);
    assert_eq!(
        journal.gameplay().buildings.jobs[&id].status,
        BuildStatus::Waiting(BuildWait::MissingMaterials)
    );
    assert_eq!(totals(&journal), before);
    plank(&mut journal, npc);
    let section = global_to_section([8, 20, 24]).0;
    journal
        .gameplay_mut()
        .buildings
        .jobs
        .get_mut(&id)
        .unwrap()
        .steps[0]
        .cells[0]
        .position = [8, 20, 24];
    let saved = chunks.loaded.remove(&section).unwrap();
    tick(
        &mut chunks,
        &mut journal,
        &terrain,
        &properties,
        0.1,
        &mut runtime,
    );
    assert_eq!(
        journal.gameplay().buildings.jobs[&id].status,
        BuildStatus::Waiting(BuildWait::Unloaded)
    );
    chunks.loaded.insert(section, saved);
    let j = journal.gameplay_mut().buildings.jobs.get_mut(&id).unwrap();
    j.steps[0].cells[0].position = [24, 12, 24];
    tick(
        &mut chunks,
        &mut journal,
        &terrain,
        &properties,
        0.1,
        &mut runtime,
    );
    assert_eq!(
        journal.gameplay().buildings.jobs[&id].status,
        BuildStatus::Waiting(BuildWait::Protected)
    );
    assert_eq!(journal.gameplay().buildings.jobs[&id].work_seconds, 0.0);
    assert_eq!(chunks.block_at([24, 12, 24]), Some(0));
    assert_eq!(
        journal.gameplay().buildings.inventories[&npc].count(ItemKind::Block(16)),
        1
    );
}

#[test]
fn cancelled_craftsman_with_full_bag_returns_actual_stock_after_storage_has_room() {
    let (mut chunks, mut journal, terrain, realm, owner) = fixture(1);
    let npc = journal.gameplay().buildings.settlements[&realm].craftsmen[0];
    journal
        .gameplay_mut()
        .creatures
        .animals
        .get_mut(&npc)
        .unwrap()
        .position = [8.5, 12.0, 24.5];
    let mut job = simple_job(realm, owner, vec![], [8, 12, 24]);
    job.status = BuildStatus::Cancelled;
    job.enabled = false;
    journal.gameplay_mut().buildings.insert(job).unwrap();
    let mut bag = terra_core::npc_jobs::WorkerState::default();
    for material in 1..=9 {
        assert_eq!(bag.insert(ItemKind::Block(material), 128), 0);
    }
    journal
        .gameplay_mut()
        .buildings
        .inventories
        .insert(npc, bag.inventory);
    assert!(journal.gameplay().buildings.is_busy(npc));
    for p in [[0, 12, 16], [1, 12, 16], [0, 12, 31], [1, 12, 31]] {
        let mut chest = Inventory::default();
        for slot in 0..18 {
            assert!(chest.creative_set(
                slot,
                ItemStack {
                    item: ItemKind::Block(1),
                    count: 128
                }
            ));
        }
        journal.set_container(p, chest).unwrap();
    }
    let initial_bag = journal.gameplay().buildings.inventories[&npc].clone();
    let properties = BlockPropertiesCatalog::default();
    let mut runtime = BuildRuntime::default();
    for _ in 0..20 {
        tick(
            &mut chunks,
            &mut journal,
            &terrain,
            &properties,
            0.1,
            &mut runtime,
        );
    }
    assert_eq!(
        journal.gameplay().buildings.inventories[&npc],
        initial_bag,
        "full storage never discards carried blocks"
    );
    journal
        .set_container([0, 12, 16], Inventory::default())
        .unwrap();
    let before = totals(&journal);
    let start = journal.gameplay().creatures.animals[&npc].position;
    for _ in 0..2000 {
        let old = journal.gameplay().creatures.animals[&npc].position;
        tick(
            &mut chunks,
            &mut journal,
            &terrain,
            &properties,
            0.1,
            &mut runtime,
        );
        let now = journal.gameplay().creatures.animals[&npc].position;
        assert!((now[0] - old[0]).hypot(now[2] - old[2]) <= 0.5);
        if journal.gameplay().buildings.inventories[&npc].occupied_slots() == 0 {
            break;
        }
    }
    assert_eq!(
        journal.gameplay().buildings.inventories[&npc].occupied_slots(),
        0
    );
    assert!(!journal.gameplay().buildings.is_busy(npc));
    assert_eq!(totals(&journal), before);
    assert!(
        crate::creatures::distance(start, journal.gameplay().creatures.animals[&npc].position)
            > 1.0
    );
    assert_eq!(
        chunks.block_at([8, 12, 24]),
        Some(0),
        "cancelled site is never built"
    );
}

#[test]
fn partial_ledge_contact_recenters_by_physical_sweeps_but_never_crosses_missing_or_foreign_ground()
{
    let (mut chunks, journal, terrain, realm, owner) = fixture(1);
    let id = journal.gameplay().buildings.settlements[&realm].craftsmen[0];
    let mut npc = journal.gameplay().creatures.animals[&id].clone();
    let job = simple_job(realm, owner, vec![id], [8, 12, 24]);
    set(&mut chunks, [7, 15, 24], 16);
    npc.position = [6.75, 16.0, 24.5];
    for _ in 0..8 {
        let h = Habitat {
            chunks: &chunks,
            journal: &journal,
            terrain: &terrain,
        };
        if h.ground_sized(npc.position, npc.size(), f64::from(npc.yaw))
            .is_some()
        {
            break;
        }
        let old = npc.position;
        assert!(movement::recenter(&h, &job, &mut npc, 0.1));
        assert_eq!(npc.position[1], old[1]);
        assert!(crate::creatures::distance(old, npc.position) <= 0.221_f64.powi(2));
    }
    let h = Habitat {
        chunks: &chunks,
        journal: &journal,
        terrain: &terrain,
    };
    assert!(
        h.ground_sized(npc.position, npc.size(), f64::from(npc.yaw))
            .is_some()
    );
    // A partial contact at the territorial edge cannot acquire foreign support.
    set(&mut chunks, [16, 15, 24], 16);
    npc.position = [15.75, 16.0, 24.5];
    let old = npc.position;
    assert!(!movement::recenter(
        &Habitat {
            chunks: &chunks,
            journal: &journal,
            terrain: &terrain
        },
        &job,
        &mut npc,
        0.1
    ));
    assert_eq!(npc.position, old);
    chunks.loaded.remove(&SectionPos::new(1, 0, 1));
    assert!(!movement::recenter(
        &Habitat {
            chunks: &chunks,
            journal: &journal,
            terrain: &terrain
        },
        &job,
        &mut npc,
        0.1
    ));
    assert_eq!(npc.position, old);
    // No complete support at all: normal gravity must take over, never a snap.
    set(&mut chunks, [7, 15, 24], 0);
    npc.position = [6.75, 16.0, 24.5];
    let old = npc.position;
    assert!(!movement::recenter(
        &Habitat {
            chunks: &chunks,
            journal: &journal,
            terrain: &terrain
        },
        &job,
        &mut npc,
        0.1
    ));
    assert_eq!(npc.position, old);
}

#[test]
fn paired_town_chest_requires_both_halves_inside_the_authorized_storage_area() {
    let (chunks, mut journal, _, realm, owner) = fixture(1);
    let a = [0, 12, 16];
    let b = [1, 12, 16];
    for (p, peer, secondary) in [(a, b, false), (b, a, true)] {
        journal
            .set_object_state(
                p,
                Some(BlockObjectState {
                    facing: 0,
                    peer: Some(peer),
                    secondary,
                    ..Default::default()
                }),
            )
            .unwrap();
    }
    let job = simple_job(realm, owner, vec![], [8, 12, 24]);
    assert!(supplies::chests(&chunks, &journal, &job, BuildArea::new(a, a).unwrap()).is_empty());
    assert_eq!(
        supplies::chests(&chunks, &journal, &job, BuildArea::new(a, b).unwrap()),
        vec![a, b]
    );
}

#[test]
fn crews_top_up_to_four_without_stealing_paused_workers_or_double_assignment() {
    let (mut chunks, mut journal, terrain, realm, owner) = fixture(5);
    let ids = journal.gameplay().buildings.settlements[&realm]
        .craftsmen
        .clone();
    let first = journal
        .gameplay_mut()
        .buildings
        .insert(simple_job(realm, owner, vec![ids[0]], [8, 12, 24]))
        .unwrap();
    let second = journal
        .gameplay_mut()
        .buildings
        .insert(simple_job(realm, owner, vec![], [9, 12, 24]))
        .unwrap();
    let mut paused = simple_job(realm, owner, vec![ids[1]], [10, 12, 24]);
    paused.enabled = false;
    paused.status = BuildStatus::Paused;
    let paused_id = journal.gameplay_mut().buildings.insert(paused).unwrap();
    let mut runtime = BuildRuntime::default();
    let properties = BlockPropertiesCatalog::default();
    tick(
        &mut chunks,
        &mut journal,
        &terrain,
        &properties,
        0.1,
        &mut runtime,
    );
    let registry = &journal.gameplay().buildings;
    assert_eq!(registry.jobs[&first].workers.len(), 4);
    assert!(!registry.jobs[&first].workers.contains(&ids[1]));
    assert!(registry.jobs[&second].workers.is_empty());
    assert_eq!(registry.jobs[&paused_id].workers, vec![ids[1]]);
    assert!(registry.valid());
}

#[test]
fn far_loaded_storage_is_walked_to_the_site_and_back_for_the_next_real_ingredient() {
    let (mut chunks, mut journal, terrain, realm, owner) = fixture(1);
    journal
        .gameplay_mut()
        .kingdoms
        .claim(
            realm,
            [1, 1],
            ClaimKind::ProtectedBuilding,
            terra_core::kingdom::ClaimActor::Player(owner),
        )
        .unwrap();
    let npc = journal.gameplay().buildings.settlements[&realm].craftsmen[0];
    let mut job = simple_job(realm, owner, vec![npc], [24, 12, 24]);
    job.steps.push(BuildStep {
        phase: BuildPhase::Walls,
        cells: vec![BuildCell {
            position: [25, 12, 24],
            material: 43,
            facing: 0,
            peer: None,
            secondary: false,
        }],
        costs: vec![ItemStack {
            item: ItemKind::Block(43),
            count: 1,
        }],
    });
    let id = journal.gameplay_mut().buildings.insert(job).unwrap();
    for (p, item) in [([0, 12, 16], 16), ([1, 12, 16], 43)] {
        let mut chest = Inventory::default();
        assert!(chest.creative_set(
            0,
            ItemStack {
                item: ItemKind::Block(item),
                count: 1
            }
        ));
        journal.set_container(p, chest).unwrap();
    }
    let mut runtime = BuildRuntime::default();
    let properties = BlockPropertiesCatalog::default();
    let mut reached_site = false;
    let mut returned_for_stone = false;
    let mut no_work = 0;
    let mut last_work = 0.0;
    for _ in 0..40_000 {
        let old = journal.gameplay().creatures.animals[&npc].position;
        tick(
            &mut chunks,
            &mut journal,
            &terrain,
            &properties,
            0.1,
            &mut runtime,
        );
        let p = journal.gameplay().creatures.animals[&npc].position;
        assert!(
            (p[0] - old[0]).hypot(p[2] - old[2]) <= 0.5,
            "no remote material transfers or teleports"
        );
        reached_site |= p[0] > 20.0;
        returned_for_stone |= journal.gameplay().buildings.jobs[&id].next_step == 1 && p[0] < 4.0;
        let job = &journal.gameplay().buildings.jobs[&id];
        if job.work_seconds > last_work + 0.0001 {
            last_work = job.work_seconds;
            no_work = 0;
        } else {
            no_work += 1;
        }
        assert!(
            no_work < 2000,
            "far route {:?} step{} pos{:?} bag{:?} path{:?}",
            job.status,
            job.next_step,
            p,
            journal.gameplay().buildings.inventories[&npc],
            runtime.routes.get(&npc)
        );
        if journal.gameplay().buildings.jobs[&id].status == BuildStatus::Complete {
            break;
        }
    }
    assert_eq!(
        journal.gameplay().buildings.jobs[&id].status,
        BuildStatus::Complete,
        "{:?} at {:?}",
        journal.gameplay().buildings.jobs[&id].status,
        journal.gameplay().creatures.animals[&npc].position
    );
    assert!(reached_site && returned_for_stone);
    assert_eq!(chunks.block_at([24, 12, 24]), Some(16));
    assert_eq!(chunks.block_at([25, 12, 24]), Some(43));
    assert!(totals(&journal).is_empty());
}

#[test]
fn five_onsite_projects_each_earn_the_full_elapsed_work_time() {
    let (mut chunks, mut journal, terrain, realm, owner) = fixture(5);
    let workers = journal.gameplay().buildings.settlements[&realm]
        .craftsmen
        .clone();
    let targets = [
        [4, 12, 19],
        [8, 12, 19],
        [12, 12, 19],
        [4, 12, 26],
        [10, 12, 26],
    ];
    let mut jobs = Vec::with_capacity(targets.len());
    for (npc, target) in workers.iter().copied().zip(targets) {
        journal
            .gameplay_mut()
            .creatures
            .animals
            .get_mut(&npc)
            .unwrap()
            .position = [
            target[0] as f64 - 0.5,
            target[1] as f64,
            target[2] as f64 + 0.5,
        ];
        plank(&mut journal, npc);
        jobs.push(
            journal
                .gameplay_mut()
                .buildings
                .insert(simple_job(realm, owner, vec![npc], target))
                .unwrap(),
        );
    }
    assert!(journal.gameplay().buildings.valid());
    let before = totals(&journal);
    let mut runtime = BuildRuntime::default();
    let properties = BlockPropertiesCatalog::default();
    for _ in 0..20 {
        tick(
            &mut chunks,
            &mut journal,
            &terrain,
            &properties,
            0.1,
            &mut runtime,
        );
    }
    for (job, npc) in jobs.into_iter().zip(workers) {
        let project = &journal.gameplay().buildings.jobs[&job];
        assert!(
            (project.work_seconds - 2.0).abs() < 0.00001,
            "project{job} earned{}s instead of2s",
            project.work_seconds
        );
        assert_eq!(
            project.workers,
            vec![npc],
            "each project keeps its own craftsman"
        );
        assert_eq!(project.status, BuildStatus::Building(BuildPhase::Walls));
        assert_eq!(project.next_step, 0);
    }
    assert_eq!(
        totals(&journal),
        before,
        "work time does not spend a block before placement"
    );
}

#[test]
fn defense_preserves_the_house_and_bags_and_only_active_onsite_crew_earn_work() {
    let (mut chunks, mut journal, terrain, realm, owner) = fixture(2);
    let ids = journal.gameplay().buildings.settlements[&realm]
        .craftsmen
        .clone();
    for (id, p) in ids.iter().zip([[7.5, 12.0, 24.5], [9.5, 12.0, 24.5]]) {
        journal
            .gameplay_mut()
            .creatures
            .animals
            .get_mut(id)
            .unwrap()
            .position = p;
        plank(&mut journal, *id);
    }
    let id = journal
        .gameplay_mut()
        .buildings
        .insert(simple_job(realm, owner, ids.clone(), [8, 12, 24]))
        .unwrap();
    let before = journal.gameplay().buildings.jobs[&id].clone();
    let inventory = totals(&journal);
    let mut runtime = BuildRuntime::default();
    let properties = BlockPropertiesCatalog::default();
    runtime.routes.insert(ids[0], movement::Route::default());
    runtime.door_requests.insert(
        [8, 12, 21],
        crate::npc_work::WorkerDoorRequest {
            npc: ids[0],
            owner,
            position: [8, 12, 21],
        },
    );
    runtime.set_suspended(ids.iter().copied());
    assert!(runtime.routes.is_empty() && runtime.door_requests.is_empty());
    for _ in 0..20 {
        tick(
            &mut chunks,
            &mut journal,
            &terrain,
            &properties,
            0.1,
            &mut runtime,
        );
    }
    assert_eq!(journal.gameplay().buildings.jobs[&id], before);
    for actor in &ids {
        assert_eq!(journal.gameplay().creatures.animals[actor].age, 0.0);
    }
    assert_eq!(totals(&journal), inventory);
    runtime.routes.insert(ids[0], movement::Route::default());
    runtime.set_suspended([ids[1]]);
    assert!(!runtime.routes.contains_key(&ids[0]));
    for _ in 0..20 {
        tick(
            &mut chunks,
            &mut journal,
            &terrain,
            &properties,
            0.1,
            &mut runtime,
        );
    }
    assert!((journal.gameplay().buildings.jobs[&id].work_seconds - 2.0).abs() < 0.00001);
    assert_eq!(journal.gameplay().creatures.animals[&ids[1]].age, 0.0);
    runtime.set_suspended([]);
    for _ in 0..20 {
        tick(
            &mut chunks,
            &mut journal,
            &terrain,
            &properties,
            0.1,
            &mut runtime,
        );
    }
    assert!((journal.gameplay().buildings.jobs[&id].work_seconds - 6.0).abs() < 0.00001);
    assert_eq!(journal.gameplay().buildings.jobs[&id].workers, ids);
    assert_eq!(totals(&journal), inventory);
}

#[test]
fn residential_automation_resolves_the_selected_real_template_and_never_falls_back_for_missing_ids()
{
    for selection in ["spruce-cottage-v1", "missing-cottage-v1"] {
        let (mut chunks, mut journal, terrain, realm, _) = fixture(1);
        let site = journal
            .gameplay_mut()
            .buildings
            .settlements
            .get_mut(&realm)
            .unwrap();
        site.residential = Some(BuildArea::new([2, 11, 17], [13, 12, 29]).unwrap());
        site.selected_template = selection.into();
        let mut runtime = BuildRuntime::default();
        tick(
            &mut chunks,
            &mut journal,
            &terrain,
            &BlockPropertiesCatalog::default(),
            0.1,
            &mut runtime,
        );
        if selection == "missing-cottage-v1" {
            assert!(
                journal.gameplay().buildings.jobs.is_empty(),
                "unavailable assets cannot silently select another house"
            );
        } else {
            let job = journal.gameplay().buildings.jobs.values().next().unwrap();
            assert_eq!(job.template, selection);
            let authored = template_catalog()
                .unwrap()
                .get(selection)
                .unwrap()
                .placed(job.origin, job.facing)
                .unwrap();
            assert!(authored.iter().all(|s| job.steps.contains(s)));
            let total_permanent: u32 = template_catalog()
                .unwrap()
                .get(selection)
                .unwrap()
                .materials
                .iter()
                .map(|s| u32::from(s.count))
                .sum();
            let total_site: u32 = job
                .required_materials()
                .iter()
                .map(|s| u32::from(s.count))
                .sum();
            assert!(
                total_site > total_permanent,
                "site requirements include real paid temporary scaffolds"
            );
            assert_eq!(
                totals(&journal).values().sum::<u32>(),
                0,
                "planning never invents stock"
            );
        }
    }
}

mod cleared_excavation;
mod diagnostics;
mod movement_status;
mod preparation_navigation;
mod prepared_ground;
mod repairs;
mod resources;
mod sites;
