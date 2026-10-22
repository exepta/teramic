# Phase 31: Creative-Mengen, Wasser und Grosspilze

## Implementierung

Creative-Katalogslots und deren Detailbereich zeigen keine kuenstlichen
128er-Mengen. Echte Hotbar-, Rucksack- und Cursorstapel behalten ihren Zaehler
und das vorhandene Stacklimit. TiltUI verwendet weiterhin gemeinsame Slots.

Die alte Simulation verschob endliche Wasserpakete. Ein Paket konnte beim
Abwaertswechsel oben verschwinden, unten auftauchen und anschliessend dauerhaft
als Rest stehen bleiben. Volle Fallzellen konnten ausserdem seitlich auslaufen.
Die vorhandene Simulation berechnet jetzt lokale Sollpegel: vertikaler Zufluss
fuellt den Strang, seitlicher Zufluss verliert je gestuetztem Schritt eine
Achtelstufe. Fallendes Wasser verbreitert sich erst am Boden. Ohne Zufluss
schrumpfen Nichtquellen pro Bearbeitung eine Stufe; zyklischer Pegelerhalt ist
durch die seitliche Abschwaechung ausgeschlossen. Keine Quellensuche ueber die
ganze Welt und kein neues paralleles Fluidsystem. Bei 10 Hz bleiben maximal
256 Zellen pro Schritt aktiv; unbekannte Sections pausieren statt auszutrocknen.

Naturreservoirs bleiben implizite Quellen. Der neue Fluss ist bewusst ein
quellengespeistes Fuellstandsmodell, keine massenerhaltende Hydrodynamik.
`FluidStep` bilanziert Quelleneintrag, Nachfuellung aus Flusszellen, Rueckgang
und Rueckkehr ins Reservoir getrennt. Alte gespeicherte Nichtquellen koennen
nach Laden zurueckgehen; Format 1, Protokoll 14, Seeds und IDs bleiben gleich.
Quelle ersetzen oder Zulauf blockieren laesst den abgetrennten Ausfluss versiegen.
Vertikale gemeinsame Eckpegel und eine Abschlussflaeche nur an der freien
Fallfront verhindern sichtbare Luftschlitze. Naturwasseranschluss ist lueckenlos.

Fluid Water 27 ist client- und serverseitig nicht abbaubar, in beiden Spielmodi.
Auch direkte Netzwerk-Edit-/Miningbefehle koennen das nicht umgehen. Ersetzen
durch einen festen Block bleibt erlaubt. Solid Water 14 ist ein normaler
animierter Dekoblock und bleibt abbaubar. Pumpen und Eimer fehlen weiterhin.

Alle sechs Grosspilzfarben verwenden dieselbe Baumgenerierung und IDs 20..26.
Ein bis drei Kappen wachsen deterministisch; Seitenkoepfe sitzen an ueberlappenden,
geknickten und steigenden Armen. Ganze Strukturen sind flaechenverbunden und
bleiben innerhalb der vorhandenen Suchgrenzen. Near fuegt zwei Rippen oder
zwei bis drei Flecken pro ausgewaehlter freier Flaeche hinzu, maximal 72 Vertices.
Middle/Far und gemeinsame Material-/Section-Batches bleiben erhalten. Keine
Entity pro Mikrodetail. Uneditierte v5-Pilzstrukturen erhalten die neue Form
auch beim Regenerieren vorhandener Welten; alte Blockedits behalten Koordinaten.

## Validierung

Alle fuenf angeforderten Befehle erfolgreich: fmt --check, check --workspace,
test --workspace, clippy --workspace --all-targets mit -D warnings, build --workspace.
562 Tests bestanden; ein schon vorher vorhandener Langzeit-Soak bleibt ignoriert.
Der komplette letzte Lauf lief mit normaler paralleler Testausfuehrung und
`CARGO_BUILD_JOBS=2`, ohne ausgeschlossene Tests.

Neue/erweiterte Tests pruefen Nichtabbaubarkeit in beiden Modi, direkte UDP-Edits,
Quellenersatz, 13 m Fall ohne Seitenplatten, natuerlichen Reservoiranschluss,
Rueckgang hinter einem verschlossenen Zulauf, 784 zyklisch verbundene Restzellen,
Save/Reload mitten im Rueckgang, negative/vertikale Meshgrenzen, gueltige Indizes,
geschlossene Fallfront und alle sechs verbundenen Pilzvarianten.

Separater `cargo build -p terra-server` erfolgreich. Start ohne DISPLAY und
WAYLAND_DISPLAY beendet den Smoke-Test mit 27 Sections und Exit 0. Der getrennte
normale Dependency-Baum enthaelt weder Renderer noch TiltUI, wgpu oder winit.

Native Vulkan-/Wayland-Pruefungen:

- Integrated-/UDP-Quelle: 108 Nasszellen, fuenf gezielt entfernte Props, zwei echte
  Wassermeshes mit 346 Dreiecken. Server verweigert Abbau; Stein verdraengt Quelle;
  saemtliche dynamischen Restzellen verschwinden. Zwei Aufnahmen, Exit 0.
