#!/usr/bin/env bash
# Native rigid-part voxel models and selected animation poses; no AI integration claim.
# Uses the already-built client. Never builds or opens a user save.
set -euo pipefail
repo_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)
cd -- "$repo_dir"
client_source="${TERRA_CLIENT_BINARY:-$repo_dir/target/debug/terra-client}"
[[ -x "$client_source" ]] || { printf 'Build terra-client before running this gallery.\n' >&2; exit 1; }
scenes=("$@")
if (( ${#scenes[@]} == 0 )); then scenes=(standard spirit forest aquatic horse spiders villagers sandworm dragons); fi
for scene in "${scenes[@]}"; do
    case "$scene" in standard|spirit|forest|aquatic|horse|spiders|villagers|sandworm|dragons) ;; *) printf 'Unknown gallery: %s\n' "$scene" >&2; exit 2;; esac
done
mkdir -p target/voxel-creatures
run_dir=$(mktemp -d "$repo_dir/target/voxel-creatures/run.XXXXXX")
# A concurrent Cargo link may replace the original executable. Pin the inode
# for all seven scenes without copying a large binary; release it on exit.
client_bin="$run_dir/terra-client"
ln -- "$client_source" "$client_bin"
trap 'rm -f -- "$client_bin"' EXIT
mkdir -p "$run_dir/configs" "$run_dir/profiles" "$run_dir/worlds"
cat > "$run_dir/configs/client.toml" <<'TOML'
width = 1920
height = 1080
view_distance = 3
vertical_view_distance = 3
vsync = false
debug_overlay = false
TOML
cat > "$run_dir/configs/graphics.toml" <<'TOML'
width = 1920
height = 1080
window_mode = "windowed"
render_distance = 3
vsync = false
fps_limit = 60
shadow_quality = "high"
weather = "clear"
weather_intensity = 0.0
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
sha256sum "$client_bin" > "$run_dir/client.sha256"
printf 'Isolated voxel creature run: %s\n' "$run_dir"
export TERRA_CONFIG_DIR="$run_dir/configs" TERRA_MENU_CONFIG_DIR="$run_dir/configs"
export TERRA_PROFILE_FILE="$run_dir/profiles/inspection.toml" TERRA_WORLD_DIR="$run_dir/worlds"
export TERRA_KINGDOM_SAVE_DIR="$run_dir/kingdom-save" TERRA_ASSET_DIR="$repo_dir/assets"
export TERRA_BENCH_GRAPHICS=1 TERRA_COMPUTE_THREADS=6 RUST_LOG=info
unset TERRA_FOREST_INSPECT TERRA_SPIRIT_INSPECT TERRA_SMOKE_VILLAGERS TERRA_HORSE_INSPECT TERRA_SANDWORM_INSPECT TERRA_DRAGON_INSPECT
unset TERRA_AQUATIC_INSPECT TERRA_LIFE_INSPECT TERRA_CREATURE_INSPECT TERRA_BEE_INSPECT
unset TERRA_BEAR_INSPECT TERRA_BIOME_SCENE TERRA_BIOME_ATMOSPHERE
unset TERRA_EYE_INSPECT TERRA_SPIDER_MODEL_INSPECT
cp "$run_dir/configs/graphics.toml" "$run_dir/graphics-default.toml"
for scene in "${scenes[@]}"; do
    cp "$run_dir/graphics-default.toml" "$run_dir/configs/graphics.toml"
    # The spread wings and cast targets extend across most of a three-chunk
    # fixture. Keep the far plane while retaining the full model silhouette.
    if [[ "$scene" == dragons ]]; then
        printf '\nfog_distance = 0.0\n' >> "$run_dir/configs/graphics.toml"
    fi
    mkdir -p "$run_dir/$scene"
    scene_env=()
    case "$scene" in
        spirit) scene_env+=(TERRA_SPIRIT_INSPECT=1);;
        forest) scene_env+=(TERRA_FOREST_INSPECT=1);;
        dragons) scene_env+=(TERRA_DRAGON_INSPECT=1);;
        sandworm) scene_env+=(TERRA_SANDWORM_INSPECT=1);;
        aquatic) scene_env+=(TERRA_AQUATIC_INSPECT=1);;
        horse) scene_env+=(TERRA_HORSE_INSPECT=1);;
        spiders) scene_env+=(TERRA_SPIDER_MODEL_INSPECT=1);;
        villagers) scene_env+=(TERRA_SMOKE_VILLAGERS=1);;
    esac
    if env "${scene_env[@]}" TERRA_CAPTURE_DIR="$run_dir/$scene" \
        timeout --signal=TERM --kill-after=10s 180s "$client_bin" --smoke-test-creatures \
        > "$run_dir/$scene/native.log" 2>&1; then
        printf 'Completed %s\n' "$scene"
    else
        status=$?
        printf 'Gallery %s failed (%s); log: %s/%s/native.log\n' "$scene" "$status" "$run_dir" "$scene" >&2
        exit "$status"
    fi
done
sha256sum --check "$run_dir/client.sha256"
python3 - "$run_dir" "${scenes[@]}" <<'PYTHON'
from pathlib import Path
import hashlib
import json
import struct
import sys

