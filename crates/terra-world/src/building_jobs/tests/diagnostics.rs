use super::*;

fn inspect(
    chunks: &ChunkStore,
    journal: &EditJournal,
    terrain: &TerraGenerator,
    kingdom: KingdomId,
    owner: PlayerUuid,
    facing: u8,
    template: &str,
) -> Result<BuildingJob, BuildSiteIssue> {
    plan_template_detailed(
        chunks,
        journal,
        terrain,
        kingdom,
        owner,
        [8, 12, 24],
        facing,
        template,
    )
}

#[test]
fn detailed_planning_identifies_the_first_rejected_cell_without_mutation() {
    let (mut chunks, journal, terrain, kingdom, owner) = fixture(0);
    let first = [2, 12, 17];
    let later = [3, 13, 18];
    for position in [first, later] {
        set(&mut chunks, position, 16);
    }
    let before = journal.gameplay().clone();
    let edited = journal.edited_blocks();
    let issue = inspect(
        &chunks,
        &journal,
        &terrain,
        kingdom,
        owner,
        0,
        HOUSE_TEMPLATE,
    )
    .unwrap_err();
    assert_eq!(issue.error, BuildError::Blocked);
    assert_eq!(issue.cause, BuildSiteCause::NonPreparableTerrain);
    assert_eq!(issue.position, Some(first));
    assert_eq!(issue.material, Some(16));
    assert_eq!(issue.saved_material, None);
    assert_eq!(issue.conflicting_job, None);
    assert_eq!(
        inspect(
            &chunks,
            &journal,
            &terrain,
            kingdom,
            owner,
            0,
            HOUSE_TEMPLATE
        ),
        Err(issue)
    );
    assert_eq!(journal.gameplay(), &before);
    assert_eq!(journal.edited_blocks(), edited);
    for position in [first, later] {
        assert_eq!(chunks.block_at(position), Some(16));
    }
}

#[test]
fn legacy_wrapper_returns_the_exact_detailed_plan_or_error_in_every_facing() {
    let (chunks, journal, terrain, kingdom, owner) = fixture(0);
    for facing in 0..4 {
        let detailed = inspect(
            &chunks,
            &journal,
            &terrain,
            kingdom,
            owner,
            facing,
            HOUSE_TEMPLATE,
        )
        .unwrap();
        let legacy = plan_template(
            &chunks,
            &journal,
            &terrain,
            kingdom,
            owner,
            [8, 12, 24],
            facing,
            HOUSE_TEMPLATE,
        )
        .unwrap();
        assert_eq!(legacy, detailed);
    }
    for (actor, facing, template, cause) in [
        (owner, 4, HOUSE_TEMPLATE, BuildSiteCause::InvalidOrigin),
        (
            PlayerUuid::from_u128(999),
            0,
            HOUSE_TEMPLATE,
            BuildSiteCause::MissingMembership,
        ),
        (
            owner,
            0,
            "missing-template",
            BuildSiteCause::InvalidTemplate,
        ),
    ] {
        let issue = inspect(
            &chunks, &journal, &terrain, kingdom, actor, facing, template,
        )
        .unwrap_err();
        assert_eq!(issue.cause, cause);
        assert_eq!(
            plan_template(
                &chunks,
                &journal,
                &terrain,
                kingdom,
                actor,
                [8, 12, 24],
                facing,
                template,
            ),
            Err(issue.error)
        );
    }
}

#[test]
fn detailed_reservation_reports_the_existing_job_and_preserves_it() {
    let (chunks, mut journal, terrain, kingdom, owner) = fixture(0);
    let job = plan_house(&chunks, &journal, &terrain, kingdom, owner, [8, 12, 24], 0).unwrap();
    let id = journal.gameplay_mut().buildings.insert(job).unwrap();
    let before = journal.gameplay().clone();
    let issue = inspect(
        &chunks,
        &journal,
        &terrain,
        kingdom,
        owner,
        0,
        HOUSE_TEMPLATE,
    )
    .unwrap_err();
    assert_eq!(issue.error, BuildError::Blocked);
    assert_eq!(issue.cause, BuildSiteCause::ReservedByJob);
    assert_eq!(issue.position, Some([8, 12, 24]));
    assert_eq!(issue.conflicting_job, Some(id));
    assert_eq!(issue.material, None);
    assert_eq!(journal.gameplay(), &before);
}

#[test]
fn detailed_foundation_failure_reports_the_exact_unsupported_cell() {
    let (mut chunks, journal, terrain, kingdom, owner) = fixture(0);
    let unsupported = [2, 9, 17];
    set(&mut chunks, unsupported, 0);
    let issue = inspect(
        &chunks,
        &journal,
        &terrain,
        kingdom,
        owner,
        0,
        HOUSE_TEMPLATE,
    )
    .unwrap_err();
    assert_eq!(issue.error, BuildError::InvalidSite);
    assert_eq!(issue.cause, BuildSiteCause::UnsupportedGround);
    assert_eq!(issue.position, Some(unsupported));
    assert_eq!(issue.material, Some(0));
    assert_eq!(issue.saved_material, None);
    assert_eq!(
        plan_house(&chunks, &journal, &terrain, kingdom, owner, [8, 12, 24], 0),
        Err(issue.error)
    );
}

#[test]
fn detailed_cells_distinguish_stale_voxels_unloaded_data_and_denied_claims() {
    let first = [2, 10, 17];
    for cause in [
        BuildSiteCause::StaleOverride,
        BuildSiteCause::Unloaded,
        BuildSiteCause::PermissionDenied,
    ] {
        let (mut chunks, mut journal, terrain, kingdom, owner) = fixture(0);
        match cause {
            BuildSiteCause::StaleOverride => {
                journal.record(first, 1).unwrap();
            }
            BuildSiteCause::Unloaded => {
                chunks.loaded.remove(&global_to_section(first).0);
            }
            BuildSiteCause::PermissionDenied => {
                journal.gameplay_mut().kingdoms.claims.clear();
            }
            _ => unreachable!(),
        }
        let issue = inspect(
            &chunks,
            &journal,
            &terrain,
            kingdom,
            owner,
            0,
            HOUSE_TEMPLATE,
        )
        .unwrap_err();
        assert_eq!(issue.cause, cause);
        assert_eq!(issue.position, Some(first));
        assert_eq!(issue.material, chunks.block_at(first));
        assert_eq!(issue.saved_material, journal.block(first));
        assert_eq!(
            plan_house(&chunks, &journal, &terrain, kingdom, owner, [8, 12, 24], 0),
            Err(issue.error)
        );
    }
}
