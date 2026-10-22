# Phase 102: Teilblock-Wasser, Streaming Und Render-Lifecycle

## Umfang

Diese Runde bearbeitet O01, O03, O04, O05 und O06 aus TODO.md. Bestehende
Sichtweiten, Vegetationsdichte, Materialidentitaeten und Wasserqualitaet bleiben
erhalten. Keine Dependency-Upgrades, neuen Block-IDs, Speicher-/Netzwerkformate
oder Generatorversionen. Dedicated verwendet weiter dieselbe headless Weltlogik.

## Wasser An Slabs Und Treppen

`water/space.rs` verwendet die vorhandenen Konstruktion-Boxprofile als feste
Teilvolumina. Wasser belegt nur deren freien Raum. Der bestehende Achtelwert
bleibt die absolute Oberflaechenhoehe innerhalb einer Zelle; Holz und Stein
werden davon abgezogen. Die gespeicherte FluidCell-Struktur bleibt unveraendert.
Unter-, Ober- und Vertikalslabs, gemischte Vollslabs und alle 24
Treppenausrichtungen verwenden dieselbe Geometrie wie die Kollisionsabfrage.

Freie gemeinsame Zellseiten steuern Zufuhr und Gravitation. Eine untere Slab
traegt den Wasserfilm auf ihrer oberen Haelfte, eine obere Slab hat darunter
freien Raum; eine geschlossene gemeinsame Seite uebertraegt kein Wasser.
Ersetzen einer expliziten Quelle durch einen Teilblock entfernt deren
Source-Eigenschaft. Nicht mehr gespeistes Restwasser zieht sich zurueck.

Nur Konstruktion-Grenzen verwenden exakte ausgeschnittene Wasserpolygone.
Offene Wasserflaechen behalten den bestehenden geglaetteten Mesher und Shader.
Die Fragmente bleiben in Section-Patches gebuendelt, ohne Entity/Material
pro Fragment. Gespeicherte Orientierungen und natuerliches Reservoirwasser
werden auch beim ersten Nachladen in den korrigierten Patch uebernommen.

Punktabfragen fuer Stroemung, Schwimmen, Atemluft, Unterwasserkamera, Tinte und
Tier-Wassererkennung unterscheiden feste und nasse Teilvolumina. Das ist
Waterlogging fuer die vorhandenen Slab-/Treppenkonstruktionen, kein neuer
allgemeiner Fluid-Solver fuer beliebige bewegte oder importierte Meshes.

## Fluessigkeits-Lifecycle

Der bisherige Sleeping-/Budgetpfad bleibt bestehen. Neu entfernt
`suspend_unloaded_fluids` aktive und schlafende Arbeitsbesitzer, deren eigene
Section den geladenen Bereich verlassen hat. Gespeicherte Wasserzellen werden
nicht geloescht. Geladene Zellen, die auf ungeladene Nachbarn warten, bleiben
erhalten; Reload erzeugt die benoetigte Arbeit wieder aus den gespeicherten
Edits. Es wird keine zweite Simulation eingefuehrt.

Leere Wasserpatches benoetigen keinen GPU-Uploadslot mehr, um ein altes Tile
zu entfernen. Ein ausgeschoepftes Framebudget haelt daher kein bereits
verschwundenes Wasserbild fest.

Wasserpatch-Publikationen besitzen ein eigenes `WaterPublished`-Ereignis.
Zuvor erschien die geaenderte ChunkStore-Ressource ohne Terrainereignis und
loeste konservative globale Terrain-/Kartencache-Scans aus. Nun erkennen diese
Verbraucher den Wasser-only-Lifecycle; die Karte aktualisiert die betroffene
Spalte. Reset und veraltete Job-Ergebnisse bleiben abgesichert.

## Streaming Und Kartenplanung

