# Phase 215: Zugachse und Lok-Aufbauvorlage

## Reproduzierter Montagefehler

Die Diagnose einer isolierten Kopie des vorhandenen Spielstands zeigt eine
korrekte Drehgestelllinie mit vier Metern Abstand ueber einem 28 m langen,
durchgehenden gezeichneten Gleis. Drehgestelle und Kupplungen folgen der
Z-Achse, der wuerfelfoermige Fahrgestellblock traegt jedoch eine um 90 Grad
abweichende Drehung. Die Montage bestimmte daraus die falsche Achse und meldete
irrefuehrend, beide Drehgestelle stuenden nicht ueber einem Gleis.

Die Achslinie wird jetzt aus den Drehgestellen bestimmt. Der Fahrgestellwuerfel
behaelt seine urspruengliche Pose. Fuer schon passende gespeicherte Fahrzeuge
bleibt auch die bisherige vorzeichenbehaftete Fahrbasis erhalten, damit
Neigung, Kupplungsreihenfolge und Bewegungszustand unveraendert bleiben.
Eine portable Regression rekonstruiert den aufgezeichneten Fall und prueft
Montage und Rueckbau einschliesslich aller Posen und privaten Nutzlasten.
Die Originalwelt wird nicht durch die Diagnose geladen oder veraendert;
die Analyse laeuft ausschliesslich auf einer Kopie ausserhalb der Git-Dateien.

Beide realen Achskontakte werden gegen passende geladene aktive Gleiswege
geprueft. Nachbarzellen mit Gleiseigentum werden einbezogen. Projektion auf
begrenzte Streckenabschnitte verhindert Abtastluecken auf langen oder
ungleichmaessig parametrisierten Geraden. Ausrichtungs-, Abstands- und echte
Gleisfehler erhalten getrennte Meldungen; Protokoll 119 traegt die neuen
Fehlercodes.

## Sichtbarer Lokaufbau

Die funktionale Basisvorlage bleibt unveraendert. Die normale Lokauswahl
fuegt einen optionalen Aufbau aus Kessel, Rauchkammer, Schornstein,
Fuehrerhausseiten, Fenstern und Dach hinzu. Bei vier Metern Radbasis stehen
39 Plaetze in der Vorlage; nach 13 fertigen Grundteilen bleiben 26 gruene
Aufbauplaetze. Freier Bau bleibt moeglich; die Verkleidung ist keine neue
Montagepflicht.

Vorhandene Fahrgestelle und passende Grundteile werden bei der Platzwahl
gegenueber einem naeheren leeren Gleis bevorzugt. Ein Fahrgestellwuerfel oder
Motor mit abweichender Drehung zaehlt weiterhin als passendes Grundteil.
Richtungsabhaengige Aufbauformen muessen passend ausgerichtet werden.

Die Vorlage bleibt bei abgelehnter Montage und anderen Fahrzeugaktionen
sichtbar. Ihr Abschluss wird auch bei vorher geschlossenem Fenster verarbeitet.
Erst erfolgreiche Montage entfernt die Platzhalter. Die neue Fuehrerhausskizze
laesst Fahrer, hinteren Einstieg und Kopfraum frei; die Sohlbank des verwendeten
Fensters ist auf 0,08 m angepasst, damit die vorhandene Sitzhoehe freie Sicht hat.

## Pruefung

Es laufen ausschliesslich CPU-Tests, Daten-/Modellpruefungen und Buildwerkzeuge.
Kein Spielclient, Spielfenster, Grafikbackend oder echter Screenshot wird
hierfuer gestartet. Material und Anzahl der gruenen Platzhalter werden an
echten ECS-Entities ohne Renderer kontrolliert.

Cargo verwendet einen Job mit niedriger CPU-/I/O-Prioritaet und entfernten
`DISPLAY`-/`WAYLAND_DISPLAY`-Variablen:

```sh
cargo test --locked --workspace --lib train -- --test-threads=4
cargo test --locked --workspace --lib --no-fail-fast -- --test-threads=4
cargo build --locked --workspace
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
git diff --check
python3 tools/assets/lint_train_parts.py --log target/verification215/asset-lint.log
```

Alle **4.218 Workspace-Library-Tests** und alle **135 gezielten Zugtests**
bestehen; kein Fehler, neun bestehende Library-Tests ignoriert.

| Paket | Bestanden |
| --- | ---: |
| terra-core | 453 |
| terra-network | 536 |
| terra-physics | 75 |
| terra-player | 118 |
| terra-render | 1065 |
| terra-server | 8 |
| terra-storage | 166 |
| terra-ui | 557 |
| terra-voxel | 25 |
| terra-world | 1215 |

Die Abnahme enthaelt native Netzwerkaktionen, echte Physikkontakte, alte
Speicherstaende, den konkreten Fall auf einem 28 m langen gezeichneten Gleis und alle
vier Baurichtungen auf nativen/gezeichneten Strecken. Nach dem Rueckbau bleiben
Blockposen und private Nutzlasten des Problemfalls bytegleich. Die bisherige
Neigung bleibt auch bei entgegengesetzten, achsengleichen Drehgestellen erhalten.

Registrierte HTML-Klickhandler pruefen Vorlagenauswahl, vorhandenen Aufbau,
verzoegerten Montageabschluss und vorher geschlossenes Fenster. Renderer-ECS-
Tests kontrollieren nach fertigem Grundgeruest 26 echte gruene Blend-Platzhalter.
Weitere Formtests pruefen Radbasen 3–15 m, verbundene Aufbauplaetze, freies
Fahrerprofil und seitliche Augenstrahlen durch die passenden Fenster.

Die Asset-Pruefung besteht fuer alle 84 Zugmodelle und 415 Rezepte, einschliesslich
Dreieckswindung und projizierter Flaeche. Der vollstaendige Workspace-Build von Client und Server besteht
einschliesslich Linking (7 min 37 s). Clippy aller Targets mit `-D warnings`,
Format und Diff-Pruefung bestehen. Die Programme wurden nicht gestartet.
`verification.json` enthaelt aktuelle Quell- und Protokollpruefsummen.
