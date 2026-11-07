# Einheitliche Inventare, Rahmen und Wetterkorrektur

Phase 187, 2026-11-03. [Markierte Vorlage](references/settings-frame.png).

## Wetterabsturz

`commit_block_edit` las implizites Wasser mit festem 18er-Stride und einem
einzelligen Rand. PreparedColumn kann jedoch sowohl 16x16 ohne Rand als auch
18x18 mit Rand enthalten. Bei Wetter-/Schnee-Edits wurde deshalb die falsche
Spalte gelesen oder hinter das 256-teilige Array gegriffen. Beide Abfragen
verwenden nun `PreparedColumn::surface_at` mit Weltkoordinaten.

Die Regression setzt und schmilzt Schneekappen auf allen 256 Spalten in beiden
Formaten, mit positiven und negativen Abschnittskoordinaten. Wasserhaltung
in Teilbloecken, Wasser direkt unter Rohren und Replica-Uebernahme werden
ebenfalls mit beiden Formaten geprueft. Kein Clamp, verschluckter Panic oder
Fallback auf eine fremde Nachbarspalte.

## Inventare und Rezeptaufrufe

Eine gemeinsame, aus dem Spielerlayout abgeleitete Zellgroesse ersetzt die
getrennten 31-/36-/44-Pixel-Regeln fuer Maschinen und verbundene Truhen.
Rucksack, Hotbar, Truhe, Werkbank, Ofen, Kessel, Brecher, Mischer und Filter
verwenden dieselbe quadratische Zelle. Der Katalog verwendet nun 100 Prozent
dieser Groesse; die fruehere 75-Prozent-Vorgabe wird durch den aktuellen
Benutzerwunsch ersetzt. Die Seitenkapazitaet folgt der verfuegbaren Flaeche.

Verbundene Truhen stehen im gemeinsamen Scrollbereich vor der Werkbank.
Ihre Zellen werden dadurch nicht fuer eine dritte schmale Spalte verkleinert.
Laengere Inhalte bleiben scrollbar. Die native Geraetepruefung vergleicht
Zellgroessen von Rucksack, Maschinen und Katalog und scrollt die Werkbank bis
zu den unteren Inventarplaetzen.

Rezeptanfragen ohne vorhandenes Rezept oeffnen kein leeres Popup und ersetzen
keine bereits geoeffnete Rezeptansicht. Echte generierte Faerbe-/Mischerrezepte
bleiben erreichbar. Inventar, Reservierungen und Rezeptregistries bleiben
unter den vorhandenen autoritativen Regeln.

## Weitere Oberflaechen

67 Settings-IDs verwenden 54 passende Symbole, darunter fuenf verschiedene Icons
fuer die Multiplayer-Einstellungen. Welt-/Serverkarten wachsen um 25 Pixel auf
103 Pixel beziehungsweise 97 Pixel im kompakten Layout.

Die aeusseren Kupferrahmen-Hosts sind transparent. Separate eingerueckte
Innenflaechen behalten den braunen Hintergrund innerhalb der Rahmensilhouette.
Dadurch ragt kein rechteckiger Hintergrund ueber die unregelmaessigen Ecken.
Das vorhandene PNG bleibt unveraendert.

Wegpunktfelder zeigen Kommazahlen mit zwei Nachkommastellen und akzeptieren
Komma oder Punkt bei der Eingabe. Beim unveraenderten Speichern bleiben die
urspruenglichen Weltkoordinaten mit voller Praezision erhalten.

## Pruefung

Protokolle in [logs/](logs/), native Aufnahmen in [native/](native/).
Die Spielpruefungen verwenden eine temporaere integrierte Autoritaet und private
Konfiguration. Geraeteaufnahmen zeigen validierte Client-Projektionen und
pruefen die Darstellung, ohne einen neuen Verarbeitungs-/Durchsatztest zu
behaupten. Bestehende Spielstaende werden nicht geoeffnet.

3748 Tests bestanden, keine fehlgeschlagen, acht ignoriert. Darin sind alle
449 UI-Tests und die beiden gezielten Wetter-/Wasserregressionen enthalten.
Workspace-Build, Clippy aller Targets mit `-D warnings`, Format, Diff und beide
neuen CSS-Parserpruefungen sind erfolgreich.

Die Inventar-/Rezeptgalerie besteht vollstaendig mit 25 Aufnahmen, gleichen
Katalog-/Spielerslots und unveraenderter Inventarbasis nach den Vorschauaktionen.
19 weitere aktuelle Geraeteaufnahmen bestaetigen gleiche Slots fuer Maschinen,
Rucksack und Katalog. Die Geraetegalerie wurde jedoch durch vorzeitiges
Fensterschliessen unterbrochen. Die Menuegalerie endete mit einem nicht
erreichbaren OS-Mausziel im Fenstermanager. Fuer diese beiden Gesamtlaeufe und
die zusaetzliche sichtbare Wegpunktaufnahme wird kein erfolgreicher Abschluss
behauptet. Wegpunktformat und unveraenderte Speicherpraezision sind unitgetestet.
Die native Geraetefixture prueft auch Scrollzugriff fuer die verbundene Werkbank.

- [Spielerinventar](native/player-inventory-reference-wide.png)
- [Truhe mit gleich grossen Katalogslots](native/surface-chest-wide.png)
- [Ofen](native/surface-furnace-wide.png), [Kessel](native/surface-cauldron-compact.png)
- [Werkbank mit Truhe](native/surface-workbench-linked-wide.png)
- [Settings im Spiel](native/settings-paused-wide.png)
- [Pruefergebnisse, Quellpruefsummen und PNG-Abmessungen](verification.json)

```sh
TMPDIR="$PWD/target/tmp/interface187" cargo test --workspace --locked -j1 -- --test-threads=4
TMPDIR="$PWD/target/tmp/interface187" cargo build --workspace --locked -j1
TMPDIR="$PWD/target/tmp/interface187" cargo clippy --workspace --all-targets --locked -j1 -- -D warnings
cargo fmt --all -- --check
git diff --check
```
