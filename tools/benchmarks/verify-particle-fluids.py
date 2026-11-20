"""Validate four actual GPU readbacks from materials_smoke_particles.rs.

Checks particle motion and geometry independently of the fixture's summaries.
This native/manual-input gallery verifies GPU presentation, gravity, collision
and withdrawal; authority networking/gameplay is tested separately.
"""
from pathlib import Path
import hashlib
import json
import math
import re
import sys
import tomllib

from PIL import Image, ImageStat

PHASES = ('airborne', 'impact', 'settled', 'drained')
KINDS = ('water', 'blood', 'galle', 'tar')
CAPACITY = 8192
FLOAT_MAX = 3.4028234663852886e38
# Exact domain-relative shapes authored by this fixture (floor, slab, wall, path).
SOLIDS = (
    ((0., 0., 0.), (16., 1., 16.)),
    ((5., 1., 3.), (6., 1.5, 4.)),
    ((13., 1., 1.), (14., 4., 7.)),
    ((9., 1., 9.), (15., 1.9375, 15.)),
)
MASS = (1., 1., 1., 1. / 16.)
SIMULATION_HZ = 120
CAPTURE_SECONDS = (0.07, 0.75, 4.0, None)
# Two diagnostic dispatch frames follow each requested physical timestamp.
TIME_WINDOWS = ((0.04, 0.18), (0.65, 0.95), (4.0, 4.5), None)
SETTLED_CENTROID = (1.0, 1.5)
SETTLED_MAXIMUM_Y = 2.0
SUCCESS = 'GPU particle fluid gallery passed: four fluids, real gravity/collision, stable mass and complete source withdrawal'


def require(condition, message):
    if not condition:
        raise ValueError(message)


def number(value):
    return type(value) in (int, float) and math.isfinite(value)


def vector(value, count):
    return isinstance(value, list) and len(value) == count and all(map(number, value))


def close(actual, expected, tolerance=0.0001):
    return abs(actual - expected) <= tolerance


def read_json(path):
    def invalid(value):
        raise ValueError(f'non-finite JSON constant {value}')
    return json.loads(path.read_text(), parse_constant=invalid)


def verify_readback(metadata, index):
    name = PHASES[index]
    require(metadata.get('stress', False) is False, 'stress readbacks need the separate stress verifier')
    require(metadata['phase'] == name, 'incorrect phase label')
    require(metadata['gpu_ready'] is True, 'GPU pipeline was not ready')
    frame = metadata['gpu_frame']
    revision = metadata['source_revision']
    require(type(frame) is int and frame > 0, 'invalid GPU dispatch frame')
    seconds = metadata['simulated_seconds']
    require(number(seconds) and seconds >= 0., 'missing/invalid integrated GPU simulation time')
    require(math.isclose(seconds * SIMULATION_HZ, round(seconds * SIMULATION_HZ), abs_tol=0.03),
            'simulation timestamp is not on the fixed 120 Hz integration clock')
    window = TIME_WINDOWS[index]
    if window is not None:
        require(window[0] <= seconds <= window[1],
                f'{name} readback is not at its physical capture time: {seconds:.6f}s, expected {window}')
    require(type(revision) is int and revision > 0, 'invalid source revision')
    positions = metadata['positions']
    require(isinstance(positions, list) and len(positions) == CAPACITY,
            f'expected all {CAPACITY} GPU slots, including inactive slots')
    groups = [[] for _ in KINDS]
    inactive = 0
    for slot, particle in enumerate(positions):
        require(vector(particle, 4), f'non-finite or malformed GPU slot {slot}')
        x, y, z, kind = particle
        require(kind in (-1., 0., 1., 2., 3.), f'invalid GPU kind in slot {slot}')
        if kind == -1:
            inactive += 1
            continue
        kind = int(kind)
        require(index < 3 and slot < 256 and kind == slot // 64,
                f'GPU slot identity changed: slot={slot}, kind={kind}')
        require(all(-0.001 <= value <= 16.001 for value in (x, y, z)),
                f'{KINDS[kind]} escaped the 16 m domain at slot {slot}')
        require(y >= 0.99, f'{KINDS[kind]} penetrated the floor at slot {slot}')
        for shape, (low, high) in enumerate(SOLIDS):
            require(not all(a + 0.001 < p < b - 0.001
                            for p, a, b in zip((x, y, z), low, high)),
                    f'{KINDS[kind]} penetrated solid {shape} at slot {slot}: {particle}')
        if kind == 2:
            require(abs(math.floor(x) - 3) + abs(math.floor(z) - 10) <= 4,
                    f'Galle exceeded its four-cell source range at slot {slot}')
        if kind == 3:
            require(1.936 <= y <= 2.002 and 9. <= x <= 15.001 and 9. <= z <= 15.001,
                    f'tar left the thin prepared path at slot {slot}')
            require(abs(math.floor(x) - 10) + abs(math.floor(z) - 10) <= 4,
                    f'tar exceeded its four-cell source range at slot {slot}')
        groups[kind].append((x, y, z))
    counts = [len(group) for group in groups]
    expected_counts = [64] * 4 if index < 3 else [0] * 4
    require(counts == expected_counts, f'actual GPU counts {counts}, expected {expected_counts}')
    require(inactive == CAPACITY - sum(counts), 'incorrect inactive GPU slot count')
    require(isinstance(metadata['counts'], list)
            and all(type(count) is int for count in metadata['counts'])
            and metadata['counts'] == counts, 'reported counts disagree with GPU slots')
    require(vector(metadata['mass'], 4), 'invalid authority mass')
    expected_mass = MASS if index < 3 else (0.,) * 4
    require(all(close(a, b, 1e-6) for a, b in zip(metadata['mass'], expected_mass)),
            'authority mass does not match the fixture/source withdrawal')
    centroids, minima, maxima = [], [], []
    for field in ('centroid', 'minimum', 'maximum'):
        require(isinstance(metadata[field], list) and len(metadata[field]) == 4,
                f'{field} must describe all four fluid kinds')
    for kind, group in enumerate(groups):
        require(vector(metadata['centroid'][kind], 3)
                and vector(metadata['minimum'][kind], 3)
                and vector(metadata['maximum'][kind], 3), 'invalid GPU summaries')
        centroid = [sum(p[axis] for p in group) / len(group) if group else 0.
                    for axis in range(3)]
        minimum = [min(p[axis] for p in group) if group else FLOAT_MAX for axis in range(3)]
        maximum = [max(p[axis] for p in group) if group else -FLOAT_MAX for axis in range(3)]
        require(all(close(a, b) for a, b in zip(metadata['centroid'][kind], centroid)),
                f'{KINDS[kind]} centroid does not match its GPU positions')
        for field, expected in (('minimum', minimum), ('maximum', maximum)):
            require(all(math.isclose(a, b, rel_tol=1e-6, abs_tol=0.0001)
                        for a, b in zip(metadata[field][kind], expected)),
                    f'{KINDS[kind]} {field} does not match its GPU positions')
        centroids.append(centroid)
        minima.append(minimum)
        maxima.append(maximum)
    return dict(phase=name, gpu_frame=frame, simulated_seconds=seconds, source_revision=revision,
                counts=counts, inactive=inactive, centroid=centroids,
                minimum=minima, maximum=maxima, mass=metadata['mass']), groups



