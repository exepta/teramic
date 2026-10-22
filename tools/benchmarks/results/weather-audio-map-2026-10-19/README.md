# Phase 76: Mobdichte, Karten, Wetter Und Audio

Generator 5, Seeds, Block-/Welt-IDs, Protokoll 42 / IDX6, REG9 / PLY3 und alle
Dependency-Versionen bleiben unveraendert. Keine zusaetzliche Server-GPU-Abhaengigkeit.

## Spawn-Regel

Der gemeinsame autoritative Habitat-Scheduler laesst 3 von 5 unabhaengig gehashten
neuen Habitatkandidaten zu. Ganze Herden, Arten, stabile Tier-/Bienen-IDs und
bereits gespeicherte Tiere bleiben erhalten. Die Probe kostet keinen weiteren
Noise-/Generierungsjob. Die konfigurierte 256er-Defaultobergrenze je 150-m-Umgebung
bleibt eine Obergrenze und wird nicht umgedeutet. Folglich bedeutet die Regel
40% weniger neue Kandidaten, nicht das sofortige Entfernen von 40% gespeicherter
Tiere oder eine garantierte 40%-Abnahme in bereits cap-gesaettigten Gebieten.
Der statistische Determinismustest verwendet je 16384 Habitate mit vier Seeds.

## Karten

Die orthografische 3D-Kamera teilt weiterhin Originalmodelle und Materialien der
geladenen Welt. Ein zusaetzlicher unbeleuchteter, vertexgefaerbter Batch zeigt
erkundetes entladenes Terrain aus dem vorhandenen Detail-/Tile-/Overview-Atlas.
Maximal 96x96 Rasterzellen, 48000 Quad-Faces; asynchroner Aufbau, keine
Weltgenerierung/Streaming-Erweiterung, Coverage-/Dirty-Pruefung maximal 2 Hz.
Aktuelle geladene Oberflaechenspalten werden nicht doppelt dargestellt. Ein
Render-only-Mesh laesst seine CPU-Geometrie nach Upload frei; Handlewechsel
aktualisieren auch die bestehende Batch-Entity.

Die Hoehlenkarte blendet Terrain-/Wasserdaecher und weit ausserhalb des
Hoehenschnitts liegende Objekte ausschliesslich auf Miniaturlayern aus.
Geladene echte Prop-, Mob- und Spielermodelle bleiben sichtbar; der bekannte
Hoehlenboden und Randwaende kommen aus dem Hoehenatlas. Originale Sichtbarkeiten
werden beim Verlassen exakt restauriert, Hauptwelt und Kollision bleiben gleich.
Prop-Batches verwenden die transformierte Geometrie-AABB statt des oft weit
entfernten Section-Ursprungs. Nur vollstaendig im Hoehenband liegende Batches
bleiben; gemischte Oberflaechen-/Hoehlenbatches werden konservativ ausgeblendet,
nicht auf einzelne Props aufgeteilt.
Die Karte sucht nach Daechern bis zur vorbereiteten Oberflaechenhoehe statt nur
20 m. Licht/Audio verwenden ebenfalls einen bis zur Oberflaeche erweiterten
Sampler, begrenzt auf die Weltspanne und ausschliesslich geladene Zellen.

Spielericons wurden zuvor bei noch fehlender Detailkachel oder unbekannten
Zwischenzellen verworfen. Autorisierte replizierte Peers sind nun davon
unabhaengig; Namen kommen aus dem vorhandenen SocialState und werden in 2D/2.5D
und 3D projiziert.

Grenzen: Unbekanntes bleibt unbekannt; alte Farbkacheln ohne Hoehen benoetigen
erneuten Besuch. Entfernte Originalbauten, Baeume und Tiere werden nicht erfunden.
Der Hoehlenschnitt ist keine vollstaendige rekonstruierte unterirdische Szene.
Die zusaetzliche Kamera, vorhandene Schatten/Transmission und der neue einzelne
Fernbatch sind keine kostenlosen GPU-Effekte.

## Wetter

Schneestufe 5 behaelt Dichte 180 und die bestehenden Qualitaetsbudgets, ihre
Groesse sinkt von 4 auf 2 relativ zur alten leichten Flocke. Jede Flocke besitzt
eigene Fallgeschwindigkeit (2.7..6.4 m/s), Wirbelphase und Rotation; weiche
Recyclinggrenzen verhindern synchronisierte Pop-ins. Die Winkelgroesse wird
begrenzt (bei 1080p/FOV65 etwa acht Pixel voller Flakendurchmesser), sodass nahe
Flocken keine riesigen Sterne werden. Keine neuen Partikel-Entities.
Die Streuung nutzt das ganze 2-/3-m-Stratum statt der bisherigen 0.6-m-Saeulen.
Ein 1-m-Dachcache ordnet jeder Flocke ihre tatsaechliche Zelle zu; Drift und
rotierte Geometrie bleiben innerhalb dieser Zelle. Unbekannte Zellen bleiben
ausgeblendet. Das erhoeht Cache und Aufwaermarbeit, aber nicht den festen
Grenzwert von 24 Dachabfragen je Frame oder den 125-ms-Uploadtakt.

