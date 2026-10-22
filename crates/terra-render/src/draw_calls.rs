//! Counting at successful draw commands; optional bounded GPU command readback.
mod gpu;
use bevy::{
    core_pipeline::core_3d::{AlphaMask3d, Opaque3d, Transparent3d},
    core_pipeline::deferred::{AlphaMask3dDeferred, Opaque3dDeferred},
    core_pipeline::prepass::{AlphaMask3dPrepass, Opaque3dPrepass},
    ecs::{
        query::ROQueryItem,
        system::{ReadOnlySystemParam, SystemParamItem, lifetimeless::SRes},
    },
    pbr::{DrawDepthOnlyPrepass, DrawMaterial, DrawPrepass, Shadow, Transmissive3d},
    prelude::*,
    render::{
        Render, RenderApp, RenderStartup, RenderSystems,
        render_phase::{
            DrawFunctions, PhaseItem, PhaseItemExtraIndex, RenderCommand, RenderCommandResult,
            RenderCommandState, TrackedRenderPass,
        },
    },
};
pub use gpu::GpuSubdrawSnapshot;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, AtomicU64, Ordering},
};

#[derive(Clone, Debug, Default)]
pub struct DrawCallSnapshot {
    /// Actual successful draw API submissions, by phase: opaque, mask,
    /// transparent, transmissive, opaque/mask prepass, opaque/mask deferred, shadow.
    pub phase_api_calls: [u64; 9],
    pub direct_calls: u64,
    pub indirect_calls: u64,
    /// GPU-decided multi-draw counts are deliberately not guessed from meshes.
    pub gpu_counted_calls: u64,
    /// Successful shared UI commands: nodes/text, sliced images, shadows.
    /// Bevy gradients, private Tilt materials and engine-internal fullscreen
    /// passes have no public draw hooks and are deliberately not included.
    pub ui_api_calls: [u64; 3],
    /// Actual submissions from Teramic's own cave/underwater/smoke fullscreen pass.
    /// This is deliberately not reported as the engine's complete fullscreen count.
    pub fullscreen_api_calls: u64,
    /// Delayed nine-phase readback. The legacy field name is retained for callers.
    pub gpu_color_subdraws: Option<GpuSubdrawSnapshot>,
}

#[derive(Default)]
struct Counters {
    enabled: AtomicBool,
    phases: [AtomicU64; 9],
    direct: AtomicU64,
    indirect: AtomicU64,
    gpu_counted: AtomicU64,
    ui: [AtomicU64; 3],
    fullscreen: AtomicU64,
    last: Mutex<Option<DrawCallSnapshot>>,
}

#[derive(Resource, Clone, Default)]
pub struct DrawCallStatus(Arc<Counters>);

impl DrawCallStatus {
    pub fn snapshot(&self) -> Option<DrawCallSnapshot> {
        self.0.last.lock().ok().and_then(|last| last.clone())
    }

    fn record(&self, phase: usize, indirect: bool, gpu_counted: bool) {
        self.0.phases[phase].fetch_add(1, Ordering::Relaxed);
        if indirect {
            &self.0.indirect
        } else {
            &self.0.direct
        }
        .fetch_add(1, Ordering::Relaxed);
        if gpu_counted {
            self.0.gpu_counted.fetch_add(1, Ordering::Relaxed);
        }
    }

    pub(crate) fn fullscreen_drawn(&self) {
        if self.0.enabled.load(Ordering::Relaxed) {
            self.0.fullscreen.fetch_add(1, Ordering::Relaxed);
        }
    }
}

