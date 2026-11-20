//! Bounded native evidence over a real observation window, not one particle phase.
use super::*;
use std::{
    collections::BTreeMap,
    hash::{Hash, Hasher},
};

#[derive(Resource, Default)]
struct Observations(BTreeMap<usize, Window>);

struct Window {
    started: Instant,
    samples: u64,
    active_frames: u64,
    moving_frames: u64,
    last_pose: Option<u64>,
    peak_candidates: usize,
    peak_falling: usize,
    peak_splashes: usize,
    pool_capacity: usize,
    scanned_sections: usize,
    first_animation_time: f32,
    last_animation_time: f32,
    water_triangles: usize,
    observed_seconds: f64,
}

impl Default for Window {
    fn default() -> Self {
        Self {
            started: Instant::now(),
            samples: 0,
            active_frames: 0,
            moving_frames: 0,
            last_pose: None,
            peak_candidates: 0,
            peak_falling: 0,
            peak_splashes: 0,
            pool_capacity: 0,
            scanned_sections: 0,
            first_animation_time: 0.,
            last_animation_time: 0.,
            water_triangles: 0,
            observed_seconds: 0.0,
        }
    }
}

pub(super) fn install(app: &mut App) {
    app.init_resource::<Observations>().add_systems(
        PostUpdate,
        observe.after(bevy::camera::visibility::VisibilitySystems::CheckVisibility),
    );
}

fn observe(world: &mut World) {
    let smoke = world.resource::<BiomeSmoke>();
    if !matches!(smoke.scene, Scene::BloodForest)
        || smoke.stage > 1
        || !world.resource::<RenderStatus>().ready
        || !world.resource::<WaterStatus>().ready
    {
        return;
    }
    let stage = smoke.stage;
    let Some(status) = world.get_resource::<terra_render::BloodDripStatus>() else {
        return;
    };
    let (candidates, falling, splashes, pool, scanned, time) = (
        status.candidates,
        status.falling,
        status.splashes,
        status.pool_capacity,
        status.scanned_sections,
        status.animation_time,
    );
    let water_triangles = world.resource::<WaterStatus>().triangles;
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    let mut visible = 0;
    for (entity, name, pose, visibility, in_view) in world
        .query::<(
            Entity,
            &Name,
            &Transform,
            &Visibility,
            &bevy::camera::visibility::ViewVisibility,
        )>()
        .iter(world)
    {
        if name.as_str() != "Blood canopy droplet"
            || *visibility != Visibility::Visible
            || !in_view.get()
        {
            continue;
        }
        visible += 1;
        entity.hash(&mut hasher);
        for value in pose
            .translation
            .to_array()
            .into_iter()
            .chain(pose.scale.to_array())
        {
            value.to_bits().hash(&mut hasher);
        }
    }
    let pose = (visible > 0).then(|| hasher.finish());
    let mut observations = world.resource_mut::<Observations>();
    let window = observations.0.entry(stage).or_default();
    if window.samples == 0 {
        window.first_animation_time = time;
    }
    window.samples += 1;
    window.active_frames += u64::from(visible > 0 && falling + splashes > 0);
    window.moving_frames +=
        u64::from(pose.is_some() && window.last_pose.is_some() && pose != window.last_pose);
    window.last_pose = pose;
    window.peak_candidates = window.peak_candidates.max(candidates);
    window.peak_falling = window.peak_falling.max(falling);
    window.peak_splashes = window.peak_splashes.max(splashes);
    window.pool_capacity = window.pool_capacity.max(pool);
    window.scanned_sections = window.scanned_sections.max(scanned);
    window.last_animation_time = time;
    window.water_triangles = water_triangles;
    window.observed_seconds = window.started.elapsed().as_secs_f64();
}

pub(super) fn inspect(world: &mut World, stage: usize) -> Result<bool, String> {
    let observations = world
        .get_resource::<Observations>()
        .ok_or("Missing Blood Forest observation resource")?;
    let Some(window) = observations.0.get(&stage) else {
        return Ok(false);
    };
    if window.observed_seconds < 2.0 || window.samples < 2 {
        return Ok(false);
    }
    if window.water_triangles == 0 {
        return Err("Native Blood Forest has no rendered water triangles".into());
    }
    // Overview may be outside the 42 m particle LOD. The actual canopy close-up
    // must observe source data, visible particles and changing real transforms.
    if stage == 1
        && (window.peak_candidates == 0
            || window.peak_falling == 0
            || window.active_frames == 0
            || window.moving_frames == 0)
    {
        if window.observed_seconds < 12.0 {
            return Ok(false);
        }
        return Err(
            "No loaded and animated Blood Forest droplets observed in twelve seconds".into(),
        );
    }
    let views:serde_json::Map<String,serde_json::Value>=observations.0.iter().map(|(stage,w)| {
        let name = if *stage == 0 { "overview" } else { "close" };
        (name.to_owned(),serde_json::json!({
            "observed_seconds":w.observed_seconds,"samples":w.samples,
            "active_frames":w.active_frames,"moving_frames":w.moving_frames,
            "peak_candidates":w.peak_candidates,"peak_falling":w.peak_falling,"peak_splashes":w.peak_splashes,
            "maximum_pool_capacity":w.pool_capacity,"maximum_scanned_sections":w.scanned_sections,
            "first_animation_time":w.first_animation_time,"last_animation_time":w.last_animation_time,
            "water_triangles":w.water_triangles,
        }))
    }).collect();
    let report = serde_json::json!({"status":if stage==1 {"passed"} else {"observing"},"fixture_edits":0,"views":views});
    let directory = std::env::var_os("TERRA_CAPTURE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::temp_dir().join("terra-biomes"));
    std::fs::create_dir_all(&directory).map_err(|e| e.to_string())?;
    std::fs::write(
        directory.join("blood-forest-droplets.json"),
        serde_json::to_vec_pretty(&report).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    info!(
        stage,
        samples = window.samples,
        active_frames = window.active_frames,
        moving_frames = window.moving_frames,
        candidates = window.peak_candidates,
        falling = window.peak_falling,
        "Native Blood Forest droplet inspection (real observation window)"
    );
    super::camera::inspect(world, stage)?;
    Ok(true)
}