Wolkenentladungen verlaufen horizontal unter/in der Wolkendecke, verzweigen sich
und erhalten weisse Kerne mit weiss/lila Halo. Die Wolken werden in der Naehe
des gemeinsamen autoritativen Blitzereignisses aufgehellt. Bodenblitze behalten
ihren echten Endpunkt. Bestehender einzelner Blitzbatch (560 Vertices), ein
unbeschattetes Blitzlicht und Wolkenpass bleiben; keine globale Daueremission.
Wolken behalten physische Tiefe und die bisherige Reverse-Z-Himmelsordnung.
Das Blitzmaterial unterdrueckt PBR-Fernnebel und verwendet einen begrenzten
linearen HDR-Kern. Andernfalls verschluckte der Standardnebel den Blitz vor
den direkt ausgegebenen Wolken; ein Emissionswert an einem unlit-Material
allein loest diesen Unterschied nicht.

Sturm steigert die lokale Blattobergrenze von 48 auf 144 und verkuerzt das
Emissionsintervall von 0.32 auf 0.055 s. Maximal drei Quellenversuche pro Takt,
ein gemeinsamer Batch und vorhandene 18-m-Entfernungs-/Lebenszeitgrenzen bleiben.

Unter Wasser bleibt nur der vorhandene Aufprall-Batch aktiv: maximal 289 lokale
Ringe bei 8 m Radius und 1 m Abstand, ausschliesslich auf tatsaechlich freien
Wasseroberflaechen. Ringe liegen unter der vom Dach-/Wasser-Cache ermittelten
Oberflaeche und beruecksichtigen Sturmtroegtiefen, damit bewegtes Wasser sie nicht
per Tiefentest verdeckt. Keine fliegenden Tropfen unter Wasser, keine weitere Wasserpass-
oder Fullscreen-Pipeline, keine globalen falschen Hoehlensee-Regenringe.

## Audio

Zwei Aufnahmelagen bilden einen ruhigen Stereo-Regen. Neun reine geladene
Oberflaechenstrahlen alle 0.5 s gewichten Boden-/Wasser-/Dachaufprall mit Abstand
zur Spielerkoerperposition. Bei mehr als 32 m Abstand stumm, normale Spruenge
brechen den Mix nicht ab. Dach-, Hoehlen-, Unterwasser- und Ladefilter bleiben.

Nasse Materialkontakte verwenden echten gespeicherten Nassfilm und aktuelles
lokales Regenklima aus vorbereiteten Spalten, keinen neuen Terrain-Noise.
Vier nicht direkt wiederholte Schwimmtakes, leichte Pegel-/Pitchvariation und
pro Quelle getrennte Kadenz verhindern den bisherigen identischen Einzelklang.
Der Mixer bleibt auf 24 Stimmen und dieselben Audio-Entities begrenzt.

Quellen: vorhandene CC0-Aufnahmen von Ylmir und jcpmcdonald, Bearbeitung mit
`tools/assets/prepare76-water-audio.sh`, Provenienz in `assets/audio/CREDITS.md`.
Rund 4.6 MB zusaetzlicher dekodierter Audiobedarf. Bearbeitete Varianten sind
keine neuen Originalaufnahmen. Eine subjektive Kopfhoerer-/Lautsprecherabnahme
ist damit nicht nachgewiesen.

## Validierung

Ausgefuehrt mit unveraendertem Lockfile und `CARGO_BUILD_JOBS=1`:

```sh
cargo fmt --all --check
cargo check --locked --workspace
cargo test --locked --workspace
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo build --locked --workspace
```

Alle fuenf Pruefungen erfolgreich. 1194 Tests bestanden, ein bestehender
expliziter 180-s-UDP-Soak bleibt ignoriert. Regressionen pruefen unter anderem
vier Spawn-Seeds/negative Habitate, sehr hohe und teilweise geladene Daecher,
Peerlabels ohne Detail-IO, Proxybudgets/Indizes, unverwechselbare Coverage-Hashes,
exakte Layer-Restaurierung, transformierte Pflanzen-AABBs, Schneestreuung mit
zellgenauen Daechern, Unterwasserbudget, Blitzpfade, nasse Kontakte,
Schwimmvarianten und den begrenzten Stereo-Mixer.

Native Vulkan-Pruefung auf AMD Radeon RX 6950 XT, RADV/Mesa 26.2.4,
Ryzen 9 5900X, ohne zeitgleiche Cargo-Kompilierung in den Wetterfenstern:

- Wetterfixture bis Abschlussphase 50 bestanden: fuenf Schneestaerken,
  Wolkendecken, Himmelsverdeckung, horizontale weiss/lila Entladung und
  Bodeneinschlag, Sturm/ruhiges Meer, Regenfilm-/Unterwassermasken,
  Unterwasser-Aufprallringe, Low/Off-Budgets, Nacht, Schnee und Feuer.
  Die fertigen Schnee-, Blitz- und Unterwasser-Aufnahmen wurden angesehen.
  Der letzte Unterwasseraufbau zeigt 142 echte Wasser-Aufprallquads in einem
  Batch. Das ist keine volle Kamera-/Szenenkombinationsabnahme.
