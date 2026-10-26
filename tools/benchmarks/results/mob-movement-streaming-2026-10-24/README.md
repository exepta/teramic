# Mob-Bewegung, Karten Und Streaming (Phase 119)

## Boden, Klettern Und Spawns

Die strenge Vier-Huf-Pruefung fuer die Wegsuche wurde auch als reale
Bodenauflage benutzt. Breite Tiere, die mehrere aufeinanderfolgende Stufen
ueberspannen, konnten dadurch an einer Kante stehen bleiben. Physische Auflage
erlaubt jetzt echte Teilkontakte, waehrend Routen weiterhin die gesamte
Koerperfreiheit und geladene Geometrie pruefen. Tiere koennen volle Bloecke
hinuntergehen und betreten Wasser mit ihrem wirklichen Koerper.

Bereits ueberlappende Tiere verwenden eine kleine, begrenzte Korrektursuche.
Jeder getestete Weg darf nur bestehende Kontakte verlassen, keine neuen
Blockkollisionen durchqueren. Unbekannte Sections, dicke Waende und geschlossene
Raeume bleiben gesperrt. Die Korrektur wird nicht als kuenstliche Geschwindigkeit
gespeichert. Neue Landtiere werden mit genau ihrer spaeter gespeicherten
Orientierung auf freien Platz geprueft, nicht mit einer anderen Spawn-Rotation.

Spinnen verwenden an Boden und Decke die projizierten Ausmasse ihres gedrehten
Koerpers. Wandkontakte und Oberflaechenwechsel benutzen dieselben realen
Shape-Boxen; insbesondere die lange Large Spider wird nicht mit einer falsch
ausgerichteten X/Z-Probe an eine Wand gedrueckt. Kletter-/Deckenbewegung bleibt
Bestandteil der bisherigen KI und Kollision, kein separates Navmesh.

Die validierte optionale Mob-JSON-Eigenschaft `spawn.cave_chance` erlaubt die
angeforderten relativen Erhoehungen: Stalker 1/11 -> 0.10 (+10 %), Medium/Large
Spider 0.20 -> 0.25 (+25 %). Small Spiders bleiben Oberflaechenbewohner. Die
bisherigen Populations-/Regionalgrenzen und gespeicherten Identitaeten bleiben
unveraendert. Dies garantiert nicht, dass jeder neue Versuch einen geometrisch
gueltigen Spawnplatz findet.

## Reiten Und Debug

- `X`: von Reitobjekten absteigen; als eigene Aktion umbelegbar.
- `Shift`: Pferd sprintet. Die bestehende Sprint-Taste funktioniert ebenfalls.
- `Space`: Pferdesprung. Beschleunigung, Bremsen und Drehen sind zeitbasiert;
  waehrend des Sprungs bleibt Vorwaertsmomentum erhalten.
- `F1 + H`: reale Mob-Kollisionsboxen und Blickrichtung, unabhaengig vom
  F1-Textoverlay. Beim Schreiben in Chat/Inventar wird die Aktion blockiert.

Alte Controls-Dateien bekommen die neuen Aktionen ohne fremde Belegungen zu
ueberschreiben. Pferde haben eine gemeinsame Aufstieg-/Scheitel-/Fall-/Landepose;
der Koerper dreht um denselben Sattelpunkt, dem der Reiter mit seinem Becken
folgt. Reiten benutzt keine Fussgaenger-Kamerawippbewegung. Netzwerkdarstellung
setzt die kurze Winkelinterpolation an der wirklich angezeigten Pose fort,
statt bei jedem Paket auf die vorherige Snapshot-Rotation zurueckzuspringen.
Die Hitbox-Gizmos erzeugen keine Mob-Zusatzentities oder dauernden Assets und
sind auf 128 m begrenzt.

## Karten Und HP-Balken

Live-Mobmarker haengen nicht mehr vom Laden einer detaillierten Kartenkachel
ab. Hoehen kommen aus Detail-, Relief-/Overview-Daten oder bereits vorbereiteten
Spalten. Eine unbekannte vertikale Section gilt nicht mehr als nachgewiesene
Felsdecke; bekannte Decken trennen Stockwerke weiterhin. Unterwassertiere werden
nicht wegen einer tiefen transparenten Wassersaeule als verdeckt einsortiert.
Der FullMap-Mobschalter funktioniert unabhaengig von der Minimap-Einstellung.

