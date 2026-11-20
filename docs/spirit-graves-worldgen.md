# Geistwald: Gelände und Gräber, Revision 2

`spirit_blossom_revision = 2` aktiviert die Landschaftsänderungen und die neuen
Grabstätten. Revision 0 und 1 behalten ihre bisherige Generierung. Neue Welten
verwenden Revision 2; bestehende Weltmanifeste werden nicht automatisch umgestellt.

## Hügel und Wasserflächen

Trockene Hügel im Inneren des Geistwalds erhalten **20 % mehr Höhenunterschied**.
Der Faktor wirkt um den Meeresspiegel als festen Bezugspunkt. Küsten, Flussufer,
Seenränder und Biomübergänge blenden den Zusatz weich aus. Wasserstände,
Flussverläufe und Auslässe bleiben erhalten. Kleine Bäche behalten eine Breite
unter 3,5 Metern. An Zu- und Abflüssen mit abweichendem Pegel blendet die
Seenvergrößerung auf die ursprüngliche Kontur zurück, sodass Flussbetten nicht
durch einen niedrigeren Seenpegel trockenfallen und Ufer abgestützt bleiben.
Die zusätzlichen Süßwasserflächen entstehen durch breitere offene Flussbetten
und größere, zusammenhängende Seenformen. Seen werden mit der Quadratwurzel
des Flächenfaktors skaliert. Das Meer wird nicht vergrößert.

Die Kalibrierung zählt tatsächlich wassergefüllte Voxelspalten auf einem
4-Meter-Raster in 18 Geistwald-Regionen mit den Seeds 42042, 271828 und 9317
und der ausgelieferten `configs/world.toml`. Verglichen werden Revision 1 und 2:

| Messung | Ergebnis |
| --- | --- |
| Ausgewertete Spalten mit Biomgewicht ≥ 0,85 | 377.729 |
| Wasserfläche vorher / nachher (Rasterpunkte) | 5.566 / 7.737 |
| Gemessene zusätzliche Wasserfläche | **39,00 %** |
| Trockene innere Hügel, 295.129 Nachbarpaare | **20,000 %** mehr Höhenunterschied |
| Gesamte trockene Landschaft einschließlich neuer Ufer | 25,00 % mehr Höhenunterschied |

Offene Hauptkanäle und ungestörte Seenflächen verwenden im Biomkern einen
Flächenzuwachs von bis zu 64 %. Kleine Bäche und Pegelübergänge bleiben begrenzt;
Überschneidungen, bestehendes Meer und diskrete Uferzellen reduzieren den gemessenen Gesamtzuwachs
auf rund 40 %. Der Flächenzuwachs ist ein statistisches Ziel, keine Zusicherung
für jede einzelne Region oder jeden Seed. Die drei Seeds liegen einzeln bei
21,38 %, 24,82 % und 51,28 %.

Reproduzieren:

```sh
cargo run --locked --offline -p terra-world --example measure_spirit_wetlands
```

## Friedhöfe und Grabdungeons

Die Option `structures.ruins` schaltet auch die neuen Grabstätten. Der Planer
prüft vor der Erzeugung den Geistwald-Anteil, trockenen Boden, Höhendifferenzen
und die bestehende Strukturgröße. Die Standorte verwenden dieselben regionalen
Caches, Chunk-Patches und Streaming-Budgets wie andere Weltgebäude. Es werden
keine Bewohner oder Königreiche für Friedhöfe erzeugt.

Friedhöfe besitzen eine eingefasste Anlage, Grabreihen, Grabsteine, Sarkophage,
Laternen, Kerzen und eine kleine Gedenkkapelle. Die größeren Mausoleen führen zu
drei Varianten: Rittergrab (3 Ebenen), Priesterinnengrab (4 Ebenen) und
Gelehrtengrab (4 Ebenen). Die Namen werden aus dem Standort-Seed abgeleitet.
Säulen, Gewölbedurchgänge, Nischen und variierende Grabbeigaben unterscheiden die
Varianten. Jede Ebene besitzt eine echte, befüllte verzierte Kryptentruhe.

Die Treppen sind gewöhnliche orientierte Stein-Treppenblöcke. Treppenfluchten
sind drei Blöcke breit; die Laufwege haben mindestens drei Blöcke Kopffreiheit. Tests prüfen
den gesamten Weg auf zusammenhängende Schritte, feste Unterstützung,
Kopffreiheit und echte Truhenobjekte. Weitere Tests prüfen die natürlichen
Standorte über den öffentlichen Voxelgenerator. Die Gräber bestehen aus
abbaubaren Weltblöcken; Fallenlogik, Bosse und Dungeon-Aufgaben gehören nicht zu
dieser Änderung.

`SiteSummary::grave_dungeon()` liefert den Namen, die Variante, den Eingang,
die Ebenen, den verbundenen Laufweg und die Truhenpositionen. Die Daten werden
aus dem Standort abgeleitet und brauchen keinen zusätzlichen Speicherzustand.
`TerraGenerator::grave_site_candidate()` ist der günstige Vorfilter für Locator
und Diagnosewerkzeuge; nur der vollständige Planer bestätigt einen Standort.

Die ausgelieferte Konfiguration mit Seed 42042 besitzt diese Prüfstandorte:

| Anlage | Strukturregion | Eingang / Mittelpunkt |
| --- | --- | --- |
| Friedhof | `[-8, -10]` | `[-2883, 34, -3647]` |
| Corvin, Gelehrter | `[-12, -4]` | `[-4401, 41, -1353]` |
| Eldric, Ritter | `[14, 6]` | `[5554, 57, 2487]` |
| Maelis, Priesterin | `[8, 15]` | `[3248, 32, 5932]` |

```sh
cargo run --locked --offline -p terra-world --example inspect_grave_sites
cargo test --locked --offline -p terra-world --features bevy --lib generation::structures::build::graves
```

Der Finder gibt JSON-Zeilen einschließlich der vollständigen Wege aus. Die
Messprotokolle liegen unter
`tools/benchmarks/results/spirit-world-polish-2026-11-13/`.
