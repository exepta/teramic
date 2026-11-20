// Smooth, freely shaped SPH density isosurface. Proxy box faces are invisible.
import bevy_pbr::render::mesh_functions;
import package::shaders::terra_fluid_coverage::particle_surface_pixel;

import bevy_pbr::render::mesh_view_bindings::view;
import bevy_pbr::render::view_transformations::position_world_to_clip;
@if(DEPTH_PREPASS)
import bevy_pbr::prepass::utils::prepass_depth;
@if(DEPTH_PREPASS)
import bevy_pbr::render::view_transformations::position_ndc_to_world;
@group(constants::MATERIAL_BIND_GROUP) @binding(0) var density_image:texture_3d<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(1) var density_sampler:sampler;
@group(constants::MATERIAL_BIND_GROUP) @binding(2) var tar_image:texture_2d_array<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(3) var tar_sampler:sampler;
@group(constants::MATERIAL_BIND_GROUP) @binding(4) var<uniform> domain:vec4<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(5) var<uniform> settings:vec4<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(6) var rest_image:texture_3d<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(7) var rest_sampler:sampler;
@group(constants::MATERIAL_BIND_GROUP) @binding(8) var rest_tar_image:texture_2d_array<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(9) var rest_tar_sampler:sampler;
@group(constants::MATERIAL_BIND_GROUP) @binding(10) var<uniform> rest_origin:vec4<f32>;
struct Vertex {@builtin(instance_index) instance_index:u32,@location(0)position:vec3<f32>,@location(1)normal:vec3<f32>,};
struct Input {@builtin(position)position:vec4<f32>,@location(0)world:vec3<f32>,@location(1)normal:vec3<f32>,};
struct Output {@location(0)color:vec4<f32>,@builtin(frag_depth)depth:f32,};
@vertex fn vertex(input:Vertex)->Input {
    let model=mesh_functions::get_world_from_local(input.instance_index);
    let world=mesh_functions::mesh_position_local_to_world(model,vec4<f32>(input.position,1.0));
    return Input(position_world_to_clip(world.xyz),world.xyz,normalize((model*vec4<f32>(input.normal,0.0)).xyz));
}
fn tar_values(p:vec3<f32>)->vec4<f32> {
    if settings.y<0.5 {return vec4<f32>(0.0); }
    let uv=clamp(p.xz/16.0,vec2<f32>(0.0001),vec2<f32>(0.9999));
    let layer=i32(floor(p.y));var nearest=1e20;var result=vec4<f32>(0.0);
    for(var i=-1;i<=1;i+=1) {
        let l=layer+i;if l<0||l>=16 {continue;}
        let horizontal=vec2<i32>(clamp(floor(p.xz/0.125),vec2<f32>(0.0),vec2<f32>(127.0)));
        let mask_position=vec3<i32>(horizontal.x,l*8+7,horizontal.y);
        let legality=u32(round(textureLoad(density_image,mask_position,0).w));
        if (legality&5u)!=5u {continue;}
        var candidate=textureSampleLevel(tar_image,tar_sampler,uv,l,0.0);
        candidate=vec4<f32>(candidate.xy/max(candidate.w,0.00001),candidate.zw);
        let distance=abs(p.y-clamp(p.y,candidate.y,candidate.x));
        if candidate.z>0.0&&distance<nearest {nearest=distance;result=candidate;}
    }
    return result;
}
fn rest_tar_values(p:vec3<f32>)->vec4<f32> {
    let uv=clamp((p.xz+vec2<f32>(0.5))/17.0,vec2<f32>(0.0),vec2<f32>(1.0));
    let cell=vec2<i32>(clamp(floor(p.xz),vec2<f32>(0.0),vec2<f32>(15.0)));
    var result=vec4<f32>(0.0);var nearest=1e20;
    for(var i=-1;i<=1;i+=1) {
        let layer=i32(floor(p.y))+i;if layer<0||layer>=16 {continue;}
        // Legality is a cell mask; interpolating it would coat forbidden soil.
        if textureLoad(rest_tar_image,cell,layer,0).z<0.5 {continue;}
        let t=textureSampleLevel(rest_tar_image,rest_tar_sampler,uv,layer,0.0);
        let profile=t.xy/max(t.w,0.00001);
        let distance=abs(p.y-clamp(p.y,profile.y,profile.x));
        if distance<nearest {nearest=distance;result=vec4<f32>(profile,1.0,1.0);}
    }
    return result;
}
fn activity(p:vec3<f32>)->f32 {
    // Decode before filtering: the integer part is the exact per-cell legality
    // mask, while its fractional part stores local maximum kinetic activity.
    let cell=vec3<i32>(clamp(floor(p/0.125),vec3<f32>(0.0),vec3<f32>(127.0)));
    return clamp(fract(textureLoad(density_image,cell,0).w)*4.0,0.0,1.0);
}
fn rest_values(p:vec3<f32>)->vec4<f32> {
    return textureSampleLevel(rest_image,rest_sampler,clamp((p+vec3<f32>(0.5))/17.0,vec3<f32>(0.0),vec3<f32>(1.0)),0.0);
}
fn values(p:vec3<f32>)->vec4<f32> {
    let uv=clamp(p/16.0,vec3<f32>(0.0001),vec3<f32>(0.9999));
    var d=textureSampleLevel(density_image,density_sampler,uv,0.0);
    if settings.z>0.5 {
        let mask_position=vec3<i32>(clamp(floor(p/0.125),vec3<f32>(0.0),vec3<f32>(127.0)));
        if (u32(round(textureLoad(density_image,mask_position,0).w))&2u)==0u {d.z=0.0;}
    }
    let t=tar_values(p);
    let vertical=smoothstep(t.y-0.003,t.y+0.003,p.y)*(1.0-smoothstep(t.x-0.003,t.x+0.003,p.y));
    d.w=t.z*vertical;
    if rest_origin.w>0.5 {
        let edge=min(min(p.x,16.0-p.x),min(p.z,16.0-p.z));
        let boundary=1.0-smoothstep(0.0,1.25,edge);
        let signed=rest_values(p);
        var resting=max(vec4<f32>(0.0),signed+vec4<f32>(0.55));
        if boundary>0.0&&settings.y>0.5 {
            let film=rest_tar_values(p);
            resting.w=film.z*smoothstep(film.y-0.003,film.y+0.003,p.y)*(1.0-smoothstep(film.x-0.003,film.x+0.003,p.y));
        }
        // A coarsened particle lattice represents volume, not visible spheres.
        // Reconstruct a calm supported reservoir from its same signed field.
        // Require BOTH real particles and a filled rest column below it: empty
        // regions, thin falling sheets and a droplet at its apex remain SPH.
        let lower=p-vec3<f32>(0.0,0.5,0.0);
        let deeper=p-vec3<f32>(0.0,0.9,0.0);
        let filled=smoothstep(vec3<f32>(0.10),vec3<f32>(0.35),rest_values(deeper).xyz);
        let nearby=vec3<f32>(1.0)-smoothstep(vec3<f32>(0.15),vec3<f32>(0.45),-signed.xyz);
        var calm=vec3<f32>(0.0);
        if max(filled.x,max(filled.y,filled.z))>0.0&&max(nearby.x,max(nearby.y,nearby.z))>0.0 {
            let a=textureSampleLevel(density_image,density_sampler,clamp(lower/16.0,vec3<f32>(0.0),vec3<f32>(1.0)),0.0).xyz;
            let b=textureSampleLevel(density_image,density_sampler,clamp(deeper/16.0,vec3<f32>(0.0),vec3<f32>(1.0)),0.0).xyz;
            // Coarsened particles can sit deeper after a domain rebase. Probe
            // the actual filled column rather than treating sparse top samples
            // as empty volume. The rest-column guard still excludes thin drops.
            let deep_support=textureSampleLevel(density_image,density_sampler,clamp((p-vec3<f32>(0.0,1.3,0.0))/16.0,vec3<f32>(0.0),vec3<f32>(1.0)),0.0).xyz;
            let supported=smoothstep(vec3<f32>(0.18),vec3<f32>(0.65),max(max(a,b),deep_support));
            // Motion below the interface establishes support, not surface
            // deformation: blending an empty top sample for a submerged moving
            // particle would punch a false hole in an otherwise still pool.
            let moving=activity(p);
            calm=supported*filled*nearby*(1.0-moving);
        }
        let weight=vec4<f32>(max(vec3<f32>(boundary),calm),boundary);
        d=mix(d,resting,weight);
    }
    return d;
}
fn density(p:vec3<f32>)->f32 {let d=values(p);return max(max(d.x,d.y),max(d.z,d.w));}
fn bounds(origin:vec3<f32>,ray:vec3<f32>)->vec2<f32> {
    let inverse=select(vec3<f32>(-1.0),vec3<f32>(1.0),ray>=vec3<f32>(0.0))/max(abs(ray),vec3<f32>(0.00001));
    let a=(vec3<f32>(0.0)-origin)*inverse;let b=(vec3<f32>(16.0)-origin)*inverse;
    let near=min(a,b);let far=max(a,b);return vec2<f32>(max(0.0,max(near.x,max(near.y,near.z))),min(far.x,min(far.y,far.z)));
}
fn shade_surface(normal:vec3<f32>,ray:vec3<f32>,kind:u32,edge:f32,optical_column:f32)->vec4<f32> {
    let light=normalize(vec3<f32>(0.4,0.83,0.3));let halfway=normalize(light-ray);
    let daylight=settings.x;
    let diffuse=(0.22+0.78*max(dot(normal,light),0.0))*daylight;
    let colors=array<vec3<f32>,4>(vec3<f32>(0.035,0.28,0.38),vec3<f32>(0.42,0.014,0.036),vec3<f32>(0.31,0.39,0.015),vec3<f32>(0.023,0.019,0.014));
    let fresnel=0.025+0.975*pow(1.0-max(dot(normal,-ray),0.0),5.0);
    let sky=mix(vec3<f32>(0.10,0.19,0.28),vec3<f32>(0.57,0.76,0.91),max(normal.y,0.0))*daylight;
    let exponent=select(180.0,48.0,kind>=2u);
    let specular=pow(max(dot(normal,halfway),0.0),exponent)*daylight;
    let curvature=clamp(edge*0.09,0.0,0.18);
    var color=colors[kind]*diffuse+sky*fresnel*0.72+vec3<f32>(specular)*0.65;
    if kind==0u {color+=vec3<f32>(0.52,0.64,0.63)*curvature*0.15;}
    var alpha=vec4<f32>(0.76,0.91,0.94,1.0)[kind];
    if kind==0u {
        // Clear water transmits the actual scene beneath it. Estimate the
        // optical column from this same free surface, so a thin running sheet
        // stays clear while deep pools gain color; droplets no longer shade as
        // opaque plastic balls. Reflection remains strongest at grazing angles.
        let column=optical_column;
        let absorption=1.0-exp(-column*0.85);
        alpha=mix(0.10+absorption*0.52,0.93,fresnel);
        color=colors[0]*(0.25+0.35*max(dot(normal,light),0.0))*daylight
            +sky*fresnel+vec3<f32>(specular)*0.88;
    }
    return vec4<f32>(color,alpha);
}
@fragment fn fragment(input:Input,@builtin(front_facing) front:bool)->Output {
    if settings.w>0.5 {
        // Confined machinery is an explicit volume, independent of the
        // generic terrain capture. It must remain visible inside that domain.
        if settings.w<4.5&&particle_surface_pixel(input.world,domain,input.position.xy) {discard;}
        let ray=normalize(input.world-view.world_position);
        let outward=normalize(input.normal);
        let normal=select(-outward,outward,dot(outward,-ray)>=0.0);
        let kind=(u32(settings.w)-1u)%4u;
        let clip=position_world_to_clip(input.world);
        return Output(shade_surface(normal,ray,kind,0.0,0.65),clamp(clip.z/clip.w,0.000001,1.0));
    }
    if domain.w<0.5||front {discard;}
    let eye=view.world_position;let ray=normalize(input.world-eye);let relative=eye-domain.xyz;
    var interval=bounds(relative,ray);
@if(DEPTH_PREPASS) {
    let depth=prepass_depth(input.position,0u);
    if depth>0.0 {
        let uv=(input.position.xy-view.viewport.xy)/view.viewport.zw;
        let ndc=uv*vec2<f32>(2.0,-2.0)+vec2<f32>(-1.0,1.0);
        let ground=position_ndc_to_world(vec3<f32>(ndc,depth));
        interval.y=min(interval.y,max(0.0,dot(ground-eye,ray)));
    }
}
    if interval.y<=interval.x {discard;}
    let threshold=0.55;
    var distance=interval.x+0.0005;var previous=distance;
    let inside=density(relative+ray*distance)>=threshold;
    var found=false;
    for(var step=0u;step<384u;step+=1u) {
        let p=relative+ray*distance;let d=density(p);
        if (d>=threshold)!=inside {found=true;break;}
        previous=distance;
        var advance=select(0.19,0.045,d>0.02);
        let tar=select(tar_values(p),rest_tar_values(p),rest_origin.w>0.5&&min(min(p.x,16.0-p.x),min(p.z,16.0-p.z))<0.3);
        if tar.z>0.08 {let vertical=abs(p.y-clamp(p.y,tar.y,tar.x));advance=min(advance,max(0.006,vertical*0.6));}
        distance+=advance;if distance>interval.y {break;}
    }
    if !found {discard;}
    var low=previous;var high=distance;
    for(var refine=0u;refine<6u;refine+=1u) {
        let middle=(low+high)*0.5;
        if (density(relative+ray*middle)>=threshold)==inside {low=middle;}else{high=middle;}
    }
    distance=(low+high)*0.5;let p=relative+ray*distance;let d=values(p);
    var kind=0u;for(var k=1u;k<4u;k+=1u){if d[k]>d[kind]{kind=k;}}
    let epsilon=select(0.06,0.003,kind==3u);
    let gradient=vec3<f32>(density(p+vec3<f32>(epsilon,0.0,0.0))-density(p-vec3<f32>(epsilon,0.0,0.0)),density(p+vec3<f32>(0.0,epsilon,0.0))-density(p-vec3<f32>(0.0,epsilon,0.0)),density(p+vec3<f32>(0.0,0.0,epsilon))-density(p-vec3<f32>(0.0,0.0,epsilon)));
    var normal=-gradient/max(length(gradient),0.00001);if inside {normal=-normal;}
    if length(normal)<0.5 {normal=-ray;}
    let column=0.10+0.16*min(density(p+ray*0.12),1.0)+0.28*min(density(p+ray*0.36),1.0)+0.44*min(density(p+ray*0.72),1.0);
    let color=shade_surface(normal,ray,kind,length(gradient),column);
    let clip=position_world_to_clip(eye+ray*distance);
    return Output(color,clamp(clip.z/clip.w,0.000001,1.0));
}
