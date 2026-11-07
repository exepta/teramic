// One-cell timber-and-iron crusher. Moving material names are stable renderer ports.
import fs from 'node:fs';
import path from 'node:path';
const root=path.resolve(import.meta.dirname,'../..');
const palette=[
  ['oak',[.96,.96,.96,1],.86,0],['dark_oak',[.66,.66,.66,1],.93,0],
  ['wood_edge',[.47,.245,.095,1],.86,0],['iron',[.86,.86,.86,1],.82,.32],
  ['iron_edge',[1,1,1,1],.74,.40],['brass',[.78,.47,.17,1],.68,.22],
  ['hopper_dark',[.58,.58,.58,1],.95,.04],['lamp',[1,.61,.15,1],.65,0],
  ['crusher_roller_left',[.80,.80,.80,1],.80,.40],['crusher_roller_left_edge',[1,1,1,1],.74,.45],
  ['crusher_roller_right',[.80,.80,.80,1],.80,.40],['crusher_roller_right_edge',[1,1,1,1],.74,.45],
  ['crusher_cog',[.92,.79,.63,1],.76,.40],['crusher_cog_edge',[1,.90,.76,1],.70,.45],
  ['crank_rotating',[.88,.88,.88,1],.87,.10],['crank_rotating_edge',[1,1,1,1],.81,.12],
  ['crank_rotating_grip',[.96,.96,.96,1],.88,0],
];
const groups=palette.map(()=>({p:[],n:[],uv:[],i:[]})),collision=[];
function face(points,normal,material){
  const g=groups[material],base=g.p.length/3,length=Math.hypot(...normal);normal=normal.map(v=>v/length);
  const axis=normal.map(Math.abs).indexOf(Math.max(...normal.map(Math.abs))),scale=material<=2?1.6:.85;
  for(const p of points){g.p.push(...p);g.n.push(...normal);g.uv.push(p[(axis+1)%3]*scale,p[(axis+2)%3]*scale);}
  const u=points[1].map((v,i)=>v-points[0][i]),v=points[2].map((v,i)=>v-points[0][i]);
  const cross=[u[1]*v[2]-u[2]*v[1],u[2]*v[0]-u[0]*v[2],u[0]*v[1]-u[1]*v[0]];
  const reversed=cross.reduce((sum,v,i)=>sum+v*normal[i],0)<0;
  for(let i=1;i+1<points.length;i++)g.i.push(base,base+(reversed?i+1:i),base+(reversed?i:i+1));
}
function hull(x,y,z,w,h,d){collision.push({minimum:[x,y,z],maximum:[x+w,y+h,z+d]});}
function box(x,y,z,w,h,d,material=0,solid=false,angle=0,axis=0){
  if(solid)hull(x,y,z,w,h,d);
  const center=[x+w/2,y+h/2,z+d/2],a=(axis+1)%3,b=(axis+2)%3,c=Math.cos(angle),s=Math.sin(angle);
  function rotate(p,point){const q=p.map((v,i)=>v-(point?center[i]:0));[q[a],q[b]]=[q[a]*c-q[b]*s,q[a]*s+q[b]*c];return q.map((v,i)=>v+(point?center[i]:0));}
  for(let side=0;side<6;side++){
    const axis=side>>1,sign=side%2?1:-1,u=(axis+1)%3,v=(axis+2)%3,n=[0,0,0];n[axis]=sign;
    const low=[x,y,z],size=[w,h,d];
    face([[0,0],[1,0],[1,1],[0,1]].map(([s,t])=>{const p=[...low];p[axis]+=sign>0?size[axis]:0;p[u]+=s*size[u];p[v]+=t*size[v];return rotate(p,true);}),rotate(n,false),material);
  }
}
function cylinder(x0,x1,y,z,r,material,edge=material,sides=16){
  const ring=Array.from({length:sides},(_,i)=>{const a=i*Math.PI*2/sides;return[Math.cos(a)*r,Math.sin(a)*r];});
  for(let i=0;i<sides;i++){
    const j=(i+1)%sides,a=(i+.5)*Math.PI*2/sides;
    face([[x0,y+ring[i][0],z+ring[i][1]],[x1,y+ring[i][0],z+ring[i][1]],[x1,y+ring[j][0],z+ring[j][1]],[x0,y+ring[j][0],z+ring[j][1]]],[0,Math.cos(a),Math.sin(a)],i%4===0?edge:material);
  }
  for(const[x,sign]of[[x0,-1],[x1,1]])face(ring.map(([dy,dz])=>[x,y+dy,z+dz]),[sign,0,0],edge);
}
function rivet(x,y,z){box(x-.018,y-.018,z-.009,.036,.036,.018,3);box(x-.009,y-.009,z+.009,.018,.018,.012,4);}
// Four fitted uprights and continuous metal straps, matching the kinetic frames.
for(const x of [.055,.845])for(const z of [.065,.835]){
  box(x,0,z,.10,.79,.10,1,true);
  for(let plank=0;plank<5;plank++)box(x-.004,.025+plank*.145,z-.004,.108,.136,.108,plank%3?0:2);
  for(const y of [.025,.67]){box(x-.013,y,z-.013,.126,.075,.126,3);rivet(x+.05,y+.038,z+.12);}
}
for(const z of [.065,.835])for(const y of [.07,.69]){
  box(.06,y,z,.88,.085,.10,1,true);
  for(let plank=0;plank<7;plank++)box(.071+plank*.123,y+.009,z-.007,.116,.067,.114,plank%3?0:2);
  box(.055,y-.007,z-.012,.89,.018,.124,4);
}
// Closed rear and side panels; only the front output remains open.
box(.15,.18,.085,.70,.51,.075,1,true);
for(const x of [.075,.855]){
  box(x,.18,.16,.07,.51,.67,1,true);
  for(let plank=0;plank<5;plank++)box(x-.006,.19,.17+plank*.128,.082,.48,.120,plank%3?0:2);
  for(const y of [.21,.61])box(x-.012,y,.145,.094,.043,.70,3);
}
box(.15,.095,.18,.70,.055,.68,1,true);
for(let slat=0;slat<7;slat++)box(.16+slat*.098,.15,.19,.09,.021,.64,slat%3?0:2);
box(.145,.145,.84,.71,.04,.055,4);
// A single genuinely hollow intake, centered for the real item-input surface.
const top=[[.12,1,.12],[.88,1,.12],[.88,1,.88],[.12,1,.88]];
const bottom=[[.30,.69,.30],[.70,.69,.30],[.70,.69,.70],[.30,.69,.70]];
const innerTop=top.map(p=>[p[0]+Math.sign(.5-p[0])*.06,p[1],p[2]+Math.sign(.5-p[2])*.06]);
const innerBottom=bottom.map(p=>[p[0]+Math.sign(.5-p[0])*.035,p[1]+.01,p[2]+Math.sign(.5-p[2])*.035]);
for(let side=0;side<4;side++){
  const next=(side+1)%4,points=[bottom[side],bottom[next],top[next],top[side]];
  const u=points[1].map((v,i)=>v-points[0][i]),v=points[3].map((v,i)=>v-points[0][i]);
  let n=[u[1]*v[2]-u[2]*v[1],u[2]*v[0]-u[0]*v[2],u[0]*v[1]-u[1]*v[0]];
  const center=points.reduce((a,p)=>a.map((v,i)=>v+p[i]/4),[0,0,0]);
  if(n[0]*(center[0]-.5)+n[2]*(center[2]-.5)<0)n=n.map(v=>-v);
  face(points,n,3);face([innerBottom[side],innerBottom[next],innerTop[next],innerTop[side]],n.map(v=>-v),6);
  face([top[side],top[next],innerTop[next],innerTop[side]],[0,1,0],4);
  for(let tile=0;tile<6;tile++){
    const a=top[side],b=top[next],t=(tile+.5)/6,x=a[0]+(b[0]-a[0])*t,z=a[2]+(b[2]-a[2])*t;
    box(x-(side%2?.026:.055),.966,z-(side%2?.055:.026),side%2?.052:.11,.034,side%2?.11:.052,(tile+side)%3?4:3);
  }
}
hull(.12,.69,.12,.76,.31,.06);hull(.12,.69,.82,.76,.31,.06);
hull(.12,.69,.18,.06,.31,.64);hull(.82,.69,.18,.06,.31,.64);
// Counter-rotating rollers share the same one-cell chamber and axial coupling.
for(const[z,base]of[[.35,8],[.65,10]]){
  cylinder(.18,.82,.50,z,.105,base,base+1,16);
  for(const[x0,x1]of[[.155,.19],[.81,.845]])cylinder(x0,x1,.50,z,.13,base+1,base+1,12);
  for(let row=0;row<5;row++)for(let tooth=0;tooth<8;tooth++){
    const a=tooth*Math.PI/4+(row%2)*Math.PI/8,y=.50+Math.cos(a)*.12,p=z+Math.sin(a)*.12;
    box(.23+row*.12-.027,y-.025,p-.028,.054,.05,.056,(row+tooth)%3?base:base+1,false,a);
  }
  hull(.155,.355,z-.145,.69,.29,.29);
}
cylinder(.89,.93,.50,.5,.075,3,4,12);cylinder(.925,.97,.50,.5,.145,12,13,16);
for(let tooth=0;tooth<12;tooth++){
  const a=tooth*Math.PI/6;
  box(.927,.50+Math.cos(a)*.159-.024,.5+Math.sin(a)*.159-.028,.061,.048,.056,tooth%2?12:13,false,a);
}
cylinder(.97,.995,.50,.5,.05,13,4,8);
hull(.89,.27,.30,.11,.38,.40);
// Existing quiet lantern position and authored light contract stay stable.
box(.012,.34,.835,.062,.025,.12,3);box(.02,.369,.845,.045,.17,.10,7);box(.012,.54,.835,.062,.025,.12,4);
for(const x of [.014,.062])for(const z of [.838,.943])box(x,.365,z,.009,.178,.009,3);

