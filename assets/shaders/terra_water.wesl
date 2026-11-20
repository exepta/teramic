import bevy_pbr::render::forward_io::Vertex;
import bevy_pbr::render::forward_io::VertexOutput;
import bevy_pbr::render::forward_io::FragmentOutput;
import bevy_pbr::render::mesh_functions;
import bevy_pbr::render::pbr_fragment::pbr_input_from_standard_material;
import bevy_pbr::render::pbr_functions::apply_pbr_lighting;
import bevy_pbr::render::pbr_functions::main_pass_post_lighting_processing;
import bevy_pbr::render::mesh_view_bindings::globals;
import bevy_pbr::render::mesh_view_bindings::view;
import bevy_pbr::render::view_transformations::position_ndc_to_world;
import bevy_pbr::render::view_transformations::position_world_to_clip;
@if(VIEW_TRANSMISSION_TEXTURE)
import bevy_pbr::transmission::fetch_transmissive_background_non_rough;
@if(DEPTH_PREPASS)
import bevy_pbr::prepass::utils::prepass_depth;
@if(DEPTH_PREPASS && VIEW_TRANSMISSION_TEXTURE)
import bevy_pbr::render::mesh_view_bindings::depth_prepass_texture;
@if(DEPTH_PREPASS && VIEW_TRANSMISSION_TEXTURE)
import bevy_pbr::render::view_transformations::position_world_to_ndc;
@if(DEPTH_PREPASS && VIEW_TRANSMISSION_TEXTURE && WATER_NATIVE_DEPTH_FILTER)
import terra::water_raymarch::depth_ray_march_new_from_depth;
@if(DEPTH_PREPASS && VIEW_TRANSMISSION_TEXTURE && WATER_NATIVE_DEPTH_FILTER)
import terra::water_raymarch::depth_ray_march_from_cs;
@if(DEPTH_PREPASS && VIEW_TRANSMISSION_TEXTURE && WATER_NATIVE_DEPTH_FILTER)
import terra::water_raymarch::depth_ray_march_to_ws;
@if(DEPTH_PREPASS && VIEW_TRANSMISSION_TEXTURE && WATER_NATIVE_DEPTH_FILTER)
import terra::water_raymarch::depth_ray_march_march;
@if(DEPTH_PREPASS && VIEW_TRANSMISSION_TEXTURE && !(WATER_NATIVE_DEPTH_FILTER))
import bevy_pbr::ssr::raymarch::depth_ray_march_new_from_depth;
@if(DEPTH_PREPASS && VIEW_TRANSMISSION_TEXTURE && !(WATER_NATIVE_DEPTH_FILTER))
import bevy_pbr::ssr::raymarch::depth_ray_march_from_cs;
@if(DEPTH_PREPASS && VIEW_TRANSMISSION_TEXTURE && !(WATER_NATIVE_DEPTH_FILTER))
import bevy_pbr::ssr::raymarch::depth_ray_march_to_ws;
@if(DEPTH_PREPASS && VIEW_TRANSMISSION_TEXTURE && !(WATER_NATIVE_DEPTH_FILTER))
import bevy_pbr::ssr::raymarch::depth_ray_march_march;

@group(constants::MATERIAL_BIND_GROUP) @binding(100) var<uniform> tint: vec4<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(101) var<uniform> settings: vec4<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(102) var<uniform> effects: vec4<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(103) var<uniform> reflections: vec4<f32>;
struct WaterDynamics {
    weather: vec4<f32>,
    environment: vec4<f32>,
    wake_positions: array<vec4<f32>, 16>,
    wake_motions: array<vec4<f32>, 16>,
    particle: vec4<f32>,
}
@group(constants::MATERIAL_BIND_GROUP) @binding(104) var<storage, read> terra_water: WaterDynamics;

