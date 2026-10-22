#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/../.."
mkdir -p assets/audio/footsteps/detail
for material in grass sand stone dirt snow wood; do
  for suffix in '' '-2'; do
    ffmpeg -v error -y -i "assets/audio/footsteps/${material}${suffix}.wav" -af "highpass=f=55,afftdn=nf=-35:tn=1,lowpass=f=9000,dynaudnorm=f=80:g=3:p=0.65:m=2,afade=t=in:d=0.003,areverse,afade=t=in:d=0.025,areverse,alimiter=limit=0.8:level=false" -ar 24000 -ac 1 "assets/audio/footsteps/detail/${material}${suffix}.wav"
  done
done
