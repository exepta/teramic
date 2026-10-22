// Reference-inspired framed joinery. The frame stays fixed; leaf_* materials
// and hinged collision boxes rotate together around the same authored hinge.
import fs from 'node:fs';
import path from 'node:path';
const root = path.resolve(import.meta.dirname, '../..');
const species = [
  ['oak', [0.61, 0.32, 0.095]],
  ['birch', [0.80, 0.68, 0.46]],
  ['spruce', [0.30, 0.16, 0.075]],
];
for (const [index, [wood, tint]] of species.entries()) {
  for (const window of [false, true]) {
    const id = (window ? 68 : 65) + index;
    const name = `${wood}_${window ? 'window' : 'door'}`;
    const height = window ? 1 : 2;
    const colors = [1, .79, 1.16].map(scale => tint.map(c => Math.min(.96, c * scale)));
    const materials = [];
    for (const part of ['frame', 'leaf']) {
      for (let i = 0; i < colors.length; i++)
        materials.push({name: `${part}_wood_${i}`, pbrMetallicRoughness: {
          baseColorFactor: [...colors[i], 1], roughnessFactor: .9, metallicFactor: 0,
        }});
      materials.push({name: `${part}_iron`, pbrMetallicRoughness: {
        baseColorFactor: [.17, .18, .18, 1], roughnessFactor: .58, metallicFactor: .5,
      }});
      materials.push({name: `${part}_rivet`, pbrMetallicRoughness: {
        baseColorFactor: [.43, .44, .42, 1], roughnessFactor: .5, metallicFactor: .55,
      }});
    }
    materials.push({name:'leaf_glass', alphaMode:'BLEND', doubleSided:true,
      pbrMetallicRoughness:{baseColorFactor:[.68,.85,.91,.12],roughnessFactor:.14,metallicFactor:0}});
    const groups = materials.map(() => ({p:[], n:[], i:[]}));
    const collision = [];
    const box = (x,y,z,w,h,d,material,solid=false,hinged=false) => {
      const mirror = !window && ((material>=5 && z>=.545) || (material<5 && z>=.662));
      if(!window && material>=5 && z>=.545) z-=.03;
      if(solid) collision.push({minimum:[x,y,z],maximum:[x+w,y+h,z+d],...(hinged?{hinged:true}:{})});
      // Carving, plank grain and ironwork exist on both sides of the same leaf.
      for(const depth of mirror ? [z, 1-z-d] : [z]) {
      const low=[x,y,depth], high=[x+w,y+h,depth+d], g=groups[material];
      for(let axis=0;axis<3;axis++) for(const sign of [-1,1]) {
        const u=(axis+1)%3,v=(axis+2)%3,normal=[0,0,0];normal[axis]=sign;
        const base=g.p.length/3;
        for(const[a,b]of[[0,0],[1,0],[1,1],[0,1]]) {
          const p=[...low];p[axis]=sign>0?high[axis]:low[axis];
          p[u]=a?high[u]:low[u];p[v]=b?high[v]:low[v];g.p.push(...p);g.n.push(...normal);
        }
        g.i.push(...(sign>0?[0,1,2,0,2,3]:[0,2,1,0,3,2]).map(i=>base+i));
      }
      }
    };
    // Door clearance exceeds the 0.64 x 1.80 player capsule, even with the
    // opened leaf. Its visual sill is flush and intentionally non-colliding.
    box(.04,0,.34,.11,height,.32,0,true);
    box(window?.85:.87,0,.34,window?.11:.09,height,.32,0,true);
    box(.15,height-(window?.12:.06),.34,window?.70:.72,window?.12:.06,.32,0,true);
    box(.15,0,.34,window?.70:.72,window?.075:.015,.32,1,window);
    for(const x of [.025,.835]) for(const y of [.015,height-.17]) {
      box(x,y,.32,.14,.15,.36,2);
      box(x+.05,y+.05,.687,.04,.045,.018,1);
    }
    box(.015,height-.10,.30,.97,.075,.40,2);
    box(.02,window?.06:0,.30,.96,window?.065:.015,.40,0);
    for(const x of [.06,.865]) {
      for(let y=.23;y<height-.18;y+=.23) {
        box(x,y,.662,.014,.14,.006,1);
        box(x+.049,y+.065,.664,.024,.055,.006,2);
      }
      if(wood==='birch') for(const y of [.29,.66,1.17,1.55]) {
        if(y<height-.18) box(x-.02,y,.662,.075,.025,.008,1);
      }
    }
    const bottom=window?.085:.015, top=height-(window?.135:.06);
    if(!window) {
      box(.15,bottom,.485,.72,top-bottom,.03,5,true,true);
      // Recessed plank field, projecting braces and a stepped carved arch.
      for(let i=0;i<7;i++) {
        const x=.158+i*.098;
        box(x,bottom+.055,.546,.089,top-bottom-.11,.012,5+i%3);
        for(let j=0;j<5;j++) {
          const y=bottom+.12+j*(height-.4)/5;
          box(x+.019+(j%2)*.027,y,.56,.009,.07+(i%3)*.03,.004,6);
        }
      }
      for(const y of [bottom+.05,height*.47,top-.07]) box(.16,y,.561,.68,.075,.035,7);
      for(const x of [.16,.80]) box(x,bottom+.08,.56,.04,top-bottom-.16,.028,6);
      for(let i=0;i<9;i++) {
        const x=.20+i*.064, y=top-.10-Math.abs(i-4)*.035;
        box(x,y,.565,.064,.035,.028,6);
      }
      // Square ring handle, not an opaque disc.
      for(const [x,y,w,h] of [[.735,height*.44,.065,.025],[.735,height*.44+.11,.065,.025],
        [.725,height*.44+.025,.025,.085],[.79,height*.44+.025,.025,.085]]) {
        box(x,y,.61,w,h,.025,8);
      }
      box(.74,height*.44+.07,.575,.06,.07,.04,8);
      box(.759,height*.44+.095,.62,.025,.02,.018,9);
    } else {
      // One hinged sash with four independent transparent panes.
      for(const x of [.15,.80]) box(x,bottom,.455,.05,top-bottom,.09,5,true,true);
      for(const y of [bottom,top-.05]) box(.20,y,.455,.60,.05,.09,5,true,true);
      box(.475,bottom+.05,.455,.05,top-bottom-.10,.09,7,true,true);
      box(.20,.465,.455,.60,.05,.09,7,true,true);
      for(const x of [.20,.525]) for(const [y,h] of [[bottom+.05,.465-bottom-.05],[.515,top-.05-.515]]) {
        box(x,y,.488,.275,h,.024,10,true,true);
      }
      box(.77,.41,.55,.035,.12,.025,8);
      box(.76,.45,.575,.06,.025,.025,9);
      box(.01,.02,.275,.98,.07,.45,2);
    }
    for(const y of [height*.24,height*.76]) {
      box(.125,y,.555,.025,.13,.045,3);
      box(.15,y+.025,.56,.18,.075,.035,8);
      for(const x of [.17,.285]) box(x,y+.047,.596,.018,.023,.009,9);
    }
    const chunks=[],views=[],accessors=[],primitives=[];let offset=0;
    const buffer=(data,type,integer=false)=>{
      const bytes=Buffer.from((integer?new Uint32Array(data):new Float32Array(data)).buffer);
      const view=views.length;views.push({buffer:0,byteOffset:offset,byteLength:bytes.length,target:integer?34963:34962});
      chunks.push(bytes);offset+=bytes.length;
      const width=type==='VEC3'?3:1,a={bufferView:view,componentType:integer?5125:5126,count:data.length/width,type};
      if(width===3){a.min=[0,1,2].map(i=>Math.min(...data.filter((_,j)=>j%3===i)));a.max=[0,1,2].map(i=>Math.max(...data.filter((_,j)=>j%3===i)));}
      accessors.push(a);return accessors.length-1;
    };
    for(const [material,g]of groups.entries()) if(g.i.length) {
      if(g.p.some((v,i)=>!Number.isFinite(v)||v<0||v>(i%3===1?height:1))) throw Error(`${name}: invalid bounds`);
      primitives.push({attributes:{POSITION:buffer(g.p,'VEC3'),NORMAL:buffer(g.n,'VEC3')},indices:buffer(g.i,'SCALAR',true),material});
    }
    const gltf={asset:{version:'2.0',generator:'TERAMIC opening builder'},scene:0,scenes:[{nodes:[0]}],nodes:[{mesh:0}],
      meshes:[{primitives}],materials,buffers:[{byteLength:offset,uri:'data:application/octet-stream;base64,'+Buffer.concat(chunks).toString('base64')}],
      bufferViews:views,accessors};
    const title=wood[0].toUpperCase()+wood.slice(1)+' '+(window?'Window':'Door');
    const definition={schema_version:1,id,name:title,description:`A framed ${wood} ${window?'glass window':'plank door'} with iron hinges. Opens smoothly and ventilates furnace smoke.`,
      creative_category:'decoration',render_kind:'custom_mesh',struct_block:true,model_layout:'scene',model:`models/${name}.gltf`,
      hardness:2,resistance:4,required_tool:'axe',mining_level:0,collision_boxes:collision};
    fs.writeFileSync(path.join(root,`assets/terra/blocks/models/${name}.gltf`),JSON.stringify(gltf)+'\n');
    fs.writeFileSync(path.join(root,`assets/terra/blocks/${name}.json`),JSON.stringify(definition,null,2)+'\n');
    console.log(`${name}: ${groups.reduce((n,g)=>n+g.p.length/3,0)} vertices, ${collision.length} collision boxes`);
  }
}
