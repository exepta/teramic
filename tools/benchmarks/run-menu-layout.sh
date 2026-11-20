#!/usr/bin/env bash
# Production menu handlers, shaped text, German/English widths, two real windows.
set -euo pipefail
repo_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)
cd -- "$repo_dir"
client_source="${TERRA_CLIENT_BINARY:-$repo_dir/target/debug/terra-client}"
[[ -x "$client_source" ]] || { printf 'Build terra-client before this gallery.\n' >&2; exit 1; }
mkdir -p target/menu-layout
run_dir=$(mktemp -d "$repo_dir/target/menu-layout/run.XXXXXX")
client_bin="$run_dir/terra-client"
ln -- "$client_source" "$client_bin"
trap 'rm -f -- "$client_bin"' EXIT
sha256sum "$client_bin" > "$run_dir/client.sha256"
printf 'Isolated native menu layout run: %s\n' "$run_dir"
for resolution in 1920x1080 960x640; do
    case_dir="$run_dir/$resolution"
    mkdir -p "$case_dir/configs" "$case_dir/worlds" "$case_dir/profiles"
    width=${resolution%x*}
    height=${resolution#*x}
    cat > "$case_dir/configs/client.toml" <<TOML
width = $width
height = $height
vsync = false
debug_overlay = false
TOML
    cat > "$case_dir/configs/graphics.toml" <<TOML
width = $width
height = $height
window_mode = "windowed"
vsync = false
fps_limit = 60
render_distance = 2
TOML
    printf 'language = "de"\n' > "$case_dir/configs/gameplay.toml"
    env -u TERRA_BENCH_UI -u TERRA_MENU_REFERENCE_SMOKE \
        TERRA_LOCALIZATION_GALLERY=1 TERRA_CONFIG_DIR="$case_dir/configs" \
        TERRA_MENU_CONFIG_DIR="$case_dir/configs" TERRA_MENU_SMOKE_DIR="$case_dir" \
        TERRA_PROFILE_FILE="$case_dir/profiles/local.toml" TERRA_WORLD_DIR="$case_dir/worlds" \
        TERRA_KINGDOM_SAVE_DIR="$case_dir/kingdom-save" TERRA_ASSET_DIR="$repo_dir/assets" \
        TERRA_COMPUTE_THREADS=6 RUST_LOG=info \
        timeout --signal=TERM --kill-after=10s 210s "$client_bin" --smoke-test-menu \
        > "$case_dir/native.log" 2>&1
    printf 'Completed %s\n' "$resolution"
done
sha256sum --check "$run_dir/client.sha256"
python3 - "$run_dir" <<'PY'
from pathlib import Path
import hashlib,json,re,struct,sys
root=Path(sys.argv[1])
report={'status':'passed','client_sha256':(root/'client.sha256').read_text().split()[0],'cases':{}}
for resolution in ['1920x1080','960x640']:
    case=root/resolution
    log=re.sub(r'\x1b\[[0-9;]*m','',(case/'native.log').read_text(errors='replace'))
    assert 'Native language UI passed:' in log,resolution
    assert not re.search(r'\bERROR\b|panicked at|Validation Error|Shader processing error',log),resolution
    captures=[]
    requested_size=tuple(map(int,resolution.split('x')))
    actual_size=None
    for name in ['main-menu-de','gameplay-de','gameplay-en','main-menu-en']:
        path=case/'captures'/(name+'.png')
        data=path.read_bytes()
        size=struct.unpack('>II',data[16:24])
        # Native window negotiation can differ by one physical pixel. Record
        # the real result; every capture in the case must have that same size.
        assert all(abs(a-b)<=1 for a,b in zip(size,requested_size)),(path,size)
        if actual_size is None:
            actual_size=size
        assert size==actual_size,(path,size,actual_size)
        captures.append({'file':str(path.relative_to(root)),'sha256':hashlib.sha256(data).hexdigest()})
    measurements=[]
    for language in ['de','en']:
        measurement=json.loads((case/'captures'/f'category-layout-{language}.json').read_text())
        assert measurement['status']=='passed' and len(measurement['buttons'])==5
        measurements.append(measurement)
    report['cases'][resolution]={'requested_resolution':requested_size,'actual_resolution':actual_size,
                                'captures':captures,'measurements':measurements}
(root/'verification.json').write_text(json.dumps(report,indent=2)+'\n')
print('Native menu sizing passed: 8 captures, German/English, 1920x1080 and 960x640')
PY
