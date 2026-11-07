// One-cell reinforced wooden hopper and horizontal paddle mixer.
import fs from 'node:fs';
import path from 'node:path';

const directory = path.resolve(import.meta.dirname, '../../assets/terra/blocks');
const tau = Math.PI * 2;
const palette = [
  ['oak', [.96,.96,.96,1], 0],
  ['dark_oak', [.66,.66,.66,1], 0],
  ['iron', [.86,.86,.86,1], .32],
  ['iron_edge', [1,1,1,1], .45],
  ['mixer_paddle_wood', [.94,.94,.94,1], 0],
  ['mixer_paddle_iron', [.90,.90,.90,1], .40],
  ['mixer_cog_wood', [.95,.95,.95,1], 0],
  ['mixer_cog_iron', [.95,.95,.95,1], .45],
];
const groups = palette.map(() => ({p:[], n:[], uv:[], i:[]}));
const collision = [];

function face(points, normal, material) {
  const group = groups[material], base = group.p.length / 3;
  const length = Math.hypot(...normal);
  normal = normal.map(v => v / length);
  const axis = normal.map(Math.abs).indexOf(Math.max(...normal.map(Math.abs)));
  let u = (axis + 1) % 3, v = (axis + 2) % 3;
  if (palette[material][2] === 0) {
    const extent = a => Math.max(...points.map(p => p[a])) - Math.min(...points.map(p => p[a]));
    if (extent(v) > extent(u)) [u,v] = [v,u];
  }
  for (const point of points) {
    group.p.push(...point); group.n.push(...normal);
    group.uv.push(point[u] * 1.6, point[v] * 1.6);
  }
  const a = points[1].map((v,i) => v - points[0][i]);
  const b = points[2].map((v,i) => v - points[0][i]);
  const cross = [a[1]*b[2]-a[2]*b[1],a[2]*b[0]-a[0]*b[2],a[0]*b[1]-a[1]*b[0]];
  const reverse = cross.reduce((sum,v,i) => sum + v * normal[i], 0) < 0;
  for (let i=1;i+1<points.length;i++) group.i.push(base,base+(reverse?i+1:i),base+(reverse?i:i+1));
}

function hull(x,y,z,w,h,d) { collision.push({minimum:[x,y,z],maximum:[x+w,y+h,z+d]}); }
function box(x,y,z,w,h,d,material=0,solid=false,angle=0) {
  if (solid) hull(x,y,z,w,h,d);
  const center = [x+w/2,y+h/2,z+d/2], c=Math.cos(angle),s=Math.sin(angle);
  const rotate = (p, point) => {
    const q=p.map((v,i)=>v-(point?center[i]:0));
    [q[1],q[2]]=[q[1]*c-q[2]*s,q[1]*s+q[2]*c];
    return q.map((v,i)=>v+(point?center[i]:0));
  };
  for (let axis=0;axis<3;axis++) for (const sign of [-1,1]) {
    const u=(axis+1)%3,v=(axis+2)%3,normal=[0,0,0];normal[axis]=sign;
    const low=[x,y,z],size=[w,h,d];
    face([[0,0],[1,0],[1,1],[0,1]].map(([a,b])=>{
      const p=[...low];p[axis]+=sign>0?size[axis]:0;p[u]+=a*size[u];p[v]+=b*size[v];return rotate(p,true);
    }),rotate(normal,false),material);
  }
}
function ring(x0,x1,inner,outer,material,sides=16,start=0,end=tau) {
  const point=(x,r,a)=>[x,.5+Math.cos(a)*r,.5+Math.sin(a)*r];
  for(let i=0;i<sides;i++) {
    const a=start+i*(end-start)/sides,b=start+(i+1)*(end-start)/sides,m=(a+b)/2;
    face([point(x0,outer,a),point(x1,outer,a),point(x1,outer,b),point(x0,outer,b)],[0,Math.cos(m),Math.sin(m)],material);
    if(inner>0)face([point(x0,inner,a),point(x0,inner,b),point(x1,inner,b),point(x1,inner,a)],[0,-Math.cos(m),-Math.sin(m)],material);
    for(const[x,sign]of[[x0,-1],[x1,1]]) {
      face(inner>0?[point(x,inner,a),point(x,outer,a),point(x,outer,b),point(x,inner,b)]:[[x,.5,.5],point(x,outer,a),point(x,outer,b)],[sign,0,0],material);
    }
  }
}
function panel(points,normal,thickness,material) {
  const length=Math.hypot(...normal),n=normal.map(v=>v/length),back=points.map(p=>p.map((v,i)=>v-n[i]*thickness));
  face(points,n,material);face([...back].reverse(),n.map(v=>-v),material);
  const center=[0,1,2].map(axis=>points.reduce((sum,p)=>sum+p[axis],0)/4);
  for(let i=0;i<4;i++) {
    const j=(i+1)%4,edge=points[j].map((v,a)=>v-points[i][a]);
    let side=[edge[1]*n[2]-edge[2]*n[1],edge[2]*n[0]-edge[0]*n[2],edge[0]*n[1]-edge[1]*n[0]];
    if(side.reduce((sum,v,a)=>sum+v*((points[i][a]+points[j][a])/2-center[a]),0)<0)side=side.map(v=>-v);
    face([points[i],back[i],back[j],points[j]],side,material);
  }
}
function bolt(x,y,z,front=true) {
  if(front){box(x-.018,y-.018,z-.010,.036,.036,.020,2);box(x-.008,y-.008,z+.010,.016,.016,.011,3);}
  else{box(x-.010,y-.018,z-.018,.020,.036,.036,2);box(x+.010,y-.008,z-.008,.011,.016,.016,3);}
}

