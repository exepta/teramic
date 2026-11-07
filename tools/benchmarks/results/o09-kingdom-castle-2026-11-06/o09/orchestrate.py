#!/usr/bin/env python3
"""Run only inside the root agent's exclusive O09 build/native window."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import time

SCRIPT = Path(__file__).resolve().parent
PROJECT = Path(os.environ.get('O09_PROJECT_ROOT', SCRIPT.parents[4])).resolve()
UPSTREAM = Path(os.environ.get('O09_UPSTREAM_ROOT', PROJECT / '../../tilt-us/tilt-ui'))
UPSTREAM = UPSTREAM.resolve()
ARTIFACT = Path(os.environ.get('O09_ARTIFACT_ROOT', PROJECT / 'target/o09-reproduce')).resolve()
BACKUP = SCRIPT / 'before-source'
RELATIVE = [
    'crates/tilt-ui-runtime/src/style/plugin.rs',
    'crates/tilt-ui-runtime/src/style/matcher.rs',
]
FROZEN = SCRIPT / 'configs'


def digest(path):
    with path.open('rb') as handle:
        return hashlib.file_digest(handle, 'sha256').hexdigest()


def write_json(path, value):
    path.write_text(json.dumps(value, indent=2) + '\n')


def source_hashes():
    return {name: digest(UPSTREAM / name) for name in RELATIVE}


def game_fingerprint():
    paths = {PROJECT / 'Cargo.toml', PROJECT / 'Cargo.lock'}
    for directory in ['apps', 'crates']:
        paths.update((PROJECT / directory).rglob('*.rs'))
        paths.update((PROJECT / directory).rglob('Cargo.toml'))
    paths.update(path for path in (PROJECT / 'src-ui').rglob('*')
                 if path.is_file() and path.suffix in {'.rs', '.html', '.css'})
    return {str(path.relative_to(PROJECT)): digest(path) for path in sorted(paths)}


def prepare():
    ARTIFACT.mkdir(parents=True, exist_ok=True)
    manifest = ARTIFACT / 'source-swap.json'
    if manifest.exists():
        return json.loads(manifest.read_text())
    records = {}
    for phase, source in [('before', BACKUP), ('after', UPSTREAM)]:
        values = {}
        for name in RELATIVE:
            target = ARTIFACT / f'{phase}-source' / name
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_bytes((source / name).read_bytes())
            values[name] = digest(target)
        records[phase] = values
    records['external_root'] = str(UPSTREAM)
    write_json(manifest, records)
    for phase in ['before', 'after']:
        case = ARTIFACT / phase
        config = case / 'configs'
        config.mkdir(parents=True, exist_ok=True)
        for file in FROZEN.glob('*.toml'):
            shutil.copy2(file, config / file.name)
    print('Prepared exact source snapshots and identical frozen Creative chest configs.', flush=True)
    return records


def swap(phase, expected, records):
    actual = source_hashes()
    if actual != records[expected]:
        raise RuntimeError(f'Unexpected external source before {phase}: {actual}')
    for name in RELATIVE:
        target = UPSTREAM / name
        staged = target.with_name(target.name + '.o09-swap')
        staged.write_bytes((ARTIFACT / f'{phase}-source' / name).read_bytes())
        staged.chmod(target.stat().st_mode)
        staged.replace(target)
    if source_hashes() != records[phase]:
        raise RuntimeError('Source restoration hash mismatch')
    print(f'Source switched to {phase}, exact SHA256s verified.', flush=True)


def build(phase):
    log = ARTIFACT / f'{phase}-build.log'
    print(f'Building {phase}; log={log}', flush=True)
    start = time.monotonic()
    with log.open('w') as output:
        subprocess.run(['cargo', 'build', '-p', 'terra-client'], cwd=PROJECT,
                       stdout=output, stderr=subprocess.STDOUT, check=True)
    destination = ARTIFACT / f'terra-client-{phase}'
    if destination.exists():
        raise RuntimeError(f'Immutable binary already exists: {destination}')
    shutil.copy2(PROJECT / 'target/debug/terra-client', destination)
    destination.chmod(0o555)
    result = {'path': str(destination), 'sha256': digest(destination),
              'bytes': destination.stat().st_size, 'build_seconds': time.monotonic() - start}
    write_json(ARTIFACT / f'{phase}-binary.json', result)
    print(f'Copied immutable {phase} binary, sha256={result["sha256"]}', flush=True)


def build_pair():
    records = prepare()
    game_sources = game_fingerprint()
    write_json(ARTIFACT / 'game-source-hashes.json', game_sources)
    if source_hashes() != records['after']:
        raise RuntimeError('Indexed source must be current before the exclusive build pair')
    try:
        swap('before', 'after', records)
        build('before')
        if game_fingerprint() != game_sources:
            raise RuntimeError('Game source changed during the exclusive baseline build')
    finally:
        # Always restore implemented optimizer even if the baseline build fails.
        if source_hashes() == records['before']:
            swap('after', 'before', records)
        elif source_hashes() != records['after']:
            raise RuntimeError('Concurrent external edits detected; refusing to overwrite them')
    build('after')
    assert source_hashes() == records['after']
    assert game_fingerprint() == game_sources, 'Game source changed inside the build pair'


def ui_fingerprint():
    return {str(path.relative_to(PROJECT)): digest(path)
            for path in sorted((PROJECT / 'src-ui').rglob('*'))
            if path.is_file()}


def native_pair():
    fingerprint = ui_fingerprint()
    write_json(ARTIFACT / 'native-ui-source-hashes.json', fingerprint)
    for phase in ['before', 'after']:
        case = ARTIFACT / phase
        if (case / 'native.log').exists():
            raise RuntimeError('Refusing to overwrite native evidence or reuse a warmed case')
        binary = ARTIFACT / f'terra-client-{phase}'
        saved = json.loads((ARTIFACT / f'{phase}-binary.json').read_text())
        if digest(binary) != saved['sha256']:
            raise RuntimeError(f'Immutable {phase} binary was modified')
        if ui_fingerprint() != fingerprint:
            raise RuntimeError('Live UI source changed inside the native comparison window')
        config_hashes = {p.name: digest(p) for p in sorted((case / 'configs').glob('*.toml'))}
        write_json(case / 'initial-config-hashes.json', config_hashes)
        # Native smoke flags from a preceding gallery must not change this case.
        env = {key: value for key, value in os.environ.items()
               if not key.startswith(('TERRA_', 'WGPU_'))}
        env.update({
            'TERRA_CONFIG_DIR': str(case / 'configs'),
            'TERRA_MENU_CONFIG_DIR': str(case / 'menu'),
            'TERRA_PROFILE_FILE': str(case / 'profile.toml'),
            'TERRA_WORLD_DIR': str(case / 'worlds'),
            'TERRA_BENCH_MODE': 'integrated', 'TERRA_BENCH_UI': '1',
            'TERRA_BENCH_GRAPHICS': '1', 'TERRA_BENCH_INVENTORY': '1',
            'TERRA_BENCH_INVENTORY_ROUNDS': '2', 'TERRA_BENCH_INVENTORY_SURFACE': 'chest',
            'TERRA_BENCH_SECONDS': '120', 'TERRA_BENCH_LOAD_SECONDS': '180',
            'TERRA_BENCH_WEATHER': 'clear', 'TERRA_CPU_SCOPES': '1',
            'TERRA_SCOPE_TIMELINE': '1', 'WGPU_BACKEND': 'vulkan',
        })
        write_json(case / 'environment.json', {k: v for k, v in env.items()
                                             if k.startswith(('TERRA_', 'WGPU_'))})
        print(f'Running {phase} native chest comparison; log={case / "native.log"}', flush=True)
        with (case / 'native.log').open('w') as output:
            subprocess.run([sys.executable, 'tools/benchmarks/measure.py', str(binary),
                            '--smoke-test-benchmark'], cwd=PROJECT, env=env,
                           stdout=output, stderr=subprocess.STDOUT, check=True)
        if ui_fingerprint() != fingerprint:
            raise RuntimeError('Live UI source changed during a native run')
        print(f'Finished {phase} native comparison.', flush=True)
    before = json.loads((ARTIFACT / 'before/initial-config-hashes.json').read_text())
    after = json.loads((ARTIFACT / 'after/initial-config-hashes.json').read_text())
    assert before == after, 'Before/after initial graphics/world/gameplay configs differ'


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('phase', choices=['prepare', 'build-pair', 'native-pair'])
    args = parser.parse_args()
    {'prepare': prepare, 'build-pair': build_pair, 'native-pair': native_pair}[args.phase]()
