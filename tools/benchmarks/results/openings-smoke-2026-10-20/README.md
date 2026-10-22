# Phase 88: Rauch, adaptive Schornsteine, Tueren/Fenster und Ofen-UI

Stand: 2026-10-20. Dieser Bericht beschreibt den implementierten Quellstand.
Bereits abgeschlossene Pruefungen sind unten mit ihren Rohlogs aufgefuehrt.
Finaler Gameplaylauf, Headless-Start und alle geforderten Workspace-Pruefungen
sind bestanden; konkrete Nachweise und verbleibende Grenzen stehen unten.

## Umsetzung der Anforderungen

| Anforderung | Implementierung |
| --- | --- |
| Rauch als zunehmend dichter Nebel statt sichtbarer Bloecke | Weiche, animierte WGSL-Rauchfahnen und tiefenabhaengige Sichtabschwaechung im bestehenden Cave-/Unterwasser-Pass. Die serverseitigen Rauchzellen bleiben logischer Zustand und sind keine gerenderten Wuerfel. |
| Rauch aus dem wirklichen Schornsteinauslass | Simulation und Darstellung verwenden dieselbe Portverfolgung. Der Auslass liefert Position und Austrittsrichtung, auch an seitlichen und nach unten gerichteten Enden. Die Fahne beginnt dort und steigt danach auf. |
| Ein adaptives Schornstein-Item | Block 60 deckt drei gerade Achsen und zwoelf rechtwinklige Portverbindungen ab, letztere mit runder oder schraeg geknickter Form. Platzierung, Blickneigung und Verbindungen waehlen die Pose; alte Bloecke 61 bis 64 bleiben ladbar, sind aber nicht mehr separat im TMI-Katalog oder in Crafting-Rezepten verfuegbar. |
| Animierte Holztueren und Fenster | Je eine Eichen-, Birken- und Fichtenvariante. Tueren sind zweiblockhoch, Fenster einblockhoch. Die Blaetter oeffnen und schliessen mit einer 0,45-s-Hinge-Animation. Offene Aussenverbindungen entlueften den Raum. |
| Ofen und Rohrmodell an Referenzen annaehern | Neues Stein-/Metallmodell mit differenziertem Mauerwerk, Feuerraum, Beschlaegen und offenen Rohrenden. Referenzinspiriert, keine behauptete 1:1-Reproduktion der Bilder. |
| Rezepte mit R in Creative | R auf einem ueberfahrenen TMI-Item oeffnet den bestehenden Rezeptinspektor, einschliesslich Schmelzrezepten. Kein CreativeSet und keine automatische Itemvergabe. Texteingaben und Nicht-Katalogslots werden ignoriert. |
| Ofen-UI ohne Scrollen | Groesseres Panel, zusammenhaengende Material-/Brennstoffspalte, Bellows und Ergebnis daneben; keine innere Scrollflaeche. Kompakte Hoehenregeln und dezente TiltUI-Sloticons. |
| Groessere TMI-Kategorien | Kategorie-Schaltflaechen 40 auf 80 logische Pixel, Vorschaubilder 30 auf 60 Pixel. Sechs mitgelieferte Kategorien: Building, Decoration, Materials, Animal Materials, Food, Tools. |
| Fenster und Tueren nach Bildrichtung | Separate Holzfarben, Rahmen, Beschlaege, Glasscheiben und bewegliche Blaetter. Kollisions-/Picking-Geometrie verwendet denselben geoeffneten oder geschlossenen Zustand. |

## Rauch: Darstellung und Autoritaet

Relevante Implementierung:

- `crates/terra-world/src/furnaces.rs`: bestehende begrenzte Rauchausbreitung und
  Portverfolgung, erweitert um echte Aussenverbindungen und begrenzte
  Tueren-/Fensterentlueftung.
- `crates/terra-network/src/server_furnaces.rs`: bestehender autoritativer
  Ofentakt, Zustandssynchronisation und Rauchschaden.
- `crates/terra-render/src/furnaces.rs` und
  `assets/shaders/terra_furnace_smoke.wgsl`: gemeinsame weiche Rauchgeometrie mit
  animiertem Rauschsignal, ohne sichtbare Zellflaechen.
- `assets/shaders/terra_caves.wgsl`: zunehmende lokale Sichtabschwaechung im
  bestehenden Tiefenpass; kein separater dauerhafter Vollbildpass nur fuer Rauch.

