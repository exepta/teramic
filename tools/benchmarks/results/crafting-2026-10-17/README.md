# Phase 45: Rezeptmontage Und Groessere Slots

17.10.2026. Rust 2024, bestehende Dependency-Versionen; Linux/Wayland,
RX 6950 XT, RADV Mesa 26.2.4, Vulkan. Dies ist ein Funktions-/Layoutnachweis,
kein FPS-, p99- oder Speicher-Paritaetsbenchmark.

## Umfang

Gemeinsamer RecipeBook-Katalog mit 18 Grundrezepten, stabilen Namenraum-IDs,
geformten/formlosen Zutaten, Mod-TOML/JSON und servervalidierten Transaktionen.
Die Werkbank verwendet eine rezeptbasierte Montageansicht ohne 3x3-Rasterzwang.
Das persoenliche 2x2 bleibt erhalten. Werkbank 28/Truhe 29 sind platzierte
Custom-Mesh-Objekte; normale Bloecke bleiben im Cube-Mesher. Vier Werkzeuge und
Staebe haben gemeinsame 3D-Vorschauen/Drops und JSON-Beschreibungen.

Alle Slotmasse sind um 25 Prozent erhoeht: Inventar 54 -> 67.5 px,
HUD-Hotbar 49 -> 61.25 px, Handraster 36 -> 45 px. Responsive Varianten
48 -> 60, 42 -> 52.5 beziehungsweise 32 -> 40 bleiben erhalten.
Die vorhandene UI-Skalierung wirkt zusaetzlich auf diese logischen Masse.

Neue Item-Enumvarianten sind angehaengt; alte Block-/Welt-IDs, Seed,
Generatorversion 5 und PLY3-Spielerdaten bleiben erhalten. Netzwerkprotokoll
19 erfordert gemeinsam aktualisierte Clients und Server.

## Tests

Ausgefuehrt mit `CARGO_BUILD_JOBS=2`:

```sh
cargo fmt --all --check
cargo check --workspace
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo build --workspace
```

Alle fuenf Befehle erfolgreich. **680 Tests bestanden**, ein bestehender
Langzeittest ignoriert. Ein vorheriger Gesamtlauf scheiterte beim Start eines
Netzwerktests an einer bereits belegten TCP-Testportnummer. Der unveraenderte
vollstaendige Wiederholungslauf besteht; dies war kein Crafting-Assertfehler.
Finale Ausgaben liegen in `logs/`, der fehlgeschlagene Zwischenlauf separat
als `test-port-collision.log`.

Regressionen pruefen Katalogvalidierung, Mod-Muster/Luecken, Mehrfachbelegung
eines Zutaten-Slots, atomare Mehrfachfertigung, volle Inventare, Cursorgrenzen,
Revisionswechsel und neue Item-Speicherung. Echte UDP-Tests verwenden zwei
Clients mit getrennten Inventaren: Replay kann keine zweite Truhe erzeugen;
entfernte, nicht erreichbare und ungueltige Werkbankpositionen werden abgelehnt.
Der Server akzeptiert dichten Stein erst mit echter besessener Spitzhacke
und verwendet ihre tatsaechliche Geschwindigkeit. UI-Tests pruefen Suche,
Mengengrenzen, Werkbankfehler, Vorschau und Queue ohne lokale Itemerzeugung.

Spezialmodelle werden auf endliche Koordinaten, Zellgrenzen, gueltige Indizes
und nicht degenerierte Dreiecke getestet. Eine Regression verhindert doppelte
Terrainkontakte unter partiellen Modellen; die Abbauanimation folgt dem
serverseitigen Werkzeugbonus statt noch die Handgeschwindigkeit zu begrenzen.
Die frueheren All-Cube-/27-Slots-Annahmen in Asset-/Inventar-Savetests wurden
auf Cube-Ausnahmen beziehungsweise mehrere Inventare mit je 27 Slots angepasst.

## Native Abnahme

Reproduzierbarer integrierter Test:

```sh
env -u DISPLAY TERRA_CRAFTING_SMOKE=1 \
  TERRA_CONFIG_DIR=tools/benchmarks/results/daylight-2026-10-17/configs \
  TERRA_MENU_CONFIG_DIR=/tmp/terra-crafting-menu-fresh \
  TERRA_CRAFTING_SMOKE_DIR=/tmp/terra-crafting-world-fresh \
  TERRA_CAPTURE_DIR=/tmp/terra-crafting-captures \
  target/debug/terra-client --smoke-test-inventory
```

Die Testwelt muss frisch sein. Die Fixture beginnt mit echten gespeicherten
Zutaten (8 Holz, 12 Stein, 12 Sand), deaktiviert nur fuer diesen Test Biome,
Hoehlen, Wasser und Hoehenvariation. Seed/Generatorversion bleiben gleich.
Nach echtem UDP-Join werden die materialisierten TiltUI-Eventhandler mit
HtmlEvents angesteuert, nicht durch manuelle OS-Mausklicks. Reale
Servertransaktionen erzeugen Planks per Craft Max, Staebe, Werkbank, vier
Werkzeuge und eine Truhe. Die Werkbank wird mit einem echten Bauauftrag
platziert; Werkbankrezepte benoetigen ihre serverseitig bestaetigte Position.
Am Ende stoppt der Host, und der binaere Speicher wird erneut eingelesen.

Aufnahmen in `captures/`: grosses Inventar, Handmontage, platzierte Werkbank,
Werkbankmontage, kompakte Montage, hergestellte Werkzeuge im Inventar und
HUD-Hotbar. Native Ansichten wurden visuell geprueft. Basisfenster 1280x800;
die kompakte Fixture setzt 960x640 logische Fenstereinheiten, unter Wayland
hier 1440x960 physische Capture-Pixel. Das ist kein 960x640-Pixelbildnachweis.

Die erste Abnahme fand einen realen Importfehler: Materiallabel-Nachladen mit
anderen glTF-Settings entfernte die benoetigte Szenenquelle. Identische Settings
fuer beide Ladepfade beheben ihn. Weitere Aufnahmen fanden fehlende Zutaten-
Bindings und dichte Labels; direkte Loop-Kontexte und explizite Spaltenlayouts
beheben diese. Unbekannte Icons wurden durch vorhandene TiltUI-Symbole ersetzt.

Headless-Pruefung:

```sh
env TERRA_CONFIG_DIR=tools/benchmarks/results/daylight-2026-10-17/configs \
  target/debug/terra-server --smoke-test --no-save
cargo tree -p terra-server --edges normal
```

Der Server startet ohne Fenster/Renderer und beendet die Generierung von 27
Sections im Smoke-Test. Sein normaler Dependency-Baum enthaelt weder
terra-render/terra-ui noch bevy_render/bevy_winit/wgpu.

## Grenzen

Die Truhe besitzt noch kein Containerinventar; dies ist der naechste TODO-Punkt.
Werkzeughaltbarkeit ist noch nicht implementiert. Rezeptmods koennen bestehende
Items nutzen; die vorhandene Item-Registry wird nicht zu einem allgemeinen
Modloader umgebaut. Die eingebaute Holzpalette unterscheidet aktuell Oak- und
Birch-Stamm, aber keinen eigenen Spruce-Stamm. Die vorhandenen Spruce-Planks
bleiben als Zutat nutzbar. Custom-Mesh-Szenen sind statisch, ihre Blockkollision
bleibt zellbasiert. Keine Shader-/Terrain-Generator-Aenderung, kein neues
Speichersystem und kein behaupteter Performancegewinn.
