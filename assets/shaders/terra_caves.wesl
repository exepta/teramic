import bevy_core_pipeline::fullscreen_vertex_shader::FullscreenVertexOutput;


struct CaveSettings {
    view_from_clip: mat4x4<f32>,
    parameters: vec4<f32>,
    world_from_view: mat4x4<f32>,
    underwater: vec4<f32>,
    waterline: vec4<f32>,
    scattering: vec4<f32>,
    ink: vec4<f32>,
    smoke: vec4<f32>,
    salt_window: vec4<f32>,
    sun_direction: vec4<f32>,
    beam_depths: vec4<f32>,
    beam_radii: vec4<f32>,
    beam_cross_radii: vec4<f32>,
    gallery_dust: f32,
    lava_depth: f32,
    sediment: f32,
}
@group(0) @binding(0) var scene_color: texture_2d<f32>;
@group(0) @binding(1) var scene_sampler: sampler;
@group(0) @binding(2) var<uniform> cave: CaveSettings;
@if(MULTISAMPLED) {
@group(0) @binding(3) var scene_depth: texture_depth_multisampled_2d;
} @else {
@group(0) @binding(3) var scene_depth: texture_depth_2d;
}

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

// Integrate a soft-edged shaft over the portion of the camera ray that
// intersects it. Both the aperture plane and the opaque scene depth clip it.
fn shaft_length(start: vec2<f32>, motion: vec2<f32>, radius: f32, near: f32, far: f32) -> f32 {
    let speed2 = max(dot(motion, motion), 0.00001);
    let center = clamp(-dot(start, motion) / speed2, near, far);
    let closest = start + center * motion;
    let width2 = radius * radius;
    let span = sqrt(max(width2 - dot(closest, closest), 0.0) / speed2);
    let length_in_air = max(0.0, min(center + span, far) - max(center - span, near));
    return (1.0 - exp(-0.055 * length_in_air)) * (1.0 - smoothstep(0.12, 1.0, length(closest) / radius));
}