Die Raumlogik bleibt zellbasiert, damit Laden, Speichern, Schaden und
Entlueftungsraten deterministisch und serverseitig bleiben. Die Renderdarstellung
ist davon getrennt. Der lokale Nebelfaktor wird aus erreichbaren Rauchzellen in
Kameranaehe berechnet und zeitlich geglaettet. Feste Waende trennen diese lokale
Probe; die Kamerahelligkeit wird nicht allein durch geometrisch nahe Zellen im
Nebenraum bestimmt.
Ohne lokale Rauchzellen endet die Dichteprobe vor der ersten Terrainabfrage.
Ist auch die geglaettete Restdichte null, bleibt der Cave-Uniform unveraendert.
Der Ruhepfad erzeugt damit keine Raum-BFS oder Terrainproben fuer leere Luft;
die begrenzte Suche nach vorhandenen nahen Rauchdaten bleibt bei 10 Hz.
Bei aktivem Nebel schaetzen vier feste Scene-Color-Abtastungen das Raumlicht;
die Streufarbe folgt dadurch nicht mehr der Helligkeit jedes einzelnen
dahinterliegenden Blocks. Diese vier Shader-Abtastungen werden nur im aktiven
Rauchzweig ausgefuehrt. Der vorhandene Tiefenpass wird wiederverwendet, der
Effekt ist deshalb aber nicht kostenlos. Es entsteht kein zusaetzlicher
Vollbildpass; eine gemessene GPU-Kostenparitaet liegt nicht vor.

Ein offener Zugang zum Himmel laesst den Rauch mit 2 Zellen/s verschwinden.
Geschlossene Raeume erhalten waehrend des Brennens 1 Zelle/s. Ein offenes Fenster
mit tatsaechlicher Aussenanbindung zieht 0,25 Zellen/s ab, eine offene Tuer
0,75 Zellen/s je Tuerzelle. Eine vollstaendige zweiblockhohe Tuer kann damit
1,5 Zellen/s abfuehren. Bei laufendem Ofen zaehlt die Nettorate:

- Ein Fenster: 1 - 0,25 = +0,75 Zellen/s.
- Eine zweiblockhohe Aussentuer: 1 - 1,5 = -0,5 Zellen/s.
- Eine geoeffnete Wand zum Freien: Abbau mit 2 Zellen/s.

Fenster-/Tuerleistung ist ein gemeinsames Budget des verbundenen Raums, nicht
ein eigener vollstaendiger Abzug je Ofen. Die vorhandenen aktiven oder noch
rauchhaltigen Quellen des Raums teilen sich diese Leistung. Zwei brennende
Oefen an einem Fenster erzeugen zusammen daher 2 - 0,25 = 1,75 Zellen/s,
nicht zweimal 0,25 Zellen/s Abzug. Restmengen behalten ihren anteiligen
Entlueftungsbruchteil auch beim Fortsetzen.

Jede Rauchzelle besitzt genau eine Quelle als Besitzer. Neue Ausbreitung
ueberspringt die bereits von anderen Oefen belegten Zellen. Fuer ueberlappende
historische Quellenlisten gewinnt deterministisch die kleinere Ofenkoordinate;
dadurch entstehen keine doppelten Raumzellen durch Lade- oder Verarbeitungsreihenfolge.
Der gespeicherte Zustand bleibt trotzdem beim jeweiligen Ofen, ohne neue
konkurrierende Raum-Registry.

Ein offenes Fenster in einen ebenfalls geschlossenen Nebenraum gilt nicht als
Aussenentlueftung. Der Bruchteil der Zellenbilanz bleibt gespeichert, damit
Entlueftung nicht durch kleine Zeitschritte oder Reloads verloren geht. Nach
Erloeschen eines unbeluefteten Ofens dissipiert verbliebener Rauch weiterhin;
die Simulation ist kein geschlossenes physikalisches Gasmodell.

### Feste Budgets

| Bereich | Grenze |
| --- | ---: |
| Verbundene Rohrstuecke pro Route | 64 |
| Zellen pro Raum-BFS | 4.096 |
| Gesamtbudget einer Raum-/Aussenraumpruefung | 8.192 |
| Gecachte Himmelsproben pro Rauchaktualisierung | 32.768 |
| Gepruefte Fenster-/Tuerzellen pro Raum | 64 |
| Gespeicherte Rauchzellen pro Ofen | 256 |
| Nahe Rauchzellen im Renderbatch | 256 |
| Nahe Feuerdarstellungen | 32 |
| Ofen-PointLights | 8 |

