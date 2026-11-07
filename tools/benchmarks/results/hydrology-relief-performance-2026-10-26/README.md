# Hydrologie, Relief Und Streaming (Phase 139)

## Umfang

F41 und F42 erweitern die vorhandenen RiverPath-, Lake-/OasisSample- und
WaterColumn-Systeme. Es gibt keine zweite Fluessigkeitssimulation. Neue Welten
verwenden Landschaftsrevision 3; gespeicherte Revisionen 0, 1 und 2 sowie deren
Flussoption bleiben unveraendert. Alte Templates werden nur bei der Erstellung
einer neuen Welt auf die neue Revision angehoben.

## Hydrologie

Die neue regionale Hydrologie verwendet 2048 x 2048 m grosse Einzugsgebiete.
Je geeignetem Gebiet entstehen ein gemeinsam gecachter Hauptflusslauf und
hoechstens ein See. 49 Stuetzpunkte bestimmen den gewundenen Verlauf zwischen
hoeherer Quelle und tieferer Muendung; Abstaende und Kompassrichtung variieren.
Ein Hauptkurs wird nur mit gueltigem Binnen-See oder gepruefter Meeresmuendung
freigegeben. Nicht jedes Einzugsgebiet muss einen solchen Kurs enthalten.

Seen bestehen aus drei verbundenen, unregelmaessigen Loben mit gemeinsamem
Pegel. Die Haupt-Radiusklassen sind 46-70, 80-112 und selten 138-160 m;
dies ist kein garantierter Durchmesser oder Flaechenanteil. Ein Binnen-See
kann den Fluss aufnehmen; bei passenden Kuestenbedingungen liegt ein See
innerhalb eines Quelle-See-Meer-Kurses mit Zu- und Abfluss.

Die Uferpruefung folgt exponierten Seeumrissen mit ungefaehr einem Meter
Bogenabstand sowie zwei aeusseren trockenen Ringen. Die RiverSamples passen
ihren Pegel anhand der exakt gesampelten Seeform an: im See-/Uferbereich
derselbe Pegel, ausserhalb eine graduelle Zusammenfuehrung ueber 32 m. Der
tiefere Erosionssaum eines Abflusses darf das hoehere Seeufer nicht durchbrechen.
Hauptfluss-Stuetzpegel verwenden Stuetzhoehen entlang des tatsaechlich
interpolierten Kurses; unter Fallstufen liegen breitere, tiefere Becken.

Meeresmuendungen benoetigen ein Ocean-Gewicht von mindestens 0,35 und
Terrain unter dem Meeresspiegel. Zusaetzlich prueft eine begrenzte Suche
mit hoechstens 81 Knoten eine zusammenhaengende Kuesten-Wasserflaeche zu
staerkerem Ocean-Gewicht. Jeder Meter zwischen benachbarten Suchknoten
muss unter dem Meeresspiegel bleiben. Eine beliebige tiefe Binnensenke
zaehlt damit nicht automatisch als Meer.

Die bestehenden kurzen F34-Bachkurse bleiben in Cherry Forest, Forest und
Highlands erhalten. Vor ihrer Freigabe prueft der Generator den gesamten
Bach konservativ gegen die See-AABBs und gepufferten Hauptfluss-Segment-AABBs
der wenigen beruehrten regionalen Arc-Pfade. Bei Konflikt wird der ganze
Bach verworfen und das Ergebnis gecacht, statt ihn an einer Biom- oder
Chunk-Grenze abzuschneiden. Es entstehen dabei keine unabhaengigen Pegel
ueber neuen Seen oder Hauptfluessen. Mit `rivers=false` entfallen die neuen
Hauptkurse; geeignete Seen und konfliktfreie F34-Baeche bleiben moeglich.

Der bestehende RiverCache haelt hoechstens 256 regionale Eintraege und
separat hoechstens 256 alte Fluss-/Bacheintraege. OnceLock dedupliziert
gleichzeitige Kaltgenerierung derselben Region zwischen Generator-Klonen.
Es gibt keine neue pro-Voxel-Topologiesuche und keine zweite Wassersimulation:
WaterColumns, reale Fluid-Zellen, natuerliche Auslaesse und die vorhandenen
budgetierten Wasserupdates bleiben zustaendig.

Reproduzierbarer Diagnoselauf mit dem fertig gebauten Beispiel, ohne Renderer:

```bash
target/debug/examples/hydro_connections --radius=2
```

Der Lauf untersucht pro Seed 25 Einzugsgebiete mit Indizes -2 bis 2 in X/Z.
Die Rohdaten stehen in [hydro-diagnostics.log](hydro-diagnostics.log).
Kurslaengen messen die ganze horizontale Polygonlinie und koennen erhaltene
ueberdeckte Flussabschnitte einschliessen; sie sind keine ausschliesslich
offen sichtbaren Flusslaengen.

| Seed | Gebiete | Hauptkurse | Seen | Meeresmuendungen | Kurslaenge (m) | Haupt-Radius (m) |
|---|---:|---:|---:|---:|---:|---:|
| 42042 | 25 | 17 | 12 | 13 | 956,6-2037,4 | 46-107 |
| 7 | 25 | 15 | 8 | 11 | 1167,7-1843,0 | 55-112 |
| 99173 | 25 | 17 | 8 | 16 | 815,6-1742,2 | 53-100 |

Von den Seen liegen 8 / 4 / 7 in Quelle-See-Meer-Kursen. Der gescannte
Ausschnitt enthaelt keinen See der seltenen 138-160-m-Haupt-Radiusklasse;
deren sichtbares Vorkommen ist durch diesen Lauf nicht nachgewiesen.

