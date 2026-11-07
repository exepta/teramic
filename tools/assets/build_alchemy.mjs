// Bounded one-cell alchemy vessels and campfire, using existing machinery maps.
import fs from 'node:fs';
import path from 'node:path';

const directory = path.resolve(import.meta.dirname, '../../assets/terra/blocks');
const selected = new Set(process.argv.slice(2));
const tau = Math.PI * 2;
const palette = [
  ['cauldron_iron', [.72,.75,.79,1], .65, 'machinery-iron-hd.png'],
  ['copper_bands', [1,.64,.39,1], .60, 'machinery-iron-hd.png'],
  ['oak_stand', [.92,.86,.76,1], 0, 'machinery-oak-hd.png'],
  ['charred_logs', [.29,.26,.24,1], 0, 'machinery-oak-hd.png'],
  ['stone_ring', [.59,.62,.65,1], 0, 'stone.png'],
  ['iron_rivets', [.98,.98,.98,1], .72, 'machinery-iron-hd.png'],
];

function model() {
  const groups = palette.map(() => ({p:[],n:[],uv:[],i:[]})), collision = [];
  function face(points, normal, material) {
    const g=groups[material],base=g.p.length/3,length=Math.hypot(...normal);
    const n=normal.map(v=>v/length),axis=n.map(Math.abs).indexOf(Math.max(...n.map(Math.abs)));
    const a=points[1].map((v,i)=>v-points[0][i]),b=points[2].map((v,i)=>v-points[0][i]);
    const cross=[a[1]*b[2]-a[2]*b[1],a[2]*b[0]-a[0]*b[2],a[0]*b[1]-a[1]*b[0]];
    const reverse=cross.reduce((sum,v,i)=>sum+v*n[i],0)<0;
    for(const p of points){g.p.push(...p);g.n.push(...n);g.uv.push(p[(axis+1)%3]*1.8,p[(axis+2)%3]*1.8);}
    for(let i=1;i+1<points.length;i++)g.i.push(base,base+(reverse?i+1:i),base+(reverse?i:i+1));
  }
  function hull(points) {
    collision.push({minimum:[0,1,2].map(i=>Math.min(...points.map(p=>p[i]))),maximum:[0,1,2].map(i=>Math.max(...points.map(p=>p[i])))});
  }
  function box(center,size,material,solid=true,angle=0) {
    const c=Math.cos(angle),s=Math.sin(angle),half=size.map(v=>v/2);
    const point=p=>[center[0]+p[0]*c+p[2]*s,center[1]+p[1],center[2]-p[0]*s+p[2]*c];
    const points=[];
    for(let axis=0;axis<3;axis++)for(const sign of[-1,1]) {
      const u=(axis+1)%3,v=(axis+2)%3,n=[0,0,0];n[axis]=sign;
      const normal=[n[0]*c+n[2]*s,n[1],-n[0]*s+n[2]*c];
      const corners=[[-1,-1],[1,-1],[1,1],[-1,1]].map(([a,b])=>{
        const p=[0,0,0];p[axis]=half[axis]*sign;p[u]=half[u]*a;p[v]=half[v]*b;return point(p);
      });
      points.push(...corners);face(corners,normal,material);
    }
    if(solid)hull(points);
  }
  function band(y0,y1,r0,r1,thickness,material,solid=false,sides=16) {
    const point=(y,r,a)=>[.5+Math.cos(a)*r,y,.5+Math.sin(a)*r];
    for(let i=0;i<sides;i++) {
      const a=i*tau/sides,b=(i+1)*tau/sides,m=(a+b)/2;
      const outer=[point(y0,r0,a),point(y0,r0,b),point(y1,r1,b),point(y1,r1,a)];
      face(outer,[Math.cos(m),-(r1-r0)/(y1-y0),Math.sin(m)],material);
      const inner=[point(y0,r0-thickness,a),point(y1,r1-thickness,a),point(y1,r1-thickness,b),point(y0,r0-thickness,b)];
      face(inner,[-Math.cos(m),(r1-r0)/(y1-y0),-Math.sin(m)],material);
      for(const[y,r,sign]of[[y0,r0,-1],[y1,r1,1]]) {
        face([point(y,r-thickness,a),point(y,r-thickness,b),point(y,r,b),point(y,r,a)],[0,sign,0],material);
      }
      if(solid)hull([...outer,...inner]);
    }
  }
  function disk(y,radius,material) {
    for(let side=0;side<16;side++) {
      const point=i=>[.5+Math.cos(i*tau/16)*radius,y,.5+Math.sin(i*tau/16)*radius];
      face([[.5,y,.5],point(side),point(side+1)],[0,1,0],material);
    }
  }
  function collisionHull(boxes) {
    collision.length=0;
    for(const[minimum,maximum]of boxes)collision.push({minimum,maximum});
  }
  function save(name,id,label,description) {
    if(selected.size&&!selected.has(name))return;
    const chunks=[],views=[],accessors=[],primitives=[];let offset=0,vertices=0;
    function accessor(data,width,integer=false) {
      const bytes=Buffer.from((integer?new Uint32Array(data):new Float32Array(data)).buffer),view=views.length;
      views.push({buffer:0,byteOffset:offset,byteLength:bytes.length,target:integer?34963:34962});chunks.push(bytes);offset+=bytes.length;
      const a={bufferView:view,componentType:integer?5125:5126,count:data.length/width,type:{1:'SCALAR',2:'VEC2',3:'VEC3'}[width]};
      if(width===3){a.min=[0,1,2].map(i=>Math.min(...data.filter((_,j)=>j%3===i)));a.max=[0,1,2].map(i=>Math.max(...data.filter((_,j)=>j%3===i)));}
      accessors.push(a);return accessors.length-1;
    }
    for(const[material,g]of groups.entries()) {
      if(!g.i.length)continue;vertices+=g.p.length/3;
      if([...g.p,...g.n,...g.uv].some(v=>!Number.isFinite(v))||g.p.some(v=>v<-.000001||v>1.000001))throw Error(`${name} leaves one-cell bounds`);
      for(let i=0;i<g.i.length;i+=3) {
        const[a,b,c]=g.i.slice(i,i+3).map(i=>i*3),u=[0,1,2].map(j=>g.p[b+j]-g.p[a+j]),v=[0,1,2].map(j=>g.p[c+j]-g.p[a+j]);
        const n=[u[1]*v[2]-u[2]*v[1],u[2]*v[0]-u[0]*v[2],u[0]*v[1]-u[1]*v[0]];
        if(n.reduce((sum,v,j)=>sum+v*g.n[a+j],0)<=1e-10)throw Error(`${name} has inverted/empty triangles`);
      }
      primitives.push({attributes:{POSITION:accessor(g.p,3),NORMAL:accessor(g.n,3),TEXCOORD_0:accessor(g.uv,2)},indices:accessor(g.i,1,true),material});
    }
    if(vertices>8192||collision.length>96)throw Error(`${name} exceeds bounded detail/collider budget ${vertices}/${collision.length}`);
    const used=palette.map(([name,baseColorFactor,metallicFactor])=>({name,pbrMetallicRoughness:{baseColorFactor,metallicFactor,roughnessFactor:metallicFactor?.64:.90}}));
    const gltf={asset:{version:'2.0',generator:'TERAMIC bounded alchemy builder'},scene:0,scenes:[{nodes:[0]}],nodes:[{mesh:0}],meshes:[{primitives}],materials:used,buffers:[{byteLength:offset,uri:'data:application/octet-stream;base64,'+Buffer.concat(chunks).toString('base64')}],bufferViews:views,accessors};
    fs.writeFileSync(path.join(directory,'models',`${name}.gltf`),JSON.stringify(gltf)+'\n');
    fs.writeFileSync(path.join(directory,`${name}.json`),JSON.stringify({schema_version:1,id,name:label,description,creative_category:'technic',render_kind:'custom_mesh',struct_block:true,model_layout:'scene',model:`models/${name}.gltf`,hardness:2.5,resistance:8,mining_level:0,required_tool:'pickaxe',textures:Object.fromEntries(palette.filter((_,i)=>groups[i].i.length).map(([name,_,__,texture])=>[name,`textures/${texture}`])),collision_boxes:collision},null,2)+'\n');
    console.log(`${name}: ${vertices} vertices, ${primitives.length} shared material groups, ${collision.length} colliders`);
  }
  return{box,band,disk,collisionHull,save};
}