Unbekannte oder ungeladene Geometrie erzeugt keine Ersatzluft und loest keine
synchrone Weltgenerierung aus. Ueberschrittene Raumbudgets gelten als unbekannt;
das Auffuellen pausiert dann. Der Himmelsnachweis verwendet geladene Daten bzw.
die bereits vorbereitete Generierungsobergrenze, nicht die alte Annahme
`32 leere Zellen = draussen`.

Die vorhandene Rauchschadenslogik bleibt serverseitig bei 2 Schaden/s fuer
betroffene Survival-Spieler und Mobs. Ueberlappende Rauchquellen stapeln diesen
Schaden nicht beliebig.

## Adaptive Rohre und bewegliche Oeffnungen

`crates/terra-world/src/objects.rs` definiert gemeinsam verwendete Rohrports,
Orientierungsbasen und die Pose der beweglichen Bauteile. Die Kombinationen der
sechs achsenparallelen Ports ergeben 15 ungeordnete Portpaare. Bei geeigneter
Blickneigung (`abs(pitch)` zwischen 0,25 und 0,85) kann eine der zwoelf
rechtwinkligen Verbindungen stattdessen das schraeg geknickte Modell verwenden.
Diese Posen speichern die normale Portpaar-Pose plus 32 und verwenden das
kanonische Modell 63. Sie sind kein zusaetzliches Item. Der sichtbare Rohrverlauf
ist schraeg, seine Ein- und Ausgaenge bleiben an den gleichen benachbarten
Zellflaechen; es gibt keine diagonalen 45-Grad-Nachbarzellen.

Neue IDs:

| ID | Inhalt |
| --- | --- |
| 65 | Oak Door |
| 66 | Birch Door |
| 67 | Spruce Door |
| 68 | Oak Window |
| 69 | Birch Window |
| 70 | Spruce Window |

Alle bisherigen IDs bleiben unveraendert. Furnace bleibt 59, das gemeinsame
Chimney-Item 60. Die vorhandenen Rohrmodelle 61 bis 64 werden fuer alte
Spielstaende und als wiederverwendbare Geometrie weiterhin geladen. Ihre Namen,
Beschreibungen, Zielinformationen und gespeicherten Inventar-Stacks bleiben in
der vollstaendigen Blockpalette erhalten; nur der TMI-Katalog filtert sie aus.

Die Darstellung trennt feste Rahmen von beweglichen Blaettern. Nur Letztere
werden im bestehenden Szenenrenderer transformiert; eine Umkehr waehrend der
Animation beginnt bei der aktuellen Position. Modellmaterialien werden geteilt.
Das ist kein neues allgemeines Skelett- oder Animationssystem.

Use und Place schalten Tueren/Fenster um. Die UI verhindert in diesem Frame,
dass dieselbe Eingabe gleichzeitig einen Block setzt. Eine bis zum Loslassen
beider Bindings gesetzte Sperre verhindert zudem Essen durch anschliessendes
Halten, auch nach einem Blickwechsel. Ein neuer Press kann wieder normal essen.
Der Server prueft Reichweite, Rechte, Material und erwartete Orientierung.
Veraltete oder doppelte Toggle-Anfragen koennen die Gegenstelle nicht erneut
umschalten. Beim Schliessen werden kollidierende Spielerpositionen geprueft;
eine Tuer wird nicht einfach in einen Spieler hinein geschlossen.

Offen-Zustand, Orientierung, zweiter Tuerblock und adaptive Rohrpose verwenden
den vorhandenen `BlockObjectState`. Das Regionenformat bleibt **RGA0**; es gibt
keinen zweiten Speicher fuer Oeffnungen. Das Netzwerkprotokoll ist **51**,
deshalb muessen Client und Dedicated Server gemeinsam aktualisiert werden.
Der Generator bleibt unveraendert.

## UI und datengetriebene Kategorien

