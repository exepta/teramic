// Deterministic static voxel model; ordinary terrain remains generated cube geometry.
import fs from 'node:fs';
import path from 'node:path';
const root = path.resolve(import.meta.dirname, '../..');
const palette = [
  ['haft', [.30,.17,.07,1]], ['grain', [.43,.26,.12,1]],
  ['iron', [.24,.27,.28,1]], ['coal', [.07,.045,.025,1]],
  ['flame', [1,.28,.025,1], [1.4,.32,.025]],
  ['heart', [1,.72,.15,1], [1.8,.75,.08]],
];
const groups = palette.map(() => ({p:[],n:[],i:[]}));
function polygon(g, points, n) {
  const base=g.p.length/3;
  for (const p of points) {g.p.push(...p); g.n.push(...n);}
  const a=points[1].map((v,i)=>v-points[0][i]), b=points[2].map((v,i)=>v-points[0][i]);
  const cross=[a[1]*b[2]-a[2]*b[1],a[2]*b[0]-a[0]*b[2],a[0]*b[1]-a[1]*b[0]];
  const reverse=cross.reduce((v,x,i)=>v+x*n[i],0)<0;
  for(let i=1;i+1<points.length;i++) g.i.push(base,base+(reverse?i+1:i),base+(reverse?i:i+1));
}
function box(x,y,z,w,h,d,m,bevel=true) {
  const g = groups[m];
  if(bevel) {
    const center=[x+w/2,y+h/2,z+d/2], half=[w/2,h/2,d/2], r=Math.min(w,h,d)*.14;
    for(let axis=0;axis<3;axis++) for(const sign of [-1,1]) {
      const u=(axis+1)%3,v=(axis+2)%3,n=[0,0,0]; n[axis]=sign;
      polygon(g,[[-1,-1],[1,-1],[1,1],[-1,1]].map(([a,b])=>{
        const p=[...center];p[axis]+=sign*half[axis];p[u]+=a*(half[u]-r);p[v]+=b*(half[v]-r);return p;
      }),n);
    }
    for(let axis=0;axis<3;axis++) for(const a of [-1,1]) for(const b of [-1,1]) {
      const u=(axis+1)%3,v=(axis+2)%3,n=[0,0,0];n[u]=a/Math.SQRT2;n[v]=b/Math.SQRT2;
      polygon(g,[[-1,0],[1,0],[1,1],[-1,1]].map(([end,side])=>{
        const p=[...center];p[axis]+=end*(half[axis]-r);p[u]+=a*(half[u]-(side?r:0));p[v]+=b*(half[v]-(side?0:r));return p;
      }),n);
    }
    for(const a of [-1,1]) for(const b of [-1,1]) for(const c of [-1,1]) {
      const signs=[a,b,c],n=signs.map(v=>v/Math.sqrt(3));
      polygon(g,[0,1,2].map(axis=>center.map((v,i)=>v+signs[i]*(half[i]-(axis===i?0:r)))),n);
    }
    return;
  }
  const faces = [
    [[-1,0,0],[[x,y,z+d],[x,y,z],[x,y+h,z],[x,y+h,z+d]]],
    [[1,0,0],[[x+w,y,z],[x+w,y,z+d],[x+w,y+h,z+d],[x+w,y+h,z]]],
    [[0,-1,0],[[x,y,z],[x,y,z+d],[x+w,y,z+d],[x+w,y,z]]],
    [[0,1,0],[[x,y+h,z+d],[x,y+h,z],[x+w,y+h,z],[x+w,y+h,z+d]]],
    [[0,0,-1],[[x,y,z],[x+w,y,z],[x+w,y+h,z],[x,y+h,z]]],
    [[0,0,1],[[x+w,y,z+d],[x,y,z+d],[x,y+h,z+d],[x+w,y+h,z+d]]],
  ];
  for (const [n, points] of faces) {
    const i=g.p.length/3;
    for (const p of points) {g.p.push(...p); g.n.push(...n);}
    g.i.push(i,i+2,i+1,i,i+3,i+2);
  }
}
box(.45,0,.45,.10,.58,.10,0);
for(let i=0;i<5;i++) {
  box(.447,.045+i*.086,.447,.106,.016,.106,0);
  box(.449,.032+i*.093,.444,.018,.054,.012,1);
  box(.545,.071+i*.081,.471,.012,.042,.021,1);
}
box(.405,.45,.405,.19,.065,.19,2);
box(.37,.51,.37,.26,.045,.26,2);
for (const x of [.355,.62]) for (const z of [.355,.62]) {
  box(x,.52,z,.025,.18,.025,2);
  box(x-.006,.605,z-.006,.037,.025,.037,2);
}
for(const y of [.55,.68]) {
  for(const z of [.35,.625]) box(.35,y,z,.30,.028,.025,2);
  for(const x of [.35,.625]) box(x,y,.375,.025,.028,.25,2);
}
for(const x of [.405,.47,.535]) for(const z of [.405,.48,.54]) {
  box(x,.555,z,.047,.064,.047,3);
  box(x+.012,.61,z+.008,.024,.012,.026,4);
}
// Rooted, separated tongues retain air gaps and the existing vertex fire path.
for (const [x,z,height,lean] of [[.425,.43,.27,.02],[.50,.47,.25,-.015],[.45,.50,.31,.03],[.40,.48,.19,-.01],[.54,.45,.23,.015]]) {
  for(let step=0;step<4;step++) {
    const size=.055-step*.009;
    box(x+lean*step/3,.62+height*step/4,z,size,height/4+.008,size,step<2?4:5,false);
  }
}
const buffers=[],bufferViews=[],accessors=[],primitives=[];
let offset=0;
function buffer(data, type, target, integer=false) {
  const bytes=Buffer.from((integer?new Uint32Array(data):new Float32Array(data)).buffer);
  const view=bufferViews.length;
  bufferViews.push({buffer:0,byteOffset:offset,byteLength:bytes.length,target});
  buffers.push(bytes); offset+=bytes.length;
  const n=type==='VEC3'?3:1;
  const a={bufferView:view,componentType:integer?5125:5126,count:data.length/n,type};
  if(type==='VEC3') {
    a.min=[0,1,2].map(i=>Math.min(...data.filter((_,j)=>j%3===i)));
    a.max=[0,1,2].map(i=>Math.max(...data.filter((_,j)=>j%3===i)));
  }
  accessors.push(a); return accessors.length-1;
}
groups.forEach((g,material)=>{
  primitives.push({attributes:{POSITION:buffer(g.p,'VEC3',34962),NORMAL:buffer(g.n,'VEC3',34962)},indices:buffer(g.i,'SCALAR',34963,true),material});
});
const gltf={asset:{version:'2.0',generator:'TERAMIC torch builder'},scene:0,scenes:[{nodes:[0]}],nodes:[{mesh:0}],meshes:[{primitives}],
  materials:palette.map(([name,baseColorFactor,emissiveFactor])=>({name,pbrMetallicRoughness:{baseColorFactor,roughnessFactor:name==='iron'?.58:.88,metallicFactor:name==='iron'?.28:0},...(emissiveFactor?{emissiveFactor:emissiveFactor.map(v=>v/2),extensions:{KHR_materials_emissive_strength:{emissiveStrength:2}}}:{})})),
  extensionsUsed:['KHR_materials_emissive_strength'],buffers:[{byteLength:offset,uri:'data:application/octet-stream;base64,'+Buffer.concat(buffers).toString('base64')}],bufferViews,accessors};
fs.writeFileSync(path.join(root,'assets/terra/blocks/models/torch.gltf'),JSON.stringify(gltf)+'\n');
