import bevy_pbr::render::pbr_functions::alpha_discard;
import bevy_pbr::render::pbr_fragment::pbr_input_from_standard_material;
import bevy_pbr::decal::clustered::apply_decals;
@if(PREPASS_PIPELINE)
import bevy_pbr::prepass::io::VertexOutput;
@if(PREPASS_PIPELINE)
import bevy_pbr::prepass::io::FragmentOutput;
@if(PREPASS_PIPELINE)
import bevy_pbr::deferred::functions::deferred_output;
@if(!(PREPASS_PIPELINE))
import bevy_pbr::render::forward_io::VertexOutput;
@if(!(PREPASS_PIPELINE))
import bevy_pbr::render::forward_io::FragmentOutput;
@if(!(PREPASS_PIPELINE))
import bevy_pbr::render::pbr_functions::apply_pbr_lighting;
@if(!(PREPASS_PIPELINE))
import bevy_pbr::render::pbr_functions::main_pass_post_lighting_processing;
@if(!(PREPASS_PIPELINE))
import bevy_pbr::render::pbr_types::STANDARD_MATERIAL_FLAGS_UNLIT_BIT;
import bevy_pbr::render::mesh_view_bindings::globals;
import bevy_pbr::render::pbr_bindings;
import bevy_pbr::render::mesh_bindings::mesh;
@if(VISIBILITY_RANGE_DITHER)
import bevy_pbr::render::pbr_functions::visibility_range_dither;

@if(BINDLESS)
enable wgpu_binding_array;

struct TerraWeather {
    wind: vec4<f32>,
    surface: vec4<f32>,
    tint: vec4<f32>,
    leaves: vec4<f32>,
    grass: vec4<f32>,
    vegetation: vec4<f32>,
    flowers: array<vec4<f32>, 64>,
    footprints: array<vec4<f32>, 1024>,
    liquid: vec4<f32>,
    particle: vec4<f32>,
};
@if(BINDLESS) {
struct WetMaterialIndices {
    weather: u32,
    detail: u32,
};
struct WetParameters {
    detail: vec4<f32>,
};
struct WetWeatherInputs {
    surface: vec4<f32>,
    tint: vec4<f32>,
    grass: vec4<f32>,
    vegetation: vec4<f32>,
};
@group(constants::MATERIAL_BIND_GROUP) @binding(100) var<storage, read> wet_indices: array<WetMaterialIndices>;
@group(constants::MATERIAL_BIND_GROUP) @binding(101) var<storage, read> wet_weather: binding_array<TerraWeather>;
@group(constants::MATERIAL_BIND_GROUP) @binding(102) var<storage, read> wet_parameters: array<WetParameters>;
} @else {
@group(constants::MATERIAL_BIND_GROUP) @binding(101) var<storage, read> terra_weather: TerraWeather;
// The non-bindless fallback retains the original shared parameter bindings.
@group(constants::MATERIAL_BIND_GROUP) @binding(102) var<uniform> terra_detail: vec4<f32>;
}

fn terrain_opacity(input: VertexOutput) -> f32 {
@if(BINDLESS) {
    let slot = mesh[input.instance_index].material_and_lightmap_bind_group_slot & 0xffffu;
    return pbr_bindings::material_array[pbr_bindings::material_indices[slot].material].base_color.a;
} @else {
    return pbr_bindings::material.base_color.a;
}
}

fn imprint_cutout(p: vec3<f32>, slot: u32) -> bool {
    let cell = vec2<i32>(floor(p.xz * 2.0));
    let bucket = ((bitcast<u32>(cell.x) * 0x8da6b343u) ^ (bitcast<u32>(cell.y) * 0xd8163841u)) % 128u;
    for (var i = 0u; i < 4u; i += 1u) {
        let index = bucket * 8u + i * 2u;
@if(BINDLESS) {
        let a = wet_weather[wet_indices[slot].weather].footprints[index];
        let b = wet_weather[wet_indices[slot].weather].footprints[index + 1u];
} @else {
        let a = terra_weather.footprints[index];
        let b = terra_weather.footprints[index + 1u];
}
        if a.w <= 0.0 { break; }
        let delta = p.xz - a.xy;
        if abs(p.y - a.z) < 0.006 && abs(dot(delta, b.xy)) < a.w
            && abs(dot(delta, vec2<f32>(-b.y, b.x))) < b.z { return true; }
    }
    return false;
}

