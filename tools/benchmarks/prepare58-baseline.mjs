// Recreate the previous asset contract for the captured Phase-57 executable.
// Only removes the Phase-58 additions from an isolated copy; never edits real assets.
import fs from 'node:fs';
import path from 'node:path';
const destination=process.argv[2];
if (!destination || !path.isAbsolute(destination) || fs.existsSync(destination)) throw Error('Provide a new absolute output directory');
fs.cpSync(path.resolve(import.meta.dirname,'../../assets'),destination,{recursive:true});
for(const file of ['terra/blocks/torch.json','terra/props/bee_nest.json','crafting/torch.json']) fs.unlinkSync(path.join(destination,file));
for(const name of ['diamond_ore','ruby_ore']) {
  const file=path.join(destination,`terra/blocks/${name}.json`);
  const data=JSON.parse(fs.readFileSync(file)); delete data.light;
  for (const material of Object.values(data.materials)) delete material.emission;
  fs.writeFileSync(file,JSON.stringify(data));
}
const shaderFile=path.join(destination,'shaders/terra_caves.wgsl');
const shader=fs.readFileSync(shaderFile,'utf8');
const start=shader.indexOf('    // Never lift black');
const end=shader.indexOf('    return vec4<f32>(mix(color.rgb, result, cave.parameters.x), color.a);',start);
if(start<0 || end<start) throw Error('Unexpected cave shader contract');
const previous=`    let luminance = dot(color.rgb, vec3<f32>(0.2126, 0.7152, 0.0722));
    let shade = 1.0 - smoothstep(0.04, 0.45, luminance);
    let graded = color.rgb * mix(vec3<f32>(0.88, 0.96, 1.09), vec3<f32>(1.0), 1.0 - shade)
        + vec3<f32>(0.004, 0.006, 0.009) * shade;
    let haze = (1.0 - exp(-max(distance - 8.0, 0.0) * cave.parameters.y)) * 0.28;
    let result = mix(graded, vec3<f32>(0.022, 0.034, 0.052), haze);
`;
fs.writeFileSync(shaderFile,shader.slice(0,start)+previous+shader.slice(end));
