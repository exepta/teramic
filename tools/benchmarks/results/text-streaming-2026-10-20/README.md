# Phase 84: Sichere Text-Edits Und Streaming-Spitzen

## Reproduzierter Absturz

TiltUI 0.1.0 ersetzt bei gebundenen Eingabefeldern den Parley-Puffer mit
`set_text` und haengt `TextEnd(false)` an die bestehende Edit-Warteschlange.
Parley 0.9 invalidiert dabei das Layout, behaelt aber vorerst die alte
Byteindex-Auswahl. Eine bereits wartende Einfuegung kann diese vor der
Cursorbewegung verwenden. `abc` mit Cursor 3, ersetzt durch ein vier Byte
langes Emoji, reproduziert exakt die gemeldete Assertion in
`editor.rs:1171`. Auch kuerzere Ersetzungen und markierte Bereiche sind
grundsaetzlich betroffen.

`terra-ui::text_editing` synchronisiert schmutzige Editor-Layouts vor
Bevys `apply_text_edits`, nur bei geaenderten Eingabefeldern mit wartenden
Edits oder Paste. Dadurch aktualisiert Parley selbst die Auswahl an
gueltige Zeichengrenzen. Eingaben werden nicht verworfen, Unicode wird
nicht herausgefiltert, IME/Paste bleiben im vorhandenen System.
Keine Aenderung im Cargo-Registry-Cache und kein Dependency-Upgrade.
Dies bleibt eine lokale Kompatibilitaetskorrektur fuer die vorhandenen
Versionen, kein zweiter Editor.

