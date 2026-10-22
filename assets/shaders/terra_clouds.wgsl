#import bevy_pbr::{forward_io::{Vertex, FragmentOutput}, mesh_functions, view_transformations::position_world_to_clip}

@group(#{MATERIAL_BIND_GROUP}) @binding(100) var<uniform> weather: vec4<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(101) var<uniform> observer: vec4<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(102) var<uniform> haze: vec4<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(103) var<uniform> lightning: vec4<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(104) var<uniform> lightning_tint: vec4<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(105) var<uniform> sun: vec4<f32>;

struct CloudVertex {
    @builtin(position) position: vec4<f32>,
    @location(0) world: vec3<f32>,
    @location(1) shade: vec4<f32>,
    @location(2) uv: vec2<f32>,
}
@vertex
fn vertex(input: Vertex) -> CloudVertex {
    let model = mesh_functions::get_world_from_local(input.instance_index);
    let origin = vec3<f32>(input.uv.x,input.uv_b.x,input.uv.y);
    let cover = smoothstep(0.10, 0.96, weather.x);
    let bank_visibility = smoothstep(input.color.y - 0.055, input.color.y + 0.055, 0.12 + cover * 0.88);
    // Sparse, scattered and broken banks grow horizontally into an overcast
    // deck; storm cover must not turn low puffs into isolated giant balls.
    let growth = smoothstep(0.20, 0.94, weather.x);
    let size = vec3<f32>(0.85 + growth * 1.35, 0.90 + growth * 0.28, 0.85 + growth * 1.35) * bank_visibility;
    var position = origin + (input.position-origin)*size;
    if input.uv_b.y > 0.5 {
        // Crossed world-space ribbons follow the real solar direction. Their
        // material is additive and depth-tested against the original world.
        position = input.position;
        let drop = max(0.0, origin.y - position.y);
        position.x -= sun.x * drop / max(sun.y, 0.20);
        position.z -= sun.z * drop / max(sun.y, 0.20);
    }
    let world = mesh_functions::mesh_position_local_to_world(model, vec4<f32>(position, 1.0));
    var out: CloudVertex;
    out.position = position_world_to_clip(world.xyz);
    // Bevy's infinite reverse perspective and NoFrustumCulling retain distant
    // clouds with their real depth. Aerial scattering must see that distance;
    // only the celestial background is placed at a fixed far depth (1e-7).
    out.world = world.xyz;
    out.shade = input.color;
    out.uv = input.color.rb;
    return out;
}
@fragment
fn fragment(input: CloudVertex) -> FragmentOutput {
    // Solid puffs fade into the actual scene haze, without visible screen-door dots.
    let cover = smoothstep(0.10, 0.96, weather.x);
    let bank_visibility = smoothstep(input.shade.y - 0.055, input.shade.y + 0.055, 0.12 + cover * 0.88);
    let distance=length(input.world.xz-observer.xz);
    if input.shade.a < 0.5 {
        let edge = pow(max(0.0, 1.0 - abs(input.uv.x * 2.0 - 1.0)), 2.0);
        let ends = smoothstep(0.0, 0.12, input.uv.y) * (1.0 - smoothstep(0.65, 1.0, input.uv.y));
        let near_fade = smoothstep(3.0, 18.0, length(input.world - observer.xyz));
        let far_fade = 1.0 - smoothstep(200.0, 650.0, distance);
        let gap = 1.0 - smoothstep(0.02, 0.25, bank_visibility);
        let alpha = edge * ends * near_fade * far_fade * gap * sun.w * weather.y * 0.065;
        if alpha < 0.0001 || sun.y < 0.10 { discard; }
        var shaft: FragmentOutput;
        // AlphaMode::Add uses premultiplied blending. Custom fragments must
        // perform the same RGB weighting/zero-alpha step as Bevy's PBR path.
        shaft.color = vec4<f32>(vec3<f32>(1.0, 0.90, 0.69) * alpha, 0.0);
        return shaft;
    }
    if bank_visibility<0.001 || weather.z<0.02 || distance>observer.w { discard; }
    let storm = smoothstep(0.58, 0.99, weather.x);
    let white = vec3<f32>(1.0, 0.985, 0.965);
    let gray = vec3<f32>(0.18, 0.205, 0.255);
    let base = mix(white, gray, storm);
    let grain = fract(sin(dot(floor(input.world / 5.5), vec3<f32>(127.1, 311.7, 74.7))) * 43758.5453);
    let self_shade = input.shade.x * mix(0.91, 1.07, input.shade.z) * (0.96 + grain * 0.04);
    let daylight = mix(vec3<f32>(0.16, 0.22, 0.32), vec3<f32>(1.0), weather.y);
    // Nearby banks light up around the discharge, not the complete sky at once.
    // A broad halo suggests scattering through the cloud's stylised volume.
    let strike_distance = length(input.world.xz - lightning.xz);
    let local_flash = lightning.w * exp(-strike_distance * strike_distance / 22500.0);
    let lit = base * self_shade * daylight
        + lightning_tint.rgb * (local_flash * 0.65 + weather.w * 0.02);
    let far_fog=smoothstep(observer.w*0.48,observer.w*0.97,distance);
    let color=mix(lit,haze.rgb,far_fog);
    var out: FragmentOutput;
    out.color = vec4<f32>(color, 1.0);
    return out;
}
