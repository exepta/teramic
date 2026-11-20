# Handwerker, Stadtlager Und Lokaler Hausbau

## Holzfaeller Und Untatige Villager

Eine besuchte Holzfaeller-Station ist jetzt auch eine echte Versorgungsquelle.
Fehlen Axt oder passende Setzlinge im zugewiesenen Ausgabelager, laeuft der NPC
zum Tisch und entnimmt sie dort. Ertraege gehen weiterhin in das gewaehlte
Ausgabelager. Vorrat, Weg, Rechte und Transportgrenzen werden tatsaechlich
geprueft. Ein unbegonnener, dauerhaft unerreichbarer Baum bekommt eine begrenzte
Wiederholpause, damit andere Baeume im Arbeitsgebiet nicht blockiert werden.

Alle eigenen untatigen zivilen Villager bewegen sich tagsueber innerhalb der
geladenen Build-Claims ihres Reichs. Auch ein Arbeiter ohne Arbeitsziele darf
umhergehen, waehrend die begrenzte Zielsuche weiterlaeuft. Laufende Arbeit,
Baustellen, explizite Befehle und die bestehende Bett-/Nachtlogik behalten Vorrang.
Territoriumsclaims sind keine automatischen Wandergebiete.

Im Bergfried-Tab **Arbeitsauftraege** kann eine Arbeitskraft ausgewaehlt und ihr
privates Inventar angesehen werden. Ein eigener NPC im selben Reich bleibt
auch aus der Entfernung erreichbar, solange sich der Spieler im eigenen
beanspruchten Gebiet befindet. Inventare bleiben lesend; Versorgung erfolgt
ueber die wirklichen Lager. Fremde NPCs und nicht mehr zugehoerige Gebiete
erhalten dadurch keine Inventarfreigabe.

## Hausbau Einrichten

1. Den eigenen Bergfried oeffnen, etwa mit **K**, und **Lokales Bauen** waehlen.
   Auf Englisch heisst der Tab **Building Local**.
2. Eigene zivile NPCs als **Handwerker** anmelden. Das geht im Bau-Tab oder ueber
   die Berufsauswahl der Arbeitskraft. Ein Handwerker benoetigt kein eigenes
   Arbeitsgebiet. Ein laufender Arbeitsauftrag muss zuerst pausiert und ein
   begonnener Baumauftrag abgeschlossen werden. Mitgefuehrte Gegenstaende duerfen
   bei einem Berufswechsel nicht verloren gehen.
3. **Lagerbereich waehlen**, zwei Ecken in der Welt markieren und **Stadtlager
   speichern**. In diesem Bereich normale Truhen platzieren und mit Baumaterial
   fuellen. Der Bereich selbst erfindet weder Truhen noch Inhalte.
4. **Hausstandort waehlen**, einen freien Bodenplatz in eigenen Build-Chunks
   anklicken und **Wohnhaus beauftragen**. Die Materialliste am Auftrag zeigt
   die tatsaechlich noch benoetigten Materialien einschliesslich Geruestholz.

Alternativ ein **automatisches Wohngebiet** mit zwei Ecken waehlen und aktivieren.
Die Handwerker suchen dort geeignete freie Hausplaetze. Automatik stoppen
beendet die weitere Planung; bereits bestehende Auftraege bleiben separat
verwaltbar. Bauplaetze muessen geladen, trocken, ausreichend frei und einschliesslich
Arbeitsgeruest im eigenen Build-Gebiet liegen. Vorhandene Baeume, Truhen und
Spielerbauten werden nicht stillschweigend fuer einen Bauplatz geloescht.

## Tatsaechlicher Bauablauf

Die erste Vorlage ist das vorhandene eingerichtete Dorfwohnhaus. Neue Auftraege
lesen die echte Datei `assets/buildings/templates/village/residential.json`.
Der gemeinsame Dorfhaus-Generator dient nur noch als Export- und Testreferenz.
Gefuellte Beutetruhen oder neue Bewohner werden dabei nicht verschenkt.
Format und Export: [Gebaeudevorlagen](../assets/buildings/README.md).

