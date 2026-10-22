# Phase 58: Hoehlenlicht, Fackeln, Kontakt-Foley Und Bienen

## Architektur

Die vorhandene Mesh-/PBR-/Prop-/Tierpipeline bleibt erhalten. Keine Aenderung
der Terrain-Generierung, IDs 0..39, Weltidentitaeten, Seeds oder Generatorversion.
Fackel = neue angehaengte Block-ID 40, Bee = angehaengte Tierart, BeeNest =
angehaengter Prop-Typ. Bestehende REG7-/PLY3-Dateien bleiben lesbar.
Netzwerkprotokoll 32 verhindert unvereinbare alte Clients auf neuen Servern.
Der Dedicated Server erhaelt keine Render-/Audioabhaengigkeit.

Ein nativer Kaltstart deckte zudem konkurrierende glTF-Unterassetladungen auf.
Der Import verwendet nun vorhandene PBR-Unterhandles des einmal geladenen
Root-Modells, statt erneute Label-Ladevorgaenge anzustossen. Nicht verwendete
Paletteneintraege benoetigen keine Materialslots. Dadurch bleibt die fuer den
statischen Scene-Import erforderliche glTF-Quelle erhalten; die Cube-Pipeline
und bestehenden Custom-Modelldefinitionen bleiben kompatibel.

Die native Kontrolle hat eine weitere konkrete Ursache freigelegt: Bevy 0.19
kopiert die Atmosphaeren-Lichtstaerke nur beim Erzeugen in die generierte und
gefilterte Environment-Komponente. Der alte Code veraenderte nur
`AtmosphereEnvironmentMapLight`, die PBR-wirksamen Werte blieben auf 1.05.
Nun werden auch `GeneratedEnvironmentMapLight` und `EnvironmentMapLight`
nach den Bevy-Update-Systemen synchronisiert; ein Regressionstest prueft
Hoehle, Uebergang und Rueckkehr auf die Oberflaeche.
Die Aerial-View-LUT der Aussenatmosphaere wurde ebenfalls im Inneren angewendet:
Sie kennt keine Steinwaende und streute bei niedrigerem Hoehlen-EV stark blau.
Ihre Integrationsstrecke wird deshalb weich auf einen Zentimeter reduziert,
ohne die Himmelsdarstellung am Eingang abzuschalten. Bei vollstaendigem
Verlassen werden die 32000 Meter der bisherigen Oberflaeche wiederhergestellt.
Der eigene kammerabhaengige Nebel und Ultra-Volumetrik bleiben davon getrennt.
Die Augenbelichtung blendet auf EV100 6.0. Sonne/Mond werden mit demselben
Belichtungsverhaeltnis kompensiert, damit direkt besonnte Eingangswaende nicht
ausbrennen; ihre Bildschirmhelligkeit bleibt auf Oberflaechenniveau. Lokale
Lichter profitieren von der Hoehlenbelichtung. Dies ist bewusst stilisiertes
Direktlicht, keine physikalische Belichtungs-/GI-Simulation.

Das alte Cave-Grading hatte ausserdem zwei problematische Eigenschaften: pauschalen
blauen Black-Lift/Fog zusaetzlich zur Welt-Fog und keine definierte Reihenfolge
gegenueber TAA im selben EarlyPostProcess-Set. Der Pass ist jetzt vor TAA
geordnet, hebt Schwarz nicht farbig an und verwendet sehr geringe neutrale
Entfernungshaze nur in hohen Raeumen. Unvollstaendige Roof-Probes duerfen den
letzten aufgeloesten Hoehlenzustand nicht durch vermeintlich freien Himmel
ersetzen. Die Welt-Fog wird vor der abschliessenden Umgebungsanpassung geschrieben.

AO arbeitet mit opaker 18^3-Belegung, gespeichert als 92 u64-Woerter = 736 Byte
je nichtleerem Mesh. Im Renderworker werden gemeinsame Gitterecken gecacht,
bilinear auf Detailvertices interpoliert und mit bestehenden Farbwerten
multipliziert. Keine Geometrie-Tessellation oder neue Terrain-Draws notwendig.
An Edits beteiligte diagonale Halo-Sections werden mit invalidiert. Fuer
gewisse Spezialgeometrie ist dies Kontaktabschattung, keine exakte Dreiecks-AO.
Opake Schattenproxies erhalten rueckseitige Dreiecksindizes derselben Vertices:
Innen sichtbare Decken muessen auch von oben Schatten werfen, insbesondere
wenn der Terrain-Aussenboden noch nicht geladen ist. Keine zusaetzlichen
Draws/Vertices/Materialkopien, aber mehr Schattenindizes. Maskiertes Laub bleibt
unveraendert. Die Testszene blendet fremde Generatorberge ausserhalb ihrer
eigenen Felskammer aus; andernfalls waere deren Dach statt der Testoeffnung
fuer den Sonnenlichteinfall verantwortlich.

