# Phase 50: Inventar, Stationslager Und Oekologie

17.10.2026. Funktions- und Regressionsabnahme mit echten nativen Ansichten.
Alle fuenf Cargo-Pruefungen, 816 regulaere Tests, Inventar-/Crafting-/Survival-
und Oekologieproben sowie Headless-Start sind bestanden. Beide abschliessenden
Moebelproben bestanden auch die verschaerfte Positivpruefung fuer tatsaechlich
emittierte Abbauteilchen und die verborgene intakte Modell-Auswahlkontur.
Dies ist kein FPS-Benchmark und kein neuer GPU-/Speicherparitaetsnachweis.

Native Umgebung: AMD Radeon RX 6950 XT, Vulkan/RADV, Mesa 26.2.4-arch3.1,
vorhandene Linux-/Wayland-Desktop-Sitzung. Cargo-Lockfile und Dependency-Versionen
unveraendert. Bauen/Testen verwendet begrenzte Parallelitaet, damit Bevy-Linker
nicht durch mehrere gleichzeitige Prozesse die vorhandenen RAM-/Swapbudgets sprengen.

## Umfang

| Anforderung | Implementierter Pfad |
| --- | --- |
| Crafting-Reset ohne Border | TiltUI-Resetbutton inklusive Hover/Focus ohne sichtbaren Rahmen |
| Kritische Gesundheit | Bestehender roter Schadensschein plus echter TiltUI-`signal-lost`-Filter unter 30 Prozent Gesundheit |
| Mouse-Tweaks beim Ziehen | Live-Slotbilder/Mengen und verbleibender Cursorbestand fuer Spieler-, Crafting- und Containerslots; Vorschau ohne vorzeitigen Inventarcommit |
| Keine auffaelligen Warnfarben | Status-/Fehlertexte neutral; kein rotes/gelbes Erschoepfungs-Warnbadge |
| Werkbankrezept | Vier Bretter shapeless; 2x2-Quadrat ist gueltig, Truhen und Werkzeuge bleiben shaped |
| Doppeltruhenansicht | 840 UI-Pixel hohe Ansicht fuer 36 Slots, Scroll-Fallback fuer kleine Fenster |
| Truhe links der Werkbank | Private serverseitige Verbindung zur zuerst platzierten angrenzenden Einzel-/Doppeltruhe, reservierungssichere Transfers |
| Dirt bekommt Gras | Nachbarschaftspruefung, sichtbare gespeicherte Zwischenstadien, anschliessender autoritativer Dirt->Grass-Edit |
| Kleine Abbauteile | Budgetierte Bruchteilchen von tatsaechlichen Block-/Modelloberflaechen |
| Moebel-Hitboxen | Aus denselben Modellbausteinen exportierte Teilvolumen fuer Picking, Sichtlinien und Avian-Kollision statt ganzer Zellboxen |
| Gruene Gras-Inventarvorschau | Gruene Grasoberseite nur fuer die Vorschau; Welttextur bleibt neutral zur Biomfaerbung |
| Haengende Pilzfasern | Verbundene, gebatchte Myzelbaender unter Grosspilzkappen, begrenzte Near-/Middle-Geometrie |
| Kleine Stammpilze | Deterministisches 24-Prozent-Ticket geeigneter Baeume, drei Varianten, echte Prop-IDs/Interaktion/Persistenz |

## Architektur Und Kompatibilitaet

Kein Dependency-Upgrade, kein anderes UI-Framework, kein neues Inventar- oder
Prop-System. Client und Dedicated Server verwenden gemeinsam Protokoll **24**.
Regionen schreiben **REG5**, explizite Leser erhalten REG4, REG3, REG2 und REGN.
Spielerdateien bleiben PLY3, Weltformat 2, Generatorversion 5; Seeds und bisherige
Block-/Welt-IDs bleiben erhalten.

