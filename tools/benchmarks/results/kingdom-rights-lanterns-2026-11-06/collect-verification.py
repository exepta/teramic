#!/usr/bin/env python3
"""Collect completed acceptance logs and native image hashes without modifying them."""
from pathlib import Path
import datetime
import hashlib
import json
import re
import struct

REPORT = Path(__file__).resolve().parent
ROOT = REPORT.parents[3]


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    log = (REPORT / 'logs/workspace-tests.log').read_text()
    rows = re.findall(r'test result: (\w+)\. (\d+) passed; (\d+) failed; (\d+) ignored;', log)
    assert len(rows) == 12 and all(row[0] == 'ok' and row[2] == '0' for row in rows)
    totals = dict(zip(['passed', 'failed', 'ignored'], [sum(int(r[i]) for r in rows) for i in [1, 2, 3]]))
    checks = []
    for name, command in [
        ('workspace-clippy', 'cargo clippy --workspace --all-targets --locked -j2 -- -D warnings'),
        ('workspace-build', 'cargo build --workspace --locked -j2'),
        ('format', 'cargo fmt --all -- --check'),
        ('diff', 'git diff --check'),
        ('ui-stylesheets', 'parse all 60 real UI stylesheets with TiltUI parser'),
    ]:
        path = REPORT / 'logs' / (name + '.log')
        content = path.read_text()
        if name.startswith('workspace-'):
            assert 'Finished `dev` profile' in content and '\nerror:' not in content
        elif name in ['format', 'diff']:
            assert not content
        else:
            assert 'Parsed 60 real UI stylesheets successfully' in content
        checks.append({'command': command, 'exit_code': 0, 'log': str(path.relative_to(REPORT))})
    native = []
    for name in ['lanterns', 'doors', 'claims']:
        data = json.loads((REPORT / name / 'native-metadata.json').read_text())
        assert data['status'] == 'passed' and data['exit_code'] == 0
        native.append({'case': name, 'metadata': name + '/native-metadata.json'})
    rights = json.loads((REPORT / 'rights/verification.json').read_text())
    assert rights['status'] == 'passed' and rights['exit_code'] == 0
    native.append({'case': 'rights', 'metadata': 'rights/verification.json'})
    for scene in ['kingdom-temperate', 'kingdom-winter', 'outpost']:
        path = REPORT / 'native' / scene / 'metadata.json'
        data = json.loads(path.read_text())
        if scene == 'kingdom-winter':
            assert data['status'] == 'partial' and data['not_captured'] == ['kingdom-winter-close.png']
            assert data['captured_overview']['region'] == [-10, 12]
            native.append({'case': scene, 'status': 'partial', 'metadata': str(path.relative_to(REPORT)),
                           'note': 'One real overview preserved; further graphical tests disabled by the user.'})
        else:
            assert data['exit_code'] == 0 and len(data['images']) == 2
            assert 'Native biome fixture passed:' in (path.parent / 'native.log').read_text()
            native.append({'case': scene, 'status': 'passed', 'metadata': str(path.relative_to(REPORT))})
    images = []
    for path in sorted(REPORT.rglob('*.png')):
        header = path.read_bytes()[:25]
        assert header[:8] == b'\x89PNG\r\n\x1a\n'
        width, height = struct.unpack('>II', header[16:24])
        images.append({'path': str(path.relative_to(REPORT)), 'width': width, 'height': height,
                       'bit_depth': header[24], 'bytes': path.stat().st_size, 'sha256': sha(path)})
    assert len(images) == 37
    sources = set()
    for directory in ['crates', 'apps']:
        sources.update((ROOT / directory).rglob('*.rs'))
    sources.update((ROOT / 'src-ui').rglob('*.css'))
    sources.update((ROOT / 'src-ui/components').glob('*.html'))
    for pattern in ['assets/terra/blocks/lantern.json', 'assets/terra/blocks/chain.json',
                    'assets/terra/blocks/models/lantern.*', 'assets/terra/blocks/models/chain.*',
                    'assets/crafting/lantern.json', 'assets/crafting/chain.json',
                    'tools/assets/build_lantern.py', 'configs/world.toml', 'docs/references/lantern-chain.png']:
        sources.update(ROOT.glob(pattern))
    result = {'phase': 199, 'recorded_utc': datetime.datetime.now(datetime.timezone.utc).isoformat(),
              'workspace_tests': {'command': 'cargo test --workspace --lib --bins --locked --no-fail-fast -j2 -- --test-threads=8',
                                  'exit_code': 0, 'totals': totals, 'suite_count': len(rows),
                                  'log': 'logs/workspace-tests.log'},
              'checks': checks, 'native_cases': native, 'native_images': images,
              'source_sha256': {str(p.relative_to(ROOT)): sha(p) for p in sorted(sources)},
              'log_sha256': {str(p.relative_to(REPORT)): sha(p) for p in sorted(REPORT.rglob('*.log'))},
              'notes': ['All mutable native profiles, saves and settings stay under target/.',
                        'No further graphical testing: explicitly disabled by the user while playing.',
                        'Winter overview belongs to region[-10,12]; later failed/closed attempts did not create a replacement image.',
                        'Fresh-world structure revision8; saved older revisions stay unchanged.',
                        'Protocol107, ID10 index migration, PLY7 unchanged.',
                        'Claims screenshots project temporary client data; authority is covered by actual UDP tests.',
                        'Rights gallery includes an authenticated second client and reconnects the rendered client as Visitor.']}
    (REPORT / 'verification.json').write_text(json.dumps(result, indent=2) + '\n')
    print(json.dumps({'tests': totals, 'native_images': len(images), 'native_cases': len(native)}))


if __name__ == '__main__':
    main()
