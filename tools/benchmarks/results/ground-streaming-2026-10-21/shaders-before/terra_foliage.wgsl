#import bevy_pbr::{mesh_functions, view_transformations::position_world_to_clip}

#ifdef PREPASS_PIPELINE
#import bevy_pbr::prepass_io::{Vertex, VertexOutput}
#import bevy_render::globals::Globals
@group(0) @binding(1) var<uniform> leaf_globals: Globals;
#else
#import bevy_pbr::{forward_io::{Vertex, VertexOutput}, mesh_view_bindings::globals}
#endif

@group(#{MATERIAL_BIND_GROUP}) @binding(100) var<uniform> wind: vec4<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(101) var<uniform> weather_wind: vec4<f32>;

fn wind_position(world: vec4<f32>, time: f32, rooted: vec2<f32>) -> vec4<f32> {
#ifdef FLAME_GEOMETRY
        let phase = world.x * 2.17 + world.z * 1.39;
        let flicker = sin(time * 9.0 + phase) * 0.65 + sin(time * 16.7 + phase) * 0.35;
        let height = rooted.y;
        let bend = height * height;
        return world + vec4<f32>(sin(time * wind.z + phase + height) * wind.x * bend,
            flicker * wind.y * height, cos(time * 5.3 + phase + height) * wind.x * bend, 0.0);
#else
    if rooted.y < 0.0 {
        let pulse = sin(world.x * 0.21 + world.z * 0.18 - world.y * 0.45 + time * 0.85);
        let flutter = sin(world.x * 0.8 - world.z * 0.6 + world.y * 0.7 + time * 1.4);
        let sway = (pulse * 0.026 + flutter * 0.007) * abs(rooted.y);
        return world + vec4<f32>(sway * 0.86, 0.0, sway * 0.51, 0.0);
    }
    if wind.w > 0.5 && rooted.y == 0.0 {
        return world;
    }
    // World-space phases keep shared corners identical across section/LOD borders.
    // Low spatial frequency and small displacement preserve the voxel silhouette.
    let direction = select(vec2<f32>(0.86, 0.51), weather_wind.xy, weather_wind.w > 0.5);
    let strength = select(1.0, weather_wind.z, weather_wind.w > 0.5);
    // Direction/strength may change, but phase must not jump at large world coordinates.
    let storm = clamp((strength - 1.0) / 3.0, 0.0, 1.0);
    let front = sin(world.x * 0.025 + world.z * 0.018 - time * 1.15);
    let gust = mix(sin(world.x * 0.33 + world.z * 0.27 + time * wind.z),
        0.62 + front * 0.38, storm);
    let flutter = sin(world.x * 1.17 - world.z * 0.91 + world.y * 0.67 + time * 2.3);
    let sway = (gust * wind.x + flutter * wind.y) * strength;
    if wind.w > 0.5 {
        return world + vec4<f32>(sway * direction.x, 0.0, sway * direction.y, 0.0) * rooted.y;
    }
    return world + vec4<f32>(sway * direction.x, flutter * wind.y * 0.35 * strength, sway * direction.y, 0.0);
#endif
}

@vertex
fn vertex(input: Vertex) -> VertexOutput {
    var out: VertexOutput;
    let model = mesh_functions::get_world_from_local(input.instance_index);
    let position = mesh_functions::mesh_position_local_to_world(model, vec4<f32>(input.position, 1.0));
#ifdef PREPASS_PIPELINE
    let time = leaf_globals.time;
#else
    let time = globals.time;
#endif
    var rooted = vec2<f32>(0.0);
#ifdef VERTEX_UVS_B
    rooted = input.uv_b;
#endif
    out.world_position = wind_position(position, time, rooted);
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
    let previous = mesh_functions::mesh_position_local_to_world(previous_model, vec4<f32>(input.position, 1.0));
    out.previous_world_position = wind_position(previous, time - leaf_globals.delta_time, rooted);
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
#ifdef FLAME_GEOMETRY
    let pulse = 0.88 + 0.12 * sin(time * 8.7 + position.x * 2.17 + position.z * 1.39);
    out.color = vec4<f32>(input.color.rgb * vec3<f32>(1.0, pulse, pulse * pulse), input.color.a);
#endif
#ifdef PACKED_LEAF_COLOR
    out.color = vec4<f32>(input.color.rgb * 2.0, input.color.a);
#endif
#endif
#ifdef VERTEX_OUTPUT_INSTANCE_INDEX
    out.instance_index = input.instance_index;
#endif
#ifdef VISIBILITY_RANGE_DITHER
    out.visibility_range_dither = mesh_functions::get_visibility_range_dither_level(input.instance_index, model[3]);
#endif
    return out;
}