| Seed | Kalter Diagnosedurchlauf (ms) | Warme Abfragepaare | Zeit der warmen Abfragepaare (ms) |
|---|---:|---:|---:|
| 42042 | 121,904 | 8330 | 5,927 |
| 7 | 99,983 | 7350 | 4,701 |
| 99173 | 120,186 | 8330 | 5,288 |

Ein warmes Abfragepaar besteht aus `river_at` und `lake_at` an derselben
Kursposition. Die kalte Zeit umfasst Regionsaufbau, Suche nach geeigneten
Ankern, Wasserabfragen und Diagnoseausgabe. Diese Zeiten sind weder reine
Chunk-Generierungszeiten noch FPS- oder GPU-Messungen. Es lief ein einzelner
Diagnosedurchgang; sie sind keine statistische Timing-Garantie.

Diese aktuellen Anker erfuellen die Produktions-WaterColumn-Pruefung.
Flussanker sind offen, mindestens einen Meter tief und liegen ueber dem
Meeresspiegel. Koordinaten sind X/Z, nicht der Aufstellpunkt einer Kamera:

| Seed | Typ | X | Z | Seepegel / See-Haupt-Radius |
|---|---|---:|---:|---|
| 42042 | See | 1024 | 1024 | 71,624 m / 58 m |
| 42042 | Fluss | 451,074 | 439,593 | - |
| 7 | See | 1248 | 1696 | 14,619 m / 112 m |
| 7 | Fluss | 1626 | 1285,334 | - |
| 99173 | See | 4672 | -1248 | 10,577 m / 100 m |
| 99173 | Fluss | 4511,926 | -1610,593 | - |

Grenzen: Es gibt keinen globalen Flussgraphen und keine Verknuepfung
mehrerer Einzugsgebiete zu einem unbegrenzt langen Fluss. Kurze F34-Baeche
werden erhalten, aber nicht automatisch als echte Nebenfluesse an den
Hauptkurs angeschlossen. Die regionale Haupt-Hydrologie benoetigt aktuell
aktivierte Biome; `biomes=false` oder eine explizit komplett flache Welt
erhaelt keine solchen regionalen Kurse/Seen. Gespeicherte alte Revisionen
behalten dagegen ihre bisherigen Erzeugungsregeln.

Die zehn neuen Hydro-/Bachregressionen sind im World4-Lauf bestanden,
einschliesslich dichter realer Wasserufer-Nachbarschaften, kontinuierlicher
Seeanschluesse ueber drei Seeds, negativer Section-Grenzen, Query-Reihenfolge,
Reload, realer Fluid-Zellen sowie groesserer und unterschiedlich langer Kurse.
Das ersetzt keine vollstaendige native Sichtpruefung aller Kursabschnitte.

## Relief

Die Reliefkorrektur erhaelt mehr vom vorhandenen lokalen Hoehenfeld in Ebenen,
Plateaus und Gebirgskanten. Eine weiche, oertlich variable Senkenkompression
ersetzt die bisherige konstante Mindesthoehe. Zusaetzliche Noise-Abfragen pro
Voxel werden nicht eingefuehrt.

Kontrollierter CPU-Vergleich: je 12 identische 64-m-Ausschnitte pro Landform,
einheitliches Wald-Biomprofil, Wasser deaktiviert, Revision 2 gegen 3.
Die Zahlen sind keine globalen Hoehenverteilungsprozente oder FPS-Messungen.

| Seed | Ebenen: mittleres Relief alt / neu (m) | Plateau alt / neu (m) | Senke alt / neu (m) |
|---|---:|---:|---:|
| 42042 | 1,39 / 5,10 | 6,51 / 9,11 | 2,11 / 4,90 |
| 71 | 1,41 / 5,17 | 9,65 / 11,62 | 1,42 / 5,74 |
| 918273 | 1,56 / 6,37 | 8,50 / 9,53 | 1,89 / 5,46 |

Die unveraenderten Huegelregionen liefern in diesem kontrollierten Vergleich
identische Werte. Natuerliche flache Blockstufen und ruhige Plateauabschnitte
bleiben erlaubt. Der Regressionstest prueft groessere 16-m-Flaechen statt
voxelweiser Rundungsphasen einzelner Trittstufen.

## Performance-Aenderungen

- Cave-Interest wird direkt in der gemeinsamen Section-Menge aufgebaut.
- PreparedColumn verwirft offensichtlich unpassende Hoehen ueber einen exakten
  aggregierten Bereich, bevor die 256 Oberflaechenzellen untersucht werden.
- Die Chunk-Animation verwendet persistente ECS-Abfragen statt neuer
  QueryStates pro Frame und Einzelanimation.
- Wasser-Uploads verwenden Section-/Wasser-Publikationen und Entfernungen
  statt vollstaendiger Scans bei reinen Chunk-Verwaltungsaenderungen.
  Deduplizierte Retries, Fluid-Revisionen und das bestehende Upload-Budget bleiben
  erhalten.
- Ruhige Meeresflaechen verwenden auch bei eingeschalteten Fluessen das
  vorhandene Raster mit 289 gemeinsamen statt 1024 duplizierten Vertices.
  Dreiecksdichte, Ufer-/Tiefendaten und Wellen bleiben erhalten; lokale Pegel,
  Stroemungen, Wasserfaelle und unterirdische Becken behalten ihren Flusspfad.
