# Baumfaellen, Struktursuche Und Dorf-Performance

## Funktionen

Shift + gehaltene Abbau-Taste mit Axt funktioniert an jedem unveraenderten,
natuerlich zugeordneten Holzblock eines Baums, auch an Aesten. Die vorhandene
vorbereitete Baumstruktur bestimmt weiterhin exakte Zugehoerigkeit; kein
Floodfill durch Spielerbauten oder benachbarte Baeume.

| Axt | Sekunden je gezaehltem Holz | Timer-Ausnahme |
| --- | ---: | --- |
| Holz | 0,1 | keine |
| Stein | 0,0925 | keine |
| Eisen | 0,08 | keine |
| Gold | 0,09 | jeder dritte Holzblock zaehlt nicht |
| Diamond | 0,05 | keine |
| Ruby | 0,05 | jeder dritte Holzblock zaehlt nicht |

Alle Holzstuecke werden trotzdem entfernt und in Survival regulaer gedroppt.
Creative verwendet dieselbe zeitbasierte Shift-Aktion mit Axt, aber ohne
zusaetzliche Holz-Drops; ohne Shift bleibt der sofortige Einzelabbau bestehen.
Loslassen, Werkzeugwechsel und UI-Unterbrechung brechen die Aktion ab.

Graphics enthaelt Current (Standard), Hell Fall und Magic. Hell Fall bewegt
einzelne geschlossene Blockkoerper GPU-seitig in gemeinsamen Materialbatches
zum vorher aufgenommenen Boden. Magic schrumpft den kompletten Baum weich
innerhalb von fuenf Sekunden. Keine Physics-Entity je Block, keine neuen
Netzwerkdaten und kein Mesh-Upload je Animationsframe.

`/locate struct teramic::village`, `teramic::ruin` und `teramic::mine`
verwenden echte vorhandene Strukturplaene und die begrenzte asynchrone Suche.
Hilfe und Vorschlaege gelten auch in der Serverkonsole. Creative erhaelt
zusaetzlich freie Wasser-/Luft-Landeziele, wenn kein trockener Platz verfuegbar
ist. Server-Tokens, Berechtigungen und erneute Kollisionspruefung bleiben
erhalten; Survival darf diese Creative-Ausnahme nicht verwenden.

Spinnennetze werden mit ihren wirklichen Geometriegrenzen gegen die begrenzte
Welt geprueft, nicht nur anhand ihres Ankers. Welt-Aussenseite und fehlende
Nachbarzellen sind kein freier Bauplatz. Bereits gespeicherte ungueltige Netze
ausserhalb der Welt werden ausgeblendet, ohne Spielstaende umzuschreiben.

## Strukturen Und Materialien

Neue Welten verwenden Strukturrevision 3: kleine Teppiche und zwei gestuetzte
Blumentoepfe je Dorfgebaeude, beim Medic auch ein Pilztopf. Maschinenfronten,
gedrehte Werkbank-Peers sowie Truhen in Doerfern, Minen und Ruinen werden an
Raum beziehungsweise Zugang ausgerichtet. Ruinen verwenden zusaetzlich
Stein-Zaeune, -Slabs und -Treppen.

Gespeicherte Strukturrevisionen 0/1/2 bleiben erhalten, ebenso Objektzustaende
und Spieler-Tombstones. Neue Einrichtung wird nicht still in alte Doerfer
hineingeneriert. Smooth Stone bleibt Material-ID 12 und verwendet nun gemalte
Mineralflaechen in der bestehenden gemippten Textur-Pipeline statt weitgehend
einfarbiger Pixel. Keine neue Dependency oder eigene GLB fuer Standardbloecke.

Die Rotationsregression prueft auch Kirche und Wachturm: Truhen an der
Vorderwand zeigen in die andere Richtung als Truhen an der Rueckwand.
Topfinhalte sind keine Himmelsrichtung und werden nicht mitgedreht; jeder
Dorfstil behaelt seine exakte Blumen-/Pilzart in allen vier Drehungen.
Die Tests deckten dabei eine ungueltige Pink-Blumenauswahl auf. Die zweite
Blume verwendet jetzt die vorhandene, gueltige violette Pflanzenart.

