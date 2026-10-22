# Phase 72: Wetter, HUD Und Wegpunkte

Native Validierung am 19.10.2026, Linux/KDE Wayland, RX 6950 XT,
RADV/Mesa 26.2.4, Vulkan. Bestehendes optimiertes Dev-Profil, kein Release-
Vergleich. Separate temporaere Konfigurationen, Welten und Profile; bestehende
Benutzerwelten und der bereits laufende Dedicated Server wurden nicht angeruehrt.

## Umsetzung

- Die Fuellungen folgen den tatsaechlichen abgeschraegten HUD-Kanaelen. Ein
  UI-Material am bestehenden TiltUI-Bild nutzt die animierten nativen
  Progress-Bar-Werte. Rahmen und Labels bleiben fest, nur das Herz pocht.
- Herzschlag unter 60/30/10 Prozent mit 1.20/0.62/0.36 Sekunden pro Doppelschlag.
  Gameplay-Schalter, Apply/Reset/Verwerfen und bestehende TOML-Persistenz.
- MiniMap im bestehenden HUD-Editor separat verschiebbar, DPI-unabhaengig
  gespeichert, im Editor auch bei deaktivierter MiniMap sichtbar. Hotbar bleibt fest.
- Serverseitiger Wetterzyklus, glatte Intensitaets-/Winduebergaenge, kontinuierliche
  Klima-/Hoehenmischung, Naesse, bounded Schneeakkumulation und Schmelzen.
  Kalenderpause und Tageslaenge gelten auch fuer Wetter. Keine neue Voxelaufloesung.
- GPU-animierte, weltverankerte Regen-/Schneefallphasen, Aufschlagringe und
  Wolkenschichten in einem Batch. Dachcache reagiert auf Glas, Abbau, Platzierung,
  unbekannte Bereiche und Weltwechsel; transparente Sortiergrenzen folgen der
  Wetterwolke auch weit vom Weltursprung. Geometrieuploads maximal achtmal/s,
  Dachabfragen maximal 24/Frame, kein CPU-Animieren aller Partikelvertices.
- Wetterqualitaeten Off/Low/Medium/High/Ultra betreffen nur Darstellung,
  nicht autoritatives Wetter. Gemeinsame Sonne-/Nebel-/Materialreaktionen;
  kreditierte Regenaufnahme im vorhandenen Umweltmixer mit koerperbasiertem
  Hoerpunkt, Dach-/Hoehlenfilterung und vorhandenem Unterwasserfilter.
- Wegpunktnamen, Rand-Pins, Meter-/Kilometerentfernungen, sechs Symbole und
  optionale Weltlabels. Kartenicons rund 90 Prozent der Pfeillaenge; alte TOMLs
  behalten Pin/ausgeschaltete Weltanzeige als Defaults. Maximal 32 Kartenlabels
  und 24 Weltlabels, keine zusaetzlichen 3D-Entities. Lange Namen werden geclippt.

Architektur und Grenzen stehen ausfuehrlicher im Phase-72-Abschnitt der
[README](../../../../README.md). Server und Client gemeinsam auf Protokoll 40
aktualisieren. IDX4 im kleinen Weltindex liest weiterhin IDX3/IDX2/INDX;
Regionen bleiben REG9, Spieler PLY3, Generatorversion und IDs unveraendert.

## Tests

Alle angeforderten Kommandos erfolgreich:

```bash
cargo fmt --all --check
cargo check --workspace --locked
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo build --workspace --locked
```

1098 Tests bestanden, ein bereits vorhandener langer UDP-Soak ignoriert.
Begrenzt auf zwei Cargo-Jobs waehrend Builds. Unter anderem getestet:

- Herzschwellen/Abschaltung, Gameplay-Apply/Reset/Reload und unabhaengige
  MiniMap-Position mit Save/Cancel/Reset sowie bestehende HUD-Grenzen.
- Determinismus, Frame-Raten-unabhaengige Wetness-Integration, Kalenderpause,
  zwei echte Netzwerkclients, spaeter Beitritt, Qualitaets-/Autoritaetstrennung.
- Wetter-/Schnee-Speicherung und IDX3-Migration, Glas-/Dachschutz, offene
  Schluchten, unbekannte Decken, Akkumulation/Schmelzen und Regenloeschen.
- Statische Mesh-Indizes, Qualitaetsbudgets, trockener Leerlauf, Cache-Reset
  und transparente Sortierzentren an grossen/negativen Weltkoordinaten.
- Alte Wegpunktdateien, neue Felder, Layer/Budgets, Rand-Pins und Labelrechtecke
  fuer 72 Richtungen, vier Distanzen und fuenf UI-Groessen.
- Regenaufnahme dekodiert mit echtem Signal; unter Dach leiser, tief in Hoehlen
  still. Keine subjektive Hoerabnahme durch einen Menschen behauptet.

Native Laeufe ebenfalls erfolgreich:

