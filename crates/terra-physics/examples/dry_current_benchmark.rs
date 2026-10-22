//! Synthetic dry-current CPU diagnostic. This is not a grounded gameplay benchmark.
use bevy::prelude::*;
use std::{
    hint::black_box,
    time::{Duration, Instant},
};
use terra_core::{GameConfig, TerraCorePlugin};
use terra_physics::current::body_motion;
use terra_voxel::SectionPos;
use terra_world::{
    TerraGenerator, TerraWorldPlugin, WorldMode,
    edits::EditJournal,
    streaming::{ChunkFocus, ChunkStore, LoadedSection, StreamingSettings},
};

const SAMPLES: usize = 4096;
const HEIGHT: f32 = 361.0;

fn main() -> Result<(), String> {
    let config = GameConfig::default();
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_plugins(TerraCorePlugin(config.clone()))
        .add_plugins(TerraWorldPlugin {
            mode: WorldMode::DedicatedServer,
        })
        .insert_resource(StreamingSettings {
            radius: 0,
            vertical_radius: 0,
            max_jobs: 1,
            meshes: false,
        })
        .insert_resource(ChunkFocus([8.0, f64::from(HEIGHT), 8.0]));
    app.finish();
    app.cleanup();
    let owner = SectionPos::new(0, i64::from(HEIGHT as i32).div_euclid(16), 0);
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        app.update();
        let chunks = app.world().resource::<ChunkStore>();
        if chunks.loaded.contains_key(&owner) && chunks.surface_column_at(8, 8).is_some() {
            break;
        }
        if Instant::now() >= deadline {
            return Err("prepared-column fixture timed out".into());
        }
        std::thread::sleep(Duration::from_millis(1));
    }
    let prepared = app.world_mut().remove_resource::<ChunkStore>().unwrap();
    let mut uncached = ChunkStore::default();
    uncached.loaded.insert(
        owner,
        LoadedSection {
            data: prepared.loaded[&owner].data.clone(),
            mesh: None,
            mesh_epoch: 0,
            water_epoch: 0,
            water: None,
            props: Vec::new(),
        },
    );
    println!(
        "Synthetic dry air: seed={} y={HEIGHT} samples={SAMPLES}; prepared columns versus identical loaded blocks without columns",
        config.world.seed.0
    );
    for (label, chunks) in [("prepared", &prepared), ("uncached", &uncached)] {
        let terrain = TerraGenerator::new(config.world.clone()).map_err(|e| e.to_string())?;
        for phase in ["cold", "warm"] {
            measure(label, phase, chunks, &terrain)?;
        }
    }
    for (name, sample) in terra_core::profiling::snapshot() {
        if name.starts_with("water.") {
            println!(
                "scope={name} calls={} mean_ms={:.6} max_ms={:.6} over8={}",
                sample.calls,
                sample.total_ms / sample.calls.max(1) as f64,
                sample.max_ms,
                sample.over_8_ms
            );
        }
    }
    Ok(())
}

fn measure(
    label: &str,
    phase: &str,
    chunks: &ChunkStore,
    terrain: &TerraGenerator,
) -> Result<(), String> {
    let journal = EditJournal::default();
    let half = Vec3::new(0.32, 0.90, 0.32);
    let mut times = Vec::with_capacity(SAMPLES);
    for index in 0..SAMPLES {
        let position = Vec3::new(
            (index % 16) as f32 + 0.5,
            HEIGHT,
            (index / 16 % 16) as f32 + 0.5,
        );
        let start = Instant::now();
        let motion = black_box(body_motion(
            terrain,
            chunks,
            &journal,
            position - half,
            position + half,
        ));
        times.push(start.elapsed().as_secs_f64() * 1000.0);
        if motion.immersion != 0.0 {
            return Err(format!(
                "diagnostic sample must be dry: {position:?} {motion:?}"
            ));
        }
    }
    let total: f64 = times.iter().sum();
    times.sort_by(f64::total_cmp);
    println!(
        "{label} {phase}: mean_ms={:.6} p50_ms={:.6} p95_ms={:.6} p99_ms={:.6} max_ms={:.6}",
        total / SAMPLES as f64,
        times[SAMPLES / 2],
        times[SAMPLES * 95 / 100],
        times[SAMPLES * 99 / 100],
        times[SAMPLES - 1]
    );
    Ok(())
}
