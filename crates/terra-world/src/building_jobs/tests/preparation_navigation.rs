use super::*;

fn broad_terrace(chunks: &mut ChunkStore, origin: [i64; 3], facing: u8) -> usize {
    let mut count = 0;
    for x in -6..=5 {
        for z in -7..=5 {
            for y in 0..4 {
                set(chunks, planning::point(origin, [x, y, z], facing), 19);
                count += 1;
            }
        }
    }
    count
}

#[test]
fn broad_terraces_are_opened_from_above_in_every_orientation() {
    for facing in 0..4 {
        let (mut chunks, journal, terrain, kingdom, owner) = fixture(0);
        let origin = [8, 12, 24];
        let count = broad_terrace(&mut chunks, origin, facing);
        let job = plan_house(&chunks, &journal, &terrain, kingdom, owner, origin, facing).unwrap();
        let preparation = &job.steps[..count];
        for step in preparation {
            let cell = &step.cells[0];
            assert_eq!(step.phase, BuildPhase::Foundation);
            assert!(step.costs.is_empty());
            assert_eq!(cell.material, 0);
            assert_eq!(chunks.block_at(cell.position), Some(19));
            let above = [cell.position[0], cell.position[1] + 1, cell.position[2]];
            assert_eq!(
                chunks.block_at(above),
                Some(0),
                "an excavation target must have no remaining overburden: {:?}",
                cell.position,
            );
            set(&mut chunks, cell.position, 0);
        }
        assert!(
            preparation
                .windows(2)
                .all(|pair| { pair[0].cells[0].position[1] >= pair[1].cells[0].position[1] })
        );
        let saved: BuildingJob =
            serde_json::from_slice(&serde_json::to_vec(&job).unwrap()).unwrap();
        assert_eq!(
            saved, job,
            "the executable preparation order survives saving"
        );
    }
}

#[test]
fn a_craftsman_physically_clears_a_broad_four_metre_terrace() {
    let (mut chunks, mut journal, terrain, kingdom, owner) = fixture(1);
    let origin = [8, 12, 24];
    let count = broad_terrace(&mut chunks, origin, 0);
    let npc = journal.gameplay().buildings.settlements[&kingdom].craftsmen[0];
    journal
        .gameplay_mut()
        .creatures
        .animals
        .get_mut(&npc)
        .unwrap()
        .position = [8.5, 16.0, 24.5];
    let mut job = plan_house(&chunks, &journal, &terrain, kingdom, owner, origin, 0).unwrap();
    job.free_resources = true;
    let id = journal.gameplay_mut().buildings.insert(job).unwrap();
    let properties = BlockPropertiesCatalog::default();
    let mut runtime = BuildRuntime::default();
    runtime.set_creative_owners([owner]);
    let mut previous = 0;
    let mut stalled = 0;
    let start = journal.gameplay().creatures.animals[&npc].position;
    for _ in 0..60_000 {
        tick(
            &mut chunks,
            &mut journal,
            &terrain,
            &properties,
            0.1,
            &mut runtime,
        );
        let job = &journal.gameplay().buildings.jobs[&id];
        if job.next_step >= count {
            break;
        }
        if job.next_step == previous {
            stalled += 1;
        } else {
            previous = job.next_step;
            stalled = 0;
        }
        assert!(
            stalled < 1_200,
            "builder stopped excavating at step {}: {:?}; npc={:?}; route={:?}",
            job.next_step,
            job.status,
            journal.gameplay().creatures.animals[&npc],
            runtime.inspect_route(npc),
        );
    }
    let job = &journal.gameplay().buildings.jobs[&id];
    assert!(job.next_step >= count, "all terrace layers were removed");
    assert_ne!(
        journal.gameplay().creatures.animals[&npc].position,
        start,
        "excavation requires real walking instead of remote terrain edits",
    );
    for step in &job.steps[..count] {
        assert_eq!(chunks.block_at(step.cells[0].position), Some(0));
        assert_eq!(journal.block(step.cells[0].position), Some(0));
    }
}

