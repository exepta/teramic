"""Write the German/English grave item strings from the shared asset manifest."""
import json
from pathlib import Path
from descriptions import describe

ROOT = Path(__file__).resolve().parents[3]


def german_description(d, name):
    f = d['family']
    if f.startswith('clay_'): return name + ' für farbige Wände, Böden und Ziermuster.'
    if d['id'] <= 343: return name + ' für die verwitterten Mauern alter Gräber und Krypten.'
    if 'slab' in f: return name + ' für Friedhofswege und Kryptenböden.'
    if f in ('column', 'ornate_column', 'capital', 'base', 'arch', 'moss_arch'): return name + ' für mächtige Eingänge, Arkaden und Gewölbehallen.'
    if f in ('relief', 'skull_relief', 'cross_relief'): return name + ' mit einem eingemeißelten Gedenkmotiv.'
    if f in ('banner', 'torn_banner', 'curtain'): return name + ' aus verblasstem violettem Stoff mit goldener Einfassung.'
    if f.startswith('gravestone_') or f == 'cross': return name + ' mit einem gemeißelten Gedenkzeichen und verwittertem Steinsockel.'
    if f in ('lantern', 'torch', 'candles', 'chandelier'): return name + ' taucht Wege und Kammern in warmes Licht.'
    if f in ('grass', 'thorn', 'ivy', 'moss', 'mushrooms', 'vines'): return name + ' vom überwucherten Gelände alter Begräbnisstätten.'
    if f in ('skeleton', 'bones', 'urn', 'books', 'skull', 'chain'): return name + ' zur Ausstattung alter Gräber und Kryptenkammern.'
    if f in ('plate', 'lever', 'switch', 'portcullis', 'gate', 'spikes', 'arrows'): return name + ' als Dekoration für Krypteneingänge und Grabkammern.'
    return {
        'niche': 'Eine zurückgesetzte Steinnische, erhellt von einer Gruppe warmer Kerzen.',
        'bordered_grave': 'Eine steinerne Ruhestätte mit Blumen in der eingefassten Erde.',
        'sarcophagus': 'Ein geschlossener Steinsarkophag mit goldenen Bändern.',
        'open_sarcophagus': 'Ein offener Steinsarkophag mit einem alten Deckel und verstreuten Knochen.',
        'chest': 'Eine verzierte Truhe zur Aufbewahrung von Wertgegenständen in einer Krypta.',
        'brazier': 'Eine steinerne Feuerschale voller violettem Geisterlicht.',
        'glow_mushrooms': 'Violette Pilze, die schwaches Licht zwischen alten Gräbern verbreiten.',
        'crystal_small': 'Eine Gruppe von Seelenkristallen, die violettes Licht ausstrahlen.',
        'crystal_large': 'Eine Gruppe von Seelenkristallen, die violettes Licht ausstrahlen.',
        'rune': 'Gemeißelte Runen leuchten vor violetter Geisterenergie.',
        'inactive_rune': 'Alte Runen liegen dunkel auf der Oberfläche dieses Steins.',
        'mist': 'Ein durchscheinender violetter Stein für mystische Dekorationen.',
    }[f]


def main():
    entries = {}
    for d in json.loads(Path(__file__).with_name('catalog.json').read_text()):
        name = d['german']
        for a,b in [('Gruen','Grün'),('gruen','grün'),('Weiss','Weiß'),('Tuerkis','Türkis'),('Saeule','Säule'),('saeule','säule'),('Haeng','Häng'),('Grosser','Großer'),('Buecher','Bücher'),('Schaedel','Schädel'),('Bluehend','Blühend')]: name=name.replace(a,b)
        name = {337:'Grabmauerstein',338:'Dunkler Grabmauerstein',339:'Rissiger Grabmauerstein',354:'Bemooster Kryptabogen'}.get(d['id'],name)
        entries[d['name']] = dict(source=d['name'], de=name, en=d['name'])
        desc = describe(d)
        entries[desc] = dict(source=desc, de=german_description(d,name), en=desc)
    for en,de in [('Cemetery','Friedhof'),('Grave Dungeon','Grabgewölbe'),('Tomb of {}','Grab von {}')]: entries[en]=dict(source=en,de=de,en=en)
    (ROOT/'src-ui/locales/source/cemetery.json').write_text(json.dumps(list(entries.values()),ensure_ascii=False,indent=2)+'\n')

if __name__ == '__main__': main()
