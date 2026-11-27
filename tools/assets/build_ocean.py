"""Build Teramic's mineable marine materials, seabed life and nautical debris."""
import math
from pathlib import Path
import random

import numpy as np
from PIL import Image

from build_castle import BLOCKS, Mesh
from build_salt_cave import save_json
from voxel_props.surface import export_voxels
from cemetery.geometry import arch, cylinder, ellipsoid

# IDs follow the four existing palm fronds and are part of the save format.
CATALOG = [
    ("wet_sand", "Wet Sand", "Nasser Sand"),
    ("marine_limestone", "Marine Limestone", "Meereskalkstein"),
    ("reef_stone", "Reef Stone", "Riffstein"),
    ("sea_basalt", "Submarine Basalt", "Unterwasserbasalt"),
    ("brain_coral", "Brain Coral", "Hirnkoralle"),
    ("table_coral", "Table Coral", "Tischkoralle"),
    ("staghorn_coral", "Staghorn Coral", "Geweihkoralle"),
    ("organ_pipe_coral", "Organ Pipe Coral", "Orgelkoralle"),
    ("sea_fan", "Violet Sea Fan", "Violetter Seefächer"),
    ("mushroom_coral", "Mushroom Coral", "Pilzkoralle"),
    ("coral_bush", "Coral Thicket", "Korallenbusch"),
    ("luminous_coral", "Luminous Coral", "Leuchtkoralle"),
    ("ribbon_kelp", "Ribbon Kelp", "Bandtang"),
    ("giant_kelp", "Giant Kelp", "Riesentang"),
    ("red_algae", "Red Algae", "Rotalge"),
    ("brown_algae", "Brown Algae", "Braunalge"),
    ("green_algae", "Green Algae", "Grünalge"),
    ("bladderwrack", "Bladderwrack", "Blasentang"),
    ("clam_shell", "Clam", "Venusmuschel"),
    ("reef_oyster", "Reef Oyster", "Riffauster"),
    ("spiral_shell", "Spiral Shell", "Schneckenhaus"),
    ("cone_shell", "Cone Shell", "Kegelschnecke"),
    ("scallop_shell", "Scallop", "Jakobsmuschel"),
    ("starfish", "Starfish", "Seestern"),
    ("sea_urchin", "Sea Urchin", "Seeigel"),
    ("sand_dollar", "Sand Dollar", "Sanddollar"),
    ("marine_driftwood", "Marine Driftwood", "Treibholz"),
    ("wreck_timbers", "Wreck Timbers", "Wrackplanken"),
    ("ship_anchor", "Ship Anchor", "Schiffsanker"),
    ("fishing_net", "Fishing Net", "Fischernetz"),
    ("lobster_pot", "Lobster Pot", "Reusenfalle"),
    ("marker_buoy", "Marker Buoy", "Markierungsboje"),
    ("barnacled_barrel", "Barnacled Barrel", "Bewachsenes Fass"),
    ("sea_crystal", "Sea Crystal", "Meereskristall"),
    ("tidal_boulders", "Tidal Boulders", "Gezeitenfindlinge"),
    ("barnacled_boulder", "Barnacled Boulder", "Seepockenfels"),
    ("reef_pillar", "Reef Pillar", "Riffsäule"),
    ("reef_arch", "Reef Arch", "Riffbogen"),
    ("reef_ledge", "Reef Ledge", "Riffplateau"),
    ("bubble_vent", "Bubbling Sea Vent", "Sprudelquelle"),
]
COLORS = {
    "rock": (.29,.32,.37), "slate": (.22,.26,.31), "moss": (.39,.48,.20),
    "coral": (.93,.23,.20), "salmon": (1,.47,.33), "ruby": (.77,.12,.27),
    "gold": (.97,.62,.14), "peach": (1,.76,.46), "plum": (.55,.20,.65),
    "violet": (.81,.43,.89), "cyan": (.22,.88,.98), "blue": (.20,.53,.95),
    "sand": (.79,.68,.53), "ivory": (.94,.87,.69), "nacre": (.80,.79,.88),
    "kelp": (.33,.53,.11), "lime": (.57,.69,.17), "olive": (.57,.43,.15),
    "oak": (.45,.26,.14), "timber": (.60,.39,.20), "rope": (.76,.57,.32),
    "metal": (.25,.29,.31), "rust": (.57,.30,.17), "shadow": (.16,.15,.18),
}


