use super::field::SIZE;
use bevy::{
    asset::RenderAssetUsages,
    image::{ImageSampler, ImageSamplerDescriptor},
    mesh::MeshVertexBufferLayoutRef,
    prelude::*,
    render::render_resource::{
        AsBindGroup, Extent3d, Face, RenderPipelineDescriptor, SpecializedMeshPipelineError,
        TextureDimension, TextureFormat,
    },
    shader::ShaderRef,
};

#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub(super) struct BiomeMistMaterial {
    #[uniform(0)]
    pub color: LinearRgba,
    #[uniform(1)]
    pub flow: Vec4,
    #[uniform(2)]
    pub center: Vec4,
    #[uniform(3)]
    pub half_extent: Vec4,
    /// Field origin X/Z, minimum height, height decoding span.
    #[uniform(4)]
    pub field: Vec4,
    #[texture(5, dimension = "3d")]
    #[sampler(6)]
    pub noise: Handle<Image>,
    #[texture(7)]
    #[sampler(8)]
    pub terrain: Handle<Image>,
}
impl Material for BiomeMistMaterial {
    fn vertex_shader() -> ShaderRef {
        "shaders/terra_biome_mist.wesl".into()
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
        descriptor.vertex.buffers = vec![
            layout
                .0
                .get_layout(&[Mesh::ATTRIBUTE_POSITION.at_shader_location(0)])?,
        ];
        descriptor.primitive.cull_mode = Some(Face::Front);
        if let Some(depth) = &mut descriptor.depth_stencil {
            depth.depth_write_enabled = Some(false);
            depth.bias = default();
        }
        Ok(())
    }
}
pub(super) fn terrain_image(pixels: Vec<u8>) -> Image {
    let mut image = Image::new(
        Extent3d {
            width: SIZE as u32,
            height: SIZE as u32,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        pixels,
        TextureFormat::Rgba8Unorm,
        RenderAssetUsages::default(),
    );
    image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor::linear());
    image
}
