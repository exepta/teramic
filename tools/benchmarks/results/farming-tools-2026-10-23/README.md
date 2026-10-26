# Farming Tools And HUD Corrections

Phase 110 erweitert F04 um Giesse, Hacke, Sense und eine dekorative Vogelscheuche.
Die vorhandenen Prop-, Werkzeug-, Inventar-, Rezept- und Netzwerkpfade bleiben
gemeinsam; neue Tool-/Prop-Tags sind angehaengt. Weltformat 2, Generator 5,
PLY7 und RGA2 bleiben erhalten. Netzwerkprotokoll 62 erfordert gemeinsam
aktualisierte Clients und Server.

## Bedienung Und Regeln

Alle drei Werkzeuge verwenden die konfigurierte Platzieren-/Benutzen-Taste.
Die Hacke bereitet freie Gras-/Erdflaechen als Ackerboden vor. Saaten behalten
ihre bisherige automatische Bodenvorbereitung.

Giessekannen werden durch Benutzen einer echten Wasserzelle aufgefuellt.
Die begrenzte Sichtstrahl-Abfrage durchdringt weder Waende noch ungeladene
Sections. Bewaessern setzt die Feuchtigkeit einer Kultur auf voll und
verbraucht eine Einheit. Die Feuchtigkeit bleibt gespeichert, ohne unnoetige
Terrain-Meshes zu erneuern. Der Wasservorrat gehoert derzeit zur Spielersitzung,
nicht zum einzelnen Item; nach Verbindungsende muss neu aufgefuellt werden.

| Material | Wasserreserve | Sensenflaeche |
| --- | --- | --- |
| Holz | 8 | 3x3 |
| Eisen | 12 | 3x3 |
| Gold | 16 | 3x3 |
| Diamant | 20 | 5x5 |
| Rubin | 24 | 5x5 |

Die Sense erntet ausschliesslich reife Kulturen auf derselben Bodenhoehe.
Erreichbarkeit und Sicht auf jedes Feld werden serverseitig geprueft.
Drop-Kapazitaet und Epochen werden fuer die gesamte Aktion vorab reserviert;
ueberholte Inventar-/Weltanfragen oder zu schnelle Wiederholungen werden
abgewiesen. Die lokale Figur und das Ego-Werkzeug erhalten eine seitliche
Schnittbewegung. Die Kanne kippt beim Benutzen um ihren Griff.

15 einzelne Werkzeugdefinitionen und 16 Rezepte liegen in den bestehenden
Item-/Crafting-Katalogen und erscheinen in Too Many Items. Die Vogelscheuche
belegt einen zentrierten Prop-Platz, benoetigt einen vollstaendigen Bodenblock
und zwei trockene, freie Zellen Hoehe. Sie richtet sich beim Platzieren in
Vierteldrehungen aus. Halbe Platten, Treppen und Moebel sind kein gueltiger
Sockel; fliessendes Wasser verhindert ebenfalls das Platzieren. Sie ist fuer spaetere
Vogel-Abwehr vorbereitet, besitzt aber noch keine Vogel-KI. Die Laterne ist
ein optischer Modellbestandteil, keine zusaetzliche dynamische Lichtquelle.

## Darstellung

Gemeinsame solide Modelle zeigen materialabhaengige Giessekannen, gebogene
Hacken und Sensen. Hacken sind 1,25 m, Sensen 1,5 m und Kannen 0,85 m gross;
die Ego-Ansicht verwendet weiterhin eine begrenzte Darstellungs-Skalierung.
Die Arbeitskante beziehungsweise der Ausguss zeigen in die Spielwelt und
nicht quer als breite Profilseite zur Kamera. Griffpunkte und Nah-Clipping
bleiben abgesichert.

Farmland verwendet eigene kachelbare 512px-Texturen fuer Boden und Seiten:
warme Erdklumpen, graue Steinchen und sparsame Wurzeln. Drei 75-mm-Furchen
verwenden die bestehende gemeinsame Cube-/Oberflaechenpipeline. Logische
Blockgroesse und bestehende Block-IDs aendern sich nicht.

Die Strohfigur besitzt breiten Hut, Blume, Gesicht, roten Schal, Stoffflicken,
gebundene Arme, Strohsaum und Laterne. Drei LODs, praezises Mesh-Picking und ein
geteilter Vertexfarben-Materialbatch vermeiden Einzel-Entities fuer Details.

## HUD Und Inventar

Pflanzen zeigen ihren tatsaechlichen Wachstumsfortschritt als Prozentbalken.
Die Autoritaet sendet einmal pro Sekunde maximal 256 nahe Datensaetze; die
sichtbare Modellstufe bleibt davon getrennt. Der Jahreszeitenzeiger verwendet
eine gefuellte silberne Pfeilspitze mit dunkelsilbernem Rand.

Strg + Linksklick auf die Creative-Muelltonne leert persoenliche Hotbar,
Inventar, Cursor, Ausruestung und Crafting-Reservierungen. Es entstehen keine
Drops, fremde Spieler und Weltcontainer bleiben unveraendert. Der Server
prueft Creative-Modus und Inventarrevision; Survival kann die Aktion nicht
ausfuehren.

