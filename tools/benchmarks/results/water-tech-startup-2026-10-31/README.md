# Phase 166: UI-Vorladen Und Wassertechnik

## Stand Und Grenzen

F27/F28 sind in die vorhandenen Block-, Kinetik-, Fluid-, Inventar-, Save- und
Netzwerksysteme integriert. Block-IDs 163..169 werden angehaengt; keine
Dependency-Upgrades, kein Vendor-Fork und keine zweite Terrain-Wassersimulation.
RG12 liest die bestehenden Regionen weiter; Client und Server verwenden
gemeinsam Protokoll 93. Bedienung steht im [Blockhandbuch](../../../../assets/terra/blocks/README.md#water-pipes-faucets-and-reservoirs).

- Holz-/Stein-/Eisenrohre passen ihre Arme an echte Nachbarn an.
- Ein Ventil sperrt die Versorgung; ein versorgter offener Hahn gibt echtes Wasser aus.
- Die Pumpe braucht reale Kinetik und kehrt mit negativer Drehzahl ihre Richtung um.
- Rechteckige Becken: 1x1 bis 5x5, auch 1x2, bis zehn Bloecke hoch; maximal 2000 Einheiten.
- Quellenaufnahme und Gefaessfuellung pruefen Sichtlinie, Rechte, Revision und Kapazitaet.
- Die Giessekanne zeigt Wasserstand, Kippbewegung und Giesstrahl; bepflanzte Beete erhalten echte Feuchtigkeit.
- Die Kannenreserve bleibt wie bisher sitzungslokal, nicht je einzelner Kanne gespeichert. Unbepflanzter Boden hat weiterhin keinen separat gespeicherten Feuchtigkeitszustand.
- Netzwerke sind auf 512 verbundene und 8192 gespeicherte Knoten begrenzt. Ungeladene Verbindungen pausieren statt Wasser zu erfinden.
- Referenzorientierte Modelle, keine behauptete pixelgenaue 1:1-Abnahme. Keine neuen FPS-Messungen oder Langzeitabnahme.

## Finale Automatisierte Pruefung

Alle Befehle wurden seriell mit demselben Workspace-Featuregraph ausgefuehrt.

| Befehl | Ergebnis | Rohlog |
| --- | --- | --- |
| `cargo fmt --all --check` | bestanden | [Format](fmt-final-confirmed.log) |
| `cargo check --workspace -j1` | bestanden | [Check](check-final-confirmed.log) |
| `RUST_TEST_THREADS=4 cargo test --workspace -j1` | 3511 bestanden, 0 Fehler, 8 ignoriert | [Tests](tests-final-confirmed.log) |
| `cargo clippy --workspace --all-targets -j1 -- -D warnings` | bestanden | [Clippy](clippy-final-confirmed.log) |
| `cargo build --workspace -j1` | bestanden | [Build](build-final-confirmed.log) |
| `cargo test --workspace --lib --bins -j1 plumbing::tests -- --nocapture` | alle 14 Welt-Hydrauliktests bestanden | [Gezielte Regression](tests-basin-confirmed.log) |

Die Regressionen pruefen unter anderem deterministische negative Grenzpositionen,
regionenuebergreifendes Speichern, echte kinetische Pumpen, Richtungswechsel,
Ventilsperren, endliche Transfers, volle/unvollstaendige Becken, Unload/Reload,
saemtliche Valve-Achsen, gueltige GLTF-Geometrie und gemeinsame Mesh-Lebensdauer.
Der untere Beckenzulauf muss beide Ebenen fuellen, ohne Wasser zu verlieren.
Ein positiver Druckgradient verhindert Gleichdruck-Ruecklauf in die eigene Quelle.
Verspaetete Eimeraktionen bleiben trotz echter Wasser-Meshupdates gueltig;
gewechselte Objektposen und entfernte Anschluesse werden weiter abgelehnt.

## Native Kurztests

AMD Radeon RX 6950 XT, RADV NAVI21, Mesa 26.2.5-arch3.1, Vulkan. Getrennte
kurze Pruefungen nach dem finalen Build; kein gleichzeitig laufender Cargo-Build.
Die Wassertechnik nutzt eine eigene temporaere Welt, nicht den Spielstand.

| Lauf | Ergebnis | Nachweis |
| --- | --- | --- |
| Wassertechnik | 19.51 Sekunden, 9 nichtleere Aufnahmen, echte Rechtsklick-Aktionen; endliches Becken vorher gespeichert/geladen, Gefaesse gefuellt, Ventil gesperrt, Beet auf 100 Prozent Feuchtigkeit | [Rohlog](native-hydraulics-final-clean.log) |
| Vollstaendiges Start-Gate | 454 Inventar-Icons, 1611 UI-Symbole; insgesamt 1624 UI-Vorbereitungspositionen inklusive 9 Bildern, 3 Schriften und Dokument; keine Welt, kein Server, keine offenen Vorschaujobs | [Rohlog](native-startup-final-clean.log) |
| Dedicated Server | headless bestanden, 45 Sections | [Rohlog](native-headless-final-confirmed.log) |

Die beiden grafischen Logs enthalten keine Warnung, Panic, Shader- oder
Rodio-Backendfehler. Beckenwasser, sichtbare Flaschen-/Eimerfuellung, der
Giesstrahl und das fertige Hauptmenue ohne Ladehinweis wurden an den echten
Aufnahmen kontrolliert. Das ist keine menschliche Audio- oder Langzeitabnahme.
Die Hydraulikfixture laesst den Vollkatalog bewusst aus; dessen Vorladen wird
im getrennten Hauptmenue-Test geprueft. Fruehere fehlgeschlagene Zwischenlaeufe
bleiben im Ordner, sind aber keine finale Abnahme.

## Aufnahmen

- [Fertiges Hauptmenue nach dem Vorladen](startup-final-clean/main-menu.png)
- [Gespeichertes mehrstoeckiges Becken](hydraulics-final-clean/hydraulics-network.png)
- [Flasche waehrend der Fuellung](hydraulics-final-clean/bottle-filling.png)
- [Gefuellte Flasche und angeschlossener Hahn](hydraulics-final-clean/bottle-filled.png)
- [Eimer waehrend der Fuellung](hydraulics-final-clean/bucket-filling.png)
- [Sichtbarer Wasserstand im Eimer](hydraulics-final-clean/bucket-filled.png)
- [Giessekanne waehrend der Fuellung](hydraulics-final-clean/watering-can-filling.png)
- [Volle Giessekanne](hydraulics-final-clean/watering-can-filled.png)
- [Geschlossenes Ventil verweigert weitere Fuellung](hydraulics-final-clean/valve-closed-no-water.png)
- [Giesstrahl, gekippte Kanne und echtes nasses Beet](hydraulics-final-clean/watering-can-pouring.png)

## Reproduktion

```bash
TERRA_CONFIG_DIR=/tmp/teramic165-perf-config \
TERRA_PREVIEW_WARMUP=1 TERRA_STARTUP_ONLY=1 \
TERRA_MENU_SCREENSHOTS="$PWD/tools/benchmarks/results/water-tech-startup-2026-10-31/startup-repeat" \
TERRA_COMPUTE_THREADS=16 \
target/debug/terra-client --smoke-test-menu --gender=f
```

```bash
TERRA_CONFIG_DIR=/tmp/teramic165-perf-config \
TERRA_CAPTURE_DIR="$PWD/tools/benchmarks/results/water-tech-startup-2026-10-31/hydraulics-repeat" \
TERRA_COMPUTE_THREADS=16 \
target/debug/terra-client --smoke-test-hydraulics --gender=f
```

```bash
TERRA_CONFIG_DIR=/tmp/teramic165-perf-config \
target/debug/terra-server --smoke-test --no-save
```