def verify_physical_series(stages, groups):
    """Require actual floor settling, not merely a small downward displacement."""
    errors = []
    try:
        for index in range(1, 4):
            require(stages[index]['gpu_frame'] - stages[index-1]['gpu_frame'] >= 2,
                    'readback dispatch frame did not advance after its two-frame request')
        times = [stage['simulated_seconds'] for stage in stages]
        require(times[1] - times[0] >= 0.45 and times[2] - times[1] >= 3.0,
                'impact/settling readbacks did not span the required physical time')
        # With no remaining active particles, the integration clock may stop;
        # dispatch/revision/counts still prove that withdrawal reached the GPU.
        require(times[2] - 0.0001 <= times[3] <= times[2] + 0.5,
                'source withdrawal timestamp precedes settling or was delayed')
        revision = stages[0]['source_revision']
        require(all(stage['source_revision'] == revision for stage in stages[:3])
                and stages[3]['source_revision'] == revision + 1,
                'GPU slots were reseeded between motion phases or withdrawal was not revised')
    except ValueError as error:
        errors.append(str(error))
    for kind in range(3):
        try:
            airborne = stages[0]['centroid'][kind][1]
            impact = stages[1]['centroid'][kind][1]
            settled = stages[2]['centroid'][kind][1]
            maximum = stages[2]['maximum'][kind][1]
            require(4.0 <= airborne <= 5.0 and impact <= airborne - 0.5,
                    f'{KINDS[kind]} did not fall before impact: {airborne}, {impact}')
            require(SETTLED_CENTROID[0] - 0.0001 <= settled <= SETTLED_CENTROID[1] + 0.0001,
                    f'{KINDS[kind]} is still airborne/piled up after four seconds: centroidY={settled:.6f}')
            require(maximum <= SETTLED_MAXIMUM_Y + 0.0001,
                    f'{KINDS[kind]} still has airborne jets after four seconds: maximumY={maximum:.6f}')
            require(airborne - settled >= 2.5,
                    f'{KINDS[kind]} did not complete its fall to the actual floor')
            moved = [math.dist(a, b) for a, b in zip(groups[0][kind], groups[2][kind])]
            fallen = sum(a[1] - b[1] >= 2.0 for a, b in zip(groups[0][kind], groups[2][kind]))
            require(sum(moved) / 64. >= 2.5 and fallen >= 56,
                    f'{KINDS[kind]} particles did not physically settle: mean movement={sum(moved)/64.:.6f}, fallen={fallen}/64')
        except ValueError as error:
            errors.append(str(error))
    return errors