## Messstand

Kurze Vorhermessung mit unveraendertem Client aus Phase 124, RX 6950 XT,
1280x800, Sichtweite 12, Schatten/Wasser/Wetter High, volle UI:

| Szene | FPS | p95 | p99 |
| --- | ---: | ---: | ---: |
| Dorf/Feld, Regen, integriert, 15 s | 37,91 | 35,911 ms | 40,859 ms |
| Wald-Hin-/Rueckflug, Regen, offline, 24 s | 66,06 | 22,862 ms | 30,131 ms |

Im Dorf: fuenf schattenwerfende Punktlichter, 329 Shadow-Draw-API-Aufrufe,
Main-Schedule-Wallzeit 10,37 ms und Render-Schedule-Wallzeit 25,57 ms.
Die Intervalle laufen parallel und duerfen nicht addiert werden. Der Flug
verwendet einen anderen Modus und keine integrierte UI; kein Direktvergleich
zwischen beiden Zeilen und keine Hochrechnung auf beliebige Aufloesungen.

Ein GDB-Threadsnapshot wurde fuer Diagnose aufgenommen. Die anschliessende
lange Debugger-Pause laesst Netzwerk-/Preview-Fristen ablaufen; dieser Lauf
ist **kein** Framerate- oder Stabilitaetsnachweis. Nur die ununterbrochenen
Vorher-/Nachherlaeufe werden fuer FPS-Werte verwendet.

Der isolierte `NoIndirectDrawing`-Versuch war nicht korrekt: Die Punktlicht-
Schattenphase enthielt noch Indirect-Bins, aber keinen dazu passenden Draw-Pfad.
Dadurch fehlten Schatten-Draws. Die Dateien
`invalid-marker-only-shadow-prototype.*` sind **kein** Performance-Ergebnis und
der experimentelle Code ist entfernt. Schattenqualitaet wird nicht fuer FPS
geopfert.

Die erste gezielte Korrektur betrifft die Schedule-Ausfuehrung:
Bevys `camera_driver` fuehrt `Core3d` seriell fuer jede RootNonCameraView aus,
einschliesslich der sechs Cubemap-Seiten jedes Punktlichts. Kleine, ueberwiegend
geordnete Systeme haben dort zuvor bei jedem Lauf den parallelen ECS-Executor
verwendet. Nur `Core3d` erhaelt den vorhandenen `SingleThreadedExecutor`.
Weltgenerierung, Hauptschedule, Render-Vorbereitung und interne parallele
Berechnungen behalten ihre bisherigen Worker. Keine Lichtquelle, kein
Schatten-Pass und kein Block werden entfernt oder seltener aktualisiert.

Die zweite Korrektur buendelt vollstaendige Punktlicht-Cubemaps. In Bevy 0.19
loescht `clear_indirect_parameters_metadata` globale Phasenpuffer; auch
`early_prepass_build_indirect_parameters` verarbeitet alle Phasenbereiche.
Der urspruengliche Core3d-Pfad fuehrte diese globalen Schritte fuer jede der
sechs Seiten jedes Punktlichts erneut aus. Jetzt erfolgen sie einmal pro
Gruppe. View-spezifisches Unpacking und Frustum-Preprocessing bleiben je Seite
erhalten, danach wird jede Seite mit `shared_shadow_pass::<false>` gerendert.
Die Shader, Meshes, Lichtreichweiten und Aktualisierung jedes Frames bleiben
unveraendert.

