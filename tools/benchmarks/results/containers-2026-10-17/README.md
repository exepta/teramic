# Phase 49: Truheninhalte, Inventarraum Und Modellabbau

17.10.2026. Funktions- und Regressionsabnahme, kein FPS-Benchmark.
Der Bericht beschreibt die protokollierten Workspace-, Netzwerk- und nativen
Funktionspruefungen nach den Layout- und Testfixturekorrekturen. Alle fuenf
Cargo-Pruefungen, erzwungener nativer Slot-Scrolltest, Werkbank-/Truhen-Modellabbau
und Headless-Start sind bestanden. Die unten genannten fachlichen Grenzen und
fehlenden Performance-/Langzeitnachweise bleiben bestehen.

## Umgebung Und Kompatibilitaet

- AMD Ryzen 9 5900X, 12 Kerne/24 Threads, rund 32 GiB RAM.
- AMD Radeon RX 6950 XT, Vulkan/RADV, Mesa 26.2.4-arch3.1 laut nativen Logs.
- CachyOS/Linux 7.2.9-2-cachyos, native KDE/Wayland-Desktop-Sitzung.
- Rust 1.95.0, Edition 2024, vorhandene Dev-Profile und unveraendertes Lockfile.
- Bevy 0.19.1, Avian3D 0.7.0, Lightyear 0.30.1, TiltUI 0.1.0;
  keine Dependency-Upgrades.
- Client und Server gemeinsam auf Protokoll 23. Neue Regionrecords REG4;
  explizite Leser fuer REG3, REG2 und REGN. Spielerdateien weiterhin PLY3.
  Weltformat 2, Generatorversion 5, Seeds und bestehende Material-/Welt-IDs gleich.

## Technischer Umfang

Die vorhandene Truhe (ID 29) besitzt 18 Slots pro physischer Haelfte,
verbundene Doppeltruhen 36 Slots. Bestehendes `Inventory`, Stacklimit 128 und
Spielercursor werden weiterverwendet. Atomare Gesten pruefen Indexgrenzen,
Drag-Duplikate, Kapazitaet, Itemtypen und Revisionen vor einem gemeinsamen
Spieler-/Containercommit. Inhalte liegen privat im `EditJournal`, nicht in
oeffentlichen SectionDelta-Nachrichten. Inhaltstransfers invalidieren keine
Terrainmeshes. Nichtleere physische Haelften sind gegen Abbau/Ersetzen geschuetzt;
eine leere Haelfte kann sich trennen, ohne den Inhalt ihrer Nachbartruhe zu verlieren.

REG4 speichert die Inhalte nach globaler Position im bestehenden atomaren
Mehrdatei-Commit. Leser pruefen Koordinaten, Regionseigentum, Duplikate,
aufgezeichnete Truhen und gueltige cursorfreie Bestaende. Kein zweites Save-System.

TiltUI behaelt den Inventarrahmen zentriert, der Browser verwendet rechts die
verbleibende Breite und 95 Prozent Fensterhoehe. Inhaltsbereiche scrollen.
Normales Mausrad scrollt; **Ctrl+Mausrad** uebertraegt einzelne Items.
Acht dezente Typ-Silhouetten bilden links neben dem Charakter ein 2x4-Raster.
Sie sind deaktivierte Positionen, kein bereits funktionierendes Ausruestungssystem.

Shaped-Rezepte pruefen ihren ganzen Fussabdruck. Shapeless-Matching behandelt
ueberlappende generische und exakte Zutaten reihenfolgeunabhaengig. Reserviertes
Crafting verbraucht exakt die validierte Zuordnung. Werkbaenke erhalten eine
gemeinsame Zweizellen-Auswahl; Truhen bleiben getrennte physische Ziele.
Importierter Modellabbau verwendet budgetiert die wirklichen Oberflaechen,
keine ausgefuellten Ersatzwuerfel.

## Vorliegende Automatische Pruefungen

| Pruefung | Ergebnis | Nachweis |
| --- | --- | --- |
| Format | Bestanden, Exit 0 | [fmt.log](fmt.log) |
| Workspace-Check | Bestanden, Exit 0 | [check.log](check.log) |
| Workspace-Tests | 781 bestanden, 0 fehlgeschlagen, 1 bestehender Langzeittest ignoriert | [tests.log](tests.log) |
| Clippy mit `-D warnings` | Bestanden, Exit 0 | [clippy.log](clippy.log) |
| Workspace-Build | Bestanden, Exit 0 | [build.log](build.log) |
| Dedicated-Server-Start | Bestanden ohne Display-Session, 27 Sections, sauberer Exit | [headless.log](headless.log) |
| Dedicated-Abhaengigkeitspruefung | Keine bevy_render-, bevy_ui-, bevy_winit-, TiltUI- oder wgpu-Abhaengigkeit im Serverbaum | [server-tree.log](server-tree.log) |

