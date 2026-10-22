# Phase 93: Darstellung, Auswahlbudgets Und Laufgefuehl

## Arbeitsstand

Code implementiert. Formatierung, Workspace-Check, 1545 Tests (zwei ignoriert),
strenges Clippy und Workspace-Build bestehen. Der Dedicated Server startet
ohne DISPLAY/WAYLAND_DISPLAY und bestaetigt 27 Sections. Elf Python-Tests
der Messwerkzeuge bestehen ebenfalls. Die finalen nativen Kurzpaare sind
gemessen; der laengere Nachherlauf und funktionale Gegenproben folgen unten.

## Messbedingungen

RX 6950 XT / RADV / Vulkan, Ryzen 9 5900X, 32 GiB RAM, native 3840 x 2160.
Optimiertes Dev-Profil des unveraenderten Workspaces, keine Dependency-Upgrades.
Keine parallele Kompilierung und kein zweiter Testclient waehrend GPU-Laeufen.
Vorherbinary: `target/phase92/terra-client-font-cache`.

Die beiden Vorherlaeufe verwenden je eine neue isolierte Kopie von
`progressive-collapse-2026-10-20/saved-start`. Welt-ID 6690318646380802212,
Seed 10572586493552409009, Generator 5, gespeicherte Hoehlenrevision 4.
Der Weltmanifestvertrag ersetzt den Seed 42042 aus den kopierten Configs.
Integrierter 60-Hz-Server, normales Laufen und Springen, TiltUI-HUD,
Minimap, Autosave und sieben echte 64-m-Rueckwege je 180-s-Lauf.
Es gibt keine Flug-/Teleportabkuerzung der Gehstrecke.

VSync/FPS-Limit aus, volle Vegetation, High-Wasser/-Schatten, Ultra-Wetter,
Rise-Animation, Terrain-LOD 153,6/256 m, vertikale Weite 3, vier
Generierungsjobs, 16 Meshjobs und zwoelf Uploads je Frame. Grafikdateien
`configs12` und `configs20` sind massgeblich. Server-Simulationsweite 1.

Vor dieser Phase wurden alte Testbinaries aus dem RAM-Dateisystem archiviert
(siehe Phase 92). Deshalb wurden neue Vorherlaeufe aufgenommen; alte FPS-Werte
werden nicht als zeitnahes Paar mit diesen Nachherlaeufen ausgegeben.

## Vorhermessungen

Erste native Zusammenfassungen ohne Screenshot-Nachbarframes, noch im
Fenstermodus:

| Szenario | Dauer | FPS | p95 (ms) | p99 (ms) | Anfangsladung (s) |
| --- | ---: | ---: | ---: | ---: | ---: |
| Weite 12, Clear | 180 s | 90,23 | 17,359 | 20,398 | 19,217 |
| Weite 20, Rain | 180 s | 63,21 | 24,960 | 30,777 | 50,355 |

Der erste Nachherlauf `after-walk12-clear` wurde nach Aufloesungsdrift auf
3840 x 2124 abgebrochen (SIGTERM/Exit -15). Seine Teilfenster sind **kein**
gueltiges FPS-Messpaar. Auch der Borderless-Versuch
`before-walk12-borderless` wurde mangels UI-Messbereitschaft beendet; er
liefert keine FPS-Abnahme. Stattdessen verwenden beide Seiten ausdruecklich
dieselbe verfuegbare Fensterflaeche **3840 x 2124**, keine behaupteten
3840 x 2160. Die `*-stable`-Laeufe erhalten frische Kopien derselben
gespeicherten Welt und derselben urspruenglichen Kartenkonfiguration.
Das Vorherbinary liest `assets-before`, dessen einziger
inhaltlicher Unterschied der archivierte bisherige `terra_props.wgsl` ist;
alle anderen Assetdateien verweisen auf den unveraenderten Projektbestand.

