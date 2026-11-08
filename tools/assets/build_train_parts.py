#!/usr/bin/env python3
"""Author modular train parts with shared model, collision and recipe data."""
import json
from build_castle import Mesh, BLOCKS, RECIPES

PARTS = [
    (210, 'train_chassis', 'Zug-Fahrgestell'),
    (211, 'train_bogie', 'Drehgestell'),
    (212, 'train_frame', 'Zugrahmen'),
    (213, 'train_coupler', 'Zugkupplung'),
    (214, 'train_cab', 'Führerstand'),
    (215, 'train_seat', 'Zugsitz'),
    (216, 'train_cargo', 'Wagen-Laderaum'),
    (217, 'train_assembly_station', 'Zug-Montagestelle'),
]


def frame(mesh, height=.18):
    for x in [.08, .80]:
        mesh.box([x, .08, .06], [x + .12, height, .94], 'iron', solid=True)
    for z in [.06, .82]:
        mesh.box([.08, .08, z], [.92, height, z + .12], 'iron', solid=True)


def main():
    for material, slug, name in PARTS:
        mesh = Mesh(slug)
        mesh.materials['red'] = ([.48, .055, .055], None, .05, .88)
        mesh.materials['dark'] = ([.12, .14, .18], 'machinery-iron-hd.png', .7, .6)
        mesh.materials['oak'] = ([.44, .23, .10], 'machinery-oak-hd.png', 0, .85)
        if material in [210, 212]:
            frame(mesh, .28 if material == 210 else .18)
            mesh.box([.32, 0, .33], [.68, .12, .67], 'iron', solid=True)
            if material == 210:
                mesh.box([.12, .28, .12], [.88, .38, .88], 'dark', solid=True)
                for x in [.17, .77]:
                    for z in [.16, .76]:
                        mesh.box([x, .38, z], [x + .06, .42, z + .06], 'rivet', solid=True)
        elif material == 211:
            for z in [.25, .75]:
                mesh.box([-.50, .23, z - .045], [1.50, .31, z + .045], 'rivet', solid=True)
                # Wheel centres follow the shared two-metre track gauge.
                for x in [-.56, 1.44]:
                    for low_y, high_y, half_z in [(.12, .40, .23), (.05, .12, .15), (.40, .47, .15)]:
                        mesh.box([x, low_y, z - half_z], [x + .12, high_y, z + half_z], 'dark', solid=True)
                    mesh.box([x - .01 if x > .1 else x, .21, z - .065],
                             [x + .13, .33, z + .065], 'rivet', solid=True)
            mesh.box([-.40, .40, .08], [1.40, .55, .92], 'iron', solid=True)
            mesh.box([.32, .55, .33], [.68, 1.0, .67], 'dark', solid=True)
        elif material == 213:
            mesh.box([.29, .24, .05], [.71, .58, .30], 'dark', solid=True)
            mesh.box([.40, .32, .27], [.60, .46, .77], 'iron', solid=True)
            for x in [.26, .61]:
                mesh.box([x, .26, .69], [x + .13, .52, .96], 'iron', solid=True)
            mesh.box([.26, .26, .83], [.74, .52, .96], 'rivet', solid=True)
        elif material == 214:
            mesh.box([.05, .02, .05], [.95, .16, .95], 'iron', solid=True)
            for x in [.06, .84]:
                for z in [.08, .84]:
                    mesh.box([x, .16, z], [x + .10, .91, z + .08], 'oak', solid=True)
            # Open cab: the driver's head needs room above this one-block module.
            for x in [.02, .88]:
                mesh.box([x, .90, .02], [x + .10, 1, .98], 'dark', solid=True)
            mesh.box([.13, .16, .07], [.87, .46, .17], 'red', solid=True)
            mesh.box([.18, .47, .12], [.82, .57, .40], 'dark', solid=True)
            for x in [.25, .49, .69]:
                mesh.box([x, .57, .18], [x + .055, .65, .27], 'gold', solid=True)
            mesh.box([.20, .16, .50], [.80, .34, .78], 'iron', solid=True)
            mesh.box([.17, .34, .44], [.83, .50, .80], 'red', solid=True)
            mesh.box([.17, .49, .78], [.83, .86, .88], 'red', solid=True)
        elif material == 215:
            for x in [.18, .74]:
                for z in [.19, .69]:
                    mesh.box([x, .02, z], [x + .08, .35, z + .08], 'iron', solid=True)
            mesh.box([.12, .33, .12], [.88, .50, .82], 'red', solid=True)
            mesh.box([.12, .49, .72], [.88, .96, .88], 'red', solid=True)
            for x in [.05, .87]:
                mesh.box([x, .51, .24], [x + .08, .65, .81], 'oak', solid=True)
        elif material == 216:
            mesh.box([.08, .02, .08], [.92, .87, .92], 'oak', solid=True)
            mesh.box([.05, .87, .05], [.95, .96, .95], 'oak', solid=True)
            for x in [.13, .78]:
                mesh.box([x, .05, .055], [x + .09, .88, .945], 'iron', solid=True)
            mesh.box([.43, .58, .025], [.57, .76, .09], 'rivet', solid=True)
        else:
            mesh.box([.04, .02, .04], [.96, .14, .96], 'iron', solid=True)
            mesh.box([.31, .14, .31], [.69, .72, .69], 'dark', solid=True)
            mesh.box([.10, .69, .10], [.90, .83, .90], 'oak', solid=True)
            mesh.box([.14, .83, .16], [.86, .89, .50], 'iron', solid=True)
            mesh.box([.22, .89, .22], [.34, .96, .36], 'gold', solid=True)
            mesh.box([.62, .89, .22], [.74, .96, .36], 'red', solid=True)
        mesh.export(slug)
        definition = {
            'schema_version': 1, 'id': material, 'name': name,
            'description': ('Assemble or dismantle connected modular railway vehicles. Right-click to manage the workshop.'
                            if material == 217 else
                            'Modular railway vehicle part. Connect parts beside an assembly station, then assemble the vehicle.'),
            'creative_category': 'building', 'render_kind': 'custom_mesh',
            'model_layout': 'scene', 'model': f'models/{slug}.gltf',
            'struct_block': True, 'hardness': 1.8, 'resistance': 9,
            'required_tool': 'pickaxe', 'mining_level': 0,
            'collision_boxes': mesh.collision,
        }
        (BLOCKS / f'{slug}.json').write_text(json.dumps(definition, indent=2) + '\n')
        cells = [
            {'item': {'mineral': 'iron_ingot'}, 'count': 1},
            {'item': {'block': 18}, 'count': 1, 'wood': True},
            {'item': {'mineral': 'iron_ingot'}, 'count': 1},
            None,
            {'item': {'block': 210 if material != 210 else 18}, 'count': 1},
            None,
            {'item': {'mineral': 'iron_ingot'}, 'count': 1},
            {'item': {'block': 18}, 'count': 1, 'wood': True},
            {'item': {'mineral': 'iron_ingot'}, 'count': 1},
        ]
        # Each module has a distinct centre ingredient and arrangement.
        centres = {210: {'block': 3}, 211: {'block': 142}, 212: {'block': 210},
                   213: {'mineral': 'iron_ingot'}, 214: {'block': 118},
                   215: {'block': 103}, 216: {'block': 29}, 217: {'block': 28}}
        cells[4] = {'item': centres[material], 'count': 1}
        if material == 213:
            cells[1] = cells[7] = None
        recipe = {'id': 'teramic::' + slug, 'name': name, 'station': 'workbench',
                  'output': {'item': {'block': material}, 'count': 1},
                  'pattern': {'kind': 'shaped', 'width': 3, 'height': 3, 'cells': cells}}
        (RECIPES / f'{slug}.json').write_text(json.dumps(recipe, indent=2) + '\n')
    print('Generated eight modular train part models, shared collision profiles and recipes.')


if __name__ == '__main__':
    main()