Die bisherige Dispatch-Buendelung durchsuchte fuer jeden Spaltenjob die gesamte
wartende Section-Liste. `SectionQueue` erhaelt deren Distanzreihenfolge fuer
den naechsten Job und indiziert dessen vertikale Geschwister. Die Buendelung
liest nur diese eine Spalte. Ein Regressionstest mit 29575 Sections prueft
sechs besuchte Geschwister statt einer Vollsuche, ohne Prioritaetsverlust.
Distanzschluessel fuer eine neue Interessensliste werden einmal pro Section
berechnet. Eine Aenderung der vertikalen Sichtweite wird nun auch bei
stationaerem Fokus beruecksichtigt; zuvor pruefte der Cache nur den Radius.

Die Karte braucht pro Job maximal vier Live-Snapshots und 128 Arbeitseintraege.
Begrenzte Heaps waehlen diese direkt aus, statt alle Kandidaten zu sammeln und
vollstaendig zu sortieren. Ein 20000-Kandidaten-Test vergleicht die Auswahl
gegen den vollstaendigen Sortiervertrag und prueft die festen Groessenlimits.
Distanz und bisherige Live-Prioritaet bleiben erhalten; gleiche Prioritaeten
erhalten eine stabile Koordinatenreihenfolge. Bei grossen historischen
Katalogen werden nur Koordinaten des sichtbaren Rechtecks nachgeschlagen;
bei einer sehr weit herausgezoomten, sparsamen Karte ist die Katalogiteration
guenstiger. Die Auswahl verwendet jeweils den kleineren Suchraum und besitzt
Tests fuer negative und extreme Koordinaten. Die Kartenaktualisierung laeuft
nach dem Streaming-Dispatch, sodass dessen Publikationen im selben Frame
beruecksichtigt werden.

Im ersten 32er-Nachlauf blieben 64 globale Kartenindex-Rebuilds mit bis zu
14,78 ms bestehen. Der Empfang bestaetigter Section-Deltas markierte den
gesamten ChunkStore und das Journal auch bei identischen oder veralteten
Snapshots als geaendert. Prop-only-Importe besassen zudem keine lokale
Lifecycle-Nachricht. Der echte Empfangspfad validiert weiterhin jeden Import,
vergleicht danach aber die bestehenden Revisions-/Mesh-/Prop-/Appearance-/
Fluid-/Wasser-Epochen. Nur tatsaechliche Aenderungen setzen Change Detection
und senden `SectionEditApplied`. Die Karte aktualisiert die betroffene Spalte;
Terrain bestaetigt die Nachricht ohne einen unnoetigen globalen Arrival-Scan.
Fehler mit bereits geaendertem Zustand behalten den konservativen Fallback.
Tests verwenden den echten Empfangshelper, pruefen Duplikate, veraltete und
ungueltige Payloads sowie gleichbleibende Revisionen mit geaenderten Bloecken.
Eine unveraenderte entfernte Kartenspalte behaelt ihre Revision.

## Vegetation Und Blockdetails

Windgewichte werden einmal pro Prop-Template statt pro Instanz berechnet.
Die Komposition reserviert bekannte Batchgroessen vorab; entfernte Props
werden direkt in das vorhandene Packed-Normal-/Farbformat geschrieben.
Die frueheren temporaeren Floatarrays von zusammen 28 Byte pro Vertex
entfallen in diesem Pfad. Rotierte Normalen werden einmal pro Face berechnet,
unveraenderte Terrainfaces benoetigen keine temporaere Einzel-Face-Liste.

Nach Geometrie-, Licht- und Windbearbeitung komprimiert der Worker geeignete
Indizes von U32 auf U16. Indexwerte und Dreiecke bleiben identisch; grosse
Batches behalten U32. Das halbiert deren Indexbytes in Ready-Queues und
GPU-Indexbuffers, nicht den gesamten Mesh- oder VRAM-Verbrauch. Beide Formate
werden vom bestehenden Upload-/Replacementpfad akzeptiert.

