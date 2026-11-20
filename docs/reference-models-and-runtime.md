# Referenzmodelle, Sandwurm und Laufzeitlast

Die Grundblöcke verwenden eine gemeinsame, reproduzierbar erzeugte Palette:
Stein, Sand, Kies und Bruchstein sowie Eiche, Birke, Fichte, Blut-, Geister-
und Kirschholz. Gemeinsam genutzte Farb- und lineare Normaltexturen erhalten
unterschiedliche Rinde, Stirnholzringe und Plankenfarben. Normalkarten werden
bei der Mipmap-Erzeugung als Vektoren normalisiert. Die vorhandene Grasgeometrie
bleibt an Terrain und LOD angeschlossen.

Tote Büsche erhalten gekrümmte, sich verjüngende Holzäste mit verbundenem
Wurzelansatz. Die Blütenbüsche 317 und 426 verwenden offene Zweige, Blätter und
Blüten statt großer grüner Blöcke. Dornenwächter haben eine Rinden-Silhouette,
helle Dornen, rote Blätter und einen separat leuchtenden Kern. Skelettschädel,
Rippen und die drei vorhandenen Ausrüstungen sind überarbeitet. Schmetterlinge
werden einschließlich Zielgeometrie auf die Hälfte ihrer früheren Größe skaliert.

`/spawn teramic::sand_worm` setzt den Wurm sichtbar und dem aufrufenden Spieler
zugewandt in die Welt. Der anfängliche sichtbare Zustand dauert 4,2 Sekunden.
Das natürliche Spawnen akzeptiert auch flache Sandauflagen auf natürlichem
Wüstenfels und abgestufte Dünen. Unter der Erde sind Körper, Maul und
Trefferflächen vollständig verborgen; nur die auf den Sand projizierten Wellen
zeigen seine Bewegung. Die Kartenmarkierung folgt weiterhin seinem Standort.
Sobald er auftaucht, verwenden Maul, Trefferflächen und Lebensanzeige dieselbe
Wirbelsäule. Das Maul besitzt zwei Zahnreihen.

Bestätigter Spielerschaden löst eine kurze Kamerareaktion aus, auch bei Kakteen.
Die logische Blickrichtung und die serverseitige Zielrichtung bleiben erhalten.
Die vorhandene rote Schadensanzeige ergänzt diese Bewegung.

Neue Konfigurationen starten mit acht Chunks Sichtweite und acht parallelen
Meshaufträgen. Für das aktive Fenster ist kein FPS-Limit voreingestellt.
Gespeicherte Einstellungen wie eine Sichtweite von 32 Chunks werden übernommen.
Im normalen Client begrenzt ein CPU-abhängiges Workerbudget die Hintergrundlast;
bei einem nicht fokussierten Fenster wird die Bildrate auf maximal 30 reduziert.
Der Grafiktreiber erhält die speichersparende WGPU-Allokationsstrategie. Die
Sichtweite und die FPS-Obergrenze bleiben in den Einstellungen veränderbar.

`tools/benchmarks/run-reference-palette.sh` zeigt Materialien und Vegetation.
`tools/benchmarks/run-voxel-creatures.sh forest spirit sandworm` zeigt die Modelle.
`tools/benchmarks/run-idle-soak.sh` misst drei Minuten Stillstand in einem echten
Wald mit integriertem Server bei 1920×1080 und protokolliert Speicher sowie
Assetanzahlen. Die letzten 60 Sekunden werden auf fortgesetztes Wachstum geprüft.
Alle drei Werkzeuge verwenden eigene Konfigurationen und Testwelten unter
`target/`. Eine kurze Messung kann beliebig lange Spielsitzungen nicht garantieren.

Die vorherige Überarbeitung wurde mit 280 bestandenen Rust-Tests, zwölf Assettests
und 20 nativen Aufnahmen geprüft. Der damalige Stillstandstest lief drei Minuten im Wald
mit integriertem Server; in seiner letzten Minute stieg der residente Speicher
um etwa 1,07 MiB, ohne private Auslagerung oder wachsende Assetanzahlen. Der
native Benchmark umgeht den interaktiven Frontend-FPS-Limiter und misst daher
ungekappte Bildraten. Diese Messung begrenzt Aussagen über längere Sitzungen und ersetzt keine
Messung der aktuellen Überarbeitung.

