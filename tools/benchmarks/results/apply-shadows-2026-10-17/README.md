# Phase 42: UI Scale Erst Bei Apply Und Daemmerungsschatten

## Aenderungen

`apply_ui_scale` verwendet nur den angewendeten `GraphicsConfig`, nicht mehr
den ungespeicherten `OptionsDraft`. Der Regler zeigt weiterhin den gewaehlten
Wert, verschiebt aber weder sich selbst noch andere Bedienelemente. Apply
uebernimmt und speichert die Groesse; Reset, Back/Verwerfen und fehlgeschlagene
Speicherung skalieren nicht. Der bestehende 15-s-Display-Test darf nach Apply
skalieren und stellt bei Rollback auch die vorherige UI-Groesse wieder her.
Fenster-Fit/HiDPI-Unterstuetzung bleiben bestehen.

Beim bisherigen `direction.y > 0.05`-Shadowmap-Schalter waren noch etwa 47.000
Lux Sonnenlicht aktiv. Laub und Props wurden ploetzlich unbeschattet; erneutes
Ueberqueren dieser Grenze konnte Schatten sichtbar ein-/ausschalten. Die
Shadowmap wird jetzt erst zusammen mit ihrer auf null ausgeblendeten
Lichtenergie deaktiviert, nie auf einem noch leuchtenden Himmelslicht.
Das behebt diesen harten Wechsel, nicht generell jede denkbare texelbedingte
Aliasingquelle. Wind, Masken, Shader und Schattenqualitaet werden nicht
abgeschaltet oder ersetzt. Kein neues Licht/Material und keine neue Pipeline.

Kosten: Bei Sonnen-Nachgluehen kurz unter dem Horizont und beginnendem Mondlicht
koennen die zwei vorhandenen Lichter gleichzeitig Schattenkarten benoetigen,
statt das noch sichtbare Sonnenlicht abrupt unbeschattet zu lassen. Keine
pauschale FPS-Paritaet behauptet. Shadows Off schaltet weiterhin beide aus.

## Pruefungen

CPU-Regression ueber je 1201 Zeitschritte zwischen 5-7 und 17-19 Uhr:
Jedes leuchtende Himmelslicht behaelt seine Shadowmap; bei null Energie darf
sie aus sein. Zusaetzlich Shadows Off, UI-Apply/Reset/Discard/Reload, verweigerte
Speicherung, HiDPI/Fenster-Fit und UI-Rollback bei Display-Timeout.

Der native Menue-Smoke-Test prueft den echten TiltUI-Regler vor und nach Apply,
TOML-Reload und den bestehenden Display-Timeout. Die Beleuchtungsfixture nimmt
zusaetzlich dieselbe Boden-/Baumansicht bei 6:12/6:13 sowie 17:47/17:48 auf,
also unmittelbar beiderseits der alten Schaltschwelle. Alle aktiven
Weltlichter werden dabei auf vorhandene Schatten geprueft; Inventar-
Vorschaulichter sind bewusst ausgenommen. Wind laeuft weiter. Die Aufnahmen
sind Zeitproben, keine vollstaendige Video- oder Langzeit-Flimmerabnahme.

```bash
env -u DISPLAY \
  TERRA_CONFIG_DIR=tools/benchmarks/results/daylight-2026-10-17/configs \
  TERRA_MENU_CONFIG_DIR=/tmp/terra-phase42-lighting \
  TERRA_CAPTURE_DIR=tools/benchmarks/results/apply-shadows-2026-10-17/lighting \
  target/debug/terra-client --smoke-test-daylight
```

Menues: `--smoke-test-menu` mit frischem `TERRA_MENU_SMOKE_DIR` und
`TERRA_MENU_SCREENSHOTS`. Benutzerwelten und Konfigurationen bleiben unberuehrt.

## Ergebnisse

Alle fuenf Cargo-Pruefungen erfolgreich: `cargo fmt --all --check`,
`cargo check --workspace`, `cargo test --workspace`,
`cargo clippy --workspace --all-targets -- -D warnings`,
`cargo build --workspace`. 645 Tests bestanden, ein vorhandener Langzeittest
ignoriert. Buildjobs auf zwei begrenzt. Eine erste fokussierte Teil-Crate-
Testsuite wurde bewusst abgebrochen, weil sie einen unnoetigen, abweichenden
Dependency-Feature-Build erzeugte; anschliessend vollstaendige Workspace-Suite
verwendet und erfolgreich wiederholt.

Native Vulkan-Laeufe auf RX 6950 XT/RADV erfolgreich, ohne CSS-/Shaderfehler:
Menueablauf einschliesslich realem Regler, unveraenderter Groesse vor Apply,
verkleinerter Groesse danach, Speicherung, Verwerfen und Display-Timeout.
Beide UI-Aufnahmen geoeffnet. 19 Beleuchtungs-/Unterwasseraufnahmen; beide
Morgen-/Abend-Schwellwertpaare geoeffnet. In den Zeitproben kein ploetzliches
Verschwinden der Laub-/Prop-Schlagschatten. Um 18 Uhr zwei aktive Schattenlichter
im Log bestaetigt, ohne Pipelinefehler. Headless-Server ohne DISPLAY und
WAYLAND_DISPLAY mit 27 Sections erfolgreich gestartet.

Die Aufnahmen sind keine laufende Video-Abnahme saemtlichen zeitlichen
Aliasingverhaltens. Keine neue FPS-/RAM-/VRAM-Messung; kurzfristige Kosten
ueberlappender Schattenlichter bleiben im technischen Profiling-Backlog.
Logs `terra-phase42-*.log`, Bilder `menu/` und `lighting/` neben diesem Bericht.
