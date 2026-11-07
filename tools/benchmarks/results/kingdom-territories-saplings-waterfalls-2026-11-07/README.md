# 202 — Gebietshierarchie, Weltoptionen, Setzlinge und Wasserfälle

Auftrag vom 07.11.2026. Referenz: [natürlicher Wasserfall / F2-Hand](../../../../docs/references/natural-waterfall-spill-and-f2-hand.png).

Es werden auf ausdrücklichen Nutzerwunsch keine Grafiktests, Spielclients,
Screenshots oder GPU-Galerien gestartet. Cargo arbeitet mit einem Job, effektiver
Nice-Priorität 19, IO-Klasse 2 / Priorität 7 und ohne DISPLAY/WAYLAND_DISPLAY.
Die Tests verwenden höchstens vier Threads. Ein Bevy-Window-Datenobjekt in einem
CPU-Test erzeugt kein natives Fenster oder Grafikgerät.

## Verhalten

- Setzlinge prüfen beim Pflanzen ihre tatsächliche Miniaturform, Boden, Wasser,
  Rechte und freien Inventar-/Speicherplatz. Der spätere ausgewachsene Baum ist
  kein Platzierungshindernis. Natürliches weiches Bodenbewuchs darf nach Prüfung
  der Abbaurechte atomar ersetzt werden; Spielerprops und Kulturen bleiben erhalten.
  Clientvorschau und Server verwenden dieselbe Prüfung und denselben Strahl;
  abgelehnte Aktionen erhalten eine konkrete Pflanzenmeldung.
- Natürliche Flussfälle behalten beim Auftreffen auf Terrassen ihren Flusskorridor,
  statt Gras-/Sandufer seitlich zu überfluten. Tatsächliche Upstream-Zellen bestimmen
  den Ursprung mit begrenzter Suche und einem Cache je Simulationsschritt. Gegrabene
  Kanäle, gesetzte Quellen, Rohre und Waterlogging behalten ihre gewöhnliche Physik.
- F2 blendet das eigene Item aus der Egoperspektive mit aus. Weltobjekte,
  fremde Spieler und die Charaktervorschau bleiben sichtbar.
- Neue Claims benötigen nur eine Verbindung zur bestehenden Fläche derselben
  Siedlung; die Entfernung zum Bergfried begrenzt ihre Ausdehnung nicht.
- Außenposten, Dörfer und Städte sind dem Hauptkönigreich zugeordnet und erben
  Mitgliedschaft, Rollen, Wappen und Diplomatie. Die Karte kann ein Untergebiet
  als Claimziel verwenden. Transaktionen bleiben autoritativ und atomar.
  Ein inzwischen aufgelöstes oder fehlendes Entwurfsziel wird beim Apply abgewiesen,
  statt den Entwurf an ein anderes Gebiet umzuleiten. Der Strukturvalidator verlangt
  bei vorhandenen Claims weiterhin einen geschützten Bergfried-Chunk.
- Standardmäßig haben Untersiedlungen eigene Level. Optional folgen ihre
  Chunk-Limits dem Kingdom-Level. Das Hauptlevel steuert Außenpostenanzahl und
  Stadtfreischaltung.

| Gebiet | Build Level 1 | Build je weiterem Level | Territorium Level 1 | Territorium je weiterem Level |
| --- | ---: | ---: | ---: | ---: |
| Kingdom | 32 | 6 | 64 | 16 |
| Außenposten | 10 | 2 | 22 | 6 |
| Dorf | 10 | 2 | 0 | 0 |
| Stadt | 20 | 4 | 0 | 0 |

Ein Kingdom darf je Level einen Außenposten besitzen. Dörfer und Städte werden
in eigenem Territorium gegründet; Städte verlangen Kingdom-Level 3 und zehn freie
Chunks Abstand zum übrigen Build-Land des Reichs und seiner Dörfer. Zwischen den
Landgrenzen unterschiedlicher Kingdoms/Außenposten bleiben mindestens 16 freie
Chunks, auch zwischen Hauptreich und eigenem Außenposten. Prüfungen erfolgen bei
Gründung und Erweiterung; vorhandene historische Claims werden nicht gelöscht.

