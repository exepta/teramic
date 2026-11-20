#!/usr/bin/env python3
"""Reproducible fantasy paving, tied straw and bile-coated stone; shared PBR maps."""
import json, math
from pathlib import Path
from PIL import Image
ROOT=Path(__file__).resolve().parents[2]
BLOCKS=ROOT/'assets/terra/blocks'; TEX=BLOCKS/'textures/street'
SIZE=256

def write(path,data):
 path.parent.mkdir(parents=True,exist_ok=True);path.write_text(json.dumps(data,indent=2,ensure_ascii=False)+'\n')
def noise(x,y,seed=1):
 n=(int(x)*374761393+int(y)*668265263+seed*3266489917)&0xffffffff
 n=((n^(n>>13))*1274126177)&0xffffffff
 return ((n^(n>>16))&65535)/65535

def pixel(kind,x,y):
 # Shared repeat endpoints and periodic sculpted patterns preserve adjoining faces.
 x%=255;y%=255
 if kind=='paving':
  row=int(y/51);xx=(x+(row%2)*25.5)%255;u=xx%63.75;v=y%51
  edge=min(u,63.75-u,v,51-v);seed=noise(int(xx/63.75),row,71)
  rim=max(0,min(1,edge/3));lip=max(0,1-abs(edge-4.2)/2.5)
  base=[137+seed*21,126+seed*19,119+seed*18]
  moss=noise(int(x/12),int(y/12),5)>.61 and edge<3.3
  if moss:base=[91,108,47]
  shade=.56+.40*rim+.08*lip+(noise(x,y,3)-.5)*.055
  return [int(c*shade) for c in base], .28+rim*.18+lip*.018
 if kind=='straw':
  # Irregular, fine stalk bundles; two raised rope courses around the bale.
  strand=(x+math.sin(y*math.tau/255)*2.1)%8.5
  groove=min(strand,8.5-strand);seed=noise(int(x/8.5),int(y/40),9)
  base=[212+seed*25,145+seed*25,53+seed*20]
  shade=.75+.23*min(1,groove/1.3)+(noise(x,y,11)-.5)*.09
  rope=min(abs(y-69),abs(y-187))
  height=.40+min(1,groove/2)*.05
  if rope<9:
   twist=math.sin((x+y*1.5)*math.tau/12)*.055
   shade=.80+.15*math.cos(rope/9*math.pi/2)+twist
   base=[151,106,55];height=.47+.045*math.cos(rope/9*math.pi/2)
  return [int(c*shade) for c in base],height
 if kind=='bile':
  # Periodic rounded pools over muted, finely pitted mineral substrate.
  # Independent droplets replace the old striped sine/vine pattern.
  pitch=85;cx=int(x/pitch);cy=int(y/pitch);field=0;air=0
  for j in range(cy-1,cy+2):
   for i in range(cx-1,cx+2):
    seed=noise(i%3,j%3,211)
    px=(i+.20+noise(i%3,j%3,213)*.60)*pitch
    py=(j+.20+noise(i%3,j%3,217)*.60)*pitch
    r=pitch*(.59+seed*.37)
    d=math.hypot((x-px)*(.88+seed*.22)+math.sin(y*math.tau/255)*6,y-py+math.sin(x*math.tau/255)*5)
    field+=max(0,1-(d/r)**2)**2
    # Small trapped amber bubbles, kept inside their parent liquid pool.
    air=max(air,max(0,1-math.hypot(x-px+r*.18,y-py+r*.23)/(r*.17)))
  wet=max(0,min(1,(field-.10)*5))
  mineral=noise(int(x/6),int(y/6),41)
  pit=noise(int(x/3),int(y/3),43)
  rock=[95+mineral*30,87+mineral*24,65+mineral*17]
  sheen=max(0,min(1,field*.85))
  gel=[155+sheen*37,149+sheen*32,20+sheen*10]
  # Meniscus and tiny bubbles read through the matching tangent-space normal.
  shade=.82+.15*sheen+.04*noise(x,y,53)
  if air>.30:shade*=.95+.05*air
  rgb=[int(rock[k]*(1-wet)+gel[k]*shade*wet) for k in range(3)]
  return rgb,.31+mineral*.016-pit*.009+wet*(.11+sheen*.105)+air*.018
 raise ValueError(kind)