def substrate(slug, directory):
    size = 256
    y, x = np.mgrid[:size, :size] / size
    rng = np.random.default_rng(117 + len(slug))
    def field(n):
        grid = rng.normal(size=(n, n))
        i, j = (x*n).astype(int), (y*n).astype(int)
        u, v = (x*n) % 1, (y*n) % 1
        u, v = u*u*(3-2*u), v*v*(3-2*v)
        return ((1-v) * ((1-u)*grid[j,i] + u*grid[j,(i+1)%n]) +
                v * ((1-u)*grid[(j+1)%n,i] + u*grid[(j+1)%n,(i+1)%n]))
    broad, mid, grain = field(5), field(14), field(83)
    base = {"wet_sand": (158,126,84), "marine_limestone": (188,182,163),
            "reef_stone": (151,150,137), "sea_basalt": (73,77,87)}[slug]
    rgba = np.array(base, dtype=float) + (broad*4 + mid*4 + grain*4)[:,:,None]
    if slug == "wet_sand":
        # Damp packed grains wrap the bank. Geometry and the upward-facing
        # sand shader carry the tide marks on the walking surface.
        height = mid*1.3 + grain*1.8
    elif slug == "reef_stone":
        # A periodic Voronoi field leaves deep pockets in the calcium framework.
        holes = np.ones(x.shape)
        for cx, cy, r in rng.uniform(size=(51,3)):
            dx = (x-cx+.5)%1-.5
            dy = (y-cy+.5)%1-.5
            holes = np.minimum(holes, (dx*dx*1.5 + dy*dy) / (.014+.030*r)**2)
        height = -34*np.clip(1-holes,0,1) + grain*3
        rgba += ((holes<.37)*-32)[:,:,None]
    else:
        bedding = np.sin(math.tau*(y*3 + broad*.12 + mid*.04))
        height = -7*np.exp(-12*np.abs(bedding)) + mid*4 + grain*2
        if slug == "sea_basalt":
            height -= 17*np.exp(-15*abs(mid+.12*broad))
    prefix = directory / slug
    Image.fromarray((rgba + height[:,:,None]).clip(0,255).astype('uint8')).save(str(prefix)+'.png')
    nx = (np.roll(height,1,1)-np.roll(height,-1,1))*.032
    ny = (np.roll(height,1,0)-np.roll(height,-1,0))*.032
    normal = np.stack((nx,ny,np.ones(nx.shape)),axis=-1)
    normal /= np.linalg.norm(normal,axis=-1,keepdims=True)
    Image.fromarray(((normal*.5+.5)*255).astype('uint8')).save(str(prefix)+'_normal.png')


def rod(mesh, a, b, radius, color, tip=None):
    cylinder(mesh, a, b, radius, color, 4, end_radius=tip)


def pebble(mesh, center, radii, color):
    ellipsoid(mesh, center, radii, color, 8, 3)


def tube(mesh, x, z, height, radius, color):
    cylinder(mesh,[x,.07,z],[x,height,z],radius*.74,color,10,end_radius=radius)
    cylinder(mesh,[x,height-.008,z],[x,height+.001,z],radius*.72,"shadow",10)


