import bevy_pbr::render::forward_io::VertexOutput;
import bevy_pbr::render::mesh_view_bindings::globals;

@group(constants::MATERIAL_BIND_GROUP) @binding(100) var<uniform> wind: vec4<f32>;

fn hash(p: vec2<f32>) -> f32 {
    let q = fract(p * vec2<f32>(0.1031, 0.11369));
    return fract((q.x + q.y) * (q.x + q.y + 19.19));
}
fn noise(p: vec2<f32>) -> f32 {
    let c = floor(p);
    let f = fract(p);
    let w = f*f*(3.0-2.0*f);
    return mix(mix(hash(c),hash(c+vec2<f32>(1,0)),w.x),
               mix(hash(c+vec2<f32>(0,1)),hash(c+1.0),w.x),w.y);
}

// Changing height and sideways curl give each tongue its own rhythm.
fn tongue(x: f32, h: f32, t: f32, seed: f32, center: f32, radius: f32, reach: f32) -> f32 {
    let length = reach * (0.76 + 0.30*noise(vec2<f32>(t*0.84,seed)));
    let height = h / length;
    let drift = noise(vec2<f32>(h*3.8-t*1.9,seed+4.2)) - 0.5;
    let bend = h * (drift*0.60 + wind.x*h*0.15);
    let distance = abs(x - center - bend);
    let width = radius * (1.0 - smoothstep(0.12, 1.0, height));
    return min(width - distance, (1.0-height)*0.39);
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let h = 1.0-in.uv.y;
    let x = 2.0*in.uv.x-1.0;
    let top = in.color.b;
    let seed = in.color.r*113.0;
    let t = globals.time*(0.92+in.color.r*0.21);
    let p = vec2<f32>(x*3.4+seed,h*4.8-t*3.0);
    let curl = noise(p*0.53);
    let soot_noise = noise(p + vec2<f32>(curl*1.45,0));
    let detail = 0.70*soot_noise + 0.30*noise(p*2.53);
    let height_scale = mix(1.20,1.0,top) * mix(0.71,1.0,in.color.g);
    let y = h / height_scale;
    let main = tongue(x,y,t,seed,0.04,0.43,1.09);
    let left = tongue(x,y,t,seed+14.1,-0.44,0.35,0.75);
    let right = tongue(x,y,t,seed+31.7,0.45,0.34,0.86);
    let field = max(main,max(left,right))+(detail-0.51)*(0.18+0.23*h);
    let base = smoothstep(0.005,0.17,h+(detail-0.52)*0.12);
    let fade = (1.0-smoothstep(0.81,1.0,abs(x))) * (1.0-smoothstep(0.93,1.0,h));
    let alpha = smoothstep(-0.055,0.055,field)*base*fade;
    let heat = field - h*0.16 + (detail-0.52)*0.20;
    let outer = mix(vec3<f32>(0.91,0.032,0.007),vec3<f32>(2.5,0.38,0.025),smoothstep(-0.06,0.11,heat));
    let flame = mix(outer,vec3<f32>(4.3,2.15,0.48),smoothstep(0.10,0.38,heat));
    let soot = top*0.28*smoothstep(0.38,0.76,y)*smoothstep(0.34,0.72,soot_noise)*
        (1.0-smoothstep(0.1,0.76,abs(x-(curl-0.5)*0.7)))*fade;
    let density = alpha + soot*(1.0-alpha);
    if density < 0.005 { discard; }
    let color = (flame*alpha+vec3<f32>(0.09,0.063,0.051)*soot*(1.0-alpha))/density;
    return vec4<f32>(color, density);
}
