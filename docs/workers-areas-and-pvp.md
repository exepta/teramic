# Arbeitswege, Gebiete, Vorlagen und Verteidigung

## Holzfaeller und Laufwege

Ein Holzfaeller benoetigt weiterhin einen freigegebenen Auftrag, eine erreichbare
Holzfaeller-Station und ein gueltiges Ausgabelager. Axt und Setzlinge koennen im
Ausgabelager oder im besuchten Berufstisch liegen. Die Arbeitskraft holt die
wirklichen Gegenstaende dort ab. Eine Ruby-Axt funktioniert wie die anderen
geeigneten Aexte.

Das Pflanzen braucht keinen zuvor gefaellten Baum mehr. Der Holzfaeller sucht
auch in einer noch unbepflanzten Arbeitsflaeche geeigneten Boden fuer vorhandene
Setzlinge. Baumart, Mindestabstaende, bereits reservierte Pflanzstellen, Rechte,
freie Hoehe und geladene Weltzellen werden geprueft. Er geht zur Pflanzstelle,
verbraucht einen echten Setzling und registriert ihn im normalen Wachstumssystem.
Nach dem Faellen bleibt das Nachpflanzen erhalten. Der Laubdrop betraegt weiterhin
20 Prozent; es werden keine Setzlinge ohne Vorrat erzeugt.

Eine noch laufende Wegsuche ist kein blockierter Weg. Arbeitskraefte probieren
erreichbare Standseiten, waehrend der gemeinsame begrenzte Pathfinder weiter
sucht. Teilhoehen, kleine Hoehenkorrekturen und Tueren verwenden die wirkliche
Koerperkollision. Fehlende Chunks bleiben ein eigener Wartegrund. Massive Waende,
gesperrte Claims und ungueltige Arbeitsplaetze werden weiterhin respektiert.

## Gespeicherte Gebiete anzeigen

Im Arbeitsdialog beziehungsweise im Bergfried-Tab **Lokales Bauen** schaltet
**Gebiete anzeigen** die Anzeige fuer die ausgewaehlte Arbeitskraft oder Siedlung
ein. Nach dem Schliessen des Dialogs bleibt eine HUD-Legende mit Ausblendbutton.

| Farbe | Bedeutung |
| --- | --- |
| Gruen | Arbeitsgebiet des ausgewaehlten Arbeiters |
| Blau | Ausgabelager oder Stadtlagerbereich |
| Gelb | Automatisches Wohngebiet |
| Orange | Ursprung eines nicht abgebrochenen Hausauftrags |

Die Linien markieren die vollstaendigen inklusiven Blockgrenzen, auch bei
negativen Koordinaten. Sie bleiben durch Terrain erkennbar. Baupositionen sind
einzelne Ankerbloecke, keine erfundenen Hausabmessungen. Angezeigt werden nur
vom Server bestaetigte Daten; ein noch nicht gespeicherter Auswahlentwurf wird
nicht als gespeichertes Gebiet ausgegeben. Die vorhandenen HUD-Auswahlmodi
bleiben fuer Entwuerfe zustaendig. Die Anzeige aktualisiert ihre berechtigten
Metadaten in kurzen Abstaenden und endet beim Verlassen der Sitzung.

## Rezepte und echte Hausvorlage

Alle 487 endlichen Rezepte liegen unter **`assets/crafting/`**: 447 normale
Craftingrezepte in Kategorien sowie 40 Ofen-, Brecher- und Braurezepte unter
`processes/`. Die bestehenden IDs und Rezeptinhalte bleiben gleich. Die echten
Loader, eingebetteten Fallbacks und Asset-Generatoren verwenden die neuen Pfade.
Schema und Unterordner: [Rezeptverzeichnis](../assets/crafting/README.md).