**Options > Gameplay > Mob Health Bars > Edit Bars** oeffnet einen eigenen
Dialog mit `View Port`, `Looking At`, `All Time`, `None` und drei echten
TiltUI-Farbwaehlern: freundlich, feindlich, Schrift. Aenderungen werden mit
Apply gespeichert; Discard und Reset verwenden die vorhandene Draft-Logik.
Gespeicherte Werte in `gameplay.toml` sind `mob_health_bars` sowie die drei
RGB-Arrays `mob_health_friendly_color`, `mob_health_hostile_color` und
`mob_health_font_color`. Alte Dateien erhalten die bisherigen Farben als
Standard. Vorhandene Balken reagieren auf Farbaenderungen ohne neue Mob-Entities.

## Streaming-Fix

Die Baseline zeigte nicht nur GPU-/Uploadlast: `props.dispatch` blockierte den
Main Thread in einzelnen Frames bis **38.533 ms**, mit 153 Aufrufen ueber 8 ms
einschliesslich Laden. Jeder Fokuswechsel selektierte auch Tausende leere
Luft-/Felssections und registrierte sie teilweise fuer Schneedaecher.

Der neue lebende Prop-Index enthaelt nur Sections mit Vegetation. Eine
deduplizierte FIFO bearbeitet hoechstens 96 Sections pro Frame mit einem weichen
1.5-ms-Zeitbudget; laufende Fokuswechsel setzen alte Arbeit nicht zurueck.
Echte Edits werden vorgezogen. Prop-Epochen werden unabhaengig von gleichzeitigen
Wasser-/Streaming-Publikationen beachtet, auch bei einer vormals leeren Section.
Wasser-only-Publikationen erzwingen keinen globalen Vegetations-LOD-Durchlauf.
Reine LOD-Wechsel verwenden gueltige bestehende Exposure-Snapshots erneut.
Geaenderte Dach-/Wasserstempel behalten ihre bestehende lokale Invalidierung.

Sichtweite, Vegetationsdichte, Wasser und Materialqualitaet bleiben erhalten.
FIFO, Owner-Index, Epoch- und Support-Wartemengen werden beim Entladen bereinigt.
Es gibt weder unbegrenzte neue Caches noch einen neuen Renderer.

## Schwebende Vegetation

Material 54/55 (Ranken/Wurzeln) wurde bisher an jedem Voxel unabhaengig per
Noise ausgewaehlt. Dadurch konnten unter einer Deckenkandidatin isolierte
Pflanzenstuecke entstehen. Die Generierung verlangt nun eine zusammenhaengende
54/55-Kette zu echtem tragenden Gestein. Dies wird nur fuer wirkliche seltene
Pflanzenkandidaten geprueft und ruft die Generierung nicht rekursiv auf.

Bereits gespeicherte frei schwebende Ketten werden beim Laden ueber die
vorhandene autoritative Segmentqueue korrigiert. Fehlende Deckensections sind
unbekannt, nicht automatisch Luft. Bodenprops verlangen eine volle tragende
Oberflaeche und passende freie/Wasser-Rootzelle; Wasser, andere Pflanzen und
duenne Kristallsegmente sind kein Boden. Rendering, Picking und Bienen verwenden
dieselben Supportregeln. Voruebergehend unbekannte Props werden nicht dauerhaft
entfernt; eine ankommende Nachbarsection loest ihren erneuten Renderauftrag aus,
auch bei unveraenderten Owner-Epochen.

## Messprotokoll

Native Vulkan-GPU: RX 6950 XT / RADV Mesa 26.2.4. Dev-Profil des Repositories,
1920 x 1080, Sichtweite 20, Seed 2262437629133911810, Regen, vollstaendiger
Client/HUD/Minimap plus integrierter autoritativer Server. Je 60 Sekunden echte
explore-Route ueber Netzwerkeingaben; kein FPS-Limit und kein VSync. Keine
parallele Cargo-Last waehrend der Messung. Dies ist ein kurzer Streaming-Test,
keine 15-/30-Minuten-Abnahme und kein universelles FPS-Versprechen.

| Messwert, 60 s | Vorher | Nachher |
|---|---:|---:|
| Durchschnittliche FPS | 86.28 | 93.45 |
| p95 Framezeit, ms | 20.80 | 17.32 |
| p99 Framezeit, ms | 26.53 | 23.76 |
| Initiales Laden, s | 14.73 | 13.91 |
| Prozess-RSS am Ende, MiB | 2068.77 | 2078.98 |
| `props.dispatch` ueber 8 ms waehrend der Route | 147 | 0 |
| `props.dispatch` Gesamtzeit waehrend der Route, ms | 2660.48 | 1198.61 |
| Dargestellte Props am Ende | 102730 | 102730 |
| Prop-Batches am Ende | 1938 | 1938 |
| Geladene Sections am Ende | 14465 | 14465 |
| Gerenderte Terrain-Sections am Ende | 5125 | 5145 |

