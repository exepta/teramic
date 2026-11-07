# O07: Framezeiten und Streaming-Spitzen

05.11.2026. Zwei gemessene CPU-Ursachen behoben: wiederholter Indexaufbau
beim Import dichter Section-Deltas und unnoetige HTML-Invalidierungen durch
native Kalenderanimation. Sichtweite, Geometrie und Grafikqualitaet bleiben
erhalten. Die sechs Szenarien sind mit einem endlichen Matrixlauf wiederholbar.

## Ergebnis

Je Szenario ein gueltiger Vorher-/Nachher-Lauf, gleiche Konfiguration und
Hardware. Werte sind gemessene CPU-Wall-Frameintervalle; kleine Unterschiede
sind bei diesen kurzen Einzelmessungen kein belastbarer allgemeiner FPS-Gewinn.
`p99` bezeichnet das 99. Perzentil der Framezeit, keinen separat berechneten
1%-Low-FPS-Mittelwert.

| Szenario | AVG FPS vorher → nachher | p95 ms vorher → nachher | p99 ms vorher → nachher | Maximum ms vorher → nachher |
| --- | ---: | ---: | ---: | ---: |
| Klare Sicht, neue Welt | 108,43 → 110,40 | 11,49 → 10,94 | 13,25 → 12,51 | 15,80 → 16,09 |
| Flug in neue Chunks | 91,58 → 98,12 | 14,87 → 14,09 | 18,00 → 16,54 | 26,30 → 24,37 |
| Dorf, klare Sicht | 48,63 → 49,93 | 22,18 → 21,99 | 23,76 → 23,18 | 29,21 → 26,68 |
| Regen am Dorf | 48,12 → 46,66 | 24,70 → 25,27 | 26,91 → 27,84 | 30,83 → 29,92 |
| Schnelle Platzierungen | 95,48 → 100,83 | 14,13 → 13,69 | 15,37 → 15,08 | 19,28 → 17,44 |
| Inventar offen, stationaer | 78,62 → 105,17 | 14,36 → 12,54 | 16,17 → 13,96 | 17,53 → 16,96 |

Das offene Inventar gewinnt in dieser Messung 33,8% AVG FPS. Die geschlossenen
Kontrollphasen bleiben bei etwa 109 FPS. Knoten-/Bindingzahlen sind identisch:
991/1028/1031 UI-Knoten und 32/382/32 Property-Binding-Knoten fuer
geschlossen/offen/wieder geschlossen. Die Optimierung entfernt keine Inhalte.

Beim Regen sinkt AVG FPS um 3,0%, p99 steigt um 3,4%. Dieser Lauf belegt keinen
Regengewinn; Wetter-/Schattenkosten werden durch diese Aenderung nicht geloest.
Auch das Maximum bei klarer Sicht ist geringfuegig hoeher. Der Bericht macht
keine allgemeine Mindest-FPS-Zusage.

## Behobene Ursachen

### Section-Deltas

`EditJournal::import_delta_with_simulation` (Scope `world.import_section_delta`)
pruefte Metadaten wiederholt durch lineare
Scans ueber alle Bloecke. Danach wurde fuer jede einzelne geaenderte Zelle der
Objekt-/Hold-Index der gesamten Section neu aufgebaut. Bei dichten
Konstruktionsdaten entstand quadratische Arbeit auf dem Clientpfad.

Die Validierung verwendet jetzt die ohnehin benoetigten finalen HashMaps.
Historische doppelte Blockeintraege behalten ihre bisherige Any-Match-Pruefung
und Last-Write-Speicherung. Der Indexaufbau erfolgt einmal nach Installation
der finalen Metadaten. Normale lokale Edits aktualisieren den Index sofort;
auch Fehlerpfade synchronisieren den tatsaechlichen Teilzustand.

Der gezielte CPU-Mikrobenchmark nutzt eine Section mit 4096 Bloecken und
Objekten sowie acht Holds, 200 einzelne Objektupdates und vier Vollupdates.
Das Klonen des Eingabepakets liegt ausserhalb der Messung.

