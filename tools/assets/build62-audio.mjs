// Fire source: PagDev, CC0, https://opengameart.org/content/fireplace-sound-loop
// Usage: node tools/assets/build62-audio.mjs /path/to/original/fire.wav
import fs from 'node:fs';
import { spawnSync } from 'node:child_process';
const input = process.argv[2];
if (!input || !fs.existsSync(input)) throw new Error('Pass the original CC0 fire.wav');
const result = spawnSync('ffmpeg', ['-y', '-v', 'error', '-i', input, '-t', '10',
  '-af', 'highpass=f=100,lowpass=f=5200,acompressor=threshold=0.12:ratio=3:attack=5:release=150,loudnorm=I=-27:TP=-9:LRA=5,afade=t=in:d=0.2,afade=t=out:st=9.8:d=0.2',
  '-ac', '1', '-ar', '24000', '-c:a', 'pcm_s16le', 'assets/audio/environment/torch-fire.wav'], { stdio: 'inherit' });
if (result.status !== 0) throw new Error('Fire conversion failed');

// Original quiet breathing Foley: two filtered airflow bands, no vocal/formant tone.
const rate = 24000, count = rate * 4, wav = Buffer.alloc(44 + count * 2);
wav.write('RIFF'); wav.writeUInt32LE(wav.length - 8, 4); wav.write('WAVEfmt ', 8);
wav.writeUInt32LE(16, 16); wav.writeUInt16LE(1, 20); wav.writeUInt16LE(1, 22);
wav.writeUInt32LE(rate, 24); wav.writeUInt32LE(rate * 2, 28);
wav.writeUInt16LE(2, 32); wav.writeUInt16LE(16, 34); wav.write('data', 36);
wav.writeUInt32LE(count * 2, 40);
let seed = 620062, low = 0, high = 0;
for (let i = 0; i < count; i++) {
  seed = (Math.imul(seed, 1664525) + 1013904223) >>> 0;
  const noise = seed / 2147483648 - 1;
  low += (noise - low) * .26; high += (low - high) * .025;
  const t = i / rate;
  const inhale = Math.sin(Math.PI * Math.max(0, Math.min(1, t / 1.55))) ** 2;
  const exhale = Math.sin(Math.PI * Math.max(0, Math.min(1, (t - 1.8) / 1.95))) ** 2;
  wav.writeInt16LE(Math.round((low - high) * (inhale * .09 + exhale * .06) * 32767), 44 + i * 2);
}
fs.writeFileSync('assets/audio/animals/sleep-breath.wav', wav);
