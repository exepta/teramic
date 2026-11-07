# Gleiskurven, eigene Weichen, Weichensteller und Schotter

Implementierungsbericht vom 08.11.2026, Fortsetzung des
[freien Gleisbaus](../free-rail-building-2026-11-07/README.md).
Maßgebliche Referenzen sind die
[Gleisanordnung mit markierten Kurven](../../../../docs/references/railway-curve-placement.png)
und die [Schotterfamilie](../../../../docs/references/railway-ballast.png).
Alle **4.213 Workspace-Library-/Bin-Tests bestehen**, keiner ist fehlgeschlagen;
neun bestehende Tests sind ignoriert. Clippy aller Targets mit `-D warnings`,
Format, Diff und der vollständige Client-/Server-Build sind erfolgreich.
Es werden keine Grafiktests,
Spielclients, GPU-Galerien oder Screenshots gestartet.

## HUD und Bauhinweise

Ein gehaltenes Gleis erzwingt die obere Blockinformation nicht mehr. Diese folgt
weiter ausschließlich dem tatsächlich anvisierten Block, Prop oder Wesen.
Leerer Himmel und Wasser erzeugen keine leere Zielkarte. Bauform, Bedarf und
Fehler stehen in einem getrennten, rahmenlosen Texthinweis über der Hotbar.
Die tatsächlichen Tastenbelegungen erscheinen in den Hinweisen. Menüs,
gesperrte Spielereingabe und die HUD-Ausblendung unterdrücken diese Anzeige.

## Kurven und Abzweigungen

Die Richtung eines neuen Bodenendes folgt der Verbindung statt der Kamera.
Ein Blick zurück zum Start dreht den Anschluss deshalb nicht mehr ungewollt
gegen den Fahrweg. Bestehende Ziele liefern ihre echten freien Ports. Der
kubische Bézierplaner erhält zuerst den bisherigen erfolgreichen Kontrollsatz
und prüft bei Bedarf höchstens 36 unabhängige Griffpaare pro Zielport.
Ungleich lange Schenkel können dadurch eine echte 90°-Verbindung bilden.
Gleiche Eingangsdaten liefern dieselben Kontrollpunkte; bereits gespeicherte
Kurven werden nicht neu berechnet.

Die Grenzen bleiben 32 Meter Verbindungslänge, 4,5 Meter minimaler Radius,
25 Prozent Steigung und 4,5 Meter Interaktionsreichweite. Überlappende
Besitzerzellen, fehlender Unterbau, Props ohne Abbaurechte und Spieler bleiben
Hindernisse. Eine geometrisch berechenbare, blockierte Kurve bleibt als rote
Vorschau sichtbar. Unmögliche Entwürfe zeigen einen Grund im Bauhinweis.

Seitliches Ansetzen an die Mitte eines Gleises erzeugt keinen neuen Port.
Für einen Abzweig setzt man eine eigene Weiche und verlängert deren freie
Enden mit demselben Gleisitem. Beide Weichenhände besitzen echte Anschlussports;
der Routengraph verwendet weiterhin nur den eingestellten Fahrweg.

Bedienung bei Standardbelegung:

1. Mit **Track** ein freies Gleisende rechts anklicken, zum Ziel gehen und die
   Verbindung mit Rechtsklick auf Boden oder ein anderes freies Ende bestätigen.
2. **Shift + Rechtsklick** verwirft den Start. Itemwechsel oder Abbau tun dies
   ebenfalls. **R** schaltet beim gemeinsamen Gleisitem nur Gleis, Kreuzung und
   Prellbock um.
3. **Left Switch** oder **Right Switch** halten und auf Boden oder einen
   geeigneten vorhandenen Anschluss setzen. Ein Item baut eine gesamte Weiche.
   Sie sind eigenständige Blöcke und keine R-Bauform des Gleisitems.
4. Einen **Rail Switch Stand** neben dem Weichenanfang setzen. Für jede Hand
   gibt es drei seitliche Stellplätze außerhalb des historischen Fußabdrucks.
   Rechtsklick auf den Steller schaltet die gesamte Weiche. Die Blockinformation
   zeigt Geradeaus, Abzweig oder eine fehlende eindeutige Zuordnung an.

## Eigene Weichen und getrennter Steller

Die bestehenden Material-IDs **202/203** werden als eigene linke/rechte Weichen
erhalten. Eiche, Birke, Fichte und Kirsche sind in Rezept, TMI, Inventar,
Vorschau, Platzierung, Pick, Save/Reload und Drop dieselbe gespeicherte Holzart.
Platzierung und Survival-Abbau benötigen beziehungsweise liefern ein Weichenitem.
Das gemeinsame Track-Item 196 behält seine Längenkosten und kann keine Weiche
über einen gefälschten Platzierungsbefehl erzeugen.

Der separate Steller hat die neue ID **209**. Seine Zuordnung ergibt sich aus
unmittelbar benachbarten vollständigen Weichen. Fehlende, mehrdeutige oder
unvollständige Nachbarn liefern keine Zuordnung. Es existiert kein entfernter
gespeicherter Fernsteuerungslink. Die Authority prüft den wirklichen Steller-Ray,
Reichweite, Opcode, Facing, Revision sowie Inventories-Rechte am Steller und
über die gesamte Weiche. Das Umschalten veröffentlicht alle Weichenzellen
atomar; der Steller selbst benötigt keine zweite Zustandsänderung.

