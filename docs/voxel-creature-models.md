# Voxelmodelle der Bewohner und Kreaturen

Die Kreaturen werden aus gestuften, dreidimensionalen Oberflächen aufgebaut.
Körper, Köpfe, Gliedmaßen, Federn, Flossen und Flügel behalten ihre jeweilige
Silhouette. Augen, Fellfarben, Kleidung und Varianten werden beim Aufbau in die
Oberfläche eingearbeitet. Es handelt sich um eine Änderung der Darstellung:
Die Mobs werden dadurch weder zu abbaubaren Terrainblöcken noch zu einer eigenen
Voxel-Physiksimulation.

## Aufbau und Animation

Der gemeinsame Builder unter
`crates/terra-render/src/creatures/model/voxel/` rastert die Modellvolumen in
lokalen Koordinaten eines starren Gelenkteils. Boxen, gerundete Volumen und
Ellipsoide dienen als Modellierungswerkzeuge; das resultierende Mesh besteht
aus ebenen, achsenparallelen Außenflächen. Die Flächen drehen sich anschließend
mit dem jeweiligen Gelenk. Die vorhandenen Gelenkpunkte, Animationskanäle,
Arten, Varianten und Spiel-Hitboxen bleiben erhalten.

Überlappende Volumen überschreiben die Zellfarbe. Belegte Nachbarzellen
unterdrücken Innenflächen; benachbarte Außenflächen gleicher Farbe werden zu
Rechtecken zusammengefasst. Die indizierten Dreiecke besitzen flache Normalen.
Zellen ohne Volumen am abgeschnittenen Außenrand werden vor dem Nachbartest
entfernt, damit Rundungsfehler keine doppelten Deckflächen erzeugen.

Die Modelle werden beim Initialisieren nach Art und Variante aufgebaut und als
gemeinsame Mesh-Handles im vorhandenen `Models`-Cache gespeichert. Ein Mob
erzeugt keine Entity pro Modellzelle. Während der Animation werden die
Transformationen der vorhandenen Teile aktualisiert; die Skulptur wird nicht
für jedes Bild neu gerastert.

## Auflösung und Grenzen

| Modellbereich | Raster |
| --- | --- |
| Allgemeine Körper- und Gelenkteile | 24 Zellen entlang der längsten Ausdehnung |
| Spinnen- und Blood-Stalker-Beine | 16 Zellen entlang der längsten Ausdehnung |
| Spinnenkörper | 18 Zellen entlang der längsten Ausdehnung; geschlossener Hinterleib und schmalerer Vorderkörper |
| Pferdebeine und Schweif | 16 Zellen entlang der längsten Ausdehnung |
| Augen der fünf blinzelnden Säugerarten | Eigenes 12 × 12-Pixel-Raster mit geschlossener Tiefe |
| Skelett-Rumpf und Schädel | Rumpf 1,3 cm für offene Rippen innerhalb des bestehenden Vertexbudgets, Schädel 1 cm für vertiefte Augenhöhlen; Arme 32 Zellen entlang der längsten Ausdehnung |
| Skelett-Pupillen | Separates 4-mm-Raster, opakes rotes Leuchtmaterial |
| Bewohner | Bestehendes 2,5-cm-Raster; detaillierte Köpfe 2 cm |

Die äußeren Modellgrenzen werden auf die entworfenen Maße begrenzt, insbesondere
an Bodenkontakten. Kleine Details erhalten nicht pauschal die Auflösung eines
großen Rumpfs. Der Builder begrenzt jede Achse auf 256 Zellen; der gemeinsame
Oberflächenaufbau akzeptiert höchstens 262.144 belegte Zellen und 16.000
Außenrechtecke pro Teil. Diese Sicherungen ersetzen nicht die engeren
Modelltests: Die meisten vollständigen Modelle müssen unter 16.000 Vertices
bleiben, Bewohner unter 50.000 und der bestehende aufwendige Stalker unter
64.000. Flussfische haben zusätzlich ihr kleineres Budget.

Bereits voxelbasierte Bewohner verwenden denselben Außenflächenaufbau, behalten
aber ihre entworfenen Kleider- und Gesichtsdetails. Die kleine, separat
erzeugte Wappengeometrie der Soldaten behält ihre exakte 32-Pixel-Maske mit
6-mm-Pixeln und 4-mm-Relief, damit die feinen Reichswappen nicht durch ein
gröberes Körperraster verloren gehen. Der gemeinsame Außenflächenaufbau
entfernt dabei die unsichtbaren Kontaktflächen zwischen benachbarten
Glyphenzeilen. Die Stoffflächen bilden die vereinigte Kontur von Oberteil
und schmalem Saum ohne überschneidende Quader.
Getragene Gegenstände behalten ihre vorhandenen Itemmodelle.

## Augen-, Pferde- und Spinnenkorrektur

