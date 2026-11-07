// Deterministic mechanical assemblies. Moving material groups share
// renderer meshes; tooth and spoke detail never creates individual entities.
import fs from 'node:fs';
import path from 'node:path';

const root = path.resolve(import.meta.dirname, '../..');
const directory = path.join(root, 'assets/terra/blocks');
const selectedModels = process.argv.slice(2);
const tau = Math.PI * 2;
const variants = [
  ['wooden', 'Wooden', 'textures/machinery-oak-hd.png', [.95, .95, .95, 1], 0],
  ['stone', 'Stone', 'textures/stone-strata-hd.png', [.84, .87, .89, 1], 1],
  ['iron', 'Iron', 'textures/machinery-iron-hd.png', [.86, .90, .94, 1], 1],
];

function assembly(prefix, texture, color, radius = .5, cross = null) {
  const palette = [
    [prefix, color, texture],
    [cross ?? prefix + '_edge', [.82, .86, .90, 1], 'textures/machinery-iron-hd.png'],
    ['oak_frame', [.83, .88, .90, 1], 'textures/machinery-oak-hd.png'],
    ['iron_frame', [.80, .84, .89, 1], 'textures/machinery-iron-hd.png'],
    ...(cross ? [
      ['kinetic_box_nx', [.84,.88,.92,1], 'textures/machinery-iron-hd.png'],
      [cross==='kinetic_box_y'?'kinetic_box_ny':'kinetic_box_nz', [.82,.86,.90,1], 'textures/machinery-iron-hd.png'],
    ] : []),
  ];
  const groups = palette.map(() => ({p: [], n: [], uv: [], i: []}));
  const collision = [];
  function face(points, normal, material) {
    const g = groups[material], base = g.p.length / 3;
    const length = Math.hypot(...normal);
    normal = normal.map(v => v / length);
    const axis = normal.map(Math.abs).indexOf(Math.max(...normal.map(Math.abs)));
    let u=(axis+1)%3,v=(axis+2)%3;
    if(prefix==='kinetic_belt'&&material===0&&axis===1){u=0;v=2;}
    if(prefix==='kinetic_shaft_wooden'&&material===0&&axis!==0){u=0;v=axis===1?2:1;}
    if(material===2) {
      const extent=a=>Math.max(...points.map(p=>p[a]))-Math.min(...points.map(p=>p[a]));
      if(extent(v)>extent(u))[u,v]=[v,u];
    }
    for (const p of points) {
      g.p.push(...p); g.n.push(...normal);
      g.uv.push(p[u] * 1.4, p[v] * 1.4);
    }
    const a = points[1].map((v, i) => v - points[0][i]);
    const b = points[2].map((v, i) => v - points[0][i]);
    const c = [a[1]*b[2]-a[2]*b[1], a[2]*b[0]-a[0]*b[2], a[0]*b[1]-a[1]*b[0]];
    const reverse = c.reduce((sum, v, i) => sum + v * normal[i], 0) < 0;
    for (let i = 1; i + 1 < points.length; i++) g.i.push(base, base + (reverse ? i+1 : i), base + (reverse ? i : i+1));
  }
  function box(x, y, z, w, h, d, material = 0, angle = 0) {
    const center = [x+w/2, y+h/2, z+d/2];
    const rotate = (p, point) => {
      const q = p.map((v, i) => v - (point ? center[i] : 0));
      const c = Math.cos(angle), s = Math.sin(angle);
      [q[1], q[2]] = [q[1]*c-q[2]*s, q[1]*s+q[2]*c];
      return q.map((v, i) => v + (point ? center[i] : 0));
    };
    for (let a = 0; a < 3; a++) for (const sign of [-1, 1]) {
      const u = (a+1)%3, v = (a+2)%3, n = [0,0,0]; n[a] = sign;
      const low = [x,y,z], size = [w,h,d];
      face([[0,0],[1,0],[1,1],[0,1]].map(([s,t]) => {
        const p = [...low]; p[a] += sign > 0 ? size[a] : 0;
        p[u] += s * size[u]; p[v] += t * size[v]; return rotate(p, true);
      }), rotate(n, false), material);
    }
  }
  function panel(points, normal, depth, material = 0) {
    const length=Math.hypot(...normal), n=normal.map(v=>v/length);
    const back=points.map(p=>p.map((v,i)=>v-n[i]*depth));
    const center=[0,1,2].map(axis=>points.reduce((sum,p)=>sum+p[axis],0)/4);
    face(points,n,material);face([...back].reverse(),n.map(v=>-v),material);
    for(let i=0;i<4;i++) {
      const next=(i+1)%4, edge=points[next].map((v,a)=>v-points[i][a]);
      let side=[edge[1]*n[2]-edge[2]*n[1],edge[2]*n[0]-edge[0]*n[2],edge[0]*n[1]-edge[1]*n[0]];
      if(side.reduce((sum,v,a)=>sum+v*((points[i][a]+points[next][a])/2-center[a]),0)<0)side=side.map(v=>-v);
      face([points[i],back[i],back[next],points[next]],side,material);
    }
  }
  function ring(x0, x1, inner, outer, material = 0, sides = 24, start = 0, end = tau, center = [.5,.5]) {
    for (let i = 0; i < sides; i++) {
      const a = start+i*(end-start)/sides, b = start+(i+1)*(end-start)/sides, mid = (a+b)/2;
      const point = (x,r,t) => [x, center[0]+Math.cos(t)*r, center[1]+Math.sin(t)*r];
      face([point(x0,outer,a),point(x1,outer,a),point(x1,outer,b),point(x0,outer,b)], [0,Math.cos(mid),Math.sin(mid)], material);
      if (inner > 0) face([point(x0,inner,a),point(x0,inner,b),point(x1,inner,b),point(x1,inner,a)], [0,-Math.cos(mid),-Math.sin(mid)], material);
      for (const [x, sign] of [[x0,-1],[x1,1]]) {
        if (inner === 0) face([[x,...center],point(x,outer,a),point(x,outer,b)], [sign,0,0], material);
        else face([point(x,inner,a),point(x,outer,a),point(x,outer,b),point(x,inner,b)], [sign,0,0], material);
      }
    }
    if (end-start < tau-.00001) {
      for (const [angle, sign] of [[start,-1],[end,1]]) {
        const p=(x,r)=>[x,center[0]+Math.cos(angle)*r,center[1]+Math.sin(angle)*r];
        face([p(x0,inner),p(x1,inner),p(x1,outer),p(x0,outer)], [0,-Math.sin(angle)*sign,Math.cos(angle)*sign], material);
      }
    }
  }
  function spoke(angle, radius, width = .055, material = 0) {
    const r = radius/2;
    box(.4475, .5+Math.cos(angle)*r-radius/2, .5+Math.sin(angle)*r-width/2, .105, radius, width, material, angle);
  }
  function ringAxis(axis,...args) {
    const starts=groups.map(g=>g.p.length);
    ring(...args);
    if(axis===0)return;
    groups.forEach((g,group)=>{
      for(let i=starts[group];i<g.p.length;i+=3){
        const [x,y,z]=g.p.slice(i,i+3),[nx,ny,nz]=g.n.slice(i,i+3);
        g.p.splice(i,3,...(axis===1?[1-y,x,z]:[1-z,y,x]));
        g.n.splice(i,3,...(axis===1?[-ny,nx,nz]:[-nz,ny,nx]));
      }
    });
  }
  function hull(minimum, maximum) { collision.push({minimum, maximum}); }
  function save(name, definition) {
    if (selectedModels.length && !selectedModels.includes(name)) return;
    const chunks = [], views = [], accessors = [], primitives = []; let offset = 0;
    function buffer(data, type, target, integer = false) {
      const bytes = Buffer.from((integer ? new Uint32Array(data) : new Float32Array(data)).buffer);
      const view = views.length;
      views.push({buffer:0,byteOffset:offset,byteLength:bytes.length,target}); chunks.push(bytes); offset += bytes.length;
      const width = {VEC3:3,VEC2:2,SCALAR:1}[type];
      const accessor = {bufferView:view,componentType:integer?5125:5126,count:data.length/width,type};
      if (type === 'VEC3') {
        accessor.min = [0,1,2].map(i => Math.min(...data.filter((_,j) => j%3===i)));
        accessor.max = [0,1,2].map(i => Math.max(...data.filter((_,j) => j%3===i)));
      }
      accessors.push(accessor); return accessors.length-1;
    }
    let vertices = 0;
    groups.forEach((g, material) => {
      if (!g.i.length) return;
      vertices += g.p.length/3;
      for (const value of [...g.p,...g.n,...g.uv]) if (!Number.isFinite(value)) throw Error('Nonfinite kinetic vertex');
      for (let i=0;i<g.i.length;i+=3) {
        const [a,b,c] = g.i.slice(i,i+3).map(i=>i*3);
        const u=[0,1,2].map(j=>g.p[b+j]-g.p[a+j]),v=[0,1,2].map(j=>g.p[c+j]-g.p[a+j]);
        const n=[u[1]*v[2]-u[2]*v[1],u[2]*v[0]-u[0]*v[2],u[0]*v[1]-u[1]*v[0]];
        if(n.reduce((sum,v,j)=>sum+v*g.n[a+j],0)<=1e-10) throw Error('Inverted or empty kinetic triangle');
      }
      for(let i=0;i<g.p.length;i++) {
        const low=i%3===0?0:.5-radius, high=i%3===0?1:.5+radius;
        if(g.p[i]<low-.00001 || g.p[i]>high+.00001) throw Error('Kinetic model leaves its mechanical envelope');
      }
      if (definition.id >= 141 && definition.id <= 143) {
        for(let i=0;i<g.p.length;i+=3) {
          if(Math.hypot(g.p[i+1]-.5,g.p[i+2]-.5) > .550001) throw Error('Small gear tooth leaves its mechanical radius');
        }
      }
      primitives.push({attributes:{POSITION:buffer(g.p,'VEC3',34962),NORMAL:buffer(g.n,'VEC3',34962),TEXCOORD_0:buffer(g.uv,'VEC2',34962)},indices:buffer(g.i,'SCALAR',34963,true),material});
    });
    if(vertices > 8192) throw Error('Kinetic vertex budget exceeded');
    const mechanical=prefix.startsWith('kinetic_gear')||prefix.startsWith('kinetic_shaft');
    const gltf={asset:{version:'2.0',generator:'TERAMIC kinetic assembly builder'},scene:0,scenes:[{nodes:[0]}],nodes:[{mesh:0}],meshes:[{primitives}],materials:palette.map(([name,baseColorFactor],slot)=>({name,pbrMetallicRoughness:{baseColorFactor,roughnessFactor:mechanical&&slot===1?.68:.85,metallicFactor:mechanical&&slot===1?.65:name.includes('iron')||name.startsWith('kinetic_box')?.22:0}})),buffers:[{byteLength:offset,uri:'data:application/octet-stream;base64,'+Buffer.concat(chunks).toString('base64')}],bufferViews:views,accessors};
    fs.writeFileSync(path.join(directory,`models/${name}.gltf`), JSON.stringify(gltf)+'\n');
    fs.writeFileSync(path.join(directory,`${name}.json`), JSON.stringify({schema_version:1,...definition,creative_category:'technic',render_kind:'custom_mesh',struct_block:true,model_layout:'scene',model:`models/${name}.gltf`,textures:Object.fromEntries(palette.map(([name,_,texture])=>[name,texture])),collision_boxes:collision},null,2)+'\n');
    console.log(`${name}: ${vertices} vertices, ${primitives.length} reusable material groups`);
  }
  return {box,panel,ring,ringAxis,spoke,hull,save};
}

