"""Validate maximum-budget GPU particle runs, separately from four-fluid quality.

The 8192-water-particle/128 m3 scene checks mass, bounds, edited colliders,
physical progress and source withdrawal. Clock/dispatch measurements are
reported without a hardware-specific FPS or real-time-speed requirement.
"""
from pathlib import Path
import hashlib
import json
import math
import re
import runpy
import sys
import tomllib

from PIL import Image, ImageStat

# Shared constants and input/image helpers; the regular 64-per-kind validation
# and its small-puddle settling limits are deliberately not called here.
common = runpy.run_path(str(Path(__file__).with_name('verify-particle-fluids.py')))
PHASES = common['PHASES']
CAPACITY = common['CAPACITY']
FLOAT_MAX = common['FLOAT_MAX']
SOLIDS = common['SOLIDS']
SIMULATION_HZ = common['SIMULATION_HZ']
CAPTURE_SECONDS = common['CAPTURE_SECONDS']
TIME_WINDOWS = common['TIME_WINDOWS']
SUCCESS = common['SUCCESS']
require = common['require']
number = common['number']
vector = common['vector']
read_json = common['read_json']
MASS = [128., 0., 0., 0.]


def verify_stress_readback(metadata, index):
    phase = PHASES[index]
    require(metadata['stress'] is True and metadata['phase'] == phase,
            'readback is not from the requested maximum-budget stress phase')
    require(metadata['gpu_ready'] is True, 'GPU pipeline was not ready')
    frame, seconds, real = (metadata[key] for key in
                            ('gpu_frame', 'simulated_seconds', 'real_elapsed_seconds'))
    require(type(frame) is int and frame > 0, 'invalid GPU dispatch frame')
    require(number(seconds) and seconds >= 0. and number(real) and real > 0.,
            'missing/non-finite GPU or wall-clock measurement')
    require(math.isclose(seconds * SIMULATION_HZ, round(seconds * SIMULATION_HZ), abs_tol=0.03),
            'GPU timestamp is not on the fixed 120 Hz integration clock')
    # A seed can inherit at most one pending frame's integration budget. Later
    # readback latency adds wall time; it cannot justify physics fast-forward.
    require(seconds <= real + 0.1,
            f'GPU physics ran ahead of real time: simulated={seconds:.6f}s, wall={real:.6f}s')
    window = TIME_WINDOWS[index]
    if window is not None:
        require(window[0] <= seconds <= window[1],
                f'{phase} was copied at {seconds:.6f}s, expected {window}')
    revision = metadata['source_revision']
    require(type(revision) is int and revision > 0, 'invalid source revision')
    positions = metadata['positions']
    require(isinstance(positions, list) and len(positions) == CAPACITY,
            f'expected all {CAPACITY} actual GPU slots')
    active = []
    for slot, particle in enumerate(positions):
        require(vector(particle, 4), f'GPU slot {slot} contains non-finite/malformed data')
        x, y, z, kind = particle
        require(kind == (0. if index < 3 else -1.),
                f'GPU slot {slot} lost water mass, changed kind or survived withdrawal')
        if index == 3:
            continue
        require(all(-0.001 <= p <= 16.001 for p in (x, y, z)),
                f'GPU slot {slot} escaped the physical domain')
        require(y >= 0.99, f'GPU slot {slot} penetrated the actual floor')
        for shape, (low, high) in enumerate(SOLIDS):
            require(not all(a + 0.001 < p < b - 0.001
                            for p, a, b in zip((x, y, z), low, high)),
                    f'GPU slot {slot} penetrated solid {shape}: {particle}')
        active.append((x, y, z))
    expected_counts = [CAPACITY, 0, 0, 0] if index < 3 else [0] * 4
    require(metadata['counts'] == expected_counts
            and all(type(value) is int for value in metadata['counts']),
            'reported counts disagree with the complete GPU buffer')
    expected_mass = MASS if index < 3 else [0.] * 4
    require(vector(metadata['mass'], 4)
            and all(abs(a - b) <= 1e-6 for a, b in zip(metadata['mass'], expected_mass)),
            'maximum-budget mass must remain 128 m3 until withdrawal')
    for field in ('centroid', 'minimum', 'maximum'):
        require(isinstance(metadata[field], list) and len(metadata[field]) == 4,
                f'{field} must describe all four kind channels')
    calculated = dict(centroid=[], minimum=[], maximum=[])
    for kind in range(4):
        points = active if kind == 0 else []
        mean = [sum(p[a] for p in points) / len(points) if points else 0. for a in range(3)]
        low = [min(p[a] for p in points) if points else FLOAT_MAX for a in range(3)]
        high = [max(p[a] for p in points) if points else -FLOAT_MAX for a in range(3)]
        for field, expected in (('centroid', mean), ('minimum', low), ('maximum', high)):
            require(vector(metadata[field][kind], 3), f'invalid {field} for kind {kind}')
            # The fixture sums 8192 coordinates in f32; validate against an
            # independent f64 sum without confusing roundoff with lost mass.
            tolerance = 0.002 if field == 'centroid' else 0.0001
            require(all(math.isclose(a, b, rel_tol=1e-6, abs_tol=tolerance)
                        for a, b in zip(metadata[field][kind], expected)),
                    f'{field} kind {kind} does not match its actual GPU positions')
            calculated[field].append(expected)
    return dict(phase=phase, gpu_frame=frame, simulated_seconds=seconds,
                real_elapsed_seconds=real, source_revision=revision,
                counts=expected_counts, inactive=CAPACITY-len(active), mass=metadata['mass'],
                **calculated), active


