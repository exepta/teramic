# Phase 66: Charaktere, Fackellicht, Tiermodelle

## Umfang

Weibliche/maennliche Referenzavatare mit Ellenbogen-/Kniegelenken, sichtbarer
Ego-Hand, detailliertere Schafe/Kuehe/Schweine/Bienen/Clownfische sowie Fackel-
und Barrenmodelle. Der Flussfisch und die autoritative Tier-/Spielerphysik bleiben
unveraendert. Schaden blendet einen wesentlich dezenteren Rand ein.

Zehn Koerpermeshes und ein Blinzelmesh pro Avatar statt sechs plus Blinzelmesh:
vier zusaetzliche Mesh-Entities je dargestelltem Avatar. Die Ego-Hand verwendet
eine weitere Mesh-Entity mit demselben gecachten Unterarmmesh. Bevy kann weiterhin
gleiche Mesh-/Materialpaare instanzieren; Entityzahlen sind keine GPU-Drawzahlen.
Tier-Teilzahlen bleiben
acht fuer Huftiere, vier fuer Bienen und zwei fuer Fische. Detailgeometrie erzeugt
keine weiteren Entities. Eine hohe Dreieckszahl ist kein Nachweis einer
pixelidentischen Referenzrekonstruktion oder professioneller Motion-Capture-Clips.

## Lichtvertrag

Die Fackeldefinition waehlt `light.falloff: "soft_core"`. Bestehende Definitionen
ohne das Feld bleiben `inverse_square`. Die streng signaturgepruefte Erweiterung
der geladenen Bevy-0.19-PBR-Bibliothek aendert nur markierte Punktlichter.
Reichweitenabschluss, Cluster, Schatten und alle uebrigen Lichtpfade bleiben
bestehen. Vier Meter weicher Kern und achtfacher Lichtfluss reduzieren den
Nah-Hotspot und verstaerken entfernte Flaechen. Analytisch gegen dieselbe alte
Fackel: 0.123-fache direkte Abschwaechung bei 0.5 m, 0.471 bei 1 m, 5.538 bei
6 m und 7.624 bei 18 m. Das sind Kurvenverhaeltnisse, keine gemessene
Bildhelligkeit; Normale, Sichtbarkeit, Belichtung und Tonemapping wirken weiter.

Intern markiert Radius `128.375` den Sonderfall. Die drei PBR-Radiusleser
dekodieren ihn zu `0.38`; CPU-Cluster-/Schattenbounds verwenden die Reichweite.
Quellgroessen-PCSS, Radius-Gizmos und volumetrische Fackellichter sind fuer diese
Quellen nicht aktiviert und muessen vor spaeterer Nutzung angepasst werden.
Bei abweichender Bevy-Signatur bleiben Standardradius/-fluss aktiv; die native
Hoehlenprobe scheitert dann explizit. Es entstehen keine zusaetzlichen
Lichtquellen oder Renderpaesse.

Welt und Item verwenden exakt dieselbe Fackelgeometrie. Die Itemdarstellung
backt die sechs Materialfarben in einen gemeinsamen unlit Batch ohne Emission;
gesetzte Fackeln behalten PBR-Materialien und animiertes Feuer. Damit ist die
eigene Lampe von der Itemdarstellung entkoppelt. Das Lampenpool-Update erkennt
zudem den Austausch des Lichttyps an derselben Koordinate.

## Messbedingungen

Linux/KDE Wayland, Radeon RX 6950 XT/RADV, Mesa 26.2.4. Ein Messclient,
unveraenderter Seed 42042, Sichtweite 16, vertikaler Radius 3, pausiert 10:00,
VSync aus, keine Chunkanimation. Logische 1280 x 800 entsprechen bei 150 Prozent
DPI nativen 1920 x 1200. Derselbe 60-s-Offline-LOD-Hin-/Rueckflug, native
GPU-Diagnostik und `/proc`-/DRM-Peaks mit `measure.py`.

Vorherbinary und Assets wurden vor den Aenderungen gesichert. Der Flug misst
Streaming, nicht dichtes Multiplayer-Avatar-/Herdenrendering. Keine allgemeine
FPS- oder Speicherparitaet aus diesem einzelnen Lauf ableiten. Die nativen
Licht-Vorheraufnahmen liefen teilweise waehrend der Kompilierung und sind
deshalb nur Bildreferenz, kein vergleichbares FPS-Paar.

## Finale Geometrie

