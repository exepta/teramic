# Arbeitskraefte, Patrouillen Und Kingdom-Bedienung

## Arbeit Direkt Am NPC

Eine zivile Arbeitskraft anvisieren und mit **Rechtsklick** oeffnen. Die vorhandene
Taste **Benutzen** funktioniert ebenfalls. Der Direktdialog zeigt ihren Namen,
den aktuellen Auftrag, Status und neun Inventarplaetze. Die Inventaransicht ist
lesend; Werkzeuge, Saatgut und Ertraege werden ueber das Auftragslager versorgt.
Inhalte stammen aus einer privaten Serverantwort. Die Nahinteraktion verlangt
passende Besitz-/Claimrechte und hoechstens acht Meter Entfernung. Eigene NPCs im
selben Reich lassen sich ausserdem in der Bergfried-Arbeitsliste auswaehlen und
einschliesslich ihres privaten Inventars verwalten, solange der Spieler im eigenen
beanspruchten Gebiet steht. Fremde Arbeitskraefte erhalten dadurch keine entfernte
Inventarfreigabe. Werkzeuge und Vorrat werden weiterhin in wirkliche Lager gelegt.

Beruf und Prioritaet waehlen. **Arbeitsgebiet in der Welt waehlen** schliesst den
Dialog und aktiviert den sichtbaren HUD-Auswahlmodus. Zwei Linksklicks auf
geladene Bloecke setzen gegenueberliegende Ecken. Die Reihenfolge spielt keine
Rolle; negative Koordinaten funktionieren. Die maximale Groesse bleibt
32 x 16 x 32 Bloecke. Bergarbeiter verwenden die exakt angeklickte Box.
Landwirte, Sammler und Holzfaeller schliessen die Ebene unmittelbar ueber dem
Boden mit ein, damit oberirdische Arbeitsziele bei der Bodenwahl enthalten sind.
Die tatsaechlichen Koordinaten erscheinen danach wieder im Editor.

**Lager in der Welt waehlen** aktiviert denselben HUD-Modus fuer einen einzelnen
Lagerblock. Eine normale Truhe, ein weltplatzierter Frachtlagerblock oder eine
zum Beruf passende Station ist zulaessig. Bei einer Doppeltruhe wird die
Hauptseite verwendet. Der Server prueft Rechte, Reichweite und Lagerzustand bei
der eigentlichen Zuweisung erneut.

Im Auswahlmodus bleiben Laufen und Umschauen aktiv. Abbau, Platzierung und
sonstige Weltaktionen sind gesperrt. **ESC** bricht den begonnenen Auswahlvorgang
ab und kehrt mit erhaltenem Entwurf zum Dialog zurueck. Erst **Auftrag speichern**
sendet die Zuweisung. **Aktiviert / Pausiert** verwendet die vorhandene
revisionsgepruefte Serveraktion. Die numerischen Felder bleiben zur Feinjustierung
verfuegbar. Bisherige Auftraege lassen sich weiterhin im Bergfried verwalten.

Auftragsdaten sind im normalen begrenzten Bereich bis 128 Meter verfuegbar; eigene
Arbeitskraefte im selben Reich bleiben ueber den berechtigten Bergfriedzugriff
erreichbar. Ohne diese Gebietsberechtigung bleibt fuer private Inventare nur die
nahe Interaktion mit geladener NPC-Zelle. Tod, Teleport, fehlende Rechte und
Besitzwechsel entziehen die Freigabe. Auch bei einer vollen
128er-Arbeitsliste wird der direkt angeklickte berechtigte NPC beruecksichtigt.

## Kingdom Mit K Oeffnen

Mit **K** oeffnet sich der Bergfried des eigenen Reichs, solange der Spieler in
dessen beanspruchtem Bau- oder Territoriumsgebiet steht. Dorf-/Stadtclaims mit
einem uebergeordneten Kingdom oeffnen dessen Bergfried. Der Bergfried braucht
dafuer weder in acht Metern Entfernung noch lokal geladen zu sein.

Der Server prueft aktuelle Mitgliedschaft und die tatsaechliche Spielerposition.
Die vorhandenen Rollenrechte gelten fuer jede Aktion weiter. Nach Verlassen
des Gebiets schliesst der entfernte Dialog; die gewohnte nahe Interaktion am
Bergfried bleibt verfuegbar. Chat, geoeffnete Dialoge und die NPC-Weltauswahl
fangen ihre Eingaben zuerst ab.

Die Taste ist unter Steuerung umbelegbar. Alte Tastenprofile erhalten die neue
Belegung, ohne eine selbst belegte K-Taste zu ueberschreiben; in diesem Fall
waehlt die Migration eine freie Ersatztaste.

Der zusaetzliche Tab **Lokales Bauen / Building Local** verwaltet Handwerker,
Stadtlager und Hausauftraege. Diese Arbeitskraefte brauchen kein eigenes
Arbeitsgebiet. Sie holen vorhandenes Material aus den Truhen des ausgewaehlten
Stadtlagerbereichs und bauen das Dorfwohnhaus sichtbar in Phasen auf.
[Bedienung, Bauzeiten und Grenzen](settlement-building.md).

