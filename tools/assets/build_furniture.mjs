// Deterministic, editable static glTF assets. Coordinates stay inside one unit;
// the world renderer applies the saved horizontal two-cell footprint.
import fs from 'node:fs';
import path from 'node:path';
const root = path.resolve(import.meta.dirname, '../..');
const palette = [
  ['oak', [0.8, 0.68, 0.49, 1], 0.83, 0],
  ['dark_oak', [0.43, 0.32, 0.21, 1], 0.9, 0],
  ['iron', [0.30, 0.32, 0.34, 1], 0.65, 0.55],
  ['iron_edge', [0.43, 0.44, 0.45, 1], 0.55, 0.45],
  ['brass', [0.72, 0.46, 0.16, 1], 0.46, 0.6],
  ['linen', [0.78, 0.73, 0.60, 1], 0.97, 0],
  ['indigo', [0.18, 0.25, 0.38, 1], 0.95, 0],
  ['lamp', [1.0, 0.55, 0.12, 1], 0.5, 0],
];
function model(name) {
  const groups = palette.map(() => ({ p: [], n: [], uv: [], i: [] }));
  const collisionBoxes = [];
  function box(x, y, z, w, h, d, mat = 0) {
    y -= name === 'chest' ? .065 : .04;
    collisionBoxes.push({minimum:[x,y,z], maximum:[x+w,y+h,z+d]});
    const g = groups[mat];
    const faces = [
      [[-1, 0, 0], [[x,y,z+d],[x,y,z],[x,y+h,z],[x,y+h,z+d]]],
      [[1, 0, 0], [[x+w,y,z],[x+w,y,z+d],[x+w,y+h,z+d],[x+w,y+h,z]]],
      [[0,-1,0], [[x,y,z],[x,y,z+d],[x+w,y,z+d],[x+w,y,z]]],
      [[0,1,0], [[x,y+h,z+d],[x,y+h,z],[x+w,y+h,z],[x+w,y+h,z+d]]],
      [[0,0,-1], [[x,y,z],[x+w,y,z],[x+w,y+h,z],[x,y+h,z]]],
      [[0,0,1], [[x+w,y,z+d],[x,y,z+d],[x,y+h,z+d],[x+w,y+h,z+d]]],
    ];
    for (const [normal, corners] of faces) {
      const start = g.p.length / 3;
      for (let i = 0; i < 4; i++) {
        g.p.push(...corners[i]); g.n.push(...normal);
        const p = corners[i];
        g.uv.push((normal[0] ? p[2] : p[0]) * 1.8, (normal[1] ? p[2] : p[1]) * 1.8);
      }
      g.i.push(start,start+2,start+1,start,start+3,start+2);
    }
  }
  if (name === 'chest') {
    // Inset wood panels, a stepped curved lid, iron hoops and a brass keyhole.
    box(.095,.075,.12,.81,.50,.76,1);
    for (let row=0;row<5;row++) {
      box(.105,.09+row*.09,.108,.79,.082,.018,row%2);
      box(.105,.09+row*.09,.878,.79,.082,.018,row%2);
    }
    for (let slice=0;slice<10;slice++) {
      const z=.12+slice*.076;
      const crown=.58+.29*Math.sqrt(Math.max(0,1-((slice-4.5)/5)**2));
      box(.095,.565,z,.81,crown-.565,.073,slice%2);
      for (const x of [.135,.795]) box(x,crown+.002,z,.07,.036,.075,2+(slice%2));
    }
    for (const x of [.125,.785]) {
      box(x,.07,.08,.09,.51,.055,2); box(x,.07,.865,.09,.51,.055,2);
    }
    for (const y of [.065,.515]) {
      box(.085,y,.075,.83,.06,.055,3); box(.085,y,.87,.83,.06,.055,3);
      box(.075,y,.10,.045,.06,.80,2); box(.88,y,.10,.045,.06,.80,2);
    }
    for (const x of [.13,.79]) for (const y of [.08,.51]) {
      box(x-.012,y-.005,.925,.11,.085,.026,2);
      box(x+.016,y+.017,.954,.052,.041,.016,4);
    }
    box(.455,.43,.934,.13,.18,.035,4);
    box(.482,.47,.971,.074,.095,.008,1);
    box(.47,.66,.925,.10,.08,.025,4);
  } else {
    for (const x of [.035,.83]) for (const z of [.065,.81]) {
      box(x,.045,z,.135,.69,.12,1);
      box(x-.012,.04,z-.015,.16,.095,.15,3);
      box(x-.006,.63,z-.008,.15,.085,.141,2);
    }
    box(.055,.18,.08,.90,.075,.79,1);
    box(.055,.62,.08,.90,.075,.79,1);
    for (let i=0;i<6;i++) box(.015,.70,.045+i*.15,.97,.085,.145,i%2);
    for (const x of [.015,.88]) for (const z of [.04,.79]) {
      box(x-.006,.69,z,.105,.11,.148,2); box(x+.028,.715,z+.15,.038,.046,.018,4);
    }
    // Shelf, drawer, vise, folded cloth, parchment, tool rack and lantern.
    box(.19,.27,.16,.36,.25,.61,0); box(.205,.285,.78,.33,.205,.025,1);
    box(.32,.36,.807,.105,.028,.02,2);
    for (let i=0;i<3;i++) box(.62+i*.08,.28,.35,.065,.06,.22,3-i%2);
    box(.09,.79,.69,.17,.045,.14,2); box(.105,.65,.87,.038,.153,.03,2);
    box(.61,.787,.76,.13,.006,.13,5); box(.61,.55,.895,.13,.24,.008,5);
    box(.61,.58,.906,.13,.035,.006,6);
    box(.34,.788,.30,.30,.004,.34,5);
    for (let i=0;i<4;i++) box(.37+i*.055,.794,.35,.022,.003,.20,1);
    box(.72,.79,.10,.19,.025,.22,1);
    for (const x of [.72,.89]) box(x,.815,.125,.022,.08,.175,0);
    for (const z of [.10,.30]) box(.72,.815,z,.192,.08,.025,0);
    for (let i=0;i<3;i++) {
      box(.75+i*.053,.83,.16,.016,.13,.017,1);
      box(.73+i*.053,.94,.145,.057,.045,.04,i%2?4:3);
    }
    box(.105,.79,.11,.11,.025,.14,2); box(.115,.82,.12,.09,.11,.12,7);
    for (const x of [.105,.2]) for (const z of [.11,.23]) box(x,.815,z,.012,.12,.013,2);
    box(.105,.935,.11,.11,.022,.14,2); box(.14,.96,.15,.04,.028,.05,2);
  }
  const chunks = [], views = [], accessors = [], primitives = [];
  let offset = 0;
  function buffer(data, componentType, type, target) {
    const bytes = componentType === 5126 ? Buffer.from(new Float32Array(data).buffer) : Buffer.from(new Uint32Array(data).buffer);
    const view = views.length; views.push({ buffer:0, byteOffset:offset, byteLength:bytes.length, target });
    chunks.push(bytes); offset += bytes.length;
    const size = {VEC3:3,VEC2:2,SCALAR:1}[type], a = {bufferView:view, componentType, count:data.length/size, type};
    if (type === 'VEC3') { a.min=[0,1,2].map(i=>Math.min(...data.filter((_,j)=>j%3===i))); a.max=[0,1,2].map(i=>Math.max(...data.filter((_,j)=>j%3===i))); }
    accessors.push(a); return accessors.length-1;
  }
  groups.forEach((g, material)=> {
    if (!g.i.length) return;
    primitives.push({attributes:{POSITION:buffer(g.p,5126,'VEC3',34962),NORMAL:buffer(g.n,5126,'VEC3',34962),TEXCOORD_0:buffer(g.uv,5126,'VEC2',34962)}, indices:buffer(g.i,5125,'SCALAR',34963), material});
  });
  const gltf = {asset:{version:'2.0',generator:'TERAMIC furniture builder'},scene:0,scenes:[{nodes:[0]}],nodes:[{mesh:0}],meshes:[{primitives}],
    materials:palette.map(([name,baseColorFactor,roughnessFactor,metallicFactor])=>({name,pbrMetallicRoughness:{baseColorFactor,roughnessFactor,metallicFactor},...(name==='lamp'?{emissiveFactor:[.8,.32,.06]}:{})})),
    buffers:[{byteLength:offset,uri:'data:application/octet-stream;base64,'+Buffer.concat(chunks).toString('base64')}],bufferViews:views,accessors};
  fs.writeFileSync(path.join(root,`assets/terra/blocks/models/${name}.gltf`),JSON.stringify(gltf));
  const file = path.join(root,`assets/terra/blocks/${name}.json`), definition=JSON.parse(fs.readFileSync(file));
  definition.textures={oak:'textures/oak-crafted-hd.png',dark_oak:'textures/oak-crafted-hd.png'};
  definition.collision_boxes = collisionBoxes;
  fs.writeFileSync(file,JSON.stringify(definition,null,2)+'\n');
}
model('workbench'); model('chest');
