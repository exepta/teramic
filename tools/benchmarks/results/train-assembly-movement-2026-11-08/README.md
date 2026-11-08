# Gleisbau, Bewegungsbestaetigung und modulare Zugmontage

Umsetzung vom 08.11.2026 fuer die [Gleisbaureferenz](../../../../docs/references/railway-one-support-aim.png)
und [F63-Bauteilreferenz](../../../../docs/references/railway-modular-vehicle-reference.png).
Alle Pruefungen erfolgen ohne Spielfenster, GPU, Screenshots oder grafischen Test.
Die zentrale CPU-Abnahme umfasst **4310 bestandene Tests**, keinen Fehler und
neun bestehende ignorierte Tests. Ergebnisse und Quell-/Logpruefsummen stehen
in `verification.json`.

## Gleise bauen

Der bestehende Ablauf bleibt: ein Startgleis setzen, zum Ziel schauen, die ganze
Verbindung mit der naechsten Platzierung bauen. Der spezielle Baustrahl sowie
Quell-/Zielpruefung reichen jetzt 32 Meter. Ein einzelner tatsaechlich tragender
Block unter dem Modul reicht; schwebende Strecken ohne jede Auflage sind ungueltig.
Die normale Reichweite fuer Bloecke, Abbau und Bedienung bleibt 4,5 Meter.

Die Zielplanung benutzt den genauen Strahltreffer statt nur dessen Voxelkoordinate.
Der seitliche Anschluss rastet in halben Bloecken ein; der Vorwaertsabstand folgt
der anvisierten Flaeche. Damit bleibt eine enge 90-Grad-Verbindung auf beiden
Seiten einer Blockgrenze stabil und der anvisierte Einzelblock traegt das Gleis.
Vorschau und Authority erzeugen denselben Plan. `StretchRail` bestaetigt dessen
Signatur einschliesslich Quelle, Form und Holz. Hindernisse, geladene Chunks,
Rechte, Spielerkoerper, Fahrzeuggeometrie und Materialbedarf werden weiterhin vor
der atomaren Veroeffentlichung geprueft.

## Bewegung

Ein bestaetigter Eingabeschritt entspricht jetzt genau einem simulierten festen
Zeitschritt. Vorher wurden mehrere wartende Eingaben zu einer Bewegung verdichtet
und gemeinsam bestaetigt, waehrend der Client jeden Schritt einzeln vorausberechnete.
Die Differenz fuehrte insbesondere bei Baubursts und schneller Bewegung zu
Rueckkorrekturen. Gueltige wartende Eingaben verfallen nicht nach 250 ms.

Fehlende Pakete wiederholen keine alte horizontale Bewegung. Am Boden wird der
tatsaechliche Halt erneut geprueft. In der Luft ueberbrueckt eine begrenzte,
nicht durch einzelne Pakete erneuerbare Wartezeit kurze Luecken; anschliessend
wirkt normale Schwerkraft. Die Kollisions- und Berechtigungspruefungen bleiben aktiv.

## F63 bedienen

Neue Bauteile sind im Katalog und ueber Werkbankrezepte verfuegbar:

| ID | Bauteil | Zweck |
| --- | --- | --- |
| 210 | Zug-Fahrgestell | Basis und Masse |
| 211 | Drehgestell | Radsaetze ueber echten Gleisen |
| 212 | Zugrahmen | Verbundene Tragstruktur |
| 213 | Zugkupplung | Vorderes/hinteres Verbundende |
| 214 | Fuehrerstand | Sitz und Lokomotivenrolle |
| 215 | Zugsitz | Personenwagen und Sitzplatz |
| 216 | Wagen-Laderaum | Eigenes, gespeichertes Inventar mit 18 Slots |
| 217 | Zug-Montagestelle | Aufbau zusammensetzen und zerlegen |

Ein minimaler Aufbau besitzt Fahrgestell, mindestens zwei Drehgestelle ueber
Gleisen, Rahmen und Kupplung. Die Teile muessen ueber Nachbarzellen verbunden
sein. Lokomotiven benoetigen einen Fuehrerstand, Personenwagen einen Sitz,
Tender und Gueterwagen Laderaum. Mehrere nahe Wagen werden einzeln montiert;
aneinander liegende Kupplungen verschmelzen nicht ihre Aufbauten.

