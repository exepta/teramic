use super::*;

fn resumed_job(realm: KingdomId, owner: PlayerUuid, worker: u64) -> BuildingJob {
    let mut job = simple_job(realm, owner, vec![worker], [8, 12, 24]);
    let placement = job.steps[0].clone();
    let mut removal = placement.clone();
    removal.phase = BuildPhase::Foundation;
    removal.cells[0].material = 0;
    removal.costs.clear();
    job.steps.insert(0, removal);
    // A realistic number of future placements gives this saved job its normal
    // per-block work duration; the test stops after its first pending wall.
    for index in 0..62 {
        let mut later = placement.clone();
        later.cells[0].position = [4 + index % 8, 13 + index / 8, 24];
        job.steps.push(later);
    }
    job.next_step = 1;
    job.work_seconds = 1800.0 / job.steps.len() as f64;
    job
}

#[test]
fn saved_completed_excavation_is_physically_recleared_before_the_paid_wall_resumes() {
    for soil in [1, 2] {
        let (mut chunks, mut journal, terrain, realm, owner) = fixture(1);
        let worker = journal.gameplay().buildings.settlements[&realm].craftsmen[0];
        let position = [8, 12, 24];
        journal
            .gameplay_mut()
            .creatures
            .animals
            .get_mut(&worker)
            .unwrap()
            .position = [7.5, 12.0, 24.5];
        set(&mut chunks, position, soil);
        journal.record(position, soil).unwrap();
        plank(&mut journal, worker);
        let job = resumed_job(realm, owner, worker);
        let steps = job.steps.clone();
        let progress = job.work_seconds;
        let id = journal.gameplay_mut().buildings.insert(job).unwrap();
        let properties = BlockPropertiesCatalog::default();
        let mut runtime = BuildRuntime::default();
        let before = totals(&journal);
        let mut removed = false;
        for _ in 0..400 {
            let old = journal.gameplay().creatures.animals[&worker].position;
            tick(
                &mut chunks,
                &mut journal,
                &terrain,
                &properties,
                0.1,
                &mut runtime,
            );
            let current = &journal.gameplay().buildings.jobs[&id];
            let now = journal.gameplay().creatures.animals[&worker].position;
            assert!((now[0] - old[0]).hypot(now[2] - old[2]) <= 0.221);
            assert_eq!(current.next_step, 1, "repair never replays paid work");
            assert_eq!(current.work_seconds, progress);
            assert_eq!(current.free_from_step, 0);
            assert_eq!(current.steps, steps);
            assert_eq!(totals(&journal), before, "soil repair costs no planks");
            if chunks.block_at(position) == Some(0) {
                removed = true;
                break;
            }
        }
        assert!(removed, "the builder must perform the old excavation again");
        assert_eq!(journal.block(position), Some(0));
        assert!(!runtime.repairs.contains_key(&id));
        for _ in 0..400 {
            tick(
                &mut chunks,
                &mut journal,
                &terrain,
                &properties,
                0.1,
                &mut runtime,
            );
            if journal.gameplay().buildings.jobs[&id].next_step == 2 {
                break;
            }
        }
        assert_eq!(journal.gameplay().buildings.jobs[&id].next_step, 2);
        assert_eq!(chunks.block_at(position), Some(16));
        assert_eq!(
            journal.gameplay().buildings.inventories[&worker].count(ItemKind::Block(16)),
            0
        );
    }
}

#[test]
fn saved_excavation_does_not_replace_stone_or_a_new_stocked_chest() {
    for material in [3, 29] {
        let (mut chunks, mut journal, terrain, realm, owner) = fixture(1);
        let worker = journal.gameplay().buildings.settlements[&realm].craftsmen[0];
        let position = [8, 12, 24];
        journal
            .gameplay_mut()
            .creatures
            .animals
            .get_mut(&worker)
            .unwrap()
            .position = [7.5, 12.0, 24.5];
        set(&mut chunks, position, material);
        journal.record(position, material).unwrap();
        if material == 29 {
            let mut chest = Inventory::default();
            assert_eq!(chest.insert(ItemKind::Block(3), 23), 0);
            journal.set_container(position, chest).unwrap();
        }
        plank(&mut journal, worker);
        let job = resumed_job(realm, owner, worker);
        let progress = job.work_seconds;
        let id = journal.gameplay_mut().buildings.insert(job).unwrap();
        let before = totals(&journal);
        let properties = BlockPropertiesCatalog::default();
        let mut runtime = BuildRuntime::default();
        for _ in 0..400 {
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
        assert_eq!(journal.gameplay().buildings.jobs[&id].next_step, 1);
        assert_eq!(
            journal.gameplay().buildings.jobs[&id].work_seconds,
            progress
        );
        assert_eq!(chunks.block_at(position), Some(material));
        assert_eq!(journal.block(position), Some(material));
        assert_eq!(totals(&journal), before);
        assert!(!runtime.repairs.contains_key(&id));
    }
}

#[test]
fn soil_repair_requires_the_last_completed_cell_operation_to_be_this_jobs_excavation() {
    let (mut chunks, mut journal, _, realm, owner) = fixture(1);
    let worker = journal.gameplay().buildings.settlements[&realm].craftsmen[0];
    let position = [8, 12, 24];
    set(&mut chunks, position, 1);
    journal.record(position, 1).unwrap();
    let mut job = resumed_job(realm, owner, worker);
    assert!(repair::step(&chunks, &journal, &job).is_some());
    job.steps[0].cells[0].position[0] += 1;
    assert!(repair::step(&chunks, &journal, &job).is_none());
    job.steps[0].cells[0].position = position;
    job.steps.insert(1, job.steps[1].clone());
    job.next_step = 2;
    assert!(repair::step(&chunks, &journal, &job).is_none());
}