{
  const m=model();
  // Low burner tray and four reinforced legs keep the round bowl open underneath.
  for(const x of[.11,.89])for(const z of[.11,.89]) {
    m.box([x,.31,z],[.105,.62,.105],2);
    m.box([x,.035,z],[.15,.07,.15],0);
    for(const y of[.13,.51,.60])m.box([x,y,z],[.126,.055,.126],1,false);
    for(const y of[.145,.525])m.box([x,y,z+.067],[.025,.025,.018],5,false);
  }
  for(const z of[.14,.86])m.box([.5,.11,z],[.68,.06,.07],0);
  for(const x of[.14,.86])m.box([x,.11,.5],[.07,.06,.68],0);
  m.box([.5,.075,.5],[.43,.045,.43],3);
  // Faceted closed bottom, curved outer/inner walls and thick rolled copper rim.
  m.box([.5,.365,.5],[.28,.045,.28],0);
  m.disk(.388,.18,0);
  const rows=[[.38,.20],[.46,.30],[.57,.37],[.68,.38],[.78,.35],[.87,.32]];
  for(let row=0;row+1<rows.length;row++) {
    const[y0,r0]=rows[row],[y1,r1]=rows[row+1];
    m.band(y0,y1,r0,r1,.027,row===0?1:0,row%2===0);
  }
  m.band(.865,.895,.34,.35,.047,1,true);
  m.band(.895,.935,.35,.35,.047,0,false);
  m.band(.935,.95,.35,.35,.047,1,false);
  for(let side=0;side<16;side++) {
    const a=(side+.5)*tau/16,x=.5+Math.cos(a)*.349,z=.5+Math.sin(a)*.349;
    m.box([x,.915,z],[.035,.028,.022],5,false,Math.PI/2-a);
  }
  for(const x of[.11,.89]) {
    m.box([x,.63,.5],[.085,.115,.32],1);
    for(const z of[.39,.61])m.box([x,.63,z],[.11,.028,.028],5,false);
  }
  const hull=[];
  for(const x of[.11,.89])for(const z of[.11,.89])hull.push([[x-.075,0,z-.075],[x+.075,.66,z+.075]]);
  hull.push([[.30,.34,.30],[.70,.39,.70]]);
  // Picking/physics use one conservative hollow bowl, not the rivets or bands.
  for(const x of[.12,.76])hull.push([[x,.39,.24],[x+.12,.95,.76]]);
  for(const z of[.12,.76])hull.push([[.24,.39,z],[.76,.95,z+.12]]);
  m.collisionHull(hull);
  m.save('cauldron',161,'Alchemy Cauldron','An open iron and copper brewing bowl on a reinforced wooden burner stand.');
}

{
  const m=model();
  for(let stone=0;stone<12;stone++) {
    const a=stone*tau/12;
    m.box([.5+Math.cos(a)*.375,.075+(stone%3)*.008,.5+Math.sin(a)*.375],[.18,.13,.13],4,true,Math.PI/2-a);
  }
  for(const[height,angle]of[[.12,.28],[.18,Math.PI/2+.28]]) {
    for(const offset of[-.13,.13]) {
      const c=Math.cos(angle),s=Math.sin(angle);
      m.box([.5+offset*c,height,.5-offset*s],[.105,.105,.57],3,true,angle);
      for(const end of[-1,1])m.box([.5+offset*c+end*.289*s,height,.5-offset*s+end*.289*c],[.087,.087,.012],2,false,angle);
    }
  }
  m.save('campfire',162,'Campfire','A low stone-ring fire with crossed charred logs, lit only while real fuel is burning.');
}