Schaf, Kuh, Schwein, Bär und Pferd besitzen den vorhandenen Lidkanal. Ihre
Pupillen und Glanzpunkte werden jetzt in einem kleinen eigenen Voxelraster
aufgebaut und anschließend in das gecachte Kopfmesh eingefügt. Zuvor konnte
ein winziger Glanzpunkt beim erneuten Rastern im gröberen Kopf fast die gesamte
Pupille übermalen. Die Lider stammen aus demselben Umriss wie das Auge und
liegen mit eindeutigem Tiefenabstand davor. Die kurze Schließ-/Öffnungsbewegung
verankert die Oberkante am Kopf, auch bei Kopfdrehung und Jungtierskalierung.
Andere Arten erhalten dadurch keinen neuen Blinkkanal; ihre vorhandenen Augen,
Leuchtaugen beziehungsweise insektenartigen Augen bleiben bestehen.

### Blood Slime und Skelette

Der Blood Slime verwendet für den Körper ein gemeinsames `AlphaMode::Blend`-
Material mit Alpha 0,70. Augen und Mund bleiben ein separates opakes Mesh;
beide Teile erhalten dieselbe volumenbewahrende Stauchung. Der Körper wirft
keinen irreführend vollständig opaken Schatten. Dies ist einfache transparente
Darstellung ohne Lichtbrechung oder Flüssigkeitssimulation. Instanzen und
Haustierportraits verwenden dieselbe Materialauswahl.

Die drei Skelettvarianten behalten ihre Arten-/Varianten-IDs und Spiel-Hitboxen.
Vier offene Rippenpaare, Schlüsselbeine, Halswirbel, einzelne Finger und Zehen
sowie vertiefte Augenhöhlen prägen die Silhouette. Kämpfer und Wächter tragen
ein gestuftes Schwert, der Wächter zusätzlich Plattenrüstung und Helm.
Die roten Pupillen besitzen ein eigenes kleines Mesh und folgen exakt der
Kopfdrehung; der Helm lässt die Augenhöhlen frei. Die Laufbewegung verwendet
eine stetige vertikale Bewegung und gegensinnige Schulter-/Beinschwingung.
Der Angriff geht mit geglätteten Übergängen durch Ausholen, Schlag und
Rückführung. Es werden ausschließlich die replizierten Bewegungs- und
Angriffsdaten dargestellt; Kampfentscheidungen und Schaden bleiben serverseitig.

Neue Regressionen prüfen die Sichtlinie zu beiden Pupillen aller drei
Varianten, die Materialtrennung beim Slime, identische Kopf-/Pupillentransformen,
die Angriffsübergänge und die bestehende Modellgrenze von 16.000 Vertices.
Die Waldgalerie zeigt vier Bilder mit näheren Skelett- und Slime-Ansichten.
Die neuen Modell-/Materialregressionen bestehen im zentralen Testlauf.
Aktuelle native Aufnahmen bestätigen die roten Pupillen, offenen Rippen
und den transparenten Körper mit opakem Gesicht:
[Prüfbericht](../tools/benchmarks/results/leaf-fog-ui-rivers-2026-11-15/README.md).
Die älteren Galerien weiter unten sind historische Nachweise ihrer jeweiligen
Modellstände.

Pferde behalten ihre 16 Fellvarianten, vier Beingelenke, Kopf-/Schweif-Pivots
und Spiel-Hitboxen. Brust und Hinterhand sind kräftiger, der Hals verjüngt sich
in einer gebogenen Kontur zum längeren Kopf. Fellmuster werden auf die
zusammenhängende Oberfläche gemalt, statt dünne seitliche Farbkacheln darüber
zu legen. Die maximale Brustbreite bleibt innerhalb der bisherigen
0,595-m-Referenzbreite. Mähne, Stirnhaar, Gelenkformen und flache Hufe sind
Teil der Darstellung. Neue Sattel-, Rüstungs- oder Reitmechaniken gehören nicht
zu dieser Korrektur.

Die drei vorhandenen Spinnengrößen besitzen einen geschlossenen gewölbten
Hinterleib, einen schmaleren Vorderkörper, größere primäre Leuchtaugen und
acht gebogene Beine mit drei sichtbaren Abschnitten und Panzerbändern.
Die vorhandenen Ansatzpunkte, Animationen und Hitboxen bleiben erhalten.
Die sechs Biom-Farbvarianten der Illustration werden damit nicht neu als
Gameplay-/Spawnvarianten eingeführt. Modellbudgets werden nicht angehoben.

Neue Regressionen prüfen echte sichtbare Pupillenflächen, Verdeckung durch
den Kopf und Abdeckung durch geschlossene Lider, den festen oberen Lidrand
beim Halbblink, ursprüngliche Gelenkpunkte, Pferdeproportionen und das
geschlossene Spinnenvolumen. Der opt-in-Modus
`TERRA_EYE_INSPECT=1 target/debug/terra-client --smoke-test-creatures` erzeugt
vier feste Aufnahmen (`open`, `half-blink`, `closed`, `after-blink`) mit dem
echten Animationssystem und den gecachten Kopfmeshes. Im Bild stehen links nach rechts
oben Schwein, Kuh, Schaf und darunter beide Pferde-Seitenansichten sowie Bär. `TERRA_CAPTURE_DIR`
legt das Ausgabeverzeichnis fest. Isolierte Konfigurations-/Weltpfade wie im
Galerieskript verwenden. Der Modus ist eine Darstellungskontrolle, kein
serverseitiger KI-Test.

