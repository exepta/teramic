#import bevy_pbr::{mesh_functions, view_transformations::position_world_to_clip}

#ifdef PREPASS_PIPELINE
#import bevy_pbr::prepass_io::VertexOutput
#import bevy_render::globals::Globals
@group(0) @binding(1) var<uniform> prop_globals: Globals;
#else
#import bevy_pbr::{forward_io::VertexOutput, mesh_view_bindings::globals}
#endif

#ifdef PROP_GROWTH
struct Vertex {
    @builtin(instance_index) instance_index: u32,
    @location(0) position: vec3<f32>,
#ifdef PREPASS_PIPELINE
    @location(1) uv: vec2<f32>,
    @location(2) uv_b: vec2<f32>,
    @location(3) normal: vec3<f32>,
    @location(7) color: vec4<f32>,
#else
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) uv_b: vec2<f32>,
    @location(5) color: vec4<f32>,
#endif
    @location(8) growth_anchor: vec3<f32>,
};
#else
#ifdef PREPASS_PIPELINE
#import bevy_pbr::prepass_io::Vertex
#else
#import bevy_pbr::forward_io::Vertex
#endif
#endif

@group(#{MATERIAL_BIND_GROUP}) @binding(100) var<uniform> wind: vec4<f32>;
struct TerraWeather {
    wind: vec4<f32>,
    surface: vec4<f32>,
    tint: vec4<f32>,
    leaves: vec4<f32>,
    grass: vec4<f32>,
};
@group(#{MATERIAL_BIND_GROUP}) @binding(101) var<storage, read> terra_weather: TerraWeather;

#ifdef PROP_GROWTH
fn growth_factor(input: Vertex, time: f32) -> f32 {
    let elapsed = clamp((time - input.uv_b.x) / 2.4, 0.0, 1.0);
    return mix(input.uv_b.y, 1.0, elapsed * elapsed * (3.0 - 2.0 * elapsed));
}

fn grown_local(input: Vertex, time: f32) -> vec4<f32> {
    let scale = growth_factor(input, time);
    return vec4<f32>(input.growth_anchor + (input.position - input.growth_anchor) * scale, 1.0);
}
#endif

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

fn prop_snow(input: Vertex) -> f32 {
#ifdef VERTEX_COLORS
#ifdef VERTEX_UVS_A
    // Zero is sheltered; one is untagged. Negative flexibility belongs underwater.
    if input.uv.y > 0.0 && input.color.a > 0.0 && input.color.a < 1.0 {
        let temperature = ((input.color.a * 255.0 - 2.0) / 251.0) * 128.0 - 64.0
            + terra_weather.tint.w;
        let cold = 1.0 - smoothstep(-1.0, 3.0, temperature);
        return smoothstep(0.0, 0.95, terra_weather.grass.w * cold);
    }
#endif
#endif
    return 0.0;
}

@vertex
fn vertex(input: Vertex) -> VertexOutput {
    var out: VertexOutput;
    let model = mesh_functions::get_world_from_local(input.instance_index);
#ifdef PROP_GROWTH
    // The unwrapped material clock cannot replay a completed transition hourly.
    var position = mesh_functions::mesh_position_local_to_world(model, grown_local(input, terra_weather.surface.w));
#else
    var position = mesh_functions::mesh_position_local_to_world(model, vec4<f32>(input.position, 1.0));
#endif
    let snow_cover = prop_snow(input);
#ifdef PROP_GROWTH
    let snow_root = mesh_functions::mesh_position_local_to_world(model, vec4<f32>(input.growth_anchor, 1.0));
    position.y = mix(position.y, snow_root.y + 0.012, snow_cover * 0.98);
#else
#ifdef VERTEX_UVS_A
    position.y -= sqrt(max(input.uv.x, 0.0)) * snow_cover * 0.18;
#endif
#endif
#ifdef PREPASS_PIPELINE
    let time = prop_globals.time;
#else
    let time = globals.time;
#endif
    var weights = vec2<f32>(0.0);
#ifdef VERTEX_UVS_A
    weights = input.uv;
#endif
#ifdef PROP_GROWTH
    weights.y *= growth_factor(input, terra_weather.surface.w);
#endif
    out.world_position = wind_position(position, weights, time);
    out.position = position_world_to_clip(out.world_position.xyz);
#ifdef UNCLIPPED_DEPTH_ORTHO_EMULATION
    out.unclipped_depth = out.position.z;
    out.position.z = min(out.position.z, 1.0);
#endif

#ifdef PREPASS_PIPELINE
#ifdef NORMAL_PREPASS_OR_DEFERRED_PREPASS
#ifdef VERTEX_NORMALS
    out.world_normal = mesh_functions::mesh_normal_local_to_world(input.normal, input.instance_index);
#endif
#ifdef VERTEX_TANGENTS
    out.world_tangent = mesh_functions::mesh_tangent_local_to_world(model, input.tangent, input.instance_index);
#endif
#endif
#ifdef MOTION_VECTOR_PREPASS
    let previous_model = mesh_functions::get_previous_world_from_local(input.instance_index);
    var previous_weights = vec2<f32>(0.0);
#ifdef VERTEX_UVS_A
    previous_weights = input.uv;
#endif
#ifdef PROP_GROWTH
    let previous_clock = terra_weather.surface.w - prop_globals.delta_time;
    var previous = mesh_functions::mesh_position_local_to_world(previous_model, grown_local(input, previous_clock));
    previous_weights.y *= growth_factor(input, previous_clock);
    let previous_snow_root = mesh_functions::mesh_position_local_to_world(previous_model, vec4<f32>(input.growth_anchor, 1.0));
    previous.y = mix(previous.y, previous_snow_root.y + 0.012, snow_cover * 0.98);
#else
    var previous = mesh_functions::mesh_position_local_to_world(previous_model, vec4<f32>(input.position, 1.0));
#ifdef VERTEX_UVS_A
    previous.y -= sqrt(max(input.uv.x, 0.0)) * snow_cover * 0.18;
#endif
#endif
    out.previous_world_position = wind_position(previous, previous_weights, time - prop_globals.delta_time);
#endif
#else
#ifdef VERTEX_NORMALS
    out.world_normal = mesh_functions::mesh_normal_local_to_world(input.normal, input.instance_index);
#endif
#ifdef VERTEX_TANGENTS
    out.world_tangent = mesh_functions::mesh_tangent_local_to_world(model, input.tangent, input.instance_index);
#endif
#endif
#ifdef VERTEX_UVS_A
    out.uv = input.uv;
#endif
#ifdef VERTEX_UVS_B
    out.uv_b = input.uv_b;
#endif
#ifdef VERTEX_COLORS
    out.color = input.color;
#ifdef PACKED_PROP_COLOR
    out.color = vec4<f32>(input.color.rgb * 2.0, input.color.a);
#endif
    out.color = vec4<f32>(out.color.rgb * terra_weather.tint.rgb, out.color.a);
    if weights.y > 0.0 {
        out.color = vec4<f32>(out.color.rgb * terra_weather.grass.rgb, out.color.a);
        out.color = vec4<f32>(mix(out.color.rgb, vec3<f32>(0.88, 0.91, 0.95), snow_cover), out.color.a);
    }
    out.color.a = 1.0;
#endif
#ifdef VERTEX_OUTPUT_INSTANCE_INDEX
    out.instance_index = input.instance_index;
#endif
#ifdef VISIBILITY_RANGE_DITHER
    out.visibility_range_dither = mesh_functions::get_visibility_range_dither_level(input.instance_index, model[3]);
#endif
    return out;
}
