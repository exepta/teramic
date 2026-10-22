# Phase 32: Regionen, Fallwasser und Picking

## Speicherlayout

```text
worlds/<Name>/                  # Singleplayer, sicherer eindeutiger Ordnername
  world.toml                  # Titel, Version, Modus, Seed/Generator-Einstellungen
  level.data                  # Kleiner binaerer Index, Drop-Zaehler/Revision
  regions/r.<x>.<z>.region     # 32x32 Chunk-Spalten, alle Y-Sections
  players/<UUID>.data          # Inventar, Cursor, Revision, echte Spielstatistiken
```

Dedicated verwendet denselben Inhalt direkt unter `worlds/`, ohne `<Name>`.
`--save-dir` bleibt verfuegbar. Weltformat 2 sperrt alte Leser aus; interne
Binaerheader-Version 1, Generatorversion 5 und Netzwerkprotokoll 14 sind getrennt.
IDs, Seeds und das vorhandene Sparse-EditJournal bleiben erhalten. Die Dateiformate
sind TERAMIC/Postcard, nicht Minecraft-/NBT-/MCA-kompatibel.

Regionen speichern die bestehenden SectionDeltas inklusive platzierter/abgebauter
Bloecke, Prop-IDs/Zustaende, Wasserquellen/Fuellstaende, Oberflaechenzustaende und
losen Ressourcen. Negative Koordinaten nutzen euklidische Division. Spielerdateien
enthalten UUID-gebundene Slots und Cursor sowie serverseitige Zaehler fuer gebrochene
und platzierte Bloecke und geerntete Props. Gesundheit/Hunger/Ausruestungsstatistiken
werden nicht erfunden: entsprechende Gameplay-Systeme fehlen noch.

Unveraenderte Regionen und Spielerdateien werden nicht erneut geschrieben.
Ein vorbereiteter, fsync-gesicherter Commitplan wird zuletzt freigegeben; die
Installation endet beim kleinen Index. Nach einem Absturz werden verbleibende
Dateien fertig installiert. Ohne Commitmarker werden nur unfertige Schreibdaten
verworfen. Typ, Version, Laenge, Pruefsumme, Welt-/Spieler-/Regionszuordnung und
Duplikate werden geprueft. Fehlender Index bei vorhandenen Daten ist ein Fehler,
keine leere Ersatzwelt. Die FNV-Pruefsumme ist keine kryptografische Signatur.
Session- und Schreibsperren verhindern konkurrierende normale Writer.

Alte `edits.toml` werden validiert und in binaere Dateien uebernommen. Die Originale
bleiben als `edits.toml.legacy-backup`, alte Metadaten entsprechend als
`world.toml.legacy-backup`. Metadaten werden vor dem Transfer gegen alte Leser
gesichert, damit ein aelterer Client fehlende TOML-Daten nicht als leere Welt
interpretiert. Hex-ID-Verzeichnisse werden beim Oeffnen namensbasiert verschoben.
Der alte Dedicated-Standard `saves/dedicated` wird bei leerem neuem Ziel validiert
uebernommen, bleibt selbst als Sicherung erhalten. Abweichende Konfigurationen
werden nicht stillschweigend akzeptiert.

Dedicated und Integrated speichern ueber denselben Backendpfad. Der Integrated-
Host schreibt nun ebenfalls im Taskpool, mit maximal einem Savejob und echter
Antwort auf manuelle Save-Anfragen. Shutdown wartet und speichert den neuesten
Journalstand. Backups nur bei gestopptem Host erstellen.

## Fallwasser

Die vorhandene Shader-/Materialpipeline bleibt gemeinsam. Zuvor markierten die
dynamischen Zellflaechen jeden Meter zugleich als Lippe und Aufprall. Das ergab
eine nahezu durchgehend weisse, senkrecht gestreifte Platte. Effektcode 5 trennt
Zellgardinen vom festen Wasserblock 4 und von natuerlichen Faellen 1. Lokale
Randabstaende interpolieren Schaum; der Koerper bleibt gefaerbt und durchsichtig.
Mehrskalige bewegte Normalen verwenden dieselben Wellenparameter wie horizontales
Wasser. Bestehende Brechung, Absorption, Himmel-/Lichtreflexe und Qualitaetsoptionen
bleiben erhalten. Keine neue Textur-/Materialpipeline oder zusaetzlicher Renderpass.

Die 30-Block-Testgardine mit 15 Quellen wurde nativ auf Vulkan dargestellt:
vollstaendig verbunden, blau/transluzent statt durchgehend weiss, Torfluss und
Rueckgang nach Wiederverschluss unveraendert. Drei Aufnahmen und Exit 0.
Die Steinwanne ist ein kontrollierter Testaufbau, kein natuerlicher Fluss.
`fall-before.png` aus Phase 31 hat denselben Aufbau/Blick; `fall-after.png` zeigt
den neuen Shader. Keine vollstaendige Referenzbild-Abnahme oder FPS-Paritaet.

## Treffergeometrie

