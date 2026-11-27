"""Build the moonlit Spirit Grove canopy, hanging racemes, geodes and rune stones."""
from math import cos, sin, tau
from pathlib import Path
from biome_surfaces import foliage, leaf_clusters
from build_castle import BLOCKS, Mesh
from build_salt_cave import save_json
from voxel_props.surface import export_voxels
from cemetery.geometry import cylinder, ellipsoid

DATA = [
    ("teal_spirit_leaves", "Teal Spirit Leaves", "Türkises Geisterlaub",
     "Jade leaves carrying sprays of pale turquoise blossom.", "Jadegrüne Blätter mit helltürkisen Blütenrispen."),
    ("moonlit_spirit_leaves", "Moonlit Spirit Leaves", "Mondlichtlaub",
     "Blue-silver leaves tipped with lilac blossoms.", "Silberblaue Blätter mit fliederfarbenen Blütenspitzen."),
    ("wisteria_tassel", "Wisteria Tassel", "Glyziniengirlande",
     "Pendant racemes with violet bells and pearl tips.", "Hängende Blütentrauben mit violetten Glocken und perlmuttfarbenen Spitzen."),
    ("jade_spirit_vine", "Jade Spirit Vine", "Geisterjaderanke",
     "Paired jade leaflets cascade around an evergreen creeper.", "Jadegrüne Blattpaare ranken sich um eine immergrüne Liane."),
    ("spirit_geode", "Spirit Geode", "Geistergeode",
     "Lilac crystal spires rise from a mossy bed of amethyst.", "Fliederfarbene Kristallspitzen ragen aus einem moosigen Amethystbett."),
    ("wisp_mushrooms", "Wisp Mushrooms", "Irrlichtpilze",
     "Luminous turquoise caps grow in the shade beside spirit pools.", "Leuchtend türkise Pilzhüte wachsen im Schatten der Geisterteiche."),
    ("rune_pillar", "Grove Runestone", "Runenstele",
     "A weathered stele inlaid with a turquoise spiral.", "Eine verwitterte Stele mit einer türkis leuchtenden Spirale."),
]
PROFILES = [
    (606, DATA[0][0], "spirit_teal", [54, 158, 137], [132, 229, 202]),
    (607, DATA[1][0], "spirit_moonlit", [108, 156, 192], [204, 203, 241]),
]
PIGMENT = {
    "stem": [.20,.29,.23], "jade": [.07,.48,.34], "mint": [.35,.78,.59],
    "bell": [.59,.36,.77], "lilac": [.80,.62,.90], "pearl": [.98,.88,.96],
    "rock": [.32,.35,.43], "slate": [.48,.49,.59], "moss": [.37,.50,.19],
    "amethyst": [.48,.31,.70], "facet": [.75,.67,.94], "glow": [.32,.92,.91],
    "gill": [.65,.93,.84], "cap": [.14,.65,.64], "teal": [.10,.49,.53],
}


def hanging(mesh, floral):
    # The central stem meets its counterpart above/below at the same X/Z.
    cylinder(mesh, [.5,1,.5], [.5,0,.5], .023, "stem", 5, end_radius=.019)
    for i in range(3):
        a = .6 + tau*i/3
        sx, sz = cos(a), sin(a)
        top = [.5,.98-i*.16,.5]
        elbow = [.5+sx*.22,top[1]-.13,.5+sz*.22]
        cylinder(mesh, top, elbow, .019, "stem", 5, end_radius=.013)
        cylinder(mesh, elbow, [elbow[0]+sx*.043, elbow[1]-.39, elbow[2]+sz*.043], .012, "stem", 4, end_radius=.005)
        for j in range(4):
            f = 1-j*.19
            x, y, z = elbow[0]+sx*j*.014, elbow[1]-.072-j*.102, elbow[2]+sz*j*.014
            if floral:
                ellipsoid(mesh, [x,y,z], [.078*f,.102*f,.078*f],
                          ["bell","bell","lilac","pearl"][j], 5, 3)
                ellipsoid(mesh, [x+.04*f,y-.029*f,z+.022*f], [.048*f,.056*f,.048*f], "lilac", 4, 2)
            else:
                side = 1 if j % 2 else -1
                tip = [x+(.10*sx+.065*sz*side)*f, y-.12*f, z+(.10*sz-.065*sx*side)*f]
                cylinder(mesh, [x,y+.055,z], tip, .073*f, "mint" if j % 3 == 0 else "jade", 4, end_radius=.003)


