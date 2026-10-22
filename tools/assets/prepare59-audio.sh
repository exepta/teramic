#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/../.."
node tools/assets/build_bee_audio.mjs
ffmpeg -v error -y -i assets/audio/environment/birds.wav -af highpass=f=350,lowpass=f=6500,alimiter=limit=0.75:level=false -ar 24000 -ac 1 assets/audio/environment/birds-soft.wav
ffmpeg -v error -y -i assets/audio/animals/swim.wav -t 0.85 -af highpass=f=65,lowpass=f=6500,afade=t=in:d=0.008,afade=t=out:st=0.45:d=0.4,volume=1.3 -ar 24000 -ac 1 assets/audio/animals/splash.wav
ffmpeg -v error -y -i assets/audio/animals/swim.wav -t 1.25 -af highpass=f=100,lowpass=f=3800,afade=t=in:d=0.07,afade=t=out:st=0.85:d=0.4 -ar 24000 -ac 1 assets/audio/animals/swim-stroke.wav
