# Mob-Definitionen

Jede Art besitzt eine eigene JSON-Datei. Die vorhandene Artenregistry liest
und validiert diese Definitionen; Server und Client verwenden dieselben IDs.
Die Spawn-Einstellungen betreffen neue Kandidaten, nicht bereits gespeicherte
Tiere oder deren laufende Navigation. Lebenswerte, Groessen und bestehende
Art-Felder bleiben in derselben Definition.

## Optionale Spawn-Einstellungen

Beispiel fuer eine angepasste Fischdefinition (Teil der bestehenden JSON-Datei):

```json
{
  "spawn": {
    "enabled": true,
    "biomes": ["ocean", "plains", "forest"],
    "herd_size": [1, 3],
    "biome_herds": [{ "biome": "forest", "size": [1, 2] }],
    "aquatic": {
      "ocean_weight": 47.0,
      "lake_weight": 23.0,
      "river_weight": 1.0,
      "water_depth": [2.0, 64.0],
      "depth_below_surface": [1.5, 4.0]
    }
  }
}
```

- Ohne `spawn` oder mit `spawn: {}` bleiben die bisherigen Regeln erhalten.
- `enabled = false` verhindert neue natuerliche Spawns dieser Art.
- Eine leere `biomes`-Liste erlaubt die bisher zulaessigen Biome. Gueltige Namen:
  `ocean`, `plains`, `forest`, `desert`, `highland`, `mushroom_park`, `cliffted_desert`,
  `cherry_forest`, `spirit_blossom`, `blood_forest`.
  Listen/Overrides duerfen keine Duplikate enthalten.
- `herd_size` und `biome_herds[].size` sind inklusive Min-/Max-Bereiche von 1
  bis zum bestehenden `group_size` (hoechstens 4). Der Biom-Override hat Vorrang.
  Ohne Override bleibt die bisherige Gruppenauswahl bestehen.
- `aquatic` gilt fuer die vorhandene marine Auswahl. Gewichte sind relative,
  endliche Zahlen von 0 bis 1000; auch Bruchteile sind moeglich. Gewicht 0
  deaktiviert die Art fuer diesen Gewaessertyp. Bei Gesamtsumme 0 spawnt dort
  keine marine Art. Ozean-, See- und Flussklassifikation bleiben unveraendert.
- `water_depth` begrenzt die zusammenhaengende Wassersaeule in Bloecken.
  `depth_below_surface` begrenzt die Tiefe der Tierbasis unter dem Wasserspiegel;
  die Platzierung prueft weiterhin Koerperhoehe und freien Wasserraum.
  Beide Bereiche muessen geordnet, endlich und innerhalb 0 bis 2048 sein.
  Ohne explizite Platzierungstiefe bleibt die bisherige Art-Platzierung erhalten.
  Noch ungeladene Ziel-/Koerper-Sections werden abgewartet; unbekannter Raum
  wird weder als frei behandelt noch dauerhaft als Fehlspawn gespeichert.

Die mitgelieferten zehn marinen Definitionen enthalten explizite Gewichte,
die die bisherige ganzzahlige Hash-Auswahl exakt erhalten. Meer:
Squid 13; Moon/Fire/Sun Jelly und Reef Shark je 6; Void/Crystal Jelly je 5;
Hammerhead Shark 4; Whale Shark 2; Fish 47. See: Squid 1, Fish 23.
Fluss: Fish 1. Bestehende Block-/Mob-/Welt-IDs werden nicht geaendert.

Biomfilter und Herdengroessen werden auch von der bestehenden Landtier- und
Bienengenerierung verwendet. Das ist kein neues Populationssystem und keine
Gebietsbarriere fuer Tiere. Artspezifische Populationslimits, Nahrungslisten
und Verbrauchsraten sind noch nicht durch diese Spawnfelder konfigurierbar.
Kein Hot-Reload: Definitionen vor dem Spiel-/Serverstart anpassen.

## Dorfbewohner

`villager.json` erweitert dieselbe Artenregistry. Die gespeicherten Rollen 0–12
bleiben erhalten; Ritter, Speertraeger und Garnisonsbogenschuetze behalten die
militaerischen IDs 10–12. Angehangen sind Lokfuehrer (13), Minenarbeiter (14),
Arbeitslos-Referenzvariante (15), Schlachter (16), Verkaeufer (17), fahrender
Haendler (18) und Nomaden A–D (19–22). HP, Nahrung, Wasser, Koerpergroesse und
Laufgeschwindigkeit verwenden die normalen Art-Felder. `variant_titles` benennt
alle 23 Modelle; neue Zivilisten zaehlen nicht als Garnison.

Bewohner werden aus bereits vorbereiteten Dorf-/Bettdaten erzeugt, nicht durch
die zufaellige Landtier-Herdenauswahl. Jeder primaere generierte Schlafplatz
hat genau eine stabile Bewohner-ID. Gespeicherte und tote Bewohner werden nicht
neu ausgewuerfelt. Sie zaehlen nicht zum natuerlichen Tierlimit. Neue Doerfer
verwenden Strukturrevision 4; alte gespeicherte Revisionen behalten ihre Gebaeude.
Nachts laufen Bewohner durch die Haustuer zu ihrem Bett. In freien Wohnbetten
koennen auch die neuen Berufe entstehen. Lokfuehrer, Minenarbeiter, Farmer,
Schmiede, Schlachter und Verkaeufer besuchen erreichbare nahe Arbeitsplaetze
ueber die bestehende begrenzte Wegsuche; Reisende erkunden laengere Wege innerhalb
der Siedlung. Arbeitsplaetze werden dabei weder abgebaut noch Inventare veraendert.
Rechtsklick spricht Bewohner an; Handel und berufliche Produktion sind noch nicht
Teil dieses Grundsystems. Alle neuen Referenzrollen sind ebenfalls ueber
`/spawn teramic::train_driver`, `miner`, `unemployed`, `butcher`, `seller`,
`travelling_merchant` und `nomad_a` bis `nomad_d` erreichbar (jeweils mit
`teramic::`-Praefix).