- Der Channel-Mesher prueft Section-Hoehe und verdeckte Zellen vor den vier
  Oberflaechen-Eckpunktsamples. Ausserhalb der richtigen Hoehe werden keine
  Eckpunktsamples mehr berechnet. Die Regression prueft echte Aufrufzaehler
  und weiterhin unveraenderte ausgegebene Geometrie.

Keine Verringerung von Sichtweite, Vegetation, Schatten oder Wasserqualitaet.
Kein Vendor-/Registry-Patch und keine Dependency-Aenderung.

## Ausgangsmessungen

Ryzen 9 5900X, RX 6950 XT, Vulkan/RADV, VSync aus. Kein paralleler Compiler
oder Test waehrend der nativen Messungen. Das vorherige ausfuehrbare Programm
liegt lokal unter `/tmp/teramic139-before-client`. Dieselben alten
Landschaftsrevision-0-Szenen dienen dem FPS-Vergleich; neue Hydrologie wird
getrennt getestet, damit unterschiedliche Geometrie keinen Gewinn vortaeuscht.

| Szene / Stand | AVG FPS | Min FPS | p95 ms | p99 ms | Max Frame ms | Frames unter 32 FPS |
|---|---:|---:|---:|---:|---:|---:|
| Integrated, vorher | 83,25 | 27,26 | 19,99 | 25,20 | 36,69 | 11 / 4994 |
| Integrated, nachher | 91,46 | 27,15 | 17,32 | 22,68 | 36,83 | 9 / 5487 |
| Integrated, vorher Kontrolllauf | 88,94 | 29,67 | 18,07 | 23,05 | 33,70 | 2 / 5336 |
| Integrated, nachher Kontrolllauf | 87,95 | 29,87 | 18,30 | 23,30 | 33,48 | 3 / 5277 |
| Wasser, vorher | 80,58 | 28,26 | 20,56 | 22,69 | 35,39 | 2 / 1925 |
| Wasser, nachher | 75,54 | 25,02 | 22,48 | 24,53 | 39,97 | 2 / 1804 |
| Wasser, vorher Kontrolllauf | 81,43 | 27,05 | 19,90 | 22,24 | 36,97 | 2 / 1945 |
| Wasser, nachher Kontrolllauf | 80,66 | 32,19 | 21,41 | 23,16 | 31,06 | 0 / 1927 |

Integrated: Seed 5866727212944175842, 1920x1080, Sichtweite 20, Regen,
integrierter Server, echtes HUD/Minimap, 60 Sekunden Flug-Eingaben.
Wasser: Seed 7, 1280x800, Sichtweite 20, Regen, 24 Sekunden Offline-Hin-/Rueckflug.
Die beiden Szenen sind untereinander nicht als absolute FPS-Werte vergleichbar.
Konfigurationen: `../flight-frame-budget-2026-10-26/integrated-config/` und
`../flight-frame-budget-2026-10-26/water-config/`.

Die Kontrolllaeufe relativieren den ersten AVG-Gewinn. Es gibt keinen
belastbaren allgemeinen Mindest-FPS-Gewinn: einzelne Fluege unterschreiten
weiter 32 FPS, und der Wasservergleich liefert keinen AVG-Fortschritt.
Der beste einzelne Wasserlauf ist deshalb keine Abnahme des FPS-Problems.
Die beiden Integrated-Ladezeiten vorher waren 36,38 / 36,25 s, nachher
36,17 / 35,95 s. Der im Benchmark gemeldete RSS lag dort vorher bei
2.325.952 / 2.283.516 KiB, nachher bei 2.399.456 / 2.282.980 KiB.
Das belegt weder einen Speichergewinn noch einen Langzeitverlauf.

Ein separater instrumentierter Ausgangslauf misst Main-/Render-Wallzeiten und
GPU-Passes. Die Framezeiten sind nicht mit den uninstrumentierten Messungen
gleichzusetzen. Die ungefaehren Mittelwerte Main 10,36 ms und Render 9,68 ms
ueberlappen zeitlich und duerfen nicht addiert werden. Opaque-Pass im Mittel
2,09 ms, Early-Prepass 1,16 ms. Die Instrumentierung beweist noch nicht die
Ursache jedes einzelnen langsamen Frames.

Ein separater Versuch mit unveraendertem Ausgangsbinary und vorhandener
Smoke-Option `TERRA_MESH_SLAB_MIB=64` ergab 73,35 AVG / 24,98 Min FPS,
p95 21,70 ms, p99 27,82 ms und 18 Frames unter 32 FPS. Das kleinere Slab-Limit
wird deshalb **nicht** in das normale Spiel uebernommen. Dieser Versuch
belegt keinen Gewinn durch kleinere GPU-Buffer.

### Framegenaue Eingrenzung

Zusaetzliche Instrumentierung ist ausschliesslich ueber
`TERRA_CPU_SCOPES=1 TERRA_SCOPE_TIMELINE=1` aktiv. Ein fester Ring mit 32768
abgeschlossenen CPU-Spans wird fuer die zwoelf schlechtesten tatsaechlichen
Update-zu-Update-Fenster zwei Frames verzoegert ausgewertet. Verschachtelte
und parallele Spans sind nicht addierbar; die Thread-ID ist nur der Startthread
eines Spans, nicht dessen CPU-Auslastung oder ein GPU-Zeitnachweis.
Normales Gameplay legt weder Ring noch Scope-Zeitstempel an.