Der FPS-Gewinn betraegt 8.3 Prozent, die p95-Framezeit sinkt um 16.7 Prozent.
Die CPU-Scopes sind verschachtelt und duerfen nicht miteinander addiert werden.
Die hohen Prop-Auswahlspitzen verschwinden im gemessenen Bewegungsfenster;
waehrend des Startaufbaus bleibt ein einmaliger 31.12-ms-Aufruf vorhanden.
`props.select` erreicht maximal 1.58 ms, `props.prepare` maximal 2.53 ms.
Sichtweite, finaler Prop-Umfang und Materialqualitaet sind nicht reduziert.

Getrennte kontinuierliche 30-s-Fenster des vorhandenen Soak-Reporters:

| Fenster | FPS vorher/nachher | p95 ms vorher/nachher | p99 ms vorher/nachher |
|---|---:|---:|---:|
| 0..30 s | 83.95 / 91.93 | 19.39 / 17.71 | 27.65 / 24.40 |
| 30..60 s | 88.55 / 95.19 | 21.49 / 16.74 | 29.45 / 23.77 |

Das ist kein vollstaendiges Verschwinden aller Frameausreisser: im ersten neuen
Fenster wurde ein einzelner 43.94-ms-Frame gemessen, im zweiten maximal
35.26 ms. Die verbleibenden Streaming-Interest-Spitzen erreichen 9.22 ms.
GPU-Gesamtframezeit, VRAM und echte Draw-Call-Zaehler waren nicht aktiviert;
Wall-Framezeiten sind keine isolierten GPU-Zeiten. Die RSS-Differenz von rund
10 MiB ist kein nachgewiesener Memory-Leak und auch keine Speicheroptimierung.
Unterschiedliche Autoritaets-/Framephasen ergeben am Ende rund 0.5 m
Positionsdifferenz auf derselben Route. Ein Einzelvergleich ersetzt keine breite
Hardware-/Wetter-/Seed- oder Langzeitabnahme.

Rohdaten: [vorher](streaming-before.txt), [nachher](streaming-after.txt).
Reproduktion aus dem Repository-Root, fuer den jeweiligen gebauten Stand:

```bash
env TERRA_CONFIG_DIR="$PWD/tools/benchmarks/results/mob-movement-streaming-2026-10-24/configs" \
  TERRA_MENU_CONFIG_DIR="$PWD/target/phase119-benchmark-menu" \
  TERRA_BENCH_MODE=integrated TERRA_BENCH_UI=1 TERRA_BENCH_GRAPHICS=1 \
  TERRA_BENCH_SECONDS=60 TERRA_BENCH_WEATHER=rain \
  TERRA_SOAK_ROUTE=explore TERRA_SOAK_REPORT=1 TERRA_CPU_SCOPES=1 \
  WGPU_BACKEND=vulkan target/debug/terra-client --smoke-test-benchmark
```

## Pruefungen

Der volle Workspace-Testlauf besteht **2224 Tests**, drei bereits bestehende
opt-in-Pruefungen bleiben ignoriert: Audio-Preview schreiben, 180-s-UDP-Soak,
Water-Need-Mikrobenchmark. Kein neuer Langzeitlauf wurde ausgefuehrt.

Gezielte Regressionen pruefen unter anderem:

- Alle drei Spinnengroessen an gedrehten Wand-/Deckenecken mit echten Kollisionen.
- Mehrere aufeinanderfolgende volle Abwaertsstufen fuer Landtiere.
- Echte Teilflaechen/Slab-Auflage, lokale Befreiung und Erhalt der Fallbewegung.
- Gesperrte Waende, unbekannte Sections und keine Aufwaertsrettung fallender Tiere.
- Exakte Spawn-Rotation und validierte relative Cave-Spawnchancen.
- X/Shift-Belegung, Beschleunigung und 60-Hz-Uferkontakt ohne ungueltige Velocity.
- Gemeinsame Sprungpose, Sattel/Becken und fortlaufende Remote-Winkelinterpolation.
- Live-Marker ohne Detail-Tile, tiefe Wassertiere und bekannte Felsdecken.
- HP-Farben, echte Draft-/Apply-/Reset-/Discard-Zustaende und strikt drei RGB-Werte.
- FIFO-Fairness, Deduplication, Edit-Vorrang und Prop-Neuzugang bei Wasserpublikation.
- Echte Dispatch-/Upload-Abfolge mit teilweise sichtbarem ChunkFade-Batch,
  ankommendem Stuetznachbarn und unveraenderten Owner-Epochen.
