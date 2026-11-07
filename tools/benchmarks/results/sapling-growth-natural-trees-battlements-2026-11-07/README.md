# Setzlingswachstum, natürliche Baumformen und Zinnensäulen (201)

Diese Korrekturen setzen auf Phase 200 auf. Die [neue Säulenreferenz](../../../../docs/references/battlement-pillar-stacking.png)
zeigt die unerwünschten wiederholten Kronen und Sockel im Stapel.
Es werden auf ausdrücklichen Nutzerwunsch keine Grafiktests oder Spielclients gestartet.

## Setzlingsanzeige und Modelle

Beim Anschauen zeigt die normale Zielanzeige einen Wachstumsbalken und bestätigte
Prozentwerte. Bei 100 Prozent wartet der Setzling auf das tatsächliche Wachstum.
Fehlende oder nicht zur Prop-ID, Position und Art passende Baumdaten erscheinen
als „Wird synchronisiert“, statt Fortschritt eines alten Bewohners zu übernehmen.

Die Apfelkörper hängen mit Abstand unter dem Laub; die zuvor koplanaren und
überlappenden Flächen entfallen. Kleine Cherry-Blüten sitzen auf vier freien
Außenseiten der Krone. Das vorhandene Farb-, UV- und Materialsystem bleibt erhalten.

## Generierung und gewachsene Bäume

Neue gepflanzte Bäume verwenden die größeren Formen und den Darstellungsstil der
natürlichen Baumgenerierung. Art, Seed und exakt attribuierte Holz-/Laubzellen
bleiben für Wachstum, Fällung, Früchte, Speichern und spätere Beitritte maßgeblich.
Vorhandene ausgewachsene Bäume werden nicht nachträglich umgebaut.
Unteres, tatsächlich zugeordnetes Stammholz großer Generated-Bäume startet die
Gesamtfällung auch seitlich am Stamm. Nachträglich gesetztes Holz und hohe Äste
lösen diesen Pfad nicht aus.

Oak, Birch und Cherry verlangen mindestens 16 Blöcke Wurzelabstand, Spruce und
Apple mindestens 12. Boden, Licht und das vollständige tatsächliche Kronenvolumen
werden zusätzlich geprüft. Nur kollidierendes, tatsächlich generiertes weiches
Gras beziehungsweise Blumen mit vollständigen Abbaurechten dürfen weichen.
Gesetzte Props, Nutzpflanzen, andere Setzlinge und Spielerbauten bleiben erhalten.

Die Form entsteht auf AsyncCompute. Pro Update werden höchstens 512 Zellen und
eine vorbereitete Section-Journalkopie bearbeitet; ein Baum berührt höchstens 64
Sections. Private Kopien werden erst vollständig veröffentlicht. Zwischenzeitlich
geänderte Claims, Props, Journale oder geladene Blockpayloads brechen den Versuch ab;
das gilt auch für eine Section-Ersetzung bei unveränderter Revisionsnummer.

Ökologierevision 4 verwendet ein 5-Prozent-Ticket für geeignete Baumkandidaten in
Forest und Plains. Gespeicherte ältere Generierungsrevisionen bleiben unverändert;
insbesondere behält Revision 3 ihre bisherige Apfelauswahl.
Neue Apfeläste verwenden zusammenhängende Kapseln mit 0,90 Radius, damit der
abgetastete Holzskelettgraph vollständig flächenverbunden ist. Astachsen und
Kronen bleiben gleich; Revision 3 behält die ursprünglichen dünneren Kapseln.

Die angehängten Stage-Tags `Generated` und `GeneratedFelled` unterscheiden neue
Generatorformen von alten Mature-Records und bewahren den Stil teilweise gefällter
Restbäume. Ältere Stage-Tags behalten ihre Nummern. Neue Builds lesen alte
ID11-Daten; neue große Generated-Records verlangen aktualisierte Builds.
Protokoll 109 stimmt die größere Baumzuordnung und bestätigte Fortschritte ab.