function saveScene(name,definition){
  let vertices=0,triangles=0;
  for(const g of groups){
    vertices+=g.p.length/3;triangles+=g.i.length/3;
    if(g.p.some(v=>!Number.isFinite(v)||v<-.000001||v>1.000001))throw Error(`${name} exceeds its single-cell envelope`);
    for(let i=0;i<g.i.length;i+=3){
      const[a,b,c]=g.i.slice(i,i+3).map(i=>i*3),u=[0,1,2].map(j=>g.p[b+j]-g.p[a+j]),v=[0,1,2].map(j=>g.p[c+j]-g.p[a+j]);
      const n=[u[1]*v[2]-u[2]*v[1],u[2]*v[0]-u[0]*v[2],u[0]*v[1]-u[1]*v[0]];
      if(n.reduce((sum,v,j)=>sum+v*g.n[a+j],0)<=1e-10)throw Error('Crusher has inverted or empty triangles');
    }
  }
  if(vertices>8192||collision.length>64||collision.some(b=>b.minimum.some((v,i)=>v>=b.maximum[i])))throw Error('Crusher geometry/collision budget');
  const chunks=[],views=[],accessors=[],primitives=[];let offset=0;
  function buffer(data,type,target,integer=false){
    const bytes=Buffer.from((integer?new Uint32Array(data):new Float32Array(data)).buffer),view=views.length;
    views.push({buffer:0,byteOffset:offset,byteLength:bytes.length,target});chunks.push(bytes);offset+=bytes.length;
    const width={VEC3:3,VEC2:2,SCALAR:1}[type],a={bufferView:view,componentType:integer?5125:5126,count:data.length/width,type};
    if(type==='VEC3'){a.min=[0,1,2].map(i=>Math.min(...data.filter((_,j)=>j%3===i)));a.max=[0,1,2].map(i=>Math.max(...data.filter((_,j)=>j%3===i)));}
    accessors.push(a);return accessors.length-1;
  }
  groups.forEach((g,material)=>{if(g.i.length)primitives.push({attributes:{POSITION:buffer(g.p,'VEC3',34962),NORMAL:buffer(g.n,'VEC3',34962),TEXCOORD_0:buffer(g.uv,'VEC2',34962)},indices:buffer(g.i,'SCALAR',34963,true),material});});
  const gltf={asset:{version:'2.0',generator:'TERAMIC single-cell crusher builder'},scene:0,scenes:[{nodes:[0]}],nodes:[{mesh:0}],meshes:[{primitives}],materials:palette.map(([name,baseColorFactor,roughnessFactor,metallicFactor])=>({name,pbrMetallicRoughness:{baseColorFactor,roughnessFactor,metallicFactor},...(name==='lamp'?{emissiveFactor:[.35,.12,.018]}:{})})),buffers:[{byteLength:offset,uri:'data:application/octet-stream;base64,'+Buffer.concat(chunks).toString('base64')}],bufferViews:views,accessors};
  fs.writeFileSync(path.join(root,`assets/terra/blocks/models/${name}.gltf`),JSON.stringify(gltf)+'\n');
  if(definition)fs.writeFileSync(path.join(root,`assets/terra/blocks/${name}.json`),JSON.stringify({...definition,collision_boxes:collision},null,2)+'\n');
  console.log(`${name}: ${vertices} vertices, ${triangles} triangles, ${collision.length} collision boxes, ${primitives.length} shared material groups`);
}
const metalTextures=Object.fromEntries(palette.filter(([name])=>name==='iron'||name==='iron_edge'||name==='hopper_dark'||name.startsWith('crusher_')).map(([name])=>[name,'textures/machinery-iron-hd.png']));
saveScene('crusher',{schema_version:1,id:99,name:'Block Crusher',description:'A compact one-block mechanical crusher. Twin toothed rollers grind minerals and plants into dust and dye.',creative_category:'technic',render_kind:'custom_mesh',struct_block:true,model_layout:'scene',model:'models/crusher.gltf',hardness:2.5,resistance:8,required_tool:'pickaxe',mining_level:1,light:{color:[1,.76,.45],intensity:65,range:4,offset:[.0425,.454,.896],shadows:false},textures:{oak:'textures/machinery-oak-hd.png',dark_oak:'textures/machinery-oak-hd.png',...metalTextures}});