Nur vollstaendige Sechsergruppen mit GPU-Culling werden aufgenommen, maximal
96 Seiten. Direct Drawing, SkipGpuPreprocess, Occlusion Culling, unvollstaendige
Gruppen und nicht unterstuetzte GPUs behalten den normalen Core3d-Pfad.
Bei einem Systemfehler werden alle betroffenen Seiten im selben Frame normal
neu gerendert und die Gruppierung fuer den Lauf abgeschaltet. Keine zweite
Schattenengine und kein zeitversetzter Schatten-Cache.

Gemessene Zwischen- und Nachherwerte auf derselben RX 6950 XT:

| Szene | Pfad | FPS | p95 | p99 |
| --- | --- | ---: | ---: | ---: |
| Dorf/Regen, 1280x800, Sichtweite 12, 15 s | alter Client | 37,91 | 35,911 ms | 40,859 ms |
| gleiche Szene | nur Core3d-Executor | 45,88 | 28,493 ms | 33,315 ms |
| gleiche Szene | Executor + Cubemap-Gruppe | 56,88 | 23,080 ms | 27,535 ms |
| Waldflug/Regen, 1280x800, Sichtweite 12, 24 s | alter Client | 66,06 | 22,862 ms | 30,131 ms |
| gleicher Flug, keine Punktlichtschatten | Core3d-Executor | 70,06 | 22,437 ms | 28,014 ms |
| Dorf/Regen, 3840x2160, Sichtweite 20, Ultra-Wetter, 8 s | normaler Core3d-Referenzpfad | 33,82 | 42,843 ms | 46,566 ms |
| Dorf/Regen, 3840x2160, Sichtweite 20, Ultra-Wetter, 8 s | nur Core3d-Executor | 39,53 | 36,746 ms | 42,511 ms |
| gleiche 4K-Szene | Executor + Cubemap-Gruppe | 47,48 | 31,591 ms | 43,722 ms |

In der 1280x800-Nachhermessung bleiben alle fuenf schattenwerfenden Punktlichter
und 326 Shadow-Draw-API-Aufrufe erhalten (Executor-Vergleich ebenfalls 326).
Die Render-Ausfuehrungs-Wallzeit sinkt von 17,35 ms im instrumentierten normalen
Pfad ueber 14,36 ms mit Executor auf 10,21 ms mit Gruppierung. Im 4K-Gruppenlauf
sind 16.137 Sections geladen und 7.486 gerendert, identisch zum Executor-Lauf.
Gegen den vollstaendigen 4K-Referenzpfad sinkt die Render-Ausfuehrung von
19,85 auf 11,39 ms, die Framerate steigt um etwa 40 Prozent. Gegen den
Zwischenschritt nur mit Executor verbessert sich die p99 des kurzen
4K-Fensters nicht. Ein verbleibender Spike wird deshalb nicht als behoben
dargestellt. Der 4K-Referenzlauf verwendet denselben neuen Client mit beiden
Korrekturen ausschliesslich fuer den Vergleich abgeschaltet.

Alle Messfenster sind kurz, ohne gleichzeitig laufenden Cargo-Build. Der
Spielstart/das Laden zaehlt nicht zum Fenster. Die Szene wird ueber echte
Serverbefehle teleportiert; die physikalisch gesetzte Kamerahoehe weicht bei
einigen 1280x800-Laeufen um drei Zentimeter ab. Tageszeit und Wetness laufen
waehrend des Ladens regulaer weiter. Es handelt sich nicht um bitidentische
Frame-Replays und nicht um einen Langzeit-Stabilitaetsnachweis.

Die vorhandene Nutzergrafik ist 3840x2160, Sichtweite 20 und Ultra-Wetter.
Der erste Lauf damit ueberschritt seine 90-s-Ladefrist und liefert deshalb
ausdruecklich **keinen** FPS-Wert. Fuer weitere identische Kurzvergleiche darf
`TERRA_BENCH_LOAD_SECONDS=240` nur die begrenzte Ladefrist des Testmodus
verlaengern; die FPS-Abfrage bleibt acht Sekunden lang. Kein Dauertest.