Neue Wohnhausauftraege lesen jetzt wirklich
**`assets/buildings/templates/village/residential.json`**. Die vollstaendige
Beispieldatei entspricht dem bestehenden eingerichteten Dorfwohnhaus. Kosten,
zusammengehoerige Tueren/Betten und Ausrichtungen werden validiert. Fuer das
Haus benoetigter Strukturstein ist **Material-ID 43**, kein gewoehnlicher Stein
mit ID 3; die Materialliste des Auftrags ist massgeblich. Gerueste werden weiter
vom Bauplaner passend zum Standort hinzugefuegt und bezahlt.

Die Datei wird einmal je Serverprozess aus dem Assetverzeichnis geladen;
`TERRA_ASSET_DIR` kann dessen Wurzel ersetzen. Nach Aenderungen neu starten.
Ungueltige oder fehlende Vorlagen werden als Fehler gemeldet. Bestehende
gespeicherte Auftraege behalten ihren vollstaendigen Plan. Format und
Exportbefehl: [Gebaeudevorlagen](../assets/buildings/README.md).
Die separate visuelle Baueditor-App bleibt **F82**.

## Bewohner verteidigen sich

Nach einem tatsaechlich vom Server akzeptierten Treffer verfolgt der getroffene
Bewohner seinen Angreifer und wehrt sich in Nahkampfreichweite. Der Gruender
seines aktiven Koenigreichs ist ausgenommen. Nur der wirkliche eigene Koenig
erhaelt diese Ausnahme; NPC-Besitz oder der Koenigstitel in einem fremden Reich
reichen nicht.

Verteidigung verwendet reale Laufwege und Sichtlinien, Schadensabstaende,
Ruestung, Survival- und Spawn-Schutz. Zivilisten verursachen zwei, ausgeruestete
Soldaten vier Grundschaden pro erfolgreichem Schlag. Die Reaktion endet bei Tod,
Logout, zu grosser Entfernung oder abgelaufener Verfolgung. Arbeitsauftraege und
Bauinventare bleiben erhalten und pausieren waehrend der Verteidigung. Es gibt
noch keinen Gruppenalarm oder neuen Fernkampf-Befehl.

## Spieler gegen Spieler

In **`configs/server.toml`**, auf oberster Ebene vor Tabellen:

```toml
pvp = true
```

`false` verhindert Spielerschaden durch Spieler, `true` erlaubt die vorhandenen
serverautoritativen Nahkampfangriffe. Eine ausdrueckliche Servervorgabe gilt auch
fuer vorhandene Welten und wird dem Client als wirksame Regel gemeldet.
Ohne den Eintrag bleibt die gespeicherte PvP-Weltregel massgeblich. Die
mitgelieferte Serverkonfiguration aktiviert PvP ausdruecklich. Aenderungen an der
Datei benoetigen einen Serverneustart. Eine ausdrueckliche Servervorgabe laesst
sich nicht widerspruechlich per `/gamerule pvp` ueberschreiben.

Der Server prueft Zielrichtung, Reichweite, Hindernisse, geladene Weltzellen,
Aktionsabstand, Ausdauer, Spielmodus, Ruestung und Spawn-Schutz. Ein Client kann
keinen Schadenswert festlegen. Kreativ-/Zuschauerschutz bleibt bestehen.
Die Einstellung steuert Spieler-gegen-Spieler-Schaden; die Abwehr eines
angegriffenen Bewohners ist NPC-Kampf.

## Grenzen

Die Simulation arbeitet weiterhin nur in geladenen Weltbereichen. Auftraege
halten keine entfernten Chunks dauerhaft geladen. Protokoll **140** und
Speicherformat **ID25** ergaenzen die Vorlagenauswahl; ID24 wird migriert. Die neue Verteidigung und die
Gebietsanzeige sind voruebergehender Laufzeitzustand. Die bestaetigten Arbeits-,
Lager- und Wohngebiete bleiben gespeichert.

Die Abnahme der Phase 239 umfasst 4.843 bestandene Tests, striktes Clippy,
getrennte Builds, Headless-Start und native DE/EN-Pruefungen fuer Hausbau,
Gebietsanzeige und direkte NPC-Arbeit.
[Protokolle, Aufnahmen und bekannte externe Testdatei](../tools/benchmarks/results/workers-areas-defense-2026-11-12/README.md).
