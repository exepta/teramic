#!/usr/bin/env python3
"""Collect headless acceptance evidence; never starts a game or graphics device."""
from pathlib import Path
import datetime
import hashlib
import json
import re

REPORT = Path(__file__).resolve().parent
ROOT = REPORT.parents[3]


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    log = (REPORT / 'logs/workspace-tests.log').read_text()
    rows = re.findall(r'test result: (\w+)\. (\d+) passed; (\d+) failed; (\d+) ignored;', log)
    assert len(rows) == 12 and all(row[0] == 'ok' and row[2] == '0' for row in rows)
    assert 'AdapterInfo {' not in log and 'Window created' not in log
    totals = dict(zip(['passed', 'failed', 'ignored'], [sum(int(r[i]) for r in rows) for i in [1, 2, 3]]))
    checks = []
    commands = [
        ('workspace-clippy', 'cargo clippy --workspace --all-targets --locked -j1 -- -D warnings'),
        ('workspace-build', 'cargo build --workspace --locked -j1'),
        ('format', 'cargo fmt --all -- --check'),
        ('diff', 'git diff --check'),
        ('ui-stylesheets', 'parse all 60 actual UI stylesheets with TiltUI parser'),
    ]
    for name, command in commands:
        path = REPORT / 'logs' / (name + '.log')
        content = path.read_text()
        if name.startswith('workspace-'):
            assert 'Finished `dev` profile' in content and '\nerror:' not in content
        elif name in ['format', 'diff']:
            assert not content
        else:
            assert 'Parsed 60 real UI stylesheets successfully' in content
        checks.append({'command': command, 'exit_code': 0, 'log': str(path.relative_to(REPORT))})
    sources = set()
    for directory in ['crates', 'apps']:
        sources.update((ROOT / directory).rglob('*.rs'))
    for pattern in ['assets/terra/props/*sapling.json', 'assets/terra/blocks/apple-leaves.json',
                    'assets/terra/items/apple*.json', 'assets/food/apple*.json',
                    'assets/crafting/apple*.json', 'assets/terra/blocks/*battlement*.json',
                    'assets/terra/blocks/bergfried_wall.json', 'assets/terra/blocks/models/*battlement*',
                    'assets/terra/blocks/models/bergfried_wall.*', 'tools/assets/build_castle.py',
                    'tools/assets/castle_details.py', 'configs/world.toml',
                    'docs/references/tree-saplings.png', 'docs/references/apple-tree-items.png',
                    'docs/references/battlements-connections.png']:
        sources.update(p for p in ROOT.glob(pattern) if p.is_file())
    data = {
        'phase': 200, 'recorded_utc': datetime.datetime.now(datetime.timezone.utc).isoformat(),
        'graphical_tests': {'run': False, 'reason': 'User explicitly forbids graphical testing while playing.'},
        'process_policy': {'DISPLAY': None, 'WAYLAND_DISPLAY': None, 'nice': 10,
                           'ionice_class': 2, 'ionice_priority': 7, 'cargo_jobs': 1},
        'contracts': {'protocol': 108, 'tree_index': 'ID11', 'player_format': 'PLY7',
                      'fresh_ecology_revision': 3, 'structure_revision': 8},
        'workspace_tests': {'command': 'cargo test --workspace --lib --bins --locked --no-fail-fast -j1 -- --test-threads=4',
                            'exit_code': 0, 'totals': totals, 'suite_count': len(rows),
                            'log': 'logs/workspace-tests.log'},
        'checks': checks,
        'source_sha256': {str(p.relative_to(ROOT)): sha(p) for p in sorted(sources)},
        'log_sha256': {str(p.relative_to(REPORT)): sha(p) for p in sorted(REPORT.rglob('*.log'))},
        'notes': ['No screenshots or native render galleries were started.',
                  'Asset shape tests read actual glTF/BIN positions and compare shared collision profiles on CPU.',
                  'Saved older ecology revisions stay unchanged; planted saplings work there.',
                  'Assertions cover actual server transactions, disk roundtrips and UDP clients.'],
    }
    (REPORT / 'verification.json').write_text(json.dumps(data, indent=2) + '\n')
    print(json.dumps({'tests': totals, 'graphical_tests': False, 'source_files': len(sources)}))


if __name__ == '__main__':
    main()
