# Phase 105: F02 Equipment And Armor

## Implementierung

25 Ruestungsteile: Leather, Iron, Gold, Diamond und Ruby, jeweils Helm,
Brustpanzer, Handschuhe, Hose und Stiefel. Vier Accessoires aktivieren Ohrringe,
Kette, Guertel und Socken. Die bisherigen acht Platzindizes bleiben erhalten;
Handschuhe verwenden den neunten Platz. Eigene Item-JSONs und geformte Rezepte
liegen in den bestehenden Verzeichnissen, ohne zweite Registry.

Die belegten Plaetze werden in PlayerStats gespeichert, nicht an bestehende
Truhen-/Ofen-Inventare angehaengt. Jeder Platz akzeptiert nur die passende Art;
Ausruestung ist in allen Inventaren und am Cursor auf einen Gegenstand begrenzt.
Klick/Tausch/Rechtsklick und Shift-Transfer verwenden atomare, revisionierte
Servertransaktionen. Ein voller Rucksack oder ein volles Spielerstatistikbudget
verliert keine Gegenstaende. Der bisherige Keep-Inventory-Todesmodus bleibt.

Definitionen, Werte und Schadensregeln:
[ARMOR.md](../../../../assets/terra/items/ARMOR.md).
Schutz vor Tierangriffen, Blitz und fallenden Mineralien: 2% je Ruestungspunkt,
maximal 80%. Vollstaendige Sets besitzen 10/20/16/30/35 Punkte.
HP-/Stamina-Boni werden aus der getragenen Ausruestung berechnet; ein Wechsel
heilt nicht. Beim Ablegen werden verbrauchte Werte nur an kleinere Maxima
begrenzt. Resistenzen und Shield Power behalten ihre eigenen Regeln.

Protokoll 57 prueft zusaetzlich den Equipment-Katalog. Client und Server
gemeinsam aktualisieren. Neue Spielerdateien verwenden PLY6 mit explizitem
PLY5-Leser; aeltere Formate bleiben lesbar. Regionformat, Generator und Block-IDs
werden nicht veraendert.

## Darstellung

Getragene Teile folgen den vorhandenen animierten Kopf-/Torso-/Arm-/Hand-/Bein-
und Fussgelenken. Geschichtete Platten, Lederbaender, Schnallen, Stoffschuerzen
und facettierte Edelsteine sind echte Geometrie. Die Referenz wird innerhalb
des vorhandenen prozeduralen Avatarstils interpretiert, keine Pixelkopie behauptet.

Ein begrenzter Cache haelt 120 Meshes fuer beide Koerperformen und sechs
Materialien. Ein volles Set benoetigt elf gebuendelte Mesh-Anhaenge, keine Entity
je Platte oder Kristall. Ausruestungswechsel erstellen keine neuen Mesh-Assets.
Itemansicht, TMI, Drops und gehaltene Items verwenden dieselben Bauteile;
Handschuh/Stiefel zeigen im Icon ein einzelnes repraesentatives Teil.
Das Portraet wird nur nach Aenderungen erneuert und verwendet seine zwei
Renderziele erneut; laufende Readbacks werden zuvor abgeschlossen.

Die silberne Ruestungsleiste liegt ueber HP. Sie misst den tatsaechlichen
Schutzwert, keine Haltbarkeit. Shield Power bleibt davon getrennt als schmale
zusaetzliche Anzeige. HP-/Stamina-Fuellung und kritische Schwellen verwenden
die abgeleiteten Maximalwerte.

Auf Herz und HP-Kanal liegt nur der erste Effekt der sichtbaren Kartenreihenfolge.
Auch die Regenerationsherzen erscheinen nur, wenn Regeneration dort fuehrt.
Alle weiteren Effekte wirken unveraendert und bleiben als Cards/Ueberlauf sichtbar.

## Validierung

Erfolgreich mit einem zentralen Cargo-Prozess und zuletzt `CARGO_BUILD_JOBS=1`:

- `cargo fmt --all --check`
- `cargo check --locked --workspace`
- `cargo test --locked --workspace --no-fail-fast`: 1816 bestanden,
  2 bestehende ignoriert. 38 neue Tests gegenueber Phase 104.
- `cargo clippy --locked --workspace --all-targets -- -D warnings`
- `cargo build --locked --workspace`
- Headless-Server ohne DISPLAY/WAYLAND_DISPLAY: echter UDP-Socket,
  27 Sections und sauberer Exit.

Logs: [Fmt](logs/phase105-fmt.log), [Check](logs/phase105-check.log),
[Tests](logs/phase105-tests.log), [Clippy](logs/phase105-clippy.log),
[Build](logs/phase105-build.log), [Headless](logs/phase105-headless.log).
Die zusaetzlichen Tests umfassen Stack-Limits in allen Inventaren, slotgenaue
Transfers und volle Rucksaecke, Revision/Replays, Reconnect/Respawn, echte
UDP-Replikation, Katalog-Mismatch und ausgeschoepfte Spielerstatistikbudgets.
PLY5-Migration, Schutz-/Bonusregeln, valide Geometrie beider Koerperformen,
auswaerts gerichtete Kristallflaechen, Iconprojektion, wiederholte Wechsel und
Bereinigung, Portraet-Renderziel-Wiederverwendung sowie erstes Effekt-Overlay
und relative HP-/Stamina-Fuellung werden ebenfalls geprueft.

