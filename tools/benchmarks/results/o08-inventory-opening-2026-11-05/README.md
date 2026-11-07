# O08: Erster Inventar-Oeffnungsframe

05.11.2026. Die Slots wurden versteckt mit anderen CSS-Klassen vorbereitet
als beim Oeffnen. Die Korrektur waermt die tatsaechlich benoetigten Klassen
und die fuenf Handwerksfelder vor. Die sichtbare Gestaltung bleibt erhalten.

## Gemessener Player-Uebergang

| Stand | Schlechtester erster Oeffnungsframe | `main.update` in diesem Frame |
| --- | ---: | ---: |
| O08-Baseline | 116,459 ms | 104,429 ms |
| Nur Slotklassen korrigiert | 38,507 ms | 31,743 ms |
| Zusaetzlich Handwerksprojektion vorgewaermt (strengere Bereitschaft) | 27,427 ms | 21,068 ms |
| Player-Kontrolllauf mit Handwerk und Cache-Grundschutz (urspruengliche Bereitschaft) | 25,193 ms | 18,623 ms |

Im zweiten Oeffnungszyklus der strengeren Player-Pruefung liegt die Spitze
bei 27,190 ms. Der anschliessende Player-Kontrolllauf mit denselben
Bereitschaftskriterien wie die Baseline misst 25,193 ms; die gemessene erste
Spitze sinkt damit um etwa 78%. Die geschlossenen/offenen Kontrollfenster messen je zwoelf
Sekunden in derselben stationaeren Welt.

Die zusaetzliche wiederholte Pruefung verlangt gegenueber der Baseline **zwei Frames mit
tatsaechlich sichtbarer nativer UI**: positive Geometrie, sichtbare Vorfahren
und angewendete Deckkraft. Der alte Lauf endete bereits beim gesetzten
Layoutstatus. Die Nachher-Pruefung schliesst dadurch zusaetzliche Frames ein.
Der originale dreiphasige Lauf wurde mit dem finalen Build ebenfalls
wiederholt. Die maximalen Frameintervalle werden konservativ verglichen;
`request_to_visible_ms` wird wegen der unterschiedlichen Bereitschaftskriterien
nicht als aequivalenter Latenzgewinn ausgegeben. Das Freigeben des Layouts
dauert weiterhin mehrere Frames.

Einzelne Vorher-/Nachher-Laeufe auf einer GPU belegen keinen allgemeinen
Mindest-FPS-Wert. Auch die geschlossenen Kontrollfenster zeigen Unterschiede
zwischen Laeufen. Der Befund wird durch die konkrete Klasseninvalidierung,
die gezielte Diagnose und die Zwischenschritte gestuetzt; stationaere
FPS-Unterschiede werden nicht vollstaendig dieser Korrektur zugeschrieben.

## Truhe und verbleibender Kaltaufbau

Die zwei echten Truhenzyklen bestehen einschliesslich autoritativer
Open-/Close-Antworten und sichtbarer 18 Slots. Beim erneuten Oeffnen sinkt
die Spitze von 107,337 auf 39,957 ms. Das Truhengrid `3135v1` bleibt dabei
in allen acht erfassten Uebergangssnapshots erhalten, provisorische
Player-Ausruestungsfelder entstehen nicht mehr. Auch Shell, Panel und
ihre Style-Owner bleiben gleich. Der Restore erfolgt erst bei passender
Containerprojektion. Die Messfixture schaltet ihre fuer das Platzieren
benoetigte logische Kamerasteuerung vor dem stationaeren Fenster wieder ab.

**Der erste Aufbau eines neuen Blocklayouts bleibt teuer.** Der finale
Lauf misst hier 144,300 ms; fruehere Truhenlaeufe lagen bei etwa 93 ms.
Das ist keine allgemeine Verbesserung des kalten Blockinventaraufbaus.
Anders als beim nachgewiesenen erneuten Player-/Truhen-Wechsel entsteht
hier das neue Layout zum ersten Mal. Dieser neue Befund bleibt als
**O09** offen. Er wird weder aus den Daten entfernt noch durch die
Player-Verbesserung als geloest dargestellt. Die kurzen Einzellaeufe erlauben
keine stabile Aussage ueber den genauen Betrag dieser Kaltaufbaukosten.

## Ursache und Korrektur

Eine temporaere Diagnose innerhalb des tatsaechlichen TiltUI-Stylepasses
misst beim ersten Oeffnen 90,115 ms mit acht vollstaendigen und 63 partiellen
Style-Owner-Aktualisierungen. Der gemeinsame Aufbau des Selektorsnapshots
braucht nur 0,339 ms. Viele kleine partielle Durchlaeufe verursachen zusammen
den groessten Aufwand. Eine ausprobierte Komponententrennung alleine loeste
diesen Aufwand nicht und ist zurueckgenommen. Die feine Diagnose stammt aus
diesem Zwischenversuch; ihre 63 Slot-Owner-Invalidierungen fuehrten zur
konkreten Klassenkorrektur.