Referenzpfade sind nur im Smoke-Testmodus erreichbar:
`TERRA_CORE3D_EXECUTOR=parallel` und `TERRA_GROUP_POINT_SHADOWS=0`.
Normale Spielstarts verwenden beide Korrekturen automatisch.

### GPU- Und Native Abschlusspruefung

Der finale Standardpfad wurde zusaetzlich mit dem vorhandenen GPU-Indirect-
Readback gegen abgeschaltete Cubemap-Gruppierung verglichen. Beide Laeufe
verwenden dieselbe freie Offline-Kamera im neuen Dorfhaus, 1280x800,
Sichtweite 12 und Regen; sieben schattenwerfende Punktlichter, 6043 geladene
Sections, 2506 gerenderte Sections und 10899 Mesh-Assets.

| Messung | Normaler Pfad | Gruppierung |
| --- | ---: | ---: |
| Schatten-Draw-API-Aufrufe | 384 | 385 |
| Tatsachlich nichtleere GPU-Schatten-Draws | 7623 | 7620 |
| Nicht gelesene Submissions | 0 | 0 |

Die laufende Szene ist kein bitidentischer Frame-Replay; kleine Zaehldifferenzen
bleiben bei gleichen Section-/Meshzahlen sichtbar. Alle sieben Lichtquellen
und alle sechs Seiten je Licht bleiben erhalten. `interior-gpu-*.png` wurden
visuell kontrolliert; Truhe, Teppich und Topf sind vorhanden und die Schatten
stimmen ueberein. Die Readback-Laeufe sind wegen ihrer Instrumentierung nicht
Teil der obigen regulaeren FPS-Vergleichstabelle. Der bestehende Collector
wird auch im Gruppenpfad direkt nach jeder Schattenseite aufgerufen, bevor
spaetere Views die Indirect-Daten ueberschreiben.

Ein zuvor versuchter integrierter Teleport an die tiefe Innenraumposition
wurde nicht ausgefuehrt und lief in die Testfrist (`interior-teleport-rejected.log`).
Dieser Lauf liefert keine FPS- oder Bilddaten. Die freie Offline-Kamera
veraendert keine Serverrechte und wird nicht als bestandener Teleporttest
ausgegeben; reale Teleportrechte werden in den UDP-/Servertests geprueft.

Die finale Materialgalerie besteht mit 131 Bloecken und 131 GPU-Vorschauen.
Alle 14 Stufen einschliesslich der terminalen Pruefstufe wurden erreicht;
Aufnahmen fuer Terrain, Konstruktionen, Dorfbaubloecke sowie farbige Textilien
und vier gefuellte Toepfe wurden erzeugt. Die neue Smooth-Stone-Oberflaeche ist
auf der durchgehenden Galerieplattform erkennbar. Die Textilien-/Topfaufnahme
wurde tatsaechlich betrachtet, nicht nur der Exitcode.

Current, Hell Fall und Magic wurden mit dem finalen Client erneut nativ
ausgefuehrt (`tree-*-final.log`, jeweils vier Bilder in `tree-*-final/`).
Alle drei Laeufe bestehen ohne Shaderfehler; Masken, Animationskoerper und
Mesh-Besitzer sind danach freigegeben. Die Hell-Fall-Zwischenaufnahme wurde
visuell geprueft. Ein frueher Magic-Lauf meldete erst nach vier erhaltenen
Aufnahmen einen Screenshot-Kanal beim Beenden; der finale Wiederholungslauf
ist fehlerfrei.

Zusaetzlicher Seed-7-Hin-/Rueckflug mit Wasser und Regen, 24 s, 1280x800,
Sichtweite 12: **101,71 FPS**, p95 **14,948 ms**, p99 **17,257 ms**;
625 Wassertiles, sichtbarer Regen (2425 Quads), 6459 abgeschlossene LOD-Wechsel,
am Ende keine offenen Mesh-Jobs. Andere Geometrie und weniger Props als Seed
42042: kein Vorhervergleich und keine aus beiden Seeds gemittelte Gewinnzahl.
Die eigene temporaere Vorher-Binaerkopie wurde nach den Messungen entfernt.

