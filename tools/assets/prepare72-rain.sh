#!/usr/bin/env bash
set -euo pipefail
root="$(cd "$(dirname "$0")/../.." && pwd)"
unzip -p "${1:?Path to Ylmir's CC0 Rain OGG archive required}" 1.ogg |
    ffmpeg -hide_banner -loglevel warning -i pipe:0 -ac 1 -ar 24000 \
        -af highpass=f=100,lowpass=f=9000 -c:a pcm_s16le -y "$root/assets/audio/environment/rain.wav"
