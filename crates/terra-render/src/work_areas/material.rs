//! Low, translucent density volumes. Proxy faces never supply the visible color.
use bevy::{
    asset::RenderAssetUsages,
    image::{ImageAddressMode, ImageSampler, ImageSamplerDescriptor},
    mesh::MeshVertexBufferLayoutRef,
    prelude::*,
    render::render_resource::{
        AsBindGroup, Extent3d, Face, RenderPipelineDescriptor, SpecializedMeshPipelineError,
        TextureDimension, TextureFormat,
    },
    shader::ShaderRef,
};

/// Shared by all contour pieces of one area kind; no per-piece entities or assets.
#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub(super) struct MistMaterial {
    #[uniform(0)]
    pub color: LinearRgba,
    /// Noise advection X/Z, vertical advection, optical density.
    #[uniform(1)]
    pub flow: Vec4,
    #[texture(2, dimension = "3d")]
    #[sampler(3)]
    pub noise: Handle<Image>,
}

impl Material for MistMaterial {
    fn vertex_shader() -> ShaderRef {
        "shaders/terra_area_mist.wesl".into()
    }

    fn fragment_shader() -> ShaderRef {
        Self::vertex_shader()
    }

    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Blend
    }

    fn enable_prepass() -> bool {
        false
    }

    fn enable_shadows() -> bool {
        false
    }

    fn specialize(
        _pipeline: &bevy::pbr::MaterialPipeline,
        descriptor: &mut RenderPipelineDescriptor,
        layout: &MeshVertexBufferLayoutRef,
        _key: bevy::pbr::MaterialPipelineKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        descriptor.vertex.buffers = vec![layout.0.get_layout(&[
            Mesh::ATTRIBUTE_POSITION.at_shader_location(0),
            Mesh::ATTRIBUTE_NORMAL.at_shader_location(1),
            Mesh::ATTRIBUTE_UV_0.at_shader_location(2),
            Mesh::ATTRIBUTE_COLOR.at_shader_location(3),
            Mesh::ATTRIBUTE_UV_1.at_shader_location(4),
        ])?];
        // Render only exit faces, including while the camera is inside a bank.
        // The fragment shader depth-tests the first density hit, not this box.
        descriptor.primitive.cull_mode = Some(Face::Front);
        if let Some(depth) = &mut descriptor.depth_stencil {
            depth.bias = default();
            depth.depth_write_enabled = Some(false);
        }
        Ok(())
    }
}

pub(super) fn flow(seconds: f64) -> Vec4 {
    // The noise repeats every 64 units. All three drift components wrap exactly
    // onto that period, retaining sub-second movement in long-running sessions.
    let time = if seconds.is_finite() {
        seconds.rem_euclid(2048.0) as f32
    } else {
        0.0
    };
    Vec4::new(time * 0.25, time * 0.125, time * 0.125, 0.95)
}

/// One 32 KiB periodic texture, shared by every area and local biome bank.
/// GPU trilinear filtering supplies smooth noise without per-fragment hashing.
pub(super) fn density_texture() -> Image {
    let pixels = (0..32_u32.pow(3))
        .map(|index| {
            let mut hash = index.wrapping_add(0x9e37_79b9);
            hash = (hash ^ (hash >> 16)).wrapping_mul(0x7feb_352d);
            hash = (hash ^ (hash >> 15)).wrapping_mul(0x846c_a68b);
            ((hash ^ (hash >> 16)) & 255) as u8
        })
        .collect();
    let mut image = Image::new(
        Extent3d {
            width: 32,
            height: 32,
            depth_or_array_layers: 32,
        },
        TextureDimension::D3,
        pixels,
        TextureFormat::R8Unorm,
        RenderAssetUsages::RENDER_WORLD,
    );
    image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
        address_mode_u: ImageAddressMode::Repeat,
        address_mode_v: ImageAddressMode::Repeat,
        address_mode_w: ImageAddressMode::Repeat,
        ..ImageSamplerDescriptor::linear()
    });
    image
}

/// Colored haze retains the semantic border hue without opaque primary-color ink.
pub(super) fn contour_color(color: Color) -> LinearRgba {
    let linear = color.to_linear();
    LinearRgba::new(
        linear.red * 0.70 + 0.22,
        linear.green * 0.70 + 0.22,
        linear.blue * 0.70 + 0.22,
        0.78,
    )
}
