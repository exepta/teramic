//! Batching of the stock Bevy 0.19 point-shadow render graph. No cached maps,
//! dropped faces, reduced geometry or changes to the camera/directional passes.
use bevy::{
    core_pipeline::schedule::{Core3d, RootNonCameraView},
    ecs::schedule::{InternedScheduleLabel, Schedule, ScheduleLabel, SingleThreadedExecutor},
    pbr::{
        LightEntity, PreprocessPipelines, ShadowView, SkipGpuPreprocess, ViewLightEntities,
        clear_indirect_parameters_metadata, early_gpu_preprocess,
        early_prepass_build_indirect_parameters, per_view_shadow_pass, shared_shadow_pass,
        unpack_bins,
    },
    prelude::*,
    render::{
        Render, RenderApp, RenderSystems,
        batching::gpu_preprocessing::{
            BatchedInstanceBuffers, BinUnpackingBuffers, BinUnpackingBuffersKey,
            GpuPreprocessingSupport, PreprocessWorkItemBuffers,
        },
        occlusion_culling::OcclusionCulling,
        renderer::CurrentView,
        view::{ExtractedView, NoIndirectDrawing},
    },
};

const MAX_FACES: usize = 96;
#[derive(ScheduleLabel, Debug, Clone, PartialEq, Eq, Hash)]
struct PointShadowGroup;
#[derive(ScheduleLabel, Debug, Clone, PartialEq, Eq, Hash)]
enum FacePass {
    Unpack,
    Preprocess,
}
#[derive(Resource)]
struct ShadowGroupView(Entity);

impl FromWorld for ShadowGroupView {
    fn from_world(world: &mut World) -> Self {
        Self(world.spawn(ViewLightEntities { lights: Vec::new() }).id())
    }
}
#[derive(Component)]
struct GroupedFace(InternedScheduleLabel);
#[derive(Resource, Default)]
struct Faces {
    views: Vec<Entity>,
    failed: bool,
    batch_encoders: bool,
}
#[derive(Clone, Copy)]
struct Face {
    entity: Entity,
    light: Entity,
    index: usize,
    original: InternedScheduleLabel,
}

pub(crate) fn install(app: &mut App) {
    let reference = std::env::args().any(|arg| arg.starts_with("--smoke-test"))
        && std::env::var("TERRA_GROUP_POINT_SHADOWS").is_ok_and(|mode| mode == "0");
    if reference {
        return;
    }
    let Some(render) = app.get_sub_app_mut(RenderApp) else {
        return;
    };
    let mut schedule = Schedule::new(PointShadowGroup);
    schedule.set_executor(SingleThreadedExecutor::new());
    render
        .add_schedule(schedule)
        .init_resource::<ShadowGroupView>()
        .insert_resource(Faces {
            batch_encoders: !(std::env::args().any(|arg| arg.starts_with("--smoke-test"))
                && std::env::var("TERRA_SHADOW_ENCODERS").is_ok_and(|mode| mode == "per_face")),
            ..default()
        })
        .add_systems(PointShadowGroup, render_group)
        .add_systems(
            Render,
            regroup
                .after(RenderSystems::CreateViews)
                .before(RenderSystems::Specialize),
        );
    for label in [FacePass::Unpack, FacePass::Preprocess] {
        let mut pass = Schedule::new(label);
        pass.set_executor(SingleThreadedExecutor::new());
        // A RenderContext finishes its encoder when deferred buffers apply.
        // These one-system schedules deliberately flush once per group, in
        // unpack -> preprocess -> indirect-build -> depth-render order.
        pass.set_apply_final_deferred(false);
        render.add_schedule(pass);
    }
    render
        .add_systems(FacePass::Unpack, unpack_bins)
        .add_systems(FacePass::Preprocess, early_gpu_preprocess);
}

fn render_faces(world: &mut World, faces: &[Entity], label: FacePass) {
    world.schedule_scope(label, |world, schedule| {
        for &face in faces {
            if !has_preprocess_work(world, face) {
                continue;
            }
            world.insert_resource(CurrentView(face));
            schedule.run(world);
        }
        schedule.apply_deferred(world);
    })
}

