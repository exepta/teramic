//! Re-clear only soil in cells this saved job already excavated itself.
use super::*;

pub(super) struct Progress {
    pub position: [i64; 3],
    pub seconds: f64,
}

/// A later completed placement supersedes excavation ownership at that cell.
fn owns_excavation(job: &BuildingJob, position: [i64; 3]) -> bool {
    job.steps[..job.next_step]
        .iter()
        .rev()
        .find(|step| step.cells.iter().any(|cell| cell.position == position))
        .is_some_and(|step| {
            step.phase == BuildPhase::Foundation
                && step.costs.is_empty()
                && step.cells.len() == 1
                && step.cells[0].material == 0
                && step.cells[0].peer.is_none()
                && !step.cells[0].secondary
        })
}

/// This is deliberately narrower than accepting saved soil throughout a site.
/// A planned, unfinished nonzero placement must occupy this exact excavated cell.
pub(super) fn allowed(
    chunks: &ChunkStore,
    journal: &EditJournal,
    job: &BuildingJob,
    position: [i64; 3],
) -> bool {
    let Some(material @ (1 | 2)) = chunks.block_at(position) else {
        return false;
    };
    owns_excavation(job, position)
        && job.steps.get(job.next_step).is_some_and(|step| {
            step.phase != BuildPhase::Cleanup
                && step
                    .cells
                    .iter()
                    .any(|cell| cell.position == position && cell.material != 0)
        })
        && journal
            .block(position)
            .is_none_or(|saved| saved == material)
        && journal.container_at(position).is_none()
        && journal.object_state(position).is_none()
        && journal
            .fluid(position)
            .is_none_or(|fluid| fluid.amount == 0)
        && build_access(chunks, journal, job.kingdom, job.owner, position)
        && crate::kingdom_access::can_use(
            &journal.gameplay().kingdoms,
            chunks,
            job.owner,
            KingdomPermission::Break,
            position,
        )
}

pub(super) fn step(
    chunks: &ChunkStore,
    journal: &EditJournal,
    job: &BuildingJob,
) -> Option<BuildStep> {
    let position = job
        .steps
        .get(job.next_step)?
        .cells
        .iter()
        .find(|cell| allowed(chunks, journal, job, cell.position))?
        .position;
    Some(BuildStep {
        phase: BuildPhase::Foundation,
        cells: vec![BuildCell {
            position,
            material: 0,
            facing: 0,
            peer: None,
            secondary: false,
        }],
        costs: Vec::new(),
    })
}
