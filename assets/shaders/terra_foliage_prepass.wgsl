// Bevy's standard prepass ignores vertex alpha. Keep its PBR outputs while
// matching the forward leaf mask, including seasonal shedding, in depth/shadows.
#import bevy_pbr::{
    pbr_prepass_functions,
    pbr_bindings,
    pbr_types,
    pbr_functions,
    pbr_functions::SampleBias,
    prepass_io,
    mesh_bindings::mesh,
    mesh_view_bindings::view,
}
#import bevy_render::bindless::{bindless_samplers_filtering, bindless_textures_2d}

#ifdef BINDLESS
#import bevy_pbr::pbr_bindings::material_indices
#endif

@group(#{MATERIAL_BIND_GROUP}) @binding(100) var<uniform> wind: vec4<f32>;

fn leaf_alpha_discard(in: prepass_io::VertexOutput) {
    // Rooted fibres and flames retain the engine's original alpha semantics.
    if wind.w >= 0.5 {
        pbr_prepass_functions::prepass_alpha_discard(in);
        return;
    }
#ifdef MAY_DISCARD
#ifdef BINDLESS
    let slot = mesh[in.instance_index].material_and_lightmap_bind_group_slot & 0xffffu;
    var output_color = pbr_bindings::material_array[material_indices[slot].material].base_color;
    let flags = pbr_bindings::material_array[material_indices[slot].material].flags;
    let uv_transform = pbr_bindings::material_array[material_indices[slot].material].uv_transform;
    let alpha_cutoff = pbr_bindings::material_array[material_indices[slot].material].alpha_cutoff;
#else
    var output_color = pbr_bindings::material.base_color;
    let flags = pbr_bindings::material.flags;
    let uv_transform = pbr_bindings::material.uv_transform;
    let alpha_cutoff = pbr_bindings::material.alpha_cutoff;
#endif
#ifdef VERTEX_UVS
#ifdef STANDARD_MATERIAL_BASE_COLOR_UV_B
    var uv = in.uv_b;
#else
    var uv = in.uv;
#endif
    uv = (uv_transform * vec3(uv, 1.0)).xy;
    if (flags & pbr_types::STANDARD_MATERIAL_FLAGS_BASE_COLOR_TEXTURE_BIT) != 0u {
        output_color *= textureSampleBias(
#ifdef BINDLESS
            bindless_textures_2d[material_indices[slot].base_color_texture],
            bindless_samplers_filtering[material_indices[slot].base_color_sampler],
#else
            pbr_bindings::base_color_texture,
            pbr_bindings::base_color_sampler,
#endif
            uv,
            view.mip_bias
        );
    }
#endif
#ifdef VERTEX_COLORS
    output_color.a *= in.color.a;
#endif
    // LeafExtension prepares wind.w == 0 exclusively as AlphaMode::Mask.
    if output_color.a < alpha_cutoff {
        discard;
    }
#endif
}

#ifdef PREPASS_FRAGMENT
@fragment
fn fragment(
    in: prepass_io::VertexOutput,
    @builtin(front_facing) is_front: bool,
) -> prepass_io::FragmentOutput {
#ifdef BINDLESS
    let slot = mesh[in.instance_index].material_and_lightmap_bind_group_slot & 0xffffu;
    let flags = pbr_bindings::material_array[material_indices[slot].material].flags;
    let uv_transform = pbr_bindings::material_array[material_indices[slot].material].uv_transform;
#else
    let flags = pbr_bindings::material.flags;
    let uv_transform = pbr_bindings::material.uv_transform;
#endif
#ifdef VISIBILITY_RANGE_DITHER
    pbr_functions::visibility_range_dither(in.position, in.visibility_range_dither);
#endif
    leaf_alpha_discard(in);
    var out: prepass_io::FragmentOutput;
#ifdef UNCLIPPED_DEPTH_ORTHO_EMULATION
    out.frag_depth = in.unclipped_depth;
#endif
#ifdef NORMAL_PREPASS
    if (flags & pbr_types::STANDARD_MATERIAL_FLAGS_UNLIT_BIT) == 0u {
        let double_sided = (flags & pbr_types::STANDARD_MATERIAL_FLAGS_DOUBLE_SIDED_BIT) != 0u;
        let world_normal = pbr_functions::prepare_world_normal(in.world_normal, double_sided, is_front);
        var normal = world_normal;
#ifdef VERTEX_UVS
#ifdef VERTEX_TANGENTS
#ifdef STANDARD_MATERIAL_NORMAL_MAP
#ifdef STANDARD_MATERIAL_NORMAL_MAP_UV_B
        let uv = (uv_transform * vec3(in.uv_b, 1.0)).xy;
#else
        let uv = (uv_transform * vec3(in.uv, 1.0)).xy;
#endif
        var bias: SampleBias;
        bias.mip_bias = view.mip_bias;
        let Nt = textureSampleBias(
#ifdef BINDLESS
            bindless_textures_2d[material_indices[slot].normal_map_texture],
            bindless_samplers_filtering[material_indices[slot].normal_map_sampler],
#else
            pbr_bindings::normal_map_texture,
            pbr_bindings::normal_map_sampler,
#endif
            uv,
            bias.mip_bias,
        ).rgb;
        let TBN = pbr_functions::calculate_tbn_mikktspace(normal, in.world_tangent);
        normal = pbr_functions::apply_normal_mapping(flags, TBN, double_sided, is_front, Nt);
#endif
#endif
#endif
        out.normal = vec4(normal * 0.5 + vec3(0.5), 1.0);
    } else {
        out.normal = vec4(in.world_normal * 0.5 + vec3(0.5), 1.0);
    }
#endif
#ifdef MOTION_VECTOR_PREPASS
    out.motion_vector = pbr_prepass_functions::calculate_motion_vector(in.world_position, in.previous_world_position);
#endif
    return out;
}
#else
@fragment
fn fragment(in: prepass_io::VertexOutput) {
    leaf_alpha_discard(in);
}
#endif
