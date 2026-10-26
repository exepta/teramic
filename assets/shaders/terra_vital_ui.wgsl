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

fn falling_poison(p: vec2<f32>, seconds: f32) -> f32 {
    let pixel = floor(p / 3.0) * 3.0;
    var coverage = 0.0;
    for (var index = 0u; index < 5u; index += 1u) {
        let seed = status_hash(vec2<f32>(f32(index), 23.0));
        let phase = fract(seconds * (0.25 + seed * 0.10) + seed);
        let x = 195.0 + f32(index) * 61.0 + sin(seconds * 1.3 + seed * 6.28) * 3.0;
        let edge = 574.0 - abs(x - 318.0) * 0.70;
        let center = vec2<f32>(x, edge + phase * phase * 132.0);
        let size = vec2<f32>(4.5 + seed * 2.0, 7.0 + phase * 4.0);
        let local = abs((pixel - center) / size);
        let visible = smoothstep(0.04, 0.16, phase) * (1.0 - smoothstep(0.78, 1.0, phase));
        coverage = max(coverage, (1.0 - smoothstep(0.72, 1.0, max(local.x, local.y))) * visible);
    }
    return coverage;
}

fn stamina_rail_coverage(p: vec2<f32>) -> f32 {
    let q = abs(p - vec2<f32>(522.0, 476.0));
    let medallion = max(max(q.x - 104.0, q.y - 78.0), (q.x + q.y - 142.0) * 0.7071);
    let rail = max(max(586.0 - p.x, p.y - 552.0), max(413.0 - p.y, p.x - (1940.0 - abs(p.y - 476.0))));
    return 1.0 - smoothstep(-0.5, 0.75, min(medallion, rail));
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
        let rivulet_seed = status_hash(vec2<f32>(floor(p.x / 38.0), 11.0));
        let rivulet = 0.5 + 0.5 * sin(frame.timing.x * 2.1 + rivulet_seed * 6.28);
        let drip = p.x > 155.0 && p.x < 480.0 && p.y > drip_end - 25.0 && p.y < drip_end + 14.0 + 29.0 * rivulet && abs(drip_x) < 0.08 + rivulet_seed * 0.08;
        if on_heart || on_bar || drip {
            let green = mix(vec3<f32>(0.15, 0.40, 0.015), vec3<f32>(0.67, 0.99, 0.06), grain);
            let pulse = 0.84 + sin(frame.timing.x * 2.0 + p.x / 47.0) * 0.06;
            color = vec4<f32>(mix(color.rgb, srgb_to_linear(green) * pulse, select(0.78, 1.0, drip)), 1.0);
        }
        let falling = falling_poison(p, frame.timing.x);
        if falling > 0.0 {
            let green = srgb_to_linear(vec3<f32>(0.48, 0.83, 0.025));
            color = vec4<f32>(mix(color.rgb, green, falling), max(color.a, falling));
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
    if frame.timing.w > 0.0 {
        // Reuse the complete stamina rail, including its recessed gold medallion.
        let p = vec2<f32>(430.0, 390.0) + in.uv * vec2<f32>(1540.0, 176.0);
        var armor = textureSample(chrome, chrome_sampler, p / extent);
        if p.y < 472.0 {
            // The rail shares its source image with the heart's lower-right edge.
            armor.a *= smoothstep(-1.0, 1.0, p.x - (810.0 - p.y * 0.78));
        }
        let left = 608.0 + abs(p.y - 476.0) * 0.66;
        let right = 1850.0 - max(p.y - 483.0, 0.0) * 0.85;
        let inside = p.x >= left && p.x <= right && p.y >= 444.0 && p.y <= 508.0;
        if inside && max(armor.r, max(armor.g, armor.b)) < 0.095 && p.x <= mix(608.0, 1850.0, values.x) && values.x > 0.0 {
            let t = clamp((p.y - 444.0) / 64.0, 0.0, 1.0);
            let silver = mix(vec3<f32>(0.98, 0.99, 1.0), vec3<f32>(0.49, 0.57, 0.66), t);
            armor = vec4<f32>(srgb_to_linear(silver), 1.0);
        }
        // Remove only the lightning pigment, preserving the original gold border.
        if p.x > 476.0 && p.x < 575.0 && p.y > 425.0 && p.y < 519.0 && armor.r > armor.b * 1.8 && armor.g > armor.b * 1.5 {
            armor = vec4<f32>(srgb_to_linear(vec3<f32>(0.12, 0.11, 0.075)), 1.0);
        }
        let q = (p - vec2<f32>(522.0, 475.0)) / vec2<f32>(37.0, 44.0);
        let width = select(0.84, 0.84 * (1.0 - (q.y - 0.15) / 0.85), q.y > 0.15);
        if q.y >= -0.82 && q.y <= 1.0 && abs(q.x) <= width {
            let rim = abs(q.x) > width - 0.10 || q.y < -0.70;
            let light = select(vec3<f32>(0.72, 0.81, 0.89), vec3<f32>(0.95, 0.98, 1.0), q.x < -0.08);
            let silver = select(light, vec3<f32>(0.32, 0.40, 0.47), rim);
            armor = vec4<f32>(srgb_to_linear(silver), 1.0);
        }
        // The atlas also contains the HP rail above this crop. Exclude it entirely.
        armor.a *= stamina_rail_coverage(p);
        return armor;
    }
    // UiMaterial has its own full-node UVs, independent from ImageNode.rect.
    let uv = in.uv;
    let p = uv * extent;
    var color = textureSample(chrome, chrome_sampler, uv);
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
        let health = mix(vec3<f32>(1.0, 0.16, 0.21), vec3<f32>(0.66, 0.0, 0.09), t);
        let armor = mix(vec3<f32>(0.96, 0.98, 1.0), vec3<f32>(0.54, 0.57, 0.64), t);
        let base = select(health, armor, frame.timing.w > 0.0);
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