Aus den tatsaechlichen Mesh-Indizes gezaehlt, nicht aus Vertexzahlen geschaetzt.
Die Avatarwerte enthalten alle zehn Koerperteile; die gemeinsame Blinzelschicht
mit 48 Vertices ist darin nicht enthalten. Gezielt ausgeformte Knie-/Ellenbogenuebergaenge
schliessen die bei der ersten nativen Laufprobe sichtbaren offenen Gelenkkeile.

| Modell | Dreiecke | Vertices | Teilmeshes |
| --- | ---: | ---: | ---: |
| Mann | 36 860 | 77 624 | 10 |
| Frau | 38 896 | 82 656 | 10 |
| Schaf | 6 600 | 13 200 | 8 |
| Kuh | 2 976 | 5 952 | 8 |
| Schwein | 2 532 | 5 064 | 8 |
| Clownfisch | 2 004 | 4 008 | 2 |
| Biene | 1 476 | 2 952 | 4 |
| Flussfisch, unveraendert | 468 | 936 | 2 |
| Item-Fackel | 2 528 | 5 472 | 1 |
| Eisenbarren | 3 228 | 6 456 | 1 |
| Goldbarren | 3 228 | 6 456 | 1 |

Beide Koerpergeometrien zusammen: 8 602 512 rohe Vertex-/Indexbytes. Blinzelschicht,
Treiber- und Assetverwaltung kommen hinzu. Die Ego-Hand referenziert das vorhandene
Unterarmmesh; sie kopiert keine komplette Geometrie pro Frame. Haut und Kleidung
verwenden starre artikulierte Teile, keine gewichtete Skinning-Deformation.

## Native Lichtpruefung

Alle 14 Hoehlen-/Nachtansichten bestehen mit der finalen achtfachen Flusskorrektur.
Der echte Bevy-PBR-Shader wurde auf Vulkan kompiliert, kein Ersatz-Testshader.
Inventar-/Handfackel und Barren sind zusaetzlich in den Item- und Handproben
sichtbar geprueft. Die im ersten Barrenentwurf stoerenden dunklen Mikrozellraster
wurden vor diesen finalen Aufnahmen entfernt.

| Identische Bildregion | Vorher | Nachher |
| --- | ---: | ---: |
| Nachtboden, Abstand 1 m | 0.030584 | 0.029652 |
| Nachtboden, Abstand 3 m | 0.013503 | 0.016787 |
| Nachtboden, Abstand 6 m | 0.005729 | 0.006977 |
| Hoehlenwand, Nah-Hotspot | 0.254356 | 0.019353 |
| Boden unter Hoehlenfackel | 0.026627 | 0.015687 |

Lineare Luminanz nach sRGB-Dekodierung der nativen PNGs; alle Werte enthalten
Umgebungslicht, Tonemapping und die jeweilige Flammenphase. Keine isolierte
physikalische Bestrahlungsstaerke. Nachtboden: Stage 8, identische 7x7-Pixel-
Proben; Hoehlenwand: Stage 6, 32x32 um (800, 490); Fackelbasis: Stage 5,
32x32 um (820, 900). Die finalen Logproben enthalten Projektionskoordinaten.
Vorherbilder liegen unter `terra66-before-torches`, Nachher unter `terra66-caves`.

Die weiter entfernte warme Ausleuchtung ist sichtbarer, der grelle Hoehlenkern
deutlich gedaempft. Bei 16 m und mehr bleibt das Licht schwach: weder gleichmaessige
18-Block-Helligkeit noch indirekte Beleuchtung behaupten.

## Streamingmessungen

Alle vier Laeufe bleiben dokumentiert, auch der deutlich langsamere erste
Nachherlauf. Waehrend dieser Flugmessungen liefen keine eigenen Cargo-Builds
oder zweiten Testclients. Der alte Build reproduziert spaeter ebenfalls nicht
die initialen 120 FPS. Die Ursache dieser Laufzeitdrift ist nicht isoliert;
das spaete Paar allein beweist daher weder Beschleunigung noch Paritaet.
Bei der Abschlusskontrolle lief zudem ein weiterer Spielprozess. Fremde
Hintergrundlast wurde weder beendet noch durchgaengig aufgezeichnet; ihr Anteil
an der Drift ist nicht quantifiziert. Das sind keine isolierten Systembenchmarks.

