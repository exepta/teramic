// Authored, reusable glTF assemblies for the passive belt filling station.
import fs from 'node:fs';
import path from 'node:path';

const root = path.resolve(import.meta.dirname, '../../assets');
const selected = new Set(process.argv.slice(2));
function writeChanged(file, text) {
  if (fs.existsSync(file) && fs.readFileSync(file, 'utf8') === text) return;
  fs.writeFileSync(file, text);
}
const palette = [
  ['wood_frame', 'machinery-oak-hd.png', [.95, .95, .95, 1], 0],
  ['forged_plate', 'machinery-iron-hd.png', [.76, .78, .82, 1], .45],
  ['brass_trim', 'machinery-iron-hd.png', [1, .60, .10, 1], .55],
  ['iron_rivets', 'machinery-iron-hd.png', [.95, .97, 1, 1], .8],
  ['kinetic_belt', 'machinery-belt-hd.png', [.88, .90, .93, 1], 0],
  ['infuser_glass', null, [.57, .81, .86, .24], .05],
  ['rubber_seal', 'machinery-belt-hd.png', [.36, .39, .42, 1], 0],
];

function assembly() {
  const groups = new Map();
  function face(points, normal, material, part='body') {
    const key=`${part}:${material}`;
    if (!groups.has(key)) groups.set(key, {material,part,p: [], n: [], uv: [], i: []});
    const g = groups.get(key), base = g.p.length / 3;
    const u = points[1].map((v, i) => v - points[0][i]);
    const v = points[2].map((v, i) => v - points[0][i]);
    const cross = [u[1]*v[2]-u[2]*v[1], u[2]*v[0]-u[0]*v[2], u[0]*v[1]-u[1]*v[0]];
    const reverse = cross.reduce((sum, value, i) => sum + value * normal[i], 0) < 0;
    const axis = normal.map(Math.abs).indexOf(Math.max(...normal.map(Math.abs)));
    for (const p of points) {
      g.p.push(...p); g.n.push(...normal);
      if(material===4 && axis===1) g.uv.push(p[0]-.18,p[2]);
      else g.uv.push(p[(axis+1)%3]*1.4, p[(axis+2)%3]*1.4);
    }
    for (let i=1; i+1<points.length; i++) g.i.push(base, base+(reverse?i+1:i), base+(reverse?i:i+1));
  }
  function box(c, size, material, part='body') {
    for (let axis=0; axis<3; axis++) for (const sign of [-1, 1]) {
      const u=(axis+1)%3, v=(axis+2)%3, n=[0, 0, 0]; n[axis]=sign;
      face([[-1,-1],[1,-1],[1,1],[-1,1]].map(([a,b]) => {
        const p=c.slice(); p[axis]+=size[axis]*sign/2; p[u]+=size[u]*a/2; p[v]+=size[v]*b/2; return p;
      }), n, material, part);
    }
  }
  function bevelBox(c, size, bevel, material) {
    const h=size.map(s=>s/2), inset=h.map(s=>s-bevel);
    for(let axis=0;axis<3;axis++)for(const sign of [-1,1]){
      const u=(axis+1)%3,v=(axis+2)%3,n=[0,0,0];n[axis]=sign;
      face([[-1,-1],[1,-1],[1,1],[-1,1]].map(([a,b])=>{
        const p=c.slice();p[axis]+=h[axis]*sign;p[u]+=inset[u]*a;p[v]+=inset[v]*b;return p;
      }),n,material);
    }
    for(let a=0;a<3;a++)for(let b=a+1;b<3;b++)for(const sa of [-1,1])for(const sb of [-1,1]){
      const along=3-a-b,n=[0,0,0];n[a]=sa/Math.sqrt(2);n[b]=sb/Math.sqrt(2);
      face([[-1,0],[1,0],[1,1],[-1,1]].map(([end,edge])=>{
        const p=c.slice();p[along]+=inset[along]*end;p[a]+=(edge?inset[a]:h[a])*sa;p[b]+=(edge?h[b]:inset[b])*sb;return p;
      }),n,material);
    }
    for(const x of [-1,1])for(const y of [-1,1])for(const z of [-1,1]){
      const signs=[x,y,z];
      face([0,1,2].map(axis=>c.map((v,i)=>v+(i===axis?h[i]:inset[i])*signs[i])),signs.map(s=>s/Math.sqrt(3)),material);
    }
  }
  function tube(axis, start, end, radius, thickness, material, center=[.5,.5,.5], part='body', sides=8) {
    const u=(axis+1)%3, v=(axis+2)%3;
    const point=(t,r,a) => { const p=center.slice(); p[axis]=t; p[u]+=Math.cos(a)*r; p[v]+=Math.sin(a)*r; return p; };
    for (let side=0; side<sides; side++) {
      const a=side*Math.PI*2/sides, b=(side+1)*Math.PI*2/sides, mid=(a+b)/2;
      const n=[0,0,0]; n[u]=Math.cos(mid); n[v]=Math.sin(mid);
      face([point(start,radius,a),point(end,radius,a),point(end,radius,b),point(start,radius,b)],n,material,part);
      face([point(start,radius-thickness,a),point(start,radius-thickness,b),point(end,radius-thickness,b),point(end,radius-thickness,a)],n.map(v=>-v),material,part);
      for (const [t,sign] of [[start,-1],[end,1]]) {
        const normal=[0,0,0]; normal[axis]=sign;
        face([point(t,radius,a),point(t,radius,b),point(t,radius-thickness,b),point(t,radius-thickness,a)],normal,material,part);
      }
    }
  }
  function save(name, id, label, description, collision) {
    if (selected.size && !selected.has(name)) return;
    const buffers=[], bufferViews=[], accessors=[], primitives=[]; let offset=0;
    function accessor(data, width, index=false) {
      const typed=index?new Uint32Array(data):new Float32Array(data), bytes=Buffer.from(typed.buffer);
      const bufferView=bufferViews.length;
      bufferViews.push({buffer:0,byteOffset:offset,byteLength:bytes.length,target:index?34963:34962});
      buffers.push(bytes); offset+=bytes.length;
      const a={bufferView,componentType:index?5125:5126,count:data.length/width,type:{1:'SCALAR',2:'VEC2',3:'VEC3'}[width]};
      if (width===3) {
        a.min=[0,1,2].map(i=>Math.min(...data.filter((_,j)=>j%3===i)));
        a.max=[0,1,2].map(i=>Math.max(...data.filter((_,j)=>j%3===i)));
      }
      accessors.push(a); return accessors.length-1;
    }
    let vertices=0;
    for (const g of groups.values()) {
      if ([...g.p,...g.n,...g.uv].some(v=>!Number.isFinite(v)) || g.p.some(v=>v<0 || v>1)) throw Error(`${name}: invalid geometry`);
      vertices+=g.p.length/3;
      primitives.push({attributes:{POSITION:accessor(g.p,3),NORMAL:accessor(g.n,3),TEXCOORD_0:accessor(g.uv,2)},indices:accessor(g.i,1,true),material:g.material,extras:{filling_part:g.part}});
    }
    if (vertices>3000) throw Error(`${name}: detail budget exceeded (${vertices} vertices)`);
    const images=[],textures=[];
    const materials=palette.map(([name,texture,baseColorFactor,metallicFactor])=>{
      const pbrMetallicRoughness={baseColorFactor,metallicFactor,roughnessFactor:name==='kinetic_belt'?.88:name==='brass_trim'?.4:.64};
      if(texture){const index=images.length;images.push({uri:`../textures/${texture}`});textures.push({source:index});pbrMetallicRoughness.baseColorTexture={index};}
      return {name,pbrMetallicRoughness,...(name==='infuser_glass'?{alphaMode:'BLEND',doubleSided:true}:{})};
    });
    const gltf={asset:{version:'2.0',generator:'TERAMIC belt filling builder'},scene:0,scenes:[{nodes:[0]}],nodes:[{mesh:0}],meshes:[{primitives}],
      materials,images,textures,
      buffers:[{byteLength:offset,uri:'data:application/octet-stream;base64,'+Buffer.concat(buffers).toString('base64')}],bufferViews,accessors};
    writeChanged(path.join(root,'terra/blocks/models',`${name}.gltf`),JSON.stringify(gltf)+'\n');
    const definition={schema_version:1,id,name:label,description,creative_category:'technic',render_kind:'custom_mesh',struct_block:true,model_layout:'scene',model:`models/${name}.gltf`,hardness:3,resistance:8,mining_level:0,required_tool:'pickaxe',
      textures:Object.fromEntries(palette.filter(([,texture])=>texture).map(([name,texture])=>[name,`textures/${texture}`])),
      collision_boxes:collision.map(([minimum,maximum])=>({minimum,maximum}))};
    writeChanged(path.join(root,'terra/blocks',`${name}.json`),JSON.stringify(definition,null,2)+'\n');
    console.log(`${name}: ${vertices} vertices / ${new Set(primitives.map(p=>p.material)).size} materials / ${primitives.length} parts`);
  }
  return {box,bevelBox,tube,save};
}

