import bevy_pbr::render::mesh_functions;
import bevy_pbr::render::view_transformations::position_world_to_clip;
@if(!BINDLESS)
import bevy_pbr::render::pbr_bindings::material;
@if(PREPASS_PIPELINE)
import bevy_pbr::prepass::io::Vertex;
@if(PREPASS_PIPELINE)
import bevy_pbr::prepass::io::decompress_vertex;
@if(PREPASS_PIPELINE)
import bevy_pbr::prepass::io::VertexOutput;
@if(PREPASS_PIPELINE)
import bevy_render::globals::Globals;
@if(!(PREPASS_PIPELINE))
import bevy_pbr::render::forward_io::Vertex;
@if(!(PREPASS_PIPELINE))
import bevy_pbr::render::forward_io::decompress_vertex;
@if(!(PREPASS_PIPELINE))
import bevy_pbr::render::forward_io::VertexOutput;
@if(!(PREPASS_PIPELINE))
import bevy_pbr::render::mesh_view_bindings::globals;

@if(PREPASS_PIPELINE) {
@group(0) @binding(1) var<uniform> leaf_globals: Globals;
} @else {
}

@if(DRY_OAK_WIND) {
const wind: vec4<f32> = vec4<f32>(0.043, 0.010, 0.85, 0.0);
} @else {
@group(constants::MATERIAL_BIND_GROUP) @binding(100) var<uniform> wind: vec4<f32>;
}
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

fn snow_albedo() -> vec3<f32> {
    let albedo = vec3<f32>(0.94, 0.97, 1.0);
@if(!BINDLESS) {
    // The texture keeps its relief; its green base tint must not tint the snow.
    return albedo / max(material.base_color.rgb, vec3<f32>(0.001));
} @else {
    return albedo;
}
}

fn wind_position(world: vec4<f32>, time: f32, rooted: vec2<f32>) -> vec4<f32> {
@if(FLAME_GEOMETRY) {
        let phase = world.x * 2.17 + world.z * 1.39;
        let flicker = sin(time * 9.0 + phase) * 0.65 + sin(time * 16.7 + phase) * 0.35;
        let height = rooted.y;
        let bend = height * height;
        return world + vec4<f32>(sin(time * wind.z + phase + height) * wind.x * bend,
            flicker * wind.y * height, cos(time * 5.3 + phase + height) * wind.x * bend, 0.0);
} @else {
@if(!DRY_OAK_WIND) {
    if rooted.y < 0.0 {
        let pulse = sin(world.x * 0.21 + world.z * 0.18 - world.y * 0.45 + time * 0.85);
        let flutter = sin(world.x * 0.8 - world.z * 0.6 + world.y * 0.7 + time * 1.4);
        let sway = (pulse * 0.026 + flutter * 0.007) * abs(rooted.y);
        return world + vec4<f32>(sway * 0.86, 0.0, sway * 0.51, 0.0);
    }
    if wind.w > 0.5 && rooted.y == 0.0 {
        return world;
    }
}
    // World-space phases keep shared corners identical across section/LOD borders.
    // Low spatial frequency and small displacement preserve the voxel silhouette.
    let weather_wind = terra_weather.wind;
    let direction = select(vec2<f32>(0.86, 0.51), weather_wind.xy, weather_wind.w > 0.5);
    let strength = select(1.0, weather_wind.z, weather_wind.w > 0.5);
    // Direction/strength may change, but phase must not jump at large world coordinates.
    let storm = clamp((strength - 1.0) / 3.0, 0.0, 1.0);
    var gust = sin(world.x * 0.33 + world.z * 0.27 + time * wind.z);
    if storm > 0.0 {
        let front = sin(world.x * 0.025 + world.z * 0.018 - time * 1.15);
        gust = mix(gust, 0.62 + front * 0.38, storm);
    }
    let flutter = sin(world.x * 1.17 - world.z * 0.91 + world.y * 0.67 + time * 2.3);
    let sway = (gust * wind.x + flutter * wind.y) * strength;
@if(!DRY_OAK_WIND) {
    if wind.w > 0.5 {
        return world + vec4<f32>(sway * direction.x, 0.0, sway * direction.y, 0.0) * rooted.y;
    }
}
    return world + vec4<f32>(sway * direction.x, flutter * wind.y * 0.35 * strength, sway * direction.y, 0.0);
}
}

