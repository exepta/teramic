// Cherry construction uses the ordinary connected rails, stair poses and mixed slab shapes.
import fs from 'node:fs';
import path from 'node:path';
const root = path.resolve(import.meta.dirname, '../..');
const blocks = path.join(root, 'assets/terra/blocks');
const load = file => JSON.parse(fs.readFileSync(file, 'utf8'));
const save = (file, value) => fs.writeFileSync(file, JSON.stringify(value, null, 2) + '\n');
const ids = new Map([[10,134],[17,135],[75,136],[83,137],[91,138],[66,139],[69,140]]);
const kinds = {planks:'materials/wood',stairs:'building/shapes',slab:'building/shapes',
  fence:'building/fences',door:'building/doors',window:'building/windows'};
for (const name of ['planks','stairs','slab','fence']) {
  const filename = (wood) => wood + (name === 'planks' ? '-' : '_') + name + '.json';
  const block = load(path.join(blocks,filename('birch')));
  block.id = ids.get(block.id);
  block.name = block.name.replace('Birch', 'Cherry');
  block.description = 'Rose-colored cherry timber with fine natural grain.';
  const face = 'Cherry boards';
  block.materials = {[face]:{...block.materials['Birch boards'], normal_texture:'textures/block_palette/sakura_planks_normal.png'}};
  block.textures = {[face]:'textures/block_palette/sakura_planks.png'};
  block.face_materials = Array(6).fill(face);
  if (name === 'fence') {
    // Three fixed post boxes are followed by two rails per direction. Physics
    // and rendering both consult the same neighbor mask for indices 3..10.
    const box = (minimum,maximum) => ({minimum,maximum});
    const frame = [box([.33,0,.33],[.67,.83,.67]),box([.29,.83,.29],[.71,.94,.71]),
      box([.35,.94,.35],[.65,1,.65])];
    for (const axis of [0,2]) for (const sign of [-1,1]) for (const [y,top] of [[.29,.42],[.65,.82]]) {
      const min = [.438,y,.438], max = [.562,top,.562];
      min[axis] = sign === -1 ? 0 : .5;
      max[axis] = sign === -1 ? .5 : 1;
      frame.push(box(min,max));
    }
    block.collision_boxes = frame;
    block.textures[face] = 'textures/sakura/cherry_fence_lacquer.png';
  }
  save(path.join(blocks,filename('cherry')),block);
}
const log = load(path.join(blocks, 'weathered-wood.json'));
log.id = ids.get(log.id);
log.name = 'Cherry Log';
log.description = 'Dark cherry bark with pale horizontal lenticels and a rose-colored cut end.';
const bark = 'Cherry bark', end = 'Cherry end grain';
log.materials = Object.fromEntries([[bark, 'bark'], [end, 'end']].map(([key, form]) =>
  [key, { ...log.materials[log.face_materials[form === 'bark' ? 0 : 3]],
    normal_texture:'textures/block_palette/sakura_'+form+'_normal.png' }]));
log.face_materials = [bark,bark,end,end,bark,bark];
log.textures = {[bark]:'textures/block_palette/sakura_bark.png', [end]:'textures/block_palette/sakura_end.png'};
save(path.join(blocks,'cherry-log.json'),log);
for (const [name,category] of Object.entries(kinds)) {
  const dir = path.join(root,'assets/crafting',category);
  const recipe = load(path.join(dir,'birch_'+name+'.json'));
  recipe.id = 'teramic::cherry_'+name;
  recipe.name = recipe.name.replace('Birch','Cherry');
  const remap = value => {
    if (!value || typeof value !== 'object') return;
    if (ids.has(value.block)) value.block = ids.get(value.block);
    for (const nested of Object.values(value)) remap(nested);
  };
  remap(recipe);
  save(path.join(dir,'cherry_'+name+'.json'),recipe);
}
