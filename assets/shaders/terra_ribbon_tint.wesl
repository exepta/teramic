import bevy_pbr::render::pbr_fragment::pbr_input_from_standard_material;
import bevy_pbr::render::pbr_functions::alpha_discard;
import bevy_pbr::render::mesh_functions::get_tag;
import bevy_pbr::decal::clustered::apply_decals;
@if(PREPASS_PIPELINE)
import bevy_pbr::prepass::io::{VertexOutput, FragmentOutput};
@if(PREPASS_PIPELINE)
import bevy_pbr::deferred::functions::deferred_output;
@if(!PREPASS_PIPELINE)
import bevy_pbr::render::forward_io::{VertexOutput, FragmentOutput};
@if(!PREPASS_PIPELINE)
import bevy_pbr::render::pbr_functions::{apply_pbr_lighting, main_pass_post_lighting_processing};
@if(!PREPASS_PIPELINE)
import bevy_pbr::render::pbr_types::STANDARD_MATERIAL_FLAGS_UNLIT_BIT;
@if(VISIBILITY_RANGE_DITHER)
import bevy_pbr::render::pbr_functions::visibility_range_dither;

fn linear_channel(srgb: f32) -> f32 {
    if srgb <= 0.04045 {
        return srgb / 12.92;
    }
    return pow((srgb + 0.055) / 1.055, 2.4);
}

@fragment
fn fragment(input: VertexOutput, @builtin(front_facing) front: bool) -> FragmentOutput {
@if(VISIBILITY_RANGE_DITHER) {
    visibility_range_dither(input.position, input.visibility_range_dither);
}
    var pbr = pbr_input_from_standard_material(input, front);
    let tag = get_tag(input.instance_index);
    if (tag & 0x01000000u) != 0u {
        let srgb = vec3<f32>(f32(tag & 255u), f32((tag >> 8u) & 255u), f32((tag >> 16u) & 255u)) / 255.0;
        let rgb = vec3<f32>(linear_channel(srgb.r), linear_channel(srgb.g), linear_channel(srgb.b));
        pbr.material.base_color = vec4<f32>(pbr.material.base_color.rgb * rgb, pbr.material.base_color.a);
    }
    pbr.material.base_color = alpha_discard(pbr.material.flags, pbr.material.alpha_cutoff, pbr.material.base_color);
    apply_decals(&pbr);
@if(PREPASS_PIPELINE) {
    return deferred_output(input, pbr);
} @else {
    var out: FragmentOutput;
    if (pbr.material.flags & STANDARD_MATERIAL_FLAGS_UNLIT_BIT) == 0u {
        out.color = apply_pbr_lighting(pbr);
    } else {
        out.color = pbr.material.base_color;
    }
    out.color = main_pass_post_lighting_processing(pbr, out.color);
    return out;
}
}
