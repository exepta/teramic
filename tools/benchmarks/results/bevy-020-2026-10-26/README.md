# Bevy 0.20 Migration (Phase 133)

## Stand

Teramic verwendet Bevy 0.20.0, Avian3D 0.8.0 und Lightyear 0.31.0.
TiltUI und sein Build-Tool sind auf Commit
`44134e4ee59774da94203ed3ae303e85fbf3dabf` des Branches `preview-020`
gepinnt. `rust-toolchain.toml` waehlt Rust 1.97.1 samt Rustfmt/Clippy;
die globale Rustup-Standardeinstellung wurde nicht veraendert.

Das ist ein funktionsfaehiger Update-/Teststand, **kein abgeschlossener FPS-Fix**.
Insbesondere die beleuchtete Dorf-Szene ist im neuen Build langsamer.
Die Grafikqualitaet wurde nicht reduziert. A05 bleibt offen.

## Umsetzung

- Alle 16 Teramic-Shader auf WESL und die neuen Bevy-Modulpfade migriert.
  Material-, Prepass-, Alpha-/OIT-, Tiefen- und UI-Schnittstellen angepasst.
  Die erweiterten PBR-Funktionen fuer Fackellicht und Wasser-SSR sind gegen die
  tatsaechlichen 0.20-Quellen angepasst, nicht still abgeschaltet.
- Neue RenderApp-Extraktionsattribute, Picking-Ereignisse, Komponenten-Observer,
  AABB-/Projektions- und Storage-Buffer-Schnittstellen verwendet. Native
  Textbearbeitung folgt der neuen Bevy-/Parley-Integration; minimale
  Tooltip-Testwelten initialisieren die jetzt erforderliche PointerMap.
- `vendor/bevy_render` auf die originale 0.20.0-Quelle rebasiert. Die lokalen
  begrenzten Upload-Caches fuer CPU-eigene Bin-Listen und der gemeinsame
  Slab-Growth-Encoder bleiben erhalten. GPU-geschriebene Metadaten werden
  weiterhin aktualisiert. Der neue Uniform-Allocation-Pfad bleibt aktiv.
- Gruppierte Punktlichtschatten nutzen auch die neue Uniform-Allocation vor
  Unpacking/Preprocessing. Der Zwei-Frame-Bindingscache umfasst deren neue
  Bind-Groups und Pufferidentitaeten; keine Schattenseiten entfernt.
  In 0.20 baut `per_view_shadow_pass` selbst die indirekten Befehle unmittelbar
  vor jeder Schattenseite. Der alte zusaetzliche globale Build-Aufruf wurde
  entfernt: Im ersten leeren Hoehlenframe fehlte dort CurrentView und loeste
  den sicheren, aber unnoetigen Core3d-Fallback aus. Der explizite Per-Face-
  Referenzmodus baut dagegen weiterhin unmittelbar vor jeder einzelnen Zeichnung.
  Nur bei explizitem `TERRA_GPU_DRAW_DIAGNOSTICS=1` werden Schattenseiten fuer
  die Readbacks einzeln gezeichnet/kopiert: 0.20 setzt die gemeinsamen
  indirekten Zaehler vor der naechsten Seite zurueck. Ohne diese Trennung wuerden
  Kopien am Gruppenende Zaehler verschiedener Zeichnungen verwechseln.
  Normale FPS-Messungen verwenden diesen langsameren Diagnosemodus nicht.
- Den lokalen TiltUI-Runtime-Patch mit Snapshot-Borrowing, geparkten UI-Zweigen,
  Reaktivitaetsreihenfolge und direkten Tooltip-Pointerpositionen erhalten.
  Alle TiltUI-Begleitcrates beziehen dieselbe Git-Revision; keine doppelten
  Bevy-/Avian-/Lightyear-/TiltUI-Versionsbaeume im aufgeloesten Workspace.
- Seed, Generatorversion, Block-IDs, Weltdateien und Protokollmodell unveraendert.
  Client und Server muessen zusammen mit diesem Lockfile aktualisiert werden.

## Native Messungen