Ein Zwischenlauf traf den bestehenden Survivalfall
`dead_reconnect_and_respawn_keep_inventory_counters_and_uuid_but_reset_prediction_generation`:
Der Test wartete auf `Ready`, aber noch nicht auf die separat eintreffende private
Inventarnachricht. Dadurch war sein anschliessender Slotzugriff gelegentlich leer.
Die Testfixture wartet jetzt auf wiederhergestellte Werte und Inventar; die
Produktionssynchronisation wurde dafuer nicht abgeschwaecht. Der anschliessende
vollstaendige Lauf mit `--test-threads=2` bestand ohne Fehler. Die sechs neuen
Container-UDP-Faelle bestanden auch im zuvor fehlgeschlagenen Gesamtlauf.

Sechs neue echte UDP-Containerfaelle bestanden: private Snapshots nur fuer
Oeffner und Updates an beide Betrachter; verdeckte/zu entfernte Ziele;
nichtleere Haelften und Topologieaenderung; veraltete Revisionen, ungeoeffnete
Anker und ungueltige Slots; Doppeltruhen-Klick/Drag/Wheel/Shift; Schliessen,
Unload, Disconnect und Wiederbeitritt derselben UUID mit erhaltenem Cursorbestand.
Weitere Tests decken atomare Gesten/Commits, REG4-Korruption und Altdatenmigration,
Shaped-Raender/Lochmuster, Shapeless-Zuordnung und echte Modelloberflaechen ab.
Der unveraendert ignorierte Test ist der bestehende 180-Sekunden-UDP-Soak.

Reproduktionsbefehle des abschliessenden Workspace-Laufs:

```bash
cargo fmt --all --check
CARGO_BUILD_JOBS=2 cargo check --locked --workspace
CARGO_BUILD_JOBS=2 cargo test --locked --workspace -- --test-threads=2
CARGO_BUILD_JOBS=2 cargo clippy --locked --workspace --all-targets -- -D warnings
CARGO_BUILD_JOBS=2 cargo build --locked --workspace
cargo tree --locked -p terra-server
```

## Native Pruefungen

**Crafting und Truhen:** Der abschliessende Wiederholungslauf mit aktuellem
Binary endete erfolgreich mit `Crafting native smoke passed` und Exit 0.
Ein echter Integrated Server, 2x2/3x3-Reservierungen,
Werkzeugfertigung, Zweizellen-Werkbank, Einzel-/Doppeltruhenplatzierung,
TiltUI-Containertransfers und binaeres Region-/Spieler-Save/Reload bestanden.
Die Containeransichten zeigen echte 18/36 Slots und uebertragene Bestaende,
keine vorbefuellten produktiven Testtruhen. Protokoll unter
[native-crafting.log](native-crafting.log), Aufnahmen unter [captures](captures).
Die abschliessende Inventar-/Scrollprobe prueft das korrigierte Layout separat.

**OS-Mausgesten:** [native-inventory.log](native-inventory.log) bestaetigt
echte Cursoraufnahme, Halbieren, Einzelablage, Verteilung, Sammelziehen und
Crafting-Grid-Drag mit Serverantworten. Der reale Aussenklick erhaelt alle
128 Items: 65 im Inventar und 63 als replizierter Weltdrop. Die Probe endet
mit `Native inventory mode/layout acceptance completed` und sieben Aufnahmen
unter [inventory-captures](inventory-captures).
Der erste Lauf erzwang noch keinen ueberlaufenden Slotbereich; der erfolgreiche
Folgelauf begrenzt gezielt nur dessen Test-Viewport auf 120 physische Pixel.
Ein echtes OS-Mausradereignis verschiebt den Inhalt bis zur letzten Reihe,
die anschliessend per echter Maus erreichbar ist. Picking der letzten Reihe,
Rueckscrollen sowie unveraenderte Cursor-/Itembestaende werden geprueft.
[inventory-scroll-reachable.png](inventory-captures/inventory-scroll-reachable.png)
zeigt den erreichten Bereich. Diese Einschraenkung ist eine Testfixture,
keine 120-Pixel-Begrenzung des produktiven Inventars.

**Modellabbau bestanden:** Die aktuellen [native-workbench.log](native-workbench.log)
und [native-chest.log](native-chest.log) enden jeweils mit
`Native furniture surface mining passed`. Beide Proben platzieren das Objekt echt,
verwenden autoritativen Survival-Abbau, erfassen vier sichtbare Geometriestufen und
pruefen null Wuerfel-Riss-/Debriseffekte. Abbruch stellt das urspruengliche Modell
wieder her. Jeweils sechs Aufnahmen liegen unter [furniture-workbench](furniture-workbench)
und [furniture-chest](furniture-chest); sie wurden im uebergeordneten Arbeitslauf
visuell geprueft. Das ist keine allgemeine Abnahme aller moeglichen Modmodelle.