fn wet_hash(p: vec2<f32>) -> f32 {
    var h = (bitcast<u32>(i32(p.x)) * 0x8da6b343u) ^ (bitcast<u32>(i32(p.y)) * 0xd8163841u);
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

fn authored_grass_top_palette(color: vec3<f32>) -> vec3<f32> {
    // Retain fine painted structure without the broad dark/light patchwork.
    return mix(color, vec3<f32>(0.17, 0.30, 0.045), 0.32);
}

// Height, contact occlusion and albedo variation. World-anchored features survive
// greedy faces, negative coordinates, section reloads and terrain LOD changes.
fn block_profile(p: vec2<f32>, family: f32) -> vec3<f32> {
    let grain = wet_hash(floor(p * 96.0));
    var height = 0.0;
    var contact = 1.0;
    var tint = 1.0;
    if family > 12.5 {
        // Painted opaque construction cubes retain their own albedo/roughness.
        // Tiny world-space grain adds tactile relief without a block outline.
        let fine = wet_noise(p * 42.0 + vec2<f32>(3.7, 19.2));
        height = fine * 0.0007;
        tint = 0.992 + fine * 0.016;
    } else if family > 11.5 {
        let weave_a = sin(p.x * 112.0);
        let weave_b = sin(p.y * 112.0 + 0.35);
        height = (weave_a * weave_b) * 0.0007;
        tint = 0.99 + wet_noise(p * 28.0) * 0.02;
    } else if family > 10.5 {
        let mineral = wet_noise(p * 12.0 + vec2<f32>(5.2, 13.7));
        let fine = wet_noise(p * 48.0);
        height = mineral * 0.0018 + fine * 0.0005;
        contact = 0.985 + mineral * 0.015;
        tint = 0.985 + fine * 0.03;
    } else if family > 9.5 {
        let grain_bend = wet_noise(p * vec2<f32>(2.3, 0.7));
        let fibers = sin(p.x * 78.0 + grain_bend * 2.2);
        height = fibers * 0.0010 + wet_noise(p * 34.0) * 0.0003;
        tint = 0.99 + fibers * 0.008 + wet_noise(p * 18.0) * 0.02;
    } else if family < 1.5 {
        // Low-contrast mineral grain, not a brick grid drawn over the painted stone.
        let mineral = wet_noise(p * 2.7 + vec2<f32>(17.2, 8.6));
        let fracture = abs(wet_noise(p * 6.3 + vec2<f32>(mineral * 0.65)) - 0.5);
        let fault = (1.0 - smoothstep(0.008, 0.035, fracture))
            * smoothstep(0.5, 0.75, mineral);
        height = mineral * 0.005 - fault * 0.001;
        contact = 1.0 - fault * 0.035;
        tint = 0.975 + mineral * 0.05;
    } else if family < 2.5 {
        let q = p * 9.0;
        let soil = wet_hash(floor(q));
        let clod = 1.0 - smoothstep(0.23,0.55,
            length((fract(q)-vec2<f32>(0.5))*vec2<f32>(1.0,0.75+soil*0.5)));
        height = clod * (0.002+soil*0.0015) + grain*0.0003;
        contact = 0.98 + clod*0.02;
        tint = 0.98 + soil*0.04;
    } else if family < 3.5 {
        let turf = wet_noise(p*15.0);
        height = turf*0.0008 + grain*0.00015;
        tint = 0.985 + turf*0.03;
    } else if family < 4.5 {
        let ripple = sin(p.y * 31.0 + sin(p.x * 7.0) * 0.6);
        let grains = wet_noise(p * 64.0 + vec2<f32>(11.7, 4.3));
        let packed = wet_hash(floor(p * 24.0) + vec2<f32>(19.0, 7.0));
        height = ripple * 0.0018 + grains * 0.0012 + grain * 0.00035;
        contact = 0.965 + grains * 0.035;
        tint = 0.96 + grains * 0.055 + packed * 0.025;
    } else {
        let drift = wet_noise(p * 6.0);
        height = drift*0.0025 + grain*0.0003;
        tint = 0.985 + drift*0.025;
    }
    return vec3<f32>(height,contact,tint);
}


@fragment
fn fragment(input: VertexOutput, @builtin(front_facing) front: bool) -> FragmentOutput {
@if(VERTEX_UVS_B) {
@if(BINDLESS) {
    let imprint_slot = mesh[input.instance_index].material_and_lightmap_bind_group_slot & 0xffffu;
    let imprint_family = abs(wet_parameters[wet_indices[imprint_slot].detail].detail.x);
    let imprint_active = wet_weather[wet_indices[imprint_slot].weather].vegetation.w;
    let particle_domain = wet_weather[wet_indices[imprint_slot].weather].particle;
} @else {
    let imprint_slot = 0u;
    let imprint_family = abs(terra_detail.x);
    let imprint_active = terra_weather.vegetation.w;
    let particle_domain = terra_weather.particle;
}
    // Authoritative viscous flow geometry is the only world surface. Its
    // interpolated front and animated material follow the paid fluid transfers.
    if imprint_active > 0.5 && (imprint_family == 4.0 || imprint_family == 5.0) && input.world_normal.y > 0.98
        && imprint_cutout(input.world_position.xyz, imprint_slot) { discard; }
}
@if(VISIBILITY_RANGE_DITHER) {
    visibility_range_dither(input.position, input.visibility_range_dither);
}
    var pbr = pbr_input_from_standard_material(input, front);
@if(VERTEX_COLORS) {
@if(VERTEX_UVS_B) {
    // Vertex alpha is climate metadata, not coverage. Keep the actual shared
    // material opacity: incoming chunks must remain invisible at fade zero.
    pbr.material.base_color.a = terrain_opacity(input);
}
}
    pbr.material.base_color = alpha_discard(pbr.material.flags, pbr.material.alpha_cutoff, pbr.material.base_color);
    apply_decals(&pbr);
@if(VERTEX_UVS_B) {
@if(BINDLESS) {
    let slot = mesh[input.instance_index].material_and_lightmap_bind_group_slot & 0xffffu;
    let indices = wet_indices[slot];
    let terra_detail = wet_parameters[indices.detail].detail;
    let terra_weather = WetWeatherInputs(wet_weather[indices.weather].surface,
        wet_weather[indices.weather].tint, wet_weather[indices.weather].grass,
        wet_weather[indices.weather].vegetation);
}
    // Dry meshes share this material for correct roof shadows, but retain the
    // original StandardMaterial color path without procedural detail work.
    let climate = terra_weather.surface;
    if terra_detail.x == -14.0 && input.uv_b.y < -3.5 {
        // Slow travelling capillary folds, anchored in world space across cells.
        // Surface volume stays authoritative; animated reflection is continuous.
        let p = input.world_position.xz;
        let time = globals.time * 0.65;
        let a = sin(p.x * 5.7 + sin(p.y * 3.1 - time) * 0.8 - time);
        let b = cos(p.y * 6.3 + sin(p.x * 3.7 + time) * 0.6 - time * 0.8);
        let top = max(normalize(input.world_normal).y, 0.0);
        pbr.N = normalize(pbr.N + vec3<f32>(a, 0.0, b) * 0.13 * top);
        let sheen = sin(p.x * 9.1 + p.y * 6.7 - time * 1.7) * 0.025;
        pbr.material.base_color = vec4<f32>(pbr.material.base_color.rgb * (0.97 + sheen), pbr.material.base_color.a);
        pbr.material.perceptual_roughness = 0.18 + 0.035 * a;
    }
    if terra_detail.x == -7.0 && input.uv_b.y < -2.5 {
        // Viscous motion changes the reflected normal, never the collision
        // height. World anchoring keeps adjacent films continuous at chunk edges.
        let cured = clamp(input.uv_b.x, 0.0, 1.0);
        let liquid = 1.0 - cured;
        let p = input.world_position.xz;
        let phase = globals.time * 0.22;
        let fold = sin(p.x * 8.1 + sin(p.y * 3.7 - phase) * 0.65 + phase);
        let cross_fold = cos(p.y * 7.3 + sin(p.x * 4.2 + phase) * 0.55 - phase);
        let top = max(normalize(input.world_normal).y, 0.0);
        pbr.N = normalize(pbr.N + vec3<f32>(fold, 0.0, cross_fold) * 0.045 * liquid * top);
        pbr.material.perceptual_roughness = mix(0.16, 0.88, cured);
        pbr.material.base_color = vec4<f32>(pbr.material.base_color.rgb * mix(0.72, 1.0, cured),
            pbr.material.base_color.a);
    }
    var calendar_turf = false;
    if terra_detail.x == -6.0 || terra_detail.x == -8.0 || terra_detail.x == -9.0 {
        var vertex_tint = vec3<f32>(1.0);
@if(VERTEX_COLORS) {
        vertex_tint = max(input.color.rgb, vec3<f32>(0.0001));
}
        let source_color = pbr.material.base_color.rgb / vertex_tint;
        let green_texel = source_color.g > source_color.r * 1.12
            && source_color.g > source_color.b * 1.12;
        // Use geometry, not the normal map, and exclude horizontal lips on sides.
        let top_face = normalize(input.world_normal).y > 0.86
            && abs(input.world_position.y - round(input.world_position.y)) < 0.035;
        let side_face = abs(normalize(input.world_normal).y) < 0.86;
        if terra_detail.x == -6.0 && green_texel && (top_face || side_face) {
            calendar_turf = true;
            pbr.material.base_color = vec4<f32>(authored_grass_top_palette(source_color)
                * vertex_tint, pbr.material.base_color.a);
        }
        if terra_detail.x == -8.0 || terra_detail.x == -9.0 {
            // Spirit/Blood turf used to bypass the climate field and jump at
            // the block-ID threshold. Normalize the paint, retaining every
            // grain/vein, then apply the same interpolated tint as meadow turf.
            let spirit = terra_detail.x == -8.0;
            let cap_texel = select(source_color.b > source_color.g * 1.24,
                source_color.g > source_color.r * 1.6 && source_color.b > source_color.r * 1.6,
                spirit);
            let turf = top_face || (side_face && cap_texel);
            calendar_turf = turf;
            if top_face || side_face {
                let reference = select(vec3<f32>(0.423243, 0.103447, 0.157820),
                    vec3<f32>(0.081140, 0.450920, 0.386200), spirit);
                let soil_reference = select(vec3<f32>(0.204663, 0.089321, 0.098386),
                    vec3<f32>(0.319667, 0.157852, 0.270654), spirit);
                let meadow_turf = vec3<f32>(0.155556, 0.275173, 0.033548);
                let meadow_soil = vec3<f32>(0.245898, 0.130539, 0.055692);
                let normalized = source_color * select(meadow_soil / soil_reference,
                    meadow_turf / reference, turf);
                pbr.material.base_color = vec4<f32>(normalized * vertex_tint,
                    pbr.material.base_color.a);
            }
        }
    }
    // Select living turf before biome recoloring. Cyan and crimson grass must
    // follow the same calendar as green turf; soil keeps its mineral albedo.
    if calendar_turf {
        pbr.material.base_color = vec4<f32>(pbr.material.base_color.rgb * terra_weather.grass.rgb,
            pbr.material.base_color.a);
    }
    if terra_detail.y < 0.5 && terra_detail.x != -14.0 {
        pbr.N = normalize(input.world_normal);
    }
    if terra_detail.x > 0.5 {
        let n = pbr.N;
        let absolute = abs(n);
        var uv = input.world_position.xy;
        if absolute.y > max(absolute.x,absolute.z) { uv = input.world_position.xz; }
        else if absolute.x > absolute.z { uv = input.world_position.zy; }
        let footprint = max(length(dpdx(uv)),length(dpdy(uv)));
        let detail_fade = 1.0 - smoothstep(0.012,0.11,footprint);
        // Most terrain pixels are beyond fine relief distance. Skip all noise
        // there, and when both detail outputs are disabled. Only the pure
        // profile work branches: derivatives stay in the reconverged scope.
        var profile = vec3<f32>(0.0, 1.0, 1.0);
        if detail_fade > 0.0001 && (terra_detail.y > 0.0 || terra_detail.z > 0.0) {
            profile = block_profile(uv,terra_detail.x);
        }
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
        // Natural terrain remains matte; worked surfaces keep authored finish.
        if terra_detail.x < 6.0 {
            pbr.material.perceptual_roughness = max(pbr.material.perceptual_roughness,0.86);
        }
    }
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
@if(VERTEX_COLORS) {
    // Opaque terrain alpha carries local climate, including through Unorm8 packing.
    if input.color.a > 0.0 && input.color.a < 1.0 {
        let temperature = ((input.color.a * 255.0 - 2.0) / 251.0) * 128.0 - 64.0
            + terra_weather.tint.w;
        frost = 1.0 - smoothstep(-1.0, 3.0, temperature);
    }
}
    // Natural deciduous crowns can shed while the original rain roof remains intact.
    let snow_sky = max(exposed, snow_sky_override);
    // Real worlds use local authority only. Global frost is an explicit isolated-gallery preview.
    let preview_snow = terra_weather.grass.w * frost * snow_sky * clamp(terra_weather.vegetation.y, 0.0, 1.0);
    let snow_amount = max(saved_snow, preview_snow) * (1.0 - sheds_snow);
    let grass_fringe = select(0.0, 1.0, (abs(terra_detail.x) == 3.0 || terra_detail.x == -6.0)
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
}
@if(PREPASS_PIPELINE) {
    return deferred_output(input, pbr);
} @else {
    var out: FragmentOutput;
    if (pbr.material.flags & STANDARD_MATERIAL_FLAGS_UNLIT_BIT) == 0u {
        out.color = apply_pbr_lighting(pbr);
    } else { out.color = pbr.material.base_color; }
    out.color = main_pass_post_lighting_processing(pbr, out.color);
    return out;
}
}
