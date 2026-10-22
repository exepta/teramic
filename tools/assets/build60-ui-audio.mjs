// Original short UI Foley. Smooth envelopes avoid hard clicks and fatiguing transients.
import fs from 'node:fs';
import path from 'node:path';
const root = path.resolve(import.meta.dirname, '../../assets/audio/ui');
fs.mkdirSync(root, {recursive: true});
function wave(name, seconds, sample) {
  const rate = 24000, count = Math.ceil(seconds * rate);
  const data = Buffer.alloc(44 + count * 2);
  data.write('RIFF'); data.writeUInt32LE(data.length - 8, 4); data.write('WAVEfmt ', 8);
  data.writeUInt32LE(16, 16); data.writeUInt16LE(1, 20); data.writeUInt16LE(1, 22);
  data.writeUInt32LE(rate, 24); data.writeUInt32LE(rate * 2, 28);
  data.writeUInt16LE(2, 32); data.writeUInt16LE(16, 34); data.write('data', 36); data.writeUInt32LE(count * 2, 40);
  for (let i = 0; i < count; i++) { const t = i / rate; const env = Math.min(1, t / 0.005) * Math.min(1, (seconds - t) / 0.018); data.writeInt16LE(Math.round(Math.max(-1, Math.min(1, sample(t) * env)) * 32767), 44 + i * 2); }
  fs.writeFileSync(path.join(root, name + '.wav'), data);
}
wave('click', .065, t => Math.sin(2 * Math.PI * (540 * t - 1200 * t * t)) * Math.exp(-t * 48) * .45);
wave('mention', .32, t => (Math.sin(2 * Math.PI * (t < .13 ? 660 : 880) * t) + Math.sin(2 * Math.PI * 1320 * t) * .14) * Math.exp(-t * 6) * .32);