Saemtliche UI-Aenderungen bleiben im vorhandenen TiltUI-System. Der
Rezeptinspektor kann auch neben einer Truhe angezeigt werden, er darf dort aber
kein unsichtbares Crafting-Grid befuellen. Das Plus zum Uebernehmen in einen
passenden Ofen bzw. ein sichtbares Crafting-Grid bleibt die ausdrueckliche
Aktion; der neue R-Shortcut zeigt nur das Rezept.

Die sechs Standardkategorien liegen weiterhin als einzelne JSON-Dateien unter
`assets/terra/inventory/categories/`. Bestehende Standardzuordnungen wie
`terrain`, `wood`, `nature` und `minerals` werden beim Laden auf die neuen
Kategorien abgebildet. Eigene und namespaced Kategorien bleiben unveraendert.
Essbare Tierprodukte und Pilze erscheinen bei Food; Leder und Wolle bei Animal
Materials. Kleine Steine, Sticks und verarbeitete Minerale gehoeren zu Materials.

Suche und Kategorie werden weiterhin kombiniert: Eine Suche filtert die
ausgewaehlte Kategorie und wechselt sie nicht heimlich. Der Default ohne bisherige
Auswahl ist die erste Kategorie, Building. Der native Ofentest waehlt deshalb
vor der Iron-Ingot-Suche ausdruecklich Materials.

## Validierung

**Status: finale automatisierte Regressionen und nativer Gameplaylauf
bestanden.** Ergebnisse gelten fuer den jeweiligen protokollierten Lauf.
Die Modell-/Gameplayfixtures ersetzen keine kuenstlerische 1:1-Abnahme,
mehrstuendige Mehrspielerlast oder FPS-Paritaet.

| Pruefung | Ergebnis | Nachweis |
| --- | --- | --- |
| Formatpruefung | Final bestanden, Exit 0; keine Formatdifferenzen | [fmt-final.log](fmt-final.log) |
| Workspace-Check | Final bestanden, Exit 0; 3,39 s | [check-accepted.log](check-accepted.log) |
| Workspace-Tests | Final: 1.442 bestanden, 0 fehlgeschlagen, 1 explizit ignoriert; 23 Suites einschliesslich Doc-Tests | [tests-accepted.log](tests-accepted.log) |
| Clippy mit Warnungen als Fehler | Final bestanden; 14,07 s | [clippy-accepted.log](clippy-accepted.log) |
| Workspace-Build | Aktueller Fixture-Quellstand bestanden in 31,73 s | [build-accepted.log](build-accepted.log) |
| Headless Dedicated Server | Bestanden; 27 Sections ohne Fenster | [headless-final.log](headless-final.log) |
| Native Modell-/Shaderfixture mit finaler Rauch-WGSL | Bestanden; zehn Aufnahmen | [native-art-accepted.log](native-art-accepted.log) |
| Native Ofen-/Oeffnungs-/Creative-R-Gesamtfixture | Final bestanden, Exit 0; 13 Aufnahmen, Creative-R und Save/Reload | [native-gameplay-accepted3.log](native-gameplay-accepted3.log) |
| 180-s-UDP-Mehrspieler-Dauerlauf | Nicht ausgefuehrt; explizit ignorierter Test | `sustained_two_profile_udp_session_keeps_identity_and_replication` |
| Langzeit-FPS-/Speichervergleich | Nicht ausgefuehrt | Keine neue Performance-Paritaet behauptet |

Der finale Testlauf kompiliert in 21,81 s. Die eine ignorierte Pruefung
ist der bestehende 180-s-Echt-UDP-Dauerlauf, kein uebergangener neuer Defekt.

Die native Modellfixture laeuft auf AMD Radeon RX 6950 XT, Vulkan/RADV,
Mesa 26.2.4-arch3.1. Der erste Versuch brach wegen inkonsistenter 1x-/4x-MSAA-
Bindungen ab ([native-art.log](native-art.log)); nach Korrektur der
Fixture-Kamerakonfiguration besteht der zweite Versuch. Nach anschliessender
Rauch-WGSL-Abstimmung besteht auch `native-art-accepted.log` mit der aktuellen
Shaderfassung und `captures/furnaces-0.png` bis `captures/furnaces-9.png`.
Ausgewaehlte finale Modell-/Rauchansichten wurden nativ angesehen; das ist
keine vollstaendige Referenzqualitaets- oder Performance-Abnahme.

