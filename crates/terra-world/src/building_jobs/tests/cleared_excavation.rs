use super::*;

fn excavation() -> BuildStep {
    BuildStep {
        phase: BuildPhase::Foundation,
        cells: vec![BuildCell {
            position: [8, 12, 24],
            material: 0,
            facing: 0,
            peer: None,
            secondary: false,
        }],
        costs: Vec::new(),
    }
}

#[test]
fn a_player_cleared_excavation_advances_without_walking_spending_or_earning_work() {
    for recorded_air in [false, true] {
        let (mut chunks, mut journal, terrain, realm, owner) = fixture(1);
        let worker = journal.gameplay().buildings.settlements[&realm].craftsmen[0];
        if recorded_air {
            journal.record([8, 12, 24], 0).unwrap();
        }
        plank(&mut journal, worker);
        let mut job = simple_job(realm, owner, vec![worker], [8, 12, 24]);
        job.steps.insert(0, excavation());
        job.work_seconds = 7.25;
        let id = journal.gameplay_mut().buildings.insert(job).unwrap();
        let before = totals(&journal);
        let position = journal.gameplay().creatures.animals[&worker].position;
        let mut runtime = BuildRuntime::default();
        tick(
            &mut chunks,
            &mut journal,
            &terrain,
            &BlockPropertiesCatalog::default(),
            0.1,
            &mut runtime,
        );
        let job = &journal.gameplay().buildings.jobs[&id];
        assert_eq!(job.next_step, 1);
        assert_eq!(job.work_seconds, 7.25);
        assert_eq!(job.status, BuildStatus::Travelling);
        assert_eq!(totals(&journal), before);
        assert_eq!(
            journal.gameplay().creatures.animals[&worker].position,
            position
        );
        assert_eq!(chunks.block_at([8, 12, 24]), Some(0));
        assert_eq!(journal.block([8, 12, 24]), recorded_air.then_some(0));
    }
}

#[test]
fn satisfying_excavation_requires_exact_free_single_air_step_and_authorized_dry_loaded_air() {
    let (mut chunks, mut journal, terrain, realm, owner) = fixture(1);
    let worker = journal.gameplay().buildings.settlements[&realm].craftsmen[0];
    let job = simple_job(realm, owner, vec![worker], [8, 12, 24]);
    let step = excavation();
    let allowed = |chunks: &ChunkStore, journal: &EditJournal, step: &BuildStep| {
        preparation::already_cleared(chunks, journal, &terrain, &job, step)
    };
    assert!(allowed(&chunks, &journal, &step));
    for phase in [BuildPhase::Walls, BuildPhase::Floor, BuildPhase::Cleanup] {
        let mut other = step.clone();
        other.phase = phase;
        assert!(!allowed(&chunks, &journal, &other));
    }
    let mut other = step.clone();
    other.cells[0].material = 16;
    assert!(!allowed(&chunks, &journal, &other));
    other = step.clone();
    other.costs.push(ItemStack {
        item: ItemKind::Block(16),
        count: 1,
    });
    assert!(!allowed(&chunks, &journal, &other));
    other = step.clone();
    other.cells.push(other.cells[0].clone());
    assert!(!allowed(&chunks, &journal, &other));
    let position = step.cells[0].position;
    journal.record(position, 1).unwrap();
    assert!(
        !allowed(&chunks, &journal, &step),
        "stale journal is not air"
    );
    journal.record(position, 0).unwrap();
    journal
        .set_fluid(
            position,
            crate::water::FluidCell {
                amount: 8,
                source: true,
                ..Default::default()
            },
        )
        .unwrap();
    assert!(
        !allowed(&chunks, &journal, &step),
        "water is not a dry excavation"
    );
    let mut dry = EditJournal::default();
    dry.gameplay_mut().kingdoms = journal.gameplay().kingdoms.clone();
    assert!(allowed(&chunks, &dry, &step));
    dry.gameplay_mut().kingdoms.claims.clear();
    assert!(
        !allowed(&chunks, &dry, &step),
        "foreign ground is not skipped"
    );
    let section = global_to_section(position).0;
    chunks.loaded.remove(&section);
    assert!(
        !allowed(&chunks, &journal, &step),
        "unloaded ground is unknown"
    );
}
