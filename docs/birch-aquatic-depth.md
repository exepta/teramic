# Birkenkronen und Tiefenbewegung der Wassertiere

Neue Welten verwenden `ecology_revision = 7`. Die Änderung betrifft nur die
Birkenform: ein breiter verwurzelter Stammfuß verjüngt sich in eine
sichtbare schmale Mitte. Erst im oberen Drittel verzweigt sich der Stamm zu
einer breiteren, gegliederten Krone. Andere Baumarten, Flora und Gelände behalten
die Regeln aus Revision 6. Generierte Birkenanker werden nicht versetzt.

Gespeicherte Revisionen 0–6 behalten ihre bisherigen Birken. Ein fehlendes Feld
bedeutet weiterhin Revision 0. Neue Dedicated-/Clientwelten erhalten 7; beim
Neustart hat der gespeicherte Weltvertrag Vorrang vor neuen Konfigurationsdefaults.
Spieleränderungen und vorhandene Weltdateien werden nicht nachträglich verändert.

Neu gepflanzte Birken wählen beim Wachstum dieselbe neue Form, sofern die
gespeicherte Welt Revision 7 verwendet. In älteren Welten bleibt die bisherige
Pflanzform erhalten. Bereits ausgewachsene Spielerbäume behalten ihre exakten
gespeicherten Holz- und Laubzellen. Der asynchrone Wachstumspfad übernimmt die
Weltrevision; neue Speicherfelder und eine Protokolländerung sind nicht nötig.

Der Baumaufbau liegt in `generation/vegetation/birch.rs`. Er verwendet verbundene
Holzkapseln und vorhandene Blattkronen. Die bestehenden Section-, Abbau-,
Kollisions- und Speicherrouten verwenden dieselben Weltblöcke. Regressionen prüfen
den breiten Fuß, die schmale blattfreie Mitte, die obere Kronenbreite, verbundene
Wurzeln und Äste sowie die unveränderten historischen Formen und anderen Arten.

Die native Prüfszene `reference-birch` sucht einen vorhandenen Baum in einer
Revision-7-Welt und prüft Stamm, Kronenextrema und weitere Stichproben zuerst
im Generator und anschließend in geladenen Sections. Ganzbaum- und Nahansicht
verwenden keine künstlich gesetzten Bäume. Der JSON-Prüfbericht enthält den
Wurzelpunkt, die gemessenen Formbreiten, Kameras und geprüften Weltzellen.

## Wassertiere

Die frühere Schwimmbewegung variierte die Höhe überwiegend sinusförmig um die
Starttiefe. Jetzt wählen Fische, Tintenfische, Quallen und Haie ein anhaltendes
Tiefenziel innerhalb der tatsächlich zusammenhängenden geladenen Wassersäule.
Die Präferenz wechselt alle 32 Simulationssekunden und verwendet die bereits
gespeicherten Werte Identität und Alter. Dadurch bleiben Verlauf und Tiefe über
Save/Reload reproduzierbar, ohne neue Speicherfelder oder Netzwerktypen.

Ein Tier hält Abstand zum Wasserboden und berücksichtigt seine gesamte Höhe
unter der Oberfläche. Spielerblöcke, Teilwasserzellen und fehlende Sections
begrenzen den verwendbaren Raum. Die Zielabfrage nutzt den vorhandenen begrenzten
Wassersäulenpfad und fordert keine Chunks an. Tiefere Bereiche werden schwimmend
erreicht; es gibt kein Teleportieren zu Boden oder Oberfläche.

Bewegung und Drehung prüfen den vollständigen gedrehten Körper entlang des
Weges in kurzen Schritten. Lange Hai-/Tintenfischkörper können dadurch nicht
mit der Seite durch Uferwände schwenken. Begrenzte Ausweichbewegungen können
auch senkrecht erfolgen. Bestehende Angriffe, Flucht, Tintenabgabe, Bodenruhe
und Fangsituationen bleiben erhalten. Normale Fische behalten ihren bestehenden
Sprung- und Strandpfad; die Tiefensteuerung greift beim normalen Schwimmen.

Die Simulation läuft weiterhin über die autoritativen Kreaturenpfade.
Geladene Replikas übernehmen die bestätigten Positionen. Diese Änderung erzeugt
keine neuen Wasserreservoirs, verändert keine Wasserhöhen und ersetzt keine
Art-/Spawnfilter. In flachem Wasser oder bei fehlenden tiefen Sections ist der
nutzbare Tiefenbereich entsprechend begrenzt.

Neue Regressionen führen alle Wasserarten durch die echte Simulationsdispatch,
prüfen messbares Abtauchen, vollständige Körperbedeckung, bearbeitete Böden und
Decken, unbekannte Tiefe, Drehkollisionen, Teilwasser und Population-Save/Reload.
Die gemeinsame Projektvalidierung führt diese Tests aus; dieses Dokument
behauptet keine noch nicht bestätigten Test- oder Leistungswerte.
