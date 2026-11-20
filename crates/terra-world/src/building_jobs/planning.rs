use super::*;
use std::collections::BTreeSet;
use terra_core::{ItemStack, building_jobs::template::BuildingTemplate};

/// At most four natural blocks above the selected foot plane are levelled.
/// This covers the world's four-metre terraces without accepting entire hills.
pub(super) const MAX_TERRACE_PREPARATION_HEIGHT: i64 = 4;

/// Only raw terrain can be prepared. Roads, farms and building stone remain
/// obstacles even when their generated cells have no player edit entry.
pub(super) fn natural_ground(material: u16) -> bool {
    terra_voxel::MaterialId::from_raw(material).is_some_and(|material| {
        material.is_natural_ground()
            && !matches!(
                material,
                terra_voxel::MaterialId::Farmland | terra_voxel::MaterialId::Path
            )
    })
}

/// Saved grass/dirt below the foot plane is a prepared building pad. Its
/// journal entry can also come from natural grass succession. Other saved
/// materials and every above-ground edit remain protected constructions.
pub(super) fn preparable_ground(
    material: u16,
    saved_material: Option<u16>,
    height: i64,
    origin_height: i64,
) -> bool {
    natural_ground(material)
        && height.saturating_sub(origin_height) < MAX_TERRACE_PREPARATION_HEIGHT
        && saved_material.is_none_or(|saved| {
            saved == material
                && height < origin_height
                && matches!(
                    terra_voxel::MaterialId::from_raw(material),
                    Some(terra_voxel::MaterialId::Grass | terra_voxel::MaterialId::Dirt)
                )
        })
}

pub(super) fn point(origin: [i64; 3], p: [i64; 3], facing: u8) -> [i64; 3] {
    let [x, z] = match facing {
        0 => [p[0], p[2]],
        1 => [p[2], -p[0]],
        2 => [-p[0], -p[2]],
        _ => [-p[2], p[0]],
    };
    [origin[0] + x, origin[1] + p[1], origin[2] + z]
}
fn single(
    origin: [i64; 3],
    p: [i64; 3],
    material: u16,
    phase: BuildPhase,
    facing: u8,
) -> BuildStep {
    BuildStep {
        phase,
        cells: vec![BuildCell {
            position: point(origin, p, facing),
            material,
            facing: 0,
            peer: None,
            secondary: false,
        }],
        costs: if material == 0 {
            vec![]
        } else {
            vec![ItemStack {
                item: ItemKind::Block(material),
                count: 1,
            }]
        },
    }
}

pub fn preview_house() -> Result<Vec<BuildStep>, BuildError> {
    authored([0; 3], 0)
}

pub(super) fn authored(origin: [i64; 3], facing: u8) -> Result<Vec<BuildStep>, BuildError> {
    authored_template(origin, facing, HOUSE_TEMPLATE)
}

fn authored_template(
    origin: [i64; 3],
    facing: u8,
    template_id: &str,
) -> Result<Vec<BuildStep>, BuildError> {
    template_catalog()?
        .get(template_id)
        .ok_or(BuildError::InvalidTemplate)?
        .placed(origin, facing)
        .map_err(|_| BuildError::InvalidTemplate)
}

pub fn plan_house(
    chunks: &ChunkStore,
    journal: &EditJournal,
    terrain: &TerraGenerator,
    kingdom: KingdomId,
    owner: PlayerUuid,
    origin: [i64; 3],
    facing: u8,
) -> Result<BuildingJob, BuildError> {
    plan_template(
        chunks,
        journal,
        terrain,
        kingdom,
        owner,
        origin,
        facing,
        HOUSE_TEMPLATE,
    )
}

/// Resolve a server catalog ID into a complete, independently persisted plan.
#[allow(clippy::too_many_arguments)]
pub fn plan_template(
    chunks: &ChunkStore,
    journal: &EditJournal,
    terrain: &TerraGenerator,
    kingdom: KingdomId,
    owner: PlayerUuid,
    origin: [i64; 3],
    facing: u8,
    template_id: &str,
) -> Result<BuildingJob, BuildError> {
    plan_template_detailed(
        chunks,
        journal,
        terrain,
        kingdom,
        owner,
        origin,
        facing,
        template_id,
    )
    .map_err(|issue| issue.error)
}

