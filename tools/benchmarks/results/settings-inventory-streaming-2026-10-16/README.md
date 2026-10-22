# Phase 33: Einstellungen, Inventar Und Streaming

Datum: 16.10.2026. Rust/Bevy/TiltUI-Versionen, Generator 5, Weltformat 2,
Protokoll 14 und bestehende IDs bleiben unveraendert.

## Umsetzung

- Aufloesungen aus `OnMonitor`/Primaermonitor, physischen Abmessungen und
  Videomodi, mit deduplizierten passenden Fensterformaten einschliesslich 4K.
  Fensterabmessungen sind keine Monitormodi. Exklusives Fullscreen beharrt auf
  echten Modi; Borderless und 15-Sekunden-Rollback bleiben erhalten.
- Dynamische Optionen liegen in einem echten TiltUI-Gruppenknoten im Popup.
  Die TiltUI-0.1-Optionsbeschriftung wird nach Text-Binding synchronisiert,
  einschliesslich spaeter gesetzter Auswahl. Keine UI-Framework-Abhaengigkeit dazu.
- Beide Wasserarten zeichnen keine Auswahlumrandung. Picking/Quellenersetzung
  und die absichtlich feste SolidWater-Kollision werden nicht abgeschaltet.
- Feste Platzierung ueber Grass schreibt im vorhandenen autoritativen Journal
  zusaetzlich Dirt darunter. Grenzen bei negativen Koordinaten, Replikation und
  Section-Neugenerierung verwenden denselben Pfad. Wiederabbau begruent nicht.
- Einzeilige Detailskennung mit gemessener Breite: nur bei Platzmangel ohne
  Namespace, Unterstriche bleiben erhalten. Herkunfts-Tooltip `von teramic` bzw.
  `von my_mod`. Vorhandene fremde Namespaces bekommen keinen Teramic-Praefix.
- Kategoriequadrate 54/48/42 -> 76/67/59 UI-Pixel, Bilder entsprechend groesser.
  Alle authored TiltUI-Tooltips folgen dem Cursor mit zehn UI-Pixeln Abstand.
  An Fensterrandern weichen sie nach links/oben aus und bleiben im Viewport.
- Creative-Katalog links/rechts: 1 auf Cursor; Shift links/rechts: bis 128 als
  bisheriges Quick-Move in den Bestand. Tastaturauswahl ebenfalls 1 ohne Shift.
  Middle-Pick bleibt das getrennte Creative-Kopierwerkzeug. Catalog-Vorschauen
  zeigen weiterhin keine endlichen Mengen; reale Spieler-/Cursorstacks schon.
- Sichtweitenvalidierung und Slider bis 32 Chunk-Spalten. Vorhandenes Netzwerk
  unterstuetzt 32 bereits, keine Protokollaenderung. Defaultdistanz unveraendert.
- `fog_distance`: 0 aus, sonst bis 150 Prozent der bisherigen Oberflaechendistanz;
  Default 100 Prozent. Unterwassernebel bleibt unabhaengig erhalten.
- `chunk_animation`: `none` (Default), `fade` (300 ms), `fade_slow` (750 ms).
  Terrain-, Pflanzen- und Wasserbatches erhalten beim Laden temporaere Material-
  varianten und beim Entladen eine verzogerte Mesh-Freigabe. Danach werden
  Originalhandles wiederhergestellt bzw. Assets entfernt. Uebergangs-Blending
  wirft keine falschen Vollschatten. Limit: 96 gleichzeitig animierende Wurzeln,
  danach sofortiger Wechsel. Keine weitere Entity je Pflanze/Microvoxel.
  LOD-/Edit-Tausch bleibt atomar; kein neuer Dither-/Tiefenpass, Uploadbudgets
  und asynchrone Generierung bleiben bestehen.

## Automatische Nachweise

Alle folgenden Befehle erfolgreich; vollstaendige Protokolle unter `logs/`:

```bash
cargo fmt --all --check
CARGO_BUILD_JOBS=2 cargo check --workspace
CARGO_BUILD_JOBS=2 cargo test --workspace
CARGO_BUILD_JOBS=2 cargo clippy --workspace --all-targets -- -D warnings
CARGO_BUILD_JOBS=2 cargo build --workspace
```

582 Tests bestanden, ein vorhandener Netzwerk-Langzeittest ignoriert.
Gezielt geprueft: 4K-Monitor bei 150 Prozent Skalierung auch ohne aufgelistete
Videomodi, unveraenderte Defaults, 32er-Grenze, ungueltige Nebelwerte, TOML-
Roundtrip/Apply/Discard, Wasserumrandung, Grass/Dirt ueber negative Section-
Grenzen mit Delta-Replay/Neugenerierung, benachbarte Pflanzen, Creative-Mengen,
fremde Namespaces, gemessene Kennung ohne Umbruch, Cursor-/Randpositionierung,
Fade-Materialwiederverwendung und Mesh-Lebensdauer.

