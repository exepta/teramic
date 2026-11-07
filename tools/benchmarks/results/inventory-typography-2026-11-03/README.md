# Kleinere Inventarschrift und Icons (Phase 190)

Die grossen Inventare und Slots aus Phase 189 bleiben erhalten.
Ueberschriften, Beschriftungen, Mengenangaben und Bedienicons wurden gezielt
verkleinert. Haupt- und Crafting-Titel verwenden 14 logische Pixel,
Abschnittstitel 11, weitere Labels und Mengenangaben 9 bis 10. Bedienicons
verwenden meist 14 Pixel; Prozesspfeile 18, Tonne 16 und Rezeptbuch 12.
Klickflaechen, Itemvorschauen und Character behalten ihre Groesse.
Suchfeld-Platzhalter, Eingabetext und Auswahlwerte besitzen ebenfalls
explizit kleinere Schriftgroessen; diese Widget-Teile hatten eigene
groessere Standardwerte.

Die Hotkey-Beschriftungen 1 bis 9 sind aus den Inventarslots entfernt.
Mengenangaben bleiben sichtbar. Die eigenstaendige HUD-Hotbar verwendet
weiterhin ihre Tastenbeschriftungen, mit kleinerer Schrift fuer Tastenzahlen,
Mengen und Itemnamen. Das Stylesheet ist auf Inventar, Werkbank, Itembrowser,
Ausruestungsplatzhalter und diese Hotbartexte begrenzt; Menues und
Rezept-Popups behalten ihre bisherige Typografie.

## Pruefung

- Der echte Tilt-CSS-Parser und der bestehende Stylesheet-Regressionstest
  pruefen das neue Stylesheet (ein gezielter Test bestanden).
- Workspace-Build, Clippy aller Targets mit `-D warnings`, Format- und
  Diffpruefung.
- Die native Galerie prueft alle zwoelf Blockinventare und das
  Spielerinventar bei 1680 x 936 und 1024 x 640 (ein Pixel
  Wayland-Rundung). Sie verlangt unveraenderte 28/32er-Grundmasse,
  sichtbare Klickflaechen, passende Panelfuellung, Platz fuer die Hotbar
  sowie fehlende `item-hotkey`-Elemente im Inventar.
- Die Galerie nutzt eine temporaere Welt und validierte Clientdaten;
  das autoritative Inventar wird auf Gleichheit geprueft. Keine
  OS-Maus-/Tastatureingaben oder Aenderungen vorhandener Spielstaende.

Die Protokolle stehen unter `logs/`; `verification.json` erfasst
Pruefergebnisse und Quell-/Bildpruefsummen.

## Bilder

- [Spielerinventar breit](native/fixed-player-wide.png),
  [kompakt](native/fixed-player-compact.png).
- [Werkbank mit Truhe](native/surface-workbench-linked-wide.png).
- [Ofen](native/surface-furnace-wide.png),
  [Kessel](native/surface-cauldron-wide.png).
- [Vorherige Typografie](../responsive-inventory-2026-11-03/native/fixed-player-wide.png).
