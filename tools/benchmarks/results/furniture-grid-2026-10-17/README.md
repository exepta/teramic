# Phase 46: Moebel, 3x3 Und Assetdateien

17.10.2026. Linux/Wayland, RX 6950 XT, RADV Mesa 26.2.4, Vulkan.
Funktionale Regression und native Bildabnahme, kein FPS-/Speicherbenchmark.

## Implementierung

Echtes servergeprueftes 3x3-Mustercrafting an der Werkbank, persoenliches 2x2
bleibt erhalten. Rezeptklick fuellt passende Referenzen auf bestehende echte
Inventarslots vor. Manuelle Quellen-/Feldwahl, Rechtsklick zum Leeren, Reset,
Menge, Craft Max und Cursorfertigung. Wiederverwendete Stacks werden fuer
den atomaren Verbrauch aufsummiert; es wird kein zweites Inventar erfunden.
Raster, Ergebnis und scrollbarer Bestand sind gemeinsam sichtbar.

18 einzelne Rezepte unter `assets/crafting/`, keine `terra/recipes/` mehr.
Items, Props und Creative-Kategorien sind ebenfalls einzelne JSON-Dateien.
Die bestehende Block-Registry bleibt die Quelle fuer Rendering und Mechanik:
hardness in Sekunden, resistance als Widerstandswert, required_tool und
mining_level (0 frei, 1 Holz, 2 Stein). Nicht vorhandene Explosionen werden
nicht als Funktion behauptet. Serverabbaudauer steuert die Clientanimation.

Horizontale Werkbankpaare mit vier Blickrichtungen, vollem Boden-/Freiraum-
und Spielerkollisions-Preflight. Ein Item setzt beide Zellen; Entfernen einer
Haelfte entfernt das Paar und erzeugt nur einen Drop im vorhandenen Ablauf.
Schmale Einzeltruhen verbinden sich zu breiten Doppeltruhen mit gleicher
Ausrichtung, nur seitlich und auf gleicher Hoehe, ohne dritte Verbindung.
Entfernung einer Truhe loest die Verbindung der anderen auf.

Facing/Peer/Teilflag im bestehenden SectionDelta/EditJournal, kein konkurrierender
Objektspeicher. Metadata-only-Aenderungen invalidieren auch das Mesh. REG2 liest
alte REGN-Regionen ueber explizite Legacy-Strukturen ein; PLY3 bleibt unveraendert.
Generator 5, Weltformat 2, Seed und vorhandene Block-IDs bleiben gleich.
Netzwerkprotokoll 20 erfordert gemeinsame Client-/Serveraktualisierung.

Neue editierbare statische Modelle: gewoelbter Truhendeckel, Eisenbaender und
Messingschloss; Werkbank mit Schublade, Ablagen, Vise, Werkzeugen, Plan, Tuch
und emissiver Laterne. Reproduzierbarer Builder:

```sh
node tools/assets/build_furniture.mjs
```

Der Renderer emittiert nur die Primaerhaelfte und rotiert/skaliert das gemeinsame
Modell. Keine Detailvoxel-Entities. Normale Bloecke bleiben Cube-Meshes ohne GLB.
Die Bildgenerierung lieferte fuer die neue Holztextur **1254x1254 Pixel** statt
der angefragten 2048; keine hochskalierte Datei wird als echte 2K ausgegeben.
Materialmuster wachsen 64 -> 512 Pixel, Mips 7 -> 10, Anisotropie 4 -> 8.
PNG-Mips entstehen weiterhin asynchron; fertige Handles werden geteilt.

## Pruefung

Die fuenf Workspace-Pruefungen werden mit CARGO_BUILD_JOBS=2 ausgefuehrt:

```sh
cargo fmt --all --check
cargo check --workspace
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo build --workspace
```

