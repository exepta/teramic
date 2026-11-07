#!/usr/bin/env python3
"""Isolated native lantern acceptance. Requires Root's current client build."""
from pathlib import Path
import datetime
import hashlib
import json
import os
import shutil
import subprocess
import sys

REPORT = Path(__file__).resolve().parent
ROOT = REPORT.parents[4]
PRIVATE = ROOT / 'target/phase199-lanterns'
CONFIG = PRIVATE / 'configs'
BINARY = ROOT / 'target/debug/terra-client'
CAPTURES = ['lantern-gallery-day', 'lantern-gallery-night', 'lantern-wall-close',
            'lantern-table-close', 'lantern-chain-close']


def prepare():
    CONFIG.mkdir(parents=True, exist_ok=True)
    for source in (ROOT / 'configs').glob('*.toml'):
        if source.name != 'profile.toml':
            shutil.copy2(source, CONFIG / source.name)
    (CONFIG / 'client.toml').write_text('''width = 1920
height = 1080
fov_degrees = 55.0
vsync = false
debug_overlay = false
log_filter = "info"
view_distance = 3
vertical_view_distance = 3
max_chunk_jobs = 4
max_mesh_jobs = 8
mesh_uploads_per_frame = 12
weather = "clear"
fog_distance = 0.0
''')
    graphics = (CONFIG / 'graphics.toml').read_text()
    for old, new in [('width = 2560', 'width = 1920'), ('height = 1440', 'height = 1080'),
                     ('render_distance = 20', 'render_distance = 3'),
                     ('fog_distance = 1.0', 'fog_distance = 0.0'),
                     ('fov = 90.0', 'fov = 55.0'),
                     ('debug_overlay = true', 'debug_overlay = false')]:
        graphics = graphics.replace(old, new)
    (CONFIG / 'graphics.toml').write_text(graphics)
    (CONFIG / 'world.toml').write_text('''id = 1
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
    audio = (CONFIG / 'audio.toml').read_text().replace('muted = false', 'muted = true')
    (CONFIG / 'audio.toml').write_text(audio)
    server = (CONFIG / 'server.toml').read_text().replace('save_directory = "worlds"',
              'save_directory = ' + json.dumps(str(PRIVATE / 'worlds')))
    (CONFIG / 'server.toml').write_text(server)


def main():
    prepare()
    relevant = [ROOT / 'apps/client/src/lantern_smoke.rs', ROOT / 'apps/client/src/bootstrap.rs',
                ROOT / 'crates/terra-core/src/lantern.rs',
                ROOT / 'crates/terra-world/src/lanterns.rs',
                ROOT / 'crates/terra-render/src/meshing/custom.rs',
                ROOT / 'crates/terra-render/src/scene/caves/lights.rs']
    metadata = {'prepared_at': datetime.datetime.now(datetime.timezone.utc).isoformat(),
                'binary': str(BINARY), 'arguments': ['--smoke-test-lanterns'],
                'source_files': {str(p.relative_to(ROOT)): hashlib.sha256(p.read_bytes()).hexdigest()
                                 for p in relevant}}
    if not BINARY.exists() or BINARY.stat().st_mtime_ns < max(p.stat().st_mtime_ns for p in relevant):
        metadata['status'] = 'waiting_for_current_binary'
        (REPORT / 'native-metadata.json').write_text(json.dumps(metadata, indent=2)+'\n')
        print('Waiting: the current target/debug/terra-client predates lantern fixture sources.')
        return 2
    if '--prepare-only' in sys.argv:
        print('Private configs ready; binary is current. Native execution was not requested.')
        return 0
    env = os.environ.copy()
    env.update({'TERRA_CONFIG_DIR': str(CONFIG), 'TERRA_MENU_CONFIG_DIR': str(CONFIG),
                'TERRA_WORLD_DIR': str(PRIVATE / 'worlds'),
                'TERRA_ASSET_DIR': str(ROOT / 'assets'), 'TERRA_CAPTURE_DIR': str(REPORT),
                'TERRA_BENCH_GRAPHICS': '1', 'TERRA_COMPUTE_THREADS': '4',
                'TMPDIR': str(ROOT / 'target/tmp197'), 'WINIT_UNIX_BACKEND': 'x11'})
    metadata['environment'] = {key: env[key] for key in [
        'TERRA_CONFIG_DIR', 'TERRA_MENU_CONFIG_DIR', 'TERRA_WORLD_DIR', 'TERRA_ASSET_DIR',
        'TERRA_CAPTURE_DIR', 'TERRA_BENCH_GRAPHICS', 'TERRA_COMPUTE_THREADS', 'TMPDIR',
        'DISPLAY', 'WINIT_UNIX_BACKEND'] if key in env}
    metadata['binary_mtime_ns'] = BINARY.stat().st_mtime_ns
    metadata['status'] = 'running'
    (REPORT / 'native-metadata.json').write_text(json.dumps(metadata, indent=2)+'\n')
    with (REPORT / 'native.log').open('w') as log:
        try:
            result = subprocess.run([str(BINARY), '--smoke-test-lanterns'], cwd=ROOT, env=env,
                                    stdout=log, stderr=subprocess.STDOUT, timeout=190)
            code = result.returncode
        except subprocess.TimeoutExpired:
            code = 124
    log = (REPORT / 'native.log').read_text()
    metadata['exit_code'] = code
    metadata['captures'] = {name: {'exists': (REPORT / (name+'.png')).exists(),
                                   'bytes': (REPORT / (name+'.png')).stat().st_size
                                   if (REPORT / (name+'.png')).exists() else 0}
                            for name in CAPTURES}
    passed = code == 0 and 'Lantern native gallery passed' in log and all(
        item['exists'] and item['bytes'] > 1024 for item in metadata['captures'].values())
    metadata['status'] = 'passed' if passed else 'failed'
    (REPORT / 'native-metadata.json').write_text(json.dumps(metadata, indent=2)+'\n')
    print(json.dumps({'status': metadata['status'], 'exit_code': code,
                      'capture_count': sum(item['exists'] for item in metadata['captures'].values()),
                      'report': str(REPORT)}))
    return 0 if passed else 1


if __name__ == '__main__':
    raise SystemExit(main())