Regressionen vergleichen Wind fuer alle Arten/Varianten/Skalen, direkte
Packed-Geometrie mit dem bisherigen Floatpfad sowie Indexgrenzen und reale
Grass-/Busch-/Pilz-/Korallen-/Seegras-Batches bei allen drei LODs. Dichte,
Blattluecken, Alpha-Modi, Formen und Materialzuordnungen werden nicht reduziert.
Bestehende begrenzte Fade- und Laubcaches bleiben erhalten.

## Licht Und Exposition

`shadow_quality = Off` gilt nun auch fuer erzeugte und bereits vorhandene
Voxel- und gehaltene Fackellichter. Zuvor stellte deren laufendes Update
Schatten unabhaengig von dieser Option wieder an. Lichtstaerke, Reichweite
und Auswahlbudgets bleiben unveraendert; Off/Low/High-Roundtrips behalten
denselben Emitterpool.

Hoehlennebel und Wasserfall-Dunst teilen sich Bevy-Komponenten. Getrennte
Cave-/Mist-Claims und eine feste Reihenfolge verhindern, dass der eine Pfad
beim Austritt die noch benoetigten Komponenten des anderen entfernt. Der
Hoehlenpfad verwendet weiterhin 32 Schritte, der Mistpfad erhaelt danach seine
24 Schritte zurueck. Nicht mehr benoetigte VolumetricLight-/Fog-Komponenten
werden entfernt statt ausserhalb der Hoehle haengenzubleiben.

Die vorhandene lokale Dach-/Kroneninvalidierung bleibt bestehen: nur betroffene
Spalten werden budgetiert neu gesampelt, unveraenderte Masken loesen keinen
Appearance-Rebuild aus. Regressionen pruefen hohe nachladende Daecher und
unveraenderte entfernte Geometrie. Der Soft-Core-Guard prueft weiterhin den
tatsaechlichen gepinnten Bevy-Shadervertrag und weist zusaetzliche nicht
uebersetzte Radiuszugriffe zurueck.

## Validierung Und Messwerte

### Cargo Und Native Funktionen

Finaler Quellstand, jeweils mit `--locked` und zwei Buildjobs:

- `cargo fmt --all --check`: erfolgreich.
- `cargo check --workspace`: erfolgreich.
- `cargo test --workspace`: 1734 bestanden, 0 fehlgeschlagen, 2 bestehende ignoriert.
- `cargo clippy --workspace --all-targets -- -D warnings`: erfolgreich.
- `cargo build --workspace`: erfolgreich, Client und Dedicated Server.

Rohlogs: `phase102-{fmt,check,tests,clippy,build}-final2.log`.
Der Headless-Server lief ohne DISPLAY/WAYLAND_DISPLAY und mit `--no-save`
erfolgreich durch (`phase102-server-final.log`).

Der native Fluidtest verwendet normale autoritative Block-Edits, die installierten
Simulations-Ticks und asynchrone Wasserjobs. Am Section-Uebergang x=16 fliesst
Wasser durch eine echte untere Slab: gemessene Oberflaeche 133,875, fester Boden
133,5; die feste Haelfte bleibt trocken. Danach wird die Oeffnung verschlossen,
das gespeiste Restwasser drainiert und die Terrain-Meshepochen bleiben bei
reinen Fluidschritten unveraendert. Alle drei Aufnahmen wurden geschrieben.
Nach dem erfolgreichen Testende protokolliert Bevy einmal einen geschlossenen
Screenshot-Kanal beim Shutdown; das ist keine Slab-Allocator-/WGSL-Meldung,
wird aber im Rohlog nicht ausgeblendet (`phase102-fluid-partial-final.log`).

Der native Lichttest besteht alle 31 Ansichten: Tages-/Nachtwechsel, laufende
Sonnenbewegung, Schatten Off -> Low/MSAA -> High/TAA, Unterwasser mit MSAA/TAA
und die Sonnen-/Mond-Ueberlappung. 24 wirklich platzierte Fackeln halten den
lokalen Lichtpool bei Qualitaetswechseln; die gemessenen Budgets 4/8/16 bleiben
eingehalten. Off hat null lokale Schattenwerfer. Der gepinnte reale
Soft-Core-Shadervertrag wurde erkannt. Alle Ansichten besitzen nichtleere
Screenshot-Pixel; keine ERROR-/WARN-Zeile im Lichtlauf
(`phase102-daylight-final.log`).