// The compact frame leaves the paddles visible from the open front.
for(const x of [.055,.795])for(const z of [.065,.785]) {
  box(x,0,z,.12,.72,.12,1,true);
  for(let plank=0;plank<4;plank++)box(x-.003,.020+plank*.17,z-.003,.126,.158,.126,plank%3?0:1);
  box(x-.018,0,z-.018,.156,.055,.156,2);
  for(const y of [.11,.49,.66]){
    box(x-.009,y,z-.009,.138,.065,.138,2);
    bolt(x+.060,y+.032,z+.132);bolt(x+.132,y+.032,z+.06,false);
  }
}
for(const z of [.09,.79]) {
  box(.15,.14,z,.65,.085,.095,1,true);
  box(.14,.208,z-.006,.67,.025,.107,3);
}
box(.18,.225,.24,.62,.04,.54,1,true);
// Closed plank rear, with open front and throat rather than a solid cube.
for(let plank=0;plank<6;plank++)box(.20+plank*.098,.275,.235,.091,.37,.045,plank%3?0:1);
hull(.20,.275,.235,.59,.37,.045);
for(const x of [.19,.75]) {
  ring(x,x+.045,.188,.223,2,16);
  ring(x-.006,x+.051,.212,.230,3,16);
  box(x-.01,.655,.28,.065,.055,.44,2,true);
  for(let bolt=0;bolt<4;bolt++) {
    const a=(bolt+.5)*tau/4;
    box(x-.015,.5+Math.cos(a)*.214-.017,.5+Math.sin(a)*.214-.017,.075,.034,.034,3);
  }
}

// Individually fitted sloped hopper boards and an uninterrupted dark inner liner.
for(let side=0;side<4;side++) {
  const axis=side<2?0:2,other=side<2?2:0;
  const normal=[0,-.22,0];normal[axis]=side%2?.235:-.235;
  const point=(u,t)=>{
    const extent=.22+t*.22,p=[.5,.725+t*.235,.5];
    p[axis]=.5+(side%2?extent:-extent);p[other]=.5+(u-.5)*extent*2;return p;
  };
  for(let row=0;row<3;row++)for(let plank=0;plank<6;plank++) {
    const a=plank/6+.004,b=(plank+1)/6-.004,t0=row/3+.008,t1=(row+1)/3-.008;
    panel([point(a,t0),point(b,t0),point(b,t1),point(a,t1)],normal,.025,(row+plank)%4?0:1);
  }
  const liner=[point(0,0),point(1,0),point(1,1),point(0,1)].map(p=>p.map((v,a)=>a===1?v:v+Math.sign(.5-v)*.027));
  face(liner,normal.map(v=>-v),1);
}
for(const x of [.055,.885])box(x,.94,.055,.06,.06,.89,0);
for(const z of [.055,.885])box(.115,.94,z,.77,.06,.06,0);
for(const x of [.045,.875])for(const z of [.045,.875]) {
  box(x,.905,z,.08,.095,.08,2);bolt(x+.04,.95,z+.081);
}
for(const x of [.265,.685])box(x,.69,.265,.05,.055,.47,2);
for(const z of [.265,.685])box(.315,.69,z,.37,.055,.05,2);
hull(.055,.725,.055,.89,.275,.08);hull(.055,.725,.865,.89,.275,.08);
hull(.055,.725,.135,.08,.275,.73);hull(.865,.725,.135,.08,.275,.73);

// Downward-sloping front chute, including a real open output channel.
const chute=[[.32,.285,.69],[.68,.285,.69],[.68,.085,.985],[.32,.085,.985]];
panel(chute,[0,.295,.20],.026,0);
for(let strip=0;strip<6;strip++) {
  const z=.69+strip*.295/6,y=.285-strip*.20/6;
  hull(.32,y-.026,z,.36,.026,.295/6);
}
for(const x of [.305,.665]) {
  panel([[x,.285,.69],[x+.030,.285,.69],[x+.030,.085,.985],[x,.085,.985]],[0,.295,.20],.015,3);
  hull(x,.08,.71,.035,.22,.28);
}
box(.305,.062,.955,.39,.035,.035,2);bolt(.323,.10,.975);bolt(.677,.10,.975);