Handwerker gehen zu den echten Lagertruhen, tragen begrenzte Materialmengen
im eigenen Inventar und platzieren wirkliche Bloecke. Fundamentvorbereitung,
Boden, Waende, Dach und Einrichtung folgen Bauphasen. Fuer hoeher gelegene
Arbeit entstehen bezahlte temporaere Aufstiege und Arbeitsplattformen. Bei
deren Rueckbau gehen die vorhandenen Holzmaterialien zurueck in die Versorgung.
Fuer das Dach entsteht auch innen ein Arbeitsdeck mit offenem Treppenaufgang;
die Einrichtung folgt nach dessen Rueckbau.
Handwerker tragen das zurueckgewonnene Geruestholz zu den wirklichen Stadtlagertruhen;
bei fehlendem Platz bleibt der Vorrat erhalten und die Arbeit wartet.
Tueren und Betten behalten ihre zusammengehoerigen Weltzustaende.

Material wird erst bei erfolgreichem Bauschritt verbraucht. Vorhandene Koerper,
ungueltige Rechte, belegte Plaetze, ungeladene Sections, volle Lager und
fehlende Materialien halten die betreffende Arbeit auf. Es gibt keine
entfernte Komplettplatzierung eines fertigen Hauses. Fortschritt und Wartegrund
stehen am Bauauftrag; Pausieren und Abbrechen sind revisionsgepruefte
Serveraktionen. Ein Abbruch entfernt keine bereits gebauten Hausteile.

## Wege und Geländevorbereitung

Terrassen werden von oben geöffnet. Ist die nächste Aushubzelle von einem
noch vorhandenen Steilrand verdeckt, bearbeitet der Handwerker eine andere
bereits geplante, sichtbare Zelle derselben Aushubphase. Dadurch legt er sich
seinen Zugang tatsächlich frei. Sichtprüfung, Handreichweite, Bodenkontakt,
Körperkollision und Gebietsrechte gelten auch für Zwischenpositionen zwischen
Voxelmitten. Es gibt keine Teleportation durch Hindernisse.

Beim Fortsetzen älterer Aufträge wird ausschließlich die Reihenfolge noch
offener kostenloser Erdarbeiten korrigiert. Fertige Arbeit und bezahlte
Bauschritte werden nicht wiederholt. Liegt an einer bereits von diesem Auftrag
ausgehobenen Stelle erneut Gras oder Erde, räumt der Handwerker sie vor der
geplanten Blockplatzierung mit zusätzlicher echter Arbeitszeit weg. Neue
Steinblöcke, Truhen und andere Objekte bleiben Hindernisse. Bereits vom Spieler
freigeräumte Aushubzellen werden ohne zusätzlichen Material- oder Arbeitszeitgewinn
übersprungen.

Fehlt ein erreichbarer Arbeitsstandpunkt, zeigt der Auftrag den konkreten
Wegfehler an. Die Suche wird weiter regelmäßig wiederholt; nach Entfernen
eines Hindernisses kann der Handwerker selbstständig fortsetzen. Das gilt
auch für den Weg zum Materiallager und zum Abladen des Gerüstholzes.

## Echtzeit Und Mannschaften

Pro Haus arbeiten hoechstens vier Handwerker. Ein NPC gehoert gleichzeitig
hoechstens einer Baustelle an. Die Zeiten beziehen sich auf aktive Arbeit
mit tatsaechlich anwesenden Handwerkern und vorhandenen Materialien:

| Anwesende Handwerker | Aktive Echtzeit pro Haus |
| --- | --- |
| 1 | 30 Minuten |
| 2 | 15 Minuten |
| 3 | 10 Minuten |
| 4 | 5 Minuten |

