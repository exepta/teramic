//! Reuse stock Bevy preprocessing bindings, not the per-frame GPU data they bind.
use std::{
    any::TypeId,
    collections::{HashMap, HashSet},
};

use bevy::{
    core_pipeline::{
        mip_generation::experimental::depth::ViewDepthPyramid,
        prepass::{PreviousViewUniformOffset, PreviousViewUniforms},
    },
    ecs::{
        schedule::ScheduleCleanupPolicy,
        system::{CombinatorSystem, Combine, RunSystemError, SystemIn, SystemParam},
    },
    pbr::{
        BinUnpackingBindGroups, BuildIndirectParametersBindGroups, MeshCullingDataBuffer,
        MeshInputUniform, MeshUniform, PreprocessBindGroups, PreprocessPipelines,
        prepare_preprocess_bind_groups, prepare_preprocess_pipelines,
    },
    prelude::*,
    render::{
        Render, RenderApp, RenderStartup, RenderSystems,
        batching::gpu_preprocessing::{
            BatchedInstanceBuffers, BinUnpackingBuffers, IndirectParametersBuffers,
            PreprocessWorkItemBuffers,
        },
        render_resource::{
            BindGroupLayoutId, BindingResource, Buffer, BufferId, PipelineCache, TextureViewId,
        },
        renderer::RenderDevice,
        view::{ExtractedView, RenderVisibilityRanges, RetainedViewEntity, ViewUniforms},
    },
};