def coral(mesh, slug, rng):
    if slug == "brain_coral":
        pebble(mesh,[.5,.19,.5],[.43,.22,.39],"gold")
        for row in range(-3,4):
            z = .5+row*.09
            extent = math.sqrt(1-((z-.5)/.39)**2)*.39
            prev = None
            for n in range(13):
                x = .5 + extent*(n/6-1)
                pz = z + .017*math.sin(n*1.4+row)
                h = .2+.22*math.sqrt(max(0,1-((x-.5)/.45)**2-((pz-.5)/.42)**2))
                ridge = [[x,h-.014,pz-.021],[x,h+.016,pz],[x,h-.014,pz+.021]]
                if prev:
                    for j in range(2):
                        mesh.polygon([prev[j],ridge[j],ridge[j+1],prev[j+1]],"peach",[0,1,0])
                prev = ridge
        return
    if slug in ("table_coral","mushroom_coral"):
        cap = slug == "mushroom_coral"
        for x,z,h,r in [(.49,.5,.61,.32),(.24,.66,.35,.17),(.72,.34,.31,.20)]:
            rod(mesh,[x,.01,z],[x,h,z],r*.18,"ruby",r*.11)
            center = [x,h+.084,z]
            for n in range(16):
                a,b=math.tau*n/16,math.tau*(n+1)/16
                q=lambda t,y,rad:[x+math.cos(t)*rad,y,z+math.sin(t)*rad]
                radius=lambda t:r*(1+.065*math.cos(t*5))
                outer=[q(t,h-(.10 if cap else 0),radius(t)) for t in (a,b)]
                inner=[q(t,h+.046,r*.63) for t in (a,b)]
                col=("violet","plum")[n%2] if cap else ("salmon","peach")[n%2]
                mesh.polygon([center,*inner],col,[0,1,0])
                mesh.polygon([inner[0],*outer,inner[1]],col,[0,1,0])
                mesh.polygon([outer[1],outer[0],[x,h-.03,z]],"ruby",[0,-1,0])
        return
    if slug=="organ_pipe_coral":
        for i in range(9):
            a=i*2.4
            r=.28*math.sqrt(i/8)
            tube(mesh,.5+r*math.cos(a),.5+r*math.sin(a),rng.uniform(.37,.78),rng.uniform(.065,.09),"gold")
        return
    fan=slug=="sea_fan"
    body,tip={"sea_fan":("plum","violet"),"luminous_coral":("blue","cyan"),
              "coral_bush":("ruby","salmon"),"staghorn_coral":("coral","peach")}[slug]
    pebble(mesh,[.5,.055,.5],[.23,.06,.2],"rock")
    for i in range(7 if fan or slug=="coral_bush" else 5):
        a=i*2.4
        dx=(i/6-.5)*.78 if fan else .33*math.sin(a)
        dz=.04*math.cos(i) if fan else .33*math.cos(a)
        y=(.37 if slug=="coral_bush" else .59)+.18*rng.random()
        start=[.5,.08,.5]
        mid=[.5+dx*.53,y*.62,.5+dz*.53]
        end=[.5+dx,y,.5+dz]
        rod(mesh,start,mid,.04,body,.028)
        rod(mesh,mid,end,.028,body,.017)
        for sign in (-1,1):
            branch=[mid[0]+sign*.067,mid[1]+.129,mid[2]+.023]
            rod(mesh,mid,branch,.021,body,.014)
            rod(mesh,branch,[branch[0]+sign*.014,branch[1]+.076,branch[2]],.014,tip,.008)
        ellipsoid(mesh,end,[.021,.026,.021],tip,5,2)


