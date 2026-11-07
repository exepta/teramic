// Native, bounded hydraulic models; block definitions are owned by the registry.
import fs from 'node:fs';
import path from 'node:path';

const directory = path.resolve(import.meta.dirname, '../../assets/terra/blocks/models');
const selected = new Set(process.argv.slice(2));
const tau = Math.PI * 2;
const palette = [
  ['oak_frame', [.77,.58,.34,1], 0, 'machinery-oak-hd.png'],
  ['pipe_stone', [.56,.59,.61,1], 0, 'stone.png'],
  ['pipe_iron', [.43,.49,.54,1], .75, 'machinery-iron-hd.png'],
  ['brass_flanges', [.87,.61,.22,1], .67, 'machinery-iron-hd.png'],
  ['iron_bolts', [.76,.80,.84,1], .8, 'machinery-iron-hd.png'],
  ['valve_red', [.63,.055,.025,1], .4, 'machinery-iron-hd.png'],
  ['pump_glass', [.64,.86,.93,.26], .05, null],
  ['hydraulic_drive', [.39,.43,.46,1], .8, 'machinery-iron-hd.png'],
  ['pipe_glass', [.74,.91,.96,.16], 0, null],
];

// Unchanged definitions are compile-time inputs; preserve their timestamps.
function writeChanged(file, contents) {
  if(fs.existsSync(file)&&fs.readFileSync(file,'utf8')===contents)return;
  fs.writeFileSync(file,contents);
}