Der erste Werkbankversuch scheiterte bei `FurnitureAim` mit
`Furniture fixture could not enter authoritative survival mode`. Die Ursache war
die fehlende ausdrueckliche Gamemode-Berechtigung des Testprofils. Nur die
Fixture erstellt nun ihr eigenes Profil mit dieser Berechtigung; produktive
Serverberechtigungen wurden nicht aufgeweicht. Die aktuellen beiden Erfolgslogs
stammen aus den danach wiederholten Proben.

**Visuelle Layoutabnahme:** Die nach den ersten Screenshots korrigierten
Silhouetten und Pagination wurden im uebergeordneten Arbeitslauf erneut nativ
gesichtet. [inventory-open.png](inventory-captures/inventory-open.png) zeigt acht
deutliche Ausruestungs-Silhouetten, zentrierten Inventarrahmen und Browser nach
100 logischen UI-Pixeln Abstand. Seitenzaehler stehen korrekt zwischen den
beiden sichtbaren Pfeilen. Die Scrollaufnahme wurde ebenfalls gesichtet.
Das ist eine gezielte Funktions-/Layoutpruefung, keine allgemeine 4K-/Mehr-GPU-Abnahme.

## Native Reproduktionsvorlagen

Frische temporaere Menue-/Weltpfade verwenden, keine vorhandenen Spielwelten.
Die Craftingprobe verlangt einen noch nicht bestehenden `world`-Unterordner.
Physische Mausgesten benoetigen den vorhandenen uinput-/KWin-Helfer
`tools/ui/wayland_pointer.py` und eine ungestoerte Desktop-Sitzung.
Es werden keine Bevy-Picking-Ereignisse als Ersatz fuer OS-Mausgesten injiziert.

```bash
env -u DISPLAY TERRA_CRAFTING_SMOKE=1 TERRA_NATIVE_OS_POINTER=1 \
  TERRA_CONFIG_DIR=tools/benchmarks/results/daylight-2026-10-17/configs \
  TERRA_MENU_CONFIG_DIR=/tmp/terra49-menu-crafting-fresh \
  TERRA_CRAFTING_SMOKE_DIR=/tmp/terra49-world-crafting-fresh \
  TERRA_CAPTURE_DIR=tools/benchmarks/results/containers-2026-10-17/captures \
  target/debug/terra-client --smoke-test-inventory

env -u DISPLAY TERRA_NATIVE_OS_POINTER=1 \
  TERRA_CONFIG_DIR=tools/benchmarks/results/daylight-2026-10-17/configs \
  TERRA_MENU_CONFIG_DIR=/tmp/terra49-menu-inventory-fresh \
  TERRA_CAPTURE_DIR=tools/benchmarks/results/containers-2026-10-17/inventory-captures \
  target/debug/terra-client --smoke-test-inventory

env -u DISPLAY TERRA_BUILD_CREATIVE=1 TERRA_FURNITURE_MINING_SMOKE=workbench \
  TERRA_CONFIG_DIR=tools/benchmarks/results/daylight-2026-10-17/configs \
  TERRA_MENU_CONFIG_DIR=/tmp/terra49-menu-workbench-fresh \
  TERRA_CAPTURE_DIR=tools/benchmarks/results/containers-2026-10-17/workbench-captures \
  target/debug/terra-client --smoke-test-building

env -u DISPLAY -u WAYLAND_DISPLAY target/debug/terra-server \
  --bind 127.0.0.1:0 --no-save --smoke-test
```

Fuer die Truhen-Modellprobe `TERRA_FURNITURE_MINING_SMOKE=chest` und eigene
frische Menue-/Aufnahmepfade verwenden. Diese Vorlagen setzen die oben genannte
native Desktop-/uinput-Umgebung voraus.

## Grenzen

Kein Ofen-Backend, keine nutzbaren Ausruestungsitems/-werte, keine Deckelanimation
und keine neue Dimensionensimulation. Nichtleere Truhen werden vor dem Abbau
geleert, statt ihren Inhalt automatisch zu zerstreuen. Lokale UUID-Sitzungen
beweisen keinen verifizierten Online-Account. Loopback-UDP ersetzt keine
Mehrrechner-/Internet-/Abbruch- oder Langzeitabnahme auf echter Netzwerkinfrastruktur.

Keine Vorher-/Nachher-Messung fuer FPS, p95/p99, GPU-Zeit, Draw Calls, RSS oder
VRAM durchgefuehrt. Funktionstests und Aufnahmen belegen keine Performanceparitaet,
4K-/Mehr-GPU-Vollabnahme oder vollstaendige Referenz-Art-Qualitaet.