Die erste Nachbartruhe wird ueber private, persistent gespeicherte
Platzierungsordinale im bestehenden EditJournal ausgewaehlt. Beide Zellen der
Werkbank und hoechstens zwei physische Truhenhaelften werden beruecksichtigt.
Faerben und Verbinden erhalten die Reihenfolge; echtes Ersetzen erzeugt eine
neue. Entfernte oder topologisch geaenderte Container schliessen die alte
Ansicht. Alte Saves haben keine historische Bauzeit: Ordinal 0 wird deshalb
stabil nach Koordinaten aufgeloest, nicht als angeblich rekonstruierte Reihenfolge.
Der Client bekommt nur seinen berechtigten privaten Stationssnapshot.

Physische Moebelvolumen werden auf jede beteiligte Section geclippt, auch auf
eine zuerst geladene zweite Haelfte bei noch fehlender primaerer Chunksection.
Innerhalb einer Section verhindert logisches Owner-Dedup doppelte komplette
Modellvolumen. Exakt gleiche geclippte Boxen werden ebenfalls nur einmal
eingefuegt. Itembewegung und Pickup-Sichtlinien verwenden denselben genauen
Weltkontext, damit offene Werkbankzwischenraeume weder unsichtbar blockieren
noch zwar erreichbare, aber nicht einsammelbare Drops erzeugen.

Inventartransfers pruefen die vier Craftingreservierungen vor dem gemeinsamen
Player-/Containercommit. Sperren durch reservierte Zutaten duerfen weder
Items duplizieren noch einen groesseren Anteil eines Stacks entfernen als
frei verfuegbar ist. Ein geoeffneter Rezeptdialog uebernimmt weiter nur ueber
das ausdrueckliche Plus.

### Wachstumsbudget

Der vorhandene autoritative Welt-Tick prueft maximal zwei Discovery-Sections
pro Sekunde, behaelt maximal 256 aktive Zellen und fuehrt maximal acht
Wachstumsaenderungen je Sekunde aus. Blockedits aktivieren begrenzt ihre direkte
Nachbarschaft; die Warteschlange endet bei 4096 Kandidaten. Es gibt keine
vollstaendige Terrain-Neugenerierung pro Wachstumsframe.

Nach 8-16 Sekunden starten vier Stadien im Abstand von jeweils etwa acht
Sekunden. Unter geringer Last dauert der komplette Zyklus etwa 40-48 Sekunden;
volle Budgets koennen ihn verlaengern. Vollbedeckung, Feuer, starke Verkohlung,
Schnee und Ueberflutung stoppen Wachstum. `struct_block` erstickt den Boden nicht.

Vorhandene Prop-Datensaetze speichern die Stadien: Grass 64..67 sichtbar,
68..71 nur als unsichtbarer Marker, wenn bereits ein echter Grasbueschel auf
der Dirt-Zelle steht. Diese kosmetischen Daten sind nicht anwaehlbar oder
erntbar und verbrauchen keine der vier Gameplay-Pflanzenpositionen. Vier
vorhandene Blumen werden weder verdraengt noch verschoben. Zwischenstufen
erhoehen ausschliesslich Prop-Epochen; der Abschluss aendert die Block-ID im
bestehenden Journal. Unload pausiert vorhandenen Fortschritt. Bereits
gespeicherte Stadien bleiben nach Reload erhalten; eine noch unverstrichene
Startwartezeit wird neu angesetzt, kein globaler Kalendertimer vorgetaeuscht.

### Pilzdetails

Grosspilzkappen erhalten an einem deterministischen Teil ihrer sichtbaren
Unterseiten verbundene 0.35-1.30 Meter lange Baender mit drei Segmenten.
Near nutzt bis zu zwei Baender je ausgewaehlter Zelle, Middle eines, Far keines.
Geometrie wird in die vorhandenen Section-/Materialbatches geschrieben.

