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
#import bevy_pbr::{pbr_bindings, mesh_bindings::mesh}
struct TerraWeather {
    wind: vec4<f32>,
    surface: vec4<f32>,
    tint: vec4<f32>,
    leaves: vec4<f32>,
    grass: vec4<f32>,
};
@group(#{MATERIAL_BIND_GROUP}) @binding(101) var<storage, read> terra_weather: TerraWeather;
// One parameter block per shared material, not per terrain cell.
@group(#{MATERIAL_BIND_GROUP}) @binding(102) var<uniform> terra_detail: vec4<f32>;

fn terrain_opacity(input: VertexOutput) -> f32 {
#ifdef BINDLESS
    let slot = mesh[input.instance_index].material_and_lightmap_bind_group_slot & 0xffffu;
    return pbr_bindings::material_array[pbr_bindings::material_indices[slot].material].base_color.a;
#else
    return pbr_bindings::material.base_color.a;
#endif
}

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

// Height, contact occlusion and albedo variation. World-anchored features survive
// greedy faces, negative coordinates, section reloads and terrain LOD changes.
fn block_profile(p: vec2<f32>, family: f32) -> vec3<f32> {
    let grain = wet_hash(floor(p * 96.0));
    var height = 0.0;
    var contact = 1.0;
    var tint = 1.0;
    if family < 1.5 {
        let row = floor(p.y * 4.0);
        let q = p * vec2<f32>(3.0,4.0) + vec2<f32>(wet_hash(vec2<f32>(row,13.0)) * 1.3,0.0);
        let cell = floor(q);
        let tile = fract(q);
        let mineral = wet_hash(cell);
        let edge = min(min(tile.x,1.0-tile.x),min(tile.y,1.0-tile.y));
        let ridge = smoothstep(0.005,0.055+mineral*0.045,edge);
        let fault = (1.0 - smoothstep(0.004,0.018,
            abs(tile.y-0.32-mineral*0.25-tile.x*(0.10+mineral*0.22))))
            * step(0.54,mineral);
        let pit = 1.0 - smoothstep(0.025,0.10,
            length(tile-vec2<f32>(0.28+mineral*0.28,0.63-mineral*0.20)));
        height = ridge * (0.015 + mineral * 0.014) - fault * 0.004 - pit*0.003;
        contact = mix(0.86,1.0,ridge) - fault*0.04 - pit*0.025;
        tint = 0.94 + mineral*0.12;
    } else if family < 2.5 {
        let q = p * 9.0;
        let soil = wet_hash(floor(q));
        let clod = 1.0 - smoothstep(0.23,0.55,
            length((fract(q)-vec2<f32>(0.5))*vec2<f32>(1.0,0.75+soil*0.5)));
        height = clod * (0.008+soil*0.005) + grain*0.001;
        contact = 0.93 + clod*0.07;
        tint = 0.95 + soil*0.10;
    } else if family < 3.5 {
        let turf = wet_noise(p*15.0);
        height = turf*0.006 + grain*0.0007;
        tint = 0.95 + turf*0.10;
    } else if family < 4.5 {
        let ripple = sin(p.y*31.0+sin(p.x*7.0)*0.6);
        height = ripple*0.0012 + grain*0.00035;
        tint = 0.985 + ripple*0.012 + grain*0.015;
    } else {
        let drift = wet_noise(p * 6.0);
        height = drift*0.0025 + grain*0.0003;
        tint = 0.985 + drift*0.025;
    }
    return vec3<f32>(height,contact,tint);
}
#ifdef VISIBILITY_RANGE_DITHER
#import bevy_pbr::pbr_functions::visibility_range_dither
#endif

@fragment
fn fragment(input: VertexOutput, @builtin(front_facing) front: bool) -> FragmentOutput {
    let climate = terra_weather.surface;
#ifdef VISIBILITY_RANGE_DITHER
    visibility_range_dither(input.position, input.visibility_range_dither);
#endif
    var pbr = pbr_input_from_standard_material(input, front);
#ifdef VERTEX_COLORS
#ifdef VERTEX_UVS_B
    // Vertex alpha is climate metadata, not coverage. Keep the actual shared
    // material opacity: incoming chunks must remain invisible at fade zero.
    pbr.material.base_color.a = terrain_opacity(input);
#endif
#endif
    pbr.material.base_color = alpha_discard(pbr.material, pbr.material.base_color);
    apply_decals(&pbr);
    if terra_detail.x > 0.5 {
        let n = pbr.N;
        let absolute = abs(n);
        var uv = input.world_position.xy;
        if absolute.y > max(absolute.x,absolute.z) { uv = input.world_position.xz; }
        else if absolute.x > absolute.z { uv = input.world_position.zy; }
        let footprint = max(length(dpdx(uv)),length(dpdy(uv)));
        let detail_fade = 1.0 - smoothstep(0.012,0.11,footprint);
        let profile = block_profile(uv,terra_detail.x);
        let height = profile.x * detail_fade;
        let dx = dpdx(input.world_position.xyz);
        let dy = dpdy(input.world_position.xyz);
        let r1 = cross(dy,n);
        let r2 = cross(n,dx);
        let determinant = dot(dx,r1);
        let gradient = (dpdx(height)*r1+dpdy(height)*r2) / max(abs(determinant),0.00000001) * sign(determinant);
        pbr.N = normalize(n-gradient*terra_detail.y*terra_detail.w*0.65);
        pbr.material.base_color = vec4<f32>(pbr.material.base_color.rgb
            * mix(1.0,profile.y*profile.z,terra_detail.z*detail_fade),pbr.material.base_color.a);
        // Fine sand and snow remain matte; no noisy metallic sparkle or bloom.
        pbr.material.perceptual_roughness = max(pbr.material.perceptual_roughness,0.86);
    }
#ifdef VERTEX_UVS_B
    // The mesher carries authoritative, roof-aware surface water in UV1.
    // Indoor stone stays matte even while the surrounding biome is raining.
    let exposed = clamp(input.uv_b.y, 0.0, 1.0);
    let shore = clamp(input.uv_b.y - 1.0, 0.0, 1.0);
    let sheds_snow = floor(max(input.uv_b.x, 0.0) / 2048.0);
    let eligible_surface = input.uv_b.x - sheds_snow * 2048.0;
    let snow_sky_override = floor(max(eligible_surface, 0.0) / 1024.0);
    let saved_surface = eligible_surface - snow_sky_override * 1024.0;
    let snow_byte = floor(max(saved_surface, 0.0) * 0.5);
    let saved_snow = snow_byte / 255.0;
    let saved_water = saved_surface - snow_byte * 2.0;
    var amount = max(clamp(saved_water, 0.0, 1.0), climate.x * exposed);
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
    var frost = select(0.0, 1.0, saved_snow > 0.0);
#ifdef VERTEX_COLORS
    // Opaque terrain alpha carries local climate, including through Unorm8 packing.
    if input.color.a > 0.0 && input.color.a < 1.0 {
        let temperature = ((input.color.a * 255.0 - 2.0) / 251.0) * 128.0 - 64.0
            + terra_weather.tint.w;
        frost = 1.0 - smoothstep(-1.0, 3.0, temperature);
    }
#endif
    // Natural deciduous crowns can shed while the original rain roof remains intact.
    let snow_sky = max(exposed, snow_sky_override);
    let snow_amount = max(saved_snow, terra_weather.grass.w * frost * snow_sky) * (1.0 - sheds_snow);
    let grass_fringe = select(0.0, 1.0, abs(terra_detail.x) == 3.0
        && pbr.material.base_color.g > pbr.material.base_color.r * 1.12);
    let snow_cover = max(smoothstep(0.0, 0.95, snow_amount) * smoothstep(0.15, 0.65, pbr.N.y),
        grass_fringe * smoothstep(0.0, 0.10, snow_amount));
    if snow_cover > 0.0 {
        // Texture changes albedo only: complete cover never contains grass-shaped holes.
        let grain = 0.96 + wet_noise(p * 7.0) * 0.04;
        pbr.material.base_color = vec4<f32>(
            mix(pbr.material.base_color.rgb, vec3<f32>(0.88, 0.91, 0.95) * grain, snow_cover),
            pbr.material.base_color.a);
        pbr.material.perceptual_roughness = mix(pbr.material.perceptual_roughness, 0.92, snow_cover);
        pbr.material.reflectance = mix(pbr.material.reflectance, vec3<f32>(0.45), snow_cover);
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
