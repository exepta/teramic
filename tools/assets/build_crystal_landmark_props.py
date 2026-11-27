"""Build the carved fittings, instruments and mine equipment of the Crystal Highlands."""
import math
from pathlib import Path

from build_castle import BLOCKS, Mesh
import highland_gems as gems
from build_salt_cave import save_json
from voxel_props.surface import export_voxels
from cemetery.geometry import cylinder

ROOT = Path(__file__).resolve().parents[2]
PALETTE = {
    'slate': (.26, .23, .34), 'pearl': (.76, .72, .81),
    'oak': (.36, .20, .12), 'cedar': (.54, .32, .18), 'iron': (.21, .23, .28),
    'brass': (.72, .47, .19), 'gilt': (.97, .76, .39), 'silk': (.29, .10, .52),
    'velvet': (.49, .20, .71), 'amethyst': (.57, .21, .88),
    'facet': (.84, .53, .99), 'quartz': (.80, .90, .97), 'ice': (.25, .76, .95),
    'amber': (1., .62, .19), 'lamp': (1., .87, .55),
    'lilac': (.68, .39, .93), 'heliotrope': (.38, .12, .68),
    'inclusion': (.93, .72, 1.),
}


def rod(mesh, a, b, radius, material, sides=8):
    cylinder(mesh, a, b, radius, material, sides)


def jewel(mesh, center, radius, height, colors, sides=6, lean=0.):
    """Faceted crystal with a pointed termination and separately shaded prism faces."""
    x, y, z = center
    base = []
    shoulder = []
    for i in range(sides):
        angle = math.tau * i / sides
        dx, dz = radius * math.cos(angle), radius * math.sin(angle)
        base.append([x + dx * .83, y, z + dz * .83])
        shoulder.append([x + dx + lean * .7, y + height * .68, z + dz])
    tip = [x + lean, y + height, z]
    for i in range(sides):
        j = (i + 1) % sides
        normal = [math.cos(math.tau * (i+.5)/sides), 0., math.sin(math.tau * (i+.5)/sides)]
        mesh.polygon([base[i], shoulder[i], shoulder[j], base[j]], colors[i % len(colors)], normal)
        mesh.polygon([shoulder[i], tip, shoulder[j]], colors[(i+1) % len(colors)], [normal[0], .6, normal[2]])
    mesh.polygon(base, colors[0], [0., -1., 0.])


def diamond(mesh, x, y, z, width, height, material, radius):
    points = [(x, y + height, z), (x + width, y, z), (x, y - height, z), (x - width, y, z)]
    for i, point in enumerate(points):
        rod(mesh, point, points[(i+1) % 4], radius, material, 5)


def pennant(mesh):
    rod(mesh, (.05,.93,.49), (.95,.93,.49), .032, 'brass')
    for i in range(8):
        low_x = .17 + i * .0825
        high_x = low_x + .0825
        hem = .045 + abs(i - 3.5) * .045
        wave = .492 + math.sin(i * 1.2) * .014
        mesh.box([low_x, hem, wave-.025], [high_x, .90, wave+.025], 'silk' if i%3 else 'velvet', .004)
    for z in [.450, .533]:
        diamond(mesh, .5, .56, z, .134, .22, 'gilt', .012)
        diamond(mesh, .5, .56, z, .068, .12, 'quartz', .010)
        rod(mesh, (.50,.18,z), (.50,.34,z), .009, 'gilt')
    for x in [.22, .78]:
        mesh.box([x-.014,.22,.45], [x+.014,.89,.53], 'brass', .006)


