# Phase 60: Tierwelt, Weltgrenzen Und Bedienung

## Autoritaet Und Konfiguration

Natuerliche Tiere werden in einem 150-m-Habitat um alle verbundenen Spieler
gesucht und bis 150 m repliziert. Zwei Habitatproben je KI-Takt, maximal
128 Tierupdates je Takt und 128 dargestellte Tiere je Client bleiben begrenzt.
Die Autoritaet erweitert beim Verbinden ihren asynchronen Terrainbereich auf
mindestens zehn Sections Radius, ohne Renderer oder synchrone Generierung.
Ohne Spieler beziehungsweise bei abgeschalteter Naturpopulation bleibt der
konfigurierte Simulationsbereich erhalten. Mehr Serverterrain ist nicht kostenlos.

Standard: 256 lebende natuerliche Tiere weltweit, kein Multiplikator je Spieler.
In `configs/server.toml` setzt `natural_mob_limit = 256` einen Dedicated-Override
(0..4096). Ohne Override gilt `WorldConfig.natural_mob_limit`. Der Slider im
Erstellen-Dialog speichert 0..1024 je Einzelspielerwelt. Ein Integrated Server
verwendet diese Weltgrenze, nicht einen Dedicated-Override.
Ein nachtraeglich kleineres Limit stoppt weitere natuerliche Spawns, loescht
aber keine vorhandenen Tiere. Kontrollierter Despawn/Respawn bleibt offen.

Besitzerzuordnungen sind als persistente `CreaturePopulation.owners` getrennt
von stabilen Tier-IDs vorhanden; diese Tiere zaehlen nicht zum Wildtierlimit.
Das implementiert noch keine spielbare Zaehmung. Die harte Speicher-Sicherheits-
grenze der Gesamtpopulation bleibt unabhaengig bestehen.

Netzwerkprotokoll **34**: Client und Dedicated Server gemeinsam neu starten.
Neue Regionsdateien **REG9** speichern Besitzer; REG8 wird mit leerer
Besitzerzuordnung explizit migriert. REG7 und die zuvor unterstuetzten Leser
bleiben erhalten. IDX3 und PLY3 aendern sich nicht.

## Tierverhalten Und Darstellung

Bienen reservieren unterschiedliche Blumen statt gemeinsam die naechste Blume
anzufliegen. Nektar bleibt endlich und regeneriert wie bisher. Das Nest ist
der Leinenanker mit maximal 100 m Abstand. Bei Tageslieferung verschwinden
Bienen kurz im Nest, geben serverseitig Nektar ab und kommen wieder heraus;
nachts bleiben sie darin. Bestehende Ein-/Ausflug-Cues werden wiederverwendet.

Landtiere schlafen mit eingefalteten Beinen, abgesenktem Kopf und geschlossenen
Augen. Vorbeigehende Spieler wecken sie nicht mehr; echte Schadens-Flucht und
der Tageswechsel bleiben erhalten. Vollstaendig blockierte Tiere pausieren,
statt pro Takt ihre Richtung weiterzudrehen. Das bleibt budgetierte lokale
Navigation, keine globale Wegsuche.

Fische treiben nicht mehr wie Gegenstaende mit voller Stroemungsantwort ab:
aktive Eigenbewegung kann den moderaten Drift ueberwinden. Neben bisherigen
Clownfish-Varianten gibt es einen tuerkis-goldenen Riverfin mit segmentierter
Koerperkontur, Ruecken-/Brustflossen und Faecherschwanz. Gemeinsame gebackene
Meshes, keine Entity je Detailvoxel. Namen kommen aus `fish.json.variant_titles`.
Die Vorlage ist Art-Richtung, keine behauptete pixelgenaue Kopie.

## Welt Und Licht

Neue Welten speichern `bounded_world = true`. Alte Metadaten ohne diesen
Schluessel bleiben `false`: keine stille Neugenerierung historischer Welten.
Der Erstellen-Dialog und der Dedicated-Resolver fuer eine noch nicht vorhandene
Welt aktivieren den neuen Vertrag. Beim erneuten Oeffnen gilt die gespeicherte
Weltpolitik. Bestehende Metadaten nicht nachtraeglich als gefahrlosen
Retrogen-Schalter behandeln.