def textures():
 TEX.mkdir(parents=True,exist_ok=True)
 for kind in ['paving','straw','bile']:
  values=[pixel(kind,x,y) for y in range(SIZE) for x in range(SIZE)]
  image=Image.new('RGBA',(SIZE,SIZE));image.putdata([tuple(rgb)+(255,) for rgb,h in values]);image.save(TEX/(kind+'.png'))
  normal=[]
  for y in range(SIZE):
   for x in range(SIZE):
    xx=x%255;yy=y%255
    dx=(values[yy*SIZE+(xx+1)%255][1]-values[yy*SIZE+(xx-1)%255][1])*10
    dy=(values[((yy+1)%255)*SIZE+xx][1]-values[((yy-1)%255)*SIZE+xx][1])*10
    length=math.sqrt(dx*dx+dy*dy+1)
    normal.append((round((.5-dx/length*.5)*255),round((.5-dy/length*.5)*255),round((.5+.5/length)*255),255))
  image=Image.new('RGBA',(SIZE,SIZE));image.putdata(normal);image.save(TEX/(kind+'_normal.png'))

def box(lo,hi):return dict(minimum=lo,maximum=hi)
def definition(id,slug,name,texture,shapes=None):
 d=dict(schema_version=1,id=id,name=name,description={'bile':'Acid-coated, pitted stone. Contact burns creatures and players.','straw':'A tightly tied bale of golden straw with woven rope courses.','paving':'Carved fantasy paving with fine joints and moss in sheltered seams.'}[texture],creative_category='building',render_kind='boxes' if shapes else 'cube',bevel_width=.015 if shapes else .022,hardness=1.6 if texture!='straw' else .7,resistance=5 if texture!='straw' else .8,required_tool='pickaxe' if texture!='straw' else None,mining_level=0,
  materials={'Surface':dict(base_color=[1,1,1,1],roughness=.24 if texture=='bile' else .94,reflectance=.45 if texture=='bile' else .12,normal_texture='textures/street/'+texture+'_normal.png')},textures={'Surface':'textures/street/'+texture+'.png'},face_materials=['Surface']*6)
 if shapes:
  d['collision_boxes']=shapes
  if 442<=id<=447:
   # The curb is a darker part of the same walking surface, not a raised rail.
   d['materials']['Deck']=dict(d['materials']['Surface'])
   d['textures']['Deck']='textures/street/decks/'+slug+'.png'
   d['face_materials'][3]='Deck'
 else:d['surface']='mushroom' if texture=='bile' else 'rock' if texture!='straw' else 'wood'
 write(BLOCKS/(slug+'.json'),d)
 return d

def shapes():
 result={441:[box([0,0,0],[1,.5,1])]}
 for id in (442,443,444):
  result[id]=[box([0,0,0],[1,.25,1])]
 # Equal angular intervals bound the contact error even at the tangencies.
 for id in (445,446):
  strips=[]
  for i in range(32):
   lo=1-math.cos(math.pi*i/64)
   hi=1-math.cos(math.pi*(i+1)/64) if i<31 else 1.
   span=math.sin(math.pi*(i+.5)/64)
   strips.append(box([1-span if id==445 else 0,0,lo],[1 if id==445 else span,.25,hi]))
  result[id]=strips
 result[447]=[box([0,0,i/16],[1,.125+.125*(i+1)/16,(i+1)/16]) for i in range(16)]
 return result

def deck_edge(id,x,z):
 if id in [442,447]:return z<.18
 if id==443:return z<.18 or x<.18
 if id==444:return z<.18 or x>.82
 center=1. if id==445 else 0.
 return .82<=math.hypot(x-center,z-1.)<=1.

def deck_textures(entries):
 dest=TEX/'decks';dest.mkdir(parents=True,exist_ok=True)
 for id,slug,_ in entries:
  if not 442<=id<=447:continue
  values=[]
  for y in range(SIZE):
   for x in range(SIZE):
    rgb,_=pixel('paving',x,y)
    # A one-pixel repeat gutter lies inside the existing bevel and keeps mips
    # from importing a dark stripe onto the opposite end of a block.
    gutter=min(1.,min(x,y,255-x,255-y)/2.)
    shade=1.-.24*gutter if deck_edge(id,x/255,y/255) else 1.
    values.append(tuple(round(c*shade) for c in rgb)+(255,))
  image=Image.new('RGBA',(SIZE,SIZE));image.putdata(values);image.save(dest/(slug+'.png'))