- Wiederholtes Prop-Streaming/LOD, Asset-Freigabe und stale Worker-Ergebnisse.
- Reale seeded Cave-Rankengenerierung mit originalem Rohdaten-Luftspalt,
  Negativ-/Positivkontrolle, negativen Koordinaten und gemischten 54/55-Ketten.

Native Vulkan-Pruefungen verwenden isolierte Fixture-Verzeichnisse und echte
vorhandene Systeme, keine Spielerwelt. Aufnahmen wurden tatsaechlich angesehen.

- **Edit Bars:** echter Dialog mit Choice-Box und geoeffneter TiltUI-Palette,
  gebundene Farben, persistentes Apply sowie Reset/Discard. Der erste Fixture-
  Versuch bestaetigte einen zu fruehen Back-Testklick; der korrigierte Lauf
  wartet auf den tatsaechlich geschlossenen Dialog und besteht alle Stufen.
- **HP/Hitboxen:** fuenf nichtleere Screenshots und Pixel-/UI-Geometriepruefung
  fuer Looking At, View Port, verdeckte Mobs, All Time und None. Die View-Port-
  Aufnahme zeigt reale weisse Kollisionskanten und cyanfarbene Blickpfeile.
- **Reiten:** echte UDP-Mount-Anfrage, First-/Third-Person, Shift-Sprint, Sprung
  und X-Absteigen. Vorhergesagte Sprunghoehe 2.0159 m, autoritativ 2.0135 m;
  der Test prueft den tatsaechlichen Sattel-/Reiter-Beckenbezug waehrend des Sprungs.
- **Block-/Wasserinteraktion:** separater 20-s-Creative-Lauf mit HUD und Regen,
  echte Quelle -> Steinversiegelung -> Entwaessern -> Versiegelung abbauen ->
  Boden abbauen/wiederherstellen. Alle fuenf Edits bestaetigt/repliziert,
  306 temporaere Fluidrecords gehen wieder auf 0 zurueck. Kein massiver
  Rapid-Click-/Mehrspieler-Latenztest und keine gemessene Einzel-Edit-Latenz.
- **Dedicated Server:** `--no-save --smoke-test` besteht headless mit 27 Sections.

Aufnahmen: [Balken-Dialog](native/menu/mob-health-edit-bars.png),
[offene Farbpalette](native/menu/mob-health-color-picker.png),
[Balken und Hitboxen](native/health/mob-health-in-view.png),
[Sprung mit Reiter](native/riding/riding-sprint-jump.png).

Die vollstaendigen Validierungsbefehle wurden mit `CARGO_BUILD_JOBS=1` fuer die
Build-/Test-Kommandos ausgefuehrt:

```text
cargo fmt --all --check
cargo check --locked --workspace --all-targets
cargo test --locked --workspace --no-fail-fast
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo build --locked --workspace
```

Native Schalter: `--smoke-test-menu-mob-health`, `--smoke-test-mob-health`,
`--smoke-test-riding --gender=f`. Der Edit-Test verwendet die mitgelieferten
`configs-edits` (explizit Creative), `TERRA_SOAK_EDITS=1`,
`TERRA_SOAK_EDIT_CYCLES=1` und `TERRA_BENCH_SECONDS=20` statt der Flugroute.
Die normalen Vergleichsconfigs bleiben Survival. Keine Fehlermeldungen,
Slab-Allocator-Use-after-free-Logs oder Panics in den erfolgreichen nativen Laeufen.

Protokolle: [Tests](validation/tests.txt), [Check](validation/check.txt),
[Clippy](validation/clippy.txt), [Build](validation/build.txt),
[Dialog](validation/native-menu.txt), [Balken/Hitboxen](validation/native-health.txt),
[Reiten](validation/native-riding.txt), [Edits](validation/native-edits.txt),
[Headless](validation/headless-server.txt).

Die KI-Kletterbewegung ist mit der echten Simulation getestet; eine neue
native Langzeitaufnahme aller Spinnen an jeder moeglichen Spielerbauform wird
nicht behauptet. Neue Spawnchancen benoetigen gueltigen freien Raum und umgehen
keine Limits. A01/A04/A05 bleiben offen. Save-/Netzwerkschema und Dependencies
bleiben unveraendert.