Ein Record erlaubt 4.096 Holz- und 16.384 Laubzellen. Interest-Deltas enthalten
höchstens 256 Records; ein Patch ist auf 32.768 übertragene Offsetzellen und
128 KiB kanonisch kodierte Daten begrenzt. Die Geometrie wird beim ersten Empfang
vollständig übertragen, danach bei Fällung als entfernte Offsets. Reine Frucht-
und Fortschrittsänderungen behalten bestehende Geometriepuffer und invalidieren
keine unveränderten Meshes. Setzlingsfortschritt ist auf ganze Prozent quantisiert.
Bestätigte Sectionimporte replayen die Blockdaten ohne lokale Entziehung von
Baumzellen. Dadurch stimmen Session und Journal auch nach spätem Beitritt oder
wiederholtem Chunkimport weiter überein. Echte lokale Blockänderungen geben ihre
Zellen weiterhin ausdrücklich aus der Baumzuordnung frei.

## Zinnen und Säulen

Zinnenmauer (180) bildet Ecken über ihre Nachbarn. Die separate Ecke (181) verschwindet
nur aus Creative/TMI und Rezepten; bestehende IDs und alte Blöcke bleiben lesbar.
Aufnehmen und Abbauen einer alten Ecke liefert die Zinnenmauer (180).

Säulen behalten ihre vier ursprünglichen Kronen. Niedrige seitliche Verbinder
kommen zusätzlich hinzu. Gestapelte Säulen zeigen einen durchgehenden Schaft,
einen Fuß ganz unten und den ursprünglichen Kopf ganz oben. Mesh, Auswahl,
Kollision und Wasser verwenden dieselben Nachbarprofile; vertikale Übergänge
über Sektionsgrenzen und sämtliche Posen werden berücksichtigt.

## Prüfung

4.050 Workspace-Library-/Bin-Tests bestanden, keine fehlgeschlagen, neun
bestehende Tests ignoriert. Alle zwölf Testsuiten sind vollständig durchgelaufen.

| Prüfung | Ergebnis |
| --- | --- |
| `cargo test --workspace --lib --bins --locked --no-fail-fast -j1 -- --test-threads=4` | [4.050 bestanden](logs/workspace-tests.log) |
| `cargo build --workspace --locked -j1` | [Erfolgreich](logs/workspace-build.log) |
| `cargo clippy --workspace --all-targets --locked -j1 -- -D warnings` | [Ohne Warnungen](logs/workspace-clippy.log) |
| `cargo fmt --all -- --check` und `git diff --check` | Erfolgreich |
| Tatsächlicher TiltUI-Parser | [60 Stylesheets gültig](logs/ui-stylesheets.log) |

Die Tests prüfen bestätigte Wachstumsanzeige und Identitätswechsel, echte UVs/
Normalen und freie Blüten-/Apfelflächen, sämtliche 112 Zinnenprofile und 24 Posen,
vertikale Sektionsgrenzen, Auswahl, Wasser und Save/Reload. Gemeinsame Generator-
formen aller fünf Arten, 64 Apfel-Seeds, historische Revisionen, vorbereitete
Wachstumsbatches, Spielerersatz und fremde Prop-Claims sind geprüft.

Echte UDP-Tests prüfen Teilpakete mit jeweils 20.000 Zellen pro großem Testbaum,
Zwischenfortschritt, unveränderte Geometriepuffer, Entfernungsdeltas und späten
Beitritt. Bestätigte Sectionimporte müssen die vollständige Session-/Journal-
Zuordnung erhalten; lokale Änderungen geben ihre Zellen weiterhin frei. ID11-
Roundtrips erhalten Generated-/GeneratedFelled-Reste und teilweise gepflückte
Früchte; eingefrorene Legacy-Mature-Daten behalten ihre bisherige Kodierung.

Der Gesamtlauf verwendet einen Cargo-Job, vier Testthreads, niedrige Prozess-/
I/O-Priorität und entfernt `DISPLAY`/`WAYLAND_DISPLAY`. Es wurden keine Grafiktests,
Spielclients oder GPU-Galerien gestartet. Der Bildvergleich zur Laufzeit bleibt
wegen dieser ausdrücklichen Nutzervorgabe aus.

[Maschinenlesbare Ergebnisse sowie Quell-/Log-Prüfsummen](verification.json)
werden durch [collect-verification.py](collect-verification.py) aus den finalen
Logs erzeugt. Bereits gespeicherte Weltgenerierung und alte ausgewachsene
Mature-Formen bleiben erhalten; neu gewachsene Setzlinge verwenden den neuen Stil.
