// Beds share one two-cell silhouette, with separate frame and dyeable fabric palettes.
import fs from 'node:fs';
import path from 'node:path';
const root = path.resolve(import.meta.dirname, '../..');
for (const [index, wood, tint] of [[0,'oak',[.61,.32,.095]],[1,'birch',[.82,.71,.47]],[2,'spruce',[.30,.16,.075]]]) {
  const name = `${wood}_bed`, groups = Array.from({length:9},()=>({p:[],n:[],i:[]})), collision=[];
  const colors = [tint,tint.map(c=>c*.76),tint.map(c=>Math.min(.98,c*1.18)),[.92,.91,.85],[1,.98,.92],[.72,.71,.67],[1,1,1],[.82,.82,.82],[.16,.17,.16]];
  const materials=colors.map((c,i)=>({name:i===6?'blanket_main':i===7?'blanket_stitch':`bed_part_${i}`,pbrMetallicRoughness:{baseColorFactor:[...c,1],roughnessFactor:.92,metallicFactor:i===8?.4:0}}));
  const box=(x,y,z,w,h,d,m,solid=false)=>{
    if(solid)collision.push({minimum:[x,y,z],maximum:[x+w,y+h,z+d]});
    const g=groups[m],lo=[x,y,z],hi=[x+w,y+h,z+d];
    for(let axis=0;axis<3;axis++)for(const sign of[-1,1]){
      const u=(axis+1)%3,v=(axis+2)%3,n=[0,0,0];n[axis]=sign;const base=g.p.length/3;
      for(const[a,b]of[[0,0],[1,0],[1,1],[0,1]]){const p=[...lo];p[axis]=sign>0?hi[axis]:lo[axis];p[u]=a?hi[u]:lo[u];p[v]=b?hi[v]:lo[v];g.p.push(...p);g.n.push(...n);}
      g.i.push(...(sign>0?[0,1,2,0,2,3]:[0,2,1,0,3,2]).map(i=>base+i));
    }
  };
  // Foot is the anchor cell; the pillow and headboard occupy its paired cell.
  for(const x of[.035,.835])for(const z of[.035,1.805]){
    box(x,0,z,.13,z>1?1:.76,.16,0,true);box(x-.012,z>1?.94:.70,z-.012,.154,.06,.184,2);
    for(const y of[.18,.48])box(x+.04,y,z-.006,.035,.035,.012,8);
  }
  for(const x of[.07,.85])box(x,.20,.15,.08,.20,1.70,1,true);
  box(.14,.30,.15,.72,.10,1.70,0,true);
  for(const z of[.055,1.855]){
    box(.16,.26,z,.68,.12,.09,0,true);box(.16,.43,z,.68,z>1?.36:.16,.09,0,true);
    box(.16,z>1?.80:.60,z-.015,.68,.055,.12,2,true);
    for(let i=0;i<5;i++)box(.19+i*.13,.46,z-.008,.11,z>1?.30:.10,.012,i%3);
  }
  box(.13,.40,.17,.74,.19,1.64,3,true);
  // Stepped seams break the pillow into a soft block silhouette.
  box(.21,.59,1.42,.58,.10,.32,4,true);box(.24,.69,1.45,.52,.025,.26,4);
  for(const x of[.24,.49,.74])box(x,.692,1.48,.012,.026,.19,3);
  box(.12,.58,.19,.76,.10,1.18,6,true);
  for(const x of[.09,.85])box(x,.44,.18,.06,.22,1.20,6,true);
  box(.12,.44,.17,.76,.22,.055,6,true);
  // Quilted tiles, folded top edge, piping, and small alternating fabric stitches.
  for(let x=0;x<4;x++)for(let z=0;z<6;z++)box(.13+x*.187,.68,.20+z*.19,.177,.012,.18,(x+z)%3===0?7:6);
  box(.105,.68,1.30,.79,.035,.09,7);box(.10,.61,1.38,.80,.07,.055,3);
  for(let z=0;z<10;z++)for(const x of[.085,.91])box(x,.49,.22+z*.105,.009,.018,.038,7);
  for(let z=0;z<8;z++)for(const x of[.075,.925])box(x,.25,.24+z*.19,.012,.08,.095,2);
  const chunks=[],views=[],accessors=[],primitives=[];let offset=0;
  const buffer=(data,type,integer=false)=>{
    const bytes=Buffer.from((integer?new Uint32Array(data):new Float32Array(data)).buffer),view=views.length;
    views.push({buffer:0,byteOffset:offset,byteLength:bytes.length,target:integer?34963:34962});chunks.push(bytes);offset+=bytes.length;
    const width=type==='VEC3'?3:1,a={bufferView:view,componentType:integer?5125:5126,count:data.length/width,type};
    if(width===3){a.min=[0,1,2].map(i=>Math.min(...data.filter((_,j)=>j%3===i)));a.max=[0,1,2].map(i=>Math.max(...data.filter((_,j)=>j%3===i)));}
    accessors.push(a);return accessors.length-1;
  };
  for(const[m,g]of groups.entries())if(g.i.length)primitives.push({attributes:{POSITION:buffer(g.p,'VEC3'),NORMAL:buffer(g.n,'VEC3')},indices:buffer(g.i,'SCALAR',true),material:m});
  const gltf={asset:{version:'2.0',generator:'TERAMIC bed builder'},scene:0,scenes:[{nodes:[0]}],nodes:[{mesh:0}],meshes:[{primitives}],materials,buffers:[{byteLength:offset,uri:'data:application/octet-stream;base64,'+Buffer.concat(chunks).toString('base64')}],bufferViews:views,accessors};
  const definition={schema_version:1,id:71+index,name:`${wood[0].toUpperCase()+wood.slice(1)} Bed`,description:`A two-cell ${wood} bed with a pillow, quilted blanket and carved headboard. Flower dyes recolor its blanket.`,creative_category:'decoration',render_kind:'custom_mesh',struct_block:true,model_layout:'scene',model:`models/${name}.gltf`,hardness:1.4,resistance:3,collision_boxes:collision};
  if (!process.argv.includes('--dyes-only')) {
    fs.writeFileSync(path.join(root,`assets/terra/blocks/models/${name}.gltf`),JSON.stringify(gltf)+'\n');
    fs.writeFileSync(path.join(root,`assets/terra/blocks/${name}.json`),JSON.stringify(definition,null,2)+'\n');
  }
  const plank={item:{block:16+index},count:1},wool={item:{animal:'wool'},count:1};
  const recipe={id:`teramic::${name}`,name:definition.name,station:'workbench',pattern:{kind:'shaped',width:3,height:2,cells:[wool,wool,wool,plank,plank,plank]},output:{item:{block:71+index},count:1}};
  if (!process.argv.includes('--dyes-only')) fs.writeFileSync(path.join(root,`assets/crafting/${name}.json`),JSON.stringify(recipe,null,2)+'\n');
  console.log(`${name}: ${groups.reduce((n,g)=>n+g.p.length/3,0)} vertices, ${collision.length} shared render/collision pieces`);
  for (const color of ['red','yellow','blue','orange','green','purple']) {
    const dyeRecipe = {id:`teramic::${name}_${color}`,name:`${color[0].toUpperCase()+color.slice(1)} ${definition.name}`,station:'hand',pattern:{kind:'shapeless',ingredients:[{item:{block:71+index},count:1},{item:{dye:color},count:1}]},output:{item:{dyed_bed:{material:71+index,color}},count:1}};
    fs.writeFileSync(path.join(root,`assets/crafting/${name}_${color}.json`),JSON.stringify(dyeRecipe,null,2)+'\n');
  }
}
const ingredient=(item)=>({item,count:1});
const flower={prop:'flower'},grass={prop:'grass'},plant={prop:'small_plant'};
for(const[color,parts]of(process.argv.includes('--dyes-only')?[]:[['red',[flower]],['yellow',[flower,grass]],['blue',[flower,plant]],['orange',[{dye:'red'},{dye:'yellow'}]],['green',[{dye:'yellow'},{dye:'blue'}]],['purple',[{dye:'red'},{dye:'blue'}]]])){
  fs.writeFileSync(path.join(root,`assets/terra/items/${color}_dye.json`),JSON.stringify({item:{dye:color},name:`${color[0].toUpperCase()+color.slice(1)} Dye`,description:'Flower pigment for coloring bed blankets.'},null,2)+'\n');
  fs.writeFileSync(path.join(root,`assets/crafting/${color}_dye.json`),JSON.stringify({id:`teramic::${color}_dye`,name:`${color[0].toUpperCase()+color.slice(1)} Dye`,station:'hand',pattern:{kind:'shapeless',ingredients:parts.map(ingredient)},output:{item:{dye:color},count:color==='red'?2:1}},null,2)+'\n');
}
