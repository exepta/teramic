# Kontextgleise, sanfte Stufen und freier Unterbau

Implementierungsbericht vom 08.11.2026 zur
[Track-Logic-Referenz](../../../../docs/references/track-logic-ramps-and-curves.png),
aufbauend auf [Weichen und Schotter](../rail-curves-switches-ballast-2026-11-08/README.md).
Die zentrale CPU-Abnahme umfasst 4.233 bestandene Tests, keinen Fehler und
neun bestehende ignorierte Tests. Keine Grafiktests, Spielclients,
GPU-Tests oder Screenshots wurden gestartet.

## Rampe an der markierten Stufe

Bei direkt voreinanderliegenden unteren und oberen Gleisenden fehlt der Platz
für ein zusätzliches Steigungsstück. Der Kontextplaner kann deshalb die letzte
vollständige gerade Viermeter-Baugruppe in eine echte kubische Rampe umbauen.
Der Anschluss am anderen Ende bleibt exakt erhalten; die neue Oberkante dockt
an das obere Gleis oder die nahe Oberkante der erhöhten Bodenfläche an.
Auch ein tatsächlicher Treffer auf die senkrechte Stufenwand wird ausgewertet.
Es entsteht kein senkrechter Sprung zwischen zwei flachen Enden.

Der Umbau ist auf eine unveränderte, vollständige gerade Baugruppe mit genau
einem Block Höhenunterschied am freien Ende begrenzt. Das gehaltene Gleis muss
dieselbe Holzart besitzen. Andere Baugruppen und Gelände werden nicht
überschrieben. Alte und neue Besitzerzellen, Kontrolldaten, Wasser sowie
berührte Props werden gemeinsam und atomar umgestellt. Die öffentliche
Commitprüfung rekonstruiert den kanonischen Umbau und lehnt abweichende oder
unvollständige Pläne ab.

Die dynamischen Sicherheitsgrenzen sind jetzt **2,5 Meter minimaler Radius**
und **50 Prozent maximale Steigung**, damit ein sanfter Block Höhenunterschied
über vier Meter möglich ist. Die tatsächliche kubische Rampe beginnt und endet
mit passender Tangente. Bestehende native Kurven mit Radien 4,5/8,5 und die
gespeicherten alten Steigungsmodelle behalten ihre Form. Die maximale freie
Verbindungslänge bleibt 32 Meter. Der begrenzte Bézierplaner prüft bei Rampen
höchstens 49 Griffpaare, bei ebenen Verbindungen weiterhin höchstens 36.

## Direkte Andockvorschau und Mausrad

Mit gehaltenem **Track 196**, aktivem Spielcursor und Blick auf ein tatsächlich
erreichbares freies Ende erscheinen passende direkte Bauvorschläge:

- **Geradeaus:** normale gerade Verlängerung beziehungsweise die passende
  sanfte Rampe an einer Stufe.
- **Kurve:** eine echte Verbindung zu einem anderen passenden freien Gleisende
  in der Nähe. Eine fest vorgegebene Dekorationskurve wird nicht erfunden.

Die Vorschläge verwenden den aktuellen Zieltreffer und die realen Ports.
Ein Blick auf die Mitte eines Gleises erzeugt keinen seitlichen Anschluss.
Wenn am sichtbaren Andockpunkt genau zwei geometrisch unterschiedliche
Vorschläge bestehen, wechselt das **Mausrad** zwischen Geradeaus und Kurve.
Ein normaler Platzieren-Klick baut den gewählten Vorschlag. Ungültige räumliche
Vorschläge bleiben rot; der rahmenlose Bauhinweis erklärt den Grund.

Außerhalb dieses Kontexts bleibt das Mausrad bei Hotbar beziehungsweise
Offline-Blockpalette. Ein gehaltenes Track-Item allein genügt nicht für die
Übernahme. Menüs, gesperrte Eingabe, Props vor dem Gleis, andere gehaltene Items,
unpassende Bauformen und fehlende echte Alternativen übernehmen das Rad nicht.
Erhöhte Boden-/Wandtreffer dürfen die Rampenvorschau auslösen, übernehmen aber
das Mausrad nicht als vermeintlichen sichtbaren Gleisanschluss.

Längere Verbindungen bleiben zusätzlich manuell planbar:

1. **Shift + Platzieren** an einem freien Gleisende merkt den Start.
2. Zum Ziel gehen und mit normalem Platzieren bestätigen.
3. **Shift + Platzieren** mit gemerktem Start verwirft diese Auswahl.

Die Hinweise verwenden die tatsächlich konfigurierten Tasten. Die bisherigen
R-Bauformen für Kreuzung und Prellbock sowie eigenständige Weichen bleiben
erhalten.

## Eingabereihenfolge

Die Kontextprüfung folgt dem aktuellen Targeting. Danach entscheidet ein
gemeinsamer Zustand, ob das Rad die Bauform oder die normale Navigation ändert.
Beide Radleser verarbeiten ihre Ereignisse auch bei unterdrückter Navigation,
damit nach dem Verlassen des Kontexts keine alten Scrollereignisse nachwirken.

Eine normale Slotänderung aktualisiert Hotbarzustand, aktives Item, ausgewählten
Netzwerkslot und Palette sofort. Das Targeting wird noch vor der Interaktion
erneut mit dem neuen Item ausgeführt. Reine Hotbar-/Cursoransichten folgen
dieser Auswahl ebenfalls im selben Frame. Ein Holzartenwechsel übernimmt
keinen noch aus dem alten Item stammenden Start oder Sondermodus.

