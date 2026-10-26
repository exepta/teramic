//! Optional native GPU profiling. Values are published in Bevy's DiagnosticsStore.
use bevy::{
    app::{Main, SubApp},
    ecs::schedule::ScheduleLabel,
    prelude::*,
    render::{
        Render, RenderApp, RenderScheduleOrder, RenderStartup,
        diagnostic::RenderDiagnosticsPlugin,
        pipelined_rendering::RenderExtractApp,
        renderer::{RenderAdapterInfo, RenderDevice},
        settings::WgpuFeatures,
    },
};

mod stages;
mod systems;

#[derive(Resource, Default)]
struct MainWall(Option<terra_core::profiling::ScopeTimer>);

#[derive(Resource, Default)]
struct RenderWall(Option<terra_core::profiling::ScopeTimer>);

#[derive(ScheduleLabel, Clone, Debug, PartialEq, Eq, Hash)]
struct RenderWallStart;

#[derive(ScheduleLabel, Clone, Debug, PartialEq, Eq, Hash)]
struct RenderWallStop;

/// Independent of GPU query diagnostics. Timings include scheduler/driver waits,
/// not OS thread CPU usage, and overlapping main/render scopes are not additive.
pub(crate) fn install_cpu_schedules(app: &mut App) {
    if !std::env::var("TERRA_CPU_SCOPES").is_ok_and(|value| value == "1") {
        return;
    }
    app.init_resource::<MainWall>().add_systems(
        Main,
        (
            start_main.before(Main::run_main),
            stop_main.after(Main::run_main),
        ),
    );
    if let Some(render_app) = app.get_sub_app_mut(RenderApp) {
        install_render_schedules(render_app);
        stages::install(render_app);
        render_app.add_systems(RenderStartup, systems::install);
        wrap_extract(render_app, "cpu.extract_wall");
    }
    // The pipelined callback waits for the render world before invoking its
    // extractor. Keep this separate from the actual extraction measured above.
    if let Some(handoff) = app.get_sub_app_mut(RenderExtractApp) {
        wrap_extract(handoff, "cpu.render_handoff_wall");
    }
}

fn start_main(mut timer: ResMut<MainWall>) {
    timer.0 = Some(terra_core::profiling::ScopeTimer::new(
        "cpu.main_schedule_wall",
    ));
}

fn stop_main(mut timer: ResMut<MainWall>) {
    drop(timer.0.take());
}

fn install_render_schedules(render_app: &mut SubApp) {
    render_app
        .init_resource::<RenderWall>()
        .add_systems(RenderWallStart, start_render)
        .add_systems(RenderWallStop, stop_render);
    let mut order = render_app.world_mut().resource_mut::<RenderScheduleOrder>();
    order.insert_before(First, RenderWallStart);
    order.insert_after(Render, RenderWallStop);
}

fn start_render(mut timer: ResMut<RenderWall>) {
    timer.0 = Some(terra_core::profiling::ScopeTimer::new(
        "cpu.render_schedule_wall",
    ));
}

fn stop_render(mut timer: ResMut<RenderWall>) {
    drop(timer.0.take());
}

fn wrap_extract(app: &mut SubApp, name: &'static str) {
    if let Some(mut extract) = app.take_extract() {
        app.set_extract(move |main, render| {
            let _scope = terra_core::profiling::ScopeTimer::new(name);
            extract(main, render);
        });
    }
}

pub struct TerraRenderDiagnosticsPlugin;

impl Plugin for TerraRenderDiagnosticsPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(RenderDiagnosticsPlugin);
        if let Some(render_app) = app.get_sub_app_mut(RenderApp) {
            render_app.add_systems(RenderStartup, report_capabilities);
        }
    }
}

fn report_capabilities(device: Res<RenderDevice>, adapter: Res<RenderAdapterInfo>) {
    let features = device.features();
    info!(
        adapter = %adapter.name,
        backend = ?adapter.backend,
        timestamps = features.contains(WgpuFeatures::TIMESTAMP_QUERY),
        pass_timestamps = features.contains(WgpuFeatures::TIMESTAMP_QUERY_INSIDE_PASSES),
        encoder_timestamps = features.contains(WgpuFeatures::TIMESTAMP_QUERY_INSIDE_ENCODERS),
        pipeline_statistics = features.contains(WgpuFeatures::PIPELINE_STATISTICS_QUERY),
        "Render diagnostics enabled; GPU pass timings are milliseconds, not total frame latency; TERRA_DRAW_DIAGNOSTICS enables separate PBR draw API counters"
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Resource, Default)]
    struct Observed(Vec<&'static str>);

    #[test]
    fn main_wall_scope_brackets_the_actual_main_schedule_and_releases_afterwards() {
        let mut app = App::new();
        app.init_resource::<MainWall>()
            .init_resource::<Observed>()
            .add_systems(
                Main,
                (
                    start_main.before(Main::run_main),
                    stop_main.after(Main::run_main),
                ),
            )
            .add_systems(
                Update,
                |timer: Res<MainWall>, mut seen: ResMut<Observed>| {
                    assert!(timer.0.is_some());
                    seen.0.push("main");
                },
            );
        for expected in 1..=3 {
            app.update();
            assert!(app.world().resource::<MainWall>().0.is_none());
            assert_eq!(app.world().resource::<Observed>().0.len(), expected);
        }
    }

    #[test]
    fn render_wall_scope_includes_first_prepare_and_render_without_gpu_diagnostics() {
        let mut render = SubApp::new();
        render
            .init_resource::<RenderScheduleOrder>()
            .init_resource::<Observed>()
            .add_systems(
                First,
                |timer: Res<RenderWall>, mut seen: ResMut<Observed>| {
                    assert!(timer.0.is_some());
                    seen.0.push("first");
                },
            )
            .add_systems(
                Render,
                |timer: Res<RenderWall>, mut seen: ResMut<Observed>| {
                    assert!(timer.0.is_some());
                    seen.0.push("render");
                },
            );
        install_render_schedules(&mut render);
        let labels = render
            .world()
            .resource::<RenderScheduleOrder>()
            .labels
            .clone();
        for label in labels {
            render.world_mut().run_schedule(label);
        }
        assert!(render.world().resource::<RenderWall>().0.is_none());
        assert_eq!(render.world().resource::<Observed>().0, ["first", "render"]);
        assert!(
            !render
                .world()
                .contains_resource::<bevy::diagnostic::DiagnosticsStore>()
        );
    }

    #[test]
    fn timed_extractor_preserves_the_original_callback_exactly_once_per_frame() {
        let mut main = World::new();
        main.init_resource::<Observed>();
        let mut render = SubApp::new();
        render.init_resource::<Observed>();
        render.set_extract(|main, render| {
            main.resource_mut::<Observed>().0.push("main");
            render.resource_mut::<Observed>().0.push("render");
        });
        wrap_extract(&mut render, "cpu.extract_wall");
        for expected in 1..=3 {
            render.extract(&mut main);
            assert_eq!(main.resource::<Observed>().0.len(), expected);
            assert_eq!(render.world().resource::<Observed>().0.len(), expected);
        }
    }
}
