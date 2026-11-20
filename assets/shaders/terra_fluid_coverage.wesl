// Exact ownership within the new reconstructed fluid renderer. There is one
// surface per world position; parked fluid and live particles never overlap.
fn particle_surface_pixel(world:vec3<f32>,domain:vec4<f32>,pixel:vec2<f32>)->bool {
    let p=world-domain.xyz;
    return domain.w>0.5&&all(p>=vec3<f32>(0.0))&&all(p<vec3<f32>(16.0));
}
