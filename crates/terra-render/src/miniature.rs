use bevy::camera::visibility::RenderLayers;

pub(crate) fn layers() -> RenderLayers {
    RenderLayers::default().with(terra_core::miniature::WORLD_LAYER)
}

pub(crate) fn creature_layers() -> RenderLayers {
    RenderLayers::default().with(terra_core::miniature::CREATURE_LAYER)
}
