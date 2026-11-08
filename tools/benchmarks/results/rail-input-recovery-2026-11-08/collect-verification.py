#!/usr/bin/env python3
"""Collect completed CPU-only evidence for rail placement and input recovery."""
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
    log = (REPORT / 'logs/affected-tests.log').read_text()
    suites = re.findall(r'test result: (\w+)\. (\d+) passed; (\d+) failed; (\d+) ignored;', log)
    assert len(suites) == 4 and all(row[0] == 'ok' and row[2] == '0' for row in suites)
    assert 'AdapterInfo {' not in log and 'Creating new window' not in log
    totals = dict(zip(['passed', 'failed', 'ignored'],
                      [sum(int(row[index]) for row in suites) for index in [1, 2, 3]]))
    css_rlib = max((ROOT / 'target/debug/deps').glob('libtilt_ui_css-*.rlib'), key=lambda path: path.stat().st_mtime)
    commands = [
        ('clippy', 'cargo clippy --workspace --all-targets --locked --keep-going -j1 -- -D warnings'),
        ('build', 'cargo build --workspace --locked -j1'),
        ('format', 'cargo fmt --all -- --check'),
        ('diff', ['git diff --check', 'git diff --cached --check']),
        ('css-parser', {
            'compile_argv': ['rustc', '--edition=2024', 'tools/ui/check_styles.rs', '--extern',
                             f'tilt_ui_css={css_rlib.relative_to(ROOT)}', '-L', 'dependency=target/debug/deps',
                             '-o', 'target/ui-stylecheck211/check-styles'],
            'run_argv': ['target/ui-stylecheck211/check-styles', 'src-ui'],
        }),
    ]
    checks = []
    for name, command in commands:
        path = REPORT / 'logs' / (name + '.log')
        content = path.read_text()
        if name in ['clippy', 'build']:
            assert 'Finished `dev` profile' in content and '\nerror:' not in content
        elif name == 'css-parser':
            assert 'Validated 61 shipped TiltUI stylesheets without a window or GPU.' in content
        else:
            assert not content
        checks.append({'command': command, 'exit_code': 0, 'log': str(path.relative_to(REPORT))})
    sources = {REPORT / 'README.md', Path(__file__).resolve()}
    for directory in ['crates', 'apps', 'src-ui', 'tools', 'tests']:
        sources.update((ROOT / directory).rglob('*.rs'))
    for pattern in ['src-ui/**/*.css', 'src-ui/**/*.html', 'Cargo.toml', 'Cargo.lock',
                    'crates/*/Cargo.toml', 'apps/*/Cargo.toml', 'README.md', 'PATCH.md', 'TODO.md']:
        sources.update(path for path in ROOT.glob(pattern) if path.is_file())
    movement = re.search(r'movement_long_run simulated_client_seconds=.*', log)
    assert movement, 'The complete long-running UDP regression must report its observed metrics.'
    data = {
        'phase': 211,
        'recorded_utc': datetime.datetime.now(datetime.timezone.utc).isoformat(),
        'graphical_tests': {'run': False, 'reason': 'User forbids graphical tests while playing.'},
        'process_policy': {'DISPLAY': None, 'WAYLAND_DISPLAY': None, 'nice': 19,
                           'ionice_class': 2, 'ionice_priority': 7, 'cargo_jobs': 1, 'test_threads': 4},
        'contracts': {
            'protocol': 116, 'region': 'RG13', 'gameplay_index': 'ID13', 'player_format': 'PLY7',
            'prediction_lead_seconds': 0.3, 'prediction_lead_min_steps': 8,
            'prediction_lead_max_steps': 64, 'server_input_abuse_cap': 128,
            'no_progress_watchdog_seconds': 30, 'server_input_steps_per_fixed_tick': 1,
            'rail_build_reach': 32, 'rail_builder_body_exempt': True,
            'other_player_body_protection': True, 'rail_recovery_max_lift': 1.02,
            'css_stylesheets': 61, 'global_physics_schedule_changed': False,
        },
        'affected_tests': {
            'command': 'cargo test -p terra-network -p terra-physics -p terra-player -p terra-ui --lib --features terra-network/client,terra-network/server --locked --no-fail-fast -j1 -- --test-threads=4 --nocapture',
            'exit_code': 0, 'totals': totals, 'suite_count': len(suites),
            'log': 'logs/affected-tests.log',
            'long_udp_observation': movement.group(),
        },
        'checks': checks,
        'source_sha256': {str(path.relative_to(ROOT)): sha(path) for path in sorted(sources)},
        'log_sha256': {f'logs/{name}.log': sha(REPORT / 'logs' / f'{name}.log')
                       for name in ['affected-tests'] + [name for name, _ in commands]},
        'notes': [
            'No game window, renderer, screenshot or GPU test was started.',
            'All shipped CSS uses the same strict parser as the runtime stylesheet asset loader.',
            'Client prediction waits at a time-scaled bounded lead; only real ACKs retire simulated commands.',
            'Watchdog liveness is evaluated after inbound snapshots; no extra server ticks or discarded unacknowledged movement hide the debt.',
            'Current collider jobs publish before catch-up fixed steps with explicit spatial pose and source-epoch/content checks.',
            'Only the actual builder entity and its local replica are exempt from rail placement body checks.',
            'Deep initial rail penetration receives a bounded stationary support correction with solid/headroom and actual mesh-overlap guards.',
            'Existing save, wire, material, recipe and nonrail placement contracts remain unchanged.',
        ],
    }
    (REPORT / 'verification.json').write_text(json.dumps(data, indent=2) + '\n')
    print(json.dumps({'tests': totals, 'graphical_tests': False, 'source_files': len(sources)}))


if __name__ == '__main__':
    main()
