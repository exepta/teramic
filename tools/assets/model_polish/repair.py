"""Remove provably invisible static triangles without rewriting vertex data."""
from .gltf import Document


def remove_collapsed_faces(path):
    document=Document(path)
    if document.doc.get('skins') or document.doc.get('animations'):
        raise ValueError('Animated/rigged models require an artist review')
    removed=0
    for mesh in document.doc.get('meshes',[]):
        for primitive in mesh['primitives']:
            if primitive.get('mode',4)!=4 or primitive.get('targets'):
                continue
            positions=document.values(primitive['attributes']['POSITION'])
            indices=[i[0] for i in document.values(primitive['indices'])] if 'indices' in primitive else list(range(len(positions)))
            kept=[]
            for start in range(0,len(indices),3):
                triangle=indices[start:start+3]
                # Exact coincident corners prove zero area, without a threshold
                # that might remove a small but visible surface at another scale.
                if len({positions[i] for i in triangle})<3:
                    removed+=1
                else:kept.extend(triangle)
            if len(kept)!=len(indices):
                primitive['indices']=document.append([(i,) for i in kept],'SCALAR',5125,34963)
    if removed:document.save()
    return removed
