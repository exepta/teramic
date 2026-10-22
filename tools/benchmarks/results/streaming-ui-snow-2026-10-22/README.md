# Phase 99: Streaming-Identitaet, UI-Lebensdauer Und Wetter-Schnee

Nachtrag Phase 100: Der hier beschriebene permanent montierte Inventar-Slotbaum
verursachte eine FPS-Regression und ist ersetzt. Nur die leichte Huelle bleibt
montiert; Inhalte werden beim Schliessen entfernt. Dieser Bericht bleibt als
historischer Stand erhalten. [Isolierter Nachweis und Korrektur](../inventory-idle-regression-2026-10-22/README.md).

## Reproduzierter Fehler

`PreparedColumn` speicherte die vollstaendige `WorldConfig`. Der Worker verglich
sie beim naechsten vertikalen Abschnitt oder Remesh mit dem aktuellen Generator.
Seit der Live-Aktualisierung der Spielregeln veraendern sich dort auch Kalender,
Zeitgeschwindigkeit und Schlafquote. Damit wurden gueltige Terrain-Daten
abgewiesen; Abschnitte und aktualisierte Kollisions-/Renderdaten fehlten.

Der neue Test
`prepared_columns_survive_runtime_rule_changes_for_new_sections_and_edits`
schlug vor der Korrektur mit `Err(ColumnMismatch)` fehl. Er verwendet unter
anderem die gemeldete Spalte `[8, 37]`, Sections Y=-2/4 und negative Koordinaten.

## Korrektur

- `WorldConfig::same_terrain` vergleicht ohne Allokationen die Terrain-Parameter
  einschliesslich Welt-ID, Seed, Revisionsstaenden, Klima, Baeumen und Erzen.
  Kalender-, Zeit-, Spiel- und Populationsregeln sind kein Terrain-Cache-Schluessel.
  Exhaustives Destrukturieren erzwingt bei neuen Feldern eine bewusste Zuordnung.
- `SectionGenerator::build_prepared` verwendet diesen Abgleich. Kein Abschalten
  der Validierung, kein Cache-Leeren und keine zusaetzliche Terrain-Generierung
  bei Regelwechseln. Der Streamingtest prueft vorhandene und laufende Worker,
  neu geladene Hoehen und anschliessende Edits mit derselben `Arc<PreparedColumn>`.
- Echte Seed-, Welt-ID-, Meeresspiegel- oder Vegetationsaenderungen werden
  weiterhin abgewiesen. Keine Aenderung bestehender Welt- oder Block-IDs.

## UI Und Schlafen

- Inventar-Slotbaum und Itembrowser bleiben pro Spielsitzung montiert, geschlossen
  aber mit `display: none`. Wiedereroeffnen instanziiert sie nicht erneut.
  Die native Probe vergleicht die echten Entities vor und nach dem Oeffnen.
- Tooltips bleiben ausserhalb von Scrollcontainern. Unvermessene Popups sind
  verborgen. Nach dem Layout werden Popup und Text-Nachfahren noch im selben
  Frame an die berechnete Cursorposition verschoben; das Layout der Folgeframes
  erhaelt dieselben Koordinaten. Keine zweite Tooltip-Implementierung.
- Die native Inventarprobe kontrolliert jeden sichtbaren Tooltip-Frame,
  einschliesslich des ersten Frames, auf den Abstand von zehn UI-Pixeln.
  Geschlossene Popups verursachen keine unveraenderten Visibility-/State-Writes.
- Ablehnungen von Bauaktionen benutzen zwei vordefinierte TiltUI-`toast`-Widgets
  mit dessen Top-Right-Stack: Konflikte und Rate-Limits als Warnung,
  unerlaubte/verdeckte/unerreichbare Aktionen als Fehler. `show_toast` und
  `close_toast` starten deren begrenzte Lebensdauer neu; es entsteht kein
  unbeschraenkter Entity-Strom. Die native Probe kontrolliert auch Styles und
  Layout, nicht nur Koordinaten. Rust-erzeugte Widgets hatten dabei zunaechst
  ihren offenen Zustand nach der Materialisierung verloren. Verbindungsabbruch
  behaelt seine gesonderte Wiederverbinden-Aktion.
- Neue Betten setzen das Kopfende vom Spieler weg in Blickrichtung. Bestehende
  Bettzustaende werden nicht umorientiert. Tests decken alle vier Richtungen,
  negative Winkel, gefaerbte Gegenstaende und gepaarte Unterstuetzung ab.
- Schlafen oeffnet den Chat und blendet den echten TiltUI-Old-Film-Filter mit
  einer sanften Abdunklung bis 80% Alpha ein. Beim Aufwachen blendet er aus.
  Ein automatisch geoeffneter leerer Chat schliesst dann; angefangene Nachrichten
  und ein bereits vorher geoeffneter Chat bleiben erhalten.
- Kalendernadel: silberne Fuellung mit dunkelsilberner Kontur. Die Buchstaben
  verwenden die tatsaechlichen Rautenpositionen der Rahmentextur; die sichtbaren
  Glyphen werden unabhaengig von Font-Ober-/Unterlaengen zentriert.
