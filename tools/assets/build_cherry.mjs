// Cherry uses the existing cube/shape pipeline and shared opening authoring tool.
import fs from 'node:fs';
import path from 'node:path';
const root = path.resolve(import.meta.dirname, '../..');
const read = p => JSON.parse(fs.readFileSync(path.join(root, p), 'utf8'));
const write = (p, data) => fs.writeFileSync(path.join(root, p), JSON.stringify(data, null, 2) + '\n');
const ids = new Map([[10,134],[17,135],[75,136],[83,137],[91,138],[66,139],[69,140]]);
for (const [source, name] of [['birch-planks','cherry-planks'], ['birch_stairs','cherry_stairs'], ['birch_slab','cherry_slab'], ['birch_fence','cherry_fence']]) {
  const block = read(`assets/terra/blocks/${source}.json`);
  block.id = ids.get(block.id);
  block.name = block.name.replace('Birch', 'Cherry');
  block.description = 'Rose-colored cherry timber with fine natural grain.';
  if (block.render_kind === 'cube') block.surface = 'wood';
  for (const material of Object.values(block.materials)) material.base_color = [.88,.57,.58,1];
  block.materials = Object.fromEntries(Object.entries(block.materials).map(([key,value]) => [key.replace('Birch','Cherry'),value]));
  block.face_materials = block.face_materials.map(key => key.replace('Birch','Cherry'));
  write(`assets/terra/blocks/${name}.json`, block);
}
const log = read('assets/terra/blocks/weathered-wood.json');
log.id = 134; log.name = 'Cherry Log';
log.description = 'Dark cherry bark with pale horizontal lenticels and a rose-colored cut end.';
log.materials = {
  'Cherry bark': {base_color:[.41,.24,.27,1], texture_pattern:'cherry_bark'},
  'Cherry end grain': {base_color:[.83,.53,.55,1], texture_pattern:'cherry_end'},
};
log.face_materials = ['Cherry bark','Cherry bark','Cherry end grain','Cherry end grain','Cherry bark','Cherry bark'];
write('assets/terra/blocks/cherry-log.json', log);
for (const name of ['planks','stairs','slab','fence','door','window']) {
  const recipe = read(`assets/crafting/birch_${name}.json`);
  recipe.id = `teramic::cherry_${name}`;
  recipe.name = recipe.name.replace('Birch', 'Cherry');
  const remap = value => {
    if (!value || typeof value !== 'object') return;
    if (typeof value.block === 'number' && ids.has(value.block)) value.block = ids.get(value.block);
    for (const nested of Object.values(value)) remap(nested);
  };
  remap(recipe);
  write(`assets/crafting/cherry_${name}.json`, recipe);
}