Der erste Gameplayversuch blieb bei ungezielter Tuerplatzierung nach dem
Zuruecklaufen stehen ([native-gameplay.log](native-gameplay.log)). Die Fixture
nutzt danach regulare Vorwaertsbewegung und den tatsaechlich anvisierten
tragenden Block, ohne Servervalidierung zu umgehen. Im zweiten Lauf funktionieren
Ofenplatzierung, Rezeptsicht, Slottransfers, Fortschritt/Ausgabe, breites und
kompaktes Layout sowie Tuer-/Fensterplatzierung und deren sechs Animationsbilder.
Er bleibt anschliessend in Stufe 32 stehen; der komplette Lauf ist deshalb
**nicht bestanden**. Der naechste Lauf mit getrenntem Modus-Ack und
Inventaroeffnen zeigt weiterhin `mode=Survival`. Ursache ist nicht die
Reihenfolge der UI-Aktionen: Die direkt gestartete Testwelt hatte dem
Testprofil keine `game_mode`-Berechtigung gegeben. Die Fixture konfiguriert
dieses Recht jetzt fuer ihre eigene UUID ueber die vorhandene Serverkonfiguration.
Beide Wechsel bleiben normale autoritative Chatbefehle; kein lokaler
Modus-Bypass wurde eingebaut. Die getrennte Ack-/Layoutsequenz bleibt erhalten.

Der danach gestartete Lauf `native-gameplay-accepted.log` endet bei der
kompakten Layoutpruefung in Stufe 11: gemessen wurden 721 px Inhaltsbreite bei
668 px Bodybreite. Drei vorherige Laeufe hatten diesen Resize bestanden.
Die Fixture wartet deshalb nun auf vier identische, vollstaendig passende
Layoutmessungen statt nur auf 0,7 s Wanduhr. Der Ueberlauftest bleibt strikt
und bricht nach maximal vier Sekunden mit den Einzelmassen von Panel, Body,
Workspace, Grid und Hotbar ab. Ein dauerhafter Layoutfehler wird nicht
ignoriert; die abschliessende Wiederholung besteht diese strikte Pruefung.
Die letzte Gameplay-Stufe ist 37, mit insgesamt 13 erwarteten Aufnahmen.

`native-gameplay-accepted2.log` wurde in Stufe 1 abgebrochen, nachdem externer
Desktop-Fokusverlust den einmaligen Capture-Request verworfen hatte. Die
Produktionslogik gibt bei unfokussiertem Fenster korrekt die Steuerung frei.
Nur die automatisierte Fixture setzt nun den Bevy-Fokus fuer ihre synthetischen
Eingaben und fordert Welt-Capture in den passenden Teststufen erneut ueber
`CaptureRequest` an. In Inventarstufen bleibt der Cursor frei; ein
nicht wiederherstellbarer Zustand bricht nach vier Sekunden ab. Diese
Automatisierung ersetzt keinen Test mit realem Betriebssystem-Mausfokus.

Der abschliessende Lauf [native-gameplay-accepted3.log](native-gameplay-accepted3.log)
beendet Stufe 37 erfolgreich mit 13 Aufnahmen und Exit 0. Er bestaetigt die
autoritativen Ofen-/Tuer-/Fensterplatzierungen, Place-Interaktion bei gehaltenem
Essen ohne Verbrauch, Slotgesten, Fortschritt und Ergebnisentnahme, beide
Ofenlayoutgroessen, die sechs geschlossenen/animierenden/offenen Bauteilbilder,
Creative-R ohne Inventarmutation sowie anschliessenden Survival-Wechsel und
gespeicherte Ofen-/Oeffnungszustaende nach sauberem Serverende.
Das [kompakte Ofenpanel](captures/furnace-running-compact.png) und die
[Creative-Rezeptvorschau](captures/creative-recipe-shortcut.png) wurden zusaetzlich
visuell kontrolliert. Fehlversuche bleiben als Entwicklungsnachweise erhalten
und werden nicht nachtraeglich als bestandene Laeufe umgedeutet.
`native-gameplay-accepted3.log`, `native-art-accepted.log` und
`headless-final.log` enthalten keine ERROR-, Panic- oder ValidationError-Meldung.
Seit dem erfolgreichen Build und diesen nativen Laeufen wurde kein
Produktions- oder Fixture-Code mehr geaendert, nur Dokumentation.

Die automatisierten Regressionen enthalten unter anderem:

