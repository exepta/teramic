#import bevy_pbr::{forward_io::VertexOutput, mesh_view_bindings::view}

struct Haze { color: vec4<f32> }
@group(#{MATERIAL_BIND_GROUP}) @binding(100) var<uniform> haze: Haze;
@group(#{MATERIAL_BIND_GROUP}) @binding(101) var<uniform> moon: vec4<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(102) var<uniform> sky: vec4<f32>;

fn hash(p: vec3<f32>) -> f32 {
    return fract(sin(dot(p, vec3<f32>(127.1, 311.7, 74.7))) * 43758.5453);
}

fn celestial_uv(direction: vec3<f32>, axis: vec3<f32>, radius: f32) -> vec2<f32> {
    let right = normalize(cross(axis, vec3<f32>(0.0, 1.0, 0.0)));
    let up = cross(right, axis);
    return vec2<f32>(dot(direction, right), dot(direction, up)) / radius;
}

fn crescent(p: vec2<f32>) -> bool {
    return dot(p, p) < 1.0 && length(p - vec2<f32>(0.43, 0.27)) > 0.88;
}

// Quantized silhouettes and stepped relief share the sky pass: no per-voxel draws.
fn voxel_moon(uv: vec2<f32>) -> vec4<f32> {
    if max(abs(uv.x), abs(uv.y)) > 1.45 { return vec4<f32>(0.0); }
    let grid = uv * 12.0;
    let p = (floor(grid) + 0.5) / 12.0;
    let front = crescent(p);
    let back = crescent(p + vec2<f32>(0.12, 0.08));
    let grain = hash(vec3<f32>(floor(grid), 41.0));
    let edge = min(fract(grid).x, 1.0 - fract(grid).y);
    let relief = floor(sqrt(max(0.0, 1.0 - dot(p, p))) * 5.0) / 5.0;
    var color = mix(vec3<f32>(0.24, 0.38, 0.72), vec3<f32>(0.94, 0.91, 0.72), relief);
    color *= 0.88 + grain * 0.12 + select(0.0, 0.12, edge < 0.16);
    if front { return vec4<f32>(color * 1.35, 1.0); }
    if back { return vec4<f32>(vec3<f32>(0.24, 0.38, 0.78) * (0.8 + grain * 0.2), 1.0); }
    let outside = max(length(uv) - 1.0, 0.88 - length(uv - vec2<f32>(0.43, 0.27)));
    return vec4<f32>(0.35, 0.50, 0.9, exp(-max(outside, 0.0) * 18.0) * 0.14);
}

fn voxel_sun(uv: vec2<f32>) -> vec4<f32> {
    if max(abs(uv.x), abs(uv.y)) > 2.1 { return vec4<f32>(0.0); }
    let grid = uv * 10.0;
    let p = (floor(grid) + 0.5) / 10.0;
    let a = abs(p);
    let radius = length(p);
    let relief = floor(sqrt(max(0.0, 1.0 - dot(p, p))) * 6.0) / 6.0;
    let light = clamp(relief * 0.75 + (p.x + p.y) * 0.16 + 0.18, 0.0, 1.0);
    var color = mix(vec3<f32>(1.0, 0.22, 0.008), vec3<f32>(1.0, 0.83, 0.12), light);
    let bevel = select(0.0, 0.04, min(fract(grid).x, 1.0 - fract(grid).y) < 0.14);
    color += vec3<f32>(bevel + hash(vec3<f32>(floor(grid), 13.0)) * 0.045);
    if radius < 1.0 { return vec4<f32>(color, 1.0); }
    return vec4<f32>(1.0, 0.61, 0.15, exp(-max(length(uv) - 1.0, 0.0) * 4.0) * 0.16);
}

struct HorizonOutput {
    @location(0) color: vec4<f32>,
    @builtin(frag_depth) depth: f32,
}

@fragment
fn fragment(in: VertexOutput) -> HorizonOutput {
    let direction = normalize(in.world_position.xyz - view.world_position);
    // Cover the planetary ground edge completely; gently retain the real sky above it.
    let opacity = (1.0 - smoothstep(0.005, 0.08, direction.y)) * haze.color.a;
    let horizon = smoothstep(0.0, 0.07, direction.y);
    var celestial = vec4<f32>(0.0);
    if dot(direction, -moon.xyz) > 0.0 {
        celestial = voxel_sun(celestial_uv(direction, -moon.xyz, 0.065));
        celestial.a *= sky.y * horizon;
    } else {
        celestial = voxel_moon(celestial_uv(direction, moon.xyz, 0.085));
        celestial.a *= moon.w * horizon;
    }
    // Rotating 3D cells avoid a longitude seam or a cluster at the poles.
    let axis_y = dot(direction, vec3<f32>(0.0, 0.9, 0.4358899));
    let axis_z = dot(direction, vec3<f32>(0.0, -0.4358899, 0.9));
    let rotated = vec3<f32>(direction.x * cos(sky.x) + axis_y * sin(sky.x),
        -direction.x * sin(sky.x) + axis_y * cos(sky.x), axis_z);
    let grid = rotated * 150.0;
    let cell = floor(grid);
    let center = vec3<f32>(hash(cell), hash(cell + 19.0), hash(cell + 47.0));
    let distance = length(fract(grid) - center);
    let width = max(length(fwidth(grid)) * 0.5, 0.025);
    let stars = (1.0 - smoothstep(0.065, 0.065 + width, distance))
        * step(0.86, hash(cell + 83.0)) * 0.85 * moon.w * moon.w * horizon;
    let star_alpha = stars * (1.0 - celestial.a);
    let celestial_rgb = celestial.rgb * celestial.a + vec3<f32>(0.72, 0.82, 1.0) * star_alpha;
    let celestial_alpha = celestial.a + star_alpha;
    // Keep the stylized night sky dark even with the gameplay-friendly moonlight.
    let night_alpha = moon.w * moon.w;
    // Deliberate linear HDR sky palette, not copied sRGB fog values. A soft
    // neutral-blue floor stays above the ACES toe without the old blue cast.
    let night_color = mix(haze.color.rgb, vec3<f32>(0.035, 0.045, 0.065), smoothstep(0.0, 0.6, direction.y));
    let base_alpha = opacity + night_alpha * (1.0 - opacity);
    let base_rgb = haze.color.rgb * opacity + night_color * night_alpha * (1.0 - opacity);
    let alpha = celestial_alpha + base_alpha * (1.0 - celestial_alpha);
    var rgb = base_rgb * (1.0 - celestial_alpha)
        + celestial_rgb;
    var final_alpha = alpha;
    // The sky shell also closes the submerged medium. Its old horizon-only
    // opacity left a black upper wedge when looking below finite water meshes.
    // This reuses the exact biome/daylight fog hue and costs no extra pass.
    if sky.z > 0.001 {
        let depth_light = 0.35 + 0.65 * exp(-min(sky.w, 96.0) * 0.028);
        let upper = smoothstep(-0.30, 0.90, direction.y);
        let marine = haze.color.rgb * depth_light * (0.82 + upper * 0.52);
        let medium = clamp(sky.z, 0.0, 1.0);
        rgb = mix(rgb, marine, medium);
        final_alpha = mix(alpha, 1.0, medium);
    }
    var out: HorizonOutput;
    out.color = vec4<f32>(rgb / max(final_alpha, 0.00001), final_alpha);
    // A camera-centered shell is only a ray-direction carrier. Its physical
    // radius must not let the sun or moon render in front of cloud geometry.
    out.depth = 0.0000001;
    return out;
}
