# Phase 48: Crafting-Bedienung Und Rechte Sidebar

17.10.2026. Keine Dependency-Aenderungen. Netzwerkprotokoll 22 benoetigt
gemeinsam aktualisierte Clients und Server. REG3/PLY3, Weltformat 2,
Generatorversion 5, Block-/Welt-IDs und Seeds bleiben unveraendert.

## Umsetzung

- Mengenraster bleibt eine Reservierung auf dem bestehenden Inventar.
  Linksziehen verteilt beim Loslassen; Rechtsziehen legt je Eintritt ein Item
  ab. Halbe Stapel, Sammelziehen, Doppelklick, Shift-Rueckgabe und Mausrad sind
  auch im Raster verfuegbar. 2x2 und 3x3 verwenden dieselbe Implementierung.
- Weitere Rasterbesuche werden begrenzt ueber ausstehende Antworten hinweg
  gepuffert. Exakte `TransferCursor`-Mengen werden atomar serverseitig geprueft;
  kein zweiter Itembestand und keine einzelne Nachricht pro verteiltem Item.
  Revisionskonflikte laufen ueber die bestehende begrenzte Retry-Queue.
- Rezeptvorschau aendert keine Reservierungen. Erst der Plus-Button ordnet
  verfuegbare Zutaten an einer passenden Station ein und schliesst das Popup.
  Mit belegtem Cursor oder fehlenden Zutaten bleibt der Button deaktiviert.
  Eine einzelne Rezeptvariante zeigt keine Pagination.
- Itembrowser rechts am Fensterrand, 95 Prozent Fensterhoehe, Desktopabstand
  100 UI-Pixel. Engere Fenster reduzieren Abstand und Rastergroessen.
  Native Layoutpruefungen unterscheiden Desktop-DPI, CSS-Media-Breite und
  Bevy-UI-Skalierung, statt diese Einheiten gleichzusetzen.
- Keine leeren Tooltiprahmen nach geleerten Slots. Der native Erstlauf fand
  ausserdem einen realen Layoutfehler durch Auto-Margin plus Endausrichtung;
  die ueberfluessige Margin wurde entfernt und der Client erneut geprueft.
- Moebelgenerator setzt Werkbankfuesse und Truhenboden auf Zell-Y=0 statt
  0.04/0.065 darueber. Geometrieindizes, Bodenkontakt und importierte Modelle
  werden getestet; Holzfarben und gespeicherte Ausrichtung bleiben erhalten.

## Automatische Pruefung

736 Tests bestanden, ein bestehender 180-Sekunden-UDP-Langzeittest ignoriert.
Alle fuenf Workspace-Pruefungen mit unveraendertem Lockfile:

```bash
cargo fmt --all --check
CARGO_BUILD_JOBS=2 cargo check --locked --workspace
CARGO_BUILD_JOBS=2 cargo test --locked --workspace
CARGO_BUILD_JOBS=2 cargo clippy --locked --workspace --all-targets -- -D warnings
CARGO_BUILD_JOBS=2 cargo build --locked --workspace
```

Logs: `fmt.log`, `check.log`, `tests.log`, `clippy.log`, `build.log`.
Tests decken unter anderem neun verteilte Zellen, gemeinsam verwendete
Backing-Slots, ausstehende Antworten, Bidirektionalitaet, Mengenobergrenzen,
atomaren Fehler-Rollback und echte UDP-Inventarreplikation ab.

Dedicated-Server-Smoke ohne DISPLAY/WAYLAND_DISPLAY: 27 Sections, sauberer
Exit. `server-tree.log` enthaelt keine bevy_render/bevy_ui/wgpu/TiltUI/
bevy_winit-Abhaengigkeit.

## Native Pruefung

KDE/Wayland, RX 6950 XT, RADV Vulkan. Isolierte Testprofile und Welten; keine
Benutzerwelt wird fuer die Probe ueberschrieben. Ausgewaehlte Screenshots
unter `captures/` und `inventory-captures/` wurden tatsaechlich angesehen.

`native.log`: Survival am Integrated Server, echte TiltUI-Rezept-/Plus-Handler,
2x2-/3x3-Ausgabeaktionen, fehlende Zutaten mit echtem Signal-Lost-Filter,
OS-Maus-Werkbanktooltip, Holzmoebel, Einzel-/Doppeltruhe und REG3-Reload.

`inventory-native.log`: echte OS-Eingaben ueber den vorhandenen uinput-/KWin-
Helfer, kein Ersatz durch injizierte Picking-Events. Neben den Inventargesten
verteilt die Probe einen 64er-Stapel auf zwei Crafting-Zellen (32/32), sammelt
beide Zellen und gibt den Stack zurueck. Die abschliessende Bilanz umfasst
Inventar UND replizierte Welt-Drops: 128 Items. Zwischenzeitliches legitimes
automatisches Aufsammeln der zuvor gedroppten 63 Items wird mitgezaehlt.
Fenstergrenzen, 95-Prozent-Hoehe, Abstand, Tooltips und Suche werden geprueft.

## Grenzen

Rastermengen verwenden weiterhin reale Inventarslots als Backing. Ist eine
Quelle voll und kein passender freier Backing-Platz verfuegbar, bleibt der
nicht uebertragbare Rest am Cursor. Es wird kein zweites Containerinventar
simuliert. Truheninhalt und Ofen-Backend bleiben in TODO.md offen.

OS-Pointertests benoetigen eine ungestoerte Desktop-Sitzung; ein frueher
Kategorie-Hoverlauf verfehlte das Ziel und wurde nicht als bestanden gewertet.
Keine Mehr-GPU-/4K-Vollabnahme und keine neuen FPS-/p95-/p99-/RSS-/VRAM-
Vergleichsmessungen aus diesen Funktionsproben ableiten.
