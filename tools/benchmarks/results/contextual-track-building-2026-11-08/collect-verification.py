#!/usr/bin/env python3
"""Collect completed CPU-only contextual track acceptance evidence."""
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
    regressions = [line for line in log.splitlines()
                   if line.startswith('test ') and ' ... ok' in line
                   and any(word in line for word in ['rail', 'ballast', 'switch', 'curve', 'hotbar', 'wheel'])]
    checks = []
    commands = [
        ('clippy', 'cargo clippy --workspace --all-targets --locked --keep-going -j1 -- -D warnings'),
        ('build', 'cargo build --workspace --locked -j1'),
        ('format', 'cargo fmt --all -- --check'),
        ('diff', 'git diff --check'),
        ('asset-export', 'python3 tools/assets/build_rails.py --export-core'),
    ]
    for name, command in commands:
        path = REPORT / 'logs' / (name + '.log')
        content = path.read_text()
        if name in ['clippy', 'build']:
            assert 'Finished `dev` profile' in content and '\nerror' not in content
        elif name == 'asset-export':
            assert 'Generated ten complete rail assemblies from Core profile' in content
        else:
            assert not content
        checks.append({'command': command, 'exit_code': 0, 'log': str(path.relative_to(REPORT))})
    sources = {REPORT / 'README.md', Path(__file__).resolve()}
    for directory in ['crates', 'apps', 'src-ui', 'tools', 'tests']:
        sources.update((ROOT / directory).rglob('*.rs'))
    for pattern in [
        'assets/terra/blocks/rail_*.json', 'assets/terra/blocks/ballast*.json',
        'assets/terra/blocks/models/rail_*', 'assets/terra/items/*_track.json',
        'assets/terra/items/*_rail_switch_*.json', 'assets/crafting/rail_*.json',
        'assets/crafting/ballast*.json', 'tools/assets/build_rails.py',
        'tools/assets/build_switch_stand.py', 'tools/assets/build_castle.py',
        'Cargo.toml', 'Cargo.lock', 'crates/*/Cargo.toml', 'apps/*/Cargo.toml',
        'src-ui/**/*.html', 'src-ui/**/*.css', 'TODO.md', 'PATCH.md',
        'docs/references/railway-*.png',
        'docs/references/track-logic-ramps-and-curves.png',
    ]:
        sources.update(path for path in ROOT.glob(pattern) if path.is_file())
    manifest = json.loads((ROOT / 'assets/terra/blocks/models/rail_profiles.json').read_text())
    digest = hashlib.sha256()
    for relative in ['crates/terra-core/src/rails.rs', 'crates/terra-core/src/rails/curve.rs',
                     'crates/terra-core/src/rails/geometry.rs', 'crates/terra-core/src/rails/surfaces.rs']:
        digest.update(relative.encode() + b'\0' + (ROOT / relative).read_bytes())
    assert digest.hexdigest() == manifest['core_source_sha256']
    data = {
        'phase': 207, 'recorded_utc': datetime.datetime.now(datetime.timezone.utc).isoformat(),
        'graphical_tests': {'run': False, 'reason': 'User forbids graphical tests while playing.'},
        'process_policy': {'DISPLAY': None, 'WAYLAND_DISPLAY': None, 'nice': 19,
                           'ionice_class': 2, 'ionice_priority': 7, 'cargo_jobs': 1},
        'contracts': {
            'protocol': 114, 'region': 'RG13', 'gameplay_index': 'ID12', 'player_format': 'PLY7',
            'base_track': 196, 'switch_items': [202, 203], 'switch_item_cost': 1,
            'switch_stand': 209, 'ballast_blocks': [206, 207, 208], 'legacy_gravel': 42,
            'ballast_speed_multiplier': 1.5, 'recipe_count': 331, 'extra_item_count': 175,
            'wood_variants': ['oak', 'birch', 'spruce', 'cherry'],
            'minimum_curve_radius': 2.5, 'maximum_curve_length': 32,
            'maximum_grade': 0.5, 'max_flat_handle_pairs': 36, 'max_slope_handle_pairs': 49,
            'ramp_replacement_length': 4, 'ramp_replacement_step': 1,
            'context_choices': ['straight', 'curve'], 'intent_fingerprint': 'FNV64',
            'max_source_sections': 8, 'local_port_lookup_sections': 75,
            'support_mining_allowed': True, 'legacy_switch_cells': 38,
            'rail_bed_speed_api': True, 'driving_trains_implemented': False,
        },
        'workspace_tests': {
            'command': 'cargo test --workspace --lib --bins --locked --no-fail-fast -j1 -- --test-threads=4',
            'exit_code': 0, 'totals': totals, 'suite_count': len(suites),
            'log': 'logs/workspace-tests.log', 'rail_regressions': regressions,
        },
        'checks': checks,
        'source_sha256': {str(path.relative_to(ROOT)): sha(path) for path in sorted(sources)},
        'log_sha256': {f'logs/{name}.log': sha(REPORT / 'logs' / f'{name}.log')
                       for name in ['workspace-tests'] + [name for name, _ in commands]},
        'notes': [
            'No game window, renderer, screenshot or GPU test was started.',
            'A fresh endpoint docking context owns the wheel only for two actual distinct proposals.',
            'Ordinary wheel navigation updates item, slot, palette and targeting before interaction.',
            'Shift plus Place remembers a manual connection start, then cancels an existing selection.',
            'A literal zero-gap one-block step atomically rewrites the last plain four-metre straight as a smooth cubic.',
            'Stable intent fingerprints prevent silently changing source ports or control points during server recomputation.',
            'Ramp inventory consumes only positive additional cost and never refunds the replaced module.',
            'Actual UDP clients exercise context construction, survival costs, replicas, late join and stale-intent rejection.',
            'Mining bedding keeps existing rail ownership, fluid metadata and routing while immediately removing its ballast bonus.',
            'Actual train construction and driving remain open F63/F64/F53; speed APIs and route timing are implemented.',
        ],
    }
    (REPORT / 'verification.json').write_text(json.dumps(data, indent=2) + '\n')
    print(json.dumps({'tests': totals, 'graphical_tests': False,
                      'rail_test_lines': len(regressions), 'source_files': len(sources)}))


if __name__ == '__main__':
    main()