- Offline-Fallfixture: 15 Quellen, 30 Blockhoeheneinheiten, mehrere vertikale
  Sections. Vollstaendiger Strang ohne Luftloechern oder seitliche Zwischenplatten.
  Geoeffnetes Section-Grenztor laesst 70 Pegel-Einheiten in die rechte Kammer;
  Wiederverschliessen trocknet diese vollstaendig aus, Quellen links bleiben.
  Terrain-Mesh-Epochen aendern sich bei Fluidschritten nicht. Drei Aufnahmen, Exit 0.
  Die Steinwanne ist ein kontrollierter Testaufbau, keine erzeugte Landschaft.
- Echter Wayland/uinput-Inventarlauf: Auswahl, Hover, Slots ohne Katalogzahlen,
  echte Hotbarzahlen, Split/Drag/Sweep, kompakte/breite Ansicht und Aussenwurf.
  65 Inventar + 63 replizierter Drop = 128. Drei Aufnahmen, Exit 0.
  Erster Versuch waehrend des Linkens traf ein Input-Acknowledgement-Timeout;
  getrennte Wiederholungen ohne Compiler liefen erfolgreich.
- Generierter Mushroom Park: Nah-/Uebersichtsaufnahme einschliesslich echter
  Seitenkoepfe und Oberflaechenrelief, keine GPU-/Shaderfehler, Exit 0.

## Fester Pilzparkblick: Vorher/Nachher

Gleicher Seed 42042, Welt 1/v5, archivierte `configs`, 18 s stationaere Messung,
1920x1200 physische Pixel, Wayland 150%, RADV RX 6950 XT, Mesa 26.2.4, VSync aus,
Sichtweite 16/3, LOD 96/160, Wind-/Maskenlaub aktiv. Jeweils genau ein eigener
GPU-Prozess, keine parallelen Cargo-Jobs. Beide erfolgreichen Rohlogs archiviert.
Der Vorher-Build wurde vor dem ersten Workspace-Build separat aufbewahrt.

| Metrik | Vorher | Nachher |
|---|---:|---:|
| Durchschnitt FPS | 145.470 | 144.167 |
| p95 Frame ms | 8.171 | 8.244 |
| p99 Frame ms | 9.162 | 8.951 |
| Erstladen s | 3.310 | 3.258 |
| Terrain-Workerarbeit s | 0.723190 | 0.725049 |
| Prop-Workerarbeit s | 0.320949 | 0.316516 |
| Opaque-GPU-Pass Mittel ms | 1.226607 | 1.226776 |
| RSS nach Messfenster KiB | 853376 | 907596 |
| Peak RSS KiB | 868756 | 923172 |
| Geladene Sections | 7807 | 7809 |
| Gerenderte Sections | 4115 | 4118 |
| Meshassets | 9636 | 9687 |
| Sichtbare Meshentities | 4546 | 4596 |
| Repraesentierte Props | 92789 | 92784 |
| Prop-Batches | 1306 | 1306 |

Neue Seitenkoepfe und Near-Relief aendern bewusst die Geometrie dieses Blicks.
Das ist keine isolierte Optimierungs-A/B-Messung und kein Speedup: rund 0.9%
weniger mittlere FPS sowie rund 53 MiB mehr Peak RSS in diesem Paar. Weder
allgemeine Speicherparitaet noch verbesserte Streaming-Spitzen werden behauptet.
Die Szene enthaelt kein Wasser und keinen Controller/Netzwerkverkehr, sie misst
nicht die Kosten grosser Fluidnetze. Einzelne GPU-Passzeiten sind keine gesamte
GPU-Framezeit; Draw Calls wurden in diesem Paar nicht instrumentiert.
Mehrstuendige Last-/Mehrspielerabnahme und menschliche Art-Abnahme bleiben offen.

## Reproduktion

Vom Repository-Root, Client vorher bauen; Befehle getrennt ausfuehren:

```bash
env -u DISPLAY TERRA_CONFIG_DIR=tools/benchmarks/results/fluid-recession-mushrooms-2026-10-16/source-configs TERRA_CAPTURE_DIR=/tmp/terra-fluid-fall TERRA_FLUID_HEIGHT=30 target/debug/terra-client --smoke-test-fluid
env -u DISPLAY TERRA_CONFIG_DIR=tools/benchmarks/results/fluid-recession-mushrooms-2026-10-16/source-configs TERRA_MENU_CONFIG_DIR=/tmp/terra-source-menu target/debug/terra-client --smoke-test-fluid-source
env -u DISPLAY TERRA_CONFIG_DIR=tools/benchmarks/results/fluid-recession-mushrooms-2026-10-16/configs TERRA_BIOME_SCENE=mushroom TERRA_CAPTURE_DIR=/tmp/terra-mushroom target/debug/terra-client --smoke-test-biomes
env -u DISPLAY TERRA_CONFIG_DIR=tools/benchmarks/results/fluid-recession-mushrooms-2026-10-16/configs TERRA_BENCH_SECONDS=18 TERRA_RENDER_DIAGNOSTICS=1 python3 tools/benchmarks/measure.py target/debug/terra-client --smoke-test-forest
```
