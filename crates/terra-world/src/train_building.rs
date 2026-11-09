//! Atomic material-backed construction of the same guide shown by the workshop.
use super::*;
use terra_core::train_blueprint::{self, BlueprintCell};
#[derive(Clone, Copy)]
pub struct BuildRequest {
    pub actor: PlayerUuid,
    pub station: Cell,
    pub chassis: Cell,
    pub facing: u8,
    pub kind: VehicleKind,
    pub wheelbase: u8,
    pub width: u8,
    pub creative: bool,
}
pub struct BuildPlan {
    pub request: BuildRequest,
    pub cells: Vec<(BlueprintCell, terra_core::WoodSpecies)>,
    pub(crate) before: Inventory,
    pub(crate) after: Inventory,
}
pub fn plan_blueprint(
    chunks: &ChunkStore,
    journal: &EditJournal,
    request: BuildRequest,
) -> Result<BuildPlan, TrainError> {
    station_ready(chunks, journal, request.actor, request.station)?;
    let guide = train_blueprint::building_template_with_width(
        request.chassis,
        request.facing,
        request.kind,
        request.wheelbase,
        request.width,
    );
    if guide.is_empty()
        || guide.iter().any(|part| {
            (0..3).any(|axis| part.cell[axis].abs_diff(request.station[axis]) > 32)
                || !(-128..=356).contains(&part.cell[1])
        })
    {
        return Err(TrainError::Invalid);
    }
    if guide
        .iter()
        .filter(|part| part.material == train_parts::BOGIE)
        .any(|part| !bogie_supported_with_facing(chunks, journal, part.cell, part.facing))
    {
        return Err(TrainError::Unsupported);
    }
    let before = journal
        .gameplay()
        .inventories
        .get(&request.actor)
        .cloned()
        .unwrap_or_default();
    if before.revision == u64::MAX || journal.gameplay().revision == u64::MAX {
        return Err(TrainError::Overflow);
    }
    let mut after = before.clone();
    let mut cells = Vec::new();
    for part in guide {
        if !allowed(
            chunks,
            journal,
            request.actor,
            part.cell,
            KingdomPermission::Build,
        ) {
            return Err(TrainError::Protected);
        }
        let current = chunks.block_at(part.cell).ok_or(TrainError::Incomplete)?;
        if train_blueprint::matches(
            part,
            Some(current),
            journal
                .object_state(part.cell)
                .map_or(0, |state| state.facing),
        ) {
            continue;
        }
        if current != 0 {
            return Err(TrainError::Occupied);
        }
        if terra_core::train_boiler::is_wide(part.material, part.facing) {
            // A three-wide model must fit outside its one owner voxel as well.
            // Existing matching parts are skipped above; only new geometry is checked.
            for shape in terra_core::train_parts::boxes(part.material, part.facing) {
                let low: [i64; 3] = std::array::from_fn(|axis| {
                    (part.cell[axis] as f64 + shape.minimum[axis] + 1e-6).floor() as i64
                });
                let high: [i64; 3] = std::array::from_fn(|axis| {
                    (part.cell[axis] as f64 + shape.maximum[axis] - 1e-6).floor() as i64
                });
                for x in low[0]..=high[0] {
                    for y in low[1]..=high[1] {
                        for z in low[2]..=high[2] {
                            let cell = [x, y, z];
                            if cell == part.cell {
                                continue;
                            }
                            if !allowed(
                                chunks,
                                journal,
                                request.actor,
                                cell,
                                KingdomPermission::Build,
                            ) {
                                return Err(TrainError::Protected);
                            }
                            if chunks.block_at(cell).ok_or(TrainError::Incomplete)? != 0 {
                                return Err(TrainError::Occupied);
                            }
                        }
                    }
                }
            }
        }
        let mut wood = terra_core::WoodSpecies::Spruce;
        if !request.creative {
            let stack = after
                .slots
                .iter()
                .copied()
                .find(|stack| stack.count > 0 && stack.item.block_material() == Some(part.material))
                .ok_or(TrainError::MissingMaterials)?;
            wood = stack.item.wood_species().unwrap_or(wood);
            if !after.take_one(stack.item) {
                return Err(TrainError::MissingMaterials);
            }
        }
        cells.push((part, wood));
    }
    if !cells.is_empty() && !request.creative {
        after.revision = before.revision + 1;
    }
    Ok(BuildPlan {
        request,
        cells,
        before,
        after,
    })
}
pub fn commit_blueprint(
    chunks: &mut ChunkStore,
    journal: &mut EditJournal,
    plan: &BuildPlan,
) -> Result<(), TrainError> {
    let fresh = plan_blueprint(chunks, journal, plan.request)?;
    if fresh.cells != plan.cells || fresh.before != plan.before || fresh.after != plan.after {
        return Err(TrainError::Stale);
    }
    journal.commit_train_build(chunks, plan)
}
