# Phase 92: Bodenbewegung Und Chunk-Streaming

Stand: Zwei native Vorherlaeufe, zwei Zwischenmessungen v1 und eine weitere
Zwischenmessung v2 mit inkrementellen Streamingmeldungen abgeschlossen.
Eine anschliessende CPU-Stichprobe zeigt weitere Asset-/Renderarbeit.
Iteration 3 mit gemeinsamem Wetterpuffer ist fuer Regen/20 und Clear/12
nativ ueber jeweils 180 Sekunden gemessen. Ihr 900-s-Clear/12-Lauf ist
ebenfalls abgeschlossen, zeigt aber weiter steigende Bild-Assets und
Speicherwachstum mit spaetem Swap; eine vollstaendige Ressourcenabnahme
steht damit weiter aus.
Die separate Image-Diagnose weist anschliessend wachsende Fontatlanten bei
wechselnden Fontdaten-IDs nach. Iteration 4 mit begrenzter Fontdaten-
Identitaetsstabilisierung ist fuer Clear/12 und Regen/20 ueber jeweils
180 Sekunden und fuer Clear/12 ueber 900 Sekunden abgeschlossen. Im
v4-Langlauf bleiben Fontatlanten, Fontdaten-IDs, Atlastexturen und Pins ab
dem ersten Fenster konstant; Images liegen bei 192..194. RSS steigt dennoch
weiter. Das ist ein begrenzter 15-Minuten-Nachweis fuer den konkreten
Atlaspfad, keine globale Speicherparitaet oder Behebung aller Frame-Spitzen.
Die aktuelle v4-Suite besteht mit 1506 Tests und zwei ignorierten Tests;
Formatpruefung, Check, Build und Clippy bestehen. Weitere Seeds, ein
4K-Langlauf mit Sichtweite 20 und 30 Minuten bleiben offen beziehungsweise
nicht gemessen. v1/v2 und die abgeloesten v3-Laeufe bleiben historische
Zwischenstaende.

## Messbedingungen

Vorherbinary: `target/phase92/terra-client-before`, optimiertes Dev-Profil.
RX 6950 XT / RADV / Vulkan, Mesa 26.2.4-arch3.1. Die Laeufe wurden einzeln
auf dem nativen Desktop ohne parallelen Compiler ausgefuehrt.

Beide Laeufe verwenden eine isolierte Kopie der gespeicherten Welt `Test`,
inklusive integriertem Server, normalem Controller, Netzwerkbestaetigungen,
TiltUI-HUD, Minimap und regulaerem Autosave. Creative verhindert den Tod,
die Strecke verwendet dennoch den normalen Gehmodus. Die gespeicherte
Spieleridentitaet ist `3daf7669-97e1-469c-9017-354f4ca07906`.

Die massgeblichen kopierten Manifeste stehen unter
`before12-mainland/library/Test/world.toml` und
`before20-mainland/library/Test/world.toml`: Welt-ID `6690318646380802212`,
Seed **10572586493552409009**, Generatorversion 5, Hoehlenrevision 4,
Landschafts-/Ozeanrevision 1 und natuerliches Tierlimit 256. Der Seed `42042`
aus den isolierten `configs*/world.toml` wird beim Laden dieser gespeicherten
Welt durch den Manifestvertrag ersetzt und ist nicht der gemessene Seed.

| Einstellung | `before-walk12-mainland` | `before-walk20-rain` |
| --- | --- | --- |
| Grafikdatei | `configs12/graphics.toml` | `configs20/graphics.toml` |
| Physische Aufloesung | 3840 x 2160 | 3840 x 2160 |
| Sichtweite / vertikal | 12 / 3 | 20 / 3 |
| Vorbereitete Spalten je 30-s-Probe | 625 | 1681 |
| Wetter am Messbeginn | autoritativ Clear | autoritativ Rain |
| Terrain-LOD | 153,6 / 256 m | 153,6 / 256 m |
| Messdauer nach Szenenbereitschaft | 180 s | 180 s |
| Gesamte Prozesslaufzeit | 201,107 s | 233,996 s |

Alle sechs Zeitfenster und beide Enddiagnosen bestaetigen echte 3840 x 2160.
Fenstermodus, VSync aus, kein FPS-Limit, FOV 90, Fog-Faktor 1,25,
Rise-Animation, volle Vegetationsdichte 1,0, High-Schatten/-Wasser,
Ultra-Wetter, Wasserreflexionen und volumetrischer Wassernebel bleiben aktiv.
Vier Generierungsjobs, 16 Terrain-Meshjobs und zwoelf Uploads pro Frame sind
konfiguriert. Die integrierte Autoritaet verwendet Simulationsweite 1 und
60 Hz. Das ist kein Vergleich mit Phase 91 bei Simulationsweite 6.

`graphics.toml` bestimmt die effektive Sichtweite; der dort nicht massgebliche
alte `client.toml`-Wert 16 wird nicht als gemessene Sichtweite ausgegeben.
Weltzeit und Wetter laufen weiter. Rain erreicht im zweiten Lauf Intensitaet
0,78; die beiden Szenarien unterscheiden sich sowohl in Sichtweite als auch
Wetter und sind deshalb kein isolierter Qualitaetsstufenvergleich.

## Reale Gehstrecke

Die Route startet nach Szenenbereitschaft bei
`[-759.5, 39.910007, -743.5]`, laeuft entlang +X bis zu einem 64-m-Ziel und
kehrt ueber normale Vorwaerts-/Sprungeingaben zurueck. Die Umkehrschwelle
liegt 0,8 m vor dem Ziel; gemessene Maximaldistanzen sind 63,284 m bzw.
63,374 m. Sieben vollstaendige Rueckwege sind in jedem Log mit lokaler
Position und Serverposition bestaetigt. Die Rueckwege steigen den Hang mit
echten Spruengen hinauf; `grounded=false` waehrend eines Sprungs ist erwartet.
Es gibt waehrend der Route keine Flugbewegung, Teleports oder Terrain-Edits.

Der erste Versuch `before-walk12.log` begann dagegen bei
`[8.5, 69.91018, 24.5]` und meldete nach 31,2 m eine blockierte Strecke.
Dieser abgebrochene Versuch wird nicht als erfolgreicher Gehbenchmark und
nicht in den Ergebnistabellen gezaehlt.

## Finale Kurzvergleiche

Die folgende Tabelle verwendet die nativen 180-s-Zusammenfassungen aus den
jeweiligen Logs. Screenshot-Nachbarframes sind dort ausgeschlossen. Beide
Nachherzeilen verwenden jetzt v4 mit Fontcache. Das ist die Abnahme der
beiden 180-s-Kurzvergleiche, keine pauschale Langzeit-/Ressourcenfreigabe.
Der abgeschlossene 900-s-Lauf steht mit seinen Grenzen separat weiter unten.
Historische v1/v2/v3-Werte und die laengeren
Clear/12-Laeufe stehen gesondert weiter unten.

| Szenario | Nachherstand | FPS vorher | p95 vorher (ms) | p99 vorher (ms) | FPS nachher | p95 nachher (ms) | p99 nachher (ms) |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: |
| Sichtweite 12, Clear | v4, Fontcache | 73,05 | 23,909 | 28,615 | 87,95 | 17,609 | 20,572 |
| Sichtweite 20, Rain | v4, Fontcache | 43,71 | 35,544 | 42,805 | 63,79 | 24,340 | 30,622 |

Die JSON-Zeitreihen enthalten unabhaengig davon sechs vollstaendige
30-s-Fenster einschliesslich Screenshot-Nachbarframes. Ihre gewichteten
FPS sind Gesamtframes geteilt durch die Summe der tatsaechlichen Intervalle.
Perzentile verschiedener Fenster werden nicht zu einem Gesamtperzentil
gemittelt.

| JSON-Zeitreihe | Gewichtete FPS | FPS je 30-s-Fenster | p95 je Fenster (ms) | p99 je Fenster (ms) | Groesster Frame (ms) |
| --- | ---: | ---: | ---: | ---: | ---: |
| `before-walk12-mainland.json` | 72,96 | 68,00..76,66 | 23,354..25,784 | 27,773..30,724 | 164,754 |
| `before-walk20-rain.json` | 43,71 | 39,75..48,52 | 30,947..38,144 | 37,069..46,250 | 225,318 |

Beide Laeufe enden regulaer mit Exit 0. Diese kurzen Wiederholungen zeigen
teure Frames beim Bodenstreaming, belegen aber weder einen zeitabhaengigen
Langzeitkollaps noch jede gemeldete Folge von 110 -> 50 -> 80 -> 30 FPS.