def verify_stress_series(stages, particles):
    errors, intervals = [], []
    try:
        for index in range(1, 4):
            before, after = stages[index-1], stages[index]
            frames = after['gpu_frame'] - before['gpu_frame']
            real = after['real_elapsed_seconds'] - before['real_elapsed_seconds']
            seconds = after['simulated_seconds'] - before['simulated_seconds']
            require(frames >= 2 and real > 0. and seconds >= -0.0001,
                    'stress readbacks reused a buffer or moved backwards in either clock')
            intervals.append(dict(from_phase=before['phase'], to_phase=after['phase'],
                                  gpu_dispatch_frames=frames, real_seconds=real,
                                  simulated_seconds=seconds,
                                  observed_dispatch_hz=frames/real,
                                  observed_simulation_speed=max(0., seconds)/real))
        times = [stage['simulated_seconds'] for stage in stages]
        require(times[1]-times[0] >= 0.45 and times[2]-times[1] >= 3.0,
                'stress capture phases did not span the requested physical simulation time')
        require(times[2]-0.0001 <= times[3] <= times[2]+0.5,
                'withdrawal preceded the settled readback or was not captured promptly')
        revision = stages[0]['source_revision']
        require(all(stage['source_revision'] == revision for stage in stages[:3])
                and stages[3]['source_revision'] == revision+1,
                'maximum-budget GPU slots were reseeded or withdrawal was not revised')
        require(stages[2]['centroid'][0][1] < stages[0]['centroid'][0][1]-0.35,
                'the maximum-budget water volume did not actually fall')
        displacement = sum(math.dist(a,b) for a,b in zip(particles[0],particles[2]))/CAPACITY
        require(displacement > 0.35, 'stress readbacks reused static seed positions')
    except ValueError as error:
        errors.append(str(error))
    return errors, intervals


