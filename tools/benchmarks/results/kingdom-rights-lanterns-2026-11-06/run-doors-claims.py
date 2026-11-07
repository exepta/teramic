#!/usr/bin/env python3
"""Prepare private native fixtures; --run doors|claims starts one current client.
Configuration/profile/world/map data stay under target/phase199-{doors,claims}.
The report receives only logs, screenshots and reproducibility metadata.
"""
import argparse
import datetime
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import struct
import subprocess
import tomllib

REPORT = Path(__file__).resolve().parent
ROOT = next(parent for parent in REPORT.parents if (parent / 'Cargo.toml').exists())
BINARY = ROOT / 'target/debug/terra-client'
CASES = {
    'doors': {
        'argument': '--smoke-test-window-gallery',
        'flag': 'TERRA_DOOR_GALLERY',
        'captures': ['doors-closed', 'doors-open', 'oak-double-door-open', 'birch-door-separated'],
        'success': 'Native connected-opening gallery passed',
        'timeout': 90,
        'sources': ['apps/client/src/window_gallery_smoke.rs', 'crates/terra-world/src/objects/doors.rs',
                    'crates/terra-render/src/openings.rs', 'crates/terra-render/src/openings/windows.rs'],
    },
    'claims': {
        'argument': '--smoke-test-inventory',
        'flag': 'TERRA_CLAIMS_GALLERY',
        'captures': ['diplomacy-claims', 'diplomacy-live-color', 'diplomacy-compact',
                     'diplomacy-rectangle-preview', 'diplomacy-rightclick-unselect'],
        'success': 'Native claims projection gallery passed',
        'timeout': 175,
        'sources': ['apps/client/src/claims_smoke.rs', 'crates/terra-ui/src/realm_relations.rs',
                    'crates/terra-ui/src/minimap/fullmap.rs', 'src-ui/components/full-map.component.html',
                    'src-ui/components/full-map.component.css'],
    },
}


def replace_field(text, key, value):
    line = f'{key} = {value}'
    pattern = rf'(?m)^{re.escape(key)}\s*=.*$'
    if re.search(pattern, text):
        return re.sub(pattern, lambda _: line, text, count=1)
    return line + '\n' + text


def prepare(case):
    private = ROOT / 'target' / ('phase199-' + case)
    configs = private / 'configs'
    configs.mkdir(parents=True, exist_ok=True)
    (private / 'worlds').mkdir(exist_ok=True)
    for source in (ROOT / 'configs').glob('*.toml'):
        if source.name != 'profile.toml':
            shutil.copy2(source, configs / source.name)
    (configs / 'client.toml').write_text('''width = 2560
height = 1440
fov_degrees = 55.0
vsync = false
debug_overlay = false
log_filter = "info"
view_distance = 4
vertical_view_distance = 3
max_chunk_jobs = 4
max_mesh_jobs = 8
mesh_uploads_per_frame = 12
weather = "clear"
fog_distance = 0.8
''')
    graphics = (configs / 'graphics.toml').read_text()
    for key, value in {'width': '2560', 'height': '1440', 'window_mode': '"windowed"',
                       'fps_limit': '60', 'render_distance': '4', 'fog_distance': '0.8',
                       'fov': '55.0', 'ui_scale': '1.0', 'debug_overlay': 'false'}.items():
        graphics = replace_field(graphics, key, value)
    (configs / 'graphics.toml').write_text(graphics)
    (configs / 'world.toml').write_text('''id = 1
seed = 42042
base_height = 16.0
hill_amplitude = 0.0
mountain_amplitude = 0.0
sea_level = 0.0
terrace_strength = 0.0
landscape_revision = 0
ecology_revision = 0
geology_revision = 0
ocean_revision = 0
structures_revision = 0
cave_structure_revision = 0
river_terrain_revision = 0
biomes = false
caves = false
trees = false
water = false
rivers = false
''')
    server = (configs / 'server.toml').read_text()
    for key, value in {'save_directory': json.dumps(str(private / 'worlds')),
                       'lan_discovery': 'false', 'bind_address': json.dumps('127.0.0.1:' + ('51991' if case == 'doors' else '51992'))}.items():
        server = replace_field(server, key, value)
    (configs / 'server.toml').write_text(server)
    audio = replace_field((configs / 'audio.toml').read_text(), 'muted', 'true')
    (configs / 'audio.toml').write_text(audio)
    for path in configs.glob('*.toml'):
        tomllib.loads(path.read_text())
    return private, configs


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--run', choices=CASES, help='Run exactly one native client after preparation')
    parser.add_argument('--prepare-only', action='store_true', help='Prepare both private configurations without launching')
    args = parser.parse_args()
    if args.run and args.prepare_only:
        parser.error('Choose --run or --prepare-only')
    for case in ([args.run] if args.run else CASES):
        spec = CASES[case]
        private, configs = prepare(case)
        output = REPORT / case
        output.mkdir(exist_ok=True)
        metadata = {
            'prepared_at': datetime.datetime.now(datetime.timezone.utc).isoformat(),
            'private_root': str(private.relative_to(ROOT)),
            'argument': spec['argument'], 'native_flag': spec['flag'],
            'expected_captures': spec['captures'],
            'config_sha256': {p.name: hashlib.sha256(p.read_bytes()).hexdigest() for p in configs.glob('*.toml')},
            'source_sha256': {p: hashlib.sha256((ROOT / p).read_bytes()).hexdigest() for p in spec['sources']},
            'status': 'prepared_without_launch',
        }
        if args.run:
            if not BINARY.exists() or BINARY.stat().st_mtime_ns < max((ROOT / p).stat().st_mtime_ns for p in spec['sources']):
                raise SystemExit('Current client build required; no native app started.')
            env = {key: value for key, value in os.environ.items() if not key.startswith('TERRA_')}
            env.update(TERRA_CONFIG_DIR=str(configs), TERRA_MENU_CONFIG_DIR=str(configs),
                       TERRA_PROFILE_FILE=str(private / 'profile.toml'), TERRA_WORLD_DIR=str(private / 'worlds'),
                       TERRA_ASSET_DIR=str(ROOT / 'assets'), TERRA_CAPTURE_DIR=str(output),
                       TERRA_BENCH_GRAPHICS='1', TERRA_COMPUTE_THREADS='4')
            env[spec['flag']] = '1'
            metadata['environment'] = {key: value for key, value in env.items() if key.startswith('TERRA_')}
            metadata['status'] = 'running'
            (output / 'native-metadata.json').write_text(json.dumps(metadata, indent=2) + '\n')
            with (output / 'native.log').open('w') as log:
                try:
                    result = subprocess.run([str(BINARY), spec['argument']], cwd=ROOT, env=env,
                                            stdout=log, stderr=subprocess.STDOUT, timeout=spec['timeout'])
                    code = result.returncode
                except subprocess.TimeoutExpired:
                    code = 124
            captures = {}
            for name in spec['captures']:
                path = output / (name + '.png')
                if path.exists():
                    data = path.read_bytes()
                    captures[name] = {'size': list(struct.unpack('>II', data[16:24])),
                                      'bytes': len(data), 'sha256': hashlib.sha256(data).hexdigest()}
            passed = code == 0 and spec['success'] in (output / 'native.log').read_text() and len(captures) == len(spec['captures'])
            metadata.update(exit_code=code, captures=captures, status='passed' if passed else 'failed')
        (output / 'native-metadata.json').write_text(json.dumps(metadata, indent=2) + '\n')
        print(json.dumps({'case': case, 'status': metadata['status'], 'private_root': str(private.relative_to(ROOT))}))
        if metadata['status'] == 'failed':
            return 1
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
