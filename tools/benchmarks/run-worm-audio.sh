#!/usr/bin/env bash
# Actual shared decoder and device sink; contacts are explicitly injected.
set -euo pipefail
repo_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)
cd -- "$repo_dir"
client_source="${TERRA_CLIENT_BINARY:-$repo_dir/target/debug/terra-client}"
[[ -x "$client_source" ]] || { printf 'Build terra-client first.\n' >&2; exit 1; }
mkdir -p target/worm-audio
run_dir=$(mktemp -d "$repo_dir/target/worm-audio/run.XXXXXX")
client_bin="$run_dir/terra-client"
ln -- "$client_source" "$client_bin"
trap 'rm -f -- "$client_bin"' EXIT
sha256sum "$client_bin" > "$run_dir/client.sha256"
mkdir -p "$run_dir/configs" "$run_dir/worlds" "$run_dir/profiles" "$run_dir/audio"
cat > "$run_dir/configs/client.toml" <<'TOML'
width = 960
height = 640
view_distance = 2
vertical_view_distance = 2
vsync = false
debug_overlay = false
TOML
cat > "$run_dir/configs/graphics.toml" <<'TOML'
width = 960
height = 640
window_mode = "windowed"
render_distance = 2
fps_limit = 60
vsync = false
TOML
cat > "$run_dir/configs/world.toml" <<'TOML'
seed = 42042
biomes = false
trees = false
water = false
caves = false
ecology_revision = 0
TOML
cat > "$run_dir/configs/server.toml" <<'TOML'
natural_mob_limit = 0
lan_discovery = false
require_verified_accounts = false
TOML
printf 'Isolated native worm audio run: %s\n' "$run_dir"
env TERRA_CONFIG_DIR="$run_dir/configs" TERRA_MENU_CONFIG_DIR="$run_dir/configs" \
    TERRA_WORLD_DIR="$run_dir/worlds" TERRA_PROFILE_FILE="$run_dir/profiles/local.toml" \
    TERRA_KINGDOM_SAVE_DIR="$run_dir/kingdom-save" TERRA_ASSET_DIR="$repo_dir/assets" \
    TERRA_WORM_AUDIO_INSPECT=1 TERRA_WORM_AUDIO_EXPORT="$run_dir/audio" \
    TERRA_COMPUTE_THREADS=4 RUST_LOG=info \
    timeout --signal=TERM --kill-after=10s 120s "$client_bin" --smoke-test-steps \
    > "$run_dir/native.log" 2>&1
sha256sum --check "$run_dir/client.sha256"
python3 - "$run_dir" <<'PY'
from pathlib import Path
import hashlib,json,re,struct,sys,wave
root=Path(sys.argv[1]);log=re.sub(r'\x1b\[[0-9;]*m','',(root/'native.log').read_text(errors='replace'))
assert 'Sandworm audio passed:' in log,'real audio sink did not consume observed worm phase sounds'
assert re.search(r'clips=132\b',log),'expanded shared audio bank was not decoded'
assert not re.search(r'\bERROR\b|panicked at|Validation Error|Shader processing error',log)
clips=[]
for name in ['roar','dig','landing','bite','spit','hurt','death','breath']:
    path=root/'audio'/f'worm-{name}.wav'
    with wave.open(str(path),'rb') as source:
        assert source.getnchannels()==1 and source.getframerate()==24000 and source.getsampwidth()==2
        data=source.readframes(source.getnframes());samples=struct.unpack('<'+'h'*(len(data)//2),data)
        peak=max(abs(sample) for sample in samples)/32767
        rms=(sum((sample/32767)**2 for sample in samples)/len(samples))**.5
        assert .008<rms<.35 and .29<=peak<=.681,(name,rms,peak)
        assert samples[0]==0 and samples[-1]==0
        clips.append(dict(name=name,seconds=len(samples)/24000,peak=peak,rms=rms,sha256=hashlib.sha256(path.read_bytes()).hexdigest()))
report=dict(status='passed',client_sha256=(root/'client.sha256').read_text().split()[0],decoded_clips=132,clips=clips,
            scope='Observed replicated phase fixtures and real shared audio decoder/device sink; one breach roar across Leap/Emerge, granular digging, landing, bite, spit, hurt, death. Eight original/CC0-derived WAV exports for listening. This is an injected phase fixture, not natural AI footage.')
(root/'verification.json').write_text(json.dumps(report,indent=2)+'\n')
print('Native worm audio passed:',root/'verification.json')
PY
