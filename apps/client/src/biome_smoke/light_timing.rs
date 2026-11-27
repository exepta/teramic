//! Passive native instrumentation: actual streamed data and rendered light pools.
use super::*;
use std::collections::BTreeMap;

#[derive(Resource, Default)]
pub(super) struct Timing {
    frame: u64,
    stages: BTreeMap<usize, Stage>,
}

#[derive(Default)]
struct Stage {
    loaded_frame: Option<u64>,
    local_frame: Option<u64>,
    distant_frame: Option<u64>,
    loaded_sections: usize,
    max_pending: usize,
    local_count: usize,
    distant_count: usize,
    furthest_source_m: f32,
}

#[derive(Resource, Default)]
struct ExposureTrace {
    start: Option<f32>,
    samples: Vec<(f32, f32, f32)>,
}

/// Measure adaptation on entry, then hold the same underground camera for eight
/// more seconds after the initial six-second transition.
pub(super) fn natural_exposure_ready(world: &mut World, scene: Scene) -> Result<bool, String> {
    let now = world.resource::<Time>().elapsed_secs();
    let blend = world.resource::<terra_render::CaveState>().blend;
    let (exposure, ambient) = world
        .query_filtered::<(&bevy::camera::Exposure, &AmbientLight), With<TerraCamera>>()
        .single(world)
        .map_err(|e| e.to_string())?;
    let (ev, fill) = (exposure.ev100, ambient.brightness);
    if !ev.is_finite() || !fill.is_finite() || !blend.is_finite() {
        return Err("Nonfinite underground lighting".into());
    }
    let mut trace = world.get_resource_or_init::<ExposureTrace>();
    let elapsed = now
        - *trace.start.get_or_insert_with(|| {
            info!(
                ev,
                fill, blend, "Started stationary underground lighting sample"
            );
            now
        });
    trace.samples.push((elapsed, ev, fill));
    if elapsed < 14.0 {
        return Ok(false);
    }
    let settled = &trace.samples[trace.samples.partition_point(|s| s.0 < 6.0)..];
    if settled.len() < 16 {
        return Ok(false);
    }
    let first = trace.samples[0].1;
    let movement: f32 = trace
        .samples
        .windows(2)
        .map(|w| (w[1].1 - w[0].1).abs())
        .sum();
    let reversal = movement - (ev - first).abs();
    let (mut min_ev, mut max_ev) = (ev, ev);
    let (mut min_fill, mut max_fill) = (fill, fill);
    for &(_, ev, fill) in settled {
        min_ev = min_ev.min(ev);
        max_ev = max_ev.max(ev);
        min_fill = min_fill.min(fill);
        max_fill = max_fill.max(fill);
    }
    info!(
        frames = trace.samples.len(),
        seconds = elapsed,
        first_ev = first,
        final_ev = ev,
        reversed_ev = reversal,
        cave_blend = blend,
        settled_frames = settled.len(),
        settled_secs = elapsed - settled[0].0,
        min_ev,
        max_ev,
        min_fill,
        max_fill,
        "Natural underground lighting at rest"
    );
    // Nests use a covered alcove. The broad natural gallery also sees daylight
    // through side passages, so its camera can be partly adapted to the sky.
    let minimum_cover = if matches!(scene, Scene::DragonLair(_)) {
        0.9
    } else {
        0.5
    };
    if blend < minimum_cover {
        return Err(format!(
            "Close camera is not under sufficient cover (blend {blend:.3})"
        ));
    }
    if reversal > 0.22 || max_ev - min_ev > 0.02 || max_fill - min_fill > 0.5 {
        return Err(format!(
            "Stationary light varied: reversal {reversal:.3} EV, settled {min_ev:.4}–{max_ev:.4} EV, fill {min_fill:.3}–{max_fill:.3}"
        ));
    }
    Ok(true)
}

