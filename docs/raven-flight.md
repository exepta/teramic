# Raben: Reiseflug über den Baumkronen

Raben verwenden einen eigenen Flugpfad in
`crates/terra-world/src/creatures/spirit/raven.rs`. Der bisher gemeinsam mit
Schmetterlingen verwendete Schwebezustand senkte sie kontinuierlich ab und hob
sie erst unmittelbar über dem Boden wieder an. Schmetterlinge und Geister
behalten ihren bisherigen Bewegungsablauf.

Ein Rabe strebt beim Reiseflug einen individuellen Abstand von 12–20 Blöcken
über dem erkannten Gelände, Baumkronen oder der Wasseroberfläche an. Eine kleine
Höhenbewegung von höchstens 0,45 Blöcken kommt hinzu. Vorausschauende Proben
berücksichtigen höhere Bäume in Flugrichtung und in der unmittelbaren Umgebung.
Auch Spielerbauten und installierte Änderungen beeinflussen die Flughöhe.

Die Bewegung steigt höchstens 2,6 Blöcke pro Sekunde und sinkt höchstens 1,4.
Vollständige gedrehte Körper werden entlang der Bewegung und Drehung geprüft.
Es gibt keine Höhenteleports. Hindernisse führen zu begrenzten Steig- oder
Seitwärtsversuchen. Der Vogel hält sich ungefähr im Umkreis von 28 Blöcken um
seinen Habitatmittelpunkt auf und dreht bei größerem Abstand wieder zurück.
Ein aus Identität und gespeichertem Alter berechneter Wegpunkt bleibt acht
Sekunden bestehen. Die begrenzte Drehung verfolgt ihn über mehrere Ticks;
unterschiedliche Simulationsschrittgrößen ändern dadurch nicht die Zahl der
ausgeführten Kursänderungen.

Landepausen sind selten: ungefähr alle zwei Minuten wird eine erreichbare,
trockene Standfläche gesucht. Der Rabe sinkt kontrolliert, ruht dort drei bis
fünf Sekunden und steigt anschließend wieder auf. Fehlende Unterstützung,
blockierte Landung oder eine nahe Bedrohung brechen die Pause ab.

Alle Proben lesen ausschließlich geladene Chunks einschließlich des Journals.
Pro Bewegung werden höchstens zehn Spalten geprüft, jeweils maximal 64 Zellen
nach unten und 48 nach oben; es werden keine Chunks angefordert und keine
Chunks synchron nachgeneriert. Wasserhöhen verwenden die vorhandene
Wasserspalte der geladenen Welt. Ihre natürlichen Höhen werden pro geprüfter
X/Z-Spalte höchstens einmal aufgelöst; vorhandene vorbereitete Spalten haben
Vorrang. Auch ohne diesen Cache findet keine erneute Noise-Abfrage je Luftzelle
statt. Individuelle Flüssigkeitsänderungen bleiben pro Zelle wirksam.
Unbekannter Luftraum begrenzt
die erreichbare Flughöhe. Unter einem geschlossenen Dach kann ein Rabe deshalb
nicht seine normale Reisehöhe erreichen. Die normalen Streaming-Radien müssen
ausreichend Luftraum laden.

Position, Alter, Verhalten und Timer verwenden die vorhandenen Speicherfelder.
Bereits gespeicherte oder neu gespawnte Raben steigen aus ihrer tatsächlichen
Position auf. Eine Weltrevision, Protokolländerung oder Save-Migration ist nicht
erforderlich.

Gezielte Simulationstests prüfen Aufsteigen und anhaltenden Flug über echten
Voxel-Baumkronen, horizontale Bewegung, Dächer und ungeladene Höhen, Landung mit
erneutem Abflug, Wasserflächen sowie deterministische Bewegung nach Save/Reload.
Weitere Regressionen prüfen anhaltende Drehung bei 0,05/0,1/0,2 Sekunden pro
Schritt und die begrenzte Wasserauflösung bei fehlendem Spaltencache.