Der erste Timeline-Lauf (`teramic139-timeline.log`) zeigt fuer vier von fuenf
Frames unter 32 FPS einen 16,12--18,43-ms-Aufruf von
`allocate_and_free_meshes`. Im schlechtesten 38,47-ms-Fenster wartet der
Render-Handoff 17,15 ms. Der fuenfte Ausreisser hat einen anderen Schwerpunkt
in Render-Ausfuehrung/Main-Scheduling. Die alte Logspalte `main_thread` in
diesem ersten Rohlog meint den Sampling-Thread; sie wurde anschliessend in
`same_sampling_thread` korrigiert.

Der folgende reine Diagnoselauf (`teramic139-handoff.log`) protokolliert
zusaetzlich die wirklich extrahierten Mesh-Mengen. Cloud-Asset 1634/Generation0
wird nach dem Startfenster drei Mal komplett ersetzt: jeweils etwa 404000
Vertices, 22,6 MB Vertexdaten und 2,42 MB Indexdaten. Die Veroeffentlichungen
um 21:47:42,400 / 21:47:51,669 / 21:48:06,640 UTC liegen direkt vor den
gemessenen 32,06 / 31,20 / 39,29-ms-Frames. Damit ist ein eigener,
unbudgetierter Vollhimmel-Upload als konkreter Streaming-Ausloeser belegt.
Grosse Prop-Materialbatches bis etwa 265000 Vertices bleiben ein weiterer
Messpunkt. Die Diagnose-FPS sind nicht mit profillerfreien Laeufen zu mischen.

Der finale Wolkenpfad verwendet feste geografische 2x2-Bankgruppen. Der Key
enthaelt Gruppenkoordinaten, Skalierungsring und eine 4-Bit-Mitgliedermaske;
dadurch behalten Randgruppen und die Aussparungen der fernen Ringe exakt ihre
bisherige Geometrie, ohne zusaetzliche Wolken. Unveraenderte Gruppen behalten
Mesh- und Entity-Handles. Nur neue Gruppen oder geaenderte Mitgliedermasken
werden in maximal vier laufenden/fertigen Jobs aus jeweils hoechstens vier
Originalbanken erzeugt, ausschliesslich im Worker. Dieselbe gemeinsame
Upload-Budget-Ressource laesst in ihrer Cloud-Domaene 32000 Vertices und vier
Gruppen je Frame zu; ihre vorhandene Solo-Oversize-Ausnahme bleibt erhalten.
Terrain-, Prop- und Wasserquoten bleiben unveraendert.
Der bisherige komplette Himmel bleibt sichtbar, bis alle Zielgruppen staged
sind und mindestens zwei weitere Main-Updates verstrichen sind. Das ist ein
Pipeline-Wartefenster, kein behaupteter GPU-Residency-Nachweis.

Es gibt einen gemeinsamen Wind-Root und ein gemeinsames Cloud-Material.
Maximal 73 sichtbare plus 73 vorbereitete Gruppenmeshes und vier CPU-Jobs
werden gehalten; obsolete Arbeit und Assets werden entfernt. High stellt
dieselben 129 logischen Banken jetzt mit 46 Meshgruppen dar, Ultra seine 225
Banken mit 73 Gruppen. Damit gibt es weiterhin mehr Mesh-Entities und
moegliche Draws als beim urspruenglichen Monolithen, aber weniger als im
gemessenen Einzelbank-Zwischenstand. `cloud_uploads` zaehlt im finalen Pfad
Gruppen, nicht komplette Himmel oder einzelne Originalbanken, und ist nicht
direkt mit den alten Zaehlern vergleichbar.
Geometrie-Regressionen vergleichen alle urspruenglichen Face-Attribute und
Indices, einschliesslich negativer Koordinaten, Mitgliedermasken, Rand- und
Ring-Aussparungen sowie Hoehen bei Sturmwachstum. Die FPS-Wirkung dieses
2x2-Gruppenstands ist zum Zeitpunkt dieser Dokumentation noch nicht gemessen.

### Gemessener Cloud-Fix und verbleibende Ausreisser

Die folgenden Messungen betreffen ausdruecklich den historischen Zwischenstand
mit 129 Einzelbank-Meshes, nicht die finale 2x2-Gruppierung.
Der saubere Cloud-Reuse-Lauf (`teramic139-cloud-clean.log`) nutzt dieselbe
integrierte Regenflug-Szene, Aufloesung, Sichtweite und das 60-s-Messfenster wie
oben, ohne die zusaetzliche Scope-/Handoff-Instrumentierung:

| AVG FPS | Min FPS | p95 ms | p99 ms | Max Frame ms | Frames unter 32 FPS | RSS KiB | Initiales Laden s |
|---:|---:|---:|---:|---:|---:|---:|---:|
| 90,2712 | 27,59185 | 17,8087 | 22,1809 | 36,2426 | 5 | 2332356 | 36,2393 |

Der Screenshot des Zwischenstands zeigt dasselbe Wolkenbild; sein Endstand hat exakt
100986 Faces wie vorher, nun verteilt auf 129 Banken statt einen Monolithen.
Diese erhoehte Mesh-/Entity- und potenzielle Draw-Anzahl ist beim Vergleich
der Gesamtzaehler zu beruecksichtigen. Der einzelne Lauf belegt keinen
allgemeinen AVG-Gewinn gegenueber allen Kontrolllaeufen. Die Mindest-FPS bleiben
unter 32; der gesamte Performance-Fehler ist damit **nicht abgenommen**.

Im separaten Wasserszenario verursachte derselbe Einzelbank-Zwischenstand
einen reproduzierbaren AVG-Rueckgang gegenueber einer frischen Kontrolle:

