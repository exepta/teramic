# Inventar mit 27 Plaetzen und Kupfer-Menues

Phase 183, 2026-11-02. Die aktuellen Vorlagen stehen unter `references/`.

- Spielerinventar: 18 Rucksackplaetze und neun Hotbarplaetze, auch serverseitig.
  Die zwei entfallenen Reihen geben Character und Crafting mehr Hoehe.
  Kleinere quadratische Slots; der Itembrowser uebernimmt die tatsaechliche
  Zellgroesse des Rucksacks und berechnet passende Zeilen/Spalten daraus.
- Hotbar: kompletter Ausschnitt der 2172 Pixel breiten Originalgrafik,
  Platz fuer beide Endornamente und Nummern. Keine zusaetzliche dunkle Platte.
- Singleplayer hat einen sichtbaren Hoverzustand. Der primaere Button wird
  nicht mehr durch einen identischen dauerhaften Leuchteffekt verdeckt.
- Pause verwendet Logo und Buttons des Hauptmenues ueber dem laufenden Spiel.
- Settings: Kupferrahmen, vertikale Kategorien links, Einstellungszeilen mit
  Symbolen und Beschreibungen, vorhandene Slider/Choices/Switches und
  Apply/Reset/Back. Alle bisherigen Kategorien und Funktionen bleiben erreichbar.
- Singleplayer: Weltliste mit Auswahl, Versions- und Spielmodusplaketten,
  Scrollbar und Play/Create/Delete/Back. Metadaten stammen aus den gespeicherten
  Weltmanifesten. Die Landschaft in den Kacheln ist eine gemeinsame Illustration,
  keine Aufnahme der jeweiligen Welt.

Die vorhandenen PNGs werden wiederverwendet. Es wurden in dieser Phase keine
neuen Rasterbilder erzeugt. Der korrekte Ausschnitt wird im Renderer gesetzt;
die Originaldateien und ihre Transparenz bleiben unveraendert.

## Gespeicherte Inventare

Netzwerkprotokoll 100 verwendet 27 Inventareintraege. Client und Server muessen
aus demselben Stand stammen. Aeltere gespeicherte Inventare mit bis zu 45
Plaetzen bleiben lesbar. Beim Beitritt werden die zusaetzlichen Reihen zuerst
in die 27 Plaetze einsortiert; verbleibende Gegenstaende erscheinen am sicher
aufgeloesten Beitrittsort. Die Uebernahme ist atomar: Falls keine Welt-Drops mehr
angelegt werden koennen, wird der Beitritt abgelehnt und der Bestand bleibt
vollstaendig erhalten. Wiederholter Beitritt dupliziert keine Gegenstaende.

## Pruefung

Die native Pruefung verwendet eine temporaere integrierte Autoritaet und
separate Testeinstellungen. Vorhandene Spielstaende werden nicht geladen.
Die Menuepruefung erzeugt fuenf private Weltmanifeste fuer die Listendarstellung
und entfernt sie anschliessend. Auswahl sowie Create/Delete und Cancel werden
ueber echte Betriebssystem-Klicks bedient. Es wird keine Welt daraus gestartet.

3743 Tests bestanden, keine fehlgeschlagen, acht ignoriert. Die neuen
Regressionen decken die 27-Plaetze-Grenze, atomare Altbestandsuebernahme,
Zusammenfuehren ohne Verlust sowie Laden/Uebernahme/erneutes Speichern ab.
Inventar- und Containerbefehle durchlaufen weiterhin Binaer-/JSON-Roundtrips.
Workspace-Build, Clippy fuer alle Targets mit `-D warnings`, Format- und
Diff-Pruefung sind erfolgreich. Das aktuelle Stylesheet besteht ausserdem die
direkte Pruefung mit dem TiltUI-CSS-Parser. Die abschliessende Aenderung des
nativen Treibers betrifft nur Aufnahme-Reihenfolge und Pause-Geometriepruefung;
sie wird durch Build, Clippy und den letzten nativen Durchlauf abgedeckt.

34 native Aufnahmen in 1680 x 936 und 1025 x 641 Pixeln: 24 im Spiel und zehn
im Hauptmenue. Die Spielpruefung kontrolliert 18 + 9 Plaetze, quadratische und
gleich grosse Browser-Slots, Kategorie-Tabs, Navigation, Rezepte ohne JEI,
Holzauswahl, Kessel/Ofen, Hotbar, E/ESC sowie Settings/Back/Resume. Die Pause-
Pruefung kontrolliert die Zentrierung, fuenf breite Buttons und deren Grenzen.
Das autoritative Inventar bleibt bei allen Vorschauaktionen unveraendert.

Die Menuepruefung kontrolliert den echten Singleplayer-Hoverzustand, Settings,
Controls und Back sowie fuenf private Weltmanifeste, Auswahl und Abbrechen
der Create-/Delete-Dialoge. Dabei startet kein Server und kein Terrain.
Aufnahmen warten vor direkten Ansichtswechseln einen weiteren Testschritt,
damit z. B. das Inventarbild nicht bereits das nachfolgende Rezept-Popup zeigt.
Die kleine Fensterbreite 1024 wird bei gebrochener Wayland-DPI auf 1025
physische Pixel gerundet. Die Geometriepruefung toleriert diesen einen Pixel.

- Inventar: [gross](native/player-inventory-reference-wide.png),
  [klein](native/player-inventory-reference-compact.png),
  [Browser eingeklappt](native/player-inventory-catalog-collapsed.png)
- Hotbar: [gross](native/hotbar-wide.png), [klein](native/hotbar-compact.png)
- Pause: [gross](native/pause-menu-wide.png), [klein](native/pause-menu-compact.png)
- Settings: [gross](native/settings-wide.png), [klein](native/settings-compact.png),
  [Controls](native/settings-controls-wide.png),
  [aus Pause](native/settings-paused-wide.png)
- Singleplayer: [gross](native/singleplayer-wide.png),
  [klein](native/singleplayer-compact.png),
  [Create-Dialog](native/singleplayer-create-dialog.png),
  [Delete-Dialog](native/singleplayer-delete-dialog.png)
- Hauptmenue: [normal](native/start-menu-wide.png),
  [Singleplayer-Hover](native/singleplayer-button-hover.png)

[Befehle](commands.txt), [Pruefergebnisse und PNG-Abmessungen](verification.json),
[Quellpruefsummen](source-sha256.json), [Protokolle](logs/).
