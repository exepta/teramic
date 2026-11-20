# Fantasy-Natur, Kreaturen, Umgebungsgeräusche und Straßen

## Natur und Laub

Neue Welten speichern Ecology 11. Monumentale, 18–25 Meter hohe geschwungene
Felsbögen und gespaltene, weit geneigte Hornspitzen verstärken die Fantasy-Silhouette.
Spirit Stone, Blood Stone, Sandstein oder vermooster Stein passen sie dem Biom an.
Die Formationen ergänzen die bisherigen Felsgruppen und asymmetrischen Überhänge.
Ihre einzelnen Füße passen sich dem tatsächlichen Gelände an; die Steine sind
gewöhnliche abbaubare Voxel. Gekrümmte Bänder und ungleichmäßige Pflanzengruppen
lockern die Verteilung auf. Pro 512-Meter-Region gibt es höchstens einen
Kandidaten für einen Bogen oder eine Hornformation; mindestens 349 Meter liegen
zwischen den Mittelpunkten. Gelände, Bäume und geschützte Bauflächen können
Kandidaten zusätzlich ausschließen. Kleine natürliche Felsgruppen bleiben erhalten.
Gespeicherte Revisionen 0–10 behalten ihre Generation
einschließlich vorhandener Spieleränderungen.

Neue Welten verwenden Landschaftsversion 9. Jedes Landbiom hat ein breites
Inneres um einen vom Seed bestimmten Mittelpunkt. Dazwischen mischen sich die
benachbarten Biome entlang gekrümmter Grenzen. Gebirge, Flussbetten und Buchten
behalten ihre lokalen Geländeformen. An einer Insel oder Landzunge kann der
sichtbare Anteil eines Bioms entsprechend schmal ausfallen. Beim Laden bestimmt
weiterhin die im Spielstand gespeicherte Version die gesamte Welt.

Seit Landschaftsversion 7 konzentriert ein großräumiges Regionsfeld
Gebirge auf seltenere zusammenhängende Bereiche. Außerhalb werden
die Profilgewichte auf Tiefland verteilt; Plains und Rolling Plains erhalten
sanftere Höhenverläufe ohne die zusätzlichen Fantasy-Felsschultern. Die
Biomgewichte folgen dabei den tatsächlichen Geländeprofilen. In drei geprüften
Seeds sinkt der Anteil bergdominierter Inlandproben von etwa 30 auf etwa
10 Prozent; dies ist eine Stichprobe, keine garantierte Flächenquote jeder Welt.
Explizit konfigurierte reine Bergwelten bleiben möglich.

Landschaft 6 führte asymmetrische, gezackte Bergkämme, erhöhte Felsschultern und
breite erodierte Täler ein. Version 7 behält diese Formen für Bergprofile.
Die Formen werden an zwischengespeicherten Feldpunkten vor Flüssen und Seen
berechnet. Hydrologie 6 ergänzt weit geschwungene Flussläufe, offenere Schluchten
und tiefere Wasserfallbecken. Flüsse bleiben mit ihren Seen oder Meeresmündungen
verbunden, fließen bergab und behalten begrenzte Nassbreiten. Gespeicherte
Landschaftsversionen 0–6 und Flussversionen 0–5 bleiben unverändert; die neue
Bergverteilung gilt für neu angelegte Welten.

## Wasserfälle und Wasserpflanzen

Die dynamische Wassersimulation führt fallende Säulen im vorhandenen Arbeitsbudget
priorisiert durch geladene Luft bis zum nächsten Hindernis fort. Fehlende Sections
werden weiterhin abgewartet. Uferwände bleiben innerhalb ihrer eigenen Wasserzelle;
umgedrehte oder degenerierte Seitenflächen werden verworfen. Der Wasserfall-Schaum
verwendet eine entfernungsabhängige Filterung gegen flackernde feine Muster.

Seagrass wird nur über dem tatsächlich vorhandenen, geeigneten Boden nach allen
Geländeoperationen erzeugt. Vertikale Segmente teilen dieselbe horizontale Phase.
Die vorhandene Stützprüfung repariert auch geladene, frei schwebende Altbestände;
bei unbekanntem Boden wartet sie auf die fehlende Section.

