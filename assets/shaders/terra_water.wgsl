#import bevy_pbr::{
    forward_io::{Vertex, VertexOutput, FragmentOutput},
    mesh_functions,
    pbr_fragment::pbr_input_from_standard_material,
    pbr_functions::{apply_pbr_lighting, main_pass_post_lighting_processing},
    mesh_view_bindings::{globals, view},
    view_transformations::{position_ndc_to_world, position_world_to_clip},
    transmission::fetch_transmissive_background_non_rough,
}
#ifdef DEPTH_PREPASS
#import bevy_pbr::prepass_utils::prepass_depth
#import bevy_pbr::{
    mesh_view_bindings::depth_prepass_texture,
    view_transformations::position_world_to_ndc,
}
#ifdef WATER_NATIVE_DEPTH_FILTER
#import terra::water_raymarch::{
    depth_ray_march_new_from_depth,
    depth_ray_march_from_cs,
    depth_ray_march_to_ws,
    depth_ray_march_march,
}
#else
#import bevy_pbr::raymarch::{
        depth_ray_march_new_from_depth,
        depth_ray_march_from_cs,
        depth_ray_march_to_ws,
        depth_ray_march_march,
}
#endif
#endif

@group(#{MATERIAL_BIND_GROUP}) @binding(100) var<uniform> tint: vec4<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(101) var<uniform> settings: vec4<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(102) var<uniform> effects: vec4<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(103) var<uniform> reflections: vec4<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(104) var<uniform> weather: vec4<f32>;

// Keep the stock forward fields so lighting and chunk fades use the same path.
// The extra varying leaves all depth/current/foam metadata intact.
struct WaterVertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) world_position: vec4<f32>,
    @location(1) world_normal: vec3<f32>,
#ifdef VERTEX_UVS_A
    @location(2) uv: vec2<f32>,
#endif
#ifdef VERTEX_UVS_B
    @location(3) uv_b: vec2<f32>,
#endif
#ifdef VERTEX_TANGENTS
    @location(4) world_tangent: vec4<f32>,
#endif
#ifdef VERTEX_COLORS
    @location(5) color: vec4<f32>,
#endif
#ifdef VERTEX_OUTPUT_INSTANCE_INDEX
    @location(6) @interpolate(flat) instance_index: u32,
#endif
#ifdef VISIBILITY_RANGE_DITHER
    @location(7) @interpolate(flat) visibility_range_dither: i32,
#endif
    @location(8) biome_tint: vec3<f32>,
}

@vertex
fn vertex(input: Vertex,
#ifdef WATER_BIOME_TINT
    @location(8) biome_tint: vec3<f32>,
#endif
#ifdef WATER_WAVE_WEIGHT
    @location(9) wave_weight: f32,
#endif
) -> WaterVertexOutput {
    var out: WaterVertexOutput;
    let model = mesh_functions::get_world_from_local(input.instance_index);
    out.world_position = mesh_functions::mesh_position_local_to_world(model, vec4<f32>(input.position, 1.0));
#ifdef WATER_WAVE_WEIGHT
    // The async mesher supplies corner-shared weights. Lip/impact quads taper
    // into anchored sheets; per-cell flow/mask values never split shared edges.
    out.world_position.y += wave_height(out.world_position.xz, globals.time * settings.x)
        * wave_weight;
#endif
    out.position = position_world_to_clip(out.world_position.xyz);
    out.world_normal = mesh_functions::mesh_normal_local_to_world(input.normal, input.instance_index);
#ifdef VERTEX_UVS_A
    out.uv = input.uv;
#endif
#ifdef VERTEX_UVS_B
    out.uv_b = input.uv_b;
#endif
#ifdef VERTEX_TANGENTS
    out.world_tangent = mesh_functions::mesh_tangent_local_to_world(model, input.tangent, input.instance_index);
#endif
#ifdef VERTEX_COLORS
    out.color = input.color;
#endif
#ifdef VERTEX_OUTPUT_INSTANCE_INDEX
    out.instance_index = input.instance_index;
#endif
#ifdef VISIBILITY_RANGE_DITHER
    out.visibility_range_dither = mesh_functions::get_visibility_range_dither_level(input.instance_index, model[3]);
#endif
#ifdef WATER_BIOME_TINT
    out.biome_tint = biome_tint;
#else
    // Inventory previews and loose item meshes have no terrain-biome payload.
    out.biome_tint = tint.rgb;
#endif
    return out;
}