fn has_preprocess_work(world: &World, face: Entity) -> bool {
    let (Some(view), Some(buffers), Some(unpack)) = (
        world.get::<ExtractedView>(face),
        world.get_resource::<BatchedInstanceBuffers<bevy::pbr::MeshUniform, bevy::pbr::MeshInputUniform>>(),
        world.get_resource::<BinUnpackingBuffers>(),
    ) else { return true; };
    buffers
        .phase_instance_buffers
        .iter()
        .any(|(&phase, buffers)| {
            let work = buffers.work_item_buffers.get(&view.retained_view_entity);
            let direct = match work {
                Some(PreprocessWorkItemBuffers::Direct(b)) => !b.is_empty(),
                Some(PreprocessWorkItemBuffers::Indirect {
                    indexed,
                    non_indexed,
                    gpu_occlusion_culling,
                }) => {
                    !indexed.is_empty()
                        || !non_indexed.is_empty()
                        || gpu_occlusion_culling.is_some()
                }
                None => false,
            };
            direct
                || unpack
                    .view_phase_buffers
                    .get(&BinUnpackingBuffersKey {
                        phase,
                        view: view.retained_view_entity,
                    })
                    .is_some_and(|jobs| {
                        jobs.indexed_unpacking_jobs
                            .iter()
                            .chain(&jobs.non_indexed_unpacking_jobs)
                            .any(|job| job.mesh_instance_count > 0)
                    })
        })
    // Only compute dispatches are omitted. Depth clears and all six face render
    // passes still execute, so an emptied face cannot retain an old shadow.
}

fn complete_faces(mut faces: Vec<Face>) -> Vec<Face> {
    if faces.len() > MAX_FACES {
        return Vec::new();
    }
    faces.sort_unstable_by_key(|f| (f.light.to_bits(), f.index, f.entity.to_bits()));
    faces
        .chunk_by(|a, b| a.light == b.light)
        .filter(|group| group.len() == 6 && group.iter().zip(0..6).all(|(f, i)| f.index == i))
        .flatten()
        .copied()
        .collect()
}

#[allow(clippy::type_complexity)]
fn regroup(
    mut commands: Commands,
    views: Query<
        (
            Entity,
            &LightEntity,
            Option<&RootNonCameraView>,
            Option<&GroupedFace>,
            Has<OcclusionCulling>,
            Has<SkipGpuPreprocess>,
            Has<NoIndirectDrawing>,
        ),
        With<ExtractedView>,
    >,
    mut state: ResMut<Faces>,
    support: Option<Res<GpuPreprocessingSupport>>,
) {
    let faces = if state.failed
        || !support
            .as_deref()
            .is_some_and(GpuPreprocessingSupport::is_culling_supported)
    {
        Vec::new()
    } else {
        complete_faces(
            views
                .iter()
                .filter_map(|(entity, light, root, previous, occlusion, skip, direct)| {
                    let LightEntity::Point {
                        light_entity,
                        face_index,
                    } = light
                    else {
                        return None;
                    };
                    let original = previous.map(|p| p.0).or_else(|| root.map(|r| r.0))?;
                    (!occlusion && !skip && !direct && original == Core3d.intern()).then_some(
                        Face {
                            entity,
                            light: *light_entity,
                            index: *face_index,
                            original,
                        },
                    )
                })
                .collect(),
        )
    };
    state.views.clear();
    state.views.extend(faces.iter().map(|f| f.entity));
    let selected: std::collections::HashSet<_> = state.views.iter().copied().collect();
    for (entity, _, _, previous, _, _, _) in &views {
        if let Some(previous) = previous
            && !selected.contains(&entity)
        {
            commands
                .entity(entity)
                .insert(RootNonCameraView(previous.0))
                .remove::<GroupedFace>();
        }
    }
    for (i, face) in faces.iter().enumerate() {
        let mut entity = commands.entity(face.entity);
        entity.insert(GroupedFace(face.original));
        // prepare_lights can reinsert Core3d each frame for a moving flame.
        if i == 0 {
            entity.insert(RootNonCameraView(PointShadowGroup.intern()));
        } else {
            entity.remove::<RootNonCameraView>();
        }
    }
}

