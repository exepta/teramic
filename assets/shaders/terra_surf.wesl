import bevy_pbr::render::mesh_view_bindings::globals;
import bevy_pbr::render::mesh_view_bindings::view;
import bevy_pbr::render::view_transformations::position_world_to_clip;

@group(constants::MATERIAL_BIND_GROUP) @binding(0) var<uniform> lighting: vec4<f32>;

struct In {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
}
struct Out {
    @builtin(position) position: vec4<f32>,
    @location(0) world: vec3<f32>,
    @location(1) shore_phase: vec2<f32>,
}

fn foam_hash(position: vec2<f32>) -> f32 {
    let v = fract(position.xyx * 0.1031);
    let q = v + vec3<f32>(dot(v, v.yzx + vec3<f32>(33.33)));
    return fract((q.x + q.y) * q.z);
}

fn foam_noise(position: vec2<f32>) -> f32 {
    let cell = floor(position);
    let f = fract(position);
    let weight = f * f * (vec2<f32>(3.0) - 2.0 * f);
    let bottom = mix(foam_hash(cell), foam_hash(cell + vec2<f32>(1.0, 0.0)), weight.x);
    let top = mix(foam_hash(cell + vec2<f32>(0.0, 1.0)), foam_hash(cell + vec2<f32>(1.0)), weight.x);
    return mix(bottom, top, weight.y);
}

@vertex
fn vertex(v: In) -> Out {
    var out: Out;
    let d = max(0.0, v.uv.y);
    let shore = v.position.xz - v.normal.xz * d * 3.8;
    let beat = globals.time * 0.235 + dot(shore, vec2<f32>(0.018, 0.012))
        + 0.10 * sin(dot(shore, vec2<f32>(0.19, -0.21)));
    let phase = fract(beat);
    let front = 1.15 - phase * 1.86;
    let ridge = exp(-55.0 * (d - front) * (d - front));
    let impact = smoothstep(0.49, 0.64, phase) * (1.0 - smoothstep(0.67, 0.95, phase));
    var world = v.position;
    world.y += lighting.y * (ridge * 0.28 + impact * 0.30 * (1.0 - d));
    world.y -= max(-v.uv.y, 0.0) * 0.79 * (1.0 - impact);
    out.world = world;
    out.shore_phase = vec2<f32>(v.uv.y, beat);
    out.position = position_world_to_clip(world);
    return out;
}

@fragment
fn fragment(i: Out) -> @location(0) vec4<f32> {
    let d = i.shore_phase.x;
    let phase = fract(i.shore_phase.y);
    let point = i.world.xz * 3.7 + vec2<f32>(i.world.y * 2.8, globals.time * 0.36);
    let grain = foam_noise(point);
    let holes = foam_noise(point * 2.2 + vec2<f32>(9.0));
    let front = 1.15 - phase * 1.86 + (grain - 0.5) * 0.16;
    let offset = max(d, 0.0) - front;
    let crest = exp(-84.0 * offset * offset);
    let crash = smoothstep(0.46, 0.65, phase) * (1.0 - smoothstep(0.70, 0.99, phase));
    let climb = crash * (0.64 + 0.38 * grain);
    var foam = (crest * (0.47 + 0.45 * grain) + crash * exp(-max(d, 0.0) * 4.3))
        * (1.0 - smoothstep(0.80, 1.0, d));
    if d < 0.0 {
        foam = crash * (1.0 - smoothstep(climb - 0.27, climb + 0.05, -d));
    }
    let lace = smoothstep(0.16, 0.65, grain + holes * 0.25);
    let reach = 1.0 - smoothstep(48.0, 65.0, distance(view.world_position.xyz, i.world));
    let opacity = clamp(foam * lace * reach * lighting.y, 0.0, 0.9);
    if opacity < 0.02 { discard; }
    let color = mix(vec3<f32>(0.28, 0.55, 0.57), vec3<f32>(0.94, 0.99, 1.0), clamp(foam, 0.0, 1.0));
    return vec4<f32>(color * lighting.x, opacity);
}