for (let variant=0;variant<variants.length;variant++) {
  const [file, label, texture, color, mining] = variants[variant];
  for (const large of [false,true]) {
    const pitch=large?1:.5, tip=large?1.05:.55, name=file+(large?'_large_gear':'_gear');
    const m=assembly('kinetic_gear_'+(large?'large_':'')+file,texture,color,tip);
    const teeth=large?16:8, body=pitch-.065, flange=large?.25:.185;
    // Thick paired timber spokes leave real open windows behind the tooth ring.
    const spokes=large?8:4;
    for(let i=0;i<spokes;i++) {
      const a=i*tau/spokes, half=large?.095:.13;
      m.ring(.355,.645,flange-.035,body-.065,0,2,a-half,a-.009);
      m.ring(.355,.645,flange-.035,body-.065,0,2,a+.009,a+half);
    }
    m.ring(.35,.65,body-.095,body,0,large?32:24);
    m.ring(.345,.655,body-.014,body+.002,1,large?32:24);
    for(let i=0;i<teeth;i++) {
      const a=i*tau/teeth, half=large?.085:.145;
      // Equal physical tooth pitch preserves the existing 2:1 transmission.
      m.ring(.35,.65,pitch-.085,tip,0,4,a-half,a+half);
      for(const x of [.332,.65]) {
        m.ring(x,x+.018,pitch-.13,pitch+.006,1,1,a-half*.38,a+half*.38);
        const r=pitch-.055;
        m.ring(x-.012,x+.030,0,large?.020:.016,1,6,0,tau,[.5+Math.cos(a)*r,.5+Math.sin(a)*r]);
      }
    }
    m.ring(.325,.675,.078,flange-.025,0,8);
    for(const x of [.285,.665]) {
      m.ring(x,x+.05,.076,flange,1,8);
      for(let i=0;i<4;i++) {
        const a=tau*(i+.5)/4,r=flange*.73;
        m.ring(x-.012,x+.062,0,large?.020:.015,1,6,0,tau,[.5+Math.cos(a)*r,.5+Math.sin(a)*r]);
      }
    }
    m.ring(0,1,0,.093,0,8);
    m.hull([0,.407,.407],[1,.593,.593]);
    const low=.5-tip, high=.5+tip, cross=large?.30:.18;
    m.hull([.36,low,.5-cross],[.64,high,.5+cross]);
    m.hull([.36,.5-cross,low],[.64,.5+cross,high]);
    m.save(name,{id:(large?149:141)+variant,name:label+(large?' Large Gear':' Gear'),description:large?'A large mechanical gear with twice the pitch radius. Diagonal small gears turn twice as fast.':'A compact meshing gear. Matching side gears reverse rotation; axial shafts carry it onward.',hardness:1.5+variant,resistance:5+variant*3,mining_level:mining,...(mining?{required_tool:'pickaxe'}:{})});
  }
  {
    const m=assembly('kinetic_shaft_'+file,texture,color);
    // The central coupling meets the neighboring cell; the octagonal body
    // consists of paired longitudinal boards with real, shallow recessed seams.
    m.ring(0,1,0,.078,0,8);
    m.ring(.035,.965,0,.103,0,8);
    for(let side=0;side<8;side++) {
      const a=side*tau/8,b=(side+1)*tau/8,mid=(a+b)/2;
      const start=[.5+Math.cos(a)*.108,.5+Math.sin(a)*.108];
      const end=[.5+Math.cos(b)*.108,.5+Math.sin(b)*.108];
      for(let plank=0;plank<2;plank++) {
        const t0=plank/2+.012,t1=(plank+1)/2-.012;
        const p=(x,t)=>[x,...start.map((v,i)=>v+(end[i]-v)*t)];
        m.panel([p(.035,t0),p(.965,t0),p(.965,t1),p(.035,t1)],[0,Math.cos(mid),Math.sin(mid)],.006);
      }
    }
    for(const x of [.04,.88]) {
      m.ring(x,x+.08,.103,.132,1,8);
      for(let i=0;i<8;i++) {
        const a=(i+.5)*tau/8,r=.126;
        m.box(x+.025,.5+Math.cos(a)*r-.005,.5+Math.sin(a)*r-.011,.03,.01,.022,1,a);
      }
    }
    m.hull([0,.368,.368],[1,.632,.632]);
    m.save(file+'_shaft',{id:144+variant,name:label+' Shaft',description:'An axial mechanical transmission. Matching shafts carry rotation through walls and between machines.',hardness:1.5+variant,resistance:5+variant*3,mining_level:mining,...(mining?{required_tool:'pickaxe'}:{})});
  }
}
{
  const m=assembly('kinetic_wheel','textures/machinery-oak-hd.png',[.95,.95,.95,1],1.5);
  for(const [a,b] of [[.15,.22],[.78,.85]]) {
    m.ring(a,b,1.25,1.36,0,32);m.ring(a-.008,b+.008,1.335,1.375,1,32);
  }
  for(let i=0;i<8;i++) m.spoke(i*tau/8,1.335,.095);
  for(let i=0;i<12;i++) {
    const a=i*tau/12;
    // Open inward-facing cups: a curved outer floor, axial end walls and lips.
    m.ring(.15,.85,1.44,1.5,0,4,a-.16,a+.16);
    for(const [x0,x1] of [[.12,.16],[.84,.88]]) {
      m.ring(x0,x1,1.20,1.5,0,4,a-.16,a+.16);
      m.ring(x0-.008,x1+.008,1.46,1.5,1,4,a-.16,a+.16);
    }
    for(const side of [-1,1]) m.ring(.16,.84,1.20,1.47,0,1,a+side*.16-.018,a+side*.16+.018);
  }
  m.ring(0,1,0,.093,0,8);m.ring(.30,.70,0,.21,0,16);m.ring(.28,.32,.17,.23,1,16);m.ring(.68,.72,.17,.23,1,16);
  m.hull([0,.27,.27],[1,.73,.73]);m.hull([.11,-1,.20],[.89,2,.80]);m.hull([.11,.20,-1],[.89,.80,2]);
  m.save('water_wheel',{id:147,name:'Water Wheel',description:'A paddled wheel driven by flowing water touching its lower or side paddles.',hardness:2,resistance:7,mining_level:0});
}
{
  const m=assembly('kinetic_belt','textures/machinery-belt-hd.png',[.88,.90,.93,1]);
  // One shared tread profile follows the renderer's closed conveyor loop.
  m.box(.18,.603,.455,.64,.018,.09);
  m.box(.19,.621,.473,.62,.004,.054,1);
  for(const x of [.04,.84]) {
    m.box(x,.27,.006,.12,.088,.988,2);
    for(const y of [.37,.47])for(const [z,d] of [[.006,.324],[.67,.324]])m.box(x,y,z,.12,.088,d,2);
    for(const [z,d] of [[.015,.315],[.67,.315]])m.box(x-.005,.575,z,.13,.025,d,3);
    for(const z of [.11,.89]) {
      m.box(x+.015,0,z-.055,.09,.30,.11,2);
      m.box(x+.005,0,z-.065,.11,.035,.13,3);
      m.box(x-.008,.46,z-.018,.136,.035,.036,3);
    }
    for(const z of [.14,.86]) {
      m.box(x-.01,.36,z-.055,.14,.11,.11,3);
      m.box(x-.018,.393,z-.022,.156,.044,.044,3);
    }
  }
  m.box(.16,.325,.1,.68,.025,.8,3);
  // Open bearings sit behind the shaft's end straps; no dummy axle hides real wood.
  for(const [x0,x1] of [[.125,.18],[.82,.875]])m.ring(x0,x1,.111,.125,3,12);
  for(const x of [.035,.835]) {
    m.hull([x,.27,0],[x+.13,.36,1]);
    for(const [z0,z1] of [[0,.33],[.67,1]])m.hull([x,.36,z0],[x+.13,.60,z1]);
    for(const z of [.045,.825])m.hull([x+.01,0,z],[x+.12,.30,z+.13]);
  }
  m.hull([.16,.325,.1],[.84,.35,.9]);
  for(const [x0,x1] of [[.125,.18],[.82,.875]]) {
    for(const [z0,z1] of [[.375,.389],[.611,.625]])m.hull([x0,.375,z0],[x1,.625,z1]);
    for(const [y0,y1] of [[.375,.389],[.611,.625]])m.hull([x0,y0,.389],[x1,y1,.611]);
  }
  m.hull([.17,.575,.015],[.83,.625,.985]);
  m.save('conveyor',{id:148,name:'Conveyor',description:'A powered conveyor that moves loose items in its placement direction.',hardness:2.5,resistance:8,mining_level:1,required_tool:'pickaxe'});
}

