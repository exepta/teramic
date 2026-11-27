import bevy_pbr::render::pbr_prepass_functions;
import bevy_pbr::render::pbr_bindings;
import bevy_pbr::render::pbr_types;
import bevy_pbr::render::pbr_functions;
import bevy_pbr::render::pbr_functions::SampleBias;
import bevy_pbr::prepass::io as prepass_io;
import bevy_pbr::render::mesh_bindings::mesh;
import bevy_pbr::render::mesh_view_bindings::view;
import bevy_render::bindless::bindless_samplers_filtering;
import bevy_render::bindless::bindless_textures_2d;
@if(BINDLESS)
import bevy_pbr::render::pbr_bindings::material_indices;

// Bevy's standard prepass ignores vertex alpha. Keep its PBR outputs while
// matching the forward leaf mask, including seasonal shedding, in depth/shadows.

@group(constants::MATERIAL_BIND_GROUP) @binding(100) var<uniform> wind: vec4<f32>;

@if(LEAF_SHADOW_PROFILE) {
const THREE_LOBES: array<vec2<f32>, 15> = array(
    vec2<f32>(0.00, 0.10),
    vec2<f32>(0.15, 0.10),
    vec2<f32>(0.19, 0.40),
    vec2<f32>(0.21, 0.90),
    vec2<f32>(0.27, 1.00),
    vec2<f32>(0.48, 1.00),
    vec2<f32>(0.52, 0.77),
    vec2<f32>(0.53, 0.37),
    vec2<f32>(0.68, 0.37),
    vec2<f32>(0.70, 0.55),
    vec2<f32>(0.92, 0.55),
    vec2<f32>(1.00, 0.39),
    vec2<f32>(1.0, 0.0),
    vec2<f32>(1.0, 0.0),
    vec2<f32>(1.0, 0.0)
);
const FIVE_LOBES: array<vec2<f32>, 15> = array(
    vec2<f32>(0.00, 0.10),
    vec2<f32>(0.14, 0.10),
    vec2<f32>(0.18, 0.43),
    vec2<f32>(0.20, 0.92),
    vec2<f32>(0.25, 1.00),
    vec2<f32>(0.37, 1.00),
    vec2<f32>(0.39, 0.43),
    vec2<f32>(0.49, 0.43),
    vec2<f32>(0.51, 0.83),
    vec2<f32>(0.69, 0.83),
    vec2<f32>(0.72, 0.35),
    vec2<f32>(0.80, 0.35),
    vec2<f32>(0.82, 0.48),
    vec2<f32>(0.95, 0.48),
    vec2<f32>(1.00, 0.32)
);

// Each nibble is the last station at or before (bin + 1) / 128 for the
// three- and five-lobed profiles. A bin can contain at most one station.
const BLADE_STATIONS: array<vec2<u32>, 16> = array(
    vec2<u32>(0x00000000u, 0x00000000u),
    vec2<u32>(0x00000000u, 0x00000000u),
    vec2<u32>(0x11111000u, 0x21111110u),
    vec2<u32>(0x33333322u, 0x43333332u),
    vec2<u32>(0x44444433u, 0x44444444u),
    vec2<u32>(0x44444444u, 0x54444444u),
    vec2<u32>(0x44444444u, 0x66666665u),
    vec2<u32>(0x55544444u, 0x77666666u),
    vec2<u32>(0x77777655u, 0x88888887u),
    vec2<u32>(0x77777777u, 0x88888888u),
    vec2<u32>(0x87777777u, 0x88888888u),
    vec2<u32>(0x99999998u, 0xaaaa9999u),
    vec2<u32>(0x99999999u, 0xbbaaaaaau),
    vec2<u32>(0x99999999u, 0xccccccccu),
    vec2<u32>(0xaaa99999u, 0xccccccccu),
    vec2<u32>(0xbaaaaaaau, 0xeddddddcu)
);

fn blade_covers(uv: vec2<f32>, encoded: vec2<f32>) -> bool {
    let bytes = vec2<u32>(round(encoded * 255.0));
    let mask = bytes.x | (bytes.y << 8u);
    let five = (mask & 32768u) != 0u;
    let bin = min(u32(clamp(uv.x, 0.0, 1.0) * 128.0), 127u);
    let packed = BLADE_STATIONS[bin >> 3u];
    let candidate = (select(packed.x, packed.y, five) >> ((bin & 7u) * 4u)) & 15u;
    let station = select(THREE_LOBES[candidate].x, FIVE_LOBES[candidate].x, five);
    let upper = clamp(candidate + u32(uv.x > station), 1u, select(11u, 14u, five));
    // Select the nearest retained stations on either side of this sample.
    // The mesher always retains both endpoints, at every detail level.
    let lower_bits = (1u << upper) - 1u;
    let lo = firstLeadingBit(mask & lower_bits);
    let hi = firstTrailingBit(mask & ~lower_bits);
    let a = select(THREE_LOBES[lo], FIVE_LOBES[lo], five);
    let b = select(THREE_LOBES[hi], FIVE_LOBES[hi], five);
    let t = (min(uv.x, 1.0) - a.x) / (b.x - a.x);
    return uv.y <= mix(a.y, b.y, t);
}
}

