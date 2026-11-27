use super::resources::*;
use crate::{SectionGenerator, edits::EditJournal};
use bevy::{
    prelude::*,
    tasks::{AsyncComputeTaskPool, futures_lite::future},
};
use std::{collections::BTreeSet, sync::Arc};
use terra_voxel::SectionPos;

const BATCH_SIZE: usize = 16;

async fn generate_sections<T, R>(requests: Vec<T>, mut generate: impl FnMut(T) -> R) -> Vec<R> {
    let mut sections = Vec::with_capacity(requests.len());
    for request in requests {
        // Generation is CPU-bound. Release the executor between sections (and
        // after column preparation) so edits/render work can actually run even
        // when every physical AsyncCompute worker owns a streaming column.
        future::yield_now().await;
        sections.push(generate(request));
    }
    sections
}

pub(super) fn dispatch_jobs(
    settings: Res<StreamingSettings>,
    terrain: Res<WorldTerrain>,
    journal: Res<EditJournal>,
    mut chunks: ResMut<ChunkStore>,
    mut pending: ResMut<PendingSections>,
    focus: Option<Res<ChunkFocus>>,
) {
    let _scope = terra_core::profiling::ScopeTimer::new("streaming.dispatch");
    if chunks.dirty.is_empty() && chunks.queue.is_empty() {
        return;
    }
    // Dispatch only consumes worker bookkeeping; publication changes the world.
    let chunks = chunks.bypass_change_detection();
    // An old column task must not keep a newly edited loaded section captive.
    // Requeue its other requests and replace it with the newest snapshot.
    let stale: Vec<_> = pending
        .0
        .iter()
        .filter_map(|(&key, job)| {
            job.positions
                .iter()
                .any(|p| chunks.dirty.contains(p) && chunks.loaded.contains_key(p))
                .then_some(key)
        })
        .collect();
    for key in stale {
        if let Some(job) = pending.0.remove(&key) {
            for position in job.positions {
                if !chunks.is_wanted(position) {
                    continue;
                }
                if chunks.loaded.contains_key(&position) {
                    chunks.dirty.insert(position);
                } else {
                    chunks.queue.push_back(position);
                }
            }
        }
    }
    loop {
        let urgent = chunks
            .dirty
            .iter()
            .copied()
            .filter(|p| chunks.loaded.contains_key(p) && !pending.contains(p))
            .min_by_key(|p| {
                let point = focus.as_ref().map_or([0.0; 3], |f| f.0);
                let distance = [p.x, p.y, p.z]
                    .into_iter()
                    .enumerate()
                    .map(|(axis, v)| (v as f64 * 16.0 + 8.0 - point[axis]).powi(2))
                    .sum::<f64>();
                (distance as u64, p.y, p.z, p.x)
            });
        // One bounded edit slot remains available even when all ordinary column
        // workers are busy. It publishes one section, never a serial column batch.
        if pending.0.len() >= settings.max_jobs + usize::from(urgent.is_some()) {
            break;
        }
        if settings.meshes
            && let Some(position) = urgent
            && let Some(column) = chunks.columns.get(&[position.x, position.z]).cloned()
        {
            chunks.dirty.remove(&position);
            let loaded = &chunks.loaded[&position];
            let data = loaded.data.clone();
            let origin = match position.origin() {
                Ok(origin) => origin,
                Err(_) => continue,
            };
            let mut known = Vec::with_capacity(terra_voxel::meshing::PADDED_VOLUME);
            for z in -1..=16 {
                for y in -1..=16 {
                    for x in -1..=16 {
                        known.push(chunks.block_at([origin[0] + x, origin[1] + y, origin[2] + z]));
                    }
                }
            }
            let revision = journal.revision(position);
            let epoch = journal.mesh_epoch(position);
            let water_epoch = journal.water_mesh_epoch(position);
            let edits = journal.snapshot(position);
            let generator = SectionGenerator::for_streaming(terrain.0.clone());
            let task = AsyncComputeTaskPool::get().spawn(async move {
                let _scope = terra_core::profiling::ScopeTimer::new("streaming.remesh_loaded");
                let result = edits.map_err(|e| e.to_string()).and_then(|edits| {
                    generator
                        .remesh_loaded(&data, revision, &edits, &column, &known)
                        .map_err(|e| e.to_string())
                });
                ColumnOutput {
                    coordinate: [position.x, position.z],
                    remesh: true,
                    prepared: None,
                    sections: vec![(position, epoch, water_epoch, result)],
                }
            });
            pending.0.insert(
                position,
                SectionJob {
                    positions: vec![position],
                    task,
                },
            );
            continue;
        }
        if pending.0.len() >= settings.max_jobs {
            break;
        }
        let dirty = chunks
            .dirty
            .iter()
            .copied()
            .filter(|p| !pending.contains(p))
            .min_by_key(|p| (p.y, p.z, p.x));
        let first = if let Some(p) = dirty {
            chunks.dirty.remove(&p);
            Some(p)
        } else {
            let mut next = None;
            while let Some(p) = chunks.queue.pop_front() {
                if chunks.is_wanted(p) && !chunks.loaded.contains_key(&p) && !pending.contains(&p) {
                    next = Some(p);
                    break;
                }
            }
            next
        };
        let Some(first) = first else { break };
        let coordinate = [first.x, first.z];
        let mut positions = vec![first];
        let siblings: Vec<_> = chunks
            .dirty
            .iter()
            .copied()
            .filter(|p| {
                [p.x, p.z] == coordinate && !chunks.loaded.contains_key(p) && !pending.contains(p)
            })
            .take(BATCH_SIZE - 1)
            .collect();
        for p in siblings {
            chunks.dirty.remove(&p);
            positions.push(p);
        }
        let queued_siblings =
            chunks
                .queue
                .take_column(coordinate, BATCH_SIZE - positions.len(), |p| {
                    !pending.contains(&p) && !positions.contains(&p)
                });
        positions.extend(queued_siblings);
        let requests: Vec<_> = positions
            .iter()
            .map(|&p| {
                (
                    p,
                    journal.revision(p),
                    journal.mesh_epoch(p),
                    journal.water_mesh_epoch(p),
                    journal.snapshot(p),
                )
            })
            .collect();
        let generator = SectionGenerator::for_streaming(terrain.0.clone());
        let cached = chunks.columns.get(&coordinate).cloned();
        let with_mesh = settings.meshes;
        let task = AsyncComputeTaskPool::get().spawn(async move {
            let prepared = cached.map_or_else(
                || {
                    generator
                        .prepare_column(coordinate, with_mesh)
                        .map(Arc::new)
                },
                Ok,
            );
            let sections =
                generate_sections(requests, |(p, revision, epoch, water_epoch, edits)| {
                    let result = match (&prepared, edits) {
                        (Ok(column), Ok(edits)) => generator
                            .build_prepared(p, revision, &edits, column)
                            .map_err(|e| e.to_string()),
                        (Err(error), _) => Err(error.to_string()),
                        (_, Err(error)) => Err(error.to_string()),
                    };
                    (p, epoch, water_epoch, result)
                })
                .await;
            ColumnOutput {
                coordinate,
                remesh: false,
                prepared: prepared.ok(),
                sections,
            }
        });
        pending.0.insert(first, SectionJob { positions, task });
    }
}

