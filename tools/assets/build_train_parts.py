#!/usr/bin/env python3
"""Author modular train parts with shared model, collision and recipe data."""
import json
from build_castle import Mesh, BLOCKS
from recipe_paths import recipe_path
from build_locomotive_parts import palette, wheel, cylinder

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


def main(materials=None):
    generated = 0
    for material, slug, name in PARTS:
        if materials is not None and material not in materials:
            continue
        generated += 1
        mesh = Mesh(slug)
        mesh.materials['red'] = ([.48, .055, .055], None, .05, .88)
        mesh.materials['dark'] = ([.12, .14, .18], 'machinery-iron-hd.png', .7, .6)
        mesh.materials['oak'] = ([.44, .23, .10], 'machinery-oak-hd.png', 0, .85)
        palette(mesh)
        if material in [210, 212]:
            # Structural cells expose a complete attachable face on all six sides.
            mesh.box([0, 0, 0], [1, 1, 1], 'train_black_iron', solid=True)
            for x in [.02, .90]:
                mesh.box([x, .03, .01], [x + .08, .97, .04], 'train_red' if material == 210 else 'train_dark_steel')
            for x in [.15, .75]:
                for y in [.15, .75]:
                    mesh.box([x, y, .005], [x + .06, y + .06, .05], 'rivet')
        elif material == 211:
            for z in [0.0, 1.0]:
                cylinder(mesh, [.5, .5, z], .07, -.5, 1.5, 0, 'train_axle')
            for index, (x, z) in enumerate([(-.5, 0), (-.5, 1), (1.5, 0), (1.5, 1)]):
                wheel(mesh, .45, [x, .5, z], x - .065, x + .065, 'train_bogie_wheel_' + str(index))
            mesh.box([-.40, .82, .0], [1.40, 1, 1], 'train_black_iron', solid=True)
            for x in [-.3, 1.1]:
                mesh.box([x, .60, .10], [x + .2, .85, .90], 'train_red', solid=True)
        elif material == 213:
            mesh.box([.29, .24, 0], [.71, .58, .30], 'dark', solid=True)
            mesh.box([.40, .32, .27], [.60, .46, 1], 'iron', solid=True)
            for x in [.26, .61]:
                mesh.box([x, .26, .69], [x + .13, .52, .96], 'iron', solid=True)
            mesh.box([.26, .26, .83], [.74, .52, .96], 'rivet', solid=True)
        elif material == 214:
            mesh.box([0, 0, 0], [1, .16, 1], 'iron', solid=True)
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
            mesh.materials['train_throttle_lever_shaft'] = ([.89, .57, .16], None, .82, .33)
            mesh.materials['train_throttle_lever_grip'] = ([.08, .09, .10], None, .12, .85)
            mesh.box([.69, .57, .22], [.75, .87, .28], 'train_throttle_lever_shaft')
            mesh.box([.64, .85, .20], [.80, .92, .30], 'train_throttle_lever_grip')
            mesh.box([.20, .16, .50], [.80, .34, .78], 'iron', solid=True)
            mesh.box([.17, .34, .44], [.83, .50, .80], 'red', solid=True)
            mesh.box([.17, .49, .78], [.83, .86, .88], 'red', solid=True)
        elif material == 215:
            for x in [.18, .74]:
                for z in [.19, .69]:
                    mesh.box([x, 0, z], [x + .08, .35, z + .08], 'iron', solid=True)
            mesh.box([.12, .33, .12], [.88, .50, .82], 'red', solid=True)
            mesh.box([.12, .49, .72], [.88, .96, .88], 'red', solid=True)
            for x in [.05, .87]:
                mesh.box([x, .51, .24], [x + .08, .65, .81], 'oak', solid=True)
        elif material == 216:
            mesh.box([.08, 0, .08], [.92, .87, .92], 'oak', solid=True)
            mesh.box([.05, .87, .05], [.95, .96, .95], 'oak', solid=True)
            for x in [.13, .78]:
                mesh.box([x, .05, .055], [x + .09, .88, .945], 'iron', solid=True)
            mesh.box([.43, .58, .025], [.57, .76, .09], 'rivet', solid=True)
        else:
            mesh.box([.04, 0, .04], [.96, .14, .96], 'iron', solid=True)
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
            'creative_category': 'trains', 'render_kind': 'custom_mesh',
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
        recipe_path(slug).write_text(json.dumps(recipe, indent=2) + '\n')
    print(f'Generated {generated} modular train part models, shared collision profiles and recipes.')


if __name__ == '__main__':
    main()
