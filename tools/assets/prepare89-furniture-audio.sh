#!/usr/bin/env bash
set -euo pipefail
root="$(cd "$(dirname "$0")/../.." && pwd)"
audio_tmp="${1:-/tmp/terra89-audio}"
mkdir -p "$audio_tmp" "$root/assets/audio/objects" "$root/assets/audio/player"
curl -fsSL https://cdn.freesound.org/previews/125/125958_981397-hq.mp3 -o "$audio_tmp/door.mp3"
curl -fsSL https://cdn.freesound.org/previews/457/457972_2841496-hq.mp3 -o "$audio_tmp/cough.mp3"
for part in open close; do
  start=0; if [[ "$part" == close ]]; then start=1.88; fi
  ffmpeg -v error -y -ss "$start" -t 0.82 -i "$audio_tmp/door.mp3" -ac 1 -ar 24000 -af 'highpass=f=90,lowpass=f=8000,volume=0.7,afade=t=in:d=0.006,afade=t=out:st=0.72:d=0.1,alimiter=limit=0.72:level=false' "$root/assets/audio/objects/door-$part.wav"
done
ffmpeg -v error -y -t 0.95 -i "$audio_tmp/cough.mp3" -ac 1 -ar 24000 -af 'highpass=f=100,lowpass=f=8500,volume=3.5,afade=t=in:d=0.008,afade=t=out:st=0.81:d=0.14,alimiter=limit=0.68:level=false' "$root/assets/audio/player/cough.wav"
ffmpeg -v error -y -i "$root/assets/audio/environment/torch-fire.wav" -ac 1 -ar 24000 -af 'highpass=f=70,lowpass=f=3000,volume=1.1,alimiter=limit=0.65:level=false' "$root/assets/audio/objects/furnace-fire.wav"