def cart(mesh):
    mesh.collision.append({'minimum':[.10, .08, .06], 'maximum':[.90, .61, .94]})
    for z in [.26,.73]:
        rod(mesh, (.1,.15,z), (.9,.15,z), .05, 'iron')
        for x in [.14,.86]:
            rod(mesh, (x-.035,.15,z), (x+.035,.15,z), .148, 'iron', 12)
            rod(mesh, (x-.047,.15,z), (x+.047,.15,z), .046, 'brass')
    mesh.box([.105,.235,.075], [.895,.32,.925], 'iron')
    for i in range(3):
        y = .32 + i * .093
        for x in [.11,.83]:
            mesh.box([x,y,.06],[x+.055,y+.085,.94], ('cedar','oak','cedar')[i])
        for z in [.065,.88]:
            mesh.box([.135,y,z],[.865,y+.085,z+.056], ('oak','cedar','oak')[i])
    for x in [.12,.86]:
        for z in [.06,.88]:
            mesh.box([x-.027,.29,z-.014],[x+.027,.625,z+.045],'iron')
            for y in [.34,.55]:
                rod(mesh,(x,y,z-.023),(x,y,z+.061),.019,'brass')
    for i,(x,z,h) in enumerate([(.33,.32,.40),(.61,.44,.51),(.43,.70,.43),(.71,.72,.31)]):
        jewel(mesh,(x,.43,z),.11,h,('amethyst','facet','silk') if i!=2 else ('ice','quartz'),lean=.045)


def lantern(mesh):
    mesh.box([.25,.055,.25],[.75,.15,.75],'brass',.02)
    cylinder(mesh,(.5,.14,.5),(.5,.24,.5),.33,'iron',8,end_radius=.23)
    jewel(mesh,(.50,.20,.50),.208,.49,('lamp','amber','lamp'),8)
    for x in [.31,.69]:
        for z in [.31,.69]:
            rod(mesh,(x,.16,z),(x,.66,z),.021,'brass')
    cylinder(mesh,(.5,.66,.5),(.5,.73,.5),.32,'brass',8,end_radius=.30)
    cylinder(mesh,(.5,.73,.5),(.5,.86,.5),.30,'iron',8,end_radius=.065)
    for z in [.48,.52]:
        diamond(mesh,.5,.905,z,.068,.073,'brass',.014)
    mesh.box([.29,0.,.29],[.71,.055,.71],'iron')


def sigil(mesh):
    mesh.box([.07,.07,.33],[.93,.93,.67],'slate',.07)
    for side in [-1,1]:
        z=.5+side*.185
        diamond(mesh,.50,.50,z,.425,.46,'brass',.034)
        diamond(mesh,.50,.50,z+side*.024,.319,.365,'gilt',.023)
        tip=[.50,.50,z+side*.155]
        corners=[[.5,.83,z],[.738,.49,z],[.5,.17,z],[.262,.49,z]]
        for i,p in enumerate(corners):
            mesh.polygon([p,corners[(i+1)%4],tip],('facet','amethyst','silk','quartz')[i],[0,0,side])


def capital(mesh):
    mesh.collision.append({'minimum':[.05, 0., .05], 'maximum':[.95, 1., .95]})
    mesh.box([.04,0.,.04],[.96,.13,.96],'pearl',.024)
    mesh.box([.10,.13,.10],[.90,.20,.90],'brass')
    for x in [.18,.82]:
        for z in [.18,.82]:
            cylinder(mesh,(x,.20,z),(x,.68,z),.09,'slate',8,end_radius=.12)
            jewel(mesh,(x,.36,z),.105,.34,('amethyst','facet'),5)
    cylinder(mesh,(.5,.2,.5),(.5,.70,.5),.32,'pearl',8,end_radius=.38)
    mesh.box([.07,.69,.07],[.93,.77,.93],'gilt')
    mesh.box([.025,.77,.025],[.975,.93,.975],'pearl',.028)
    mesh.box([.055,.93,.055],[.945,1.,.945],'slate')


def tools(mesh):
    for x in [.19,.82]:
        mesh.box([x-.05,0.,.3],[x+.05,.65,.63],'oak')
    for y in [.12,.52]:
        mesh.box([.06,y,.41],[.94,y+.085,.55],'cedar')
    for i,x in enumerate([.29,.48,.66]):
        y=.81-i*.07
        rod(mesh,(x-.07,.035,.18),(x+.06,y,.42),.028,'cedar')
        if i==1:
            mesh.box([x-.04,y-.025,.35],[x+.17,y+.12,.48],'iron',.026)
        else:
            rod(mesh,(x-.078,y-.04,.42),(x+.048,y+.057,.42),.03,'iron')
            rod(mesh,(x+.048,y+.057,.42),(x+.19,y-.025,.42),.031,'iron')
    mesh.box([.085,.02,.66],[.34,.17,.86],'iron')