Der CreatureStates-Receiver leert ankommende Pakete auch vor dem vollstaendigen
Weltbeitritt und verarbeitet nur die aktuelle Verbindung. Dadurch bleibt
keine unbehandelte Queue fuer Lightyears Last-Cleanup liegen; die Warnung
wird nicht mit einem Logfilter verborgen.

## Native Funktionspruefung

Auf der RX 6950 XT mit Vulkan wurden zwei kurze Clientlaeufe tatsaechlich
ausgefuehrt. Dies sind Funktions- und Bildtests, keine FPS-Benchmarks:

- `--smoke-test-farming`: integrierter Survival-Server, drei Saatarten,
  Wachstum, manuelle Ernte, Umgraben, Wasser aus einer echten Zelle nachfuellen,
  Giessen, drei reife Kulturen mit einer Sensenaktion ernten, unreife Kulturen
  erhalten, Vogelscheuche platzieren und Wachstumsbalken anzeigen. Anschliessend
  Spielstand neu gelesen: 29 Kulturen und die Vogelscheuche korrekt erhalten.
- `--smoke-test-hands`: 16 Ansichten einschliesslich Hacke, Kanne, Sense,
  Schnitt-/Giessbewegung und Third-Person-Griff. Keine First-Person-Arme.
- `terra-server --smoke-test`: Dedicated Server startet weiterhin headless.

Alle drei Laeufe endeten erfolgreich. Ihre Logs enthalten keine Warnungen,
Renderfehler oder Panics, insbesondere keine unbehandelten CreatureStates.
Die Bilder wurden geoeffnet und kontrolliert; der Pflanzen-HUD zeigt zum
Beispiel einen echten Wachstumswert von 66 Prozent.

Nachweise: [Farming-Log](logs/native-farming.log),
[Werkzeug-Log](logs/native-hands.log), [Server-Log](logs/native-server.log).
Ansichten: [Acker und Wachstumsbalken](native/farming/farming-growth-hud.png),
[Vogelscheuche und Jahreszeitenzeiger](native/farming/farming-scarecrow.png),
[Sense gehalten](native/hands/hands-11.png),
[Sensenbewegung](native/hands/hands-12.png),
[Giessbewegung](native/hands/hands-13.png).

Reproduktion mit getrenntem Testspielstand und optionalem Capture-Verzeichnis:

```bash
TERRA_FARMING_SMOKE_DIR=target/phase110/farm-clean \
TERRA_CAPTURE_DIR=target/phase110/farm-captures \
target/debug/terra-client --smoke-test-farming
TERRA_CAPTURE_DIR=target/phase110/hand-captures \
target/debug/terra-client --smoke-test-hands
target/debug/terra-server --smoke-test
```

Einen frischen Pfad fuer `TERRA_FARMING_SMOKE_DIR` verwenden. Der Test nutzt
eine flache isolierte Welt; normale Spielerwelten werden nicht umgebaut.

## Automatisierte Pruefungen

Die abschliessende Workspace-Suite besteht mit 1.938 erfolgreichen Tests,
keinem Fehler und drei bereits explizit ignorierten Tests (Audioexport,
180-s-UDP-Soak, opt-in Wasser-Microbenchmark). Der ausfuehrliche Lauf steht in
[tests-final.log](logs/tests-final.log).

Neue beziehungsweise erweiterte Regressionen pruefen unter anderem:

- Alle 15 Werkzeuge mit genau einem Katalogeintrag/Rezept; kanonische 2x3-Hacken.
- Echte Inventarquellen, veraltete Revisionen, Aktionsabstand und freie Boeden.
- Wasser-DDA mit negativen Koordinaten, Waenden, ungeladenen Sections,
  endlichem abfliessendem Wasser und ungueltigen Blickrichtungen.
- Atomare Sensen-Ernte, reife/unreife Kulturen, Sicht und begrenzte Drop-Slots.
- Trockene volle Sockel fuer Vogelscheuchen, auch bei angepassten JSON-Formen.
- Kreatives Gesamtleeren mit Cursor/Ausruestung, Ablehnung in Survival,
  fremde Spieler und Drops unveraendert; Crafting-Reservierungen korrekt geleert.
- Echter UDP-Beitritt mit CreatureStates vor Ready, ohne unbehandelte Pakete.
- Gueltige Modelle/Indizes, drei Vogelscheuchen-LODs, matte Farmland-Texturen,
  Griffpunkte, vorwaerts gerichtete Werkzeugkanten und Animationen.

`cargo fmt --all --check`, `cargo check --locked --workspace --all-targets`,
`cargo clippy --locked --workspace --all-targets -- -D warnings`,
`cargo build --locked --workspace` und `git diff --check` sind ebenfalls
erfolgreich. Logs: [Check](logs/check.log), [Clippy](logs/clippy.log),
[Build](logs/build.log).

## Grenzen

Die Referenzen bestimmen Form und Materialrichtung; die nativen Modelle sind
keine pixelidentischen Kopien der Illustrationen. Die Wasserreserve ist
sitzungsgebunden. Vogel-KI und eine echte Lichtquelle an der Vogelscheuche
sind nicht implementiert. Kurze Funktionspruefungen liefern keine allgemeine
FPS-/Speicherparitaet; keine langen Soak-Tests gemaess Nutzerwunsch.
