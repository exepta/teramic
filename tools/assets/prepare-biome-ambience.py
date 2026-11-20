#!/usr/bin/env python3
"""Reproduce the CC0 environmental inputs; runtime derives shared biome beds."""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import urllib.request

SOURCES = [
    ('wind1.wav', 'https://opengameart.org/content/wind1', 'Luke.RUSTLTD'),
    ('Crows.ogg', 'https://opengameart.org/content/crows-singing', 'IgnasD'),
    ('birds-isaiah658.ogg', 'https://opengameart.org/content/ambient-bird-sounds', 'isaiah658'),
] + [(f'wave_0{i}_cc0-18363__jasinski__alkaibeach.flac',
      'https://opengameart.org/content/beach-ocean-waves', 'jasinski / qubodup') for i in range(1, 5)]

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('source_dir', type=Path)
    args = parser.parse_args()
    args.source_dir.mkdir(parents=True, exist_ok=True)
    repo = Path(__file__).resolve().parents[2]
    target = repo/'assets/audio/environment'
    manifest = []
    for name, page, author in SOURCES:
        path = args.source_dir/name
        url = 'https://opengameart.org/sites/default/files/'+name
        if not path.exists():
            path.write_bytes(urllib.request.urlopen(url, timeout=45).read())
        manifest.append(dict(source=name, url=url, page=page, author=author, license='CC0-1.0',
                             sha256=hashlib.sha256(path.read_bytes()).hexdigest()))
    def convert(source, dest, seconds, high, low):
        subprocess.run(['ffmpeg', '-nostdin', '-v', 'error', '-y', '-i', str(source),
                        '-t', str(seconds), '-af',
                        f'highpass=f={high},lowpass=f={low},loudnorm=I=-24:TP=-8:LRA=10,'
                        f'afade=t=in:d=0.15,afade=t=out:st={seconds-0.2}:d=0.2',
                        '-ar', '24000', '-ac', '1', '-c:a', 'pcm_s16le', str(target/dest)], check=True)
    convert(args.source_dir/'wind1.wav', 'wind-ambience.wav', 44, 65, 8000)
    convert(args.source_dir/'Crows.ogg', 'crows.wav', 9.3, 170, 6800)
    convert(args.source_dir/'birds-isaiah658.ogg', 'woodland-birds.wav', 28, 280, 7800)
    # Four different real waves preserve variation instead of repeating one splash.
    playlist = args.source_dir/'waves.txt'
    playlist.write_text(''.join(f"file '{(args.source_dir/name).resolve()}'\n" for name,_,_ in SOURCES[3:]))
    joined = args.source_dir/'ocean-joined.wav'
    subprocess.run(['ffmpeg','-nostdin','-v','error','-y','-f','concat','-safe','0',
                    '-i',str(playlist),'-c:a','pcm_s16le',str(joined)], check=True)
    convert(joined, 'ocean-surf.wav', 12, 80, 5700)
    (target/'biome-sources.json').write_text(json.dumps(manifest,indent=2)+'\n')

if __name__ == '__main__':
    main()
