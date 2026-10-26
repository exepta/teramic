// Authored deterministic scene geometry. The existing paired furniture path
// expands normalized X by two; no logical block-size changes are involved.
import fs from 'node:fs';
import path from 'node:path';
const root = path.resolve(import.meta.dirname, '../..');
const palette = [
  ['oak', [.96,.96,.96,1], .86, 0],
  ['dark_oak', [.66,.66,.66,1], .93, 0],
  ['wood_edge', [.47,.245,.095,1], .86, 0],
  ['iron', [.86,.86,.86,1], .90, .08],
  ['iron_edge', [1,1,1,1], .84, .10],
  ['brass', [.78,.47,.17,1], .68, .22],
  ['hopper_dark', [.58,.58,.58,1], .95, .04],
  ['lamp', [1,.61,.15,1], .65, 0],
  ['crusher_roller_left', [.80,.80,.80,1], .88, .10],
  ['crusher_roller_left_edge', [1,1,1,1], .82, .12],
  ['crusher_roller_right', [.80,.80,.80,1], .88, .10],
  ['crusher_roller_right_edge', [1,1,1,1], .82, .12],
  ['crusher_cog', [.92,.79,.63,1], .80, .12],
  ['crusher_cog_edge', [1,.90,.76,1], .76, .15],
  ['crank_rotating', [.88,.88,.88,1], .87, .10],
  ['crank_rotating_edge', [1,1,1,1], .81, .12],
  ['crank_rotating_grip', [.96,.96,.96,1], .88, 0],
];
const groups = palette.map(() => ({p:[],n:[],uv:[],i:[]}));
const collision = [];
let xScale=.5;
function face(points, normal, material) {
  const g=groups[material], base=g.p.length/3;
  const unit=Math.hypot(...normal); normal=normal.map(v=>v/unit);
  for(const p of points) {
    g.p.push(p[0]*xScale,p[1],p[2]);
    const n=[normal[0]/xScale,normal[1],normal[2]], length=Math.hypot(...n);
    g.n.push(...n.map(v=>v/length));
    const axis=normal.map(Math.abs).indexOf(Math.max(...normal.map(Math.abs)));
    const scale=material===0||material===1?1.6:0.52;
    g.uv.push(p[(axis+1)%3]*scale,p[(axis+2)%3]*scale);
  }
  const u=points[1].map((v,i)=>v-points[0][i]),v=points[2].map((v,i)=>v-points[0][i]);
  const c=[u[1]*v[2]-u[2]*v[1],u[2]*v[0]-u[0]*v[2],u[0]*v[1]-u[1]*v[0]];
  const reverse=c.reduce((sum,v,i)=>sum+v*normal[i],0)<0;
  for(let i=1;i+1<points.length;i++)g.i.push(base,base+(reverse?i+1:i),base+(reverse?i:i+1));
}
function hull(x,y,z,w,h,d) {
  collision.push({minimum:[x*xScale,y,z],maximum:[(x+w)*xScale,y+h,z+d]});
}
function box(x,y,z,w,h,d,mat=0,solid=false,angle=0,axis=0) {
  if(solid)hull(x,y,z,w,h,d);
  const center=[x+w/2,y+h/2,z+d/2];
  function rotate(p,point=true) {
    const a=(axis+1)%3,b=(axis+2)%3,c=Math.cos(angle),s=Math.sin(angle),r=p.map((v,i)=>v-(point?center[i]:0));
    [r[a],r[b]]=[r[a]*c-r[b]*s,r[a]*s+r[b]*c];
    return r.map((v,i)=>v+(point?center[i]:0));
  }
  for(let a=0;a<3;a++)for(const sign of [-1,1]) {
    const u=(a+1)%3,v=(a+2)%3,normal=[0,0,0];normal[a]=sign;
    const low=[x,y,z],size=[w,h,d];
    face([[0,0],[1,0],[1,1],[0,1]].map(([s,t])=>{
      const p=[...low];p[a]+=sign>0?size[a]:0;p[u]+=s*size[u];p[v]+=t*size[v];return rotate(p);
    }),rotate(normal,false),mat);
  }
}
function rivet(x,y,z,side=1,mat=4) {
  box(x-.020,y-.020,z-.008,.040,.040,.016,3);
  box(x-.010,y-.010,z+.007*side,.020,.020,.017,mat);
}
function cylinder(x0,x1,y,z,r,material,edge=material,sides=16) {
  const ring=Array.from({length:sides},(_,i)=>{const a=i*Math.PI*2/sides;return [Math.cos(a)*r,Math.sin(a)*r];});
  for(let i=0;i<sides;i++){
    const j=(i+1)%sides,a=(i+.5)*Math.PI*2/sides;
    face([[x0,y+ring[i][0],z+ring[i][1]],[x1,y+ring[i][0],z+ring[i][1]],[x1,y+ring[j][0],z+ring[j][1]],[x0,y+ring[j][0],z+ring[j][1]]],[0,Math.cos(a),Math.sin(a)],i%4===0?edge:material);
  }
  for(const [x,sign]of[[x0,-1],[x1,1]])face(ring.map(([dy,dz])=>[x,y+dy,z+dz]),[sign,0,0],edge);
}
function wallPoint(points,u,v) {
  const low=points[0].map((value,i)=>value+(points[1][i]-value)*u);
  const high=points[3].map((value,i)=>value+(points[2][i]-value)*u);
  return low.map((value,i)=>value+(high[i]-value)*v);
}
function raisedPatch(points,normal,height,material) {
  const length=Math.hypot(...normal),n=normal.map(v=>v/length);
  const raised=points.map(p=>p.map((v,i)=>v+n[i]*height));
  face(raised,n,material);
  const center=points.reduce((sum,p)=>sum.map((v,i)=>v+p[i]/4),[0,0,0]);
  for(let i=0;i<4;i++){
    const j=(i+1)%4,edge=points[j].map((v,a)=>v-points[i][a]);
    let side=[edge[1]*n[2]-edge[2]*n[1],edge[2]*n[0]-edge[0]*n[2],edge[0]*n[1]-edge[1]*n[0]];
    if(side.reduce((sum,v,a)=>sum+v*((points[i][a]+points[j][a])/2-center[a]),0)<0)side=side.map(v=>-v);
    face([points[i],points[j],raised[j],raised[i]],side,3);
  }
}
// Three heavy timber uprights and two full-depth end frames.
for(const x of [.060,.955,1.85])for(const z of [.085,.805]){
  box(x,0,z,.090,.91,.110,1,true);
  for(let course=0;course<9;course++) {
    box(x-.006,.027+course*.092,z-.005,.102,.085,.120,course%3===0?2:0);
    if(course%3===1)box(x+.014,.034+course*.092,z+.116,.021,.058,.007,1);
  }
  for(const y of [.015,.780]){
    box(x-.023,y,z-.020,.136,.100,.151,3);
    box(x-.027,y+.013,z-.027,.144,.074,.017,4);
    rivet(x+.045,y+.049,z-.033,-1,5);
    rivet(x+.045,y+.049,z+.140,1,5);
  }
  for(const y of [.185,.675]) {
    box(x-.015,y,z-.011,.122,.039,.135,5);
    rivet(x+.045,y+.021,z+.130,1,4);
  }
}
for(const z of [.085,.815])for(const y of [.055,.805]){
  const height=y>.5?.125:.100;
  box(.065,y,z,1.875,height,.095,1,true);
  for(let segment=0;segment<16;segment++)box(.069+segment*.116,y+.011,z-.009,.112,height-.022,.116,(segment+Math.round(y*10))%3===0?2:0);
  box(.073,y-.009,z-.018,1.855,.019,.132,3);
  box(.073,y+height-.007,z-.018,1.855,.019,.132,4);
  if(z>.5)box(.079,y+.015,z+.107,1.843,height-.030,.021,3);
  for(const x of [.112,.535,1.0,1.465,1.895])rivet(x,y+.047,z+.113,1,4);
}
for(const x of [.07,1.84])for(const y of [.08,.795])box(x,y,.18,.10,.085,.64,0,true);
// A solid timber housing, rather than a scaffold: the crushing mouths stay
// open in front, but neither grass nor sky can show through behind the rolls.
box(.155,.180,.120,1.690,.650,.080,1,true);
for(const x of [.130,1.800]){
  box(x,.180,.200,.070,.650,.610,0,true);
  for(const y of [.210,.745])box(x-.007,y,.190,.084,.064,.630,3);
}
box(.966,.180,.200,.068,.650,.610,1,true);
for(const center of [.545,1.455]){
  box(center-.345,.672,.205,.690,.185,.555,1,true);
  for(const z of [.240,.740])rivet(center,.773,z,1,4);
}
// Recessed slatted output trays; the mouth stays open in front of the rollers.
for(const center of [.545,1.455]){
  box(center-.350,.110,.18,.700,.055,.61,1,true);
  for(let slat=0;slat<8;slat++)box(center-.340+slat*.085,.168,.19,.080,.023,.595,slat%3===0?2:0);
  box(center-.355,.150,.79,.710,.038,.071,4);
}
// Sloped hollow hoppers are built from real inner and outer walls, not solid boxes.
for(const center of [.545,1.455]){
  const top=[[center-.330,1.198,.19],[center+.330,1.198,.19],[center+.330,1.198,.81],[center-.330,1.198,.81]];
  const bottom=[[center-.160,.902,.35],[center+.160,.902,.35],[center+.160,.902,.65],[center-.160,.902,.65]];
  const innerTop=top.map(p=>[p[0]+Math.sign(center-p[0])*.034,p[1],p[2]+Math.sign(.5-p[2])*.034]);
  const innerBottom=bottom.map(p=>[p[0]+Math.sign(center-p[0])*.025,p[1]+.015,p[2]+Math.sign(.5-p[2])*.025]);
  for(let side=0;side<4;side++){
    const next=(side+1)%4, points=[bottom[side],bottom[next],top[next],top[side]];
    const u=points[1].map((v,i)=>v-points[0][i]),v=points[3].map((v,i)=>v-points[0][i]);
    let n=[u[1]*v[2]-u[2]*v[1],u[2]*v[0]-u[0]*v[2],u[0]*v[1]-u[1]*v[0]];
    const middle=points.reduce((a,p)=>a.map((v,i)=>v+p[i]/4),[0,0,0]);
    if(n[0]*(middle[0]-center)+n[2]*(middle[2]-.5)<0)n=n.map(v=>-v);
    face(points,n,3);
    const inside=[innerBottom[side],innerBottom[next],innerTop[next],innerTop[side]];
    face(inside,n.map(v=>-v),6);
    // Raised, staggered cast courses give the sloping bowl a voxel-built
    // surface while preserving a genuinely hollow, continuous funnel.
    for(let row=0;row<3;row++)for(let column=0;column<5;column++) {
      const u0=(column+.035)/5,u1=(column+.965)/5,v0=(row+.045)/3,v1=(row+.955)/3;
      const patch=[[u0,v0],[u1,v0],[u1,v1],[u0,v1]].map(([u,v])=>wallPoint(points,u,v));
      raisedPatch(patch,n,.003+((row+column+side)%3)*.002,(row*3+column+side)%4===0?3:4);
    }
    for(let row=0;row<1;row++)for(let column=0;column<4;column++) {
      const u0=(column+.045)/4,u1=(column+.955)/4,v0=.06,v1=.94;
      const length=Math.hypot(...n),inner=n.map(v=>-v/length);
      const patch=[[u0,v0],[u1,v0],[u1,v1],[u0,v1]].map(([u,v])=>wallPoint(inside,u,v).map((value,i)=>value+inner[i]*.0015));
      face(patch,inner,(row+column+side)%3===0?3:6);
    }
    face([top[side],top[next],innerTop[next],innerTop[side]],[0,1,0],4);
    const a=top[side],b=top[next];
    for(let tile=0;tile<7;tile++){
      const t=(tile+.5)/7,x=a[0]+(b[0]-a[0])*t,z=a[2]+(b[2]-a[2])*t;
      box(x-(side%2?.035:.056),1.198,z-(side%2?.056:.035),side%2?.070:.112,.032,side%2?.112:.070,(tile+side)%3===0?3:4);
    }
  }
  hull(center-.33,.90,.19,.66,.33,.045);hull(center-.33,.90,.765,.66,.33,.045);
  hull(center-.33,.90,.235,.055,.33,.530);hull(center+.275,.90,.235,.055,.33,.530);
  for(const z of [.295,.675])box(center-.245,.862,z,.49,.050,.03,4,true);
}
// Two interleaved toothed rollers. Separate material names preserve rigid
// submeshes in the existing importer; runtime shares six meshes for all machines.
for(const [center,base]of[[.545,8],[1.455,10]]){
  cylinder(center-.34,center+.34,.455,.515,.135,base,base+1);
  cylinder(center-.38,center-.335,.455,.515,.154,base+1);
  cylinder(center+.335,center+.38,.455,.515,.154,base+1);
  for(let row=0;row<6;row++){
    const x=center-.285+row*.114;
    for(let tooth=0;tooth<8;tooth++){
      const angle=tooth*Math.PI/4+(row%2)*Math.PI/8;
      const y=.455+Math.cos(angle)*.165,z=.515+Math.sin(angle)*.165;
      box(x-.034,y-.035,z-.040,.068,.070,.080,(row+tooth)%3===0?base:base+1,false,angle,0);
    }
  }
  hull(center-.38,.24,.285,.76,.43,.46);
}
// Side drive gear, open spokes and dark bearing behind it.
cylinder(1.866,1.933,.455,.515,.095,3,4);
cylinder(1.932,1.969,.455,.515,.196,12,13);
for(let tooth=0;tooth<14;tooth++){
  const a=tooth*Math.PI*2/14,y=.455+Math.cos(a)*.219,z=.515+Math.sin(a)*.219;
  box(1.930,y-.039,z-.045,.066,.078,.090,tooth%2?12:13,false,a,0);
}
for(let spoke=0;spoke<6;spoke++){
  const a=spoke*Math.PI/3;
  box(1.969,.455-.018,.515-.170,.018,.036,.34,13,false,a,0);
}
cylinder(1.971,1.997,.455,.515,.064,13,4,8);
hull(1.92,.18,.24,.08,.55,.55);
// Quiet amber lantern on the left front corner.
box(.014,.313,.824,.145,.050,.145,3);
box(.027,.365,.837,.116,.177,.118,7);
box(.014,.542,.824,.145,.035,.145,4);
for(const x of [.018,.143])for(const z of [.828,.949])box(x,.357,z,.014,.191,.014,3);
box(.054,.577,.868,.064,.035,.052,3);
box(.066,.612,.878,.040,.018,.032,5);

