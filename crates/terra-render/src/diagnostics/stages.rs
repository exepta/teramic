//! Opt-in wall intervals locate render preparation stalls without GPU readbacks.
use bevy::{
    app::SubApp,
    core_pipeline::schedule::{Core3d, Core3dSystems},
    prelude::*,
    render::{
        Render, RenderSystems,
        mesh::{RenderMesh, allocator::allocate_and_free_meshes},
        render_asset::prepare_assets,
        renderer::{RenderGraph, RenderGraphSystems},
    },
};

#[derive(Resource, Default)]
struct Interval<const N: usize>(Option<terra_core::profiling::ScopeTimer>);

const NAMES: [&str; 19] = [
    "render.assets",
    "render.meshes",
    "render.create_views",
    "render.specialize",
    "render.views",
    "render.queue",
    "render.sort",
    "render.resources",
    "render.bind_groups",
    "render.execute",
    "render.cleanup",
    "render.mesh_allocator",
    "render.prepare_mesh_assets",
    "render.preprocess_bind_groups",
    "render.mesh_bind_groups",
    "render.graph_submit",
    "render.camera_prepass",
    "render.camera_mainpass",
    "render.camera_postprocess",
];

fn start<const N: usize>(mut timer: ResMut<Interval<N>>) {
    timer.0 = Some(terra_core::profiling::ScopeTimer::new(NAMES[N]));
}

fn stop<const N: usize>(mut timer: ResMut<Interval<N>>) {
    drop(timer.0.take());
}

pub(super) fn install(app: &mut SubApp) {
    macro_rules! measure {
        ($n:literal, $previous:ident, $set:ident, $next:ident) => {
            app.init_resource::<Interval<$n>>().add_systems(
                Render,
                (
                    start::<$n>
                        .after(RenderSystems::$previous)
                        .before(RenderSystems::$set),
                    stop::<$n>
                        .after(RenderSystems::$set)
                        .before(RenderSystems::$next),
                ),
            );
        };
    }
    measure!(0, ExtractCommands, PrepareAssets, PrepareMeshes);
    measure!(1, PrepareAssets, PrepareMeshes, CreateViews);
    measure!(2, PrepareMeshes, CreateViews, Specialize);
    measure!(3, CreateViews, Specialize, PrepareViews);
    measure!(4, Specialize, PrepareViews, Queue);
    measure!(5, PrepareViews, Queue, PhaseSort);
    measure!(6, Queue, PhaseSort, Prepare);
    measure!(7, PhaseSort, PrepareResources, PrepareResourcesBatchPhases);
    measure!(8, PrepareResourcesFlush, PrepareBindGroups, Render);
    measure!(9, Prepare, Render, Cleanup);
    measure!(10, Render, Cleanup, PostCleanup);
    macro_rules! system {
        ($n:literal, $set:ident, $system:expr) => {
            app.init_resource::<Interval<$n>>().add_systems(
                Render,
                (start::<$n>.before($system), stop::<$n>.after($system))
                    .in_set(RenderSystems::$set),
            );
        };
    }
    system!(11, PrepareAssets, allocate_and_free_meshes);
    system!(12, PrepareAssets, prepare_assets::<RenderMesh>);
    system!(
        13,
        PrepareBindGroups,
        bevy::pbr::prepare_preprocess_bind_groups
    );
    system!(14, PrepareBindGroups, bevy::pbr::prepare_mesh_bind_groups);
    app.init_resource::<Interval<15>>().add_systems(
        RenderGraph,
        (
            start::<15>
                .after(RenderGraphSystems::Render)
                .before(RenderGraphSystems::Submit),
            stop::<15>
                .after(RenderGraphSystems::Submit)
                .before(RenderGraphSystems::Finish),
        ),
    );
    macro_rules! view {
        ($n:literal, $set:ident) => {
            app.init_resource::<Interval<$n>>().add_systems(
                Core3d,
                (
                    start::<$n>.before(Core3dSystems::$set),
                    stop::<$n>.after(Core3dSystems::$set),
                ),
            );
        };
    }
    view!(16, Prepass);
    view!(17, MainPass);
    view!(18, PostProcess);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn intervals_enclose_existing_render_stages_and_release_every_frame() {
        let mut app = SubApp::new();
        install(&mut app);
        app.add_systems(
            Render,
            (|timer: Res<Interval<9>>| {
                assert!(timer.0.is_some());
            })
            .in_set(RenderSystems::Render),
        );
        for _ in 0..3 {
            app.world_mut().run_schedule(Render);
            assert!(app.world().resource::<Interval<9>>().0.is_none());
        }
    }
}