- Das Multiplayer-Icon im Hauptmenue verwendet nun auf beiden Seiten denselben
  gueltigen Namen `users-round`.

## Schnee

- Winterbeginn erzeugt weder eine globale Shaderdecke noch eine Mindestauflage.
  Nur tatsaechlicher Niederschlag laesst Schnee wachsen; vorhandener Schnee
  bleibt im Winter erhalten. Staerkere Schneestufen lagern schneller ab.
- Die zeitbudgetierte Spaltenarbeit schreibt einen gerade gestarteten Schneefall
  nicht rueckwirkend ueber vorheriges trockenes Wetter fort. Die bisherigen
  acht physischen Hoehenstufen, Spielerdaecher und Abschnittsbudgets bleiben.
- Betten, Oefen, Truhen und Werkbaenke behalten keine natuerliche Auflage und
  sind von der globalen Shaderbedeckung ausgenommen. Alte als wettererzeugt
  markierte Auflagen koennen entfernt werden; Spielerbloecke werden nicht geloescht.
- Natuerliches Stapeln endet weiterhin am vollen Schneeblock. Spieler koennen
  Schnee weiterhin manuell stapeln. Keine neue Generatorversion oder Retrogen.

## Native Pruefungen

RX 6950 XT / RADV / Vulkan, Ryzen 5900X, bestehender optimierter Dev-Build.
Isolierte Konfigurationen, echte integrierte UDP-Autoritaet; keine Benutzerwelt
wurde fuer die Fixtures geaendert. Konkrete Laeufe stehen in den Logs.

- Creative-Inventar: echte OS-Maus, Katalog, MouseTweaks, Crafting-Grid,
  Scrollen, Tooltip-Positionen, Muelltonne, Schliessen/Wiederoeffnen und HUD
  unter Pause/Optionen; elf Aufnahmen.
- Bett: gepaarte Platzierung, eine Survival-Faerbung, replizierte Liegepose,
  geoeffneter Chat und Filmblende, anschliessend naechster Morgen 08:00.
- Kalender: vier Jahreszeiten, gemeinsame Luft-/Stamina-Leiste und trockener
  Winter ohne spontan erscheinende Schneedecke; vier Aufnahmen.
- Hauptmenue: Multiplayer-Icon visuell geprueft. Die vollstaendige Menueprobe
  umfasst Einstellungen, Eingaben, Serverliste, Speichern und Display-Rollback.
  Die erste Probe griff vor Abschluss der Audio-Control-Materialisierung zu;
  der Pruefer wartet nun auf den echten Control-Baum. Der Wiederholungslauf besteht.
- Dedicated Server: ohne DISPLAY/WAYLAND_DISPLAY, ohne Speichern, 27 Sections.
- Zusaetzliche Toastprobe: echte vom Server abgewiesene Aktion, semantischer
  Fehlertyp, offener Widgetzustand, angewandtes Theme, Spaltenlayout und
  Top-Right-Koordinaten. Lesbarkeit und Schliessen-Symbol sind nativ geprueft.

Beispielaufnahmen:
[Tooltip](native/inventory-hotbar-tooltip.png),
[Inventar](native/inventory-open.png),
[Bett und Chat](native/bed-sleeping-pose.png),
[Aufgewacht](native/bed-morning-awake.png),
[Trockener Winter](native/calendar-hud-3.png),
[Hauptmenue](native/main-menu.png),
[Fehlertoast](native/network-error-toast.png).

## Workspace-Pruefungen

Alle folgenden Befehle bestehen auf dem abschliessenden Stand:

```sh
cargo fmt --all --check
cargo check --locked --workspace
cargo test --locked --workspace
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo build --locked --workspace
```

1678 Tests erfolgreich, zwei bestehende Langzeittests ignoriert. Builds liefen
mit `CARGO_BUILD_JOBS=1`. Die vollstaendigen Ausgaben stehen in `fmt.log`,
`check.log`, `tests.log`, `clippy.log` und `build.log`; native Laeufe in den
entsprechend benannten `*-native.log` beziehungsweise Inventarlogs.

Keine `ColumnMismatch`-/Section-Generierungsfehler in den geprueften nativen
Laeufen. Im letzten Toastlauf erschien einmal die bereits bestehende
Lightyear-Joinwarnung zu vorzeitig empfangenen `CreatureStates`; dies ist kein
Nachweis eines neuen Queue-Leaks und wurde nicht als behoben ausgegeben.

## Einordnung

Keine Sichtweite, Vegetationsdichte oder Wasser-/Schattenqualitaet wurde reduziert.
Der Regressionsnachweis betrifft den konkreten Cache-Fehler und die geprueften
UI-/Wetterpfade, keine allgemeine FPS- oder Speicherparitaet. Es wurden wie
gewuenscht keine 10-/15-/30-Minuten-Laeufe durchgefuehrt. Die verbleibenden
Streaming-Spitzen und langfristige Grafikabnahme bleiben gesondert im Backlog.

Die Schnee-Nachfuehrung bleibt auf geladene Spalten begrenzt. Es gibt keine
vollstaendige historische Rekonstruktion aller Wettergebiete. Schlafkollision
verwendet weiterhin die vorhandene sichere aufrechte Kapsel am Einstiegspunkt.