function saveScene(name,definition,maximumY=1.23){
let vertices=0,triangles=0;
for(const g of groups){
  vertices+=g.p.length/3;triangles+=g.i.length/3;
  if(g.p.some((v,i)=>!Number.isFinite(v)||v<-.000001||v>(i%3===1?maximumY+.000001:1.000001)))throw Error(`${name} exceeds occupied cells`);
  for(let i=0;i<g.i.length;i+=3){
    const [a,b,c]=g.i.slice(i,i+3).map(i=>i*3),u=[0,1,2].map(j=>g.p[b+j]-g.p[a+j]),v=[0,1,2].map(j=>g.p[c+j]-g.p[a+j]);
    const n=[u[1]*v[2]-u[2]*v[1],u[2]*v[0]-u[0]*v[2],u[0]*v[1]-u[1]*v[0]];
    if(n.reduce((sum,v,j)=>sum+v*g.n[a+j],0)<=1e-10)throw Error('Crusher has inverted or empty triangles');
  }
}
if(vertices>16384||collision.length>128||collision.some(b=>b.minimum.some((v,i)=>v>=b.maximum[i])))throw Error('Crusher geometry/collision budget');
const chunks=[],views=[],accessors=[],primitives=[];let offset=0;
function buffer(data,type,target,integer=false){
  const bytes=Buffer.from((integer?new Uint32Array(data):new Float32Array(data)).buffer),view=views.length;
  views.push({buffer:0,byteOffset:offset,byteLength:bytes.length,target});chunks.push(bytes);offset+=bytes.length;
  const width={VEC3:3,VEC2:2,SCALAR:1}[type],a={bufferView:view,componentType:integer?5125:5126,count:data.length/width,type};
  if(type==='VEC3'){a.min=[0,1,2].map(i=>Math.min(...data.filter((_,j)=>j%3===i)));a.max=[0,1,2].map(i=>Math.max(...data.filter((_,j)=>j%3===i)));}
  accessors.push(a);return accessors.length-1;
}
groups.forEach((g,material)=>{if(g.i.length)primitives.push({attributes:{POSITION:buffer(g.p,'VEC3',34962),NORMAL:buffer(g.n,'VEC3',34962),TEXCOORD_0:buffer(g.uv,'VEC2',34962)},indices:buffer(g.i,'SCALAR',34963,true),material});});
const gltf={asset:{version:'2.0',generator:'TERAMIC crusher builder'},scene:0,scenes:[{nodes:[0]}],nodes:[{mesh:0}],meshes:[{primitives}],materials:palette.map(([name,baseColorFactor,roughnessFactor,metallicFactor])=>({name,pbrMetallicRoughness:{baseColorFactor,roughnessFactor,metallicFactor},...(name==='lamp'?{emissiveFactor:[.35,.12,.018]}:{})})),buffers:[{byteLength:offset,uri:'data:application/octet-stream;base64,'+Buffer.concat(chunks).toString('base64')}],bufferViews:views,accessors};
fs.writeFileSync(path.join(root,`assets/terra/blocks/models/${name}.gltf`),JSON.stringify(gltf)+'\n');
if(definition)fs.writeFileSync(path.join(root,`assets/terra/blocks/${name}.json`),JSON.stringify({...definition,collision_boxes:collision},null,2)+'\n');
console.log(`${name}: ${vertices} vertices, ${triangles} triangles, ${collision.length} collision boxes, ${primitives.length} shared material groups`);
}
const metalTextures=Object.fromEntries(palette.filter(([name])=>name==='iron'||name==='iron_edge'||name==='hopper_dark'||name.startsWith('crusher_')).map(([name])=>[name,'textures/forged-iron-warm.png']));
saveScene('crusher',{schema_version:1,id:99,name:'Block Crusher',description:'A two-block mechanical crusher. Twin toothed rollers grind minerals and plants into dust and dye.',creative_category:'decoration',render_kind:'custom_mesh',struct_block:true,model_layout:'scene',model:'models/crusher.gltf',hardness:2.5,resistance:8,required_tool:'pickaxe',mining_level:1,textures:{oak:'textures/oak-crafted-hd.png',dark_oak:'textures/oak-crafted-hd.png',...metalTextures}});

