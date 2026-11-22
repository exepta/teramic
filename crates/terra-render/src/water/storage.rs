//! One stable dynamic binding shared by all water materials and tiles.
use bevy::{
    asset::{RenderAssetUsages, uuid_handle},
    prelude::*,
    render::{
        Render, RenderApp, RenderSystems,
        extract_resource::{ExtractResource, ExtractResourcePlugin},
        render_asset::{RenderAssets, prepare_assets},
        render_resource::{BufferId, BufferUsages, ShaderType, encase},
        renderer::RenderQueue,
        storage::{GpuShaderBuffer, ShaderBuffer},
    },
};

pub(super) const WATER_BUFFER_HANDLE: Handle<ShaderBuffer> =
    uuid_handle!("41469997-2158-45d0-8bc3-4db895f71911");

#[derive(Clone, Copy, Debug, PartialEq, ShaderType)]
pub(super) struct WaterGpuData {
    /// Wind direction, storm energy and wind speed.
    pub weather: Vec4,
    /// Solar access, wake count and the shader-clock wrap period.
    pub environment: Vec4,
    pub wake_positions: [Vec4; super::contacts::WAKE_CAPACITY],
    pub wake_motions: [Vec4; super::contacts::WAKE_CAPACITY],
    pub gallery_aperture: Vec4,
}

impl Default for WaterGpuData {
    fn default() -> Self {
        Self {
            weather: Vec4::new(0.86, 0.51, 0.0, 0.0),
            environment: Vec4::new(1.0, 0.0, 3600.0, 0.0),
            wake_positions: [Vec4::ZERO; super::contacts::WAKE_CAPACITY],
            wake_motions: [Vec4::ZERO; super::contacts::WAKE_CAPACITY],
            gallery_aperture: Vec4::ZERO,
        }
    }
}

#[derive(Resource, Clone, Copy, Default, ExtractResource)]
#[extract_app(RenderApp)]
pub(super) struct WaterPayload(pub WaterGpuData);

pub(super) fn install(app: &mut App) {
    app.init_resource::<Assets<ShaderBuffer>>()
        .init_resource::<WaterPayload>();
    let mut buffers = app.world_mut().resource_mut::<Assets<ShaderBuffer>>();
    if !buffers.contains(WATER_BUFFER_HANDLE.id())
        && let Some(bytes) = encode(&WaterGpuData::default())
    {
        let mut buffer = ShaderBuffer::new(bytes, RenderAssetUsages::RENDER_WORLD);
        buffer.label = "terra shared water dynamics".into();
        buffer.buffer_usage = BufferUsages::STORAGE | BufferUsages::COPY_DST;
        if let Err(error) = buffers.insert(WATER_BUFFER_HANDLE.id(), buffer) {
            tracing::error!(%error, "Shared water buffer initialization failed");
        }
    }
    if app.get_sub_app(RenderApp).is_some() {
        app.add_plugins(ExtractResourcePlugin::<WaterPayload>::default());
        app.sub_app_mut(RenderApp).add_systems(
            Render,
            upload
                .in_set(RenderSystems::PrepareResources)
                .after(prepare_assets::<GpuShaderBuffer>),
        );
    }
}

fn encode(data: &WaterGpuData) -> Option<Vec<u8>> {
    let mut buffer = encase::StorageBuffer::new(Vec::with_capacity(560));
    if let Err(error) = buffer.write(data) {
        tracing::error!(%error, "Shared water payload encoding failed");
        return None;
    }
    Some(buffer.into_inner())
}

fn upload(
    payload: Option<Res<WaterPayload>>,
    buffers: Res<RenderAssets<GpuShaderBuffer>>,
    queue: Res<RenderQueue>,
    mut previous: Local<Option<(BufferId, WaterGpuData)>>,
) {
    let Some(payload) = payload else {
        return;
    };
    let Some(buffer) = buffers.get(WATER_BUFFER_HANDLE.id()) else {
        return;
    };
    let current = (buffer.buffer.id(), payload.0);
    if previous.as_ref() == Some(&current) {
        return;
    }
    let Some(bytes) = encode(&payload.0) else {
        return;
    };
    queue.write_buffer(&buffer.buffer, 0, &bytes);
    *previous = Some(current);
}

#[cfg(test)]
#[path = "storage_tests.rs"]
mod tests;
