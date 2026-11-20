# Bergfried und Gebaeudevorlagen

Der Bergfried verwendet die gewohnten braunen Oberflaechen, goldenen Rahmen und
gruenen Aktionsakzente. **K** im eigenen Gebiet beziehungsweise die Interaktion
mit einem Bergfried oeffnet seine Verwaltung. Die sechs Tabs sind getrennte
UI-Komponenten unter `src-ui/components/`.

| Tab | Verfuegbare Daten und Aktionen |
| --- | --- |
| Allgemein | Wappen, Name, Bannerfarbe, Gruender, eigene Rolle, Siedlungszahlen, Level/Aufwertung und Claims |
| Mitglieder | Mitgliedsauswahl, Rollen, tatsaechliche Berechtigungen, Rollenwechsel und Entfernen entsprechend den eigenen Rechten |
| Diplomatie | Reichsauswahl mit Wappen, bestaetigte Beziehung, eigene Haltung und vorhandene Neutral-/Buendnis-/Feindlich-Aktionen |
| Gebaeude | Vorlagenkatalog, Materialbedarf, manuelle Bauplaetze, Stadtlager, Wohngebiete und laufende Auftraege |
| Arbeitskraefte | Berechtigter Arbeitskraeftebestand, Berufe, Auftraege, Lager-/Gebietszuweisung und Inventare |
| Armee | Eigene geladene Soldaten, Ausruestung, Lebenspunkte, Position und aktueller Zustand |

Mitglieder und diplomatische Ziele verwenden eine Liste mit Detailansicht.
Die UI zeigt wirkliche Registry- beziehungsweise Replikationsdaten. Die Armee-
Zahlen betreffen die geladene Umgebung; eine entfernte Weltarmee wird nicht
geschaetzt. Individuelle Gruppen, Boten, globale Kriegshistorien, Strategien und
zusaetzliche Wirtschaftsgebaeude aus den Referenzbildern sind keine neu
implementierten Spielsysteme.

## Vorlage auswaehlen und bauen

1. Im Tab **Gebaeude** eine Vorlagenkarte auswaehlen. Auswahl und Vorschau allein
   erzeugen keinen Auftrag und verbrauchen kein Material.
2. Rechts stehen Name, Kategorie, Abmessungen, eine Planvorschau und die komplette
   Materialliste dieser Vorlage mit wirklichen Item-Symbolen und Mengen.
3. Einen Stadtlagerbereich mit echten Truhen speichern und diese befuellen.
4. **Hausstandort waehlen**, einen geeigneten Boden in eigenen Build-Claims
   anklicken, Ausrichtung waehlen und **Bau beauftragen**.
5. Handwerker anmelden. Die bekannten Laufwege, Transporte und Bauphasen bauen
   weiterhin sichtbar mit echten Materialien. Maximal vier Handwerker arbeiten
   an einem Haus; aktive Zeiten bleiben 30/15/10/5 Minuten.

Bauauftraege und Wohngebietsbefehle uebermitteln die Auswahl als ID an den Server.
Er verwendet seine gepruefte JSON-Datei. Der Client kann keine Baumaterialpreise oder Voxelplaene vorgeben.
Die Materialliste der Vorlage enthaelt alle **dauerhaften** Bauteile einschliesslich
gepaarter Objekte und Topfinhalte. Bodenvorbereitung und bezahlte temporaere
Gerueste entstehen passend zum Standort. Der angelegte Auftrag zeigt deshalb
zusaetzlich den Materialbedarf seiner noch offenen Bauschritte, einschliesslich bereits
in Truhen oder Handwerkerinventaren vorhandener Materialien. Beim Rueckbau nehmen
die Handwerker das Geruestholz in ihr Inventar und bringen es zum Stadtlager.

Fuer das automatische Wohngebiet gilt die gerade ausgewaehlte Vorlage, sobald
**Wohngebiet aktivieren** bestaetigt wird. Die gespeicherte Auswahl steht am
Wohngebiet und bleibt nach Serverneustart erhalten. Ein spaeterer Kartenwechsel
ist zunaechst eine lokale Vorschau und aendert bestehende Auftraege nicht.
Fehlt eine gespeicherte Vorlage, entstehen keine Ersatzhaeuser: erst eine andere
Vorlage waehlen und speichern. Das Stoppen der Automatik bleibt moeglich.

## Mitgelieferte und eigene Vorlagen

| Datei | Anzeige | Dauerhafte Schritte / Zellen | Materialarten |
| --- | --- | --- | --- |
| `assets/buildings/templates/village/residential.json` | Dorfwohnhaus | 390 / 392 | 17 |
| `assets/buildings/templates/village/spruce-cottage.json` | Fichtenhuette | 388 / 390 | 16 |

Beide Dateien haben eigene explizite `materials`-Listen. Falsche Summen, kostenlose
Bauteile, defekte Objektpaare, doppelte IDs und nicht unterstuetzte Baugeometrie
werden abgelehnt. Die bestehenden Dorfhaus- und neuen Fichtenvarianten werden
in allen vier Ausrichtungen mit vollstaendiger Materialbilanz getestet.

Der Katalog liest rekursiv bis zu 32 Vorlagen unter `assets/buildings/`.
Unterordner erscheinen als Kategoriepfad; der alte erste Ordner `templates/` wird
dabei ausgeblendet. Nach Editor- oder Asset-Aenderungen aktualisiert ein
berechtigter Verwalter den Katalog ueber den Aktualisieren-Knopf. HUD-Abfragen
lesen nur den Cache; Dateineuladen ist serverseitig auf einmal pro zwei Sekunden
begrenzt. Eine fehlerhafte Datei wird
sichtbar gemeldet; der Server nimmt keine stillschweigende Ersatzvorlage.
Schema, Grenzen und Export: [Gebaeude-Assets](../assets/buildings/README.md).
Die gesonderte visuelle Editor-App bleibt **F82**.

Die beiden eingebauten SVG-Planvorschauen werden aus den tatsaechlichen
JSON-Zellen und Blockfarben erzeugt:

```sh
python3 tools/assets/render_building_previews.py
```

Es sind schematische Blockplaene; die feinen GLB-Formen des Spielrenderers werden
dadurch nicht ersetzt. Andere Katalogeintraege erhalten ein allgemeines Haussymbol.

## Kompatibilitaet

Client und Server verwenden **Protokoll 141**. Der **Speicherindex ID26** ergaenzt
die autorisierte Creative-Baufinanzierung. Die eingefrorenen ID24-/ID25-Decoder
uebernehmen vorhandene Siedlungen, Bauplaene, Fortschritt und Inventare; nur bei
ID24 wird die fehlende Auswahl durch die alte Standardvorlage ergaenzt. Beim
naechsten Speichern wird ID26 geschrieben.
Aeltere unterstuetzte Speicherstaende werden weiterhin geladen.

Ein bereits beauftragtes Haus speichert seinen aufgeloesten Plan und benoetigt
seine urspruengliche JSON-Datei zum Fortsetzen nicht mehr. Die Simulation arbeitet
weiterhin nur in geladenen Weltbereichen; Auftraege laden entfernte Chunks nicht
selbst nach.