- Gegenseitige Rohrports, alle 15 Portpaare und Erhalt historischer Posen.
- Rauchablauf ueber Fenster/Tueren, geschlossene Nebenraeume, Wandloch,
  Bruchteilsbilanz und begrenzte Raum-/Himmelssuche.
- Gemeinsame Fenster-/Tuerleistung fuer mehrere Oefen, eindeutige Rauchzellen,
  ueberlappende historische Quellenlisten und fortgesetzte anteilige Entlueftung.
- Leere Haze-Dichteprobe ohne Terrainzugriff sowie Abschirmung durch feste Waende.
- Tueren-/Fensterplatzierung, doppelter Tuerblock, negative Koordinaten,
  Sectionsgrenzen, Toggle-CAS und blockiertes Schliessen.
- Persistenz von Offen-Zustaenden und allen adaptiven Rohrposen.
- Stetige Hinge-Bewegung und Richtungswechsel.
- Creative-R ohne Itemvergabe, Suchfeldschutz, Rezeptanzeige an Truhen,
  Eingabeprioritaet fuer alle sechs Oeffnungsbloecke und kein Essen durch
  laengeres Halten der Tuer-/Fensteraktion.
- Die sechs mitgelieferten Kategorien und Erhalt benutzerdefinierter Kategorien.
- TiltUI-HTML-/CSS-Parsing und Inventarslot-Routing.

Der erweiterte native `--smoke-test-furnace-gameplay` prueft ueber den echten
Integrated Server den bisherigen Ofenablauf und zusaetzlich:

1. Scrollbarfreie Ofenflaeche bei breitem und kompaktem Fenster; alle Inhalte
   muessen in die gemessene Layoutflaeche passen.
2. Kategoriebuttons als 80-px-Quadrate.
3. Autoritative Platzierung einer Eichen-Tuer und eines Eichen-Fensters.
4. Oeffnen mit der Place-Taste bei ausgewaehltem Essen ohne Verzehr.
5. Aufnahmen geschlossen, waehrend der Animation und offen.
6. Creative-Rezeptinspektion ueber echten TMI-Hover-Handler und R-Taste bei
   unveraendertem Inventar.
7. Gespeicherter Ofeninhalt und geoeffnete Bauteile nach sauberem Serverende.

Die Fixture erwartet 13 Aufnahmen und verwendet eine eigene frische Welt.
Sie wartet vor der Creative-Rezeptprobe zunaechst auf die autoritative
Modusbestaetigung und danach getrennt auf das eingabebereite Inventarlayout.
Ziel- und Antwortwartezeiten sind begrenzt und melden konkrete Statuswerte.
Screenshots allein ersetzen keine Pruefung der Raumventilation.

## Bekannte Grenzen

- Rauch ist ein begrenztes Gameplay-Zellmodell mit weicher Shaderdarstellung,
  kein CFD und kein vollstaendiges volumetrisches Raymarching durch alle Raeume.
- Das lokale Tiefen-Haze approximiert den Rauch in Kameranaehe; die sichtbaren
  Fahnen verwenden weich maskierte, gebatchte Flaechen.
- Raumsuche, Himmelssuche und Rohrnetz sind absichtlich budgetiert. Sehr grosse,
  teilweise ungeladene oder unbekannte Raeume werden konservativ behandelt.
- Ein einzelnes Fenster entfernt weniger Rauch als ein laufender Ofen erzeugt.
  Das ist die geforderte Nettorate, kein sofortiges Leerraeumen jedes Raums.
- Die logische Kollisionspose folgt dem bestaetigten Offen-/Geschlossen-Zustand;
  die 0,45-s-Darstellung ist eine visuelle Ueberblendung, keine kontinuierliche
  dynamische Tuerphysik.
- Spezielle Mining-Frakturmeshes fuer bewegliche Tuer-/Fensterblaetter sind noch
  nicht isoliert. Die vorhandenen Abbaupartikel bleiben der Fallback.
- Neue Modelle orientieren sich an den Referenzen; identische Bildgeometrie,
  Texturen oder eine exakte 1:1-Qualitaet werden nicht behauptet.
- Laengere Tests mit vielen brennenden Oefen, grossen Rohrnetzen, mehreren
  Spielern und Streaming sind noch erforderlich. Keine gemessene
  Performanceverbesserung oder Speicherparitaet wird aus Kurztests abgeleitet.
