//! Core3d runs once per camera and per point-shadow face. Its mostly ordered,
//! short view systems do not benefit from waking the ECS worker pool each time.
use bevy::{
    core_pipeline::schedule::Core3d, ecs::schedule::SingleThreadedExecutor, prelude::*,
    render::RenderApp,
};

pub(crate) fn install(app: &mut App) {
    let reference = std::env::args().any(|arg| arg.starts_with("--smoke-test"))
        && std::env::var("TERRA_CORE3D_EXECUTOR").is_ok_and(|mode| mode == "parallel");
    if !reference && let Some(render) = app.get_sub_app_mut(RenderApp) {
        render.edit_schedule(Core3d, |schedule| {
            schedule.set_executor(SingleThreadedExecutor::new());
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::app::SubApp;

    #[derive(Resource, Default)]
    struct Seen(Vec<u8>);

    #[test]
    fn view_executor_keeps_ordered_systems_and_runs_every_view_without_a_gpu() {
        let mut app = App::new();
        let mut render = SubApp::new();
        render.init_resource::<Seen>().add_systems(
            Core3d,
            (
                |mut seen: ResMut<Seen>| seen.0.push(1),
                |mut seen: ResMut<Seen>| seen.0.push(2),
            )
                .chain(),
        );
        app.insert_sub_app(RenderApp, render);
        install(&mut app);
        for _ in 0..30 {
            app.sub_app_mut(RenderApp).world_mut().run_schedule(Core3d);
        }
        let render = app.sub_app(RenderApp);
        assert_eq!(render.world().resource::<Seen>().0, [1, 2].repeat(30));
        assert!(
            app.get_schedule(Core3d).is_none(),
            "main schedules are untouched"
        );
    }
}
