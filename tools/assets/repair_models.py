#!/usr/bin/env python3
"""Targeted repair for a reviewed static asset; never changes UVs or vertex data."""
import argparse
import json
from pathlib import Path
from model_polish.repair import remove_collapsed_faces

ROOT=Path(__file__).resolve().parents[2]

def main():
    parser=argparse.ArgumentParser();parser.add_argument('models',nargs='+');args=parser.parse_args()
    report_path=ROOT/'tools/assets/model_polish/report.json'
    report=json.loads(report_path.read_text())
    for value in args.models:
        path=Path(value).resolve();path.relative_to(ROOT/'assets')
        removed=remove_collapsed_faces(path)
        for entry in report:
            if entry['path']==str(path.relative_to(ROOT)) and removed:
                entry['triangles_after']-=removed
                entry['removed_collapsed_triangles']=entry.get('removed_collapsed_triangles',0)+removed
        print(f'{path.relative_to(ROOT)}: removed {removed} collapsed triangles')
    report_path.write_text(json.dumps(report,indent=2)+'\n')

if __name__=='__main__':main()