Punktlichtpositionen liegen ausserhalb der opaken Fackelflammen und
Kristallspitzen. Gleichfarbige Erzgruppen teilen in sechs Metern Abstand
einen Emitter. Alle 250 ms werden hoechstens 64 sortierte Kandidaten auf
Sicht durch bekannte Zellen geprueft; fremde geschlossene Raeume verbrauchen
keine Schattenmap. Quellen innerhalb sechs Metern bleiben fuer Licht um
nahe Tueroeffnungen zugelassen. Dies ist kein raumbasiertes Portal-/GI-System.
Licht-Scans ueberlagern auf Cache-Misses die Journal-Edits mit einem temporaeren
8-KiB-Zellpuffer: neue/entfernte Fackeln benoetigen keinen fertigen Mesh-Rebuild.
Ein Regressionstest prueft beide Richtungen gegen absichtlich veraltete Blockdaten.

Qualitaeten, Datenformate und Bedienung stehen im
[Projekt-README](../../../../README.md#hoehlenlicht-fackeln-und-bienen-phase-58).
High/Ultra verwenden die echten Bevy-SSAO-/Bloom-Pfade; Ultra zusaetzlich einen
begrenzten Bevy-FogVolume mit 32 Raymarch-Schritten. Nur dessen Sonnenstrahlen
sind volumetrisch: keine volumetrischen Punktlichtstrahlen oder Voxel-GI.
Der MSAA-Pfad behaelt Voxel-AO, verzichtet aber auf inkompatible Bevy-SSAO.
Das hinzugefuegte Bevy-Postprocess-Feature begruendet den einzigen neuen
optionalen Bevy-Teil im Lockfile; alle Versionsvorgaben bleiben erhalten.

Die Bienen-Oekologie wird nachgeladen, nicht in die Terrain-Dichte/Vegetation
des Generators eingebaut. Nester verwenden bestehende PlacedProps-Deltas,
Entfernung/Picking und Renderbatches. Die stabile Home-/Slot-ID ist unabhaengig
von normaler Tierpopulation und Chunk-Reihenfolge; tote Eintraege bleiben
gespeichert. Maximal ein Kandidat je vier Chunkspalten, davon ein Viertel
ueberhaupt ausgewaehlt, Wald und realer Baum-Support weiterhin erforderlich.
Aufwendige Ressourcensuchen bleiben im gemeinsamen Acht-Suchen-Budget.

Die beiden Bienenaufnahmen sind bewusst als originale Synthese/Foley
dokumentiert, keine echten Feldaufnahmen. Materialkontakte werden nicht als
neue Tonaufnahmen ausgegeben. Native Renderpruefung ist keine Audioabhoere;
ohne Audioausgabegeraet sind nur Decoder-/Signal-/Routingtests moeglich.

## Technische Grenzen

- Direkte farbige PBR-Lichter und Kontakt-/Screen-Space-AO, kein persistentes
  RGB-/Skylight-Floodfill und keine indirekte Lichttransport-Simulation.
- Noch keine Lava-Spielmechanik oder leuchtenden Pflanzen; deren Emittereinbindung
  bleibt offen, statt nicht vorhandene Inhalte zu simulieren.
- Die explizite Testhoehle enthaelt einen statischen Solid-Water-See. Der
  Generator fuer natuerliche unterirdische Mehrschichtseen bleibt im Backlog.
- Bienen sammeln visuell Nektar, aber speichern noch keinen Honigertrag.
  Navigation ist begrenztes lokales Steering, keine globale Pfadplanung.
- Ein nativer GPU-Typ, kurze Szenen und Ruhebenchmarks beweisen weder
  Referenzqualitaet noch Langzeitparitaet auf allen GPUs.

## Validierung Und Messungen

### Reproduzierbares Waldpaar

AMD Radeon RX 6950 XT / RADV NAVI21, Vulkan, Mesa 26.2.4, opt-level-1-Devbuild.
Identische Konfiguration aus `../soundscape-landscape-2026-10-18/configs`, Seed
42042, Landschaftsrevision 0, Erzrevision 1, 10:00 pausiert, Sichtweite acht,
kein VSync. Fenster logisch 1280x720, nativ 1920x1080 durch Wayland-Skalierung.
Identische Kamera, 60 s Messfenster, jeweils kein paralleler Compiler/Testclient.
Bestehende Desktopprozesse und der vorhandene Benutzerserver blieben unangetastet.

Der vor Aenderungen gesicherte Phase-57-Client benoetigt die isolierte alte
Assetstruktur: `node tools/benchmarks/prepare58-baseline.mjs /tmp/terra58-original-assets`.
Das Werkzeug darf nur ein neues Zielverzeichnis anlegen und veraendert keine
produktiven Assets. Der Benchmark ist offline, also kein Protokoll-Mischbetrieb.

```bash
env -u DISPLAY ALSA_CONFIG_PATH=/dev/null \
  TERRA_ASSET_DIR=/tmp/terra58-original-assets \
  TERRA_CONFIG_DIR=tools/benchmarks/results/soundscape-landscape-2026-10-18/configs \
  TERRA_BENCH_SECONDS=60 TERRA_RENDER_DIAGNOSTICS=1 TERRA_MEASURE_GPU_MEMORY=1 \
  python3 tools/benchmarks/measure.py target/debug/terra58-before --smoke-test-forest
```

Nachher identisch, ohne `TERRA_ASSET_DIR`, mit `target/debug/terra-client`.
Das abschliessende Paar wurde nach Sonnenkompensation, Journal-Lichtcache und
beidseitigen Schattenproxies erneut gemessen, nicht aus einem Zwischenbuild
uebernommen. Es ist dennoch kein Langzeit-/Mehr-GPU-Test.

| Messgroesse | Vorher | Nachher |
| --- | ---: | ---: |
| FPS, CPU-Wall-Frames | 208.59 | 213.57 |
| p95 Frame ms | 6.022 | 5.834 |
| p99 Frame ms | 6.882 | 6.627 |
| Szenenbereitschaft s | 1.816 | 1.859 |
| Terrain-Komposition, aufsummierte Workers s | 0.735 | 1.263 |
| Teuerste Terrain-Komposition ms | 9.052 | 13.629 |
| Terrain-Kompositionsjobs | 894 | 894 |
| Prop-Komposition, aufsummierte Workers s | 0.180 | 0.186 |
| Geladene / gerenderte Sections | 2041 / 875 | 2041 / 875 |
| Mesh-Assets inkl. Vorlagen | 3512 | 3521 |
| Sichtbare Mesh-Entities | 1745 | 1745 |
| Repraesentierte aktive Props / Batches | 25808 / 328 | 25808 / 328 |
| Peak RSS, KiB, Linux rusage | 752132 | 754080 |
| Peak DRM VRAM, KiB | 2016320 | 2137148 |
| Peak DRM GTT, KiB | 69780 | 69780 |
| GPU Main-Opaque, mittlere ms | 1.02197 | 1.02469 |
| GPU Early-Prepass, mittlere ms | 0.34099 | 0.32259 |

Auswertung: Keine FPS-Regression im kurzen Paar; die kleine FPS-/p99-Differenz
belegt keine allgemeine Beschleunigung. Das Paket mit Kontaktabschattung
erhoeht die einmalige asynchrone Terrain-Komposition um rund 0.53 Worker-Sekunden,
den RSS-Peak um rund 1.9 MiB und den VRAM-Peak um rund 118 MiB. Dies wird nicht
als kostenlose Verbesserung oder allgemeine Speicherparitaet ausgegeben.
Die neun neuen Mesh-Assets gehoeren zu den wiederverwendeten Fackel-/Bienen-
Vorlagen, nicht einer Entity je Detailvoxel. Der Ruhetest misst weder
Bewegungsstreaming noch eine grosse laufende Bienenpopulation.
Reine Chunk-Generierung wurde nicht separat als neues Vorher/Nachher-Paar
isoliert. GPU-Passzeiten sind keine vollstaendige GPU-Framezeit; PBR-Draw-API-
Zaehler waren in diesem Paar nicht aktiviert und werden nicht erfunden.

Rohdaten: [forest-before.log](forest-before.log), [forest-after.log](forest-after.log).

### Funktionspruefungen

Workspace: `cargo fmt --all --check`, `cargo check --workspace --locked`,
`cargo test --workspace --locked`,
`cargo clippy --workspace --all-targets --locked -- -D warnings`,
`cargo build --workspace --locked`. Tests mit `RUST_TEST_THREADS=1`, um
Portkonflikte zwischen bestehenden Netzwerk-Integrationstests zu vermeiden.
Ein expliziter dreiminuetiger Netzwerk-Soak bleibt als solcher ignoriert.
Abschliessend 922 Tests bestanden, einer bewusst ignoriert (Baseline 907).
[Tests](tests.log), [Check](check.log), [Clippy](clippy.log), [Build](build.log),
[Formatierung](fmt.log), [Headless-Start](headless.log). Alle fuenf Befehle
und der Headless-Smoke wurden erfolgreich beendet, nicht nur gestartet.

Regressionen umfassen Material-/AO-Randwerte bei negativen Koordinaten,
diagonale Edit-Halos, Environment-/Aerial-Rueckkehr zur Oberflaeche, echte
Qualitaetskomponenten und deren Cleanup, Emitter-Sicht, Fackelrezept,
stabile Bienen-/Nest-IDs und Abbau-/Reload-/REG7-Persistenz.
Der Dedicated-Smoke startet ohne DISPLAY/WAYLAND und laedt 27 Sections;
der Paket-Abhaengigkeitsbaum enthaelt keine bevy_render/pbr/audio/winit-Pakete.

Ein Wiederholungslauf traf in einem bestehenden UDP-Integrationstest auf eine
bereits belegte TCP-Statusportnummer. Das war kein Rust-/Shaderfehler; der
unveraenderte Netzwerktest wurde ohne Port-/Servermanipulation erneut ausgefuehrt.
Der fehlgeschlagene Lauf bleibt in
[tests-transient-port-conflict.log](tests-transient-port-conflict.log) nachvollziehbar.

### Native Ansichten

`--smoke-test-caves` prueft vier Qualitaeten und den geschlossenen Seitentunnel.
Je Stufe vier Sekunden Warmup und 180 Frames: eine kurze Funktion-/Kostenprobe,
keine belastbare Langzeit-p99-Verteilung. Kein paralleler Compiler/Testclient
waehrend dieses nativen Laufs. Die Szene kontrolliert, dass Ultra wirklich
ein FogVolume aktiviert; Logs melden auch den echten SSAO-Komponentenstatus.

| Szene | FPS | p95 ms | p99 ms | Punktlichter | SSAO / FogVolume |
| --- | ---: | ---: | ---: | ---: | ---: |
| Kammer Low | 85.44 | 12.596 | 12.965 | 4 | 0 / 0 |
| Kammer Medium | 80.50 | 13.635 | 14.407 | 5 | 0 / 0 |
| Kammer High | 79.26 | 13.725 | 14.173 | 5 | 1 / 0 |
| Kammer Ultra | 78.81 | 14.236 | 16.301 | 5 | 1 / 1 |
| Geschlossener Tunnel Medium | 145.12 | 8.005 | 9.303 | 4 | 0 / 0 |

[Low](captures/caves/cave-0-Low.png),
[Medium](captures/caves/cave-1-Medium.png),
[High](captures/caves/cave-2-High.png),
[Ultra](captures/caves/cave-3-Ultra.png),
[dunkler Tunnel](captures/caves/cave-4-Medium.png), [Log](caves.log).
Native Screenshots wurden tatsaechlich geoeffnet und geprueft: kein pauschaler
blauer Schleier, lesbare direkt besonnte Steinflaechen, warme Fackelbereiche,
farbige Kristalle und dunkle unbeleuchtete Bereiche. Die 32-Schritt-Volumetrik
ist bewusst sehr dezent, keine permanenten dicken Lichtkegel. Vollstaendige
Hytale-Referenzqualitaet oder eine mehrstuendige Flackerabnahme wird nicht behauptet.

Der bestehende Geologie-Smoke hat ausserdem sechs natuerliche Ansichten mit
aktivem Hoehlenzustand ohne Inspektionslicht durchlaufen:
[Cheese vorher](captures/natural-cheese-before.png),
[Cheese nachher](captures/natural-cheese-after.png), [Log](geology-after.log).
Ohne Fackeln ist die tiefe Hoehle absichtlich dunkel; das Vergleichsbild ist
kein von einer kuenstlichen Inspektionslampe aufgehelltes Art-Approval.
Diese Geologieprobe erfolgte vor dem abschliessenden beidseitigen Schattenproxy-
Fix; dessen Abnahme steckt in den separaten Kammer-/Tunnelansichten oben.

Der Tier-Smoke mit `TERRA_BEE_INSPECT=1` hat zwei Animationsansichten aufgenommen,
darunter die [Biene](captures/bee.png), [Log](bee.log). Nektarbesuch, Kollisions-
und Nest-/Persistenzregeln werden zusaetzlich ueber echte Datenmodelltests
geprueft; die Modellansicht allein ist kein kompletter KI-Langzeittest.