Ryzen 9 5900X / RX 6950 XT, Vulkan/RADV, Mesa 26.2.5, 1280x800,
Sichtweite 12, VSync aus, bestehendes optimiertes Dev-Profil. Keine parallelen
Builds oder Unit-Tests in den FPS-Messfenstern. Nur kurze Tests.
Vorher: alter nativer Build zu `d2b7956`; dessen Shader stammen aus demselben
Git-Stand. Nachher: migrierter 0.20-Build. Keine Auswahl nur guter Frames.

| Szene | Version | FPS Mittel | p95 ms | p99 ms | Max. Frame ms | Laden s | RAM KiB |
|---|---|---:|---:|---:|---:|---:|---:|
| Regen-/Wasserflug, Seed 7, 24 s | 0.19.1 | 97.50 | 15.80 | 18.66 | 28.82 | 8.60 | 1312804 |
| Gleicher Flug | 0.20.0 | 101.83 | 15.06 | 17.33 | 31.81 | 8.47 | 1373996 |
| Dorf nachts, Seed 42042, 8 s | 0.19.1 | 74.92 | 16.79 | 18.62 | 23.79 | 22.41 | 1936992 |
| Dorf, erster Migrationsstand | 0.20.0 | 57.40 | 22.05 | 23.36 | 25.98 | 23.99 | 2106384 |
| Dorf, Diagnose vor Schattenkorrektur | 0.20.0 | 56.72 | 22.03 | 23.16 | 29.42 | 24.14 | 1998624 |
| Gleiches Dorf, finaler Build | 0.20.0 | 59.74 | 20.93 | 22.32 | 23.66 | 23.77 | 2073328 |

Im Flug identisch: 6574 geladene / 1336 gerenderte Sections, 7544 Mesh-Assets,
10151 dargestellte Props und 625 Wasser-Tiles. Im Dorf: 5625 / 2878 Sections,
80732 dargestellte Props und acht schattenwerfende Punktlichter. Die integrierte
Physik verursacht kleine Unterschiede der endgueltigen Kamerahoehe; dies ist
kein pixelidentischer GPU-Mikrobenchmark. Es gibt keinen allgemeinen FPS-Gewinn:
der schlechteste Flugframe ist schlechter und das Dorf deutlich langsamer.
Der Flug wurde vor der abschliessenden Schattenkorrektur gemessen; dort waren
keine Punktlichtschatten aktiv. Der finale Dorf-Lauf ist gesondert ausgewiesen;
die ersten beiden 0.20-Dorfwerte werden nicht als finaler Stand ausgegeben.

Die Diagnose-Wiederholung zeigt wechselnde Unpacking-/Allocation-Metadaten-
Offsets bei stabilen Pufferidentitaeten. Die Vorbereitung der Bind-Groups kostet
dort im Messfenster im Mittel 3.04 ms. Das ist ein konkreter Ansatzpunkt fuer
weitere Profilierung, **kein Nachweis der vollstaendigen Regressionsursache**.
Einzelne Streaming-Spitzen, Speicherparitaet und Langzeitstabilitaet sind nicht
abgenommen. Keine neuen 10-/15-/30-Minuten-Laeufe durchgefuehrt.

Rohdaten und ausgewaehlte Screenshots liegen lokal in diesem Ordner. Wie andere
Benchmark-Artefakte werden `.log`/`.png` durch die bestehenden Gitignore-Regeln
nicht automatisch versioniert. Die Messwerte oben bleiben im Bericht erhalten.

## Reproduktion

Flug: `cargo run --locked -p terra-client -- --smoke-test-lod` mit
`TERRA_CONFIG_DIR=tools/benchmarks/results/tree-structures-performance-2026-10-25/configs-seed7`,
`TERRA_BENCH_GRAPHICS=1`, `TERRA_BENCH_WEATHER=rain`, `TERRA_BENCH_SECONDS=24`,
`TERRA_BENCH_LOAD_SECONDS=240`, `TERRA_CPU_SCOPES=1`, `WGPU_BACKEND=vulkan`.

