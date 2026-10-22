# Phase 89: Ofen, Oeffnungen, Rauch und Itembrowser

Stand 2026-10-20. Implementiert und mit Workspace- sowie nativen Pruefungen
validiert. Langzeitlast und menschliche Hoerabnahme bleiben separat offen.

## Aenderungen

- Die Inventar-Vorabblockierung erfasst nur Inventar, Truhe, Werkbank und Ofen.
  Tueren und Fenster behalten den Gameplay-Cursor beim Umschalten.
- Die Serverpruefung testet die neue Fluegelposition sowohl beim Oeffnen als auch
  beim Schliessen gegen Spielerkoerper.
- Die Ofen-Querbaender und Dachkurse besitzen getrennte Flaechen statt koplanarer
  Ueberlagerungen mit Mauerwerk und Bogen. Die vorhandenen Modelle werden
  reproduzierbar gebaut.
- Vier passende Fuellelemente schliessen gerade Rohrdurchfuehrungen durch eine
  vollstaendig umgebende Wand oder Decke. Jede Seite uebernimmt ihr eigenes
  angrenzendes opakes Blockmaterial. Das gilt auch fuer den integrierten oberen
  Ofenblock. Der Rohrkanal bleibt offen; Auswahl und Kollision nutzen dieselben
  Fuellelemente. Die Geometrie landet in vorhandenen Section-Materialbatches.
- Offene Fenster und freie Raumoeffnungen erhalten sichtbare, nach oben
  abziehende Rauchfahnen. Die bestehende Raum-/Himmelspruefung bestimmt die
  Auslaesse; die lokale Darstellung cached hoechstens acht Auslaesse bei 2 Hz
  aus maximal vier nahen Rauchquellen. Keine synchrone Terrain-Generierung.
- Raeumliche Oeffnungs-/Schliesssounds, ein separater leiser Ofenloop und
  Husten bei Koerperkontakt mit replizierten Rauchzellen verwenden den
  bestehenden Audiomixer und dessen Lautstaerke-/Unterwasserregeln.
  Erstkontakt hustet nach 1,4 s, danach mit 6,5..8,5 s Abstand; Verlassen des
  Rauchs beendet den Zyklus. Quellen/Verarbeitung: `assets/audio/CREDITS.md`,
  `tools/assets/prepare89-furniture-audio.sh`.
- Die Ofenoberflaeche verwendet Sloticons und Balken ohne Slotlabels und
  Prozenttext. Der vorhandene Slot-Tooltip bleibt verfuegbar.
- Der Itembrowser zeigt zuerst All. Bei zu wenig gemessenem Platz werden alle
  Kategorien ueber dieselbe TiltUI-ChoiceBox ausgewaehlt. Eigene Kategorien und
  Such-/Inventarzustand bleiben erhalten.

## Validierung

Alle folgenden Befehle wurden erfolgreich ausgefuehrt, mit unveraendertem Lockfile:

| Pruefung | Ergebnis / Nachweis |
| --- | --- |
| `cargo fmt --all --check` | bestanden, `fmt.log` |
| `cargo check --locked --workspace` | bestanden, `check.log` |
| `cargo test --locked --workspace --no-fail-fast` | 1449 bestanden, 0 fehlgeschlagen, 1 ignoriert, `tests-accepted.log` |
| `cargo clippy --locked --workspace --all-targets -- -D warnings` | bestanden, `clippy.log` |
| `cargo build --locked --workspace` | bestanden, `build-accepted.log` |
| Nativer Integrated-Server-/TiltUI-Lauf | 13 Aufnahmen, `native-gameplay-accepted.log` |
| Native Modell-/Shaderansichten | 12 Aufnahmen, `native-art.log` |
| Dedicated Server ohne Display | 27 Sections, sauber beendet, `headless.log` |

Der vorhandene dreiminuetige UDP-Soak ist standardmaessig ignoriert und wurde
in dieser Phase nicht erneut ausgefuehrt. Die finalen nativen Logs enthalten
keine ERROR-, Panic- oder ValidationError-Meldungen.

Neue Regressionen pruefen reale Avian-Kapselbewegung fuer sechs Bauteile in
allen vier Ausrichtungen, Cursorblockierung, blockiertes Oeffnen/Schliessen,
Rauch-Auslaesse, Rohrfuellungen und responsive Kategorien. Die Kapseltests
verwenden feste Zeitschritte und pruefen auch, dass Fenster tatsaechlich
kollidieren, bevor sich der Koerper wieder zurueckbewegen kann.

Der native Ofentest prueft breite/kompakte Layouts, verborgene Dropdownoptionen,
Kategorieauswahl, Ofenablauf, Oeffnungsanimation, Creative-R und Save/Reload.
Er darf Cursorverlust in den Oeffnungsstufen nicht durch automatische
Wiederaufnahme verdecken. Die Modellfixture besitzt zwei weitere Ansichten
fuer gemischte Rohr-/Ofen-Dachdurchfuehrungen. Kompakte Ofenoberflaeche und diese
Dachansichten wurden tatsaechlich als Bilder kontrolliert:
[kompakte UI](captures/furnace-running-compact.png),
[Rohr im Dach](captures/furnaces-10.png),
[integrierter Ofen im Dach](captures/furnaces-11.png).

Fruehere Laeufe bleiben nachvollziehbar: `native-gameplay.log` scheiterte an
einer Fixture-Frist nach OS-Fenstergroessenaenderung; die Wiederaufnahmefrist
beginnt jetzt beim tatsaechlichen Capture-Versuch. `native-gameplay-final.log`
bestand, zeigte aber in der Bildpruefung danebenliegende Dropdownoptionen.
Ein Template-Wrapper im ChoiceBox-Popup und strengere Laufzeitpruefungen
beheben dies im finalen `native-gameplay-accepted.log`. Produktions-Focusregeln
wurden dafuer nicht umgangen.

## Grenzen

Die Fuellelemente werden nur bei vier passenden opaken Nachbarn senkrecht zu
einem geraden Rohr erzeugt. Frei stehende Rohre bekommen keine schwebende Wand.
Schraege oder gebogene Wanddurchfuehrungen benoetigen weiterhin passend gebaute
Anschluesse. Die Fuellung ist Teil des Rohr-/Ofenobjekts, kein frei erhaltener
Spielerblock oder zusaetzlicher Drop.

Rauch bleibt das bestehende begrenzte Zellmodell mit Shaderdarstellung. Die
lokalen Auslassfahnen sind kosmetisch und ersetzen keine Gasdrucksimulation.
Husten wird am lokalen Spielerkoerper ausgeloest; es gibt kein neues
repliziertes Sprach-/Hustennachrichtenprotokoll. Audiowahl und Lautstaerke
benoetigen zusaetzlich menschliche Hoerabnahme. Keine neue FPS- oder
Speicherparitaet behauptet.
