import bevy_pbr::render::mesh_functions;
import bevy_pbr::render::view_transformations::position_world_to_clip;
@if(PREPASS_PIPELINE)
import bevy_pbr::prepass::io::VertexOutput;
@if(PREPASS_PIPELINE)
import bevy_render::globals::Globals;
@if(!(PREPASS_PIPELINE))
import bevy_pbr::render::forward_io::VertexOutput;
@if(!(PREPASS_PIPELINE))
import bevy_pbr::render::mesh_view_bindings::globals;
@if(!(PROP_GROWTH) && PREPASS_PIPELINE)
import bevy_pbr::prepass::io::Vertex;
@if(!(PROP_GROWTH) && !(PREPASS_PIPELINE))
import bevy_pbr::render::forward_io::Vertex;


@if(PREPASS_PIPELINE) {
@group(0) @binding(1) var<uniform> prop_globals: Globals;
} @else {
}

@if(PROP_GROWTH) {
struct Vertex {
    @builtin(instance_index) instance_index: u32,
    @location(0) position: vec3<f32>,
@if(PREPASS_PIPELINE)
    @location(1) uv: vec2<f32>,
@if(PREPASS_PIPELINE)
    @location(2) uv_b: vec2<f32>,
@if(PREPASS_PIPELINE)
    @location(3) normal: vec3<f32>,
@if(PREPASS_PIPELINE)
    @location(7) color: vec4<f32>,
@if(!(PREPASS_PIPELINE))
    @location(1) normal: vec3<f32>,
@if(!(PREPASS_PIPELINE))
    @location(2) uv: vec2<f32>,
@if(!(PREPASS_PIPELINE))
    @location(3) uv_b: vec2<f32>,
@if(!(PREPASS_PIPELINE))
    @location(5) color: vec4<f32>,
    @location(8) growth_anchor: vec3<f32>,
};
} @else {

}

@group(constants::MATERIAL_BIND_GROUP) @binding(100) var<uniform> wind: vec4<f32>;
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
@group(constants::MATERIAL_BIND_GROUP) @binding(101) var<storage, read> terra_weather: TerraWeather;

fn flower_progress(input: Vertex, seconds_ago: f32) -> f32 {
@if(VERTEX_COLORS) {
    if input.color.a < 1.0 {
        let rank = u32(round(input.color.a * 255.0));
        let start = terra_weather.flowers[rank / 4u][rank % 4u];
        if start == -1.0 { return 0.0; }
        if start == -2.0 { return 1.0; }
        return clamp((terra_weather.vegetation.z - seconds_ago - start) / 10.0, 0.0, 1.0);
    }
}
    return 0.0;
}

fn flower_scale(input: Vertex, seconds_ago: f32) -> f32 {
    let progress = flower_progress(input, seconds_ago);
    return 1.0 - progress * progress * (3.0 - 2.0 * progress);
}

@if(PROP_GROWTH) {
fn growth_factor(input: Vertex, time: f32) -> f32 {
    if input.uv_b.y < -2.5 { return 1.0; }
    let ratio = select(input.uv_b.y, -1.0 - input.uv_b.y, input.uv_b.y < -0.5);
    let elapsed = clamp((time - input.uv_b.x) / 2.4, 0.0, 1.0);
    return mix(ratio, 1.0, elapsed * elapsed * (3.0 - 2.0 * elapsed));
}

fn grown_local(input: Vertex, time: f32) -> vec4<f32> {
    let seconds_ago = max(0.0, terra_weather.surface.w - time);
    let scale = growth_factor(input, time) * flower_scale(input, seconds_ago);
    return vec4<f32>(input.growth_anchor + (input.position - input.growth_anchor) * scale, 1.0);
}
}

fn wind_position(world: vec4<f32>, weights: vec2<f32>, time: f32) -> vec4<f32> {
    // UV0 carries height and flexibility: these meshes use vertex colors, not textures.
    // Squared root-relative height keeps roots fixed; stones and mushrooms have zero weight.
    let weight = weights.x * weights.y;
    if weight == 0.0 {
        return world;
    }
    // Negative flexibility tags submerged fronds, independently of atmospheric wind.
    if weights.y < 0.0 {
        let pulse = sin(world.x * 0.21 + world.z * 0.18 - world.y * 0.45 + time * 0.85);
        let flutter = sin(world.x * 0.8 - world.z * 0.6 + world.y * 0.7 + time * 1.4);
        let sway = (pulse * wind.x + flutter * wind.y) * abs(weight);
        return world + vec4<f32>(sway * 0.86, 0.0, sway * 0.51, 0.0);
    }
    let weather_wind = terra_weather.wind;
    let direction = select(vec2<f32>(0.86, 0.51), weather_wind.xy, weather_wind.w > 0.5);
    let strength = select(1.0, weather_wind.z, weather_wind.w > 0.5);
    let storm = clamp((strength - 1.0) / 3.0, 0.0, 1.0);
    // Broad travelling gusts move neighbouring plants together; the flutter
    // remains local, so strong wind never turns into synchronised rigid motion.
    let front = sin(world.x * 0.025 + world.z * 0.018 - time * 1.15);
    let gust = mix(sin(world.x * 0.33 + world.z * 0.27 + time * wind.z),
        0.62 + front * 0.38, storm);
    let flutter = sin(world.x * 1.17 - world.z * 0.91 + time * 2.1);
    let sway = (gust * wind.x + flutter * wind.y) * weight * strength;
    return world + vec4<f32>(sway * direction.x, 0.0, sway * direction.y, 0.0);
}