## Zwischenmessungen v1

`after-walk20-rain` ist trotz seines Dateinamens ein **Zwischenstand v1**,
kein finaler Nachherlauf. Das gilt ebenfalls fuer
`intermediate-walk12-clear`. Die zugehoerigen Befehlszeilen nennen
`target/phase92/terra-client-fixed` bzw.
`target/phase92/terra-client-intermediate`. Beide Laeufe enden mit Exit 0,
umfassen 180 Messsekunden und sieben vollstaendige reale Rueckwege; ihre
Enddiagnosen bestaetigen physische 3840 x 2160.

| Zwischenstand | Native FPS | Nativer p95 (ms) | Nativer p99 (ms) | Vorher-FPS | Vorher-p99 (ms) |
| --- | ---: | ---: | ---: | ---: | ---: |
| `intermediate-walk12-clear`, v1 | 70,63 | 23,941 | 29,299 | 73,05 | 28,615 |
| `after-walk20-rain`, v1 | 46,57 | 33,029 | 39,233 | 43,71 | 42,805 |

Bei Sichtweite 20 steigt das native Mittel um etwa 6,6%, der p99 sinkt von
42,805 auf 39,233 ms. Das ist eine begrenzte Verbesserung in dieser Szene;
die verbleibenden Framezeiten liegen weiterhin deutlich ueber 16,7 ms.
Bei Sichtweite 12 faellt das Mittel dagegen um etwa 3,3%, und der p99 wird
mit 29,299 statt 28,615 ms etwas schlechter. Diese Einzelpaare belegen keine
allgemeine v1-Beschleunigung und rechtfertigen keinen Abschluss der Arbeit.

Die getrennten JSON-Zeitreihen einschliesslich Screenshot-Nachbarframes
liefern folgende Werte; auch hier werden Fensterperzentile nicht gemittelt:

| v1-JSON-Zeitreihe | Gewichtete FPS | FPS je 30-s-Fenster | p99 je Fenster (ms) | Groesster Frame (ms) |
| --- | ---: | ---: | ---: | ---: |
| `intermediate-walk12-clear.json` | 70,51 | 66,98..75,43 | 28,423..30,773 | 193,002 |
| `after-walk20-rain.json` | 46,55 | 43,07..51,08 | 33,179..42,546 | 227,590 |

Die erste Iteration begrenzt Terrain-Vorbereitung und ersetzt globale
Terrain-Auswahl bei Animationsende durch gezielte Wiederholungen. Bei Weite
20 sinken volle Auswahlscans von 2499 auf 448 und `terrain.dispatch` von
5,187 auf 4,208 ms je Aufruf. Gleichzeitig bleiben jedoch 4814 globale
Arrival-Scans statt zuvor 3372; sie kosten durchschnittlich 3,646 ms und
zusammen 17,552 Sekunden in den 180 Messsekunden. Der vorherige
Arrival-Mittelwert war 3,519 ms. Die Auswahl aller geladenen Sections bei
jeder Veroeffentlichung bleibt damit ein konkreter Kostenpfad. Das ist der
Grund fuer die zweite Iteration mit expliziten Positionsmeldungen, nicht
eine aus dem FPS-Mittel allein abgeleitete Ursachenbehauptung.

## Zwischenmessung v2

Auch `final-walk20-rain` ist trotz seines Dateinamens **kein finaler Lauf**.
Er misst Iteration 2 nach der inkrementellen Arrival-Verarbeitung und vor
dem gemeinsamen Wetterpuffer. Die native Zusammenfassung meldet
50,1059 FPS, p95 33,399117 ms und p99 40,630459 ms ueber 180 Sekunden,
physische 3840 x 2160 und Exit 0.

| Zwischenstand | Native FPS | Nativer p95 (ms) | Nativer p99 (ms) |
| --- | ---: | ---: | ---: |
| `final-walk20-rain`, v2 vor Wetterpuffer | 50,11 | 33,399 | 40,630 |

Das Mittel steigt weiter gegenueber v1 mit 46,57 FPS. Der p99 wird jedoch
gegenueber v1 mit 39,233 ms etwas schlechter und liegt weiterhin nahe
40 ms. Eine vollstaendige Behebung der Ruckler ist damit nicht nachgewiesen.
Die v2-JSON-Timeline liefert separat 50,11 gewichtete FPS,
45,33..54,56 FPS je 30-s-Fenster, Fenster-p99 von 37,092..44,490 ms und
einen groessten Frame von 210,640 ms einschliesslich Screenshot-Nachbarframes.

Der entfernte globale Arrival-Pfad ist im v2-Messfenster nicht mehr als
`terrain.refresh.queue.arrivals` vorhanden. Der neue lokale Pfad
`terrain.refresh.queue.incremental` verarbeitet 4817 Aufrufe in insgesamt
287,120 ms, durchschnittlich 0,0596 ms. Die lokale Kuestenaktualisierung
braucht 274,398 ms fuer 5163 Aufrufe. Zum Vergleich: v1 verbrauchte
17,552 Sekunden fuer 4814 globale Arrival-Scans.

`terrain.dispatch` sinkt auf 1,162 ms je Aufruf und zusammen 10,481 Sekunden
statt 4,208 ms bzw. 35,267 Sekunden in v1. Ueberschreitungen von 8 ms sinken
von 252 auf 48. Die 492 vollen Auswahlscans mit zusammen 3,026 Sekunden
bleiben im Log erhalten; Fokus-/Journal- und andere echte globale
Invalidierungen sind weiterhin moeglich. Verschachtelte Scopezeiten werden
auch hier nicht addiert. Der beseitigte Arrival-Aufwand allein erklaert die
verbleibende gesamte Framezeit nicht.

## CPU-Stichprobe nach v2

`diagnostic-walk20-cpu.log` ist ein separater instrumentierter Lauf;
`cpu-final20.samples.tsv`, `.threads.tsv`, `.maps` und `.summary.json`
gehoeren zu dieser Diagnose. Der Samplingname `cpu-final20` bezeichnet
ebenfalls den Stand vor dem Wetterpuffer. Seine FPS werden nicht als weiterer
unbeeinflusster A/B-Nachherwert in die Benchmarktabelle uebernommen.

Der Sampler erfasst 18080 CPU-IP-Stichproben bei 84 erfassten Threads,
5 ms CPU-Zeit als Samplingperiode und null gemeldeten verlorenen Samples.
Die Zusammenfassung ordnet 11750 Samples Compute-Task-Threads, 3641 den
beiden gleichnamig gekuerzten `terra-client-fi`-Threads, 2328 Async-Compute-
Threads und 207 dem integrierten Server zu. Die 84 Threadeintraege bedeuten
nicht, dass jeder Thread CPU-Samples beigetragen hat; die Zusammenfassung
listet 28 Threads mit Treffern.

Unter den aufgeloesten einschliessenden Funktionen erscheinen zusammen
1523 Treffer in `AssetChangeCheck::has_changed`, dazu Hash-Tabellenzugriffe,
Allokation/Freigabe und Mesh-Extraktion/-Aufbereitung. Das zeigt verbleibende
CPU-Arbeit ausserhalb des entfernten globalen Arrival-Scans. Es ist keine
vollstaendige Aufteilung der Framezeit oder alleinige Ursachenzuordnung.

**Nur 1151 von 18080 Samples, etwa 6,4%, haben einen gueltigen Caller.**
Die Stichprobe liefert daher ueberwiegend Instruktionsadressen und
Funktionszuordnungen, keine vollstaendigen belastbaren Aufrufketten. Weder
alle Asset-Checks noch alle langsamen Frames werden pauschal dem Wetter
zugeschrieben. Der Befund motiviert die gezielte Pruefung des bisher pro
Frame geaenderten gemeinsamen Wetter-/Materialzustands.

## Iteration 3: Wetterpuffer (v3)

Der aktuelle Code nutzt einen stabilen gemeinsamen GPU-Wetterpuffer fuer
Vegetation und nasses Terrain. Sein Payload besteht aus drei `Vec4`, also
48 Byte: Wind, Oberflaechenwetter samt Wachstumsuhr und Wetterfarbton. Die
Materialbindung behaelt ihren Handle; aktuelle Daten werden im Renderpfad
in den vorhandenen Buffer geschrieben.

Die v3-Laufnamen sind `weather-buffer-walk20-rain`,
`weather-buffer-walk12-clear` und anschliessend
`weather-buffer-walk12-long`. Die 180-s-Kurzlaeufe Regen/20 und Clear/12 sind
abgeschlossen. Auch der 900-s-Clear/12-Lauf endet regulaer; sein stabileres
FPS-Niveau ersetzt wegen des weiter wachsenden Ressourcenbestands keine
vollstaendige Langzeit-/Ressourcenabnahme.

