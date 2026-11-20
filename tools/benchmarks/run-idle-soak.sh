#!/usr/bin/env bash
# Three minutes after scene readiness in a real generated forest. No user save/config writes.
set -euo pipefail
repo_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)
cd -- "$repo_dir"
client_source=$(realpath -- "${TERRA_CLIENT_BINARY:-$repo_dir/target/debug/terra-client}")
[[ -x "$client_source" ]] || { printf 'Build terra-client first.\n' >&2; exit 1; }
mkdir -p target/reference-polish-soak
run_dir=$(mktemp -d "$repo_dir/target/reference-polish-soak/run.XXXXXX")
mkdir -p "$run_dir"/{configs,worlds,profiles,captures,kingdom-save}
ln -- "$client_source" "$run_dir/terra-client"
sha256sum "$run_dir/terra-client" > "$run_dir/client.sha256"
cp configs/world.toml "$run_dir/configs/world.toml"
python3 - "$run_dir/configs/world.toml" <<'PY'
from pathlib import Path
import re,sys,tomllib
p=Path(sys.argv[1]); text=p.read_text()
# Use the actual server spawn search with an explicit forest preference.
# No camera teleport is needed; the stationary player lands through normal physics.
for key,value in {'seed':'42042','paused':'true','start_hour':'10.0'}.items():
    text,count=re.subn(r'(?m)^'+key+r'\s*=.*$',key+' = '+value,text)
    assert count==1,key
text=re.sub(r'(?m)^preferred_spawn_biome\s*=.*\n?', '', text)
text="preferred_spawn_biome = 'forest'\n"+text
p.write_text(text)
world=tomllib.loads(text)
assert world['trees'] and world['biomes'] and world['water'] and world['caves']
PY
cat > "$run_dir/configs/client.toml" <<'TOML'
width = 1920
height = 1080
view_distance = 8
vertical_view_distance = 3
fov_degrees = 75.0
vsync = false
debug_overlay = false
max_chunk_jobs = 4
max_mesh_jobs = 8
TOML
cat > "$run_dir/configs/graphics.toml" <<'TOML'
width = 1920
height = 1080
window_mode = "windowed"
render_distance = 8
fov = 75.0
vsync = false
fps_limit = 60
shadow_quality = "high"
cave_quality = "medium"
water_reflections = true
weather = "clear"
weather_intensity = 0.0
TOML
cat > "$run_dir/configs/server.toml" <<'TOML'
natural_mob_limit = 24
lan_discovery = false
require_verified_accounts = false
TOML
printf 'Isolated three-minute forest soak: %s\n' "$run_dir"
status=0
env -u TERRA_BENCH_CAMERA -u TERRA_BENCH_LOOK_AT -u TERRA_BENCH_WEATHER \
    -u TERRA_BENCH_SAVE_DIR -u TERRA_BENCH_WORLD_ID -u TERRA_BENCH_UI \
    -u TERRA_BIOME_SCENE -u TERRA_BIOME_ATMOSPHERE -u TERRA_FOLIAGE_INSPECT \
    -u TERRA_ROADS_INSPECT -u TERRA_TAR_INSPECT -u TERRA_FOG_BLOCK_INSPECT \
    -u TERRA_FOREST_INSPECT -u TERRA_SPIRIT_INSPECT -u TERRA_SANDWORM_INSPECT \
    -u TERRA_PERF_EDIT_MODE -u TERRA_INVENTORY_BENCHMARK \
    TERRA_CONFIG_DIR="$run_dir/configs" TERRA_MENU_CONFIG_DIR="$run_dir/configs" \
    TERRA_WORLD_DIR="$run_dir/worlds" TERRA_PROFILE_FILE="$run_dir/profiles/inspection.toml" \
    TERRA_KINGDOM_SAVE_DIR="$run_dir/kingdom-save" TERRA_ASSET_DIR="$repo_dir/assets" \
    TERRA_RUNTIME_HEALTH_FILE="$run_dir/runtime-health.jsonl" TERRA_BENCH_GRAPHICS=1 \
    TERRA_BENCH_SECONDS=180 TERRA_BENCH_LOAD_SECONDS=180 TERRA_BENCH_MODE=integrated \
    TERRA_SCREENSHOT="$run_dir/captures/forest-soak.png" \
    TERRA_COMPUTE_THREADS=4 RUST_LOG=info \
    timeout --signal=TERM --kill-after=10s 540s "$run_dir/terra-client" --smoke-test-benchmark \
    > "$run_dir/native.log" 2>&1 || status=$?