## Automatische Soldatenpatrouillen

Eigene Schwert-, Bogen- und Lanzensoldaten ohne expliziten Auftrag patrouillieren
automatisch in geladenen **Build-Chunks** ihres Reichs. Dazu gehoeren auch
zugehoerige Dorf-/Stadt-Bauclaims. Reine Territoriumsclaims sind keine
Patrouillenflaechen. Die Startsoldaten aus F80 benoetigen keine generierte
Bewohnerstelle oder ein Bett fuer ihre Patrouille.

Ziele wechseln mit kleinen Pausen. Bewegung, Tueren, Koerperbreite und
Ausweichbewegungen verwenden dieselben physischen Grenzen und Rechte. Nach
Claimfreigabe, Aufloesung oder Unload wird angehalten und bei wieder gueltiger
Umgebung begrenzt neu geplant. Soldaten mit einem vorhandenen gueltigen Bett
behalten Versorgung und Schlafschichten. Die vorhandenen expliziten Auftraege
haben Vorrang. Neue militaerische Befehlsoberflaechen gehoeren nicht zu dieser
Patrouillenergaenzung.

Auch eigene untatige zivile Villager wandern tagsueber innerhalb geladener
Build-Chunks ihres Reichs. Arbeiter ohne Arbeitsziele duerfen sich bewegen,
waehrend weiter nach passenden Zielen gesucht wird. Konkrete Arbeitsauftraege,
Baustellen und bestehende Schlafschichten behalten Vorrang.

## Beleuchtung Und Dialoglayout

Leere Himmels-Sections verbrauchen nicht mehr das Entdeckungsbudget vor
entfernten belegten Lampen-Sections. Eine einzelne gebatchte, tiefengepruefte
Darstellung haelt geladene Dorflichtquellen bis zur Renderreichweite sichtbar.
Ein weicher Uebergang verbindet sie mit den nahen Flammen. Der vorhandene Pool
echter Nahlichter und Schatten behaelt sein Budget.

Der Kingdom-Dialog nutzt mehr Fensterflaeche bis 1480 x 1120 mit scrollbarem
Inhalt und sicherem Abstand zur dekorativen Umrandung. Betroffene Aktionsleisten
koennen umbrechen. Aktionsbuttons ordnen Symbol und Text horizontal an und wachsen mit ihrer
Beschriftung. Ihre Zeilen umbrechen bei Platzmangel; lange Beschreibungen
behalten Zeichenumbruch fuer deutsche Komposita. Eingabefelder, Dropdownwerte und
Inventarmengen behalten ihre eigene Darstellung. Alle neuen Texte sind in den
deutschen und englischen Fluent-Katalogen enthalten.

## Technik Und Abnahme

Client und Server verwenden gemeinsam **Protokoll 140**. Protokoll 137 fuehrte
private NPC-Inspektionsdaten ein; 138 ergaenzte den Stations-Wartezustand und
139 die getrennten Bauauftraege; 140 ergaenzt den geprueften Vorlagenkatalog. **ID25** speichert Stadtlagerbereiche, Bauplaene,
Handwerkerlisten und ihre mitgefuehrten Materialien und liest ID23 sowie aeltere
Spielstaende weiterhin.
Zufriedenheit und weitere Villager-/Kingdom-Statistiken stehen als **F81** in
der TODO.

Der kurze native Test arbeitet mit eigenen Konfigurations-, Profil- und
Weltordnern. Er prueft die Kingdom-Taste, Rechtsklick-Targeting, private Inventarplaetze, Auswahl
und Abbruch ohne Weltedits, echte Serverzuweisung/Pause und deutsche/englische
Dialoge bei 1600 x 900 sowie 960 x 640. Geometriepruefungen sichern den Abstand
zum Rahmen, die volle Editorbreite und passende Aktionsbuttons:

```sh
cargo build --locked -p terra-client -p terra-server --bins
bash tools/benchmarks/run-npc-direct.sh
```

Den nativen Test nach abgeschlossener Kompilierung starten. Die Galerie nutzt
echte Welt-Raycasts, framegesteuerte Gameplay-/HTML-Eingaben und den lokalen
UDP-Server. Die Screenshots und das Pruefergebnis liegen unter
`target/npc-work-direct/`.

Die erste Fassung (Phase 236) wurde mit 4750 erfolgreichen Workspace-Tests, nativer DE/EN-Galerie,
Client-/Server-Build und Headless-Start abgeschlossen:
[Pruefbericht und ausgewaehlte Aufnahmen](../tools/benchmarks/results/npc-work-patrol-ui-2026-11-12/README.md).

Die nachfolgende Erweiterung verlangt beim Holzfaeller eine echte Station vor
dem Berufswechsel und verwendet 20-%-Setzlingsbeute aus Laub:
[Holzfaeller, Stationen und Setzlinge](lumberjack-stations-and-drops.md).

Die Phase-238-Gesamtabnahme ist noch offen. Fuer den neuen Bauablauf steht der
isolierte native Lauf `bash tools/benchmarks/run-building-local.sh` bereit;
derzeit wird dafuer kein bestandener Lauf behauptet.