### Regen/20 Mit Wetterpuffer (v3-Zwischenstand)

`weather-buffer-walk20-rain.log` meldet sieben vollstaendige reale Rueckwege,
180 Messsekunden, 232,993 Sekunden Prozesslaufzeit und Exit 0. Alle sechs
Zeitfenster sowie die native Enddiagnose bestaetigen physische 3840 x 2160.
Es gibt keine ERROR-/Panic-Meldung im Log. Dieselben High-/Ultra-Einstellungen,
Rise-Animation, volle Vegetationsdichte, HUD und integrierte Autoritaet bleiben
konfiguriert; autoritatives Rain bleibt bei Intensitaet 0,78.

| Messmethode | FPS | p95 (ms) | p99 (ms) | Groesster Frame (ms) |
| --- | ---: | ---: | ---: | ---: |
| Native 180-s-Zusammenfassung | 62,82 | 24,756 | 31,623 | nicht als Gesamtmaximum ausgegeben |
| Sechs JSON-Fenster, gewichtet | 62,79 | 24,035..25,810 je Fenster | 30,256..33,179 je Fenster | 184,823 |

Die JSON-FPS je 30-s-Fenster liegen bei 61,59..65,13. Das native Mittel
steigt gegenueber der Baseline von 43,71 auf 62,82 FPS, der native p99 sinkt
von 42,805 auf 31,623 ms. Das entspricht in diesem Einzelpaar etwa +43,7%
FPS und -26,1% p99; gegenueber v2 steigt das Mittel um etwa 25,4%.
Diese Szene belegt eine Verbesserung, aber keine pauschale Behebung der
Folge 110 -> 50 -> 80 -> 30 FPS, aller Frame-Spitzen oder einer Langzeitlast.
Insbesondere verbleiben p99-Werte ueber 30 ms und einzelne groessere Frames.

Die native Enddiagnose zeigt 13052 geladene Sections, 83572 geladene und
83572 dargestellte Props, 4330 Prop-Batches und 710 Wassertiles. Diese
Endzaehler sind kein Nachweis aller Identitaeten oder unveraenderter
Darstellung waehrend jeder Zwischenphase.

| Ressource | Baseline Regen/20 | Wetterpuffer Regen/20 |
| --- | ---: | ---: |
| Peak-RSS, gesamter Prozess (MiB) | 2131,50 | 2192,36 |
| Peak-DRM-VRAM (MiB) | 5080,89 | 5263,18 |
| Peak-DRM-GTT (MiB) | 68,14 | 68,14 |

Die Peaks enthalten Startup. Im v3-Messfenster steigt RSS von 2027,69 auf
2189,45 MiB. Sieben Ressourcenproben melden Swap 0; von insgesamt 341
Major-Fault-Deltas liegen 335 vor Messbeginn, sechs in der Probe bei
8,39 Messsekunden und keine in den spaeteren Ressourcenproben. Mesh-Assets
liegen je Zeitfenster zwischen 31608 und 35418, am Ende bei 32315;
Standardmaterialien zwischen 206 und 274, am Ende bei 209. Die Bild-Assets
steigen von 205 auf 286. Daraus wird weder Speicherparitaet noch ein
bereits nachgewiesenes Speicherleck abgeleitet. Die hoehere Performance
bedeutet hier keinen niedrigeren Speicherpeak.

Der Screenshot `weather-buffer-walk20-rain.png` wurde gegen
`before-walk20-rain.png` visuell kontrolliert: keine offensichtlichen Farb-
oder Geometrieabbrueche festgestellt. Das ist eine lokale Szenenpruefung,
keine umfassende Referenz-, Wetterwechsel- oder Mehr-GPU-Abnahme.

### Clear/12 Mit Wetterpuffer (v3-Zwischenstand)

`weather-buffer-walk12-clear.log` endet ebenfalls regulaer mit Exit 0:
180 Messsekunden, 201,102 Sekunden Prozesslaufzeit und sieben vollstaendige
reale Rueckwege mit Serverpositionen. Die maximale Hinausdistanz betraegt
63,374 m. Alle sechs Zeitfenster und die native Enddiagnose bestaetigen
physische 3840 x 2160. Im Log gibt es keine WARN-, ERROR- oder Panic-Meldung.
Sichtweite 12, vertikal 3, Rise, volle Vegetationsdichte, High-Schatten/-Wasser,
Ultra-Wetter, HUD und integrierter Server entsprechen der Clear/12-Baseline.
Die Enddiagnose meldet autoritatives Clear und sichtbare Regenstaerke 0.

| Messmethode | FPS | p95 (ms) | p99 (ms) | Groesster Frame (ms) |
| --- | ---: | ---: | ---: | ---: |
| Native 180-s-Zusammenfassung | 86,86 | 18,001 | 21,003 | nicht als Gesamtmaximum ausgegeben |
| Sechs JSON-Fenster, gewichtet | 86,72 | 17,987..18,499 je Fenster | 21,250..21,756 je Fenster | 142,786 |

Die JSON-FPS je Fenster liegen bei 85,36..89,07. Gegenueber der nativen
Baseline mit 73,05 FPS und p99 28,615 ms steigen die FPS in diesem
Einzelpaar um etwa 18,9%; der p99 sinkt um etwa 26,6%. Diese v3-Zwischenmessung
ist damit vom verschlechterten v1-Zwischenstand mit 70,63 FPS/p99 29,299 ms
zu unterscheiden. Ein groesserer einzelner Frame bleibt in der Zeitreihe;
weder dieser Kurzlauf noch sein Mittelwert belegen eine Behebung aller
Spitzen oder der gemeldeten gesamten FPS-Folge.

| Ressource | Baseline Clear/12 | Wetterpuffer Clear/12 |
| --- | ---: | ---: |
| Peak-RSS, gesamter Prozess (MiB) | 1515,72 | 1496,93 |
| Peak-DRM-VRAM (MiB) | 4604,91 | 4629,25 |
| Peak-DRM-GTT (MiB) | 68,14 | 68,14 |

RSS steigt innerhalb der v3-Messfenster von 1380,34 auf 1482,31 MiB. Alle
sechs Ressourcenproben melden Swap 0. Die erste Probe bei 10,14 Messsekunden
enthaelt 111 Major-Fault-Deltas seit Prozessbeginn, alle spaeteren Proben
melden null weitere. Der kleinere RSS-Peak und gleichzeitig groessere
VRAM-Peak ergeben keinen allgemeinen Speicherparitaetsnachweis.

Die native Enddiagnose zeigt 4522 Sections, 31254 geladene und dargestellte
Props, 1612 Prop-Batches, 157 Wassertiles und 12894 Mesh-Assets. In den
30-s-Fenstern variieren Mesh-Assets zwischen 12528 und 14560 sowie
Standardmaterialien zwischen 199 und 254; am Ende sind es 199 Materialien.
Bild-Assets steigen von 203 auf 275. Die Client-Fluidqueue bleibt bei null
bei 644 gespeicherten Fluidzellen und null Quellen.

### 15 Minuten Bodenbewegung (v3-Zwischenstand)

`weather-buffer-walk12-long.log` misst denselben normalen Gehmodus mit
Sichtweite 12 ueber 900 Sekunden nach Szenenbereitschaft. Der Prozess endet
nach 921,088 Sekunden mit Exit 0. Alle 30 Zeitfenster, die effektive
Konfiguration und die native Enddiagnose bestaetigen physische 3840 x 2160.
35 vollstaendige reale Rueckwege sind mit Serverpositionen protokolliert;
die maximale Hinausdistanz betraegt 63,302 m. WARN-, ERROR- und Panic-Zaehler
sind jeweils null. High-/Ultra-Einstellungen, Rise, volle Vegetationsdichte,
HUD und integrierte Autoritaet bleiben aktiv.

Clear bezeichnet hier den Startzustand, kein ueber 15 Minuten festgehaltenes
Wetter: die Simulation wechselt im Verlauf von Clear zu Climate, mit
protokollierten Intensitaeten zwischen 0 und 0,630173. Der Langlauf ist
deshalb keine Wiederholung von 15 Minuten unveraendertem Clear und besitzt
keinen entsprechenden 900-s-Vorherlauf.