def main():
    require(len(sys.argv) in (2, 3), 'usage: verify-particle-fluids.py RUN_DIR [EXIT_STATUS]')
    root = Path(sys.argv[1])
    errors, captures, stages, groups = [], [], [], []
    status = int(sys.argv[2]) if len(sys.argv) == 3 else int((root / 'exit-status').read_text())
    if status:
        errors.append(f'Client exit {status}')
    try:
        log = re.sub(r'\x1b\[[0-9;]*m', '', (root / 'native.log').read_text(errors='replace'))
        require(SUCCESS in log, 'missing GPU gallery completion marker')
        require(not re.search(r'\bERROR\b|panicked at|validation error|shader processing error|'
                              r'devicelost|device lost|out of memory', log, re.I),
                'runtime/GPU failure in native log')
        run = read_json(root / 'run.json')
        require(run['fixture'] == 'materials_smoke_particles'
                and run['mode'] == 'native_gpu_manual_input'
                and run.get('stress', False) is False
                and run['readback_slots'] == CAPACITY
                and run['phases'] == list(PHASES)
                and run['simulation_hz'] == SIMULATION_HZ
                and run['capture_seconds'] == list(CAPTURE_SECONDS)
                and run['settled_limits'] == {'centroid_y': list(SETTLED_CENTROID), 'maximum_y': SETTLED_MAXIMUM_Y}
                and run['world_water'] is False, 'incorrect isolated fixture manifest')
        require(run['environment'] == {'TERRA_PARTICLE_INSPECT': '1',
                                       'TERRA_PARTICLE_MANUAL_INPUT': '1'},
                'manual GPU inspection flags are missing')
        world = tomllib.loads((root / 'configs/world.toml').read_text())
        require(world['water'] is False and world['trees'] is False
                and world['caves'] is False and world['rivers'] is False,
                'world water or generated obstacles contaminate the manual fixture')
        expected = {f'pbf-{i}-{phase}.{suffix}' for i, phase in enumerate(PHASES)
                    for suffix in ('png', 'json')}
        actual = {p.name for p in (root / 'captures').iterdir() if p.suffix in ('.png', '.json')}
        require(actual == expected, f'incorrect capture set: missing={expected-actual}, extra={actual-expected}')
    except (OSError, ValueError, KeyError, TypeError) as error:
        errors.append(f'run: {error}')
    for index, phase in enumerate(PHASES):
        name = f'pbf-{index}-{phase}'
        try:
            metadata = read_json(root / 'captures' / f'{name}.json')
            summary, actual_groups = verify_readback(metadata, index)
            path = root / 'captures' / f'{name}.png'
            with Image.open(path) as frame:
                require(frame.format == 'PNG' and frame.size == (1920, 1080),
                        'expected native 1920x1080 PNG')
                rgb = frame.convert('RGB')
                sample = rgb.resize((192, 108))
                require(len(sample.getcolors(20736) or []) > 32
                        and max(ImageStat.Stat(sample).stddev) > 8., 'blank/uniform native screenshot')
            summary.update(name=name, image_sha256=hashlib.sha256(path.read_bytes()).hexdigest(),
                           readback_sha256=hashlib.sha256((root / 'captures' / f'{name}.json').read_bytes()).hexdigest())
            captures.append(summary)
            stages.append(summary)
            groups.append(actual_groups)
        except (OSError, ValueError, KeyError, TypeError, IndexError) as error:
            errors.append(f'{name}: {error}')
    if len(stages) == 4:
        try:
            require(len({s['image_sha256'] for s in stages}) == 4, 'native phases produced identical images')
            require(len({s['readback_sha256'] for s in stages}) == 4, 'readbacks were reused between phases')
        except ValueError as error:
            errors.append(str(error))
        errors.extend(verify_physical_series(stages, groups))
    client_hash = None
    try:
        client_hash = (root / 'client.sha256').read_text().split()[0]
        require(re.fullmatch(r'[0-9a-f]{64}', client_hash) is not None, 'invalid client hash manifest')
        binary = root / 'terra-client'
        if binary.exists():
            with binary.open('rb') as stream:
                require(hashlib.file_digest(stream, 'sha256').hexdigest() == client_hash,
                        'client executable changed during the native run')
    except (OSError, ValueError, IndexError) as error:
        errors.append(f'executable: {error}')
    report = dict(status='failed' if errors else 'passed', errors=errors, captures=captures,
                  client_sha256=client_hash, gpu_readback_slots=CAPACITY,
                  simulation_hz=SIMULATION_HZ, capture_seconds=CAPTURE_SECONDS,
                  settled_limits=dict(centroid_y=SETTLED_CENTROID, maximum_y=SETTLED_MAXIMUM_Y),
                  scope='Four actual GPU readbacks and native screenshots: gravity, collision, '
                        'four-second floor settling without airborne jets, stable per-fluid mass, prepared tar and complete source withdrawal. '
                        'Manual scene inputs; authority/gameplay are validated separately.')
    (root / 'verification.json').write_text(json.dumps(report, indent=2) + '\n')
    print(f"GPU particle fluid {report['status']}: {root / 'verification.json'}")
    for error in errors:
        print(error, file=sys.stderr)
    return bool(errors)


if __name__ == '__main__':
    try:
        raise SystemExit(main())
    except (OSError, ValueError) as error:
        print(f'GPU particle verification failed: {error}', file=sys.stderr)
        raise SystemExit(1)
