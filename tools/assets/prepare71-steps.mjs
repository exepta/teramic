import { spawnSync } from 'node:child_process';
import { mkdirSync, writeFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import path from 'node:path';

const source = process.argv[2];
if (!source) throw new Error('Provide the extracted recordings directory; see assets/audio/CREDITS.md');
const output = fileURLToPath(new URL('../../assets/audio/footsteps/detail/', import.meta.url));
const rate = 24000;
mkdirSync(output, { recursive: true });

function contact(name, file, start, duration) {
  const seek = start === undefined ? [] : ['-ss', String(start)];
  const length = duration === undefined ? [] : ['-t', String(duration)];
  const decoded = spawnSync('ffmpeg', [
    '-nostdin', '-v', 'error', ...seek, '-i', path.join(source, file), ...length,
    '-af', 'highpass=f=55,lowpass=f=10500', '-ac', '1', '-ar', String(rate), '-f', 'f32le', '-',
  ], { maxBuffer: 8000000 });
  if (decoded.status !== 0) throw new Error(`${file}: ${decoded.stderr.toString()}`);
  const samples = Array.from({ length: decoded.stdout.length / 4 }, (_, i) => decoded.stdout.readFloatLE(i * 4));
  const window = 120;
  const envelope = [];
  for (let i = 0; i < samples.length; i += window) {
    const frame = samples.slice(i, i + window);
    envelope.push(Math.sqrt(frame.reduce((sum, s) => sum + s * s, 0) / frame.length));
  }
  const threshold = Math.max(0.0004, Math.max(...envelope) * 0.025);
  const first = envelope.findIndex(s => s >= threshold);
  const last = envelope.findLastIndex(s => s >= threshold);
  if (first < 0) throw new Error(`No contact in ${file}`);
  // Preserve a 5-ms attack lead and the recorded decay, without long gaps or noise tails.
  const begin = Math.max(0, (first - 1) * window);
  const end = Math.min(samples.length, Math.max(begin + rate * 0.10, (last + 1) * window + rate * 0.055));
  const clip = samples.slice(begin, end);
  const rms = Math.sqrt(clip.reduce((sum, s) => sum + s * s, 0) / clip.length);
  const peak = Math.max(...clip.map(Math.abs));
  const gain = Math.min(0.105 / rms, 0.72 / peak);
  const wav = Buffer.alloc(44 + clip.length * 2);
  wav.write('RIFF', 0); wav.writeUInt32LE(wav.length - 8, 4); wav.write('WAVEfmt ', 8);
  wav.writeUInt32LE(16, 16); wav.writeUInt16LE(1, 20); wav.writeUInt16LE(1, 22);
  wav.writeUInt32LE(rate, 24); wav.writeUInt32LE(rate * 2, 28);
  wav.writeUInt16LE(2, 32); wav.writeUInt16LE(16, 34); wav.write('data', 36);
  wav.writeUInt32LE(clip.length * 2, 40);
  for (let i = 0; i < clip.length; i++) {
    const fade = Math.min(1, i / (rate * 0.002), (clip.length - 1 - i) / (rate * 0.022));
    wav.writeInt16LE(Math.round(clip[i] * gain * fade * 32767), 44 + i * 2);
  }
  writeFileSync(path.join(output, `${name}.wav`), wav);
  console.log(`${name}: ${(clip.length / rate).toFixed(3)}s, gain ${gain.toFixed(3)}`);
}
const numbered = (material, variant) => `${material}${variant === 1 ? '' : `-${variant}`}`;
for (const material of ['grass', 'wood']) {
  for (let i = 0; i < 5; i++) {
    contact(numbered(material, i + 1), `kenney-impact/Audio/footstep_${material}_${String(i).padStart(3, '0')}.ogg`);
  }
}
for (const material of ['sand', 'stone']) {
  for (let i = 0; i < 6; i++) {
    const foot = i % 2 === 0 ? 'L' : 'R';
    const take = Math.floor(i / 2) + 1;
    const title = material[0].toUpperCase() + material.slice(1);
    contact(numbered(material, i + 1), `Fantozzi-footsteps/flac/Fantozzi-${title}${foot}${take}.flac`);
  }
}
for (const [i, start] of [0.30, 1.29, 2.25, 3.18, 4.15, 5.12].entries()) {
  contact(numbered('dirt', i + 1), 'monte32-dirt-hq.mp3', start, 0.57);
}
for (let i = 0; i < 6; i++) {
  contact(numbered('snow', i + 1), `Corsica_S-Walking_in_Snow/Corsica_S-Walking_on_snow_covered_gravel_${String(i + 1).padStart(2, '0')}.flac`);
}
for (let i = 0; i < 10; i++) {
  contact(numbered('gravel', i + 1), `cdogs-footsteps/footsteps/gravel/${i}.ogg`);
}
for (let i = 0; i < 5; i++) {
  contact(numbered('glass', i + 1), `kenney-impact/Audio/impactGlass_light_${String(i).padStart(3, '0')}.ogg`);
}
for (let i = 1; i <= 2; i++) {
  contact(numbered('leaves', i), `dirt/leaves0${i}.ogg`);
}
