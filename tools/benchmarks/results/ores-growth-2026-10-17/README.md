# Phase 51: Erze, Wachstum Und UI

Datum: 2026-10-17. Status: implementiert und funktional abgenommen.
845 Workspace-Tests, alle fuenf Cargo-Pruefungen, native Erz-/Oekologie-/Survival-/
Crafting-/Inventarproben, Headless-Abnahme und CPU-Worker-Messpaare liegen vor.
Ein vorhandener 180-s-UDP-Soak bleibt ignoriert. Der Erzschritt wurde aus TODO
nach PATCH verschoben; naechster nummerierter Schritt ist Tier-KI.
Zusaetzliche GPU-/Speicher- und Langzeittests bleiben im technischen Backlog.

## Implementierter Umfang

Sechs Erztypen, gebatchte herausstehende Einschluesse, praezises Picking,
Werkzeugfortschritt, Ressourcen und einfache Werkbankverarbeitung erweitern die
bestehenden Registries und autoritativen Spiel-/Speicherpfade. Verknuepfte
Truhenslots, scrollfreie Tooltips, Rand-Schadenslicht, windbewegte Pilzfasern
und weich sichtbare Grasstadien verbessern die vorhandenen Systeme.

| Material-ID | Erz | Ressource | Minimale Spitzhacke |
| --- | --- | --- | --- |
| 30 | Salz | `teramic::salt_crystal` | Holz, Stufe 1 |
| 31 | Eisen | `teramic::raw_iron` | Stein, Stufe 2 |
| 32 | Kohle | `teramic::coal` | Holz, Stufe 1 |
| 33 | Gold | `teramic::raw_gold` | Stein, Stufe 2 |
| 34 | Diamant | `teramic::diamond` | Eisen, Stufe 3 |
| 35 | Rubin | `teramic::ruby` | Eisen, Stufe 3 |

`iron_ingot` und `gold_ingot` ergaenzen diese sechs Rohstoffe. `IronPickaxe` und
`ItemKind::Mineral` sind angehaengte Varianten; bestehende IDs und Varianten
werden nicht neu nummeriert. Block-JSONs unter `assets/terra/blocks/*_ore.json`,
einzelne Itemdefinitionen unter `assets/terra/items/` und Rezepte unter
`assets/crafting/` bleiben die datengetriebenen Schnittstellen.

## Weltgenerierung Und Kompatibilitaet

- Sparse, deterministische Suchzellen begrenzen Kandidaten pro Section. Globale
  Koordinaten mit euklidischer Division bewahren negative Koordinaten und Raender.
- Nur geeigneter natuerlicher Stein wird ersetzt. Anschliessend werden dieselben
  gespeicherten Edits angewendet; Spielerbloecke und abgebautes Erz bleiben erhalten.
- Tiefe ist relativ zur lokalen Gelaendeoberflaeche. Je Art sind `min_depth`,
  `max_depth`, `frequency`, `radius` und `cave_affinity` einstellbar.
- Frequenz ist eine Suchzellen-Chance, kein globaler Erzprozentsatz. Salz nutzt
  10-m-, andere Arten 32-m-Zellen. Radius ist 0.5..8 m, Frequenz/Affinitaet 0..1,
  Tiefe 3..1024 m mit nichtleerem Intervall.
- Salzreiche geologische Regionen beschichten Waende der vorhandenen Hoehlen.
  Salz bleibt wandgebunden; es gibt keinen zusaetzlichen Hoehlenalgorithmus und
  keine Zusage, an jedem Rasterpunkt eine Salzkammer zu finden.
- Neue Welten speichern `ores.enabled = true, ores.revision = 1`. Alte Welten ohne
  diesen Vertrag bleiben aus/0. Aktivierung ohne nativen Featurestand wird abgewiesen.
- Kein Retrogen alter Welten. Generator 5, Weltformat 2, Seed und Welt-ID bleiben
  bestehen. Erze sind eine explizite gespeicherte Featureerweiterung.
- Dedicated uebernimmt bei fehlender alter globaler Erzoption den Vertrag seines
  Saves. Nur ein tatsaechlich neuer Save wird automatisch nativ angelegt. Explizite
  Einstellungen mit Revision 1 werden nicht ersetzt.
- Neue Items benoetigen Protokoll 25 auf Client und Server. REG5 und PLY3 sowie
  bestehende Leser fuer REG4/REG3/REG2/REGN bleiben erhalten.