Die nachstehenden 43 Bilder stammen **vor** dieser anschließenden Augen- und
Silhouettenkorrektur. Ihre Ergebnisse werden nicht als native Abnahme der
neuesten Änderungen ausgegeben; dafür sind ein neuer Build und neue Aufnahmen
erforderlich.

## Prüfung und Aufnahmen

Regressionstests prüfen unter anderem Achsen-Normalen im lokalen Gelenkraum,
Winding, doppelte Flächen, zusammengefasste Außenflächen, Farben, Bodenkontakte,
Gelenkpunkte, Geschlechter und Varianten sowie die vorhandenen Modellbudgets.
Für sämtliche Wappen im Katalog werden außerdem Pixel-Fläche, geschlossenes
Volumen und das Fehlen doppelter Dreiecke geprüft.
Die Ergebnisse des abgeschlossenen gemeinsamen Prüfstands stehen im
[Prüfbericht zu den Referenzbiomen](../tools/benchmarks/results/reference-biomes-f14-2026-11-14/README.md)
einschließlich der Modell- und Geometrieprüfungen.

Der abschließende native Lauf `target/voxel-creatures/run.fbMsGs/` ist nach den
Geometrie-, Budget- und Wappenkorrekturen erfolgreich durchgelaufen. Alle
**43 Aufnahmen** wurden einzeln visuell geprüft: 13 Standardtier-, vier Spirit-,
vier Wald-, 13 Wasserwesen-, vier Pferde- und fünf Bewohneransichten. Die
[archivierten Rohbilder und Logs](../tools/benchmarks/results/reference-biomes-f14-2026-11-14/mobs/)
enthalten keine Spielstände oder ausführbaren Dateien. Der
[Prüfbericht des Laufs](../tools/benchmarks/results/reference-biomes-f14-2026-11-14/mobs/verification.json)
belegt die erwarteten Abschlussmarker, Bildzahlen, 1920×1080 Pixel und SHA256.
Der Client blieb während aller sechs Galerien per SHA256 unverändert. Der
spätere Neuaufbau für die Biomauswahlfotos änderte keine Modellquellen.

Die Sichtprüfung zeigt gestufte Körperkonturen, intakte Rippenzwischenräume,
Flossen, Flügel, acht Blood-Stalker-Beine und verbundene Gelenkteile in den
geprüften Posen. Es waren keine offenen Flächen, fehlenden Materialien oder
explodierten Normalen erkennbar. Einzelne HUD-Elemente verdecken Teile der
Nahaufnahmen; der stark leuchtende Spirit-Fox-Schweif verliert lokale
Schattierungsdetails. Die Galerien zeigen ausgewählte Varianten und Posen,
keine vollständige Rundumprüfung jeder Art oder einen Nachweis für Navigation,
Kampf-KI und Schwimmverhalten. Die Wasserwesen stehen über einer Testfläche,
Pferde sind überwiegend rückseitig zu sehen. Bei Bewohnern werden zehn
repräsentative Rollen jeweils männlich und weiblich gezeigt.

Wiederholung mit bereits gebautem Client:

```sh
bash tools/benchmarks/run-voxel-creatures.sh
```

Das Skript verwendet isolierte Konfigurationen und Testwelten. Es startet
keinen Cargo-Build und hält inzwischen zusätzlich einen temporären Hardlink
auf die ausführbare Datei, damit ein paralleler Build keine Modellstände
innerhalb eines neuen Galerielaufs mischt.

### Historischer erster Galerielauf

Der native Galerielauf `target/model-galleries/run.MFCWSo/` enthält 30 tatsächlich
geprüfte Aufnahmen: vier Spirit-, fünf Bewohner-, vier Wald-, dreizehn
Wasserwesen- und vier Pferdeansichten. Die Aufnahmen zeigen Modellvarianten und
ausgewählte Ruhe-, Arbeits-, Schlaf- und Angriffsposen in isolierten
Testszenen. Wasserwesen stehen dort zur Modellprüfung über einer Testfläche;
dies ist kein Beleg für ihr Schwimmverhalten. Die Pferdeansichten zeigen
überwiegend die Rückseite und ersetzen keine vollständige Prüfung aller
Gesichter. Die Bewohnergalerie zeigt zehn repräsentative Rollen mit beiden
Geschlechtern, nicht jede verfügbare Berufskombination.

Dieser erste Galerielauf entstand **vor** der abschließenden Korrektur der
Spinnen-/Blood-Stalker-Geometriebudgets, der abgeschnittenen Randzellen und
der Wappen-Innenflächen.
Er ist daher ein historischer visueller Zwischenstand. Für die Abnahme des
endgültigen Binärstands gelten die späteren Ergebnisse und Aufnahmen im
Prüfbericht.
