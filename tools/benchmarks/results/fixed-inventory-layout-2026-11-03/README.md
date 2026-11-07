# Feste Inventargroessen und passende Rahmen (Phase 188)

Die Inventare verwenden feste **28 x 28 logische Pixel**, Ergebnisfelder
**32 x 32**. Der globale UI-/DPI-Massstab skaliert weiterhin die gesamte
Oberflaeche. Die Panelabmessungen passen zur Zellenbreite; Character und
Crafting stehen ueber den 18 Rucksack- und neun Hotbarplaetzen. Der Itembrowser
verwendet dieselbe Zellengroesse und blaettert seine verfuegbare Flaeche aus.

Blockinventare haben ihren Arbeitsbereich oben und das Spielerinventar
darunter, ohne Inhalts-Scrollbar. Eine angeschlossene Werkbanktruhe liegt in
einem eigenen Rahmen links neben dem Hauptpanel. Die Gesamtkombination wird
zentriert. Die Tonne steht bei jedem Itembrowser unmittelbar links unten;
im Survival-Modus bleibt sie sichtbar und deaktiviert.

Die HUD-Hotbar verwendet den vorhandenen transparenten Grafikausschnitt
2172 x 320 in einem 352 x 52 grossen Rahmen. Neun 28er-Slots mit zwei Pixeln
Abstand sitzen innerhalb der abgerundeten Enden; ihre Tastenplaetten stehen
mittig darunter. Das Rezeptbuch besitzt eine explizite Mindest-/Maximalhoehe,
damit die allgemeine Buttonhoehe seinen kleinen Rahmen nicht streckt.

FPS 0 heisst `Unlimited` ohne zusaetzlichen `Current: 0`-Eintrag. Eigene Werte
bleiben als `Custom` erhalten. Bei Borderless ist die Aufloesungsauswahl aktiv
und zeigt `Desktop (Borderless)`. Die Auswahl einer festen Aufloesung wechselt
den Entwurf zu Windowed; Apply, Bestaetigung und Ruecknahme funktionieren
weiterhin. Dropdowns erhalten einen kupferfarbenen Chevron mit passender
Offenstellung.

## Pruefung

- Workspace-Library-Tests: **3549 bestanden, 0 fehlgeschlagen, 7 ignoriert**;
  darin **451 UI-Tests**. Neue Regressionen pruefen FPS-Auswahl sowie
  Borderless/Aufloesung inklusive Apply und Ruecknahme.
- `cargo build --workspace --locked -j1`: erfolgreich.
- `cargo clippy --workspace --all-targets --locked -j1 -- -D warnings`:
  erfolgreich.
- `cargo fmt --all -- --check`, `git diff --check` und der echte Tilt-CSS-Parser
  fuer die drei neuen Stylesheets: erfolgreich.
- Native Layoutpruefung: **26 Aufnahmen**, jeweils breit (1680 x 936) und
  kompakt (1024 x 640, mit einem Pixel Wayland-Toleranz). Spielerinventar plus
  Ofen, Brecher, Mischer, Kessel, Lagerfeuer, einfache/doppelte Truhe,
  verbundene Werkbank, gefilterter Trichter, Tunnel, gefilterter Tunnel und
  Verteiler.
- Die native Pruefung misst tatsaechliche ComputedNodes: 28er-Zellen,
  32er-Ergebnisse, vollstaendig sichtbare Slots, passender Inhalt ohne Scroll,
  links angeschlossene Truhe und Tonne neben dem Katalog. Sie vergleicht das
  autoritative Inventar vor/nach der Galerie auf Gleichheit.

Die Galerie nutzt echte UI-Komponenten mit validierten temporaeren
Client-Containerdaten in einer temporaeren integrierten Welt. Sie fuehrt
**keine OS-Maus- oder Tastatureingaben** aus. Vorhandene Spielstaende und die
normalen Benutzereinstellungen werden nicht veraendert. Die Settings-Tests
pruefen Laufzeitfelder und Speicherlogik; eine interaktive Borderless-Umschaltung
des Desktopfensters wurde nicht als Teil dieser Galerie ausgefuehrt.

## Aufnahmen und Protokolle

- Vorlagen: [Layouts](references/layouts.png) und
  [Player Inventar](references/player-inventar.png).
- [Spielerinventar breit](native/fixed-player-wide.png) und
  [kompakt](native/fixed-player-compact.png).
- [Werkbank mit Truhe links](native/surface-workbench-linked-wide.png) und
  [kompakt](native/surface-workbench-linked-compact.png).
- [Kessel kompakt](native/surface-cauldron-compact.png),
  [Doppeltruhe kompakt](native/surface-double-chest-compact.png).
- [Native Pruefung](logs/layout-native.log), [Tests](logs/library-tests.log),
  [Build](logs/build.log), [Clippy](logs/clippy.log).
- `verification.json` enthaelt die Bildabmessungen und SHA-256-Werte der
  Aufnahmen und der relevanten Quelldateien.

Reproduktion nach dem Workspace-Build mit einer eigenen temporaeren Config:

```sh
TERRA_CONFIG_DIR=/tmp/teramic188-ui-config \
TERRA_MENU_CONFIG_DIR=/tmp/teramic188-ui-config \
TERRA_COMPUTE_THREADS=16 \
TERRA_CAPTURE_DIR="$PWD/tools/benchmarks/results/fixed-inventory-layout-2026-11-03/native" \
TERRA_INVENTORY_REFERENCE_SMOKE=1 \
TERRA_SURFACES_SMOKE=1 \
TERRA_LAYOUT_ONLY_SMOKE=1 \
timeout 240 target/debug/terra-client --smoke-test-inventory
```