pub(super) fn receive_jobs(
    access: Option<Res<WorldAccess>>,
    mut pending: ResMut<PendingSections>,
    mut chunks: ResMut<ChunkStore>,
    mut journal: ResMut<EditJournal>,
    mut generation: ResMut<WorldGeneration>,
    mut published: MessageWriter<SectionPublished>,
    mut columns_changed: MessageWriter<PreparedColumnChanged>,
) {
    let _scope = terra_core::profiling::ScopeTimer::new("streaming.receive");
    let authoritative = access.as_ref().is_none_or(|access| access.authoritative());
    let mut finished = Vec::new();
    for (&key, job) in &mut pending.0 {
        if let Some(output) = future::block_on(future::poll_once(&mut job.task)) {
            finished.push((key, output));
        }
    }
    let mut structure_refresh = BTreeSet::new();
    let mut fresh_publications = BTreeSet::new();
    for (key, output) in finished {
        pending.0.remove(&key);
        if output
            .sections
            .iter()
            .any(|(p, _, _, _)| chunks.is_wanted(*p))
            && let Some(prepared) = output.prepared
            && chunks
                .columns
                .get(&output.coordinate)
                .is_none_or(|previous| !Arc::ptr_eq(previous, &prepared))
        {
            // A tall room can extend beyond the ordinary player-height band.
            // Request its prepared envelope when the column arrives, including
            // while the player is standing still during the initial load.
            for position in super::interest::cave_sections_for_column(
                output.coordinate,
                &prepared,
                &chunks.cave_views,
            ) {
                if chunks.wanted.insert(position) && !pending.contains(&position) {
                    chunks.queue.push_back(position);
                }
            }
            chunks.columns.insert(output.coordinate, prepared);
            columns_changed.write(PreparedColumnChanged {
                coordinate: output.coordinate,
            });
        }
        for (position, epoch, mut water_epoch, result) in output.sections {
            if !chunks.is_wanted(position) {
                continue;
            }
            if epoch != journal.mesh_epoch(position) {
                chunks.dirty.insert(position);
                continue;
            }
            match result {
                Ok(mut section) => {
                    let _scope =
                        terra_core::profiling::ScopeTimer::new("streaming.publish_section");
                    if authoritative && !output.remesh {
                        let objects = chunks
                            .columns
                            .get(&[position.x, position.z])
                            .map(|column| column.structure_objects(position))
                            .unwrap_or_default();
                        if !objects.is_empty() {
                            let previous_fluid_epoch = journal.fluid_epoch(position);
                            match journal
                                .bypass_change_detection()
                                .initialize_structures(&objects)
                            {
                                Ok(affected) => {
                                    if !affected.is_empty()
                                        || journal.fluid_epoch(position) != previous_fluid_epoch
                                    {
                                        journal.set_changed();
                                        for neighbor in affected {
                                            if chunks.loaded.contains_key(&neighbor)
                                                && !fresh_publications.contains(&neighbor)
                                            {
                                                structure_refresh.insert(neighbor);
                                            }
                                        }
                                        water_epoch = journal.water_mesh_epoch(position);
                                    }
                                }
                                Err(error) => {
                                    tracing::error!(?position, %error, "Structure object initialization failed")
                                }
                            }
                        }
                    }
                    let blocked_fall = !output.remesh
                        && section.water.is_some()
                        && chunks
                            .columns
                            .get(&[position.x, position.z])
                            .is_some_and(|col| {
                                position.origin().is_ok_and(|origin| {
                                    crate::water::falls::interrupted(
                                        col.water_columns(),
                                        origin,
                                        &journal,
                                    )
                                })
                            });
                    if blocked_fall && journal.water_mesh_epoch(position) == 0 {
                        let _ = journal.invalidate_generated_water_sections(
                            &std::collections::BTreeSet::from([position]),
                        );
                        water_epoch = journal.water_mesh_epoch(position);
                    }
                    let has_fluids = blocked_fall
                        || journal
                            .fluid_snapshot(position)
                            .is_ok_and(|snapshot| !snapshot.is_empty())
                        || journal.snapshot(position).is_ok_and(|snapshot| {
                            let Ok(origin) = position.origin() else {
                                return false;
                            };
                            // A dry construction at an integral water plane can
                            // replace the wet cell below, across a section edge.
                            // Inspect sparse edits in the same one-cell halo as
                            // the water job, not its wider mineral clearance.
                            snapshot.into_iter().any(|(p, material)| {
                                crate::water::space::partial(material)
                                    && (0..3)
                                        .all(|i| (origin[i] - 1..=origin[i] + 16).contains(&p[i]))
                            })
                        });
                    if output.remesh
                        && let Some(live) = chunks.loaded.get_mut(&position)
                    {
                        // Fluids and props can publish while block meshing runs.
                        // A geometry-only result never owns their older snapshots.
                        if has_fluids || live.water_epoch >= water_epoch {
                            section.water = live.water.take();
                            water_epoch = live.water_epoch;
                        }
                        section.props = std::mem::take(&mut live.props);
                    }
                    if authoritative {
                        for &p in &section.fluid_wake {
                            journal.bypass_change_detection().wake_fluid_neighbors(p);
                        }
                        journal.bypass_change_detection().resume_fluids(position);
                    }
                    journal.apply_prop_removals(position, &mut section.props);
                    // A remote guardian needs collision near its body. Only player
                    // columns request every surface section below an airborne focus.
                    if let Some([low, high]) = section.surface_sections
                        && chunks.surface_views.as_ref().is_none_or(|(views, radius)| {
                            let coordinate = [position.x, position.z];
                            views.iter().any(|c| {
                                (c.x - position.x).abs() <= *radius
                                    && (c.z - position.z).abs() <= *radius
                            }) || chunks.gallery_columns.contains(&coordinate)
                                && chunks
                                    .columns
                                    .get(&coordinate)
                                    .is_some_and(|c| c.gallery_skyline)
                        })
                        && chunks
                            .surface_ranges
                            .insert([position.x, position.z], [low, high])
                            != Some([low, high])
                    {
                        // A newly discovered floor under the player should reach the
                        // workers before the far edge of a flight-sized view radius.
                        let nearby = chunks.cave_views.iter().any(|(view, _)| {
                            (position.x - view.x).abs().max((position.z - view.z).abs()) <= 2
                        });
                        for y in low..=high {
                            let surface = SectionPos::new(position.x, y, position.z);
                            if chunks.wanted.insert(surface) && !pending.contains(&surface) {
                                chunks.queue.insert(surface, nearby);
                            }
                        }
                    }
                    if !chunks.loaded.contains_key(&position) {
                        fresh_publications.insert(position);
                    }
                    chunks.loaded.insert(
                        position,
                        LoadedSection {
                            data: section.data,
                            mesh: section.mesh,
                            mesh_epoch: epoch,
                            water_epoch: if has_fluids && !output.remesh {
                                water_epoch.saturating_sub(1)
                            } else {
                                water_epoch
                            },
                            water: section.water,
                            props: section.props,
                        },
                    );
                    chunks.unknown_voxel_bases.remove(&position);
                    if authoritative {
                        super::support::loaded(&mut chunks, &journal, position);
                    }
                    if has_fluids || water_epoch != journal.water_mesh_epoch(position) {
                        chunks.water_dirty.insert(position);
                    }
                    chunks.dirty.remove(&position);
                    published.write(SectionPublished { position });
                }
                Err(error) => {
                    tracing::error!(?position,%error,"Section generation failed");
                    generation.status = GenerationStatus::Failed(error);
                }
            }
        }
    }
    // Joinery metadata changes only meshes which were already visible. Coalesce
    // their refresh after publication so fresh sibling outputs remain valid.
    if !structure_refresh.is_empty() {
        match journal
            .bypass_change_detection()
            .invalidate_structure_meshes(&structure_refresh)
        {
            Ok(()) => {
                journal.set_changed();
                chunks.dirty.extend(structure_refresh);
            }
            Err(error) => {
                tracing::error!(%error, "Structure mesh invalidation failed");
                generation.status = GenerationStatus::Failed(error.to_string());
            }
        }
    }
    generation.loaded_sections = chunks.loaded.len();
    generation.pending_jobs = pending.0.len();
    generation.wanted_sections = chunks.wanted.len();
    generation.prepared_columns = chunks.columns.len();
    if !matches!(generation.status, GenerationStatus::Failed(_)) {
        let ready = chunks.loaded.len() == chunks.wanted.len()
            && pending.0.is_empty()
            && chunks.dirty.is_empty();
        if generation.status != GenerationStatus::Ready && ready {
            tracing::debug!(
                sections = chunks.loaded.len(),
                cached_columns = chunks.columns.len(),
                "Chunk streaming ready"
            );
        }
        generation.status = if ready {
            GenerationStatus::Ready
        } else {
            GenerationStatus::Pending
        };
    }
}