Die native Zusammenfassung meldet **85,62 FPS, p95 18,249 ms und
p99 21,416 ms** ueber 900 Sekunden. Die 30 JSON-Fenster ergeben gewichtet
85,60 FPS; die Fenster reichen von 79,94 bis 89,21 FPS, p95 von 17,343 bis
19,270 ms und p99 von 20,271 bis 23,244 ms. Der groesste einzelne Frame
betraegt 148,227 ms. Das ist auf dieser wiederholten Route kein beobachteter
FPS-Kollaps bis Minute 15, aber kein Nachweis gegen alle Spitzen oder
langfristige Verschlechterung.

Die Kontrollpunkte stammen aus `weather-buffer-walk12-long.json` und zeigen
jeweils das letzte vollstaendige 30-s-Fenster nahe der Zielminute, keine
kumulativen FPS oder gemittelten Gesamtperzentile. Es wird nicht interpoliert.
ND bedeutet nicht durchgefuehrt.

| Zielminute | Tatsaechliche Messsekunde | FPS | p95 (ms) | p99 (ms) | RSS (MiB) | Bild-Assets |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| 1 | 60,015 | 87,57 | 17,595 | 20,573 | 1444,40 | 219 |
| 5 | 300,071 | 79,94 | 19,244 | 22,387 | 1612,91 | 340 |
| 10 | 600,131 | 85,67 | 18,740 | 21,818 | 1788,50 | 483 |
| 15 | 900,196 | 86,32 | 19,057 | 23,244 | 1660,56 | 627 |
| 30 | ND | ND | ND | ND | ND | ND |

| Ressource | Bereich / Peak | Erster -> letzter 30-s-Messwert |
| --- | ---: | ---: |
| RSS in den 30-s-Fenstern (MiB) | 1423,07..1950,95 | 1423,07 -> 1660,56 |
| Peak-RSS, gesamter Prozess (MiB) | 2000,86 | nicht fenstersynchron |
| Peak-DRM-VRAM, gesamter Prozess (MiB) | 4120,94 | nicht fenstersynchron |
| Peak-DRM-GTT, gesamter Prozess (MiB) | 68,14 | nicht fenstersynchron |
| Mesh-Assets | 12553..14572 | 12557 -> 14549 |
| Standardmaterialien | 199..273 | 216 -> 199 |
| Bild-Assets | 207..627 | 207 -> 627 |
| Entities | 17915..20511 | 17962 -> 20454 |
| Geladene Sections | 3976..4541 | 4541 -> 4509 |
| Gespeicherte Props | 28860..32750 | 30109 -> 30505 |

**Der Ressourcenbestand hat kein nachgewiesenes Plateau.** Bild-Assets
steigen vom ersten bis zum letzten Fenster um 420, auch zwischen Minute 10
und 15 weiter von 483 auf 627. RSS steigt von rund 1,49 GB im ersten
30-s-Fenster bis auf rund 2,05 GB in der Zeitreihe; der Prozesspeak betraegt
rund 2,10 GB. Der kleinere letzte RSS-Wert ist kein Freigabe- oder
Plateaunachweis: die getrennten Ressourcenproben melden spaet Swap, zuletzt
128880 KiB beziehungsweise 125,86 MiB bei 883,82 Messsekunden. Nach der
ersten Ressourcenprobe entstehen noch 519 Major-Fault-Deltas. RSS und
Ressourcenproben sind nicht zeitgleich; ausgelagerte Seiten fehlen im RSS.
Die gemessenen VRAM-Proben reichen von 3683,16 bis 4120,42 MiB, GTT bleibt
bei 68,14 MiB. Weder konstante Section-Grenzen noch begrenzte Meshzaehler
erklaeren damit bereits das Bild-/Speicherwachstum.

Der anschliessende opt-in Diagnosesnapshot fuer Images und FontAtlas liefert
den folgenden separaten Befund. Die oben genannten v3-Messungen bleiben
unveraendert und werden dadurch nicht nachtraeglich als ressourcenstabil
abgenommen. Der nicht durchgefuehrte 30-Minuten-Punkt bleibt offen.

### Image-/Fontatlas-Diagnose

`image-audit-walk12.log` ist eine separate 120-s-Diagnose des v3-Stands mit
zusaetzlichen opt-in Image-/FontAtlas-Zaehlern, keine weitere Produktions-
oder Qualitaetsaenderung an Iteration 3. Alle vier Zeitfenster bestaetigen
3840 x 2160, vier reale Rueckwege sind protokolliert. Der Lauf endet mit
Exit 0 ohne WARN-, ERROR- oder Panic-Meldung.

| Messsekunde | Images | CPU-Bilddaten (MiB) | Fontatlanten | Fontdaten-/Face-IDs | Atlas-CPU-Daten (MiB) |
| --- | ---: | ---: | ---: | ---: | ---: |
| 30,011 | 206 | 61,451 | 25 | 18 | 25 |
| 60,014 | 218 | 73,451 | 37 | 30 | 37 |
| 90,023 | 234 | 87,473 | 51 | 44 | 51 |
| 120,029 | 249 | 102,473 | 66 | 59 | 66 |

Die Rohzaehler fuer CPU-Bilddaten steigen von 64436520 auf 107450664 Byte
(dezimal 64,44 -> 107,45 MB, nicht MiB). Von diesem Zuwachs um
41,021484 MiB entfallen exakt 41 MiB auf 41 zusaetzliche Fontatlanten;
22 KiB stammen aus anderen Images. Damit erklaeren die Atlanten mehr als
99,9% des CPU-Bilddatenwachstums dieser Probe. Die Zahl der unterschiedlichen
Schriftgroessen bleibt bei neun, ihr Bereich bei 18,150002..108,90001 und die
Zahl der Variations-Hashes bei eins. Dagegen wachsen die Fontdaten-/Face-IDs
von 18 auf 59. Die Diagnose spricht damit fuer Identitaetswechsel derselben
Schriftquellen, nicht fuer immer mehr gewollte Schriftgroessen.

Der lokale Abhaengigkeitscode bestaetigt den Mechanismus: Parley 0.9 ruft
beim Layoutaufbau `SourceCache::prune(128, false)` auf. Fontique 0.9 kann
dadurch die Systemfontdaten aus dem lokalen Quellcache entfernen; ein
erneutes Laden erzeugt einen neuen `Blob` mit neuer ID. Bevy Text 0.19.1
verwendet `font.data.id()` in `FontAtlasKey`. Dieselbe Schriftdatei mit neuer
Blob-ID erzeugt so weitere Atlanten, obwohl Groessen und Variationen
unveraendert bleiben. Ein lediglich geteilter Cache haelt nur schwache
Verweise und garantiert ohne lebende Fontdaten keine dauerhafte Identitaet.

Die daraus abgeleitete begrenzte Fontdaten-Stabilisierung und ihre eigene
v4-Messung stehen im folgenden Abschnitt. Sie aendern die historischen
v3-Daten nicht.

Der belegte Atlaspfad erklaert CPU-Bilddaten in dieser kurzen Diagnose,
nicht automatisch das gesamte RSS-/VRAM-Wachstum, alle Bevy-Asset-Checks
oder alle Frame-Spitzen. Textur-Deskriptorwerte sind rechnerische Groessen,
keine gemessene VRAM-Residency. Die v3-Langmessung mit Wachstum und Swap
bleibt deshalb weiterhin ohne vollstaendige Ressourcenabnahme.

### Verbleibende CPU-Kosten

Die folgende Tabelle gehoert zum Regen/20-Kurzlauf von v3:

| v3-Regen/20-Scope | Aufrufe | Mittel je Aufruf (ms) | Intervallsumme (ms) | Aufrufe ueber 8 ms |
| --- | ---: | ---: | ---: | ---: |
| `terrain.dispatch` | 11306 | 0,962 | 10877,136 | 37 |
| `terrain.prepare_jobs` | 7787 | 0,855 | 6654,002 | 0 |
| `terrain.refresh.queue.incremental` | 5028 | 0,0439 | 220,815 | 0 |
| `terrain.refresh.queue.full` | 518 | 6,137 | 3178,769 | 8 |
| `props.dispatch` | 11306 | 0,177 | 2002,074 | 4 |
| `mapping.schedule` | 863 | 4,787 | 4131,019 | 0 |
| `mapping.index` | 858 | 2,070 | 1776,238 | 0 |