| Wasserstand vor Gruppierung | AVG FPS | Min FPS |
|---|---:|---:|
| Frische Kontrolle vor Cloud-Einzelbanken | 79,219 | 26,53 |
| 129 Einzelbank-Meshes, Lauf 1 | 75,248 | 29,33 |
| 129 Einzelbank-Meshes, Kontrolllauf | 73,777 | 31,61 |

Alle drei Szenen enthalten dieselben 3756 Terrain-Sections, 27122 Props und
1681 Wasser-Tiles; der Einzelbankpfad fuegt 128 Cloud-Meshes hinzu. Die
verbesserten einzelnen Minima heben den AVG-Rueckgang nicht auf. Dieser
Tradeoff ist der gemessene Anlass fuer die anschliessende feste
2x2-Gruppierung; deren Wirkung darf erst nach neuen nativen Laeufen bewertet
werden. Die Wasserwerte sind nicht mit dem integrierten Regenflug oben als
absolute FPS-Werte vergleichbar.

Der getrennte instrumentierte Lauf (`teramic139-cloud-profile.log`) misst 83,68 AVG /
26,11 Min FPS. Diese Werte duerfen nicht als Vergleich zum sauberen Lauf
verwendet werden. Er belegt jedoch die geaenderte Datenbewegung: Fuer dieselben
drei Anchorwechsel wurden vorher 1211512 Vertices und 75113744 Bytes
Vertex-/Indexdaten erneut hochgeladen, jetzt 61 neue Banken mit 180180 Vertices
und 11171160 Bytes. Das sind rund 85,1 % weniger Cloud-Uploaddaten ohne
Geometrievereinfachung. Von insgesamt 217 Bank-Veroeffentlichungen war die
groesste einzelne Bank nur 7364 Vertices beziehungsweise 412384 Vertexbytes
plus 44184 Indexbytes gross. Ein 25-MB-Vollhimmel-Reinsert tritt nicht mehr auf.
`clouds.publish` benoetigte innerhalb der 5019 Messframes insgesamt 3,019 ms;
sein Maximum einschliesslich Startphase war 0,059 ms. Der laengste gemessene
Bank-Compose-Worker dauerte 3,712 ms.

Die verbleibenden Worst-Frame-Fenster korrelieren dagegen mit grossen
Prop-Materialbatches. Der Vertex-Stride dieser Payloads ist 68 Bytes. Die
folgenden Asset-Transaktionen liegen jeweils am Ende des im identischen
Framefenster aufgezeichneten Allocator-Spans; alle Fenster haben unverkuerzte
Ringhistorie. Payloads sind Vertex- plus Indexdaten in dezimalen MB:

| Samplezeit s | Frame ms | Groesstes Prop-Asset (Index/Generation) | Vertices | Payload MB | Allocator-Body ms |
|---:|---:|---|---:|---:|---:|
| 23,162 | 38,302 | 14598 / 1 | 167608 | 12,403 | 10,111 |
| 25,487 | 36,254 | 8511 / 1 | 258720 | 19,145 | 15,958 |
| 42,477 | 37,153 | 15253 / 2 | 328152 | 24,283 | 19,636 |
| 54,491 | 34,592 | 16052 / 3 | 276160 | 20,436 | 16,653 |

Im 37,153-ms-Fenster wartet der Render-Handoff 20,589 ms; die grosse
Prop-Transaktion wird um 22:15:52,149 UTC protokolliert. Der benachbarte
Cloud-Commit folgt erst um 22:15:52,470 UTC und ist nicht dieselbe Transaktion.
Beim 38,302-ms-Fenster kommen Render-Spezialisierung mit 3,603 ms und Queue mit
4,213 ms als weitere CPU-Spans hinzu. Diese teilweise verschachtelten und
parallel laufenden Zeiten duerfen nicht addiert oder als GPU-Zeiten ausgegeben
werden. Der nachgewiesene verbleibende Uploadpfad motiviert begrenzte
Prop-Batch-Splits; deren Wirkung ist separat zu messen.

### Begrenzte Prop-Batches

Der Prop-Composer teilt grosse Ausgaben bereits im vorhandenen Worker an
vollstaendigen Prop-Grenzen auf. Stabile Stuecke enthalten hoechstens 65536
Vertices je bestehendem Static-/Rooted-Layout. Ausgaben unter dieser Grenze
behalten ihre bisherige Gruppierung und Reihenfolge. Die Teilmeshes werden
direkt mit passenden Kapazitaeten geschrieben, nicht nachtraeglich aus einem
fertigen Grossmesh kopiert. Attribute, Dreiecks-Winding, Materialzuordnung,
Geometrie und Vegetationsdichte bleiben erhalten. Geeignete Stuecke verwenden
weiter die vorhandene U16-Indexkompaktierung.

Das bestehende gemeinsame Prop-Budget bleibt bei 96000 zugelassenen Vertices
pro Frame. Es ist kein hartes GPU-Zeitbudget: Ein intrinsisch einzelnes
Template mit mehr als 65536 Vertices bleibt ungeteilt. Ebenfalls ungeteilt
bleibt ein letztes Rooted-Batch mit allen noch wartenden Wachstumsposen und
ihren alten/neuen Crossfade-Varianten. Beide sind ausdrueckliche Ausnahmen,
die ueber die bereits vorhandene Oversize-Zulassung allein fortschreiten
duerfen; sie koennen weiterhin das normale Vertexbudget ueberschreiten.