Der konkrete Ausloeser steht im Slot-Template: `reference-slot`,
`square-player-slot` und die Klassen fuer normale/Geister-Mengenanzeigen
waren an `inventoryView.open` gebunden. Im versteckten Vorwaermen ist dieses
Flag absichtlich falsch. Beim ersten Oeffnen wechselten deshalb die Klassen
aller betroffenen Slots und loesten einen erneuten Stilabgleich aus.

Die drei Bedingungen verwenden jetzt `inventoryView.visible`. Damit erhalten
die ohnehin nur innerhalb des Inventars oder der Rezeptansicht verwendeten
Slots bereits beim Vorwaermen ihre spaeter sichtbaren Klassen. Die HUD-Slots
verwenden eine eigene Komponente. Die bestehenden Sonderfaelle fuer Rezepte,
Maschinenergebnisse, Brauen, Auswahl und fehlende Zutaten bleiben erhalten.

Zusaetzlich erzeugte die Handwerksprojektion ihre vier Eingabefelder und das
Ausgabefeld erst bei `open=true`. Sie wird jetzt auch waehrend des expliziten,
versteckten Vorwaermens berechnet. Danach wird sie bei geschlossenem Inventar
wie bisher zurueckgesetzt; der Projektionsstempel wird dabei ebenfalls
zurueckgesetzt, damit spaeteres Vorwaermen nach einer Groessenaenderung
funktioniert. Handwerksaktionen erfordern weiterhin ein wirklich geoeffnetes
Inventar. Die bestehende Layoutfreigabe und der retained DOM-Cache bleiben
fuer das Anzeigen und Parken zustaendig.

Die echte Truhenpruefung deckte ausserdem einen verfruehten Cache-Restore
auf: Nach `OpenChest` war die Ansicht bereits als offen markiert, waehrend
der autoritative Containersnapshot noch fehlte. Der Inhalt konnte dadurch
erst als Player-Inventar und kurz danach als Truhe aufgebaut werden. Der
Cache bleibt jetzt bis zur Antwort geparkt. Eine autoritative Schliessaktion
parkt ihn sofort, auch wenn die getaktete Ansicht noch einen Frame lang
offen aussieht. Fehlgeschlagene/abgebrochene Anfragen bleiben geparkt;
normales Oeffnen und explizites verstecktes Vorwaermen koennen ihn danach
wiederherstellen. Die vorhandene Layoutfreigabe haelt den alten Inhalt
waehrend der Anfrage verborgen.

Die Frame-Telemetrie belegte eine zweite Reihenfolge innerhalb dieses Pfads:
Beim erneuten Oeffnen war die Antwort bereits angekommen und die private
Pending-Markierung geloescht, waehrend die auf 60 Hz getaktete Ansicht noch
`player=true` meldete. Ein Frame entfernte dadurch das behaltene Truhengrid
und erzeugte Player-Felder; der naechste ersetzte diese wieder durch eine
neue Truhe. Allein die Stilintervalle dieser zwei Frames lagen bei rund
78 und 70 ms.

`InventoryView.source_container` merkt sich deshalb jetzt nativ, aus welchem
Anker und welcher logischen Containeransicht die Projektion stammt.
Geparkte Bindings werden erst wiederhergestellt, wenn diese Identitaet zum
aktuellen Snapshot passt. Ein bereits aktives Inventar wird durch normale
Snapshotaktualisierungen nicht neu geparkt. Das Feld wird nicht in HTML
serialisiert. Der neue Test trennt Antwort und View-Tick absichtlich ueber
drei Frames und prueft auch den Wechsel zur angehaengten Feuerstelle am
gleichen Anker.

Die dauerhafte Korrektur liegt im Spielprojekt. Die temporaer instrumentierte
externe TiltUI-Datei ist bytegenau wiederhergestellt (SHA256 in den
Diagnosemetadaten). Das Spiel kann ueber denselben bereits verwendeten
Runtime-Baustein jetzt auch das oeffentliche Style-Set instrumentieren.
`ui.style` ist dabei ein **Schedulerintervall** vor/nach diesem Set; andere
bereite Systeme koennen dazwischen laufen. Es ist kein isolierter Funktions-
oder GPU-Timer, und verschachtelte Intervalle werden nicht addiert.

## Regressionen und Messaufbau

Die neuen TiltUI-Tests verwenden die tatsaechlichen Klassenbindungen des
Slot-Templates und die echten Shared-Resources. Bei Player-, Hotbar-, Katalog-,
Ergebnis-, Container- und Mixer-Slots bleiben Entity, Klassen und Change-Tick
ueber verstecktes Vorwaermen, vier Oeffnen-/Schliessen-Wechsel und
Parken/Wiederherstellen gleich. Rezept-/Maschinenwechsel und Auswahl-/Fehl-
Zustaende muessen dagegen weiterhin die passenden Klassen aendern.

Ein weiterer Test prueft die vollstaendige versteckte Handwerksprojektion,
ignorierte Aktionen bei geschlossenem Inventar, unveraendertes Inventar und
Nachrichtenqueue, stabile Warm-/Ruheframes, erneutes Vorwaermen sowie
Weltaustritt und verlorene Sessions.

