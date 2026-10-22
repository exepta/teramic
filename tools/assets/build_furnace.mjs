// Deterministic stone-and-iron scene models with hollow, connectable chimney ports.
import fs from 'node:fs';
import path from 'node:path';
const root = path.resolve(import.meta.dirname, '../..');
const palette = [
  ['masonry', [.47, .43, .37, 1], .92, 0],
  ['masonry_light', [.63, .58, .49, 1], .91, 0],
  ['masonry_dark', [.34, .32, .29, 1], .96, 0],
  ['iron', [.17, .19, .20, 1], .65, .48],
  ['iron_edge', [.29, .30, .29, 1], .56, .44],
  ['brass', [.72, .45, .17, 1], .48, .58],
  ['soot', [.055, .045, .035, 1], 1, 0],
];
function model(name, id, title, description, build) {
  const groups = palette.map(() => ({p: [], n: [], i: []}));
  const collision = [];
  function polygon(points, normal, material) {
    const g = groups[material], start = g.p.length / 3;
    for (const p of points) { g.p.push(...p); g.n.push(...normal); }
    const a = points[1].map((v, i) => v - points[0][i]);
    const b = points[2].map((v, i) => v - points[0][i]);
    const cross = [a[1]*b[2]-a[2]*b[1], a[2]*b[0]-a[0]*b[2], a[0]*b[1]-a[1]*b[0]];
    const reverse = cross.reduce((v, x, i) => v + x * normal[i], 0) < 0;
    for (let i = 1; i + 1 < points.length; i++) g.i.push(start, start+(reverse?i+1:i), start+(reverse?i:i+1));
  }
  function bounds(points) {
    collision.push({
      minimum: [0,1,2].map(i => Math.max(0, Math.min(...points.map(p => p[i])))),
      maximum: [0,1,2].map(i => Math.min(i === 1 && id === 59 ? 2 : 1, Math.max(...points.map(p => p[i])))),
    });
  }
  function box(x, y, z, w, h, d, material = 0, solid = false) {
    const low=[x,y,z], high=[x+w,y+h,z+d];
    if (solid) bounds([low, high]);
    for (let axis=0;axis<3;axis++) for (const sign of [-1,1]) {
      const u=(axis+1)%3,v=(axis+2)%3, normal=[0,0,0]; normal[axis]=sign;
      polygon([[0,0],[1,0],[1,1],[0,1]].map(([a,b])=>{
        const p=[...low];p[axis]=sign>0?high[axis]:low[axis];p[u]=a?high[u]:low[u];p[v]=b?high[v]:low[v];return p;
      }), normal, material);
    }
  }
  function archStone(angle, span, inner=.255, outer=.38, material=0) {
    const center=[.5,.72], points=[];
    for(const [a,r]of[[angle-span/2,inner],[angle+span/2,inner],[angle+span/2,outer],[angle-span/2,outer]])
      points.push([center[0]+Math.cos(a)*r,center[1]+Math.sin(a)*r]);
    for(const [z,sign]of[[.77,-1],[.93,1]]) polygon(points.map(p=>[...p,z]),[0,0,sign],material);
    for(let i=0;i<4;i++){
      const j=(i+1)%4,dx=points[j][0]-points[i][0],dy=points[j][1]-points[i][1],len=Math.hypot(dx,dy);
      polygon([[...points[i],.77],[...points[j],.77],[...points[j],.93],[...points[i],.93]],[-dy/len,dx/len,0],material);
    }
    bounds(points.flatMap(p=>[[...p,.77],[...p,.93]]));
  }
  function ring(y, z, dy, dz, radius) {
    const length=Math.hypot(dy,dz);dy/=length;dz/=length;
    return [[-1,-1],[1,-1],[1,1],[-1,1]].map(([x,v])=>[.5+x*radius,y-v*dz*radius,z+v*dy*radius]);
  }
  function tube(points, radius=.24, inner=.14, material=null) {
    const rings=points.map(([y,z,dy,dz])=>[ring(y,z,dy,dz,radius),ring(y,z,dy,dz,inner)]);
    for(let i=0;i+1<rings.length;i++) {
      for(let wall=0;wall<4;wall++) {
        const next=(wall+1)%4;
        const vertices=[rings[i][0][wall],rings[i][0][next],rings[i+1][0][next],rings[i+1][0][wall]];
        const innerVertices=[rings[i][1][wall],rings[i][1][next],rings[i+1][1][next],rings[i+1][1][wall]];
        const a=vertices[1].map((v,j)=>v-vertices[0][j]), b=vertices[3].map((v,j)=>v-vertices[0][j]);
        let n=[a[1]*b[2]-a[2]*b[1],a[2]*b[0]-a[0]*b[2],a[0]*b[1]-a[1]*b[0]];
        const length=Math.hypot(...n);n=n.map(v=>v/length);
        const center=[.5,(points[i][0]+points[i+1][0])/2,(points[i][1]+points[i+1][1])/2];
        const side=vertices[0].map((v,j)=>v-center[j]).reduce((v,x,j)=>v+x*n[j],0);
        if(side<0) n=n.map(v=>-v);
        polygon(vertices,n,material??(i+wall)%3);
        polygon(innerVertices,n.map(v=>-v),6);
        if(material===null) {
          // Each stone course has two slightly raised, inset masonry blocks.
          // Mortar is the darker continuous tube underneath, never an air gap.
          for(let brick=0;brick<2;brick++) {
            const lo=.025+brick*.5,hi=.475+brick*.5;
            const lerp=(a,b,t)=>a.map((v,j)=>v+(b[j]-v)*t);
            const lower=[lerp(vertices[0],vertices[1],lo),lerp(vertices[0],vertices[1],hi)];
            const upper=[lerp(vertices[3],vertices[2],lo),lerp(vertices[3],vertices[2],hi)];
            const face=[lerp(lower[0],upper[0],.045),lerp(lower[1],upper[1],.045),lerp(lower[1],upper[1],.955),lerp(lower[0],upper[0],.955)];
            polygon(face.map(p=>p.map((v,j)=>v+n[j]*.004)),n,(i+wall+brick)%3);
          }
        }
        bounds([...vertices,...innerVertices]);
      }
    }
    for (const index of [0,rings.length-1]) {
      const point=points[index], sign=index===0?-1:1, length=Math.hypot(point[2],point[3]);
      const n=[0,sign*point[2]/length,sign*point[3]/length];
      for(let i=0;i<4;i++) {
        const j=(i+1)%4;
        polygon([rings[index][0][i],rings[index][0][j],rings[index][1][j],rings[index][1][i]],n,material??1);
      }
    }
  }
  function collar(y,z,dy,dz) {
    const length=Math.hypot(dy,dz);dy/=length;dz/=length;
    tube([[y-dy*.05,z-dz*.05,dy,dz],[y+dy*.05,z+dz*.05,dy,dz]],.29,.24,3);
    const ringPoints=ring(y,z,dy,dz,.295);
    for(const p of ringPoints) {
      if(p.every((v,i)=>v>=.025&&v<=(i===1&&id===59?1.975:.975))) box(p[0]-.022,p[1]-.022,p[2]-.022,.044,.044,.044,5);
    }
  }
  build({box,tube,collar,archStone});
  for(const g of groups) {
    if(g.p.some((v,i)=>!Number.isFinite(v)||v<-.001||v>(i%3===1&&id===59?2.001:1.001))) throw Error(`${name}: model exceeds occupied cells`);
    for(let i=0;i<g.i.length;i+=3) {
      const [a,b,c]=g.i.slice(i,i+3).map(v=>v*3);
      const u=[0,1,2].map(j=>g.p[b+j]-g.p[a+j]),v=[0,1,2].map(j=>g.p[c+j]-g.p[a+j]);
      const cross=[u[1]*v[2]-u[2]*v[1],u[2]*v[0]-u[0]*v[2],u[0]*v[1]-u[1]*v[0]];
      if(cross.reduce((s,x,j)=>s+x*g.n[a+j],0)<=1e-10) throw Error(`${name}: inverted or empty face`);
    }
  }
  if(collision.length>128||collision.some(b=>b.minimum.some((v,i)=>v>=b.maximum[i]))) throw Error(`${name}: invalid collision boxes`);
  const chunks=[],views=[],accessors=[],primitives=[];let offset=0;
  function buffer(data,type,target,integer=false) {
    const bytes=Buffer.from((integer?new Uint32Array(data):new Float32Array(data)).buffer);
    const view=views.length;views.push({buffer:0,byteOffset:offset,byteLength:bytes.length,target});chunks.push(bytes);offset+=bytes.length;
    const width=type==='VEC3'?3:1,a={bufferView:view,componentType:integer?5125:5126,count:data.length/width,type};
    if(type==='VEC3') {
      a.min=[0,1,2].map(i=>Math.min(...data.filter((_,j)=>j%3===i)));
      a.max=[0,1,2].map(i=>Math.max(...data.filter((_,j)=>j%3===i)));
    }
    accessors.push(a);return accessors.length-1;
  }
  groups.forEach((g,material)=>{
    if(g.i.length) primitives.push({attributes:{POSITION:buffer(g.p,'VEC3',34962),NORMAL:buffer(g.n,'VEC3',34962)},indices:buffer(g.i,'SCALAR',34963,true),material});
  });
  const gltf={asset:{version:'2.0',generator:'TERAMIC furnace builder'},scene:0,scenes:[{nodes:[0]}],nodes:[{mesh:0}],meshes:[{primitives}],
    materials:palette.map(([name,baseColorFactor,roughnessFactor,metallicFactor])=>({name,pbrMetallicRoughness:{baseColorFactor,roughnessFactor,metallicFactor}})),
    buffers:[{byteLength:offset,uri:'data:application/octet-stream;base64,'+Buffer.concat(chunks).toString('base64')}],bufferViews:views,accessors};
  const definition={schema_version:1,id,name:title,description,creative_category:'decoration',render_kind:'custom_mesh',struct_block:true,model_layout:'scene',model:`models/${name}.gltf`,
    hardness:id===59?3.0:1.5,resistance:id===59?9:5,required_tool:'pickaxe',mining_level:1,collision_boxes:collision};
  fs.writeFileSync(path.join(root,`assets/terra/blocks/models/${name}.gltf`),JSON.stringify(gltf)+'\n');
  fs.writeFileSync(path.join(root,`assets/terra/blocks/${name}.json`),JSON.stringify(definition,null,2)+'\n');
}
model('furnace',59,'Furnace','A two-block stone furnace. Burns fuel to smelt and cook; chimney pipes carry its smoke outside.',({box,tube,collar,archStone})=>{
  box(.045,0,.06,.91,.18,.88,2,true);
  box(.065,.18,.10,.87,.11,.80,3,true);
  // Open firebox framed by individual stone courses and an iron grate.
  box(.09,.29,.10,.18,.63,.78,0,true);
  box(.73,.29,.10,.18,.63,.78,0,true);
  box(.27,.29,.10,.46,.63,.15,2,true);
  box(.27,.29,.25,.46,.05,.57,6,true);
  for(let row=0;row<4;row++) {
    for(const x of [.085,.735]) {
      box(x,.30+row*.15,.085,.18,.143,.81,(row+1)%3);
      box(x+.008,.30+row*.15,.895,.164,.143,.015,row%3);
    }
    for(let i=0;i<4;i++) box(.09+i*.205,.30+row*.15,.08,.198,.143,.025,(row+i)%3);
  }
  box(.085,.91,.09,.83,.16,.66,0,true);
  box(.16,1.07,.16,.68,.13,.59,1,true);
  box(.23,1.20,.23,.54,.10,.54,2,true);
  for(let i=0;i<7;i++) archStone((i+.5)*Math.PI/7,Math.PI/7-.022,.255,.385,i%3);
  // Stepped sloping roof, segmented masonry and corner straps match the heavy
  // stone/iron silhouette without adding any per-stone runtime entities.
  for(let row=0;row<3;row++) for(let col=0;col<4-row;col++){
    // Keep the outer courses clear of the support's coplanar front and sides.
    const width=.838-row*.15,x=.081+row*.075+col*width/(4-row);
    box(x,1.04+row*.075,.11+row*.035,width/(4-row)-.008,.075,.65-row*.075,(row+col+1)%3);
  }
  for(const x of [.065,.91]) for(let y=.30;y<.87;y+=.145)
    box(x,y,.20,.025,.135,.52,2);
  for(const x of [.06,.88]) for(const z of [.08,.86]) box(x,.035,z,.06,.20,.06,3,true);
  for(const y of [.15,.91]) {
    box(.06,y,.065,.88,.075,.04,3);
    box(.06,y,.915,.88,.075,.04,4);
    for(const x of [.12,.33,.62,.83]) box(x,y+.02,.936,.04,.035,.02,5);
  }
  box(.24,.32,.915,.52,.045,.035,3,true);
  box(.24,.64,.915,.52,.04,.035,4,true);
  for(let i=0;i<7;i++) box(.27+i*.073,.35,.867,.025,.30,.035,3,true);
  box(.43,.20,.935,.14,.05,.025,5);
  box(.285,.075,.945,.43,.10,.025,3);
  box(.34,.11,.973,.32,.04,.018,4);
  box(.43,.12,.991,.14,.035,.009,5);
  for(const x of [.12,.82]) {
    box(x,.43,.93,.045,.33,.025,3);
    for(const y of [.43,.72]) box(x-.005,y,.955,.055,.04,.025,5);
  }
  for(const x of [.04,.94]) {
    // Two tools hang from the side strap, with open ring handles.
    for(const z of [.38,.62]) {
      box(x,.38,z,.025,.48,.026,3);
      box(x-.012,.78,z-.015,.049,.035,.055,5);
      box(x-.012,.87,z-.015,.049,.035,.055,5);
      box(x-.012,.81,z-.015,.018,.06,.055,5);
      box(x+.019,.81,z-.015,.018,.06,.055,5);
      box(x-.012,.34,z-.022,.05,.075,.07,4);
    }
  }
  tube([[1.3,.5,1,0],[1.48,.5,1,0],[1.66,.5,1,0],[1.84,.5,1,0],[2,.5,1,0]]);
  collar(1.36,.5,1,0);collar(1.91,.5,1,0);
});
model('chimney_straight',60,'Chimney Pipe','A hollow modular chimney. Placement aligns straight and elbow connections automatically.',({tube,collar})=>{
  tube([[0,.5,1,0],[.25,.5,1,0],[.5,.5,1,0],[.75,.5,1,0],[1,.5,1,0]]);
  collar(.07,.5,1,0);collar(.93,.5,1,0);
});
model('chimney_elbow',61,'Elbow Chimney Pipe','A ninety-degree chimney turn connecting the bottom to the facing side.',({tube,collar})=>{
  tube([[0,.5,1,0],[.25,.5,1,0],[.43,.57,1,1],[.5,.75,0,1],[.5,1,0,1]]);
  collar(.07,.5,1,0);collar(.5,.93,0,1);
});
model('chimney_horizontal',62,'Horizontal Chimney Pipe','A straight horizontal connector between opposite chimney ports.',({tube,collar})=>{
  tube([[.5,0,0,1],[.5,.25,0,1],[.5,.5,0,1],[.5,.75,0,1],[.5,1,0,1]]);
  collar(.5,.07,0,1);collar(.5,.93,0,1);
});
model('chimney_bend_up',63,'Rising Chimney Bend','A forty-five-degree rising bend connecting the bottom to the facing side.',({tube,collar})=>{
  tube([[0,.5,1,0],[.25,.55,1,1],[.40,.70,1,1],[.5,1,0,1]]);
  collar(.06,.5,1,0);collar(.5,.94,0,1);
});
model('chimney_bend_down',64,'Falling Chimney Bend','A forty-five-degree falling bend connecting the top to the facing side.',({tube,collar})=>{
  tube([[1,.5,-1,0],[.75,.55,-1,1],[.60,.70,-1,1],[.5,1,0,1]]);
  collar(.94,.5,-1,0);collar(.5,.94,0,1);
});