function model(pipeUVs=false) {
  const groups = new Map();
  const group = (material, part='body') => {
    const key=`${part}:${material}`;
    if(!groups.has(key))groups.set(key,{material,part,p:[],n:[],uv:[],t:[],i:[]});
    return groups.get(key);
  };
  function face(points, normal, material, part, sweepUVs=null) {
    const g=group(material,part),base=g.p.length/3,length=Math.hypot(...normal),n=normal.map(v=>v/length);
    const u=points[1].map((v,i)=>v-points[0][i]),v=points[2].map((v,i)=>v-points[0][i]);
    const cross=[u[1]*v[2]-u[2]*v[1],u[2]*v[0]-u[0]*v[2],u[0]*v[1]-u[1]*v[0]];
    const reverse=cross.reduce((s,v,i)=>s+v*n[i],0)<0;
    const axis=n.map(Math.abs).indexOf(Math.max(...n.map(Math.abs)));
    const uvs=sweepUVs??points.map(p=>[p[(axis+1)%3]*2,p[(axis+2)%3]*2]);
    let tangent;
    if(pipeUVs) {
      const a=uvs[1].map((v,i)=>v-uvs[0][i]),b=uvs[2].map((v,i)=>v-uvs[0][i]),det=a[0]*b[1]-a[1]*b[0];
      if(Math.abs(det)<1e-12)throw Error('collapsed pipe UV');
      const du=u.map((x,i)=>(x*b[1]-v[i]*a[1])/det),dv=v.map((x,i)=>(x*a[0]-u[i]*b[0])/det);
      const dot=du.reduce((s,x,i)=>s+x*n[i],0),projected=du.map((x,i)=>x-n[i]*dot),len=Math.hypot(...projected),t=projected.map(x=>x/len);
      const bitangent=[n[1]*t[2]-n[2]*t[1],n[2]*t[0]-n[0]*t[2],n[0]*t[1]-n[1]*t[0]];
      tangent=[...t,bitangent.reduce((s,x,i)=>s+x*dv[i],0)<0?-1:1];
    }
    for(let i=0;i<points.length;i++){g.p.push(...points[i]);g.n.push(...n);g.uv.push(...uvs[i]);if(tangent)g.t.push(...tangent);}
    for(let i=1;i+1<points.length;i++)g.i.push(base,base+(reverse?i+1:i),base+(reverse?i:i+1));
  }
  function box(c,size,material,part='body') {
    for(let axis=0;axis<3;axis++)for(const sign of[-1,1]) {
      const u=(axis+1)%3,v=(axis+2)%3,n=[0,0,0];n[axis]=sign;
      face([[-1,-1],[1,-1],[1,1],[-1,1]].map(([a,b])=>{
        const p=c.slice();p[axis]+=size[axis]*sign/2;p[u]+=size[u]*a/2;p[v]+=size[v]*b/2;return p;
      }),n,material,part);
    }
  }
  function tube(axis,start,end,radius,thickness,material,part='body',center=[.5,.5,.5],sides=8) {
    const u=(axis+1)%3,v=(axis+2)%3;
    const point=(t,r,a)=>{const p=center.slice();p[axis]=t;p[u]+=Math.cos(a)*r;p[v]+=Math.sin(a)*r;return p;};
    for(let side=0;side<sides;side++) {
      const a=side*tau/sides,b=(side+1)*tau/sides,mid=(a+b)/2,n=[0,0,0];n[u]=Math.cos(mid);n[v]=Math.sin(mid);
      const ua=side*2/sides,ub=(side+1)*2/sides;
      face([point(start,radius,a),point(end,radius,a),point(end,radius,b),point(start,radius,b)],n,material,part,pipeUVs?[[start*2,ua],[end*2,ua],[end*2,ub],[start*2,ub]]:null);
      face([point(start,radius-thickness,a),point(start,radius-thickness,b),point(end,radius-thickness,b),point(end,radius-thickness,a)],n.map(v=>-v),material,part,pipeUVs?[[start*2,ua],[start*2,ub],[end*2,ub],[end*2,ua]]:null);
      for(const[t,sign]of[[start,-1],[end,1]]){const normal=[0,0,0];normal[axis]=sign;face([point(t,radius,a),point(t,radius,b),point(t,radius-thickness,b),point(t,radius-thickness,a)],normal,material,part);}
    }
  }
  function flange(axis,t,part='body',radius=.215,bore=radius-.063) {
    tube(axis,t-.035,t+.035,radius,radius-bore,3,part);
    for(let bolt=0;bolt<4;bolt++) {
      const a=(bolt+.5)*tau/4,c=[.5,.5,.5],size=[.043,.043,.043];
      c[axis]=t+(t<.5?.04:-.04);c[(axis+1)%3]+=Math.cos(a)*radius*.82;c[(axis+2)%3]+=Math.sin(a)*radius*.82;
      size[axis]=.027;box(c,size,4,part);
    }
  }
  function save(name,id,label,collision) {
    if(selected.size&&!selected.has(name))return;
    const buffers=[],views=[],accessors=[],primitives=[];let offset=0,total=0;
    function accessor(data,width,index=false){
      const bytes=Buffer.from((index?new Uint32Array(data):new Float32Array(data)).buffer),view=views.length;
      views.push({buffer:0,byteOffset:offset,byteLength:bytes.length,target:index?34963:34962});buffers.push(bytes);offset+=bytes.length;
      const result={bufferView:view,componentType:index?5125:5126,count:data.length/width,type:{1:'SCALAR',2:'VEC2',3:'VEC3',4:'VEC4'}[width]};
      if(width===3){result.min=[0,1,2].map(i=>Math.min(...data.filter((_,j)=>j%3===i)));result.max=[0,1,2].map(i=>Math.max(...data.filter((_,j)=>j%3===i)));}
      accessors.push(result);return accessors.length-1;
    }
    for(const g of groups.values()) {
      total+=g.p.length/3;
      if([...g.p,...g.n,...g.uv].some(v=>!Number.isFinite(v))||g.p.some(v=>v<-.00001||v>1.00001))throw Error(`${name} invalid geometry`);
      for(let i=0;i<g.i.length;i+=3){
        const[a,b,c]=g.i.slice(i,i+3).map(v=>v*3),u=[0,1,2].map(j=>g.p[b+j]-g.p[a+j]),v=[0,1,2].map(j=>g.p[c+j]-g.p[a+j]);
        const n=[u[1]*v[2]-u[2]*v[1],u[2]*v[0]-u[0]*v[2],u[0]*v[1]-u[1]*v[0]];
        if(n.reduce((sum,v,j)=>sum+v*g.n[a+j],0)<1e-10)throw Error(`${name} reversed/empty triangle`);
      }
      primitives.push({attributes:{POSITION:accessor(g.p,3),NORMAL:accessor(g.n,3),TEXCOORD_0:accessor(g.uv,2),...(pipeUVs?{TANGENT:accessor(g.t,4)}:{})},indices:accessor(g.i,1,true),material:g.material,extras:{hydraulic_part:g.part}});
    }
    if(total>10000)throw Error(`${name} detail budget exceeded: ${total}`);
    const images=[],textures=[];
    const exportPalette=name==='glass_pipe'?palette:palette.filter(([material])=>material!=='pipe_glass');
    const materials=exportPalette.map(([name,baseColorFactor,metallicFactor,texture])=>{
      const pbrMetallicRoughness={baseColorFactor,metallicFactor,roughnessFactor:name==='pipe_glass'?.12:metallicFactor?.56:.88};
      if(texture){const index=images.length;images.push({uri:`../textures/${texture}`});textures.push({source:index});pbrMetallicRoughness.baseColorTexture={index};}
      return{name,pbrMetallicRoughness,...(['pump_glass','pipe_glass'].includes(name)?{alphaMode:'BLEND',doubleSided:name==='pump_glass'}:{})};
    });
    const gltf={asset:{version:'2.0',generator:'TERAMIC bounded hydraulic builder'},scene:0,scenes:[{nodes:[0]}],nodes:[{mesh:0}],meshes:[{primitives}],materials,images,textures,buffers:[{byteLength:offset,uri:'data:application/octet-stream;base64,'+Buffer.concat(buffers).toString('base64')}],bufferViews:views,accessors};
    if(name==='basin') {
      const replacements={'machinery-oak-hd.png':'oak-crafted-hd.png','stone.png':'stone-strata-hd.png','machinery-iron-hd.png':'forged-iron-warm.png'};
      for(const image of gltf.images) for(const[from,to] of Object.entries(replacements)) image.uri=image.uri.replace(from,to);
    }
    writeChanged(path.join(directory,`${name}.gltf`),JSON.stringify(gltf)+'\n');
    const used=new Set(primitives.map(p=>p.material));
    const textureBindings=Object.fromEntries(exportPalette.filter((entry,i)=>used.has(i)&&entry[3]).map(([name,,,texture])=>[name,`textures/${texture}`]));
    writeChanged(path.join(directory,'..',`${name}.json`),JSON.stringify({schema_version:1,id,name:label,description:`A reinforced ${label.toLowerCase()} for the finite hydraulic network.`,creative_category:'technic',render_kind:'custom_mesh',struct_block:true,model_layout:'scene',model:`models/${name}.gltf`,hardness:id===172?.9:id===163?2:3,resistance:id===172?2:8,mining_level:0,required_tool:id===163||id===169?'axe':'pickaxe',textures:name==='basin'?Object.fromEntries(Object.entries(textureBindings).map(([key,value])=>[key,value.replace('machinery-oak-hd.png','oak-crafted-hd.png').replace('stone.png','stone-strata-hd.png').replace('machinery-iron-hd.png','forged-iron-warm.png')])):textureBindings,collision_boxes:collision.map(([minimum,maximum])=>({minimum,maximum}))},null,2)+'\n');
    console.log(`${name}: ${total} vertices / ${primitives.length} primitives / ${new Set(primitives.map(p=>p.material)).size} shared PBR materials`);
  }
  function rotateY(){for(const g of groups.values())for(let i=0;i<g.p.length;i+=3){const x=g.p[i],nx=g.n[i];g.p[i]=g.p[i+2];g.p[i+2]=1-x;g.n[i]=g.n[i+2];g.n[i+2]=-nx;}}
  return{box,tube,flange,save,rotateY};
}