pub(super) fn observe(world: &mut World) {
    let smoke = world.resource::<BiomeSmoke>();
    if !matches!(smoke.scene, Scene::Grave(_)) || smoke.views.is_none() || smoke.stage > 2 {
        return;
    }
    let stage = smoke.stage;
    let Some(eye) = world
        .query_filtered::<&Transform, With<TerraCamera>>()
        .iter(world)
        .next()
        .map(|pose| pose.translation)
    else {
        return;
    };
    let loaded = world.resource::<ChunkStore>();
    let loaded_count = loaded.loaded.len();
    let has_lamps = loaded.loaded.values().any(|section| {
        [369, 370, 371, 372, 373]
            .iter()
            .any(|&id| section.data.may_contain(id) && section.data.blocks().contains(&id))
    });
    // Same-floor filtering prevents the old outdoor pool from passing the
    // underground-camera transition before local illumination catches up.
    let local_count = world
        .query::<(&Name, &Transform, &PointLight)>()
        .iter(world)
        .filter(|(name, pose, light)| {
            name.as_str() == "Voxel local light"
                && (stage == 0 || (pose.translation.y - eye.y).abs() < 6.0)
                && light.intensity > 0.0
        })
        .count();
    let mut distant_count = 0;
    let mut furthest_source_m = 0_f32;
    for (name, geometry, visibility) in world.query::<(&Name, &Mesh3d, &Visibility)>().iter(world) {
        if name.as_str() != "Distant village lamp sources" || *visibility == Visibility::Hidden {
            continue;
        }
        let Some(mesh) = world.resource::<Assets<Mesh>>().get(&geometry.0) else {
            continue;
        };
        let Some(bevy::mesh::VertexAttributeValues::Float32x3(positions)) =
            mesh.attribute(Mesh::ATTRIBUTE_POSITION)
        else {
            continue;
        };
        // Each depth-tested source consists of one center and eight edge vertices.
        distant_count += positions.len() / 9;
        for source in positions.iter().step_by(9) {
            furthest_source_m = furthest_source_m.max(Vec3::from_array(*source).distance(eye));
        }
    }
    let pending = world
        .get_resource::<terra_render::LightDiscoveryStatus>()
        .map_or(0, |status| status.pending_sections);
    let mut timing = world.resource_mut::<Timing>();
    timing.frame += 1;
    let frame = timing.frame;
    let record = timing.stages.entry(stage).or_default();
    if has_lamps && record.loaded_frame.is_none() {
        record.loaded_frame = Some(frame);
    }
    if local_count > 0 && record.local_frame.is_none() {
        record.local_frame = Some(frame);
    }
    if distant_count > 0 && record.distant_frame.is_none() {
        record.distant_frame = Some(frame);
    }
    record.loaded_sections = loaded_count;
    record.max_pending = record.max_pending.max(pending);
    record.local_count = local_count;
    record.distant_count = distant_count;
    record.furthest_source_m = furthest_source_m.max(record.furthest_source_m);
}

pub(super) fn snapshot(world: &World) -> serde_json::Value {
    let Some(timing) = world.get_resource::<Timing>() else {
        return serde_json::Value::Null;
    };
    let status = world
        .get_resource::<terra_render::LightDiscoveryStatus>()
        .copied()
        .unwrap_or_default();
    serde_json::json!({
        "discovery": {"selections":status.selections,"source_scans":status.source_scans,
            "fast_rejections":status.fast_rejections,"pending_sections":status.pending_sections,
            "cached_sections":status.cached_sections,"discovered_emitters":status.discovered_emitters},
        "stages": timing.stages.iter().map(|(stage, record)| serde_json::json!({
            "stage":stage,"first_loaded_lamp_frame":record.loaded_frame,
            "first_same_floor_local_light_frame":record.local_frame,
            "first_distant_batch_frame":record.distant_frame,
            "loaded_sections":record.loaded_sections,"peak_pending_sections":record.max_pending,
            "local_same_floor_count":record.local_count,"distant_sources":record.distant_count,
            "furthest_source_m":record.furthest_source_m,
        })).collect::<Vec<_>>()
    })
}