{
  const m=assembly();
  // A real belt passes through a reinforced timber depot, exactly at belt height.
  // World ribbons are animated by the same material as adjacent conveyors.
  m.box([.5,.30,.5],[.70,.38,.72],0);
  m.box([.5,.075,.5],[.82,.10,.84],1);
  m.box([.5,.525,.5],[.78,.09,.86],1);
  m.box([.5,.615,.5],[.64,.02,1],4,'ribbon');
  for (const x of [.115,.885]) {
    m.box([x,.5775,.5],[.09,.095,1],2);
    m.box([x,.49,.5],[.13,.06,.89],1);
  }
  for (const x of [.125,.875]) for (const z of [.125,.875]) {
    m.box([x,.30,z],[.11,.60,.11],0);
    for(const y of [.125,.465])m.box([x,y,z],[.145,.075,.145],1);
    m.box([x,.606,z],[.12,.028,.12],3);
  }
  for (const z of [.12,.88]) {
    m.box([.5,.32,z],[.55,.21,.035],1);
    m.box([.5,.205,z],[.58,.045,.055],2);
    m.box([.5,.445,z],[.58,.045,.055],2);
    m.box([.5,.30,z],[.14,.09,.07],2);
    for(const x of [.29,.71])m.box([x,.32,z],[.035,.04,.055],3);
  }
  for(const x of [.137,.863]) {
    m.box([x,.33,.5],[.035,.13,.48],2);
    for(const z of [.32,.68])m.box([x,.33,z],[.055,.04,.035],3);
  }
  m.save('hold_block',170,'Hold Block','A passive platform at conveyor height. Holds vessels below a Fluid Infuser, then hands them to the next belt.',[[[.04,0,.04],[.96,.625,.96]]]);
}