690 Tests erfolgreich, ein bestehender UDP-Langzeittest ignoriert. Finale
Ergebnisse und native Protokolle stehen in `logs/`. Native Aufnahmen sind
visuell geprueft; ihr Modell-Mesh-Austausch wird vor dem Capture abgewartet,
ohne die produktive Generierung oder das Streaming zu verlangsamen.

Neue Regressionen: Muster/Leerstelle und wiederholter Quellstack, alle
Rezept-Autobelegungen, Holz-/Stein-Werkzeuglevel und falsche Klassen, NaN-
Mechanikwerte, vier Moebelrichtungen, negative Chunkgrenzen, blockierte zweite
Zelle ohne Teilplatzierung, Spielerkollision, Doppel-/Dreifachtruhe, Trennung,
Metadata-only-Replikation, Legacy-REGN und REG2-Reload ueber Regionsgrenzen.
Modelltests pruefen Zellgrenzen, endliche Werte, Dreiecksflaeche und nach aussen
gerichtete Winding/Normalen. Die Cube-Texturtestmenge filtert nun tatsaechlich
Cube-Bloecke und zaehlt die neue Moebeltextur nicht als Terraintextur.
Ein aelterer Platzierungstest erhielt Boden unter seinem schwebenden Testziel,
damit er die neuen Moebelregeln prueft statt eine ungueltige Platzierung zu erwarten.

Native Probe mit echten Integrated-Servertransaktionen und TiltUI-Events:

```sh
env -u DISPLAY TERRA_CRAFTING_SMOKE=1 \
  TERRA_CONFIG_DIR=tools/benchmarks/results/daylight-2026-10-17/configs \
  TERRA_MENU_CONFIG_DIR=/tmp/terra46-menu-fresh \
  TERRA_CRAFTING_SMOKE_DIR=/tmp/terra46-world-fresh \
  TERRA_CAPTURE_DIR=/tmp/terra46-captures \
  target/debug/terra-client --smoke-test-inventory
```

Frische Verzeichnisse verwenden. Dies sind programmatisch ausgeloeste TiltUI-
HtmlEvents im nativen Client, keine manuelle Mausabnahme. Reale Zutaten werden
verbraucht, die Werkbank physisch platziert, vier Werkzeuge und zwei Truhen
hergestellt. Neue Truhen werden real platziert und zusammengefuehrt; der Host
stoppt und der binaere Speicher wird erneut eingelesen. Aufnahmen liegen unter
`captures/`. Die kompakte 960x640-Fensterkonfiguration ergibt bei Wayland-Skalierung
1440x960 physische Bildpixel, nicht einen 960x640-Pixelnachweis.

Die erste Modellprobe fand versehentlich nach innen gewickelte neue Quader.
Der Builder wurde korrigiert; der Outward-Winding-Test verhindert die Regression.
Das 3x3-Layout wurde anschliessend so umgestellt, dass der Bestand neben der
Musterbedienung erreichbar bleibt. Finale Logs stehen unter `logs/`.

## Grenzen

Truhen-Containerinventar und Deckelanimation bleiben der naechste TODO-Punkt.
Die Laterne ist emissive Geometrie, kein neues dynamisches Punktlicht. Modelle
sind statisch und blockzellenbasiert kollidierbar. Alte Einzellen-Werkbaenke werden
nicht ungefragt in Nachbarbauten vergroessert; fuer zwei Zellen neu platzieren.
Neue JSON-Rezepte verwenden bereits registrierte Itemtypen; kein allgemeiner
Modloader. Resistance wird geladen, aber noch kein Explosionssystem implementiert.

Hoehere Materialaufloesungen benoetigen mehr GPU-Speicher; kein FPS- oder RSS-
Paritaetswert wurde gemessen. Nach aussen gerichtete eindeutige Modellflaechen,
vollstaendige Mipketten und staerkere anisotrope Filterung adressieren
Oberflaechenartefakte. Einzelbilder sind kein allgemeiner Nachweis fuer
Flimmerfreiheit bewegter Schatten, TAA oder weit entfernter Vegetation.
