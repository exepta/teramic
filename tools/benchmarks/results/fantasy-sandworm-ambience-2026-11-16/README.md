# Fantasy-Natur, Sandwurm, Laub und Biomatmosphären

Arbeiten vom 16.–17. November 2026. Die Änderungen verwenden den bestehenden
Voxelrenderer, die Serverautorität und den gemeinsamen Audiomixer.

## Umsetzung

Ecology 9 ergänzt große offene Felsbögen, geneigte Hornspitzen und asymmetrische
Pflanzengruppen. Einzelne Bodenfüße ersetzen eine rechteckige Fundamentfläche.
Die Formation ist normales, abbaubares Gelände. Gespeicherte Generationen 0–8
behalten ihr Gelände. Die neue Laubdarstellung und Kaktusstacheln gelten auch
in vorhandenen Welten.

Laub hat je freier Seite zwölf einzelne gefaltete Hauptmotive und zwei räumlich
vorgelagerte Blüten, Früchte oder Zapfen. Kirschlaub verwendet rosa beziehungsweise
cremeweiße Blüten. Elf vorhandene Texturpfade liefern
vier getrennte Motive je Bild. Nahe freistehende Zellen verwenden höchstens
504 Dreiecke/588 Vertices, Middle 168/336; Far/Low und Inventar bleiben bei
ihren bisherigen Darstellungen. Innere verdeckte Seiten werden weiter entfernt.
Das ist mehr Nahgeometrie als zuvor und keine Behauptung eines FPS-Gewinns.

Kaktus 422 erhält echte 3D-Stacheln. Der Server prüft tatsächlichen Kontakt
über geladene Blöcke und Journaländerungen. Spieler verwenden eine runde
Kapsel, Mobs ihre gedrehten anatomischen Hitboxen. 3 Basisschaden je 0,8 Sekunden,
Immunität für Creative/Spectator/Spawnschutz und normale Todes-/Drop-Regeln.

Der Sandwurm ist eine neue feindliche Wüstenart mit überwiegend eingegrabener
Verfolgung. Er warnt 1,8 Sekunden durch bebenden Sand und Grollen, taucht
0,85 Sekunden auf, schlägt einmal zu und zieht sich nach einer verwundbaren
Phase zurück. Der angekündigte Standort bleibt fest; rechtzeitiges Ausweichen
verhindert den Treffer. Dächer, Wasser, Stein und ungeladene Bereiche verhindern
das Auftauchen. Alte Artennummern bleiben erhalten; Protokoll 147 schützt die
neuen Arten-, Phasen- und Schadenswerte.

Neun weich überblendete Audioebenen und unregelmäßige entfernte Details bilden
Blood Forest, Geisterwald, Wüste, Ozean, Hochland, übrige Biome und Höhlen ab.
Tag/Nacht, Höhe, Regen, Unterwasserzustand und Höhlenraum wirken am Körper des
Spielers. Der gemeinsame Decoder behält die bestehenden Regler und das
begrenzte Stimmenbudget. Vier neue frei lizenzierte Eingaben und abgeleitete
Klangbetten sind in den [Credits](../../../../assets/audio/CREDITS.md) dokumentiert.

## Prüfungen und Grenzen

[Cargo-Prüfläufe](checks.json), [Testzusammenfassung](test-summary.json),
[vollständiges Testlog](tests.log), [letzte Modellpflege und Build](polish-checks.json) und
[native Prüfungen mit ausführbarer Datei und Bildprüfsummen](native-checks.json).
Der vollständige Workspace-Lauf vor der letzten Modellpflege besteht mit
**5.203 erfolgreichen Tests**, ohne Fehler. Danach bestehen erneut zwölf
Laub-, 35 Modell- und sieben Phasen-/KI-Tests sowie Check und striktes Clippy.
Zehn bereits ignorierte Tests bleiben ignoriert; eine weitere Prüfung benötigt
eine fehlende externe Original-GLB-Datei und ist ausdrücklich ausgefiltert.

Die Regressionen prüfen tatsächliche Sectionblöcke, reproduzierbare Bögen,
Abbau, alte Weltrevisionen, Kaktuskontakt, Immunität, Cooldown und einmaligen
Tod. Der echte UDP-Test mit zwei Clients verwendet normale Bewegungspakete:
beide Clients müssen dieselben Warn-/Angriffsphasen sehen, Creative bleibt
unverletzt und der Survival-Spieler kann dem zweiten Angriff ausweichen.
Die korrigierte Testfläche hält die Figur nach fünf Metern bestätigter
Bewegung an, damit nicht ein Sturz vom erhöhten Testboden das Ergebnis verfälscht.

Die Bilder sind native Modell-/Posen- und Weltaufnahmen. Die Sandwurmgalerie
erzwingt einzelne replizierte Posen und beweist selbst keine KI. Dafür ist der
getrennte Server-/UDP-Test zuständig. Die Naturgalerie prüft tatsächlich
generierte Gesteins-, Wurzel- und Luftzellen nach dem Streaming, ohne Szene-Edits.

- [Laub und Augen](captures/foliage/verification.json)
- [Sandwurm und fünf Phasen](captures/sandworm/verification.json)
- [Echter Bogen und Kaktus mit sichtbaren Stacheln](captures/nature/verification.json)
- [Birkenwald, Wald und Kirschwald](captures/biomes/verification.json)
- [Gemeinsamer Decoder und tatsächlicher AudioSink](captures/audio/verification.json)

Alle fünf nativen Läufe verwenden dieselbe abschließend gebaute Clientdatei.
24 Bilder dokumentieren den finalen Stand. Die 25 vorhandenen Weltdateien
besitzen nach allen isolierten Läufen dieselben Prüfsummen wie vorher:
[Weltintegrität](world-integrity.json), [vorher](worlds-before.json),
[nachher](worlds-after.json). Der [Abschlussstatus](completion.json) bündelt die
Prüfungen und Prüfsummen.

## Hörproben

Diese achtsekündigen Stereo-WAVs stammen aus dem tatsächlich verwendeten
Decoder einschließlich der neuen Audiobank und Effekte. Der Test prüft
endliche begrenzte Samples, unterschiedliche Signalenergie, Stereoausgabe,
Überblendung und wirksame Stummschaltung der Hallfahnen. Es sind kontrollierte
Offline-Mischungen und keine Aufnahme eines vollständigen Biom-Spaziergangs.
Die zusätzliche native Audio-Probe prüft die Wiedergabekette mit ausdrücklich
injizierten Materialkontakten. Eine subjektive Klangbewertung bleibt davon
getrennt.

- [Blood Forest](audio/blood-forest.wav)
- [Geisterwald mit Krähen](audio/spirit-forest.wav)
- [Wüste](audio/desert.wav)
- [Ozean](audio/ocean.wav)
- [Hochland](audio/highlands.wav)
- [Wald](audio/woodland.wav)
- [Höhle mit Hall](audio/caves.wav)

[Audiodaten und Prüfsummen](audio-proof.json),
[Blatt-Assettests](foliage-asset-tests.log),
[Geometrie-/PNG-Projektion](foliage-projection.log).

Die Referenzbilder bestimmen die Gestaltung, sind aber keine Behauptung
pixelgleicher Grafik. Blattmotive sind gefaltete Cutout-Flächen; der Hall ist
eine begrenzte gedämpfte Verzögerung. Die Galerien sind kein Leistungstest.

[Technik und Verhalten](../../../../docs/fantasy-nature-sandworm-ambience.md).