fn render_group(world: &mut World) {
    let _scope = terra_core::profiling::ScopeTimer::new("render.point_shadow_group");
    let faces = world.resource::<Faces>().views.clone();
    if faces.is_empty() {
        return;
    }
    let previous = world.remove_resource::<CurrentView>();
    let gpu = world
        .get_resource::<GpuPreprocessingSupport>()
        .is_some_and(GpuPreprocessingSupport::is_culling_supported)
        && world.contains_resource::<PreprocessPipelines>();
    // Per-view jobs write disjoint ranges. Flush each complete phase in GPU
    // order; global metadata clearing/building is needed once for the group.
    let result = (|| -> Result<(), String> {
        if !gpu {
            return Err(
                "GPU indirect preprocessing unavailable; use the complete Core3d fallback".into(),
            );
        }
        world
            .run_system_cached(clear_indirect_parameters_metadata)
            .map_err(|e| e.to_string())?;
        if world.resource::<Faces>().batch_encoders {
            render_faces(world, &faces, FacePass::Unpack);
            render_faces(world, &faces, FacePass::Preprocess);
        } else {
            for &face in &faces {
                world.insert_resource(CurrentView(face));
                world
                    .run_system_cached(unpack_bins)
                    .map_err(|e| e.to_string())?;
                world
                    .run_system_cached(early_gpu_preprocess)
                    .map_err(|e| e.to_string())?;
            }
        }
        world
            .run_system_cached(early_prepass_build_indirect_parameters)
            .map_err(|e| e.to_string())?;
        if world.resource::<Faces>().batch_encoders {
            // The stock per-view pass accepts a list of shadow views. It calls
            // the same depth renderer for every face using a single context.
            // This helper is not a camera and never enters view extraction.
            let group_view = world.resource::<ShadowGroupView>().0;
            world
                .get_mut::<ViewLightEntities>(group_view)
                .unwrap()
                .lights
                .clone_from(&faces);
            world.insert_resource(CurrentView(group_view));
            world
                .run_system_cached(per_view_shadow_pass::<false>)
                .map_err(|e| e.to_string())?;
            crate::draw_calls::capture_grouped_shadow_boundary(world)?;
        } else {
            for &face in &faces {
                world.insert_resource(CurrentView(face));
                world
                    .run_system_cached(shared_shadow_pass::<false>)
                    .map_err(|e| e.to_string())?;
                crate::draw_calls::capture_grouped_shadow_boundary(world)?;
            }
        }
        Ok(())
    })();
    if let Err(error) = result {
        error!(%error, "Grouped point shadows failed; replaying every face through Core3d and disabling grouping");
        world.resource_mut::<Faces>().failed = true;
        world.resource_mut::<Faces>().views.clear();
        for face in faces {
            let Some(original) = world.get::<GroupedFace>(face).map(|f| f.0) else {
                continue;
            };
            world
                .entity_mut(face)
                .insert(RootNonCameraView(original))
                .remove::<GroupedFace>();
            world.insert_resource(CurrentView(face));
            if let Some(shadow) = world.get::<ShadowView>(face) {
                shadow.depth_attachment.prepare_for_new_frame();
            }
            // The stock pass clears each depth attachment, replacing any
            // partially emitted face instead of silently dropping its shadows.
            if let Err(error) = world.try_run_schedule(Core3d) {
                error!(%error, ?face, "Point-shadow Core3d fallback could not run");
            }
        }
    }
    if let Some(previous) = previous {
        world.insert_resource(previous);
    } else {
        world.remove_resource::<CurrentView>();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::render::{
        batching::gpu_preprocessing::GpuPreprocessingMode, render_resource::TextureFormat,
        sync_world::MainEntity, view::RetainedViewEntity,
    };

    #[derive(Resource, Default)]
    struct PassTrace(Vec<Entity>);
    #[derive(Resource, Default)]
    struct Applied(Vec<Entity>);

    #[test]
    fn empty_faces_skip_compute_but_new_work_and_unknown_resources_are_conservative() {
        use bevy::render::{
            batching::gpu_preprocessing::{PreprocessWorkItem, UntypedPhaseBatchedInstanceBuffers},
            render_resource::{BufferUsages, RawBufferVec},
        };
        let mut world = World::new();
        let light = world.spawn_empty().id();
        let extracted = view(light, 0);
        let id = extracted.retained_view_entity;
        let entity = world.spawn(extracted).id();
        assert!(has_preprocess_work(&world, entity));
        world.init_resource::<BatchedInstanceBuffers<bevy::pbr::MeshUniform, bevy::pbr::MeshInputUniform>>();
        world.init_resource::<BinUnpackingBuffers>();
        assert!(!has_preprocess_work(&world, entity));
        let mut work = RawBufferVec::new(BufferUsages::STORAGE);
        work.push(PreprocessWorkItem {
            input_index: 0,
            output_or_indirect_parameters_index: 0,
        });
        let mut phase = UntypedPhaseBatchedInstanceBuffers::default();
        phase
            .work_item_buffers
            .insert(id, PreprocessWorkItemBuffers::Direct(work));
        world.resource_mut::<BatchedInstanceBuffers<bevy::pbr::MeshUniform, bevy::pbr::MeshInputUniform>>()
            .phase_instance_buffers.insert(std::any::TypeId::of::<u8>(), phase);
        assert!(has_preprocess_work(&world, entity));
    }

    #[test]
    fn deferred_face_pass_preserves_all_views_and_flushes_only_after_the_group() {
        let mut app = App::new();
        app.init_resource::<PassTrace>().init_resource::<Applied>();
        let mut schedule = Schedule::new(FacePass::Unpack);
        schedule.set_executor(SingleThreadedExecutor::new());
        schedule.set_apply_final_deferred(false);
        schedule.add_systems(
            |view: Res<CurrentView>,
             mut trace: ResMut<PassTrace>,
             applied: Res<Applied>,
             mut commands: Commands| {
                assert!(applied.0.is_empty(), "must not flush between faces");
                trace.0.push(view.0);
                let entity = view.0;
                commands
                    .queue(move |world: &mut World| world.resource_mut::<Applied>().0.push(entity));
            },
        );
        app.add_schedule(schedule);
        let faces: Vec<_> = (0..48)
            .map(|_| app.world_mut().spawn_empty().id())
            .collect();
        render_faces(app.world_mut(), &faces, FacePass::Unpack);
        assert_eq!(app.world().resource::<PassTrace>().0, faces);
        assert_eq!(app.world().resource::<Applied>().0, faces);
    }

    fn view(light: Entity, index: usize) -> ExtractedView {
        ExtractedView {
            retained_view_entity: RetainedViewEntity::new(
                MainEntity::from(light),
                None,
                index as u32,
            ),
            clip_from_view: Mat4::IDENTITY,
            world_from_view: GlobalTransform::IDENTITY,
            clip_from_world: None,
            target_format: TextureFormat::Depth32Float,
            viewport: UVec4::new(0, 0, 512, 512),
            color_grading: default(),
            invert_culling: false,
        }
    }

    #[test]
    fn grouping_keeps_all_six_faces_and_restores_original_roots_when_a_light_becomes_incomplete() {
        let mut app = App::new();
        app.init_resource::<Faces>()
            .insert_resource(GpuPreprocessingSupport {
                max_supported_mode: GpuPreprocessingMode::Culling,
            })
            .add_systems(Update, regroup);
        let lights: Vec<_> = (0..2).map(|_| app.world_mut().spawn_empty().id()).collect();
        let mut faces = Vec::new();
        for &light in &lights {
            for index in 0..6 {
                faces.push(
                    app.world_mut()
                        .spawn((
                            LightEntity::Point {
                                light_entity: light,
                                face_index: index,
                            },
                            view(light, index),
                            RootNonCameraView(Core3d.intern()),
                        ))
                        .id(),
                );
            }
        }
        let camera = app
            .world_mut()
            .spawn((view(lights[0], 10), RootNonCameraView(Core3d.intern())))
            .id();
        for _ in 0..3 {
            // Model prepare_lights updating every flame's render view.
            for &face in &faces {
                app.world_mut()
                    .entity_mut(face)
                    .insert(RootNonCameraView(Core3d.intern()));
            }
            app.update();
            assert_eq!(app.world().resource::<Faces>().views.len(), 12);
            assert_eq!(
                faces
                    .iter()
                    .filter(|&&e| app.world().get::<RootNonCameraView>(e).is_some())
                    .count(),
                1
            );
            assert!(
                faces
                    .iter()
                    .all(|&e| app.world().get::<GroupedFace>(e).is_some())
            );
            assert_eq!(
                app.world().get::<RootNonCameraView>(camera).unwrap().0,
                Core3d.intern()
            );
        }
        app.world_mut()
            .entity_mut(faces[0])
            .insert(NoIndirectDrawing);
        app.update();
        assert_eq!(app.world().resource::<Faces>().views.len(), 6);
        assert!(
            faces[..6]
                .iter()
                .all(|&e| app.world().get::<RootNonCameraView>(e).is_some())
        );
        app.world_mut()
            .entity_mut(faces[0])
            .remove::<NoIndirectDrawing>()
            .insert(SkipGpuPreprocess);
        app.update();
        assert_eq!(app.world().resource::<Faces>().views.len(), 6);
        app.world_mut()
            .entity_mut(faces[0])
            .remove::<SkipGpuPreprocess>();
        for mode in [
            GpuPreprocessingMode::None,
            GpuPreprocessingMode::PreprocessingOnly,
        ] {
            app.world_mut()
                .resource_mut::<GpuPreprocessingSupport>()
                .max_supported_mode = mode;
            app.update();
            assert!(app.world().resource::<Faces>().views.is_empty());
            assert!(
                faces
                    .iter()
                    .all(|&e| app.world().get::<RootNonCameraView>(e).is_some())
            );
        }
        app.world_mut().remove_resource::<GpuPreprocessingSupport>();
        app.update();
        assert!(app.world().resource::<Faces>().views.is_empty());
        app.world_mut().insert_resource(GpuPreprocessingSupport {
            max_supported_mode: GpuPreprocessingMode::Culling,
        });
        app.update();
        assert_eq!(app.world().resource::<Faces>().views.len(), 12);
        app.world_mut().despawn(faces[0]);
        app.update();
        assert_eq!(app.world().resource::<Faces>().views.len(), 6);
        for &face in &faces[1..6] {
            assert!(app.world().get::<GroupedFace>(face).is_none());
            assert_eq!(
                app.world().get::<RootNonCameraView>(face).unwrap().0,
                Core3d.intern()
            );
        }
        app.world_mut()
            .entity_mut(faces[6])
            .insert(OcclusionCulling);
        app.update();
        assert!(app.world().resource::<Faces>().views.is_empty());
        assert!(
            faces[6..]
                .iter()
                .all(|&e| app.world().get::<RootNonCameraView>(e).is_some())
        );
    }

    #[test]
    fn duplicate_invalid_or_over_budget_faces_never_form_a_partial_cubemap() {
        let mut world = World::new();
        let light = world.spawn_empty().id();
        let original: Vec<_> = (0..6)
            .map(|index| Face {
                entity: world.spawn_empty().id(),
                light,
                index,
                original: Core3d.intern(),
            })
            .collect();
        assert_eq!(complete_faces(original.clone()).len(), 6);
        let mut duplicate = original.clone();
        duplicate[5].index = 4;
        assert!(complete_faces(duplicate).is_empty());
        let mut invalid = original.clone();
        invalid[0].index = 6;
        assert!(complete_faces(invalid).is_empty());
        assert!(
            complete_faces(original.into_iter().cycle().take(MAX_FACES + 1).collect()).is_empty()
        );
    }

    #[test]
    fn failed_group_replays_every_face_restores_current_view_and_keeps_the_normal_path_next_frame()
    {
        #[derive(Resource, Default)]
        struct Replayed(Vec<Entity>);
        let mut app = App::new();
        app.init_resource::<Faces>()
            .insert_resource(GpuPreprocessingSupport {
                max_supported_mode: GpuPreprocessingMode::Culling,
            })
            .init_resource::<Replayed>()
            .add_systems(Update, regroup)
            .add_systems(
                Core3d,
                |view: Res<CurrentView>, mut seen: ResMut<Replayed>| seen.0.push(view.0),
            );
        let light = app.world_mut().spawn_empty().id();
        let faces: Vec<_> = (0..6)
            .map(|index| {
                app.world_mut()
                    .spawn((
                        LightEntity::Point {
                            light_entity: light,
                            face_index: index,
                        },
                        view(light, index),
                        RootNonCameraView(Core3d.intern()),
                    ))
                    .id()
            })
            .collect();
        app.update();
        assert_eq!(app.world().resource::<Faces>().views.len(), 6);
        let camera = app.world_mut().spawn_empty().id();
        app.world_mut().insert_resource(CurrentView(camera));
        // No preprocessing pipelines exist in this CPU fixture: the incomplete
        // GPU path must replay the full stock schedule, not only its shadow draw.
        app.world_mut().run_system_cached(render_group).unwrap();
        assert_eq!(app.world().resource::<CurrentView>().0, camera);
        assert!(app.world().resource::<Faces>().failed);
        let seen = &app.world().resource::<Replayed>().0;
        assert_eq!(seen.len(), 6);
        assert!(faces.iter().all(|face| seen.contains(face)));
        app.update();
        assert!(app.world().resource::<Faces>().views.is_empty());
        for face in faces {
            assert!(app.world().get::<GroupedFace>(face).is_none());
            assert_eq!(
                app.world().get::<RootNonCameraView>(face).unwrap().0,
                Core3d.intern()
            );
        }
    }
}