Im v3-Regen/20-Messfenster bleibt der globale Arrival-Scope aus. Clear/12
enthaelt drei konservative Arrival-Scans mit zusammen 5,514 ms, daneben
2548 lokale Auswahlaufrufe mit zusammen 55,932 ms. `terrain.dispatch` kostet
dort 0,346 ms je Aufruf, volle Auswahl 2,152 ms bei 543 Aufrufen und
`mapping.schedule` 1,818 ms bei 872 Aufrufen. Volle Auswahlscans,
weitere Kandidaten-/LOD-Zuordnung, Terrain-Snapshots sowie Kartenplanung
und Kartenindex sind konkrete verbleibende Kosten fuer spaetere Arbeit.
Diese werden hier dokumentiert, nicht durch weitere ungemessene Aenderungen
in denselben Vergleich aufgenommen. Verschachtelte Zeiten sind nicht
addierbar, und mehr FPS bedeuten mehr Dispatcheraufrufe pro Messsekunde.

18 Autosave-Snapshots kosten auf dem Serverthread zusammen 5,599 ms,
durchschnittlich 0,311 ms; das kumulative Maximum betraegt 0,695 ms.
Die getrennte Worker-Dateiarbeit `storage.save_edits` summiert sich auf
4907,584 ms mit kumulativem Maximum 1627,603 ms. Diese Workerzeiten sind
weiterhin keine gleich langen Client-Hauptthreadpausen.

## Iteration 4: Stabile Fontdaten (v4)

`crates/terra-ui/src/fonts.rs` aktiviert `make_shared` am vorhandenen
Quellcache und haelt starke Referenzen auf tatsaechlich in Textlayouts
genutzte Systemfontdaten. Der Bestand ist auf hoechstens 32 Fontdaten-
Bloecke und 64 MiB begrenzt; bereits durch Font-Assets gehaltene Daten
werden nicht nochmals gepinnt. Die Verdraengung ist LRU-basiert, zu grosse
Einzelfonts werden nicht aufgenommen. Es gibt keine Dependency-, CSS-,
Schriftgroessen-, Shader-, Atlas- oder Qualitaetsaenderung. Die Grenzen
betreffen den neuen Pinningbestand, nicht jede legitime Atlasbelegung bei
beliebigen neuen Schriften oder Groessen.

### Clear/12 Mit Fontcache

`font-cache-walk12-clear.log` endet nach 180 Messsekunden und 201,866 Sekunden
Prozesslaufzeit mit Exit 0. Sieben vollstaendige reale Rueckwege sind mit
Serverpositionen protokolliert; die maximale Hinausdistanz ist 63,296 m.
Alle sechs Zeitfenster und die Enddiagnose bestaetigen physische
3840 x 2160. WARN-, ERROR- und Panic-Zaehler sind jeweils null. Sichtweite,
High-/Ultra-Einstellungen, Rise, Vegetationsdichte, HUD und Autoritaet
entsprechen den vorherigen Clear/12-Laeufen.

| Messmethode | FPS | p95 (ms) | p99 (ms) | Groesster Frame (ms) |
| --- | ---: | ---: | ---: | ---: |
| Native 180-s-Zusammenfassung | 87,95 | 17,609 | 20,572 | nicht als Gesamtmaximum ausgegeben |
| Sechs JSON-Fenster, gewichtet | 87,80 | 17,461..18,258 je Fenster | 20,620..21,601 je Fenster | 186,995 |

Die JSON-FPS liegen bei 86,73..89,49 je Fenster. Das native Mittel liegt
in diesem Einzelpaar etwa 20,4% ueber der Baseline von 73,05 FPS; der p99
liegt etwa 28,1% unter den vorherigen 28,615 ms. Gegenueber v3 mit 86,86 FPS
ist der Mittelwert nur rund 1,3% hoeher, kein Nachweis einer allgemein
reproduzierbaren zusaetzlichen FPS-Steigerung. Der groessere einzelne
JSON-Frame bleibt sichtbar; nicht alle Spitzen sind damit behoben.

Die eigene Zeitreihe zeigt ab dem ersten Messfenster einen stabilen
Fontatlas-/ID-/Pinningbestand:

| Messsekunde | Images | CPU-Bilddaten (MiB) | Fontatlanten / Face-IDs | Atlas-CPU-Daten (MiB) | Pins / Pin-Daten (Byte) |
| --- | ---: | ---: | ---: | ---: | ---: |
| 30,016 | 192 | 47,451 | 11 / 3 | 11 | 2 / 1368716 |
| 60,019 | 192 | 47,451 | 11 / 3 | 11 | 2 / 1368716 |
| 90,028 | 192 | 47,451 | 11 / 3 | 11 | 2 / 1368716 |
| 120,034 | 194 | 47,473 | 11 / 3 | 11 | 2 / 1368716 |
| 150,035 | 194 | 47,473 | 11 / 3 | 11 | 2 / 1368716 |
| 180,042 | 192 | 47,451 | 11 / 3 | 11 | 2 / 1368716 |

In allen sechs Proben bleiben die neun Schriftgroessen und ein Variations-
Hash gleich; Evictions und uebergrosse Fonts stehen jeweils bei null.
Zwei gepinnte Datenbloecke belegen konstant rund 1,305 MiB. Die 22 KiB
voruebergehender Bilddaten bei 120/150 Sekunden stammen nicht aus neuen
Fontatlanten und sind bis zum letzten Fenster wieder verschwunden.
Anders als in `image-audit-walk12` gibt es in diesem Kurzlauf damit keinen
fortgesetzten Zuwachs der Fontdaten-IDs oder Atlanten.

| Ressource | Baseline Clear/12 | v3 Wetterpuffer | v4 Fontcache |
| --- | ---: | ---: | ---: |
| Peak-RSS, gesamter Prozess (MiB) | 1515,72 | 1496,93 | 1508,36 |
| Peak-DRM-VRAM (MiB) | 4604,91 | 4629,25 | 4093,73 |
| Peak-DRM-GTT (MiB) | 68,14 | 68,14 | 68,14 |

Auch bei stabilen Fontdaten steigt RSS im Messfenster von 1430,48 auf
zuletzt 1490,42 MiB, mit einem Fenstermaximum von 1503,51 MiB. RSS umfasst
weiterhin Streamingdaten und Allokatorreserven, nicht nur CPU-Bilddaten;
die verbleibenden Bytes sind hier nicht einzeln kausal zugeordnet. Alle
sechs getrennten Ressourcenproben melden Swap 0. Nach der ersten Probe
bei 9,21 Messsekunden mit 286 Major-Fault-Deltas seit Prozessbeginn gibt
es keine weiteren Major-Fault-Deltas. Mesh-Assets variieren zwischen
12555 und 14570, Standardmaterialien zwischen 199 und 243. Das ist ein
begrenzter Nachweis fuer den konkreten Fontatlaspfad, keine pauschale
RSS-Paritaet oder vollstaendige Langzeit-/Ressourcenabnahme.
Der gesamte Prozess verbraucht 520,762 User-CPU-Sekunden und 60,532
System-CPU-Sekunden; diese summierten Threadzeiten sind keine Client-
Hauptthreadpause oder CPU-Framezeit.

Der Screenshot `font-cache-walk12-clear.png` wurde auf Fonts/CSS kontrolliert;
keine offensichtlichen Textfehler festgestellt. Das ist keine umfassende
Pruefung aller Schriften, Schriftgroessen, Sprachen oder UI-Zustaende.

### Regen/20 Mit Fontcache

`font-cache-walk20-rain.log` endet nach 180 Messsekunden und 231,982 Sekunden
Prozesslaufzeit mit Exit 0. Alle sechs Zeitfenster und die Enddiagnose
bestaetigen physische 3840 x 2160, Sichtweite 20 und vertikale Sichtweite 3.
Sieben reale Rueckwege mit Serverpositionen erreichen maximal 63,306 m.
WARN-, ERROR- und Panic-Zaehler sind jeweils null. High-/Ultra-Qualitaet,
Rise, volle Vegetation, HUD und integrierte Autoritaet bleiben aktiv;
autoritatives Rain betraegt in allen Fenstern 0,78. Der alte Clear-Wert in
der Grafikdatei wird nicht mit dem effektiven Weltwetter verwechselt.

| Messmethode | FPS | p95 (ms) | p99 (ms) | Groesster Frame (ms) |
| --- | ---: | ---: | ---: | ---: |
| Native 180-s-Zusammenfassung | 63,79 | 24,340 | 30,622 | nicht als Gesamtmaximum ausgegeben |
| Sechs JSON-Fenster, gewichtet | 63,73 | 23,680..25,655 je Fenster | 30,225..33,205 je Fenster | 190,487 |