Zwei weitere echte TiltUI-Bindungstests pruefen, dass waehrend ausstehender
Containerantworten kein provisorisches Player-Layout ueber den behaltenen
Blockinhalt gebunden wird. Erfolg, Abbruch, Fehler, asynchrones Schliessen,
ein veraltetes View-Flag sowie anschliessendes Player-Oeffnen/Vorwaermen
sind enthalten.

Der opt-in Native-Benchmark unterstuetzt jetzt `ROUNDS=1|2` und
`SURFACE=player|chest`. Die Truhenfixture verwendet einen echten Creative-
Inventargrant, Hotbarauswahl, Produktions-Picking und bestaetigte Blockedits.
Oeffnen und Schliessen laufen ueber die normalen UI-/Containernachrichten;
replizierter Container, Viewerzustand, sichtbares Grid und Slotzahl werden
geprueft. Jeder Wechsel protokolliert Eingabesperre, Fokuszuordnung, versteckte
Tooltips, Displayzustand und erhaltene Style-Owner. Fensterfokus wird dabei
korrekt von verbliebenem Inventar-Eingabefokus unterschieden.

Der Smoke-Modus deaktiviert das Fangen des OS-Cursors. Diese Laeufe pruefen
die Spiel-Eingabesperre und die native UI, aber keine erneute OS-Capture-
Abnahme. Der Capture-Produktionspfad wird durch diese Korrektur nicht geaendert.

Die gesonderte Bedienpruefung des finalen Builds besteht mit **25 nativen
Aufnahmen** in breiter und kompakter Ansicht: echte OS-Klicks, Suche/Fokus,
ESC/E, Rezeptseiten und Holzvorschauen, Ofen/Kessel, Hotbar sowie Pause und
Einstellungen. Slotgroessen, Seitenstabilitaet und das unveraenderte
autoritative Inventar werden geprueft. Das Pointer-Werkzeug benoetigte fuer
die grosse Entfernung zwischen Monitoren ein entfernungsabhaengiges,
weiterhin begrenztes Bewegungsbudget; der erste gescheiterte Pointerlauf
ist ausgeschlossen. Bildpfade, Abmessungen und Hashes stehen in `captures.json`.

Hardware/Konfiguration: RX 6950 XT / RADV NAVI21, Vulkan,
Mesa 26.2.5-arch3.1, Seed 42042, 2562 × 1440, Sichtweite 20,
kein FPS-Limit/VSync, hohe Block-/Laub-/Schatten-/Wasserqualitaet,
Mikrogeometrie 48 m. Normales Dev-Build mit Projekt-`opt-level=1`,
Dependencies und Render-/Voxel-Hotpaths mit `opt-level=3`.
Die eingefrorenen technischen TOMLs und Hashes liegen bei. Die Player-
Performancepruefung verwendet Survival; Truhenfixture und gesonderte
Bedienpruefung verwenden explizite Creative-Ableitungen. Profile und Welten
sind isolierte Wegwerfdaten.

## Reproduktion

```sh
cargo build -p terra-client
o08_root=$(mktemp -d /tmp/teramic-o08-repeat-XXXXXX)
mkdir "$o08_root/configs"
cp tools/benchmarks/results/o08-inventory-opening-2026-11-05/configs/*.toml "$o08_root/configs/"
TERRA_CONFIG_DIR="$o08_root/configs" \
TERRA_MENU_CONFIG_DIR="$o08_root/menu" \
TERRA_PROFILE_FILE="$o08_root/profile.toml" \
TERRA_WORLD_DIR="$o08_root/worlds" \
TERRA_BENCH_MODE=integrated TERRA_BENCH_UI=1 TERRA_BENCH_GRAPHICS=1 \
TERRA_BENCH_INVENTORY=1 TERRA_BENCH_INVENTORY_ROUNDS=2 \
TERRA_BENCH_INVENTORY_SURFACE=player TERRA_BENCH_SECONDS=120 \
TERRA_BENCH_LOAD_SECONDS=180 TERRA_BENCH_WEATHER=clear \
TERRA_CPU_SCOPES=1 TERRA_SCOPE_TIMELINE=1 WGPU_BACKEND=vulkan \
python3 tools/benchmarks/measure.py target/debug/terra-client --smoke-test-benchmark
```

Fuer die Truhenpruefung einen neuen isolierten Ordner verwenden, die
beiliegende `configs/chest/server.toml` als `configs/server.toml` einsetzen
und `TERRA_BENCH_INVENTORY_SURFACE=chest` setzen. Der normale dreiphasige
Benchmark ohne diese zwei Erweiterungsoptionen bleibt verfuegbar.

3813 Workspace-Library-/Bin-Tests einschliesslich abschliessendem UI-/Client-
Nachlauf bestanden, neun ignoriert, keine verbleibenden Fehler.
Acht Python-Runner-Tests, Build, Clippy aller Targets mit `-D warnings`,
gezieltes Rustfmt und Diff-Pruefung erfolgreich. Maschinenlesbare
Messdaten, native Nachweise und die gesonderte Diagnose liegen bei.