#[cfg(test)]
mod structure_tests;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generation::sections::GeneratedSection;
    use std::time::{Duration, Instant};
    use terra_voxel::{BlockSection, MaterialId, global_to_section};

    #[derive(Resource, Default)]
    struct ChunkChanges(usize);

    #[test]
    fn cpu_bound_column_generation_yields_to_interactive_work_on_one_physical_worker() {
        use std::sync::{
            Arc,
            atomic::{AtomicBool, AtomicUsize, Ordering},
            mpsc,
        };
        let pool = bevy::tasks::TaskPoolBuilder::new().num_threads(1).build();
        let completed = Arc::new(AtomicUsize::new(0));
        let queued = Arc::new(AtomicBool::new(false));
        let (started, receiver) = mpsc::sync_channel(1);
        let cpu_completed = completed.clone();
        let cpu_queued = queued.clone();
        let column = pool.spawn(async move {
            generate_sections((0..BATCH_SIZE).collect(), |section| {
                if section == 0 {
                    started.send(()).unwrap();
                    let deadline = Instant::now() + Duration::from_secs(5);
                    while !cpu_queued.load(Ordering::Acquire) {
                        assert!(Instant::now() < deadline);
                        std::thread::yield_now();
                    }
                }
                let finish = Instant::now() + Duration::from_millis(2);
                while Instant::now() < finish {
                    std::hint::spin_loop();
                }
                cpu_completed.fetch_add(1, Ordering::AcqRel);
                section
            })
            .await
        });
        receiver.recv_timeout(Duration::from_secs(5)).unwrap();
        let interactive = pool.spawn(async move { completed.load(Ordering::Acquire) });
        queued.store(true, Ordering::Release);
        let observed = future::block_on(interactive);
        assert!(
            observed < BATCH_SIZE,
            "interactive work waited for every CPU-bound section despite a free logical edit slot: {observed}"
        );
        assert_eq!(
            future::block_on(column),
            (0..BATCH_SIZE).collect::<Vec<_>>()
        );
    }

    #[test]
    fn loaded_remesh_publication_keeps_newer_water_and_live_props_and_applies_removals() {
        use crate::props::{PropId, PropKind, WorldProp};
        let owner = SectionPos::new(-1, 0, -1);
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .init_resource::<PendingSections>()
            .init_resource::<ChunkStore>()
            .init_resource::<EditJournal>()
            .init_resource::<WorldGeneration>()
            .add_message::<SectionPublished>()
            .add_message::<PreparedColumnChanged>()
            .insert_resource(WorldAccess::Replica { ready: true })
            .add_systems(Update, receive_jobs);
        let prop = WorldProp {
            id: PropId(81),
            kind: PropKind::Mushroom,
            position: [-3.5, 1.0, -3.5],
            rotation: 0.0,
            scale: 1.0,
            variant: 5,
            section: owner,
            removed: false,
        };
        let removed = WorldProp {
            id: PropId(82),
            ..prop
        };
        let water = crate::water::WaterPatch {
            positions: vec![[0.0, 4.75, 0.0]],
            normals: Vec::new(),
            flow: Vec::new(),
            depth_shore: Vec::new(),
            surface_effects: Vec::new(),
            wave_weights: Vec::new(),
            indices: Vec::new(),
        };
        app.world_mut()
            .resource_mut::<EditJournal>()
            .remove_prop(owner, removed.id)
            .unwrap();
        let water_epoch = app
            .world()
            .resource::<EditJournal>()
            .water_mesh_epoch(owner);
        {
            let mut chunks = app.world_mut().resource_mut::<ChunkStore>();
            chunks.wanted.insert(owner);
            chunks.loaded.insert(
                owner,
                LoadedSection {
                    data: BlockSection::empty(owner).unwrap(),
                    mesh: None,
                    mesh_epoch: 0,
                    water_epoch,
                    water: Some(water.clone()),
                    props: vec![prop, removed],
                },
            );
        }
        let epoch = app.world().resource::<EditJournal>().mesh_epoch(owner);
        let output = ColumnOutput {
            coordinate: [-1, -1],
            remesh: true,
            prepared: None,
            sections: vec![(
                owner,
                epoch,
                0,
                Ok(GeneratedSection {
                    data: BlockSection::empty(owner).unwrap(),
                    mesh: Some(Default::default()),
                    surface_sections: None,
                    water: None,
                    props: Vec::new(),
                    fluid_wake: Vec::new(),
                }),
            )],
        };
        app.world_mut().resource_mut::<PendingSections>().0.insert(
            owner,
            SectionJob {
                positions: vec![owner],
                task: AsyncComputeTaskPool::get().spawn(async move { output }),
            },
        );
        let deadline = Instant::now() + Duration::from_secs(5);
        while !app.world().resource::<PendingSections>().0.is_empty() {
            app.update();
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(1));
        }
        let section = &app.world().resource::<ChunkStore>().loaded[&owner];
        assert_eq!(
            section.water,
            Some(water),
            "a newer fluid patch cannot be rolled back by terrain meshing"
        );
        assert_eq!(section.water_epoch, water_epoch);
        assert_eq!(
            section.props,
            vec![
                prop,
                WorldProp {
                    removed: true,
                    ..removed
                }
            ],
            "live variant data is retained and journal removals still win"
        );
    }

    #[test]
    fn loaded_edit_remesh_bypasses_full_generation_workers_and_preserves_world_data() {
        use crate::props::{PropId, PropKind, WorldProp};
        for coordinate in [[0, 0], [-2, -3]] {
            let owner = SectionPos::new(coordinate[0], 10, coordinate[1]);
            let cell = owner.global_block([15, 7, 15]).unwrap();
            let terrain = crate::TerraGenerator::new(terra_core::WorldConfig {
                caves: false,
                trees: false,
                water: false,
                cave_structure_revision: 0,
                ..Default::default()
            })
            .unwrap();
            let prepared = Arc::new(
                SectionGenerator::new(terrain.clone())
                    .prepare_column(coordinate, true)
                    .unwrap(),
            );
            let mut app = App::new();
            app.add_plugins(MinimalPlugins)
                .init_resource::<PendingSections>()
                .init_resource::<ChunkStore>()
                .init_resource::<EditJournal>()
                .init_resource::<WorldGeneration>()
                .add_message::<SectionPublished>()
                .add_message::<PreparedColumnChanged>()
                .insert_resource(WorldTerrain(terrain))
                .insert_resource(ChunkFocus(cell.map(|v| v as f64 + 0.5)))
                .insert_resource(StreamingSettings {
                    radius: 2,
                    vertical_radius: 1,
                    max_jobs: 1,
                    meshes: true,
                })
                .add_systems(Update, (receive_jobs, dispatch_jobs).chain());
            let prop = WorldProp {
                id: PropId(991),
                kind: PropKind::Mushroom,
                position: owner.global_block([2, 1, 2]).unwrap().map(|v| v as f64),
                rotation: 0.0,
                scale: 1.0,
                variant: 0,
                section: owner,
                removed: false,
            };
            {
                let mut chunks = app.world_mut().resource_mut::<ChunkStore>();
                chunks.columns.insert(coordinate, prepared);
                for z in -1..=1 {
                    for y in -1..=1 {
                        for x in -1..=1 {
                            let p = owner.offset([x, y, z]).unwrap();
                            let mut data = BlockSection::empty(p).unwrap();
                            data.set_block([2, 0, 2], 11).unwrap();
                            chunks.wanted.insert(p);
                            chunks.loaded.insert(
                                p,
                                LoadedSection {
                                    data,
                                    mesh: None,
                                    mesh_epoch: 0,
                                    water_epoch: 0,
                                    water: None,
                                    props: if p == owner { vec![prop] } else { Vec::new() },
                                },
                            );
                        }
                    }
                }
            }
            let blocked = owner.offset([20, 0, 0]).unwrap();
            app.world_mut().resource_mut::<PendingSections>().0.insert(
                blocked,
                SectionJob {
                    positions: vec![blocked],
                    task: AsyncComputeTaskPool::get().spawn(std::future::pending()),
                },
            );
            app.world_mut()
                .resource_mut::<EditJournal>()
                .record(cell, 36)
                .unwrap();
            app.world_mut()
                .resource_mut::<ChunkStore>()
                .dirty
                .insert(owner);
            app.update();
            let pending = app.world().resource::<PendingSections>();
            assert_eq!(pending.0.len(), 2, "only one bounded extra edit slot");
            assert_eq!(
                pending.0[&owner].positions,
                vec![owner],
                "edits cannot wait for sixteen serial sections"
            );
            let deadline = Instant::now() + Duration::from_secs(5);
            while app.world().resource::<ChunkStore>().loaded[&owner].mesh_epoch != 1 {
                app.update();
                assert!(
                    Instant::now() < deadline,
                    "edit waited for a never-completing unrelated generation job"
                );
                assert!(app.world().resource::<PendingSections>().0.len() <= 2);
                std::thread::sleep(Duration::from_millis(1));
            }
            let chunks = app.world().resource::<ChunkStore>();
            assert_eq!(chunks.block_at(cell), Some(36));
            let section = &chunks.loaded[&owner];
            assert_eq!(
                section.data.block([2, 0, 2]).unwrap(),
                11,
                "existing authored blocks must not be regenerated"
            );
            assert_eq!(
                section.props,
                vec![prop],
                "unchanged props retain their identity and state"
            );
            assert!(
                section
                    .mesh
                    .as_ref()
                    .unwrap()
                    .faces
                    .iter()
                    .any(|face| face.material == 36)
            );
            assert!(
                app.world()
                    .resource::<PendingSections>()
                    .0
                    .contains_key(&blocked)
            );
        }
    }

    #[test]
    fn new_edit_invalidates_an_old_loaded_section_task_instead_of_waiting_for_it() {
        let owner = SectionPos::new(-1, 8, -1);
        let terrain = crate::TerraGenerator::new(terra_core::WorldConfig {
            caves: false,
            trees: false,
            water: false,
            ..Default::default()
        })
        .unwrap();
        let column = Arc::new(
            SectionGenerator::new(terrain.clone())
                .prepare_column([-1, -1], true)
                .unwrap(),
        );
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .insert_resource(WorldTerrain(terrain))
            .init_resource::<EditJournal>()
            .init_resource::<PendingSections>()
            .init_resource::<ChunkStore>()
            .insert_resource(StreamingSettings {
                radius: 1,
                vertical_radius: 1,
                max_jobs: 1,
                meshes: true,
            })
            .add_systems(Update, dispatch_jobs);
        {
            let mut chunks = app.world_mut().resource_mut::<ChunkStore>();
            chunks.columns.insert([-1, -1], column);
            chunks.wanted.insert(owner);
            chunks.loaded.insert(
                owner,
                LoadedSection {
                    data: BlockSection::empty(owner).unwrap(),
                    mesh: None,
                    mesh_epoch: 0,
                    water_epoch: 0,
                    water: None,
                    props: Vec::new(),
                },
            );
        }
        app.world_mut().resource_mut::<PendingSections>().0.insert(
            owner,
            SectionJob {
                positions: vec![owner],
                task: AsyncComputeTaskPool::get().spawn(std::future::pending()),
            },
        );
        app.world_mut()
            .resource_mut::<EditJournal>()
            .record(owner.global_block([3, 4, 5]).unwrap(), 3)
            .unwrap();
        app.world_mut()
            .resource_mut::<ChunkStore>()
            .dirty
            .insert(owner);
        app.update();
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            let output = {
                let mut pending = app.world_mut().resource_mut::<PendingSections>();
                future::block_on(future::poll_once(
                    &mut pending.0.get_mut(&owner).unwrap().task,
                ))
            };
            if let Some(output) = output {
                assert_eq!(output.sections.len(), 1);
                assert_eq!(output.sections[0].1, 1);
                assert_eq!(
                    output.sections[0]
                        .3
                        .as_ref()
                        .unwrap()
                        .data
                        .block([3, 4, 5])
                        .unwrap(),
                    3
                );
                break;
            }
            assert!(Instant::now() < deadline, "stale task was never replaced");
            std::thread::sleep(Duration::from_millis(1));
        }
    }

    #[test]
    fn arriving_tall_cave_column_requests_its_roof_and_floor_without_player_motion() {
        for cave_structure_revision in [4, 5] {
            let terrain = crate::TerraGenerator::new(terra_core::WorldConfig {
                cave_structure_revision,
                ..Default::default()
            })
            .unwrap();
            let room = (-16..0)
                .flat_map(|z| (-16..0).map(move |x| (x, z)))
                .filter_map(|(x, z)| {
                    terrain.cave_chamber_center((x * 192 + 96) as f64, (z * 192 + 96) as f64)
                })
                .map(|center| {
                    (
                        [center[0], center[2]],
                        terrain.cave_structure_at(center[0], center[2]),
                    )
                })
                .find(|(_, room)| {
                    room.active() && room.ceiling - room.floor > 48.0 && !room.open_to_sky()
                })
                .unwrap();
            let coordinate = room.0.map(|p| (p.floor() as i64).div_euclid(16));
            let eye = room.1.floor + 5.0;
            let focus = SectionPos::new(
                coordinate[0],
                (eye.floor() as i64).div_euclid(16),
                coordinate[1],
            );
            let prepared = Arc::new(
                SectionGenerator::new(terrain)
                    .prepare_column(coordinate, true)
                    .unwrap(),
            );
            let envelope = super::super::interest::cave_sections_for_column(
                coordinate,
                &prepared,
                &[(focus, 2)],
            );
            assert!(
                envelope.iter().any(|p| p.y - focus.y > 1),
                "fixture must expose the former vertical-band ceiling hole"
            );
            let bounds = prepared
                .cave_view_sections(focus.y as f64 * 16.0 + 8.0)
                .unwrap();
            assert_eq!(envelope.len(), (bounds[1] - bounds[0] + 1) as usize);
            for y in bounds[0]..=bounds[1] {
                assert!(envelope.contains(&SectionPos::new(coordinate[0], y, coordinate[1])));
            }
            assert!(
                super::super::interest::cave_sections_for_column(coordinate, &prepared, &[])
                    .is_empty(),
                "headless interest does not load visual-only room envelopes"
            );
            let mut app = App::new();
            app.add_plugins(MinimalPlugins)
                .init_resource::<PendingSections>()
                .init_resource::<ChunkStore>()
                .init_resource::<EditJournal>()
                .init_resource::<WorldGeneration>()
                .add_message::<SectionPublished>()
                .add_message::<PreparedColumnChanged>()
                .add_systems(Update, receive_jobs);
            {
                let mut chunks = app.world_mut().resource_mut::<ChunkStore>();
                chunks.wanted.insert(focus);
                chunks.cave_views = vec![(focus, 2)];
            }
            let generated = GeneratedSection {
                data: BlockSection::empty(focus).unwrap(),
                mesh: None,
                surface_sections: None,
                water: None,
                props: Vec::new(),
                fluid_wake: Vec::new(),
            };
            app.world_mut().resource_mut::<PendingSections>().0.insert(
                focus,
                SectionJob {
                    positions: vec![focus],
                    task: AsyncComputeTaskPool::get().spawn(async move {
                        ColumnOutput {
                            coordinate,
                            remesh: false,
                            prepared: Some(prepared),
                            sections: vec![(focus, 0, 0, Ok(generated))],
                        }
                    }),
                },
            );
            let deadline = Instant::now() + Duration::from_secs(5);
            while !app.world().resource::<PendingSections>().0.is_empty() {
                app.update();
                assert!(Instant::now() < deadline);
                std::thread::yield_now();
            }
            let chunks = app.world().resource::<ChunkStore>();
            for position in envelope {
                assert!(chunks.wanted.contains(&position));
            }
            assert!(!chunks.queue.is_empty());
        }
    }

    #[test]
    fn reloading_a_naturally_waterlogged_partial_block_requests_a_geometry_patch() {
        for (position, material, owner, needs_patch) in [
            ([-1, -1, -1], 82, SectionPos::new(-1, -1, -1), true),
            ([-8, 16, -8], 163, SectionPos::new(-1, 0, -1), true),
            ([-8, -16, -8], 163, SectionPos::new(-1, -2, -1), true),
            ([-8, 17, -8], 163, SectionPos::new(-1, 0, -1), false),
        ] {
            let mut app = App::new();
            app.add_plugins(MinimalPlugins)
                .init_resource::<PendingSections>()
                .init_resource::<ChunkStore>()
                .init_resource::<EditJournal>()
                .init_resource::<WorldGeneration>()
                .insert_resource(WorldAccess::Replica { ready: true })
                .add_message::<SectionPublished>()
                .add_message::<PreparedColumnChanged>()
                .add_systems(Update, receive_jobs);
            let (construction_owner, local) = global_to_section(position);
            let mut saved = EditJournal::default();
            saved.record(position, material).unwrap();
            for section in saved.changed_sections() {
                app.world_mut()
                    .resource_mut::<EditJournal>()
                    .import_delta(saved.section_delta(section))
                    .unwrap();
            }
            assert!(
                app.world()
                    .resource::<EditJournal>()
                    .fluid_snapshot(owner)
                    .unwrap()
                    .is_empty()
            );
            let mut data = BlockSection::empty(owner).unwrap();
            if owner == construction_owner {
                data.set_block(local, material).unwrap();
            }
            let journal = app.world().resource::<EditJournal>();
            let epoch = journal.mesh_epoch(owner);
            let water_epoch = journal.water_mesh_epoch(owner);
            let generated = GeneratedSection {
                data,
                mesh: None,
                surface_sections: None,
                water: None,
                props: Vec::new(),
                fluid_wake: Vec::new(),
            };
            app.world_mut()
                .resource_mut::<ChunkStore>()
                .wanted
                .insert(owner);
            app.world_mut().resource_mut::<PendingSections>().0.insert(
                owner,
                SectionJob {
                    positions: vec![owner],
                    task: AsyncComputeTaskPool::get().spawn(async move {
                        ColumnOutput {
                            coordinate: [owner.x, owner.z],
                            remesh: false,
                            prepared: None,
                            sections: vec![(owner, epoch, water_epoch, Ok(generated))],
                        }
                    }),
                },
            );
            let deadline = Instant::now() + Duration::from_secs(5);
            while !app.world().resource::<PendingSections>().0.is_empty() {
                app.update();
                assert!(Instant::now() < deadline);
                std::thread::sleep(Duration::from_millis(1));
            }
            assert_eq!(
                app.world()
                    .resource::<ChunkStore>()
                    .water_dirty
                    .contains(&owner),
                needs_patch,
                "construction at {position:?} must affect only the one-cell water halo"
            );
            assert_eq!(
                app.world().resource::<ChunkStore>().loaded[&owner].water_epoch,
                if needs_patch {
                    water_epoch - 1
                } else {
                    water_epoch
                }
            );
        }
    }

    #[test]
    fn rule_changes_keep_cached_and_inflight_columns_valid_during_streaming_and_edits() {
        for coordinate in [[8, 37], [-9, -38]] {
            let mut app = App::new();
            app.add_plugins(MinimalPlugins)
                .init_resource::<PendingSections>()
                .init_resource::<ChunkStore>()
                .init_resource::<EditJournal>()
                .init_resource::<WorldGeneration>()
                .add_message::<SectionPublished>()
                .add_message::<PreparedColumnChanged>()
                .insert_resource(StreamingSettings {
                    radius: 1,
                    vertical_radius: 1,
                    max_jobs: 2,
                    meshes: false,
                })
                .insert_resource(WorldTerrain(
                    crate::TerraGenerator::new(Default::default()).unwrap(),
                ))
                .add_systems(Update, (receive_jobs, dispatch_jobs).chain());
            let prepared = Arc::new(
                SectionGenerator::new(app.world().resource::<WorldTerrain>().0.clone())
                    .prepare_column(coordinate, false)
                    .unwrap(),
            );
            app.world_mut()
                .resource_mut::<ChunkStore>()
                .columns
                .insert(coordinate, prepared.clone());
            let mut rules = terra_core::WorldRuleSettings::from_world(
                app.world().resource::<WorldTerrain>().0.config(),
            );
            for (step, y) in [-2, 4, -2].into_iter().enumerate() {
                let p = SectionPos::new(coordinate[0], y, coordinate[1]);
                let cell = [p.x * 16 + 15, p.y * 16 + 15, p.z * 16 + 15];
                let edit = (step == 2).then_some(11);
                if let Some(material) = edit {
                    app.world_mut()
                        .resource_mut::<EditJournal>()
                        .record(cell, material)
                        .unwrap();
                    app.world_mut().resource_mut::<ChunkStore>().dirty.insert(p);
                } else {
                    let mut chunks = app.world_mut().resource_mut::<ChunkStore>();
                    chunks.wanted.insert(p);
                    chunks.queue.push_back(p);
                }
                rules.game_rules.game_time_speed = 1.0 + step as f64;
                app.world_mut()
                    .resource_mut::<WorldTerrain>()
                    .0
                    .set_world_rules(rules);
                app.update();
                // A second policy change while workers own the previous snapshot.
                rules.game_rules.sleep_percentage = 50.0;
                app.world_mut()
                    .resource_mut::<WorldTerrain>()
                    .0
                    .set_world_rules(rules);
                let deadline = Instant::now() + Duration::from_secs(5);
                loop {
                    app.update();
                    assert!(!matches!(
                        app.world().resource::<WorldGeneration>().status,
                        GenerationStatus::Failed(_)
                    ));
                    let chunks = app.world().resource::<ChunkStore>();
                    if app.world().resource::<PendingSections>().0.is_empty()
                        && chunks.dirty.is_empty()
                        && chunks.loaded.contains_key(&p)
                    {
                        break;
                    }
                    assert!(Instant::now() < deadline, "section did not finish");
                    std::thread::sleep(Duration::from_millis(1));
                }
                let chunks = app.world().resource::<ChunkStore>();
                assert!(
                    Arc::ptr_eq(&chunks.prepared_column(coordinate).unwrap(), &prepared),
                    "rules must not regenerate terrain"
                );
                if let Some(material) = edit {
                    assert_eq!(chunks.block_at(cell), Some(material));
                }
            }
        }
    }

    #[test]
    fn replica_reset_discards_streaming_messages_from_the_previous_session() {
        let mut app = App::new();
        app.add_message::<SectionPublished>()
            .add_message::<SectionRemoved>()
            .add_message::<PreparedColumnChanged>();
        app.world_mut()
            .resource_mut::<Messages<SectionPublished>>()
            .write(SectionPublished {
                position: SectionPos::default(),
            });
        app.world_mut()
            .resource_mut::<Messages<SectionRemoved>>()
            .write(SectionRemoved {
                position: SectionPos::default(),
            });
        app.world_mut()
            .resource_mut::<Messages<PreparedColumnChanged>>()
            .write(PreparedColumnChanged { coordinate: [0, 0] });
        reset_replica(app.world_mut());
        assert!(
            app.world()
                .resource::<Messages<SectionPublished>>()
                .is_empty()
        );
        assert!(
            app.world()
                .resource::<Messages<SectionRemoved>>()
                .is_empty()
        );
        assert!(
            app.world()
                .resource::<Messages<PreparedColumnChanged>>()
                .is_empty()
        );
    }

    #[test]
    fn publication_messages_cover_installed_outputs_and_expire_without_consumers() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .init_resource::<PendingSections>()
            .init_resource::<ChunkStore>()
            .init_resource::<EditJournal>()
            .init_resource::<WorldGeneration>()
            .add_message::<SectionPublished>()
            .add_message::<PreparedColumnChanged>()
            .insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
                Duration::from_secs_f64(1.0 / 60.0),
            ))
            .add_systems(Update, receive_jobs);
        let positions = std::array::from_fn::<_, 5, _>(|y| SectionPos::new(-1, y as i64, -1));
        let [fresh, replacement, stale, unwanted, failed] = positions;
        let prepared = Arc::new(
            SectionGenerator::new(crate::TerraGenerator::new(Default::default()).unwrap())
                .prepare_column([-1, -1], false)
                .unwrap(),
        );
        {
            let mut chunks = app.world_mut().resource_mut::<ChunkStore>();
            chunks.wanted.extend([fresh, replacement, stale, failed]);
            for position in positions {
                chunks.unknown_voxel_bases.insert(position, [[0; 3]].into());
            }
            chunks.loaded.insert(
                replacement,
                LoadedSection {
                    data: BlockSection::empty(replacement).unwrap(),
                    mesh: None,
                    mesh_epoch: 99,
                    water_epoch: 99,
                    water: None,
                    props: Vec::new(),
                },
            );
        }
        let generated = |position| {
            Ok(GeneratedSection {
                data: BlockSection::empty(position).unwrap(),
                mesh: Some(Default::default()),
                surface_sections: None,
                water: None,
                props: Vec::new(),
                fluid_wake: Vec::new(),
            })
        };
        let output = ColumnOutput {
            coordinate: [-1, -1],
            remesh: false,
            prepared: Some(prepared.clone()),
            sections: vec![
                (fresh, 0, 0, generated(fresh)),
                (replacement, 0, 0, generated(replacement)),
                (stale, 1, 0, generated(stale)),
                (unwanted, 0, 0, generated(unwanted)),
                (failed, 0, 0, Err("expected fixture failure".into())),
            ],
        };
        app.world_mut().resource_mut::<PendingSections>().0.insert(
            fresh,
            SectionJob {
                positions: positions.into(),
                task: AsyncComputeTaskPool::get().spawn(async move { output }),
            },
        );
        let deadline = Instant::now() + Duration::from_secs(5);
        while !app.world().resource::<PendingSections>().0.is_empty() {
            app.update();
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(1));
        }
        let messages = app.world().resource::<Messages<SectionPublished>>();
        let mut cursor = bevy::ecs::message::MessageCursor::<SectionPublished>::default();
        let published: Vec<_> = cursor.read(messages).map(|event| event.position).collect();
        assert_eq!(published, [fresh, replacement]);
        let chunks = app.world().resource::<ChunkStore>();
        assert!(Arc::ptr_eq(
            &chunks.prepared_column([-1, -1]).unwrap(),
            &prepared
        ));
        for position in published {
            let section = &chunks.loaded[&position];
            assert!(section.mesh.is_some());
            assert_eq!(section.mesh_epoch, 0);
        }
        assert_eq!(chunks.loaded.len(), 2);
        assert!(chunks.dirty.contains(&stale));
        assert!(!chunks.voxel_base_pending(fresh));
        assert!(!chunks.voxel_base_pending(replacement));
        for position in [stale, unwanted, failed] {
            assert!(
                chunks.voxel_base_pending(position),
                "unpublished base {position:?}"
            );
        }
        let mut cursor = bevy::ecs::message::MessageCursor::<PreparedColumnChanged>::default();
        assert_eq!(
            cursor
                .read(app.world().resource::<Messages<PreparedColumnChanged>>())
                .map(|event| event.coordinate)
                .collect::<Vec<_>>(),
            [[-1, -1]]
        );
        for _ in 0..3 {
            app.update();
        }
        assert!(
            app.world()
                .resource::<Messages<SectionPublished>>()
                .is_empty()
        );
        assert!(
            app.world()
                .resource::<Messages<PreparedColumnChanged>>()
                .is_empty()
        );
    }

    #[test]
    fn dispatch_bookkeeping_invalidates_render_consumers_only_when_a_section_is_published() {
        for dirty in [false, true] {
            let mut app = App::new();
            app.add_plugins(MinimalPlugins)
                .init_resource::<PendingSections>()
                .init_resource::<ChunkStore>()
                .init_resource::<EditJournal>()
                .init_resource::<WorldGeneration>()
                .init_resource::<ChunkChanges>()
                .add_message::<SectionPublished>()
                .add_message::<PreparedColumnChanged>()
                .insert_resource(StreamingSettings {
                    radius: 0,
                    vertical_radius: 0,
                    max_jobs: 0,
                    meshes: false,
                })
                .insert_resource(WorldTerrain(
                    crate::TerraGenerator::new(Default::default()).unwrap(),
                ))
                .add_systems(
                    Update,
                    (
                        receive_jobs,
                        dispatch_jobs,
                        |chunks: Res<ChunkStore>, mut changes: ResMut<ChunkChanges>| {
                            changes.0 += usize::from(chunks.is_changed());
                        },
                    )
                        .chain(),
                );
            let owner = SectionPos::new(-1, 100, -1);
            {
                let mut chunks = app.world_mut().resource_mut::<ChunkStore>();
                chunks.wanted.insert(owner);
                if dirty {
                    chunks.loaded.insert(
                        owner,
                        LoadedSection {
                            data: BlockSection::empty(owner).unwrap(),
                            mesh: None,
                            mesh_epoch: 0,
                            water_epoch: u64::MAX,
                            water: None,
                            props: Vec::new(),
                        },
                    );
                    chunks.dirty.insert(owner);
                } else {
                    chunks.queue.push_back(owner);
                }
            }
            app.update();
            app.update();
            assert_eq!(app.world().resource::<ChunkChanges>().0, 1);
            app.world_mut().resource_mut::<StreamingSettings>().max_jobs = 1;
            app.update();
            assert_eq!(app.world().resource::<PendingSections>().0.len(), 1);
            assert_eq!(app.world().resource::<ChunkChanges>().0, 1);
            let chunks = app.world().resource::<ChunkStore>();
            assert!(chunks.queue.is_empty());
            assert!(chunks.dirty.is_empty());
            let deadline = Instant::now() + Duration::from_secs(5);
            while !app.world().resource::<PendingSections>().0.is_empty() {
                app.update();
                assert!(Instant::now() < deadline);
                std::thread::sleep(Duration::from_millis(1));
            }
            assert_eq!(app.world().resource::<ChunkChanges>().0, 2);
            assert_eq!(
                app.world().resource::<ChunkStore>().loaded[&owner].water_epoch,
                0
            );
            for _ in 0..3 {
                app.update();
            }
            assert_eq!(app.world().resource::<ChunkChanges>().0, 2);
        }
    }

    #[test]
    fn repeated_replica_section_reload_never_schedules_authoritative_fluid_or_support_work() {
        for access in [WorldAccess::Replica { ready: true }, WorldAccess::Server] {
            let mut app = App::new();
            app.add_plugins(MinimalPlugins)
                .insert_resource(access)
                .init_resource::<PendingSections>()
                .init_resource::<ChunkStore>()
                .init_resource::<EditJournal>()
                .init_resource::<WorldGeneration>()
                .add_message::<SectionPublished>()
                .add_message::<PreparedColumnChanged>()
                .add_systems(Update, receive_jobs);
            let position = [-1, -1, -1];
            let (owner, local) = global_to_section(position);
            let mut host = EditJournal::default();
            host.record(position, MaterialId::FluidWater as u16)
                .unwrap();
            // Reload reconciles actual support losses, not every mineral in
            // a column touched by an unrelated horizontal water edit.
            host.record([-2, -2, -1], MaterialId::Air as u16).unwrap();
            app.world_mut()
                .resource_mut::<EditJournal>()
                .import_replica_delta(host.section_delta(owner))
                .unwrap();
            app.world_mut()
                .resource_mut::<ChunkStore>()
                .wanted
                .insert(owner);
            for cycle in 0..100 {
                app.world_mut().resource_mut::<ChunkStore>().loaded.clear();
                let journal = app.world().resource::<EditJournal>();
                let epoch = journal.mesh_epoch(owner);
                let water_epoch = journal.water_mesh_epoch(owner);
                let mut data = BlockSection::empty(owner).unwrap();
                data.set_block(local, MaterialId::FluidWater as u16)
                    .unwrap();
                data.set_block([14, 15, 15], MaterialId::SaltCrystal as u16)
                    .unwrap();
                let generated = GeneratedSection {
                    data,
                    mesh: None,
                    surface_sections: None,
                    water: None,
                    props: Vec::new(),
                    fluid_wake: vec![position],
                };
                app.world_mut().resource_mut::<PendingSections>().0.insert(
                    owner,
                    SectionJob {
                        positions: vec![owner],
                        task: AsyncComputeTaskPool::get().spawn(async move {
                            ColumnOutput {
                                coordinate: [owner.x, owner.z],
                                remesh: false,
                                prepared: None,
                                sections: vec![(owner, epoch, water_epoch, Ok(generated))],
                            }
                        }),
                    },
                );
                let deadline = Instant::now() + Duration::from_secs(5);
                while !app.world().resource::<PendingSections>().0.is_empty() {
                    app.update();
                    assert!(Instant::now() < deadline);
                    std::thread::sleep(Duration::from_millis(1));
                }
                let journal = app.world().resource::<EditJournal>();
                let work = journal.fluid_work_counts();
                assert_eq!(
                    work.active > 0,
                    access.authoritative(),
                    "{access:?} cycle {cycle}"
                );
                assert_eq!(work.queued, work.active);
                assert_eq!(work.recorded_cells, 1);
                assert_eq!(work.sources, 1);
                assert_eq!(journal.section_delta(owner), host.section_delta(owner));
                assert_eq!(app.world().resource::<ChunkStore>().loaded.len(), 1);
                assert_eq!(
                    app.world().resource::<ChunkStore>().support_queue.len(),
                    usize::from(access.authoritative()),
                    "{access:?} cycle {cycle}"
                );
                assert_eq!(
                    app.world().resource::<ChunkStore>().loaded[&owner].mesh_epoch,
                    epoch
                );
                app.world_mut()
                    .resource_mut::<EditJournal>()
                    .take_fluid_work(usize::MAX);
                app.world_mut()
                    .resource_mut::<ChunkStore>()
                    .support_queue
                    .clear();
            }
        }
    }
}
