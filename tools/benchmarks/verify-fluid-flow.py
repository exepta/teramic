"""Check real GPU silhouettes between fixed authoritative fluid states."""
from pathlib import Path
import hashlib
import json
import re
import sys
from PIL import Image

root = Path(sys.argv[1])
errors, captures, series = [], [], {}
if int(sys.argv[2]):
    errors.append('Client exit ' + sys.argv[2])
log = re.sub(r'\x1b\[[0-9;]*m', '', (root / 'native.log').read_text(errors='replace'))
if 'Temporal fluid gallery passed:' not in log:
    errors.append('Missing temporal gallery success marker')
if re.search(r'\bERROR\b|panicked at|Validation Error|Shader processing error|DeviceLost', log):
    errors.append('Runtime/GPU error in native log')
for fluid, big in [('water', 7), ('galle', 56), ('tar', 48)]:
    counts = []
    for stage in range(5):
        name = f'flow-{fluid}-{stage}'
        try:
            metadata = json.loads((root / 'captures' / f'{name}.json').read_text())
            frame = Image.open(root / 'captures' / f'{name}.png').convert('RGB')
            assert frame.size == (1920, 1080)
            assert metadata['roi_units'] == 'physical_pixels'
            assert metadata['replica']
            assert metadata['authoritative_units'] == [1, big, big, 0, 0][stage]
            if stage in [1, 3]:
                assert .3 <= metadata['tween_fraction'] <= .95
            if stage in [2, 4]:
                assert metadata['tween_fraction'] >= 1.1
            pixels = frame.crop(metadata['roi']).getdata()
            if fluid == 'water':
                # A draining film is transparent: its faint green/blue tint
                # still separates it from the neutral/brown stone backdrop.
                count = sum(g > r * 1.04 and b > r * 1.04 and g > 35 for r, g, b in pixels)
            elif fluid == 'galle':
                count = sum(r > b * 1.6 and g > b * 1.8 and g > r * .8 for r, g, b in pixels)
            else:
                count = sum(max(r, g, b) < 45 for r, g, b in pixels)
            counts.append(count)
            data = (root / 'captures' / f'{name}.png').read_bytes()
            captures.append(dict(name=name, **metadata, mask_pixels=count,
                                 sha256=hashlib.sha256(data).hexdigest()))
        except (OSError, ValueError, AssertionError, KeyError) as error:
            errors.append(f'{name}: {type(error).__name__}: {error}')
    if len(counts) == 5:
        low, middle, full, draining, empty = counts
        fill, drain = full - low, full - empty
        if not (fill > 60 and low + fill * .08 < middle < full - fill * .08):
            errors.append(f'{fluid}: silhouette did not advance continuously: {counts}')
        if not (drain > 60 and empty + drain * .05 < draining < full - drain * .05):
            errors.append(f'{fluid}: partial-cell disappearance was abrupt: {counts}')
    series[fluid] = counts
report = dict(status='failed' if errors else 'passed', errors=errors,
              captures=captures, series=series,
              client_sha256=hashlib.file_digest((root / 'terra-client').open('rb'), 'sha256').hexdigest(),
              scope='Actual GPU fill/drain between fixed authoritative targets; target disappears '
                    'while source remains in section. Diagnostic 0.9s tween uses production shaders.')
(root / 'verification.json').write_text(json.dumps(report, indent=2) + '\n')
print(f"Temporal fluid {report['status']}: {root / 'verification.json'}")
for error in errors:
    print(error, file=sys.stderr)
raise SystemExit(bool(errors))