- `--smoke-test-survival`: integrierte Autoritaet, Hunger/Stamina/Tod/Respawn,
  echte TiltUI-Meter, voller Gesundheitskanal, aktiver Herzschlag, HUD- und
  MiniMap-Drag/Save/Reload, Spielmoduswechsel.
- `--smoke-test-menu-gameplay --smoke-test-menu-small`: echte Gameplay-Seite,
  MiniMap-Dialog, Widgets/Apply/Reset/Reload und Footer bei kleiner Aufloesung.
- `--smoke-test-weather`: WGSL kompiliert nativ, Regen/Schnee/Nacht gerendert,
  Low/Off-Budgets, Schneedecke, Zuenden/Loeschen. Geschlossene Innenraeume und
  Unterwasseransichten sind hier durch Logiktests, nicht eigene Bildszenen belegt.
- `--smoke-test-minimap`: 36 Stufen mit echtem Netzwerkpeer, Kartenmodi/FPS,
  mehreren Pan-/Halte-/Zoom-Schritten, Maus-/Tastatureingaben, Boostflug,
  14px-Iconnachweis, 1.2km-Rand-Pins, Weltlabel, Wegpunkt-CRUD und TOML-Reload.
- Dedicated Server ohne DISPLAY/WAYLAND_DISPLAY gestartet: 27 Sections, kein Fenster.

Zwei fruehe Map-Fixturelaeufe sind separat archiviert: einmal verlangte die
Fixture faelschlich erkundetes Terrain nach dem Zentrieren auf 1200m Entfernung;
einmal unterbrach externer OS-Fokusverlust den synthetischen Pan. Der letzte
Lauf akzeptiert dort nur exakt definiertes Unknown-Grau und haelt seinen eigenen
logischen Eingabefokus, ohne den OS-Fokus oder Produktionsguards zu veraendern.
Eine bekannte Wayland-Cursor-Warp-Meldung der synthetischen Fixture ist kein
Shaderfehler. Der abschliessende volle Lauf ist `logs/native-map.log`.

## Kurze Framefenster

Dieselbe stationaere Standalone-Szene/Kamera, 1920x1080 physische Capturepixel,
kontinuierlicher Eventloop, unlimitierte Ausgabe. Rund 277..286 erfasste Frames
je Phase nach 0.5 Sekunden Warmup; Aufnahme wird vor Zustandswechsel abgeschlossen.
Kein Vorher-/Nachher-Buildvergleich und kein laengerer Flug-/Streamingbenchmark.

| Darstellung | FPS | p95 ms | p99 ms | Quads | Batches |
| --- | ---: | ---: | ---: | ---: | ---: |
| Klar, High | 184.08 | 6.364 | 7.444 | 0 | 0 |
| Regen, High | 185.70 | 6.286 | 6.575 | 1255 | 1 |
| Schnee, High | 185.04 | 6.343 | 6.909 | 579 | 1 |
| Regen, Low | 185.39 | 6.255 | 6.601 | 190 | 1 |
| Regen, Effekte Off | 185.54 | 6.323 | 6.719 | 0 | 0 |
| Nachtregen, High | 190.26 | 6.083 | 6.768 | 1255 | 1 |

Sieben Wetter-Meshuploads insgesamt inklusive Warmup und Qualitaets-/Artwechsel;
stabile Phasen erzeugen keine Vertexanimation-Uploads. Die kleinen FPS-Differenzen
belegen keine Verbesserung. GPU-Zeitstempel, GPU-interne Drawzahlen, RSS-Paritaet,
Ultra und andere GPUs wurden nicht vermessen. Lange Lastlaeufe bleiben in TODO.md.

## Bilder

- [Voll gefuellte Kanaele](captures/respawn.png)
- [Verschobene MiniMap](captures/hud-minimap-moved.png)
- [Gameplay-Schalter](captures/gameplay-options.png)
- [Regen](captures/rain.png), [Schnee](captures/snowfall.png), [Nachtregen](captures/rain-night.png)
- [Wegpunkteditor](captures/waypoint-editor.png)
- [FullMap-Pin](captures/fullmap-pin.png), [Weltlabel/MiniMap-Pin](captures/world-label-minimap-pin.png)
- [Nach dem Loeschen](captures/waypoint-removed.png)

## Grenzen

Schnee ist kosmetischer persistenter Oberflaechenzustand, keine Schicht mit
Kollision oder Schneemassenphysik. Nur geladene Spalten werden budgetiert
bearbeitet; sehr viele Spalten koennen deutliche Akkumulationslatenz erzeugen.
Kein Offline-/Regions-Catch-up, kein Jahreszeitensystem, keine volumetrischen
Wolken und keine physische Wasserverformung durch Regentropfen. Wegpunkte sind
lokal je Spieler/Welt/Dimension, nicht im Multiplayer geteilt. Native Bilder
ersetzen keine gemeinsame vollstaendige Art-Abnahme oder Langzeitprofilierung.
