use bevy::{app::AppExit, prelude::*};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
#[derive(Resource)]
struct Shutdown(Arc<AtomicBool>);
pub(crate) fn install(app: &mut App) -> Result<(), ctrlc::Error> {
    let shutdown = Arc::new(AtomicBool::new(false));
    let signal = Arc::clone(&shutdown);
    ctrlc::set_handler(move || signal.store(true, Ordering::Relaxed))?;
    app.insert_resource(Shutdown(shutdown))
        .add_systems(Update, shutdown_on_signal);
    Ok(())
}
fn shutdown_on_signal(signal: Res<Shutdown>, mut exit: MessageWriter<AppExit>) {
    if signal.0.load(Ordering::Relaxed) {
        tracing::info!("Shutdown signal received; exiting ECS loop");
        exit.write(AppExit::Success);
    }
}
