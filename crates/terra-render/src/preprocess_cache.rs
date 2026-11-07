//! Smoke-only experiment: reuse stock bindings, not the per-frame GPU data they bind.
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
        UniformAllocationBindGroups, prepare_preprocess_bind_groups, prepare_preprocess_pipelines,
    },
    prelude::*,
    render::{
        Render, RenderApp, RenderStartup, RenderSystems,
        batching::gpu_preprocessing::{
            BatchedInstanceBuffers, IndirectParametersBuffers, IndirectParametersBuildJobs,
            PreprocessWorkItemBuffers, SceneUnpackingBuffers,
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
    unpack_global: Vec<Binding>,
    build_global: Vec<Binding>,
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
    allocated: UniformAllocationBindGroups,
    indirect: Option<BuildIndirectParametersBindGroups>,
}
#[derive(Component)]
#[component(storage = "SparseSet")]
struct NeedsBindings;
impl Cache {
    fn select_buffers(
        &mut self,
        next: &Snapshot,
        unpacked: &mut BinUnpackingBindGroups,
        allocated: &mut UniformAllocationBindGroups,
        built: Option<&mut BuildIndirectParametersBindGroups>,
    ) -> bool {
        let matches = |signature: &Snapshot| {
            signature.global == next.global
                && signature.unpack_global == next.unpack_global
                && signature.build_global == next.build_global
                && !phase_buffers_changed(signature, next)
        };
        if self
            .3
            .as_ref()
            .is_some_and(|frame| matches(&frame.signature))
        {
            let old = self.3.take().unwrap();
            let unpacked = std::mem::replace(unpacked, old.unpacked);
            let allocated = std::mem::replace(allocated, old.allocated);
            let indirect =
                built.map(|current| std::mem::replace(current, old.indirect.unwrap_or_default()));
            self.3 = self.0.take().map(|signature| CachedFrame {
                signature,
                views: std::mem::take(&mut self.2),
                unpacked,
                allocated,
                indirect,
            });
            self.0 = Some(old.signature);
            self.2 = old.views;
            true
        } else {
            if self.0.as_ref().is_some_and(|old| !matches(old)) {
                // Keep the current view table for the dependency diff. A single
                // phase reallocation must not discard every unrelated view.
                // These clones retain handles, not new GPU bind groups.
                self.3 = self.0.as_ref().map(|signature| CachedFrame {
                    signature: signature.clone(),
                    views: self.2.clone(),
                    unpacked: unpacked.clone(),
                    allocated: allocated.clone(),
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
                unpack_global = previous.unpack_global != next.unpack_global,
                build_global = previous.build_global != next.build_global,
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
    unpack: Res<'w, SceneUnpackingBuffers>,
    jobs: Res<'w, IndirectParametersBuildJobs>,
    culling: Res<'w, MeshCullingDataBuffer>,
    ranges: Res<'w, RenderVisibilityRanges>,
    uniforms: Res<'w, ViewUniforms>,
    previous: Res<'w, PreviousViewUniforms>,
    pipelines: Res<'w, PreprocessPipelines>,
    pipeline_cache: Res<'w, PipelineCache>,
    built: Option<ResMut<'w, BuildIndirectParametersBindGroups>>,
    unpacked: ResMut<'w, BinUnpackingBindGroups>,
    allocated: ResMut<'w, UniformAllocationBindGroups>,
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
        ],
        unpack_global: vec![
            buffer(inputs.unpack.bin_unpacking_metadata.buffer()),
            buffer(inputs.unpack.uniform_allocation_metadata.buffer()),
        ],
        build_global: vec![buffer(inputs.jobs.buffer())],
        ..Default::default()
    };
    let p = &inputs.pipelines;
    for layout in [
        &p.direct_preprocess.bind_group_layout,
        &p.gpu_frustum_culling_preprocess.bind_group_layout,
        &p.early_gpu_occlusion_culling_preprocess.bind_group_layout,
        &p.late_gpu_occlusion_culling_preprocess.bind_group_layout,
    ] {
        next.global.push(Binding::Layout(
            inputs.pipeline_cache.get_bind_group_layout(layout).id(),
        ));
    }
    for layout in [
        &p.bin_unpacking.bind_group_layout,
        &p.uniform_allocation.local_scan.bind_group_layout,
        &p.uniform_allocation.global_scan.bind_group_layout,
        &p.uniform_allocation.fan.bind_group_layout,
    ] {
        next.unpack_global.push(Binding::Layout(
            inputs.pipeline_cache.get_bind_group_layout(layout).id(),
        ));
    }
    for layout in [
        &p.gpu_frustum_culling_build_indexed_indirect_params
            .bind_group_layout,
        &p.gpu_frustum_culling_build_non_indexed_indirect_params
            .bind_group_layout,
        &p.early_phase.reset_indirect_batch_sets.bind_group_layout,
    ] {
        next.build_global.push(Binding::Layout(
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
                        buffer($class.metadata_buffer()),
                        buffer($class.batch_sets_buffer()),
                    ]);
                };
            }
            class!(parameters.indexed);
            class!(parameters.non_indexed);
            // Only the non-indexed metadata binding is sized by batch_count.
            // Indexed counts stay in GPU job data and dispatch resources;
            // changing them alone does not alter a bind group resource/range.
            signature.push(Binding::Count(parameters.non_indexed.batch_count()));
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
                buffer(Some(&job.bin_index_to_bin_metadata_index_buffer)),
                buffer(Some(&job.bin_metadata_buffer)),
                buffer(Some(&job.fan_buffer)),
                Binding::Count(job.bin_unpacking_metadata_index.uniform_offset() as usize),
                Binding::Count(job.uniform_allocation_metadata_index.uniform_offset() as usize),
                Binding::Count(job.bin_count as usize),
                Binding::Count(job.mesh_instance_count as usize),
            ]);
        }
        if next.views.contains(&key.view) && next.work.contains_key(&(key.phase, key.view)) {
            complete &= inputs.unpacked.contains_key(key) && inputs.allocated.contains_key(key);
            next.unpack.insert((key.phase, key.view), signature);
        }
    }
    // Equality is structural and order-independent, not a probabilistic hash.
    // Bevy collect_buffers_for_phase swaps two persistent buffer sets. Compare
    // with the frame owning this set, not invariably with the opposite set.
    let reused = cache.select_buffers(
        &next,
        &mut inputs.unpacked,
        &mut inputs.allocated,
        inputs.built.as_deref_mut(),
    );
    let previous = cache.0.as_ref();
    let shared = previous.is_none_or(|old| old.global != next.global);
    let build = inputs.built.as_ref().is_none_or(|b| b.is_empty())
        || previous
            .is_none_or(|old| old.phases != next.phases || old.build_global != next.build_global);
    let mut required = previous.map_or_else(HashSet::new, |old| changed_views(old, &next));
    for key in inputs.unpack.view_phase_buffers.keys() {
        if next.unpack.contains_key(&(key.phase, key.view))
            && (!inputs.unpacked.contains_key(key) || !inputs.allocated.contains_key(key))
        {
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
    include_indirect_builder_view(&mut required, &next, build);
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
    inputs.allocated.retain(|key, _| {
        next.views.contains(&key.view) && next.work.contains_key(&(key.phase, key.view))
    });
    cache.needs_rebuild(next, complete);
}

fn include_indirect_builder_view(
    required: &mut HashSet<RetainedViewEntity>,
    next: &Snapshot,
    build: bool,
) {
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
    let changed_phases: HashSet<_> = old
        .phases
        .keys()
        .chain(next.phases.keys())
        .filter(
            |&phase| match (old.phases.get(phase), next.phases.get(phase)) {
                (Some(previous), Some(current)) => !previous
                    .iter()
                    .filter(|value| !matches!(value, Binding::Count(_)))
                    .eq(current
                        .iter()
                        .filter(|value| !matches!(value, Binding::Count(_)))),
                _ => true,
            },
        )
        .copied()
        .collect();
    for &(phase, view) in old.work.keys().chain(next.work.keys()) {
        if changed_phases.contains(&phase) {
            result.insert(view);
        }
    }
    if old.unpack_global != next.unpack_global {
        for ((_, view), jobs) in old.unpack.iter().chain(&next.unpack) {
            // The leading count is present even when both job lists are empty.
            // Empty outputs contain no bind groups referencing these buffers.
            if jobs.len() > 1 {
                result.insert(*view);
            }
        }
    }
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
    jobs: Res<'w, IndirectParametersBuildJobs>,
    unpack: Res<'w, SceneUnpackingBuffers>,
    culling: Res<'w, MeshCullingDataBuffer>,
    ranges: Res<'w, RenderVisibilityRanges>,
    uniforms: Res<'w, ViewUniforms>,
    previous: Res<'w, PreviousViewUniforms>,
    pipelines: Res<'w, PreprocessPipelines>,
    unpacked: ResMut<'w, BinUnpackingBindGroups>,
    allocated: ResMut<'w, UniformAllocationBindGroups>,
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
        inputs.jobs,
        inputs.unpack,
        inputs.culling,
        inputs.ranges,
        inputs.uniforms,
        inputs.previous,
        inputs.pipelines,
        inputs.unpacked,
        inputs.allocated,
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
pub(crate) fn experiment_enabled() -> bool {
    experiment_requested(
        std::env::args().any(|argument| argument.starts_with("--smoke-test")),
        std::env::var("TERRA_PREPROCESS_CACHE").ok().as_deref(),
    )
}

fn experiment_requested(smoke: bool, value: Option<&str>) -> bool {
    smoke && value == Some("1")
}

pub(crate) fn install(app: &mut App) {
    if !experiment_enabled() {
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
    fn cached_bindings_require_explicit_smoke_opt_in_and_never_enable_during_gameplay() {
        assert!(!experiment_requested(false, None));
        assert!(!experiment_requested(false, Some("1")));
        assert!(!experiment_requested(true, None));
        assert!(!experiment_requested(true, Some("0")));
        assert!(!experiment_requested(true, Some("true")));
        assert!(experiment_requested(true, Some("1")));
    }

    #[test]
    fn binding_markers_preserve_dense_view_order_and_metadata_offsets() {
        let mut world = World::new();
        let views: Vec<_> = (0..24).map(|_| world.spawn(Created).id()).collect();
        let mut query = world.query_filtered::<Entity, With<Created>>();
        let original: Vec<_> = query.iter(&world).collect();
        for frame in 0..30 {
            for (i, &entity) in views.iter().enumerate() {
                if (i + frame) % 3 == 0 {
                    world.entity_mut(entity).insert(NeedsBindings);
                } else {
                    world.entity_mut(entity).remove::<NeedsBindings>();
                }
            }
            assert_eq!(query.iter(&world).collect::<Vec<_>>(), original);
        }
    }

    #[test]
    fn alternating_gpu_buffers_reuse_only_their_own_bounded_cache_entry() {
        let mut cache = Cache::default();
        let mut unpacked = BinUnpackingBindGroups::default();
        let mut allocated = UniformAllocationBindGroups::default();
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
        assert!(!cache.select_buffers(&a, &mut unpacked, &mut allocated, None));
        cache.needs_rebuild(a.clone(), true);
        assert!(!cache.select_buffers(&b, &mut unpacked, &mut allocated, None));
        assert_eq!(cache.0.as_ref(), Some(&a));
        assert_eq!(cache.3.as_ref().unwrap().signature, a);
        cache.needs_rebuild(b.clone(), true);
        for next in [&a, &b, &a, &b] {
            assert!(cache.select_buffers(next, &mut unpacked, &mut allocated, None));
            assert_eq!(cache.0.as_ref(), Some(next));
            cache.needs_rebuild(next.clone(), true);
        }
        assert!(!cache.select_buffers(&c, &mut unpacked, &mut allocated, None));
        assert_eq!(cache.0.as_ref(), Some(&b));
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

    fn view(index: u32) -> RetainedViewEntity {
        RetainedViewEntity::new(Entity::PLACEHOLDER.into(), None, index)
    }

    #[test]
    fn phase_reallocation_and_removal_only_invalidate_that_phases_views() {
        let opaque = TypeId::of::<u8>();
        let shadow = TypeId::of::<u16>();
        let mut previous = Snapshot::default();
        previous.phases.insert(opaque, vec![buffer(None)]);
        previous.phases.insert(shadow, vec![buffer(None)]);
        previous
            .work
            .insert((opaque, view(0)), vec![Binding::Count(1)]);
        previous
            .work
            .insert((shadow, view(1)), vec![Binding::Count(1)]);
        previous
            .work
            .insert((shadow, view(2)), vec![Binding::Count(1)]);
        let mut next = previous.clone();
        next.phases.get_mut(&opaque).unwrap()[0] = Binding::Buffer(Some(BufferId::new()), 0, None);
        assert_eq!(changed_views(&previous, &next), HashSet::from([view(0)]));
        next = previous.clone();
        next.phases.remove(&shadow);
        assert_eq!(
            changed_views(&previous, &next),
            HashSet::from([view(1), view(2)])
        );
    }

    #[test]
    fn new_phase_buffers_preserve_cached_unrelated_views_instead_of_starting_over() {
        let opaque = TypeId::of::<u8>();
        let shadow = TypeId::of::<u16>();
        let mut previous = Snapshot::default();
        previous.phases.insert(opaque, vec![buffer(None)]);
        previous.phases.insert(shadow, vec![buffer(None)]);
        previous
            .work
            .insert((opaque, view(0)), vec![Binding::Count(1)]);
        previous
            .work
            .insert((shadow, view(1)), vec![Binding::Count(1)]);
        let mut cache = Cache(Some(previous.clone()), 0, HashMap::new(), None);
        for id in [view(0), view(1)] {
            cache.2.insert(id, PreprocessBindGroups(Default::default()));
        }
        let mut next = previous.clone();
        next.phases.get_mut(&opaque).unwrap()[0] = Binding::Buffer(Some(BufferId::new()), 0, None);
        assert!(!cache.select_buffers(
            &next,
            &mut BinUnpackingBindGroups::default(),
            &mut UniformAllocationBindGroups::default(),
            None,
        ));
        assert_eq!(cache.0.as_ref(), Some(&previous));
        assert!(cache.2.contains_key(&view(1)));
        assert_eq!(
            changed_views(cache.0.as_ref().unwrap(), &next),
            HashSet::from([view(0)])
        );
        assert_eq!(cache.3.as_ref().unwrap().views.len(), 2);
    }

    #[test]
    fn indirect_job_binding_changes_need_only_one_indirect_view() {
        let phase = TypeId::of::<u8>();
        let mut previous = Snapshot::default();
        previous.build_global.push(buffer(None));
        for index in 0..8 {
            previous.views.insert(view(index));
            previous
                .work
                .insert((phase, view(index)), vec![Binding::Count(1)]);
        }
        let mut next = previous.clone();
        next.build_global[0] = Binding::Buffer(Some(BufferId::new()), 0, None);
        let mut required = changed_views(&previous, &next);
        assert!(required.is_empty());
        include_indirect_builder_view(&mut required, &next, true);
        assert_eq!(required.len(), 1);
        include_indirect_builder_view(&mut required, &next, true);
        assert_eq!(required.len(), 1);
        let mut no_build = HashSet::new();
        include_indirect_builder_view(&mut no_build, &next, false);
        assert!(no_build.is_empty());
    }

    #[test]
    fn build_and_unpack_buffers_keep_separate_bounded_history_entries() {
        for unpack in [false, true] {
            let mut cache = Cache::default();
            let mut unpacked = BinUnpackingBindGroups::default();
            let mut allocated = UniformAllocationBindGroups::default();
            let a = Snapshot::default();
            let mut b = a.clone();
            if unpack {
                b.unpack_global.push(buffer(None));
            } else {
                b.build_global.push(buffer(None));
            }
            assert!(!cache.select_buffers(&a, &mut unpacked, &mut allocated, None));
            cache.needs_rebuild(a.clone(), true);
            assert!(!cache.select_buffers(&b, &mut unpacked, &mut allocated, None));
            cache.needs_rebuild(b.clone(), true);
            assert!(cache.select_buffers(&a, &mut unpacked, &mut allocated, None));
            assert_eq!(cache.0.as_ref(), Some(&a));
            assert!(cache.select_buffers(&b, &mut unpacked, &mut allocated, None));
            assert_eq!(cache.0.as_ref(), Some(&b));
        }
    }

    #[test]
    fn unpack_metadata_buffers_only_invalidate_views_with_actual_jobs() {
        let phase = TypeId::of::<u8>();
        let mut previous = Snapshot::default();
        previous.unpack_global.push(buffer(None));
        previous
            .unpack
            .insert((phase, view(0)), vec![Binding::Count(0)]);
        previous.unpack.insert(
            (phase, view(1)),
            vec![Binding::Count(1), buffer(None), Binding::Count(12)],
        );
        let mut next = previous.clone();
        next.unpack_global[0] = Binding::Buffer(Some(BufferId::new()), 0, None);
        assert_eq!(changed_views(&previous, &next), HashSet::from([view(1)]));
    }

    #[test]
    fn counts_binding_ranges_and_metadata_offsets_remain_part_of_invalidation() {
        let phase = TypeId::of::<u8>();
        let id = view(0);
        let mut previous = Snapshot::default();
        previous
            .phases
            .insert(phase, vec![buffer(None), Binding::Count(8)]);
        previous.work.insert(
            (phase, id),
            vec![Binding::Count(1), buffer(None), Binding::Count(16)],
        );
        previous.unpack.insert(
            (phase, id),
            vec![
                Binding::Count(1),
                buffer(None),
                Binding::Count(256),
                Binding::Count(512),
                Binding::Count(8),
                Binding::Count(16),
            ],
        );
        let mut next = previous.clone();
        next.phases.get_mut(&phase).unwrap()[1] = Binding::Count(9);
        assert!(!phase_buffers_changed(&previous, &next));
        assert_ne!(
            previous.phases, next.phases,
            "sized metadata still rebuilds shared bindings"
        );
        for slot in [2, 3, 4, 5] {
            next = previous.clone();
            next.unpack.get_mut(&(phase, id)).unwrap()[slot] = Binding::Count(1024);
            assert_eq!(changed_views(&previous, &next), HashSet::from([id]));
        }
        next = previous.clone();
        next.work.get_mut(&(phase, id)).unwrap()[2] = Binding::Count(17);
        assert_eq!(changed_views(&previous, &next), HashSet::from([id]));
        next = previous.clone();
        next.work.get_mut(&(phase, id)).unwrap()[1] = Binding::Buffer(None, 16, Some(128));
        assert_eq!(changed_views(&previous, &next), HashSet::from([id]));
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
