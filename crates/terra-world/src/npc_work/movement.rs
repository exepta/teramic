use super::*;

#[allow(clippy::too_many_arguments)]
pub(super) fn approach(
    chunks: &ChunkStore,
    journal: &EditJournal,
    terrain: &TerraGenerator,
    order: &WorkOrder,
    npc: &mut Creature,
    worker: &mut WorkerState,
    cell: [i64; 3],
    dt: f32,
    navigation: &mut Navigation,
    storage: bool,
) -> bool {
    let habitat = Habitat {
        terrain,
        chunks,
        journal,
    };
    if chunks.block_at(cell).is_none() {
        wait(worker, npc, WaitReason::Unloaded);
        return false;
    }
    if reachable(&habitat, npc, cell) {
        npc.velocity = [0.0; 3];
        npc.behavior = CreatureBehavior::Rest;
        return true;
    }
    let mut candidates = Vec::with_capacity(24);
    for dy in [-1, 0, 1] {
        for [dx, dz] in [
            [-1, 0],
            [1, 0],
            [0, -1],
            [0, 1],
            [-1, -1],
            [1, 1],
            [-1, 1],
            [1, -1],
        ] {
            let point = [
                cell[0] as f64 + 0.5 + f64::from(dx),
                cell[1] as f64 + f64::from(dy),
                cell[2] as f64 + 0.5 + f64::from(dz),
            ];
            if let Some(point) = habitat.ground(point, CreatureKind::Villager)
                && allowed(
                    chunks,
                    journal,
                    order,
                    KingdomPermission::NpcWork,
                    point.map(|v| v.floor() as i64),
                )
            {
                candidates.push(point);
            }
        }
    }
    let target = candidates.into_iter().min_by(|a, b| {
        crate::creatures::distance(*a, npc.position)
            .total_cmp(&crate::creatures::distance(*b, npc.position))
    });
    let Some(target) = target else {
        wait(worker, npc, WaitReason::NoPath);
        return false;
    };
    worker.status = if storage {
        WorkStatus::Delivering
    } else {
        WorkStatus::Travelling
    };
    let before = npc.clone();
    if !crate::creatures::work_step(npc, &habitat, target, dt, navigation) {
        wait(worker, npc, WaitReason::NoPath);
    } else if !allowed(
        chunks,
        journal,
        order,
        KingdomPermission::NpcWork,
        npc.position.map(|v| v.floor() as i64),
    ) {
        *npc = before;
        wait(worker, npc, WaitReason::Protected);
    }
    false
}

fn reachable(habitat: &Habitat, npc: &Creature, cell: [i64; 3]) -> bool {
    let center = cell.map(|v| v as f64 + 0.5);
    if (npc.position[0] - center[0]).hypot(npc.position[2] - center[2]) > 1.8
        || (npc.position[1] - center[1]).abs() > 1.8
    {
        return false;
    }
    let eye = [npc.position[0], npc.position[1] + 1.25, npc.position[2]];
    let direction = std::array::from_fn(|axis| center[axis] - eye[axis]);
    let reach = direction.iter().map(|v| v * v).sum::<f64>().sqrt();
    if reach > 3.5 {
        return false;
    }
    terra_voxel::blocks::raycast_blocks(eye, direction, reach + 0.01, |position| {
        habitat.block(position)
    })
    .is_none_or(|hit| hit.position == cell)
}