Eiche, Herbstbaeume, Birke und Fichte haben ein 24-Prozent-Ticket fuer einen
kleinen roten, braunen oder weissgrauen Stammpilzcluster. Die tatsaechliche
Platzierung erfordert freie Flaeche und Stammholz. Palmen und Grosspilze sind
ausgenommen. Die Suche wird im bestehenden PreparedColumn-Cache einmal
vorbereitet und fuer vertikale Sections wiederverwendet. Mushroom-Varianten
128..130 besitzen kanonische Wandanker, genaue gemeinsame Mesh-/Pickingformen,
stabile IDs und das vorhandene Removal-Journal. Entfernen des Supports oder
verbauende Nachbarzellen entfernen nur betroffene Cluster, auch an Sectiongrenzen.

## Pruefstatus

| Pruefung | Ergebnis | Nachweis |
| --- | --- | --- |
| `cargo fmt --all --check` | Bestanden | [fmt.log](fmt.log) |
| `cargo check --locked --workspace` | Bestanden | [check.log](check.log) |
| `cargo test --locked --workspace` | 816 bestanden, 0 fehlgeschlagen, 1 bestehender Langzeittest ignoriert | [tests.log](tests.log) |
| `cargo clippy --locked --workspace --all-targets -- -D warnings` | Bestanden | [clippy.log](clippy.log) |
| `cargo build --locked --workspace` | Bestanden | [build.log](build.log) |
| Headless-Server-Start | Bestanden, 27 Sections, sauberer Abschluss ohne Fenster | [headless.log](headless.log) |
| Server-Abhaengigkeiten | Kein bevy_render/bevy_ui/bevy_winit/wgpu/TiltUI im Dedicated-Baum | [server-tree.log](server-tree.log) |
| Native Inventar-/OS-Mausprobe | Bestanden, 9 GPU-Aufnahmen, Live-Drag und Scroll-Reichweite | [native-inventory.log](native-inventory.log) |
| Native Crafting-/Stationsprobe | Bestanden, vier Werkzeuge, 36 unverdeckte Truhenslots, reservierte Transfers und REG5-Reload | [native-crafting.log](native-crafting.log) |
| Native kritische Gesundheit/Respawn | Bestanden, HUD-Filter sowie Heilung/Tod/Respawn | [native-survival.log](native-survival.log) |
| Native Pilze/Graswachstum | Bestanden, 6 GPU-Aufnahmen, alle 4 Entwicklungsstufen beobachtet | [native-ecology.log](native-ecology.log) |
| Native Moebelmodell-Abbau | Beide abschliessenden Laeufe bestanden; Werkbank 4 / Truhe 10 beobachtete Bruchteilchen, vier Geometriestufen und Wiederherstellung | [Werkbank](native-workbench.log), [Truhe](native-chest.log) |

Neue Regressionstests decken unter anderem private Stationslager, reservierte
Transfers, persistente Ordinale/REG5-Altdaten, vier Blumen bei Graswachstum,
unveraenderte Pflanzenpositionen, versteckte Fortschrittsmarker, Cover/Replica,
Stage-Reload, deterministische Stammpilze, Meshed-/Headless-Paritaet,
Praezisions-Picking, Supportverlust und begrenzte Myzelgeometrie ab.
Vorhandene Langzeittests oder Tests aus Phase 49 werden hier nicht als neue
bestandene Phase-50-Abnahme ausgegeben.

Der ignorierte Test ist der bestehende dreiminuetige UDP-Soak; er wurde in
dieser Abnahme nicht erneut als Langzeittest ausgefuehrt. Clippy war nach der
letzten Typalias-Korrektur und der anschliessenden Auswahlkontur-Korrektur
erfolgreich. Der abschliessende volle Testlauf bestand mit `--test-threads=1`:
816 Tests erfolgreich, ein bestehender Langzeittest ignoriert. Der anschliessende
Build bestand in 29.67 Sekunden; die beiden abschliessenden Moebelproben mit
strengeren Partikelassertionen und Auswahlkontur-Korrektur bestanden ebenfalls.
Aus dieser lokalen Buildzeit wird keine Gameplay-Performance abgeleitet.

