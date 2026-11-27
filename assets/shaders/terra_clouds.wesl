import bevy_pbr::render::forward_io::Vertex;
import bevy_pbr::render::mesh_functions;
import bevy_pbr::render::mesh_view_bindings::view;
import bevy_pbr::render::view_transformations::position_world_to_clip;
@if(DEPTH_PREPASS)
import bevy_pbr::prepass::utils::prepass_depth;
@if(DEPTH_PREPASS)
import bevy_pbr::render::view_transformations::position_ndc_to_world;

struct CloudUniforms {
    weather: vec4<f32>,
    observer: vec4<f32>,
    haze: vec4<f32>,
    lightning: vec4<f32>,
    lightning_tint: vec4<f32>,
    sun: vec3<f32>,
    ash_cover: f32,
    volume: vec4<f32>,
    drift: vec4<f32>,
}
@group(constants::MATERIAL_BIND_GROUP) @binding(100) var<uniform> cloud: CloudUniforms;
@group(constants::MATERIAL_BIND_GROUP) @binding(108) var density_noise: texture_3d<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(109) var density_sampler: sampler;

struct CloudVertex {
    @builtin(position) position: vec4<f32>,
    @location(0) world: vec3<f32>,
}

@vertex
fn vertex(input: Vertex) -> CloudVertex {
    let model = mesh_functions::get_world_from_local(input.instance_index);
    let world = mesh_functions::mesh_position_local_to_world(model, vec4<f32>(input.position, 1.0));
    var out: CloudVertex;
    out.position = position_world_to_clip(world.xyz);
    out.world = world.xyz;
    return out;
}

fn density(position: vec3<f32>) -> f32 {
    let p = position - vec3<f32>(cloud.drift.x, 0.0, cloud.drift.y);
    let noise = textureSampleLevel(density_noise, density_sampler, p / vec3<f32>(2048.0, 256.0, 2048.0), 0.0).rg;
    let column = textureSampleLevel(density_noise, density_sampler,
        vec3<f32>(p.x / 8192.0, 0.31, p.z / 8192.0), 0.0).r;
    let height = (position.y - cloud.volume.x) / (cloud.volume.y - cloud.volume.x);
    let bottom = 0.04 + column * 0.06 + noise.g * 0.14;
    let top = 0.44 + column * 0.32 + noise.r * 0.24;
    let profile = smoothstep(bottom, bottom + 0.13, height)
        * (1.0 - smoothstep(top - 0.22, top, height));
    let cover = smoothstep(0.04, 0.98, cloud.weather.x);
    let threshold = mix(0.82, 0.26, cover);
    let shape = noise.r + (column - 0.5) * 0.24 - (noise.g - 0.5) * 0.16;
    return smoothstep(threshold - 0.10, threshold + 0.10, shape) * profile;
}

// Intersection remains valid below, within and above the layer, including horizontal rays.
fn layer_interval(ray: vec3<f32>) -> vec2<f32> {
    let far = cloud.observer.w / max(length(ray.xz), 0.0001);
    if abs(ray.y) < 0.00001 {
        if cloud.observer.y < cloud.volume.x || cloud.observer.y > cloud.volume.y { return vec2<f32>(0.0); }
        return vec2<f32>(0.0, far);
    }
    let first = (cloud.volume.x - cloud.observer.y) / ray.y;
    let last = (cloud.volume.y - cloud.observer.y) / ray.y;
    return vec2<f32>(max(0.0, min(first, last)), min(far, max(first, last)));
}

struct CloudFragment {
    @location(0) color: vec4<f32>,
    @builtin(frag_depth) depth: f32,
}

