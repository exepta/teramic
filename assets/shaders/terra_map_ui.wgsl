#import bevy_ui::ui_vertex_output::UiVertexOutput
#import bevy_render::color_operations::srgb_to_linear

@group(1) @binding(0) var<uniform> row_x: vec4<f32>;
@group(1) @binding(1) var<uniform> row_y: vec4<f32>;
@group(1) @binding(2) var<uniform> clip: vec4<f32>;
@group(1) @binding(3) var terrain: texture_2d<f32>;
@group(1) @binding(4) var terrain_sampler: sampler;
@group(1) @binding(5) var overlay: texture_2d<f32>;
@group(1) @binding(6) var overlay_sampler: sampler;
@group(1) @binding(7) var<uniform> overlay_x: vec4<f32>;
@group(1) @binding(8) var<uniform> overlay_y: vec4<f32>;
@group(1) @binding(9) var<uniform> player: vec4<f32>;
@group(1) @binding(10) var<uniform> grid_x: vec4<f32>;
@group(1) @binding(11) var<uniform> grid_y: vec4<f32>;

@fragment
fn fragment(in: UiVertexOutput) -> @location(0) vec4<f32> {
    if clip.x > 0.0 && distance(in.uv, vec2<f32>(0.5)) > clip.x { discard; }
    if clip.y > 0.5 { return vec4<f32>(0.007, 0.010, 0.009, 1.0); }
    let coordinate = vec3<f32>(in.uv, 1.0);
    let uv = vec2<f32>(dot(row_x.xyz, coordinate), dot(row_y.xyz, coordinate));
    var ground = vec4<f32>(0.007, 0.010, 0.009, 1.0);
    if all(uv >= vec2<f32>(0.0)) && all(uv <= vec2<f32>(1.0)) {
        ground = textureSample(terrain, terrain_sampler, uv);
    }
    let marker_uv = vec2<f32>(dot(overlay_x.xyz, coordinate), dot(overlay_y.xyz, coordinate));
    var marker = vec4<f32>(0.0);
    if all(marker_uv >= vec2<f32>(0.0)) && all(marker_uv <= vec2<f32>(1.0)) {
        marker = textureSample(overlay, overlay_sampler, marker_uv);
    }
    var color = mix(ground.rgb, marker.rgb, marker.a);
    if grid_x.w > 0.5 {
        let cell = vec2<f32>(dot(grid_x.xyz, coordinate), dot(grid_y.xyz, coordinate));
        let width = max(fwidth(cell), vec2<f32>(0.0001));
        let line = min(fract(cell), vec2<f32>(1.0) - fract(cell)) / width;
        let alpha = (1.0 - smoothstep(0.0, 1.1, min(line.x, line.y))) * 0.28;
        color = mix(color, srgb_to_linear(vec3<f32>(0.82, 0.74, 0.49)), alpha);
    }
    let point = (in.uv * clip.z - player.xy) * vec2<f32>(1.0, clip.w);
    let distance_squared = dot(point, point);
    if distance_squared >= 140.0 && distance_squared <= 165.0 {
        color = mix(color, srgb_to_linear(vec3<f32>(224.0, 222.0, 166.0) / 255.0), 220.0 / 255.0);
    }
    let forward = -point.x * player.z - point.y * player.w;
    let side = point.x * player.w - point.y * player.z;
    if forward >= -6.0 && forward <= 10.0 && abs(side) <= (10.0 - forward) * 0.48 {
        let edge = abs(side) > (10.0 - forward) * 0.48 - 1.6 || forward < -4.0;
        color = srgb_to_linear(select(vec3<f32>(82.0, 221.0, 255.0), vec3<f32>(13.0, 27.0, 34.0), edge) / 255.0);
    }
    return vec4<f32>(color, 1.0);
}