## Server, Kosten und Besitz

**Protocol 114** ergänzt den Kontextbefehl `BuildRail`. Er übermittelt den
tatsächlichen Zielblock, Geradeaus/Kurve und einen stabilen Fingerabdruck des
Bauvorhabens. Der Server berechnet den Vorschlag aus Welt, echtem Zielstrahl,
Holzart und aktueller Pose neu. Der Fingerabdruck ist eine Übereinstimmungsprüfung
der Bauabsicht und ersetzt keine Berechtigung. Andere inzwischen gewählte
Quell-/Zielports oder Kontrollpunkte führen zum Konflikt, statt unbemerkt eine
andere Verbindung zu bauen.

Revision, richtiger gehaltener Slot, Holzart, geladene Besitzerzellen und Rechte
werden vor der Veröffentlichung geprüft. Automatische Verbindungen verlangen
erreichbare Quell- und Zielports; der tatsächliche Klick bleibt auf 4,5 Meter
begrenzt. Eine normale Viermeter-Verlängerung behält ihre bisherige Reichweite
am angeklickten Anschluss. Beim Umbau darf der erhaltene hintere Anschluss
weiter weg liegen, da eine bereits angeklickte vollständige Baugruppe geändert
wird. Das neue obere Ende bleibt erreichbar.

Der Server autorisiert alle neuen Zellen und beteiligten vollständigen
Baugruppen. Der Umbau benötigt außerdem Break-Rechte an sämtlichen ersetzten
Zellen. Die bisherigen Prop-, Spieler- und Weltgrenzen bleiben aktiv.
Eine veraltete Auswahl, falsche Holzart oder fehlende Inventarmenge verändert
keinen Teil der Welt.

Für den Umbau zählt ausschließlich der positive Mehrbedarf gegenüber dem
bereits verbauten Gleis. Beispiel: Ein vorhandenes gerades Stück kostet ein
Gleisitem, die längere Rampe insgesamt zwei; beim Umbau wird ein weiteres Item
verbraucht. Es gibt keine Rückerstattung oder zusätzlichen alten Gleisdrop.
Ein Bedarf von null verändert den Inventarstapel und seine Revision nicht.

## Unter den Schienen abbauen

Die besondere Sperre für Änderungen an Gleisauflagen ist entfernt. Stein,
Schotter und geformter Untergrund dürfen mit den normalen Rechten, Werkzeugen,
Zielstrahlen und Abbauzeiten entfernt oder geändert werden. Vorhandene Gleise
fallen dadurch nicht als zusätzlicher Drop herunter und verlieren weder
Besitzmetadaten noch Wasser, Geometrie oder Streckennetz.

Der live ausgewertete Schotterbonus entfällt unmittelbar, sobald die erforderliche
Auflage fehlt. Ein späterer normaler Gleisabbau oder eine Weichenschaltung bleibt
auch bei entferntem Untergrund möglich. Neue Platzierung und Rampenumbau prüfen
weiterhin ihren eigenen benötigten Bauplatz und Unterbau. Die Freigabe lockert
keine Rechte an Gleisen oder fremden Grundstücken.

## Speicher, Arbeitsgrenzen und CPU-Abnahme

**RG13**, **ID12** und **PLY7** bleiben unverändert. Dynamische Kontrollpunkte
werden weiter in sämtlichen Besitzerzellen gespeichert. Die Rekonstruktion
funktioniert über Sectiongrenzen und bei negativen Koordinaten. Ein Rampenumbau
kopiert nur die betroffenen Sparse-Sections und ihren Mesh-Halo.

Die automatische Suche erzeugt keine Terrainchunks. Sie betrachtet höchstens
die acht vom Augenbereich geschnittenen Sections. Verbundene Ports werden über
75 direkte lokale Sectionabfragen statt einen Scan aller gespeicherten Gleissections
erkannt. Vorschläge und Transaktionen bleiben auf ihr festes Zell-/Propbudget
begrenzt. Native Modelle und Vorschau verwenden weiterhin dieselbe Core-Geometrie.

CPU-Regressionen prüfen insbesondere echte Wandtreffer und Rampenumbau in allen
vier Ausrichtungen, erhaltene hintere/obere Anschlüsse, Replikatimport und Graph,
Props und späte Überlauf-Rollbacks, veränderte Fingerabdrücke und Fremdclaims,
reale Geradeaus-/Kurvenalternativen, Mausradübernahme ohne Hotbar-Nachlauf sowie
erneutes Targeting mit dem neuen Slot. Ein echter UDP-Lauf prüft den Kontextbau
mit Survival-Inventar, zwei Clients, späterem Beitritt und veralteter Bauabsicht.
Alle zwölf Workspace-Library-/Bin-Suiten bestehen mit insgesamt 4.233 Tests;
neun bestehende Tests bleiben ignoriert. Der kopflose Assetexport stimmt die
nativen Modelle mit dem aktuellen Core-Profil ab. Clippy aller Targets mit
`-D warnings`, Formatierung, Diff und der vollständige Workspace-Build mit
aktuellen Client-/Serverdateien sind sauber. Ergebnisse und Quell-/Logprüfsummen
sind in `verification.json` festgehalten.
