# Phase 47: Inventar, Rezepte, Strukturmaterialien Und Holz

17.10.2026. Rust 2024, Bevy 0.19.1, TiltUI 0.1.0, Avian 0.7.0 und
Lightyear 0.30.1 unveraendert. Weltformat 2, Generator 5, Seed und Block-IDs
bleiben erhalten. Client und Server verwenden gemeinsam Protokoll 21.

## Implementierung

- `struct_block` ist Teil der bestehenden gemeinsamen BlockProperties und der
  Block-JSONs. Werkbank/Truhe erhalten Gras; betroffene Props werden weiterhin
  ueber denselben EditJournal-Pfad entfernt. Kein paralleler Registry-Cache.
- Gemeinsamer TiltUI-Inventarrahmen in Survival/Creative: 18 Backpack-Slots,
  neun Hotbarslots, persoenliches 2x2 bzw. Werkbank-3x3, rechter Itembrowser,
  Kategorien, Suche, Seiten und Rezeptinspektor. Keine separate Creative-Seite.
- Survival verwendet den echten Server-Rezeptkatalog. Auto-Fill nur mit
  vorhandenen Zutaten und passender Station; fehlende Zutaten bleiben als rote
  pulsierende Rezeptvorschau mit echtem TiltUI-Signal-Lost-Filter sichtbar.
- Rasterfelder reservieren konkrete Mengen aus vorhandenen Inventarslots.
  Servervalidierung summiert Mehrfachverwendung; Fertigung ist atomar.
  Ergebnis anklicken fertigt zum Cursor, Shift-Klick ins Inventar. Groessere
  Rasterstacks ersetzen Plus-/Minus- und Craft-Max-Schaltflaechen.
- Exakte Cursorentnahme und geschuetzte bestehende Sammel-/Wheel-/Shift-
  Operationen erhalten reservierte Zutaten. Ungueltige Transaktionen werden
  komplett verworfen. Pending-Zustaende und zwei Revision-Retries stimmen mit
  der vorhandenen Requestqueue ueberein; schnelle Rezeptwechsel ueberschreiben
  keine noch unbestaetigten Mengen. Q/Shift-Q lassen Rasterzutaten stehen.
- Creative gibt echte Serverstacks: ein Item, mit Shift 128. Freie Hotbarslots
  haben Vorrang. Browserbilder zeigen keine erfundenen endlichen Creativecounts.
- Zehn eindeutige Grundrezepte unter `assets/crafting/`; gemeinsame Holzrezepte
  statt acht redundanter Varianten. `wood: true` akzeptiert Bretter 16/17/18.
  Die verbrauchte Mehrheit bestimmt Moebelholz; Gleichstand Eiche/Birke/Fichte.
- Die angehaengte WoodenBlock-Itemvariante behaelt Inventar-/Cursor-/Dropdaten
  kompatibel. Objektzellen speichern die Holzart im bestehenden Journal.
  REG3 schreibt Holz; REG2/REGN werden explizit als alte Eichenzustaende gelesen.
  PLY3 und bestehende Spielerstaende bleiben lesbar.
- Holz wird selektiv ueber Vertexfarben innerhalb gemeinsamer Materialbatches
  dargestellt, inklusive echter 3D-Previews und Drops. Metall/Tuch bleiben gleich.
  Gemischte Doppeltruhen teilen Holzflaechen an der Zellmitte mit stetigen UVs;
  beide Holzarten bleiben sichtbar, ohne Material-/Entityduplikate.
- Birkenlaub ist eichengruener; schmalere asymmetrische Near-/Middle-Kronen
  verwenden denselben gebatchten Wind-/Maskenpfad und geteilte Vorlagen.

## Automatische Pruefung

727 Tests bestanden, ein bestehender Langzeit-Netzwerktest ignoriert.
Die vollstaendigen letzten Befehlsausgaben stehen in den entsprechenden Logs:

```bash
cargo fmt --all --check
CARGO_BUILD_JOBS=2 cargo check --workspace
CARGO_BUILD_JOBS=2 cargo test --workspace
CARGO_BUILD_JOBS=2 cargo clippy --workspace --all-targets -- -D warnings
CARGO_BUILD_JOBS=2 cargo build --workspace
```

`fmt.log`, `check.log`, `tests.log`, `clippy.log`, `build.log`.
Coverage: datengetriebener Bodenerhalt und Propentfernung an negativen
Section-Grenzen; gemischtes Holz/Mehrheiten; Rastermengen/atomare Fehler;
geschuetzte Cursor-/Sammel-/Wheel-/Shift-/Drop-Gesten; ausstehende Bestaetigungen;
REG3-/REG2-/REGN- und Spieler-Roundtrips; echte UDP-Rezept-/Inventarreplikation;
Farb-/UV-/Flaechenkontinuitaet gemischter Truhen; geschlossene Birken-Micro-
Geometrie bei gemischten Arten/LODs; echte TiltUI-CSS-Parser und Slot-Events.

## Native Pruefung