Im Regen-/20-Lauf kostet `mapping.schedule` 5,404..5,698 ms pro Aufruf;
der Index wird 144..145-mal pro 30-s-Fenster geplant. Die volle Terrain-
Auswahl kostet 5,843..6,059 ms pro Aufruf, maximal 8,779 ms im Messfenster.
Das sind gemessene CPU-Kostenpfade, keine aus dem FPS-Mittel erratenen Ursachen.

## Implementierte Korrekturen

- Inkrementeller, Atlas-gehaltener Kartenindex; unveraenderte Kandidaten vor
  Sortierung ausschliessen und ueberholte Worker-Ergebnisse ablehnen.
- Globale Terrain-Auswahl nach Initialladen auf 1024 Kandidaten/weich 750 us
  verteilen; Publikationen separat auf 256/weich 250 us begrenzen; lokale
  Retrys, deduplizierte Queue und korrekter Weltwechsel.
- Schatten-Off verhindert Proxy-Komposition/-Upload und entfernt alte
  Shadow-Meshes, ohne sichtbare Geometrie oder Windmasken zu reduzieren.
- Bei verspaeteten Grasjobs Wachstum am Uploadzeitpunkt neu basieren und
  Motion-Vectors mit vorigem Wachstumsgewicht berechnen.
- Distanzabhaengiger, abschaltbarer Geh-/Sprintkamerarhythmus mit Landefeder;
  keine Aenderung von Controller, Netzposition, Zielrotation oder Chunk-Fokus.
- Echte GPU-Subdrawkopien an Early-/Late-/Color-Grenzen pro Kamera;
  budgetierter asynchroner Readback und eigener Fullscreen-Drawzaehler.

## Zwischenstand v1

`before-walk12-stable` und `after-walk12-stable` sind das erste gueltige
180-s-Paar, beide durchgehend 3840 x 2124. Der Nachherstand ist noch keine
finale Performanceabnahme:

| Weite 12 / Clear | Vorher | v1 |
| --- | ---: | ---: |
| Native FPS | 86,81 | 86,01 |
| p95 (ms) | 18,381 | 18,180 |
| p99 (ms) | 21,965 | 21,521 |
| Anfangsladung (s) | 19,462 | 19,360 |
| `mapping.schedule`, gesamt (ms) | 1660,154 | 1036,836 |
| `mapping.schedule`, Maximum (ms) | 2,898 | 3,896 |
| Terrain-Auswahl, gesamt (ms) | 1292,023 | 1205,833 |
| Terrain-Auswahl, Maximum (ms) | 3,855 | 2,069 |

Die Karten-Gesamtkosten sinken, ihre seltenen Vollindex-Rueckfaelle werden
aber langsamer. v1 baut den vollstaendigen Index noch 109-mal statt 871-mal
auf, mit rund 1,8..1,9 statt 0,9 ms je Aufbau. Das rechtfertigt keine
pauschale FPS-Verbesserung: Das Mittel faellt um rund 0,9%, waehrend die
Framezeitperzentile etwas besser werden. Die Terrain-Zaehler sind nicht
eins zu eins vergleichbar: v1 zaehlt 2108 kurze Slices statt 592 ganzer
Scans. Gesamtarbeit und Einzelmaximum werden daher getrennt ausgewiesen.

Iteration 2 optimiert deshalb den Index-Bulkaufbau und trennt reine
Journal-Epochenaenderungen vom notwendigen vollstaendigen Store-Fallback.
Auch geloeschte Journal-Epochen muessen dabei zurueckgesetzt werden;
Publikationen mit identischen Revisionen duerfen weiterhin nicht als
unveraendert durchrutschen.

## Finales v2-Kurzpaar

Frische Welt-/Kartenkopien, unveraenderte Qualitaetsoptionen, auf beiden Seiten
3840 x 2124. Rohdaten: `before-walk12-stable`, `after-walk12-v2`,
`before-walk20-stable`, `after-walk20-v2`, jeweils `.log`, `.json`, `.csv`,
`.png`. Das Nachherbinary liegt unter `target/phase93/terra-client-render-walk-v2`.

