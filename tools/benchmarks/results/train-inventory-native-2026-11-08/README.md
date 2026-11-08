# Gerenderte Zug-Inventare

Die urspruenglichen dynamischen Button-IDs wurden vom Runtime-Binding nicht
als ElementId uebernommen. Statische IDs mit typisiertem TrainIntent ersetzen
diese Bindings fuer Slots, Fahrzeugauswahl und Fahrhebelstufen.

Die Live-Fixture laeuft im echten Vulkan-Client mit dem produktiven TiltUI-DOM,
Layout und Picking. Sie wartet auf alle 45 materialisierten Slots und emittiert
Pointer-Press/Release an berechneten Controlpositionen. Es erfolgt kein direkter
Aufruf des Klickhandlers. Die kontrollierten Serverantworten verwenden die echte
Inventartransaktion; dieser UI-Lauf ist kein kompletter UDP-Integrationstest.
Die 145 gezielten Zugtests enthalten separat die bestehenden echten UDP-Faelle.

Live geprueft: 128 Kohle einlegen und zurueckholen, sieben Lagerbloecke einlegen
und zurueckholen. Die beiden Aufnahmen zeigen die gefuellten Behaelter. Beide
Transferwege erhalten die Gegenstandsmenge. Die Slots haben 33,6 logische Pixel,
neun feste Spalten und dieselbe Zoomfunktion wie das Spielerinventar.

Reproduktion mit isolierten Konfigurationen und Weltverzeichnis:

```sh
TERRA_TRAIN_INVENTORY_LIVE=1 \
TERRA_CONFIG_DIR="$PWD/target/live217/configs" \
TERRA_MENU_CONFIG_DIR="$PWD/target/live217/configs" \
TERRA_WORLD_DIR="$PWD/target/live217/worlds" \
target/debug/terra-client
```

Die Fixture beendet den Client nach vier bestaetigten Klicks erfolgreich.
Aufnahmen liegen unter target/live217. Der Benutzer-Spielstand wird nicht geladen.
