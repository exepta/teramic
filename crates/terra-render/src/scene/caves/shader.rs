use bevy::{
    core_pipeline::{
        FullscreenShader,
        prepass::ViewPrepassTextures,
        schedule::{Core3d, Core3dSystems},
    },
    prelude::*,
    render::{
        RenderApp, RenderStartup,
        extract_component::{
            ComponentUniforms, DynamicUniformIndex, ExtractComponent, ExtractComponentPlugin,
            UniformComponentPlugin,
        },
        render_resource::{
            binding_types::{sampler, texture_2d, uniform_buffer},
            *,
        },
        renderer::{RenderContext, RenderDevice, ViewQuery},
        view::ViewTarget,
    },
};

#[derive(Component, Clone, Copy, ExtractComponent, ShaderType)]
pub(crate) struct CaveSettings {
    pub view_from_clip: Mat4,
    pub parameters: Vec4,
    pub world_from_view: Mat4,
    pub underwater: Vec4,
    pub scattering: Vec4,
    pub ink: Vec4,
    pub smoke: Vec4,
}
impl Default for CaveSettings {
    fn default() -> Self {
        Self {
            view_from_clip: Mat4::IDENTITY,
            parameters: Vec4::ZERO,
            world_from_view: Mat4::IDENTITY,
            underwater: Vec4::ZERO,
            scattering: Vec4::ZERO,
            ink: Vec4::ZERO,
            smoke: Vec4::ZERO,
        }
    }
}
#[derive(Resource)]
struct Pipelines {
    variants: Vec<(
        TextureFormat,
        bool,
        BindGroupLayoutDescriptor,
        CachedRenderPipelineId,
    )>,
    sampler: Sampler,
}
pub(super) fn install(app: &mut App) {
    app.add_plugins((
        ExtractComponentPlugin::<CaveSettings>::default(),
        UniformComponentPlugin::<CaveSettings>::default(),
    ));
    if let Some(render) = app.get_sub_app_mut(RenderApp) {
        render.add_systems(RenderStartup, setup).add_systems(
            Core3d,
            draw.in_set(Core3dSystems::EarlyPostProcess)
                .before(bevy::anti_alias::taa::temporal_anti_alias)
                .before(bevy::core_pipeline::tonemapping::tonemapping),
        );
    }
}
fn setup(
    mut commands: Commands,
    device: Res<RenderDevice>,
    assets: Res<AssetServer>,
    fullscreen: Res<FullscreenShader>,
    cache: Res<PipelineCache>,
) {
    let mut variants = Vec::new();
    let shader = assets.load("shaders/terra_caves.wgsl");
    for format in [
        TextureFormat::Rgba16Float,
        TextureFormat::Rgba8UnormSrgb,
        TextureFormat::Rgba8Unorm,
    ] {
        for multisampled in [false, true] {
            let mut entries = BindGroupLayoutEntries::sequential(
                ShaderStages::FRAGMENT,
                (
                    texture_2d(TextureSampleType::Float { filterable: true }),
                    sampler(SamplerBindingType::Filtering),
                    uniform_buffer::<CaveSettings>(true),
                ),
            )
            .to_vec();
            entries.push(BindGroupLayoutEntry {
                binding: 3,
                visibility: ShaderStages::FRAGMENT,
                ty: BindingType::Texture {
                    sample_type: TextureSampleType::Depth,
                    view_dimension: TextureViewDimension::D2,
                    multisampled,
                },
                count: None,
            });
            let layout = BindGroupLayoutDescriptor::new("terra_cave_layout", &entries);
            let pipeline = cache.queue_render_pipeline(RenderPipelineDescriptor {
                label: Some("terra_cave_depth_grade".into()),
                layout: vec![layout.clone()],
                vertex: fullscreen.to_vertex_state(),
                fragment: Some(FragmentState {
                    shader: shader.clone(),
                    shader_defs: if multisampled {
                        vec!["MULTISAMPLED".into()]
                    } else {
                        vec![]
                    },
                    targets: vec![Some(ColorTargetState {
                        format,
                        blend: None,
                        write_mask: ColorWrites::ALL,
                    })],
                    ..default()
                }),
                ..default()
            });
            variants.push((format, multisampled, layout, pipeline));
        }
    }
    commands.insert_resource(Pipelines {
        variants,
        sampler: device.create_sampler(&SamplerDescriptor::default()),
    });
}
fn draw(
    view: ViewQuery<(
        &ViewTarget,
        &ViewPrepassTextures,
        &Msaa,
        &CaveSettings,
        &DynamicUniformIndex<CaveSettings>,
    )>,
    pipelines: Res<Pipelines>,
    cache: Res<PipelineCache>,
    uniforms: Res<ComponentUniforms<CaveSettings>>,
    draws: Option<Res<crate::DrawCallStatus>>,
    mut ctx: RenderContext,
) {
    let (target, prepass, msaa, settings, index) = view.into_inner();
    // The same optional pass serves caves, underwater optics and short ink hits.
    // Dry outdoor views and Low water quality without ink have no extra pass.
    if settings.parameters.x <= 0.001
        && settings.parameters.z <= 0.001
        && settings.underwater.x <= 0.001
        && settings.ink.x <= 0.001
        && settings.smoke.x <= 0.001
    {
        return;
    }
    let Some((_, _, layout, id)) = pipelines.variants.iter().find(|(format, multi, _, _)| {
        *format == target.main_texture_format() && *multi == (msaa.samples() > 1)
    }) else {
        return;
    };
    let (Some(pipeline), Some(binding), Some(depth)) = (
        cache.get_render_pipeline(*id),
        uniforms.uniforms().binding(),
        prepass.depth_view(),
    ) else {
        return;
    };
    let post = target.post_process_write();
    let group = ctx.render_device().create_bind_group(
        "terra_cave_bindings",
        &cache.get_bind_group_layout(layout),
        &BindGroupEntries::sequential((post.source, &pipelines.sampler, binding, depth)),
    );
    let mut pass = ctx
        .command_encoder()
        .begin_render_pass(&RenderPassDescriptor {
            label: Some("terra_cave_depth_grade"),
            color_attachments: &[Some(RenderPassColorAttachment {
                view: post.destination,
                depth_slice: None,
                resolve_target: None,
                ops: Operations::default(),
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
    pass.set_pipeline(pipeline);
    pass.set_bind_group(0, &group, &[index.index()]);
    pass.draw(0..3, 0..1);
    if let Some(draws) = draws {
        draws.fullscreen_drawn();
    }
}
