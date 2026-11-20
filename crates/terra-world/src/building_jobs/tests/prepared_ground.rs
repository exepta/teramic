use super::*;

#[test]
fn a_paid_craftsman_finishes_on_saved_grass_and_dirt_without_excavating_the_working_ring() {
    build_full_house_with_preparation(
        0,
        HOUSE_TEMPLATE,
        false,
        false,
        SitePreparation::SavedSoilPad,
    );
}

#[test]
fn a_craftsman_excavates_a_four_metre_terrace_and_physically_finishes_the_house() {
    build_full_house_with_preparation(
        0,
        HOUSE_TEMPLATE,
        false,
        true,
        SitePreparation::FourMetreTerrace,
    );
}

#[test]
fn saved_soil_is_only_replaced_in_authored_floor_and_foundation_cells() {
    let (chunks, mut journal, terrain, kingdom, owner) = fixture(0);
    let origin = [8, 12, 24];
    let template = template_catalog().unwrap();
    let authored = template
        .get(HOUSE_TEMPLATE)
        .unwrap()
        .placed(origin, 0)
        .unwrap();
    let floor = authored
        .iter()
        .find(|step| step.phase == BuildPhase::Floor)
        .unwrap()
        .cells[0]
        .position;
    let foundation = authored
        .iter()
        .find(|step| step.phase == BuildPhase::Foundation)
        .unwrap()
        .cells[0]
        .position;
    let ring = [4, 11, 24];
    assert!(
        authored
            .iter()
            .flat_map(|step| &step.cells)
            .all(|cell| cell.position != ring)
    );
    for position in [floor, foundation, ring] {
        let material = chunks.block_at(position).unwrap();
        assert!(matches!(material, 1 | 2));
        journal.record(position, material).unwrap();
    }
    let edits = journal.edited_blocks();
    let job = plan_house(&chunks, &journal, &terrain, kingdom, owner, origin, 0).unwrap();
    assert!(job.valid());
    assert!(job.steps.iter().any(|step| {
        step.costs.is_empty()
            && step
                .cells
                .iter()
                .any(|cell| cell.position == floor && cell.material == 0)
    }));
    assert!(job.steps.iter().any(|step| {
        step.phase == BuildPhase::Foundation
            && step
                .cells
                .iter()
                .any(|cell| cell.position == foundation && cell.material != 0)
    }));
    assert!(
        job.steps
            .iter()
            .flat_map(|step| &step.cells)
            .all(|cell| cell.position != ring)
    );
    assert_eq!(journal.edited_blocks(), edits);
    for position in [floor, foundation, ring] {
        assert_eq!(chunks.block_at(position), journal.block(position));
    }
    let saved: BuildingJob = serde_json::from_str(&serde_json::to_string(&job).unwrap()).unwrap();
    assert_eq!(saved, job);
}

#[test]
fn four_block_terraces_have_one_persisted_removal_per_cell_in_every_facing() {
    assert_eq!(planning::MAX_TERRACE_PREPARATION_HEIGHT, 4);
    for facing in 0..4 {
        let (mut chunks, journal, terrain, kingdom, owner) = fixture(0);
        let terrace: Vec<_> = (12..16).map(|y| [4, y, 24]).collect();
        for &position in &terrace {
            set(&mut chunks, position, 19);
        }
        let job = plan_house(
            &chunks,
            &journal,
            &terrain,
            kingdom,
            owner,
            [8, 12, 24],
            facing,
        )
        .unwrap();
        for &position in &terrace {
            let steps: Vec<_> = job
                .steps
                .iter()
                .filter(|step| {
                    step.cells
                        .iter()
                        .any(|cell| cell.position == position && cell.material == 0)
                })
                .collect();
            assert_eq!(steps.len(), 1);
            assert_eq!(steps[0].phase, BuildPhase::Foundation);
            assert!(steps[0].costs.is_empty());
            assert_eq!(chunks.block_at(position), Some(19));
            assert_eq!(journal.block(position), None);
        }
        let saved: BuildingJob =
            serde_json::from_str(&serde_json::to_string(&job).unwrap()).unwrap();
        assert_eq!(saved, job);
        set(&mut chunks, [4, 16, 24], 19);
        assert_eq!(
            plan_house(
                &chunks,
                &journal,
                &terrain,
                kingdom,
                owner,
                [8, 12, 24],
                facing,
            ),
            Err(BuildError::Blocked)
        );
    }
}