| Messwert | Vorher 18:05 UTC | Nachher 18:47 UTC | Vorher 18:49 UTC | Nachher 18:51 UTC |
| --- | ---: | ---: | ---: | ---: |
| Mittlere FPS | 120.35 | 88.58 | 96.88 | 99.16 |
| p95 Frame ms | 12.375 | 15.735 | 15.620 | 14.384 |
| p99 Frame ms | 15.041 | 17.955 | 18.109 | 16.883 |
| Groesster Frame ms | 31.048 | 36.770 | 30.101 | 39.125 |
| Szenenbereitschaft s | 5.648 | 6.981 | 6.735 | 6.052 |
| Main-Opaque-GPU-Pass, Mittel ms | 1.728 | 1.774 | 1.721 | 1.700 |
| Peak RSS KiB | 1 179 600 | 1 181 212 | 1 187 328 | 1 208 192 |
| Peak VRAM KiB | 4 246 560 | 4 317 084 | 4 281 072 | 3 750 908 |
| Mesh-Assets | 10 783 | 10 790 | 10 783 | 10 790 |
| Terrain-Komposition, aufsummierte Worker-s | 15.890 | 16.572 | 16.482 | 17.127 |
| Prop-Komposition, aufsummierte Worker-s | 2.369 | 2.480 | 2.444 | 2.582 |

In allen vier Laeufen: 7 809 geladene Sections, 4 118 gerenderte Sections,
6 351 sichtbare Mesh-Entities, 110 304 dargestellte Props in 1 289 Batches.
Kompositionsjobs unterscheiden sich wegen der asynchronen LOD-Arbeit; Worker-
Sekunden sind keine identische isolierte Generierungszeit. Der GPU-Wert ist
nur ein instrumentierter Pass, keine Gesamt-GPU-Framezeit. Draw-Submissions
waren in diesem Lauf nicht aktiviert; keine Draw-Call-Zahl daraus ableiten.
RSS-/DRM-Werte stammen aus zeitlich gesampelten Prozess-Peaks von `measure.py`.

Rohlogs: `terra66-before-stream.log`, `terra66-after-stream.log`,
`terra66-before-recheck-stream.log`, `terra66-after-recheck-stream.log`.
Die sieben zusaetzlichen Mesh-Assets sind nicht sieben zusaetzliche Draw Calls.
Der Flug enthaelt keine dichte Gruppe sichtbarer Avatare/Tiere. Deren tatsaechliche
Mehrkosten brauchen einen separaten Lasttest, nicht eine Ableitung aus diesem Flug.

## Validierung

- `cargo fmt --all --check`: bestanden.
- `cargo check --workspace`: bestanden.
- `cargo test --workspace --no-fail-fast`: 1007 bestanden, keine Fehler,
  ein bestehender expliziter 180-s-UDP-Soak ignoriert.
- `cargo clippy --workspace --all-targets -- -D warnings`: bestanden.
- `cargo build --workspace`: bestanden.
- Headless-Serverstart ohne `DISPLAY`/`WAYLAND_DISPLAY`: bestanden, 27 Sections.
- Native Vulkan-Proben: 14 Hoehlen-/Lichtansichten, zweimal acht Avataransichten,
  sechs Itemansichten, sieben Hand-/F5-Ansichten und acht Tieransichten bestanden.
- Integrierter Survival-HUD-Test inklusive kritischem Schaden, Tod, Respawn,
  HUD-Verschieben/Speichern und Moduswechsel bestanden.

Neue Regressionen pruefen unter anderem Geometrie/Indizes, 20k-40k-Avatarbudget,
Gelenkabdeckung, Griffpunkte, Handprojektion, Materialtrennung, opt-in Lichtkurve,
Lichttypwechsel sowie den nur beim Nektarsammeln sichtbaren Bienenruessel.
Schadensstaerke und Ausblendung werden unabhaengig von den Bildern zeitlich getestet.
Finale Cargo-/Server-/Native-Logs und Screenshots liegen neben diesem Bericht.
`terra66-mob-geometry.log` enthaelt die separat ausgegebenen Modellzaehlungen.

## Grenzen

Die Referenznaehe wurde an echten GPU-Ansichten geprueft. Gesicht, Haare,
Stoffdetails und Proportionen bleiben aber prozedurale Annaeherungen, keine
1:1-Kopie. Die Gelenke bewegen sich weich, haben jedoch weder Foot-IK noch
Motion Capture; Standbilder belegen keine umfassende menschliche
Animationsqualitaet in allen Spielsituationen. Dichte Mehrspieler-/Tierlast,
lange Flugszenen, andere GPUs und extreme Kamera-FOVs bleiben gesonderte
Abnahmen. Der Server erhaelt keine neue Renderabhaengigkeit, Welt-/Block-IDs,
Physik, Inventarzustaende, Abhaengigkeitsversionen und Weltgenerierung bleiben
unveraendert.