pub(crate) fn install(app: &mut App) {
    let status = DrawCallStatus::default();
    status.0.enabled.store(
        std::env::var_os("TERRA_DRAW_DIAGNOSTICS").is_some()
            || std::env::var_os("TERRA_GPU_DRAW_DIAGNOSTICS").is_some(),
        Ordering::Relaxed,
    );
    app.insert_resource(status.clone());
    if let Some(render) = app.get_sub_app_mut(RenderApp) {
        render
            .insert_resource(status)
            .add_systems(RenderStartup, instrument)
            .add_systems(Render, finish.in_set(RenderSystems::Cleanup));
        if std::env::var_os("TERRA_GPU_DRAW_DIAGNOSTICS").is_some() {
            render
                .insert_resource(
                    bevy::render::batching::gpu_preprocessing::IndirectParametersBuffersSettings {
                        allow_copies_from_indirect_parameter_buffers: true,
                    },
                )
                .init_resource::<gpu::Collector>()
                .add_systems(
                    Render,
                    gpu::readback.in_set(RenderSystems::Cleanup).before(finish),
                )
                .add_systems(
                    bevy::core_pipeline::schedule::Core3d,
                    (
                        gpu::capture_early
                            .after(bevy::core_pipeline::prepass::node::early_prepass)
                            // Bevy orders this public boundary after its private
                            // early-deferred pass and after early shadow passes.
                            .after(bevy::core_pipeline::mip_generation::experimental::depth::early_downsample_depth)
                            .after(bevy::pbr::per_view_shadow_pass::<false>)
                            .after(bevy::pbr::shared_shadow_pass::<false>)
                            .before(bevy::pbr::late_prepass_build_indirect_parameters),
                        gpu::capture_late
                            .after(bevy::core_pipeline::prepass::node::late_prepass)
                            .after(bevy::core_pipeline::deferred::node::late_deferred_prepass)
                            .after(bevy::pbr::per_view_shadow_pass::<true>)
                            .after(bevy::pbr::shared_shadow_pass::<true>)
                            .before(bevy::pbr::main_build_indirect_parameters),
                        gpu::capture_color
                            .after(bevy::core_pipeline::schedule::Core3dSystems::MainPass)
                            .before(bevy::core_pipeline::schedule::Core3dSystems::EarlyPostProcess),
                    ),
                );
        }
    }
}

struct Counted<C, const PHASE: usize>(std::marker::PhantomData<C>);

impl<P: PhaseItem, C: RenderCommand<P>, const PHASE: usize> RenderCommand<P> for Counted<C, PHASE> {
    type Param = (
        C::Param,
        SRes<DrawCallStatus>,
        SRes<crate::transitions::TransitionDraws>,
        Option<SRes<gpu::Collector>>,
        SRes<bevy::pbr::RenderMeshInstances>,
        SRes<bevy::render::render_asset::RenderAssets<bevy::render::mesh::RenderMesh>>,
        SRes<bevy::render::batching::gpu_preprocessing::IndirectParametersBuffers>,
    );
    type ViewQuery = C::ViewQuery;
    type ItemQuery = C::ItemQuery;

    fn render<'w>(
        item: &P,
        view: ROQueryItem<'w, '_, Self::ViewQuery>,
        entity: Option<ROQueryItem<'w, '_, Self::ItemQuery>>,
        (param, status, transitions, gpu, instances, meshes, buffers): SystemParamItem<
            'w,
            '_,
            Self::Param,
        >,
        pass: &mut TrackedRenderPass<'w>,
    ) -> RenderCommandResult {
        let result = C::render(item, view, entity, param, pass);
        // These registered command tuples end in DrawMesh. Success means exactly
        // one direct/indirect API submission; Skip and Failure submit nothing.
        if matches!(result, RenderCommandResult::Success) {
            if PHASE < 4 {
                transitions.drawn(item.main_entity().id());
            }
            let (indirect, counted) = match item.extra_index() {
                PhaseItemExtraIndex::IndirectParametersIndex {
                    batch_set_index, ..
                } => (true, batch_set_index.is_some()),
                _ => (false, false),
            };
            if status.0.enabled.load(Ordering::Relaxed) {
                status.record(PHASE, indirect, counted);
            }
            if let Some(gpu) = gpu {
                gpu.record(PHASE, item, &instances, &meshes, &buffers);
            }
        }
        result
    }
}

fn replace<P, C, const PHASE: usize>(world: &mut World)
where
    P: PhaseItem,
    C: RenderCommand<P> + Send + Sync + 'static,
    C::Param: ReadOnlySystemParam,
{
    let Some(id) = world
        .get_resource::<DrawFunctions<P>>()
        .and_then(|f| f.read().get_id::<C>())
    else {
        return;
    };
    let counted = RenderCommandState::<P, Counted<C, PHASE>>::new(world);
    let functions = world.resource::<DrawFunctions<P>>();
    if let Some(slot) = functions.write().draw_functions.get_mut(id.0 as usize) {
        *slot = Box::new(counted);
    }
}

fn instrument(world: &mut World) {
    replace::<Opaque3d, DrawMaterial, 0>(world);
    replace::<AlphaMask3d, DrawMaterial, 1>(world);
    replace::<Transparent3d, DrawMaterial, 2>(world);
    replace::<Transmissive3d, DrawMaterial, 3>(world);
    replace::<Opaque3dPrepass, DrawPrepass, 4>(world);
    replace::<Opaque3dPrepass, DrawDepthOnlyPrepass, 4>(world);
    replace::<AlphaMask3dPrepass, DrawPrepass, 5>(world);
    replace::<Opaque3dDeferred, DrawPrepass, 6>(world);
    replace::<AlphaMask3dDeferred, DrawPrepass, 7>(world);
    replace::<Shadow, DrawPrepass, 8>(world);
    replace::<Shadow, DrawDepthOnlyPrepass, 8>(world);
    instrument_ui::<bevy::ui_render::DrawUi, 0>(world);
    instrument_ui::<bevy::ui_render::ui_texture_slice_pipeline::DrawUiTextureSlices, 1>(world);
    instrument_ui::<bevy::ui_render::box_shadow::DrawBoxShadows, 2>(world);
}

