# Inventare passend zur Aufloesung (Phase 189)

Inventare und HUD-Hotbar wachsen jetzt mit der Fensteraufloesung. Die
kompakten 28/32er-Zellen bleiben die gemeinsamen logischen Grundmasse;
Rahmen, Slots, Schrift, Icons und Abstaende werden zusammen vergroessert.
Der UI-Regler wirkt weiterhin. OS-DPI und der globale UI-Massstab werden
nicht doppelt angewendet.

Ungefaehre sichtbare Slotgroessen bei 100 Prozent UI-Massstab:

| Aufloesung | Inventar | Ergebnis | HUD-Hotbar |
| --- | ---: | ---: | ---: |
| 1280 x 720 | 37 px | 42 px | 36 px |
| 1920 x 1080 | 56 px | 64 px | 54 px |
| 2560 x 1440 | 74 px | 85 px | 72 px |
| 3840 x 2160 | 111 px | 127 px | 108 px |

Die Tabelle beschreibt das normale Inventar. Bei einer angeschlossenen
Truhe wird auch die gesamte zusaetzliche Breite eingepasst. Das Inventar
steht etwas oberhalb der Fenstermitte, damit die groessere Hotbar darunter
Platz hat. Bestehende Slotverhaeltnisse, Kategoriepositionen und der linke
Truhenrahmen bleiben erhalten.

## Umsetzung

`frames.rs` vergroessert den gesamten Inventar-Arbeitsbereich ueber dessen
UI-Transformation. Die Zielgroesse folgt der Fensterhoehe; eine Obergrenze
fuer Breite und Hoehe verhindert abgeschnittenen Inhalt. Die Hotbar verwendet
einen eigenen passenden Massstab und eine nachgefuehrte Unterkante. Menues,
Minimap und andere HUD-Elemente behalten ihren bisherigen Massstab.

Tooltippositionen beruecksichtigen jetzt ihre tatsaechlich gezeichnete
Groesse unter einer skalierten UI-Transformation. Ihre lokalen
Layoutkoordinaten bleiben korrekt. Die native Galerie misst transformierte
Eckpunkte, zusaetzlich zu den unveraenderten logischen Slotmassen.

## Pruefung

- **3552 Library-Tests bestanden, keine fehlgeschlagen, sieben ignoriert**;
  darin 454 UI-Tests. Workspace-Build, Clippy aller Targets mit `-D warnings`,
  Format- und Diffpruefung erfolgreich.
- **26 native Aufnahmen bestanden**, bei 1680 x 936 und 1024 x 640
  (ein Pixel Wayland-Rundung). Alle zwoelf Blockinventare und das
  Spielerinventar wurden in beiden Groessen geprueft.
- Neue Regressionen pruefen wachsende Inventar-/Hotbargroessen von 1024 x 640
  bis 4K, die Begrenzung verbundener Truhen, UI-Einstellungen von 75 bis
  150 Prozent sowie identische sichtbare Groessen bei DPI 1/1.25/1.5/2.
- Eine Tooltip-Regression prueft zwoelf Kombinationen aus globalem
  UI-Massstab, DPI und lokalem Zoom am unteren rechten Fensterrand.
- Die native Galerie prueft sichtbar ausreichend grosse Zellen,
  quadratische Slots, passende Klickflaechen, vollstaendige Inhalte ohne
  Scroll, die Tonne neben JEI und eine Hotbar ohne abgeschnittene Labels.
  Inventar und Hotbar duerfen sich nicht ueberlappen.
- Vollstaendige Testergebnisse und Quell-/Bildpruefsummen stehen in
  `verification.json`; die zugehoerigen Build-, Test-, Clippy- und
  Grafikprotokolle liegen unter `logs/`.

Die Bildpruefung verwendet eine temporaere integrierte Welt und validierte
Client-Projektionen. Das autoritative Inventar wird vor/nach der Galerie
verglichen. Es werden keine OS-Maus- oder Tastatureingaben ausgefuehrt;
vorhandene Spielstaende und Benutzereinstellungen bleiben unberuehrt.

## Aufnahmen

- [Spielerinventar breit](native/fixed-player-wide.png),
  [kompakt](native/fixed-player-compact.png).
- [Werkbank und Truhe](native/surface-workbench-linked-wide.png),
  [kompakt](native/surface-workbench-linked-compact.png).
- [Ofen](native/surface-furnace-wide.png),
  [Doppeltruhe kompakt](native/surface-double-chest-compact.png).
- [Vorherige kleinere Darstellung](../fixed-inventory-layout-2026-11-03/native/fixed-player-wide.png).
