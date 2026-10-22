#import bevy_ui::ui_vertex_output::UiVertexOutput
#import bevy_render::color_operations::srgb_to_linear

@group(1) @binding(0) var<uniform> calendar: vec4<f32>;

@fragment
fn fragment(in: UiVertexOutput) -> @location(0) vec4<f32> {
    let p = in.uv - vec2<f32>(0.5);
    let radius = length(p);
    let aa = max(fwidth(radius), 0.001);
    // Spring west, summer north, autumn east, winter south, clockwise.
    let turn = fract((atan2(p.y, p.x) + calendar.y - 2.35619449) / 6.28318531 + 1.0);
    let delta = abs(fract(turn - calendar.x + 0.5) - 0.5);
    let needle = (1.0 - smoothstep(0.002, 0.005, delta))
        * smoothstep(0.345, 0.35 + aa, radius) * (1.0 - smoothstep(0.45, 0.456 + aa, radius));
    let inset = (1.0 - smoothstep(0.0012, 0.0027, delta))
        * smoothstep(0.352, 0.357 + aa, radius) * (1.0 - smoothstep(0.443, 0.448 + aa, radius));
    let silver = mix(vec3<f32>(0.25, 0.28, 0.31), vec3<f32>(0.86, 0.89, 0.92), inset);
    return vec4<f32>(srgb_to_linear(silver), needle);
}
