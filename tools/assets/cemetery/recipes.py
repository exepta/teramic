"""Rebuild the craftable masonry, clay and lighting subset of the grave catalog."""
import json
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / 'tools/assets'))
from recipe_paths import write_builtins

CATALOG = json.loads(Path(__file__).with_name('catalog.json').read_text())
BY_ID = {entry['id']: entry for entry in CATALOG}
DESTINATION = ROOT / 'assets/crafting/biomes/graves'


def block(material, count=1):
    return {'item': {'block': material}, 'count': count}


def write(material, ingredients, count=1):
    entry = BY_ID[material]
    recipe = dict(id='teramic::'+entry['slug'], name=entry['name'], station='workbench',
                  output={'item': {'block': material}, 'count': count},
                  pattern={'kind': 'shapeless', 'ingredients': ingredients})
    DESTINATION.mkdir(parents=True, exist_ok=True)
    (DESTINATION / (entry['slug']+'.json')).write_text(json.dumps(recipe, indent=2)+'\n')


def main():
    for material in range(324,337):
        color = BY_ID[material]['family'].removeprefix('clay_')
        color = {'gray': {'rgb':[128,128,128]}, 'dark_gray': {'rgb':[64,64,64]},
                 'turquoise': {'rgb':[64,192,192]}}.get(color, color)
        write(material,[block(19,4),{'item':{'dye':color},'count':1}],4)
    recipes = {
        337: ([block(3,4),block(19)],4),
        338: ([block(337,4),{'item':{'dye':'black'},'count':1}],4),
        339: ([block(337),block(42)],2),
        340: ([block(337,4),{'item':{'mineral':'gold_ingot'},'count':1}],4),
        341: ([block(337,4),block(13)],4),
        342: ([block(341,4),block(54)],4),
        343: ([block(337,4),block(19,2)],4),
        344: ([block(337,3)],6),
        345: ([block(340,3)],6),
        346: ([block(337,2),block(338,2),block(340,2)],6),
        347: ([block(339,3)],6),
        348: ([block(344,2),block(308)],2),
        349: ([block(343,3)],2),
        350: ([block(349),block(340)],1),
        351: ([block(340,2),block(349)],2),
        352: ([block(337,2),block(349)],2),
        353: ([block(343,4),block(349,2)],2),
        354: ([block(353),block(341)],1),
        369: ([block(349),block(193)],1),
        370: ([block(340),block(40)],1),
        371: ([block(19),block(40,3)],1),
        372: ([block(371,4),block(194),{'item':{'mineral':'iron_ingot'},'count':2}],1),
        373: ([block(338,2),block(395),block(40)],1),
        377: ([block(29),block(340,2),{'item':{'mineral':'gold_ingot'},'count':2}],1),
        397: ([block(398),block(395)],1),
        398: ([block(337,4),block(308)],4),
    }
    for material,(ingredients,count) in recipes.items(): write(material,ingredients,count)
    write_builtins()
    print(f'Wrote {13+len(recipes)} grave recipes')

if __name__=='__main__': main()
