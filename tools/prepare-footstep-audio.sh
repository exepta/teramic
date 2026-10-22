#!/usr/bin/env bash
# Rebuild only material contacts from the recordings documented in CREDITS.md.
set -euo pipefail
src="${1:?Provide the extracted recording directory documented in assets/audio/CREDITS.md}"
root="$(cd -- "$(dirname -- "$0")/.." && pwd)"
node "$root/tools/assets/prepare71-steps.mjs" "$src"
