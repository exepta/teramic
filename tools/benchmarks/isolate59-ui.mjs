// The old catalog cannot instantiate a newly added component. Redirect only its
// embedded UI asset root in a separate executable copy; render/game code is untouched.
import fs from 'node:fs';
import path from 'node:path';
const repo=path.resolve(import.meta.dirname,'../..');
const [input,output,uiRoot]=process.argv.slice(2);
if(!input||!output||!uiRoot||fs.existsSync(output)||fs.existsSync(uiRoot)) throw new Error('Supply input and two new output paths');
const original=Buffer.from(path.join(repo,'crates/terra-ui')+'/../../src-ui');
const resolved=path.resolve(uiRoot);
if(resolved.length>original.length) throw new Error('UI root too long for a path-only executable patch');
const data=fs.readFileSync(input);
const at=data.indexOf(original);
if(at<0||data.indexOf(original,at+1)>=0) throw new Error('Expected exactly one embedded UI asset root');
const replacement=Buffer.from(resolved+'/'.repeat(original.length-resolved.length));
replacement.copy(data,at);
fs.cpSync(path.join(repo,'src-ui'),resolved,{recursive:true});
const game=path.join(resolved,'pages/game.component.html');
fs.writeFileSync(game,fs.readFileSync(game,'utf8').replace('  <target-card></target-card>\n',''));
fs.writeFileSync(output,data,{mode:0o755});
process.stdout.write(`Redirected one ${original.length}-byte UI root, no rendering or simulation changes.\n`);