// The paddle rotor and external gear share the exact X-axis and acknowledged RPM.
ring(.18,.985,0,.052,5,12);
for(let row=0;row<3;row++)for(let blade=0;blade<3;blade++) {
  const a=(blade+row*.18)*tau/3,x=.32+row*.18,y=.5+Math.cos(a)*.102,z=.5+Math.sin(a)*.102;
  box(x-.055,y-.077,z-.026,.11,.154,.052,4,false,a);
  box(x-.036,y-.052,z-.031,.072,.04,.062,5,false,a);
}
for(const x of [.205,.765])ring(x,x+.028,.052,.075,5,12);
ring(.915,.975,.105,.157,6,16);
for(let tooth=0;tooth<8;tooth++){
  const a=tooth*tau/8;
  box(.919,.5+Math.cos(a)*.168-.025,.5+Math.sin(a)*.168-.025,.052,.050,.050,6,false,a);
}
for(let spoke=0;spoke<4;spoke++){
  const a=spoke*tau/4;
  box(.922,.5+Math.cos(a)*.081-.045,.5+Math.sin(a)*.081-.017,.049,.09,.034,6,false,a);
}
ring(.908,.924,.14,.163,7,16);ring(.971,.990,.14,.163,7,16);
ring(.971,1,0,.068,7,8);
hull(.91,.29,.29,.09,.42,.42);
hull(.22,.31,.31,.56,.38,.38);

let vertices=0;const chunks=[],views=[],accessors=[],primitives=[];let offset=0;
function buffer(data,width,integer=false){
  const bytes=Buffer.from((integer?new Uint32Array(data):new Float32Array(data)).buffer),view=views.length;
  views.push({buffer:0,byteOffset:offset,byteLength:bytes.length,target:integer?34963:34962});chunks.push(bytes);offset+=bytes.length;
  const a={bufferView:view,componentType:integer?5125:5126,count:data.length/width,type:{1:'SCALAR',2:'VEC2',3:'VEC3'}[width]};
  if(width===3){a.min=[0,1,2].map(i=>Math.min(...data.filter((_,j)=>j%3===i)));a.max=[0,1,2].map(i=>Math.max(...data.filter((_,j)=>j%3===i)));}
  accessors.push(a);return accessors.length-1;
}
groups.forEach((g,material)=>{
  if(!g.i.length)return;vertices+=g.p.length/3;
  if([...g.p,...g.n,...g.uv].some(v=>!Number.isFinite(v))||g.p.some(v=>v<-.000001||v>1.000001))throw Error(`Mixer ${palette[material][0]} leaves its finite one-cell envelope`);
  for(let i=0;i<g.i.length;i+=3){
    const[a,b,c]=g.i.slice(i,i+3).map(i=>i*3),u=[0,1,2].map(j=>g.p[b+j]-g.p[a+j]),v=[0,1,2].map(j=>g.p[c+j]-g.p[a+j]);
    const n=[u[1]*v[2]-u[2]*v[1],u[2]*v[0]-u[0]*v[2],u[0]*v[1]-u[1]*v[0]];
    if(n.reduce((sum,v,j)=>sum+v*g.n[a+j],0)<=1e-10)throw Error('Mixer contains inverted or empty triangles');
  }
  primitives.push({attributes:{POSITION:buffer(g.p,3),NORMAL:buffer(g.n,3),TEXCOORD_0:buffer(g.uv,2)},indices:buffer(g.i,1,true),material});
});
if(vertices>8192||collision.length>64)throw Error(`Mixer geometry budget: ${vertices}/${collision.length}`);
const gltf={asset:{version:'2.0',generator:'TERAMIC one-cell paddle mixer builder'},scene:0,scenes:[{nodes:[0]}],nodes:[{mesh:0}],meshes:[{primitives}],materials:palette.map(([name,baseColorFactor,metallicFactor])=>({name,pbrMetallicRoughness:{baseColorFactor,metallicFactor,roughnessFactor:metallicFactor?.76:.86}})),buffers:[{byteLength:offset,uri:'data:application/octet-stream;base64,'+Buffer.concat(chunks).toString('base64')}],bufferViews:views,accessors};
fs.writeFileSync(path.join(directory,'models/mixer.gltf'),JSON.stringify(gltf)+'\n');
fs.writeFileSync(path.join(directory,'mixer.json'),JSON.stringify({schema_version:1,id:156,name:'Mixer',description:'A mechanically driven paddle mixer with multiple inputs and a reinforced wooden hopper.',creative_category:'technic',render_kind:'custom_mesh',struct_block:true,model_layout:'scene',model:'models/mixer.gltf',hardness:2.5,resistance:8,mining_level:1,required_tool:'pickaxe',textures:Object.fromEntries(palette.map(([name,_,metal])=>[name,metal?'textures/machinery-iron-hd.png':'textures/machinery-oak-hd.png'])),collision_boxes:collision},null,2)+'\n');
console.log(`mixer: ${vertices} vertices, ${primitives.length} shared material groups, ${collision.length} collision boxes`);