def main():
    require(len(sys.argv) in (2, 3), 'usage: verify-particle-stress.py RUN_DIR [EXIT_STATUS]')
    root = Path(sys.argv[1])
    status = int(sys.argv[2]) if len(sys.argv) == 3 else int((root/'exit-status').read_text())
    errors, captures, particles = [], [], []
    if status:
        errors.append(f'Client exit {status}')
    try:
        log = re.sub(r'\x1b\[[0-9;]*m', '', (root/'native.log').read_text(errors='replace'))
        require(SUCCESS in log, 'missing native GPU gallery completion marker')
        require(not re.search(r'\bERROR\b|panicked at|validation error|shader processing error|'
                              r'devicelost|device lost|out of memory', log, re.I),
                'runtime/GPU error in stress log')
        run = read_json(root/'run.json')
        require(run['fixture'] == 'materials_smoke_particles'
                and run['mode'] == 'native_gpu_manual_stress' and run['stress'] is True
                and run['contract'] == 'maximum_particle_budget_mass_collision_clock'
                and run['readback_slots'] == CAPACITY and run['active_counts'] == [CAPACITY,0,0,0]
                and run['target_mass'] == MASS and run['world_water'] is False
                and run['phases'] == list(PHASES) and run['simulation_hz'] == SIMULATION_HZ
                and run['capture_seconds'] == list(CAPTURE_SECONDS) and run['fps_limit'] == 0,
                'incorrect maximum-budget stress manifest')
        require(run['environment'] == {'TERRA_PARTICLE_INSPECT':'1',
                                       'TERRA_PARTICLE_MANUAL_INPUT':'1','TERRA_PARTICLE_STRESS':'1'},
                'missing explicit manual GPU stress flags')
        world = tomllib.loads((root/'configs/world.toml').read_text())
        require(all(world[key] is False for key in ('water','trees','caves','rivers')),
                'generated fluid/obstacles contaminate the isolated stress fixture')
        graphics = tomllib.loads((root/'configs/graphics.toml').read_text())
        require(graphics['fps_limit'] == 0 and graphics['vsync'] is False,
                'stress capture should record uncapped rendering')
        expected = {f'pbf-{i}-{phase}.{suffix}' for i,phase in enumerate(PHASES) for suffix in ('png','json')}
        actual = {p.name for p in (root/'captures').iterdir() if p.suffix in ('.png','.json')}
        require(actual == expected, f'incorrect stress capture set: missing={expected-actual}, extra={actual-expected}')
    except (OSError, ValueError, KeyError, TypeError) as error:
        errors.append(f'run: {error}')
    for index,phase in enumerate(PHASES):
        name = f'pbf-{index}-{phase}'
        try:
            metadata = read_json(root/'captures'/f'{name}.json')
            summary, points = verify_stress_readback(metadata,index)
            path = root/'captures'/f'{name}.png'
            with Image.open(path) as image:
                require(image.format == 'PNG' and image.size == (1920,1080),
                        'stress capture must be a native 1920x1080 PNG')
                sample = image.convert('RGB').resize((192,108))
                require(len(sample.getcolors(20736) or []) > 32
                        and max(ImageStat.Stat(sample).stddev) > 8., 'blank/uniform native stress capture')
            summary.update(image_sha256=hashlib.sha256(path.read_bytes()).hexdigest(),
                           readback_sha256=hashlib.sha256((root/'captures'/f'{name}.json').read_bytes()).hexdigest())
            captures.append(summary)
            particles.append(points)
        except (OSError, ValueError, KeyError, TypeError, IndexError) as error:
            errors.append(f'{name}: {error}')
    intervals = []
    if len(captures) == 4:
        if len({capture['image_sha256'] for capture in captures}) != 4:
            errors.append('native stress phases produced identical PNGs')
        if len({capture['readback_sha256'] for capture in captures}) != 4:
            errors.append('native stress phases reused a GPU readback')
        motion_errors, intervals = verify_stress_series(captures,particles)
        errors.extend(motion_errors)
    client_hash = None
    try:
        client_hash = (root/'client.sha256').read_text().split()[0]
        require(re.fullmatch(r'[0-9a-f]{64}',client_hash) is not None, 'invalid pinned executable hash')
        binary = root/'terra-client'
        if binary.exists():
            with binary.open('rb') as stream:
                require(hashlib.file_digest(stream,'sha256').hexdigest() == client_hash,
                        'stress executable changed during capture')
    except (OSError,ValueError,IndexError) as error:
        errors.append(f'executable: {error}')
    clock = dict(intervals=intervals, hardware_fps_threshold=None,
                 note='Dispatch rates are measured between GPU readbacks; wall time includes fixture, screenshots and readback latency.')
    if len(captures) == 4:
        first,settled = captures[0],captures[2]
        real = settled['real_elapsed_seconds']-first['real_elapsed_seconds']
        if real > 0.:
            clock.update(observed_dispatch_hz=(settled['gpu_frame']-first['gpu_frame'])/real,
                         observed_simulation_speed=(settled['simulated_seconds']-first['simulated_seconds'])/real,
                         wall_to_settled_seconds=settled['real_elapsed_seconds'],
                         simulated_to_settled_seconds=settled['simulated_seconds'])
    report = dict(status='failed' if errors else 'passed', errors=errors,
                  contract='maximum_particle_budget_mass_collision_clock', captures=captures,
                  client_sha256=client_hash, active_particle_budget=CAPACITY, water_mass=128.,
                  fixed_integration_hz=SIMULATION_HZ, clock_performance=clock,
                  scope='8192 actual GPU water particles/128 m3: finite positions, edited colliders, mass, '
                        'physical motion, clocks and complete withdrawal with four native screenshots. '
                        'This maximum-budget stress contract is separate from the four-kind small-puddle settling gallery.')
    (root/'verification.json').write_text(json.dumps(report,indent=2)+'\n')
    print(f"GPU particle stress {report['status']}: {root/'verification.json'}")
    for error in errors:
        print(error,file=sys.stderr)
    return bool(errors)


if __name__ == '__main__':
    try:
        raise SystemExit(main())
    except (OSError,ValueError) as error:
        print(f'GPU particle stress verification failed: {error}',file=sys.stderr)
        raise SystemExit(1)
