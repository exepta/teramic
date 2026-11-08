# Phase 214: Dunkle Inventarvorschauen

Der Startup-Fehler `inventory preview remained blank after shader readiness
grace` entsteht bei sichtbaren dunklen Bildern durch die bisherige
Helligkeitsschwelle: Ein Pixel wurde nur bei Alpha groesser 8 und mindestens
einem RGB-Kanal groesser 8 gezaehlt. Der erste fehlgeschlagene Eintrag sperrt
den Ladebildschirm; weitere Eintraege versuchen jeweils zehn Sekunden ihre
Vorschau zu erzeugen.

Die Vorschaukamera loescht ihr Ziel transparent. Ihr Tonemapping behaelt den
Alpha-Kanal bei. Die Pruefung zaehlt deshalb jetzt alle Pixel mit Alpha groesser 8,
unabhaengig von RGB. Mehr als 64 sichtbare Pixel sind weiterhin erforderlich.
Eine schwarze deckende Flaeche ist gueltig; eine transparente Flaeche und
kleine Stoerflecken bleiben ungueltig. Die Startbarriere fuer echte Fehler
bleibt erhalten.

## CPU-Regressionen

- 64 gedeckte Pixel werden abgelehnt, 65 akzeptiert, auch bei RGB 0/0/0.
- Niedrige Helligkeit und teilweise Deckkraft sind RGB-unabhaengig gueltig.
- Alpha 0, Alpha 8 und fehlende Pixeldaten werden abgelehnt.
- Ein synthetisches schwarzes Bild durchlaeuft den echten registrierten
  `ScreenshotCaptured`-Observer und beendet den endlichen Vorladedurchlauf ohne
  Fehler; das richtige Zielbild wird veroeffentlicht.
- Ein synthetisches transparentes Bild erzeugt keine fertige Vorschau. Es wird
  erneut versucht und bei Ablauf weiterhin als Fehler an die Startbarriere
  gemeldet.

Diese Tests verwenden nur In-Memory-Bilder und ECS-Ereignisse. Sie nehmen keine
Screenshots auf und initialisieren kein Grafikbackend.

## Modelle

Die erweiterte Asset-Pruefung kontrolliert alle 84 echten GLTF-Zugmodelle auf
Dreieckswindung, Normalen und projizierte sichtbare Flaeche aus dem vorhandenen
128x128-Vorschauwinkel. Alle 84 besitzen mehr als 64 gedeckte Pixel; der kleinste
Wert ist 252. Bei den gemeldeten Teilen sind es 2.449 bis 5.331 Pixel. 22.413
Dreiecke zeigen zur Kamera. Opaque Materialien besitzen Alpha 1. Es wurden keine
Modelle neu generiert; die Modellpruefung macht keine Aussage ueber GPU-Beleuchtung.

## Verifikation

Die Pruefungen laufen ohne Spielclients, Spielfenster oder GPU-Tests. Cargo
verwendet einen Job mit niedriger CPU-/I/O-Prioritaet und entfernten
`DISPLAY`-/`WAYLAND_DISPLAY`-Variablen.

```sh
cargo test --locked --workspace --lib previews:: -- --test-threads=4
cargo build --locked --workspace
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
git diff --check
python3 tools/assets/lint_train_parts.py --log target/verification214/asset-lint.log
```

Alle **52 Vorschau-Tests** bestehen, kein Fehler. Die zwei neuen Regressionen
pruefen dunkle/transparent fehlschlagende Bilddaten und die wirkliche
Veroeffentlichung samt endlicher Startup-Barriere. Clippy aller Workspace-Targets
mit `-D warnings`, Format und die erweiterte Asset-Pruefung bestehen.

Der aktuelle vollstaendige Workspace-Build von Client und Server besteht
einschliesslich Linking (2 min 30 s). Die Programme wurden nicht gestartet.
`verification.json` enthaelt Pruefergebnisse sowie Quell- und Protokollpruefsummen.