root = Path(sys.argv[1])
scenes = (
    ('standard', 'creatures', 13, 'Creature model inspection passed: six species, two fish variants'),
    ('spirit', 'spirit-creatures', 4, 'Spirit gallery rendered seven butterfly variants'),
    ('forest', 'forest-creatures', 6, 'Forest gallery rendered three Skeleton loadouts'),
    ('dragons', 'dragon', 31, 'Dragon gallery passed:'),
    ('sandworm', 'sandworm', 15, 'Sandworm gallery passed:'),
    ('aquatic', 'creatures', 13, 'Aquatic creature model inspection passed:'),
    ('horse', 'horses', 4, 'Horse inspection passed: sixteen coats'),
    ('spiders', 'spiders', 4, 'Spider model gallery passed: three sizes;'),
    ('villagers', 'villagers', 5, 'Villager gallery passed: ten representative professions'),
)
failures = (
    'panicked at', 'Validation Error', 'Shader processing error',
    'failed to create shader module', 'Creature model failed',
    'failed to process shader error',
    'screenshot is blank', 'DeviceLost', 'device lost',
)
report = {
    'status': 'passed',
    'scope': 'Native model/pose fixtures; not authoritative AI, navigation or swimming tests.',
    'client_sha256': (root / 'client.sha256').read_text().split()[0],
    'scenes': {},
}
for scene, prefix, count, marker in scenes:
    if scene not in sys.argv[2:]:
        continue
    folder = root / scene
    log = (folder / 'native.log').read_text(errors='replace')
    assert marker in log, (scene, 'missing completion marker')
    for failure in failures:
        assert failure.casefold() not in log.casefold(), (scene, failure)
    poses = ('dark-rest', 'elder-roar', 'shadow-rune', 'earth-rest', 'earth-takeoff', 'earth-impact', 'water-flight', 'water-glide', 'water-bank', 'water-landing', 'earth-stride', 'ice-breath', 'shadow-breath', 'earth-breath', 'deep-swim', 'earth-sleep', 'dark-fall', 'ruin-fissure', 'wyrm-phase', 'void-orbs', 'tidal-wave', 'deepwater-barrier', 'icewater-geyser', 'mountain-boulders', 'granite-shield', 'magma-fissure', 'water-baby', 'water-young', 'water-adolescent', 'water-young-adult', 'water-grown') if scene == 'dragons' else range(count)
    expected = {f'{prefix}-{pose}.png' for pose in poses}
    actual = {p.name for p in folder.glob('*.png')}
    assert actual == expected, (scene, expected - actual, actual - expected)
    captures = []
    for pose in poses:
        path = folder / f'{prefix}-{pose}.png'
        data = path.read_bytes()
        assert data[:8] == b'\x89PNG\r\n\x1a\n', path
        assert struct.unpack('>II', data[16:24]) == (1920, 1080), path
        captures.append({'file': path.name, 'sha256': hashlib.sha256(data).hexdigest()})
    if scene == 'sandworm':
        proof = json.loads((folder / 'sandworm-report.json').read_text())
        assert proof['status'] == 'passed' and proof['captures'] == count
        assert proof['phases'] == ['burrowed', 'tremor', 'leap', 'upright', 'bite', 'spit', 'retreat', 'mouth_detail', 'vertical_rise', 'top_bend', 'moving_burrow', 'stepped_burrow', 'distant_burrow', 'passive_burrow', 'underground_charge']
        assert proof['combat_vertical_exit'] and proof['combat_reuses_dive_hole'] and proof['combat_retreat_same_shaft']
        assert proof['body_sections'] == 24 and proof['tapered_tail']
        assert proof['rear_collar_attachment'] and proof['body_kept_out_of_mouth'] and proof['recessed_throat']
        assert proof['scalloped_cuticle']
        assert proof['buried_anatomy_hidden'] and not proof['visible_buried_back'] and proof['body_length_multiple'] == 5
        assert proof['animated_closing_mandibles'] == 3 and proof['cached_bile_droplets'] == 3
        assert proof['continuous_advancing_sand_crest'] and proof['distance_sampled_trail'] and proof['stepped_dune_projection']
        assert proof['passive_trail_without_attack_goal'] and proof['charge_has_sand_trail']
        trails = []
        for stage in (10, 11, 12, 13, 14):
            trail = json.loads((folder / f'sandworm-{stage}-trail.json').read_text())
            assert 4 <= trail['visible_track_meshes'] <= 128, (stage, trail)
            assert trail['moving_buried_body_hidden']
            assert trail['stepped_dune'] == (stage == 11)
            assert trail['distant_view'] == (stage == 12)
            assert trail['passive_burrow'] == (stage == 13)
            assert trail['underground_charge'] == (stage == 14)
            if stage >= 13:
                assert trail['passive_lane_track_meshes'] >= 4 and trail['no_attack_goal']
                assert trail['zero_reported_velocity'] == (stage == 13)
            trails.append(dict(stage=stage, **trail))
    report['scenes'][scene] = {'count': count, 'marker': marker, 'captures': captures}
    if scene == 'sandworm':
        report['scenes'][scene]['motion_trails'] = trails
report['total_captures'] = sum(scene['count'] for scene in report['scenes'].values())
(root / 'verification.json').write_text(json.dumps(report, indent=2) + '\n')
print(f'Native voxel creature galleries passed: {report["total_captures"]} captures in {root}')
PYTHON