printf '%s\n' "$status" > "$run_dir/exit-status"
python3 - "$run_dir" "$status" <<'PY'
from pathlib import Path
import hashlib,json,re,statistics,struct,sys,tomllib
root=Path(sys.argv[1]); errors=[]
if int(sys.argv[2]): errors.append('Client exited with status'+sys.argv[2])
log=re.sub(r'\x1b\[[0-9;]*m','',(root/'native.log').read_text(errors='replace'))
if re.search(r'\bERROR\b|panicked at|Validation Error|Shader processing error|DeviceLost',log):
    errors.append('Runtime/render error in native log')
if 'Client smoke test completed' not in log: errors.append('Missing successful native completion')
summary=next((line for line in log.splitlines() if 'Native smoke benchmark (CPU wall frames;' in line),'')
fps={}
for field in ['average_fps','minimum_fps','maximum_fps','p95_frame_ms','p99_frame_ms','sample_frames','sample_window_seconds']:
    match=re.search(r'\b'+field+r'=(?:Some\()?([0-9.eE+-]+)',summary)
    if match: fps[field]=float(match[1])
if fps.get('sample_window_seconds',0)<180 or fps.get('sample_frames',0)<30:
    errors.append('Missing complete three-minute native FPS sample')
path=root/'runtime-health.jsonl'
samples=[json.loads(line) for line in path.read_text().splitlines() if line.strip()] if path.is_file() else []
ready=[s for s in samples if s.get('ready') and s.get('rss_kib') is not None]
plateau={}; late=[]
if ready:
    if samples[-1]['seconds']-ready[-1]['seconds']>4.5:
        errors.append('Scene did not remain ready at the end of the soak')
    late=[s for s in ready if s['seconds']>=ready[-1]['seconds']-60]
if len(late)<25 or late[-1]['seconds']-late[0]['seconds']<55:
    errors.append('Need at least55seconds and25ready samples for the final memory window')
else:
    rss=[s['rss_kib'] for s in late]
    plateau=dict(window_seconds=late[-1]['seconds']-late[0]['seconds'],samples=len(late),
                 rss_first_kib=rss[0],rss_last_kib=rss[-1],rss_min_kib=min(rss),rss_max_kib=max(rss),
                 rss_net_growth_kib=rss[-1]-rss[0],rss_range_kib=max(rss)-min(rss),
                 material_assets_growth=late[-1]['material_assets']-late[0]['material_assets'],
                 image_assets_growth=late[-1]['image_assets']-late[0]['image_assets'])
    low=rss[0]; upward=0
    for value in rss:
        low=min(low,value); upward=max(upward,value-low)
    plateau['rss_max_upward_excursion_kib']=upward
    if upward>=128*1024: errors.append('Final-minute RSS grew by128MiB after an earlier sample')
    swaps=[s.get('swap_kib') for s in late if s.get('swap_kib') is not None]
    if swaps:
        plateau['swap_first_kib']=swaps[0]; plateau['swap_last_kib']=swaps[-1]
        if swaps[-1]-swaps[0]>64*1024: errors.append('Private swapped pages grew by64MiB in final minute')
    for field in ['material_assets','image_assets']:
        values=[s[field] for s in late]
        if values[-1]>values[0]+2 or max(values)-min(values)>4:
            errors.append(field+' kept growing in the final minute')
    plateau['last']=late[-1]
capture=root/'captures/forest-soak.png'
if not capture.is_file(): errors.append('Missing forest camera capture')
else:
    data=capture.read_bytes()
    if data[:8]!=b'\x89PNG\r\n\x1a\n' or struct.unpack('>II',data[16:24])!=(1920,1080):
        errors.append('Forest capture must be1920x1080')
digest=lambda p:hashlib.file_digest(p.open('rb'),'sha256').hexdigest()
expected=(root/'client.sha256').read_text().split()[0]
if digest(root/'terra-client')!=expected: errors.append('Pinned client changed')
report=dict(status='failed' if errors else 'passed',errors=errors,client_sha256=expected,
            fps=fps,plateau=plateau,sample_count=len(samples),ready_samples=len(ready),
            forest_anchor=dict(seed=42042,preferred_spawn_biome='forest',placement='actual server safe-spawn search'),
            capture=str(capture.relative_to(root)),
            scope='Three-minute stationary native integrated-server forest benchmark at1080p, view8. The interactive frontend FPS limiter is absent, so this measures uncapped throughput despite the requested60FPS configuration. '
                  'Final-minute RSS and material/image asset counts measure local boundedness; includes a local integrated server; not a long-duration guarantee or a remote multiplayer soak.')
(root/'verification.json').write_text(json.dumps(report,indent=2)+'\n')
print(f"Forest idle soak {report['status']}: {root/'verification.json'}")
for error in errors: print(error,file=sys.stderr)
raise SystemExit(bool(errors))
PY
