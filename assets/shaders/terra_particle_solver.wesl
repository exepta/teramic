// GPU position-based fluids: persistent particles, exact linked neighborhoods,
// Jacobi density constraints, solid collision, XSPH viscosity and finite domains.
struct Params {
    shift_dt: vec4<f32>, counts: vec4<u32>,
    h: vec4<f32>, volume: vec4<f32>, frame: vec4<f32>,
};
struct Seed { position:vec4<f32>, velocity:vec4<f32>, origin_radius:vec4<f32>, tags:vec4<u32>, };
struct Particle { position:vec4<f32>, velocity:vec4<f32>, previous:vec4<f32>, tags:vec4<u32>, };
struct Scratch { density_lambda:vec4<f32>, correction:vec4<f32>, velocity:vec4<f32>, contact:vec4<f32>, };
struct Shape { minimum:vec4<f32>, maximum:vec4<f32>, tags:vec4<u32>, };
@group(0) @binding(0) var<uniform> params:Params;
@group(0) @binding(1) var<storage,read> seeds:array<Seed>;
@group(0) @binding(2) var<storage,read_write> particles:array<Particle>;
@group(0) @binding(3) var<storage,read_write> predicted:array<vec4<f32>>;
@group(0) @binding(4) var<storage,read_write> scratch:array<Scratch>;
@group(0) @binding(5) var<storage,read_write> grid:array<atomic<u32>>;
@group(0) @binding(6) var<storage,read> shapes:array<Shape>;
const EMPTY:u32=0xffffffffu;
const HEADS:u32=32768u;
fn is_live(i:u32)->bool { return i<params.counts.x && particles[i].tags.z==1u; }
fn cell(p:vec3<f32>)->vec3<i32> { return vec3<i32>(floor(p/params.frame.y)); }
fn key(p:vec3<i32>)->u32 { let c=vec3<u32>(clamp(p,vec3<i32>(0),vec3<i32>(31)));return c.x+32u*(c.y+32u*c.z); }
fn axes(kind:u32)->vec3<f32> { return vec3<f32>(params.h[kind],params.h[kind]*select(1.0,0.0625,kind==3u),params.h[kind]); }
fn kernel(d:vec3<f32>,a:vec3<f32>)->f32 {let q=dot(d/a,d/a);let x=max(1.0-q,0.0);return x*x*x;}
fn gradient(d:vec3<f32>,a:vec3<f32>)->vec3<f32> {let q=dot(d/a,d/a);let x=max(1.0-q,0.0);let scaled=d*vec3<f32>(1.0,a.x/a.y,1.0);return -6.0*x*x*scaled/(a.x*a.x);}
fn compatible(a:u32,b:u32)->bool {
    // Unequal particle volumes cannot share a number-density constraint. Water
    // and blood can blend at matching resolution; Galle/Tar remain separate.
    return a==b || (a<2u&&b<2u&&abs(params.volume[a]-params.volume[b])<params.volume[a]*0.01);
}
fn rest()->f32 {return 4.6;}
fn flow_bound(p:vec3<f32>,origin:vec3<f32>,limit:f32)->vec3<f32> {
    if limit<=0.0 {return p;}
    let base=vec2<i32>(floor(origin.xz));var owner=vec2<i32>(floor(p.xz));
    for(var step=0u;step<64u;step+=1u) {
        let d=owner-base;if abs(d.x)+abs(d.y)<=i32(floor(limit)){break;}
        if abs(d.x)>=abs(d.y) {owner.x-=select(-1,1,d.x>0);}else{owner.y-=select(-1,1,d.y>0);}
    }
    let horizontal=clamp(p.xz,vec2<f32>(owner)+vec2<f32>(0.0001),vec2<f32>(owner)+vec2<f32>(0.9999));
    return vec3<f32>(horizontal.x,p.y,horizontal.y);
}
fn solid_contact(start:vec3<f32>,candidate:vec3<f32>,lo:vec3<f32>,hi:vec3<f32>)->vec3<f32> {
    let delta=candidate-start;
    if !(all(start>lo)&&all(start<hi)) {
        // Enter through the first crossed face. A closest-face repair can push
        // water underneath a wall at a floor junction and endlessly fight the
        // floor constraint; swept contacts preserve the side of entry instead.
        var near=-1e20;var far=1e20;var axis=0u;
        for(var a=0u;a<3u;a+=1u) {
            if abs(delta[a])<0.0000001 {
                if start[a]<=lo[a]||start[a]>=hi[a] {return candidate;}
            } else {
                let t0=(lo[a]-start[a])/delta[a];let t1=(hi[a]-start[a])/delta[a];
                let entry=min(t0,t1);if entry>near {near=entry;axis=a;}
                far=min(far,max(t0,t1));
            }
        }
        if near<=far&&near>=0.0&&near<=1.0&&far>=0.0 {
            var result=candidate;
            result[axis]=start[axis]+delta[axis]*near+select(0.00001,-0.00001,delta[axis]>0.0);
            return result;
        }
        return candidate;
    }
    // Only a newly supplied/edited seed may begin inside a solid skin.
    if all(candidate>lo)&&all(candidate<hi) {
        let a=candidate-lo;let b=hi-candidate;let depth=min(a,b);
        var axis=0u;if depth.y<depth.x {axis=1u;}if depth.z<depth[axis] {axis=2u;}
        var result=candidate;result[axis]=select(hi[axis]+0.00001,lo[axis]-0.00001,a[axis]<b[axis]);return result;
    }
    return candidate;
}
fn bounded(p:vec3<f32>,start:vec3<f32>,kind:u32,i:u32)->vec3<f32> {
    let radius=vec3<f32>(0.09,select(0.09,0.0065,kind==3u),0.09);
    var q=clamp(p,radius,vec3<f32>(16.0)-radius);
    let origin=seeds[i].origin_radius.xyz;
    let limit=seeds[i].origin_radius.w;
    q=flow_bound(q,origin,limit);
    for(var s=0u;s<params.counts.y;s+=1u) {
        if (shapes[s].tags.x & (1u<<kind))==0u {continue;}
        let lo=shapes[s].minimum.xyz-radius;let hi=shapes[s].maximum.xyz+radius;
        q=solid_contact(start,q,lo,hi);
    }
    if kind==3u {
        var best=1e20;var projected=q;
        for(var j=0u;j<params.counts.z;j+=1u) {
            let s=params.counts.y+j;let lo=shapes[s].minimum.xyz;let hi=shapes[s].maximum.xyz;
            let candidate=clamp(q,lo+vec3<f32>(0.002,0.0065,0.002),hi-vec3<f32>(0.002,0.0065,0.002));
            let delta=abs(vec2<i32>(floor(candidate.xz))-vec2<i32>(floor(origin.xz)));
            if limit>0.0 && delta.x+delta.y>i32(floor(limit)) {continue;}
            let d=dot(candidate-q,candidate-q);
            if d<best {best=d;projected=candidate;}
        }
        if params.counts.z>0u {q=projected;}else{q=seeds[i].position.xyz;}
    }
    return flow_bound(q,origin,limit);
}
fn boundary_viscosity(p:vec3<f32>,velocity:vec3<f32>,kind:u32)->vec3<f32> {
    // No-slip solid coupling: a shallow sheet loses tangential momentum to the
    // supporting block. Without this boundary term a coarse particle layer is
    // a frictionless billiard table even when internal XSPH viscosity is high.
    var coupling=0.0;var normal=vec3<f32>(0.0);
    let radius=select(0.09,0.0065,kind==3u);
    for(var s=0u;s<params.counts.y;s+=1u) {
        if (shapes[s].tags.x&(1u<<kind))==0u {continue;}
        let closest=clamp(p,shapes[s].minimum.xyz,shapes[s].maximum.xyz);
        let delta=p-closest;let distance=length(delta);
        let weight=1.0-smoothstep(radius,radius+0.08,distance);
        if distance>0.00001&&weight>coupling {coupling=weight;normal=delta/distance;}
    }
    let drag=vec4<f32>(8.0,10.0,12.0,14.0)[kind];
    let tangent=velocity-normal*dot(velocity,normal);
    return velocity-tangent*min(0.15,coupling*drag*params.shift_dt.w);
}
// Contact projection is a geometric constraint, not an impulse. Accumulate it
// separately from pressure displacement so a nearest-face repair cannot launch
// a particle off a block edge or make resting water continually splash.
fn constrained(p:vec3<f32>,start:vec3<f32>,kind:u32,i:u32)->vec3<f32> {
    let q=bounded(p,start,kind,i);
    scratch[i].contact=vec4<f32>(scratch[i].contact.xyz+(q-p),0.0);
    return q;
}
@compute @workgroup_size(64)
fn reconcile(@builtin(global_invocation_id) id:vec3<u32>) {
    let i=id.x;if i>=8192u{return;}
    if params.frame.z<0.5 || i>=params.counts.x || seeds[i].tags.y>3u {particles[i].tags.z=0u;return;}
    if particles[i].tags.z==0u || particles[i].tags.x!=seeds[i].tags.x || particles[i].tags.y!=seeds[i].tags.y {
        particles[i].position=seeds[i].position;particles[i].velocity=seeds[i].velocity;
        particles[i].tags=vec4<u32>(seeds[i].tags.x,seeds[i].tags.y,1u,0u);
    } else {
        let shifted=particles[i].position.xyz+params.shift_dt.xyz;
        // A moving near window hands outgoing mass back to coarse authority.
        // The refreshed seed is a candidate in this pool's incoming region;
        // never clamp departed particles into a visible wall at the new edge.
        if any(shifted<vec3<f32>(0.0))||any(shifted>=vec3<f32>(16.0)) {
            particles[i].position=seeds[i].position;particles[i].velocity=seeds[i].velocity;
        } else {particles[i].position=vec4<f32>(shifted,1.0);}
    }
    particles[i].position=vec4<f32>(bounded(particles[i].position.xyz,particles[i].position.xyz,particles[i].tags.y,i),1.0);
}
@compute @workgroup_size(64)
fn predict(@builtin(global_invocation_id) id:vec3<u32>) {
    let i=id.x;if !is_live(i){return;}let kind=particles[i].tags.y;
    let dt=params.shift_dt.w;let damping=select(select(0.995,0.98,kind==2u),0.94,kind==3u);
    let velocity=particles[i].velocity.xyz*damping+vec3<f32>(0.0,-9.81,0.0)*dt;
    particles[i].previous=particles[i].position;
    scratch[i].contact=vec4<f32>(0.0);
    predicted[i]=vec4<f32>(constrained(particles[i].position.xyz+velocity*dt,particles[i].position.xyz,kind,i),1.0);
}
@compute @workgroup_size(64)
fn clear_grid(@builtin(global_invocation_id) id:vec3<u32>) {if id.x<HEADS {atomicStore(&grid[id.x],EMPTY);}}
@compute @workgroup_size(64)
fn build_grid(@builtin(global_invocation_id) id:vec3<u32>) {
    let i=id.x;if !is_live(i){return;}
    let head=key(cell(predicted[i].xyz));let previous=atomicExchange(&grid[head],i);atomicStore(&grid[HEADS+i],previous);
}
@compute @workgroup_size(64)
fn density(@builtin(global_invocation_id) id:vec3<u32>) {
    let i=id.x;if !is_live(i){return;}let p=predicted[i].xyz;let c=cell(p);let kind=particles[i].tags.y;let a=axes(kind);
    var rho=0.0;var sum=0.0;var gi=vec3<f32>(0.0);
    for(var z=-1;z<=1;z+=1){for(var y=-1;y<=1;y+=1){for(var x=-1;x<=1;x+=1){
        let bucket=c+vec3<i32>(x,y,z);if any(bucket<vec3<i32>(0))||any(bucket>vec3<i32>(31)){continue;}
        var j=atomicLoad(&grid[key(bucket)]);var traversed=0u;
        loop {if j==EMPTY||traversed>=8192u{break;}traversed+=1u;
            let d=p-predicted[j].xyz;
            let same=compatible(kind,particles[j].tags.y);
            if same {let w=kernel(d,a);rho+=w;if j!=i {let g=gradient(d,a)/rest();gi+=g;sum+=dot(g,g);}}
            j=atomicLoad(&grid[HEADS+j]);
        }
    }}}
    // A free surface has fewer neighbours, not negative pressure. Pulling an
    // under-filled neighbourhood back to rest density coagulates particles
    // into coincident beads and pumps energy into them on contact with a floor.
    // Viscosity transports momentum; density constraints only resist compression.
    let constraint=max(rho/rest()-1.0,0.0);
    scratch[i].density_lambda=vec4<f32>(rho,-constraint/(sum+dot(gi,gi)+0.1),0.0,0.0);
}
@compute @workgroup_size(64)
fn correction(@builtin(global_invocation_id) id:vec3<u32>) {
    let i=id.x;if !is_live(i){return;}let p=predicted[i].xyz;let c=cell(p);let kind=particles[i].tags.y;let a=axes(kind);
    var delta=vec3<f32>(0.0);
    for(var z=-1;z<=1;z+=1){for(var y=-1;y<=1;y+=1){for(var x=-1;x<=1;x+=1){
        let bucket=c+vec3<i32>(x,y,z);if any(bucket<vec3<i32>(0))||any(bucket>vec3<i32>(31)){continue;}
        var j=atomicLoad(&grid[key(bucket)]);var traversed=0u;
        loop {if j==EMPTY||traversed>=8192u{break;}traversed+=1u;
            if j!=i && (compatible(kind,particles[j].tags.y)) {
                let d=p-predicted[j].xyz;let w=kernel(d,a);
                // Positive-only density constraints do not have the tensile
                // instability that s_corr addresses. Adding artificial pair
                // repulsion here would keep doing work against the static
                // hydrostatic pressure of a settled pool.
                delta+=(scratch[i].density_lambda.y+scratch[j].density_lambda.y)*gradient(d,a)/rest();
            }
            j=atomicLoad(&grid[HEADS+j]);
        }
    }}}
    if kind==3u {delta.y*=0.0625;}
    let limit=vec3<f32>(0.075,select(0.075,0.004,kind==3u),0.075);
    scratch[i].correction=vec4<f32>(clamp(delta,-limit,limit),0.0);
}
@compute @workgroup_size(64)
fn apply_correction(@builtin(global_invocation_id) id:vec3<u32>) {
    let i=id.x;if !is_live(i){return;}predicted[i]=vec4<f32>(constrained(predicted[i].xyz+scratch[i].correction.xyz,predicted[i].xyz,particles[i].tags.y,i),1.0);
}
@compute @workgroup_size(64)
fn velocity(@builtin(global_invocation_id) id:vec3<u32>) {
    let i=id.x;if !is_live(i){return;}
    let contact=scratch[i].contact.xyz;
    var velocity=(predicted[i].xyz-particles[i].previous.xyz-contact)/max(params.shift_dt.w,0.0001);
    for(var axis=0u;axis<3u;axis+=1u) {
        if contact[axis]*velocity[axis]<0.0 {velocity[axis]=0.0;}
    }
    scratch[i].velocity=vec4<f32>(velocity,0.0);
}
@compute @workgroup_size(64)
fn commit(@builtin(global_invocation_id) id:vec3<u32>) {
    let i=id.x;if !is_live(i){return;}let p=predicted[i].xyz;let c=cell(p);let kind=particles[i].tags.y;let a=axes(kind);
    let original=scratch[i].velocity.xyz;var viscous=vec3<f32>(0.0);var weight=0.0;
    var cohesion=vec3<f32>(0.0);var cohesion_weight=0.0;
    for(var z=-1;z<=1;z+=1){for(var y=-1;y<=1;y+=1){for(var x=-1;x<=1;x+=1){
        let bucket=c+vec3<i32>(x,y,z);if any(bucket<vec3<i32>(0))||any(bucket>vec3<i32>(31)){continue;}
        var j=atomicLoad(&grid[key(bucket)]);var traversed=0u;
        loop {if j==EMPTY||traversed>=8192u{break;}traversed+=1u;
            if j!=i && kind==particles[j].tags.y {
                let delta=p-predicted[j].xyz;let w=kernel(delta,a);
                viscous+=(scratch[j].velocity.xyz-original)*w;weight+=w;
                // Bounded pair attraction is a physical acceleration, never a
                // density projection. It cannot turn tiny frame times into an
                // unbounded launch velocity or collapse coincident particles.
                let q=length(delta/a);
                let attraction=smoothstep(0.45,0.8,q)*(1.0-smoothstep(0.8,1.0,q));
                cohesion-=delta/max(length(delta),0.0001)*attraction;
                cohesion_weight+=attraction;
            }
            j=atomicLoad(&grid[HEADS+j]);
        }
    }}}
    let viscosity=vec4<f32>(0.25,0.30,0.50,0.75)[kind];
    // Capillarity acts at the free surface. Bulk particles do not receive a
    // normalised several-m/s² attraction on every step of an otherwise calm pool.
    let surface=clamp((rest()-scratch[i].density_lambda.x)/(rest()-1.0),0.0,1.0);
    let tension=vec4<f32>(0.0,0.10,1.2,0.8)[kind]*surface;
    let coupled=original+viscous/max(weight,1.0)*viscosity
        +cohesion/max(cohesion_weight,1.0)*tension*params.shift_dt.w;
    let v=boundary_viscosity(p,coupled,kind);
    let maximum=vec4<f32>(16.0,13.0,6.0,2.0)[kind];
    particles[i].velocity=vec4<f32>(v*min(1.0,maximum/max(length(v),0.0001)),0.0);
    particles[i].position=predicted[i];
    particles[i].previous=seeds[i].origin_radius;
}