def model(slug):
    mesh = Mesh("stone")
    for key, rgb in PIGMENT.items():
        mesh.materials[key] = (rgb, None, 0, .40 if key in ("facet", "amethyst") else .85)
    if slug in ("wisteria_tassel", "jade_spirit_vine"):
        hanging(mesh, slug == "wisteria_tassel")
    elif slug == "spirit_geode":
        ellipsoid(mesh, [.5,.09,.5], [.43,.09,.34], "rock", 8, 4)
        for i, (dx,dz,h,r) in enumerate([(-.25,-.1,.39,.095), (.02,.06,.82,.13), (.19,.14,.51,.11), (.17,-.17,.34,.085),(-.13,.14,.26,.068)]):
            foot = [.5+dx*.8,.10,.5+dz*.8]
            shoulder = [.5+dx,h*.76,.5+dz]
            cylinder(mesh, foot, shoulder, r, "amethyst", 5, end_radius=r*.85)
            cylinder(mesh, shoulder, [.5+dx*1.1,h,.5+dz*1.1], r*.85, "facet" if i % 2 == 0 else "lilac", 5, end_radius=.003)
    elif slug == "wisp_mushrooms":
        for i,(x,z,h,r) in enumerate([(.37,.44,.57,.23),(.68,.64,.37,.172),(.66,.24,.21,.113)]):
            cylinder(mesh, [x+.035,0,z+.02], [x,h*.86,z], r*.17, "gill", 6, end_radius=r*.13)
            ellipsoid(mesh,[x,h*.85,z],[r,r*.24,r*.90],"gill",9,4)
            ellipsoid(mesh,[x,h*.91,z],[r,r*.46,r*.90],"teal" if i==1 else "cap",9,5)
            for j in range(5):
                a=tau*j/5+i
                ellipsoid(mesh,[x+cos(a)*r*.62,h*.91+r*.34,z+sin(a)*r*.55],
                          [r*.095,r*.04,r*.088],"glow",5,3)
    else:
        mesh.box([.16,0,.15],[.84,.15,.85],"slate",.04,True)
        mesh.box([.23,.13,.27],[.77,.86,.73],"rock",.045,True)
        mesh.box([.20,.85,.22],[.80,.97,.78],"slate",.033,True)
        for j in range(3):
            mesh.box([.21+j*.13,.96,.25+j*.03],[.36+j*.13,.99,.41+j*.06],"moss",.008)
        for face in (.265,.735):
            points = []
            for i in range(24):
                a = .43*tau+i/23*tau*1.45
                r=.19-i/23*.156
                points.append([.50+cos(a)*r,.53+sin(a)*r,face])
            for a,b in zip(points,points[1:]):
                cylinder(mesh,a,b,.014,"glow",5)
    if slug == "wisp_mushrooms":
        base = min(y for part in mesh.parts.values() for _,y,_ in part["position"])
        for part in mesh.parts.values():
            part["position"] = [[x,y-base,z] for x,y,z in part["position"]]
    luminous = ("facet", "lilac") if slug == "spirit_geode" else ("glow", "gill")
    return export_voxels(mesh,slug,luminous), mesh.collision


def build():
    locale = []
    for ident,(slug,en,de,description,translation) in enumerate(DATA,606):
        block = dict(schema_version=1,id=ident,name=en,description=description,
                     creative_category="nature",mining_level=0,required_tool=None,hardness=.35,resistance=.6)
        if ident <= 607:
            style = PROFILES[ident-606][2]
            block.update(render_kind="cube",bevel_width=0,
                         materials={key:dict(base_color=[1,1,1,1],roughness=.92,reflectance=.09)
                                    for key in ("side","top","spray")},
                         textures={key:f"textures/foliage/{style}_{face}.png"
                                   for key,face in [("side","side"),("top","top"),("spray","cluster")]},
                         face_materials=["side","side","side","top","side","side"],
                         foliage_cluster=dict(material="spray"))
        else:
            textures, boxes = model(slug)
            block.update(render_kind="custom_mesh",model_layout="scene",model=f"models/{slug}.glb",
                         struct_block=True,textures=textures,collision_boxes=boxes)
            if ident == 612:
                block.update(creative_category="decoration",hardness=1.5,resistance=2.3,
                             light=dict(color=[.38,.86,.91],intensity=130,range=5,shadows=False))
        save_json(BLOCKS/f"{slug}.json",block)
        for src,tr in [(en,de),(description,translation)]:
            locale.append(dict(source=src,en=src,de=tr))
    save_json(Path(__file__).resolve().parents[2]/"src-ui/locales/source/spirit-grove.json",locale)
    folder = BLOCKS/"textures/foliage"
    foliage.export(folder,PROFILES)
    leaf_clusters.export(folder,PROFILES)


if __name__ == "__main__":
    build()