// A fixed shared history disturbs the actual water normals. Each stroke leaves
// widening Kelvin-like arms and a softer expanding crest, not a tiny white disc.
fn actor_wake(position: vec3<f32>, footprint: f32) -> vec3<f32> {
    let environment = terra_water.environment;
    var slope = vec2<f32>(0.0);
    var foam = 0.0;
    for (var i = 0u; i < min(u32(environment.y), 16u); i += 1u) {
        let source = terra_water.wake_positions[i];
        let motion = terra_water.wake_motions[i];
        var age = globals.time - source.w;
        if age < 0.0 { age += environment.z; }
        let offset = position.xz - source.xz;
        if age < 0.0 || age > 3.2 || abs(position.y - source.y) > 0.4
            || dot(offset, offset) > 42.25 { continue; }
        let direction = motion.xy;
        let sideways = vec2<f32>(-direction.y, direction.x);
        let behind = -dot(offset, direction);
        let across = dot(offset, sideways);
        let width = 0.085 + age * 0.028 + min(footprint, 0.16);
        let arm_distance = abs(across) - (0.20 + max(behind, 0.0) * 0.43 + age * 0.19);
        let arms = exp(-arm_distance * arm_distance / (width * width))
            * smoothstep(0.2, 0.8, length(direction));
        let trail = smoothstep(-0.25, 0.15, behind)
            * (1.0 - smoothstep(1.3 + age * 0.8, 2.4 + age, behind));
        let radius = 0.18 + age * 1.4;
        let ring_distance = length(offset) - radius;
        let ring = exp(-ring_distance * ring_distance / (width * width * 1.8)) * 0.32;
        let fade = pow(1.0 - age / 3.2, 1.6) * min(motion.w * 1.5, 1.0)
            * (1.0 - smoothstep(0.18, 0.65, footprint));
        let turn = sign(across) * (-arm_distance / width) * arms * trail;
        slope += sideways * turn * fade * 0.06;
        slope += offset / max(length(offset), 0.001) * (-ring_distance / width) * ring * fade * 0.05;
        let breakup = 0.70 + 0.30 * sin(across * 17.0 + behind * 9.0 + age * 2.1);
        foam = max(foam, (arms * trail * breakup * 0.12 + ring * 0.06) * fade);
    }
    return vec3<f32>(clamp(slope, vec2<f32>(-0.12), vec2<f32>(0.12)), min(foam, 0.12));
}

// A damped, bipolar gravity-wave packet: displacement stays local to actual
// bodies and never changes a reservoir's mean level or follows the camera.
fn actor_height(position: vec3<f32>, depth: f32) -> f32 {
    if depth < 0.08 { return 0.0; }
    var height = 0.0;
    let environment = terra_water.environment;
    for (var i = 0u; i < min(u32(environment.y), 16u); i += 1u) {
        let source = terra_water.wake_positions[i];
        let motion = terra_water.wake_motions[i];
        var age = globals.time - source.w;
        if age < 0.0 { age += environment.z; }
        let delta = position.xz - source.xz;
        if age < 0.0 || age > 3.2 || abs(position.y - source.y) > 0.4
            || dot(delta, delta) > 42.25 { continue; }
        let radius = 0.18 + age * 1.4;
        let width = 0.24 + age * 0.12;
        let band = (length(delta) - radius) / width;
        let fade = pow(1.0 - age / 3.2, 2.0) * clamp(motion.w, 0.0, 0.75);
        height += sin(band * 2.0) * exp(-band * band) * fade * 0.018;
    }
    return clamp(height, -0.025, 0.025);
}

// Keep the stock forward fields so lighting and chunk fades use the same path.
// The extra varying leaves all depth/current/foam metadata intact.
struct WaterVertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) world_position: vec4<f32>,
    @location(1) world_normal: vec3<f32>,
@if(VERTEX_UVS_A)
    @location(2) uv: vec2<f32>,
@if(VERTEX_UVS_B)
    @location(3) uv_b: vec2<f32>,
@if(VERTEX_TANGENTS)
    @location(4) world_tangent: vec4<f32>,
@if(VERTEX_COLORS)
    @location(5) color: vec4<f32>,
@if(VERTEX_OUTPUT_INSTANCE_INDEX)
    @location(6) @interpolate(flat) instance_index: u32,
@if(VISIBILITY_RANGE_DITHER)
    @location(7) @interpolate(flat) visibility_range_dither: i32,
    @location(8) biome_tint: vec4<f32>,
    @location(9) blood_weight: f32,
}