Die finalen Aufnahmen `native/fluid/fluid-flowing.png`,
`native/daylight/shadow-high-taa.png` und `native/daylight/underwater-msaa.png`
wurden tatsaechlich angesehen. Die Fluidaufnahme ist eine Uebersicht des
Beckens, keine bildbasierte Nahvermessung des Slab-Wasserfilms. Volumen und
Grenzflaechen sind zusaetzlich durch Geometrie-/Punktabfragetests abgesichert.
Eine Referenz-Art-Abnahme wird damit nicht behauptet.

Zwei erste native Versuche fanden Fehler in alten Testfixtures: die
Fluessigkeitspruefung liess ausschliesslich Air/Water zu, und die Lichtfixture
schrieb Fackeln nur ins Journal statt ueber den echten Editpfad. Die Fixtures
pruefen jetzt reale freie Teilvolumina beziehungsweise echte geladene
Voxel-/Meshepochen. Kein produktiver Fehler wurde durch gelockerte Grenzwerte
oder hoehere Timeouts verdeckt; die ersten fehlgeschlagenen Logs bleiben
als `phase102-fluid-partial.log` und `phase102-daylight.log` erhalten.

### Vergleichsbedingungen

Native Linux-/Vulkan-Laeufe auf Ryzen 9 5900X und Radeon RX 6950 XT mit
Mesa 26.2.4 RADV, optimiertem Dev-Profil, VSync/FPS-Limit aus. Integrierter
autoritativer UDP-Server, Survival-HUD, Minimap und TiltUI bleiben aktiv.
Hohe Schatten-/Wasser-/Blockqualitaet, Ultra-Wetter, Rise-Chunkanimation,
LOD-Distanz 1,6 und volle Vegetationsdichte bleiben unveraendert.
Keine gleichzeitigen Compiler oder weiteren Testclients. Der uebrige Desktop
lief weiter; dies ist keine exklusive Laborumgebung.

Messfenster: 45 s reale Gehsteuerung mit Hin-/Rueckweg und 20 s fuer die
stationaeren Vergleichsszenen. Die Ladephase wird separat erfasst.
CPU-Wall-Frames ohne Screenshot-Nachbarframes; RSS am Messende, nicht Peak-RAM.
Keine GPU-Passzeiten, isolierte Overdraw- oder VRAM-Messung. Neue
10-/15-/30-Minuten-Laeufe wurden wie gewuenscht nicht durchgefuehrt.

### Tatsaechliche Vergleichswerte

`vorher` ist der vor dieser Runde gemessene Stand; `final` enthaelt auch die
Korrektur der Section-Delta-Invalidierung. p95/p99 sind Millisekunden,
RSS ist MiB am Messende. Der erste Regen-Nachlauf und dessen unveraenderte
Wiederholung stehen absichtlich beide in der Tabelle.

| Szene / Seed | Aufloesung / Sichtweite | FPS vorher -> final | p95 vorher -> final | p99 vorher -> final | RSS vorher -> final |
|---|---|---:|---:|---:|---:|
| Gehen, 42042, 45 s | 2880x1620 / 12 | 108,75 -> 104,13 | 12,70 -> 12,94 | 15,23 -> 14,84 | 1343,74 -> 1325,30 |
| Regen, 2262437629133911810, 20 s | 3840x2160 / 20 | 87,29 -> 59,75 | 15,95 -> 25,39 | 24,79 -> 26,77 | 1793,55 -> 1828,64 |
| Regen, unveraenderte Wiederholung | 3840x2160 / 20 | 87,29 -> 90,97 | 15,95 -> 14,91 | 24,79 -> 21,89 | 1793,55 -> 1784,30 |
| Schnee 3, 104729, 20 s | 2880x1620 / 12 | 108,07 -> 109,84 | 13,61 -> 13,37 | 16,22 -> 15,72 | 1379,65 -> 1362,29 |
| Grosse Sichtweite, 42042, 20 s | 2880x1620 / 32 | 76,68 -> 75,42 | 20,19 -> 16,32 | 39,30 -> 36,84 | 2927,46 -> 2955,94 |

