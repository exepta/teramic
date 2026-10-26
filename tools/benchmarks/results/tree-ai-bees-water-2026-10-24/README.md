# Baumfall, Mob-KI, Trefferboxen, Wasser Und Bienen

Stand: 2026-10-24, Phase 121. Ausschliesslich tatsaechliche Testlaeufe und
Beobachtungen; keine Langzeit- oder erfundenen FPS-Messungen.

## Baumfaellen

Der bestehende Survival-Ganzbaumabbau mit Sneak benoetigt weiterhin 0,5 Sekunden
pro Holzblock. Erst der autoritativ angenommene Abschluss erzeugt eine begrenzte
kosmetische Momentaufnahme fuer nahe Clients. Abgebrochener Abbau erzeugt keine
Fallanimation. Holz-/Laub-Edits, Drops, Besitzerschutz und Speichern bleiben im
bisherigen serverseitigen Felling-Queue-Pfad.

Ein Hintergrundjob erzeugt hoechstens fuenf Hoehenbaender mit getrenntem Holz und
Laub, gebuendelt nach den vorhandenen Materialslots. Der Baum kippt vom Spieler
weg, bricht in geschlossene Abschnitte und laesst seine Krone nachfallen. Die
Fragmente werden am geladenen Boden abgefangen und schrumpfen zum Abschluss aus.
Es gibt keine Entity pro Baumblock und keine zusaetzlichen Gameplay-Collider.

Temporare Terrainmasken entfernen nur die zugehoerigen natuerlichen Baumzellen
aus der Darstellung, bis die normalen autoritativen Section-Meshes nachgezogen
haben. Zwischenzeitliche Spielerbloecke bleiben geschuetzt. Streaming, ein neuer
Section-Root, Weltwechsel und Animationsende haben explizite Cleanup-Pfade.
Maximal vier kosmetische Arbeiten und ein Mesh-Worker sind gleichzeitig aktiv.
Keine unbeschraenkten Mesh-/Material-Caches; temporare Meshes haben ausschliesslich
ihre Renderkomponenten und begrenzten Animationsrecords als Besitzer.

## KI Und Trefferboxen

- Land-/Flugnavigation verwendet die individuelle Groesse einschliesslich
  Nachwuchs statt immer die groesste erwachsene Artgroesse. Junge Pferde koennen
  dadurch tatsaechlich passende niedrige Passagen benutzen.
- Ungueltige oder unbeschraenkt entfernte Ziele gelangen nicht in unbeschraenkte
  Streckensweeps. Eine lokale Kante hat maximal 64 Proben / 11,52 m, A*-Scores
  verwenden saettigende Arithmetik. Ungueltige Ziele verwerfen nur die jeweilige
  Route, nicht die Population.
- Spinnen wechseln von Wand-/Deckenorientierung nur mit gepruefter freier
  Koerperposition zur Bodenpose. Supportverlust loest Neuorientierung/Fallen aus,
  kein Rotieren des Koerpers in benachbarten Stein. Nichtendliche Oberflaechenziele
  gelten nicht mehr als erfolgreich gepruefte Sichtlinie.
- Beim Pferd pruefen Reiten und Sprung dieselbe physische Hufunterstuetzung.
  Treppenkanten fuehren dadurch nicht mehr allein wegen einer strengeren
  Pfadprobe zu abwechselndem Grounded/Leap. Ungueltige ballistische Zeitschritte
  oder Geschwindigkeiten werden vor einer Teilmutation abgewiesen.
- Der Stalker kodiert beim ebenerdigen Treppensteigen keinen Hoehenversatz mehr
  als `1 / dt`-Vertikalgeschwindigkeit (60 m/s bei 60 Hz). Das haette die gueltige
  Creature-Geschwindigkeitsgrenze verletzt und folgende KI-Schritte scheitern
  lassen. Nach blockiertem Ziehen werden alte Geschwindigkeit, Griff und
  Wiederangriffszeit sauber zurueckgesetzt. Beobachterposition/Blick werden
  validiert, bevor sie Wahrnehmung und Ziele beeinflussen.
- Gemeinsame anatomische, gedrehte Teilboxen ersetzen den quadratischen
  `max(Breite, Laenge)`-Trefferkasten. Koerper, Kopf und Gliedmassen von Pferd,
  Stalker, Spinnen und Huftieren werden getrennt getestet; Alter, individuelle
  Groesse, Yaw und Spinnen-Anhaftung skalieren mit. Client und Server nutzen
  dieselbe Ray-Funktion. Kein Heap-Array je Test, maximal 24 Teilboxen.
- F1+H zeigt die wirklichen Trefferteile weiss und den bewusst konservativen
  Navigationsraum grau. Der Navigationsraum wurde nicht beliebig verkleinert.

Die Teilboxen sind anatomische starre Annaeherungen, keine per-frame GPU-Skinning-
oder vertexexakte Extremposen. Ohne Absturz-Stacktrace wird keine vollstaendige
Beseitigung jeder unbekannten Crashklasse behauptet; die genannten konkreten
Fehlerpfade sind durch gezielte Regressionen abgesichert.