Wachstumsuhren starten nur fuer Prop-IDs, deren Geometrieranges im aktuell
zugelassenen Batch liegen. Vorher hochgeladene stabile Rooted-Stuecke starten
keine Animation eines spaeteren Stuecks. Noch unsichtbare wartende Posen
behalten ihren gespeicherten Wachstumsstand, bis sichtbare Geometrie beginnt.
Der bestehende Root wird weiterhin erst nach vollstaendigem Staging atomar
ersetzt. Revisionierung, Abbruch und Unloading entfernen saemtliche bereits
gestagten Teilassets.

Die Aufteilung erhoeht bei grossen Outputs die Anzahl der Mesh-Entities und
potenzieller Render-Bins/Draws. Props bleiben `NotShadowCaster`, daher erzeugt
die Aufteilung keine zusaetzlichen Schatten-Draws. Sie ist keine Reduktion der
Szenenqualitaet, aber auch kein kostenloser Renderpfad; der CPU- und
Entity-Trade-off muss im nativen Vergleich mitgemessen werden.

Acht neue Regressionstests decken kleine/unveraenderte Outputs,
Prop-Grenzen und Oversize-Ausnahmen, vollstaendige Attribute und Winding,
U16-/U32-Indizes, verzoegerten Crossfade-Start, verborgenes Wachstum sowie
Asset-Freigabe nach Abbruch und Unloading ab. Diese acht Tests bestanden im
fokussierten Render-Lauf. Bestehende Tests wurden an echte `Pose::begin`-
Aufrufe und mehrere begrenzte Stuecke statt eines einzelnen Grossmeshes
angepasst; der abschliessende Workspace-Testlauf bestaetigt diese Integration.

### Prop-Split Mit Cloud-2x2-Gruppen

Der anschliessende Stand teilt die Props und verwendet 46 statt 129
Cloudgruppen bei High, zunaechst noch ohne Frustum-Culling der Gruppen.
Die folgenden Messungen verwenden wieder die unveraenderten alten Szenen,
ohne parallelen Compiler oder zusaetzliche Scope-Instrumentierung:

| Szene / Stand | AVG FPS | Min FPS | p95 ms | p99 ms | Max Frame ms | Frames unter 32 FPS |
|---|---:|---:|---:|---:|---:|---:|
| Integrated, urspruenglicher Stand, frischer Kontrolllauf | 80,34 | 27,69 | 19,97 | 25,79 | 36,11 | 15 / 4820 |
| Integrated, Propsplit/Cloud-2x2 | 83,67 | 30,41 | 19,13 | 23,48 | 32,89 | 1 / 5019 |
| Wasser, vor Cloud-Reuse, frischer Kontrolllauf | 77,38 | 25,42 | 21,90 | 24,59 | 39,33 | 3 / 1847 |
| Wasser, Propsplit/Cloud-2x2 | 74,54 | 29,06 | 22,54 | 24,20 | 34,42 | 2 / 1779 |

Rohdaten: `teramic139-before-final.log`, `teramic139-final1.log`,
`teramic139-water-control-final.log`, `teramic139-final-water1.log`.
Der zeitnahe Integrated-Vergleich zeigt bessere Lows und weniger langsame
Frames, aber noch keine feste 32-FPS-Untergrenze. Der Wasserpfad hat bessere
Min-FPS, jedoch etwa 3,7 % weniger AVG FPS. Die Cloud-Gruppierung allein ist
deshalb keine allgemeine Performance-Abnahme.

Der getrennte Scope-Lauf `teramic139-final-profile.log` liefert 84,61 AVG /
31,78 Min FPS und einen Frame unter 32 FPS. Die Diagnose-FPS sind weiterhin
nicht mit den sauberen Laeufen gleichzusetzen. Im schlechtesten 31,469-ms-
Fenster benoetigt der Allocator-Body 4,156 ms statt der zuvor gemessenen
16-20-ms-Ausreisser. Render-Spezialisierung und Queue belegen dort weiterhin
ungefaehr 4,1 beziehungsweise 4,3 ms; die Spans sind nicht addierbar und kein
GPU-Zeitnachweis. Die derzeitige Untersuchung konzentriert sich deshalb
zusaetzlich auf unnoetig eingereichte Cloudgruppen ausserhalb des Sichtfelds.

Ein zweiter Wasserlauf desselben Zwischenstands mit passendem, gesichertem
Uniform-Wassershader (`teramic139-final-water2.log`) liefert 75,62 AVG /
27,17 Min FPS, p95 21,90 ms, p99 24,39 ms und 5 Frames unter 32 FPS.
Auch dieser Lauf bestaetigt keine Mindest-FPS-Garantie oder allgemeine
AVG-Verbesserung durch Gruppierung allein.

Die neue Landschaft wird zusaetzlich getrennt mit Seed 42042, Revision 3,
Schnee, 1920x1080 und Sichtweite 20 fuer 60 s geflogen
(`teramic139-rev3-snow.log`, Konfiguration `connected-config/`). Der
Zwischenstand ohne Cloud-Culling/Water-Storage erreicht 65,24 AVG /
23,85 Min FPS, p95 25,82 ms, p99 30,08 ms und 22 Frames unter 32 FPS.
Er laedt 15442 Sections mit 101864 Props und 32 Wassertiles. Diese andere
Szene ist kein Vergleich mit dem alten Revision-0-Regenflug; sie belegt
ausdruecklich noch verbleibende Lastspitzen in der neuen Landschaft.

### Wolken-Culling Und Dynamische Wasserdaten