for(const[name,material,id,label]of[['wood_pipe',0,163,'Wood Pipe'],['stone_pipe',1,164,'Stone Pipe'],['iron_pipe',2,165,'Iron Pipe'],['glass_pipe',8,172,'Glass Pipe']]) {
  const m=model(true);m.box([.5,.5,.5],[.30,.30,.30],material,'core');
  for(let side=0;side<6;side++) {
    const axis=Math.floor(side/2),positive=side%2===1,part=`arm_${side}`;
    m.tube(axis,positive?.64:0,positive?1:.36,.15,.032,material,part);
    m.flange(axis,positive?.955:.045,part,.215,.15);
    // Keep the support collar on the straight arm, outside the bend's .25 radius.
    m.tube(axis,positive?.75:.20,positive?.80:.25,.163,.04,3,part);
  }
  m.save(name,id,label,[[[0,.285,.285],[1,.715,.715]],[[.285,0,.285],[.715,1,.715]],[[.285,.285,0],[.715,.715,1]]]);
}

{
  const m=model();m.tube(2,0,1,.155,.037,2);m.flange(2,.055);m.flange(2,.945);
  m.box([.5,.5,.5],[.40,.36,.34],2);m.tube(1,.60,.78,.065,.025,3);
  m.tube(1,.79,.87,.205,.045,5,'valve_handle');
  m.box([.5,.83,.5],[.30,.045,.045],5,'valve_handle');m.box([.5,.83,.5],[.045,.045,.30],5,'valve_handle');
  m.box([.5,.84,.5],[.072,.07,.072],4,'valve_handle');m.rotateY();
  m.save('valve',166,'Shutoff Valve',[[[0,.285,.285],[1,.715,.715]],[[.435,.60,.435],[.565,.79,.565]],[[.295,.79,.295],[.705,.875,.705]]]);
}

