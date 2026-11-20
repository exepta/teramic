import bevy_pbr::render::mesh_functions;
import bevy_pbr::render::mesh_view_bindings::view;
import bevy_pbr::render::view_transformations::position_world_to_clip;
@if(DEPTH_PREPASS)
import bevy_pbr::prepass::utils::prepass_depth;
@if(DEPTH_PREPASS)
import bevy_pbr::render::view_transformations::position_ndc_to_world;

@group(constants::MATERIAL_BIND_GROUP) @binding(0) var<uniform> color: vec4<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(1) var<uniform> flow: vec4<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(2) var<uniform> center: vec4<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(3) var<uniform> half_extent: vec4<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(4) var<uniform> field: vec4<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(5) var density_noise: texture_3d<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(6) var density_sampler: sampler;
@group(constants::MATERIAL_BIND_GROUP) @binding(7) var terrain_field: texture_2d<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(8) var terrain_sampler: sampler;
@group(constants::MATERIAL_BIND_GROUP) @binding(9) var<uniform> sources: array<vec4<f32>,16>;

struct Vertex { @builtin(instance_index) instance_index: u32, @location(0) position: vec3<f32>, }
struct FragmentInput { @builtin(position) position: vec4<f32>, @location(0) world: vec3<f32>, }
@vertex
fn vertex(input: Vertex) -> FragmentInput {
    let model = mesh_functions::get_world_from_local(input.instance_index);
    let world = mesh_functions::mesh_position_local_to_world(model, vec4<f32>(input.position, 1.0));
    return FragmentInput(position_world_to_clip(world.xyz), world.xyz);
}
fn interval_in_box(origin: vec3<f32>, ray: vec3<f32>) -> vec2<f32> {
    let direction = select(vec3<f32>(-1.0), vec3<f32>(1.0), ray >= vec3<f32>(0.0));
    let inverse_ray = direction / max(abs(ray), vec3<f32>(0.00001));
    let a = (-half_extent.xyz - origin) * inverse_ray;
    let b = (half_extent.xyz - origin) * inverse_ray;
    let near = min(a,b); let far = max(a,b);
    return vec2<f32>(max(0.0, max(near.x,max(near.y,near.z))), min(far.x,min(far.y,far.z)));
}
fn density(world: vec3<f32>) -> f32 {
    // A strict union of circles, independent of field filtering or overlap.
    // Radius is eight blocks, with a soft edge inward; it never expands outward.
    var circular = 0.0;
    for (var i=0u; i<16u; i+=1u) {
        if f32(i) >= center.w { break; }
        let delta=world.xz-sources[i].xz;
        let squared=dot(delta,delta);
        if squared >= 64.0 { continue; }
        let distance=sqrt(squared);
        circular=max(circular,1.0-smoothstep(6.2,8.0,distance));
        if circular >= 0.999 { break; }
    }
    if circular <= 0.0 { return 0.0; }
    let uv=((world.xz-field.xy)/2.0+vec2<f32>(0.5))/41.0;
    let ground=textureSampleLevel(terrain_field,terrain_sampler,uv,0.0);
    let floor_height=field.z+dot(ground.rg,vec2<f32>(65280.0,255.0))/65535.0*field.w;
    let height=(world.y-floor_height)/half_extent.w;
    if height <= 0.0 || height >= 1.0 || ground.b <= 0.002 { return 0.0; }
    let advected = (world - vec3<f32>(flow.x, flow.z, flow.y)) / 64.0;
    let broad = textureSampleLevel(density_noise,density_sampler,advected,0.0).r;
    let fine = textureSampleLevel(density_noise,density_sampler,advected * 2.0 + vec3<f32>(0.37),0.0).r;
    let rising = height + (broad - 0.5) * 0.23;
    let vertical = smoothstep(0.02,0.18,rising) * (1.0 - smoothstep(0.4,0.98,rising));
    let wisps = 0.20 + 0.80 * smoothstep(0.18,0.76,broad * 0.80 + fine * 0.20);
    return vertical * wisps * ground.b * circular;
}
struct FragmentOutput { @location(0) color: vec4<f32>, @builtin(frag_depth) depth: f32, }
@fragment
fn fragment(input: FragmentInput) -> FragmentOutput {
    let eye = view.world_position;
    let ray = normalize(input.world - eye);
    var interval = interval_in_box(eye - center.xyz,ray);
@if(DEPTH_PREPASS) {
    let depth = prepass_depth(input.position,0u);
    if depth > 0.0 {
        let screen = (input.position.xy - view.viewport.xy) / view.viewport.zw;
        let ndc = screen * vec2<f32>(2.0,-2.0) + vec2<f32>(-1.0,1.0);
        let surface = position_ndc_to_world(vec3<f32>(ndc,depth));
        interval.y = min(interval.y,max(0.0,dot(surface - eye,ray)));
    }
}
    if interval.y <= interval.x { discard; }
    let step = (interval.y - interval.x) / 6.0;
    var optical_depth = 0.0;
    var first_hit = interval.y;
    // One volume and six samples even when sixteen sources overlap.
    for (var i = 0u; i < 6u; i += 1u) {
        let distance = interval.x + (f32(i) + 0.5) * step;
        let d = density(eye + ray * distance);
        if d > 0.008 { first_hit = min(first_hit,distance); }
        optical_depth += d * step * flow.w;
    }
    let alpha = (1.0 - exp(-optical_depth)) * color.a;
    if alpha < 0.003 { discard; }
    let hit = position_world_to_clip(eye + ray * first_hit);
    return FragmentOutput(vec4<f32>(color.rgb,alpha),clamp(hit.z / hit.w,0.000001,1.0));
}