Die JSON-FPS reichen von 62,16 bis 67,26. Gegenueber der nativen Baseline
mit 43,71 FPS und p99 42,805 ms entspricht das in diesem Einzelpaar rund
+45,9% FPS und -28,5% p99. Gegenueber v3 mit 62,82 FPS ist das Mittel nur
etwa 1,5% hoeher; sein p99 von 31,623 ms sinkt um rund 3,2%. Daraus wird
ohne Wiederholungsserie keine allgemein reproduzierbare zusaetzliche
FPS-Steigerung durch den Fontcache abgeleitet. Der p99 bleibt ueber 30 ms,
und ein einzelner deutlich groesserer Frame ist weiterhin vorhanden.

| Messsekunde | Images | CPU-Bilddaten (MiB) | Fontatlanten / Face-IDs | Atlas-CPU-Daten (MiB) | Pins / Pin-Daten (Byte) |
| --- | ---: | ---: | ---: | ---: | ---: |
| 30,002 | 192 | 47,451 | 11 / 3 | 11 | 2 / 1368716 |
| 60,003 | 192 | 47,451 | 11 / 3 | 11 | 2 / 1368716 |
| 90,007 | 192 | 47,451 | 11 / 3 | 11 | 2 / 1368716 |
| 120,010 | 194 | 47,473 | 11 / 3 | 11 | 2 / 1368716 |
| 150,014 | 194 | 47,473 | 11 / 3 | 11 | 2 / 1368716 |
| 180,019 | 192 | 47,451 | 11 / 3 | 11 | 2 / 1368716 |

Alle sechs Proben bestaetigen unveraendert neun Schriftgroessen, einen
Variations-Hash, null Evictions und null uebergrosse Fonts. Der konkrete
Fontatlas-/Pinningbestand ist damit auch im Regen/20-Kurzlauf stabil; die
anderen Bilddaten zeigen dieselbe kleine temporaere Schwankung von 22 KiB.

| Ressource | Baseline Regen/20 | v3 Wetterpuffer | v4 Fontcache |
| --- | ---: | ---: | ---: |
| Peak-RSS, gesamter Prozess (MiB) | 2131,50 | 2192,36 | 2066,86 |
| Peak-DRM-VRAM (MiB) | 5080,89 | 5263,18 | 5517,89 |
| Peak-DRM-GTT (MiB) | 68,14 | 68,14 | 68,14 |

RSS steigt innerhalb der v4-Zeitfenster von 1956,23 auf 2050,82 MiB.
Sieben getrennte Ressourcenproben einschliesslich einer Startup-Probe
melden Swap 0; von 51 Major-Fault-Deltas liegen 50 vor Messbeginn und eines
bei 9,14 Messsekunden, danach null weitere. Der gesamte Prozess verbraucht
723,956 User- und 58,158 System-CPU-Sekunden, summiert ueber seine Threads,
nicht als Client-Hauptthreadzeit. Mesh-Assets liegen bei 31697..35567,
zuletzt 32321; Standardmaterialien bei 206..262, zuletzt 209. Die native
Enddiagnose meldet 13052 Sections, 83572 geladene und dargestellte Props,
4330 Prop-Batches und 710 Wassertiles.

**Keine globale Speicherparitaet:** Obwohl der RSS-Peak in diesem Paar
niedriger ist, liegt der VRAM-Peak rund 437,00 MiB ueber der Baseline und
254,71 MiB ueber v3. Stabile CPU-Fontdaten garantieren keine gleichen
GPU-Allokationen, Residency oder Gesamtprozessreserven. Diese Unterschiede
werden nicht ohne weitere Zuordnung vollstaendig dem Fontcache oder einem
anderen einzelnen System zugeschrieben.

### 15 Minuten Bodenbewegung Mit Fontcache

`font-cache-walk12-long.log` endet nach 900 Messsekunden und 921,144 Sekunden
Prozesslaufzeit mit Exit 0. Alle 30 Zeitfenster, die effektive Konfiguration
und die native Enddiagnose bestaetigen physische 3840 x 2160, Sichtweite 12
und vertikale Sichtweite 3. 35 vollstaendige reale Rueckwege erreichen
maximal 63,308 m. In 90 Fortschrittsproben steigen die bestaetigten
Input-Sequenzen monoton von 1065 auf 54495; die ausstehenden Eingaben liegen
bei 1..8. Lokale und autoritative Positionen sind protokolliert. WARN-,
ERROR- und Panic-Zaehler sind jeweils null.

High-/Ultra-Qualitaet, Rise, volle Vegetation, HUD und integrierte Autoritaet
bleiben aktiv. Wie beim v3-Langlauf ist Clear der Startzustand: waehrend
der Messung laeuft die Simulation weiter bis Climate mit Intensitaeten
zwischen 0 und 0,630173. Es gibt keinen passenden 900-s-Vorherlauf der
urspruenglichen Baseline.

Die native 900-s-Zusammenfassung meldet **85,18 FPS, p95 18,326 ms und
p99 21,559 ms**. Die 30 JSON-Fenster ergeben gewichtet 85,18 FPS; ihre
FPS reichen von 80,48 bis 89,13, p95 von 17,470 bis 19,597 ms und p99 von
20,312 bis 23,027 ms. Der groesste einzelne JSON-Frame betraegt 144,650 ms.
Auf dieser Route wird bis Minute 15 kein anhaltender FPS-Kollaps beobachtet;
einzelne groessere Frames und andere Szenarien sind damit nicht ausgeschlossen.

Die Kontrollpunkte aus `font-cache-walk12-long.json` zeigen jeweils das
vollstaendige 30-s-Fenster nahe der Zielminute, keine kumulativen
Perzentile. Die tatsaechlichen Messzeiten bleiben erhalten, ohne Interpolation.

| Zielminute | Messsekunde | FPS | p95 (ms) | p99 (ms) | RSS (MiB) | Images |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| 1 | 60,013 | 87,02 | 17,523 | 20,312 | 1389,82 | 192 |
| 5 | 300,052 | 80,48 | 19,096 | 22,390 | 1479,75 | 194 |
| 10 | 600,113 | 83,42 | 18,818 | 21,294 | 1591,49 | 194 |
| 15 | 900,168 | 85,97 | 18,569 | 21,885 | 1699,20 | 194 |
| 30 | ND | ND | ND | ND | ND | ND |

| Image-/Fontressource | Bereich in allen 30 Fenstern | Erster -> letzter Wert |
| --- | ---: | ---: |
| Images | 192..194 | 192 -> 194 |
| CPU-Bilddaten (Byte) | 49756456..49778984 | 49756456 -> 49778984 |
| Textur-Deskriptoren, geschaetzt (Byte) | 130135044..130157572 | 130135044 -> 130157572 |
| Fontatlas-Sets / Atlasrecords | konstant 11 / 11 | 11 / 11 -> 11 / 11 |
| Fontdaten-/Face-IDs | konstant 3 | 3 -> 3 |
| Atlastexturen / vorhandene Atlasimages | konstant 11 / 11 | 11 / 11 -> 11 / 11 |
| Atlas-CPU-Daten / Atlasdeskriptoren (MiB) | konstant 11 / 11 | 11 / 11 -> 11 / 11 |
| Schriftgroessen / Variations-Hashes | konstant 9 / 1 | 9 / 1 -> 9 / 1 |
| Pins / Pin-Daten (Byte) | konstant 2 / 1368716 | 2 / 1368716 -> 2 / 1368716 |
| Evictions / uebergrosse Fonts | konstant 0 / 0 | 0 / 0 -> 0 / 0 |

Die maximale Schwankung der CPU-Bilddaten betraegt 22 KiB ausserhalb der
Fontatlanten, nicht den frueheren fortgesetzten Zuwachs. Die Atlastexturen,
ihre Fontdaten-IDs und der starke Pinningbestand sind ab Sekunde 30 bis
900 in jeder Probe unveraendert. Die CPU-Bilddaten bleiben damit bei
47,451..47,473 MiB, die Deskriptorschaetzung bei 124,106..124,128 MiB.
Letztere ist ausdruecklich keine gemessene VRAM-Allokation oder Residency.
Die 39 Render-world-only-Images und 45 Images im Format 512 x 512 bleiben
ebenfalls konstant. Der konkrete zuvor gemessene Fontatlas-Zuwachs wird
in diesem 15-Minuten-Lauf nicht mehr beobachtet.

| Weitere Ressource | Bereich / Peak | Erster -> letzter 30-s-Wert |
| --- | ---: | ---: |
| RSS in den Zeitfenstern (MiB) | 1379,05..1720,29 | 1379,05 -> 1699,20 |
| Peak-RSS, gesamter Prozess (KiB / MiB) | 1792628 / 1750,61 | nicht fenstersynchron |
| Peak-DRM-VRAM, gesamter Prozess (MiB) | 4121,27 | nicht fenstersynchron |
| Peak-DRM-GTT, gesamter Prozess (MiB) | 68,14 | nicht fenstersynchron |
| Mesh-Assets | 12528..14571 | 12557 -> 13035 |
| Standardmaterialien | 199..273 | 216 -> 237 |
| Entities | 17918..20508 | 17955 -> 18548 |
| Geladene Sections | 3964..4541 | 4541 -> 4150 |
| Gespeicherte Props | 28596..32756 | 30109 -> 29453 |