Die Montagestelle neben den Aufbau setzen und mit Rechtsklick oeffnen. Den
Fahrzeugtyp waehlen und **Zusammensetzen** klicken. Die Ansicht listet Fahrzeuge,
Masse, Teile und Sitze. Zwei Fahrzeuge auswaehlen, deren Kupplungsenden einstellen
und **koppeln**. Zum Zerlegen zuerst den Verbund loesen; die urspruenglichen
Zellen muessen frei sein. Fremde Besitzer, Claims, blockierter Raum und benutzte
Sitze verhindern unzulaessige Aktionen ohne Teilveraenderung.

Ein Klick auf den montierten Sitz/Fuehrerstand setzt den Spieler auf den echten
Sitz; die konfigurierte Aktion **Fly Down** (standardmaessig Shift) verlaesst ihn.
Andere Clients erhalten dieselbe Sitzbindung. Rechtsklick auf den Laderaum
oeffnet dessen privates Inventar. Ein Klick verschiebt ganze Stapel zwischen
Laderaum und den 27 Spielerslots. Der Server prueft beide Revisionen sowie
reservierte Craftingmengen; der Client verschiebt keine Items vor der Bestaetigung.

Die Fahrzeugdarstellung verwendet dieselben nativen Modelle wie die gesetzten
Bauteile. Raeder folgen der Zwei-Meter-Spurweite. Der gemeinsame Modellversatz
von -0,7375 Metern bringt die montierten Raeder auf den Schienenkopf; Darstellung,
Sitzposition, Treffer und echte zusammengesetzte Kollisionen teilen diesen Bezug.
Geometrie wird pro Fahrzeug und Material zusammengefasst und bei reinen
Ladungs-/Verbund-/Revisionsaenderungen nicht neu erzeugt. Beim Zerlegen werden
alte Kollisionen erst nach den zugehoerigen vollstaendigen Section-Aenderungen
und bereiter lokaler Terrainkollision entfernt. Schnelles erneutes Montieren
uebergibt die Kollision an das aktive Ersatzfahrzeug; geaenderte oder geloeschte
Bauteile halten keine veraltete Fahrzeugkollision fest.

F63 stellt montierte, stehende Fahrzeuge bereit. Fahrbetrieb/Dampfantrieb (F53),
freier Aufbau aus allen Blocktypen (F64) und der detaillierte BR52-Baukasten (F54)
bleiben eigene Aufgaben in TODO.md.

## Persistenz, Grenzen und CPU-Abnahme

Protokoll **116** ergaenzt signierte Gleisverbindungen, Zugbefehle, oeffentliche
Fahrzeugansichten und Sitzbindungen. Private Ladungsinhalte werden ausschliesslich
dem berechtigten Benutzer geschickt. **ID13** erweitert den eingefrorenen
ID12-Index um die Zugregistry; alte Welten laden mit leerer Zugregistry. **RG13**
und **PLY7** bleiben erhalten. Retirierte Fahrzeug-IDs werden nicht wiedervergeben.

Bis zu 64 Fahrzeuge mit je 256 Teilen, insgesamt 4096 Teilen und 16 Fahrzeugen
je Verbund begrenzen Arbeit und Snapshotgroesse. Montage/Demontage bereiten nur
betroffene Sections und ihre Invalidierungsraender vor. Registry, Zellen,
Ausrichtungen, Besitzer und Inventare werden gemeinsam veroeffentlicht.

CPU-Tests pruefen den Einzelhalt mit echtem Baustrahl und allen vier Drehungen,
Reichweitengrenzen, Hindernisse, unveraenderte Ablehnungen und signierte Requests.
UDP-Pruefungen behandeln schnelle Bewegung mit Baubursts, Spruenge und Paketluecken.
Fahrzeugpruefungen decken Rollen, Masse, symmetrische Kupplungen, Revisionsgrenzen,
atomare Montage, Cargo, Speicherkompatibilitaet, echte Modellgeometrie,
Kollisionen, Sitzreplikation und spaet beitretende Clients ab.
