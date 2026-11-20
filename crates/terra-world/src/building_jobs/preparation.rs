//! Resume older excavation plans without digging underneath their overburden.
use super::*;
use std::cmp::Reverse;

fn excavation(step: &BuildStep) -> bool {
    step.phase == BuildPhase::Foundation
        && step.costs.is_empty()
        && step.cells.len() == 1
        && step.cells[0].material == 0
        && step.cells[0].peer.is_none()
        && !step.cells[0].secondary
}

fn key(step: &BuildStep) -> (Reverse<i64>, i64, i64) {
    let [x, y, z] = step.cells[0].position;
    (Reverse(y), x, z)
}

/// A player may finish a queued excavation before the worker arrives. Only
/// an actual, dry empty cell satisfies it; no placement or cleanup is skipped.
pub(super) fn already_cleared(
    chunks: &ChunkStore,
    journal: &EditJournal,
    terrain: &TerraGenerator,
    job: &BuildingJob,
    step: &BuildStep,
) -> bool {
    if !excavation(step) {
        return false;
    }
    let position = step.cells[0].position;
    chunks.block_at(position) == Some(0)
        && journal.block(position).is_none_or(|material| material == 0)
        && journal.container_at(position).is_none()
        && journal.object_state(position).is_none()
        && journal
            .fluid(position)
            .is_none_or(|fluid| fluid.amount == 0)
        && chunks.fluid_cell_at(terrain, journal, position, 0).amount == 0
        && build_access(chunks, journal, job.kingdom, job.owner, position)
}

/// Only reorder pending terrain removal within its original construction phase.
/// Completed work, supplies, scaffolds, costs and progress retain their indices.
pub(super) fn resume(job: &mut BuildingJob) -> bool {
    let mut changed = false;
    let mut remaining = &mut job.steps[job.next_step..];
    while !remaining.is_empty() {
        let skip = remaining.iter().take_while(|s| !excavation(s)).count();
        remaining = &mut remaining[skip..];
        let count = remaining.iter().take_while(|s| excavation(s)).count();
        let (segment, rest) = remaining.split_at_mut(count);
        if segment.windows(2).any(|pair| key(&pair[0]) > key(&pair[1])) {
            segment.sort_unstable_by_key(key);
            changed = true;
        }
        remaining = rest;
    }
    changed
}

/// A cliff's exposed side can be reached from below even when its interior
/// crown cannot. Work another physically visible cell in this same excavation
/// run instead of demanding an impossible climb onto the still solid terrace.
/// No paid placement, completed operation or construction phase is crossed.
pub(super) fn advance_frontier(job: &mut BuildingJob, npc: &Creature, habitat: &Habitat) -> bool {
    let start = job.next_step;
    let pending = &job.steps[start..];
    if !pending.first().is_some_and(excavation) {
        return false;
    }
    let candidate = pending
        .iter()
        .take_while(|step| excavation(step))
        .enumerate()
        .skip(1)
        .find(|(_, step)| {
            let cell = &step.cells[0];
            habitat
                .chunks
                .block_at(cell.position)
                .is_some_and(|material| {
                    planning::preparable_ground(
                        material,
                        habitat.journal.block(cell.position),
                        cell.position[1],
                        job.origin[1],
                    )
                })
                && movement::clear_for_cells(npc, &step.cells)
                && movement::reachable(habitat, npc, cell.position)
        })
        .map(|(offset, _)| start + offset);
    if let Some(index) = candidate {
        job.steps.swap(start, index);
        true
    } else {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn saved_excavation_resumes_top_down_without_replaying_completed_or_paid_work() {
        let cell = |p, material| BuildStep {
            phase: BuildPhase::Foundation,
            cells: vec![BuildCell {
                position: p,
                material,
                facing: 0,
                peer: None,
                secondary: false,
            }],
            costs: if material == 0 {
                vec![]
            } else {
                vec![terra_core::ItemStack {
                    item: ItemKind::Block(16),
                    count: 1,
                }]
            },
        };
        let steps = vec![
            cell([0, 12, 0], 0),
            cell([0, 12, 1], 0),
            cell([0, 13, 0], 0),
            cell([0, 13, 1], 0),
            cell([0, 11, 0], 16),
            cell([0, 11, 1], 0),
        ];
        let mut job = BuildingJob {
            id: 1,
            kingdom: KingdomId(1),
            owner: PlayerUuid::from_u128(1),
            origin: [0, 12, 0],
            facing: 0,
            template: HOUSE_TEMPLATE.into(),
            workers: vec![],
            steps: steps.clone(),
            next_step: 1,
            work_seconds: 12.0,
            status: BuildStatus::Travelling,
            enabled: true,
            revision: 5,
            free_resources: true,
            free_from_step: 1,
        };
        assert!(resume(&mut job));
        assert_eq!(job.steps[0], steps[0]);
        assert_eq!(
            job.steps[1..4],
            [steps[2].clone(), steps[3].clone(), steps[1].clone()]
        );
        assert_eq!(job.steps[4..], steps[4..]);
        assert_eq!(job.next_step, 1);
        assert_eq!(job.free_from_step, 1);
        assert_eq!(job.work_seconds, 12.0);
        assert!(!resume(&mut job), "migration must be idempotent");
        assert!(job.valid());
    }
}
