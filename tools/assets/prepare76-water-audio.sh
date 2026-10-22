#!/usr/bin/env bash
# Reprocess the existing CC0 rain collection and Skippy Fish water recording.
set -euo pipefail
root="$(cd -- "$(dirname -- "$0")/../.." && pwd)"
rain="${1:?Path to the CC0 Rain OGG archive by Ylmir required}"
swim="${2:?Path to original CC0 swim.wav by jcpmcdonald required}"

unzip -p "$rain" 1.ogg |
    ffmpeg -hide_banner -loglevel warning -i pipe:0 -ac 1 -ar 24000 \
        -af 'highpass=f=140,lowpass=f=7200,acompressor=threshold=0.10:ratio=1.6:attack=18:release=180,volume=1.3,alimiter=limit=0.5:level=false' \
        -c:a pcm_s16le -y "$root/assets/audio/environment/rain.wav"
unzip -p "$rain" 3.ogg |
    ffmpeg -hide_banner -loglevel warning -i pipe:0 -ac 1 -ar 24000 \
        -af 'highpass=f=600,lowpass=f=9500,acompressor=threshold=0.12:ratio=1.5:attack=12:release=140,volume=0.7,alimiter=limit=0.5:level=false' \
        -c:a pcm_s16le -y "$root/assets/audio/environment/rain-detail.wav"

for take in 1 2 3 4; do
    case "$take" in
        1) start=0.03; gain=1.8; target=swim-stroke.wav ;;
        2) start=0.34; gain=1.8; target=swim-stroke-2.wav ;;
        3) start=0.69; gain=1.3; target=swim-stroke-3.wav ;;
        4) start=1.02; gain=0.85; target=swim-stroke-4.wav ;;
    esac
    ffmpeg -hide_banner -loglevel warning -ss "$start" -i "$swim" -t 0.78 \
        -ac 1 -ar 24000 \
        -af "highpass=f=85,lowpass=f=6800,acompressor=threshold=0.18:ratio=1.5:attack=8:release=100,volume=$gain,afade=t=in:d=0.016,afade=t=out:st=0.56:d=0.22,alimiter=limit=0.6:level=false" \
        -c:a pcm_s16le -y "$root/assets/audio/animals/$target"
done
for take in 1 2; do
    start=0.31
    [[ "$take" = 2 ]] && start=1.06
    ffmpeg -hide_banner -loglevel warning -ss "$start" -i "$swim" -t 0.34 \
        -ac 1 -ar 24000 \
        -af 'highpass=f=240,lowpass=f=5400,afade=t=in:d=0.006,afade=t=out:st=0.18:d=0.16,volume=1.6,alimiter=limit=0.5:level=false' \
        -c:a pcm_s16le -y "$root/assets/audio/footsteps/wet-film-$take.wav"
done
