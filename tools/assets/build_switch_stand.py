#!/usr/bin/env python3
"""Author the separate switch stand from shared model/collision volumes."""
import json
from build_castle import Mesh, BLOCKS

VOLUMES = [
    ([.10, 0, .18], [.90, .12, .82], 'iron'),
    ([.36, .12, .36], [.64, .44, .64], 'iron'),
    ([.28, .38, .30], [.72, .50, .70], 'rivet'),
    ([.45, .50, .45], [.55, .82, .55], 'iron'),
    ([.28, .82, .34], [.72, .96, .66], 'red'),
]

def main():
    mesh = Mesh('switch-stand')
    mesh.materials['red'] = ([.72, .075, .035], None, .18, .65)
    for low, high, material in VOLUMES:
        mesh.box(low, high, material, bevel=.008, solid=True)
    mesh.export('rail_switch_stand')
    definition = {
        'schema_version': 1, 'id': 209, 'name': 'Rail Switch Stand',
        'description': 'Place directly beside the throat of a left or right switch. Right-click to select its route.',
        'creative_category': 'building', 'render_kind': 'custom_mesh',
        'model_layout': 'scene', 'model': 'models/rail_switch_stand.gltf',
        'struct_block': True, 'hardness': .8, 'resistance': 4,
        'required_tool': 'pickaxe', 'mining_level': 0,
        'collision_boxes': mesh.collision,
    }
    (BLOCKS / 'rail_switch_stand.json').write_text(json.dumps(definition, indent=2)+'\n')

if __name__ == '__main__':
    main()
