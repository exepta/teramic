use super::resources::*;
use crate::{SectionGenerator, edits::EditJournal};
use bevy::{
    prelude::*,
    tasks::{AsyncComputeTaskPool, futures_lite::future},
};
use std::sync::Arc;
use terra_voxel::SectionPos;

const BATCH_SIZE: usize = 16;

pub(super) fn dispatch_jobs(
    settings: Res<StreamingSettings>,
    terrain: Res<WorldTerrain>,
    journal: Res<EditJournal>,
    mut chunks: ResMut<ChunkStore>,
    mut pending: ResMut<PendingSections>,
) {
    let _scope = terra_core::profiling::ScopeTimer::new("streaming.dispatch");
    if chunks.dirty.is_empty() && chunks.queue.is_empty() {
        return;
    }
    // Dispatch only consumes worker bookkeeping; publication changes the world.
    let chunks = chunks.bypass_change_detection();
    while pending.0.len() < settings.max_jobs {
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
            .filter(|p| [p.x, p.z] == coordinate && !pending.contains(p))
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
        let generator = SectionGenerator::new(terrain.0.clone());
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
            let mut sections = Vec::with_capacity(requests.len());
            for (p, revision, epoch, water_epoch, edits) in requests {
                let result = match (&prepared, edits) {
                    (Ok(column), Ok(edits)) => generator
                        .build_prepared(p, revision, &edits, column)
                        .map_err(|e| e.to_string()),
                    (Err(error), _) => Err(error.to_string()),
                    (_, Err(error)) => Err(error.to_string()),
                };
                sections.push((p, epoch, water_epoch, result));
            }
            ColumnOutput {
                coordinate,
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
            chunks.columns.insert(output.coordinate, prepared);
            columns_changed.write(PreparedColumnChanged {
                coordinate: output.coordinate,
            });
        }
        for (position, epoch, water_epoch, result) in output.sections {
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
                    if authoritative {
                        for &p in &section.fluid_wake {
                            journal.bypass_change_detection().wake_fluid_neighbors(p);
                        }
                        journal.bypass_change_detection().resume_fluids(position);
                    }
                    journal.apply_prop_removals(position, &mut section.props);
                    let has_fluids = journal
                        .fluid_snapshot(position)
                        .is_ok_and(|snapshot| !snapshot.is_empty())
                        || journal
                            .blocks_in_section(position)
                            .any(|(_, material)| crate::water::space::partial(material));
                    if let Some([low, high]) = section.surface_sections {
                        chunks
                            .surface_ranges
                            .insert([position.x, position.z], [low, high]);
                        for y in low..=high {
                            let surface = SectionPos::new(position.x, y, position.z);
                            if chunks.wanted.insert(surface) && !pending.contains(&surface) {
                                chunks.queue.push_back(surface);
                            }
                        }
                    }
                    chunks.loaded.insert(
                        position,
                        LoadedSection {
                            data: section.data,
                            mesh: section.mesh,
                            mesh_epoch: epoch,
                            water_epoch: if has_fluids {
                                water_epoch.saturating_sub(1)
                            } else {
                                water_epoch
                            },
                            water: section.water,
                            props: section.props,
                        },
                    );
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
mod tests {
    use super::*;
    use crate::generation::sections::GeneratedSection;
    use std::time::{Duration, Instant};
    use terra_voxel::{BlockSection, MaterialId, global_to_section};

    #[derive(Resource, Default)]
    struct ChunkChanges(usize);

    #[test]
    fn reloading_a_naturally_waterlogged_partial_block_requests_a_geometry_patch() {
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
        let position = [-1, -1, -1];
        let (owner, local) = global_to_section(position);
        app.world_mut()
            .resource_mut::<EditJournal>()
            .record(position, 82)
            .unwrap();
        assert!(
            app.world()
                .resource::<EditJournal>()
                .fluid_snapshot(owner)
                .unwrap()
                .is_empty()
        );
        let mut data = BlockSection::empty(owner).unwrap();
        data.set_block(local, 82).unwrap();
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
        assert!(
            app.world()
                .resource::<ChunkStore>()
                .water_dirty
                .contains(&owner)
        );
        assert_eq!(
            app.world().resource::<ChunkStore>().loaded[&owner].water_epoch,
            water_epoch - 1
        );
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