| Import | Aufrufe | Mittel ms vorher → nachher | p95 ms vorher → nachher | Maximum ms vorher → nachher |
| --- | ---: | ---: | ---: | ---: |
| Einzelnes Objekt in dichter Section | 200 | 5,445 → 1,369 | 5,992 → 1,547 | 6,708 → 2,557 |
| Alle Objektposen | 4 | 518,018 → 9,346 | 550,926 → 9,748 | 550,926 → 9,748 |

Vollupdates sind damit in dieser CPU-Probe etwa 55-mal schneller. Vier
Vollupdates erlauben keine robuste statistische Aussage ueber seltene
Ausreisser. Diese konstruierten Importkosten sind keine nativen GPU- oder
FPS-Werte und werden nicht mit der Flugmessung gleichgesetzt.

### Inventar und Kalender

`MiniMapView.season_cycle` und `frame_rotation` werden nur von nativen
Kalendermaterialien benutzt. Ihre kontinuierlichen Werte gelangten dennoch in
den globalen HTML-Snapshot. Dadurch wurden unter anderem die lexikalischen
Bindings des offenen Inventars bei jeder Aenderung erneut ausgewertet.

Beide Felder werden jetzt bei der HTML-Serialisierung ausgelassen. Die
nativen Uniforms und ihre bisherige Animationsfrequenz bleiben erhalten.
Ein echter TiltUI-Integrationstest prueft 120 unterschiedliche
Animationsschritte ohne HTML-Snapshot-/Change-Tick-Aenderung, die exakten
nativen Materialwerte und weiterhin sichtbare Aktualisierung des Uhrtexts.

Im nativen offenen Inventar sinkt die mittlere `ui.reactivity`-Wall-Zeit
von 4,275 auf 1,103 ms und `main.update` von 6,429 auf 3,121 ms.
`main.post_update` bleibt bei etwa 4,5 ms. Die sparse Importarbeit des
Fluglaufs ist dagegen bereits klein (nachher 17 Aufrufe, Mittel 0,0071 ms);
die konstruierten dichten Imports erklaeren nicht pauschal alle nativen FPS.

Die sieben vorhandenen Biomvorschaubilder sind ausserdem im statischen
Frontend-Preload registriert. Die Native-Matrix startet ohne diesen
Frontend-Preload; daraus wird kein Matrix-FPS-Gewinn abgeleitet.

### Verbleibender erster Oeffnungsframe

Der erste Inventaruebergang bleibt langsam. Die Matrix meldet etwa 98–102 ms
fuer ihren schlechtesten Oeffnungsframe. Eine zusaetzliche Timelineprobe mit
frischem Anforderungszeitstempel **nach** dem Phasenbericht bestaetigt
101,086 ms und insgesamt 143,750 ms bis zum sichtbaren, gesetzten Layout.
Der Phasenbericht allein erklaert den Ruckler daher nicht.

`main.update` braucht dort 92,061 ms, die enthaltene Reaktivitaet 10,424 ms;
`main.post_update` braucht 6,811 ms, das enthaltene Layout 1,491 ms.
Etwa 80 ms liegen nach Ende der Reaktivitaet in noch nicht einzeln
instrumentierter Update-Arbeit. Der private TiltUI-Stylepass ist aufgrund
seiner Reihenfolge und der rekursiven Behandlung geaenderter Inline-Styles
ein Kandidat, keine gemessene Ursache. Die bestehende Oeffnungs-/Fokuslogik
wurde nicht aufgrund dieser Vermutung veraendert. Der engere Befund bleibt
als **O08** offen. Die alten Matrix-Uebergangszeiten enthalten zudem
Berichtsarbeit vor dem Request und werden nicht als bewiesener Gewinn
beim Oeffnen verwendet; die stationaeren zwoelfsekundigen Fenster sind
davon getrennt.

## Messaufbau und Reproduktion

- Linux, AMD Radeon RX 6950 XT, RADV NAVI21, Vulkan,
  Mesa 26.2.5-arch3.1. GPU-/Backend-Umgebung und Adapter stehen in den Manifesten.
- Normaler `cargo build -p terra-client`: Dev-Profil mit `opt-level=1`,
  Dependencies sowie terra-render/terra-voxel mit `opt-level=3`.
- Seed 42042, Sichtweite 20, 2562 × 1440 physische Pixel, VSync aus,
  kein FPS-Limit, Schatten/Wasser/Wetter/Blockdetails/Laub hoch,
  Mikrogeometrie bis 48 m. Die eingefrorenen TOMLs und Hashes liegen bei.