Auch die erste Geh-Baseline lag bei nur 53,15 FPS/p99 35,94 ms; bereits ihre
unveraenderte Wiederholung erreichte 108,75 FPS/p99 15,23 ms. Dieser Ausreisser
bleibt als `phase102-before-clear.log` erhalten; die Tabelle verwendet klar
benannt `phase102-before-clear-repeat.log`. Zwischenmessungen vor dem letzten
Delta-Fix (`phase102-after-*.log`) lagen bei Gehen 111,62, Regen 89,75,
Schnee 113,35 und Sichtweite 32 bei 78,89 FPS. Diese Schwankungen verhindern
eine serioese Behauptung eines allgemeinen FPS-Gewinns durch diese Runde.
Der schlechtere finale Regenlauf ist in seiner Ursache nicht bewiesen;
die gleiche binaere Datei und Konfiguration liefen direkt danach wieder
mit 90,97 FPS. Keine Qualitaet wurde dafuer veraendert. Der Monitor-Warnhinweis
beim Borderless-Start bleibt protokolliert; die tatsaechliche 4K-Aufloesung
wurde jeweils kontrolliert.

Bei Sichtweite 32 bleiben 29890 geladene/18458 gerenderte Sections,
35182 Mesh-Assets und 3178 Prop-Batches erhalten. Der Regenvergleich hat
unveraendert 11798/7211 Sections, 17520 Mesh-Assets, 1982 Prop-Batches und
987 Wassertiles mit 463876 Dreiecken. Die Gehscene enthaelt weiter 37270
Props; kleine Abweichungen anderer lebender Szenen sind im Rohlog sichtbar.
Kein pauschaler RAM-Gewinn: insbesondere der finale 32er-Lauf benoetigt
rund 28,48 MiB mehr RSS. Initiale Ladezeiten sind ebenfalls kein neuer
Geschwindigkeitsgewinn: rund 12,2/30,3/12,4/67,7 s fuer Gehen/Regen/Schnee/32.

Die gezielten CPU-/Datenkorrekturen sind dagegen direkt nachvollziehbar:

| Messung | Vorher | Final |
|---|---:|---:|
| 32: globale Kartenindex-Rebuilds | 78 | 1 beim Start |
| 32: Kartenindex gesamt / Maximum | 880,13 / 14,11 ms | 0,011 / 0,011 ms |
| 32: Kartenplanung gesamt / Maximum | 3600,86 / 20,98 ms | 279,19 / 1,23 ms |
| 32: Kartenplanung-Aufrufe ueber 8 ms | 76 | 0 |
| 32: Streaming-Dispatch gesamt / Maximum | 343,19 / 1,32 ms | 81,91 / 0,79 ms |
| Gehen: Kartenplanung gesamt / Maximum | 283,27 / 3,59 ms | 70,33 / 0,61 ms |
| Prop-Workerzeit je Job: Gehen | 0,547 ms | 0,368 ms |
| Prop-Workerzeit je Job: Regen-Wiederholung | 0,214 ms | 0,152 ms |
| Prop-Workerzeit je Job: Schnee | 0,534 ms | 0,340 ms |
| Prop-Workerzeit je Job: Sichtweite 32 | 0,283 ms | 0,213 ms |
| Indexbytes im identischen 7-Arten-/3-LOD-Test | 1255104 | 627552 |

Scope-Werte enthalten auch Startup und sind wegen Verschachtelung nicht
addierbar. Die Propzeiten sind Worker-Komposition, keine komplette Framezeit
oder GPU-Zeit. Sie sinken hier je nach Szene rund 24 bis 36 Prozent pro Job.
Der Indextest verwendet identische Instanzen und Dreiecke; er halbiert nur
Indexdaten, nicht alle Meshdaten. Siehe `phase102-vegetation-budget.log`.