Tests reproduzieren die urspruengliche Panic kontrolliert und pruefen
Emoji, Umlaut, kombinierende Zeichen, leeren/kuerzeren Text sowie den
tatsaechlichen geplanten Bevy-Editablauf vor dem TiltUI-Cursorreset.
API-Grundlage: [Bevy EditableText](https://docs.rs/bevy_text/0.19.1/bevy_text/struct.EditableText.html)
und die lokal installierten TiltUI-/Parley-Quellen.

## Streaming

Der erste 60-s-Flug hatte 129.00 FPS, p99 13.342 ms und einzelne Frames
bis 30.905 ms. Mehrere Spitzen folgten groesseren Uploadpaketen; das ist
eine zeitliche Korrelation, keine bewiesene GPU-Stall-Zuordnung.

- Dach-, Naesse- und Ufer-Snapshots bleiben fuer rein visuelle LOD-Wechsel
  erhalten. Streaming-Ankunft/Entfernung, Journal-Aenderungen, Konfiguration
  oder Terrainwechsel invalidieren den Cache konservativ. Keine zweite
  Expositionsberechnung oder neue Terrainlogik.
- Die priorisierte Terrain-Auftragsliste bleibt zwischen unveraenderten
  Frames bestehen. Frei werdende Worker-Slots entnehmen weitere Kandidaten,
  statt alle geladenen Sections erneut zu durchsuchen. Fokus-Bucket,
  Welt-/Journal-/Asset-Aenderungen und abgeschlossene Einblendungen bauen
  die Auswahl neu auf. Veraltete Worker-Ergebnisse werden explizit erneut
  eingereiht, auch wenn die Kamera nach ihrer Bewegung stehen bleibt.
- Uploadquoten je Frame: Terrain 64k, Props 48k, Wasser 16k Vertices
  statt 128k/96k/32k. Zusaetzlich maximal 8/4/4 Section-Batches, damit
  viele kleine oder leere Pakete keinen unbegrenzten Uebergabeframe erzeugen.
- Ein uebergrosser erster Batch pro Bereich darf weiterhin fortschreiten.
  Quoten sind deshalb keine harte Obergrenze fuer einzelne atomare Sections.
- Kein Entfernen von Vegetation, keine kuerzere Sichtweite oder neue
  Generatorrevision. Die staerkere Verteilung kann die Ladezeit verlaengern;
  diese wird im Vergleich mitgemessen.

## Messbedingungen

`--smoke-test-lod`, je 60 s auf demselben Flugpfad, Seed 42042,
Hoehlenrevision 4, Vulkan, native 1920x1080, VSync aus,
`TERRA_CPU_SCOPES=1`. Keine parallele Kompilierung waehrend der Messungen.
Sichtweite 8 verwendet die Phase-83-Testkonfiguration, Sichtweite 16
`configs16/` in diesem Ordner. Benutzerkonfigurationen bleiben unangetastet.
Rohdaten liegen unter `logs/`.

GPU: Radeon RX 6950 XT / RADV. Der vorherige Phase-83-Client wurde vor
den Aenderungen als `target/phase84/terra-client-before` gesichert.
Die regulaere Desktop-Sitzung und der bereits laufende Benutzer-Server
blieben bestehen. CPU-Wall-Framezeiten, keine GPU-Zeitmessung.
RSS ist der Prozesswert am Messende, nicht Peak-RSS oder VRAM.

## Messergebnisse

| Sichtweite / Lauf | FPS | p95 ms | p99 ms | Max. Frame ms | Erstladen s | RSS KiB |
|---|---:|---:|---:|---:|---:|---:|
| 8 vorher | 129.00 | 11.176 | 13.342 | 30.905 | 2.777 | 940380 |
| 8 final | 130.79 | 10.138 | 12.062 | 28.414 | 3.271 | 936912 |
| 16 vorher | 69.34 | 18.457 | 21.140 | 33.720 | 8.323 | 1446348 |
| 16 final | 72.65 | 17.418 | 19.683 | 44.341 | 9.228 | 1466264 |
| 16 vorher, Wiederholung | 68.39 | 18.661 | 21.229 | 32.757 | 8.629 | 1476580 |
| 16 final, Wiederholung | 73.51 | 17.241 | 19.669 | 32.705 | 9.057 | 1462028 |

Dateien: `stream-before.log`, `stream-final.log`, `stream16-before.log`,
`stream16-final.log`, `stream16-before-repeat.log`,
`stream16-final-repeat.log`.
`stream-after.log` und `stream16-after.log` sind Zwischenstaende ohne
die persistente Kandidatenliste, nicht die finale Implementierung.

Bei Sichtweite 8 bleiben 2033 geladene / 925 gerenderte Sections,
4636 Mesh-Assets und 25560 repraesentierte Props erhalten.
Bei Sichtweite 16 bleiben 7785 / 4169 Sections und 6122 sichtbare
Mesh-Entities erhalten. Kleine Prop-/Asset-Anzahldifferenzen entstehen
im laufenden Test; keine Behauptung eines bitidentischen Framezustands.
Es wurde weder Sichtweite noch Detaildichte reduziert.

Die gesamte gemessene CPU-Zeit in `terrain.dispatch` sinkt im ersten
16er-Paar von 19851 auf 13393 ms; die maximale einzelne Ausfuehrung von
10.271 auf 7.505 ms. Der Scope umfasst auch den Start und ist mit
verschachtelten Scopes nicht additiv. Die zugelassenen Uploadspitzen
sinken von 238481 auf 179050 Vertices pro Frame, ueberschreiten aber
wegen atomarer Section-Meshes weiterhin die nominelle Summe der Quoten.

Die typischen langsamen Frames verbessern sich, nicht alle Ausreisser:
Der erste finale 16er-Lauf hat einen schlechteren maximalen Frame als
der alte Build. Erstladen dauert hier rund 0.9 s laenger.
Keine allgemeine FPS-/Speicherparitaet, keine Behebung aller Streaming-
Stalls und keine Uebertragbarkeit auf 4K/andere GPUs behaupten.
Das zweite 16er-Paar bestaetigt die p95-/p99-Verbesserung:
CPU-Mesh-Planung 19871 -> 13274 ms, Erstladen 8.629 -> 9.057 s.
Der maximale Frame bleibt hier mit 32.757 -> 32.705 ms praktisch gleich.
Die vier staerksten Frames liegen auf beiden Builds an aehnlichen
Flugabschnitten; deren verbliebene Ursache ist noch nicht isoliert.

## Validierung

Alle Befehle erfolgreich auf dem finalen Quellstand:

```text
cargo fmt --all --check
cargo check --workspace
cargo test --workspace --no-fail-fast
cargo clippy --workspace --all-targets -- -D warnings
cargo build --workspace
```

1297 Tests erfolgreich, kein fehlgeschlagener Test, ein vorhandener Test
ignoriert. Logs: `fmt-final.log`, `check-final.log`,
`workspace-tests-final.log`, `clippy-final.log`, `build-final.log`.
Neben den drei Texteditor-Regressionstests pruefen die Renderertests
Batchquoten fuer kleine/leere Uploads und das erneute Einreihen eines
veralteten LOD-Ergebnisses bei anschliessend stillstehender Kamera.
Client und Dedicated Server bauen ohne Dependency-Aenderungen.
Der finale Dedicated Server besteht ausserdem `--smoke-test --no-save
--bind 127.0.0.1:0` ohne `DISPLAY` und `WAYLAND_DISPLAY`, generiert 27
Sections und beendet sich erfolgreich (`headless-final.log`).

## Zusaetzliche Beobachtungen

Die 16er-Laeufe protokollieren sowohl mit dem alten als auch dem neuen Build
beim Start zweimal Bevys Slab-Allocator-Meldung ueber einen unallocated key.
Kein Absturz, aber die Ursache ist hier nicht behoben oder als harmlos
bewiesen. Nicht mit dem reproduzierten Parley-Texteditorfehler verwechseln.
Eine bekannte moegliche Ursache sind leere Meshes
([Bevy #24874](https://github.com/bevyengine/bevy/issues/24874)); ohne
Zuordnung zum konkreten Asset ist das keine Diagnose dieses Laufs.

Der native Chat-Smoke-Test mit integriertem Server prueft TiltUI-Eingabe,
Vorschlaege, Escape, Locate/Teleport, Wetter und Help-Pagination erfolgreich.
Er endet mit Exit 0. Beim sofortigen Testende erscheint noch die bereits
bekannte Screenshot-Rueckkanalmeldung nach gespeichertem Bild. Keine
Parley-Panic in diesem Lauf; Unicode-Ersatzfaelle werden separat durch
die neuen automatisierten Regressionstests abgedeckt.
Die fuenf nativen Screenshots liegen in `captures/`. Sie wurden in diesem
Durchlauf nicht manuell visuell abgenommen; die Aussagen zur UI beruhen
auf den automatisierten nativen Interaktionspruefungen.
