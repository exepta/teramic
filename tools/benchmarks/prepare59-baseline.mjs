// Compatibility-only assets for the preserved Phase-58 executable.
import fs from 'node:fs';
import path from 'node:path';
const destination=process.argv[2];
if(!destination||fs.existsSync(destination)) throw new Error('Supply a new destination directory');
fs.cpSync(path.resolve(import.meta.dirname,'../../assets'),destination,{recursive:true});
for(const name of ['bee_nest','flower']) {
  const file=path.join(destination,`terra/props/${name}.json`);
  const data=JSON.parse(fs.readFileSync(file,'utf8'));
  delete data.hud_stats;
  fs.writeFileSync(file,JSON.stringify(data)+'\n');
}
const foliage=path.join(destination,'shaders/terra_foliage.wgsl');
const fire=`#ifdef FLAME_GEOMETRY
        let phase = world.x * 2.17 + world.z * 1.39;
        let flicker = sin(time * 9.0 + phase) * 0.65 + sin(time * 16.7 + phase) * 0.35;
        return world + vec4<f32>(sin(time * wind.z + phase) * wind.x,
            flicker * wind.y, cos(time * 5.3 + phase) * wind.x, 0.0);
#else
`;
const shader=fs.readFileSync(foliage,'utf8');
if(!shader.includes(fire)) throw new Error('Expected the Phase-59 fire branch');
const end='    return world + vec4<f32>(sway, flutter * wind.y * 0.35, sway * 0.55, 0.0);\n#endif\n}';
if(!shader.includes(end)) throw new Error('Expected the fire specialization end');
fs.writeFileSync(foliage,shader.replace(fire,'').replace(end,end.replace('\n#endif','')));
