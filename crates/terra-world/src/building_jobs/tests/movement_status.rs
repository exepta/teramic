use super::*;

fn wall_box(
    chunks: &mut ChunkStore,
    min: [i64; 3],
    max: [i64; 3],
    retain: impl Fn([i64; 3]) -> bool,
) -> Vec<[i64; 3]> {
    let mut walls = Vec::new();
    for x in min[0]..=max[0] {
        for y in min[1]..=max[1] {
            for z in min[2]..=max[2] {
                let p = [x, y, z];
                if !retain(p) {
                    assert_eq!(chunks.block_at(p), Some(0));
                    set(chunks, p, 16);
                    walls.push(p);
                }
            }
        }
    }
    walls
}

#[test]
fn no_work_stance_reports_no_path_through_retries_and_recovers_when_the_obstacle_is_removed() {
    let (mut chunks, mut journal, terrain, realm, owner) = fixture(1);
    let worker = journal.gameplay().buildings.settlements[&realm].craftsmen[0];
    let target = [8, 12, 24];
    let walls = wall_box(&mut chunks, [6, 12, 22], [10, 16, 26], |p| p == target);
    plank(&mut journal, worker);
    let id = journal
        .gameplay_mut()
        .buildings
        .insert(simple_job(realm, owner, vec![worker], target))
        .unwrap();
    let properties = BlockPropertiesCatalog::default();
    let mut runtime = BuildRuntime::default();
    let start = journal.gameplay().creatures.animals[&worker].position;
    for _ in 0..100 {
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
            BuildStatus::Waiting(BuildWait::NoPath)
        );
    }
    let job = &journal.gameplay().buildings.jobs[&id];
    assert_eq!(job.work_seconds, 0.0);
    assert_eq!(job.next_step, 0);
    assert_eq!(chunks.block_at(target), Some(0));
    assert_eq!(
        journal.gameplay().creatures.animals[&worker].position,
        start
    );
    assert!(journal.gameplay().creatures.animals[&worker].age > 9.9);
    for p in walls {
        set(&mut chunks, p, 0);
    }
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
        let now = journal.gameplay().creatures.animals[&worker].position;
        assert!((now[0] - old[0]).hypot(now[2] - old[2]) <= 0.221);
        if journal.gameplay().buildings.jobs[&id].work_seconds > 0.0 {
            break;
        }
    }
    assert!(journal.gameplay().buildings.jobs[&id].work_seconds > 0.0);
    assert!(matches!(
        journal.gameplay().buildings.jobs[&id].status,
        BuildStatus::Building(_)
    ));
}

#[test]
fn a_blocked_physical_route_reports_no_path_and_resumes_without_resetting_the_worker() {
    let (mut chunks, mut journal, terrain, realm, owner) = fixture(1);
    let worker = journal.gameplay().buildings.settlements[&realm].craftsmen[0];
    let walls = wall_box(&mut chunks, [0, 12, 17], [2, 15, 19], |p| {
        p[0] == 1 && p[2] == 18
    });
    plank(&mut journal, worker);
    let id = journal
        .gameplay_mut()
        .buildings
        .insert(simple_job(realm, owner, vec![worker], [8, 12, 24]))
        .unwrap();
    let properties = BlockPropertiesCatalog::default();
    let mut runtime = BuildRuntime::default();
    let start = journal.gameplay().creatures.animals[&worker].position;
    for _ in 0..40 {
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
        journal.gameplay().creatures.animals[&worker].position,
        start
    );
    assert_eq!(
        journal.gameplay().buildings.jobs[&id].status,
        BuildStatus::Waiting(BuildWait::NoPath)
    );
    assert_eq!(journal.gameplay().buildings.jobs[&id].work_seconds, 0.0);
    for p in walls {
        set(&mut chunks, p, 0);
    }
    for _ in 0..400 {
        tick(
            &mut chunks,
            &mut journal,
            &terrain,
            &properties,
            0.1,
            &mut runtime,
        );
        if journal.gameplay().buildings.jobs[&id].work_seconds > 0.0 {
            break;
        }
    }
    assert!(journal.gameplay().buildings.jobs[&id].work_seconds > 0.0);
}

#[test]
fn material_pickup_and_final_unloading_report_an_unreachable_town_chest() {
    for unloading in [false, true] {
        let (mut chunks, mut journal, terrain, realm, owner) = fixture(1);
        let worker = journal.gameplay().buildings.settlements[&realm].craftsmen[0];
        journal
            .gameplay_mut()
            .creatures
            .animals
            .get_mut(&worker)
            .unwrap()
            .position = [8.5, 12.0, 24.5];
        wall_box(&mut chunks, [-2, 12, 14], [2, 16, 18], |p| {
            matches!(p, [0 | 1, 12, 16])
        });
        let mut job = simple_job(realm, owner, vec![worker], [8, 12, 24]);
        if unloading {
            plank(&mut journal, worker);
            job.next_step = job.steps.len();
            job.work_seconds = 1800.0;
        } else {
            stock(
                &mut journal,
                vec![ItemStack {
                    item: ItemKind::Block(16),
                    count: 1,
                }],
            );
        }
        let id = journal.gameplay_mut().buildings.insert(job).unwrap();
        let before = totals(&journal);
        let mut runtime = BuildRuntime::default();
        let properties = BlockPropertiesCatalog::default();
        for _ in 0..40 {
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
            BuildStatus::Waiting(BuildWait::NoPath),
            "unloading={unloading}"
        );
        assert_eq!(totals(&journal), before);
    }
}