Die [Salzwandprobe](salt-coverage.log) mit Seed 42042 erfasst 157 Salzstellen unter
1422 geprueften Wandstellen in Salzregionen (11.04 Prozent), gegenueber 72 unter
18841 gewoehnlichen Hoehlenwandstellen (0.38 Prozent). Die reale freigelegte Stelle
`[-115, -34, 108]` wird auch in der nativen Erzprobe verwendet. Das ist eine
Stichprobe von Hoehlenwaenden, kein behaupteter Salzanteil des gesamten Weltvolumens.

## Rendering Und Interaktion

Normale Cube-Geometrie bleibt im Section-Meshing. Geschlossene Felskerne und
facettierte Einschluesse werden nach Material gebatcht, nicht pro Kristall als
Entity oder GLB gerendert. Das geteilte Modul `terra-world::ore_shapes` liefert
deterministische Geometrie fuer Darstellung und genaue Auswahl. Herausstehende
Spitzen gehoeren weiterhin zum tragenden Erzblock; sie sind weder eigene Items
noch zusaetzliche logische Zellen. Grobe LODs behalten den Primaer-Erztyp.

Abbau benoetigt die passende Spitzhackenstufe. Bestehende Rechte, Revisionen,
Haltezeiten, Creative-Sofortabbau, Drops und Inventarregeln bleiben massgeblich.
Die einfache Verarbeitung ist absichtlich ein Werkbankrezept: zwei Roheisen
oder Rohgold plus eine Kohle ergeben einen Barren, shapeless. Drei Eisenbarren
und zwei Staebe ergeben shaped eine Eisenspitzhacke. Ein echter Ofen mit
Fuel-/Zeit-/Stationsinventar ist weiterhin nicht implementiert.

## UI Und Oekologie

Verknuepfte Truhen besitzen dieselben responsiven Slotkanten wie das Hauptinventar:
66/53/46/36/31 logische Pixel. Links passen sechs oder drei Spalten; sehr kleine
Fenster stapeln die Ansicht mit neun Spalten. Tooltips haben sichtbaren Overflow
statt Scrollflaechen, begrenzte Breite und Zeichenumbruch fuer lange Registry-IDs.
Sie duerfen Inventar-Scrollclips verlassen. Beliebig lange Modtexte auf beliebig
kleinen Fenstern sind dennoch keine garantierte vollstaendige Darstellung.

Rote Trefferbeleuchtung liegt an vier Raendern und klingt weich ab. Unter 30 Prozent
Gesundheit kommen pulsierende Randlichter und der vorhandene TiltUI-`signal-lost`-
Filter zusammen. Heilung, Tod, Respawn und Creative beenden den kritischen Zustand.
Keine rote Vollbildflaeche und kein weiterer UI-Renderer.

Pilzfasern reichen bis etwa 2.5 m, bleiben am Ansatz verankert und bewegen sich
im vorhandenen Windshader. Grasspruesslinge interpolieren neue Groessenstadien
ueber 2.4 s aus der Wurzel. Persistente logische Stadien und serverseitige
Wartezeiten bleiben erhalten; erstes Laden eines gespeicherten Stadiums beginnt
nicht erneut bei Null. Keine Entity pro Faser/Halm und keine neue Speicherung.

## Abnahme

| Pruefung | Status |
| --- | --- |
| `cargo fmt --all --check` | [Bestanden](fmt.log) |
| `cargo check --workspace` | [Bestanden](check.log) |
| `cargo test --workspace` | [845 bestanden, ein vorhandener 180-s-UDP-Soak ignoriert](tests.log) |
| `cargo clippy --workspace --all-targets -- -D warnings` | [Bestanden](clippy.log) |
| `cargo build --workspace` | [Bestanden](build.log) |
| Native Erzformen und Picking | [Bestanden: sechs natuerlich freigelegte Erztypen, sechs GPU-Aufnahmen](native-ores.log) |
| Autoritativer Erzabbau, Drops und Weiterverarbeitung | [Direkte/UDP-Servertests und reale JSON-Rezept-/Inventartests bestanden](tests.log); kein gesonderter kompletter nativer Playthrough der Erzwerkzeugkette |
| Native Crafting-/Truhenansichten, Slotgroessen und scrollfreie Werkbank-Tooltips | [Voller Lauf bestanden: Seitennavigation, vier Werkzeuge, 36 Slots, verknuepftes Lager, reservierte Transfers, REG5-Reload und 22 Aufnahmen](native-crafting.log); relevante Ansichten gesichtet |
| Native Spielerinventar, OS-Mausgesten und allgemeine Tooltips | [Voller Lauf bestanden: neun Aufnahmen, echte OS-Draggesten, Server-ACK, scrollfreie Tooltips und erreichbare letzte Scrollreihe](native-inventory.log); Hotbar-Tooltip nativ gesichtet |
| Native Schadensraender, kritischer Zustand und Respawn | [Voller Survival-Lauf bestanden: vier Randflaechen, kritischer Filter, Tod/Respawn, HUD-Drag/Save/Reload und zwoelf Aufnahmen](native-survival.log) |
| Native Windfasern und weiches Graswachstum | [Bestanden: neun Aufnahmen, alle vier Stadien und vier GPU-Uebergaenge](native-ecology.log) |
| Dedicated-Start und rendererfreier Abhaengigkeitsbaum | [27 Sections ohne Display-/Wayland-Umgebung gestartet](headless.log), [kein bevy_render/bevy_ui/bevy_winit/wgpu/TiltUI im Abhaengigkeitsbaum](server-tree.log) |
| CPU-Worker-Featurevergleich | Vier ON/OFF-Messpaare unten; keine historischen Buildvergleiche |
| Native FPS-/Frame-/GPU-/Speicherwerte | Nicht gemessen; kein Paritaetsnachweis |

