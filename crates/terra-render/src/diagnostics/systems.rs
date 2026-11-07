//! Exact system bodies, retaining their public function sets and resource access.
use bevy::{
    ecs::{
        schedule::ScheduleCleanupPolicy,
        system::{CombinatorSystem, Combine, RunSystemError, SystemIn},
    },
    prelude::*,
    render::{
        Render, RenderSystems,
        mesh::{RenderMesh, allocator::allocate_and_free_meshes},
        render_asset::prepare_assets,
        texture::GpuImage,
    },
};
use std::{
    sync::LazyLock,
    time::{Duration, Instant},
};

struct Timed<const N: usize>;
const NAMES: [&str; 8] = [
    "render.mesh_allocator.body",
    "render.prepare_mesh_assets.body",
    "render.preprocess_bind_groups.body",
    "render.mesh_bind_groups.body",
    "render.shadow_bin_uploads.body",
    "render.prepare_lights.body",
    "render.prepare_windows.body",
    "render.prepare_view_targets.body",
];

pub(super) fn stall_start() -> Option<Instant> {
    static ENABLED: LazyLock<bool> =
        LazyLock::new(|| std::env::var("TERRA_STALL_TRACE").is_ok_and(|value| value == "1"));
    ENABLED.then(Instant::now)
}

fn stalled_ms(elapsed: Option<Duration>) -> Option<f64> {
    elapsed
        .filter(|elapsed| *elapsed >= Duration::from_millis(32))
        .map(|elapsed| elapsed.as_secs_f64() * 1000.0)
}

pub(super) fn report_stall(scope: &'static str, elapsed: Option<Duration>) {
    if let Some(elapsed_ms) = stalled_ms(elapsed) {
        warn!(
            scope,
            elapsed_ms, "Render wall-time stall; not a GPU-time attribution"
        );
    }
}

