# Wandfackeln, Rohrboegen und Wasseroberflaechen

Pruefstand vom 2026-11-02. [Ergebnisse](verification.json),
[Quellhashes](source-sha256.json) und vollstaendige [Logs](logs/) liegen bei.

| Pruefung | Ergebnis |
| --- | --- |
| Workspace-Build | Bestanden |
| Gesamtsuite, zwoelf Programme | 3686 bestanden, 0 Fehler, 8 ignoriert |
| Clippy, alle Targets, `-D warnings` | Bestanden |
| Formatierung und Diff | Bestanden |
| Nativer Hydrauliklauf | Bestanden, 74,95 s, 35 Pflichtaufnahmen |
| Nativer Naturwasserlauf | Bestanden, 33 Pflichtaufnahmen |

Vier Wandfackeln werden per normalem Rechtsklick samt Serverbestaetigung und
Verbrauch genau eines Items gesetzt. Ein entferntes grosses Zahnrad aktiviert
die echte erweiterte Kollisionspruefung. Die Materialgalerie zeigt Holz, Stein,
Eisen und Glas; das Quellbecken enthaelt diese vier Rohrarten, Ventil und Pumpe.

Die Oasenreihe bewahrt sechs natuerliche Quellen mit jeweils vier Einheiten bei
Wasserhoehe 26,5. Die generierte Geometrie verlangt aktive Fernwellen und feste
Kontakte: 928 bewegte und 72 gepinnte Vertices. Die Nahansichten verwenden
gegenueberliegende Kameras und fortschreitende Zeit. Wellen werden nicht
pauschal abgeschaltet.

- [Wandfackel](hydraulics/wall-torch-facing-7.png)
- [Holzbogen](hydraulics/pipe-bend-wood.png), [Steinbogen](hydraulics/pipe-bend-stone.png), [Eisenbogen](hydraulics/pipe-bend-iron.png)
- [Glasbogen von vorne](hydraulics/pipe-bend-glass.png) und [hinten](hydraulics/pipe-bend-glass-reverse.png)
- [Quellbecken mit allen sechs Teilen](hydraulics/waterlogged-round-parts-oblique.png)
- [Naturwasser vorne](natural-water/natural-water-contact-front.png) und [hinten](natural-water/natural-water-contact-back.png)
- [Nutzerreferenz](reference.png)

Verwendet wurden `cargo build --workspace --locked -j1`,
`RUST_TEST_THREADS=4 cargo test --workspace --lib --bins --no-fail-fast -j1`
und `cargo clippy --workspace --all-targets -j1 -- -D warnings`.
Die nativen Aufrufe waren `--smoke-test-hydraulics --gender=f` sowie
`TERRA_WATER_CONTACT_SMOKE=1 --smoke-test-daylight --gender=f`, jeweils mit
16 Compute-Threads. Kopien der Welt-, Grafik- und Eingabekonfigurationen liegen
unter [config](config/). Die bestehenden Zeitlimits bleiben unveraendert.

Nach der Gesamtsuite wurden ausschliesslich native Testhelfer fuer Proben und
Uploadbereitschaft sowie gleichwertige Clippy-Schreibweisen angepasst. Diese
Helfer wurden erneut gebaut, gelintet und im erfolgreichen nativen Lauf
ausgefuehrt. Produktionsgeometrie und Physik blieben dabei unveraendert.

Der Hydrauliklauf protokollierte beim Szenenwechsel eine Bevy-Warnung zu einem
RenderLayers-Befehl fuer eine bereits entfernte Entity; alle Assertions und
Aufnahmen wurden erfolgreich abgeschlossen. Der Naturwasserlauf blieb ohne
Warnungen und Fehler. Fruehe Testaufbau-Diagnosen sind separat in `logs/`
archiviert; massgeblich sind `hydraulics.log` und `natural-water.log`.