## Spirit Blossom fauna

`butterfly.json`, `spirit_gloom.json` und `spirit_fox.json` erweitern die Registry
am Ende; die gespeicherten IDs 0–20 bleiben erhalten. Die neuen IDs sind 21, 22,
23. Varianten werden als bestehendes `variant`-Feld gespeichert und repliziert:

- Butterfly 0–6: Blossom, Petal, Wisteria, Spirit, Goldenbloom, Moonpetal, Sakurawing.
  Natürliche Habitate: Plains, Forest, Cherry Forest und Spirit Blossom. Die Tiere
  fliegen mit sichtbar schlagenden Flügeln, landen kurz und können von kleinen
  Spinnen gefressen werden.
- Spirit Gloom 0–1: Blossom und Moon. Ausschließlich im Spirit-Blossom-Biom.
  Schwebende, friedliche Wesen in Weiß/Cyan beziehungsweise Violett mit
  eigenständig animierten Armen, Flammenschweif und zwei kreisenden Minigeistern.
  Lebende Glooms gewähren Überlebensspielern innerhalb von sechs Metern
  Regeneration I. Der Server prüft einmal je Sekunde; mehrere Glooms addieren
  keine Stufen und überschreiben keine stärkeren Tränke.
- Spirit Fox 0: friedlicher laufender Fuchs mit drei animierten Leuchtschweifen,
  ausschließlich in Spirit Blossom. Die Tiere fliehen bei Bedrohung. Keine
  Zähmung, Zucht oder neuen Beutegegenstände werden hier behauptet.

Alle drei verwenden die bestehenden Populations-, Sichtbarkeits- und
Aktualisierungsbudgets. Flieger benötigen geladene, freie Voxelzellen und werden
bei der Abstandskorrektur nicht auf den Boden projiziert. Geteilte Meshes liegen
in `terra-render/src/creatures/model/spirit.rs`, Animationen separat daneben.

Manuell testen: `/spawn teramic::blossom_butterfly` (beziehungsweise die anderen
sechs Variantennamen), `/spawn teramic::spirit_gloom_blossom`,
`/spawn teramic::spirit_gloom_moon`, `/spawn teramic::spirit_fox`.
`TERRA_SPIRIT_INSPECT=1` zusammen mit `--smoke-test-creatures` startet die isolierte
Rendergalerie; `TERRA_CAPTURE_DIR` bestimmt ihr Screenshot-Verzeichnis.

## Kryptawachen und Blood Forest

Neue Arten werden append-only gespeichert: Skeleton=24, Raven=25, BloodSlime=26,
BloodStalker=27, BloodWisp=28, ThornBrute=29. Skeleton hat drei Varianten:
0 unbewaffnet, 1 Schwertkämpfer, 2 gerüsteter Wächter. Alle Modelle bestehen aus
geteilten, geschlossenen 3D-Meshes mit eigenen Gliedmaßen; keine flachen Sprites.
Die neue Geometrie liegt modular unter `terra-render/src/creatures/model/forest/`.

Raben fliegen und landen gelegentlich, Blutschleime laufen friedlich mit einem
federnden Körper. Blood Stalker sind achtbeinige Lauerer. Skelette, Lauerer,
Blutirrlichter und Dornenwächter verfolgen sichtbare Überlebensspieler und greifen
mit JSON-gesteuerter Reichweite und Abklingzeit an. Blockierte Wege nutzen die
begrenzte Navigation; unbekannte Chunks blockieren Bewegung. Kein Schaden durch
Wände. Die eigentliche Simulation und Beutevergabe laufen ausschließlich auf dem
Server. Die neuen Arten besitzen noch keine Zähmung, Fortpflanzung oder
Fernkampfangriffe.

Natürliche Blood-Forest-Habitate wählen friedliche und feindliche Arten über den
Weltseed. Im Geistwald entstehen zusätzlich Skelette auf geladenen Kryptaböden.
Ihr eigener ID-Namensraum bewahrt vorhandene Tiere, Bewohner und tote Wächter;
ein Reload würfelt sie nicht erneut aus. Spawnfilter bleiben über JSON steuerbar.

Beute: Skelette/Krähen hinterlassen Knochen, Blutschleime rotes Schleimgel,
Lauerer Blutkristallsplitter, Irrlichter Blutsporen und Dornenwächter Dornenzweige
und dunkles Harz. Blutbeeren und Karmesinpilze sind essbar. Die neun neuen Items
haben eigene JSON-Dateien unter `assets/terra/items/`, Inventar-/Hand-/Drop-Meshes
und deutsche/englische Beschreibungen.

Manuell: `/spawn teramic::skeleton`, `teramic::skeleton_fighter`,
`teramic::skeleton_guardian`, `teramic::raven`, `teramic::blood_slime`,
`teramic::blood_stalker`, `teramic::blood_wisp`, `teramic::thorn_brute`.
`TERRA_FOREST_INSPECT=1 --smoke-test-creatures` zeigt acht echte Modelle in einer
isolierten Galerie. Screenshots verwenden `TERRA_CAPTURE_DIR`. Dies prüft
Darstellung/Animation; AI, Treffer und Persistenz werden getrennt getestet.