Ein vorheriger Wiederholungslauf scheiterte im vorhandenen Test
`authoritative_walk_and_prediction_keep_two_distant_streaming_regions_alive`
beim Serverstart: Ein temporaer per UDP ausgewaehlter Port 35707 war fuer den
zusaetzlichen TCP-Listener bereits belegt. Das war ein Bind-/Startfehler,
keine fehlgeschlagene Bewegungs- oder Replikationsassertion. Der danach
vollstaendige serielle Wiederholungslauf bestand. Diese Zwischenstoerung
wird nicht als neuer Gameplayfehler oder als bestandenes Zwischenergebnis
ausgegeben; massgeblich ist der verlinkte erfolgreiche Abschlusslauf.

## Native Ergebnisse

**Inventar:** Die echte OS-Mausprobe bestand mit neun GPU-Aufnahmen, unter
anderem [gehaltenem Inventar-Drag](inventory/inventory-held-drag-preview.png)
und [gehaltenem Crafting-Drag](inventory/crafting-held-drag-preview.png).
Slotmengen und Cursorrest sind bereits vor dem Loslassen sichtbar; autoritative
Transfers, kompaktes Layout, Hotbar-Tooltips und echtes Scrollen bis zur letzten
Slotreihe blieben funktionsfaehig. Es wurden keine kuenstlichen Bevy-Picking-
Ereignisse als Ersatz fuer OS-Zeigergesten injiziert.

**Crafting und Stationslager:** Der Integrated-Server-Lauf bestaetigt vier
Bretter als shapeless Werkbankrezept, vier geformte Werkzeugrezepte und die
gemeinsamen 2x2-/3x3-Reservierungen. Die [gefuellte Doppeltruhe](crafting/double-chest-inventory-filled.png)
zeigt 36 Slots vollstaendig; [Werkbank mit linker Doppeltruhe](crafting/workbench-linked-double-chest.png)
und [reservierter Transfer](crafting/workbench-linked-chest-reserved-transfer.png)
verwenden echte Containerinhalte. Binaeres REG5-Region-/Spieler-Save/Reload ist
Teil der erfolgreichen Probe, keine produktiven vorbefuellten Testkisten.

**Gesundheit:** [Kritische Gesundheit](survival/critical-health-disturbance.png)
zeigt den echten TiltUI-Stoerfilter am HUD. Die Survivalprobe pruefte
Nahrungszustand, Ausdauer, Tod, Respawn und HUD-Verschiebung/Speichern/Reload.
Die Ausdaueranzeige benoetigt kein rotes/gelbes Erschoepfungs-Warnbadge.

**Oekologie:** Der native Lauf zeigt einen [tatsaechlich generierten Stammpilz](ecology/ecology-trunk-fungus.png)
mit exakt passendem Picking und eine [Kappenunterseite mit haengenden Myzelbaendern](ecology/ecology-cap-fibres.png).
Danach wurde ein echter exponierter Grasblock ueber den normalen autoritativen
Edit-Pfad zu Dirt geaendert. [Dirt](ecology/ecology-bare-dirt.png),
[kleiner Spruessling](ecology/ecology-sprout-small.png),
[gewachsener Spruessling](ecology/ecology-sprout-grown.png) und
[fertiger Grasblock](ecology/ecology-grass-complete.png) wurden als echte
GPU-Readbacks gespeichert. Das Log bestaetigt alle vier Zwischenstadien mit
`seen=[true, true, true, true]` an `[-309, 23, 1622]`. Nach insgesamt
55.355 Sekunden seit Fixturestart war der finale Grasblock erreicht;
darin sind Pilzansichten, Streaming und die Dirt-Vorbereitung enthalten,
nicht nur die reine Wachstumszeit.