Laufwege, Materialholen, fehlender Vorrat und Blockaden verlaengern die gesamte
Auftragsdauer. Tageszeitbeschleunigung schenkt keinen Baufortschritt.
Speichern und Laden erhalten den verdienten Fortschritt und die echten
Materialinventare; Offline-Zeit wird nicht als Bauarbeit angerechnet.

## Technik Und Grenzen

Client und Server verwenden gemeinsam **Protokoll 140**. Bauplaene, Fortschritt,
Stadtlagerbereiche, Handwerkerlisten und mitgefuehrte Materialien werden in
**ID25** gespeichert. ID24 erhaelt die bisherige Standardvorlage als neue Auswahl;
ID23 und aeltere Dateien werden ohne erfundene Bauauftraege
weiter geladen. Clients erhalten schlanke Bauanzeigen statt saemtlicher
persistierter Voxelplaene oder privater Stadtlagerinventare.

Bereiche sind auf 64 x 32 x 64 begrenzt. Alle maximal 32 aktiven Projekte
erhalten pro Simulationstick die volle verstrichene Zeit; weitere Baustellen
verlangsamen die Arbeitsuhr eines bereits vor Ort arbeitenden Teams nicht. Es gibt hoechstens 32 gespeicherte
Bauauftraege insgesamt, 128 Siedlungseintraege und 32 registrierte Handwerker pro
Siedlung. Auch abgeschlossene oder abgebrochene Auftraege belegen einen gespeicherten
Auftragsplatz. Die erste Fassung
verwendet eine feste Dorfwohnhaus-Vorlage; weitere Gebaeudearten und die
separate visuelle JSON-Editor-App stehen als **F82** in der TODO.

Die Simulation arbeitet wie die bisherigen NPC-Auftraege nur in geladenen
Weltbereichen. Ein Bauauftrag haelt entfernte Chunks nicht selbst geladen.
Verlaesst der Spieler die Simulationsreichweite, wartet die Baustelle bei Unload;
nach erneutem Laden setzt sie mit erhaltenen Materialien und Bauschritten fort.

## Validierungsstand Der Phase 238

4800 Library-/Binary-Tests, vier Integrationstests, die Dokumentationstest-Ziele,
Cargo-Check, Formatierung und Clippy mit `-D warnings` sind erfolgreich.
Zehn bestehende Tests bleiben ignoriert; der bekannte Original-GLB-Test ist wegen
der fehlenden externen Datei `FIVERR/VoxelGrassBlock glb file.glb` ausgefiltert.
Alle vier Hausausrichtungen bauen in den Konstruktionstests jeweils 832 Schritte
vollstaendig mit richtiger Materialbilanz und abschliessend leeren Taschen.
Client und Server wurden separat gebaut; der Headless-Server startet ohne Display.
Die Nachweise der Phasen 235 bis 237 bleiben historische Pruefstaende.
Der isolierte native Bauablauf laesst sich so wiederholen:

```sh
cargo build --locked -p terra-client -p terra-server --bins
bash tools/benchmarks/run-building-local.sh
```

Der bestandene Lauf verwendet einen echten lokalen Server und temporaere
Welt-/Profilordner unter `target/building-local/`. Geprueft wurden reale
Truhenversorgung, Welt-Auswahl, Build-Claim, Hausauftrag, Handwerkerregistrierung,
die erste tatsaechliche Blockplatzierung und bestaetigte Pause.
Fuenf DE/EN-Aufnahmen pruefen die breite und kompakte UI.
Die gepruefte Versorgung erfolgte mit 128 Stein und 128 Eichenholzbrettern aus
echten Inventartransfers. Reine Aushub-Edits reichen nicht als Baunachweis.
Der kurze Grafiklauf prueft den Beginn der Arbeit; die CPU-Tests pruefen den
vollstaendigen Hausbau und die Echtzeitraten. Protokolle, Aufnahmen und genaue
Testbefehle: [Abnahme Phase 238](../tools/benchmarks/results/settlement-building-2026-11-12/README.md).
