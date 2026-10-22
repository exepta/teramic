# Phase 67: Baeren, Fackelbelichtung Und Ego-Items

## Ursache Der Ueberbelichtung

Die laufende Tageslichtaktualisierung schrieb in `Update` eine vorlaeufige
Aussenbelichtung. Die Fackelkompensation las diesen Wert, bevor die
Hoehlenanpassung in `PostUpdate` die endgueltige Belichtung setzte. Mittags konnte
dadurch eine fuer Aussenlicht verstaerkte Fackel in der viel empfindlicheren
Hoehlenbelichtung landen. Die bisherigen pausierten Szenen prueften diesen
Scheduling-Fall nicht.

Gehaltene und gesetzte lokale Lichter werden jetzt nach der Hoehlenbelichtung
und vor der Transform-Propagation aktualisiert. Lichtkurve, Reichweite und
Lichtpoolbudgets bleiben erhalten. Der neue Regressionstest verwendet den echten
Tageslichtplan; die native Probe vergleicht dieselbe geschlossene Fackelkammer
mit pausierter und laufender Mittagsuhr.

## Baer

Die Art erweitert das vorhandene autoritative Mob-, Habitat-, Beduerfnis-,
Navigations- und Speichersystem. Keine zweite KI oder globales Navmesh.
`assets/terra/mobs/bear.json` definiert 24 Leben, 45 Hunger, 20 Durst sowie
Angriffsradius, Schaden, Reichweite und Cooldown. Ein Teil neu untersuchter
feuchter Uferhabitate erzeugt einzelne Baeren; vorhandene Habitatidentitaeten
und Todesmarkierungen bleiben bestehen.

Hunger bis 15 loest die Suche nach echten lebenden Fischen aus. Fangen und
Spielerschaden werden serverseitig mit aktueller Reichweite, Sichtpruefung und
Zielzustand bestaetigt. Ein gefressener Fisch kann nicht doppelt verbucht werden.
Creative-Spieler und Respawn-Schutz bleiben ausgenommen. Baumklettern verwendet
vorhandene tragende Stammzellen, Koerperfreiheit und geladene Nachbarn; Abbau
des Stamms loest Abstieg aus. Die Hoehe ist auf sechs Meter begrenzt.

Die gemeinsame lokale Wegsuche bleibt auf acht Suchauftraege pro Servertick
und 96 Knoten pro Suchlauf begrenzt. Ein Hindernisdetour fordert nach wieder
freiem Weg keine unnoetigen Wiederholungen an. Jagd/Angriff/Klettern reservieren
keine ungenutzten Beduerfnissuchen. Fischkandidaten werden einmal pro Tick
raeumlich indiziert, nicht fuer jeden Baeren durch die ganze Population gesucht.

Gemessene Modellgeometrie: **12 024 Vertices, 6 012 Dreiecke, acht Teilmeshes**.
Die Fell-, Schnauzen- und Krallendetails sind in diesen geteilten Meshes enthalten;
keine Detailentities. Gehen, Jagen, Trinken, Schlafen, Klettern und Angriff
verwenden eigene Posen im bestehenden Animationssystem. Teilmeshes sind keine
Messung der tatsaechlichen GPU-Drawanzahl.

## First Person

Hand und Unterarm aus Phase 66 werden in First Person nicht mehr erzeugt.
Gecachte Modellbounds und die echte Kameraprojektion verankern das untere
Itemende ausserhalb des Bildrands. Werkzeughaltung ist um eine Vierteldrehung
gedreht. Weite FOVs begrenzen die Skalierung vor der Near-Plane; die
Vorwaertsschlagbewegung bleibt erhalten. F5 und Weltavatare behalten ihre Arme
und Griffpunkte. Keine pro Frame wiederholte Mesh-Vertexanalyse im Gameplay.

## Native Modellpruefung

Vulkan auf Radeon RX 6950 XT/RADV, Mesa 26.2.4. Die tatsaechlichen Aufnahmen
haben 1920 x 1080 Pixel. `--smoke-test-creatures` mit `TERRA_BEAR_INSPECT=1`
besteht fuenf Modellansichten: Ruhe, Jagd, Angriff, Schlafen und Klettern.
Die Bilder pruefen die Darstellung; echte Bewegung, Fang und Schaden werden
getrennt durch Welt-/Netzwerktests geprueft.

- [Baer in Ruhe](bear/creatures-0.png)
- [Tatzenangriff](bear/creatures-2.png)
- [Schlafpose](bear/creatures-3.png)
- [Kletterpose](bear/creatures-4.png)

`--smoke-test-hands` besteht alle sieben Ansichten fuer Block, Spitzhacke,
Schwert, Schlagphase, F5, Fackel und leere Hand. Geometriegrenzen werden in
Bildkoordinaten projiziert: First-Person-Items schneiden den unteren Rand und
bleiben sichtbar. Der Test verwendet vorhandene AABBs, da renderexklusive
Meshes ihre CPU-Vertices nach dem Upload freigeben.

- [Gedrehte Spitzhacke ohne Ego-Arm](held-items/hands-1.png)
- [Fackel am unteren Bildrand](held-items/hands-5.png)
- [Leere Hand ohne Arm](held-items/hands-6.png)

Die native Schlagaufnahme allein belegt keine komplette zeitliche
Animationsabnahme; der Vorwaertshub wird zusaetzlich geometrisch getestet.

## Automatisierte Validierung

Erfolgreich ausgefuehrt:

```text
cargo fmt --all --check
cargo check --workspace
cargo test --workspace --no-fail-fast
cargo clippy --workspace --all-targets -- -D warnings
cargo build --workspace
```

**1024 Tests bestanden, null fehlgeschlagen, ein bestehender 180-s-UDP-Soak
ignoriert.** Nach der letzten reinen Korrektur am nativen Testaufbau wurden
die 29 Clienttests erneut bestanden; Check, Clippy und Build wurden ebenfalls
erneut ausgefuehrt. Der Dedicated Server startet ohne `DISPLAY` und
`WAYLAND_DISPLAY`, bindet einen echten ephemeren UDP-Port, erzeugt 27 Sections
und beendet seinen Smoke-Test erfolgreich.

Die neuen Tests decken Hunger/Fang, doppelte Fangversuche, echte geladene Ufer,
deterministische Spawn-IDs und Todesmarkierungen, Trinken/Schlaf, tatsaechlichen
Auf-/Abstieg, Stamm-/Bodenabbau, breite Koerperkollision, gecachte Umwege und das
gemeinsame Suchbudget ab. Der Umwegtest prueft das Erreichen des Ziels bei
begrenzten Suchen und keine weiteren Suchanforderungen nach Ankunft. Ein echter
UDP-Test prueft Survival-Schaden, Tod genau einmal und Creative-Schutz.
Regionstests pruefen gespeicherten Kletter-/Hungerzustand im bestehenden Layout.
Der Lichttest prueft die echte Tageslicht-Reihenfolge bei unterschiedlichen
Helligkeiten, Uhrzeiten und Hoehlenuebergaengen. Ego-Tests pruefen FOVs,
Seitenverhaeltnisse, Kamerarotation, Near-Plane und Vorwaertshub.

## Native Lichtpruefung

Alle **16** Ansichten der finalen `--smoke-test-caves`-Probe mit
`TERRA_TORCH_INSPECT=1` bestehen auf dem echten Bevy-PBR-Pfad. Die geschlossene
Kammer verwendet Medium, Helligkeit 1.0, 90 Grad FOV und 1920 x 1200 Pixel.
Der Test haelt ueber die vorhandenen `ChunkFoci` sowohl seine von Hand erstellten Sections
als auch den wechselnden Kamerabereich geladen. Der zusaetzliche Fackelanker
invalidiert den Lichtcache ueber das normale Journal.

| Dieselbe Kammer | Uhr pausiert, Stage 14 | Uhr laeuft, Stage 15 |
| --- | ---: | ---: |
| Kamera EV100 | 6.0 | 6.0 |
| Fackellichtfluss, Flackerphase | 7252.51 lm | 6854.55 lm |
| Mittlere linearisierte Bildluminanz | 0.010282 | 0.009911 |
| p95 Bildluminanz | 0.017191 | 0.016379 |
| Pixel mit Luminanz > 0.70 | 0.0608 % | 0.0521 % |
| Nahezu schwarze Pixel | 0.3038 % | 0.3082 % |

Die Messung verwendet die inneren 80 Prozent des tonemapped Bildes und enthaelt
Material, UI und Flammenphase. Sie misst keine reine physikalische
Bestrahlungsstaerke. Kleine Unterschiede entsprechen den verschiedenen
Flackerphasen; die falsche Aussenlichtverstaerkung tritt nicht mehr auf.

Die finale Bildkontrolle zeigt vollstaendige Steinwaende, Boden und Decke,
erkennbare Texturen und einen lokal begrenzten Flammenkopf. Eine zusaetzliche
Schwarzanteilpruefung verhindert, dass ausgeladene Raumgeometrie als gueltiger
dunkler Vergleich durchgeht. Erste ungueltige Aufnahmen mit fehlender Geometrie
wurden verworfen und sind nicht Grundlage dieser Tabelle.

- [Kammer mit pausierter Uhr](caves/cave-14-Medium.png)
- [Kammer mit laufender Uhr](caves/cave-15-Medium.png)
- [Wandfackel](caves/cave-6-Medium.png)
- [Nachtboden](caves/cave-8-Medium.png)

Die kompletten finalen Bilder liegen unter `caves/`; die zugehoerigen Logs
`terra67-caves.log`, `terra67-bear.log`, `terra67-hands.log`,
`terra67-tests-final.log` und die Check-/Build-/Clippy-/Serverlogs liegen neben
diesem Bericht. Fruehe Proben liefen teilweise neben Kompilierungen; ihre
Frame-Samples sind keine isolierte Performance-Messung.

## Grenzen

Keine neuen eigenen Baerenlaut-Aufnahmen, Baerenprodukte oder Zaehmung.
Bereits untersuchte Gebiete werden nicht nachtraeglich neu bevoelkert. Die lokale
Navigation ist kein globaler Wegfinder; tiefe Wasser-/komplexe Baumlandschaften
benoetigen weiterhin laengere Spieltests. Keine pixelidentische Rekonstruktion
des Referenzbildes oder vollstaendige Animationsabnahme behauptet.

Netzwerkprotokoll **39** erfordert aktualisierte Clients und Server. Regionen,
Spielerformate, bestehende IDs, Seeds und Generatorversion bleiben unveraendert.
Neue Enumwerte sind angehaengt. Keine neuen Dependencies oder Renderabhaengigkeit
im Dedicated Server.

Es wurde kein isoliertes Vorher-/Nachher-FPS-Paar fuer diese Aenderung gemessen.
Auf dem Testrechner liefen fremde Anwendungen einschliesslich eines weiteren
Spiels; native Bildproben eignen sich hier nicht als Performance-Paritaetsbeleg.