Laub verwendet in der Nähe einzelne gefaltete Blattmotive mit räumlich
versetzten Blüten, Äpfeln oder Zapfen. Je freier Seite werden zwölf Hauptmotive
und zwei zusätzliche Motive erzeugt; Kirschlaub benutzt vorwiegend vollständige
rosa beziehungsweise cremeweiße Blüten. Es gibt weiterhin echte Zwischenräume und
Überhang; verdeckte innere Seiten werden nicht erzeugt. Die elf bestehenden
Gruppentexturen sind nun vierteilige Motivbilder mit transparenten Zellrändern.
Block-IDs und Materialslots bleiben erhalten. Neue und bestehende Bäume nutzen
die Darstellung. [Geometrie, LOD und Grenzen](foliage-reference-surfaces.md).

Die Gras-Seitenkarten des Geisterwalds sowie von Blood Grass, Crimson Mycelium
und Blood Moss folgen derselben positiven Welt-Y-Projektion wie die übrigen
Würfelseiten. Dadurch schließt ihre Gras-/Mooskappe oben an die Deckfläche an;
Erde und Wurzeln liegen darunter. Die korrigierten Seitenbilder gelten auch
für vorhandene Blöcke und ändern keine Gelände-, Top- oder Bottom-Flächen.

## Kakteen

Kaktus 422 bleibt ein normaler fester Block. Frei sichtbare Seiten und Kronen
tragen echte elfenbeinfarbene Stacheln mit bis zu 16 cm Überstand. Die normale
Kollision verhindert das Durchlaufen des Stamms. Der Server prüft zusätzlichen
Stachelkontakt anhand real geladener Blöcke und bestätigter Änderungen.

Spieler benutzen eine runde Körperkapsel, Mobs ihre gedrehten anatomischen
Hitboxen. Ein bloßer Aufenthalt in der Umgebung reicht nicht aus. Kontakt kann
3 Basisschaden verursachen, höchstens einmal je 0,8 Sekunden pro betroffenem
Wesen. Creative, Spectator und Spawnschutz bleiben geschützt. Normale
Schadensregeln, Tod, Drops und die Synchronisierung gerittener Pferde gelten
weiter. Abbau entfernt den Kontakt sofort. Darstellung und Schaden gelten auch
für vorhandene Kakteen in älteren Welten.

## Sandwurm

`teramic::sand_worm` ist eine seltene feindliche Wüstenart mit 120 maximalen
Lebenspunkten. Neu erzeugte Würmer beginnen mit 120/120; bereits gespeicherte
Verletzungen und Todesfälle bleiben erhalten. Er verfolgt Ziele überwiegend
eingegraben und patrouilliert auch ohne Gegner langsam unter der Sandoberfläche.
Die bewegte, angehobene Sandspur bleibt dabei sichtbar; der Körper bleibt verborgen.
Eine echte Sanddecke über Sand oder natürlichem,
unverändertem Sandstein und ausreichend freier, geladener Raum sind Voraussetzung.
Damit wird die tatsächliche Wüstenschichtung akzeptiert, deren Sand schon nach
etwa anderthalb Metern in Sandstein übergeht. Anderer Stein, Wasser, Dächer und
vom Spieler als Untergrund eingesetzter Sandstein verhindern das Auftauchen.
Im eingegrabenen Zustand gibt es keine auswählbare Körper-Hitbox.

Der Angriff kündigt sich durch 1,8 Sekunden bebenden Sand und dumpfes Grollen
an. Ein naher Angriff steigt senkrecht aus dem gewarnten Loch auf; die vollständige
Aufstiegsanimation dauert 1,35 Sekunden. Der Wurm kann vier Angriffe einsetzen:

- **Bite:** Das Maul schließt nach sichtbarem Ausholen; der einzelne Treffer
  verursacht 6 Basisschaden und prüft die tatsächliche Maulposition.
- **Spit:** Eine sichtbare verwundbare Zielperson kann bis 40 Meter entfernt sein.
  Die Galle fliegt mit einer berechneten ballistischen Bahn zum festgelegten
  Zielpunkt. Der Wurm spuckt weder ohne Gegner noch durch verdeckendes Gelände.
- **Charge:** Nach der Warnung rast er höchstens 1,6 Sekunden mit etwa 14 m/s
  unter dem Sand auf einer festgelegten Linie. Ein Kontakt verursacht einmalig
  8 Basisschaden. Seitliches Ausweichen funktioniert; Stein, Wasser, Klippen und
  ungeladener Boden begrenzen die Bewegung.
