# Phase 44: HUD-Layout, Gamemode Und Schatten

Datum: 2026-10-17. Linux/Wayland, AMD Radeon RX 6950 XT,
RADV/Mesa 26.2.4, Vulkan. Kein Dependency-Upgrade; Bevy 0.19.1
bevy_anti_alias ist neu als Client-Renderfeature aktiviert.

## Implementierung

- TiltUI-Survival-HUD mit Gesundheits-/Ausdauerbalken und Keule/Prozentwert,
  Standard oben links; vorhandene Hunger-Bissstufen bleiben erhalten.
- Options > Graphics > HUD Layout > Edit Layout: Statuscontainer ziehen,
  Reset, Cancel/Escape, Save. Hotbar nicht beweglich oder bedienbar.
  Werkzeugleiste weicht in eine freie Ecke aus. Grafikeinstellungen werden
  nicht nebenbei angewendet; nur hud_position wird gespeichert.
- graphics.toml: relative Position [0..1, 0..1], Default [0, 0], bestehende
  Konfigurationen kompatibel. Bounds beruecksichtigen UI Scale und Sauerstoffzeile.
- /gamemode survival|0|creative|1: eigener Spieler, serverseitige game_mode-
  Berechtigung, Inventar erhalten, alte Inputs verwerfen, Abbau/Fallhistorie
  beenden. Lokaler Besitzer automatisch berechtigt; Dedicated explizit je UUID.
- PLY3-Spielerdaten mit Modus-Override und expliziter PLY2-/PLYR-Migration.
  Welt-/Generator-/Block-IDs unveraendert, Netzwerkprotokoll 18.
- Mittel/Hoch: TAA plus Temporal-Schattenfilter, maximal 16 statt 24 m erste
  Kaskade, 25 statt 20 Prozent Ueberlappung. Kartenaufloesung, Kaskadenzahl und
  maximale Schattenreichweite unveraendert. Niedrig/Aus: Gaussian/MSAA.
  Verlauf bei Teleport, starken Kameradrehungen und Zeitspruengen zuruecksetzen.

## Verifikation

Alle fuenf Cargo-Pruefungen erfolgreich: fmt --all --check, check --workspace,
test --workspace, clippy --workspace --all-targets -- -D warnings und
build --workspace. 668 Tests erfolgreich, ein vorhandener Langzeittest ignoriert.
Logs liegen als terra44-*.log in diesem Ordner.

Neue Tests decken skalierte/clampende Drag-Koordinaten, Save/Reload, Cancel,
getrennte Options-Entwuerfe, fehlgeschlagenes Speichern, Toolbar-Ausweichen,
echte TiltUI-CSS-Syntax, Modus-Aliase/Vorschlaege, Zwei-Spieler-Rechte,
Reconnect, veraltete Flugpakete, PLY2-/PLY3-Daten und TAA-Qualitaets-/Resetregeln ab.

Native HUD-Runden mit echtem Integrated Server und UDP: Ausgangswerte aus Save,
fuenf Hunger-Bissstufen, Essen, Erschoepfung, Sturztod, TiltUI-Respawn,
Graphics-Button, Statuscontainer ziehen, Save und TOML-Reload, Creative/Survival.
Ereignisse werden im laufenden Bevy/TiltUI-Client injiziert; kein manueller
Maus-/Tastaturtest behauptet. Die letzte Runde prueft auch, dass das Stylesheet
am gerenderten Container angekommen ist.

`hud/`: erste erfolgreiche Runde bei 1280x800. `hud-final/`: finaler Stand bei
960x640 und angeforderter UI Scale 150 Prozent (vorhandene Viewport-Anpassung
begrenzt den effektiven Faktor). Beide Groessen wurden geoeffnet; finale
Editor-Buttons passen einzeilig und ueberdecken das HUD nicht.
Zwischenaufnahmen aus der Layout-Entwicklung sind keine finalen Abnahmen.

`daylight/`: 25 native Aufnahmen, darunter Tag, Morgen, Abend, Nacht,
Unterwasser, Himmel und sechs laufende Sonnenstaende. In der korrigierten
Bewegungssequenz steigt die Uhr von 17.850807 auf 18.106500 Stunden; die
Sonnen-/Mond-Ueberlappung wird tatsaechlich durchlaufen. TAA-Komponente und
nichtleere Screenshot-Pixel werden geprueft. Tag, Unterwasser und die erste/
letzte Bewegungsaufnahme wurden geoeffnet. Keine Shaderfehler im finalen Log.

Headless-Server-Smoke erfolgreich mit 27 Sections. Der separate normale
Abhaengigkeitsbaum von terra-server enthaelt weder bevy_render/PBR/UI/anti_alias
noch TiltUI. Die Client-Feature-Erweiterung ist keine neue Server-GPU-Pflicht.

## Reproduktion

Je HUD-Durchlauf ein neues Weltverzeichnis und eine frische Menu-Konfiguration
ohne verschobene hud_position verwenden. Die archivierten TOMLs zeigen teils
absichtlich die danach gespeicherte Position.

```bash
env -u DISPLAY \
  TERRA_CONFIG_DIR=tools/benchmarks/results/daylight-2026-10-17/configs \
  TERRA_MENU_CONFIG_DIR=/tmp/terra44-menu-fresh \
  TERRA_SURVIVAL_SMOKE_DIR=/tmp/terra44-world-fresh \
  TERRA_CAPTURE_DIR=/tmp/terra44-hud-captures \
  target/debug/terra-client --smoke-test-survival

env -u DISPLAY \
  TERRA_CONFIG_DIR=tools/benchmarks/results/daylight-2026-10-17/configs \
  TERRA_CAPTURE_DIR=/tmp/terra44-shadow-captures \
  target/debug/terra-client --smoke-test-daylight
```

## Grenzen

Kein identisches Vorher-/Nachher-FPS-, p95/p99-, RSS- oder VRAM-Messpaar in dieser
Phase. TAA benoetigt Verlaufstexturen und einen zusaetzlichen Fullscreen-Pass,
ersetzt hier aber MSAA. Keine allgemeine Performance-Paritaet behauptet.
Die Aufnahmen ersetzen weder eine menschliche Bewegungsabnahme noch lange
Streamingtests auf mehreren GPUs. Besonders transparente Wasseroberflaechen
und Blend-Fades koennen zeitliche Nachzieheffekte zeigen; weiter im TODO.
Lokale UUIDs sind weiterhin keine verifizierten Online-Accounts.