// The existing reusable crank retains its normalized socket and rigid material names.
for(const group of groups)for(const key of['p','n','uv','i'])group[key].length=0;
collision.length=0;
box(.012,.205,.325,.068,.510,.380,1,true);box(.078,.220,.335,.044,.480,.360,3);
for(const z of[.325,.670])box(.083,.212,z,.045,.496,.035,4);
for(const y of[.220,.665])box(.083,y,.335,.045,.035,.350,4);
for(const y of[.265,.660])for(const z of[.365,.665]){box(.124,y-.024,z-.024,.025,.048,.048,3);box(.148,y-.014,z-.014,.018,.028,.028,5);}
cylinder(.112,.185,.455,.515,.106,3,4,12);cylinder(.185,.245,.455,.515,.126,14,15,12);
for(let tooth=0;tooth<10;tooth++){const a=tooth*Math.PI/5;box(.187,.455+Math.cos(a)*.128-.022,.515+Math.sin(a)*.128-.023,.060,.044,.046,tooth%2?14:15,false,a);}
box(.245,.426,.480,.064,.248,.070,14);box(.255,.431,.474,.047,.237,.011,15);
cylinder(.307,.357,.640,.515,.050,14,15,8);cylinder(.355,.402,.640,.515,.056,15,15,8);cylinder(.403,.662,.640,.515,.049,16,16,12);
for(const x of[.411,.451,.491,.531,.571,.611])cylinder(x,x+.018,.640,.515,.050,16,16,8);
cylinder(.665,.700,.640,.515,.056,15,15,8);cylinder(.701,.719,.640,.515,.024,15,15,8);
hull(.112,.325,.385,.135,.260,.260);hull(.245,.426,.474,.064,.248,.081);hull(.355,.584,.459,.364,.112,.112);
saveScene('hand_crank',null);
