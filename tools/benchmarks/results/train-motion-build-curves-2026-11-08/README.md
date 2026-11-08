# Zugbewegung, Baupruefungen und Kurven

Die Darstellung verwendet geglaettete bestaetigte Bewegungen fuer Wagen,
Laufwerk und Sitze. Neue Pakete beginnen am aktuellen Darstellungsstand;
Teleports, geaenderte Drehpunkte und Empfangspausen setzen ihn zurueck.
Winkel folgen dem kuerzesten Weg. Der lokale Sitz umgeht zusaetzliche
Physikinterpolation, Geh-Wackeln und Treppenabfederung der Kamera. Sichtbare
Mitfahrer verwenden ebenfalls die Sitzposition des dargestellten Fahrzeugs.
Die bestaetigten Fahrzeugdaten und serverseitigen Transaktionen bleiben die
Autoritaet fuer Kollisionen und Berechtigungen.

Die lokale Geometriehuelle bleibt ueber Bewegungs- und Inventarupdates hinweg
wiederverwendbar. Der begrenzte Cache besitzt Geometrieschluessel; geaenderte
Bauteile erhalten sofort eine neue Huelle. Acht transformierte Ecken liefern
konservative Weltgrenzen. Die anschliessenden exakten Formpruefungen bleiben
erhalten. Kollisions-Snapshotvorbereitung besitzt zwischen Abschnitten ein
Budget von zwei Millisekunden; ein erster Abschnitt darf stets fortschreiten.

Ein direkter CPU-Vergleich mit demselben Aufbau misst 1.000 Anfragen:
237 us fuer die wiederverwendete Huelle gegen 22.414 us fuer den bisherigen
vollstaendigen Formdurchlauf. Das misst diese Operation, keine allgemeine
FPS-Steigerung. Der Kameratest kontrolliert 60 bewegte Frames einschliesslich
Hoehenwechseln gegen die erwartete Sitzposition.

Neue gezeichnete Kurven benoetigen 16 m horizontalen Radius. Gerade Rampen und
Validierung gespeicherter Kurven behalten ihre bisherigen Regeln. Zu enge
Vorschlaege werden vor der aufwendigen Schnittpruefung verworfen. Bestehende
Sicherheits-/Bautests verwenden ausreichend grosse Strecken; kompakte
Kontextkurven pruefen jetzt Ablehnung und erhaltene gerade Alternativen.
Protokoll 120 verlangt passende Client-/Server-Bauregeln. Zugindex ID15 bleibt.
Die nativen Schienenmodelle unterscheiden sich nur in ihrer Quellprovenienz.

Alle 4.235 Workspace-Library-Tests bestehen, neun bestehende Tests sind
ignoriert. Enthalten sind echte UDP-Transaktionen, Holz-/Kostenpruefung,
Grundstuecksschutz, Kollisionskontakte, Kurven, Steigungen, Kamera und
persistente Weltdaten. Client-/Server-Build mit Linking und Clippy aller
Targets mit -D warnings bestehen. Der Fahrtest ist eine ECS-/Mathe-/UDP-
Abnahme; eine manuelle Fahrt in einer Bestandswelt wurde nicht durchgefuehrt.


Der native Vulkan-Bautest verwendet eine isolierte Creative-Welt mit einem
lokalen autoritativen Server und Sichtweite sechs. Die Aufnahme besitzt
1.920 x 1.200 Pixel. Echter Abbau, Inventarauswahl, Platzieren und die aktuelle
sichtbare Terrain-Geometrie werden geprueft. Der Test erwartet keinen entfernten
Schliessen-Knopf mehr und initialisiert die sonst vom Menue gesetzte
Grafikkonfiguration. Vollstaendiges Preview-Warmup wird explizit aktiviert.

114 Frames in der eigentlichen Platzierungsphase: Median 6,86 ms, P95 8,29 ms,
Maximum 12,60 ms. Anfrage bis bestaetigtem sichtbarem Mesh: 66,61 ms. Diese
Latenz ist asynchron und entspricht keinem Frame-Stillstand. Der Lauf enthaelt
keinen dekorierten fahrenden Zug und ersetzt keine Fahrt in der Problemwelt.

Befehl (Konfiguration und Welt liegen ausschliesslich unter target/live218):

```bash
env -u WAYLAND_DISPLAY WINIT_UNIX_BACKEND=x11 \
  TERRA_PREVIEW_WARMUP=1 TERRA_BUILD_CREATIVE=1 TERRA_PLACEMENT_TIMING=1 \
  TERRA_CONFIG_DIR="$PWD/target/live218/configs" \
  TERRA_MENU_CONFIG_DIR="$PWD/target/live218/configs" \
  TERRA_WORLD_DIR="$PWD/target/live218/worlds" \
  TERRA_CAPTURE_DIR="$PWD/target/live218/screenshots" \
  target/debug/terra-client --smoke-test-building
```

[Live-Protokoll](native-building.log), [Platzierung](building-restored.png),
[Tests](tests.log), [Build](build.log), [Clippy](clippy.log),
[Kostenvergleich](bounds-cost.log).
