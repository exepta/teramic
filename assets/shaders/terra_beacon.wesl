import bevy_pbr::render::mesh_functions;
import bevy_pbr::render::mesh_view_bindings::view;
import bevy_pbr::render::view_transformations::position_world_to_clip;
@if(DEPTH_PREPASS)
import bevy_pbr::prepass::utils::prepass_depth;
@if(DEPTH_PREPASS)
import bevy_pbr::render::view_transformations::position_ndc_to_world;

@group(constants::MATERIAL_BIND_GROUP) @binding(0) var<uniform> light_color: vec4<f32>;

struct BeamVertex {
    @builtin(position) position: vec4<f32>,
    @location(0) world: vec3<f32>,
    @location(1) @interpolate(flat) origin: vec3<f32>,
    @location(2) @interpolate(flat) direction: vec3<f32>,
    @location(3) @interpolate(flat) dimensions: vec2<f32>,
}

@vertex
fn vertex(@builtin(instance_index) instance: u32, @location(0) point: vec3<f32>) -> BeamVertex {
    let model = mesh_functions::get_world_from_local(instance);
    let world = mesh_functions::mesh_position_local_to_world(model, vec4<f32>(point, 1.0));
    var out: BeamVertex;
    out.position = position_world_to_clip(world.xyz);
    out.world = world.xyz;
    out.origin = model[3].xyz;
    out.direction = normalize(-model[2].xyz);
    out.dimensions = vec2<f32>(length(model[2].xyz), length(model[0].xyz));
    return out;
}

// Intersect a camera-ray interval with the half space value + t * rate <= 0.
fn clip_plane(span: vec2<f32>, value: f32, rate: f32) -> vec2<f32> {
    if abs(rate) < 0.000001 { return select(span, vec2<f32>(1.0, 0.0), value > 0.0); }
    let t = -value / rate;
    return select(vec2<f32>(max(span.x, t), span.y), vec2<f32>(span.x, min(span.y, t)), rate > 0.0);
}

struct BeamFragment {
    @location(0) color: vec4<f32>,
    @builtin(frag_depth) depth: f32,
}

@fragment
fn fragment(input: BeamVertex) -> BeamFragment {
    let eye = view.world_position;
    let exit_distance = distance(input.world, eye);
    let ray = (input.world - eye) / max(exit_distance, 0.0001);
    let relative = eye - input.origin;
    let sideways = vec3<f32>(input.direction.z, 0.0, -input.direction.x);
    let p = vec3<f32>(dot(relative, sideways), relative.y, dot(relative, input.direction));
    let v = vec3<f32>(dot(ray, sideways), ray.y, dot(ray, input.direction));
    let reach = input.dimensions.x;
    let r0 = 0.24;
    let slope = (input.dimensions.y - r0) / reach;
    let radius = r0 + slope * p.z;
    let dr = slope * v.z;
    var span = vec2<f32>(0.0, exit_distance);
    span = clip_plane(span, -p.z, -v.z);
    span = clip_plane(span, p.z - reach, v.z);
    span = clip_plane(span, p.x - radius, v.x - dr);
    span = clip_plane(span, -p.x - radius, -v.x - dr);
    span = clip_plane(span, p.y - radius, v.y - dr);
    span = clip_plane(span, -p.y - radius, -v.y - dr);

@if(DEPTH_PREPASS) {
    let depth = prepass_depth(input.position, 0u);
    if depth > 0.0 {
        let screen = (input.position.xy - view.viewport.xy) / view.viewport.zw;
        let ndc = screen * vec2<f32>(2.0, -2.0) + vec2<f32>(-1.0, 1.0);
        let surface = position_ndc_to_world(vec3<f32>(ndc, depth));
        span.y = min(span.y, dot(surface - eye, ray));
    }
}
    if span.y <= span.x { discard; }
    let step_size = (span.y - span.x) / 6.0;
    var density = 0.0;
    var first = span.y;
    for (var i = 0u; i < 6u; i += 1u) {
        let t = span.x + (f32(i) + 0.5) * step_size;
        let sample_position = p + v * t;
        let r = r0 + slope * sample_position.z;
        let rr = dot(sample_position.xy, sample_position.xy) / (r * r);
        let profile = max(0.0, 1.0 - rr);
        let air = 1.0 - smoothstep(18.0, 96.0, sample_position.z);
        let scattered = profile * profile * air / r;
        if scattered > 0.0002 { first = min(first, t); }
        density += scattered;
    }
    let radiance = 1.0 - exp(-0.30 * density * step_size);
    if radiance < 0.001 { discard; }
    let hit = position_world_to_clip(eye + ray * first);
    var out: BeamFragment;
    // Bevy's additive pipeline uses premultiplied blending with zero alpha.
    out.color = vec4<f32>(light_color.rgb * radiance, 0.0);
    out.depth = clamp(hit.z / hit.w, 0.000001, 1.0);
    return out;
}
