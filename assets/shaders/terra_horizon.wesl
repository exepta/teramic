import bevy_pbr::render::forward_io::VertexOutput;
import bevy_pbr::render::mesh_view_bindings::view;


struct Haze { color: vec4<f32> }
@group(constants::MATERIAL_BIND_GROUP) @binding(100) var<uniform> haze: Haze;
@group(constants::MATERIAL_BIND_GROUP) @binding(101) var<uniform> moon: vec4<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(102) var<uniform> sky: vec4<f32>;

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

// Distance to the two visible arcs, including their endpoints. A distance
// to the complete circles leaves a ghost rim across the missing side.
fn moon_glow_distance(uv: vec2<f32>) -> f32 {
    let center = vec2<f32>(0.43, 0.27);
    let cut_radius = 0.88;
    if crescent(uv) { return 0.0; }
    let separation = length(center);
    let axis = center / separation;
    let along = (1.0 - cut_radius * cut_radius + separation * separation) / (2.0 * separation);
    let side = vec2<f32>(-axis.y, axis.x) * sqrt(1.0 - along * along);
    let middle = axis * along;
    var nearest = min(distance(uv, middle + side), distance(uv, middle - side));
    let outside_radius = length(uv);
    let on_outer = uv / max(outside_radius, 0.00001);
    if distance(on_outer, center) >= cut_radius {
        nearest = min(nearest, abs(outside_radius - 1.0));
    }
    let inside_radius = distance(uv, center);
    let on_inner = center + cut_radius * (uv - center) / max(inside_radius, 0.00001);
    if length(on_inner) <= 1.0 {
        nearest = min(nearest, abs(inside_radius - cut_radius));
    }
    return nearest;
}

// Quantized silhouettes and stepped relief share the sky pass: no per-voxel draws.
fn voxel_moon(uv: vec2<f32>, ash: f32) -> vec4<f32> {
    if dot(uv, uv) >= 9.0 { return vec4<f32>(0.0); }
    let grid = uv * 12.0;
    let p = (floor(grid) + 0.5) / 12.0;
    let front = crescent(p);
    let back = crescent(p + vec2<f32>(0.12, 0.08));
    let grain = hash(vec3<f32>(floor(grid), 41.0));
    let edge = min(fract(grid).x, 1.0 - fract(grid).y);
    let relief = floor(sqrt(max(0.0, 1.0 - dot(p, p))) * 5.0) / 5.0;
    var color = mix(vec3<f32>(0.24, 0.38, 0.72), vec3<f32>(0.94, 0.91, 0.72), relief);
    color = mix(color, color * vec3<f32>(1.32, 0.72, 0.64), ash);
    color *= 0.88 + grain * 0.12 + select(0.0, 0.12, edge < 0.16);
    if front { return vec4<f32>(color * 1.35, 1.0); }
    if back { return vec4<f32>(vec3<f32>(0.24, 0.38, 0.78) * (0.8 + grain * 0.2), 1.0); }
    let falloff = moon_glow_distance(uv);
    let halo = (0.10 * exp(-falloff * 9.0) + 0.045 * exp(-falloff * 3.8))
        * (1.0 - smoothstep(2.4, 3.0, length(uv)));
    return vec4<f32>(0.48, 0.62, 0.95, halo);
}

// The angular texture is anchored to the disk as it follows the local sun or moon.
fn crater_noise(p: vec2<f32>) -> f32 {
    let base = floor(p);
    let f = smoothstep(vec2<f32>(0.0), vec2<f32>(1.0), fract(p));
    return mix(mix(hash(vec3<f32>(base, 17.0)), hash(vec3<f32>(base + vec2<f32>(1.0, 0.0), 17.0)), f.x),
               mix(hash(vec3<f32>(base + vec2<f32>(0.0, 1.0), 17.0)), hash(vec3<f32>(base + 1.0, 17.0)), f.x), f.y);
}

fn voxel_sun(uv: vec2<f32>, gaze: f32, ash: f32) -> vec4<f32> {
    let angular_radius = length(uv);
    if angular_radius >= 6.0 { return vec4<f32>(0.0); }
    let grid = uv * 10.0;
    let p = (floor(grid) + 0.5) / 10.0;
    let radius = length(p);
    if radius >= 1.0 {
        let distance = max(angular_radius - 1.0, 0.0);
        let corona = 0.24 * exp(-distance * 3.5);
        let glare = gaze * 0.23 * exp(-0.40 * distance * distance);
        let alpha = (corona + glare) * (1.0 - smoothstep(5.0, 6.0, angular_radius));
        return vec4<f32>(mix(vec3<f32>(1.9, 1.15, 0.47), vec3<f32>(1.40, 0.46, 0.20), ash), alpha);
    }
    let relief = floor(sqrt(max(0.0, 1.0 - dot(p, p))) * 6.0) / 6.0;
    let light = clamp(relief * 0.75 + (p.x + p.y) * 0.16 + 0.18, 0.0, 1.0);
    var color = mix(vec3<f32>(1.0, 0.22, 0.008), vec3<f32>(1.0, 0.83, 0.12), light);
    let bevel = select(0.0, 0.04, min(fract(grid).x, 1.0 - fract(grid).y) < 0.14);
    color += vec3<f32>(bevel + hash(vec3<f32>(floor(grid), 13.0)) * 0.045);
    color *= vec3<f32>(1.60, 1.40, 1.10);
    if ash > 0.001 {
        let depth = sqrt(max(0.0, 1.0 - dot(p, p)));
        let detail = floor(uv * 75.0) / 75.0;
        let strata = 0.52 * crater_noise(5.0 * detail + depth) + 0.32 * crater_noise(detail * 16.0)
            + 0.16 * crater_noise(detail * 38.0);
        let pigment = smoothstep(0.26, 0.70, strata + depth * 0.13 + p.x * 0.11);
        let ember = mix(vec3<f32>(0.49, 0.035, 0.057), vec3<f32>(1.48, 0.33, 0.042), pigment);
        color = mix(color, ember, ash);
    }
    return vec4<f32>(color + vec3<f32>(0.55, 0.38, 0.19) * gaze, 1.0);
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
    let ash = (1.0 - sky.y) * (1.0 - sky.z);
    // One silhouette at every biome weight. Mixing straight RGB of two
    // differently sized sprites exposed their transparent rectangles and disks.
    if dot(direction, -moon.xyz) > 0.0 {
        let sun = -moon.xyz;
        let gaze = smoothstep(0.90, 0.995, dot(-view.world_from_view[2].xyz, sun));
        celestial = voxel_sun(celestial_uv(direction, sun, mix(0.065, 0.104, ash)), gaze, ash);
        celestial.a *= (1.0 - sky.z) * horizon;
    } else {
        celestial = voxel_moon(celestial_uv(direction, moon.xyz, 0.085), ash);
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
    // Warm suspended ash persists all the way to the zenith. The horizon fog
    // and this gradual sky veil meet in the same linear HDR colour space.
    if ash > 0.001 {
        let altitude = smoothstep(-0.03, 0.85, direction.y);
        let dusk = mix(0.24, 1.0, 1.0 - moon.w);
        let canopy = mix(haze.color.rgb * vec3<f32>(1.20, 0.91, 0.84), vec3<f32>(0.12,0.072,0.145) * dusk, altitude);
        let veil = ash * (0.88 + altitude * 0.06) * (1.0 - celestial_alpha);
        rgb = rgb * (1.0 - veil) + canopy * veil;
        final_alpha += (1.0 - final_alpha) * veil;
    }
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