@vertex
fn vertex(input: Vertex,
@if(WATER_BIOME_TINT)
    @location(8) biome_tint: vec4<f32>,
@if(WATER_WAVE_WEIGHT)
    @location(9) wave_weight: f32,
@if(WATER_BLOOD_WEIGHT)
    @location(10) blood_weight: f32,
@if(LIQUID_MOTION)
    @location(11) liquid_from: vec4<f32>,
@if(LIQUID_MOTION)
    @location(12) liquid_data: vec4<f32>,
) -> WaterVertexOutput {
    var out: WaterVertexOutput;
    let model = mesh_functions::get_world_from_local(input.instance_index);
    out.world_position = mesh_functions::mesh_position_local_to_world(model, vec4<f32>(input.position, 1.0));
@if(LIQUID_MOTION) {
    let phase = clamp((terra_water.environment.w - liquid_from.w) / max(liquid_data.x, 0.001), 0.0, 1.0);
    let amount = phase * phase * (3.0 - 2.0 * phase);
    let previous = mesh_functions::mesh_position_local_to_world(model, vec4<f32>(liquid_from.xyz, 1.0));
    out.world_position = mix(previous, out.world_position, amount);
}
    var blood = 0.0;
@if(WATER_BLOOD_WEIGHT) {
    blood = clamp(blood_weight, 0.0, 1.0);
}
@if(VERTEX_COLORS) {
    // The sea is a connected marine body. Nearby red land must not turn the
    // entire ocean into brown liquid; inland Blood rivers keep their palette.
    let marine_body = select(0.0, smoothstep(0.35, 0.85, input.color.z), input.color.w < 0.5);
    blood *= 1.0 - marine_body;
}
    out.blood_weight = blood;
@if(WATER_WAVE_WEIGHT) {
    // The async mesher supplies corner-shared weights. Lip/impact quads taper
    // into anchored sheets; per-cell flow/mask values never split shared edges.
    var swell = 0.18;
@if(VERTEX_COLORS) {
    swell = select(0.18, mix(0.18, 1.0, smoothstep(0.02, 0.45, input.color.z)), input.color.w < 0.5);
}
@if(VERTEX_UVS_B) {
    swell = max(swell, select(0.18, 0.40, dot(input.uv_b, input.uv_b) > 0.01));
}
    let time = terra_water.environment.w * settings.x;
    var height = swell_wave(out.world_position.xz, time).z;
    if blood > 0.0 {
        height = mix(height, swell_wave(out.world_position.xz, time * 0.24).z, blood);
    }
    out.world_position.y += height * effects.x * wave_weight * swell * mix(1.0, 0.45, blood);
@if(VERTEX_UVS_A) {
    if terra_water.environment.y > 0.0 {
        out.world_position.y += actor_height(out.world_position.xyz, input.uv.x)
            * wave_weight * mix(1.0, 0.45, blood);
    }
}
}
    out.position = position_world_to_clip(out.world_position.xyz);
    out.world_normal = mesh_functions::mesh_normal_local_to_world(input.normal, input.instance_index);
@if(VERTEX_UVS_A) {
    out.uv = input.uv;
}
@if(VERTEX_UVS_B) {
    out.uv_b = input.uv_b;
}
@if(VERTEX_TANGENTS) {
    out.world_tangent = mesh_functions::mesh_tangent_local_to_world(model, input.tangent, input.instance_index);
}
@if(VERTEX_COLORS) {
    out.color = input.color;
}
@if(VERTEX_OUTPUT_INSTANCE_INDEX) {
    out.instance_index = input.instance_index;
}
@if(VISIBILITY_RANGE_DITHER) {
    out.visibility_range_dither = mesh_functions::get_visibility_range_dither_level(input.instance_index, model[3]);
}
@if(WATER_BIOME_TINT) {
    out.biome_tint = biome_tint;
} @else {
    // Inventory previews and loose item meshes have no terrain-biome payload.
    out.biome_tint = vec4<f32>(tint.rgb, 0.0);
}
    return out;
}