def weed(mesh, slug, rng):
    red=slug=="red_algae"
    color= "ruby" if red else "olive" if slug in ("brown_algae","bladderwrack") else "kelp"
    bright="coral" if red else "gold" if color=="olive" else "lime"
    if slug == "giant_kelp":
        rod(mesh,[.50,0,.50],[.53,.51,.47],.025,"kelp",.018)
        rod(mesh,[.53,.51,.47],[.50,1,.50],.018,"kelp",.025)
    for i in range(4):
        angle = i*2.4
        x,z = .5+.17*math.cos(angle),.5+.17*math.sin(angle)
        h = (.43 if slug in ("red_algae","green_algae","brown_algae") else .87)*rng.uniform(.72,1.0)
        prev=[x,.01,z]
        for j in range(1,5):
            u=j/4
            cur=[x+u*.068*math.sin(angle+3*u),h*u,z+u*.073*math.cos(angle+3*u)]
            rod(mesh,prev,cur,.016,color,.012)
            sign=(-1)**j
            tip=[cur[0]+sign*.13*math.cos(angle),cur[1]+.068,cur[2]+sign*.13*math.sin(angle)]
            mid=[(cur[k]+tip[k])/2 for k in range(3)]
            w=.022 if red else .035
            l=[mid[0]-w*math.sin(angle),mid[1]+.016,mid[2]+w*math.cos(angle)]
            r=[mid[0]+w*math.sin(angle),mid[1]+.016,mid[2]-w*math.cos(angle)]
            for p in [cur,l,tip], [cur,tip,r]:
                mesh.polygon(p, bright if j%2 else color)
                mesh.polygon(p[::-1],color)
            if slug=="bladderwrack" and j%2:
                pebble(mesh,mid,[.024,.035,.024],"gold")
            prev=cur