struct CountedUi<C, const KIND: usize>(std::marker::PhantomData<C>);
impl<C: RenderCommand<bevy::ui_render::TransparentUi>, const KIND: usize>
    RenderCommand<bevy::ui_render::TransparentUi> for CountedUi<C, KIND>
{
    type Param = (C::Param, SRes<DrawCallStatus>);
    type ViewQuery = C::ViewQuery;
    type ItemQuery = C::ItemQuery;
    fn render<'w>(
        item: &bevy::ui_render::TransparentUi,
        view: ROQueryItem<'w, '_, Self::ViewQuery>,
        entity: Option<ROQueryItem<'w, '_, Self::ItemQuery>>,
        (param, status): SystemParamItem<'w, '_, Self::Param>,
        pass: &mut TrackedRenderPass<'w>,
    ) -> RenderCommandResult {
        let result = C::render(item, view, entity, param, pass);
        if matches!(result, RenderCommandResult::Success)
            && status.0.enabled.load(Ordering::Relaxed)
        {
            status.0.ui[KIND].fetch_add(1, Ordering::Relaxed);
        }
        result
    }
}

fn instrument_ui<C, const KIND: usize>(world: &mut World)
where
    C: RenderCommand<bevy::ui_render::TransparentUi> + Send + Sync + 'static,
    C::Param: ReadOnlySystemParam,
{
    let Some(id) = world
        .get_resource::<DrawFunctions<bevy::ui_render::TransparentUi>>()
        .and_then(|f| f.read().get_id::<C>())
    else {
        return;
    };
    let counted =
        RenderCommandState::<bevy::ui_render::TransparentUi, CountedUi<C, KIND>>::new(world);
    if let Some(slot) = world
        .resource::<DrawFunctions<bevy::ui_render::TransparentUi>>()
        .write()
        .draw_functions
        .get_mut(id.0 as usize)
    {
        *slot = Box::new(counted);
    }
}

fn finish(status: Res<DrawCallStatus>, gpu: Option<Res<gpu::Collector>>) {
    if !status.0.enabled.load(Ordering::Relaxed) {
        return;
    }
    let snapshot = DrawCallSnapshot {
        phase_api_calls: std::array::from_fn(|i| status.0.phases[i].swap(0, Ordering::Relaxed)),
        direct_calls: status.0.direct.swap(0, Ordering::Relaxed),
        indirect_calls: status.0.indirect.swap(0, Ordering::Relaxed),
        gpu_counted_calls: status.0.gpu_counted.swap(0, Ordering::Relaxed),
        ui_api_calls: std::array::from_fn(|i| status.0.ui[i].swap(0, Ordering::Relaxed)),
        fullscreen_api_calls: status.0.fullscreen.swap(0, Ordering::Relaxed),
        gpu_color_subdraws: gpu
            .and_then(|collector| collector.last.lock().ok().and_then(|last| last.clone())),
    };
    if let Ok(mut last) = status.0.last.lock() {
        *last = Some(snapshot);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn submissions_are_not_confused_with_indirect_subdraws_or_meshes() {
        let status = DrawCallStatus::default();
        status.fullscreen_drawn();
        status.0.enabled.store(true, Ordering::Relaxed);
        assert!(status.snapshot().is_none());
        status.record(0, false, false);
        status.record(1, true, true);
        status.record(8, true, false);
        status.fullscreen_drawn();
        let mut app = App::new();
        app.insert_resource(status.clone())
            .add_systems(Update, finish);
        app.update();
        let sample = status.snapshot().unwrap();
        assert_eq!(sample.phase_api_calls.iter().sum::<u64>(), 3);
        assert_eq!(sample.fullscreen_api_calls, 1);
        assert_eq!(
            (
                sample.direct_calls,
                sample.indirect_calls,
                sample.gpu_counted_calls
            ),
            (1, 2, 1)
        );
        app.update();
        assert_eq!(status.snapshot().unwrap().phase_api_calls, [0; 9]);
        assert_eq!(status.snapshot().unwrap().fullscreen_api_calls, 0);
    }
}
