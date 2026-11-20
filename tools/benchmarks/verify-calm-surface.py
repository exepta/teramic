"""Reconstruct production field from native GPU readback; measure actual zero crossings.
Uses production quantized density rules with the real native pool fixture.
The native screenshots separately validate the actual fragment shader.
"""
import itertools,json,math,sys
from pathlib import Path
import numpy as np
source=Path(sys.argv[1]); report=json.loads(source.read_text())
p=np.asarray(report['gpu_positions'],dtype=np.float32);v=np.asarray(report['gpu_velocities'],dtype=np.float32); live=p[:,3]==0;p=p[live,:3];v=v[live,:3]
quantum=report['mass'][0]/len(p);h=np.clip(.475*(quantum/(1/64))**(1/3),.24,2);radius=max(h*.8,.28);voxel=.125
field=np.zeros(128**3,dtype=np.uint32);activity=np.zeros(128**3,dtype=np.uint8);centers=np.floor(p/voxel).astype(np.int32)
speed=np.linalg.norm(v,axis=1);a=np.clip((speed-.1)/.5,0,1);activity_per=np.rint(a*a*(3-2*a)*255).astype(np.uint8);bound=math.ceil(radius/voxel)
for offset in itertools.product(range(-bound,bound+1),repeat=3):
 c=centers+offset;d=(c+.5)*voxel-p;q=np.einsum('ij,ij->i',d,d)/(radius*radius);weight=np.rint(np.maximum(1-q,0)**3*64).astype(np.uint32);good=(weight>0)&np.all((c>=0)&(c<128),axis=1);c=c[good];i=c[:,0]+128*(c[:,1]+128*c[:,2]);np.add.at(field,i,weight[good]);np.maximum.at(activity,i,activity_per[good])
field=np.minimum(field,255).astype(np.float32)/64;activity=activity.astype(np.float32)/256
# World pool boundaries and base are fixture-authored (not fitted to measured shape).
origin=np.asarray(report['domain']);level=report['pool_surface_y']-origin[1];bottom=level-2.;minimum=np.asarray([8-origin[0],bottom,8-origin[2]]);maximum=np.asarray([25-origin[0],level,25-origin[2]])
def sampled(a,q):
 t=q/voxel-.5;i=np.floor(t).astype(np.int32);f=t-i;result=np.zeros(len(q))
 for bit in itertools.product([0,1],repeat=3):
  b=np.asarray(bit);c=np.clip(i+b,0,127);w=np.prod(np.where(b,f,1-f),axis=1);result+=a[c[:,0]+128*(c[:,1]+128*c[:,2])]*w
 return result
def nearest_activity(q):
 c=np.clip(np.floor(q/voxel).astype(np.int32),0,127);return activity[c[:,0]+128*(c[:,1]+128*c[:,2])]
def rest_nodes(q):
 inside=np.all((q>=minimum)&(q<=maximum),axis=1);distance=np.linalg.norm(q-np.clip(q,minimum,maximum),axis=1);return np.where(inside,np.clip(maximum[1]-q[:,1],.001,1),-np.minimum(distance,1))
def rest(q):
 i=np.floor(q).astype(np.int32);f=q-i;result=np.zeros(len(q))
 for bit in itertools.product([0,1],repeat=3):
  b=np.asarray(bit);w=np.prod(np.where(b,f,1-f),axis=1);result+=rest_nodes(i+b)*w
 return result
sstep=lambda lo,hi,x: (lambda t:t*t*(3-2*t))(np.clip((x-lo)/(hi-lo),0,1))
def corrected(q):
 raw=sampled(field,q);signed=rest(q);lower=q-[0,.5,0];deeper=q-[0,.9,0];support=sstep(.18,.65,np.maximum.reduce([sampled(field,lower),sampled(field,deeper),sampled(field,q-[0,1.3,0])]));filled=sstep(.10,.35,rest(deeper));nearby=1-sstep(.15,.45,-signed);moving=nearest_activity(q);weight=support*filled*nearby*(1-moving);return raw*(1-weight)+np.maximum(0,signed+.55)*weight
# Sample interior away from walls and the near/far ownership handoff.
xs=np.arange(max(minimum[0]+1.5,1.5),min(maximum[0]-1.5,14.5)+.01,.5);zs=np.arange(max(minimum[2]+1.5,1.5),min(maximum[2]-1.5,14.5)+.01,.5);xz=np.asarray(list(itertools.product(xs,zs)));n=len(xz)
def heights(fn):
 result=np.full(n,np.nan);previous=level+1;prev=np.zeros(n)
 for y in np.arange(level+1,bottom+.05,-.02):
  q=np.column_stack((xz[:,0],np.full(n,y),xz[:,1]));d=fn(q);cross=np.isnan(result)&(d>=.55);den=np.maximum(d-prev,1e-9);t=np.clip((.55-prev)/den,0,1);result[cross]=previous+(y-previous)*t[cross];previous=y;prev=d
 return result
summary={}
for name,fn in [('raw_sph',lambda q:sampled(field,q)),('supported_calm',corrected)]:
 heights_value=heights(fn);finite=heights_value[np.isfinite(heights_value)];summary[name]={'samples':n,'missing_surfaces':int(n-len(finite)),'minimum_world_y':float(np.min(finite)+origin[1]),'maximum_world_y':float(np.max(finite)+origin[1]),'height_range_m':float(np.ptp(finite)),'p99_minus_p01_m':float(np.quantile(finite,.99)-np.quantile(finite,.01)),'maximum_level_error_m':float(np.max(np.abs(finite-level)))}
# Guard the other half of the fix: moving and unsupported fluid must not be
# replaced by saved rest volume. These are actual field queries at the pool.
q=np.asarray([[4.,level-.15,8.],[7.,level-.15,8.]])
saved_activity=activity.copy();activity[:]=1.;moving_error=float(np.max(np.abs(corrected(q)-sampled(field,q))));activity[:]=saved_activity
saved_field=field.copy();field[:]=0.;empty_density=float(np.max(corrected(q)));field[:]=saved_field
assert moving_error<1e-6,'Fast fluid was flattened by the rest field'
assert empty_density==0.,'The rest field painted liquid where no particles support it'
summary['moving_field_preservation_error']=moving_error
summary['missing_particle_density']=empty_density
summary['source_gpu_readback']=str(source);summary['radius_m']=float(radius);summary['p99_speed_m_s']=float(np.quantile(speed,.99));print(json.dumps(summary,indent=2));output=source.parent/(source.stem+'-surface-heights.json');output.write_text(json.dumps(summary,indent=2))
assert summary['supported_calm']['missing_surfaces']==0
assert summary['supported_calm']['height_range_m']<.04,'Calm liquid surface is visibly uneven'
assert summary['supported_calm']['maximum_level_error_m']<.04,'Calm volume does not meet authored free surface'