## Konfiguration und Kompatibilität

Alle 20 Gebietsregeln stehen in [configs/server.toml](../../../../configs/server.toml)
unter `[kingdom_rules]` und in World Create. Die Erstellungsansicht verwendet die
Tabs **General**, **Biome / Structs**, **Kingdom**; Tabwechsel erhalten den Entwurf.
Ungültige Eingaben ändern keine Teilwerte. Die Weltbibliothek speichert sämtliche
Einstellungen; Singleplayer nutzt die Regeln seiner Welt. Ein ausdrücklicher
Dedicated-Server-Override wird nach dem Laden angewendet und mit gespeichert.

Der Abstand natürlicher neuer Kingdoms/Außenposten wird bei Erstellung als
`structure_border_gap` eingefroren. Neue Welten verwenden Strukturenrevision 9;
Revisionen 0–8 bleiben unverändert. Eine begrenzte Auswahl von höchstens 576
Kandidatenproben pro Region verhindert typübergreifend zu nahe natürliche Anlagen.
Der tatsächliche Autorenradius wird zusätzlich gegen die Abstandsgarantie geprüft.
Spätere Änderungen der Live-Regeln verändern kein bereits festgelegtes Terrain.
Serverprüfungen nutzen für Revision 9 eine begrenzte, gecachte Makrozell-Abfrage;
auch ein bei Erstellung gewählter Abstand von 4096 Chunks verlangt keinen flächigen
Scan aller Rohregionen. Extrem große nachträgliche Abstände in dicht generierten
historischen Welten können die feste Prüfkapazität überschreiten; die Transaktion
wird dann vollständig abgelehnt.

Protokoll **110** überträgt die neue Hierarchie und die konkreten Pflanzenfehler.
Index **ID12** speichert Elternzuordnung; explizite Legacy-DTOs lesen ältere
IDX8/IDX9/ID10/ID11-Daten einschließlich alter unabhängiger Dörfer und Städte.
Baumdaten und PLY7 bleiben erhalten.

## Prüfung

4.097 Workspace-Library-/Bin-Tests bestanden, keiner fehlgeschlagen; neun bestehende
Tests ignoriert, verteilt auf zwölf Testsuiten. Der vollständige Lauf verwendet
`cargo test --workspace --lib --bins --locked --no-fail-fast -j1 -- --test-threads=4`.

- Clippy aller Targets mit `-D warnings`: bestanden.
- Formatprüfung und `git diff --check`: bestanden.
- Alle 60 tatsächlichen UI-Stylesheets mit dem TiltUI-Parser: bestanden.
- Abschließender Workspace-Build der Server-/Clientdateien: bestanden (2m 08s).

Die Regressionen prüfen unter anderem echte UDP-Transaktionen, die Gründung und
Aufwertung von Untergebieten, Entwurfsbindung trotz Zielwechsel/Auflösung, den
pflichtigen geschützten Seat-Claim, eigene/fremde Rollen, die Konfiguration aller
20 Felder durch echte Menüaktionen, Neustarts mit gespeicherter Policy sowie
ID12- und Legacy-Migrationen. Wasserprüfungen untersuchen reale Fluidmesh-Patches
über positive und negative Sektionsgrenzen. Das F2-Verhalten wird anhand der
vorhandenen/entfernten Hand-Entities und der unveränderten Charakteransicht geprüft.

Die zuerst gefundenen Fehler und ihre Korrekturen bleiben in den diagnostischen
`*-initial`, `*-first`, `*-second`, `*-third` und `*-fourth`-Logs nachvollziehbar.
Die abschließenden Logs und Quellhashes stehen in `verification.json`.