impl<A: System<In = ()>, B: System<In = (), Out = ()>, const N: usize> Combine<A, B> for Timed<N> {
    type In = ();
    type Out = A::Out;
    fn combine<T>(
        _: (),
        data: &mut T,
        a: impl FnOnce(SystemIn<'_, A>, &mut T) -> Result<A::Out, RunSystemError>,
        _: impl FnOnce(SystemIn<'_, B>, &mut T) -> Result<(), RunSystemError>,
    ) -> Result<Self::Out, RunSystemError> {
        let started = stall_start();
        let scope = terra_core::profiling::ScopeTimer::new(NAMES[N]);
        let result = a((), data);
        let elapsed = started.map(|started| started.elapsed());
        drop(scope);
        report_stall(NAMES[N], elapsed);
        result
    }
}

fn timed<const N: usize, S: IntoSystem<(), (), M>, M>(system: S) -> impl System<In = (), Out = ()> {
    CombinatorSystem::<Timed<N>, _, _>::new(
        IntoSystem::into_system(system),
        IntoSystem::into_system(|| {}),
        NAMES[N].into(),
    )
}

pub(super) fn install(world: &mut World) {
    world.schedule_scope(Render, |world, schedule| {
        macro_rules! probe {
            ($system:expr, $replacement:expr) => {
                match schedule.remove_systems_in_set(
                    $system,
                    world,
                    ScheduleCleanupPolicy::RemoveSystemsOnly,
                ) {
                    Ok(1) => {
                        schedule.add_systems($replacement);
                    }
                    result => {
                        warn!(
                            ?result,
                            "Exact render probe could not replace its one target"
                        );
                    }
                }
            };
        }
        // Replacement systems keep both their function sets and the original
        // node-level dependencies/conditions; default sets alone are not enough.
        probe!(
            allocate_and_free_meshes,
            timed::<0, _, _>(allocate_and_free_meshes)
                .in_set(RenderSystems::PrepareAssets)
                .before(prepare_assets::<RenderMesh>)
        );
        probe!(
            prepare_assets::<RenderMesh>,
            timed::<1, _, _>(prepare_assets::<RenderMesh>)
                .in_set(RenderSystems::PrepareAssets)
                .after(prepare_assets::<GpuImage>)
        );
        // Stock is the production path. The optional cache times its own misses
        // and must not have that wrapper replaced while profiling.
        if !crate::preprocess_cache::experiment_enabled() {
            probe!(
                bevy::pbr::prepare_preprocess_bind_groups,
                timed::<2, _, _>(bevy::pbr::prepare_preprocess_bind_groups)
                    .in_set(RenderSystems::PrepareBindGroups)
                    .after(bevy::pbr::prepare_preprocess_pipelines)
                    .run_if(
                        resource_exists::<
                            bevy::render::batching::gpu_preprocessing::BatchedInstanceBuffers<
                                bevy::pbr::MeshUniform,
                                bevy::pbr::MeshInputUniform,
                            >,
                        >
                    )
            );
        }
        probe!(
            bevy::pbr::prepare_mesh_bind_groups,
            timed::<3, _, _>(bevy::pbr::prepare_mesh_bind_groups)
                .in_set(RenderSystems::PrepareBindGroups)
        );
        probe!(
            bevy::render::batching::gpu_preprocessing::write_binned_instance_buffers::<
                bevy::pbr::Shadow,
                bevy::pbr::MeshPipeline,
            >,
            timed::<4, _, _>(
                bevy::render::batching::gpu_preprocessing::write_binned_instance_buffers::<
                    bevy::pbr::Shadow,
                    bevy::pbr::MeshPipeline,
                >
            )
            .in_set(RenderSystems::PrepareResourcesWritePhaseBuffers)
            .run_if(
                resource_exists::<
                    bevy::render::batching::gpu_preprocessing::BatchedInstanceBuffers<
                        bevy::pbr::MeshUniform,
                        bevy::pbr::MeshInputUniform,
                    >,
                >
            )
        );
        probe!(
            bevy::pbr::prepare_lights,
            timed::<5, _, _>(bevy::pbr::prepare_lights)
                .in_set(RenderSystems::CreateViews)
                .after(bevy::render::camera::sort_cameras)
        );
        probe!(
            bevy::render::view::window::prepare_windows,
            timed::<6, _, _>(bevy::render::view::window::prepare_windows)
                .in_set(RenderSystems::PrepareViews)
        );
        probe!(
            bevy::render::view::prepare_view_targets,
            timed::<7, _, _>(bevy::render::view::prepare_view_targets)
                .in_set(RenderSystems::PrepareViews)
                .after(bevy::render::view::window::prepare_windows)
                .after(prepare_assets::<GpuImage>)
                .ambiguous_with(bevy::render::camera::sort_cameras)
        );
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stall_threshold_requires_one_enabled_call_of_at_least_thirty_two_ms() {
        assert_eq!(stalled_ms(None), None);
        assert_eq!(stalled_ms(Some(Duration::ZERO)), None);
        assert_eq!(stalled_ms(Some(Duration::from_nanos(31_999_999))), None);
        assert_eq!(stalled_ms(Some(Duration::from_millis(32))), Some(32.0));
        assert_eq!(stalled_ms(Some(Duration::from_millis(100))), Some(100.0));
    }
    #[derive(Resource, Default)]
    struct Calls(Vec<u8>);
    #[derive(Component)]
    struct Created;
    fn body(mut calls: ResMut<Calls>, mut commands: Commands) {
        calls.0.push(1);
        commands.spawn(Created);
    }
    #[test]
    fn wrapper_retains_function_ordering_and_deferred_commands() {
        let mut app = App::new();
        app.init_resource::<Calls>().add_systems(
            Update,
            (
                timed::<0, _, _>(body),
                (|mut calls: ResMut<Calls>| calls.0.push(2)).after(body),
            ),
        );
        for i in 1..=3 {
            app.update();
            assert_eq!(app.world().resource::<Calls>().0, [1, 2].repeat(i));
            assert_eq!(
                app.world_mut()
                    .query::<&Created>()
                    .iter(app.world())
                    .count(),
                i
            );
        }
    }
}