// One reusable crank scene; its fixed mounting plate remains in the section
// batch, while three named material groups share one animated pivot.
for(const group of groups)for(const key of ['p','n','uv','i'])group[key].length=0;
collision.length=0;xScale=1;
box(.012,.205,.325,.068,.510,.380,1,true);
box(.078,.220,.335,.044,.480,.360,3);
for(const z of [.325,.670])box(.083,.212,z,.045,.496,.035,4);
for(const y of [.220,.665])box(.083,y,.335,.045,.035,.350,4);
for(const y of [.265,.660])for(const z of [.365,.665]){
  box(.124,y-.024,z-.024,.025,.048,.048,3);
  box(.148,y-.014,z-.014,.018,.028,.028,5);
}
cylinder(.112,.185,.455,.515,.106,3,4,12);
cylinder(.185,.245,.455,.515,.126,14,15,12);
for(let tooth=0;tooth<10;tooth++){
  const a=tooth*Math.PI/5;
  box(.187,.455+Math.cos(a)*.128-.022,.515+Math.sin(a)*.128-.023,.060,.044,.046,tooth%2?14:15,false,a,0);
}
// Offset forged arm and rounded-octagonal timber grip.
box(.245,.426,.480,.064,.248,.070,14);
box(.255,.431,.474,.047,.237,.011,15);
cylinder(.307,.357,.640,.515,.050,14,15,8);
cylinder(.355,.402,.640,.515,.056,15,15,8);
cylinder(.403,.662,.640,.515,.049,16,16,12);
for(const x of [.411,.451,.491,.531,.571,.611])cylinder(x,x+.018,.640,.515,.050,16,16,8);
cylinder(.665,.700,.640,.515,.056,15,15,8);
cylinder(.701,.719,.640,.515,.024,15,15,8);
hull(.112,.325,.385,.135,.260,.260);
hull(.245,.426,.474,.064,.248,.081);
hull(.355,.584,.459,.364,.112,.112);
saveScene('hand_crank',null,1.0);