@fragment
fn fragment(input: CloudVertex) -> CloudFragment {
    if cloud.weather.x < 0.015 || cloud.weather.z < 0.002 { discard; }
    let ray = normalize(input.world - cloud.observer.xyz);
    var interval = layer_interval(ray);
@if(DEPTH_PREPASS) {
    // Bound integration at the actual terrain, including mountains inside the layer.
    let opaque_depth = prepass_depth(input.position, 0u);
    if opaque_depth > 0.0 {
        let screen = (input.position.xy - view.viewport.xy) / view.viewport.zw;
        let ndc = screen * vec2<f32>(2.0, -2.0) + vec2<f32>(-1.0, 1.0);
        let terrain = position_ndc_to_world(vec3<f32>(ndc, opaque_depth));
        interval.y = min(interval.y, max(0.0, dot(terrain - cloud.observer.xyz, ray)));
    }
}
    if interval.y <= interval.x { discard; }
    let steps = min(u32(cloud.volume.z), 28u);
    if steps == 0u { discard; }
    // Static spatial jitter avoids both moving grain and visible sample bands.
    let pixel = floor(input.position.xy);
    let jitter = 0.05 + fract(sin(dot(pixel, vec2<f32>(12.9898, 78.233))) * 43758.5453) * 0.90;
    let storm = max(cloud.volume.w, smoothstep(0.64, 0.99, cloud.weather.x));
    let extinction = mix(0.032, 0.080, storm);
    let light_direction = select(-cloud.sun, cloud.sun, cloud.sun.y > 0.0);
    let daylight = mix(vec3<f32>(0.13, 0.18, 0.26), vec3<f32>(1.0, 0.985, 0.96), cloud.weather.y);
    let cloud_color = mix(mix(vec3<f32>(1.0), vec3<f32>(0.40, 0.43, 0.48), storm),
        vec3<f32>(0.22, 0.14, 0.23), cloud.ash_cover * 0.91) * daylight;
    let forward_light = pow(max(0.0, dot(ray, light_direction)), 8.0) * 0.18;
    var transmittance = 1.0;
    var scattered = vec3<f32>(0.0);
    var first_hit = interval.y;
    for (var i = 0u; i < steps; i += 1u) {
        // More samples close to the observer; distant banks merge into aerial haze.
        let a = f32(i) / f32(steps);
        let b = f32(i + 1u) / f32(steps);
        let start = mix(interval.x, interval.y, a * a);
        let end = mix(interval.x, interval.y, b * b);
        let distance = mix(start, end, jitter);
        let position = cloud.observer.xyz + ray * distance;
        let d = density(position);
        if d < 0.001 { continue; }
        let opacity = 1.0 - exp(-d * (end - start) * extinction);
        if opacity < 0.0001 { continue; }
        first_hit = min(first_hit, distance);
        let height = clamp((position.y - cloud.volume.x) / (cloud.volume.y - cloud.volume.x), 0.0, 1.0);
        let shadow = density(position + light_direction * 26.0);
        let illumination = 0.44 + exp(-shadow * 2.4) * 0.43 + height * 0.18 + forward_light;
        let strike_distance = length(position.xz - cloud.lightning.xz);
        let flash = cloud.lightning.w * exp(-strike_distance * strike_distance / 22500.0);
        // Indirect lava light catches the bases; the warmest rims face the disk.
        let ember = cloud.ash_cover * (0.035 * (1.0 - height)
            + forward_light * exp(-shadow * 3.0));
        let lit = cloud_color * illumination + vec3<f32>(1.0, 0.25, 0.10) * ember
            + cloud.lightning_tint.rgb * (flash * 0.75 + cloud.weather.w * 0.02);
        let aerial = smoothstep(cloud.observer.w * 0.32, cloud.observer.w * 0.98, length(position.xz - cloud.observer.xz));
        scattered += mix(lit, cloud.haze.rgb, aerial) * opacity * transmittance;
        transmittance *= 1.0 - opacity;
        if transmittance < 0.008 {
            transmittance = 0.0;
            break;
        }
    }
    let alpha = 1.0 - transmittance;
    if alpha < 0.002 { discard; }
    let hit_clip = position_world_to_clip(cloud.observer.xyz + ray * first_hit);
    var out: CloudFragment;
    out.color = vec4<f32>(scattered / max(alpha, 0.0001), alpha * cloud.weather.z);
    // The proxy exit face never determines occlusion or celestial depth.
    out.depth = clamp(hit_clip.z / hit_clip.w, 0.000001, 1.0);
    return out;
}
