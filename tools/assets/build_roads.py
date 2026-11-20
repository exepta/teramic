#!/usr/bin/env python3
"""Reproduce F68 road definitions, seamless aggregate textures and ramp meshes."""
import json
import math
from pathlib import Path
import struct
import zlib

ROOT = Path(__file__).resolve().parents[2]
BLOCKS = ROOT / "assets/terra/blocks"
ROADS = [
    (431, "asphalt", "Asphalt", "Dense matte asphalt made from road aggregate, sand and coal tar."),
    (432, "asphalt_cover", "Asphalt Road Cover", "An eighth-block road surface. Eight covers recover one asphalt block."),
    (433, "asphalt_slope", "Asphalt Road Slope", "A continuous full-height road grade, rising in its facing direction."),
    (434, "asphalt_entry", "Asphalt Lower Transition", "A road transition from the thin cover to half-block height."),
    (435, "asphalt_exit", "Asphalt Upper Transition", "A road transition from half-block height to a full-height surface."),
    (436, "road_curb", "Stone Road Curb", "A quarter-height stone edging strip. Rotate it to follow the road."),
]


def write(path, value):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, indent=2, ensure_ascii=False) + "\n")


def png(path, concrete=False):
    size = 512
    data = bytearray()
    for y in range(size):
        data.append(0)
        for x in range(size):
            # Duplicated edge samples give bilinear/repeat filtering identical
            # borders on adjacent blocks and on the ramps' projected top UVs.
            h = ((x % (size - 1)) * 374761393 + (y % (size - 1)) * 668265263 + 190681) & 0xffffffff
            h = ((h ^ (h >> 13)) * 1274126177) & 0xffffffff
            grain = h % 17
            stone = (h >> 12) % 37 == 0
            if concrete:
                v = 146 + grain + (12 if stone else 0)
                data.extend([v + 5, v + 3, v, 255])
            else:
                # Charcoal asphalt retains readable aggregate under the game's
                # daylight/shadow response instead of collapsing to pure black.
                v = 65 + grain + (21 if stone else 0)
                data.extend([v, v + 1, v + 2, 255])
    def chunk(kind, payload):
        return struct.pack('>I', len(payload)) + kind + payload + struct.pack('>I', zlib.crc32(kind + payload))
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(b'\x89PNG\r\n\x1a\n' + chunk(b'IHDR', struct.pack('>IIBBBBB', size, size, 8, 6, 0, 0, 0)) + chunk(b'IDAT', zlib.compress(data, 9)) + chunk(b'IEND', b''))


def tar_png(path, wet):
    """Seamless restrained oil ripples or fine, cured aggregate, without baked light."""
    size = 256
    data = bytearray()
    for y in range(size):
        data.append(0)
        for x in range(size):
            u, v = x / (size - 1), y / (size - 1)
            wave = math.sin(math.tau * (3 * u + math.sin(math.tau * v) * 0.3))
            wave += 0.4 * math.cos(math.tau * (2 * v - u))
            h = ((x % (size - 1)) * 374761393 + (y % (size - 1)) * 668265263 + 68017) & 0xffffffff
            h = ((h ^ (h >> 13)) * 1274126177) & 0xffffffff
            if wet:
                value = 30 + int(3 * wave) + h % 3
                data.extend([value, value - 1, value + 1, 255])
            else:
                value = 53 + h % 17 + (12 if (h >> 12) % 41 == 0 else 0)
                # Sparse, periodic hairline cracks, narrow enough to read as tar.
                crack = abs(math.sin(math.tau * (u + 0.12 * math.sin(math.tau * v)))) < 0.018
                value -= 12 if crack else 0
                data.extend([value, value, value + 1, 255])
    def chunk(kind, payload):
        return struct.pack('>I', len(payload)) + kind + payload + struct.pack('>I', zlib.crc32(kind + payload))
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(b'\x89PNG\r\n\x1a\n' + chunk(b'IHDR', struct.pack('>IIBBBBB', size, size, 8, 6, 0, 0, 0)) + chunk(b'IDAT', zlib.compress(data, 9)) + chunk(b'IEND', b''))