- Die Breite der isolierten Messkonfiguration ist um zwei Pixel gegenueber
  der lokalen 2560er-Vorgabe erhoeht: Bei DPI 1,5 rundete der Window-Manager
  sonst auf 2561 und der strikte Aufloesungscheck schlug fehl. Kein Paar
  vergleicht unterschiedliche Aufloesungen; Projektkonfigurationen bleiben erhalten.
- Integrierter Server, echtes TiltUI und gerenderte Welt. Dorf und Regen
  verwenden Kamera `-585.5,33.65,976.5`, Blickziel `-583.5,36.6,981.5`.
  Replication und sichtbare Bewohner werden verlangt. Die Hilfsroutine sucht
  tatsaechlich generierte Doerfer statt generische Koordinaten zu behaupten.
- Je 24 s fuer klar/Dorf/Regen und 35 s fuer Flug/Platzierung nach Bereitschaft.
  Inventar misst dreimal 12 s mit separaten Uebergangs-/Beruhigungsphasen.
  Lade- und Regenbereitschaft liegen ausserhalb dieser Stichprobenfenster.
- Flug nutzt echte freigegebene Eingaben mit Serverpositionsbestaetigung.
  Dynamische Animationen und zeitbasierte Streamingpfade koennen zwischen
  Laeufen leicht unterschiedliche Endstaende erzeugen.
- Platzierung nutzt 120 Zyklen mit 240 autoritativ bestaetigten Stein-/Luft-Edits
  an derselben Zelle. Nur dieser Fall erhaelt eine explizite Creative-Ableitung
  der Serverkonfiguration. Inventargrant und Hotbarauswahl laufen ueber die
  Produktionsnachrichten. Der Mindestabstand zwischen Edits betraegt 105 ms.

```sh
cargo build -p terra-client
cargo run -p terra-world --features bevy --example performance_scene -- \
  --configs tools/benchmarks/results/o07-frame-spikes-2026-11-05/configs
python3 tools/benchmarks/run-frame-spikes.py \
  --binary target/debug/terra-client \
  --configs tools/benchmarks/results/o07-frame-spikes-2026-11-05/configs \
  --output /tmp/teramic-o07-repeat \
  --scenarios clear rain village flight inventory placement \
  --rain-camera=-585.5,33.65,976.5 \
  --rain-look-at=-583.5,36.6,981.5 \
  --village-camera=-585.5,33.65,976.5 \
  --village-look-at=-583.5,36.6,981.5
```

Der Runner friert TOMLs ein, isoliert Profile/Welten pro Fall, entfernt
geerbte Benchmarkoptionen und lehnt ueberschriebene Ergebnisse ab.
Ein Exit-Code allein ist kein Erfolg: Er verlangt effektive Konfiguration,
echte Szenarioaktivitaet und vollstaendige Messzusammenfassungen.
Vorher-Binaries wurden aus demselben Arbeitsstand nur mit den beiden
Produktionsoptimierungen zurueckgenommen gebaut; fuer Platzierung enthielt
auch die Baseline die korrigierte Messfixture. Keine Dependency-Upgrades.

Die anfangs ungueltigen Proben sind ausgeschlossen: DPI-Rundung,
Schnee statt Regen am kalten Standardspawn, zur Bodenflaeche gerundete
Teleporthoehe, fehlender gehaltener Stein und eine zu strenge
Gras-Wiederherstellungsannahme. Die finale Fixture erlaubt ausschliesslich
die normale Umwandlung von Gras zu Erde unter dem gesetzten Stein und
meldet Wiederherstellung ausdruecklich nur fuer die bearbeitete Zelle.

## Abgrenzung der Messgroessen

CPU-Scopes messen Wall-Zeit instrumentierter Abschnitte. Verschachtelte oder
nebenlaeufige Scopes werden nicht zu einer Framezeit addiert. Ganzes Main-
und Render-Schedule, Handoff und Extract stehen getrennt in den Daten.
Die Matrix aktiviert keine GPU-Pass-Timestamps; DRM-VRAM/GTT und
Meshbufferkapazitaeten sind Speicherwerte, keine GPU-Rechenzeiten.