Abgedeckte Regressionen im Workspace-Testlauf: gleiche Samples bei verschiedenen Generierungsreihenfolgen,
negative Koordinaten, Sections mit Randvorkommen, bestehende Edit-/Spielerregionen,
alte Weltstempel, Dedicated-Neustart ohne neue globale Option, Werkzeuggrenzen,
Rezeptverbrauch, Ressourcen-Reload und autoritative Multiplayerzustaende.
Der zusaetzliche Fail-Closed-Test fuer fehlende Rubin-Mechanik verweigert Abbau
und Drop auch mit Eisenspitzhacke; ein fehlender Assetvertrag wird nicht durch
einen erlaubenden Werkzeugfallback ersetzt. Ein kompletter manueller nativer
Erzabbau-bis-Werkzeug-Playthrough bleibt eine ergaenzende Spielprobe, keine fehlende
Implementierung des bereits direkt und ueber UDP geprueften Spielpfads.

## Performance Und Grenzen

### CPU-Worker, Feature Aus Gegen An

Identischer optimierter Dev-Build, Erzfeature mit Revision 1 ueber
`TERRA_BENCH_ORES=off/on` umgeschaltet. **Dies ist kein historischer Vorher-/Nachher-
Buildvergleich.** Die aktiven Erze erzeugen absichtlich andere Materialbelegungen
und mehr Greedy-Faces. Der Vergleich beziffert ihre zusaetzliche CPU-Arbeit.

Host: AMD Ryzen 9 5900X, vier Benchmarkworker, Seed 42042, Generator 5. Jeder Lauf
baut 4375 Sections in 25x25 horizontalen Spalten und sieben vertikalen Sections,
mit je einem Warmup und fuenf Messungen. Oberflaechenzentrum ist Section `(0,7,1)`,
Untergrundzentrum `(0,-3,0)`; horizontale/vertikale Radien 12/3.

| Region / Modus | Erze aus, Median s | Erze an, Median s | Mehrarbeit |
| --- | --- | --- | --- |
| Oberflaeche, Headless-Generierung | 0.257071 | 0.298418 | +16.08% / +41.347 ms |
| Oberflaeche, Render-Vorbereitung | 0.464425 | 0.520461 | +12.07% / +56.036 ms |
| Untergrund, Headless-Generierung | 0.433616 | 0.477393 | +10.10% / +43.777 ms |
| Untergrund, Render-Vorbereitung | 0.808082 | 0.885329 | +9.56% / +77.247 ms |

Rohdaten: [Oberflaeche aus](worker-surface-off.log),
[Oberflaeche an](worker-surface-on.log), [Untergrund aus](worker-underground-off.log),
[Untergrund an](worker-underground-on.log). Der
[Buildlog](benchmark-build.log) bestaetigt das optimierte Dev-Profil.

Oberflaechen-Quads steigen von 497877 auf 512458 (+2.93 Prozent), Untergrund-Quads
von 1023726 auf 1042715 (+1.85 Prozent). Props bleiben bei 43952 beziehungsweise
null pro Region, Sections bei 4375. Stabile Checksummen innerhalb jedes Messpaars
bestaetigen wiederholbare Ausgaben, ON/OFF unterscheiden sich erwartungsgemaess.
Die Konfiguration verwendet den finalen 10-m-Salzsuchraster und Radius 6; vorlaeufige
12-m-Zwischenstaende werden nicht in dieses Messpaar gemischt.