Gesamtnachweis: `target/reference-creature-rework/verification.json`.
Finale Bilder: `target/reference-palette/run.UPnUgz/captures/` und
`target/voxel-creatures/run.YYItLS/`.


## Bewegung, Sichtweite und Atmosphäre (19. November, zweite Überarbeitung)

Der voreingestellte Vordergrund-FPS-Limiter ist aufgehoben (`fps_limit = 0`).
VSync und ein selbst gewähltes FPS-Limit bleiben einstellbar; inaktive Fenster
werden weiterhin auf 30 FPS begrenzt. Die vorhandenen Worker- und Speicherbudgets
bleiben wirksam. Das verspricht keine bestimmte Bildrate auf einer gegebenen GPU.

Eine Änderung der Sichtweite aktualisiert jetzt den tatsächlichen Streamingradius,
nicht nur Kamera, Nebel und Netzwerkmetadaten. 32 Chunks entsprechen 512 Blöcken
Radius. Verkleinern entfernt auch ohne Spielerbewegung die entfernten Interessen.
Der Server behält seinen eigenen Simulationsradius.

Der Sandwurm springt vollständig aus dem Boden, landet vor seinem Biss und taucht
mit dem Kopf voran wieder ab. Darstellung, anatomische Trefferbereiche und Lebensanzeige
verwenden dieselbe Phasenpose. Drei wandernde Wellen bestehen aus 24 gecachten
Sektoren; sie folgen geladenem Sand und verschwinden an Stufen, Wasser oder
unbekanntem Boden. Die Animation erzeugt keine laufenden neuen Meshes.

Glooms erhalten eine Blütenhülle, ein ruhiges dunkles Gesicht, kleine ovale Augen,
geäderte Flügel, Kristallfühler und einen gekräuselten Leuchtschweif. Schweben,
Flattern, Neigung und Blinzeln bewegen sieben gemeinsame Modellteile. Tieraugen
haben weniger Weiß; gegnerische Augen leuchten schwächer. Waldgegner laufen bei
der Verfolgung schneller, bremsen vor dem Nahkampf und zeigen Ausholen, einen
Treffer und Erholung. Ausweichen oder eine unterbrochene Sichtlinie bricht den
Angriff ab; erneutes Annähern startet ein neues Ausholen.

Sand ist feiner und heller beige. Kaktusstacheln variieren reproduzierbar mit der
Weltposition. Normales, Geisterwald- und Blutwaldgras verwenden ein gemeinsames
räumliches Farbfeld auf Oberseiten und Graskappen. Texturdetails und Erde bleiben
lesbar; Karte und Jahreszeiten folgen derselben Farbgebung.

Sowohl Mystic Mist Block (399) als auch Fog Block (430) werfen in sämtlichen
Detailstufen keine Schatten. Die Höhlenerkennung interpoliert räumlich über den
Eingang. Die Augenanpassung ist zeitlich geglättet und reagiert beim Verlassen
schneller. Geisterwaldnebel reicht dezent in Eingänge; tiefe Höhlen und der Himmel
bekommen diese zusätzliche Nebelschicht nicht.


Prüfung: Workspace-Build, Clippy mit `-D warnings` und Formatprüfung bestehen.
396 verschiedene Rust-Tests und fünf Assettests bestehen.
29 native Aufnahmen prüfen Modelle, Sprung-/Tauchposen, Sand und Farbpalette,
reale Biomfarbfelder, Nebel sowie Höhlenbeleuchtung. Der Spawn-Befehl, zwei
UDP-Clients, Treffer und Ausweichen sind zusätzlich mit dem echten Server geprüft.
Nachweis: `target/nov19-world-polish/verification.json`.