/// The same authoritative plan with the precise first rejected cell or job.
/// Diagnostics are local-only; callers keep the existing `BuildError` on the wire.
#[allow(clippy::too_many_arguments)]
pub fn plan_template_detailed(
    chunks: &ChunkStore,
    journal: &EditJournal,
    terrain: &TerraGenerator,
    kingdom: KingdomId,
    owner: PlayerUuid,
    origin: [i64; 3],
    facing: u8,
    template_id: &str,
) -> Result<BuildingJob, BuildSiteIssue> {
    let catalog = template_catalog()
        .map_err(|error| BuildSiteIssue::new(error, BuildSiteCause::InvalidTemplate))?;
    let template = catalog.get(template_id).ok_or(BuildSiteIssue::new(
        BuildError::InvalidTemplate,
        BuildSiteCause::InvalidTemplate,
    ))?;
    plan_resolved_detailed(
        chunks, journal, terrain, kingdom, owner, origin, facing, template,
    )
}

#[cfg(test)]
#[allow(clippy::too_many_arguments)]
pub(super) fn plan_resolved(
    chunks: &ChunkStore,
    journal: &EditJournal,
    terrain: &TerraGenerator,
    kingdom: KingdomId,
    owner: PlayerUuid,
    origin: [i64; 3],
    facing: u8,
    template: &BuildingTemplate,
) -> Result<BuildingJob, BuildError> {
    plan_resolved_detailed(
        chunks, journal, terrain, kingdom, owner, origin, facing, template,
    )
    .map_err(|issue| issue.error)
}