- **Knockup:** Wer beim senkrechten Auftauchen unmittelbar über dem Loch bleibt,
  erhält einmalig 4 Basisschaden und einen Aufwärtsimpuls von 11 m/s. Der echte
  Spieler-Körper wird angehoben und über die gewöhnlichen Bewegungsschnappschüsse
  synchronisiert. Ein rechtzeitiger Ortswechsel vermeidet den Kontakt.

Der Wurm hört die Laufrichtung seiner Beute. Bei einer angekündigten Charge
kann sein Zielpunkt bis zu drei Meter vor dem letzten Schritt liegen. Wer nach
dem Grollen die Richtung wechselt, kann die festgelegte Angriffslinie verlassen.
Bei der Verfolgung versucht er, Hindernisse seitlich auf begehbarem Sand zu
umrunden. Ein Angriff auf Distanz beginnt mit erhobenem Kopf und gut einer
Sekunde Vorlauf für den Gallespeier. Im Nahkampf öffnet er mit dem Biss.

Unter 60 Lebenspunkten schleicht er seitlich an, zieht sich nach dem Biss früher
aus dem Nahkampf zurück und erholt sich unter der Erde schneller. Bei voller
Gesundheit dauert die Erholung sieben Sekunden. Ein Rückzug dauert 1,5 Sekunden.
Bricht die Sichtlinie nach dem Biss ab, kann er vorzeitig abtauchen. Creative-Spieler
und Spieler mit Spawnschutz bleiben vor Schaden und Hochwerfen geschützt. Segmentkörper, Maul
und Unterkiefer haben getrennte Animationsteile. Die Warnung verändert keine
Gelände- oder Baublöcke.

Art-ID 30 und die bestehenden Verhaltenswerte sowie das gespeicherte
Creature-Datenformat bleiben erhalten. Der unterirdische Charge verwendet den
vorhandenen Hunt-Wert; das gesperrte Angriffsziel verwendet das bestehende
AttackAim-Feld. Die tatsächliche KI läuft auf dem Server; die Galerie stellt
ausdrücklich nur ausgewählte replizierte Posen dar.

Sichtprüfungen verwenden geladene Block- und Objektkollisionen einschließlich
gedrehter Türen, dünner Bauteile und Straßenrampen. Ein naher verdeckter Spieler
verhindert nicht mehr die Auswahl eines weiter entfernten sichtbaren Gegners.
Verlorene Sicht führt bei Waldgegnern für höchstens vier Sekunden zur Suche an
der zuletzt gesehenen Position. Angriffe prüfen den tatsächlichen Angriffspunkt.
Der eingegrabene Sandwurm darf Erschütterungen wahrnehmen; sein Auftauchangriff
braucht eine sichtbare, erreichbare Zielperson.

Die natürliche Spawnplanung berücksichtigt auch Restbudgets von eins bis drei
Kreaturen und hält Abstand zu Spielern. Sandwürmer prüfen neun begrenzte
Alternativpositionen. Früher erfolglos besuchte, leere Wüstengebiete können ihren
natürlichen Wurm nachholen; gespeicherte lebende oder getötete Kreaturen werden
dabei nicht neu erzeugt.

## F68: Teer-/Asphalt-Straßen

Der angetriebene Brecher verarbeitet einen Stein in vier Straßenzuschlag (4 s)
und eine Kohle in einen Kohlestaub (4 s). Der Ofen verarbeitet einen Kohlestaub
mit zusätzlichem normalem Brennstoff in ein Kohlenteer-Bindemittel (8 s Grundzeit).
An der Werkbank ergeben vier Zuschlag, ein Sand und ein Bindemittel vier
Asphaltblöcke. TMI zeigt die neuen Materialien, Bauteile und Maschinenrezepte.

| Werkbankrezept | Ergebnis | Form |
| --- | --- | --- |
| 1 Asphalt | 8 Straßendecken | ⅛ Block hoch |
| 2 Asphalt | 4 Steigungen | 0 → 1 Block |
| 5 Asphalt | 16 untere Übergänge | ⅛ → ½ Block |
| 3 Asphalt | 4 obere Übergänge | ½ → 1 Block |
| 3 glatte Steinblöcke | 64 Randsteine | ¼ Block hoch, 3⁄16 Block breit |

