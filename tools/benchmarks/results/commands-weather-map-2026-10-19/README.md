# Phase 73: Commands, Wetter Und Karten

Abnahme am 19.10.2026 unter Linux/Wayland, Vulkan, AMD Radeon RX 6950 XT,
RADV/Mesa 26.2.4. Native Aufnahmen stammen aus dem laufenden Client, nicht aus
Mockups. Referenzlayout mit den bestehenden TERAMIC-/TiltUI-Komponenten umgesetzt.

## Implementierung

- Gemeinsamer Command-Katalog fuer Chat und Konsole; `/weather set clear|rain|snow|storm`
  setzt autoritatives Wetter. Acht Sekunden Uebergang, 240 Weltsekunden Vorgabe,
  anschliessend automatisches Klima. Bei pausierter Weltzeit wird ein ausdruecklicher
  Wetterbefehl sofort wirksam, ohne automatische Oberflaechenzeit zu erfinden.
- `/help [page]` zeigt fuenf Befehle je voller Seite, farblich getrennte Syntax und
  Beschreibung sowie klickbare TiltUI-Seitenpfeile. Aktuell sieben Befehle, zwei Seiten.
- Dedicated-Konsole ohne Slash: Tab-Vorschlaege, Spielernamen, Verlauf und begrenzte
  Eingabepuffer. Globale Befehle funktionieren ohne Spieler; `as Alice tp 10 80 20`,
  `tp Alice 10 80 20` und `gamemode creative Alice` waehlen den Spieler explizit.
  Unix-PTY, Windows-Console.ReadKey/PowerShell und einfacher stdin-Fallback sind getrennt.
  Eine volle Command-Queue behaelt Backpressure, aber keinen Editor-Mutex.
- Dichtere GPU-Regenlayer, Aufschlagringe, mehrschichtige helle Wolken und dunklere
  vorlaufende Wetterfronten. Integrierte Windadvektion vermeidet Texturspruenge.
  Autoritative Windrichtung/-staerke wirken auf Laub, Gras, Buesche, Pilzfasern und Blaetter.
- Dachgeschuetzte, gespeicherte Oberflaechennaesse steuert einen lokalen PBR-Film
  ueber vorhandene Terrainbatches und geteilte WetMaterial-Handles. Kein globales
  Glaenzen trockener Hoehlen. Unveraendertes Oberflaechenbudget: eine geladene
  Spalte je zwei Sekunden, fair zwischen Spielernaehe und Hintergrund verteilt.
- MiniMap-Relief bleibt beim Nachladen und Detailcache-Eviction erhalten.
  TRMAP002 speichert 256 Hoehen und Wasserbits je Kachel; TRMAP001 bleibt lesbar.
  Unvollstaendige neue Kacheln loeschen vorhandene Hoehen nicht.
- Rechteckige FullMap mit Werkzeugleiste, Titel, Kompass und Metadatenleiste,
  Raster, Mob-/Layerumschaltung und optionaler echter 3D-Hoehenkarte.
  Asynchrones, budgetiertes Hoehenfeld, ein Mesh/Material und isolierte orthografische
  Kamera; Kamera nur bei geoeffneter 3D-Karte aktiv. Pan, Zoom, Marker und Picking
  verwenden dieselbe Projektion. Kein zweites Weltgenerierungs- oder Prop-System.
- Eigenstaendiger Wegpunktdialog auf `N`, MiniMap standardmaessig `J`, FullMap `M`.
  Neue Wegpunkte uebernehmen die Spielerkoordinaten und haben Weltanzeige an.
  Suchbarer echter TiltUI-Iconkatalog, groesseres Welticon ueber `Name [100m]`.
  Bestehende gespeicherte Schalter und abweichende eigene Bindings bleiben erhalten.