Die festen Cloudgruppen erhalten im vorhandenen Worker einmalig konservative
AABBs. Diese schliessen die Bank-Urspruenge, Sichtbarkeit von 0 bis 1 und die
maximale Shaderausdehnung (XZ 2,20 / Y 1,18) einschliesslich Rundungspuffer ein.
`NoAutoAabb` verhindert ein spaeteres Ueberschreiben durch rohe Meshbounds.
Damit verwendet der unveraenderte Bevy-Renderer sein CPU-/GPU-Frustum-Culling
statt bisher alle 46 Gruppen einzureichen. Beide Stock-Pfade testen nur die
ersten fuenf Frustum-Planes, nicht die entfernte Terrain-Far-Plane. Echte
Horizontwolken bleiben deshalb zulaessig. Unbekannte Geometrie und die getrennten
Sonnenstrahlen behalten den sicheren ungecullten Pfad.

Im Wasserpfad aenderten Wind, Solarzugang und Schwimmspuren bisher das
gemeinsame `WaterMaterial`-Asset. Stock Bevy markiert dadurch alle dieses
Material verwendenden Wasser-Entities erneut fuer Spezialisierung und Queue;
die Wasserszene enthaelt 1681 solche Tiles. Dynamische Werte liegen nun in
einem einzigen stabilen 544-Byte-`ShaderBuffer`. Der vorhandene RenderQueue-Pfad
schreibt ihn nach dessen Asset-Aufbereitung nur bei geaenderten Daten oder
neuer Buffer-ID. Wasserbewegung, Reflexions-/Wellen-Gleichungen und
Wake-Kapazitaet bleiben gleich. Echte statische Qualitaets- oder
Native-Depth-Key-Aenderungen aktualisieren weiterhin das Material.

Acht weitere Renderregressionen pruefen Bounds und deren Publikation,
unveraenderte automatische Bounds, dynamische Updates ohne Material-/Buffer-
Asset-Modified-Events, reale statische Aenderungen sowie das exakte Rust-/WGSL-
Layout. Die native Shader-/Bildkontrolle dieses Schritts ist bestanden:
Wasser, Wolken und Horizont werden mit dem Stock-Renderer korrekt dargestellt,
ohne Shader-/Binding-Fehler oder Warnungen in den geprueften Laeufen.

### Abschliessende Native Messungen

Die folgende Tabelle enthaelt ausschliesslich saubere Laeufe ohne
Scope-/Ressourcenprofiling. Dieselben Szenen und Konfigurationen bleiben
innerhalb jeder Zeile vergleichbar; verschiedene Seeds/Wetter/Revisionen
werden nicht zu einem einzigen FPS-Wert vermischt.

| Szene / Stand | AVG FPS | Min FPS | p95 ms | p99 ms | Max Frame ms | Frames unter 32 FPS |
|---|---:|---:|---:|---:|---:|---:|
| Regenflug, alter Stand, zeitnaher Kontrolllauf | 80,34 | 27,69 | 19,97 | 25,79 | 36,11 | 15 / 4820 |
| Regenflug, alle Fixes | 86,28 | 32,02 | 18,30 | 22,45 | 31,23 | 0 / 5176 |
| Regenflug, alle Fixes, Wiederholung | 86,50 | 13,62 | 18,28 | 22,57 | 73,40 | 1 / 5190 |
| Wasser, vor Cloud-Reuse, zeitnaher Kontrolllauf | 77,38 | 25,42 | 21,90 | 24,59 | 39,33 | 3 / 1847 |
| Wasser, alle Fixes | 81,39 | 31,17 | 21,06 | 22,97 | 32,09 | 2 / 1944 |
| Wasser, alle Fixes, Wiederholung | 80,09 | 24,57 | 21,59 | 23,35 | 40,71 | 3 / 1912 |
| Revision 3 / Seed 42042 / Schnee, vor letztem Schritt | 65,24 | 23,85 | 25,82 | 30,08 | 41,93 | 22 / 3915 |
| Revision 3 / Seed 42042 / Schnee, alle Fixes | 67,62 | 27,25 | 24,53 | 28,51 | 36,70 | 10 / 4057 |

Rohlogs: `teramic139-optimized{1,2}.log`,
`teramic139-optimized-water{1,2}.log`, `teramic139-optimized-snow.log`;
die Kontrolllogs sind oben genannt. Wasser-Control-Binaries wurden mit einem
gesicherten alten Uniform-Wassershader ueber `TERRA_ASSET_DIR` gestartet,
nachdem die aktuelle Quelle auf Storage umgestellt war. Alle Shadergleichungen
blieben gleich; nur die zur jeweiligen Rust-Bindung passende Deklaration
unterscheidet sich.

AVG und p99 verbessern sich gegen die zeitnahen Kontrollen. Die Min-FPS sind
jedoch **nicht durchgaengig behoben**. Der 73,40-ms-Regenausreisser bei
Samplezeit 25,235 s bleibt vollstaendig gezaehlt; davor wurden 45488 Vertices
zugelassen. Dieser Kontext beweist keine Ursache. Alle anderen Frames dieses
Wiederholungslaufs bleiben unter 31,25 ms, der zweitschlechteste bei 30,28 ms.
Weder dieser einzelne Ausreisser noch der schlechtere Wasser-Wiederholungswert
werden herausgefiltert. Die bisherigen Kontrolllaeufe mit teils 88,94 AVG FPS
bleiben ebenfalls im Bericht und relativieren universelle Gewinnbehauptungen.