Alle fünf Rezepte lassen sich mit genau umgekehrten Mengen an der Werkbank
rückverarbeiten. Die Formrezepte belegen unterschiedliche Rastermuster, damit
die automatische Rezeptauswahl jedes Bauteil eindeutig erkennt.
Die Folge Decke → unterer Übergang → oberer Übergang → Vollblock
verbindet die Höhen ohne sichtbare Sprünge. Asphalt und Randstein verwenden
wiederholbare 512×512-Texturen mit passenden gegenüberliegenden Bildrändern.
Beim Platzieren bestimmt die Blickrichtung eine von vier horizontalen Richtungen;
Rampen steigen vom Spieler weg an, Randsteine folgen längs der Straße.
Direkte Layouts und die Platzierung über den Boden-Ray verwenden dieselbe
Zuordnung. Mit dem Schraubenschlüssel lassen sich Straßenformen nachträglich
drehen; die Rückwärtsfunktion arbeitet wie bei anderen drehbaren Bauteilen.
Gespeicherte Ausrichtungen bleiben erhalten. Die Spitzhacke
liefert jeweils ein Exemplar derselben Form. Dünne Formen gelten nicht als
vollständiger Boden für Pflanzen oder Möbel.

Die sichtbaren Rampenflächen sind glatt; ihre gemeinsame Bewegungs-/Raycast-
Kollision verwendet zwölf konservative Stufenboxen. Die volle Steigung kann
dadurch bis zu etwa 8,33 cm, der untere Übergang 3,125 cm und der obere 4,167 cm
über der sichtbaren Fläche kollidieren. IDs 431–436 und die beiden Itemvarianten
sind angehängt; bestehende IDs bleiben stabil. Client und Server müssen für
diese Erweiterung beide Protokoll 148 verwenden.

## Biomatmosphären

Der gemeinsame Audiomixer kombiniert elf geglättete Ebenen. Ein Hörumkreis von
16 Metern mischt benachbarte Biome frühzeitig bei. Normalisierte Biomgewichte
und über mehrere Sekunden geglättete Klangenergie halten die Übergänge gleichmäßig.
Bis zu zwei Sekunden lange Loop-Überlappungen glätten Wiederholungsnähte.
Tag/Nacht, Höhe, Regen, Unterwasserzustand und die vorhandene
Höhlenraumprüfung beeinflussen die Auswahl am Körper des Spielers.

| Umgebung | Klanggestaltung |
| --- | --- |
| Blood Forest | ruhige Blattbewegung, wenig Vogel-/Insektenleben und entfernte Krähen |
| Geisterwald | leise natürliche Waldgeräusche, lange Ruhephasen und vereinzelte entfernte Krähen |
| Kirschwald | Vögel, Blattbewegung und sehr selten ein leiser kristalliner Akzent |
| Pilzbiom | zurückhaltende Blattbewegung und nachts stärkere Insekten |
| Wüste | trockener gefilterter Wind |
| Ozean | vier unterschiedliche leise Meereswellen |
| Hochland | zurückgenommener offener Wind mit eigenen Abspielphasen |
| übrige Biome | tagsüber Vögel, nachts Insekten, leise Waldluft |
| Höhlen | gedämpfte Raumluft, vereinzelte Details und Hall |

Die Windebenen sind stark zurückgenommen. Normale Augenhöhe aktiviert keinen
Höhenwind mehr; Geister- und Blutwald bleiben auch in größerer Höhe windstill.
Die Außenbiome verwenden keine synthetischen Dauerdröhnen, Akkordbetten oder
Atemphrasen mehr. Echte Vogel-, Insekten- und bewegte Blattaufnahmen bilden die
Basis. Die Blätter laufen im Originaltempo mit wechselnden langen Pausen;
kurze Blatt-Schrittgeräusche werden dafür nicht zweckentfremdet. Außenhall ist
stark reduziert. Sehr seltene, leise Einzelakzente geben dem Kirschwald seine
Fantasy-Farbe, ohne eine Dauermelodie über die Landschaft zu legen.

Umgebungsdetails haben wechselnde Pausen, Panoramen und kleine Tonhöhenvarianten.
Die verschiedenen Phasen beider Kanäle vermeiden einen rein mittigen Mono-Loop.
Höhlen unterdrücken die Außenkulisse. Der vorhandene Ambience-Regler, Master,
Mute und Menü-/Weltwechsel wirken auch auf die neuen Ebenen und Hallfahnen.
Es bleibt ein gemeinsamer Decoder mit dem vorhandenen begrenzten Stimmenbudget,
kein Player pro Blatt oder Geräuschquelle. Die zusätzlichen Klangbetten werden
einmal auf dem bestehenden Lade-Worker vorbereitet. Interpoliertes Resampling,
Butterworth-Filter und DC-Unterdrückung verbessern die Aufbereitung.