## Wasser Und Hoehlen

Neue Welten verwenden Cave-Strukturrevision 8 mit moderat breiteren Verbindern
(Faktor 1,12 gegenueber Revision 7). Gespeicherte Revisionen 0 bis 7 bleiben
unveraendert; bestehende Welten werden nicht still regeneriert.

Die verbliebenen natuerlichen Wasserfall-Curtains wurden durch Aktivierung
wirklicher Quelllippen in der bestehenden Block-Flusssimulation ersetzt.
Horizontale Reservoirs bleiben bestehen. Nur der Section-Eigentuemer der Quelle
weckt seinen Abfluss bei Generierung; Remeshing weckt ihn nicht wiederholt.
Feste Spielerbarrieren an Quelle/Lippe haben Vorrang. Trockene Luft besitzt keinen
erfundenen Wasserfall-Current mehr. Der leisere Wasserfall-Sound folgt echten
geladenen fallenden Fluessigkeitszellen und endet bei Entwaesserung oder Unload.

Auch natuerliche abrupte Lippen von 1 bis 4 m aktivieren die Blockmechanik.
Reservoir-Topflaechen werden an derselben 1-m-Grenze getrennt, damit keine
diagonalen Wasserplatten ueber der wirklichen Fallsaule liegen bleiben.
Sanfte Flussgefaelle unter 1 m bleiben glatt. Ein voller Wasserschacht wird nur
bei echter verbundener Fallkante als Abwaertsstroemung behandelt, nicht bloss
wegen einer weiteren Wasserzelle darueber; ruhige tiefe Becken ziehen Tiere
nicht nach unten.

Bestehende Fluessigkeitsgrenze: Leere Fluid-Overrides werden nicht als dauerhafte
Trockenmarkierung gespeichert. Ein natuerliches Reservoir laesst sich deshalb
nicht allein durch einen Air-Override dauerhaft austrocknen. Feste Barrieren
funktionieren; entfernte explizite Quellen ausserhalb natuerlicher Reservoirs
bleiben entfernt. Eine Tombstone-/Entnahmeerweiterung waere Teil des spaeteren
Wassertransport-Systems, nicht eine zweite Simulation in dieser Aenderung.

## F12 Bienen Und Honig

- Platzieren/Benutzen am echten Nest entnimmt reifen Honig in das Inventar.
  Der Server prueft konkrete Prop-Identitaet, Bauberechtigung, Spielermodus,
  Reichweite, Sichtlinie, geladene Zellen und Inventarplatz. Kein dekorativer
  Prop-Drop wird als Honigbestand ausgegeben; das Nest bleibt erhalten.
- Echte Nektarbesuche bestaeuben bis zu vier geeignete feuchte Nutzpflanzen in
  begrenzter Naehe mit jeweils fuenf Sekunden Wachstumsfortschritt. Keine
  rein kosmetische oder permanent wiederholte Wachstumsverstaerkung.
- Gesunde erwachsene Bienen unterschiedlichen Geschlechts koennen im echten
  geladenen Nest Nachwuchs bekommen. Verwendet werden die bestehenden F06-IDs,
  Eltern, Geschlechter, Wachstum und Abklingzeiten. Nachwuchs waechst 1200 Sekunden;
  Eltern haben 300 Sekunden Abklingzeit.
- Mindestens 200 Nektar sind erforderlich, 100 werden verbraucht, mindestens
  100 fuer die unveraenderte Honigreifung bleiben zurueck. Maximal zehn Bewohner
  je Nest, zwei Geburten je Durchlauf und 16 rotierende Nestpruefungen. Das
  bestehende lokale natuerliche Moblimit bleibt massgeblich.
- Keine neue Prop-/Item-Registry und keine neuen Save-Felder. Bestehende Nektar-,
  Honig-, Nest- und Familienzustaende bleiben im bisherigen Speicherformat.

## Kompatibilitaet

Protokoll 71 ergaenzt Nestbenutzung und den kosmetischen Baumfall. Client und
Server muessen gemeinsam aktualisiert werden. Bestehende Block-/Welt-IDs,
Spielerorte, alte Cave-Revisionen und Saveformate bleiben erhalten.

## Validierung

Alle folgenden Befehle waren erfolgreich. Schwere Cargo-Laeufe liefen einzeln
mit `CARGO_BUILD_JOBS=1`, ohne parallelen nativen Client:

```sh
cargo fmt --all --check
cargo check --locked --workspace --all-targets
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo test --locked --workspace --no-fail-fast
cargo build --locked --workspace
```

Workspace: **2290 bestanden, 0 fehlgeschlagen, 3 bestehende Opt-in-Tests
ignoriert**. Ergebnisse in [tests.log](tests.log), [check.log](check.log),
[clippy.log](clippy.log), [build.log](build.log) und [fmt.log](fmt.log).
Der vollstaendige Build umfasst Client und Dedicated Server.

Die gezielten Regressionen pruefen unter anderem:

- Echte UDP-Ganzbaumaktion: exakte Abbaudauer, Abbruch ohne Fallereignis,
  begrenzte Replikation und unveraenderte Drops.
- Geschlossene Bruchflaechen, gueltige Mesh-Indizes, Terrainmasken mit spaeteren
  Spielerersatzbloecken sowie wirkliche ECS-/Handle-Freigabe bei Unload,
  Weltwechsel und veralteten Hintergrundjobs.
- Anatomische Pferdetreffer statt leerer quadratischer Randflaechen,
  Jungtiergroessen und gedrehte Wand-/Decken-Spinnentreffer.
- Pferde-Hufkontakt, echte Spruenge, Stalker-Treppen bei 60 Hz,
  ungueltige Ziele, begrenzte Pfadsuche und freie Spinnen-Posenwechsel.
- Natuerliche Quelllippen ueber Section-Grenzen, kleine Fallkanten, gespeicherte
  Wasserzustaende, Barrieren und ruhige Becken ohne falschen Abwaertssog.
- Honigentnahme ueber echte UDP-Verbindung, fehlende Rechte, Sichtblockade,
  volles Inventar, Nestbestand und Replikation zum zweiten Client.
- Bestaeubung nur geeigneter feuchter Pflanzen, Nest-/Populationslimits,
  Geschlechter, Nektarkosten, Elternabklingzeiten und Save-Roundtrip.

### Kurze Native Tests

Getestet mit der tatsaechlichen **RX 6950 XT / RADV / Vulkan**, isolierten
Konfigurations- und Speicherpfaden und dem optimierten Dev-Build. Keine
bestehende Spielerwelt wurde als Testwelt veraendert. Keine 10-, 15- oder
30-Minuten-Tests und keine Aussage ueber Langzeit-FPS.

| Test | Ergebnis |
|---|---|
| `--smoke-test-tree-collapse` | Erfolgreich, rund 9 s: echter generierter Baum bei `[39, 16, 10]` mit 239 Holz- und 651 Laubzellen. Vorherbild, sichtbares Kippen/Bruch und leerer Endzustand; anschliessend keine Baum-Queue, Terrainmaske, temporaere Entity oder verbleibende Meshbesitzer. |
| `--smoke-test-waterfalls` | Erfolgreich, rund 20 s: echte Fluessigkeitszellen ueber zwei Sections und Teilblockstufen; nach geschlossenem Zulauf sank das nachgelagerte Volumen von 959 auf 477, ohne neue nachgelagerte Quellen. |
| `--smoke-test-riding` | Erfolgreich, rund 8 s: echtes UDP-Aufsteigen, Vorwaertsbewegung, Shift-Sprint, Sprung und X-Absteigen. Vorhergesagte Sprunghoehe 2,016 m, autoritativ 2,014 m. |
| `--smoke-test-spiders` | Erfolgreich, rund 25 s: drei Modellgroessen, Wand-/Decken-/Abseilposen und zehn Netztemplates. Dies ist eine Modellgalerie, kein Ersatz fuer KI-Simulationstests. |
| `terra-server --no-save --smoke-test` | Erfolgreich, 27 Sections, ohne Fenster oder Renderer. |

Aufnahmen wurden tatsaechlich geoeffnet und kontrolliert:
[Baum vorher](native/tree/tree-before.png),
[Kippen](native/tree/tree-tilting.png),
[Zusammenbrechen](native/tree/tree-fracturing.png),
[Abgeschlossener Baumabbau](native/tree/tree-cleared.png),
[Wasserfall](native/water/waterfall-stepped-overview.png),
[Teilblockstufen](native/water/waterfall-partial-step-close.png),
[Reitersprung](native/riding/riding-sprint-jump.png),
[Spinnennetze](native/spiders/spiders-7.png).

Logs: [Baum](native-tree.log), [Wasser](native-water.log), [Reiten](native-riding.log),
[Spinnen](native-spiders.log), [Headless-Server](headless-server.log).
Im Wasser-Test meldete der Screenshot-Kanal einmal beim Prozessende
`Failed to send screenshot: sending on a closed channel`, nach beiden
erfolgreich gespeicherten Aufnahmen und abgeschlossenem Simulationstest.
Kein Panic oder Slab-Allocator-Fehler trat in diesen Laeufen auf.

Die Baumfixture verwendet eine echte, unveraenderte vorbereitete Baumstruktur
und startet deren bestehenden Queue-/Renderpfad direkt. Die komplette
0,5-s-je-Holzblock-Miningdauer wird separat im echten UDP-Test geprueft, nicht
im kurzen Grafiktest abgewartet. Ausschliesslich die Testszene hat duennere
Baumdichte und eine gepruefte freie Kamera. Vier Aufnahmen vergleichen jeweils
4096 Bildproben der projizierten Baumregion mit dem Vorherbild: beim Kippen
1127, beim Bruch 2017 und nach Entfernen 1234 veraenderte Proben. Die Bilder
wurden zusaetzlich angesehen; Bilddifferenzen allein gelten nicht als
Beweis einer gelungenen Animation.
