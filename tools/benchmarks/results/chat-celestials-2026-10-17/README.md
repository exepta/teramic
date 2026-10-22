# Phase 39: Chat Und Himmelskoerper

## Umfang

TiltUI-Vorschlagsliste mit Pfeilauswahl, Tab/Shift+Tab und Klick. Serverseitige
Zeit-/Koordinaten-/Spielerbefehle, lokale Chat-Loeschung und echte Namensliste
ueber Netzwerkprotokoll 16. Vorhandene Lade-/Delta-/Berechtigungspruefungen
bleiben aktiv; keine zweite Teleport- oder Speicherarchitektur.

Referenzorientierte Sonne mit acht Blockstrahlen und Sichelmond mit gestuftem
Relief im vorhandenen Himmelshader. Keine neuen Renderpaesse oder Einzelvoxel-
Entities. Dekorative Himmelskoerper sind nicht in der Atmosphaeren-Cubemap.
Beide Zeitformate werden ohne Sprachumschaltung akzeptiert. Kein Anspruch auf
pixelidentische Referenzgrafik oder ungemessene FPS-/Speicherparitaet.

## Native Pruefung

RX 6950 XT, RADV/Vulkan. `sky/` enthaelt neun native Aufnahmen fuer Tag,
Daemmerung, Nacht, Sichelmond, Sonne, Fog Off und Unterwasser. Sonne und Mond
wurden geoeffnet und Form/Farbe nachgebessert. `ui/` zeigt den produktiven Chat
bei 1280x800 und 960x640 sowie das reale Teleportziel.

Der native Chat-Smoke prueft Tastatureingabe, sieben gerenderte Vorschlaege,
Pfeil/Tab-Auswahl, Klick auf einen Vorschlag, Eingabesperre, farbige Locate-Zeile,
Escape und autoritativen Teleport. Finaler Lauf ohne Warnungen/Fehler.
Ein erster GUI-Lauf waehrend parallelen Linkens scheiterte mit einem
Verbindungs-Timeout; `ui-first-attempt.log` bleibt als Nachweis erhalten.
Nach Ende der Buildlast liefen beide Wiederholungen erfolgreich.
Ein weiterer Wiederholungslauf deckte eine Reihenfolgeannahme im Smoke-Test
auf: Der erste ECS-Eintrag war nicht zwingend die sichtbare Zeile `day`.
Der Test waehlt jetzt anhand des Zeilentexts; abschliessender nativer Lauf
erfolgreich. `ui-row-order-smoke.log` dokumentiert diesen Testfehler.

Reproduktion vom Repository-Root:

```bash
env -u DISPLAY \
  TERRA_CONFIG_DIR=tools/benchmarks/results/daylight-2026-10-17/configs \
  TERRA_MENU_CONFIG_DIR=/tmp/terra-chat39-menu \
  TERRA_CAPTURE_DIR=/tmp/terra-chat39-ui \
  target/debug/terra-client --smoke-test-chat
```

Fuer Himmel `--smoke-test-daylight` und ein eigenes Capture-Verzeichnis nutzen.
Diese isolierten Smoke-Configs sind keine produktiven Weltdateien. Der Dedicated
Server wurde ohne DISPLAY/WAYLAND_DISPLAY mit `--smoke-test --no-save --bind
127.0.0.1:0` gestartet: 27 Sections erfolgreich, keine Fenster/GPU erforderlich.

## Automatisierte Pruefung

634 Tests bestanden, ein bestehender Langzeittest ignoriert. Neue Tests fuer
24h/AM-PM-Grenzen, Tagesnamen, ungueltige Koordinaten, Namen mit Leerzeichen,
Vorschlagsbedienung und echte Zwei-Client-Kommandos. Zeitreplikation samt
Journal, getrenntes `/clear`, Spielerteleport, Permission-Denial, ungeladene
Luftziele und Ablehnung solider Ziele werden geprueft.

Alle fuenf Cargo-Pruefungen mit `CARGO_BUILD_JOBS=2`: Format, Workspace-Check,
Workspace-Tests, Clippy mit `-D warnings` und Workspace-Build. Reale Internet-
Mehrrechner-Tests und FPS-/Speicherbenchmarks sind nicht Teil dieses Berichts.