KDE/Wayland, RX 6950 XT, RADV Vulkan (Mesa 26.2.4). Desktop-Skalierung 150 %.
Fensteraufnahmen sind keine 4K-Fullscreen- oder Mehr-GPU-Abnahme.

`native-final.log`: Survival am echten Integrated Server. Sidebar-Rezeptklicks
laufen ueber materialisierte TiltUI-Handler; fehlende Zutaten besitzen den
Signal-Lost-Effekt im RuntimeComputedStyle. 2x2-/3x3-Fertigung, vier Werkzeuge,
Birken-Werkbank, Einzel-/Doppeltruhe, Netzwerkzustaende und binaerer Reload
werden geprueft. Der Werkbank-Ergebnis-Tooltip wird mit echtem OS-Mauszeiger
geoeffnet und auf Text, sichtbare Lage und zehn UI-Pixel Abstand geprueft.
Vorschaubilder und platzierte Birkenmoebel wurden tatsaechlich angesehen.

Creative verwendet dieselbe Ansicht; die OS-Mausprobe prueft direkte Ein-Item-
Hotbarentnahme, anschliessend Pickup, Split, Einzelablage, gleichmaessiges
Ziehen, Sammeln und Aussendrop. Der separate 128er-Konservationsteil wird nach
dem echten Ein-Item-Nachweis ausdruecklich serverseitig vorbereitet:
65 im Inventar + 63 repliziert gedroppt = 128. Das ist kein erfundener
nativer Shift-128-Nachweis; Shift-128 ist separat in Aktionstests abgedeckt.

`native-creative-probe-final.log` ist der erfolgreiche komplette Creative-Lauf,
einschliesslich Suche, Kategorien, Slot-Tooltips und 960x641-Kompaktansicht.
`native-survival-final.log` bestaetigt denselben Rahmen mit Survival-Sidebar,
echten Tooltips, Suche und kompakter Ansicht. Ausgewaehlte Bilder unter
`creative-captures/` und `survival-captures/` wurden tatsaechlich angesehen.

Ein erster Creative-Lauf fand bei 960x641 physischen Pixeln einen echten
Kompaktlayoutfehler: die Sidebar wurde unter den Rucksack gestapelt. Die
entsprechenden CSS-Breakpoints wurden korrigiert, die Viewportpruefung nicht
abgeschwaecht. Kleinere feste Raster erhalten beide Bereiche nebeneinander.
Weitere Mausproben fanden veraltete KWin-Geometrieantworten im asynchronen
Journal der Testhilfe. Die Probe verwendet jetzt Zeitstempel fuer frische
Antworten und echte Cursorbewegung nach Fensteraktivierung. Keine Hover- oder
Picking-Events wurden zur Umgehung des nativen Tests injiziert.

Headless: `headless.log`, ohne DISPLAY/WAYLAND_DISPLAY, 27 Sections und sauberer
Exit. `server-tree.log` enthaelt keine bevy_render/bevy_ui/wgpu/TiltUI/
bevy_winit-Abhaengigkeit.

## Reproduktion

Frische temporaere Menue-/Weltordner verwenden:

```bash
env -u DISPLAY TERRA_CRAFTING_SMOKE=1 TERRA_NATIVE_OS_POINTER=1 \
  TERRA_CONFIG_DIR=tools/benchmarks/results/daylight-2026-10-17/configs \
  TERRA_MENU_CONFIG_DIR=/tmp/terra47-menu-fresh \
  TERRA_CRAFTING_SMOKE_DIR=/tmp/terra47-world-fresh \
  TERRA_CAPTURE_DIR=tools/benchmarks/results/inventory-browser-2026-10-17/captures \
  target/debug/terra-client --smoke-test-inventory
```

Ohne TERRA_CRAFTING_SMOKE startet derselbe Schalter die separate Inventar-
Mausprobe. `TERRA_INVENTORY_MODE=survival` prueft deren Survivalansicht.
Die optionale physische Mausprobe benoetigt vorhandenen uinput-/KWin-Zugriff;
sie injiziert keine Bevy-Picking-Events.

## Grenzen

Truhen besitzen weiterhin noch kein Containerinventar; ein Ofen-Backend fehlt.
Der gemeinsame UI-Rahmen/Browser ist dafuer wiederverwendbar, bietet aber keine
Fake-Container oder nicht vorhandenen Stationsrezepte. Diese Arbeiten bleiben
in TODO.md offen. Birkenkronen wurden geometrisch getestet; keine vollstaendige
gemeinsame Wald-Art-Abnahme wird aus Moebelscreenshots abgeleitet.

TiltUI budgetiert animierte Effektbereiche; bei eigenen Rezeptpacks mit sehr
vielen gleichzeitig fehlenden Zutaten gilt dessen vorhandenes Effektbudget.
Weitere physische GPUs und extreme UI-Skalen sind nicht abgenommen.
Es wurden keine vergleichbaren FPS-, p95-/p99-, RSS- oder VRAM-Benchmarks fuer
diesen Schritt erhoben. Geteilte Materialien/Vorlagen bedeuten keine behauptete
allgemeine Performanceparitaet.
