#import bevy_pbr::{forward_io::VertexOutput, mesh_view_bindings::globals}

@group(#{MATERIAL_BIND_GROUP}) @binding(100) var<uniform> animation: vec4<f32>;

fn smoke_hash(p: vec2<f32>) -> f32 {
    let q = fract(p * vec2<f32>(0.1031, 0.11369));
    return fract((q.x + q.y) * (q.x + q.y + 19.19));
}

fn smoke_noise(p: vec2<f32>) -> f32 {
    let cell = floor(p);
    let local = fract(p);
    let blend = local * local * (3.0 - 2.0 * local);
    return mix(
        mix(smoke_hash(cell), smoke_hash(cell + vec2<f32>(1.0, 0.0)), blend.x),
        mix(smoke_hash(cell + vec2<f32>(0.0, 1.0)), smoke_hash(cell + 1.0), blend.x),
        blend.y,
    );
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let p = in.uv * 2.0 - 1.0;
    let time = globals.time + animation.x;
    let drift = in.world_position.xz * 0.41 + vec2<f32>(time * 0.14, -time * 0.19);
    let curl = smoke_noise(p * 2.7 + drift);
    let detail = smoke_noise(p * 6.5 - drift * 0.6);
    let radius = length(p + vec2<f32>(curl - 0.5, detail - 0.5) * 0.16);
    let edge = 1.0 - smoothstep(0.20, 1.0, radius);
    let density = edge * (0.38 + curl * 0.42 + detail * 0.20);
    let alpha = density * in.color.a;
    if alpha < 0.002 { discard; }
    return vec4<f32>(in.color.rgb * (0.82 + curl * 0.18), alpha);
}