**Auch v4 zeigt kein globales RSS-Plateau:** Zwischen erstem und letztem
Zeitfenster steigt RSS um 320,14 MiB, rund 23,2%. Zwischen Minute 10 und
15 steigt es weiter von 1591,49 auf 1699,20 MiB. Streamingdaten und
Allokatorreserven sind darin enthalten; eine byteweise Zuordnung des
verbleibenden Wachstums ist mit diesen Zaehlern nicht erfolgt. 30 getrennte
Ressourcenproben melden durchgehend Swap 0. Die erste Probe bei
10,14 Messsekunden enthaelt 60 Major-Fault-Deltas seit Prozessbeginn,
danach keine weiteren. Der Prozess verbraucht 2390,972 User- und
270,561 System-CPU-Sekunden ueber seine Threads; das ist keine Client-
Hauptthreadzeit.

Der Prozess endet waehrend der laufenden Geh-/Streamingroute, nicht nach
einem stationaeren Abfluss aller Arbeit. Die native Enddiagnose meldet noch
96 aktive und 189 wartende Einblendungen. Endzaehler sind deshalb kein
Nachweis einer vollstaendig beruhigten Szene oder aller unveraenderten
Renderidentitaeten.

Der Vergleich mit der vorherigen v3-Langmessung bleibt gesondert:

| 900-s-Messgroesse | v3 Wetterpuffer | v4 Fontcache |
| --- | ---: | ---: |
| Native FPS | 85,62 | 85,18 |
| Native p95 / p99 (ms) | 18,249 / 21,416 | 18,326 / 21,559 |
| Images, erster -> letzter Wert | 207 -> 627 | 192 -> 194 |
| Peak-RSS, gesamter Prozess (MiB) | 2000,86 | 1750,61 |
| Peak-DRM-VRAM (MiB) | 4120,94 | 4121,27 |
| Groesste gemessene Swap-Probe (MiB) | 125,86 | 0 |

Das native FPS-Mittel liegt in v4 rund 0,5% unter v3; es wird keine
zusaetzliche Langlauf-FPS-Steigerung behauptet. Der geringere RSS-Peak,
fehlender gemessener Swap und stabile Fontdaten sind konkrete Befunde
dieses Paars, keine globale Speicherparitaet oder vollstaendige
Ressourcenfreigabe fuer andere Szenen. Der v3-Langlauf wird weder ersetzt
noch nachtraeglich als ressourcenstabil erklaert.

Die v4-Kurzlaeufe und dieser 15-Minuten-Lauf sind abgeschlossen. Offen
bleiben weitere Seeds, laengere oder andere Routen, ein nativer
4K-Langlauf mit Sichtweite 20 sowie die Zuordnung des verbleibenden
RSS-Wachstums. 30 Minuten sind ND, nicht durchgefuehrt. Eine pauschale
Behebung der gesamten Folge 110 -> 50 -> 80 -> 30 FPS oder aller
Frame-Spitzen wird weiterhin nicht behauptet.

## CPU-Befund

Die Werte stammen ausschliesslich aus den Intervallen der beiden gemessenen
Vorher-JSONs. Verschachtelte Scopes werden nicht addiert.

| Scope | Mittel je Aufruf, Weite 12 (ms) | Mittel je Aufruf, Weite 20 (ms) | Aufrufe Weite 12 / 20 |
| --- | ---: | ---: | ---: |
| `terrain.dispatch` | 0,920 | 5,187 | 13137 / 7870 |
| `terrain.refresh.queue.full` | 2,038 | 5,630 | 2118 / 2499 |
| `terrain.refresh.queue.arrivals` | 1,226 | 3,519 | 2028 / 3372 |
| `props.dispatch` | 0,188 | 1,173 | 13137 / 7870 |

Bei Sichtweite 20 ueberschreitet `terrain.dispatch` in den Messintervallen
1237-mal 8 ms. 2153 Animationsinvalidierungen treffen dort auf 2499 volle
Auswahlscans; das bestaetigt wiederkehrende Auswahlkosten, aber keine
eindeutige Zuordnung jedes langsamen Frames zu einer einzelnen Ursache.

`character.current` benoetigt insgesamt 109,255 ms bei 28798 Aufrufen
(Weite 12) bzw. 106,942 ms bei 31136 Aufrufen (Weite 20). Die gemessenen
Stroemungsabfragen sind auf dieser trockenen Gehstrecke daher nicht der
primaere nachgewiesene Kostenpfad. Die Optimierung ihrer trockenen
Spaltenabfragen ist separat zu pruefen und keine bewiesene Erklaerung fuer
den gesamten FPS-Einbruch.

`cumulative_max_ms` ist ein Maximum seit Prozessbeginn einschliesslich
Startup, auch wenn es spaeter in einem Intervall erscheint. Beispielsweise
ist ein `props.dispatch`-Maximum ueber 8 ms bei null entsprechenden
Intervall-Ueberschreitungen kein neuer Geh-Spike.

`storage.save_edits` zeigt kumulative Maxima von 2058,985 bzw. 317,923 ms.
Die regulaeren integrierten Autosaves schreiben asynchron auf einem Worker;
dieser Scope misst keine Client-Hauptthreadpause von zwei Sekunden.
Globale Scope-Snapshots enthalten Client-, Server- und Workerarbeit.
Der neue separate `integrated.autosave.snapshot`-Timer soll die Kopie auf
dem Serverthread von der Dateiarbeit unterscheiden. Worker-/I/O-Konkurrenz
ist damit noch nicht kausal ausgeschlossen oder einem einzelnen Frame
zugeordnet.

GPU-Passdiagnosen sind letzte Einzelproben, keine komplette GPU-Framezeit.
Draw-API-Zaehler liefern bei indirekter Ausgabe keine vollstaendige Zahl
tatsaechlich ausgefuehrter GPU-Subdraws. RSS enthaelt keine ausgelagerten
Prozessseiten; Swap und Major-Faults bleiben eigenstaendige Messgroessen.
Kontextdaten zu den langsamsten Frames stammen aus dem vorhergehenden
Frame und erlauben Korrelation, keine Behauptung eines GPU-Stalls.

## Gezielte Aenderungen

- Terrain-Dispatch erhaelt nach vollstaendiger Anfangsbereitschaft ein weiches
  2-ms-Vorbereitungsbudget. Mindestens eine nuetzliche Uebergabe bleibt erlaubt;
  weitere Kandidaten bleiben priorisiert fuer Folgeframes erhalten. Ein
  einzelner Snapshot oder Auswahlscan kann das Budget weiterhin ueberschreiten.
- Das Ende einer Terrain-Einblendung prueft nur die betroffenen aktuellen
  Terrainwurzeln erneut. Fokus-/LOD-Wechsel waehrend einer aktiven Einblendung
  werden danach weiterhin abgearbeitet. Fremde, ausgemusterte und noch aktive
  Einblendungen loesen keinen globalen Terrain-Auswahlscan aus. Props verwenden
  denselben gezielten Wiederholungsablauf mit aktueller Batch-Identitaet.
- `SectionPublished`, `SectionRemoved` und `PreparedColumnChanged` melden
  erfolgreiche aktuelle Section-Veroeffentlichungen, tatsaechliche Evictions
  und geaenderte vorbereitete Spalten. Terrain und Props lesen diese Bevy-
  Meldungen unabhaengig und halten betroffene Positionen in ihren begrenzten
  Arbeitsmengen fest, bevor ein volles Workerlimit zum Warten fuehrt. Die
  Meldungspuffer altern regulaer; unerledigte Kandidaten bleiben trotzdem
  erhalten. Das ersetzt den globalen Loaded-Scan bei normalen Arrivals.
- Gemischte Arrivals und Evictions entfernen nur tatsaechlich fehlende
  Positionen aus den Rendercaches und Wartemengen. Vorbereitete Spalten
  invalidieren die betroffenen Exposure-Nachbarschaften und die angrenzenden
  Meeresspiegel-Sections; die Kuesten-Halomasken werden lokal aktualisiert.
  Direkte Store-Aenderungen ohne Meldungen behalten den konservativen Scan.
  Replikat-Reset leert alte Meldungspuffer, und ein Weltwechsel setzt die
  Bereitschaft fuer das Vorbereitungsbudget zurueck.