Die optionale Aufnahme natürlich generierter Kakteen erreichte ihre globale
drei Sekunden lange Ruhebedingung nicht. Für diese Szene wird keine Bildfreigabe
behauptet; die direkten Tests prüfen unterschiedliche Stacheln, Außenrichtung,
Wiederholbarkeit und LOD-Konsistenz. Die Wellenaufnahmen zeigen eine ebene
Sandfläche; Stufen, Wasser, Änderungen und unbekannter Boden sind separat getestet.


## Erweiterung: Bewegung, Galle und sieben Glooms

Der Sandwurm besitzt 24 kräftige Körperringe, einen gewölbten Vorderkörper und
ein geschlossenes, sich verjüngendes Ende. Die Angabe „500 % länger“ wird als
fünffache bisherige Längsausdehnung umgesetzt: ungefähr 23 statt 4,6 Blöcke.
Der zusätzliche Körper liegt hinter dem bisherigen Bogen; die Höhe wird nicht
verfünffacht. Eine gemeinsame Wirbelsäule steuert bis zu 25 anatomische
Trefferflächen. Die Ringhaut bewegt sich mit einer kleinen, längs wandernden
Kontraktion. Der lange Hinterkörper bleibt beim Biss am Boden.
Beim Auftauchen steigen Kopf und Vorderkörper zuerst auf. Hintere Ringe folgen
zeitlich versetzt aus dem Sand; ihre Neigung folgt dem Verlauf der benachbarten
Ringe. Die Bewegung hebt den langen Körper nicht als starren Hebel an.

Er wandert auch in der Nähe von Kreativspielern und hinterlässt eine begrenzte
Spur auf bestätigtem Sand. Drei Maulfächer schließen sich beim Biss. Für Galle
gibt es einen eigenen replizierten Angriffszustand mit festgehaltenem Ziel;
ein bloßer sichtbarer Ruhezyklus löst keinen Schuss aus. Die feste Spur nutzt
maximal 64 Instanzen, fliegende Galle maximal 32 Serverprojektile. Effekte
erzeugen keine fortlaufend neuen Meshes oder Materialien und werfen keine Schatten.

Die Glooms besitzen sieben Varianten: Standard, Schatten, Blut, Licht, Wald,
Eis und Sakura. Die beiden bisherigen gespeicherten Varianten behalten ihre
Nummern. Licht-Glooms leben auch in geladenen Höhlen; Schatten-Glooms sind dort
selten und tief. Echte Gloom-Essenz und seltene Lichtsplitter gelangen über den
normalen Drop-, Inventar- und Speicherpfad ins Spiel. Eine gemeinsame Effektcharge
zeigt Leuchtspuren, Blütenpartikel und Reaktionen für höchstens 24 nahe Glooms
mit maximal 384 Partikelkarten. Sie wird mit 20 Hz aktualisiert.

## Einstieg und Materialdetails

Das Frontend begrenzt das anfängliche Streaming auf 15 Chunks Radius, also
höchstens 961 Spalten. Diese Grenze gilt vor der ersten Streamingrunde und für
Handshake sowie spätere Sichtweitenpakete. Nach dem Einstieg wächst die Sicht
ringweise, sobald die vorherige Runde bereit ist. Die gespeicherte Sichtweite
bleibt erhalten; Kamerareichweite und Nebel folgen der aktuell geladenen Grenze.
Schatten reichen je nach Qualität höchstens 96/192/256 Blöcke weit. Kurze
Sichtweiten benötigen weniger Schattenkaskaden; nahe Schatten behalten ihre
Auflösung. Die Bildrate des aktiven Fensters bleibt unbegrenzt, wenn das
FPS-Limit auf 0 steht.

Benachbarte Grasoberseiten bilden eine ebene Fläche. Die Rundung des importierten
Modells bleibt an freien Außenkanten erhalten; das wiederholte Raster aus
abfallenden Innenkanten entfällt. Holz, Gestein, Textilien und weitere Baublöcke
verwenden zusätzlich kleine, weltfest verankerte Materialdetails. Entfernte
Pixel überspringen deren Rauschberechnung; die vorhandenen PBR-Texturen bleiben
erhalten. Diese Erweiterung fügt keine Terrainmeshes hinzu.

