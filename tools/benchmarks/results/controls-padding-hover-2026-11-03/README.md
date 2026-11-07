# Bedienelemente, Textabstaende und Hover

Phase 186. Gemeinsame Kupferfarben fuer native Dropdowns, deren Scrollleisten,
Checkboxen, Schalter und Slider. Auch bisher unklassifizierte Controls und
Popup-Optionen erhalten die Palette. Auswahltexte in Eingabefeldern verwenden
ebenfalls warme Farben.

Hotbar-Zahlen werden innerhalb ihrer Labelboxen horizontal und vertikal mit
Flexlayout zentriert; dies gilt auch fuer die kleinen Inventar-Hotkeylabels.
Text-align allein richtet die Glyphen des UI-Textknotens nicht ausreichend aus.
Beschreibungen, Auswahlwerte, Kartenanzeigen und Tooltiptexte erhalten gezielte
Innenabstaende. Normale Beschreibungstexte behalten ihre Leserichtung.

Die Weltkarte verwendet einen groesseren gemeinsamen Innenabstand fuer Header,
Werkzeugleiste, Viewport und Footer. Die berechnete Kartenflaeche wird ebenfalls
verkleinert, damit Kartenprojektion, Mausposition und sichtbare Geometrie dieselben
Masse verwenden. Die bestehende Pruefung kleiner und grosser Viewports folgt
den neuen Rahmenabstaenden.

Der Menue-Hover verwendet eine dezente, transparente Aufhellung innerhalb der
gemalten Buttonflaeche. Die zusaetzliche gelbe Kontur und der aeussere Glow
entfallen; Kartenaktionen und Kategorien verwenden gedecktere Hoverfarben.

Pruefungen und native Bilder stehen in `logs/` und `native/`. Die native
Polish-Fixture verwendet die temporaere Inventar-Testwelt, oeffnet/schliesst
die Karte mit echtem M und prueft Rahmenabstaende und neun zentrierte Labels
in 1680 x 936 und 1024 x 640 Pixeln mit Rundungstoleranz. Bestehende Spielstaende
und Benutzereinstellungen werden nicht geoeffnet.

Ergebnis: Workspace-Build und Clippy aller Targets mit `-D warnings` erfolgreich;
Format, Diff und der echte TiltUI-CSS-Parser ebenfalls erfolgreich. Vier native
Karten-/Hotbaraufnahmen mit Geometriepruefung sowie 15 Menueaufnahmen mit echten
Klicks bestanden. Der separate Unit-Test-Lauf wurde beim Neuaufbau einer weiteren
Dependency-Featurekombination beendet; fuer diesen Stand werden keine bestandenen
Unit-Tests behauptet. Die relevante Kartengeometrie wurde im echten Fenster geprueft.

- [Karte klein](native/map-compact.png), [Karte gross](native/map-wide.png)
- [Zentrierte Hotbar](native/hotbar-wide.png)
- [Dezenter Hover](native/singleplayer-button-hover.png)
- [Textabstaende und Settings](native/settings-compact.png)
- [Pruefergebnisse und PNG-Abmessungen](verification.json)

Reproduktion mit privater Konfiguration:

```sh
TMPDIR="$PWD/target/tmp/interface186" cargo build --workspace --locked -j1
TMPDIR="$PWD/target/tmp/interface186" cargo test -p terra-ui --lib --locked -j1
TMPDIR="$PWD/target/tmp/interface186" cargo clippy --workspace --all-targets --locked -j1 -- -D warnings
TERRA_CONFIG_DIR=/tmp/teramic186-ui-config TERRA_MENU_CONFIG_DIR=/tmp/teramic186-ui-config TERRA_COMPUTE_THREADS=16 TERRA_NATIVE_OS_POINTER=1 TERRA_CAPTURE_DIR="$PWD/tools/benchmarks/results/controls-padding-hover-2026-11-03/native" TERRA_INVENTORY_REFERENCE_SMOKE=1 TERRA_POLISH_SMOKE=1 target/debug/terra-client --smoke-test-inventory
```