def lens(mesh):
    mesh.collision.append({'minimum':[.12, 0., .12], 'maximum':[.88, .19, .88]})
    cylinder(mesh,(.5,0.,.5),(.5,.10,.5),.40,'slate',8)
    cylinder(mesh,(.5,.10,.5),(.5,.19,.5),.32,'gilt',8,end_radius=.23)
    jewel(mesh,(.5,.20,.5),.17,.58,('ice','quartz','facet'),8)
    for tilt in [.55,-.55]:
        points=[]
        for i in range(24):
            a=math.tau*i/24
            points.append((.50+.39*math.cos(a),.54+.34*math.sin(a),.50+tilt*.39*math.cos(a)))
        for i,p in enumerate(points):
            rod(mesh,p,points[(i+1)%24],.018,'brass',5)
    for x in [.16,.84]:
        rod(mesh,(x,.12,.5),(x,.52,.5),.033,'iron')


def prism(mesh, part):
    gems.palette(mesh)
    gems.pillar(mesh,part)
    mesh.collision.append({'minimum':[.16,0.,.16], 'maximum':[.84,.98,.84]})

PROPS = [
    ('crystal_pennant','Crystal Pennant','Kristallwimpel',pennant),
    ('crystal_mine_cart','Crystal Ore Cart','Kristalllore',cart),
    ('crystal_lantern','Highland Lantern','Hochlandlaterne',lantern),
    ('vault_sigil','Faceted Vault Seal','Facettiertes Gewölbesiegel',sigil),
    ('amethyst_capital','Amethyst Column Capital','Amethystkapitell',capital),
    ('mine_tool_rack','Mining Tool Rack','Bergbauwerkzeugständer',tools),
    ('crystal_observatory_lens','Crystal Armillary','Kristallarmillarsphäre',lens),
    ('amethyst_prism_foot','Amethyst Prism Base','Amethystprismenbasis',lambda m: prism(m, 'foot')),
    ('amethyst_prism_shaft','Amethyst Prism','Amethystprisma',lambda m: prism(m, 'shaft')),
    ('amethyst_prism_tip','Amethyst Prism Tip','Amethystprismenspitze',lambda m: prism(m, 'tip')),
]


def build():
    locale=[]
    for block,(slug,name,german,model) in enumerate(PROPS,760):
        mesh=Mesh('slate')
        for color,rgb in PALETTE.items():
            metal = color in ('brass','gilt','iron')
            mesh.materials[color]=(list(rgb),None,.48 if metal else .02,.33 if metal else .71)
        model(mesh)
        textures=gems.export_gem(mesh,slug) if block in (767,768,769) else export_voxels(mesh,slug,('lamp','amber') if slug=='crystal_lantern' else ('facet','ice','inclusion'))
        definition=dict(schema_version=1,id=block,name=name,description=name,creative_category='decoration',
                        render_kind='custom_mesh',model_layout='scene',model=f'models/{slug}.glb',
                        struct_block=True,hardness=1.2,resistance=4.,required_tool=None,mining_level=0,
                        textures=textures,collision_boxes=mesh.collision)
        if slug in ('crystal_lantern','crystal_observatory_lens'):
            lantern_light=slug=='crystal_lantern'
            definition['light']=dict(color=[1.,.64,.28] if lantern_light else [.36,.72,1.],
                                     intensity=560 if lantern_light else 170,range=9. if lantern_light else 5.,
                                     falloff='soft_core',flicker=.012 if lantern_light else 0.,shadows=False)
        if block==769:
            definition['light']=gems.light(1900,14.)
            definition['light']['offset']=[.5,.55,.5]
        if slug == 'vault_sigil':
            definition['light']=dict(color=[.70,.30,1.],intensity=1200,range=10.,
                                     falloff='soft_core',flicker=0.,shadows=False)
        save_json(BLOCKS/f'{slug}.json',definition)
        locale.append(dict(source=name,en=name,de=german))
    save_json(ROOT/'src-ui/locales/source/crystal-landmark-props.json',locale)


if __name__=='__main__':
    build()