- Kartenfixture: alle 52 Schritte bestanden, einschliesslich echter
  replizierter Peers mit Namen in beiden Ansichten, Pan/Zoom ohne Zurueckspringen,
  3D-Hoehlenschnitt und Wiederherstellung. Die Aufnahme zeigt originales
  geladenes Terrain neben der bewusst groben erkundeten Fernrekonstruktion.
  Der Hoehlenschnitt hat 1677 Cache-Faces und 21 verbleibende Prop-Teilmeshes
  statt der faelschlich eingeblendeten Oberflaechenvegetation.
  Die Oberflaechenminiatur zeigt 2612 Terrain-, 319 Prop- und 22 Spieler-Teilmeshes;
  der Mobschalter wechselt von 0 auf 400 Tier-Teilmeshes, nicht 400 Tiere.
  Drei vorhandene Schattenkaskaden und geteiltes Himmelslicht bleiben aktiv.
- Audio: 27 explizit injizierte Materialkontakte wurden vom echten AudioSink
  abgespielt. PCM-/Mixer- und Nasskontaktregeln zusaetzlich als Tests geprueft;
  kein behaupteter subjektiver Hoer- oder echter Lauf-/Schwimm-Eingabetest.
- Echte integrierte Ladesitzung: 100%-Halt, Reveal und beide Welt-Audiostreams
  erst nach Abschluss bestanden.
- Dedicated Server ohne `DISPLAY` und `WAYLAND_DISPLAY`: 27 Sections,
  erfolgreicher fensterloser Start. Normaler Dependency-Baum weiterhin ohne
  bevy_render, bevy_pbr, WGPU, Winit, TiltUI, terra-render oder terra-ui.

Die Wayland-Kartenfixture protokolliert wie zuvor zweimal einen nicht gesetzten
Cursor-Lock beim simulierten Resume. Eingabe, Pan/Zoom und Exit funktionieren;
kein Shader-/Validierungsfehler. Wetter-, Audio- und Ladefixture ohne Fehlerlogs.
Der Kartentest ist ein lokaler Mehrclient-Test, keine neue Internet-/LAN-Abnahme.

### Kurze Framezeitfenster

1920x1080, Sichtweite 8/vertikal 3, Seed 42042, VSync aus; je Zustand etwa
1.5 s nach 0.5 s Anlauf. Unterschiedliche Zustaende desselben Builds, keine
identischen Vorher-/Nachher-Messpaare:

| Zustand | FPS | p95 ms | p99 ms | Niederschlagsquads |
| --- | ---: | ---: | ---: | ---: |
| Schnee 1 | 172.08 | 6.564 | 6.699 | 2312 |
| Schnee 5 | 161.48 | 7.155 | 7.837 | 13872 |
| Sturm am Meer | 169.93 | 7.225 | 7.980 | 2423 |
| Ruhiges Meer | 176.02 | 6.756 | 8.328 | 0 |
| Unterwasser-Regenaufprall | 171.57 | 7.130 | 8.389 | 142 |

Der gesamte 120.734-s-Wetterlauf erreicht 924.5 MiB Peak-RSS und 3028.9 MiB
DRM-VRAM-Peak. Der 169.724-s-Kartenlauf erreicht 1242.1 MiB Peak-RSS und
3034.7 MiB DRM-VRAM-Peak; darin sind Szene, Assets, RTs und Treiberressourcen
enthalten, nicht allein die Kosten dieser Aenderungen. Beide GTT-Peaks: 68.0 MiB.
Peak-RSS/VRAM sind keine Speicher-Deltas oder GPU-Framezeiten.

Keine isolierten GPU-Zeiten, vollstaendigen Frame-/Subdrawzahlen, neue CPU-
Generierungsbaseline, 32er-Langzeit-/Mehr-GPU-Werte oder Vorher-/Nachher-Paritaet
erhoben. Historische Regressionen bleiben in den bisherigen Berichten erhalten.
Die begrenzten Geometrie-/Abfragebudgets und kurzen Szenenfenster ersetzen keine
Langzeit-Streaming-, Mehr-GPU- oder Speicherparitaetsmessungen.

Rohdaten: [Wetter](logs/weather.log), [Karten](logs/maps.log),
[Audio](logs/steps.log), [Laden](logs/loading.log),
[Tests](logs/tests.log), [Check](logs/check.log), [Clippy](logs/clippy.log),
[Build](logs/build.log), [Headless](logs/headless.log),
[Server-Dependencies](logs/server-dependencies.log).

Angesehene Aufnahmen: [Schnee 5](captures/snow-level-5.png),
[Wolkenblitz](captures/lilac-white-horizontal-cloud-lightning.png),
[Regen von unten](captures/rain-impacts-seen-from-underwater.png),
[2D-Spielerlabel](captures/map-4.png), [Hoehlenschnitt](captures/map-20.png),
[3D-Fernkarte und Spielername](captures/map-38.png).