- Generierungsdispatch veraendert seine internen Queues ohne
  `ChunkStore`-Invalidierung. Echte Section-Veroeffentlichung bleibt sichtbar.
  Replizierte Reloads legen keine autoritativen Segment-Supportauftraege an.
- Trockene Stroemungspruefungen nutzen vorbereitete Spaltendaten;
  Wasserfallkontakt, Live-Overrides und der Rueckfall bei fehlenden Spalten
  bleiben erhalten. Ein synthetischer CPU-Benchmark ersetzt keinen nativen
  Gameplayvergleich.
- Zusaetzliche Timings trennen Terrain-Vorbereitung, Charakterbewegung,
  Stroemung, Collider-Aktivierung und Autosave-Snapshot.

Die Codeaenderungen reduzieren keine Sichtweite, Vegetation, Wasser- oder
Schattenqualitaet. Nebenlaeufigkeit und Uploadquoten bleiben erhalten. Das
weiche Zeitbudget ist keine harte Obergrenze fuer einen vollstaendigen Frame.

## Regressionen

`tests-font-cache.log` bestaetigt die aktuelle komplette v4-Suite mit **1506
bestandenen, null fehlgeschlagenen und zwei ignorierten Tests**. Ignoriert
bleiben der explizite 180-s-UDP-Soak und der opt-in Wallclock-Mikrobenchmark
der Tier-Wasserabfragen. Formatpruefung, `check-font-cache.log`,
`build-font-cache.log` und `clippy-font-cache.log` bestehen.
`native-font-regression.log` bestaetigt den nativen Systemfonttest explizit
ohne SKIP: vor der Stabilisierung wechselnde Blob-ID nach Pruning,
anschliessend 256 erfolgreiche Prune-/Reloadzyklen mit gleichbleibender ID.
Die aeltere v3-Suite in `tests-complete.log` bestand mit 1500 Tests, zwei
ignorierten Tests sowie erfolgreichem Build/Clippy. Die v3-Laeufe und die
beiden v4-Kurzlaeufe sind separat gemessen; auch der v4-Langlauf ist mit
Exit 0 abgeschlossen. Sein konkreter Fontatlas-/Pinningpfad bleibt ueber
15 Minuten stabil, waehrend RSS weiter waechst. Weitere Seeds und ein
4K-Langlauf mit Sichtweite 20 sind nicht abgenommen.
Der v3-Langlauf mit Bild-/Speicherwachstum und spaetem Swap bleibt ohne
vollstaendige Ressourcenabnahme; 30 Minuten sind nicht gemessen.

Fruehere Testversuche der drei Meldungsablauf-Fixtures schlugen wegen nicht
zuverlaessig fortgeschriebener Testzeit fehl. Die Fixtures fuer Welt-, Terrain-
und Prop-Meldungen verwenden jetzt explizite
`TimeUpdateStrategy::ManualDuration`; ihre Meldungsalterung ist damit
deterministisch. Die fehlgeschlagenen Zwischenversuche werden nicht als
bestandene Laeufe gezaehlt; die oben genannte aktuelle komplette Suite
enthaelt die korrigierten Tests und besteht.

Wesentliche neue Regressionen:

- `publication_messages_cover_installed_outputs_and_expire_without_consumers`:
  nur akzeptierte aktuelle Ausgaben, sichtbare vorbereitete Spalte und
  auslaufende Meldungspuffer.
- `interest_messages_identify_only_actual_section_and_column_evictions` und
  `replica_reset_discards_streaming_messages_from_the_previous_session`:
  echte Entfernungen und saubere Sessiongrenzen.
- `dispatch_bookkeeping_invalidates_render_consumers_only_when_a_section_is_published`
  und `repeated_replica_section_reload_never_schedules_authoritative_fluid_or_support_work`:
  interne Dispatcharbeit und autoritative Simulationsauftraege.
- `published_terrain_arrivals_survive_full_workers_and_message_expiry` und
  `streamed_prop_messages_survive_full_workers_and_preserve_pending_lods_and_stable_batches`:
  Meldungen duerfen bei vollen Workern altern, ohne Arrivals, LOD-Arbeit oder
  unveraenderte Root-/Mesh-Identitaeten zu verlieren.
- `terrain_selection_retries_only_finished_roots_after_entry` und
  `prop_selection_retries_only_finished_roots_after_entry`:
  LOD-Konvergenz bei danach stationaerer Kamera; fremde oder ausgemusterte
  Roots loesen keine Ersetzung aus.
- `sparse_coastal_arrival_and_removal_touch_only_neighboring_sea_sections`:
  lokale Kuestenmasken bei Ankunft und Entfernung, einschliesslich vieler
  unveraenderter entfernter Cacheeintraege.
- `preparation_cap_preserves_priority_and_pending_candidates_between_frames`,
  `initial_preparation_fills_all_available_workers_without_a_time_cap` und
  `a_world_switch_resets_the_cap_without_reusing_the_previous_world_ready_flag`:
  Reihenfolge, erhaltene Kandidaten, Anfangsladen und erneuter Weltstart.
- `storage_layout_matches_three_wgsl_vectors_and_preserves_all_channels`,
  `snapshot_keeps_live_weather_precision_and_the_unwrapped_growth_clock` und
  `animated_weather_updates_payload_without_modifying_the_shared_asset`:
  48-Byte-Layout, aktuelle Wetterkanaele und stabile Asset-Identitaet des
  neuen Wetterpuffers.
- `font_pins_deduplicate_and_release_the_least_recently_used_blob` und
  `font_pins_enforce_the_byte_budget_and_skip_oversized_fonts`:
  Deduplizierung, LRU-Freigabe sowie harte Anzahl-/Bytegrenzen des Pinnings.
- `shared_system_font_sources_with_pins_preserve_blob_ids_across_cache_churn`
  und `font_cache_installation_accepts_a_fixture_without_text_resources`:
  echter nativer Fontpfad, 256 stabile Reloadzyklen und optionale Textressourcen.
- `image_aggregation_separates_cpu_data_descriptors_and_font_atlas_overlap`:
  getrennte Bilddaten-/Deskriptorwerte und keine doppelte Atlaszaehlung.

## Reproduktion

`tools/benchmarks/run-soak.sh <binary> <label> walk` verwendet echte Eingaben
fuer 180 Sekunden. Fuer diese gespeicherte Szene gehoeren ein frischer
isolierter Weltklon, `TERRA_BENCH_WORLD_ID=6690318646380802212`, ein absoluter
`TERRA_BENCH_SAVE_DIR` und das dazugehoerige isolierte `TERRA_WORLD_DIR` zum
Aufbau. Die Spielerdatei muss vor dem Start wieder dieselbe Mainland-Pose
enthalten; ein bereits durchlaufener und automatisch gespeicherter Klon
ist kein identischer Startzustand.

`TERRA_SOAK_RESULT_DIR` zeigt auf diesen Messordner,
`TERRA_SOAK_CONFIG_DIR` auf `configs12` bzw. `configs20` und
`TERRA_SOAK_WEATHER` auf `clear` bzw. `rain`. Der Runner aktiviert echten
integrierten Server, HUD/Minimap, Grafikdatei, CPU-Scopes, Renderdiagnosen,
Speicherzeitreihe und Screenshot. JSON und CSV werden aus demselben Log
erzeugt. Die Originalwelt wird fuer diesen Aufbau nicht als Saveziel benutzt.

## Abschlusskontrolle

Die vollstaendige Pruefung von `source-world.sha256` mit
`sha256sum --check --quiet` endet mit Exit 0: Die Originalwelt ist
unveraendert. Der separate Server-Smoke in `headless-font-cache.log`
endet ebenfalls mit Exit 0 und 27 geladenen Sections.

Erst **nach allen nativen Benchmarks** wurden zwoelf alte Terra-ELF-Binaries
aus dem RAM-basierten `/tmp` nach `target/phase92/legacy-tmp-baselines`
verschoben. Die alten Pfade bleiben als Symlinks erhalten; alle SHA256-
Pruefungen vor und nach dem Verschieben bestehen, dokumentiert durch
`legacy-tmp-before.sha256`. Damit wurden 11474733512 Byte, rund 10,69 GiB,
aus `/tmp` ausgelagert; dessen Belegung sank von rund 15 G auf 4,3 G (28%).
Es gibt keine FPS-Messung nach dieser Bereinigung und keine daraus
abgeleitete FPS-Verbesserung. Alle oben berichteten nativen Messwerte
stammen vor der Bereinigung.
