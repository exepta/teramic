//! Frame-paced cold load, movement and eviction benchmark, without a GPU.
use bevy::prelude::*;
use std::time::{Duration, Instant};
use terra_core::{GameConfig, TerraCorePlugin};
use terra_world::{
    TerraWorldPlugin, WorldMode,
    streaming::{ChunkFocus, GenerationStatus, WorldGeneration},
};

fn settle(app: &mut App, label: &str) -> Result<(), String> {
    let start = Instant::now();
    let mut frames = 0;
    loop {
        let frame = Instant::now();
        app.update();
        frames += 1;
        let generation = app.world().resource::<WorldGeneration>();
        if let GenerationStatus::Failed(error) = &generation.status {
            return Err(error.clone());
        }
        if generation.status == GenerationStatus::Ready {
            println!(
                "{label}: sections={}, frames={frames}, seconds={:.3}",
                generation.loaded_sections,
                start.elapsed().as_secs_f64()
            );
            return Ok(());
        }
        if start.elapsed() > Duration::from_secs(90) {
            return Err(format!("{label}: timed out"));
        }
        std::thread::sleep(Duration::from_secs_f64(1.0 / 60.0).saturating_sub(frame.elapsed()));
    }
}
fn main() -> Result<(), String> {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_plugins(TerraCorePlugin(GameConfig::default()))
        .add_plugins(TerraWorldPlugin {
            mode: WorldMode::Client,
        });
    app.finish();
    app.cleanup();
    settle(&mut app, "cold")?;
    let base = app.world().resource::<ChunkFocus>().0;
    app.world_mut().resource_mut::<ChunkFocus>().0 = [base[0] + 16.0, base[1], base[2]];
    settle(&mut app, "one-column move")?;
    app.world_mut().resource_mut::<ChunkFocus>().0 =
        [base[0] + 512.0, base[1] + 48.0, base[2] - 512.0];
    settle(&mut app, "eviction")?;
    app.world_mut().resource_mut::<ChunkFocus>().0 = base;
    settle(&mut app, "reload")
}
