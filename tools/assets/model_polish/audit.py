"""Read-only structural audit of shipped block assets (no aesthetic rewriting)."""
import argparse
import json
import math
from pathlib import Path
import sys

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from model_polish.gltf import Document

ROOT = Path(__file__).resolve().parents[3]
BLOCKS = ROOT / 'assets/terra/blocks'


def inspect_model(path):
    document = Document(path)
    errors, warnings = [], []
    triangles = vertices = 0
    reversed_faces = degenerate = invalid_normals = invalid_tangents = 0
    for mesh in document.doc.get('meshes', []):
        for primitive in mesh['primitives']:
            if primitive.get('mode', 4) != 4:
                warnings.append('non-triangle primitive')
                continue
            attributes = {key: document.values(index) for key, index in primitive['attributes'].items()}
            positions = attributes.get('POSITION', [])
            if not positions:
                errors.append('missing positions')
                continue
            vertices += len(positions)
            for semantic, values in attributes.items():
                if len(values) != len(positions):
                    errors.append(f'{semantic}: vertex count mismatch')
                if any(not math.isfinite(v) for row in values for v in row):
                    errors.append(f'{semantic}: non-finite value')
            normals = attributes.get('NORMAL', [])
            if not normals:
                errors.append('missing normals')
            invalid_normals += sum(abs(sum(v*v for v in n)-1) > .002 for n in normals)
            for n, t in zip(normals, attributes.get('TANGENT', [])):
                invalid_tangents += int(abs(sum(v*v for v in t[:3])-1) > .002
                    or abs(sum(a*b for a,b in zip(n,t))) > .002
                    or abs(abs(t[3])-1) > .001)
            indices = [row[0] for row in document.values(primitive['indices'])] if 'indices' in primitive else list(range(len(positions)))
            if len(indices) % 3:
                errors.append('incomplete triangle')
            if any(index < 0 or index >= len(positions) for index in indices):
                errors.append('invalid vertex index')
                continue
            triangles += len(indices)//3
            for i in range(0, len(indices)-2, 3):
                tri = indices[i:i+3]
                a, b, c = [positions[index] for index in tri]
                u, v = [b[k]-a[k] for k in range(3)], [c[k]-a[k] for k in range(3)]
                cross = [u[1]*v[2]-u[2]*v[1],u[2]*v[0]-u[0]*v[2],u[0]*v[1]-u[1]*v[0]]
                area = sum(value*value for value in cross)
                if area < 1e-20:
                    degenerate += 1
                elif normals:
                    normal = [sum(normals[index][axis] for index in tri) for axis in range(3)]
                    if sum(x*y for x,y in zip(cross,normal)) < -1e-6*math.sqrt(area):
                        reversed_faces += 1
            # Accessor bounds drive renderer culling: validate all active data.
            accessor = document.doc['accessors'][primitive['attributes']['POSITION']]
            for axis in range(3):
                if accessor.get('min', [-math.inf]*3)[axis] > min(p[axis] for p in positions)+1e-5 or accessor.get('max', [math.inf]*3)[axis] < max(p[axis] for p in positions)-1e-5:
                    errors.append('position accessor excludes active geometry')
    for image in document.doc.get('images', []):
        uri = image.get('uri', '')
        if uri and not uri.startswith('data:') and not (path.parent / uri).is_file():
            errors.append(f'missing texture {uri}')
    for label, count in [('collapsed triangles', degenerate), ('non-unit normals', invalid_normals), ('invalid tangents', invalid_tangents)]:
        if count:
            errors.append(f'{count} {label}')
    if reversed_faces:
        warnings.append(f'{reversed_faces} triangles oppose vertex normals; inspect authored double-sided sheets separately')
    return dict(path=str(path.relative_to(ROOT)), vertices=vertices, triangles=triangles,
                errors=sorted(set(errors)), warnings=sorted(set(warnings)))


def audit(maximum_id=399):
    records, models, ids = [], {}, set()
    for path in sorted(BLOCKS.glob('*.json')):
        block = json.loads(path.read_text())
        if block['id'] > maximum_id:
            continue
        errors = []
        if block['id'] in ids:
            errors.append('duplicate block ID')
        ids.add(block['id'])
        for name, texture in block.get('textures', {}).items():
            if not (BLOCKS / texture).is_file():
                errors.append(f'missing texture {name}: {texture}')
        if model := block.get('model'):
            model = BLOCKS / model.split('#', 1)[0]
            if model not in models:
                try:
                    models[model] = inspect_model(model)
                except (OSError, ValueError, KeyError, AssertionError) as error:
                    models[model] = dict(path=str(model.relative_to(ROOT)), errors=[str(error)], warnings=[])
            errors.extend(models[model]['errors'])
        records.append(dict(id=block['id'], definition=str(path.relative_to(ROOT)), errors=errors))
    return dict(maximum_block_id=maximum_id, blocks=len(records), models=len(models),
        failing_blocks=sum(bool(row['errors']) for row in records),
        definitions=records, geometry=sorted(models.values(), key=lambda row: row['path']))


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--maximum-id', type=int, default=399)
    args = parser.parse_args()
    result = audit(args.maximum_id)
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(result, indent=2)+'\n')
    print(json.dumps({key:value for key,value in result.items() if key not in ('definitions','geometry')}))
    sys.exit(bool(result['failing_blocks']))
