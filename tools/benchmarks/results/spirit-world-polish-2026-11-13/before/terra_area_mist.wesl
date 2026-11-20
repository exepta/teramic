import bevy_pbr::render::mesh_functions;
import bevy_pbr::render::mesh_view_bindings::view;
import bevy_pbr::render::view_transformations::position_world_to_clip;
@if(DEPTH_PREPASS)
import bevy_pbr::prepass::utils::prepass_depth;
@if(DEPTH_PREPASS)
import bevy_pbr::render::view_transformations::position_ndc_to_world;

@group(constants::MATERIAL_BIND_GROUP) @binding(0) var<uniform> color: vec4<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(1) var<uniform> flow: vec4<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(2) var density_noise: texture_3d<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(3) var density_sampler: sampler;

struct Vertex {
    @builtin(instance_index) instance_index: u32,
    @location(0) position: vec3<f32>,
    @location(1) center: vec3<f32>,
    @location(2) slope: vec2<f32>,
    @location(3) bounds: vec4<f32>,
    @location(4) strength: vec2<f32>,
}

struct MistVertex {
    @builtin(position) position: vec4<f32>,
    @location(0) world: vec3<f32>,
    @location(1) @interpolate(flat) center: vec3<f32>,
    @location(2) @interpolate(flat) bounds: vec4<f32>,
    @location(3) @interpolate(flat) slope: vec2<f32>,
    @location(4) @interpolate(flat) strength: f32,
}

@vertex
fn vertex(input: Vertex) -> MistVertex {
    let model = mesh_functions::get_world_from_local(input.instance_index);
    let world = mesh_functions::mesh_position_local_to_world(model, vec4<f32>(input.position, 1.0));
    var output: MistVertex;
    output.position = position_world_to_clip(world.xyz);
    output.world = world.xyz;
    // Contour vertices and their density domains are cached in world space.
    output.center = input.center;
    output.bounds = input.bounds;
    output.slope = input.slope;
    output.strength = input.strength.x;
    return output;
}

fn density(world: vec3<f32>, input: MistVertex) -> f32 {
    let local = world - input.center;
    let height = (local.y - dot(local.xz, input.slope)) / input.bounds.w;
    let advected = (world - vec3<f32>(flow.x, flow.z, flow.y)) / 64.0;
    let broad = textureSampleLevel(density_noise, density_sampler, advected, 0.0).r;
    let small = textureSampleLevel(density_noise, density_sampler, advected * 2.0 + vec3<f32>(0.37), 0.0).r;
    // The profile drifts upwards as well as sideways and vanishes before the
    // terrain; geometry does not trace the floor with a colored surface.
    let rising = height + (broad - 0.5) * 0.70;
    let lift = smoothstep(-0.88, -0.36, rising);
    let dissipate = 1.0 - smoothstep(0.02, 0.98, rising);
    let normalized = local.xz / input.bounds.xz;
    var horizontal = 0.0;
    if abs(input.bounds.x - input.bounds.z) > 0.05 {
        // Contour domains extend one meter each way. Adjacent centers are one
        // meter apart: these Hann windows sum to one, avoiding a dotted chain.
        let along_x = input.bounds.x > input.bounds.z;
        let along = select(local.z, local.x, along_x);
        let across = select(normalized.x, normalized.y, along_x);
        let longitudinal = 0.5 + 0.5 * cos(clamp(along, -1.0, 1.0) * 3.14159265);
        let sway = (broad - 0.5) * 0.34 + (small - 0.5) * 0.16;
        let width = 0.44 + broad * 0.20;
        let crossfade = exp(-pow((across - sway) / width, 2.0) * 2.0);
        horizontal = longitudinal * crossfade;
    } else {
        // Overlapping biome domains create broad, slowly drifting atmosphere.
        let drift = (vec2<f32>(broad, small) - vec2<f32>(0.5)) * 0.20;
        horizontal = 1.0 - smoothstep(0.25, 1.0, length(normalized + drift));
    }
    let boundary = (1.0 - smoothstep(0.86, 1.0, abs(normalized.x)))
        * (1.0 - smoothstep(0.86, 1.0, abs(normalized.y)));
    // A very light continuous body supports passing wisps without regularly
    // punching holes in the border at every noise cell.
    let wisps = 0.20 + 0.80 * smoothstep(0.18, 0.76, broad * 0.80 + small * 0.20);
    let vertical_edge = 1.0 - smoothstep(0.84, 1.0, abs(local.y) / input.bounds.y);
    return lift * dissipate * horizontal * boundary * vertical_edge * wisps * input.strength;
}

fn box_interval(origin: vec3<f32>, ray: vec3<f32>, extent: vec3<f32>) -> vec2<f32> {
    // A finite reciprocal also handles an axis-aligned construction camera.
    let direction = select(vec3<f32>(-1.0), vec3<f32>(1.0), ray >= vec3<f32>(0.0));
    let inverse_ray = direction / max(abs(ray), vec3<f32>(0.00001));
    let a = (-extent - origin) * inverse_ray;
    let b = (extent - origin) * inverse_ray;
    let near = min(a, b);
    let far = max(a, b);
    return vec2<f32>(max(0.0, max(near.x, max(near.y, near.z))), min(far.x, min(far.y, far.z)));
}

struct MistFragment {
    @location(0) color: vec4<f32>,
    @builtin(frag_depth) depth: f32,
}

@fragment
fn fragment(input: MistVertex) -> MistFragment {
    let eye = view.world_position;
    let ray = normalize(input.world - eye);
    var interval = box_interval(eye - input.center, ray, input.bounds.xyz);
@if(DEPTH_PREPASS) {
    let depth = prepass_depth(input.position, 0u);
    if depth > 0.0 {
        let screen = (input.position.xy - view.viewport.xy) / view.viewport.zw;
        let ndc = screen * vec2<f32>(2.0, -2.0) + vec2<f32>(-1.0, 1.0);
        let surface = position_ndc_to_world(vec3<f32>(ndc, depth));
        interval.y = min(interval.y, max(0.0, dot(surface - eye, ray)));
    }
}
    if interval.y <= interval.x { discard; }
    let step = (interval.y - interval.x) / 8.0;
    var optical_depth = 0.0;
    var first_hit = interval.y;
    for (var i = 0u; i < 8u; i += 1u) {
        let distance = interval.x + (f32(i) + 0.5) * step;
        let d = density(eye + ray * distance, input);
        if d > 0.008 { first_hit = min(first_hit, distance); }
        optical_depth += d * step * flow.w;
    }
    let opacity = (1.0 - exp(-optical_depth)) * color.a;
    if opacity < 0.003 { discard; }
    var output: MistFragment;
    output.color = vec4<f32>(color.rgb, opacity);
    let hit = position_world_to_clip(eye + ray * first_hit);
    output.depth = clamp(hit.z / hit.w, 0.000001, 1.0);
    return output;
}