Vorhandene Botanikfunktionen wurden CPU-seitig nach terra-world verschoben,
nicht nachgebaut. Rendering und Interaktion verwenden dieselben Vorlagen fuer
Gras, Blumen, Buesche, Farne, Pilze und kleine Pflanzen/Steine. Unveraenderte
Rendervertexbudgets werden weiterhin fuer alle Varianten und LODs getestet.
Pick-Geometrie ist einmal pro Typ/Variante gecacht. Section-/Boxpruefungen sind
nur Broad Phase; der eigentliche Treffer verwendet Dreiecke, Rotation und
`visual_scale()`. Luft unter Pilzkappen oder zwischen Pflanzen bleibt Luft.

Die Client-Pruefung und die schmalen Auswahlkanten verformen dieselben Vertices
mit der aktuellen gewrappten Shaderzeit. Der Server verwendet nur die begrenzte
Windhuelle jedes einzelnen Dreiecks, nicht eine Pflanzenbox oder eine zweite
abweichende Animation. Verschiedene Uhren benoetigen kein neues Netzwerkprotokoll.
Normale Terrainbloecke bleiben logische volle Meterzellen fuer Bau und Kollision;
kosmetische Bevels/Reliefs werden nicht zu abbaubaren Mikro-Kollidern umgebaut.
Frei geformte importierte Spezialmodelle erhalten hier keinen neuen Collider-
Importer. Es entstehen keine zusaetzlichen Mikro-Entities oder Prop-Drawcalls.

Native Botanikpruefung: Gras, Blumen, Nahblueten und Busch wurden mit echten
ausgewaehlten Prop-IDs dargestellt; feste Wind-Bildpaare und Flaechenumrisse
aufgenommen, Exit 0. Unit-Tests decken skalierte/rotierte Pilze, Lufttreffer,
negative Sectiongrenzen und Shader-Windposen gegen autoritative Windhuellen ab.

## Messung

`cargo run -p terra-storage --example region_benchmark`: 16384 Edits mit Quellen,
16 Regionen und 16 identischen Spielerinventaren, identischer logischer Zustand.
Erfolgreicher binaerer Reload wurde mit dem Eingabejournal verglichen.

| Metrik | Wert |
|---|---:|
| Alte TOML-Struktur | 3980604 Byte |
| Binaere Dateien insgesamt | 270140 Byte |
| Groessenreduktion | rund 93.2 Prozent |
| TOML-Serialisierung ohne Dateischreiben | 32.106 ms |
| Binaeres Encode/Schreiben/fsync | 7.333 ms |
| Unveraenderter erneuter Save | 4.974 ms |
| Binaeres Laden inklusive Journalvalidierung | 49.724 ms |

Dies ist ein einzelner Linux/tmpfs-Lauf, kein SSD-/Festplattenbenchmark. Die
unterschiedlichen Zeitumfaenge sind keine behauptete Disk-Speedup-Rate. Alte
TOML-Ladezeit, GPU-Frametimes und Gameplay-FPS wurden hier nicht vergleichend
gemessen. Region-Paging/Komprimierung und Bereinigung alter, nicht referenzierter
Dateien sind noch offen; das Journal wird weiterhin vollstaendig im RAM gehalten.

## Validierung

Alle fuenf angeforderten Cargo-Kommandos bestanden: fmt --all --check,
check --workspace, test --workspace, clippy --workspace --all-targets mit
-D warnings, build --workspace. 574 Tests bestanden, ein bereits vorhandener
Langzeit-Soak bleibt ignoriert. Der finale Lauf verwendete normale parallele
Testausfuehrung, CARGO_BUILD_JOBS=2 und keine ausgelassenen Tests.

Separater Headless-Build und Start ohne DISPLAY/WAYLAND: 27 Sections, Exit 0.
Normaler Dedicated-Start mit echtem Save-Verzeichnis und SIGINT: finaler Save
erfolgreich, Exit 0; anschliessender Start uebernahm alte v1-Metadaten zu v2 mit
Sicherung. Der getrennte Dependency-Baum enthaelt keine Render-/UI-/wgpu-/winit-
Abhaengigkeiten. Postcard 1.1.3 und Bevy Math 0.19.1 waren bereits im Lockfile;
sie werden direkt benutzt, ohne Versionsupgrade.

Der echte TiltUI-Menue-/UDP-Lauf erzeugte `worlds/Cedar Valley/`, speicherte
einen autoritativen Edit, liess den Integrated Host enden, startete ihn erneut
und verifizierte denselben Zustand. `world.toml` meldet Format 2, echte `.region`
und UUID-`.data` liegen in den geforderten Ordnern; keine aktive `edits.toml`.
Beispielzustand unter `sample-world/` und Screenshots/Logs sind archiviert.
Weder Loeschen einer Quelldatei noch korrupte/unterbrochene Binaerdaten werden
als erfolgreiche Migration oder leere Standardwelt ausgegeben.

Mehrdimensionenbetrieb, mehrstuendige Crash-/Lastabnahme und plattformuebergreifende
Durabilitaetspruefungen bleiben offen. Native Nachweise stammen von Linux/Wayland.
Dateien sind derzeit auf 64 MiB begrenzt; groessere Datensaetze melden einen
Savefehler statt gekuerzt geschrieben zu werden. Weitere Groessen-/Pagingarbeit
steht in TODO. Die Renderabnahmen stammen von
RADV RX 6950 XT/Mesa 26.2.4. Das bestehende Wassercolumn-/CustomMesh-Backend wurde
nicht durch einen vollstaendigen Hydrodynamik-/Collider-/Dimensions-Rewrite ersetzt.
