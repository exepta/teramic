//! Bounded, opt-in native soak telemetry; never retains per-frame histories.
use bevy::prelude::*;
use std::{
    io::Write,
    path::PathBuf,
    time::{Duration, Instant},
};

#[derive(Resource)]
struct Output {
    path: PathBuf,
    started: Instant,
    next: Instant,
}

pub(crate) fn install(app: &mut App) {
    if !std::env::args().any(|a| a.starts_with("--smoke-test")) {
        return;
    }
    let Some(path) = std::env::var_os("TERRA_RUNTIME_HEALTH_FILE") else {
        return;
    };
    let now = Instant::now();
    app.insert_resource(Output {
        path: path.into(),
        started: now,
        next: now,
    })
    .add_systems(Update, sample);
}

fn sample(
    mut out: ResMut<Output>,
    chunks: Res<terra_world::streaming::ChunkStore>,
    rendered: Res<terra_render::RenderedSections>,
    status: Res<terra_render::RenderStatus>,
    meshes: Res<Assets<Mesh>>,
    materials: Res<Assets<StandardMaterial>>,
    images: Res<Assets<Image>>,
) {
    let now = Instant::now();
    if now < out.next {
        return;
    }
    out.next = now + Duration::from_secs(2);
    let memory = std::fs::read_to_string("/proc/self/status").ok();
    let kib = |name: &str| {
        memory.as_ref().and_then(|text| {
            text.lines().find_map(|line| {
                line.strip_prefix(name)
                    .and_then(|value| value.split_whitespace().next()?.parse::<u64>().ok())
            })
        })
    };
    let rss = kib("VmRSS:");
    let swap = kib("VmSwap:");
    let value = serde_json::json!({"seconds":now.duration_since(out.started).as_secs_f64(),
        "ready":status.ready,"rss_kib":rss,"swap_kib":swap,"loaded_sections":chunks.loaded.len(),
        "rendered_sections":rendered.sections.len(),"pending_mesh_jobs":status.pending_jobs,
        "completed_mesh_jobs":status.completed_mesh_jobs,"mesh_assets":meshes.len(),
        "material_assets":materials.len(),"image_assets":images.len(),"far_cache_bytes":status.far_cache_bytes});
    if let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&out.path)
    {
        let _ = writeln!(file, "{value}");
    }
}
