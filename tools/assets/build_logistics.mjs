// Open wooden belt ports. Geometry and conservative shell colliders share sources.
import fs from 'node:fs';
import path from 'node:path';
const root = path.resolve(import.meta.dirname, '../..');
const dir = path.join(root, 'assets/terra/blocks');
const selectedModels = new Set(process.argv.slice(2));
const basePalette = [
  ['oak', [.88,.92,.96,1], 'textures/machinery-oak-hd.png'],
  ['port_iron', [.86,.90,.96,1], 'textures/machinery-iron-hd.png'],
  ['dark_oak', [.60,.64,.67,1], 'textures/machinery-oak-hd.png'],
  ['port_arrow', [.98,.82,.30,1], 'textures/machinery-iron-hd.png'],
  ['port_support_wood', [.88,.92,.96,1], 'textures/machinery-oak-hd.png'],
  ['port_support_iron', [.86,.90,.96,1], 'textures/machinery-iron-hd.png'],
];
function model(extraPalette=[]) {
  const palette=[...basePalette,...extraPalette];
  const groups=palette.map(()=>({p:[],n:[],uv:[],i:[]})), collision=[];
  function face(points, normal, material) {
    const g=groups[material],base=g.p.length/3;
    const a=points[1].map((v,i)=>v-points[0][i]),b=points[2].map((v,i)=>v-points[0][i]);
    const cross=[a[1]*b[2]-a[2]*b[1],a[2]*b[0]-a[0]*b[2],a[0]*b[1]-a[1]*b[0]];
    const reversed=cross.reduce((sum,v,i)=>sum+v*normal[i],0)<0;
    const length=Math.hypot(...normal);normal=normal.map(v=>v/length);
    const axis=normal.map(Math.abs).indexOf(Math.max(...normal.map(Math.abs)));
    let u=(axis+1)%3,v=(axis+2)%3;
    if(material===0||material===2||material===4){
      const extent=a=>Math.max(...points.map(p=>p[a]))-Math.min(...points.map(p=>p[a]));
      if(extent(v)>extent(u))[u,v]=[v,u];
    }
    for(const p of points){g.p.push(...p);g.n.push(...normal);g.uv.push(p[u]*1.5,p[v]*1.5);}
    for(let i=1;i+1<points.length;i++)g.i.push(base,base+(reversed?i+1:i),base+(reversed?i:i+1));
  }
  function solid(points, material, collides=true) {
    const center=[0,1,2].map(i=>points.reduce((sum,p)=>sum+p[i],0)/points.length);
    for(const indices of [[0,1,2,3],[4,5,6,7],[0,4,5,1],[3,2,6,7],[0,3,7,4],[1,5,6,2]]) {
      const p=indices.map(i=>points[i]);
      const a=p[1].map((v,i)=>v-p[0][i]),b=p[2].map((v,i)=>v-p[0][i]);
      let n=[a[1]*b[2]-a[2]*b[1],a[2]*b[0]-a[0]*b[2],a[0]*b[1]-a[1]*b[0]];
      const outward=[0,1,2].map(i=>p.reduce((sum,p)=>sum+p[i],0)/p.length-center[i]);
      if(n.reduce((sum,v,i)=>sum+v*outward[i],0)<0)n=n.map(v=>-v);
      face(p,n,material);
    }
    if(collides)collision.push({minimum:[0,1,2].map(i=>Math.min(...points.map(p=>p[i]))),maximum:[0,1,2].map(i=>Math.max(...points.map(p=>p[i])))});
  }
  function box(x,y,z,w,h,d,material=0,collides=true) {
    solid([[x,y,z],[x+w,y,z],[x+w,y,z+d],[x,y,z+d],
      [x,y+h,z],[x+w,y+h,z],[x+w,y+h,z+d],[x,y+h,z+d]],material,collides);
  }
  function arrow(x,y,z,width,length,turns=0) {
    const rotate=p=>{let [x,y,z]=p;for(let i=0;i<turns;i++)[x,z]=[z,1-x];return[x,y,z];};
    const normal=n=>{let[x,y,z]=n;for(let i=0;i<turns;i++)[x,z]=[z,-x];return[x,y,z];};
    const x0=x-width*.12,x1=x+width*.12,z1=z+length*.58;
    solid([[x0,y,z],[x1,y,z],[x1,y,z1],[x0,y,z1],[x0,y+.016,z],[x1,y+.016,z],[x1,y+.016,z1],[x0,y+.016,z1]].map(rotate),3,false);
    const p=[[x-width*.5,z+length*.58],[x+width*.5,z+length*.58],[x,z+length]];
    for(const [height,sign] of [[y,-1],[y+.016,1]])face(p.map(([a,b])=>rotate([a,height,b])),[0,sign,0],3);
    for(let i=0;i<3;i++){
      const a=p[i],b=p[(i+1)%3];
      face([[a[0],y,a[1]],[b[0],y,b[1]],[b[0],y+.016,b[1]],[a[0],y+.016,a[1]]].map(rotate),
        normal([b[1]-a[1],0,a[0]-b[0]]),3);
    }
  }
  function save(name,id,label,description) {
    if(selectedModels.size&&!selectedModels.has(name))return;
    const chunks=[],views=[],accessors=[],primitives=[];let offset=0;
    function accessor(data,width,integer=false){
      const bytes=Buffer.from((integer?new Uint32Array(data):new Float32Array(data)).buffer);
      const view=views.length;views.push({buffer:0,byteOffset:offset,byteLength:bytes.length,target:integer?34963:34962});chunks.push(bytes);offset+=bytes.length;
      const value={bufferView:view,componentType:integer?5125:5126,count:data.length/width,type:{1:'SCALAR',2:'VEC2',3:'VEC3'}[width]};
      if(width===3){value.min=[0,1,2].map(i=>Math.min(...data.filter((_,j)=>j%3===i)));value.max=[0,1,2].map(i=>Math.max(...data.filter((_,j)=>j%3===i)));}
      accessors.push(value);return accessors.length-1;
    }
    for(const [material,g] of groups.entries()){
      if(!g.i.length)continue;
      for(let i=0;i<g.p.length;i++)if(!Number.isFinite(g.p[i])||g.p[i]<(i%3===1?-.375:0)-1e-6||g.p[i]>1.000001)throw Error('Port exceeds authored envelope');
      for(let i=0;i<g.i.length;i+=3){
        const [a,b,c]=g.i.slice(i,i+3).map(i=>i*3);
        const u=[0,1,2].map(j=>g.p[b+j]-g.p[a+j]),v=[0,1,2].map(j=>g.p[c+j]-g.p[a+j]);
        const n=[u[1]*v[2]-u[2]*v[1],u[2]*v[0]-u[0]*v[2],u[0]*v[1]-u[1]*v[0]];
        if(n.reduce((sum,v,j)=>sum+v*g.n[a+j],0)<=1e-10)throw Error('Port has an inverted or empty triangle');
      }
      primitives.push({attributes:{POSITION:accessor(g.p,3),NORMAL:accessor(g.n,3),TEXCOORD_0:accessor(g.uv,2)},indices:accessor(g.i,1,true),material});
    }
    const vertices=groups.reduce((n,g)=>n+g.p.length/3,0);if(vertices>4096)throw Error('Port vertex budget');
    const gltf={asset:{version:'2.0',generator:'TERAMIC open wooden logistics builder'},scene:0,scenes:[{nodes:[0]}],nodes:[{mesh:0}],materials:palette.map(([name,baseColorFactor])=>({name,pbrMetallicRoughness:{baseColorFactor,metallicFactor:name.includes('iron')?.2:0,roughnessFactor:.85}})),meshes:[{primitives}],buffers:[{byteLength:offset,uri:'data:application/octet-stream;base64,'+Buffer.concat(chunks).toString('base64')}],bufferViews:views,accessors};
    fs.writeFileSync(path.join(dir,'models',name+'.gltf'),JSON.stringify(gltf)+'\n');
    fs.writeFileSync(path.join(dir,name+'.json'),JSON.stringify({schema_version:1,id,name:label,description,hardness:2.5,resistance:8,mining_level:1,required_tool:'pickaxe',creative_category:'technic',render_kind:'custom_mesh',struct_block:true,model_layout:'scene',model:'models/'+name+'.gltf',textures:Object.fromEntries(palette.map(([n,_,t])=>[n,t])),collision_boxes:collision},null,2)+'\n');
    console.log(`${name}: ${vertices} vertices, ${collision.length} open-shell colliders`);
  }
  return {box,solid,arrow,save};
}
for(const filtered of [false,true]) {
  const m=model(filtered?[['port_filter',[.88,.95,1,1],'textures/machinery-iron-hd.png']]:[]);
  for(const x of [.075,.815]){
    m.box(x,-.305,.07,.11,.815,.12);m.box(x,-.305,.81,.11,.815,.12);
    for(let plank=0;plank<4;plank++)m.box(x,-.06,.19+plank*.155,.11,.54,.148,plank%3===0?2:0);
    for(const z of [.055,.795])m.box(x-.015,-.375,z,.14,.07,.15,1);
    m.box(x-.012,.42,.055,.134,.055,.89,1);
    for(const z of [.22,.50,.78]){
      m.box(x-.02,.14,z-.055,.15,.12,.11,1,false);
      m.box(x-.027,.179,z-.021,.164,.042,.042,1,false);
    }
    for(const z of [.08,.86])m.box(x-.018,.46,z,.146,.11,.045,1,false);
  }
  for(let z=.07;z<.9;z+=.145)m.box(.075,.505,z,.85,.08,.137);
  for(const z of [.045,.905])m.box(.06,.47,z,.88,.07,.05,1);
  m.box(.34,.59,.20,.32,.025,.65,2,false);
  m.arrow(.5,.616,.27,.27,.49);
  if(filtered){
    m.box(.32,.16,.934,.36,.24,.048,1,false);
    m.box(.35,.19,.983,.30,.18,.011,2,false);
    for(let bar=0;bar<7;bar++)m.box(.366+bar*.04,.205,.994,.009,.15,.006,6,false);
    m.box(.34,.589,.19,.32,.018,.08,1,false);
    for(let bar=0;bar<5;bar++)m.box(.37+bar*.05,.608,.205,.012,.012,.05,6,false);
  }
  m.save(filtered?'filtered_tunnel':'item_hopper',filtered?159:152,filtered?'Tunnel (Filtered)':'Tunnel',
    'A wooden belt tunnel. Its direction arrow shows item flow between the belt and the attached inventory; a wrench reverses it.');
}
for(const filtered of [false,true]) {
  const m=model(filtered?[['port_filter',[.88,.95,1,1],'textures/machinery-iron-hd.png']]:[]);
  for(let row=0;row<7;row++){
    const y0=.15+row*.65/7,y1=.15+(row+1)*.65/7;
    const r0=.21+row*.21/7,r1=.21+(row+1)*.21/7;
    for(let side=0;side<4;side++){
      const points=[[-r0,-r0],[r0,-r0],[r0,-r0+.10],[-r0,-r0+.10],
        [-r1,-r1],[r1,-r1],[r1,-r1+.10],[-r1,-r1+.10]].map(([x,z],i)=>{
          for(let turn=0;turn<side;turn++)[x,z]=[-z,x];
          return [.5+x,i<4?y0:y1,.5+z];
        });
      m.solid(points,row%3===0?2:0);
    }
  }
  for(const z of [.07,.79])m.box(.07,.80,z,.86,.16,.14);
  for(const x of [.07,.79])m.box(x,.80,.21,.14,.16,.58);
  for(const z of [.055,.915])m.box(.055,.925,z,.89,.055,.03,1);
  for(const x of [.055,.915])m.box(x,.925,.085,.03,.055,.83,1);
  for(const x of [.075,.865])for(const z of [.075,.865]){
    m.box(x,.83,z,.065,.115,.065,1,false);
    m.box(x+.014,.945,z+.014,.037,.023,.037,1,false);
  }
  for(const x of [.29,.64])m.box(x,-.02,.29,.07,.20,.42,2);
  for(const z of [.29,.64])m.box(.36,-.02,z,.28,.20,.07,2);
  for(const x of [.10,.82])for(const z of [.10,.82]){
    m.box(x,-.305,z,.08,.50,.08,4);m.box(x-.02,-.375,z-.02,.12,.07,.12,5);
  }
  for(const x of [.31,.64])m.box(x,.39,.74,.05,.22,.26,1);
  for(const y of [.39,.57])m.box(.36,y,.74,.28,.04,.26,1);
  m.arrow(.86,.981,.29,.105,.42);
  if(filtered) {
    // Removable filter cassette with an unmistakable raised funnel glyph.
    m.box(.34,.40,.87,.32,.24,.10,1,false);
    m.box(.37,.43,.969,.26,.18,.013,2,false);
    for(let bar=0;bar<6;bar++)m.box(.382+bar*.041,.445,.982,.010,.145,.009,6,false);
    m.box(.36,.785,.956,.28,.16,.025,1,false);
    m.box(.386,.910,.982,.228,.018,.012,6,false);
    for(const side of [-1,1]) {
      const x0=.5+side*.105,x1=.5+side*.024;
      m.solid([[x0-.009,.910,.982],[x0+.009,.910,.982],[x1+.009,.855,.982],[x1-.009,.855,.982],
        [x0-.009,.910,.994],[x0+.009,.910,.994],[x1+.009,.855,.994],[x1-.009,.855,.994]],6,false);
    }
    m.box(.482,.810,.982,.036,.054,.012,6,false);
    for(const x of [.33,.65])for(const y of [.405,.605])m.box(x,y,.972,.020,.020,.022,1,false);
  }
  m.save(filtered?'filtered_funnel':'item_importer',filtered?157:153,filtered?'Funnel (Filtered)':'Funnel',
    filtered?'An open wooden funnel with a reinforced removable item-filter cassette.':'An open wooden funnel mounted beside a chest or machine. Its arrow shows insertion or extraction; a wrench reverses it.');
}
for(const distributor of [false,true]) {
  const m=model();
  // Four open belt mouths share a clear item-height cross beneath the housing.
  for(const x of [.065,.835])for(const z of [.065,.835]) {
    // Contact the .18/.82 ribbon edges without entering any transport mouth.
    m.box(x-.02,-.375,z-.02,.14,.06,.14,1);
    m.box(x,-.375,z,.10,.66,.10,0);
    for(const y of [-.35,.13])m.box(x-.012,y,z-.012,.124,.06,.124,1);
    m.box(x-.018,.225,z-.018,.136,.09,.136,1);
    m.box(x+.027,.31,z+.027,.046,.018,.046,1,false);
  }
  for(const side of [0,1]) {
    for(const edge of [.055,.895]) {
      if(side===0) {
        m.box(.155,-.105,edge,.69,.085,.05,1);
        m.box(.155,.19,edge,.69,.10,.05,0);
      } else {
        m.box(edge,-.105,.155,.05,.085,.69,1);
        m.box(edge,.19,.155,.05,.10,.69,0);
      }
    }
  }
  for(let plank=0;plank<6;plank++)m.box(.16+plank*.114,.085,.16,.107,.11,.68,plank%3?0:2);
  for(const z of [.15,.805])m.box(.15,.19,z,.70,.025,.045,1);
  for(const x of [.15,.805])m.box(x,.19,.195,.045,.025,.61,1);
  m.box(.35,.211,.24,.30,.023,.56,2,false);
  if(distributor){
    m.arrow(.5,.237,.57,.14,.25);
    m.arrow(.5,.237,.57,.14,.25,1);
    m.arrow(.5,.237,.57,.14,.25,3);
    m.box(.47,.237,.26,.06,.018,.20,3,false);
    for(const turns of [0,1,3]){
      const rotate=p=>{let[x,y,z]=p;for(let i=0;i<turns;i++)[x,z]=[z,1-x];return[x,y,z];};
      for(let bar=0;bar<4;bar++){
        const x=.385+bar*.07;
        m.solid([[x,.13,.846],[x+.016,.13,.846],[x+.016,.13,.858],[x,.13,.858],
          [x,.21,.846],[x+.016,.21,.846],[x+.016,.21,.858],[x,.21,.858]].map(rotate),1,false);
      }
    }
  }else m.arrow(.5,.237,.31,.22,.42);
  // The exit receives a broad bright threshold; input mouths have paired studs.
  m.box(.27,.005,.93,.46,.055,.055,3,false);
  for(const x of [.20,.75])m.box(x,.035,.035,.05,.07,.05,1,false);
  for(const z of [.20,.75])for(const x of [.025,.925])m.box(x,.035,z,.05,.07,.05,1,false);
  m.save(distributor?'belt_distributor':'belt_merger',distributor?160:158,distributor?'Belt Distributor':'Belt Merger',distributor?'A filtered belt distributor with one rear inlet and three independently filtered left, forward and right outlets.':'A four-way belt junction. Three open inputs merge into the clearly marked forward output.');
}