fn standard_input(input: WaterVertexOutput) -> VertexOutput {
    var out: VertexOutput;
    out.position = input.position;
    out.world_position = input.world_position;
    out.world_normal = input.world_normal;
#ifdef VERTEX_UVS_A
    out.uv = input.uv;
#endif
#ifdef VERTEX_UVS_B
    out.uv_b = input.uv_b;
#endif
#ifdef VERTEX_TANGENTS
    out.world_tangent = input.world_tangent;
#endif
#ifdef VERTEX_COLORS
    // COLOR is water metadata, never a PBR color or ambient-occlusion factor.
    out.color = vec4<f32>(1.0);
#endif
#ifdef VERTEX_OUTPUT_INSTANCE_INDEX
    out.instance_index = input.instance_index;
#endif
#ifdef VISIBILITY_RANGE_DITHER
    out.visibility_range_dither = input.visibility_range_dither;
#endif
    return out;
}

fn hash(p: vec2<f32>) -> f32 {
    // Integer mixing avoids the large-coordinate precision bands of sin(dot()).
    var h = bitcast<u32>(i32(p.x)) * 0x8da6b343u ^ bitcast<u32>(i32(p.y)) * 0xd8163841u;
    h = (h ^ (h >> 16u)) * 0x7feb352du;
    h = (h ^ (h >> 15u)) * 0x846ca68bu;
    return f32((h ^ (h >> 16u)) & 0x00ffffffu) / 16777216.0;
}

fn noise(p: vec2<f32>) -> f32 {
    let cell = floor(p);
    let f = fract(p);
    let t = f * f * (3.0 - 2.0 * f);
    return mix(mix(hash(cell), hash(cell + vec2<f32>(1.0, 0.0)), t.x),
        mix(hash(cell + vec2<f32>(0.0, 1.0)), hash(cell + vec2<f32>(1.0)), t.x), t.y);
}

fn wave_slope(p: vec2<f32>, time: f32, footprint: f32) -> vec2<f32> {
    if effects.x == 0.0 { return vec2<f32>(0.0); }
    let waves = array<vec4<f32>, 3>(vec4<f32>(0.8, 0.6, 0.9, 0.075),
        vec4<f32>(-0.45, 0.893, 1.7, 0.034), vec4<f32>(0.96, -0.28, 3.8, 0.012));
    var slope = vec2<f32>(0.0);
    for (var i = 0u; i < u32(effects.y); i += 1u) {
        let w = waves[i];
        let phase = dot(p, w.xy) * w.z - time * sqrt(9.81 * w.z) + f32(i) * 2.4;
        let fade = 1.0 - smoothstep(0.5, 2.0, footprint * w.z);
        slope += w.xy * w.z * w.w * cos(phase) * fade;
    }
    if weather.z > 0.0 {
        let direction = weather.xy;
        let wind_phase = dot(p, direction) * 0.74 - time * (2.1 + weather.z * 2.0);
        slope += direction * cos(wind_phase) * weather.z * 0.28
            * (1.0 - smoothstep(0.7, 2.5, footprint));
        let cross_phase = dot(p, vec2<f32>(-direction.y, direction.x)) * 1.38 + time * 2.9;
        slope += vec2<f32>(-direction.y, direction.x) * cos(cross_phase) * weather.z * 0.12
            * (1.0 - smoothstep(0.35, 1.8, footprint));
    }
    return slope * effects.x * (1.0 + weather.z * 0.9);
}

fn wave_height(p: vec2<f32>, time: f32) -> f32 {
    let direction = weather.xy;
    let along = sin(dot(p, direction) * 0.74 - time * (2.1 + weather.z * 2.0));
    let cross = sin(dot(p, vec2<f32>(-direction.y, direction.x)) * 1.38 + time * 2.9);
    // Maximum storm displacement is 0.22m at the strongest user wave setting.
    return (along * (0.009 + weather.z * 0.073) + cross * weather.z * 0.028) * effects.x;
}