for(const vertical of [false,true]) {
  const cross=vertical?1:2;
  const m=assembly('kinetic_box_x','textures/machinery-iron-hd.png',[.84,.88,.92,1],.5,vertical?'kinetic_box_y':'kinetic_box_z');
  // Recessed core, separate fitted planks and a rigid strapped corner frame.
  m.box(.165,.165,.165,.67,.67,.67,2);
  for(let plank=0;plank<6;plank++){
    const p=.17+plank*.112;
    for(const z of [.12,.845])m.box(p,.14,z,.103,.72,.035,2);
    for(const x of [.12,.845])m.box(x,.14,p,.035,.72,.103,2);
    for(const y of [.12,.845])m.box(p,y,.14,.103,.035,.72,2);
  }
  for(const a of [.105,.815])for(const b of [.105,.815]){
    m.box(a,.10,b,.08,.80,.08,3);
    m.box(.10,a,b,.80,.08,.08,3);
    m.box(a,b,.10,.08,.08,.80,3);
  }
  for(const a of [.15,.85])for(const b of [.15,.85]){
    for(const x of [.09,.875])m.box(x,a-.025,b-.025,.035,.05,.05,3);
    for(const y of [.09,.875])m.box(a-.025,y,b-.025,.05,.035,.05,3);
    for(const z of [.09,.875])m.box(a-.025,b-.025,z,.05,.05,.035,3);
  }
  for(const [axis,material] of [[0,0],[cross,1]]){
    const opposite=axis===0?4:5;
    m.ringAxis(axis,0,.5,0,.083,opposite,8);
    m.ringAxis(axis,.5,1,0,.083,material,8);
    for(const end of [0,.90]){
      const rotor=end===0?opposite:material;
      m.ringAxis(axis,end,end+.10,.08,.17,rotor,12);
      for(let tooth=0;tooth<8;tooth++){
        const angle=tooth*tau/8;
        m.ringAxis(axis,end+.018,end+.082,.155,.195,rotor,1,angle-.12,angle+.12);
      }
    }
  }
  m.hull([.10,.10,.10],[.90,.90,.90]);
  m.hull([0,.305,.305],[1,.695,.695]);
  if(vertical)m.hull([.305,0,.305],[.695,1,.695]);
  else m.hull([.305,.305,0],[.695,.695,1]);
  m.save(vertical?'gearbox_vertical':'gearbox_horizontal',{
    id:vertical?155:154,name:vertical?'Vertical Gearbox':'Horizontal Gearbox',
    description:vertical?'A strapped wooden right-angle gearbox with horizontal and vertical axle ports.':'A strapped wooden right-angle gearbox with perpendicular horizontal axle ports.',
    hardness:2.5,resistance:8,mining_level:1,required_tool:'pickaxe',
  });
}