| Szenario, je 180 s | FPS vorher | FPS v2 | p95 vorher/v2 (ms) | p99 vorher/v2 (ms) | Laden vorher/v2 (s) |
| --- | ---: | ---: | ---: | ---: | ---: |
| 12 / Clear | 86,81 | 86,65 | 18,381 / 18,069 | 21,965 / 21,086 | 19,462 / 20,327 |
| 20 / Rain | 62,49 | 63,42 | 24,885 / 23,130 | 31,199 / 27,817 | 50,799 / 51,005 |

Kein grosser Durchschnitts-FPS-Gewinn: 12 bleibt praktisch gleich, 20 steigt
um rund 1,5%. Der Regen-p99 sinkt um rund 10,8%; weitere Spikes bleiben.
Anfangsladung wird nicht schneller. Native Screenshots zeigen weiterhin
Terrain, Windvegetation, HUD und Minimap; das ist keine Referenz-Art-Abnahme.

CPU-Scope-Intervalle ueber dieselben 180 s (verschachtelte Scopes nicht addieren):

| Scope | 12 vorher/v2 gesamt (ms) | 20 vorher/v2 gesamt (ms) | 20 vorher/v2 Maximum (ms) |
| --- | ---: | ---: | ---: |
| Kartenindex-Bulkaufbau | 785,619 / 167,316 | 1974,994 / 210,803 | 4,494 / 5,448 |
| Kartenplanung | 1660,154 / 1010,997 | 4375,222 / 2591,736 | 7,777 / 8,490 |
| Terrain-Auswahl | 1292,023 / 1317,596 | 3037,899 / 3497,863 | 8,516 / 6,788 |

Der Index wird bei Weite 20 nur noch 48 statt 864-mal voll aufgebaut.
Die seltenen Rueckfaelle sind einzeln weiterhin teurer; das hoehere Maximum
und die insgesamt hoehere Terrain-Auswahlarbeit sind explizite Regressionen.
2212/5232 kleine Terrain-Slices sind nicht mit 592/499 alten Vollscans als
identische Arbeitseinheiten vergleichbar. Die Karte spart insgesamt CPU,
die Terrain-Zerlegung verteilt Arbeit und reduziert das Einzelmaximum;
sie beseitigt nicht jede Hauptthread-Spitze.

Der Kartenindex prueft bei Journal-Aenderungen den kleineren Bereich aus
historisch geaenderten und aktuell geladenen Sections. Eine Regression mit
4096 entfernten Edit-Sections bei einer geladenen Section bleibt bei einer
Beitragspruefung, auch nach Journal-Reset und in der Hoehlenebene.

Native Benchmarks mit expliziter Grafikdatei brechen jetzt ab, wenn die vom
Fenstersystem tatsaechlich gelieferte Renderaufloesung nach der Startphase
von der angeforderten abweicht. So koennen reduzierte Fenster nicht mehr
unbemerkt als vermeintlicher FPS-Gewinn in ein Messpaar geraten.

## Weiterhin Nicht Abgeschlossen

Der gesamte TODO-Abschnitt ist nicht mit diesem Patch erledigt. Lokal weiterhin
moeglich und offen sind die vollstaendige Mehrseed-/Edit-/Flug-/Mehrclient-
Szenarienmatrix, alle neun Connector-Kamerafahrten, dichte Pilzpark-/Fackel-
Langtests und die historische Flugdriftanalyse. Weitere physische GPUs fehlen.
Globale unterirdische Wassernetze, Tier-Region-Paging/-Koerperkollision,
historische Kristallarm-Bewegungskollision, optionale Offscreen-Reflexionen,
dreidimensionale Nebeldichte/indirektes Voxellicht und assetabhaengiges
Skinning sind nicht neu implementiert. Dies sind echte Erweiterungen, keine
als abgeschlossen ausgegebenen Messblockaden.