// Vertex alpha carries either a deciduous shedding seed or evergreen climate.
fn deciduous_coverage(tag: f32, loss: f32) -> f32 {
    let seed = clamp((tag - 0.125) * 4.0, 0.0, 1.0);
    return 1.0 - smoothstep(seed * 0.84, seed * 0.84 + 0.16, loss);
}

fn foliage_coverage(tag: f32) -> f32 {
@if(DRY_OAK_WIND) {
    // In packed UNORM8 the only value above 0.999 is exactly one.
    let loss = clamp(terra_weather.leaves.w, 0.0, 1.0);
    if tag >= 0.5 || loss == 0.0 {
        return 1.0;
    }
    return deciduous_coverage(tag, loss);
} @else {
    if wind.w < 0.5 && tag < 0.5 {
        let loss = clamp(terra_weather.leaves.w, 0.0, 1.0);
        return deciduous_coverage(tag, loss);
    }
    if wind.w > 0.5 || (wind.w < 0.5 && tag >= 0.5 && tag < 0.999) {
        return 1.0;
    }
    return tag;
}
}

fn crown_normal(normal: vec3<f32>, instance: u32, translated: bool) -> vec3<f32> {
@if(DRY_OAK_WIND) {
    if translated {
        if any(normal != vec3<f32>(0.0)) { return normalize(normal); }
        return normal;
    }
}
    return mesh_functions::mesh_normal_local_to_world(normal, instance);
}

fn crown_tangent(model: mat4x4<f32>, tangent: vec4<f32>, instance: u32, translated: bool) -> vec4<f32> {
@if(DRY_OAK_WIND) {
    if translated {
        if any(tangent != vec4<f32>(0.0)) { return vec4<f32>(normalize(tangent.xyz), tangent.w); }
        return tangent;
    }
}
    return mesh_functions::mesh_tangent_local_to_world(model, tangent, instance);
}

