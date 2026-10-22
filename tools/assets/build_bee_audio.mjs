// Original procedural wing/landing Foley. No third-party/game audio samples.
import fs from 'node:fs';
import path from 'node:path';
const root=path.resolve(import.meta.dirname,'../../assets/audio/animals');
function wav(name, seconds, sample) {
  const n=Math.floor(seconds*24000), bytes=Buffer.alloc(44+n*2);
  bytes.write('RIFF'); bytes.writeUInt32LE(36+n*2,4); bytes.write('WAVEfmt ',8);
  bytes.writeUInt32LE(16,16); bytes.writeUInt16LE(1,20); bytes.writeUInt16LE(1,22);
  bytes.writeUInt32LE(24000,24); bytes.writeUInt32LE(48000,28);
  bytes.writeUInt16LE(2,32); bytes.writeUInt16LE(16,34); bytes.write('data',36); bytes.writeUInt32LE(n*2,40);
  for(let i=0;i<n;i++) bytes.writeInt16LE(Math.round(Math.max(-1,Math.min(1,sample(i/24000,i)))*32767),44+i*2);
  fs.writeFileSync(path.join(root,name),bytes);
}
const tau=Math.PI*2;
wav('bee-flight.wav',1.2,(t)=>{
  const phase=tau*184*t+.32*Math.sin(tau*3*t);
  const fade=Math.min(1,t/.025,(1.2-t)/.025);
  return fade*(.052*Math.sin(phase)+.026*Math.sin(phase*2)+.010*Math.sin(phase*3))*(.85+.15*Math.sin(tau*7*t));
});
wav('bee-land.wav',.22,(t,i)=>{
  const r=((Math.imul(i+137,1103515245)>>>0)/4294967295)*2-1;
  const pulse=Math.exp(-t*45)+.5*Math.exp(-Math.abs(t-.045)*100);
  return r*.065*pulse+Math.sin(tau*160*t)*.018*Math.exp(-t*28);
});
for (const [name,enter] of [['bee-enter.wav',true],['bee-leave.wav',false]]) {
  wav(name,.65,(t)=>{
    const u=t/.65, gain=Math.sin(Math.PI*u)*.16;
    const hz=enter?220-80*u:135+100*u;
    return gain*(Math.sin(tau*hz*t)*.7+Math.sin(tau*hz*2*t)*.2+Math.sin(tau*hz*3*t)*.1);
  });
}
