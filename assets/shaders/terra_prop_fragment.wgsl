// Opaque screen-door coverage shares the same clock in color and depth passes.
#import bevy_pbr::{
    pbr_functions,
    pbr_prepass_functions,
    pbr_fragment::pbr_input_from_standard_material,
    decal::clustered::apply_decals,
    pbr_types::STANDARD_MATERIAL_FLAGS_UNLIT_BIT,
}
#ifdef PREPASS_PIPELINE
#import bevy_pbr::prepass_io::VertexOutput
#ifdef PREPASS_FRAGMENT
#import bevy_pbr::prepass_io::FragmentOutput
#endif
#ifdef DEFERRED_PREPASS
#import bevy_pbr::pbr_deferred_functions::deferred_output
#endif
#else
#import bevy_pbr::forward_io::{VertexOutput, FragmentOutput}
#ifdef OIT_ENABLED
#import bevy_core_pipeline::oit::oit_draw
#import bevy_pbr::pbr_types
#endif
#endif

struct TerraWeather {
    wind: vec4<f32>,
    surface: vec4<f32>,
    tint: vec4<f32>,
    leaves: vec4<f32>,
    grass: vec4<f32>,
    vegetation: vec4<f32>,
    flowers: array<vec4<f32>, 64>,
};
@group(#{MATERIAL_BIND_GROUP}) @binding(101) var<storage, read> terra_weather: TerraWeather;

fn crop_coverage(input: VertexOutput) {
#ifdef VERTEX_COLORS
    if input.color.a < 1.0 {
        let bayer = array<f32, 16>(0.0, 8.0, 2.0, 10.0, 12.0, 4.0, 14.0, 6.0,
            3.0, 11.0, 1.0, 9.0, 15.0, 7.0, 13.0, 5.0);
        let pixel = vec2<u32>(input.position.xy);
        let threshold = (bayer[(pixel.y % 4u) * 4u + pixel.x % 4u] + 0.5) / 16.0;
        if threshold >= input.color.a { discard; }
    }
#endif
#ifdef VERTEX_UVS_B
    if input.uv_b.y < -0.5 {
        let t = clamp((terra_weather.surface.w - input.uv_b.x) / 2.4, 0.0, 1.0);
        let eased = t * t * (3.0 - 2.0 * t);
        let bayer = array<f32, 16>(0.0, 8.0, 2.0, 10.0, 12.0, 4.0, 14.0, 6.0,
            3.0, 11.0, 1.0, 9.0, 15.0, 7.0, 13.0, 5.0);
        let pixel = vec2<u32>(input.position.xy);
        let threshold = (bayer[(pixel.y % 4u) * 4u + pixel.x % 4u] + 0.5) / 16.0;
        if input.uv_b.y < -2.5 {
            if threshold < eased { discard; }
        } else if threshold >= eased { discard; }
    }
#endif
}

#ifdef PREPASS_PIPELINE
#ifdef PREPASS_FRAGMENT
@fragment
fn fragment(input: VertexOutput, @builtin(front_facing) front: bool) -> FragmentOutput {
    crop_coverage(input);
#ifdef VISIBILITY_RANGE_DITHER
    pbr_functions::visibility_range_dither(input.position, input.visibility_range_dither);
#endif
#ifdef DEFERRED_PREPASS
    var pbr = pbr_input_from_standard_material(input, front);
    pbr.material.base_color = pbr_functions::alpha_discard(pbr.material, pbr.material.base_color);
    pbr.material.base_color.a = 1.0;
    apply_decals(&pbr);
    return deferred_output(input, pbr);
#else
    pbr_prepass_functions::prepass_alpha_discard(input);
    var out: FragmentOutput;
#ifdef UNCLIPPED_DEPTH_ORTHO_EMULATION
    out.frag_depth = input.unclipped_depth;
#endif
#ifdef NORMAL_PREPASS
    out.normal = vec4<f32>(pbr_functions::prepare_world_normal(input.world_normal, true, front) * 0.5 + vec3<f32>(0.5), 1.0);
#endif
#ifdef MOTION_VECTOR_PREPASS
    out.motion_vector = pbr_prepass_functions::calculate_motion_vector(input.world_position, input.previous_world_position);
#endif
    return out;
#endif
}
#else
@fragment
fn fragment(input: VertexOutput) {
    crop_coverage(input);
    pbr_prepass_functions::prepass_alpha_discard(input);
}
#endif
#else
@fragment
fn fragment(input: VertexOutput, @builtin(front_facing) front: bool) -> FragmentOutput {
    crop_coverage(input);
#ifdef VISIBILITY_RANGE_DITHER
    pbr_functions::visibility_range_dither(input.position, input.visibility_range_dither);
#endif
    var pbr = pbr_input_from_standard_material(input, front);
    pbr.material.base_color = pbr_functions::alpha_discard(pbr.material, pbr.material.base_color);
    pbr.material.base_color.a = 1.0;
    apply_decals(&pbr);
    var out: FragmentOutput;
    if (pbr.material.flags & STANDARD_MATERIAL_FLAGS_UNLIT_BIT) == 0u {
        out.color = pbr_functions::apply_pbr_lighting(pbr);
    } else {
        out.color = pbr.material.base_color;
    }
    out.color = pbr_functions::main_pass_post_lighting_processing(pbr, out.color);
#ifdef OIT_ENABLED
    let mode = pbr.material.flags & pbr_types::STANDARD_MATERIAL_FLAGS_ALPHA_MODE_RESERVED_BITS;
    if mode != pbr_types::STANDARD_MATERIAL_FLAGS_ALPHA_MODE_OPAQUE {
        oit_draw(input.position, out.color);
        discard;
    }
#endif
    return out;
}
#endif
