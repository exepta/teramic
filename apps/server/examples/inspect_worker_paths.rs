//! Replays saved construction in memory. The source world is never written.
use std::path::PathBuf;
use terra_core::{WorldId, block_properties::BlockPropertiesCatalog, kingdom::ClaimKind};
use terra_world::{building_jobs::BuildRuntime, creatures::Habitat};

fn main() -> Result<(), String> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if !(2..=4).contains(&args.len()) {
        return Err("Usage: inspect_worker_paths <world directory> <decimal world ID> [seconds] [job ID to finish]".into());
    }
    let path = PathBuf::from(&args[0]);
    let id = WorldId(args[1].parse().map_err(|_| "Invalid world ID")?);
    let seconds: u32 = args
        .get(2)
        .map_or(Ok(60), |v| v.parse())
        .map_err(|_| "Invalid duration")?;
    if seconds > 10_800 {
        return Err("Duration exceeds three hours".into());
    }
    let required: Option<u64> = args
        .get(3)
        .map(|v| v.parse())
        .transpose()
        .map_err(|_| "Invalid job ID")?;
    let entry = terra_storage::WorldLibrary {
        root: path
            .parent()
            .ok_or("World directory needs a parent")?
            .into(),
    }
    .load(id)?;
    let mut journal = terra_storage::load_edits(&path, id)?;
    let terrain =
        terra_world::TerraGenerator::new(entry.manifest.world).map_err(|e| e.to_string())?;
    let generator = terra_world::SectionGenerator::new(terrain.clone());
    let mut chunks = terra_world::streaming::ChunkStore::default();
    let mut section = journal.changed_sections().next().unwrap_or_default();
    for claim in journal
        .gameplay()
        .kingdoms
        .claims
        .iter()
        .filter(|c| c.kind == ClaimKind::ProtectedBuilding)
    {
        let column = generator
            .prepare_column(claim.chunk, false)
            .map_err(|e| e.to_string())?;
        for y in 0..=3 {
            section.x = claim.chunk[0];
            section.y = y;
            section.z = claim.chunk[1];
            let generated = generator
                .build_prepared(
                    section,
                    0,
                    &journal.snapshot(section).map_err(|e| e.to_string())?,
                    &column,
                )
                .map_err(|e| e.to_string())?;
            chunks.loaded.insert(
                section,
                terra_world::streaming::LoadedSection {
                    data: generated.data,
                    mesh: None,
                    mesh_epoch: 0,
                    water_epoch: 0,
                    water: None,
                    props: generated.props,
                },
            );
        }
    }
    let mut runtime = BuildRuntime::default();
    runtime.set_creative_owners(
        journal
            .gameplay()
            .buildings
            .jobs
            .values()
            .filter(|j| j.free_resources)
            .map(|j| j.owner),
    );
    if required.is_some_and(|id| !journal.gameplay().buildings.jobs.contains_key(&id)) {
        return Err("Required job does not exist".into());
    }
    let properties = BlockPropertiesCatalog::default();
    for n in 0..=seconds * 10 {
        if n % 100 == 0 || n == seconds * 10 {
            for job in journal.gameplay().buildings.jobs.values() {
                println!(
                    "t={:.1} job={} origin={:?} state={:?} step={}/{} target={:?}",
                    n as f32 / 10.0,
                    job.id,
                    job.origin,
                    job.status,
                    job.next_step,
                    job.steps.len(),
                    job.steps.get(job.next_step)
                );
                if job.status
                    == terra_core::building_jobs::BuildStatus::Waiting(
                        terra_core::building_jobs::BuildWait::Blocked,
                    )
                    && let Some(step) = job.steps.get(job.next_step)
                {
                    for cell in &step.cells {
                        println!(
                            "blocked-cell={:?} old={:?} saved={:?} object={:?}",
                            cell.position,
                            chunks.block_at(cell.position),
                            journal.block(cell.position),
                            journal.object_state(cell.position)
                        );
                        for (index, previous) in job.steps[..job.next_step]
                            .iter()
                            .enumerate()
                            .filter(|(_, s)| s.cells.iter().any(|c| c.position == cell.position))
                        {
                            println!("earlier-step={index} {previous:?}");
                        }
                        for npc in journal.gameplay().creatures.animals.values().filter(|n| {
                            (n.position[0] - cell.position[0] as f64)
                                .hypot(n.position[2] - cell.position[2] as f64)
                                < 2.0
                        }) {
                            println!(
                                "nearby-actor={} kind={:?} pos={:?}",
                                npc.id, npc.kind, npc.position
                            );
                        }
                    }
                }
                for id in job.workers.iter().filter(|_| job.active()) {
                    if let Some(npc) = journal.gameplay().creatures.animals.get(id) {
                        let h = Habitat {
                            terrain: &terrain,
                            chunks: &chunks,
                            journal: &journal,
                        };
                        println!(
                            "npc={id} pos={:?} yaw={} velocity={:?} behavior={:?} age={} ground={:?}",
                            npc.position,
                            npc.yaw,
                            npc.velocity,
                            npc.behavior,
                            npc.age,
                            h.ground_sized(npc.position, npc.size(), f64::from(npc.yaw))
                        );
                        println!("route={:?}", runtime.inspect_route(*id));
                    }
                }
            }
        }
        if n == seconds * 10 {
            break;
        }
        terra_world::building_jobs::tick(
            &mut chunks,
            &mut journal,
            &terrain,
            &properties,
            0.1,
            &mut runtime,
        );
        for request in runtime.drain_door_requests() {
            if let Some(state) = journal.object_state(request.position) {
                for p in std::iter::once(request.position).chain(state.peer) {
                    if let Some(old) = journal.object_state(p) {
                        terra_world::streaming::commit_object_state(
                            &mut chunks,
                            &mut journal,
                            p,
                            Some(terra_world::edits::BlockObjectState {
                                facing: old.facing % 4 + 4,
                                ..old
                            }),
                        )
                        .map_err(|e| e.to_string())?;
                    }
                }
            }
        }
    }
    if !journal.gameplay().valid() {
        return Err("Replay produced invalid persistent gameplay state".into());
    }
    if let Some(id) = required {
        let job = &journal.gameplay().buildings.jobs[&id];
        if job.status != terra_core::building_jobs::BuildStatus::Complete {
            return Err(format!(
                "Job {id} did not finish: {:?}, {}/{}",
                job.status,
                job.next_step,
                job.steps.len()
            ));
        }
        println!("PASS job={id} completed in memory; source world untouched");
    }
    Ok(())
}
