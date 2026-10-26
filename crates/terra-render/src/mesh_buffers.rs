//! Opt-in telemetry for mesh slab growth while streaming geometry.
use bevy::{
    prelude::*,
    render::{
        Render, RenderApp, RenderSystems,
        mesh::allocator::{MeshAllocator, MeshSlabId, allocate_and_free_meshes},
        render_resource::BufferId,
    },
};
use std::collections::HashMap;

pub(crate) fn install(app: &mut App) {
    if std::env::args().any(|a| a.starts_with("--smoke-test"))
        && std::env::var("TERRA_BIN_UPLOAD_CACHE").is_ok_and(|v| v == "0")
        && let Some(render) = app.get_sub_app_mut(RenderApp)
    {
        render.insert_resource(
            bevy::render::batching::gpu_preprocessing::BinnedInstanceBufferUploadSettings {
                cache_unchanged: false,
            },
        );
    }
    if !std::env::var("TERRA_CPU_SCOPES").is_ok_and(|v| v == "1") {
        return;
    }
    if let Some(render) = app.get_sub_app_mut(RenderApp) {
        render.add_systems(
            Render,
            observe
                .after(allocate_and_free_meshes)
                .in_set(RenderSystems::PrepareAssets),
        );
    }
}

fn observe(
    allocator: Res<MeshAllocator>,
    mut previous: Local<HashMap<MeshSlabId, (BufferId, u64)>>,
) {
    let mut copies = 0;
    let mut copied_bytes = 0;
    let mut allocations = 0;
    for (id, slab) in &allocator.slabs {
        let Some(buffer) = slab.buffer() else {
            continue;
        };
        let next = (buffer.id(), buffer.size());
        match previous.insert(*id, next) {
            Some((old_id, old_size)) if old_id != next.0 => {
                copies += 1;
                copied_bytes += old_size;
            }
            None => allocations += 1,
            _ => {}
        }
    }
    previous.retain(|id, _| allocator.slabs.contains_key(id));
    if copies > 0 || allocations > 0 {
        info!(
            copies,
            copied_bytes,
            allocations,
            slabs = allocator.slab_count(),
            reserved_bytes = allocator.slabs_size(),
            "Mesh GPU buffer allocation (buffer capacity, not total VRAM)"
        );
    }
}
