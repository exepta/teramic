#import bevy_pbr::{mesh_functions, view_transformations::position_world_to_clip}
#ifdef PREPASS_PIPELINE
#import bevy_pbr::prepass_io::{Vertex, VertexOutput}
#else
#import bevy_pbr::forward_io::{Vertex, VertexOutput}
#endif

@group(#{MATERIAL_BIND_GROUP}) @binding(100) var<uniform> fall_age: vec4<f32>;

fn fall_position(position: vec3<f32>, cell: vec4<f32>, age: f32) -> vec3<f32> {
    let seed = fract(sin(dot(cell.xyz, vec3<f32>(12.9898, 78.233, 19.171))) * 43758.5453);
    let elapsed = max(0.0, age - seed * 0.32);
    let drop = min(cell.w, 5.5 * elapsed * elapsed);
    let shrink = 1.0 - smoothstep(fall_age.y - 0.7, fall_age.y, age);
    return cell.xyz + (position - cell.xyz) * shrink - vec3<f32>(0.0, drop, 0.0);
}

@vertex
fn vertex(input: Vertex, @location(8) cell: vec4<f32>) -> VertexOutput {
    var out: VertexOutput;
    let model = mesh_functions::get_world_from_local(input.instance_index);
    out.world_position = mesh_functions::mesh_position_local_to_world(model,
        vec4<f32>(fall_position(input.position, cell, fall_age.x), 1.0));
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
    out.previous_world_position = mesh_functions::mesh_position_local_to_world(previous_model,
        vec4<f32>(fall_position(input.position, cell, max(0.0, fall_age.x - fall_age.z)), 1.0));
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
#endif
#ifdef VERTEX_OUTPUT_INSTANCE_INDEX
    out.instance_index = input.instance_index;
#endif
#ifdef VISIBILITY_RANGE_DITHER
    out.visibility_range_dither = mesh_functions::get_visibility_range_dither_level(input.instance_index, model[3]);
#endif
    return out;
}
