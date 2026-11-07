#!/usr/bin/env python3
"""Collect CPU-only railway acceptance evidence from completed checks."""
import datetime
import hashlib
import json
import re
from pathlib import Path

REPORT = Path(__file__).resolve().parent
ROOT = REPORT.parents[3]


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    log = (REPORT / 'logs/workspace-tests.log').read_text()
    suites = re.findall(r'test result: (\w+)\. (\d+) passed; (\d+) failed; (\d+) ignored;', log)
    assert len(suites) == 12 and all(row[0] == 'ok' and row[2] == '0' for row in suites)
    assert 'AdapterInfo {' not in log and 'Window created' not in log
    totals = dict(zip(['passed', 'failed', 'ignored'],
                      [sum(int(row[index]) for row in suites) for index in [1, 2, 3]]))
    railway_cases = [line for line in log.splitlines()
                    if line.startswith('test ') and ' ... ok' in line
                    and any(word in line for word in ['rail', 'rails::', 'route', 'frog'])]
    checks = []
    for name, command in [
        ('clippy', 'cargo clippy --workspace --all-targets --locked --keep-going -j1 -- -D warnings'),
        ('build', 'cargo build --workspace --locked -j1'),
        ('format', 'cargo fmt --all -- --check'),
        ('diff', 'git diff --check'),
    ]:
        path = REPORT / 'logs' / (name + '.log')
        content = path.read_text()
        if name in ['clippy', 'build']:
            assert 'Finished `dev` profile' in content and '\nerror' not in content
        else:
            assert not content
        checks.append({'command': command, 'exit_code': 0, 'log': str(path.relative_to(REPORT))})
    sources = set()
    sources.add(REPORT / 'README.md')
    sources.add(Path(__file__).resolve())
    for directory in ['crates', 'apps', 'src-ui', 'tools', 'tests']:
        sources.update((ROOT / directory).rglob('*.rs'))
    for pattern in ['assets/terra/blocks/rail_*.json', 'assets/terra/blocks/models/rail_*',
                    'assets/terra/items/*_track.json',
                    'assets/crafting/rail_*.json', 'tools/assets/build_rails.py',
                    'Cargo.toml', 'Cargo.lock', 'crates/*/Cargo.toml', 'apps/*/Cargo.toml',
                    'TODO.md', 'PATCH.md', 'docs/references/railway-tracks-three-blocks.png']:
        sources.update(path for path in ROOT.glob(pattern) if path.is_file())
    manifest = json.loads((ROOT / 'assets/terra/blocks/models/rail_profiles.json').read_text())
    digest = hashlib.sha256()
    for relative in ['crates/terra-core/src/rails.rs', 'crates/terra-core/src/rails/curve.rs', 'crates/terra-core/src/rails/geometry.rs',
                     'crates/terra-core/src/rails/surfaces.rs']:
        digest.update(relative.encode() + b'\0' + (ROOT / relative).read_bytes())
    assert digest.hexdigest() == manifest['core_source_sha256']
    data = {
        'phase': 205, 'recorded_utc': datetime.datetime.now(datetime.timezone.utc).isoformat(),
        'graphical_tests': {'run': False, 'reason': 'User forbids graphical tests while playing.'},
        'process_policy': {'DISPLAY': None, 'WAYLAND_DISPLAY': None, 'nice': 19,
                           'ionice_class': 2, 'ionice_priority': 7, 'cargo_jobs': 1},
        'contracts': {'protocol': 112, 'region': 'RG13', 'gameplay_index': 'ID12', 'player_format': 'PLY7',
                      'rail_materials': list(range(196, 206)), 'track_width': 3, 'gauge': 2,
                      'minimum_curve_radius': 4.5, 'maximum_curve_length': 32, 'base_item': 196, 'wood_variants': ['oak', 'birch', 'spruce', 'cherry'], 'maximum_cleared_props': 256, 'maximum_grade': 0.25,
                      'max_modules': 8192, 'max_route_visits': 32768},
        'workspace_tests': {
            'command': 'cargo test --workspace --lib --bins --locked --no-fail-fast -j1 -- --test-threads=4',
            'exit_code': 0, 'totals': totals, 'suite_count': len(suites),
            'log': 'logs/workspace-tests.log', 'railway_regressions': railway_cases,
        },
        'checks': checks,
        'source_sha256': {str(path.relative_to(ROOT)): sha(path) for path in sorted(sources)},
        'log_sha256': {
            f'logs/{name}.log': sha(REPORT / 'logs' / f'{name}.log')
            for name in ['workspace-tests', 'clippy', 'build', 'format', 'diff']
        },
        'notes': [
            'No game window, renderer, screenshot or GPU test was started.',
            'Native assets, preview, world slices and collision use shared Core geometry.',
            'Exact fixed-point ports include height and grade; crossings keep their axes separate.',
            'Stored module ownership rebuilds the graph even when chunks are unloaded.',
            'Save/reload, negative coordinates, chunk seams, length-based wood inventory transactions and real UDP clients are exercised.',
            'RG13 extends region curves while explicit four-field adapters preserve old payloads; protocol 112 updates both peers.',
            'Free cubic Bezier connections share one Track item; four wood types affect actual materials.',
            'Clearable props, their metadata and reserved drops are committed atomically with rail ownership.',
            'Train construction and driving remain separate open tasks F63/F64/F53.',
        ],
    }
    (REPORT / 'verification.json').write_text(json.dumps(data, indent=2) + '\n')
    print(json.dumps({'tests': totals, 'graphical_tests': False,
                      'railway_test_lines': len(railway_cases), 'source_files': len(sources)}))


if __name__ == '__main__':
    main()
