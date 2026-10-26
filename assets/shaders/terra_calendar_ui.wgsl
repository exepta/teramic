#import bevy_ui::ui_vertex_output::UiVertexOutput
#import bevy_render::color_operations::srgb_to_linear

@group(1) @binding(0) var<uniform> calendar: vec4<f32>;

@fragment
fn fragment(in: UiVertexOutput) -> @location(0) vec4<f32> {
    let p = in.uv - vec2<f32>(0.5);
    // Spring west, summer north, autumn east, winter south, clockwise.
    let angle = calendar.x * 6.28318531 + 2.35619449 - calendar.y;
    let direction = vec2<f32>(cos(angle), sin(angle));
    let radial = dot(p, direction);
    let lateral = abs(dot(p, vec2<f32>(-direction.y, direction.x)));
    // A solid inward-facing arrowhead stays legible over the detailed seasons.
    let edge = max(max(0.345 - radial, radial - 0.443),
        lateral - (radial - 0.345) * (0.026 / 0.098));
    let aa = max(fwidth(edge), 0.0008);
    let alpha = 1.0 - smoothstep(-aa, aa, edge);
    let inner_edge = max(max(0.361 - radial, radial - 0.435),
        lateral - (radial - 0.361) * (0.017 / 0.074));
    let inset = 1.0 - smoothstep(-aa, aa, inner_edge);
    let highlight = clamp((0.443 - radial) / 0.098, 0.0, 1.0);
    let silver = mix(vec3<f32>(0.70, 0.74, 0.78), vec3<f32>(0.94, 0.96, 0.98), highlight);
    let color = mix(vec3<f32>(0.20, 0.23, 0.27), silver, inset);
    return vec4<f32>(srgb_to_linear(color), alpha);
}
