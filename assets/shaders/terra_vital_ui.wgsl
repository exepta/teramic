#import bevy_ui::ui_vertex_output::UiVertexOutput
#import bevy_render::color_operations::srgb_to_linear

struct VitalUniforms {
    values: vec4<f32>,
    effects: vec4<f32>,
    timing: vec4<f32>,
}
@group(1) @binding(0) var<uniform> frame: VitalUniforms;
@group(1) @binding(1) var chrome: texture_2d<f32>;
@group(1) @binding(2) var chrome_sampler: sampler;

fn is_heart(color: vec4<f32>) -> bool {
    return color.r > 0.10 && color.r > color.g * 2.5 && color.r > color.b * 2.0;
}

fn status_hash(p: vec2<f32>) -> f32 {
    return fract(sin(dot(p, vec2<f32>(127.1, 311.7))) * 43758.5453);
}

fn surface_status(p: vec2<f32>, original: vec4<f32>, health_inside: bool) -> vec4<f32> {
    if all(frame.effects == vec4<f32>(0.0)) && frame.timing.z == 0.0 {
        return original;
    }
    var color = original;
    let hp = (p.x - 544.0) / 1392.0;
    let heart = is_heart(original) && p.x > 130.0 && p.x < 495.0 && p.y > 190.0 && p.y < 550.0;
    let cell = floor(p / 20.0);
    let grain = status_hash(cell);
    let side = min(hp, 1.0 - hp);
    let poison = frame.effects.x;
    if poison > 0.0 {
        let on_heart = heart && (p.y > 354.0 + sin(p.x / 24.0) * 28.0 || abs(p.x - 318.0) > 125.0);
        let on_bar = health_inside && side < 0.14 + grain * 0.075;
        let drip_x = fract(p.x / 38.0) - 0.5;
        let drip_end = 566.0 - abs(p.x - 318.0) * 0.84;
        let drip = p.x > 155.0 && p.x < 480.0 && p.y > drip_end - 25.0 && p.y < drip_end + 35.0 * (0.5 + grain) && abs(drip_x) < 0.08 + grain * 0.08;
        if on_heart || on_bar || drip {
            let green = mix(vec3<f32>(0.15, 0.40, 0.015), vec3<f32>(0.67, 0.99, 0.06), grain);
            let pulse = 0.84 + sin(frame.timing.x * 2.0 + p.x / 47.0) * 0.06;
            color = vec4<f32>(mix(color.rgb, srgb_to_linear(green) * pulse, select(0.78, 1.0, drip)), 1.0);
        }
    }
    if frame.effects.y > 0.0 {
        let edge = abs(p.x - 318.0) / 170.0;
        let on_heart = heart && (edge > 0.55 - grain * 0.13 || p.y > 450.0 + grain * 20.0 || p.y < 230.0 + grain * 35.0);
        let on_bar = health_inside && side < 0.13 + grain * 0.055;
        let tooth = 1.0 - abs(fract(p.x / 42.0) * 2.0 - 1.0);
        let edge_y = 548.0 - abs(p.x - 318.0) * 0.9;
        let icicle = p.x > 160.0 && p.x < 475.0 && p.y > edge_y - 18.0 && p.y < edge_y + tooth * 45.0;
        let crown = p.x > 145.0 && p.x < 480.0 && abs(p.x - 318.0) > 45.0 && p.y > 205.0 - tooth * 48.0 && p.y < 225.0;
        if on_heart || on_bar || icicle || crown {
            let facet = select(0.0, 0.26, fract(p.x / 20.0) + fract(p.y / 20.0) > 1.0);
            let ice = mix(vec3<f32>(0.10, 0.46, 0.88), vec3<f32>(0.78, 0.97, 1.0), min(grain + facet, 1.0));
            color = vec4<f32>(srgb_to_linear(ice), 1.0);
        }
    }
    if frame.effects.z > 0.0 && (heart || health_inside && side < 0.08) {
        let fire = mix(vec3<f32>(0.95, 0.13, 0.015), vec3<f32>(1.0, 0.72, 0.08), grain);
        color = vec4<f32>(mix(color.rgb, srgb_to_linear(fire), 0.45 + sin(frame.timing.x * 5.0 + p.x / 24.0) * 0.10), color.a);
    }
    if frame.effects.w > 0.0 && heart && grain > 0.62 {
        color = vec4<f32>(color.rgb * vec3<f32>(0.60, 0.27, 0.30), color.a);
    }
    if frame.timing.z > 0.0 && heart && grain > 0.58 {
        color = vec4<f32>(mix(color.rgb, srgb_to_linear(vec3<f32>(0.38, 0.22, 0.48)), 0.55), color.a);
    }
    return color;
}

@fragment
fn fragment(in: UiVertexOutput) -> @location(0) vec4<f32> {
    let values = frame.values;
    let extent = vec2<f32>(2106.0, 747.0);
    let p = in.uv * extent;
    var color = textureSample(chrome, chrome_sampler, in.uv);
    // Only the red faceted heart moves. Its medallion and both rails stay fixed.
    let heart_center = vec2<f32>(318.0, 365.0);
    let heart_uv = ((p - heart_center) / values.z + heart_center) / extent;
    let heart = textureSample(chrome, chrome_sampler, heart_uv);
    if p.x > 130.0 && p.x < 495.0 && p.y > 190.0 && p.y < 540.0 && is_heart(heart) {
        color = heart;
    }
    // The generated artwork has pointed channels, not rectangular end caps.
    // Keep its gold rim intact; the native TiltUI fill supplies the eased fraction.
    let health_right = 1936.0 - abs(p.y - 318.0) * 0.79;
    let health_inside = p.x >= 544.0 && p.x <= health_right && p.y >= 255.0 && p.y <= 381.0;
    let stamina_left = 608.0 + abs(p.y - 476.0) * 0.66;
    let stamina_right = 1850.0 - max(p.y - 483.0, 0.0) * 0.85;
    let stamina_inside = p.x >= stamina_left && p.x <= stamina_right && p.y >= 444.0 && p.y <= 508.0;
    let dark_channel = max(color.r, max(color.g, color.b)) < 0.095;
    if health_inside && dark_channel && p.x <= mix(544.0, 1936.0, values.x) && values.x > 0.0 {
        let t = clamp((p.y - 255.0) / 126.0, 0.0, 1.0);
        let shine = exp(-t * 13.0) * 0.4;
        let base = mix(vec3<f32>(1.0, 0.16, 0.21), vec3<f32>(0.66, 0.0, 0.09), t);
        color = vec4<f32>(srgb_to_linear(min(base + vec3<f32>(shine), vec3<f32>(1.0))), 1.0);
    }
    if stamina_inside && dark_channel && p.x <= mix(608.0, 1850.0, values.y) && values.y > 0.0 {
        let t = clamp((p.y - 444.0) / 64.0, 0.0, 1.0);
        let stamina = mix(vec3<f32>(1.0, 0.94, 0.56), vec3<f32>(0.92, 0.55, 0.0), t);
        let oxygen = mix(vec3<f32>(0.65, 0.95, 1.0), vec3<f32>(0.12, 0.55, 0.78), t);
        let base = mix(stamina, oxygen, values.w);
        color = vec4<f32>(srgb_to_linear(base), 1.0);
    }
    return surface_status(p, color, health_inside);
}