Erste Laeufe fanden eine nicht unterstuetzte CSS-Eigenschaft, alte 128er-Test-
erwartungen, eine alte Pflanzen-Tombstone-Annahme sowie echte native Dropdown-
Layout-/Beschriftungsfehler. Diese wurden behoben; `*-final.log` bezeichnet
die bereinigten Laeufe, fruehere Logs werden nicht als Erfolge ausgegeben.

## Native Nachweise

Linux/KDE Wayland, RX 6950 XT, RADV Mesa 26.2.4. Desktop besitzt zwei echte
3840x2160-Monitore bei 150 Prozent Skalierung. Menue-/Inventartests verwenden
kleinere Fenster; dies ist keine behauptete 4K-Fullscreen-/FPS-Abnahme.

- `--smoke-test-menu-session`: echte TiltUI-Seiten/Handler, alle dynamischen
  Aufloesungsoptionen materialisiert, geoeffnetes scrollbares Dropdown mit
  lesbarer Auswahl, Grafik/Nebel/Fade-Apply, Controls, Serverliste, Display-
  Timeout, integrierter Host, autoritativer Edit, Save, Hostneustart und Reload.
  Erfolgreich, `logs/native-menu-final.log`.
- `--smoke-test-inventory` mit `TERRA_NATIVE_OS_POINTER=1`: echter OS-Mauszeiger
  und TiltUI-Hittest, groessere Tabs, Cursor-Tooltips mit real gemessenen
  Transformen, normale Katalogentnahme mit genau 1 bestaetigt, lange Pilzkennung
  ohne Namespace/Umbruch und echter Herkunfts-Hover geprueft. Danach Split,
  Rechts-Einzelablage, gleichmaessiges Ziehen, Sammelziehen und Aussen-Drop:
  65 im Inventar + 63 als replizierter Drop = 128. Dieser bestehende
  Mengenerhaltungsteil wird nach der Einzelerkennung explizit serverseitig auf
  128 vorbereitet; er ist kein vorgetaeuschter nativer Shift-Klick-Nachweis.
  Shift-128 ist separat in den UI-/Server-Aktionstests abgedeckt.
- `--smoke-test-fluid` mit beiliegenden Fade-Konfigurationen und
  `TERRA_FLUID_HEIGHT=30`: Shader/Vulkan ohne Fehler, Wasser durch abgebaute
  Section-Grenze (70 Einheiten im rechten Testbecken), wieder geschlossene
  Barriere, drei Aufnahmen, sauberer Exit. Keine neue Wassersimulation erfunden.
- `terra-server --smoke-test` ohne DISPLAY/WAYLAND_DISPLAY: 27 Sections,
  sauberer Exit. Separater Server-Build und isolierter Abhaengigkeitsbaum;
  keine bevy_render/bevy_ui/wgpu/TiltUI/bevy_winit-Abhaengigkeit im Serverbaum.

Ausgewaehlte Screenshots unter `captures/` wurden tatsaechlich geoeffnet und
geprueft, darunter Grafikseite, Dropdown, Kategorie-Hover, Inventar gross/klein,
Pilzkennung und Herkunfts-Tooltip. Reproduktion mit den beiliegenden `configs/`;
fuer Menue-/Profilpfade stets einen frischen temporaeren Ordner verwenden.

## Grenzen Und Performance

Keine neue Vorher-/Nachher-FPS-, p95/p99-, GPU- oder RSS-Paritaet behauptet.
32 ist die neue waehlbare Obergrenze, nicht ein ungefragt erhoehter Default.
Gegenueber Radius 16 kann die horizontale Spaltenzahl von 33x33 auf 65x65 steigen.
Mehr Sichtweite kostet daher Ladezeit und Speicher; die bisherigen Worker-/Upload-
budgets bleiben begrenzt. Die optionalen Blend-Materialien koennen waehrend des
Fades Batching reduzieren und GPU-Kosten erhoehen. Danach verschwinden diese
Varianten; bei `none` werden sie gar nicht erzeugt. Dauerprofilierung dichter
32er-Welten und verschiedener GPUs bleibt ausdruecklich in TODO.md offen.

Die Herkunftsanzeige ist namespacetauglich, aber kein neu erfundenes Mod-Loading-
Backend. Ein Welt-Edit auf Dirt ist persistent, jedoch kein zeitbasierter
Grasswachstums-/Lichtsimulator. Die Wasser-Auswahlumrandung ist nicht die
physikalische SolidWater-Kollision. Screenshots ersetzen keine Langzeitabnahme.
