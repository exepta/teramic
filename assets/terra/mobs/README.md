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
  `ocean`, `plains`, `forest`, `desert`, `highland`, `mushroom_park`, `cliffted_desert`.
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
