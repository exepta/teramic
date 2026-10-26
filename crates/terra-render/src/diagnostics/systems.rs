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

struct Timed<const N: usize>;
const NAMES: [&str; 5] = [
    "render.mesh_allocator.body",
    "render.prepare_mesh_assets.body",
    "render.preprocess_bind_groups.body",
    "render.mesh_bind_groups.body",
    "render.shadow_bin_uploads.body",
];
impl<A: System<In = ()>, B: System<In = (), Out = ()>, const N: usize> Combine<A, B> for Timed<N> {
    type In = ();
    type Out = A::Out;
    fn combine<T>(
        _: (),
        data: &mut T,
        a: impl FnOnce(SystemIn<'_, A>, &mut T) -> Result<A::Out, RunSystemError>,
        _: impl FnOnce(SystemIn<'_, B>, &mut T) -> Result<(), RunSystemError>,
    ) -> Result<Self::Out, RunSystemError> {
        let _scope = terra_core::profiling::ScopeTimer::new(NAMES[N]);
        a((), data)
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
        // Preprocessing's cache measures its real misses itself. Replacing that
        // wrapper here would inadvertently disable reuse while profiling.
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
    });
}

#[cfg(test)]
mod tests {
    use super::*;
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