#[test]
fn saved_stone_other_terrain_and_above_ground_soil_are_still_protected() {
    for (position, material) in [
        ([4, 11, 24], 3),
        ([4, 11, 24], 4),
        ([4, 11, 24], 5),
        ([4, 11, 24], 11),
        ([4, 11, 24], 13),
        ([4, 11, 24], 19),
        ([4, 11, 24], 42),
        ([4, 11, 24], 48),
        ([4, 12, 24], 1),
        ([4, 12, 24], 2),
        ([4, 15, 24], 1),
    ] {
        let (mut chunks, mut journal, terrain, kingdom, owner) = fixture(0);
        set(&mut chunks, position, material);
        journal.record(position, material).unwrap();
        assert_eq!(
            plan_house(&chunks, &journal, &terrain, kingdom, owner, [8, 12, 24], 0),
            Err(BuildError::Blocked),
            "saved material {material} at {position:?} remains protected"
        );
        assert_eq!(chunks.block_at(position), Some(material));
        assert_eq!(journal.block(position), Some(material));
    }
}

#[test]
fn stale_ground_or_air_edits_never_hide_an_obstacle() {
    for (loaded, saved) in [(0, 1), (0, 2), (0, 16), (1, 0), (2, 1)] {
        let (mut chunks, mut journal, terrain, kingdom, owner) = fixture(0);
        let position = [4, 11, 24];
        set(&mut chunks, position, loaded);
        journal.record(position, saved).unwrap();
        assert_eq!(
            plan_house(&chunks, &journal, &terrain, kingdom, owner, [8, 12, 24], 0),
            Err(BuildError::Blocked)
        );
        assert_eq!(chunks.block_at(position), Some(loaded));
        assert_eq!(journal.block(position), Some(saved));
    }
}

#[test]
fn later_stone_edits_pause_floor_preparation_and_foundation_without_spending_materials() {
    for foundation in [false, true] {
        let (mut chunks, mut journal, terrain, kingdom, owner) = fixture(1);
        let origin = [8, 12, 24];
        let mut job = plan_house(&chunks, &journal, &terrain, kingdom, owner, origin, 0).unwrap();
        let step = job
            .steps
            .iter()
            .find(|step| {
                step.phase == BuildPhase::Foundation
                    && if foundation {
                        step.cells[0].material != 0 && step.cells[0].position[1] == 10
                    } else {
                        step.cells[0].material == 0 && step.cells[0].position[1] == 11
                    }
            })
            .unwrap()
            .clone();
        let position = step.cells[0].position;
        let soil = chunks.block_at(position).unwrap();
        assert!(matches!(soil, 1 | 2));
        // Previous floor work exposes a buried foundation. Keep one adjacent
        // real standing cell so this tests preflight, not an obstructed path.
        for x in [position[0] - 1, position[0]] {
            for y in position[1] + 1..=12 {
                let air = [x, y, position[2]];
                set(&mut chunks, air, 0);
                journal.record(air, 0).unwrap();
            }
        }
        let worker = journal.gameplay().buildings.settlements[&kingdom].craftsmen[0];
        journal
            .gameplay_mut()
            .creatures
            .animals
            .get_mut(&worker)
            .unwrap()
            .position = [
            position[0] as f64 - 0.5,
            position[1] as f64 + 1.0,
            position[2] as f64 + 0.5,
        ];
        let mut bag = terra_core::npc_jobs::WorkerState::default();
        for cost in &step.costs {
            assert_eq!(bag.insert(cost.item, cost.count), 0);
        }
        journal
            .gameplay_mut()
            .buildings
            .inventories
            .insert(worker, bag.inventory);
        job.steps = vec![step];
        job.workers = vec![worker];
        assert!(job.valid());
        let id = journal.gameplay_mut().buildings.insert(job).unwrap();
        set(&mut chunks, position, 3);
        journal.record(position, 3).unwrap();
        let before = totals(&journal);
        let properties = BlockPropertiesCatalog::default();
        let mut runtime = BuildRuntime::default();
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
        let blocked = &journal.gameplay().buildings.jobs[&id];
        assert_eq!(blocked.status, BuildStatus::Waiting(BuildWait::Blocked));
        assert_eq!(blocked.next_step, 0);
        assert_eq!(blocked.work_seconds, 0.0);
        assert_eq!(totals(&journal), before);
        assert_eq!(chunks.block_at(position), Some(3));
        assert_eq!(journal.block(position), Some(3));
        set(&mut chunks, position, soil);
        journal.record(position, soil).unwrap();
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
        assert!(journal.gameplay().buildings.jobs[&id].work_seconds > 0.0);
        assert_eq!(totals(&journal), before);
    }
}