Die Straßengalerie nutzt `TERRA_STREET_INSPECT=1 tools/benchmarks/run-reference-palette.sh`.
Die Teergalerie prüft auch echte Galleausbreitung und unterschiedliche Pegel.


Die Erweiterung wurde mit 345 gezielten Rust-Tests, zwölf Assetprüfungen und
19 nativen Aufnahmen geprüft. Der dreiminütige Waldtest mit integriertem Server
bei 1080p und acht Chunks misst ungekappt rund 104 durchschnittliche und 173
maximale CPU-Wall-Frame-FPS. In der letzten Minute kommen 504 KiB RSS hinzu;
Mesh-, Material- und Bildanzahlen bleiben konstant. Die Messung beschreibt diese
Szene; andere Sichtweiten, Auflösungen und längere Sitzungen benötigen eigene
Messungen. Nachweis: `target/nov19-bile-street/verification.json`.

## Aktuelle Sandwurm-Regeln: Zielwahl, Galle und Untergrund

Gallespucken benötigt beim Abwurf einen sichtbaren, verwundbaren Gegner in
Reichweite. Kreativspieler, fehlende Gegner und durch Gelände verdeckte Ziele
lösen keinen Schuss aus. Nahe Ziele werden gebissen. Der Server richtet den
Schuss auf die festgehaltene Zielzelle und prüft die echte ballistische Flugbahn
gegen geladenes Gelände; eine freie Sichtlinie allein genügt bei einer niedrigen
Decke nicht. Darstellung und Serverprojektil verwenden dieselben Abwurfdaten.
Ein Treffer auf den Wurm unterbricht die bereits gestartete Flugdarstellung nicht.

Ein Galletreffer erzeugt eine aufnehmbare Quelle, die nach fünf Sekunden
Simulationszeit verschwindet. Ihre Restzeit läuft unabhängig vom budgetierten
Fluss weiter, auch wenn die Quelle außerhalb der geladenen Abschnitte liegt.
Dafür werden keine Chunks nachgeladen. Innerhalb dieser Zeit kann ein leerer
Eimer die Quelle aufnehmen; ein anschließend gesetzter Galleeimer erzeugt eine
gewöhnliche dauerhafte Quelle. Die zeitliche Begrenzung betrifft die gespuckte
Quelle, nicht einen bereits weitergeflossenen Rest.

Während der eingegrabenen Bewegung und der Vorwarnung bleiben sämtliche
anatomischen Modellteile sowie ihre Trefferflächen verborgen. Aufgeworfene
Wellen und die Spur werden nur auf bestätigtem, trockenem Sand angezeigt.
Die Bodenprojektion blendet einzelne Wellensektoren an Stufen oder unbekannten
Flächen aus. Die serverseitige Bewegung verwendet mehrere kurze Proben, prüft
Sandauflage und Untergrund und stoppt vor fehlender Deckung oder einem Abhang
von mehr als einer Blockhöhe. Beim Abtauchen verschwindet jeder vollständig
unter die Oberfläche gesunkene Körperring samt zugehöriger Trefferfläche.

Das Modell verwendet 58 gemeinsame Teile und ein festes Budget von höchstens
24.000 Vertices. Die native Sandwurmgalerie erwartet sechs Aufnahmen:
eingegraben, Vorwarnung, Sprung, Biss, gezielter Galleschuss und Abtauchen.
Ihr Bericht verlangt 24 Körperringe, die fünffache Längsausdehnung und vollständig
verdeckte eingegrabene Anatomie. Sie prüft die reale Modelldarstellung;
Zielwahl, Sichtprüfung, Bodenproben und die Lebensdauer der Quelle benötigen
zusätzlich die gezielten Logiktests. Die oben genannten historischen Prüfzahlen
beziehen sich auf frühere Überarbeitungen und belegen diese letzte Änderung nicht.
