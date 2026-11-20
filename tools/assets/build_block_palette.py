#!/usr/bin/env python3
"""Reproducible carved fantasy palette. Shared PNG/PBR maps, unchanged cells/IDs.

No imported geometry is replaced. Stone, wood and sand retain the existing
closed-cube profiles and section batching. Coordinates repeat at both edges.
"""
import json
import math
from functools import lru_cache
from pathlib import Path
from build_spirit_blossom import png
from biome_surfaces import noise, field, mix, rgb

ROOT = Path(__file__).resolve().parents[2]
BLOCKS = ROOT / 'assets/terra/blocks'
DEST = BLOCKS / 'textures/block_palette'
SIZE = 512
SPECIES = {
    'oak': ([143, 96, 53], [204, 151, 85], [186, 128, 65]),
    'birch': ([223, 213, 184], [239, 208, 156], [212, 173, 116]),
    'spruce': ([95, 64, 41], [151, 91, 49], [167, 113, 65]),
    'blood': ([117, 62, 51], [119, 51, 49], [137, 61, 55]),
    'ghost': ([112, 133, 143], [133, 159, 165], [105, 160, 174]),
    'sakura': ([116, 67, 63], [216, 133, 137], [206, 130, 121]),
}
FAMILIES = {'oak': (6, 16), 'birch': (10, 17), 'spruce': (36, 18),
            'blood': (405, 404), 'ghost': (313, 314), 'sakura': (134, 135)}


def edge_distance(value, center, period=256):
    return (value - center + period / 2) % period - period / 2