fn leaf_alpha_discard(in: prepass_io::VertexOutput) {
@if(LEAF_SHADOW_PROFILE) {
    if !blade_covers(in.uv, in.color.rg) { discard; }
}
@if(!OPAQUE_LEAF_PAINT) {
    // Rooted fibres and flames retain the engine's original alpha semantics.
    if wind.w >= 0.5 {
        pbr_prepass_functions::prepass_sample_color_and_alpha_discard(in);
        return;
    }
}
@if(MAY_DISCARD) {
@if(BINDLESS) {
    let slot = mesh[in.instance_index].material_and_lightmap_bind_group_slot & 0xffffu;
    var output_color = pbr_bindings::material_array[material_indices[slot].material].base_color;
    let alpha_cutoff = pbr_bindings::material_array[material_indices[slot].material].alpha_cutoff;
} @else {
    var output_color = pbr_bindings::material.base_color;
    let alpha_cutoff = pbr_bindings::material.alpha_cutoff;
}
@if(VERTEX_UVS && !OPAQUE_LEAF_PAINT) {
@if(BINDLESS) {
    let flags = pbr_bindings::material_array[material_indices[slot].material].flags;
    let uv_transform = pbr_bindings::material_array[material_indices[slot].material].uv_transform;
} @else {
    let flags = pbr_bindings::material.flags;
    let uv_transform = pbr_bindings::material.uv_transform;
}
@if(STANDARD_MATERIAL_BASE_COLOR_UV_B) {
    var uv = in.uv_b;
} @else {
    var uv = in.uv;
}
    uv = (uv_transform * vec3(uv, 1.0)).xy;
    if (flags & pbr_types::STANDARD_MATERIAL_FLAGS_BASE_COLOR_TEXTURE_BIT) != 0u {
@if(BINDLESS) {
        let color_texture = bindless_textures_2d[material_indices[slot].base_color_texture];
        let color_sampler = bindless_samplers_filtering[material_indices[slot].base_color_sampler];
} @else {
        let color_texture = pbr_bindings::base_color_texture;
        let color_sampler = pbr_bindings::base_color_sampler;
}
        output_color *= textureSampleBias(
            color_texture,
            color_sampler,
            uv,
            view.mip_bias
        );
    }
}
@if(VERTEX_COLORS) {
    output_color.a *= in.color.a;
}
    // LeafExtension prepares wind.w == 0 exclusively as AlphaMode::Mask.
    if output_color.a < alpha_cutoff {
        discard;
    }
}
}

@if(PREPASS_FRAGMENT) {
@fragment
fn fragment(
    in: prepass_io::VertexOutput,
    @builtin(front_facing) is_front: bool,
) -> prepass_io::FragmentOutput {
@if(BINDLESS) {
    let slot = mesh[in.instance_index].material_and_lightmap_bind_group_slot & 0xffffu;
    let flags = pbr_bindings::material_array[material_indices[slot].material].flags;
    let uv_transform = pbr_bindings::material_array[material_indices[slot].material].uv_transform;
} @else {
    let flags = pbr_bindings::material.flags;
    let uv_transform = pbr_bindings::material.uv_transform;
}
@if(VISIBILITY_RANGE_DITHER) {
    pbr_functions::visibility_range_dither(in.position, in.visibility_range_dither);
}
    leaf_alpha_discard(in);
    var out: prepass_io::FragmentOutput;
@if(UNCLIPPED_DEPTH_ORTHO_EMULATION) {
    out.frag_depth = in.unclipped_depth;
}
@if(NORMAL_PREPASS) {
    if (flags & pbr_types::STANDARD_MATERIAL_FLAGS_UNLIT_BIT) == 0u {
        let double_sided = (flags & pbr_types::STANDARD_MATERIAL_FLAGS_DOUBLE_SIDED_BIT) != 0u;
        let logical_front = pbr_functions::winding_corrected_front_facing(mesh[in.instance_index].flags, is_front);
        let world_normal = pbr_functions::prepare_world_normal(in.world_normal, double_sided, logical_front);
        var normal = world_normal;
@if(VERTEX_UVS) {
@if(VERTEX_TANGENTS) {
@if(STANDARD_MATERIAL_NORMAL_MAP) {
@if(STANDARD_MATERIAL_NORMAL_MAP_UV_B) {
        let uv = (uv_transform * vec3(in.uv_b, 1.0)).xy;
} @else {
        let uv = (uv_transform * vec3(in.uv, 1.0)).xy;
}
        var bias: SampleBias;
        bias.mip_bias = view.mip_bias;
@if(BINDLESS) {
        let normal_texture = bindless_textures_2d[material_indices[slot].normal_map_texture];
        let normal_sampler = bindless_samplers_filtering[material_indices[slot].normal_map_sampler];
} @else {
        let normal_texture = pbr_bindings::normal_map_texture;
        let normal_sampler = pbr_bindings::normal_map_sampler;
}
        let Nt = textureSampleBias(
            normal_texture,
            normal_sampler,
            uv,
            bias.mip_bias,
        ).rgb;
        let TBN = pbr_functions::calculate_tbn_mikktspace(normal, in.world_tangent);
        normal = pbr_functions::apply_normal_mapping(flags, TBN, double_sided, logical_front, Nt);
}
}
}
        out.normal = vec4(normal * 0.5 + vec3(0.5), 1.0);
    } else {
        out.normal = vec4(in.world_normal * 0.5 + vec3(0.5), 1.0);
    }
}
@if(MOTION_VECTOR_PREPASS) {
    out.motion_vector = pbr_prepass_functions::calculate_motion_vector(in.world_position, in.previous_world_position);
}
    return out;
}
} @else {
@fragment
fn fragment(in: prepass_io::VertexOutput) {
    leaf_alpha_discard(in);
}
}