#[allow(clippy::too_many_arguments)]
fn plan_resolved_detailed(
    chunks: &ChunkStore,
    journal: &EditJournal,
    terrain: &TerraGenerator,
    kingdom: KingdomId,
    owner: PlayerUuid,
    origin: [i64; 3],
    facing: u8,
    template: &BuildingTemplate,
) -> Result<BuildingJob, BuildSiteIssue> {
    if facing > 3 || origin.iter().any(|v| v.unsigned_abs() > 998_900) {
        return Err(BuildSiteIssue::new(
            BuildError::InvalidSite,
            BuildSiteCause::InvalidOrigin,
        ));
    }
    if !journal
        .gameplay()
        .kingdoms
        .get(kingdom)
        .is_some_and(|k| k.members.contains(&owner))
    {
        return Err(BuildSiteIssue::new(
            BuildError::Protected,
            BuildSiteCause::MissingMembership,
        ));
    }
    // Reserve the full building, access ramp and working ring. Natural terraces
    // can be levelled by bounded, actual preparation steps. Mobile
    // creatures do not reserve a blueprint; physical placement still checks
    // their bodies in simulation::preflight. Never erase trees, generated
    // structures, stored containers, player blocks or unknown sections.
    let mut raised_ground = BTreeSet::new();
    for x in -6_i64..=5 {
        for z in -7_i64..=5 {
            for y in -2..=9 {
                let p = point(origin, [x, y, z], facing);
                if !build_access(chunks, journal, kingdom, owner, p) {
                    return Err(BuildSiteIssue::at(
                        BuildError::Protected,
                        BuildSiteCause::PermissionDenied,
                        p,
                        chunks.block_at(p),
                        journal.block(p),
                    ));
                }
                let material = chunks.block_at(p).ok_or_else(|| {
                    BuildSiteIssue::at(
                        BuildError::Unloaded,
                        BuildSiteCause::Unloaded,
                        p,
                        None,
                        journal.block(p),
                    )
                })?;
                let saved_material = journal.block(p);
                // A stale section must not hide a saved obstacle or pretend
                // that a queued air edit has already been applied.
                if saved_material.is_some_and(|saved| saved != material) {
                    return Err(BuildSiteIssue::at(
                        BuildError::Blocked,
                        BuildSiteCause::StaleOverride,
                        p,
                        Some(material),
                        saved_material,
                    ));
                }
                if material != 0 {
                    if !preparable_ground(material, saved_material, p[1], origin[1]) {
                        return Err(BuildSiteIssue::at(
                            BuildError::Blocked,
                            BuildSiteCause::NonPreparableTerrain,
                            p,
                            Some(material),
                            saved_material,
                        ));
                    }
                    if terrain.structure_block(p).is_some_and(|block| block != 0) {
                        return Err(BuildSiteIssue::at(
                            BuildError::Blocked,
                            BuildSiteCause::GeneratedStructure,
                            p,
                            Some(material),
                            saved_material,
                        ));
                    }
                    if y >= 0 {
                        raised_ground.insert(p);
                    }
                }
                let object = if journal.container_at(p).is_some() {
                    Some(BuildSiteCause::Container)
                } else if journal.object_state(p).is_some() {
                    Some(BuildSiteCause::ObjectState)
                } else if journal.fluid(p).is_some_and(|f| f.amount > 0) {
                    Some(BuildSiteCause::Fluid)
                } else {
                    None
                };
                if let Some(cause) = object {
                    return Err(BuildSiteIssue::at(
                        BuildError::Blocked,
                        cause,
                        p,
                        Some(material),
                        saved_material,
                    ));
                }
            }
        }
    }
    for existing in journal
        .gameplay()
        .buildings
        .jobs
        .values()
        .filter(|j| j.active())
    {
        if (existing.origin[0] - origin[0]).abs() < 14
            && (existing.origin[2] - origin[2]).abs() < 14
        {
            return Err(BuildSiteIssue::reserved(existing.id, existing.origin));
        }
    }
    let house = template.placed(origin, facing).map_err(|_| {
        BuildSiteIssue::new(BuildError::InvalidTemplate, BuildSiteCause::InvalidTemplate)
    })?;
    let mut steps = Vec::new();
    let mut scaffold = Vec::new();
    // Open each terrace from above before removing its buried lower layers.
    // Otherwise an interaction ray hits the overburden instead of the next
    // excavation target, even when the builder can walk around the whole site.
    // X/Z ties keep the persisted plan deterministic in every orientation.
    // These are real, persisted zero-cost steps; runtime rechecks preserve
    // structures placed after the blueprint was queued.
    let mut excavation: Vec<_> = raised_ground.iter().copied().collect();
    excavation.sort_unstable_by_key(|p| (std::cmp::Reverse(p[1]), p[0], p[2]));
    for position in excavation {
        steps.push(BuildStep {
            phase: BuildPhase::Foundation,
            cells: vec![BuildCell {
                position,
                material: 0,
                facing: 0,
                peer: None,
                secondary: false,
            }],
            costs: Vec::new(),
        });
    }
    for x in -6_i64..=5 {
        for z in -7_i64..=5 {
            let base = point(origin, [x, -3, z], facing);
            if chunks.block_at(base) == Some(0) {
                return Err(BuildSiteIssue::at(
                    BuildError::InvalidSite,
                    BuildSiteCause::UnsupportedGround,
                    base,
                    Some(0),
                    journal.block(base),
                ));
            }
            if x == -6 && (-7..=0).contains(&z)
                || (x.abs() == 5 || z == -7 || z == 5) && !(z == -7 && (-1..=1).contains(&x))
            {
                for y in -2..=-1 {
                    let p = point(origin, [x, y, z], facing);
                    if chunks.block_at(p) == Some(0) {
                        let step = single(origin, [x, y, z], 16, BuildPhase::Foundation, facing);
                        scaffold.push(p);
                        steps.push(step);
                    }
                }
            }
        }
    }
    // Open the natural ground layer before replacing buried house foundations.
    // A custom floor at local y=0 may already have a terrace preparation step.
    let mut excavate = raised_ground;
    for step in house.iter().filter(|s| s.phase == BuildPhase::Floor) {
        for c in &step.cells {
            if chunks.block_at(c.position) != Some(0) && excavate.insert(c.position) {
                steps.push(BuildStep {
                    phase: BuildPhase::Foundation,
                    cells: vec![BuildCell {
                        position: c.position,
                        material: 0,
                        facing: 0,
                        peer: None,
                        secondary: false,
                    }],
                    costs: Vec::new(),
                });
            }
        }
    }

    steps.extend(
        house
            .iter()
            .filter(|s| matches!(s.phase, BuildPhase::Foundation | BuildPhase::Floor))
            .cloned(),
    );
    // An interior working deck lets builders assemble the roof from below.
    // Its stairwell remains open, so a completed roof never strands workers on
    // disconnected wings or removes their route back to the town chests.
    let mut interior = BTreeSet::new();
    for y in 0..=3 {
        for z in (-2 + y)..=1 {
            let step = single(origin, [-2, y, z], 16, BuildPhase::Walls, facing);
            interior.insert([-2, y, z]);
            scaffold.push(step.cells[0].position);
            steps.push(step);
        }
    }
    let mut deck = Vec::<[i64; 3]>::new();
    for x in -2_i64..=2 {
        for z in -2_i64..=2 {
            let p = [x, 3, z];
            if !(x == -2 && z <= 0) && !interior.contains(&p) {
                deck.push(p);
            }
        }
    }
    deck.sort_by_key(|p| (p[0] + 2).abs() + (p[2] - 1).abs());
    for p in deck {
        let step = single(origin, p, 16, BuildPhase::Walls, facing);
        scaffold.push(step.cells[0].position);
        steps.push(step);
    }
    let mut ramp_added = -1;
    for level in 0..=9 {
        // Separate decks leave two metres of real headroom. Solid stacked rings
        // would form a wall trapping ground-level workers inside the scaffold.
        if let Some(ring) = match level {
            2 => Some(0),
            3 => Some(3),
            6 => Some(6),
            _ => None,
        } {
            while ramp_added < ring {
                ramp_added += 1;
                for z in (ramp_added - 7)..=0 {
                    let step = single(origin, [-6, ramp_added, z], 16, BuildPhase::Walls, facing);
                    scaffold.push(step.cells[0].position);
                    steps.push(step);
                }
            }
            let mut cells: Vec<[i64; 3]> = Vec::new();
            for z in -7_i64..=5 {
                for x in -5_i64..=5 {
                    if (x.abs() == 5 || z == -7 || z == 5) && !(z == -7 && (-1..=1).contains(&x)) {
                        cells.push([x, ring, z]);
                    }
                }
            }
            cells.sort_by_key(|p| (p[0] + 5).abs() + (p[2] - (ring - 7)).abs());
            for p in cells {
                let step = single(origin, p, 16, BuildPhase::Walls, facing);
                scaffold.push(step.cells[0].position);
                steps.push(step);
            }
        }
        steps.extend(
            house
                .iter()
                .filter(|s| {
                    !matches!(
                        s.phase,
                        BuildPhase::Foundation | BuildPhase::Floor | BuildPhase::Furnishing
                    ) && s.cells[0].position[1] - origin[1] == level
                })
                .cloned(),
        );
    }
    for p in scaffold.into_iter().rev() {
        steps.push(BuildStep {
            phase: BuildPhase::Cleanup,
            cells: vec![BuildCell {
                position: p,
                material: 0,
                facing: 0,
                peer: None,
                secondary: false,
            }],
            costs: Vec::new(),
        });
    }
    // Furniture is reached through the still-open doorway; paired doors are last.
    let mut furniture: Vec<_> = house
        .into_iter()
        .filter(|s| s.phase == BuildPhase::Furnishing)
        .collect();
    furniture.sort_by_key(|s| {
        (
            matches!(s.cells[0].material, 65..=67),
            s.cells[0].position[1],
        )
    });
    steps.extend(furniture);
    let job = BuildingJob {
        id: 1,
        kingdom,
        owner,
        origin,
        facing,
        template: template.id.clone(),
        free_resources: false,
        free_from_step: 0,
        workers: Vec::new(),
        steps,
        next_step: 0,
        work_seconds: 0.0,
        status: BuildStatus::Queued,
        enabled: true,
        revision: 1,
    };
    job.valid().then_some(job).ok_or(BuildSiteIssue::new(
        BuildError::InvalidTemplate,
        BuildSiteCause::InvalidTemplate,
    ))
}
