import bevy_pbr::render::mesh_functions;
import bevy_pbr::render::view_transformations::position_world_to_clip;
import bevy_pbr::render::mesh_bindings::mesh;
@if(PREPASS_PIPELINE)
import bevy_pbr::prepass::io::Vertex;
@if(PREPASS_PIPELINE)
import bevy_pbr::prepass::io::VertexOutput;
@if(PREPASS_PIPELINE)
import bevy_pbr::prepass::io::decompress_vertex;
@if(!(PREPASS_PIPELINE))
import bevy_pbr::render::forward_io::Vertex;
@if(!(PREPASS_PIPELINE))
import bevy_pbr::render::forward_io::VertexOutput;
@if(!(PREPASS_PIPELINE))
import bevy_pbr::render::forward_io::decompress_vertex;

@if(BINDLESS)
enable wgpu_binding_array;
struct TerraWeather {
    wind: vec4<f32>, surface: vec4<f32>, tint: vec4<f32>,
    leaves: vec4<f32>, grass: vec4<f32>, vegetation: vec4<f32>,
    flowers: array<vec4<f32>, 64>, footprints: array<vec4<f32>, 1024>,
    liquid: vec4<f32>,
    particle: vec4<f32>,
};
@if(BINDLESS) {
struct WetMaterialIndices { weather: u32, detail: u32, };
@group(constants::MATERIAL_BIND_GROUP) @binding(100) var<storage, read> wet_indices: array<WetMaterialIndices>;
@group(constants::MATERIAL_BIND_GROUP) @binding(101) var<storage, read> wet_weather: binding_array<TerraWeather>;
} @else {
@group(constants::MATERIAL_BIND_GROUP) @binding(101) var<storage, read> terra_weather: TerraWeather;
}

fn gel_position(world: vec4<f32>, seconds: f32, flow: vec2<f32>, depth: f32, galle: bool) -> vec4<f32> {
    let direction = select(vec2<f32>(0.77, 0.64), flow / max(length(flow), 0.001), dot(flow, flow) > 0.01);
    let across = vec2<f32>(-direction.y, direction.x);
    let time = seconds * select(0.65, 0.95, galle);
    let fold = sin(dot(world.xz, direction) * 5.3 - time) * cos(dot(world.xz, across) * 4.1 + sin(time * 0.43));
    return world + vec4<f32>(0.0, fold * min(depth * 0.10, 0.018), 0.0, 0.0);
}
@vertex
fn vertex(vertex_input: Vertex,
@if(LIQUID_MOTION)
    @location(11) liquid_from: vec4<f32>,
@if(LIQUID_MOTION)
    @location(12) liquid_data: vec4<f32>,
) -> VertexOutput {
    var out: VertexOutput;
    let input = decompress_vertex(vertex_input, vertex_input.instance_index);
    let model = mesh_functions::get_world_from_local(input.instance_index);
    var local = input.position;
    var seconds = 0.0;
    var delta_seconds = 0.0;
@if(LIQUID_MOTION) {
@if(BINDLESS) {
    let slot = mesh[input.instance_index].material_and_lightmap_bind_group_slot & 0xffffu;
    seconds = wet_weather[wet_indices[slot].weather].liquid.x;
    delta_seconds = wet_weather[wet_indices[slot].weather].liquid.y;
} @else {
    seconds = terra_weather.liquid.x;
    delta_seconds = terra_weather.liquid.y;
}
    let phase = clamp((seconds - liquid_from.w) / max(liquid_data.x, 0.001), 0.0, 1.0);
    local = mix(liquid_from.xyz, local, phase * phase * (3.0 - 2.0 * phase));
}
    out.world_position = mesh_functions::mesh_position_local_to_world(model, vec4<f32>(local, 1.0));
@if(LIQUID_MOTION) {
@if(VERTEX_UVS_B) {
    if input.uv_b.y < -2.5 {
        out.world_position = gel_position(out.world_position, seconds, liquid_data.yz, liquid_data.w, input.uv_b.y < -3.5);
    }
}
}
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
    var previous_local = input.position;
@if(LIQUID_MOTION) {
    let previous_phase = clamp((seconds - delta_seconds - liquid_from.w) / max(liquid_data.x, 0.001), 0.0, 1.0);
    previous_local = mix(liquid_from.xyz, previous_local, previous_phase * previous_phase * (3.0 - 2.0 * previous_phase));
}
    out.previous_world_position = mesh_functions::mesh_position_local_to_world(previous_model, vec4<f32>(previous_local, 1.0));
@if(LIQUID_MOTION) {
@if(VERTEX_UVS_B) {
    if input.uv_b.y < -2.5 {
        out.previous_world_position = gel_position(out.previous_world_position, seconds - delta_seconds, liquid_data.yz, liquid_data.w, input.uv_b.y < -3.5);
    }
}
}
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
@if(LIQUID_MOTION) {
@if(VERTEX_UVS_B) {
    if input.uv_b.y < -2.5 {
        let slime = select(0.055, 0.10, input.uv_b.y < -3.5);
        out.uv += liquid_data.yz * seconds * slime;
    }
}
}
}
@if(VERTEX_UVS_B) {
    out.uv_b = input.uv_b;
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
    return out;
}
