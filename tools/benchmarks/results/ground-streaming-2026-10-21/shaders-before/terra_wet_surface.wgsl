#import bevy_pbr::{
    pbr_functions::alpha_discard,
    pbr_fragment::pbr_input_from_standard_material,
    decal::clustered::apply_decals,
}
#ifdef PREPASS_PIPELINE
#import bevy_pbr::{prepass_io::{VertexOutput, FragmentOutput}, pbr_deferred_functions::deferred_output}
#else
#import bevy_pbr::{
    forward_io::{VertexOutput, FragmentOutput},
    pbr_functions::{apply_pbr_lighting, main_pass_post_lighting_processing},
    pbr_types::STANDARD_MATERIAL_FLAGS_UNLIT_BIT,
}
#endif

#import bevy_pbr::mesh_view_bindings::globals
@group(#{MATERIAL_BIND_GROUP}) @binding(100) var<uniform> climate: vec4<f32>;

fn wet_hash(p: vec2<f32>) -> f32 {
    var h = bitcast<u32>(i32(p.x)) * 0x8da6b343u ^ bitcast<u32>(i32(p.y)) * 0xd8163841u;
    h = (h ^ (h >> 16u)) * 0x7feb352du;
    h = (h ^ (h >> 15u)) * 0x846ca68bu;
    return f32((h ^ (h >> 16u)) & 0x00ffffffu) / 16777216.0;
}
fn wet_noise(p: vec2<f32>) -> f32 {
    let cell = floor(p);
    let f = fract(p);
    let t = f * f * (3.0 - 2.0 * f);
    return mix(mix(wet_hash(cell), wet_hash(cell + vec2<f32>(1.0, 0.0)), t.x),
        mix(wet_hash(cell + vec2<f32>(0.0, 1.0)), wet_hash(cell + vec2<f32>(1.0)), t.x), t.y);
}
fn shore_pulse(p: vec2<f32>, time: f32) -> f32 {
    return smoothstep(-0.35, 0.95, sin(time * 0.48 + wet_noise(p * 0.035) * 1.4));
}
#ifdef VISIBILITY_RANGE_DITHER
#import bevy_pbr::pbr_functions::visibility_range_dither
#endif

@fragment
fn fragment(input: VertexOutput, @builtin(front_facing) front: bool) -> FragmentOutput {
#ifdef VISIBILITY_RANGE_DITHER
    visibility_range_dither(input.position, input.visibility_range_dither);
#endif
    var pbr = pbr_input_from_standard_material(input, front);
    pbr.material.base_color = alpha_discard(pbr.material, pbr.material.base_color);
    apply_decals(&pbr);
#ifdef VERTEX_UVS_B
    // The mesher carries authoritative, roof-aware surface water in UV1.
    // Indoor stone stays matte even while the surrounding biome is raining.
    let exposed = clamp(input.uv_b.y, 0.0, 1.0);
    let shore = clamp(input.uv_b.y - 1.0, 0.0, 1.0);
    var amount = max(clamp(input.uv_b.x, 0.0, 1.0), climate.x * exposed);
    // Negative exposure is an explicit submerged surface, not a dry indoor face.
    // Even a previously saved rain condition must not turn the lake bed glossy.
    if input.uv_b.y < -0.5 { amount = 0.0; }
    // During rainfall every exposed face follows the same smooth world clock,
    // not the order in which budgeted column snapshots happen to finish.
    if exposed > 0.5 && climate.y > 0.05 { amount = climate.x; }
    let p = input.world_position.xz;
    let footprint = max(length(dpdx(p)), length(dpdy(p)));
    if amount > 0.001 || shore > 0.001 {
    // World-space basins span faces/sections; no cell hash and no whole-block tint.
    let basin = wet_noise(p * 0.72) * 0.72 + wet_noise(p * 2.7 + vec2<f32>(17.0, 31.0)) * 0.28;
    let coverage = smoothstep(basin - 0.075, basin + 0.075, amount * 0.88);
    let retained_wash = max(shore_pulse(p, globals.time), shore_pulse(p, globals.time - 2.4)) * shore;
    let wet = max(coverage * amount, retained_wash * 0.82);
    let upward = smoothstep(0.05, 0.85, pbr.N.y);
    let film = wet * mix(0.22, 1.0, upward);
    pbr.material.base_color = vec4<f32>(pbr.material.base_color.rgb * (1.0 - film * 0.22), pbr.material.base_color.a);
    // Broad damp sheen, never a metallic mirror or glowing rock surface.
    pbr.material.perceptual_roughness = mix(pbr.material.perceptual_roughness, 0.19, film * film);
    pbr.material.reflectance = mix(pbr.material.reflectance, vec3<f32>(0.62), film);
    let ripple = sin(length(fract(p * 1.6) - vec2<f32>(0.5)) * 42.0 - globals.time * 9.0)
        * climate.y * film * upward * (1.0 - smoothstep(0.08, 0.45, footprint));
    pbr.N = normalize(pbr.N + vec3<f32>(ripple * 0.018, 0.0, ripple * 0.012));
    }
#endif
#ifdef PREPASS_PIPELINE
    return deferred_output(input, pbr);
#else
    var out: FragmentOutput;
    if (pbr.material.flags & STANDARD_MATERIAL_FLAGS_UNLIT_BIT) == 0u {
        out.color = apply_pbr_lighting(pbr);
    } else { out.color = pbr.material.base_color; }
    out.color = main_pass_post_lighting_processing(pbr, out.color);
    return out;
#endif
}