Diese Zeiten sind Wall-Clock-Durchsatz ueber alle Worker, keine Hauptthread-
Frametimes. Die ebenfalls geloggten Workerzeiten addieren verstrichene Spannen und
sind kein isolierter CPU-Zykluszaehler. Render-Vorbereitung enthaelt gepaddete
Voxel, Greedy-Meshing, Props und Wasser; die Differenz zur Headless-Zeit ist **keine
isolierte Meshing-Dauer**. Bevy-ECS-Pacing, Sessioncaches, Render-spezifische
Kristall-/Micro-Mesh-Aufbereitung, GPU-Uploads, GPU-Zeit, Draws und Arbeitsspeicher
werden hier nicht gemessen. Aus den Summen folgen weder FPS noch p95/p99 oder
Speicherparitaet; insbesondere kein Vergleich zur historischen 2.6-s-Erstgenerierung.

### Native Grenzen

Die bisherigen nativen Erz-/Oekologieproben verwenden eine Radeon RX 6950 XT,
Vulkan/RADV, Mesa 26.2.4. Die Erzansichten nutzen ausdruecklich ein diagnostisches
Inspektionslicht, damit die Formen im echten Hoehlenraum lesbar sind. Sie ersetzen
keine Abnahme natuerlicher Hoehlenbeleuchtung oder eine Art-Direction-Freigabe.
Die Aufnahmen liegen unter [ores/](ores/) und [ecology/](ecology/).

Die vollstaendige Survival-Wiederholung mit zwoelf Bildern ist bestanden; die
kritischen Randlichter wurden nativ gesichtet und lassen die Bildmitte frei.
Der vollstaendige Craftinglauf ist ebenfalls bestanden: Verknuepfte Doppeltruhen
verwenden die gleichen Slotgroessen; der gesichtete Werkbank-Tooltip besitzt
weder Scrollbar noch abgeschnittene Rahmenkante. Echte Katalog-Seitennavigation
wurde fuer Werkzeuge auf Seite 2 und die Rueckkehr zur Truhe auf Seite 1 beobachtet.
Headless-Start erfolgte mit nicht gesetztem `DISPLAY` und `WAYLAND_DISPLAY`.
Der Inventarlauf erzeugte neun erfolgreiche Bilder mit echter OS-Mausbedienung.
Zusammen mit sechs Erz-, neun Oekologie-, zwoelf Survival- und 22 Craftingbildern
sind es 58 erfolgreich gespeicherte GPU-Aufnahmen. Zwei Crafting- und ein
Survivalauftrag forderten denselben Screenshot-Render-Target im selben Frame
doppelt an; Bevy uebersprang den redundanten Auftrag mit einer Readbackwarnung.
Keine Shaderfehler wurden in den finalen nativen Laeufen beobachtet.

Bekannte Wachstumsgrenzen: Ein ungewoehnlich lange laufender Mesh-Job kann einen
alten Interpolations-Snapshot spaeter wieder aufnehmen. Normale logische Stadien
liegen etwa acht Sekunden auseinander, deutlich laenger als die 2.4-s-Ueberblendung;
die normale native Probe lief mit vier sichtbaren GPU-Uebergaengen erfolgreich.
Waehrend der Wachstumsskalierung koennen kleine Motion-Vector-Abweichungen
auftreten. Diese Faelle sind kein globales Kalender-/Pflanzenphysiksystem und
werden nicht als vollstaendig behoben ausgegeben.

Fruehe komplette UI-Fixtures brachen an veralteten Testannahmen ab: Der nunmehr
55 Items grosse Katalog verlangt echte Seitennavigation; generische
`row-command`-IDs duerfen nicht ohne den sichtbaren HUD-Layouttext ausgewaehlt
werden. Die korrigierten Survival-, Crafting- und Inventarwiederholungen sind
vollstaendig bestanden. Inventarproben warten auf die
vollstaendige asynchrone Vorschauqueue einschliesslich neuer Mineralicons; ein
Workbench-Hover wird nur bei nachweislich ausserhalb liegendem echtem OS-Cursor
begrenzt neu angefahren. Produktionszustand oder Inventarbestaende wurden
zur Umgehung nicht simuliert und die Zielassertionen nicht entfernt.

Ein abschliessender nativer Performancevergleich muss Buildstand, Feature-
konfiguration, Seed, Region, LOD/Sichtweite, Bewegungsablauf, Hardware und Laufdauer
nennen; FPS, p95/p99 und RSS/VRAM sind getrennt auszuweisen. Fehlende GPU-Timer oder
Subdrawzahlen werden nicht geschaetzt.

Keine allgemeine FPS-/Speicherparitaet, keine native Bildabnahme ohne Ansichten und
keine vollstaendige Referenzqualitaet behauptet. Kurztests ersetzen keine langen
Streaming-/Mehr-GPU-/Mehrrechnerlaeufe. Echte Ofenprozesse, Retrogen alter Welten,
ein neuer Salzkammer-Hoehlentyp und separate Gameplay-Kristallzellen sind kein
Bestandteil dieser Implementierung.