@lru_cache(maxsize=262144)
def wood_form(x, y, form):
    if form == 'end':
        dx, dy = x - 127.5, y - 127.5
        angle = math.atan2(dy, dx)
        radius = math.hypot(dx, dy) + math.sin(angle * 5) * 2.6
        ridge = (radius / 13.2 + math.sin(angle * 3) * .035) % 1
        furrow = max(0, 1 - ridge / .11)
        light = max(0, 1 - abs(ridge - .16) / .10)
        bark = max(0, min(1, (radius - 119) / 7))
        return .91 - furrow * .22 + light * .10 - bark * .25, .45 - furrow * .06 - bark * .08
    if form == 'planks':
        board = int(y // 64)
        v = y % 64
        end = (x + [0, 91, 23, 157][board]) % 256
        seam = min(v, 64 - v, end, 256 - end)
        relief = max(0, min(1, seam / 2.0))
        edge = max(0, 1 - abs(seam - 3.3) / 3)
        line = math.sin(y * .85 + math.sin(x * math.tau / 128) * 1.8) + .3*math.sin(y*2.1+math.sin(x*math.tau/256)*3)
        knots = 0
        for cx, cy in [(54, 31), (181, 158)]:
            dx = edge_distance(x, cx) / 19
            dy = edge_distance(y, cy) / 8
            radius = math.hypot(dx, dy)
            if radius < 1:
                knots += math.sin(radius * 15) * (1 - radius) * .10
        return .65 + relief * .30 + edge * .08 + line * .070 + knots, .32 + relief * .12 + edge * .018 + knots * .08
    # Staggered, elongated bark scales; no continuous barcode-like channels.
    wx=(x+math.sin(y*math.tau/256)*5)%256
    wy=(y+math.sin(x*math.tau/256)*4)%256
    cx,cy=int(wx/42.6666667),int(wy/85.3333333)
    nearest,second,chosen=1e9,1e9,None
    for row in range(cy-1,cy+2):
        for col in range(cx-1,cx+2):
            seed=noise(col%6,row%3,971)
            px=(col+.25+seed*.5)*42.6666667
            py=(row+.25+noise(col%6,row%3,977)*.5)*85.3333333
            dx,dy=(wx-px)/42.6666667,(wy-py)/85.3333333
            distance=math.hypot(dx,dy)
            if distance<nearest:second,nearest,chosen=nearest,distance,(seed,dx,dy)
            elif distance<second:second=distance
    seed,dx,dy=chosen
    gap=second-nearest
    shoulder=max(0,min(1,gap/.085))
    lip=max(0,1-abs(gap-.12)/.07)
    fibre=math.sin(x*1.6+math.sin(y*math.tau/128)*1.9)*.019
    return (.61+shoulder*.31+lip*.06+seed*.13-dx*.06-dy*.03+fibre,
            .33+shoulder*.095+lip*.018-dx*.012)


def wood_pixel(species, form, x, y):
    colors = SPECIES[species]
    base = colors[{'bark': 0, 'planks': 1, 'end': 2}[form]]
    shade, height = wood_form(x, y, form)
    if species == 'birch' and form == 'bark':
        row = int(y // 29)
        xx = (x + noise(row, 0, 212) * 256) % 256
        if y % 29 < 2.7 and xx < 13 + noise(row, 0, 214) * 26:
            base = [78, 68, 55]
            shade = .88
    if form == 'bark' and species in ('oak', 'spruce', 'birch'):
        moss = field(x, y, 8, 819) * .7 + field(x, y, 16, 823) * .3
        if moss > .78:
            base = mix(base, [106, 121, 45], min(.65, (moss - .78) * 4))
    if species == 'ghost':
        seam = abs(math.sin((x + math.sin(y * math.tau / 128) * 3) * math.tau / 64))
        if form == 'end':
            dx, dy = x-127.5, y-127.5
            seam = abs(math.sin(math.hypot(dx,dy) / 13.2 * math.pi))
        if seam < .08:
            base = mix(base, [65, 206, 214], .64)
    fine = .985 + .03 * noise(int(x * 2), int(y * 2), 337)
    return rgb(base, shade * fine), height


def mineral_pixel(kind, x, y):
    if kind == 'sand':
        ripple = math.sin(y * math.tau / 32 + math.sin(x * math.tau / 128) * .55)
        broad = field(x, y, 4, 419)
        # Fine pale beige grains; broad ochre bands made a beach read as rock.
        grain = noise(int(x*2), int(y*2), 3)
        height = .43 + ripple * .0045 + broad * .004 + (grain-.5) * .0015
        return rgb(mix([232, 211, 175], [241, 223, 191], broad),
                   .975 + ripple * .008 + grain * .03), height
    cells = 5 if kind == 'stone' else 8 if kind == 'cobble' else 15
    pitch = 256 / cells
    cx, cy = int(x // pitch), int(y // pitch)
    nearest, second, chosen = 1e9, 1e9, None
    for row in range(cy-1,cy+2):
        for col in range(cx-1,cx+2):
            n = noise(col%cells,row%cells,47)
            px = (col+.25+n*.5)*pitch
            py = (row+.25+noise(col%cells,row%cells,53)*.5)*pitch
            dx,dy=x-px,y-py
            distance=math.hypot(dx,dy)
            if distance < nearest:
                second,nearest,chosen=nearest,distance,(n,dx/pitch,dy/pitch)
            elif distance < second: second=distance
    n,dx,dy=chosen
    edge=second-nearest
    rim=max(0,min(1,edge/(1.2 if kind=='stone' else 2.3)))
    relief=.91 + n*.10 - dx*.07 - dy*.08
    shade=(.66+rim*.34)*relief
    if kind=='stone': base=mix([111,112,119],[145,139,143],n)
    elif kind=='gravel': base=mix([135,117,103],[172,154,135],n)
    else:
        base=mix([115,112,107],[151,142,130],n)
        moss=field(x,y,8,433)
        if edge<5 and moss>.59: base=mix(base,[105,123,51],.7)
    return rgb(base,shade*(.985+noise(int(x*2),int(y*2),61)*.03)), .29+rim*.13-dx*.018-dy*.02


def write_texture(name, sampler):
    colors=[]; heights=[]
    for y in range(SIZE):
        for x in range(SIZE):
            color,height=sampler(x*256/(SIZE-1),y*256/(SIZE-1))
            colors.append(color+[255]);heights.append(height)
    png(DEST/(name+'.png'),colors,SIZE)
    normals=[]
    for y in range(SIZE):
        for x in range(SIZE):
            # Edges are identical samples, so use the same periodic interior.
            xx=x%(SIZE-1); yy=y%(SIZE-1)
            dx=(heights[yy*SIZE+(xx+1)%(SIZE-1)]-heights[yy*SIZE+(xx-1)%(SIZE-1)])*20
            dy=(heights[((yy+1)%(SIZE-1))*SIZE+xx]-heights[((yy-1)%(SIZE-1))*SIZE+xx])*20
            length=math.sqrt(dx*dx+dy*dy+1)
            normals.append([round((-.5*dx/length+.5)*255),round((-.5*dy/length+.5)*255),round((.5/length+.5)*255),255])
    png(DEST/(name+'_normal.png'),normals,SIZE)


def material(name):
    return {'base_color':[1,1,1,1], 'roughness':.94, 'reflectance':.12,
            'normal_texture':'textures/block_palette/'+name+'_normal.png'}


def bind(definition, side, end=None):
    names=definition['face_materials']
    side_key=names[0]; end_key=(names[3] if names[3]!=side_key else side_key+' end grain') if end else side_key
    definition['materials']={side_key:material(side)}
    definition['textures']={side_key:'textures/block_palette/'+side+'.png'}
    definition['face_materials']=[side_key]*6
    if end:
        definition['materials'][end_key]=material(end)
        definition['textures'][end_key]='textures/block_palette/'+end+'.png'
        for face in [2,3]:definition['face_materials'][face]=end_key
    if side not in ('stone','sand','gravel','cobble'):
        # The authored PBR carries bark/board relief; the old strip displacement
        # creates corrugated barcodes and hides the grain. Keep a closed bevel.
        definition.pop('surface',None)
        definition['bevel_width']=.025
    else:
        definition['surface']='rock' if side in ('stone','cobble') else 'sand' if side=='sand' else 'earth'

    return definition


def propagate_shared_minerals(paths=None):
    paths=paths or {json.loads(p.read_text())['id']:p for p in BLOCKS.glob('*.json')}
    patterns={'stone':'stone','sand':'sand','mossy_stone':'cobble','gravel':'gravel'}
    changed=[]
    for path in paths.values():
        d=json.loads(path.read_text());dirty=False
        for key,m in list(d.get('materials',{}).items()):
            if m.get('texture_pattern') in patterns:
                name=patterns[m['texture_pattern']]
                replacement=material(name)
                if 'base_color' in m:
                    replacement['base_color']=m['base_color']
                d['materials'][key]=replacement
                d.setdefault('textures',{})[key]='textures/block_palette/'+name+'.png'
                dirty=True
        if dirty:
            path.write_text(json.dumps(d,indent=2)+'\n');changed.append(path.name)
    return changed


def main():
    DEST.mkdir(parents=True,exist_ok=True)
    paths={json.loads(p.read_text())['id']:p for p in BLOCKS.glob('*.json')}
    for form in ['bark','planks','end']:
        for species in FAMILIES:
            write_texture(species+'_'+form,lambda x,y,s=species,f=form:wood_pixel(s,f,x%256,y%256))
    for species,ids in FAMILIES.items():
        for block_id,form in [(ids[0],'bark'),(ids[1],'planks')]:
            path=paths[block_id];d=json.loads(path.read_text())
            bind(d,species+'_'+form,species+'_end' if form=='bark' else None)
            path.write_text(json.dumps(d,indent=2)+'\n')
        # All existing stairs/slabs/fences of this family share exactly the same
        # material maps, keeping construction silhouettes and inventory identity.
        for path in paths.values():
            d=json.loads(path.read_text())
            for key,m in list(d.get('materials',{}).items()):
                if m.get('texture_pattern')=='planks' and key in json.loads(paths[ids[1]].read_text())['materials']:
                    d['materials'][key]=material(species+'_planks')
                    d.setdefault('textures',{})[key]='textures/block_palette/'+species+'_planks.png'
                    path.write_text(json.dumps(d,indent=2)+'\n')
    for block_id,kind in [(3,'stone'),(4,'sand'),(42,'gravel'),(13,'cobble')]:
        write_texture(kind,lambda x,y,k=kind:mineral_pixel(k,x%256,y%256))
        path=paths[block_id];d=json.loads(path.read_text());bind(d,kind);path.write_text(json.dumps(d,indent=2)+'\n')
    propagate_shared_minerals(paths)
    print('Generated 22 shared 512px albedo/linear-normal pairs and bound the reference palette.')

if __name__=='__main__':main()
