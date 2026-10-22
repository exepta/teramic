#import bevy_core_pipeline::fullscreen_vertex_shader::FullscreenVertexOutput

struct CaveSettings {
    view_from_clip: mat4x4<f32>,
    parameters: vec4<f32>,
    world_from_view: mat4x4<f32>,
    underwater: vec4<f32>,
    scattering: vec4<f32>,
    ink: vec4<f32>,
    smoke: vec4<f32>,
}
@group(0) @binding(0) var scene_color: texture_2d<f32>;
@group(0) @binding(1) var scene_sampler: sampler;
@group(0) @binding(2) var<uniform> cave: CaveSettings;
#ifdef MULTISAMPLED
@group(0) @binding(3) var scene_depth: texture_depth_multisampled_2d;
#else
@group(0) @binding(3) var scene_depth: texture_depth_2d;
#endif

fn ink_hash(n: f32) -> f32 { return fract(sin(n * 37.17 + cave.ink.z * 0.73) * 43758.55); }

fn ink_mask(uv: vec2<f32>) -> f32 {
    let lane = floor(uv.x * 11.0);
    let variation = ink_hash(lane + 4.0);
    let sway = sin(uv.y * 6.0 + lane * 1.7 + cave.ink.y * 0.9) * 0.12;
    let x = fract(uv.x * 11.0) - 0.5 + sway;
    let width = 0.15 + variation * 0.22;
    let streak = 1.0 - smoothstep(width * 0.55, width, abs(x));
    let tip = 0.30 + variation * 0.55 + cave.ink.y * (0.14 + variation * 0.07);
    let tail = tip - 0.58 - variation * 0.32;
    let falling = smoothstep(tail - 0.16, tail + 0.13, uv.y) * (1.0 - smoothstep(tip, tip + 0.11, uv.y));
    let center = vec2<f32>(0.5, 0.44 + cave.ink.y * 0.16);
    let blot = (1.0 - smoothstep(0.20, 0.65, length((uv - center) * vec2<f32>(0.90, 1.15)))) * exp(-cave.ink.y * 0.85);
    return clamp(max(streak * falling * 0.86, blot), 0.0, 1.0);
}

@fragment
fn fragment(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    let water = cave.underwater.x;
    let time = cave.underwater.z;
    // A very small coherent ripple; no repeated blur, chromatic fringe or extra texture pass.
    let ripple = vec2<f32>(sin(in.uv.y * 18.0 + time * 0.65), cos(in.uv.x * 16.0 - time * 0.55));
    let uv = clamp(in.uv + ripple * (water * 0.00055), vec2<f32>(0.001), vec2<f32>(0.999));
    let color = textureSample(scene_color, scene_sampler, uv);
    let size = vec2<i32>(textureDimensions(scene_depth));
    let pixel = clamp(vec2<i32>(in.uv * vec2<f32>(size)), vec2<i32>(0), size - 1);
    let depth = textureLoad(scene_depth, pixel, 0);
    let clip = vec4<f32>(in.uv * vec2<f32>(2.0, -2.0) + vec2<f32>(-1.0, 1.0), depth, 1.0);
    let view = cave.view_from_clip * clip;
    let view_position = view.xyz / max(abs(view.w), 0.000001);
    let world = (cave.world_from_view * vec4<f32>(view_position, 1.0)).xyz;
    let distance = length(view_position);
    let edge_x = dpdx(world);
    let edge_y = dpdy(world);
    let normal = normalize(cross(edge_x, edge_y) + vec3<f32>(0.0, 0.000001, 0.0));
    // Never lift black to blue: unlit tunnels stay dark and emitters keep their hue.
    // The density is zero in low/narrow caves and fades in only in large chambers.
    let haze = (1.0 - exp(-max(distance - 18.0, 0.0) * cave.parameters.y)) * 0.16;
    var result = color.rgb;
    // Infinite reversed-Z: untouched sky/portals must not get cave haze painted over them.
    if depth > 0.000001 {
        result = mix(result, vec3<f32>(0.006, 0.0065, 0.007), haze * cave.parameters.x);
        if water > 0.001 {
            // Wavelength-dependent absorption keeps nearby reef pigments clear.
            let absorption = exp(-vec3<f32>(0.014, 0.0035, 0.0020) * min(distance, 64.0) * water);
            result *= absorption;
            let p = world.xz * 1.35;
            let warp = vec2<f32>(sin(p.y * 0.72 + time * 0.29), cos(p.x * 0.79 - time * 0.26));
            let network = abs(sin(p.x + warp.x + time * 0.16) + sin(p.y + warp.y - time * 0.19));
            let lines = 1.0 - smoothstep(0.035, 0.18, network);
            let sunlight = clamp(cave.scattering.w, 0.0, 1.0);
            let shallow = exp(-cave.underwater.y * 0.065);
            let near = 1.0 - smoothstep(20.0, 42.0, distance);
            let caustic = lines * abs(normal.y) * sunlight * shallow * near * water * 0.055;
            result += color.rgb * caustic;
        }
    } else if water > 0.001 {
        // Transmissive water does not write the opaque prepass. Its distant
        // backfaces can therefore return near-black sky outside the screen-space
        // reflection buffer. Close that missing medium, never opaque cave walls.
        let direction = normalize((cave.world_from_view * vec4<f32>(view_position, 0.0)).xyz);
        let upper = smoothstep(-0.30, 0.90, direction.y);
        let depth_light = 0.35 + 0.65 * exp(-min(cave.underwater.y, 96.0) * 0.028);
        let marine = cave.scattering.rgb * depth_light * (0.82 + upper * 0.52);
        result = mix(result, marine, water * 0.86);
    }
    // Local furnace smoke reuses this depth pass. It is a smooth participating
    // medium around the viewer, never visible logical cell faces or a flat overlay.
    if cave.smoke.x > 0.001 {
        let path = min(distance, 16.0);
        let drift = sin(in.uv.x * 9.0 + in.uv.y * 5.0 + cave.smoke.y * 0.29)
            * sin(in.uv.y * 7.0 - cave.smoke.y * 0.23);
        let extinction = cave.smoke.x * (0.22 + drift * 0.018);
        let opacity = 1.0 - exp(-path * extinction);
        // Smoke is lit by the room, not by the individual surface behind it.
        // Four fixed, cache-friendly taps estimate that light only while smoke
        // is active. This reduces contrast across lit/shadowed surfaces without
        // giving an entirely unlit room an artificial luminous fog layer.
        let room_light = (
            textureSampleLevel(scene_color, scene_sampler, vec2<f32>(0.25, 0.30), 0.0).rgb
            + textureSampleLevel(scene_color, scene_sampler, vec2<f32>(0.75, 0.30), 0.0).rgb
            + textureSampleLevel(scene_color, scene_sampler, vec2<f32>(0.25, 0.70), 0.0).rgb
            + textureSampleLevel(scene_color, scene_sampler, vec2<f32>(0.75, 0.70), 0.0).rgb
        ) * 0.25;
        let illumination = smoothstep(
            0.001, 0.060, dot(room_light, vec3<f32>(0.2126, 0.7152, 0.0722))
        );
        let scattered = vec3<f32>(0.24, 0.24, 0.235) * illumination;
        result = mix(result, scattered, opacity * 0.95);
    }
    // Ink is a gameplay obstruction on every quality level, even when optics are disabled.
    if cave.ink.x > 0.001 {
        let opacity = ink_mask(in.uv) * cave.ink.x * 0.84;
        result = mix(result, vec3<f32>(0.004, 0.006, 0.013), opacity);
    }
    return vec4<f32>(result, color.a);
}
