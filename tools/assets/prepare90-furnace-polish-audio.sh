#!/usr/bin/env bash
set -euo pipefail
root="$(cd "$(dirname "$0")/../.." && pwd)"
source="${1:-/tmp/terra90-audio/cough.mp3}"
mkdir -p "$(dirname "$source")" "$root/assets/audio/player"
if [[ ! -s "$source" ]]; then
  curl -fsSL https://cdn.freesound.org/previews/457/457972_2841496-hq.mp3 -o "$source"
fi
starts=(0 1.08 2.20)
names=(cough cough-2 cough-3)
for i in 0 1 2; do
  ffmpeg -v error -y -ss "${starts[$i]}" -t 0.94 -i "$source" -ac 1 -ar 24000 \
    -af 'highpass=f=160,lowpass=f=6500,loudnorm=I=-25:TP=-9:LRA=6,afade=t=in:d=0.012,afade=t=out:st=0.80:d=0.14' \
    "$root/assets/audio/player/${names[$i]}.wav"
done
ffmpeg -v error -y -i "$root/assets/audio/environment/torch-fire.wav" -ac 1 -ar 24000 \
  -af 'highpass=f=70,lowpass=f=3000,loudnorm=I=-24:TP=-9:LRA=5,afade=t=in:d=0.15,afade=t=out:st=9.80:d=0.20' \
  "$root/assets/audio/objects/furnace-fire.wav"