def tar_assets():
    entries = [
        (437, 'wet_tar', 'Liquid Tar', 'Viscous, glossy tar on a prepared path. Flows slowly onto adjacent paths, then dries into a firm road.', True),
        (438, 'dried_tar', 'Dried Tar Road', 'A cured tar film bonded to its compacted path. Its thickness preserves the amount poured.', False),
    ]
    for material, slug, name, description, wet in entries:
        tar_png(BLOCKS / f'textures/{slug}.png', wet)
        write(BLOCKS / f'{slug}.json', dict(
            schema_version=1, id=material, name=name, description=description,
            creative_category='building', render_kind='boxes', bevel_width=0,
            collision_boxes=[dict(minimum=[0, 0, 0], maximum=[1, 0.9375, 1]), dict(minimum=[0, 0.9375, 0], maximum=[1, 1, 1])],
            materials={'Packed earth': dict(texture_pattern='path_side', roughness=1, reflectance=0.1),
                       'Tar surface': dict(base_color=[1, 1, 1, 1], roughness=0.16 if wet else 0.94, reflectance=0.68 if wet else 0.08)},
            textures={'Tar surface': f'textures/{slug}.png'},
            face_materials=['Packed earth', 'Packed earth', 'Packed earth', 'Tar surface', 'Packed earth', 'Packed earth'],
            hardness=0.7 if wet else 1.4, resistance=0.6 if wet else 5, required_tool='shovel' if wet else 'pickaxe', mining_level=0))
    write(ROOT / 'assets/terra/items/tar_bucket.json', dict(item={'tool': 'tar_bucket'}, name='Tar Bucket',
        description='A finite amount of viscous tar. Coats prepared paths within four blocks, leveling into a thin film before drying.'))
    write(ROOT / 'assets/crafting/building/roads/tar_bucket.json', dict(id='teramic::tar_bucket', name='Fill Tar Bucket', station='workbench',
        output=dict(item={'tool': 'tar_bucket'}, count=1), pattern=dict(kind='shaped', width=1, height=2,
        cells=[dict(item='bitumen', count=4), dict(item={'tool': 'bucket'}, count=1)])))
    return entries


def ramp(slug, lower, upper):
    faces = [
        ([[0,0,0],[1,0,0],[1,0,1],[0,0,1]], (0,2)),
        ([[0,lower,0],[0,upper,1],[1,upper,1],[1,lower,0]], (0,2)),
        ([[0,0,0],[0,0,1],[0,upper,1],[0,lower,0]], (2,1)),
        ([[1,0,1],[1,0,0],[1,lower,0],[1,upper,1]], (2,1)),
        ([[1,0,0],[0,0,0],[0,lower,0],[1,lower,0]], (0,1)),
        ([[0,0,1],[1,0,1],[1,upper,1],[0,upper,1]], (0,1)),
    ]
    positions, normals, uvs, indices = [], [], [], []
    for vertices, axes in faces:
        for first,second in [(1,2),(2,3)]:
            a = [vertices[first][i] - vertices[0][i] for i in range(3)]
            b = [vertices[second][i] - vertices[0][i] for i in range(3)]
            n = [a[1]*b[2]-a[2]*b[1], a[2]*b[0]-a[0]*b[2], a[0]*b[1]-a[1]*b[0]]
            length = math.sqrt(sum(v*v for v in n))
            if length > 1e-8: break
        else:
            continue
        start = len(positions)
        positions.extend(vertices)
        normals.extend([[v / length for v in n]] * 4)
        uvs.extend([[p[axes[0]], p[axes[1]]] for p in vertices])
        for local in [(0,1,2),(0,2,3)]:
            p,q,r = [vertices[i] for i in local]
            u,v = [[point[i]-p[i] for i in range(3)] for point in (q,r)]
            area = [u[1]*v[2]-u[2]*v[1],u[2]*v[0]-u[0]*v[2],u[0]*v[1]-u[1]*v[0]]
            if sum(a*a for a in area) > 1e-12:
                indices.extend(start+i for i in local)
    buffer = bytearray()
    views, accessors = [], []
    for values, width, component, fmt in [(positions,3,5126,'f'),(normals,3,5126,'f'),(uvs,2,5126,'f'),(indices,1,5123,'H')]:
        while len(buffer) % 4: buffer.append(0)
        flat = [v for row in values for v in row] if width > 1 else values
        payload = struct.pack('<' + fmt * len(flat), *flat)
        views.append(dict(buffer=0,byteOffset=len(buffer),byteLength=len(payload)))
        accessor = dict(bufferView=len(views)-1,componentType=component,count=len(values),type={1:'SCALAR',2:'VEC2',3:'VEC3'}[width])
        if len(accessors) == 0:
            accessor.update(min=[min(p[i] for p in positions) for i in range(3)],max=[max(p[i] for p in positions) for i in range(3)])
        accessors.append(accessor)
        buffer.extend(payload)
    models = BLOCKS/'models'; models.mkdir(parents=True,exist_ok=True)
    (models/f'{slug}.bin').write_bytes(buffer)
    write(models/f'{slug}.gltf', dict(asset=dict(version='2.0',generator='TERAMIC F68 road builder'),scene=0,scenes=[dict(nodes=[0])],nodes=[dict(mesh=0)],meshes=[dict(primitives=[dict(attributes=dict(POSITION=0,NORMAL=1,TEXCOORD_0=2),indices=3,material=0)])],buffers=[dict(uri=f'{slug}.bin',byteLength=len(buffer))],bufferViews=views,accessors=accessors,materials=[dict(name='Road aggregate',pbrMetallicRoughness=dict(baseColorTexture=dict(index=0),metallicFactor=0,roughnessFactor=1))],images=[dict(uri='../textures/asphalt.png')],textures=[dict(source=0,sampler=0)],samplers=[dict(magFilter=9729,minFilter=9987,wrapS=10497,wrapT=10497)]))