Geister-, Pilz-, Blut- und Kirschbiome erhalten zusätzlich leuchtende Irrlichter,
Sporen oder Glutpunkte. Sie steigen in langsamen Spiralen auf, pulsieren und
blenden mit Entfernung und Biomgewicht aus. Ein gemeinsamer Zeichenbatch zeichnet
je nach Laubqualität höchstens 48/80/112 Partikel; nur geladener freier Raum wird
verwendet. Dafür werden keine zusätzlichen Lichtquellen angelegt.

Die Gestaltung folgt der Idee von Regionen, Überblendungen und wechselnden
Details aus der [AmbientSounds-3-Dokumentation](https://github.com/CreativeMD/AmbientSounds/wiki/Outdated:-Creating-your-own-sound-engine-(for-AS-3)).
Die verwendeten Aufnahmen und Windklänge haben eigene freie Quellen;
es sind keine Audiodateien aus Minecraft oder dem Mod enthalten.
[Credits, Lizenzen und Reproduktion](../assets/audio/CREDITS.md).

Der Hall ist eine begrenzte gedämpfte Verzögerung, keine vollständige akustische
Raumsimulation. Die Natur bleibt voxelartig. Die Referenzbilder sind das
Gestaltungsziel und kein Nachweis pixelgleicher Grafik oder gleicher Leistung.

Der vorangegangene Ecology-10-Stand wurde mit 45 Audio-, 16 Gelände-, drei
Konfigurations-/Speicher- und vier Partikeltests geprüft. Native Aufnahmen zeigen
einen 22 Meter hohen und breiten Ecology-10-Bogen sowie animierte Geister- und
Kirschwaldpartikel. Der echte Audiosink lädt 124 Clips und konsumiert die
27 eingespeisten Materialkontakte; die zusammenhängende Decoder-Hörprobe führt
42 Sekunden durch sieben Biome.

Die anschließende Überarbeitung von Wind, Geisterwald, Gelände und Abständen
ist mit 410 Tests aus Audio, Konfiguration, Speicherung, Höhenfeldern,
Naturformationen und Hydrologie geprüft. Dabei wurden alte feste Suchgebiete
ausdrücklich auf ihre ursprüngliche Landschaftsversion 1 gesetzt; neue Fälle
prüfen die aktuellen Generationsversionen separat.
Der abschließend gebaute Client liefert drei native Aufnahmen von Bergen und
einem realen offenen Fluss mit geladenen Bett-/Luftproben in Version 6/6/11.
Der Entfernungsnebel ist ausschließlich in diesen isolierten Übersichtskameras
ausgeschaltet, damit die Geländeform erkennbar bleibt; dies ist kein FPS-Vergleich.

[Historischer Prüfbericht, native Bilder und Hörproben vom 16. November](../tools/benchmarks/results/fantasy-sandworm-ambience-2026-11-16/README.md).

Die anschließende Korrektur von Bergverteilung, Wasser, Kreaturen-KI und F68 ist
mit 5.254 erfolgreichen Workspace-Tests geprüft. Zehn bestehende Tests bleiben
ausdrücklich ignoriert; die fehlende externe Original-GLB-Fixture ist ausgefiltert.
Striktes Clippy und Builds für Client, Server und Building Editor bestehen.
Fünf native Läufe mit derselben Clientdatei liefern 17 geprüfte Aufnahmen.
Die Ebenen und der Fluss stammen aus unverändert generiertem Gelände 7/6/11;
die Straßen- und Wasserfallgalerien sind ausdrücklich gesetzte Prüfbauten.
Die Bilder prüfen Geometrie und Darstellung, keine zeitliche Flimmerfreiheit.

Reproduktion nach dem Build:

```sh
cargo test --workspace --lib --bins --offline -- --skip fiverr_grass_smoke::tests::original_glb_loads_through_the_named_reader_with_uvs_tangents_and_pbr_images
tools/benchmarks/run-roads-water-ai.sh --partial-water
tools/benchmarks/run-fantasy-landscape.sh
```

Die nativen Skripte verwenden eigene Konfigurationen, Profile und Welten unter
`target/` und schreiben dort Bilder, Logs und `verification.json`. Nur die
Übersichtskameras deaktivieren für die Geländeprüfung den Entfernungsnebel.
