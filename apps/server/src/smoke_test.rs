use bevy::{app::AppExit, prelude::*};
use std::time::{Duration, Instant};
use terra_world::streaming::{GenerationStatus, WorldGeneration};
#[derive(Resource)]
struct SmokeTest(Instant);
pub(crate) fn install(app: &mut App) {
    if std::env::args().any(|arg| arg == "--smoke-test") {
        app.insert_resource(SmokeTest(Instant::now()))
            .add_systems(Update, smoke_test);
    }
}
fn smoke_test(
    test: Res<SmokeTest>,
    world: Res<WorldGeneration>,
    server: Res<terra_network::server::ServerRuntime>,
    mut exit: MessageWriter<AppExit>,
) {
    if let Some(error) = &server.error {
        tracing::error!(%error,"Server transport failed");
        exit.write(AppExit::error());
        return;
    }
    match &world.status {
        GenerationStatus::Ready => {
            tracing::info!(
                sections = world.loaded_sections,
                "Headless server smoke test completed"
            );
            exit.write(AppExit::Success);
        }
        GenerationStatus::Failed(error) => {
            tracing::error!(%error,"Smoke test failed");
            exit.write(AppExit::error());
        }
        GenerationStatus::Pending if test.0.elapsed() > Duration::from_secs(30) => {
            exit.write(AppExit::error());
        }
        _ => {}
    }
}
