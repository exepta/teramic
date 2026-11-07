# Rohrdurchfuehrungen, Infuser-Strahl und Teilblockwasser

Pruefstand vom 2026-11-02. [Ergebnisse](verification.json),
[Quellhashes](source-sha256.json), [Konfiguration](config/) und [Logs](logs/).

| Pruefung | Ergebnis |
| --- | --- |
| Workspace-Build und Clippy, alle Targets, `-D warnings` | Bestanden |
| Gesamtsuite, zwoelf Programme | 3712 bestanden, 0 Fehler, 8 ignoriert |
| Formatierung und Diffpruefung | Bestanden |
| Nativer Hydrauliklauf | Bestanden, 35 Aufnahmen, 76,04 s |
| Neue Partial-Water-Szene | Bestanden, 5 Aufnahmen, 11,66 s |
| Bisherige Wasserfallregression | Bestanden, 2 Aufnahmen |

Die neue Szene prueft vier endliche Wasserzellen mit jeweils vier Einheiten
hinter einer halbhohen Slabwand. Alle 16 Einheiten bleiben erhalten, die
Aussenseiten trocken. Echte Quellen versorgen eine Treppen-Innenecke und
verbundene Zaunzwischenraeume ohne neue Sourceflags; der Pfosten selbst bleibt
fest. Ein separates voll gefuelltes Becken bewahrt sieben echte Teilblockposen
und ihre Quellen. Eine fallende Wasserfahne trifft Glasrohr und Ventil. Vier
Wand-/Dachtuerme zeigen alle normalen Rohrmaterialien mit runden Durchfuehrungen.

Slabs dichten bis zur festen Oberkante. Eine volle Eimerquelle oberhalb einer
halbhohen Kante kann weiterhin darueberlaufen. Dieser Unterschied ist gezielt
getestet, ebenso Sourceflags, Mengenbilanz, Abschnittsgrenzen und Save/Reload.

- [Infuser beim echten Fuellen](hydraulics/infuser-filling.png): durchgehender Strahl, 794 bestaetigte Vertices.
- [Slab-, Treppen- und Zaunbecken](partial-water/partial-basins-close.png)
- [Untergetauchte Teilbloecke](partial-water/waterlogged-partial-pool.png)
- [Glasrohr und Ventil in fallendem Wasser](partial-water/waterlogged-pipe-valve-fall.png)
- [Alle Rohrdurchfuehrungen](partial-water/all-pipe-wall-roof-surrounds.png)
- [Bisherige Wasserfallregression](waterfall-regression/waterfall-partial-step-close.png)
- [Nutzerreferenzen](references/)

Verwendet wurden `cargo build --workspace --locked -j1`,
`RUST_TEST_THREADS=4 cargo test --workspace --lib --bins --no-fail-fast -j1`,
`cargo clippy --workspace --all-targets -j1 -- -D warnings` und
`cargo fmt --all --check`.
Native Aufrufe: `--smoke-test-hydraulics --gender=f`,
`TERRA_PARTIAL_WATER_SMOKE=1 --smoke-test-waterfalls --gender=f` sowie
`--smoke-test-waterfalls --gender=f`, jeweils mit 16 Compute-Threads.
Zeitlimits und vorhandene Gameplayassertions bleiben erhalten.

Die Bildabnahme erkannte einen zunaechst noch zu schwachen Strahl auf hellem
Stein; Radius und aerierte Lichtstreifen wurden anschliessend verstaerkt und
erneut nativ geprueft. `analysis/jet-initial.png` ist die unveraenderte fruehe
Aufnahme; `analysis/jet-initial-detail.png` ist ausschliesslich ein vergroesserter
Diagnoseausschnitt. Die endgueltigen Originalaufnahmen stehen in `hydraulics/`.

Der Hydrauliklauf protokollierte beim Szenenwechsel eine bereits bekannte
Bevy-Warnung zu einem RenderLayers-Befehl fuer eine entfernte Entity. Alle
Assertions und Aufnahmen bestanden; die beiden Wasserfalllaeufe blieben ohne
Fehler. Produktionsdaten und Speicherformate wurden nicht migriert.