Netzwerkprotokoll **41** erfordert gemeinsam aktualisierte Clients/Server.
IDX5 speichert den Wettermodus und liest aeltere Weltindizes; REG9/PLY3,
Seed, Generatorversion und Block-/Welt-IDs bleiben unveraendert.
Keine Dependency-Versionen geaendert. Bevy-PBR wurde nur fuer die Client-Kartenkamera
aktiviert; der standalone Dedicated-Dependencybaum bleibt ohne Renderer/UI/Audio.

## Tests

Abschliessende Workspace-Laeufe stehen unter `logs/`:

```bash
cargo fmt --all --check
cargo check --locked --workspace
cargo test --locked --workspace
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo build --locked --workspace
```

Ergebnis: **1126 bestanden, 0 fehlgeschlagen, 1 vorhandener UDP-Langzeittest
ignoriert**. Formatpruefung, Check, Clippy mit `-D warnings` und Workspace-Build
sind erfolgreich. Der abschliessende Test-/Buildlauf nutzt `CARGO_BUILD_JOBS=1`:
ein vorheriger paralleler Linklauf wurde wegen starkem RAM-/Swapdruck abgebrochen,
nicht als bestandener Test ausgegeben. Der serielle Wiederholungslauf ist vollstaendig.
Siehe [Tests](logs/terra73-tests-final-single-job.log),
[Clippy](logs/terra73-clippy-final.log), [Check](logs/terra73-check-final.log),
[Format](logs/terra73-fmt-final.log) und [Build](logs/terra73-build-accepted.log).

Die Regressionen pruefen unter anderem Wetterrechte und echte Zwei-Client-UDP-
Replikation, explizites Wetter bei pausierter Zeit, unveraenderte Oberflaechen
bei pausierter/stehender Wetterzeit, Naessematerial-/Mip-Updates, Cache-Hoehen,
negative Koordinaten, TRMAP001/002, 3D-Projektion/Indizes/Picking, schnelle
Moduswechsel, Icon-/Wegpunktpersistenz, Quick-Waypoint-Eingaben und volle Konsolenqueues.

Native Funktionspruefungen:

- 52-stufiger Kartenlauf mit OS-Eingaben, echtem Peer-/Mobmarker, allen MiniMap-FPS-
  Modi, Flug/Hoehen-/Yawwechsel, Pan/Zoom, Cave-Layer, 3D-Pixelpruefung, N-Dialog,
  Iconsuche/-auswahl, Save/Reload/Delete und UI-Bounds: bestanden.
- Chat mit echter TiltUI-Tastatureingabe, `/locate`, serverbestaetigtem Teleport,
  Wetterbefehl und beiden klickbaren Hilfeseiten: bestanden.
- 16 Wetterphasen mit Regen/Schnee, Dachregeln, Feuerloeschen, Low/Off, Nacht,
  klarem Himmel, Wetterfront und trockener/feuchter/nasser Oberflaeche: bestanden.
  WetMaterial-Batches vorhanden; WGSL-Pipelines nativ kompiliert, keine Naga-Fehler.
- Linux-PTY mit Tab, Pfeiltasten, History und wiederhergestelltem `stty`-Zustand:
  bestanden. Headless/Pipe-Befehle und Serverstart ohne Display: bestanden.

Zwischenlaeufe deckten reale Fehler auf: nicht unterstuetzte HTML/CSS-Tags,
ausserhalb liegende Dialogschliesser und erfundene Oberflaechenzeit beim ersten
Besuch trotz Pause. Diese sind korrigiert; spaetere native Laeufe bestanden.
Der Wayland-Test kann bei synthetischem Cursor-Warp die bekannte Meldung ausgeben,
dass Warping nur bei gesperrtem Cursor erlaubt ist; die Karteninteraktion wird
zusaetzlich ueber echte Eingaben und Geometrie-/Pixelassertionen validiert.

## Kurze Framefenster