**Modellabbau:** Die beiden abschliessenden nativen Laeufe bestanden mit echter
Platzierung, Survival-Fortschritt, vier Modelldeformationsstufen und
Wiederherstellung des Originals nach Abbruch. Die urspruengliche komplette
Modell-Auswahlkontur wird waehrend des Abbaus ausgeblendet, damit sie bereits
herausgebrochene Teile nicht weiterhin als intaktes Gitter umzeichnet.
Die strengere Positivpruefung beobachtete vier Bruchteilchen an der Werkbank
und zehn an der Truhe und verlangte ausdruecklich `peak_particles > 0`.
Die [Werkbank-Bruchstufe](workbench/furniture-28-mining-3.png) und
[Truhen-Bruchstufe](chest/furniture-29-mining-3.png) verwenden die wirklichen
Modelloberflaechen ohne weiterhin intakt gezeichneten Auswahlkaefig.
Die Unitpruefung stellt zusaetzlich sicher, dass die natuerlichen Bruchteilchen
Wuerfelproportionen haben.

Die im uebergeordneten Arbeitslauf gesichteten PNGs umfassen gehaltenen Drag,
linkes Stationslager, gefuellte Doppeltruhe, kritische Gesundheit, Stammpilz,
Myzelbaender sowie Wachstums- und Endzustand. Das ist eine konkrete Funktions-
und Layoutsichtung, keine vollstaendige Referenz-Art-Abnahme oder 4K-/Mehr-GPU-Pruefung.

In den nativen Crafting-/Survivallogs stehen drei Bevy-Warnungen
`Duplicate render target for screenshot`: Zwei Screenshotauftraege trafen
dasselbe Fenster im selben Frame; Bevy uebersprang jeweils den doppelten Auftrag.
Die erwarteten Aufnahmen und Funktionspruefungen wurden trotzdem abgeschlossen.
Es wurden in diesen Laeufen keine UI-Runtimewarnungen beobachtet. Die genannten
Readback-Warnungen werden nicht als Gameplay- oder Layoutfehler verschwiegen.

## Native Reproduktion

Workspace- und Headless-Pruefungen:

```bash
cargo fmt --all --check
CARGO_BUILD_JOBS=2 cargo check --locked --workspace
CARGO_BUILD_JOBS=2 cargo test --locked --workspace -- --test-threads=1
CARGO_BUILD_JOBS=2 cargo clippy --locked --workspace --all-targets -- -D warnings
CARGO_BUILD_JOBS=2 cargo build --locked --workspace
cargo tree --locked -p terra-server
env -u DISPLAY -u WAYLAND_DISPLAY \
  TERRA_WORLD_DIR=/tmp/terra50-headless-fresh \
  target/debug/terra-server --smoke-test
```

Fuer zustandsbehaftete native Proben frische temporaere Pfade benutzen; die
Craftingprobe benoetigt einen noch nicht vorhandenen Weltordner. Echte
Mausgesten verlangen eine ungestoerte native Desktop-Sitzung und den vorhandenen
uinput-/KWin-Helfer `tools/ui/wayland_pointer.py`.