def shell(mesh, slug):
    if slug in ("starfish","sea_urchin","sand_dollar"):
        star=slug=="starfish"
        urchin=slug=="sea_urchin"
        radius=.16 if urchin else .235 if not star else .137
        color="plum" if urchin else "salmon" if star else "ivory"
        h=.13 if urchin else .056
        pebble(mesh,[.5,h,.5],[radius,h,radius],color)
        for i in range(21 if urchin else 5):
            a=i*2.4 if urchin else i*math.tau/5+.19
            for j in range(2 if urchin else 1):
                def ray(d,y): return [.5+d*math.cos(a),y,.5+d*math.sin(a)]
                if urchin:
                    length=.27 if j==0 else .17
                    rod(mesh,ray(.09,.13),ray(length,.17+j*.16+.048*math.sin(a*37)),.015,"violet",.002)
                elif star:
                    rod(mesh,ray(.067,.065),ray(.35,.025),.087,"salmon",.013)
                    for k in (0,1,2): ellipsoid(mesh,ray(.13+k*.054,.113-k*.018),[.012]*3,"peach",5,2)
                else:
                    rod(mesh,ray(.018,.11),ray(.125,.086),.010,"sand",.006)
        return
    if slug == "cone_shell":
        # Spired cone with alternating bands and a dark, flared aperture.
        for i in range(9):
            end = [(0.24 + i*.047), .21-i*.006, .50]
            start = [end[0]-.047, end[1]+.006, .50]
            cylinder(mesh,start,end,.025+i*.016,"peach" if i%2 else "ivory",12,end_radius=.041+i*.016)
        cylinder(mesh,[.665,.162,.50],[.674,.162,.50],.105,"shadow",12)
        return
    if slug == "spiral_shell":
        previous=None
        for i in range(31):
            t=i/30
            angle=math.tau*2.0*t
            radius=.014+.147*t
            tube_radius=.016+.06*t
            radial=[0,math.sin(angle),math.cos(angle)]
            center=[.70-.34*t,.17+radius*radial[1],.50+radius*radial[2]]
            ring=[]
            for j in range(8):
                phase=math.tau*j/8
                ring.append([center[0]+tube_radius*math.cos(phase),
                             center[1]+tube_radius*math.sin(phase)*radial[1],
                             center[2]+tube_radius*math.sin(phase)*radial[2]])
            if previous:
                for j in range(8):
                    nxt=(j+1)%8
                    normal=[(ring[j][k]+ring[nxt][k])*.5-center[k] for k in range(3)]
                    length=math.sqrt(sum(v*v for v in normal))
                    for face in ([previous[j],ring[j],ring[nxt]],[previous[j],ring[nxt],previous[nxt]]):
                        mesh.polygon(face,("peach","ivory","sand")[(i//3)%3],[v/length for v in normal])
            previous=ring
        mesh.polygon(previous,"shadow")
        return
    oyster=slug=="reef_oyster"
    scallop=slug=="scallop_shell"
    for j in range(14):
        a=math.pi*(.1+.8*j/14); b=math.pi*(.1+.8*(j+1)/14)
        def edge(t,r,y):
            return [.50+r*math.cos(t),y,.28+r*math.sin(t)]
        r=.48 if scallop else .40
        m="nacre" if oyster else ("salmon","peach","ivory")[j%3] if scallop else ("sand","ivory")[j%2]
        mesh.polygon([edge(a,.02,.04),edge(a,r*.64,.15),edge(b,r*.64,.15)],m,[0,1,0])
        mesh.polygon([edge(a,r*.64,.15),edge(a,r,.055+(j%2)*.015),edge(b,r,.055),edge(b,r*.64,.15)],m,[0,1,0])
        mesh.polygon([edge(b,r,.055),edge(a,r,.055+(j%2)*.015),[.5,.023,.43]],"sand",[0,-1,0])
    if oyster:
        pebble(mesh,[.49,.155,.49],[.048,.045,.048],"ivory")


def nautical(mesh, slug):
    if slug in ("marine_driftwood","wreck_timbers"):
        for i in range(3 if slug=="wreck_timbers" else 1):
            z=.34+i*.15
            rod(mesh,[.09,.13,z],[.91,.15+i*.045,z+.06],.084,"timber",.052)
            rod(mesh,[.37,.15,z],[.45,.30,z-.16],.047,"oak",.022)
        if slug=="wreck_timbers":
            rod(mesh,[.28,.043,.16],[.37,.25,.87],.06,"oak")
        return
    if slug=="ship_anchor":
        rod(mesh,[.5,.13,.43],[.5,.79,.57],.040,"metal")
        arch(mesh,[.5,.79,.57],.045,.065,.041,"rust",10)
        rod(mesh,[.31,.65,.54],[.69,.65,.54],.035,"rust")
        for s in (-1,1):
            rod(mesh,[.50,.16,.43],[.5+s*.22,.22,.43],.048,"metal")
            rod(mesh,[.5+s*.22,.22,.43],[.5+s*.32,.42,.43],.044,"rust",.005)
    elif slug=="fishing_net":
        for x in (0.08, 0.92):
            rod(mesh, [x, 0.01, 0.47], [x, 0.98, 0.53], 0.035, "timber")
            for y in (0.76, 0.80, 0.84):
                rod(mesh, [x, y, 0.515], [x, y + 0.025, 0.517], 0.048, "rope")

        def knot(i, j):
            u, v = i / 8, j / 6
            sag = math.sin(math.pi * u)
            return [0.08 + 0.84 * u,
                    0.19 + 0.66 * v - (0.10 + 0.08 * v) * sag,
                    0.50 + sag * (0.04 + 0.065 * (1.0 - v))]

        for i in range(8):
            for j in range(7):
                if (i + j) % 2 == 0:
                    for step in (-1, 1):
                        if 0 <= j + step <= 6:
                            rod(mesh, knot(i, j), knot(i + 1, j + step), 0.011, "rope")
            rod(mesh, knot(i, 6), knot(i + 1, 6), 0.022, "oak")
            rod(mesh, knot(i, 0), knot(i + 1, 0), 0.014, "rope")
        for i in (1, 3, 5, 7):
            x, y, z = knot(i, 6)
            ellipsoid(mesh, (x, y, z), (0.033, 0.055, 0.033), "timber", 6, 3)
            x, y, z = knot(i, 0)
            rod(mesh, [x, y, z], [x, y - 0.035, z], 0.011, "rope")
            pebble(mesh, [x, y - 0.045, z], [0.025, 0.029, 0.021], "metal")
    elif slug=="lobster_pot":
        for z in (.18,.38,.60,.82):
            arch(mesh,[.5,.12,z],.27,.29,.04,"rope",9)
        for i in range(9):
            a=math.pi*i/8
            x,y=.5+.28*math.cos(a),.12+.28*math.sin(a)
            rod(mesh,[x,y,.16],[x,y,.84],.014,"timber")
        mesh.box([.20,.055,.15],[.80,.115,.85],"oak")
    elif slug=="marker_buoy":
        for lo,hi,r,s,c in [(0,.18,.14,.20,"metal"),(.18,.38,.20,.27,"coral"),
                             (.38,.49,.27,.22,"ivory"),(.49,.70,.22,.08,"coral")]:
            cylinder(mesh,[.5,lo,.5],[.5,hi,.5],r,c,12,end_radius=s)
        arch(mesh,[.5,.70,.5],.044,.075,.04,"rust")
    else:
        for lo,hi,r,s,c in [(.01,.14,.22,.26,"oak"),(.14,.20,.27,.28,"metal"),
                             (.20,.57,.27,.27,"timber"),(.57,.63,.28,.27,"metal"),
                             (.63,.74,.26,.22,"oak")]:
            cylinder(mesh,[.5,lo,.5],[.5,hi,.5],r,c,12,end_radius=s)
        for i in range(13):
            a=i*2.4
            pebble(mesh,[.5+.26*math.cos(a),.19+.08*(i%4),.5+.26*math.sin(a)],[.032]*3,"ivory")


def stones(mesh, slug, rng):
    if slug=="sea_crystal":
        for i in range(7):
            a=i*2.4
            r=.24*math.sqrt(i/7)
            x,z=.5+r*math.cos(a),.5+r*math.sin(a)
            h=.81-r*1.7
            radius=.09+r*.08
            cylinder(mesh,[x,.01,z],[x,h*.77,z],radius,"blue",6,end_radius=radius*.88)
            cylinder(mesh,[x,h*.77,z],[x,h,z],radius*.88,"cyan",6,end_radius=.001)
        return
    if slug=="reef_arch":
        arch(mesh,[.50,.14,.50],.27,.46,.31,"rock",8)
        for x in (.13,.87):
            mesh.box([x-.098,.01,.32],[x+.098,.23,.68],"slate")
    elif slug in ("reef_ledge","reef_pillar"):
        pillar=slug=="reef_pillar"
        for i in range(4 if pillar else 2):
            width=(.25-.025*i) if pillar else (.31+.11*i)
            y=.01+i*.19
            mesh.box([.50-width,y,.50-width*.69],[.50+width,y+.21,.50+width*.69],"rock" if i%2 else "slate",.035)
        width=.22 if pillar else .42
        y=.79 if pillar else .40
        mesh.box([.50-width,y,.50-width*.68],[.50+width,y+.035,.50+width*.68],"moss",.01)
    elif slug=="bubble_vent":
        for i in range(5):
            a=i*2.4
            tube(mesh,.5+.17*math.cos(a),.5+.17*math.sin(a),.18+.045*(i%3),.078,"slate")
    else:
        count=5 if slug=="tidal_boulders" else 2
        for i in range(count):
            a=i*2.4
            r=.31*math.sqrt(i/count)
            size=.21+.075*rng.random()
            pebble(mesh,[.5+r*math.cos(a),size*.66,.5+r*math.sin(a)],
                   [size,size*.72,size*.87],"rock" if i%2==0 else "slate")
        if slug=="barnacled_boulder":
            for i in range(13):
                a=i*2.4
                d=.04+.13*math.sqrt(i/13)
                x,z=.5+d*math.cos(a),.5+d*math.sin(a)
                h=.28+.04*math.sin(a)
                rod(mesh,[x,h-.075,z],[x,h,z],.033,"ivory",.021)
                pebble(mesh,[x,h,z],[.014,.006,.014],"shadow")


DESCRIPTIONS = [
    ("Fine wet grains bear parallel tide marks.", "Feine nasse Körner tragen parallele Gezeitenspuren."),
    ("Pale limestone laid down in an ancient sea.", "Heller Kalkstein aus einem urzeitlichen Meer."),
    ("A porous reef skeleton cut by holes and channels.", "Ein poröses Riffskelett voller Löcher und Kanäle."),
    ("Dark volcanic rock fractured beneath the waves.", "Dunkles, unter Wasser zerklüftetes Vulkangestein."),
    ("Golden ridges wind across a domed coral colony.", "Goldene Rillen winden sich über eine gewölbte Korallenkolonie."),
    ("Overlapping orange plates catch the sunlight.", "Überlappende orangefarbene Teller fangen das Sonnenlicht ein."),
    ("Pale tips crown sturdy red coral branches.", "Helle Spitzen krönen kräftige rote Korallenäste."),
    ("A cluster of hollow golden coral tubes.", "Eine Gruppe hohler goldener Korallenröhren."),
    ("A broad fan of violet branches.", "Ein breiter Fächer aus violetten Ästen."),
    ("Frilled caps shade a colony of pink polyps.", "Gekrauste Hüte beschatten eine Kolonie rosa Polypen."),
    ("Short vermilion branches grow in a dense thicket.", "Kurze zinnoberrote Zweige bilden ein dichtes Gebüsch."),
    ("Azure coral tips glow on the reef at dusk.", "Azurfarbene Korallenspitzen leuchten in der Dämmerung."),
    ("Fine green blades rise from a rocky holdfast.", "Feine grüne Blätter sprießen aus einer felsigen Haftwurzel."),
    ("Long leafy stems form connected kelp forests.", "Lange belaubte Stängel bilden zusammenhängende Tangwälder."),
    ("Small scarlet fronds cluster on the seabed.", "Kleine scharlachrote Wedel wachsen auf dem Meeresboden."),
    ("Olive and amber blades grow in cold currents.", "Olivgrüne und bernsteinfarbene Blätter wachsen in kalten Strömungen."),
    ("A tuft of fresh green seaweed.", "Ein Büschel frischer grüner Meeresalgen."),
    ("Golden floats lift olive blades toward the light.", "Goldene Schwimmblasen heben olivgrüne Blätter zum Licht."),
    ("A ridged bivalve rests on the sand.", "Eine gerippte Muschel ruht auf dem Sand."),
    ("A rough oyster shell with a pearly lining.", "Eine raue Austernschale mit perlmuttfarbener Innenseite."),
    ("A coiled, striped sea snail shell.", "Ein gewundenes, gestreiftes Meeresschneckenhaus."),
    ("Ivory bands wrap a tapering shell.", "Elfenbeinfarbene Bänder umziehen ein spitzes Schneckenhaus."),
    ("Pink radial ribs fan out from the hinge.", "Rosa Rippen fächern vom Scharnier aus auf."),
    ("Five orange arms spread across the seafloor.", "Fünf orangefarbene Arme liegen auf dem Meeresboden."),
    ("Violet spines protect a round sea urchin.", "Violette Stacheln schützen einen runden Seeigel."),
    ("Five pale petals mark a flattened shell.", "Fünf helle Blätter zieren eine flache Kalkschale."),
    ("Salt-worn wood washed from a distant shore.", "Salzgebleichtes Holz von einem fernen Ufer."),
    ("Broken timbers from a stranded hull.", "Gebrochene Balken eines gestrandeten Rumpfs."),
    ("A rusted iron anchor with two curved flukes.", "Ein rostiger Eisenanker mit zwei gebogenen Flunken."),
    ("A diamond-meshed net slung between timber posts.", "Ein rautenförmig geknüpftes Netz zwischen Holzpfählen."),
    ("An arched wicker cage for catching shellfish.", "Eine gewölbte Korbreuse zum Krebsfang."),
    ("A striped float marking a submerged mooring.", "Eine gestreifte Boje markiert einen versunkenen Liegeplatz."),
    ("Pale barnacles cling to a hooped oak cask.", "Helle Seepocken haften an einem eisenbereiften Eichenfass."),
    ("A spray of luminous blue marine crystals.", "Eine Gruppe leuchtend blauer Meereskristalle."),
    ("Rounded stones piled by a strong tide.", "Von starker Strömung aufgehäufte runde Steine."),
    ("Barnacles encrust the face of a sea-worn boulder.", "Seepocken bedecken die Flanke eines vom Meer geformten Felsens."),
    ("Algae cling to a layered column of reef rock.", "Algen haften an einer geschichteten Säule aus Riffgestein."),
    ("Water has worn a passage through the reef.", "Das Wasser hat einen Durchgang durch das Riff geschliffen."),
    ("A mossy stone ledge overlooking a reef.", "Ein moosiges Felsplateau über einem Riff."),
    ("Bubbles rise from holes in the submerged stone.", "Luftblasen steigen aus Öffnungen im versunkenen Stein auf."),
]


def build():
    directory=BLOCKS/"textures/ocean"
    directory.mkdir(exist_ok=True)
    entries=[]
    for number,(slug,name,german) in enumerate(CATALOG,541):
        description,translation = DESCRIPTIONS[number-541]
        spec=dict(schema_version=1,id=number,name=name,description=description,creative_category="nature",
                  hardness=1.6 if number<545 else .45,resistance=2.5 if number<545 else .7,
                  mining_level=0,required_tool=None)
        if number<=544:
            substrate(slug,directory)
            tex=f"textures/ocean/{slug}"
            spec.update(render_kind="cube",bevel_width=0,textures={slug:tex+".png"},
                        face_materials=[slug]*6,materials={slug:dict(roughness=.72,normal_texture=tex+"_normal.png")},
                        surface="sand" if number==541 else "rock")
        else:
            mesh=Mesh("stone")
            for color,rgb in COLORS.items():
                mesh.materials[color]=(list(rgb),None,0,.75)
            rng=random.Random(932+number)
            if number<=552:
                coral(mesh,slug,rng)
            elif number<=558:
                weed(mesh,slug,rng)
            elif number<=566:
                shell(mesh,slug)
            elif number<=573:
                nautical(mesh,slug)
            else:
                stones(mesh,slug,rng)
            # Seat the mesh on its actual floor, keeping all decorative tips inside one cell.
            points=[p for primitive in mesh.parts.values() for p in primitive["position"]]
            lo=[min(p[i] for p in points) for i in range(3)]
            hi=[max(p[i] for p in points) for i in range(3)]
            scale=min(1.,.992/max(hi[i]-lo[i] for i in range(3)))
            for p in {id(point):point for point in points}.values():
                for i in (0,2):
                    p[i]=.5+(p[i]-(lo[i]+hi[i])/2)*scale
                p[1]=(p[1]-lo[1])*scale
            glow=("cyan","blue") if number in (552,574) else ()
            textures=export_voxels(mesh,slug,glow)
            boxes=[]
            if number in (545,546,547,548,575,576,577,579):
                boxes=[dict(minimum=[.20,0,.23],maximum=[.80,min(.47,(hi[1]-lo[1])*scale),.77])]
            if number==578:
                boxes=[dict(minimum=[x,0,.35],maximum=[x+.15,.36,.65]) for x in (.08,.77)]
            spec.update(render_kind="custom_mesh",model_layout="scene",model=f"models/{slug}.glb",
                        collision_boxes=boxes,textures=textures)
            if glow:
                spec["light"]=dict(color=[.19,.65,.96],intensity=62,range=3.5,shadows=False)
        save_json(BLOCKS/(slug+".json"),spec)
        entries.extend([dict(source=name,en=name,de=german),dict(source=description,en=description,de=translation)])
    save_json(Path(__file__).resolve().parents[2]/"src-ui/locales/source/ocean.json",entries)


if __name__=="__main__":
    build()