Beim neuen Vertrag sind Blockzellen Y -128..356 zulaessig. Y -128/-127 sind
Grundstein (neue ID 41; alle alten IDs bleiben stabil); Hoehlen enden vor Y -120.
Generierung, direkte Blockabfrage, Streaming und autoritative Platzierung
beruecksichtigen die Grenzen. Survival kann Grundstein unabhaengig vom Werkzeug
nicht zerstoeren; Creative darf ihn abbauen. Gewoehnliche Cube-Pipeline,
keine separate GLB.

Zusatz-Portale haben geringere Hang-Anforderungen. Seltene trockene, weich
variierende Schluchten oeffnen Hoehlen nach oben. Das bestehende gecachte
Spalten-/Hoehlensystem wird erweitert, nicht ersetzt.

Neue Erzverteilung besitzt zusaetzliche absolute Obergrenzen: Kohle 200,
Eisen 170, Gold 56, Diamant -5, Rubin -32. Bestehende Tiefen-/Frequenzregeln
gelten weiterhin. Salz bleibt an seine vorhandenen Salzhoehlen gebunden.
Alte Weltvertraege behalten ihre Erzverteilung.

Hoehlen-Grundlicht 42 -> 25.2 (-40%). Fackeln behalten echte Schattenlichter
und bekommen im selben ausgewaehlten Lampenpool einen kleinen schattenlosen
Nahlichtanteil fuer ihren Traegerblock. Keine weiteren Schatten-Cubemaps;
dennoch zusaetzliche lokale Lichtauswertung. Sonnenstrahl-Spitzen entfernt,
Mondshader unveraendert.

## Bedienung, Chat Und Audio

Chat beginnt fuenf Sekunden nach dem Schliessen beziehungsweise neuen Eintraegen
eine zweisekuendige Ausblendung. Mit `/` navigieren Pfeile durch Vorschlaege,
Tab uebernimmt; normaler Text verwendet den Verlauf mit Wiederherstellung des
Entwurfs. `@` vervollstaendigt echte Spielernamen. Erwaehnungen sind sichere
Textsegmente, fett und pink/lila; nur der angesprochene lokale Spieler bekommt
einen rate-limitierten Hinweiston. Keine Auswertung von Chattext als HTML.

HUD Layout verschiebt Vitalbalken, Chat und Info-Karte unabhaengig, DPI-/UI-
skaliert und auf den sichtbaren Bereich begrenzt. Speichern aktualisiert
`graphics.toml`; Abbrechen uebernimmt nichts. Die Hotbar bleibt unveraendert.

Strg (rebindbare Sprint-Aktion) antippen schaltet im Wasser gerichtetes Tauchen;
Leertaste beendet es und bringt den Spieler an die Oberflaeche. Clientvorhersage
und Server benutzen denselben Bewegungsweg. Spieleravatare zeigen eine weiche
Schwimm-/Tauchpose mit Armzuegen und Beinschlag. Die vorhandene Kollisionskapsel
wird dabei nicht auf einen neuen liegenden Physikkoerper umgestellt.

Aufsammelbare 3D-Drops fliegen ueber mehrere autoritative Schritte zum Spieler.
Kapazitaet, Reichweite, Verzogerung und Sichtlinie werden weiter geprueft;
erst in unmittelbarer Naehe erfolgt die Inventaruebernahme. Kein doppelter Drop.

Schritt-Foley nutzt konservativ entrauschte, dynamisch angeglichene Varianten
der bereits lizenzierten Aufnahmen; Originale bleiben erhalten. Sanfte eigene
UI-Toene sind kurz und rate-limitiert. Quellen und reproduzierbare Bearbeitung:
`assets/audio/CREDITS.md`, `prepare60-steps.sh`, `build60-ui-audio.mjs`.
Kein Audio aus anderen Spielen und keine erfundene Aufnahme-/Aufloesungsqualitaet.

## Validierung

Alle fuenf Workspace-Pruefungen sind erfolgreich:

- `cargo fmt --all --check`
- `cargo check --workspace`
- `cargo test --workspace`: 950 bestanden, ein separat laufender Soak ignoriert.
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo build --workspace`

Die Tests pruefen unter anderem REG8-Migration/Besitzerpersistenz, globale
Populationszaehlung, Bienenreservierung/Nestlieferung, Schlaf/Blockierung,
Weltgrenzen/Grundstein/Erzobergrenzen, Wasserbecken/Flussdach-Konnektivitaet,
Chatpfeile/Erwaehnungen/Fade, unabhaengige HUD-Positionen, Tauchen/Auftauchen
und autoritative Drop-Anziehung samt Reconnect. Der Dedicated Server startete
ohne DISPLAY/WAYLAND_DISPLAY und ohne Renderer.

Der separat ausgefuehrte Test
`tests::sustained_two_profile_udp_session_keeps_identity_and_replication`
ist ebenfalls erfolgreich: 180.004 s, 25874 Takte, jeweils 4313 bestaetigte
Zustandsbeobachtungen fuer zwei echte lokale UDP-Profile. Das ist kein
Mehrrechner-/Internettest und keine lange 150-m-Herdenlastmessung.
Prueflogs liegen als `terra60-*-final.log`, `terra60-headless.log` und
`terra60-udp-soak.log` in diesem Verzeichnis.

Native Wayland-Pruefung auf Radeon RX 6950 XT, RADV/Mesa 26.2.4:
Tieransichten inklusive Schlaf und Riverfin, acht Hoehlen-/Fackelansichten,
echter Chat mit fetter pinker Erwaehnung und zwoelf Survival-/HUD-Ansichten
erfolgreich; PNGs in `creatures/`, `caves/`, `chat/`, `hud/`.
Zusaetzlich 25 erfolgreiche Tageszeit-/Unterwasseransichten in `daylight/`;
die Sonnen-Nahansicht zeigt keine dreieckigen Strahlen. Bilder wurden
stichprobenartig angesehen, kein behaupteter pixelgenauer Referenzvergleich.
Audiodekodierung ist getestet, eine menschliche Klangabnahme ist ohne
Audiogeraet nicht erfolgt. Native Logs: `terra60-native-*.log`.

## Messpaar: Ruhender Wald

Je 60 s gemessene Frames, 1920x1080, gleiche gespeicherte Benchmarkkonfiguration
aus `../soundscape-landscape-2026-10-18/configs`, Seed 42042, Sichtweite 8,
identische Kamera, keine parallel laufenden Compiler oder Testclients.
Der bereits vorher laufende Benutzer-Dedicated blieb in beiden Laeufen aktiv.
Baseline ist das vor Aenderungen gesicherte Phase-59-Binary, dessen eingebetteter
UI-Pfad fuer alte Templates isoliert wurde. Der isolierte Assetbaum entfernt
nur das neue, dort unbekannte Grundstein-Asset und die neue Fisch-Namensliste
und stellt den alten Sonnenshader wieder her (`prepare60-baseline.mjs`).
Keine Terrain-/Geometrieaenderung am Baseline-Binary. Dieser Offline-Wald nutzt
den historischen Weltvertrag und misst keine neue 150-m-Serverpopulation.

| Messung | Vorher | Nachher |
| --- | ---: | ---: |
| Mittlere FPS | 225.93 | 217.58 |
| p95 Framezeit | 5.609 ms | 5.935 ms |
| p99 Framezeit | 6.666 ms | 7.398 ms |
| Szenenbereitschaft | 2.054 s | 1.917 s |
| Terrain-Komposition, aufsummierte Workerzeit | 1.354 s | 1.287 s |
| Prop-Komposition, aufsummierte Workerzeit | 0.200 s | 0.187 s |
| Prozess Peak RSS | 792108 KiB | 790276 KiB |
| DRM Peak VRAM | 1874996 KiB | 1875004 KiB |
| Geladene / gerenderte Sections | 2041 / 875 | 2041 / 875 |
| Mesh-Assets | 3524 | 3529 |
| Repraesentierte Props / Batches | 25808 / 328 | 25808 / 328 |
| Sichtbare Mesh-Entities | 1672 | 1672 |
| GPU Early-Prepass, Mittel | 0.387 ms | 0.403 ms |
| GPU Opaque-Pass, Mittel | 1.009 ms | 1.033 ms |

Rohlogs: `forest-before.log`, `forest-after.log`; Messwerkzeug `measure.py`.
Rund 3.7 Prozent weniger FPS und hoeheres p99 in diesem einzelnen Kurzpaar:
keine behauptete Verbesserung oder allgemeine Performanceparitaet.
Workerzeiten sind nicht reine Terrain-Generierungszeit, einzelne GPU-Paesse
keine Gesamt-GPU-Framezeit und sichtbare Entities keine Draw-Call-Zahl.
CPU-Erstgenerierung wurde hier nicht separat zeitgemessen.
Lange bewegte 150-m-Mehrspielerlaeufe, grosse zahme Herden und weitere GPUs
bleiben eigenstaendige Lastabnahmen.