Der rote Hebel ist aus dem Gleismodell ausgelagert. Eine kleine Stahlplatte
bleibt als Anschlussdetail. Der historische 38-Zellen-Fußabdruck ist ausdrücklich
konserviert, damit alte Welten ihre Sekundärzellen behalten. Leere historische
Reservierungszellen zeichnen nichts, kollidieren nicht und benötigen keine
Phantom-Stützblöcke. Sie bleiben ausschließlich Besitzmetadaten. Der separate
Steller besitzt ein eigenes natives Modell, Kollisionsprofil, Rezept und
normale Platzierungs-/Abbaupfade.

Alte getragene Formitems bleiben lesbar und verwendbar: 197–201 erlauben ihre
bisherige Form oder den automatischen Gleisbau; 204/205 ihre eigene bisherige
Form. Weichen sind von dieser Umwandlung ausgenommen. Der Server verbraucht
jeweils den tatsächlich ausgewählten Stapel in passender Menge.

## Schotter und Gleisbett

Neue Material-IDs:

| ID | Block | Form |
| --- | --- | --- |
| 206 | Railway Ballast | voller Schotterblock |
| 207 | Ballast Stairs | vier Tritte, alle 24 Treppenposen |
| 208 | Ballast Slab | halber Block, alle sechs Slab-Ebenen und Kombinationen |
| 209 | Rail Switch Stand | getrennter Weichensteller |

Die deterministische native 512er-Schottertextur verwendet kantige warme
grau/beige Steine. Alte dreistufige Treppen bleiben unverändert; neue vierstufige
Innen-/Außenecken verwenden denselben Formaufbau für Mesh, Kollision und Wasser.
Der begrenzte Rendercache unterscheidet die Trittzahl von Block und Nachbarn.
Historische Mixed-Slab-Facings 10–69 bleiben gleich; Schotter ergänzt 70–75.

`terra_world::rails::ballast` liefert live **1,5 × Grundgeschwindigkeit**, wenn
alle tatsächlichen Auflagen einer vollständigen Baugruppe auf tragfähigem
Kies oder Schotter liegen. Bestehender Kies 42 und alle neuen Schotterformen
qualifizieren. Treppen benötigen eine volle tragende Oberseite, Slabs eine
obere oder vollständige Form. Bei gemischten Slabs zählt die wirkliche
Oberseite: Stein unten und Schotter oben darf beschleunigen, die umgekehrte
Kombination nicht. Teilweise Steinauflage und fehlender Unterbau geben keinen
Bonus. Ein Faktor wird genau einmal angewandt; 8 m/s werden 12 m/s.

`speed_limit` und `travel_seconds` verwenden dieselbe Auflagenprüfung und
aktive bestätigte Pfade, ohne geometrische Routendistanzen zu verändern.
Terrainänderungen wirken unmittelbar und benötigen keinen zusätzlichen
persistenten Geschwindigkeitscache. Fehlende Module, inaktive Weichenpfade
und ungültige Geschwindigkeiten werden abgelehnt. Die Blockinformation zeigt
den tatsächlichen Gleisbettbonus an. Der eigentliche Fahrbetrieb und die
Fahrzeugmontage bleiben die offenen Aufgaben **F53/F63/F64**; hier wird kein
bereits fahrender Zug behauptet.

Alle drei Schotterformen verwenden Gravel-Kontaktklänge. Der Steller verwendet
Metall-/Steinkontakt und einen bestätigten Schaltimpuls. Dafür werden vorhandene
Sounds verwendet; es startet keine Audio- oder Grafikabnahme.

## Daten und Prüfungen

**Protocol 113** grenzt die neuen Materialien und getrennten Itemregeln von
älteren Clients ab. Regionhülle **RG13**, Gameplayindex **ID12** und
Spielerformat **PLY7** bleiben unverändert. Sechs neue Rezepte bringen den
eingebauten Katalog auf **331** Rezepte. Die sechs neuen Holz-Weichenvarianten
erweitern die Itemdefinitionen von 169 auf **175**.

CPU-Regressionen decken insbesondere kamerafreie Bodenrichtungen, echte
rechtwinklige Verbindungen und Weichenabgänge, separate Stellerrays und
Rechte, atomare Drops/Inventare, Legacybesitz, Slab-Codes, Treppenecken,
Katalog-/Rezeptdateien, Zielkarten, Bauhinweise sowie den live ausgewerteten
50-Prozent-Bonus ab. Echte UDP-Clients prüfen Stellerplatzierung, Replikation,
Late Join, Umschalten und sekundären Survival-Abbau. Alle zwölf Testsuiten
bestätigen **4.213 bestandene Tests, null Fehler und neun bestehende ignorierte
Tests**. Clippy aller Targets mit `-D warnings`, Formatprüfung und Diff sind
sauber. Der abschließende Workspace-Build hat aktuelle Client-/Serverdateien
erfolgreich erstellt. Der kopflose Gleisexport bestätigt die gemeinsame
Core-Provenienz. Vollständige Logs und Quell-/Asset-Prüfsummen stehen in
[verification.json](verification.json). Es wurden keine Grafiktests,
Spielclients oder Screenshots gestartet.
