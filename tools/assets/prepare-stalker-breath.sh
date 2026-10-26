#!/usr/bin/env bash
set -euo pipefail
root="$(cd "$(dirname "$0")/../.." && pwd)"
source="${1:-/tmp/terra56-audio/eat.wav}"
if [[ ! -s "$source" ]]; then
  printf '%s\n' 'Supply the CC0 Scorcher24 animal_eating_breathing.wav original; no download is performed.' >&2
  exit 1
fi
ffmpeg -v error -y -ss 7.48 -t 0.85 -i "$source" -ac 1 -ar 24000 -c:a pcm_s16le \
  -af 'highpass=f=220,lowpass=f=5600,loudnorm=I=-24:TP=-9:LRA=5,afade=t=in:d=0.025,afade=t=out:st=0.70:d=0.15' \
  "$root/assets/audio/animals/stalker-airway.wav"
