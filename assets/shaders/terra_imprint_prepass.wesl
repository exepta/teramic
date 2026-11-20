// Preserve the existing PBR depth/normal/motion passes while cutting the same
// real footprint openings as the color pass. No renderer dependency is patched.
import bevy_pbr::render::{pbr_prepass_functions, pbr_bindings, pbr_types, pbr_functions};
import bevy_pbr::render::mesh_bindings::mesh;
import bevy_pbr::render::mesh_view_bindings::view;
import bevy_pbr::prepass::io as prepass_io;
import bevy_render::bindless::{bindless_samplers_filtering, bindless_textures_2d};

@if(BINDLESS)
enable wgpu_binding_array;

struct TerraWeather {
    wind: vec4<f32>, surface: vec4<f32>, tint: vec4<f32>,
    leaves: vec4<f32>, grass: vec4<f32>, vegetation: vec4<f32>,
    flowers: array<vec4<f32>, 64>,
    footprints: array<vec4<f32>, 1024>,
    liquid: vec4<f32>,
    particle: vec4<f32>,
};
@if(BINDLESS) {
struct WetMaterialIndices { weather: u32, detail: u32, };
struct WetParameters { detail: vec4<f32>, };
@group(constants::MATERIAL_BIND_GROUP) @binding(100) var<storage, read> wet_indices: array<WetMaterialIndices>;
@group(constants::MATERIAL_BIND_GROUP) @binding(101) var<storage, read> wet_weather: binding_array<TerraWeather>;
@group(constants::MATERIAL_BIND_GROUP) @binding(102) var<storage, read> wet_parameters: array<WetParameters>;
} @else {
@group(constants::MATERIAL_BIND_GROUP) @binding(101) var<storage, read> terra_weather: TerraWeather;
@group(constants::MATERIAL_BIND_GROUP) @binding(102) var<uniform> terra_detail: vec4<f32>;
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

fn clip_imprint(input: prepass_io::VertexOutput) {
@if(VERTEX_UVS_B) {
@if(BINDLESS) {
    let slot = mesh[input.instance_index].material_and_lightmap_bind_group_slot & 0xffffu;
    let family = abs(wet_parameters[wet_indices[slot].detail].detail.x);
    let has_imprints = wet_weather[wet_indices[slot].weather].vegetation.w;
    let particle_domain = wet_weather[wet_indices[slot].weather].particle;
} @else {
    let slot = 0u;
    let family = abs(terra_detail.x);
    let has_imprints = terra_weather.vegetation.w;
    let particle_domain = terra_weather.particle;
}
    let face = cross(dpdx(input.world_position.xyz), dpdy(input.world_position.xyz));
    if has_imprints > 0.5 && (family == 4.0 || family == 5.0) && abs(face.y) > length(face) * 0.98
        && imprint_cutout(input.world_position.xyz, slot) { discard; }
}
}

@if(PREPASS_FRAGMENT)
@fragment
fn fragment(input: prepass_io::VertexOutput, @builtin(front_facing) front: bool) -> prepass_io::FragmentOutput {
    clip_imprint(input);
@if(VISIBILITY_RANGE_DITHER)
    pbr_functions::visibility_range_dither(input.position, input.visibility_range_dither);
    pbr_prepass_functions::prepass_sample_color_and_alpha_discard(input);
    var out: prepass_io::FragmentOutput;
@if(UNCLIPPED_DEPTH_ORTHO_EMULATION)
    out.frag_depth = input.unclipped_depth;
@if(NORMAL_PREPASS) {
@if(BINDLESS) {
    let slot = mesh[input.instance_index].material_and_lightmap_bind_group_slot & 0xffffu;
    let indices = pbr_bindings::material_indices[slot];
    let material = pbr_bindings::material_array[indices.material];
} @else {
    let material = pbr_bindings::material;
}
    let double_sided = (material.flags & pbr_types::STANDARD_MATERIAL_FLAGS_DOUBLE_SIDED_BIT) != 0u;
    let logical_front = pbr_functions::winding_corrected_front_facing(mesh[input.instance_index].flags, front);
    var normal = pbr_functions::prepare_world_normal(input.world_normal, double_sided, logical_front);
@if(VERTEX_UVS && VERTEX_TANGENTS && STANDARD_MATERIAL_NORMAL_MAP) {
@if(STANDARD_MATERIAL_NORMAL_MAP_UV_B)
    let uv = (material.uv_transform * vec3<f32>(input.uv_b, 1.0)).xy;
@else
    let uv = (material.uv_transform * vec3<f32>(input.uv, 1.0)).xy;
@if(BINDLESS)
    let nt = textureSampleBias(bindless_textures_2d[indices.normal_map_texture], bindless_samplers_filtering[indices.normal_map_sampler], uv, view.mip_bias).rgb;
@else
    let nt = textureSampleBias(pbr_bindings::normal_map_texture, pbr_bindings::normal_map_sampler, uv, view.mip_bias).rgb;
    normal = pbr_functions::apply_normal_mapping(material.flags, pbr_functions::calculate_tbn_mikktspace(normal, input.world_tangent), double_sided, logical_front, nt);
}
    out.normal = vec4<f32>(normal * 0.5 + vec3<f32>(0.5), 1.0);
}
@if(MOTION_VECTOR_PREPASS)
    out.motion_vector = pbr_prepass_functions::calculate_motion_vector(input.world_position, input.previous_world_position);
    return out;
}
@else
@fragment
fn fragment(input: prepass_io::VertexOutput) {
    clip_imprint(input);
    pbr_prepass_functions::prepass_sample_color_and_alpha_discard(input);
}
