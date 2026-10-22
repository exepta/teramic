#import bevy_pbr::{
    forward_io::{Vertex, FragmentOutput},
    mesh_view_bindings::{globals, view},
    view_transformations::position_world_to_clip,
}

@group(#{MATERIAL_BIND_GROUP}) @binding(100) var<uniform> conditions: vec4<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(101) var<uniform> center: vec4<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(102) var<uniform> wind: vec4<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(103) var<uniform> sky: vec4<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(104) var<uniform> snow_profile: vec4<f32>;

struct WeatherVertex {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) data: vec4<f32>,
    @location(2) world_position: vec3<f32>,
}

@vertex
fn vertex(input: Vertex) -> WeatherVertex {
    var out: WeatherVertex;
    let seed = input.color.x;
    let kind = input.uv_b.y;
    let roof = input.uv_b.x;
    let snow = kind > 0.5 && kind < 1.5;
    let splash = kind > 1.5 && kind < 2.5;
    let time = globals.time;
    // Recycle at the volume bounds, but keep each fall phase in world space.
    // Jumping/flying must not carry the rain and snow vertically with the camera.
    // Extreme whiteouts concentrate a bounded mesh around eye height instead
    // of allocating hundreds of thousands of overlapping transparent quads.
    // A baseline layer still falls all the way to the ground; only extra
    // whiteout layers use the concentrated volume.
    let concentration = select(1.0, max(snow_profile.w, 1.0), snow && input.color.z >= sky.z);
    let lower = wind.z / concentration;
    let upper = wind.w / concentration;
    let span = max(upper - lower, 1.0);
    // Independent fall speeds and eddies prevent parallel, synchronised rows.
    // Concentrated whiteout layers retain physical speed rather than slowing
    // down as their wrapping volume becomes shallower.
    let snow_speed = mix(2.7, 6.4, input.color.w);
    let fall_rate = select(0.43, snow_speed / span, snow);
    let cycle = fract(seed - time * fall_rate - center.y / span);
    var p = vec3<f32>(input.position.x, center.y + mix(lower, upper, cycle), input.position.z);
    let right = normalize(view.world_from_view[0].xyz);
    let up = normalize(view.world_from_view[1].xyz);
    // Stay inside the sampled roof column, even in strong wind beside buildings.
    let drift_time = time * mix(0.48, 1.35, input.color.w);
    let eddy = vec2<f32>(sin(seed * 32.0 + drift_time),
        cos(input.color.w * 37.0 + drift_time * 0.73));
    let wind_offset = clamp((wind.xy * 0.035 + eddy * select(0.035, 0.12, snow))
        * select(1.0, 1.5, snow), vec2<f32>(-0.18), vec2<f32>(0.18));
    p.x += wind_offset.x;
    p.z += wind_offset.y;
    var phase = cycle;
    if splash {
        phase = fract(seed + time * (1.7 + input.color.y));
        // A shallow underside indentation remains visible through the surface
        // from underwater; the above-water rings stay just above the impact.
        let storm = clamp((length(wind.xy) - 2.2) / 4.4, 0.0, 1.0);
        // Keep the underside ring below the trough as well as the still-water
        // plane; otherwise storm displacement can depth-occlude the impact.
        let underside = -0.065 - storm * 0.11 * sky.w;
        p.y = roof + select(underside, 0.008, conditions.w > 0.5);
        let radius = 0.035 + phase * 0.22;
        p += vec3<f32>(input.uv.x * radius, 0.0, input.uv.y * radius);
    } else if snow {
        let rotation = seed * 6.283185 + sin(drift_time * 0.81 + input.color.w * 21.0) * 0.8;
        let rotated_uv = vec2<f32>(input.uv.x * cos(rotation) - input.uv.y * sin(rotation),
            input.uv.x * sin(rotation) + input.uv.y * cos(rotation));
        // Close flakes must not become giant decorative stars. Cap angular
        // size while retaining real depth/variation and all density layers.
        let size = min((0.022 + input.color.y * 0.024) * min(snow_profile.y, 3.5),
            max(length(p - center.xyz), 0.3) * 0.0045);
        // This flake uses the roof of its actual jittered one-metre cell.
        // Keep its complete rotating card and drift inside that sampled cell.
        let column = floor(input.position.xz);
        let margin = size * 1.415 + 0.005;
        let contained = clamp(p.xz, column + vec2<f32>(margin),
            column + vec2<f32>(1.0 - margin));
        p = vec3<f32>(contained.x, p.y, contained.y);
        p += right * rotated_uv.x * size + up * rotated_uv.y * size;
    } else {
        let fall = normalize(vec3<f32>(wind.x * 0.07, -1.0, wind.y * 0.07));
        p += right * input.uv.x * (0.014 + input.color.y * 0.008);
        p += fall * input.uv.y * (0.26 + input.color.y * 0.23);
    }
    let strength = select(conditions.x, conditions.y, snow);
    // Spatially stable thinning avoids blinking when intensity changes smoothly.
    // Never thin with the fall-phase seed: that selects one descending sheet
    // followed by a gap on every wrap, especially during light rain.
    let density_seed = input.normal.x;
    let density = sqrt(max(strength, 0.0));
    var visible = select(smoothstep(density_seed - 0.025, density_seed + 0.025, density), 0.0, strength < 0.001);
    if snow && input.color.z >= floor(snow_profile.x) {
        visible *= smoothstep(density_seed - 0.025, density_seed + 0.025, fract(snow_profile.x));
    }
    out.position = position_world_to_clip(p);
    out.uv = input.uv;
    out.data = vec4<f32>(kind, roof, visible, phase);
    out.world_position = p;
    return out;
}

@fragment
fn fragment(input: WeatherVertex) -> FragmentOutput {
    let p = input.world_position;
    if (p.y < input.data.y && input.data.x < 1.5) || input.data.z < 0.001 { discard; }
    let distance = length(p.xz - center.xz);
    let fade = (1.0 - smoothstep(center.w * 0.65, center.w, distance))
        * smoothstep(0.18, 0.8, length(p - center.xyz))
        * select(conditions.w, 1.0, input.data.x > 1.5);
    let kind = input.data.x;
    var alpha = 0.0;
    var color = vec3<f32>(0.65, 0.76, 0.81);
    if kind > 1.5 {
        let radius = length(input.uv);
        let ring = exp(-pow((radius - 0.68) * 14.0, 2.0));
        alpha = ring * (1.0 - input.data.w) * 0.67;
        color = vec3<f32>(0.70, 0.81, 0.87);
    } else if kind > 0.5 {
        // Six soft, pixel-styled branches rather than opaque square snow cards.
        let uv = abs(input.uv);
        let core = exp(-dot(uv,uv) * 4.5);
        let arms = max(exp(-uv.x * 22.0), exp(-abs(uv.x - uv.y * 1.73) * 15.0));
        alpha = max(core * 0.52, arms * (1.0 - smoothstep(0.6,1.0,length(uv)))) * 0.88;
        alpha *= smoothstep(0.0, 0.08, input.data.w)
            * (1.0 - smoothstep(0.92, 1.0, input.data.w));
        color = vec3<f32>(0.86,0.90,0.94);
    } else {
        alpha = exp(-input.uv.x * input.uv.x * 5.0)
            * (1.0 - smoothstep(0.45,1.0,abs(input.uv.y))) * (0.48 + conditions.x * 0.23);
    }
    alpha *= fade * input.data.z;
    if alpha < 0.006 { discard; }
    var out: FragmentOutput;
    out.color = vec4<f32>(color * max(conditions.z,0.24), alpha);
    return out;
}