Der finale Flug-Kurzlauf mit Seed 42042 steigt per realen replizierten Inputs
von Y=113,91 auf rund 190,31 und fliegt anschliessend bis X=297,23.
45 s ergeben 109,52 FPS, p95 12,68 ms, p99 15,05 ms und 1273,12 MiB RSS.
Am Ende: 5623 geladene/1680 gerenderte Sections, 5164 Mesh-Assets,
29332 Props in 417 Batches; maximal 96 aktive Chunkanimationen. Nur ein
globaler Kartenindex-Aufbau beim Start. Es gibt hier keinen gemessenen
Vorher-Flugpartner und keinen vollstaendigen Flug-Rueckweg;
`phase102-final-fly.log` ist ein Funktions-/Lastnachweis, kein A/B-Gewinn.

### Reproduktion Und Grenzen

Die verwendeten TOML-Dateien liegen unter `configs/{clear,rain,snow,wide}`;
`configs/native-config` gehoert zu den Funktionsfixtures. Profilidentitaeten
und die grossen Mapping-Caches werden nicht als Benchmarkassets kopiert.
Dadurch kann Cache-Warmup bei einer neuen Reproduktion abweichen.

Beispiel nach erfolgreichem Build aus dem Repository-Hauptverzeichnis:

```bash
TERRA_CONFIG_DIR="$PWD/tools/benchmarks/results/render-water-2026-10-22/configs/clear" \
TERRA_MENU_CONFIG_DIR="$PWD/tools/benchmarks/results/render-water-2026-10-22/configs/clear" \
TERRA_BENCH_MODE=integrated TERRA_BENCH_UI=1 TERRA_BENCH_GRAPHICS=1 \
TERRA_BENCH_SECONDS=45 TERRA_SOAK_ROUTE=walk TERRA_SOAK_ROUTE_DELAY_SECONDS=5 \
TERRA_CPU_SCOPES=1 TERRA_SOAK_REPORT=1 \
target/debug/terra-client --smoke-test-benchmark
```

Regen/Schnee verwenden ihre jeweiligen Configs, 20 Sekunden und
`TERRA_BENCH_WEATHER=rain` beziehungsweise `TERRA_BENCH_WEATHER='snow 3'`
ohne Bewegungsroute. Sichtweite 32 verwendet `configs/wide`, 20 Sekunden
und kein Wetteroverride. Der Flug-Kurzlauf verwendet `clear` und
`TERRA_SOAK_ROUTE=explore`; dies sind echte replizierte Steuerinputs,
keine kameraseitige Teleport-Rundfahrt.

Grenzen bleiben explizit:

- Keine allgemeine FPS-/Speicherparitaet und kein Langzeit-Leak-Nachweis.
- Einzelne hohe Frames bleiben bestehen; vorangehende Job-/Uploadzaehler sind
  Korrelation, kein bewiesener GPU-Stall. 32er-p99 bleibt bei rund 37 ms.
- Fokuswechsel haben weiter endliche Wanted-/Eviction-/Sortierkosten;
  die Basispolitik und Detaildistanzen wurden nicht verschlechtert.
- Windlaub-/Blend-Overdraw sowie isolierte Nah-/Mittel-Pilzpark-/Riffkosten
  und VRAM auf weiteren GPUs benoetigen die gesonderte Abnahme A05.
- Zwei weit entfernte Foci mit getrenntem Verlassen und gemeinsamem
  Wasser-Pause-/Reload-Wakeup sind noch nicht als kombinierter Lauf getestet;
  Union, Unload, Sleeping und Wakeup sind einzeln durch Regressionen abgedeckt.
- Original-Schattengeometrie bleibt nach Off -> On korrekt; eine erneute
  Proxyoptimierung kann bis zur regulaeren Sectionkomposition warten.
- Die native Lichtfixture ersetzt keinen vollstaendigen menschlichen
  Bewegungs-/Flackervergleich oder die Art-Abnahme A01.
