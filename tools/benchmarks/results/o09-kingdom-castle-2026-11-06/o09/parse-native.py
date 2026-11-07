#!/usr/bin/env python3
"""Parse the extended two-round inventory log, including exact CPU windows."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re

ARTIFACT = Path(os.environ.get('O09_ARTIFACT_ROOT', Path(__file__).resolve().parent)).resolve()
FIELD = re.compile(r'\b([A-Za-z_][A-Za-z_0-9]*)=(?:"([^"]*)"|([^\s]+))')
SCOPES = ('main.update', 'ui.reactivity', 'ui.layout', 'ui.style')


def value(text):
    if text.startswith('Some(') and text.endswith(')'):
        inner = text[5:-1]
        if re.fullmatch(r'[\d.eE+-]+', inner):
            return value(inner)
    if text == 'true':
        return True
    if text == 'false':
        return False
    if text == 'None':
        return None
    try:
        return float(text) if any(c in text for c in '.eE') else int(text)
    except ValueError:
        return text


def fields(line):
    return {match[1]: value(match[2] if match[2] is not None else match[3])
            for match in FIELD.finditer(line)}


def parse(path):
    text = path.read_text()
    result = {'log': str(path), 'log_sha256': hashlib.sha256(path.read_bytes()).hexdigest(),
              'phases': [], 'transitions': [], 'surface_checks': [],
              'effective_configuration_raw': [], 'process_summary': None}
    transitions = {}
    current_round = 1
    current_window = None
    completed = False
    for line_number, line in enumerate(text.splitlines(), 1):
        if line.startswith('{'):
            try:
                process = json.loads(line)
            except json.JSONDecodeError:
                continue
            if 'exit_code' in process and 'command' in process:
                result['process_summary'] = process
            continue
        data = fields(line)
        if 'round' in data and 'terra_client::inventory_benchmark' in line:
            current_round = data['round']
        if 'Effective benchmark configuration' in line:
            result['effective_configuration_raw'].append(line)
        if 'Inventory benchmark phase: CPU wall frames' in line:
            result['phases'].append({'round': current_round, 'line': line_number, **data})
        if 'Inventory benchmark surface/input validation' in line:
            result['surface_checks'].append({'line': line_number, **data})
        if 'Inventory benchmark real UI transition;' in line:
            entry = {'round': current_round, 'line': line_number, **data, 'cpu_windows': []}
            entry['request_to_native_readiness_ms'] = entry['request_to_visible_ms']
            transitions[(current_round, data['stage'])] = entry
            result['transitions'].append(entry)
        if 'Inventory transition CPU scope window ' in line:
            entry = transitions[(current_round, data['stage'])]
            current_window = {'line': line_number, **data, 'groups': {}, 'individual_scopes': []}
            entry['cpu_windows'].append(current_window)
        elif 'Inventory transition CPU scope group ' in line:
            assert current_window and current_window['stage'] == data['stage']
            assert abs(current_window['interval_ms'] - data['interval_ms']) < 1e-6
            current_window['groups'][data['scope']] = data
        elif 'Inventory transition closed CPU scope ' in line:
            assert current_window and current_window['stage'] == data['stage']
            if data['scope'] in SCOPES:
                current_window['individual_scopes'].append(data)
        if 'Inventory benchmark completed all requested real surface rounds' in line:
            completed = data.get('rounds') == 2
    assert completed, 'Expected completion of both requested chest rounds'
    assert result['process_summary'] and result['process_summary']['exit_code'] == 0
    assert len(result['phases']) == 6, 'Extended benchmark must preserve all six steady phases'
    assert len(result['transitions']) == 4, 'Expected open/close for two rounds'
    assert 'Inventory benchmark failed' not in text
    for entry in result['transitions']:
        frame_windows = [w for w in entry['cpu_windows'] if w['window'] == 'transition-frame']
        worst = max(frame_windows, key=lambda w: w['interval_ms'])
        assert abs(worst['interval_ms'] - entry['max_ms']) < 1e-6
        assert not worst['history_truncated']
        entry['worst_transition_frame'] = {
            'interval_ms': worst['interval_ms'], 'line': worst['line'],
            'scopes': {scope: worst['groups'].get(scope) for scope in SCOPES},
            'individual_scopes': worst['individual_scopes'],
            'additional_scheduling_scopes': {scope: worst['groups'].get(scope) for scope in ('main.post_update', 'ui.content', 'cpu.render_schedule_wall', 'render.execute', 'cpu.render_handoff_wall')},
        }
        entry['cold_first_layout'] = entry['round'] == 1 and entry['stage'] == 'Opening'
        entry['retained_layout_repeat'] = entry['round'] == 2 and entry['stage'] == 'Opening'
    return result


def concise(case):
    return {
        'openings': [{key: entry[key] for key in (
            'round', 'max_ms', 'frames', 'request_to_native_readiness_ms',
            'cold_first_layout', 'retained_layout_repeat', 'worst_transition_frame')}
            for entry in case['transitions'] if entry['stage'] == 'Opening'],
        'controls': [{key: phase.get(key) for key in (
            'round', 'phase', 'window_seconds', 'frames', 'fps', 'median_ms', 'p95_ms',
            'p99_ms', 'max_ms', 'resident_memory_kib', 'entities', 'images', 'ui_nodes')}
            for phase in case['phases']],
        'process_summary': case['process_summary'],
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--reference', type=Path)
    args = parser.parse_args()
    if args.reference:
        reference = parse(args.reference)
        (ARTIFACT / 'parser-reference-check.json').write_text(json.dumps(concise(reference), indent=2) + '\n')
        print('Reference parser validated:', [(e['round'], e['max_ms']) for e in concise(reference)['openings']])
        return
    cases = {variant: parse(ARTIFACT / variant / 'native.log') for variant in ['before', 'after']}
    metrics = {
        'readiness_definition': 'Two consecutive Last samples with applied native geometry/visibility/positive ancestor opacity and valid authoritative/projected chest surface; no GPU/present acknowledgement.',
        'frame_definition': 'CPU wall Last-to-Last intervals from the fresh actual request through native readiness.',
        'scope_caveat': 'Nested/concurrent scopes are not additive. ui.style is a scheduler interval, not an isolated private style-system body timer.',
        'same_game_source_manifest': 'game-source-hashes.json',
        'same_ui_source_manifest': 'native-ui-source-hashes.json',
        'binary_provenance': {'before': 'before-binary.json', 'after': 'after-binary.json'},
        'cases': cases,
        'summary': {variant: concise(case) for variant, case in cases.items()},
    }
    before = metrics['summary']['before']['openings']
    after = metrics['summary']['after']['openings']
    metrics['opening_comparison'] = [{
        'round': old['round'], 'kind': 'cold' if old['round'] == 1 else 'retained-repeat',
        'before_max_cpu_frame_ms': old['max_ms'], 'after_max_cpu_frame_ms': new['max_ms'],
        'max_cpu_frame_reduction_percent': (old['max_ms'] - new['max_ms']) / old['max_ms'] * 100.0,
        'before_native_readiness_ms': old['request_to_native_readiness_ms'],
        'after_native_readiness_ms': new['request_to_native_readiness_ms'],
    } for old, new in zip(before, after, strict=True)]
    output = ARTIFACT / 'native-metrics.json'
    output.write_text(json.dumps(metrics, indent=2) + '\n')
    print(json.dumps(metrics['opening_comparison'], indent=2))
    print('Wrote', output)


if __name__ == '__main__':
    main()