## Nativer 15-Minuten-Lauf

`after-walk20-long`: reale Integrated-/HUD-/Minimap-Strecke, Weite 20,
3840 x 2124, frische Welt-/Kartenkopie, Rise, unveraenderte High-Qualitaeten.
900 s Messfenster nach 51,011 s Anfangsladung, 35 echte 64-m-Rueckkehrzyklen.
61,47 FPS insgesamt, p95 24,226 ms, p99 29,517 ms, Exit 0, keine WARN/ERROR.

| Laufzeit | FPS vorher | FPS v2 (letzte 30 s) | RAM vorher | RSS v2 (MiB) |
| --- | --- | ---: | --- | ---: |
| 1 Minute | nicht als Langlauf gemessen | 64,48 | nicht gemessen | 1982,57 |
| 5 Minuten | nicht gemessen | 61,92 | nicht gemessen | 2083,95 |
| 10 Minuten | nicht gemessen | 60,67 | nicht gemessen | 2167,40 |
| 15 Minuten | nicht gemessen | 56,98 | nicht gemessen | 2251,77 |
| 30 Minuten | nicht durchgefuehrt | nicht durchgefuehrt | nicht gemessen | nicht gemessen |

Alle 30-s-Fenster liegen zwischen 56,63 und 66,55 FPS; der gemeldete
Unter-18-FPS-Kollaps ist hier nicht reproduziert. Dies ist keine identische
stationaere Szene: Position und sichtbare Geometrie wechseln, die Weltzeit
laeuft von 14,31 bis 04,81 des Folgetags, und der temporaere Regen-Override
kehrt zum normalen Klimawetter zurueck. Deshalb weder konstantes Regenwetter
noch aus den fallenden FPS allein einen Leak bzw. dessen Behebung ableiten.

Ressourcen schwanken: 44133..49540 Entities, 31475..35572 Mesh-Assets,
199..274 Standardmaterialien und 192..194 Bilder; keine monotone Assetexplosion.
Generierungsqueue am Ende leer, Terrain-/Propjobs am Endsample null;
Unterwegs bleibt Streaming-/LOD-Arbeit aktiv. Peak-RSS 2306,80 MiB,
Peak-VRAM 5050,93 MiB. RSS liegt am 15-Minuten-Checkpoint rund 269 MiB hoeher
als an Minute 1: **keine nachgewiesene Speicherparitaet**. Der Benchmark haelt
ausserdem seine CPU-/GPU-Messreihen bis zum Abschluss; deren Einfluss ist nicht
als alleinige Ursache des Zuwachses isoliert. Weitere Last-/Standlaeufe bleiben
notwendig. Ohne vorherigen 15-Minuten-Lauf desselben Standes existiert kein
belastbares Langzeit-Vorher-/Nachherpaar.

Der abschliessende Schatten-Randfallpatch betrifft nur `shadow_quality = off`:
ein outgoing `QueuedChunk` darf beim Freigeben seines Proxys nicht vorzeitig
auf den unsichtbaren Queue-Layer wechseln. Der High-Soak verwendet diesen
Pfad nicht; der neue Regressionstest deckt ihn separat ab.

## Grenzen

Nur eine physische GPU ist vorhanden. Mehr-GPU-Abnahme, echte grosse
Mehrspielergruppen und menschliche Art-/Audioabnahmen sind damit nicht
erledigt. Optionale Offscreen-Reflexionen, indirektes Voxellicht,
dreidimensionaler Nebel und Animation/Skinning fremder Spezialassets sind
keine Bestandteile dieses gezielten Performancepatches. Private
Gradient-/Tilt-Materialien und engineinterne Fullscreen-Draws lassen sich
ueber die vorhandenen oeffentlichen Draw-Hooks nicht vollstaendig zaehlen.
