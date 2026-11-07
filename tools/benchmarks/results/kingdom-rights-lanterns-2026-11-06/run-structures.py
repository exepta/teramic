#!/usr/bin/env python3
"""Native revision-eight photographs; all mutable fixtures stay under target/."""
from pathlib import Path
import argparse
import hashlib
import json
import os
import shutil
import subprocess

REPORT = Path(__file__).resolve().parent
ROOT = REPORT.parents[3]
PRIVATE = ROOT / 'target/phase199-structures'


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('scene', choices=['kingdom-temperate', 'kingdom-winter', 'outpost'])
    parser.add_argument('--region', type=json.loads, help='Explicit real seeded region [x,z]')
    args = parser.parse_args()
    config = PRIVATE / 'configs'
    config.mkdir(parents=True, exist_ok=True)
    for source in (ROOT / 'configs').glob('*.toml'):
        if source.name != 'profile.toml':
            shutil.copy2(source, config / source.name)
    photo = ROOT / 'tools/benchmarks/results/kingdom-terrain-worldcreate-2026-11-06/configs/structure-photo-graphics.toml'
    (config / 'graphics.toml').write_text(photo.read_text().replace('debug_overlay = true', 'debug_overlay = false'))
    audio = config / 'audio.toml'
    audio.write_text(audio.read_text().replace('muted = false', 'muted = true'))
    output = REPORT / 'native' / args.scene
    output.mkdir(parents=True, exist_ok=True)
    env = os.environ.copy()
    env.update({
        'TERRA_CONFIG_DIR': str(config), 'TERRA_MENU_CONFIG_DIR': str(config),
        'TERRA_ASSET_DIR': str(ROOT / 'assets'),
        'TERRA_WORLD_DIR': str(PRIVATE / 'worlds'),
        'TERRA_CAPTURE_DIR': str(output), 'TERRA_BIOME_SCENE': args.scene,
        'TERRA_BENCH_GRAPHICS': '1', 'TERRA_BENCH_SECONDS': '300',
        'TERRA_COMPUTE_THREADS': '4',
        'TMPDIR': str(ROOT / 'target/tmp197'), 'WINIT_UNIX_BACKEND': 'x11',
    })
    env.pop('TERRA_STRUCTURE_REGION', None)
    if args.region is not None:
        assert len(args.region) == 2 and all(type(v) is int and abs(v) <= 2500 for v in args.region)
        env['TERRA_STRUCTURE_REGION'] = json.dumps(args.region)
    binary = ROOT / 'target/debug/terra-client'
    command = [str(binary), '--smoke-test-biomes']
    with (output / 'native.log').open('w') as log:
        try:
            code = subprocess.run(command, cwd=ROOT, env=env, stdout=log,
                                  stderr=subprocess.STDOUT, timeout=360).returncode
        except subprocess.TimeoutExpired:
            code = 124
    captures = sorted(output.glob('*.png'))
    completed = 'Native biome fixture passed:' in (output / 'native.log').read_text()
    passed = code == 0 and len(captures) == 2 and completed
    metadata = {'scene': args.scene, 'structures_revision': 8, 'seed': 42042,
                'exit_code': code, 'status': 'passed' if passed else 'failed', 'command': command,
                'environment': {k: v for k, v in env.items() if k.startswith('TERRA_')},
                'images': {p.name: hashlib.sha256(p.read_bytes()).hexdigest() for p in captures}}
    (output / 'metadata.json').write_text(json.dumps(metadata, indent=2) + '\n')
    print(json.dumps({'scene': args.scene, 'exit_code': code, 'images': len(captures)}))
    return 0 if passed else 1


if __name__ == '__main__':
    raise SystemExit(main())
