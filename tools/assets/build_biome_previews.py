#!/usr/bin/env python3
"""Capture real biome terrain and export chooser photographs without upscaling.
Requires the existing terra-client binary and ImageMagick, no Python packages.
Example:
  python3 tools/assets/build_biome_previews.py --capture --config-dir /tmp/private-config --captures /tmp/biome-photos
Full native screenshots remain in --captures; only centre crop/downsampling enters src-ui/assets.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import struct
import subprocess

BIOMES=('plains','forest','desert','highlands','mushroom-park','cliffted-desert','cherry-forest','spirit-blossom','blood-forest','ocean')
ROOT=Path(__file__).resolve().parents[2]

def png_size(path):
    with path.open('rb') as image:
        header=image.read(24)
    if header[:8]!=b'\x89PNG\r\n\x1a\n':
        raise ValueError(f'Expected native PNG capture: {path}')
    return struct.unpack('>II',header[16:24])

def export_photo(source,destination):
    width,height=png_size(source)
    crop_height=min(height,width//2)
    crop_width=crop_height*2
    # Refuse to invent detail. The source must already exceed the export size.
    if crop_width<1536 or crop_height<768:
        raise ValueError(f'Capture too small for a genuine 1536x768 preview: {source} ({width}x{height})')
    destination.parent.mkdir(parents=True,exist_ok=True)
    subprocess.run(['magick',str(source),'-gravity','center','-crop',f'{crop_width}x{crop_height}+0+0','+repage','-colorspace','RGB','-filter','Lanczos','-resize','1536x768!','-colorspace','sRGB','-alpha','off','-depth','8',str(destination)],check=True)
    assert png_size(destination)==(1536,768)
    with destination.open('rb') as exported:
        assert exported.read(25)[24]==8, 'UI preview must remain an8-bitPNG'
    return {'source':str(source.relative_to(ROOT)) if source.is_relative_to(ROOT) else str(source),'source_size':[width,height],'source_sha256':hashlib.sha256(source.read_bytes()).hexdigest(),'crop_size':[crop_width,crop_height],'export_size':[1536,768],'sha256':hashlib.sha256(destination.read_bytes()).hexdigest()}

def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--capture',action='store_true')
    parser.add_argument('--config-dir',type=Path)
    parser.add_argument('--captures',type=Path,required=True)
    parser.add_argument('--output',type=Path,default=ROOT/'src-ui/assets')
    parser.add_argument('--client',type=Path,default=ROOT/'target/debug/terra-client')
    parser.add_argument('--only',choices=BIOMES,nargs='+',default=BIOMES)
    args=parser.parse_args();capture_dir=args.captures.resolve();capture_dir.mkdir(parents=True,exist_ok=True)
    if args.capture and args.config_dir is None:
        parser.error('--capture requires an isolated --config-dir')
    manifest_path=args.output/'biome-previews.json'
    manifest=json.loads(manifest_path.read_text()) if manifest_path.exists() else {'method':'Native game screenshots; centre crop and downsample only. No upscaling or generated artwork.','images':{}}
    for biome in args.only:
        if args.capture:
            env=os.environ.copy();env.update(TERRA_CONFIG_DIR=str(args.config_dir.resolve()),TERRA_MENU_CONFIG_DIR=str(args.config_dir.resolve()),TERRA_BIOME_SCENE='picker-'+biome,TERRA_CAPTURE_DIR=str(capture_dir),TERRA_BENCH_GRAPHICS='1')
            env.pop('TERRA_PREVIEW_WARMUP',None)
            with (capture_dir/f'picker-{biome}.log').open('w') as log:
                subprocess.run([str(args.client.resolve()),'--smoke-test-biomes'],env=env,cwd=ROOT,stdout=log,stderr=subprocess.STDOUT,check=True,timeout=150)
        source=capture_dir/f'picker-{biome}-overview.png'
        destination=args.output/f'biome-choice-{biome}.png'
        manifest['images'][biome]=export_photo(source,destination)
        print(f'{biome}: {manifest["images"][biome]["source_size"]} → 1536×768',flush=True)
        manifest_path.write_text(json.dumps(manifest,indent=2)+'\n')

if __name__=='__main__':
    main()