def main():
    tar_entries = tar_assets()
    png(BLOCKS/'textures/asphalt.png')
    png(BLOCKS/'textures/road_curb.png', True)
    for material, slug, name, description in ROADS:
        definition = dict(schema_version=1,id=material,name=name,description=description,creative_category='building',render_kind='cube' if material==431 else 'boxes',bevel_width=0,hardness=1.4,resistance=5,required_tool='pickaxe',mining_level=0)
        texture = 'road_curb' if material==436 else 'asphalt'
        definition.update(materials={'Road aggregate':dict(base_color=[1,1,1,1],roughness=1,reflectance=0.06)},textures={'Road aggregate':f'textures/{texture}.png'},face_materials=['Road aggregate']*6)
        if material==432:
            definition['collision_boxes']=[dict(minimum=[0,0,0],maximum=[1,0.125,1])]
        elif material in (433,434,435):
            lower,upper = {433:(0,1),434:(0.125,0.5),435:(0.5,1)}[material]
            ramp(slug,lower,upper)
            definition.update(render_kind='custom_mesh',model_layout='scene',model=f'models/{slug}.gltf',collision_boxes=[dict(minimum=[0,0,i/12],maximum=[1,lower+(upper-lower)*(i+1)/12,(i+1)/12]) for i in range(12)])
        elif material==436:
            definition['collision_boxes']=[dict(minimum=[0,0,0],maximum=[1,0.25,0.1875])]
        write(BLOCKS/f'{slug}.json', definition)
    write(ROOT/'assets/terra/items/bitumen.json', dict(item='bitumen',name='Coal Tar Binder',description='A heated coal-tar binder used with road aggregate and sand to make asphalt.'))
    write(ROOT/'assets/terra/items/road_aggregate.json', dict(item='road_aggregate',name='Road Aggregate',description='Crushed stone aggregate for durable asphalt roads.'))
    write(ROOT/'assets/crafting/processes/crushing/road_aggregate.json', dict(id=10619,key='teramic::crush_road_aggregate',name='Crush road aggregate',input={'block':3},output=dict(item='road_aggregate',count=4),seconds=4))
    write(ROOT/'assets/crafting/processes/smelting/bitumen.json', dict(id=10,key='teramic::smelt_bitumen',name='Heat coal tar binder',input={'dust':'coal'},output=dict(item='bitumen',count=1),seconds=8))
    def recipe(slug,name,ingredients,output,count):
        write(ROOT/f'assets/crafting/building/roads/{slug}.json',dict(id=f'teramic::{slug}',name=name,station='workbench',output=dict(item=output,count=count),pattern=dict(kind='shapeless',ingredients=[dict(item=item,count=amount) for item,amount in ingredients])))
    recipe('asphalt','Mix asphalt',[('road_aggregate',4),({'block':4},1),('bitumen',1)],{'block':431},4)
    for slug,name,inputs,outputs in [('asphalt_cover','Asphalt Road Cover',1,8),('asphalt_slope','Asphalt Road Slope',2,4),('asphalt_entry','Asphalt Lower Transition',5,16),('asphalt_exit','Asphalt Upper Transition',3,4),('road_curb','Stone Road Curb',3,64)]:
        material=next(r[0] for r in ROADS if r[1]==slug)
        base=12 if material==436 else 431
        # Distinct occupied-cell counts disambiguate road forms even when a
        # player's backing inventory stack contains more than one recipe needs.
        ingredients = [({'block':base},1)] * inputs if base==431 else [({'block':base},inputs)]
        recipe(slug,name,ingredients,{'block':material},outputs)
        recipe('recover_'+slug,'Recover '+name,[({'block':material},outputs)],{'block':base},inputs)
    translations=[('Coal Tar Binder','Kohlenteer-Bindemittel'),('Road Aggregate','Strassenzuschlag'),('Mix asphalt','Asphalt mischen'),('Heat coal tar binder','Kohlenteer erhitzen'),('Crush road aggregate','Strassenzuschlag mahlen')]
    german=['Asphalt','Asphalt-Strassendecke','Asphalt-Steigung','Unterer Asphalt-Uebergang','Oberer Asphalt-Uebergang','Stein-Randstein']
    for road,de in zip(ROADS,german):
        translations.extend([(road[2],de),('Recover '+road[2],de+' zurueckverarbeiten')])
    descriptions=list(zip([r[3] for r in ROADS],[
        'Dichter, matter Asphalt aus Strassenzuschlag, Sand und Kohlenteer.',
        'Eine achtelblockhohe Strassendecke. Acht Decken ergeben wieder einen Asphaltblock.',
        'Eine durchgehende, volle Blockhoehe ueberwindende Strassensteigung in Blickrichtung.',
        'Ein Strassenuebergang von der duennen Decke auf eine halbe Blockhoehe.',
        'Ein Strassenuebergang von einer halben auf eine ganze Blockhoehe.',
        'Ein viertelblockhoher Randstein. Durch Drehen an den Strassenverlauf anpassen.',
    ]))
    descriptions.extend([
        ('A heated coal-tar binder used with road aggregate and sand to make asphalt.', 'Ein erhitztes Bindemittel aus Kohlenteer fuer Asphalt aus Zuschlag und Sand.'),
        ('Crushed stone aggregate for durable asphalt roads.', 'Gebrochener Steinzuschlag fuer haltbare Asphaltstrassen.'),
    ])
    translations.extend([
        ('Liquid Tar', 'Flüssiger Teer'),
        ('Dried Tar Road', 'Getrockneter Teerbelag'),
        ('Tar Bucket', 'Teereimer'),
        ('Fill Tar Bucket', 'Teereimer füllen'),
    ])
    descriptions.extend([
        (tar_entries[0][3], 'Zähflüssiger, glänzender Teer auf einem vorbereiteten Weg. Fließt langsam auf benachbarte Wege und trocknet zu festem Straßenbelag.'),
        (tar_entries[1][3], 'Ein getrockneter Teerfilm auf einem verdichteten Weg. Seine Dicke entspricht der ausgegossenen Menge.'),
        ('A finite amount of viscous tar. Coats prepared paths within four blocks, leveling into a thin film before drying.', 'Acht begrenzte Einheiten zähflüssiger Teer. Auf einen freien Wegblock gießen; fließt nur entlang vorbereiteter Wege und trocknet zu Straßenbelag.'),
    ])
    write(ROOT/'src-ui/locales/source/roads.json',[dict(source=en,en=en,de=de) for en,de in translations+descriptions])


if __name__ == '__main__':
    main()