```bash
env -u DISPLAY TERRA_NATIVE_OS_POINTER=1 \
  TERRA_CONFIG_DIR=tools/benchmarks/results/daylight-2026-10-17/configs \
  TERRA_MENU_CONFIG_DIR=/tmp/terra50-menu-inventory-fresh \
  TERRA_CAPTURE_DIR=tools/benchmarks/results/ecology-inventory-2026-10-17/inventory \
  target/debug/terra-client --smoke-test-inventory

env -u DISPLAY TERRA_CRAFTING_SMOKE=1 TERRA_NATIVE_OS_POINTER=1 \
  TERRA_CONFIG_DIR=tools/benchmarks/results/daylight-2026-10-17/configs \
  TERRA_MENU_CONFIG_DIR=/tmp/terra50-menu-crafting-fresh \
  TERRA_CRAFTING_SMOKE_DIR=/tmp/terra50-world-crafting-fresh \
  TERRA_CAPTURE_DIR=tools/benchmarks/results/ecology-inventory-2026-10-17/crafting \
  target/debug/terra-client --smoke-test-inventory

env -u DISPLAY \
  TERRA_CONFIG_DIR=tools/benchmarks/results/daylight-2026-10-17/configs \
  TERRA_MENU_CONFIG_DIR=/tmp/terra50-menu-survival-fresh \
  TERRA_SURVIVAL_SMOKE_DIR=/tmp/terra50-world-survival-fresh \
  TERRA_CAPTURE_DIR=tools/benchmarks/results/ecology-inventory-2026-10-17/survival \
  target/debug/terra-client --smoke-test-survival

env -u DISPLAY TERRA_BUILD_CREATIVE=1 TERRA_FURNITURE_MINING_SMOKE=28 \
  TERRA_CONFIG_DIR=tools/benchmarks/results/daylight-2026-10-17/configs \
  TERRA_MENU_CONFIG_DIR=/tmp/terra50-menu-workbench-fresh \
  TERRA_PROFILE_FILE=/tmp/terra50-workbench-profile-fresh.toml \
  TERRA_CAPTURE_DIR=tools/benchmarks/results/ecology-inventory-2026-10-17/workbench \
  target/debug/terra-client --smoke-test-building
```

Fuer die entsprechende Truhenprobe `TERRA_FURNITURE_MINING_SMOKE=29` sowie
separate frische Profil-/Menuepfade und den Capture-Unterordner `chest` verwenden.

Das Oekologieprogramm ist eine ausdrueckliche Testfixture und startet kein
produktives Save. Es verwendet den echten Defaultgenerator, den normalen
TerraWorldPlugin und denselben autoritativen Wachstumstick wie Integrated/
Dedicated. Der Boden wird ueber den vorhandenen BlockEditRequest-Pfad von
Grass zu Dirt geaendert; es werden keine fertigen Spruesslinge oder Ersatz-
Renderdaten injiziert. Die vier Zwischenstadien muessen tatsaechlich beobachtet
werden, bevor der Endzustand als bestanden gelten kann. Die Zeit laeuft in
Echtzeit; es gibt keine beschleunigte Simulationsuhr.

```bash
env -u DISPLAY \
  TERRA_CONFIG_DIR=tools/benchmarks/results/daylight-2026-10-17/configs \
  TERRA_CAPTURE_DIR=tools/benchmarks/results/ecology-inventory-2026-10-17/ecology \
  target/debug/terra-client --smoke-test-ecology
```

Die Probe waehlt echte generierte Stammpilze mit uebereinstimmendem Picking,
zeigt danach eine Grosspilz-Unterseite und zuletzt Dirt, kleinen/grossen
Spruessling und den wiederhergestellten Grasblock. Screenshotdateien werden
erst durch echte GPU-Readbacks als aufgenommen gezaehlt. Ein gruener Logeintrag
allein ersetzt keine anschliessende Sichtung der Bilder.

## Grenzen

- Keine neue allgemeine FPS-/Speicherbaseline oder Langzeit-/Mehr-GPU-Abnahme.
- Moebel verwenden ihre aus denselben Bausteinen exportierten Teilvolumen.
  Das ist kein allgemeiner skinnbarer/animierter GLTF-Kollisionsimport.
- Die 24 Prozent sind ein Determinismusticket, keine Garantie fuer eine
  Platzierung in bereits verdecktem/verbautem Raum.
- Saisonaler Kalender, weltweites Wetter und weitere Wachstumspflanzen bleiben
  offen; der aktuelle Tick ersetzt diese kuenftigen Regeln nicht.
- Echte Ausruestungsitems und Ofen-Backend bleiben im TODO; vorhandene
  Ausruestungs-Silhouetten sind weiterhin deaktivierte Positionen.