@vertex
fn vertex(vertex_input: Vertex) -> VertexOutput {
    var out: VertexOutput;
    let input = decompress_vertex(vertex_input, vertex_input.instance_index);
    let model = mesh_functions::get_world_from_local(vertex_input.instance_index);
@if(DRY_OAK_WIND) {
    let translated = mesh_functions::get_tag(vertex_input.instance_index) == 0x544c4546u;
    var position: vec4<f32>;
    if translated {
        position = vec4<f32>(input.position + model[3].xyz, 1.0);
    } else {
        position = mesh_functions::mesh_position_local_to_world(model, vec4<f32>(input.position, 1.0));
    }
} @else {
    let translated = false;
    let position = mesh_functions::mesh_position_local_to_world(model, vec4<f32>(input.position, 1.0));
}
@if(PREPASS_PIPELINE) {
    let time = leaf_globals.time;
} @else {
    let time = globals.time;
}
    var rooted = vec2<f32>(0.0);
@if(VERTEX_UVS_B) {
    rooted = input.uv_b;
}
    out.world_position = wind_position(position, time, rooted);
    out.position = position_world_to_clip(out.world_position.xyz);
@if(UNCLIPPED_DEPTH_ORTHO_EMULATION) {
    out.unclipped_depth = out.position.z;
    out.position.z = min(out.position.z, 1.0);
}

@if(!PREPASS_PIPELINE || NORMAL_PREPASS_OR_DEFERRED_PREPASS) {
@if(VERTEX_NORMALS) {
    out.world_normal = crown_normal(input.normal, vertex_input.instance_index, translated);
}
@if(VERTEX_TANGENTS) {
    out.world_tangent = crown_tangent(model, input.tangent, vertex_input.instance_index, translated);
}
}
@if(PREPASS_PIPELINE && MOTION_VECTOR_PREPASS) {
    let previous_model = mesh_functions::get_previous_world_from_local(vertex_input.instance_index);
    let previous = mesh_functions::mesh_position_local_to_world(previous_model, vec4<f32>(input.position, 1.0));
    out.previous_world_position = wind_position(previous, time - leaf_globals.delta_time, rooted);
}
@if(VERTEX_UVS_A) {
    out.uv = input.uv;
}
@if(VERTEX_UVS_B) {
    out.uv_b = input.uv_b;
}
@if(VERTEX_COLORS) {
    out.color = input.color;
@if(!FLAME_GEOMETRY) {
    out.color.a = foliage_coverage(input.color.a);
}
// Only alpha enters the non-deferred depth, normal, velocity and shadow passes.
@if(!PREPASS_PIPELINE || DEFERRED_PREPASS) {
@if(FLAME_GEOMETRY) {
    let pulse = 0.88 + 0.12 * sin(time * 8.7 + position.x * 2.17 + position.z * 1.39);
    out.color = vec4<f32>(input.color.rgb * vec3<f32>(1.0, pulse, pulse * pulse), input.color.a);
}
@if(PACKED_LEAF_COLOR) {
    // Crowns encode a 0..2 range. Rooted fibres use ordinary UNORM colors.
    if wind.w < 0.5 {
        out.color = vec4<f32>(input.color.rgb * 2.0, out.color.a);
    }
}
@if(!FLAME_GEOMETRY) {
    if rooted.y >= 0.0 {
        var seasonal = select(terra_weather.leaves.rgb, terra_weather.grass.rgb, wind.w > 0.5);
        if wind.w < -0.5 {
            seasonal = vec3<f32>(1.0);
        }
        // Dark spruce crown metadata keeps evergreen foliage mostly green.
        let deciduous = select(smoothstep(0.35, 0.85, out.color.r), 1.0, wind.w > 0.5);
        out.color = vec4<f32>(out.color.rgb * mix(vec3<f32>(1.0), seasonal, deciduous), out.color.a);
    }
    if wind.w < 0.5 && input.color.a >= 0.5 && input.color.a < 0.999 && terra_weather.vegetation.y > 0.0 && terra_weather.grass.w > 0.0 {
        // Evergreen crown tags retain foliage and carry their own local climate.
        let climate_tag = (input.color.a - 0.5) / 0.49;
        let temperature = ((climate_tag * 255.0 - 2.0) / 251.0) * 128.0 - 64.0 + terra_weather.tint.w;
        let frost = 1.0 - smoothstep(-1.0, 3.0, temperature);
        var upward = 0.0;
@if(VERTEX_NORMALS) {
        upward = smoothstep(0.15, 0.65, input.normal.y);
}
        let snow = smoothstep(0.0, 0.95, terra_weather.grass.w * frost * clamp(terra_weather.vegetation.y, 0.0, 1.0)) * upward;
        out.color = vec4<f32>(mix(out.color.rgb, snow_albedo(), snow), out.color.a);
    }
    if wind.w > 0.5 {
        if rooted.y > 0.0 && input.color.a > 0.001 && input.color.a < 0.999 {
            let temperature = ((input.color.a * 255.0 - 2.0) / 251.0) * 128.0 - 64.0 + terra_weather.tint.w;
            let snow = smoothstep(0.0, 0.95, terra_weather.grass.w * (1.0 - smoothstep(-1.0, 3.0, temperature)) * clamp(terra_weather.vegetation.y, 0.0, 1.0));
            out.color = vec4<f32>(mix(out.color.rgb, snow_albedo(), snow), out.color.a);
        }
    }
}
}
}
@if(VERTEX_OUTPUT_INSTANCE_INDEX) {
    out.instance_index = vertex_input.instance_index;
}
@if(VISIBILITY_RANGE_DITHER) {
    out.visibility_range_dither = mesh_functions::get_visibility_range_dither_level(vertex_input.instance_index, model[3]);
}
    return out;
}