fn shore_front(p: vec2<f32>, time: f32) -> f32 {
    return smoothstep(-0.35, 0.95, sin(time * 0.48 + noise(p * 0.035) * 1.4));
}

fn current_slope(p: vec2<f32>, flow: vec2<f32>, time: f32, footprint: f32) -> vec2<f32> {
    // River paths run toward +Z. A shared phase avoids jumps at path segments;
    // their local direction only steers the ripple normal, never its coordinates.
    let phase = p.y * 5.0 - time * 7.5;
    let meander = sin(p.x * 1.8 + time * 0.6);
    let fade = 1.0 - smoothstep(0.18, 0.7, footprint);
    return flow * cos(phase + meander) * 0.085 * fade * effects.x;
}

fn waterfall_across(position: vec3<f32>, normal: vec3<f32>) -> f32 {
    // Keep noise phase fixed when curved height bands have different normals.
    return select(position.x, position.z, abs(normal.x) > abs(normal.z));
}

fn fall_foam(position: vec3<f32>, normal: vec3<f32>, effect: vec4<f32>, time: f32, footprint: f32) -> f32 {
    let across = waterfall_across(position, normal);
    let strands = noise(vec2<f32>(across * 3.1, position.y * 0.38 + time * 3.0));
    let streaks = smoothstep(0.48, 0.76, strands);
    let detail = 1.0 - smoothstep(0.3, 1.4, footprint);
    let crest = exp(-effect.x * 2.6);
    let impact = exp(-effect.y * 2.0);
    let breakup = noise(vec2<f32>(across * 1.5, time * 0.8));
    // The body stays translucent; concentrated whitewater belongs at the lip and foot.
    return clamp(0.04 + streaks * 0.48 * detail
        + crest * (0.18 + 0.42 * breakup) + impact * (0.38 + 0.48 * breakup), 0.0, 0.92);
}

fn scene_reflection(position: vec3<f32>, normal: vec3<f32>, view_direction: vec3<f32>) -> vec4<f32> {
#ifdef DEPTH_PREPASS
    if reflections.x > 0.0 && normal.y > 0.0 {
        let reflected = reflect(-view_direction, normal);
        if reflected.y > 0.01 {
            // Reuse Bevy's SSR ray marcher and the existing opaque transmission
            // snapshot: transmissive water cannot participate in deferred SSR.
            let origin = position + normal * 0.025;
            var ray = depth_ray_march_new_from_depth(vec2<f32>(textureDimensions(depth_prepass_texture)));
            depth_ray_march_from_cs(&ray, position_world_to_ndc(origin));
            depth_ray_march_to_ws(&ray, origin + reflected * reflections.z);
            ray.linear_steps = u32(reflections.x);
            ray.bisection_steps = u32(reflections.y);
            ray.use_secant = true;
            ray.linear_march_exponent = 2.0;
            ray.depth_thickness_linear_z = reflections.w;
            let hit = depth_ray_march_march(&ray);
            if hit.hit {
                let edge = min(min(hit.hit_uv.x, 1.0 - hit.hit_uv.x),
                    min(hit.hit_uv.y, 1.0 - hit.hit_uv.y));
                let confidence = smoothstep(0.015, 0.12, edge)
                    * (1.0 - smoothstep(0.55, 1.0, hit.hit_penetration_frac));
                // Depth 1 bypasses refraction's foreground rejection: reflected
                // objects may be closer to the camera than this water fragment.
                let scene = fetch_transmissive_background_non_rough(hit.hit_uv, vec3<f32>(0.0, 0.0, 1.0));
                return vec4<f32>(scene.rgb, confidence);
            }
        }
    }
#endif
    return vec4<f32>(0.0);
}

