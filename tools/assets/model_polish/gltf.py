"""Small lossless glTF container/accessor helpers; preserve unknown JSON fields."""
import base64
import json
import struct
from pathlib import Path

FORMAT={5120:'b',5121:'B',5122:'h',5123:'H',5125:'I',5126:'f'}
WIDTH={'SCALAR':1,'VEC2':2,'VEC3':3,'VEC4':4,'MAT4':16}

class Document:
    def __init__(self,path):
        self.path=Path(path);self.glb=self.path.suffix=='.glb';self.buffers=[]
        if self.glb:
            data=self.path.read_bytes();magic,version,length=struct.unpack_from('<III',data)
            assert magic==0x46546c67 and version==2 and length==len(data),self.path
            at=12;binary=None
            while at<len(data):
                size,kind=struct.unpack_from('<I4s',data,at);chunk=data[at+8:at+8+size];at+=size+8
                if kind==b'JSON':self.doc=json.loads(chunk)
                elif kind==b'BIN\0':binary=chunk
        else:self.doc=json.loads(self.path.read_text());binary=None
        for b in self.doc.get('buffers',[]):
            uri=b.get('uri')
            self.buffers.append(bytearray(binary if uri is None else base64.b64decode(uri.split(',',1)[1]) if uri.startswith('data:') else (self.path.parent/uri).read_bytes()))
    def values(self,index):
        a=self.doc['accessors'][index];assert 'sparse' not in a,'Sparse accessor needs an artist review'
        v=self.doc['bufferViews'][a['bufferView']];fmt='<'+FORMAT[a['componentType']]*WIDTH[a['type']];size=struct.calcsize(fmt)
        start=v.get('byteOffset',0)+a.get('byteOffset',0);stride=v.get('byteStride',size);data=self.buffers[v['buffer']]
        return [struct.unpack_from(fmt,data,start+i*stride) for i in range(a['count'])]
    def append(self,values,kind,component=5126,target=34962):
        data=self.buffers[0];data.extend(b'\0'*(-len(data)%4));offset=len(data);fmt='<'+FORMAT[component]*WIDTH[kind]
        for v in values:data.extend(struct.pack(fmt,*v))
        views=self.doc.setdefault('bufferViews',[]);accessors=self.doc.setdefault('accessors',[])
        index=len(accessors);a=dict(bufferView=len(views),componentType=component,count=len(values),type=kind)
        if kind=='VEC3':a.update(min=[min(v[i] for v in values) for i in range(3)],max=[max(v[i] for v in values) for i in range(3)])
        views.append(dict(buffer=0,byteOffset=offset,byteLength=len(data)-offset,target=target));accessors.append(a)
        return index
    def save(self):
        for i,b in enumerate(self.doc['buffers']):
            b['byteLength']=len(self.buffers[i])
            if self.glb and i==0:b.pop('uri',None)
            else:b['uri']='data:application/octet-stream;base64,'+base64.b64encode(self.buffers[i]).decode()
        if self.glb:
            j=json.dumps(self.doc,separators=(',',':')).encode();j+=b' '*(-len(j)%4);b=bytes(self.buffers[0]);b+=b'\0'*(-len(b)%4)
            data=struct.pack('<III',0x46546c67,2,28+len(j)+len(b))+struct.pack('<I4s',len(j),b'JSON')+j+struct.pack('<I4s',len(b),b'BIN\0')+b
            self.path.write_bytes(data)
        else:self.path.write_text(json.dumps(self.doc,separators=(',',':'))+'\n')
