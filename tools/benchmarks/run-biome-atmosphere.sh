#!/usr/bin/env bash
# Requires a built terra-client. No original configs, profiles or worlds are used.
set -euo pipefail
repo_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)
cd -- "$repo_dir"
mkdir -p target/biome-atmosphere
run_dir=$(mktemp -d "$repo_dir/target/biome-atmosphere/run.XXXXXX")
mkdir -p "$run_dir/configs" "$run_dir/profiles" "$run_dir/worlds" "$run_dir/captures"
cat > "$run_dir/configs/client.toml" <<'TOML'
title = "Teramic Biome Atmosphere Acceptance"
width = 1920
height = 1080
view_distance = 5
vertical_view_distance = 4
fov_degrees = 65.0
vsync = false
debug_overlay = false
TOML
cat > "$run_dir/configs/graphics.toml" <<'TOML'
width = 1920
height = 1080
window_mode = "windowed"
render_distance = 5
fov = 65.0
vsync = false
fps_limit = 60
shadow_quality = "high"
water_reflections = true
weather = "clear"
weather_intensity = 0.0
debug_overlay = false
TOML
cat > "$run_dir/configs/server.toml" <<'TOML'
natural_mob_limit = 0
lan_discovery = false
require_verified_accounts = false
TOML
cat > "$run_dir/configs/world.toml" <<'TOML'
seed = 42042
spirit_blossom_revision = 2
[time]
start_hour = 10.0
paused = true
TOML
printf 'Isolated biome atmosphere run: %s\n' "$run_dir"
export TERRA_CONFIG_DIR="$run_dir/configs" TERRA_MENU_CONFIG_DIR="$run_dir/configs"
export TERRA_PROFILE_FILE="$run_dir/profiles/inspection.toml" TERRA_WORLD_DIR="$run_dir/worlds"
export TERRA_KINGDOM_SAVE_DIR="$run_dir/kingdom-save" TERRA_ASSET_DIR="$repo_dir/assets"
export TERRA_BENCH_GRAPHICS=1 TERRA_COMPUTE_THREADS=6 RUST_LOG=info
export TERRA_BIOME_ATMOSPHERE=1 TERRA_CAPTURE_DIR="$run_dir/captures"
for biome in spirit-blossom cherry-forest; do
    env TERRA_BIOME_SCENE="picker-$biome" \
        timeout --signal=TERM 180s target/debug/terra-client --smoke-test-biomes \
        > "$run_dir/captures/picker-$biome.log" 2>&1
done
python3 - "$run_dir" <<'PY'
from pathlib import Path
import hashlib, json, os, struct, sys
root = Path(sys.argv[1])
results = {}
for biome in ('spirit-blossom', 'cherry-forest'):
    scene = 'picker-' + biome
    log = (root/'captures'/f'{scene}.log').read_text(errors='replace')
    assert 'Native biome preview passed:' in log, scene
    for failure in ('Shader processing error', 'failed to process shader error', 'failed to create shader module', 'Validation Error', 'panicked at', 'Biome atmosphere inspection failed'):
        assert failure not in log, (scene, failure)
    report = json.loads((root/'captures'/f'{scene}-atmosphere.json').read_text())
    assert report['status'] == 'passed'
    if biome == 'spirit-blossom':
        assert report['first_banks'] > 0 and report['later_banks'] > 0
        assert report['first_drift'] != report['later_drift']
    else:
        assert report['first_banks'] == report['later_banks'] == 0 or report['nearby_spirit_columns'] > 0
    images = {}
    for view in ('overview', 'animated', 'close'):
        image = root/'captures'/f'{scene}-{view}.png'
        data = image.read_bytes()
        assert data[:8] == b'\x89PNG\r\n\x1a\n'
        size = struct.unpack('>II', data[16:24])
        assert size[0] >= 1536 and size[1] >= 768, (image, size)
        images[view] = dict(width=size[0], height=size[1], bytes=len(data), sha256=hashlib.sha256(data).hexdigest())
    if biome == 'spirit-blossom':
        assert images['overview']['sha256'] != images['animated']['sha256'], scene
    results[biome] = dict(atmosphere=report, screens=images)
    if os.environ.get('TERRA_BIOME_MOTES'):
        motes = json.loads((root/'captures'/f'{scene}-close-motes.json').read_text())
        assert 0 < motes['motes'] <= 112 and motes['batches'] == 1, (scene, motes)
        results[biome]['motes'] = motes
(root/'verification.json').write_text(json.dumps(dict(
    status='passed', seed=42042, biomes=results,
    visual_review='Actual terrain colors and light-source clarity must be reviewed in the captures; no color is asserted from synthetic fixtures.'
), indent=2)+'\n')
print('Spirit and Cherry native atmosphere, animation and close ground views passed')
PY
