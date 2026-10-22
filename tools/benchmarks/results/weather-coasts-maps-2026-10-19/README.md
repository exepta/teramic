# Phase 74: Karten, Wetterstaerken Und Meeresufer

Implementierung und native Abnahme unter Linux/Wayland mit Vulkan,
AMD Radeon RX 6950 XT, RADV/Mesa 26.2.4. Keine neuen Dependencies,
keine geaenderten Seeds, Block-IDs oder Generatorversionen.

## Kartenfehler

Der Mapping-Dauerlauf hatte mehrere Ursachen: Jede Wettertoenung aenderte die
Kartenpalette, bereits erfasste Quellrevisionen gingen mit Detail-Eviction verloren,
und unveraenderte Disk-Ergebnisse loesten erneute Paintjobs aus. Stabile authored
Albedos, getrennte Quellrevisionen und Inhaltsvergleiche verhindern diese Schleifen.
Ein kurzlebiger Arbeitsstatus wird entprellt statt bei jeder Mikroaufgabe umzuschalten.

Die Detailerfassung war auf 96 m beschraenkt; hinzu kamen ein +/-32-m-Hoehendeckel
und vollstaendig flach werdende entfernte Uebersichten. Jetzt werden alle geladenen
Spalten fortlaufend erfasst, weiterhin maximal vier Snapshots pro Durchlauf.
Fernuebersichten behalten 4x4 Hoehen-/Wasserwerte, vollstaendige TRMAP002-Kacheln
ihre Meterhoehen. Vollstaendige Topflaechen statt Pixelsplats schliessen Relief-Luecken.

Die optionale echte 3D-Karte zeichnet geschlossene Seitenflaechen, lokale Kontakt-
und Richtungsverschattung, ein groesseres rechteckiges Renderziel und MSAA4.
2.5D/3D nutzen dieselben bekannten Hoehen, Pan/Zoom/Marker dieselbe Projektion.
Rueckkehr von einem rechteckigen 3D-Renderziel ersetzt die 2.5D-Textur mit der
korrekten quadratischen Groesse statt inkompatible Pixelbytes weiterzuverwenden.
Die FullMap-Werkzeugtexte und Metadaten sind zentriert.

MiniMap: Name direkt am Icon, aber nicht an randfixierten Pins; keine Distanzen.
FullMap und Weltlabels behalten ihre Entfernungen.

## Naesse Und Brandung

Die bisherige einheitliche Blockabdunklung ist aus dem Terrain-Vertexfarbpfad
entfernt. UV1 traegt lokale Naesse und konservative Dachexposition; ein geteilter
WetMaterial-Shader verwendet Weltkoordinaten fuer Becken, teilweise nasse
Flaechen, Glanz und kleine Regenrippel. Im Regen wird exponierte Naesse aus dem
gemeinsamen Wetterwert gespeist, nicht dem Fertigstellungszeitpunkt einer Section.
Trockene Flaechen ueberspringen die Pfuetzenberechnung, Materialien bleiben geteilt.

Eine bestehende vorbereitete 3x3-Spaltennachbarschaft verbindet Seehoehenwasser
mit lokalen Ozeanproben. Nur passende flache Ufer erhalten eine gebatchte Auflaufhaut
im vorhandenen Wassermaterial; Weltphase und Tiefenkonturen lassen Schaumbaender
ankommen, auflaufen und zurueckweichen. Ein Teilfilm bleibt kurz nass.
Seehoehen-/Ozeangewichte in vorhandenen Wassermetadaten vermeiden neue Attribute
und Materialwechsel fuer die anlaufenden Wasserbaender. Binnengewasser, platziertes
Solid Water und Wasserfall-Metadaten werden nicht pauschal in Brandung umgewandelt.
Ein kleiner Abhaengigkeitsschluessel laesst nur betroffene Kuestensections erneut
komponieren, wenn vorbereitete Nachbarspalten ankommen oder Nachbarbloecke
geaendert werden. Wasserbelegung wird direkt unterhalb der Oberflaeche geprueft;
auch gebaute und abgegrabene Ufer sind zulaessig. Begrenzte Welten erfassen ihre
Dachhoehen mit 31 direkten Section-Abfragen statt mit einem weltweiten Loaded-Scan.