fn standard_input(input: WaterVertexOutput) -> VertexOutput {
    var out: VertexOutput;
    out.position = input.position;
    out.world_position = input.world_position;
    out.world_normal = input.world_normal;
@if(VERTEX_UVS_A) {
    out.uv = input.uv;
}
@if(VERTEX_UVS_B) {
    out.uv_b = input.uv_b;
}
@if(VERTEX_TANGENTS) {
    out.world_tangent = input.world_tangent;
}
@if(VERTEX_COLORS) {
    // COLOR is water metadata, never a PBR color or ambient-occlusion factor.
    out.color = vec4<f32>(1.0);
}
@if(VERTEX_OUTPUT_INSTANCE_INDEX) {
    out.instance_index = input.instance_index;
}
@if(VISIBILITY_RANGE_DITHER) {
    out.visibility_range_dither = input.visibility_range_dither;
}
    return out;
}

fn hash(p: vec2<f32>) -> f32 {
    // Integer mixing avoids the large-coordinate precision bands of sin(dot()).
    var h = (bitcast<u32>(i32(p.x)) * 0x8da6b343u) ^ (bitcast<u32>(i32(p.y)) * 0xd8163841u);
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

// Two long gravity waves carry the silhouette and its lighting together.
// Wind changes their energy; it cannot rotate an absolute world coordinate
// or retime the entire ocean when the weather updates.
fn swell_wave(p: vec2<f32>, time: f32) -> vec3<f32> {
    let wind = terra_water.weather;
    let a = vec2<f32>(0.8, 0.6);
    let b = vec2<f32>(-0.42, 0.9075);
    let crest_a = dot(p, a) * 0.72 - time * 2.66;
    let crest_b = dot(p, b) * 1.11 - time * 3.30 + 1.4;
    let height_a = 0.020 + wind.z * (0.025 + 0.032 * abs(dot(wind.xy, a)));
    let height_b = 0.009 + wind.z * (0.016 + 0.012 * abs(dot(wind.xy, b)));
    let gradient = a * (0.72 * height_a * cos(crest_a)) + b * (1.11 * height_b * cos(crest_b));
    return vec3<f32>(gradient, height_a * sin(crest_a) + height_b * sin(crest_b));
}

fn wave_slope(p: vec2<f32>, time: f32, footprint: f32) -> vec2<f32> {
    if effects.x == 0.0 { return vec2<f32>(0.0); }
    var slope = swell_wave(p, time).xy * (1.0 - smoothstep(0.8, 2.4, footprint));
    // Short capillary waves break up the reflection close to the observer.
    // Resolve them to their mean as their wavelength approaches a pixel.
    let waves = array<vec4<f32>, 2>(vec4<f32>(0.96, -0.28, 2.9, 0.012),
        vec4<f32>(-0.32, 0.947, 5.1, 0.005));
    for (var i = 1u; i < u32(effects.y); i += 1u) {
        let w = waves[i - 1u];
        let phase = dot(p, w.xy) * w.z - time * sqrt(9.81 * w.z) + f32(i) * 2.4;
        let fade = 1.0 - smoothstep(0.38, 1.8, footprint * w.z);
        slope += w.xy * w.z * w.w * cos(phase) * fade;
    }
    return slope * effects.x;
}

fn shore_front(p: vec2<f32>, time: f32) -> f32 {
    return smoothstep(-0.35, 0.95, sin(time * 0.48 + noise(p * 0.035) * 1.4));
}

fn current_slope(p: vec2<f32>, flow: vec2<f32>, time: f32, footprint: f32) -> vec2<f32> {
    // Cross-fade two bounded advection phases. A bend or a changed flow
    // direction moves the pattern locally, even after hours of runtime.
    let phase = fract(time * 0.48);
    let alternate = fract(phase + 0.5);
    let a = p - flow * ((phase - 0.5) * 1.8);
    let b = p - flow * ((alternate - 0.5) * 1.8);
    let wave_a = vec2<f32>(cos(a.x * 5.2 + 0.8 * sin(a.y * 1.3)), cos(a.y * 5.2 + 0.8 * sin(a.x * 1.3)));
    let wave_b = vec2<f32>(cos(b.x * 5.2 + 0.8 * sin(b.y * 1.3)), cos(b.y * 5.2 + 0.8 * sin(b.x * 1.3)));
    let weight = smoothstep(0.0, 1.0, abs(phase * 2.0 - 1.0));
    let fade = 1.0 - smoothstep(0.09, 0.46, footprint);
    return flow * dot(abs(flow) / max(abs(flow.x) + abs(flow.y), 0.001), mix(wave_a, wave_b, weight)) * 0.090 * fade * effects.x;
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
@if(DEPTH_PREPASS && VIEW_TRANSMISSION_TEXTURE) {
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
}
    return vec4<f32>(0.0);
}

@fragment
fn fragment(input: WaterVertexOutput, @builtin(front_facing) front: bool) -> FragmentOutput {
    let weather = terra_water.weather;
    let environment = terra_water.environment;
    // Evaluate derivatives before varying branches/discard. The meshed bed field
    // steers crests independently of depth-buffer edges from creatures or props.
    let world_x = dpdx(input.world_position.xz);
    let world_y = dpdy(input.world_position.xz);
    let contour = vec2<f32>(dpdx(input.uv.x), dpdy(input.uv.x));
    let footprint = max(length(dpdx(input.world_position.xyz)), length(dpdy(input.world_position.xyz)));
    var pbr = pbr_input_from_standard_material(standard_input(input), front);
    let screen = (input.position.xy - view.viewport.xy) / view.viewport.zw;
    let blood = clamp(input.blood_weight, 0.0, 1.0);
    let time = environment.w * settings.x;
    var surface_effect = vec4<f32>(0.0);
@if(VERTEX_COLORS) {
    surface_effect = input.color;
}
    let dynamic_fall = surface_effect.w > 4.5 && surface_effect.w < 5.5;
    let falling = (surface_effect.w > 0.5 && surface_effect.w < 1.5) || dynamic_fall;
    let solid_block = surface_effect.w > 3.5 && surface_effect.w < 4.5;
    let coastal_wash = surface_effect.w > 5.5 && surface_effect.w < 6.5;
    let free_jet = surface_effect.w > 7.5 && surface_effect.w < 8.5;
    // The advancing film has a measured thickness down to its dry contact line.
    let pour_surface = surface_effect.w > 8.5 && surface_effect.w < 9.5;
    let contained = (surface_effect.w > 6.5 && surface_effect.w < 7.5) || free_jet || pour_surface;
    var wash_pulse = 0.0;
    if coastal_wash {
        wash_pulse = shore_front(input.world_position.xz, environment.w);
        let broken_edge = surface_effect.x + (noise(input.world_position.xz * 2.1) - 0.5) * 0.065;
        if wash_pulse <= broken_edge + 0.012 { discard; }
    }
    var flow = vec2<f32>(0.0);
@if(VERTEX_UVS_B) {
    flow = input.uv_b;
}
    let bed_depth = max(input.uv.x, 0.0);
    var depth = bed_depth;
    var thickness = depth;
    var bed = input.world_position.xyz - vec3<f32>(0.0, depth, 0.0);
@if(DEPTH_PREPASS) {
    let opaque_depth = prepass_depth(input.position, 0u);
    if opaque_depth > 0.0 {
        bed = position_ndc_to_world(vec3<f32>(screen * vec2<f32>(2.0, -2.0) + vec2<f32>(-1.0, 1.0), opaque_depth));
        depth = max(input.world_position.y - bed.y, 0.0);
        thickness = min(distance(bed, input.world_position.xyz), 32.0);
    }
}
    if solid_block {
        depth = 1.0;
        thickness = min(thickness, 1.0);
    }
    if coastal_wash {
        depth = 0.035 + max(wash_pulse - surface_effect.x, 0.0) * 0.10;
        thickness = depth;
    }
    // A pipe/jet is a small real liquid volume, not the distant terrain bed.
    // Keep lake optics while using its own thickness and excluding shore foam.
    if contained {
        depth = input.uv.x;
        thickness = min(thickness, input.uv.x);
        bed = input.world_position.xyz - vec3<f32>(0.0, depth, 0.0);
    }
    let shore_calm = mix(0.5, 1.0, smoothstep(0.0, 1.2, bed_depth));
    let moving_position = input.world_position.xz - vec2<f32>(0.0, time * 1.5);
    let face = select(-1.0, 1.0, front);
    var normal_offset = vec3<f32>(0.0);
    var actor_foam = 0.0;
    if free_jet {
        // A round moving stream has highlights along its sides, rather than
        // lake waves across a horizontal plane. Geometry remains continuous.
        let ripple = sin(input.uv.y * 36.0 - time * 19.0);
        let tangent = cross(input.world_normal, vec3<f32>(0.0, 1.0, 0.0));
        normal_offset = tangent / max(length(tangent), 0.001) * ripple * 0.08;
        pbr.N = normalize(input.world_normal + normal_offset) * face;
    } else if falling {
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
        let marine = select(0.0, smoothstep(0.02, 0.45, surface_effect.z), surface_effect.w < 0.5);
        let swell = max(mix(0.18, 1.0, marine), select(0.18, 0.40, dot(flow, flow) > 0.01));
        let xz = input.world_position.xz;
        var slope = wave_slope(xz, time, footprint) * swell;
        if dot(flow, flow) > 0.001 {
            slope += current_slope(xz, flow, time, footprint);
        }
        if blood > 0.0 {
            let slow_time = time * 0.24;
            var viscous = wave_slope(xz, slow_time, footprint) * swell;
            if dot(flow, flow) > 0.001 {
                viscous += current_slope(xz, flow, slow_time, footprint);
            }
            slope = mix(slope, viscous, blood);
        }
        slope *= shore_calm * mix(1.0, 0.45, blood);
        if !solid_block && !contained && input.world_normal.y > 0.5 && environment.y > 0.0 {
            let wake = actor_wake(input.world_position.xyz, footprint);
            slope += wake.xy;
            actor_foam = wake.z;
        }
        normal_offset = vec3<f32>(-slope.x, 0.0, -slope.y);
        pbr.N = normalize(input.world_normal + normal_offset) * face;
    }
    // Shorelines belong to the meshed bed. An opaque fish, player or grazing
    // camera ray only changes optical thickness, never the position of foam.
    let shallow = 1.0 - smoothstep(0.04, settings.w, bed_depth);
    let coast = select(max(shallow * 0.45, input.uv.y * (1.0 - smoothstep(0.2, 1.8, bed_depth))), 0.0, contained);
    let foam_detail = 1.0 - smoothstep(0.09, 0.48, footprint);
    var bubbles = 0.0;
    if !falling && (coast > 0.0 || coastal_wash || (surface_effect.w > 2.5 && !solid_block && !contained)) {
        bubbles = mix(0.48, smoothstep(0.3, 0.78, noise(moving_position * 3.0 + vec2<f32>(time * 0.12, -time * 0.08))), foam_detail);
    }
    var foam = actor_foam;
    if free_jet {
        // Small entrained highlights make the nozzle-to-vessel volume legible
        // without recoloring its biome water or making it emissive.
        let streak = 0.5 + 0.5 * sin(input.uv.y * 46.0 - time * 23.0);
        let impact = smoothstep(0.86, 1.0, input.uv.y);
        foam = 0.25 + streak * 0.15 + impact * 0.18;
    }
    if !falling && !coastal_wash && coast > 0.0 {
        // Traveling shallow-water bands advance toward the bank and break into patches.
        let wash = 0.5 + 0.5 * sin(bed_depth * 8.0 + time * 1.6
            + noise(input.world_position.xz * 0.35) * 3.0);
        foam = max(foam, clamp(coast * (0.25 + bubbles * 0.75) * (0.35 + wash * 0.65) * effects.w, 0.0, effects.w));
    }
    // Depth contours steer these low, incoming crests towards the actual bank;
    // the sea-only vertex weight excludes inland lakes and placed water cubes.
    if surface_effect.w < 0.5 && surface_effect.z > 0.02 && bed_depth < 2.6 {
        let ocean = smoothstep(0.02, 0.35, surface_effect.z);
        let phase = bed_depth * 2.15 + environment.w * 0.48
            + noise(input.world_position.xz * 0.12) * 1.2;
        let incoming = 0.5 + 0.5 * sin(phase);
        let crest = smoothstep(0.85, 0.985, incoming);
        let breaker = (1.0 - smoothstep(0.20, 2.6, bed_depth)) * ocean;
        let lace = 0.58 + 0.42 * mix(0.5, noise(input.world_position.xz * 6.5 - vec2<f32>(0.33, 0.16) * time), foam_detail);
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
        let lace = mix(0.5, noise(input.world_position.xz * 7.5 - vec2<f32>(0.33, 0.16) * time), foam_detail);
        let trails = smoothstep(0.65, 0.88, lace) * exp(-distance_from_front * 4.0);
        foam = clamp(edge * (0.52 + 0.31 * bubbles) + trails * 0.15, 0.0, 0.90);
    } else if !falling && !solid_block && surface_effect.w < 0.5 && weather.z > 0.0 {
        let cap_phase = dot(input.world_position.xz, vec2<f32>(0.8, 0.6)) * 0.72 - time * 2.66;
        let caps = smoothstep(0.83, 0.99, sin(cap_phase))
            * smoothstep(0.44, 0.76, noise(moving_position * 0.45));
        let marine = smoothstep(0.02, 0.45, surface_effect.z);
        foam = max(foam, caps * weather.z * marine * 0.46 * smoothstep(0.25, 1.4, depth));
    }
    if falling {
        if dynamic_fall {
            // Cell waterfalls used to mark every metre as both crest and impact,
            // turning the entire sheet into flat white stripes. Keep its body wet.
            // Resolve subpixel whitewater to its mean, just like the surface
            // waves. Unfiltered moving noise made distant falls sparkle.
            let detail = 1.0 - smoothstep(0.12, 0.55, footprint);
            let body = mix(0.5, noise(vec2<f32>(waterfall_across(input.world_position.xyz, input.world_normal) * 4.0,
                input.world_position.y * 0.8 + time * 4.5)), detail);
            let edges = exp(-surface_effect.x * 2.6) * 0.30 + exp(-surface_effect.y * 2.0) * 0.48;
            let streaks = mix(0.22, smoothstep(0.58, 0.90, body), detail);
            foam = clamp(0.02 + streaks * 0.16 + edges * (0.35 + body * 0.65), 0.0, 0.76);
        } else {
            foam = fall_foam(input.world_position.xyz, input.world_normal, surface_effect, time, footprint);
        }
    } else if surface_effect.w > 1.5 && surface_effect.w < 2.5 {
        let distance_from_impact = abs(surface_effect.x - 0.21);
        let spread = 1.0 - smoothstep(0.05, 0.79, distance_from_impact);
        let churn = noise(moving_position * 4.2 - vec2<f32>(time * 0.7));
        foam = max(foam, spread * (0.38 + 0.52 * churn));
    } else if surface_effect.w > 2.5 && !solid_block && !coastal_wash && !contained {
        foam = max(foam, exp(-surface_effect.x * 5.0) * (0.08 + 0.4 * bubbles));
    }
    let camera_normal = (view.view_from_world * vec4<f32>(normal_offset, 0.0)).xy;
    var refraction_uv = clamp(screen + camera_normal * vec2<f32>(view.viewport.w / view.viewport.z, -1.0)
        * settings.z * min(thickness, 2.0) * select(1.0, 1.8, falling), vec2<f32>(0.001), vec2<f32>(0.999));
@if(DEPTH_PREPASS) {
    if settings.z > 0.0 {
        let offset_pixel = vec4<f32>(refraction_uv * view.viewport.zw + view.viewport.xy, input.position.zw);
        let offset_depth = prepass_depth(offset_pixel, 0u);
        if offset_depth > input.position.z || offset_depth == 0.0 { refraction_uv = screen; }
    }
}
    // Secondary views may have no opaque transmission snapshot. Keep ordinary
    // water scattering and environment lighting without reading an absent binding.
    var scene = vec4<f32>(0.0);
@if(VIEW_TRANSMISSION_TEXTURE) {
    scene = fetch_transmissive_background_non_rough(refraction_uv, input.position.xyz);
}
    // Absorption vanishes at the shore, preserving the exposed sandy bottom.
    let optical_thickness = thickness + 0.12 * smoothstep(0.0, 0.6, depth);
    let marine_body = select(0.0, smoothstep(0.35, 0.85, surface_effect.z), surface_effect.w < 0.5);
    let water_color = mix(input.biome_tint.rgb, vec3<f32>(0.015, 0.16, 0.235), marine_body);
    let spirit = clamp(input.biome_tint.a, 0.0, 1.0);
    // Scattering is weighted by lost transmission below. Suppressing extinction
    // in a blue channel would therefore suppress blue scattering as well and
    // leave colored biomes brown over a dirt bed. Neutral extinction preserves
    // their scattering hue; lowland water retains its exact original spectrum.
    let tint_shift = smoothstep(0.0, 0.10, length(water_color - tint.rgb));
    let absorption = mix(mix(vec3<f32>(0.72, 0.22, 0.14), vec3<f32>(0.65), tint_shift),
        vec3<f32>(2.2, 5.5, 5.0), blood);
    let transmitted = exp(-absorption * optical_thickness * settings.y);
    var caustics = 0.0;
    if effects.z > 0.0 && !free_jet {
        let ribbons = abs(sin(bed.x * 1.7 + sin(bed.z * 1.3 + time * 0.8))
            + sin(bed.z * 1.6 - time * 0.6));
        caustics = (1.0 - smoothstep(0.03, max(0.08, footprint), ribbons))
            * exp(-depth * 0.5) * (1.0 - smoothstep(0.1, 0.8, footprint)) * effects.z * environment.x;
    }
    caustics *= 1.0 - blood;
    foam *= mix(1.0, 0.22, blood);
    let fresnel = 0.02 + 0.98 * pow(1.0 - clamp(dot(pbr.N, pbr.V), 0.0, 1.0), 5.0);
    let transmission = transmitted * (1.0 - fresnel) * (1.0 - foam) * scene.a;
    // PBR already includes Fresnel in the atmosphere and sun reflection. Weighting
    // its result by Fresnel again suppresses highlights and tints grazing views.
    let scatter = water_color * (vec3<f32>(1.0) - transmitted * scene.a)
        * (1.0 - fresnel) * (1.0 - foam);
    let foam_color = mix(mix(vec3<f32>(0.7, 0.78, 0.73), vec3<f32>(0.86, 1.0, 0.98), spirit),
        vec3<f32>(0.52, 0.14, 0.19), blood);
    pbr.material.base_color = vec4<f32>(scatter + foam_color * foam, 1.0);
    // Low radiance pearl body and brighter white crests only in Spirit water.
    // Independent of sun exposure, but fades at the shore to preserve the bed.
    let pearl = spirit * (1.0 - exp(-optical_thickness * 0.42));
    pbr.material.emissive = vec4<f32>(vec3<f32>(0.70, 1.0, 0.95)
        * pearl * (0.055 + foam * 0.22), 0.0);
    pbr.material.specular_transmission = 0.0;
    let roughness = select(0.19 + weather.z * 0.07, 0.25, falling);
    let unresolved = smoothstep(0.035, 0.38, footprint);
    pbr.material.perceptual_roughness = mix(mix(roughness + unresolved * 0.08, 0.48, foam), 0.29, blood);
    var reflection = vec4<f32>(0.0);
    if !falling {
        reflection = scene_reflection(input.world_position.xyz, pbr.N, pbr.V);
    }
    let reflection_weight = reflection.a * (1.0 - foam) * mix(0.80, 0.45, unresolved);
    // Replace the environment specular contribution where an actual scene hit
    // is available, preserving direct sunlight and underwater transmission.
    pbr.specular_occlusion *= 1.0 - reflection_weight;
    let reflected_and_scattered = apply_pbr_lighting(pbr);
    // Both values are exposure-adjusted linear HDR; no additional exposure here.
    var sky_reflection = vec3<f32>(0.0);
@if(!(ENVIRONMENT_MAP)) {
    // Views without an IBL cubemap still see the atmosphere reflected in water.
    // Use a broad sky lobe; actual scene hits replace it and PBR keeps the sun.
    if front && !falling {
        let reflected_ray = reflect(-pbr.V, pbr.N);
        let sky = mix(vec3<f32>(0.16, 0.28, 0.34), vec3<f32>(0.055, 0.18, 0.32),
            smoothstep(0.0, 0.8, max(reflected_ray.y, 0.0)));
        sky_reflection = sky * (0.025 + environment.x * 0.95)
            * fresnel * (1.0 - reflection_weight) * (1.0 - foam);
    }
}
    let color = scene.rgb * transmission * (1.0 + caustics) + reflected_and_scattered.rgb + sky_reflection
        + reflection.rgb * fresnel * reflection_weight;
    var output: FragmentOutput;
    output.color = main_pass_post_lighting_processing(pbr, vec4<f32>(color, tint.w));
    return output;
}
