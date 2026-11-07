#!/usr/bin/env python3
"""Build native rail scenes from the exact Core::rails beam export.

Compile export_rails.rs with rustc against the existing serde rlib, write its
stdout under target/, then pass that JSON path here. No second path/slope
formula lives in this generator. Runtime uses these same cached Core beams.
"""
from pathlib import Path
import hashlib
import json
import os
import subprocess
import sys
from build_castle import Mesh

ROOT = Path(__file__).resolve().parents[2]
BLOCKS = ROOT / 'assets/terra/blocks'
PALETTE = {
    'Wood': ('rail_0_wood', [.78, .48, .22], 'machinery-oak-hd.png', 0., .88),
    'Steel': ('rail_1_steel', [.38, .32, .28], 'machinery-iron-hd.png', .74, .63),
    'SteelHead': ('rail_2_head', [.57, .53, .48], 'machinery-iron-hd.png', .79, .42),
    'Bolt': ('rail_3_bolt', [.64, .60, .54], None, .72, .48),
    'Red': ('rail_4_red', [.72, .075, .035], None, .18, .65),
}
NAMES = [
    'Rail: Straight', 'Rail: Small Curve', 'Rail: Large Curve', 'Rail: Slope',
    'Rail: Slope Entry', 'Rail: Slope Exit', 'Rail: Left Switch',
    'Rail: Right Switch', 'Rail: Crossing', 'Rail: Buffer Stop',
]


def core_sources():
    return [ROOT / 'crates/terra-core/src/rails.rs',
            ROOT / 'crates/terra-core/src/rails/geometry.rs',
            ROOT / 'crates/terra-core/src/rails/surfaces.rs']


def source_hashes():
    digest = hashlib.sha256()
    stable = 0xcbf29ce484222325
    for source in core_sources():
        payload = str(source.relative_to(ROOT)).encode() + b'\0' + source.read_bytes()
        digest.update(payload)
        for byte in payload:
            stable = ((stable ^ byte) * 0x100000001b3) & ((1 << 64) - 1)
    return digest.hexdigest(), f'{stable:016x}'


def export_core():
    """Adapt module paths only; all profile types/formulas remain production code."""
    temporary = ROOT / 'target/rail-asset-export'
    temporary.mkdir(parents=True, exist_ok=True)
    source = (ROOT / 'crates/terra-core/src/rails.rs').read_text()
    source = source.replace('//!', '//')
    for module in ['geometry', 'network', 'surfaces']:
        path = ROOT / f'crates/terra-core/src/rails/{module}.rs'
        source = source.replace(f'mod {module};', f'#[path = {json.dumps(str(path))}] mod {module};')
    adapted = temporary / 'production-rails.rs'
    adapted.write_text(source)
    serde = max((ROOT / 'target/debug/deps').glob('libserde-*.rlib'), key=lambda path: path.stat().st_mtime)
    executable = temporary / 'export-rails'
    subprocess.run(['rustc', '--edition=2024', str(ROOT / 'tools/assets/export_rails.rs'),
                    '--extern', f'serde={serde}', '-L', f'dependency={ROOT / "target/debug/deps"}',
                    '-o', str(executable)], check=True,
                   env={**os.environ, 'TERRA_RAIL_PROFILE_SOURCE': str(adapted)})
    exported = temporary / 'profiles.json'
    with exported.open('w') as output:
        subprocess.run([str(executable)], stdout=output, check=True)
    return exported


def native_mesh(profile):
    mesh = Mesh('rail')
    mesh.materials = {}
    mesh.parts = {}
    used = {beam['material'] for beam in profile['beams']}
    # The importer and runtime use this explicit, fixed semantic slot order.
    for role, (name, color, texture, metal, roughness) in PALETTE.items():
        if role not in used:
            continue
        mesh.materials[name] = (color, texture, metal, roughness)
        mesh.parts[name] = {key: [] for key in ['position', 'normal', 'uv', 'tangent', 'indices']}
    for surface in profile['surfaces']:
        mesh.polygon(surface['points'], PALETTE[surface['material']][0], surface['normal'])
    return mesh


def main():
    if len(sys.argv) != 2:
        raise SystemExit('Usage: build_rails.py target/rail-profile-export.json')
    source = export_core() if sys.argv[1] == '--export-core' else Path(sys.argv[1])
    exported = json.loads(source.read_text())
    provenance, source_fnv64 = source_hashes()
    manifest = {'schema_version': 1, 'core_source_sha256': provenance, 'core_source_fnv64': source_fnv64,
                'generator': 'tools/assets/export_rails.rs + tools/assets/build_rails.py',
                'profiles': exported['profiles']}
    (BLOCKS / 'models/rail_profiles.json').write_text(json.dumps(manifest, separators=(',', ':'))+'\n')
    for profile in exported['profiles']:
        if profile['facing'] != 0:
            continue
        material, slug = profile['material'], profile['key']
        native_mesh(profile).export(slug)
        model_path = BLOCKS / 'models' / f'{slug}.gltf'
        gltf = json.loads(model_path.read_text())
        gltf['asset']['generator'] = 'Teramic Core::rails oriented beam exporter'
        gltf['extras'] = {'core_source_sha256': provenance, 'core_source_fnv64': source_fnv64, 'rail_material': material}
        model_path.write_text(json.dumps(gltf, separators=(',', ':'))+'\n')
        definition = {
            'schema_version': 1, 'id': material, 'name': NAMES[material-196],
            'description': 'Three-block-wide track with wooden sleepers, steel rail heads and bolted feet. '
                           'The complete module follows its shared collision and routing profile.',
            'creative_category': 'building', 'render_kind': 'custom_mesh',
            'model_layout': 'scene', 'model': f'models/{slug}.gltf',
            'struct_block': True, 'hardness': 1.5, 'resistance': 8.,
            'required_tool': 'pickaxe', 'mining_level': 0,
            'collision_boxes': profile['anchor_collision'],
        }
        (BLOCKS / f'{slug}.json').write_text(json.dumps(definition, indent=2)+'\n')
    print(f'Generated ten complete rail assemblies from Core profile {provenance[:12]}.')


if __name__ == '__main__':
    main()
