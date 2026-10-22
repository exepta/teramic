// Asset-only compatibility for the preserved Phase-59 executable; never modify live assets.
import fs from 'node:fs';
import path from 'node:path';
const destination = process.argv[2];
if (!destination || fs.existsSync(destination)) throw new Error('Supply a new destination directory');
fs.cpSync(path.resolve(import.meta.dirname, '../../assets'), destination, {recursive: true});
fs.rmSync(path.join(destination, 'terra/blocks/bedrock.json'));
const fish = path.join(destination, 'terra/mobs/fish.json');
const definition = JSON.parse(fs.readFileSync(fish, 'utf8'));
delete definition.variant_titles;
fs.writeFileSync(fish, JSON.stringify(definition) + '\n');
const horizon = path.join(destination, 'shaders/terra_horizon.wgsl');
let shader = fs.readFileSync(horizon, 'utf8');
shader = shader.replace('    let radius = length(p);', `    let radius = length(p);
    let axial = max(a.x, a.y) > 1.15 && max(a.x, a.y) < 1.75
        && min(a.x, a.y) < select(0.18, 0.09, max(a.x, a.y) > 1.5);
    let diagonal = radius > 1.2 && radius < 1.65 && abs(a.x - a.y) < 0.16;`)
    .replace('if radius < 1.0 {', 'if radius < 1.0 || axial || diagonal {');
fs.writeFileSync(horizon, shader);