Die Brandung ist bewusst kosmetisch. Sie aendert keine Fluid-Fuellstaende,
erzeugt keine weiteren Gameplay-Wasserzellen und simuliert keine Gezeiten.
Logische Blocknaesse bleibt ein Skalar; ihre Teilflaechen entstehen im Shader,
nicht als Millionen gespeicherter nasser Texel.

## Wolken Und Wind

Flache Noise-Ebenen sind durch verbundene Voxel-Wolkenvolumen ersetzt:
neun asymmetrische Puff-Volumen, unterschiedlich tiefe Unterseiten, grosse
zentrale Kuppeln und nur exponierte Faces. Ein Worker erzeugt einen geteilten
Himmelsbatch. Wind bewegt ihn ohne Geometrie-Upload pro Frame; CPU-Meshkopien
werden nach GPU-Uebergabe freigegeben. Weiss/grau/dunkel und Volumen folgen
Wetterfronten. Fernnebel ersetzt auffaelliges Screen-Door-Punktmuster.

Sturmwind verwendet eine staerker ansteigende, begrenzte Auslenkung bis 4x des
alten Maximums und kohaerente Boeen. Materialuniforms, exakte Client-Prop-Treffer,
Auswahlkonturen, Renderbounds und konservative serverseitige Swept-Treffergrenzen
passen zusammen. Servergrenzen folgen der aktuellen Wetterstaerke und Windrichtung,
nicht permanent dem maximal moeglichen Sturm. Letzteres erzeugte unsichtbare
Nachbarpflanzen-Treffer und liess einen bestehenden Prop-Abbautest reproduzierbar
scheitern. Mit dem gerichteten aktuellen Sweep besteht dieser Test wieder;
Sturm- und Gegenrichtungsabdeckung bleiben erhalten.

## Gewitter Und Schnee

```text
/weather set thunder
/weather set thunderstorm
/weather set snow 1
/weather set snow 2
/weather set snow 3
/weather set snow 4
/weather set snow 5
/weather set blizzard
```

In der Dedicated-Konsole ohne Slash, mit bestehenden Vorschlaegen/Spielerrechten.

| Schnee | Flocken relativ zu 1 | Groesse relativ zu 1 |
| --- | ---: | ---: |
| 1, leicht | 1.0 | 1.0 |
| 2 | 2.2 | 1.0 |
| 3 | 2.6 | 1.3 |
| 4 | 3.0 | 1.75 |
| 5, Blizzard | 4.0 | 2.0 |

Qualitaets-, Sicht- und Dachbudgets begrenzen die tatsaechlich sichtbaren Flocken.
Blizzard hat staerkeren Wind. Alte Clear/Rain/Snow/Storm-Befehle bleiben gueltig.

Die Autoritaet plant stabile gemeinsame Blitzereignisse: Thunder etwa alle 26
Wettersekunden, Thunderstorm 13, starke automatische Klimafronten 22, jeweils nach einem
deterministischen Vorlauf. 12% der Ereignisfenster planen einen Einschlagskandidaten;
nur wirklich geladenes exponiertes Terrain wird getroffen, sonst bleibt es ein
Wolkenblitz. Spieler-/Tierschaden, Tierdrops und Feuer verwenden vorhandene Systeme.
Clients zeichnen einen kurzen verzweigten Blitz und lokalen Lichtimpuls.
Der originale synthetisierte Donner kommt ueber den vorhandenen Audiomixer,
mit Entfernungslaufzeit, Spielerkoerper als Hoerer und Unterwasserfilter.
Blitz-/Donneralter wird von Wettersekunden in Echtzeit umgerechnet. Die erste
Weltaufnahme setzt nur den Praesentations-Baseline; gespeicherte Blitze werden
beim Beitritt nicht erneut abgespielt. Entfernte Himmelsblitze werden abgeschwaecht,
und Donner wartet nach Laufzeit sortiert statt hinter einem weiter entfernten Ereignis.