#[test]
fn a_craftsman_reaches_an_exposed_four_metre_cliff_from_a_fractional_ground_stance() {
    let (mut chunks, journal, terrain, kingdom, owner) = fixture(1);
    for x in 4..=7 {
        for z in 20..=23 {
            for y in 12..16 {
                set(&mut chunks, [x, y, z], 19);
            }
        }
    }
    let job = plan_house(&chunks, &journal, &terrain, kingdom, owner, [8, 12, 24], 0).unwrap();
    let step = &job.steps[0];
    let target = step.cells[0].position;
    assert_eq!(target, [4, 15, 20]);
    let npc = journal.gameplay().buildings.settlements[&kingdom].craftsmen[0];
    let mut npc = journal.gameplay().creatures.animals[&npc].clone();
    let start = npc.position;
    let mut runtime = BuildRuntime::default();
    let mut arrived = false;
    for _ in 0..600 {
        let previous = npc.position;
        arrived = movement::approach(
            &chunks,
            &journal,
            &terrain,
            &job,
            &mut npc,
            target,
            &step.cells,
            0.1,
            &mut runtime,
        );
        assert!((npc.position[0] - previous[0]).hypot(npc.position[2] - previous[2]) <= 0.221);
        assert_eq!(npc.position[1], 12.0, "the cliff has no climbing route");
        if arrived {
            break;
        }
    }
    assert!(
        arrived,
        "a physically supported ground stance can see the exposed top: {:?}; {:?}",
        npc.position,
        runtime.inspect_route(npc.id),
    );
    assert_ne!(npc.position, start);
    assert!(
        (npc.position[0] - npc.position[0].floor() - 0.5).abs() > 0.1
            || (npc.position[2] - npc.position[2].floor() - 0.5).abs() > 0.1,
        "the usable stance lies between ordinary cell centers",
    );
    assert!(movement::reachable(
        &Habitat {
            chunks: &chunks,
            journal: &journal,
            terrain: &terrain,
        },
        &npc,
        target,
    ));
    assert_eq!(chunks.block_at(target), Some(19));
    assert_eq!(journal.block(target), None);
}

#[test]
fn excavation_frontier_preserves_completed_work_and_stops_at_paid_or_later_phases() {
    let (mut chunks, journal, terrain, kingdom, owner) = fixture(1);
    let worker = journal.gameplay().buildings.settlements[&kingdom].craftsmen[0];
    let npc = &journal.gameplay().creatures.animals[&worker];
    let completed = [2, 12, 20];
    let blocked = [10, 12, 24];
    let exposed = [3, 12, 18];
    for position in [completed, blocked, exposed] {
        set(&mut chunks, position, 19);
    }
    let habitat = Habitat {
        chunks: &chunks,
        journal: &journal,
        terrain: &terrain,
    };
    assert!(movement::reachable(&habitat, npc, completed));
    assert!(!movement::reachable(&habitat, npc, blocked));
    assert!(movement::reachable(&habitat, npc, exposed));
    let excavation = |position| BuildStep {
        phase: BuildPhase::Foundation,
        cells: vec![BuildCell {
            position,
            material: 0,
            facing: 0,
            peer: None,
            secondary: false,
        }],
        costs: Vec::new(),
    };
    let mut job = simple_job(kingdom, owner, vec![worker], blocked);
    job.steps = vec![
        excavation(completed),
        excavation(blocked),
        excavation(exposed),
    ];
    job.next_step = 1;
    job.work_seconds = 12.0;
    let mut expected = job.clone();
    expected.steps.swap(1, 2);
    assert!(preparation::advance_frontier(&mut job, npc, &habitat));
    assert_eq!(job, expected, "only pending removals may exchange places");

    let mut paid = excavation([8, 12, 22]);
    paid.cells[0].material = 16;
    paid.costs = vec![ItemStack {
        item: ItemKind::Block(16),
        count: 1,
    }];
    let mut other_phase = excavation([8, 12, 22]);
    other_phase.phase = BuildPhase::Walls;
    let mut free_placement = excavation([8, 12, 22]);
    free_placement.cells[0].material = 16;
    for barrier in [paid, other_phase, free_placement] {
        job.steps = vec![
            excavation(completed),
            excavation(blocked),
            barrier,
            excavation(exposed),
        ];
        let before = job.clone();
        assert!(job.valid());
        assert!(!preparation::advance_frontier(&mut job, npc, &habitat));
        assert_eq!(job, before, "the next excavation run must not be entered");
    }
    assert_eq!(chunks.block_at(exposed), Some(19));
    assert_eq!(journal.block(exposed), None);
}