`--smoke-test-weather`, Seed 42042, Sichtweite 8, VSync aus, kontinuierliches
Rendering, native 1920x1080-Captures. Der vorhandene nutzereigene Server wurde
nicht beendet; dies ist keine isolierte Leistungsbaseline.
Je Phase etwa 1.5 Sekunden nach 0.5 Sekunden Wartezeit, 238-277 gemessene Frames.
Die Szenen und Kamerawinkel unterscheiden sich zwischen Phasen.

| Phase | FPS | p95 ms | p99 ms | Wetterquads / Batches |
| --- | ---: | ---: | ---: | ---: |
| Klar, High | 158.15 | 7.086 | 7.289 | 3 / 1 |
| Regen, High | 184.24 | 6.187 | 6.679 | 2413 / 1 |
| Schnee, High | 176.66 | 6.560 | 7.344 | 581 / 1 |
| Regen, Low | 178.15 | 6.573 | 7.204 | 271 / 1 |
| Effekte Off | 175.83 | 7.018 | 7.887 | 0 / 0 |
| Regen, Nacht | 171.82 | 6.778 | 7.209 | 2413 / 1 |
| Schoenwetterwolken | 168.78 | 6.768 | 8.535 | 3 / 1 |
| Wetterfront | 173.64 | 6.549 | 7.217 | 3 / 1 |
| Oberflaeche trocken | 177.15 | 6.425 | 7.052 | 3 / 1 |
| Oberflaeche feucht | 168.50 | 7.320 | 8.336 | 3 / 1 |
| Oberflaeche nass | 159.40 | 8.713 | 10.112 | 3 / 1 |

14 Wettergeometrie-Uploads insgesamt durch Qualitaets-/Szenenwechsel; keine
Geometrie-Uploads pro Animationsframe. Batches sind nicht die Drawzahl des ganzen
Frames. Die Naessephasen zeigen hoehere p99-Werte; daraus weder Verbesserung
noch allgemeine Paritaet ableiten. Keine Vorher-/Nachher-Baseline, GPU-Timer,
VRAM-/RSS-Messung oder lange Streaming-/Mehrspielerabnahme in diesem Paket.

## Native Ansichten

- [Chat-Hilfe mit Pagination](captures/chat-help-page1.png)
- [FullMap mit Tiefengeometrie](captures/map-37.png)
- [Wegpunktdialog](captures/map-20-editor.png)
- [TiltUI-Iconkatalog](captures/map-42.png)
- [Iconsuche](captures/map-43.png)
- [Schoenwetterwolken](captures/clouds-clear.png)
- [Herannahende Front](captures/clouds-approaching.png)
- [Regen und Aufschlagringe](captures/rain.png)
- [Nasse Terrainoberflaeche](captures/wet-saturated.png)

## Grenzen

- Windows/macOS-Terminalpfade nicht nativ getestet; Linux-PTY ersetzt das nicht.
- Storm ist Starkregen/Wind, ohne Blitze/Donner. Wolken sind geschichtete
  Shaderflaechen, nicht volumetrisch. Ihre Optik bleibt eine Approximation.
- PBR-Naesse haengt von Licht und Blickwinkel ab. Die Testansicht zeigt eine
  dezente Materialaenderung, keinen grossflaechig gespiegelten Regenboden.
  Nicht besuchte Bereiche holen Wetterakkumulation nicht nach; viele getrennte
  Spieler erhoehen bei festem Budget die lokale Aktualisierungslatenz.
- 3D zeigt erkundete Topografie, keine kompletten Voxelueberhaenge, kleinen Props
  oder frei drehbare Welt-Dioramen. Alte reine Farbkacheln benoetigen einen
  erneuten Besuch fuer echte Hoehen; grosse Grobuebersichten bleiben farbbasiert.
- Wegpunkte bleiben lokal je Welt/Dimension/Spieler. Keine servergeteilten Marker.
- Langzeit-GPU-/Speicherprofiling und gemeinsame Art-Abnahme bleiben in TODO.md.