@fragment
fn fragment(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    let water = cave.underwater.x;
    // Optical absorption belongs to the submerged camera, never a second
    // synthetic flat surface over the reconstructed liquid geometry.
    let color = textureSample(scene_color, scene_sampler, in.uv);
    let size = vec2<i32>(textureDimensions(scene_depth));
    let pixel = clamp(vec2<i32>(in.uv * vec2<f32>(size)), vec2<i32>(0), size - 1);
    let depth = textureLoad(scene_depth, pixel, 0);
    let clip = vec4<f32>(in.uv * vec2<f32>(2.0, -2.0) + vec2<f32>(-1.0, 1.0), depth, 1.0);
    let view = cave.view_from_clip * clip;
    let view_position = view.xyz / max(abs(view.w), 0.000001);
    let world = (cave.world_from_view * vec4<f32>(view_position, 1.0)).xyz;
    let distance = length(view_position);
    let origin = cave.world_from_view[3].xyz;
    let ray = (cave.world_from_view * vec4<f32>(view_position / max(distance, 0.0001), 0.0)).xyz;
    // An upward ray exits the water even if the opaque prepass hits a cliff beyond it.
    let exit_distance = select(80.0, max(cave.underwater.y, 0.0) / max(ray.y, 0.001), ray.y > 0.001);
    let wet_distance = min(min(distance, 80.0), exit_distance);
    let wet = water;
    // Never lift black to blue: unlit tunnels stay dark and emitters keep their hue.
    // The density is zero in low/narrow caves and fades in only in large chambers.
    let haze = (1.0 - exp(-max(distance - 18.0, 0.0) * cave.parameters.y)) * 0.16;
    var result = color.rgb;
    // Infinite reversed-Z: untouched sky/portals must not get cave haze painted over them.
    if depth > 0.000001 {
        result = mix(result, vec3<f32>(0.006, 0.0065, 0.007), haze * cave.parameters.x);
        // Integrate scattered skylight only along the visible part of a view ray.
        // Projection onto the aperture follows the sun and widens with angular diffusion.
        if cave.parameters.w > 0.001 && cave.salt_window.w > 0.0 {

            let altitude = cave.sun_direction.y;
            let travel = min(distance, 100.0);
            let height = cave.salt_window.y - origin.y;
            var near = 0.0;
            var far = travel;
            if ray.y > 0.0001 { far = min(far, height / ray.y); }
            if ray.y < -0.0001 { near = max(near, height / ray.y); }
            if far > near && altitude > 0.01 && (height > 0.0 || ray.y < -0.0001) {
                let slope = cave.sun_direction.xz / max(altitude, 0.06);
                let start = origin.xz + height * slope - cave.salt_window.xz;
                let motion = ray.xz - ray.y * slope;
                let r = cave.salt_window.w;
                let offsets = array<vec2<f32>, 4>(vec2<f32>(0., 0.), vec2<f32>(0.60, 0.),
                    vec2<f32>(-0.30, 0.52), vec2<f32>(-0.30, -0.52));
                var shaft = 0.0;
                for (var i = 0u; i < 4u; i += 1u) {
                    let depth_limit = cave.beam_depths[i];
                    if depth_limit <= 0.0 { continue; }
                    let floor_height = height - depth_limit;
                    var begin = near;
                    var end = far;
                    if ray.y > 0.0001 { begin = max(begin, floor_height / ray.y); }
                    else if ray.y < -0.0001 { end = min(end, floor_height / ray.y); }
                    else if floor_height > 0.0 { continue; }
                    if end > begin {
                        let center = floor(cave.salt_window.xz + offsets[i] * r) + vec2<f32>(0.5);
                        let axes = max(vec2<f32>(cave.beam_radii[i], cave.beam_cross_radii[i]), vec2<f32>(0.05));
                        let scattered = shaft_length((start + cave.salt_window.xz - center) / axes, motion / axes, 1.0, begin, end);
                        shaft = max(shaft, scattered);
                    }
                }
                let sun_color = mix(vec3<f32>(0.70, 0.82, 1.0), vec3<f32>(1.0, 0.70, 0.43), cave.sun_direction.w);
                result += sun_color * shaft * (0.47 * cave.parameters.w * smoothstep(0.01, 0.23, altitude));
            }
            if cave.gallery_dust > 0.001 {
                // Along the visible ray, integrate diffuse skylight scattered by
                // mineral haze. The aperture subtends progressively less sky
                // in the side galleries. Stop at the opaque surface or sky lip.
                let area = cave.salt_window.w * cave.salt_window.w;
                var incident = 0.0;
                for (var i = 0; i < 3; i += 1) {
                    let sample_pos = origin + ray * (travel * (f32(i) + 0.5) / 3.0);
                    let delta = cave.salt_window.xyz - sample_pos;
                    incident += area / (area + dot(delta, delta)) * smoothstep(-0.5, 2.0, delta.y);
                }
                let scattered = (1.0 - exp(-0.013 * travel)) * incident / 3.0 * cave.parameters.w;
                result = mix(result, vec3<f32>(0.37, 0.59, 0.93), clamp(scattered * 1.7, 0.0, 0.32) * cave.gallery_dust);
            }
        }
        // Spirit Forest air reaches a little way into the mouth. The local
        // density vanishes in deep caves; depth and height prevent a sky veil.
        if cave.parameters.z > 0.001 {
            let eye_height = cave.world_from_view[3].y;
            let ground_band = 1.0 - smoothstep(0.5, 3.2, world.y - eye_height);
            let near_path = 1.0 - exp(-min(distance, 28.0) * 0.055);
            let opacity = cave.parameters.z * ground_band * near_path * 0.13;
            let mist = color.rgb * vec3<f32>(0.82, 1.03, 1.06) + vec3<f32>(0.006, 0.013, 0.014);
            result = mix(result, mist, opacity);
        }
        if wet > 0.001 {
            // Short paths retain warm pigments. Across the basin red falls off first.
            let absorption = exp(-vec3<f32>(0.043, 0.015, 0.008) * wet_distance * wet);
            result *= absorption;
            let medium = 1.0 - exp(-wet_distance * mix(0.042, 0.76, cave.sediment) * wet);
            result = mix(result, cave.scattering.rgb, medium);
        }
    } else if wet > 0.001 {
        let upper = smoothstep(-0.25, 0.9, ray.y);
        let depth_light = 0.36 + 0.64 * exp(-min(cave.underwater.y, 96.0) * 0.03);
        let marine = cave.scattering.rgb * depth_light * (0.8 + upper * 0.65);
        result = mix(result, marine, water * 0.86);
    }
    if wet > 0.001 && cave.scattering.w > 0.001 && cave.sun_direction.y > 0.03 {
        let time = cave.underwater.z;
        // Project refracted daylight back to the waves in world coordinates.
        // Nearby rocks and reeds intercept the ray at their actual scene depth.
        let slope = cave.sun_direction.xz * 0.65;
        let height = origin.y + cave.underwater.y;
        let sun = cave.scattering.w * smoothstep(0.03, 0.4, cave.sun_direction.y) * wet;
        let sample_distance = min(wet_distance, 36.0);
        var shafts = 0.0;
        for (var i = 0; i < 3; i += 1) {
            let p = origin + ray * (sample_distance * (f32(i) + 0.5) / 3.0);
            let d = max(height - p.y, 0.0);
            let uv = p.xz + d * slope;
            let ribbon = sin(dot(uv, vec2<f32>(0.37, 0.18)) + sin(uv.y * 0.27 + time * 0.18));
            let group = sin(dot(uv, vec2<f32>(0.13, -0.17)) - time * 0.12);
            shafts += smoothstep(0.62, 1.0, ribbon) * smoothstep(-0.35, 0.86, group) * exp(-d * 0.064);
        }
        result += vec3<f32>(0.26, 0.76, 0.84) * (sun * shafts / 3.0)
            * (1.0 - exp(-sample_distance * 0.08)) * 0.21;
        if depth > 0.000001 && world.y < height && distance < 34.0 {
            let d = height - world.y;
            let floor_uv = world.xz + d * slope;
            let lens = sin(dot(floor_uv, vec2<f32>(2.4, 1.2)) + time * 0.54 + sin(floor_uv.y * 1.8))
                + sin(dot(floor_uv, vec2<f32>(-1.3, 2.1)) - time * 0.47 + sin(floor_uv.x * 1.3));
            let filaments = 1.0 - smoothstep(0.03, 0.23, abs(lens));
            let light = sun * exp(-d * 0.065) * (1.0 - smoothstep(18.0, 34.0, distance));
            result *= vec3<f32>(1.0) + vec3<f32>(0.38, 0.55, 0.50) * filaments * light;
        }
    }
    if cave.lava_depth > 0.0 {
        // Molten rock absorbs visible light within a fraction of a metre.
        // Untouched sky pixels receive the same self-lit medium as distant rock.
        let path = select(4.0, min(distance, 4.0), depth > 0.000001);
        let transmission = exp(-vec3<f32>(2.8, 5.1, 7.4) * (0.24 + path));
        let p = origin + ray * min(path, 1.3);
        let time = cave.underwater.z;
        let eddy = sin(dot(p, vec3<f32>(1.37, 0.52, -1.22)) + time * 0.38
            + sin(dot(p, vec3<f32>(0.83, -1.3, 1.19)) - time * 0.24));
        let surface_light = exp(-min(cave.lava_depth, 3.0) * 0.75);
        let heat = clamp(surface_light * (0.56 + 0.38 * smoothstep(-0.85, 0.85, ray.y))
            + 0.085 * eddy, 0.0, 1.0);
        let melt = mix(vec3<f32>(0.26, 0.015, 0.0015), vec3<f32>(0.91, 0.19, 0.014), heat);
        result = mix(melt, result, transmission);
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