@fragment
fn fragment(input: WaterVertexOutput, @builtin(front_facing) front: bool) -> FragmentOutput {
    // Evaluate derivatives before varying branches/discard. The meshed bed field
    // steers crests independently of depth-buffer edges from creatures or props.
    let world_x = dpdx(input.world_position.xz);
    let world_y = dpdy(input.world_position.xz);
    let contour = vec2<f32>(dpdx(input.uv.x), dpdy(input.uv.x));
    let footprint = max(length(dpdx(input.world_position.xyz)), length(dpdy(input.world_position.xyz)));
    var pbr = pbr_input_from_standard_material(standard_input(input), front);
    let screen = (input.position.xy - view.viewport.xy) / view.viewport.zw;
    let time = globals.time * settings.x;
    var surface_effect = vec4<f32>(0.0);
#ifdef VERTEX_COLORS
    surface_effect = input.color;
#endif
    let dynamic_fall = surface_effect.w > 4.5 && surface_effect.w < 5.5;
    let falling = (surface_effect.w > 0.5 && surface_effect.w < 1.5) || dynamic_fall;
    let solid_block = surface_effect.w > 3.5 && surface_effect.w < 4.5;
    let coastal_wash = surface_effect.w > 5.5 && surface_effect.w < 6.5;
    var wash_pulse = 0.0;
    if coastal_wash {
        wash_pulse = shore_front(input.world_position.xz, globals.time);
        let broken_edge = surface_effect.x + (noise(input.world_position.xz * 2.1) - 0.5) * 0.065;
        if wash_pulse <= broken_edge + 0.012 { discard; }
    }
    var flow = vec2<f32>(0.0);
#ifdef VERTEX_UVS_B
    flow = input.uv_b;
#endif
    var depth = max(input.uv.x, 0.0);
    var thickness = depth;
    var bed = input.world_position.xyz - vec3<f32>(0.0, depth, 0.0);
#ifdef DEPTH_PREPASS
    let opaque_depth = prepass_depth(input.position, 0u);
    if opaque_depth > 0.0 {
        bed = position_ndc_to_world(vec3<f32>(screen * vec2<f32>(2.0, -2.0) + vec2<f32>(-1.0, 1.0), opaque_depth));
        depth = max(input.world_position.y - bed.y, 0.0);
        thickness = min(distance(bed, input.world_position.xyz), 32.0);
    }
#endif
    if solid_block {
        depth = 1.0;
        thickness = min(thickness, 1.0);
    }
    if coastal_wash {
        depth = 0.035 + max(wash_pulse - surface_effect.x, 0.0) * 0.10;
        thickness = depth;
    }
    let shore_calm = mix(0.2, 1.0, smoothstep(0.0, 1.2, depth));
    let moving_position = input.world_position.xz - vec2<f32>(0.0, time * 1.5);
    let face = select(-1.0, 1.0, front);
    var normal_offset = vec3<f32>(0.0);
    if falling {
        let across = vec3<f32>(-input.world_normal.z, 0.0, input.world_normal.x);
        let tangent = select(vec3<f32>(1.0, 0.0, 0.0), across / max(length(across), 0.001), length(across) > 0.001);
        let across_position = waterfall_across(input.world_position.xyz, input.world_normal);
        let fall_position = vec2<f32>(across_position, input.world_position.y + time * 4.0);
        let ripples = wave_slope(fall_position, time, footprint);
        let breakup = noise(fall_position * vec2<f32>(2.4, 0.8)) - 0.5;
        let fade = (1.0 - smoothstep(0.25, 1.0, footprint)) * effects.x;
        normal_offset = (tangent * (ripples.x + breakup * 0.10)
            + vec3<f32>(0.0, ripples.y * 0.55, 0.0)) * fade;
        pbr.N = normalize(input.world_normal + normal_offset) * face;
        thickness = min(thickness, 0.8);
        depth = input.uv.x;
    } else {
        var slope = wave_slope(moving_position, time, footprint);
        if dot(flow, flow) > 0.01 {
            slope += current_slope(input.world_position.xz, flow, time, footprint);
        }
        slope *= shore_calm;
        normal_offset = vec3<f32>(-slope.x, 0.0, -slope.y);
        pbr.N = normalize(input.world_normal + normal_offset) * face;
    }
    let shallow = 1.0 - smoothstep(0.04, settings.w, depth);
    let coast = max(shallow, input.uv.y * (1.0 - smoothstep(0.2, 1.8, depth)));
    var bubbles = 0.0;
    if !falling && (coast > 0.0 || coastal_wash || (surface_effect.w > 2.5 && !solid_block)) {
        bubbles = smoothstep(0.3, 0.78, noise(moving_position * 3.0 + vec2<f32>(time * 0.12, -time * 0.08)));
    }
    var foam = 0.0;
    if !falling && !coastal_wash && coast > 0.0 {
        // Traveling shallow-water bands advance toward the bank and break into patches.
        let wash = 0.5 + 0.5 * sin(depth * 15.0 + time * 2.8
            + noise(input.world_position.xz * 0.35) * 3.0);
        foam = clamp(coast * (0.25 + bubbles * 0.75) * (0.35 + wash * 0.65) * effects.w, 0.0, effects.w);
    }
    // Depth contours steer these low, incoming crests towards the actual bank;
    // the sea-only vertex weight excludes inland lakes and placed water cubes.
    if surface_effect.w < 0.5 && surface_effect.z > 0.02 && depth < 2.6 {
        let ocean = smoothstep(0.02, 0.35, surface_effect.z);
        let phase = depth * 2.15 + globals.time * 0.48
            + noise(input.world_position.xz * 0.12) * 1.2;
        let incoming = 0.5 + 0.5 * sin(phase);
        let crest = smoothstep(0.85, 0.985, incoming);
        let breaker = (1.0 - smoothstep(0.20, 2.6, depth)) * ocean;
        let lace = 0.58 + 0.42 * noise(input.world_position.xz * 6.5 - weather.xy * time);
        foam = max(foam, crest * breaker * lace * (0.68 + weather.z * 0.17));
        // Shallow moving crests turn the normal as well as drawing foam, so the
        // incoming wave reads as a little rolling ridge rather than a white stripe.
        let determinant = world_x.x * world_y.y - world_x.y * world_y.x;
        if abs(determinant) > 0.00001 {
            let gradient = vec2<f32>(contour.x * world_y.y - contour.y * world_x.y,
                world_x.x * contour.y - world_y.x * contour.x) / determinant;
            let bending = clamp(gradient, vec2<f32>(-1.0), vec2<f32>(1.0))
                * cos(phase) * breaker * (0.12 + weather.z * 0.07) * effects.x;
            pbr.N = normalize(pbr.N + vec3<f32>(bending.x, 0.0, bending.y));
        }
    }
    if coastal_wash {
        let distance_from_front = max(wash_pulse - surface_effect.x, 0.0);
        let edge = 1.0 - smoothstep(0.018, 0.17, distance_from_front);
        let lace = noise(input.world_position.xz * 7.5 - weather.xy * time * 0.4);
        let trails = smoothstep(0.65, 0.88, lace) * exp(-distance_from_front * 4.0);
        foam = clamp(edge * (0.52 + 0.31 * bubbles) + trails * 0.15, 0.0, 0.90);
    } else if !falling && !solid_block && surface_effect.w < 0.5 && weather.z > 0.0 {
        let cap_phase = dot(input.world_position.xz, weather.xy) * 0.74 - time * (2.1 + weather.z * 2.0);
        let caps = smoothstep(0.83, 0.99, sin(cap_phase))
            * smoothstep(0.44, 0.76, noise(moving_position * 0.45));
        foam = max(foam, caps * weather.z * 0.46 * smoothstep(0.25, 1.4, depth));
    }
    if falling {
        if dynamic_fall {
            // Cell waterfalls used to mark every metre as both crest and impact,
            // turning the entire sheet into flat white stripes. Keep its body wet.
            let body = noise(vec2<f32>(waterfall_across(input.world_position.xyz, input.world_normal) * 4.0,
                input.world_position.y * 0.8 + time * 4.5));
            let edges = exp(-surface_effect.x * 2.6) * 0.30 + exp(-surface_effect.y * 2.0) * 0.48;
            foam = clamp(0.02 + smoothstep(0.58, 0.90, body) * 0.16 + edges * (0.35 + body * 0.65), 0.0, 0.76);
        } else {
            foam = fall_foam(input.world_position.xyz, input.world_normal, surface_effect, time, footprint);
        }
    } else if surface_effect.w > 1.5 && surface_effect.w < 2.5 {
        let distance_from_impact = abs(surface_effect.x - 0.21);
        let spread = 1.0 - smoothstep(0.05, 0.79, distance_from_impact);
        let churn = noise(moving_position * 4.2 - vec2<f32>(time * 0.7));
        foam = max(foam, spread * (0.38 + 0.52 * churn));
    } else if surface_effect.w > 2.5 && !solid_block && !coastal_wash {
        foam = max(foam, exp(-surface_effect.x * 5.0) * (0.08 + 0.4 * bubbles));
    }
    let camera_normal = (view.view_from_world * vec4<f32>(normal_offset, 0.0)).xy;
    var refraction_uv = clamp(screen + camera_normal * vec2<f32>(view.viewport.w / view.viewport.z, -1.0)
        * settings.z * min(thickness, 2.0) * select(1.0, 1.8, falling), vec2<f32>(0.001), vec2<f32>(0.999));
#ifdef DEPTH_PREPASS
    if settings.z > 0.0 {
        let offset_pixel = vec4<f32>(refraction_uv * view.viewport.zw + view.viewport.xy, input.position.zw);
        let offset_depth = prepass_depth(offset_pixel, 0u);
        if offset_depth > input.position.z || offset_depth == 0.0 { refraction_uv = screen; }
    }
#endif
    let scene = fetch_transmissive_background_non_rough(refraction_uv, input.position.xyz);
    // Absorption vanishes at the shore, preserving the exposed sandy bottom.
    let optical_thickness = thickness + 0.12 * smoothstep(0.0, 0.6, depth);
    let water_color = input.biome_tint;
    // Scattering is weighted by lost transmission below. Suppressing extinction
    // in a blue channel would therefore suppress blue scattering as well and
    // leave colored biomes brown over a dirt bed. Neutral extinction preserves
    // their scattering hue; lowland water retains its exact original spectrum.
    let tint_shift = smoothstep(0.0, 0.10, length(water_color - tint.rgb));
    let absorption = mix(vec3<f32>(0.72, 0.22, 0.14), vec3<f32>(0.65), tint_shift);
    let transmitted = exp(-absorption * optical_thickness * settings.y);
    var caustics = 0.0;
    if effects.z > 0.0 {
        let ribbons = abs(sin(bed.x * 1.7 + sin(bed.z * 1.3 + time * 0.8))
            + sin(bed.z * 1.6 - time * 0.6));
        caustics = (1.0 - smoothstep(0.03, max(0.08, footprint), ribbons))
            * exp(-depth * 0.5) * (1.0 - smoothstep(0.1, 0.8, footprint)) * effects.z;
    }
    let fresnel = 0.02 + 0.98 * pow(1.0 - clamp(dot(pbr.N, pbr.V), 0.0, 1.0), 5.0);
    let transmission = transmitted * (1.0 - fresnel) * (1.0 - foam) * scene.a;
    // PBR already includes Fresnel in the atmosphere and sun reflection. Weighting
    // its result by Fresnel again suppresses highlights and tints grazing views.
    let scatter = water_color * (vec3<f32>(1.0) - transmitted * scene.a)
        * (1.0 - fresnel) * (1.0 - foam);
    pbr.material.base_color = vec4<f32>(scatter + vec3<f32>(0.7, 0.78, 0.73) * foam, 1.0);
    pbr.material.specular_transmission = 0.0;
    pbr.material.perceptual_roughness = mix(select(0.12 + weather.z * 0.10, 0.2, falling), 0.48, foam);
    var reflection = vec4<f32>(0.0);
    if !falling {
        reflection = scene_reflection(input.world_position.xyz, pbr.N, pbr.V);
    }
    let reflection_weight = reflection.a * (1.0 - foam);
    // Replace the environment specular contribution where an actual scene hit
    // is available, preserving direct sunlight and underwater transmission.
    pbr.specular_occlusion *= 1.0 - reflection_weight;
    let reflected_and_scattered = apply_pbr_lighting(pbr);
    // Both values are exposure-adjusted linear HDR; no additional exposure here.
    let color = scene.rgb * transmission * (1.0 + caustics) + reflected_and_scattered.rgb
        + reflection.rgb * fresnel * reflection_weight;
    var output: FragmentOutput;
    output.color = main_pass_post_lighting_processing(pbr, vec4<f32>(color, tint.w));
    return output;
}