## Regressionstests

`cargo test --locked --workspace --no-fail-fast` besteht: 2447 Tests,
0 Fehler, 7 absichtlich ignorierte Audio-/Benchmark-/Soak-Tests. Keine
neuen langen Spieltests wurden gestartet. Die Kompilierzeit der abhaengigen
Testprogramme ist keine In-Game-Messzeit.

Geprueft werden unter anderem:

- Vollstaendiger identischer Faellplan von jedem eigenen Holzblock; keine
  doppelte Queue fuer verschiedene Aeste desselben Baums.
- Alle sechs Axtzeiten; Gold/Ruby behalten die volle Zahl entfernter und
  gedroppter Hoelzer. Creative erzeugt keine Gratis-Drops.
- Abbruch durch Loslassen/Shift-/Werkzeugwechsel und echte UDP-Replikation.
- Begrenzte Materialbatches, eigene Falltrajektorien, Hangboden und fehlende
  Sections sowie Magic-Dauer und Ressourcenfreigabe.
- Struktursuche gegen echte Plaene, negative Koordinaten, gemeinsame
  Console-/Chat-Vorschlaege und modusgebundene Creative-Teleportrechte.
- Netzgrenzen, fehlende Stuetznachbarn, lokale Publish-/Unload-Invalidierung.
- Neue Dorfobjekte in allen Stilen/Drehungen, unveraenderte alte Revisionen,
  Tombstones, Minenfronten, Ruinenformen und Smooth-Stone-Texturvariation.
- Vollstaendige Cubemaps, direkte/nicht unterstuetzte Views, Lichtentfernung,
  vollstaendiger Fehler-Replay sowie GPU-Diagnose-Hook ohne neue Puffer.

Die ersten Suite-Laeufe sind als `tests-initial.log`,
`tests-orientation-expectation.log` und `tests-pot-rotation.log` erhalten.
Sie sind keine Erfolgsnachweise: Die Tests haben die oben korrigierte
Blumenauswahl und Inhaltsrotation gefunden; eine zu pauschale Truhen-Test-
Erwartung wurde entsprechend der tatsaechlichen Wandposition praezisiert.
`tests-final.log` ist der erfolgreiche vollstaendige Lauf.

Auch erfolgreich: `cargo fmt --all --check`, `cargo check --locked --workspace`,
`cargo clippy --locked --workspace --all-targets -- -D warnings`,
`cargo build --locked --workspace` und `git diff --check`.
Der finale Dedicated Server besteht ohne DISPLAY/WAYLAND_DISPLAY mit
27 Sections (`server-final.log`); kein Renderer oder Fenster wird gestartet.

Ein separater Versuch ohne das Bevy-Feature (`tests-world-recheck.log`)
kompiliert bestehende Bevy-abhaengige Testmodule nicht. Die echte Spiel-/
Serverkonfiguration wird durch die vollstaendige Workspace-Featuremenge
geprueft; dieser Einzelversuch wird nicht als bestandener Test angegeben.

## Grenzen

Die Kurzfenster belegen konkrete Renderkosten und eine Verbesserung, nicht
die Behebung aller Streaming-Spitzen oder beliebiger Welt-/GPU-Last. A05
bleibt offen. Insbesondere der Waldflug gewinnt deutlich weniger als das
Dorf mit schattenwerfenden Lichtquellen. Keine neue 10-/15-/30-Minuten-Abnahme.
Bei kuenftigen Bevy-Rendergraph-Aenderungen muss die Cubemap-Gruppierung erneut
gegen die stock Shadow-View-Passes und Diagnose-Hooks geprueft werden.

F40-F43 sind neue Planungspunkte, keine implementierten Weltgenerierungs- oder
Kingdom-Funktionen. Alte Doerfer werden nicht still mit Revision 3 umgebaut.