def recipes(entries):
 dest=ROOT/'assets/crafting/building/street'
 write(dest/'sidewalk_block.json',dict(id='teramic::sidewalk_block',name='Carve Sidewalk Pavers',station='workbench',output=dict(item={'block':441},count=4),pattern=dict(kind='shapeless',ingredients=[dict(item={'block':3},count=3),dict(item={'block':42},count=1)])))
 # Each outline occupies distinct grid slots, so a stack for a curve never
 # matches the recipe for a straight paver first.
 outlines = {
  442: ("#",), 443: ("#", "#"), 444: ("###",),
  445: ("#  ", "###"), 446: (" #", "##", "##"), 447: ("###", "###"),
 }
 for material, slug, name in entries[1:-1]:
  rows = outlines[material]
  cells = [dict(item={'block':441}, count=1) if mark == '#' else None for row in rows for mark in row]
  count = sum(cell is not None for cell in cells)
  pattern = dict(kind='shaped', width=len(rows[0]), height=len(rows), cells=cells)
  write(dest/(slug+'.json'),dict(id='teramic::'+slug, name=name, station='workbench', output=dict(item={'block':material},count=max(2,count)), pattern=pattern))
 bale = dict(kind='shaped', width=3, height=3, cells=[dict(item={'crop':'wheat'}, count=1) for _ in range(9)])
 write(dest/'straw_block.json',dict(id='teramic::straw_block',name='Tie Straw Bale',station='workbench',output=dict(item={'block':448},count=1),pattern=bale))
 write(dest/'untie_straw.json',dict(id='teramic::untie_straw',name='Untie Straw Bale',station='workbench',output=dict(item={'crop':'wheat'},count=9),pattern=dict(kind='shapeless',ingredients=[dict(item={'block':448},count=1)])))
 write(dest/'bile_block.json',dict(id='teramic::bile_block',name='Coat Stone with Galle',station='workbench',output=dict(item={'block':440},count=1),pattern=dict(kind='shapeless',ingredients=[dict(item={'block':3},count=1),dict(item={'tool':'bile_bucket'},count=1)])))

def main():
 textures();definition(440,'bile_block','Galle Block','bile')
 entries=[(441,'sidewalk_block','Sidewalk Block'),(442,'sidewalk','Sidewalk'),(443,'sidewalk_left','Sidewalk Left Edge'),(444,'sidewalk_right','Sidewalk Right Edge'),(445,'sidewalk_arc_left','Sidewalk Left Curve'),(446,'sidewalk_arc_right','Sidewalk Right Curve'),(447,'sidewalk_lowered','Lowered Sidewalk'),(448,'straw_block','Straw Block')]
 deck_textures(entries)
 profiles=shapes()
 for id,slug,name in entries:definition(id,slug,name,'straw' if id==448 else 'paving',profiles.get(id))
 recipes(entries)
 names={'Galle Block':'Galleblock','Sidewalk Block':'Bürgersteigblock','Sidewalk':'Bürgersteig','Sidewalk Left Edge':'Bürgersteig – Kante links','Sidewalk Right Edge':'Bürgersteig – Kante rechts','Sidewalk Left Curve':'Bürgersteig – Bogen links','Sidewalk Right Curve':'Bürgersteig – Bogen rechts','Lowered Sidewalk':'Abgesenkter Bürgersteig','Straw Block':'Strohblock','Carve Sidewalk Pavers':'Bürgersteigplatten behauen','Tie Straw Bale':'Strohballen binden','Untie Straw Bale':'Strohballen lösen','Coat Stone with Galle':'Stein mit Galle überziehen'}
 write(ROOT/'src-ui/locales/source/street-blocks.json',[dict(source=k,en=k,de=v) for k,v in names.items()])
 print('Generated bile stone, pavers, six sidewalk profiles and tied straw with shared normal maps.')
if __name__=='__main__':main()