Ein weiterer, strikt getrennter Regen-Diagnoselauf
(`teramic139-optimized-profile.log`) mit Scope- und Ressourcenspur erreicht
85,98 AVG / 35,92 Min FPS und keinen Frame unter 32 FPS. Er reproduziert den
73-ms-Ausreisser nicht und ersetzt deshalb den sauberen Wiederholungslauf
nicht. RSS liegt in seinen 30-/60-/90-s-Proben bei
2212904 / 2281752 / 2308272 KiB, Prozess-Swap bei null,
gemessener Peak-DRM-VRAM bei 4123656 KiB und GTT bei
67588 KiB. Die Major-Fault-Proben zaehlen 114 / 116 / 116; sie erlauben keine
Zuordnung zum Ausreisser eines anderen Laufs. Diese kurzen Werte sind keine
Langzeit-Leak-Abnahme.

Die getrennte Schnee-Timeline (`teramic139-optimized-snow-profile.log`)
erreicht 65,75 AVG / 27,53 Min FPS. Alle zwoelf Worst-Frame-Fenster enthalten
vollstaendige Ringhistorie. In drei Fenstern mit 64-93k zugelassenen Vertices
belegt der Allocator-Body noch 4,59-6,41 ms. Fruehe Fenster ohne Uploads zeigen
88-96 aktive Chunkanimationen, Render-Ausfuehrung von 4,69-5,78 ms und
Preprocess-Bind-Group-Vorbereitung von 3,52-4,25 ms. Main-/Render-Wallintervalle
ueberlappen; keine Addition oder Behauptung einer einzelnen GPU-/UI-Ursache.

Der vermutete Minimap-Vollindex-Fallback ist in diesem Lauf ausdruecklich
**nicht** nachgewiesen: `mapping.index` wird nur einmal beim Start aufgerufen,
nie im Messfenster. Maxima fuer inkrementellen Index / Snapshot / Schedule
liegen bei 1,353 / 0,054 / 0,651 ms. Es gibt deshalb keine weitere spekulative
Minimap-Codeaenderung. Restarbeit sind die reproduzierbaren Schnee-Spitzen
sowie das bisher nur ohne Instrumentierung erfasste seltene 73-ms-Fenster.

## Validierung

Der Stand mit Frame-Timeline, Cloud-2x2-Gruppen und Prop-Splits hat
folgende Pruefungen bestanden:

- `cargo fmt --all --check`
- `cargo check --locked --workspace --all-targets -j2`
- `cargo clippy --locked --workspace --all-targets -j2 -- -D warnings`
- `cargo test --locked --workspace -j2 -- --test-threads=2`:
  **2636 bestanden, 0 fehlgeschlagen, 7 ignoriert**.

Die Logauswertung bestaetigt diese Summe ueber alle Workspace-Testbinaries.
Rohlogs: `/tmp/teramic139-final-workspace-tests2.log`,
`/tmp/teramic139-final-fmt3.log`, `/tmp/teramic139-final-check.log` und
`/tmp/teramic139-final-clippy.log`.
`cargo build --locked --workspace -j2` ist fuer diesen Stand ebenfalls
bestanden (`teramic139-final-build.log`), einschliesslich Client und Server.

Nach dem abschliessenden Cloud-Culling-/Water-Storage-Schritt sind erneut
Formatierung, All-Targets-Check und striktes Clippy bestanden. Der gezielte
Render-Library-Lauf besteht **691 Tests, 0 fehlgeschlagen, 1 ignoriert**;
er enthaelt die acht zusaetzlichen Bounds-/Storage-Regressionsfaelle.
Rohlogs: `teramic139-culling-water-{fmt,check,clippy,tests}.log`.
Der volle 2636-Testlauf fand vor diesen letzten renderinternen Aenderungen
statt; er wird nicht faelschlich als erneuter gesamter Workspace-Lauf
ausgegeben. Der neue Workspace-Build ist ebenfalls bestanden
(`teramic139-culling-water-build.log`); die nativen Shader-/Bildlaeufe und
deren nicht vollstaendige FPS-Abnahme stehen oben.

Ein frueherer Stand vor der Frame-Timeline hatte separat bereits **2609**
bestandene und 7 ignorierte Tests sowie einen erfolgreichen Workspace-Build.
Diese frueheren Ergebnisse sind nicht der 2636-Testlauf. Client und Dedicated
Server sind
Bestandteil der Workspace-Pruefungen. Der native Headless-Smoke-Test
laedt die neue Revision 3 ohne Display/GPU mit 27 Sections erfolgreich.

Native Vulkan-Ansichten aus echten generierten Chunks, ohne Fixture-Edits:

- See, Seed 42042, `[3296,-3296]`: Uebersicht und Nahansicht bestanden.
- Fluss, Seed 42042, `[451.074,439.593]`: Uebersicht und Nahansicht bestanden.
- Ebene, Seed 42042, automatisch gewaehlter Anker `[-2815.5,0.5]`:
  Uebersicht und Nahansicht bestanden.

Die sechs PNGs liegen unter `captures/`. Die Ansichten wurden tatsaechlich
geoeffnet und geprueft. See und Fluss zeigen zusammenhaengendes Wasser ohne
sichtbare Section-Luecken; die Kataraktkanten bleiben teilweise regelmaessig.
Der erste Flusslauf erreichte nach dem Kamerwechsel die vollstaendige Readiness
nicht innerhalb von 150 s. Der Wiederholungslauf mit Ladefortschrittsdiagnose
war nach 31 s fertig, ohne Worker-Fehler oder Codeaenderung am Generator.
Die erste Ueberschreitung ist damit nicht als behoben nachgewiesen und bleibt
zusammen mit den Streaming-Ausreissern dokumentiert.

Keine neuen 10-/15-/30-Minuten-Laeufe. Eine feste 32-FPS-Untergrenze ist
weiterhin nicht belegt; das Performance-Problem ist nicht abgeschlossen.
