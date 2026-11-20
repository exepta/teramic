// Reconstruct a continuous density field from the simulated particles.
// Integer CAS accumulation is conservative up to the visual saturation point;
// the pressure solver itself always uses the full, unclipped particle density.
struct Params { shift_dt:vec4<f32>,counts:vec4<u32>,h:vec4<f32>,volume:vec4<f32>,frame:vec4<f32>, };
struct Particle {position:vec4<f32>,velocity:vec4<f32>,previous:vec4<f32>,tags:vec4<u32>,};
@group(0) @binding(0) var<uniform> params:Params;
@group(0) @binding(1) var<storage,read> particles:array<Particle>;
@group(0) @binding(2) var<storage,read_write> field:array<atomic<u32>>;
@group(0) @binding(3) var<storage,read_write> tar:array<atomic<u32>>;
@group(0) @binding(4) var density_image:texture_storage_3d<rgba16float,write>;
@group(0) @binding(5) var tar_image:texture_storage_2d_array<rgba16float,write>;
const SIZE:u32=128u;
const VOXEL:f32=0.125;
@compute @workgroup_size(64)
fn clear(@builtin(global_invocation_id) id:vec3<u32>) {
    let i=id.x;if i<SIZE*SIZE*SIZE {atomicStore(&field[i],0u);}
    if i<SIZE*SIZE*16u {atomicStore(&tar[4u*i],0u);atomicStore(&tar[4u*i+1u],0u);atomicStore(&tar[4u*i+2u],0xffffffffu);}
}
fn permitted(p:vec3<f32>,origin:vec4<f32>)->bool {
    let delta=abs(vec2<i32>(floor(p.xz))-vec2<i32>(floor(origin.xz)));
    return origin.w<=0.0 || delta.x+delta.y<=i32(floor(origin.w));
}
fn add(index:u32,kind:u32,amount:u32) {
    let shift=kind*8u;let mask=255u<<shift;
    var previous=atomicLoad(&field[index]);
    loop {
        let value=min(255u,((previous>>shift)&255u)+amount);
        let next=(previous&~mask)|(value<<shift);
        let exchange=atomicCompareExchangeWeak(&field[index],previous,next);
        if exchange.exchanged {break;}previous=exchange.old_value;
    }
}
fn add_motion(index:u32,amount:u32) {
    // Tar has its own film image, leaving the high byte free. Store a local
    // maximum, rather than a sum: overlapping quiet particles must not invent
    // activity and moving droplets must remain visible at their true position.
    var previous=atomicLoad(&field[index]);
    loop {
        let next=(previous&0x00ffffffu)|(max(previous>>24u,amount)<<24u);
        let exchange=atomicCompareExchangeWeak(&field[index],previous,next);
        if exchange.exchanged {break;}previous=exchange.old_value;
    }
}
@compute @workgroup_size(64)
fn splat(@builtin(global_invocation_id) id:vec3<u32>) {
    let i=id.x;if i>=params.counts.x||particles[i].tags.z==0u{return;}
    let kind=particles[i].tags.y;let p=particles[i].position.xyz;
    let motion=u32(round(smoothstep(0.1,0.6,length(particles[i].velocity.xyz))*255.0));
    let radius=max(params.h[kind]*0.8,0.28);let center=vec3<i32>(floor(p/VOXEL));
    let bound=i32(ceil(radius/VOXEL));
    if kind==3u {
        for(var z=-bound;z<=bound;z+=1){for(var x=-bound;x<=bound;x+=1){
            let c=center.xz+vec2<i32>(x,z);if any(c<vec2<i32>(0))||any(c>=vec2<i32>(128)){continue;}
            let d=(vec2<f32>(c)+vec2<f32>(0.5))*VOXEL-p.xz;
            let location=vec3<f32>((f32(c.x)+0.5)*VOXEL,p.y,(f32(c.y)+0.5)*VOXEL);
            let owner=vec3<u32>(clamp(floor(location),vec3<f32>(0.0),vec3<f32>(15.0)));
            if (atomicLoad(&field[SIZE*SIZE*SIZE+owner.x+16u*(owner.y+16u*owner.z)])&5u)!=5u || !permitted(location,particles[i].previous) {continue;}
            let q=dot(d,d)/(radius*radius);if q>=1.0{continue;}
            let w=1.0-q;let weight=u32(round(w*w*w*64.0));let index=u32(c.x)+SIZE*(u32(c.y)+SIZE*u32(clamp(floor(p.y),0.0,15.0)));
            atomicAdd(&tar[index*4u],weight);
            atomicMax(&tar[index*4u+1u],u32(clamp(p.y+0.007,0.0,16.0)*4096.0));
            atomicMin(&tar[index*4u+2u],u32(clamp(p.y-0.007,0.0,16.0)*4096.0));
        }}
        return;
    }
    for(var z=-bound;z<=bound;z+=1){for(var y=-bound;y<=bound;y+=1){for(var x=-bound;x<=bound;x+=1){
        let c=center+vec3<i32>(x,y,z);if any(c<vec3<i32>(0))||any(c>=vec3<i32>(128)){continue;}
        let d=(vec3<f32>(c)+vec3<f32>(0.5))*VOXEL-p;let q=dot(d,d)/(radius*radius);
        if q>=1.0{continue;}
        if kind==2u && !permitted((vec3<f32>(c)+vec3<f32>(0.5))*VOXEL,particles[i].previous) {continue;}
        let w=1.0-q;let weight=u32(round(w*w*w*64.0));
        if weight>0u {
            let index=u32(c.x)+SIZE*(u32(c.y)+SIZE*u32(c.z));
            add(index,kind,weight);
            if motion>0u {add_motion(index,motion);}
        }
    }}}
}
@compute @workgroup_size(4,4,4)
fn pack(@builtin(global_invocation_id) id:vec3<u32>) {
    if any(id>=vec3<u32>(SIZE)){return;}
    let value=atomicLoad(&field[id.x+SIZE*(id.y+SIZE*id.z)]);
    let owner=id/8u;let legality=atomicLoad(&field[SIZE*SIZE*SIZE+owner.x+16u*(owner.y+16u*owner.z)]);
    // The integer part remains the exact cell legality mask. The fractional
    // quarter stores kinetic activity; round(alpha) still decodes legality.
    // Decode activity from an exact texel as fract(alpha)*4, never from a
    // filtered alpha that may straddle two different integer legality masks.
    let mask_motion=f32(legality)+f32(value>>24u)/1024.0;
    textureStore(density_image,vec3<i32>(id),vec4<f32>(f32(value&255u)/64.0,f32((value>>8u)&255u)/64.0,f32((value>>16u)&255u)/64.0,mask_motion));
    if id.z<16u {
        let index=(id.x+SIZE*(id.y+SIZE*id.z))*4u;let density=atomicLoad(&tar[index]);
        var result=vec4<f32>(0.0);
        if density>0u {result=vec4<f32>(f32(atomicLoad(&tar[index+1u]))/4096.0,f32(atomicLoad(&tar[index+2u]))/4096.0,f32(density)/64.0,1.0);}
        textureStore(tar_image,vec2<i32>(id.xy),i32(id.z),result);
    }
}