Dorf: `--smoke-test-benchmark` mit
`TERRA_CONFIG_DIR=tools/benchmarks/results/cherry-ai-light-2026-10-25/configs-night-lights`,
`TERRA_BENCH_MODE=integrated`, `TERRA_BENCH_UI=1`,
`TERRA_BENCH_CAMERA=-574,39,962`, `TERRA_BENCH_LOOK_AT=-574,33,970`,
`TERRA_BENCH_WEATHER=clear`, `TERRA_BENCH_SECONDS=8`,
`TERRA_BENCH_LOAD_SECONDS=240`, `TERRA_CPU_SCOPES=1`, `WGPU_BACKEND=vulkan`.
`TERRA_BINDING_DIFF=1` protokolliert Cache-Invalidierungen im Diagnose-Lauf.

Fuer isolierte Tests jeweils ein eigenes `TERRA_MENU_CONFIG_DIR` und
`TERRA_WORLD_DIR` ausserhalb bestehender Spielstaende setzen. Die vorhandenen
Versionen nicht gleichzeitig gegen dieselbe Welt starten.

## Validierung

- `cargo fmt --all --check`, `cargo check --workspace --all-targets --locked`,
  `cargo test --workspace --locked`,
  `cargo clippy --workspace --all-targets --locked -- -D warnings` und
  `cargo build --workspace --locked`: erfolgreich. Builds/Tests mit begrenzter
  Jobzahl; geaenderte lokale Renderer-/Tooltip-Dateien separat mit Rustfmt geprueft.
- Workspace-Tests: 2527 bestanden, 7 ignoriert, einschliesslich 625 Renderer-
  und 351 UI-Tests. Neuer Regressionstest parst alle 16 WESL-Module und weist
  veraltete Praeprozessorsyntax zurueck.
  Der zusaetzliche Schatten-Regressionsfall prueft leere Compute-Paesse ohne
  CurrentView, bevor der Schatten-Zeichenschritt seine eigene Ansicht setzt.
- Lokale TiltUI-Runtime: 322 Tests bestanden, einschliesslich Unicode-Editor,
  Tooltips, Snapshot-Ownership und UI-Reaktivitaet.
- Vendored Bevy-Renderer: 61 Tests bestanden, einschliesslich exakter begrenzter
  Upload-Caches, Mesh-Reextraktion, Freigabe und gemeinsamem Slab-Wachstum.
  Die Allocation-Tests verwenden Bevy-Noop-Geraete, keinen nativen GPU-Readback.
- Separater nativer GPU-Readback mit korrigierten Per-Face-Kopiergrenzen:
  letzter Dorf-Snapshot 454 Schatten-API-Aufrufe / 7897 nichtleere
  GPU-Schatten-Unterzeichnungen, `unread_submissions=0`, 283028 kopierte Bytes.
  Diese unterschiedlichen Zaehler werden nicht als dasselbe ausgegeben.
  Der langsamere Diagnose-Lauf ist kein FPS-Vergleichswert der Tabelle.
- Native Clienttests: Regenflug, integriertes beleuchtetes Dorf mit HUD/Minimap,
  Physik (Fallen, Boden, Sprung, Wand, abgebauter Untergrund) und Inventar
  (11 Captures, echte Maus, Ziehen/Teilen/Tooltips/Muelltonne) erfolgreich.
- Geschlossene Hoehlenfixture: vier Ansichten 14 bis 17 mit laufender Weltuhr,
  Fackeln und Unterwasserlicht erfolgreich; nach der Korrektur kein
  Schatten-Fallback. Alle vier Aufnahmen gespeichert, Exit 0. Beim App-Ende
  meldet ein noch offener Screenshot-Kanal einen Sendefehler; diese bereits
  bekannte Fixture-Abschlussmeldung bleibt offen und ist kein Shaderfehler.
- Dedicated Server ohne DISPLAY/WAYLAND_DISPLAY: Start, UDP-Endpoint,
  Generierung von 27 Sections und sauberer Exit erfolgreich.
- Flug-/Dorf-Screenshots visuell betrachtet: Terrain, Props, Wasser-/Lichtpfade
  und UI vorhanden. Kein behaupteter vollstaendiger Referenzbild-Abgleich.
  In den erfolgreichen nativen Laeufen keine Shaderfehler oder Slab-Use-after-free.

Nicht automatisch aktiviert: neue optionale Raytracing-/Solari-Verfahren oder
ein anderer Qualitaetsmodus. Der Test verwendet den migrierten bestehenden
PBR-/GPU-Preprocessing-Pfad.
