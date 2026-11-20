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