@vertex
fn vertex(input: Vertex) -> VertexOutput {
    var out: VertexOutput;
    let model = mesh_functions::get_world_from_local(input.instance_index);
@if(PROP_GROWTH) {
    // The unwrapped material clock cannot replay a completed transition hourly.
    var position = mesh_functions::mesh_position_local_to_world(model, grown_local(input, terra_weather.surface.w));
} @else {
    var position = mesh_functions::mesh_position_local_to_world(model, vec4<f32>(input.position, 1.0));
}
@if(PREPASS_PIPELINE) {
    let time = prop_globals.time;
} @else {
    let time = globals.time;
}
    var weights = vec2<f32>(0.0);
@if(VERTEX_UVS_A) {
    weights = input.uv;
}
@if(PROP_GROWTH) {
    weights.y *= growth_factor(input, terra_weather.surface.w);
    weights.y *= flower_scale(input, 0.0);
}
    out.world_position = wind_position(position, weights, time);
    out.position = position_world_to_clip(out.world_position.xyz);
@if(UNCLIPPED_DEPTH_ORTHO_EMULATION) {
    out.unclipped_depth = out.position.z;
    out.position.z = min(out.position.z, 1.0);
}

@if(PREPASS_PIPELINE) {
@if(NORMAL_PREPASS_OR_DEFERRED_PREPASS) {
@if(VERTEX_NORMALS) {
    out.world_normal = mesh_functions::mesh_normal_local_to_world(input.normal, input.instance_index);
}
@if(VERTEX_TANGENTS) {
    out.world_tangent = mesh_functions::mesh_tangent_local_to_world(model, input.tangent, input.instance_index);
}
}
@if(MOTION_VECTOR_PREPASS) {
    let previous_model = mesh_functions::get_previous_world_from_local(input.instance_index);
    var previous_weights = vec2<f32>(0.0);
@if(VERTEX_UVS_A) {
    previous_weights = input.uv;
}
@if(PROP_GROWTH) {
    let previous_clock = terra_weather.surface.w - prop_globals.delta_time;
    var previous = mesh_functions::mesh_position_local_to_world(previous_model, grown_local(input, previous_clock));
    previous_weights.y *= growth_factor(input, previous_clock);
    previous_weights.y *= flower_scale(input, prop_globals.delta_time);
} @else {
    var previous = mesh_functions::mesh_position_local_to_world(previous_model, vec4<f32>(input.position, 1.0));
}
    out.previous_world_position = wind_position(previous, previous_weights, time - prop_globals.delta_time);
}
} @else {
@if(VERTEX_NORMALS) {
    out.world_normal = mesh_functions::mesh_normal_local_to_world(input.normal, input.instance_index);
}
@if(VERTEX_TANGENTS) {
    out.world_tangent = mesh_functions::mesh_tangent_local_to_world(model, input.tangent, input.instance_index);
}
}
@if(VERTEX_UVS_A) {
    out.uv = input.uv;
}
@if(VERTEX_UVS_B) {
    out.uv_b = input.uv_b;
}
@if(VERTEX_COLORS) {
    out.color = input.color;
@if(PACKED_PROP_COLOR) {
    out.color = vec4<f32>(input.color.rgb * 2.0, input.color.a);
}
    out.color = vec4<f32>(out.color.rgb * terra_weather.tint.rgb, out.color.a);
    if weights.y > 0.0 {
        out.color = vec4<f32>(out.color.rgb * terra_weather.grass.rgb, out.color.a);
    }
    out.color.a = 1.0;
    if input.color.a < 1.0 {
        let progress = flower_progress(input, 0.0);
        let dry = smoothstep(0.0, 0.45, progress);
        let brightness = dot(out.color.rgb, vec3<f32>(0.30, 0.59, 0.11));
        // Fully replace saturated petals by halfway, rather than retaining
        // blue/green pigment that turns a darkening plant grey or olive.
        let dried = vec3<f32>(0.40, 0.19, 0.055) * (0.85 + clamp(brightness, 0.0, 1.0) * 0.35);
        out.color = vec4<f32>(mix(out.color.rgb, dried, dry), flower_scale(input, 0.0));
    }
}
@if(VERTEX_OUTPUT_INSTANCE_INDEX) {
    out.instance_index = input.instance_index;
}
@if(VISIBILITY_RANGE_DITHER) {
    out.visibility_range_dither = mesh_functions::get_visibility_range_dither_level(input.instance_index, model[3]);
}
    return out;
}
