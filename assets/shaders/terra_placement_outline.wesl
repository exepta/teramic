import bevy_pbr::render::mesh_functions;
import bevy_pbr::render::mesh_view_bindings::view;
import bevy_pbr::render::view_transformations::position_world_to_clip;


@group(constants::MATERIAL_BIND_GROUP) @binding(0) var<uniform> color: vec4<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(1) var<uniform> parameters: vec4<f32>;

struct Vertex {
    @builtin(instance_index) instance_index: u32,
    @location(0) position: vec3<f32>,
    // The cached edge mesh stores its endpoint delta instead of a surface normal.
    @location(1) delta: vec3<f32>,
    @location(2) corner: vec2<f32>,
};

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
};

@vertex
fn vertex(input: Vertex) -> VertexOutput {
    let model = mesh_functions::get_world_from_local(input.instance_index);
    let start = input.position - input.delta * input.corner.x;
    let a_world = mesh_functions::mesh_position_local_to_world(model, vec4<f32>(start, 1.0));
    let b_world = mesh_functions::mesh_position_local_to_world(model, vec4<f32>(start + input.delta, 1.0));
    var a = position_world_to_clip(a_world.xyz);
    var b = position_world_to_clip(b_world.xyz);
    var out: VertexOutput;
    // Clip before dividing by w, including edges crossing behind the camera.
    let near = 0.001;
    if a.w < near && b.w < near {
        out.position = vec4<f32>(2.0, 2.0, 2.0, 1.0);
        return out;
    }
    if a.w < near { a = mix(a, b, (near - a.w) / (b.w - a.w)); }
    if b.w < near { b = mix(b, a, (near - b.w) / (a.w - b.w)); }
    let viewport = max(view.viewport.zw, vec2<f32>(1.0));
    let direction = (b.xy / b.w - a.xy / a.w) * viewport;
    let length_squared = dot(direction, direction);
    if length_squared < 0.000001 {
        out.position = vec4<f32>(2.0, 2.0, 2.0, 1.0);
        return out;
    }
    let perpendicular = vec2<f32>(-direction.y, direction.x) * inverseSqrt(length_squared);
    out.position = mix(a, b, input.corner.x);
    out.position.x += perpendicular.x * parameters.x / viewport.x * out.position.w * input.corner.y;
    out.position.y += perpendicular.y * parameters.x / viewport.y * out.position.w * input.corner.y;
    return out;
}

@fragment
fn fragment() -> @location(0) vec4<f32> {
    return color;
}