### Native GPU-Pruefung

RX 6950 XT/RADV/Vulkan, realer integrierter Survival-Server und UDP,
isolierte temporaere Welt/Profilkonfiguration. 25 echte, gespeicherte Items
werden durch autoritative Inventartransaktionen an-/abgelegt, keine lokal
vorgetaeuschten Ausruestungssnapshots. Die Szene ist flach und fuer die
Betrachtung mit einem zusaetzlichen schattenlosen 5000-Lux-Fuelllicht versehen.
Das ist kein FPS-Vergleich der unveraenderten normalen Spielwelt.

Alle 14 Stufen bestanden im abschliessenden rund 50 Sekunden langen Lauf:

| Stufen | Schutz | Bilder |
|---|---:|---|
| Leather, Welt und Inventar | 10 | [Welt](native/armor-leather-world.png), [Inventar](native/armor-leather-inventory.png) |
| Iron, Welt und Inventar | 20 | [Welt](native/armor-iron-world.png), [Inventar](native/armor-iron-inventory.png) |
| Gold, Welt und Inventar | 16 | [Welt](native/armor-gold-world.png), [Inventar](native/armor-gold-inventory.png) |
| Diamond, Welt und Inventar | 30 | [Welt](native/armor-diamond-world.png), [Inventar](native/armor-diamond-inventory.png) |
| Ruby, Welt und Inventar | 35 | [Welt](native/armor-ruby-world.png), [Inventar](native/armor-ruby-inventory.png) |
| Vollstaendig abgelegt, Welt und Inventar | 0 | [Welt](native/armor-clear-world.png), [Inventar](native/armor-clear-inventory.png) |
| Ruby kompakt, Welt und Inventar | 35 | [Welt](native/armor-ruby-compact-world.png), [Inventar](native/armor-ruby-compact-inventory.png) |

Desktop: logisch 1280x800, physisch 1920x1200. Kompakt: logisch 360x800,
physisch 540x1200. Die Monitor-Skalierung betraegt 1.5. Der Test prueft die
wirklich materialisierten neun Buttons, passenden Itembilder, reale Assets und
Bildrechtecke innerhalb der Slots. Armour-Caches bleiben 120 Meshes/sechs
Materialien; lokale volle Sets haben elf Anhaenge, nach Ablegen null.
Alle 25 Gegenstaende bleiben erhalten. Gesamte Mesh-Assets: erster Snapshot
2900, anschliessend 2895, auch nach Wechseln und Ablegen. Das ist ein kurzer
Lifecycle-Nachweis, keine allgemeine Speicher-/VRAM-Paritaetsmessung.

Poison, Freeze, Regeneration III und Shield Power sind gleichzeitig aktiv.
Erstes Kartensymbol und tatsaechliche Materialflags muessen uebereinstimmen:
Poison auf Herz/HP, kein zusaetzliches Frost-Overlay und keine aufsteigenden
Regenerationsherzen. Die anderen Effekte bleiben funktional aktiv.
Auch Portraet und alle Item-Readbacks muessen vor dem sauberen Exit fertig sein.
Kein ERROR/Panic/Shaderfehler im abschliessenden
[nativen Log](logs/phase105-native-equipment.log).

Die Sichtpruefung hat zuvor fehlende verschachtelte Slot-Bindings, fehlende
komponenteneigene Bildgroessen und einen zu kleinen/verkehrt orientierten
Ohrring-Preview gefunden. Alle sind korrigiert; neue Serialisierungs-,
Projektions- und native Rechteckpruefungen sichern sie ab. Der Rechtecktest
beruecksichtigt TiltUIs zusaetzliche Hierarchie-Knoten statt eines direkten
Bild-/Button-Elternverhaeltnisses.

### Grenzen

Keine Pixelkopie der Referenz; die bestehenden Charakterproportionen und
Animationen bleiben. In der Welt kann Bodenbewuchs Teile der Stiefel verdecken;
das Inventarportraet zeigt die vollstaendige Ausruestung. Schmuck hat aktuelle
einfache prozedurale Modelle, keine neue Charakteranpassungsoberflaeche.
Die Leiste misst Schutzpunkte, nicht Materialhaltbarkeit; es gibt noch keinen
Ruestungsverschleiss. Keine automatische neue Vergiftungs-/Kaeltequelle.
Keine neuen 10-/15-/30-Minuten-Laeufe und kein isolierter FPS-/VRAM-Vergleich;
weitere Langzeit-/Mehrspieler-GPU-Abnahmen bleiben im technischen Backlog.