#[derive(Debug, Clone, PartialEq, Eq)]
enum Binding {
    Buffer(Option<BufferId>, u64, Option<u64>),
    Count(usize),
    Depth(TextureViewId, u32),
    Layout(BindGroupLayoutId),
}
fn buffer(buffer: Option<&Buffer>) -> Binding {
    Binding::Buffer(buffer.map(Buffer::id), 0, None)
}
fn binding(binding: Option<BindingResource<'_>>, source: Option<&Buffer>) -> Binding {
    match binding {
        Some(BindingResource::Buffer(b)) => {
            Binding::Buffer(source.map(Buffer::id), b.offset, b.size.map(|s| s.get()))
        }
        None => buffer(None),
        _ => unreachable!("uniform and visibility buffers have buffer bindings"),
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
struct Snapshot {
    global: Vec<Binding>,
    phases: HashMap<TypeId, Vec<Binding>>,
    work: HashMap<(TypeId, RetainedViewEntity), Vec<Binding>>,
    unpack: HashMap<(TypeId, RetainedViewEntity), Vec<Binding>>,
    views: HashSet<RetainedViewEntity>,
    depth: HashMap<RetainedViewEntity, Option<Binding>>,
}
#[derive(Default, Resource)]
struct Cache(
    Option<Snapshot>,
    usize,
    HashMap<RetainedViewEntity, PreprocessBindGroups>,
    Option<CachedFrame>,
);
struct CachedFrame {
    signature: Snapshot,
    views: HashMap<RetainedViewEntity, PreprocessBindGroups>,
    unpacked: BinUnpackingBindGroups,
    indirect: Option<BuildIndirectParametersBindGroups>,
}
#[derive(Component)]
struct NeedsBindings;
impl Cache {
    fn select_buffers(
        &mut self,
        next: &Snapshot,
        unpacked: &mut BinUnpackingBindGroups,
        built: Option<&mut BuildIndirectParametersBindGroups>,
    ) -> bool {
        let matches = |signature: &Snapshot| {
            signature.global == next.global && !phase_buffers_changed(signature, next)
        };
        if self
            .3
            .as_ref()
            .is_some_and(|frame| matches(&frame.signature))
        {
            let old = self.3.take().unwrap();
            let unpacked = std::mem::replace(unpacked, old.unpacked);
            let indirect =
                built.map(|current| std::mem::replace(current, old.indirect.unwrap_or_default()));
            self.3 = self.0.take().map(|signature| CachedFrame {
                signature,
                views: std::mem::take(&mut self.2),
                unpacked,
                indirect,
            });
            self.0 = Some(old.signature);
            self.2 = old.views;
            true
        } else {
            if self.0.as_ref().is_some_and(|old| !matches(old)) {
                self.3 = self.0.take().map(|signature| CachedFrame {
                    signature,
                    views: std::mem::take(&mut self.2),
                    unpacked: std::mem::take(unpacked),
                    indirect: built.map(std::mem::take),
                });
            }
            false
        }
    }
    fn needs_rebuild(&mut self, next: Snapshot, complete: bool) -> bool {
        self.1 += 1;
        if self.1.is_multiple_of(180)
            && std::env::var("TERRA_BINDING_DIFF").is_ok_and(|v| v == "1")
            && let Some(previous) = &self.0
        {
            info!(
                complete,
                global = previous.global != next.global,
                phases = previous.phases != next.phases,
                work = previous.work != next.work,
                unpack = previous.unpack != next.unpack,
                views = previous.views != next.views,
                depth = previous.depth != next.depth,
                "Preprocessing cache invalidation categories"
            );
            if previous.global != next.global {
                info!(old=?previous.global, new=?next.global, "Preprocessing global binding change");
            }
            if let Some((phase, old)) = previous
                .phases
                .iter()
                .find(|(key, value)| next.phases.get(key) != Some(value))
            {
                info!(?phase, ?old, new=?next.phases.get(phase),"Preprocessing phase binding change");
            }
            if let Some((key, old)) = previous
                .work
                .iter()
                .find(|(key, value)| next.work.get(key) != Some(value))
            {
                info!(?key, ?old, new=?next.work.get(key),"Preprocessing work binding change");
            }
            if let Some((key, old)) = previous
                .unpack
                .iter()
                .find(|(key, value)| next.unpack.get(key) != Some(value))
            {
                info!(?key, old=?old.get(..old.len().min(9)), new=?next.unpack.get(key).map(|v|&v[..v.len().min(9)]),"Preprocessing unpack binding change");
            }
        }
        if complete && self.0.as_ref() == Some(&next) {
            return false;
        }
        self.0 = Some(next);
        true
    }
}

#[derive(SystemParam)]
struct Inputs<'w, 's> {
    views: Query<'w, 's, (Entity, &'static ExtractedView, Has<PreprocessBindGroups>)>,
    depth: Query<
        'w,
        's,
        (
            &'static ViewDepthPyramid,
            &'static PreviousViewUniformOffset,
        ),
    >,
    buffers: Res<'w, BatchedInstanceBuffers<MeshUniform, MeshInputUniform>>,
    indirect: Res<'w, IndirectParametersBuffers>,
    unpack: Res<'w, BinUnpackingBuffers>,
    culling: Res<'w, MeshCullingDataBuffer>,
    ranges: Res<'w, RenderVisibilityRanges>,
    uniforms: Res<'w, ViewUniforms>,
    previous: Res<'w, PreviousViewUniforms>,
    pipelines: Res<'w, PreprocessPipelines>,
    pipeline_cache: Res<'w, PipelineCache>,
    built: Option<ResMut<'w, BuildIndirectParametersBindGroups>>,
    unpacked: ResMut<'w, BinUnpackingBindGroups>,
}

fn changed(mut inputs: Inputs, mut cache: ResMut<Cache>, mut commands: Commands) {
    let _scope = terra_core::profiling::ScopeTimer::new("render.preprocess_bindings.signature");
    let mut next = Snapshot {
        global: vec![
            buffer(inputs.buffers.current_input_buffer.buffer().buffer()),
            buffer(inputs.buffers.previous_input_buffer.buffer()),
            buffer(inputs.culling.buffer()),
            binding(
                inputs.ranges.buffer().binding(),
                inputs.ranges.buffer().buffer(),
            ),
            binding(
                inputs.uniforms.uniforms.binding(),
                inputs.uniforms.uniforms.buffer(),
            ),
            buffer(inputs.previous.uniforms.buffer()),
            buffer(inputs.unpack.bin_unpacking_metadata.buffer()),
        ],
        ..Default::default()
    };
    let p = &inputs.pipelines;
    for layout in [
        &p.direct_preprocess.bind_group_layout,
        &p.gpu_frustum_culling_preprocess.bind_group_layout,
        &p.early_gpu_occlusion_culling_preprocess.bind_group_layout,
        &p.late_gpu_occlusion_culling_preprocess.bind_group_layout,
        &p.gpu_frustum_culling_build_indexed_indirect_params
            .bind_group_layout,
        &p.gpu_frustum_culling_build_non_indexed_indirect_params
            .bind_group_layout,
        &p.early_phase.reset_indirect_batch_sets.bind_group_layout,
        &p.bin_unpacking.bind_group_layout,
    ] {
        next.global.push(Binding::Layout(
            inputs.pipeline_cache.get_bind_group_layout(layout).id(),
        ));
    }
    let mut complete = true;
    for (entity, view, _) in &inputs.views {
        complete &= cache.2.contains_key(&view.retained_view_entity);
        next.views.insert(view.retained_view_entity);
        next.depth.insert(
            view.retained_view_entity,
            inputs
                .depth
                .get(entity)
                .ok()
                .map(|(d, o)| Binding::Depth(d.all_mips.id(), o.offset)),
        );
    }
    let mut indirect = false;
    for (&phase, buffers) in &inputs.buffers.phase_instance_buffers {
        let mut signature = vec![
            buffer(buffers.data_buffer.buffer()),
            buffer(buffers.late_indexed_indirect_parameters_buffer.buffer()),
            buffer(buffers.late_non_indexed_indirect_parameters_buffer.buffer()),
        ];
        if let Some(parameters) = inputs.indirect.get(&phase) {
            macro_rules! class {
                ($class:expr) => {
                    signature.extend([
                        buffer($class.data_buffer()),
                        buffer($class.cpu_metadata_buffer()),
                        buffer($class.gpu_metadata_buffer()),
                        buffer($class.batch_sets_buffer()),
                        Binding::Count($class.batch_count()),
                    ]);
                };
            }
            class!(parameters.indexed);
            class!(parameters.non_indexed);
        }
        next.phases.insert(phase, signature);
        for (&view, work) in &buffers.work_item_buffers {
            let signature = match work {
                PreprocessWorkItemBuffers::Direct(work) => vec![
                    Binding::Count(0),
                    buffer(work.buffer()),
                    Binding::Count(work.len()),
                ],
                PreprocessWorkItemBuffers::Indirect {
                    indexed,
                    non_indexed,
                    gpu_occlusion_culling,
                } => {
                    indirect = true;
                    let mut work = vec![
                        Binding::Count(1),
                        buffer(indexed.buffer()),
                        Binding::Count(indexed.len()),
                        buffer(non_indexed.buffer()),
                        Binding::Count(non_indexed.len()),
                    ];
                    if let Some(late) = gpu_occlusion_culling {
                        work.extend([
                            Binding::Count(2),
                            buffer(late.late_indexed.buffer()),
                            Binding::Count(late.late_indexed.len()),
                            buffer(late.late_non_indexed.buffer()),
                            Binding::Count(late.late_non_indexed.len()),
                        ]);
                    }
                    work
                }
            };
            next.work.insert((phase, view), signature);
        }
    }
    complete &= !indirect || inputs.built.is_some();
    for (key, jobs) in &inputs.unpack.view_phase_buffers {
        let mut signature = vec![Binding::Count(jobs.indexed_unpacking_jobs.len())];
        for job in jobs
            .indexed_unpacking_jobs
            .iter()
            .chain(&jobs.non_indexed_unpacking_jobs)
        {
            signature.extend([
                buffer(Some(&job.render_binned_mesh_instance_buffer)),
                buffer(Some(&job.bin_index_to_indirect_parameters_offset_buffer)),
                Binding::Count(job.bin_unpacking_metadata_index.uniform_offset() as usize),
                Binding::Count(job.mesh_instance_count as usize),
            ]);
        }
        if next.views.contains(&key.view) && next.work.contains_key(&(key.phase, key.view)) {
            complete &= inputs.unpacked.contains_key(key);
            next.unpack.insert((key.phase, key.view), signature);
        }
    }
    // Equality is structural and order-independent, not a probabilistic hash.
    // Bevy collect_buffers_for_phase swaps two persistent buffer sets. Compare
    // with the frame owning this set, not invariably with the opposite set.
    let reused = cache.select_buffers(&next, &mut inputs.unpacked, inputs.built.as_deref_mut());
    let previous = cache.0.as_ref();
    let shared =
        previous.is_none_or(|old| old.global != next.global || phase_buffers_changed(old, &next));
    let build = inputs.built.as_ref().is_none_or(|b| b.is_empty())
        || previous.is_none_or(|old| old.phases != next.phases);
    let mut required = previous.map_or_else(HashSet::new, |old| changed_views(old, &next));
    for key in inputs.unpack.view_phase_buffers.keys() {
        if next.unpack.contains_key(&(key.phase, key.view)) && !inputs.unpacked.contains_key(key) {
            required.insert(key.view);
        }
    }
    for (_, view, _) in &inputs.views {
        let id = view.retained_view_entity;
        if shared || !cache.2.contains_key(&id) {
            required.insert(id);
        }
    }
    // The stock helper also prepares shared indirect-parameter bindings. A
    // count-only change needs one indirect view, not all shadow cubemap faces.
    let is_indirect = |id| {
        next.work
            .iter()
            .any(|((_, view), work)| *view == id && work.first() == Some(&Binding::Count(1)))
    };
    if build
        && !required.iter().any(|&id| is_indirect(id))
        && let Some(id) = next.views.iter().copied().find(|&id| is_indirect(id))
    {
        required.insert(id);
    }
    for (entity, view, has_groups) in &inputs.views {
        let id = view.retained_view_entity;
        if required.contains(&id) {
            commands.entity(entity).insert(NeedsBindings);
        } else {
            commands.entity(entity).remove::<NeedsBindings>();
            if (!has_groups || reused)
                && let Some(groups) = cache.2.get(&id)
            {
                commands.entity(entity).insert(groups.clone());
            }
        }
    }
    inputs.unpacked.retain(|key, _| {
        next.views.contains(&key.view) && next.work.contains_key(&(key.phase, key.view))
    });
    cache.needs_rebuild(next, complete);
}

fn phase_buffers_changed(old: &Snapshot, next: &Snapshot) -> bool {
    old.phases.len() != next.phases.len()
        || next.phases.iter().any(|(phase, values)| {
            old.phases.get(phase).is_none_or(|previous| {
                !previous
                    .iter()
                    .filter(|v| !matches!(v, Binding::Count(_)))
                    .eq(values.iter().filter(|v| !matches!(v, Binding::Count(_))))
            })
        })
}
fn changed_views(old: &Snapshot, next: &Snapshot) -> HashSet<RetainedViewEntity> {
    fn different(
        a: &HashMap<(TypeId, RetainedViewEntity), Vec<Binding>>,
        b: &HashMap<(TypeId, RetainedViewEntity), Vec<Binding>>,
        result: &mut HashSet<RetainedViewEntity>,
    ) {
        for (key, value) in b {
            if a.get(key) != Some(value) {
                result.insert(key.1);
            }
        }
        for key in a.keys().filter(|key| !b.contains_key(key)) {
            result.insert(key.1);
        }
    }
    let mut result = HashSet::new();
    different(&old.work, &next.work, &mut result);
    different(&old.unpack, &next.unpack, &mut result);
    for (view, depth) in &next.depth {
        if old.depth.get(view) != Some(depth) {
            result.insert(*view);
        }
    }
    result
}

#[derive(SystemParam)]
struct PrepareInputs<'w, 's> {
    depth: Query<
        'w,
        's,
        (
            &'static ViewDepthPyramid,
            &'static PreviousViewUniformOffset,
        ),
    >,
    device: Res<'w, RenderDevice>,
    pipeline_cache: Res<'w, PipelineCache>,
    buffers: Res<'w, BatchedInstanceBuffers<MeshUniform, MeshInputUniform>>,
    indirect: Res<'w, IndirectParametersBuffers>,
    unpack: Res<'w, BinUnpackingBuffers>,
    culling: Res<'w, MeshCullingDataBuffer>,
    ranges: Res<'w, RenderVisibilityRanges>,
    uniforms: Res<'w, ViewUniforms>,
    previous: Res<'w, PreviousViewUniforms>,
    pipelines: Res<'w, PreprocessPipelines>,
    unpacked: ResMut<'w, BinUnpackingBindGroups>,
}
fn prepare(
    commands: Commands,
    mut views: Query<(Entity, &ExtractedView), With<NeedsBindings>>,
    inputs: PrepareInputs,
) {
    if views.is_empty() {
        return;
    }
    let _scope = terra_core::profiling::ScopeTimer::new("render.preprocess_bind_groups.body");
    // QueryLens retains the source's matched archetypes. The original public
    // helper receives exactly the changed views without modifying ExtractedView.
    let mut lens = views.transmute_lens::<(Entity, &ExtractedView)>();
    prepare_preprocess_bind_groups(
        commands,
        lens.query(),
        inputs.depth,
        inputs.device,
        inputs.pipeline_cache,
        inputs.buffers,
        inputs.indirect,
        inputs.unpack,
        inputs.culling,
        inputs.ranges,
        inputs.uniforms,
        inputs.previous,
        inputs.pipelines,
        inputs.unpacked,
    );
}

fn collect(views: Query<(&ExtractedView, &PreprocessBindGroups)>, mut cache: ResMut<Cache>) {
    cache.2.clear();
    cache.2.extend(
        views
            .iter()
            .map(|(view, groups)| (view.retained_view_entity, groups.clone())),
    );
}

struct Cached;
impl<A: System<In = (), Out = ()>, B: System<In = (), Out = ()>> Combine<A, B> for Cached {
    type In = ();
    type Out = ();
    fn combine<T>(
        _: (),
        data: &mut T,
        _: impl FnOnce(SystemIn<'_, A>, &mut T) -> Result<(), RunSystemError>,
        filtered: impl FnOnce(SystemIn<'_, B>, &mut T) -> Result<(), RunSystemError>,
    ) -> Result<(), RunSystemError> {
        filtered((), data)
    }
}
fn cached<A: IntoSystem<(), (), M>, M, B: IntoSystem<(), (), N>, N>(
    body: A,
    guard: B,
) -> impl System<In = (), Out = ()> {
    CombinatorSystem::<Cached, _, _>::new(
        IntoSystem::into_system(body),
        IntoSystem::into_system(guard),
        "cached_preprocess_bind_groups".into(),
    )
}
pub(crate) fn install(app: &mut App) {
    if std::env::args().any(|a| a.starts_with("--smoke-test"))
        && std::env::var("TERRA_PREPROCESS_CACHE").is_ok_and(|v| v == "0")
    {
        return;
    }
    if let Some(render) = app.get_sub_app_mut(RenderApp) {
        render
            .init_resource::<Cache>()
            .add_systems(RenderStartup, replace);
    }
}
fn replace(world: &mut World) {
    world.schedule_scope(Render, |world, schedule| {
        if let Ok(1) = schedule.remove_systems_in_set(
            prepare_preprocess_bind_groups,
            world,
            ScheduleCleanupPolicy::RemoveSystemsOnly,
        ) {
            schedule.add_systems(
                (
                    changed,
                    cached(prepare_preprocess_bind_groups, prepare),
                    collect,
                )
                    .chain()
                    .in_set(RenderSystems::PrepareBindGroups)
                    .after(prepare_preprocess_pipelines)
                    .run_if(
                        resource_exists::<BatchedInstanceBuffers<MeshUniform, MeshInputUniform>>,
                    ),
            );
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn alternating_gpu_buffers_reuse_only_their_own_bounded_cache_entry() {
        let mut cache = Cache::default();
        let mut unpacked = BinUnpackingBindGroups::default();
        let a = Snapshot {
            global: vec![buffer(None)],
            ..Default::default()
        };
        let b = Snapshot {
            global: vec![buffer(None), buffer(None)],
            ..Default::default()
        };
        let c = Snapshot {
            global: vec![buffer(None); 3],
            ..Default::default()
        };
        assert!(!cache.select_buffers(&a, &mut unpacked, None));
        cache.needs_rebuild(a.clone(), true);
        assert!(!cache.select_buffers(&b, &mut unpacked, None));
        assert!(cache.0.is_none());
        assert_eq!(cache.3.as_ref().unwrap().signature, a);
        cache.needs_rebuild(b.clone(), true);
        for next in [&a, &b, &a, &b] {
            assert!(cache.select_buffers(next, &mut unpacked, None));
            assert_eq!(cache.0.as_ref(), Some(next));
            cache.needs_rebuild(next.clone(), true);
        }
        assert!(!cache.select_buffers(&c, &mut unpacked, None));
        assert!(cache.0.is_none());
        assert_eq!(cache.3.as_ref().unwrap().signature, b);
    }
    #[test]
    fn phase_counts_do_not_rebuild_other_views_but_reallocations_do() {
        let view = RetainedViewEntity::new(Entity::PLACEHOLDER.into(), None, 0);
        let other = RetainedViewEntity::new(Entity::PLACEHOLDER.into(), None, 1);
        let phase = TypeId::of::<u8>();
        let mut previous = Snapshot::default();
        previous
            .phases
            .insert(phase, vec![buffer(None), Binding::Count(8)]);
        previous.work.insert((phase, view), vec![Binding::Count(4)]);
        previous
            .work
            .insert((phase, other), vec![Binding::Count(2)]);
        let mut next = previous.clone();
        next.phases.get_mut(&phase).unwrap()[1] = Binding::Count(9);
        next.work.get_mut(&(phase, other)).unwrap()[0] = Binding::Count(3);
        assert!(!phase_buffers_changed(&previous, &next));
        assert!(!changed_views(&previous, &next).contains(&view));
        assert!(changed_views(&previous, &next).contains(&other));
        next.phases.get_mut(&phase).unwrap()[0] = Binding::Buffer(Some(BufferId::new()), 0, None);
        assert!(phase_buffers_changed(&previous, &next));
    }
    #[test]
    fn unchanged_signature_reuses_bindings_but_sizes_views_and_missing_outputs_invalidate() {
        let mut cache = Cache::default();
        assert!(cache.needs_rebuild(Snapshot::default(), false));
        assert!(!cache.needs_rebuild(Snapshot::default(), true));
        let mut snapshot = Snapshot::default();
        snapshot.global.push(Binding::Count(4));
        assert!(cache.needs_rebuild(snapshot, true));
        assert!(cache.needs_rebuild(Snapshot::default(), true));
        assert!(!cache.needs_rebuild(Snapshot::default(), true));
        assert!(cache.needs_rebuild(Snapshot::default(), false));
    }
    #[derive(Resource, Default)]
    struct Calls(usize);
    #[derive(Component)]
    struct Created;
    fn filtered(mut views: Query<&Created, With<NeedsBindings>>, mut calls: ResMut<Calls>) {
        let mut lens = views.transmute_lens::<&Created>();
        calls.0 = lens.query().iter().count();
    }
    #[test]
    fn query_lens_only_exposes_changed_views_to_the_unmodified_helper() {
        let mut app = App::new();
        app.init_resource::<Calls>();
        for _ in 0..7 {
            app.world_mut().spawn(Created);
        }
        app.world_mut().spawn((Created, NeedsBindings));
        app.add_systems(Update, cached(body, filtered));
        app.update();
        assert_eq!(app.world().resource::<Calls>().0, 1);
    }
    fn body(mut calls: ResMut<Calls>, mut commands: Commands) {
        calls.0 += 1;
        commands.spawn(Created);
    }
    #[test]
    fn cached_wrapper_preserves_function_order_and_deferred_work() {
        let mut app = App::new();
        app.init_resource::<Calls>().add_systems(
            Update,
            (
                cached(body, body),
                (|calls: Res<Calls>| assert!(calls.0 > 0)).after(body),
            ),
        );
        app.update();
        assert_eq!(app.world().resource::<Calls>().0, 1);
        assert_eq!(
            app.world_mut()
                .query::<&Created>()
                .iter(app.world())
                .count(),
            1
        );
    }
}