Protokoll **42**, Wetterindex **IDX6** mit Lesern fuer IDX5/IDX4 und aeltere Indizes.
REG9, PLY3 und TRMAP002 bleiben unveraendert. Client und Server gemeinsam aktualisieren.

## Pruefungen

Die abschliessenden Cargo- und nativen Protokolle stehen in `logs/`.
Build-/Testjobs laufen mit `CARGO_BUILD_JOBS=1`, damit paralleles Bevy-Linken
den Speicher des gemeinsamen Entwicklungsrechners nicht ueberlastet.

```bash
cargo fmt --all --check
cargo check --locked --workspace
cargo test --locked --workspace
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo build --locked --workspace
```

Alle fuenf Pruefungen bestanden, Clippy ohne Warnungen. **1154 Tests bestanden**;
ein vorhandener 180-Sekunden-UDP-Soak ist explizit ignoriert und wurde hier nicht
erneut gestartet. Enthalten sind 285 Rendering-, 279 Welt-, 211 UI-, 100 Netzwerk-,
103 Core- und 49 Speichertests sowie Client/Server/Spieler/Physik/Voxel-Tests.
Der Dedicated Server startet ohne DISPLAY/WAYLAND_DISPLAY, generiert 27 Sections
und beendet seinen Smoke-Test erfolgreich. Sein normaler Abhaengigkeitsbaum
enthaelt weder WGPU/Winit/Bevy-Render noch TiltUI oder terra-render/terra-ui.

Der native Kartenlauf umfasst 52 Schritte mit echten Eingaben, Peer-/Mobmarkern,
allen MiniMap-FPS-Modi, Hoehlenebene, Flug, Pan/Zoom, 3D-Pixel-/Renderzielpruefung,
Wegpunkt-CRUD, Iconauswahl und Save/Reload. Der native Wetterlauf umfasst 29 Phasen:
Regen, Schnee 1..5/Blizzard, Dachregeln, Low/Off, Nacht, Naesse, Wolkenfront,
gemeinsame Blitz-/Einschlagspraesentation und natuerliches Meeresufer.
Die beiden abschliessenden Uferaufnahmen warten auf gegensaetzliche Phasen des
echten Produktions-Wellenzyklus; kein eingefrorener Testshader.

Native Karten- und Wetterlaeufe bestanden. Der abschliessende Wetterlauf ist
[`terra74-weather-final.log`](logs/terra74-weather-final.log); keine Shaderfehler,
Warnungen oder Panics. Er erfasst rund 50.3k exponierte Wolkenfaces in einem
Batch, fuenf Wolken-Meshuploads ueber den gesamten Ablauf, 13 Niederschlagsuploads
und 890 gebatchte Auflaufquads am natuerlichen Testufer. Die Wolkenuploads betreffen
Qualitaets-/Ankerwechsel, nicht die Bewegung jedes Frames.

Der native Chatlauf prueft TiltUI-Tastatur, Locate/Teleport, autoritatives Wetter,
Farben und Help-Pagination. Linux-PTY prueft Tab-Vervollstaendigung von `weather set sn`,
Schnee 4, Thunderstorm und Help-Seite 2; `TERMINAL_RESTORED` bestaetigt sauberes
Zuruecksetzen des Terminals. Der Pipe-Fallback weist Schnee 9 ab, akzeptiert danach
Schnee 2 und Thunder und beendet den Server mit `stop`.

### Kurze Framefenster

1920x1080, Wetter High, Vulkan/RX 6950 XT; native Fixture ohne parallelen Cargo-Lauf.
Die Tabelle beschreibt den **neuen Stand**, keinen identisch gefahrenen alten Build.
Die Blickrichtungen, warmen Caches und Fensterlaengen unterscheiden sich je Phase;
insbesondere ist die Clear-Phase kein gueltiger FPS-Baselinevergleich zu Rain.
Kein separater GPU-Timer, RSS-/VRAM- oder Langzeitvergleich wurde erhoben.