{
  const m=assembly();
  // One complete grid block: reinforced timber chamber, stepped steel crown,
  // broad brass straps, square rivets, hollow coupling and a metal filling head.
  // Every fitting and the nozzle remain inside [0,1]^3; the air gap is below it.
  m.bevelBox([.5,.625,.5],[.70,.58,.70],.018,0);
  m.box([.5,.94,.5],[.76,.07,.76],0);
  m.box([.5,.9675,.5],[.57,.035,.57],1);
  for(const y of [.315,.545,.895]){
    for(const z of [.13,.87])m.box([.5,y,z],[.82,.065,.075],1);
    for(const x of [.13,.87])m.box([x,y,.5],[.075,.065,.68],1);
  }
  for(const x of [.14,.86])for(const z of [.14,.86]){
    m.box([x,.61,z],[.09,.58,.09],0);
    for(const y of [.345,.865]){
      m.box([x,y,z],[.14,.12,.14],1);
      // Raised bolt heads stand clear of the plates, avoiding coplanar faces.
      m.box([x,y,z<.5?z-.077:z+.077],[.045,.045,.025],3);
    }
    m.box([x,.985,z],[.05,.03,.05],3);
  }
  // Long upper front strap and square lower locks reproduce the reference.
  for(const z of [.095,.905]){
    m.box([.5,.735,z],[.145,.32,.035],2);
    for(const y of [.625,.845])m.box([.5,y,z<.5?z-.025:z+.025],[.045,.045,.025],3);
    m.box([.5,.455,z],[.17,.14,.035],2);
    m.box([.5,.455,z<.5?z-.025:z+.025],[.06,.06,.025],3);
  }
  for(const x of [.095,.905]){
    m.box([x,.69,.5],[.035,.40,.15],2);
    for(const y of [.57,.82])m.box([x<.5?x-.025:x+.025,y,.5],[.025,.045,.045],3);
  }
  // The reference's right-hand coupling is part of the permanent housing.
  // Optional fittings on the other faces only appear with connected pipes.
  for (const axis of [0,2]) for (const [a,b,side] of [[0,.20,axis*2],[.80,1,axis*2+1]]) {
    const part=side===1?'body':`inlet_${side}`;
    m.tube(axis,a,b,.16,.035,1,[.5,.5,.5],part);
    m.tube(axis,a===0?.018:.93,a===0?.07:.982,.20,.055,2,[.5,.5,.5],part,8);
  }
  m.tube(1,.965,1,.14,.04,2,[.5,.5,.5],'inlet_3',6);
  m.box([.5,.265,.5],[.31,.105,.31],1);
  m.tube(1,.16,.245,.14,.05,1,[.5,.5,.5],'body',8);
  m.tube(1,.13,.18,.11,.045,2,[.5,.5,.5],'body',6);
  m.tube(1,.035,.14,.065,.025,3,[.5,.5,.5],'body',8);
  m.tube(1,0,.05,.035,.015,2,[.5,.5,.5],'body',6);
  m.save('fluid_infuser',171,'Fluid Infuser','A full-block pipe-fed filling head. Place one air block above the Hold Block, then the Infuser. Requires no kinetic drive.',[
    [[.07,.27,.07],[.93,1,.93]],
    [[0,.29,.29],[1,.71,.71]],
    [[.29,.29,0],[.71,.71,1]],
    [[.35,.94,.35],[.65,1,.65]],
    [[.34,.12,.34],[.66,.30,.66]],
    [[.43,0,.43],[.57,.12,.57]],
  ]);
}
