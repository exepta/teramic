#!/usr/bin/env python3
"""Audit every shipped model; bevel safe closed prop cuboids without changing bounds.

Run after the asset builders. Cube/face-group terrain, skinning, animation, thin
surfaces and already-authored curves are intentionally preserved. No Blender
round-trip, global subdivision, texture resampling or material/name rewriting.
"""
import argparse
import json
import math
from pathlib import Path
from model_polish.gltf import Document
from model_polish.bevel import components,box_bounds,replacement

ROOT=Path(__file__).resolve().parents[2]
BLOCKS=ROOT/'assets/terra/blocks'


def policies():
    excluded=set()
    # Mechanical ports, rail surfaces and authored UV tangent frames are exact
    # runtime profiles. Their existing detailed geometry must not be reshaped.
    for path in BLOCKS.glob('*.json'):
        d=json.loads(path.read_text())
        if (d.get('render_kind')=='cube' or d.get('model_layout','face_groups')=='face_groups'
            or d.get('id') in (99,100) or 141<=d.get('id',0)<=172 or path.stem.startswith('rail_')):
            if d.get('model'):excluded.add((BLOCKS/d['model']).resolve())
            for ext in ('glb','gltf'):excluded.add((BLOCKS/'models'/f'{path.stem}.{ext}').resolve())
    return excluded


def inspect(path,apply,excluded,previous):
    doc=Document(path);record=dict(path=str(path.relative_to(ROOT)),triangles_before=0,triangles_after=0,beveled_boxes=0,reason='already_curved_or_thin')
    if path.resolve() in excluded:record['reason']='terrain_seam_or_authored_mechanical_profile_preserved'
    elif doc.doc.get('skins') or doc.doc.get('animations'):record['reason']='animation_or_skinning_preserved'
    elif doc.doc['asset'].get('extras',{}).get('terra_model_polish'):record['reason']='previously_polished'
    allow=record['reason']=='already_curved_or_thin'
    edited=[]
    for mesh in doc.doc.get('meshes',[]):
        for p in mesh['primitives']:
            if p.get('mode',4)!=4:continue
            attrs={key:doc.values(index) for key,index in p['attributes'].items()};positions=attrs['POSITION']
            indices=[v[0] for v in doc.values(p['indices'])] if 'indices' in p else list(range(len(positions)))
            assert len(indices)%3==0 and all(0<=i<len(positions) for i in indices),path
            assert all(math.isfinite(x) for values in attrs.values() for v in values for x in v),path
            before=len(indices)//3;record['triangles_before']+=before;record['triangles_after']+=before
            if not allow or p.get('targets') or any(key.startswith(('JOINTS','WEIGHTS')) for key in attrs):continue
            if any(doc.doc['accessors'][index]['componentType']!=5126 for index in p['attributes'].values()):continue
            if 'TANGENT' in attrs:continue # UV derivative frames need author-controlled bevel mapping.
            replacements=[];removed=set()
            for group in components(positions,indices):
                bounds=box_bounds(positions,indices,group)
                if bounds:
                    replacements.append(replacement(attrs,indices,group,bounds));removed.update(group)
            if not replacements:continue
            # Preserve untouched vertex/index topology, then append closed bevels.
            new={key:list(values) for key,values in attrs.items()};idx=[i for t in range(before) if t not in removed for i in indices[t*3:t*3+3]]
            for added,added_indices in replacements:
                offset=len(new['POSITION']);idx.extend(i+offset for i in added_indices)
                for key,values in added.items():new[key].extend(values)
            # Drop retired cuboid vertices from the new primitive. Keeping them
            # needlessly expands GPU buffers and defeats the authored budgets.
            used=sorted(set(idx));remap={old:i for i,old in enumerate(used)}
            new={key:[values[i] for i in used] for key,values in new.items()}
            idx=[remap[i] for i in idx]
            record['beveled_boxes']+=len(replacements);record['triangles_after']+=len(idx)//3-before
            if apply:
                p.setdefault('extras',{})['terra_polish_source']={'attributes':dict(p['attributes']),'indices':p['indices']}
                for key,values in new.items():p['attributes'][key]=doc.append(values,doc.doc['accessors'][p['attributes'][key]]['type'])
                p['indices']=doc.append([(i,) for i in idx],'SCALAR',5125,34963)
                edited.append(p)
    if record['reason']=='previously_polished' and record['path'] in previous:
        prior=previous[record['path']]
        if prior['triangles_after']==record['triangles_after']:
            return prior
    prior=previous.get(record['path'],{})
    if prior.get('removed_collapsed_triangles') and prior['triangles_after']==record['triangles_after']:
        record['removed_collapsed_triangles']=prior['removed_collapsed_triangles']
        record['triangles_before']=prior['triangles_before']
    if record['beveled_boxes']:
        record['reason']='closed_prop_cuboids_beveled'
        if apply:
            doc.doc['asset'].setdefault('extras',{})['terra_model_polish']=2;doc.save()
    return record


def main():
    parser=argparse.ArgumentParser();parser.add_argument('--apply',action='store_true');args=parser.parse_args()
    excluded=policies();files=sorted(p for p in (ROOT/'assets').rglob('*') if p.suffix in ('.glb','.gltf'))
    out=ROOT/'tools/assets/model_polish/report.json'
    previous={r['path']:r for r in json.loads(out.read_text())} if out.exists() else {}
    report=[inspect(path,args.apply,excluded,previous) for path in files]
    out.write_text(json.dumps(report,indent=2)+'\n')
    print(json.dumps(dict(models=len(report),changed=sum(r['beveled_boxes']>0 for r in report),boxes=sum(r['beveled_boxes'] for r in report),triangles_before=sum(r['triangles_before'] for r in report),triangles_after=sum(r['triangles_after'] for r in report)),indent=2))

if __name__=='__main__':main()