| Phase | Frames | FPS | p95 ms | p99 ms | Niederschlagsquads |
| --- | ---: | ---: | ---: | ---: | ---: |
| 1, Clear/Anlauf | 219 | 145.68 | 7.620 | 8.276 | 0 |
| 2, Regen | 251 | 166.94 | 6.712 | 7.376 | 2410 |
| 16, Schnee 1 | 241 | 160.32 | 7.152 | 8.522 | 578 |
| 17, Schnee 2 | 231 | 153.22 | 7.231 | 9.330 | 1445 |
| 18, Schnee 3 | 235 | 155.81 | 7.481 | 9.080 | 1734 |
| 19, Schnee 4 | 244 | 161.92 | 6.831 | 7.523 | 1734 |
| 20, Blizzard | 240 | 159.74 | 7.175 | 8.101 | 2312 |
| 21, Gewitter | 245 | 162.98 | 7.107 | 7.765 | 2410 |
| 27, Kuestenwechsel/Auflauf | 1264 | 172.04 | 8.266 | 10.387 | 0 |
| 28, Kuestenansicht/Ruecklauf | 1050 | 182.90 | 6.607 | 7.209 | 0 |

Quads sind allozierte Geometrie, keine sichtbare Flockenzahl: Angebrochene
Schneelagen werden im Shader anteilig ausgeduennt, damit 2.2x/2.6x nicht auf
ganze Lagen aufgerundet sichtbar werden. Niederschlagsbatches sind nicht die
Gesamtframe-Drawzahl; ein weiterer Wolkenbatch bleibt aktiv.

### Ansichten

- [Echte 3D-Topografie](captures/map-37.png), [zentrierte FullMap](captures/map-6.png).
- [MiniMap-Randpins ohne Name/Distanz](captures/map-29.png).
- [Voxel-Wolkenfront](captures/clouds-approaching.png), [Gewitterwolken](captures/storm-voxel-clouds.png).
- [Teilweise nasse Blockflaechen](captures/wet-saturated.png), [Blizzard](captures/snow-level-5.png).
- [Blitzdarstellung](captures/thunder-flash.png), [Bodeneinschlag](captures/lightning-ground-strike.png).
- [Auflaufender Wellensaum](captures/ocean-shore-wave-a.png), [Ruecklauf](captures/ocean-shore-wave-b.png).
- [Chat-Hilfeseite](captures/chat-help-page1.png).

## Grenzen Und Kosten

- Alte reine TRMAP001-Farbkacheln haben keine rekonstruierbaren Hoehen und
  benoetigen einen erneuten Streamingbesuch. Unbekannter Raum bleibt unbekannt.
- 3D ist eine erkundete Topografiekarte, kein vollstaendiger Renderer fuer einzelne
  Props und Hoehlenueberhaenge. Mehr Aufloesung/MSAA kosten zusaetzlichen GPU-Speicher.
- Wolken sind echte 3D-Geometrie, aber kein volumetrisches Lichtstreuungs-/GI-System.
  Sie benoetigen einen weiteren Batch neben Niederschlag; Gewitter zeitweise
  einen Blitzbatch und ein kurzlebiges Licht. Keine Gesamtframe-Drawparitaet behaupten.
- Brandung hat einen begrenzten geladenen Meeresnachbarschaftstest, keine globale
  Gewaessergraphanalyse und keine hydrodynamische Wellenphysik. PBR-Pfuetzenglanz
  bleibt licht-/blickwinkelabhaengig. Die anlaufenden Schaumbaender auf der normalen
  Wasseroberflaeche verwenden Ozean-Biomgewicht; ein isoliertes Becken innerhalb
  dieses Einflusses kann ebenfalls solche Baender zeigen. Die eigentliche
  Ufer-Auflaufhaut verwendet dagegen den lokalen Verbindungstest.
- Gewitter besitzen bisher keine separaten Barrierefreiheits-/Donnerregler;
  Wetterqualitaet und bestehender Umgebungs-Audiomixer bleiben wirksam.
- Kurze native Framefenster sind keine lange Streaming-/Mehrspielerabnahme,
  kein gemessener Vorher-/Nachher-Nachweis und keine GPU-/RSS-/VRAM-Paritaet.
  Diese weiteren Abnahmen bleiben ausdruecklich in TODO.md.