{
  const m=model();m.flange(2,.055);m.tube(2,0,.63,.125,.030,2);
  m.tube(1,.29,.50,.125,.030,2,'body',[.5,.5,.63]);m.tube(1,.275,.32,.153,.042,3,'body',[.5,.5,.63]);
  m.box([.5,.5,.61],[.25,.25,.25],2);m.tube(1,.59,.73,.052,.02,3);
  m.box([.5,.75,.5],[.30,.055,.055],5);m.box([.5,.75,.5],[.06,.075,.06],5);
  m.save('faucet',167,'Faucet',[[[.285,.285,0],[.715,.715,.10]],[[.375,.375,0],[.625,.625,.755]],[[.347,.275,.477],[.653,.50,.783]],[[.35,.59,.435],[.65,.79,.565]]]);
}

{
  const m=model();m.box([.5,.075,.5],[.86,.15,.80],1);
  for(const y of[.22,.77]){m.box([.5,y,.5],[.70,.13,.65],2);m.box([.5,y+.015,.5],[.75,.045,.70],3);}
  for(const x of[.19,.81])for(const z of[.22,.78]){m.box([x,.49,z],[.085,.54,.085],2);for(const y of[.25,.75])m.box([x,y,z],[.115,.045,.115],3);}
  // Real transparent chamber; water and piston are shared runtime visuals.
  for(const z of[.265,.735])m.box([.5,.50,z],[.51,.43,.018],6);
  for(const x of[.255,.745])m.box([x,.50,.5],[.018,.43,.47],6);
  m.tube(2,0,.265,.13,.03,2);m.flange(2,.045);m.tube(2,.735,1,.13,.03,2);m.flange(2,.955);
  m.tube(1,.81,.94,.075,.025,3);m.box([.5,.96,.5],[.16,.06,.16],3);
  m.tube(0,.82,.93,.20,.055,7,'drive');
  m.tube(0,0,1,.045,.02,7,'drive');
  for(const x of[.065,.935])m.tube(0,x-.035,x+.035,.082,.025,3);
  for(let tooth=0;tooth<8;tooth++){const a=tooth*tau/8;m.box([.875,.5+Math.cos(a)*.20,.5+Math.sin(a)*.20],[.10,.075,.075],7,'drive');}
  const hull=[[[.07,0,.10],[.93,.15,.90]],[[.125,.155,.15],[.875,.30,.85]],[[.125,.705,.15],[.875,.85,.85]],[[.255,.30,.265],[.745,.715,.735]],[[.285,.285,0],[.715,.715,.20]],[[.285,.285,.80],[.715,.715,1]],[[.425,.81,.425],[.575,.99,.575]],[[.82,.2625,.2625],[.93,.7375,.7375]],[[0,.418,.418],[1,.582,.582]]];
  m.save('pump',168,'Kinetic Water Pump',hull);
}

{
  const m=model();m.box([.5,.065,.5],[1,.13,1],0,'floor');
  for(let side=0;side<4;side++) {
    const axis=side<2?0:2,positive=side%2===1,part=`wall_${side}`,center=[.5,.45,.5],size=[1,.90,1];
    center[axis]=positive?.92:.08;size[axis]=.16;m.box(center,size,0,part);
    for(const y of[.18,.72]){const rail=size.slice();rail[1]=.07;rail[axis]=.15;const c=center.slice();c[1]=y;m.box(c,rail,2,part);}
    const cap=size.slice();cap[1]=.10;cap[axis]=.16;const c=center.slice();c[1]=.90;m.box(c,cap,1,part);
    for(const t of[.20,.5,.80]){const stud=center.slice();stud[(axis+2)%3]=t;stud[axis]=positive?.987:.013;stud[1]=.72;const studsize=[.035,.035,.035];studsize[axis]=.024;m.box(stud,studsize,4,part);}
  }
  m.save('basin',169,'Basin',[[[0,0,0],[1,.13,1]],[[0,.13,0],[.16,.95,1]],[[.84,.13,0],[1,.95,1]],[[.16,.13,0],[.84,.95,.16]],[[.16,.13,.84],[.84,.95,1]]]);
}