Eine separate initiale Klarwetterprobe mit GPU-Pass-Timestamps misst im Mittel
2,281 ms fuer den Main-Opaque-Pass, 1,294 ms fuer den fruehen Prepass und
0,055 ms fuer den UI-Pass. Diese drei von 26 Passserien sind kein kompletter
GPU-Frame und duerfen wegen Verschachtelung/Parallelitaet nicht addiert werden.
Es gibt keinen passenden GPU-Timestamp-Nachherlauf und damit keine belegte
GPU-Zeitverbesserung. Die Probe ist von der FPS-Matrix getrennt.

Queue-/Uploadzaehler umfassen zum Teil auch den Szenenaufbau. Maximal
zugelassene Upload-Vertices/-Bytes sind keine gemessenen GPU-Stalls; die
Worst-Frame-Kontexte liefern Korrelation, keine kausale GPU-Zuordnung.
Asynchrone Terrainjob-Maxima sind ebenfalls keine Main-Thread-Framezeiten.

Die Nachher-Laeufe melden als kumulative Spitzen etwa 219–223 Tausend
zugelassene Vertices und 9,6–10,2 MiB pro Frame, ueber die Kategorien
summiert. Beim Flug bleiben am
Ende 24 Animationen eingereiht, davon keine im Sichtbereich, sowie 72 aktiv
(Spitze 96). `deferred_upload_batches` zaehlt Zurueckstellungsentscheidungen,
keinen aktuellen Rueckstau. Bei den jeweils schlechtesten erfassten Frames
fuer klar/Flug/Dorf/Platzierung waren im vorausgehenden Kontext keine
Upload-Vertices gemeldet, beim Regen 72172. Das grenzt die Korrelation ein,
ersetzt aber keine GPU-Stallmessung.

Rapid-Edit-Latenzen messen Anforderung bis autoritative Antwort und
replizierte Zelle, nicht den ersten sichtbaren Frame. Bei allen 240 Antworten
waren Render- und Collider-Aktualisierung noch ausstehend. Auch die finale
Antwort garantiert diese Bereitschaft nicht; anschliessend laeuft das
Messfenster noch weiter. Daraus wird kein sofort fertiger Collider behauptet.

RSS/DRM-Verlaeufe und Prozessmaxima schliessen Start, Reise und Assetaufbau
ein. RSS ist nachher in allen sechs Faellen hoeher (etwa 1–4%); VRAM-Spitzen
schwanken in beide Richtungen. Es gibt keinen belegten allgemeinen
Speichergewinn. Ein erster Inventarzugriff erhoeht die Bildzahl um 36, die beim Schliessen
als Cache erhalten bleiben. Die kurzen Laeufe beweisen weder ein Leck noch
langfristig begrenzten Speicher. Die weitere Hardware-/Langzeitabnahme A05
bleibt offen; keine neuen zehnminuetigen oder laengeren Laeufe ausgefuehrt.

## Validierung

Workspace-Library-/Bin-Suiten und korrigierter Client-Nachlauf: insgesamt
3804 bestanden, neun ignoriert, keine verbleibenden Fehler. Der erste
Workspace-Lauf fand zwei Clientfehler: fehlende vorhandene Biompreloads und
eine uninitialisierte Slotliste in einem neuen Fixturetest; beide behoben
und die gesamte Client-Suite erneut ausgefuehrt (197 bestanden).
Ein weiterer Exampletest prueft den Teleportabstand bei 478 Bodenhoehen.
Acht Python-Tests pruefen Runner, Parser und Ablehnung ungueltiger Laeufe.
Build, Clippy aller Workspace-Targets mit `-D warnings`, gezieltes Rustfmt
und Diff-Pruefung erfolgreich.

Der ignorierte Importbenchmark bleibt explizit aufrufbar:

```sh
cargo test -p terra-world --lib \
  edits::journal::import_performance_tests::benchmark_dense_section_import_updates \
  -- --ignored --nocapture --test-threads=1
python3 -m unittest discover -s tools/benchmarks -p test_run_frame_spikes.py
```

Maschinenlesbare Vergleichsdaten, Einzelzusammenfassungen und Manifeste
liegen in diesem Verzeichnis. Lokale Profile, gespeicherte Welten und
gigabytegrosse Binaries sind keine Berichtsartefakte.
