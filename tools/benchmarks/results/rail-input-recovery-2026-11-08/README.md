# Gleisbau durch die eigene Figur und robuste Eingabebestaetigung

Korrektur vom 08.11.2026 fuer den gemeldeten TiltUI-Ladefehler und den
Disconnect nach rund 54 Sekunden (`sequence=3192`, `ack=3064`).
Keine Grafiktests, Spielclients, Screenshots oder GPU-Pruefungen werden gestartet.

## Ursache und Verhalten

Das neue Zug-Stylesheet enthielt `inset`, das der verwendete TiltUI-Parser nicht
unterstuetzt. Vier explizite Seitenabstaende ersetzen die Kurzform. Der neue
CPU-Check in `tools/ui/check_styles.rs` prueft alle 61 ausgelieferten Stylesheets
mit demselben Parser wie der echte Assetloader. Die Zugvorlage wird ebenfalls
in die bestehende kopflose Template-Ladepruefung aufgenommen.

Die bisherige Bewegungsabnahme verbrauchte auf dem Server hoechstens einen
Input pro festem Simulationsschritt. Bei nur serverseitig blockierten Schritten
blieb die Eingabe stehen, waehrend der Client weiter neue Schritte vorhersagte.
Danach gab es bei gleicher Taktfrequenz keinen Spielraum, diesen Rueckstand
abzubauen. Unabhaengige Clocks und bei langen Serverframes verworfene virtuelle
Zeit vergroesserten die Schuld. Die bisher ausgeglichen getakteten UDP-Tests
bildeten das nicht ab.

Der Client begrenzt seinen unbestaetigten Simulationsvorsprung jetzt auf ungefaehr
300 ms, abhaengig vom festen Takt und begrenzt auf 8 bis 64 Inputs. Eine volle
Vorhersage wartet auf bestaetigten Fortschritt, statt weitere Sequenzen zu
erzeugen oder die Verbindung wegen 128 wartender Inputs zu beenden. Bereits
simulierte Eingaben werden nicht verworfen. Bestaetigte Eingaben werden auch
dann aus der Warteschlange entfernt, wenn frische Kollisionen das eigentliche
Replay noch verzoegern. Der Server bestaetigt weiterhin nur wirklich simulierte
Schritte; es gibt keine zusaetzlichen autoritativen Bewegungs-Ticks.

Ein separater 30-Sekunden-Watchdog fuer fehlenden echten ACK-Fortschritt bleibt
aktiv. Er laeuft nach der Verarbeitung empfangener Snapshots, damit nach einer
Pause bereits gepufferte Bestaetigungen zuerst verarbeitet werden. Neue
Verbindungen, Teleports und eine neue leere Eingabewarteschlange setzen die
passenden Zeit-/Sequenzbezuege zurueck. Wartende Sprungimpulse bleiben erhalten.

Fertige Terrainkollisionen werden zusaetzlich vor der festen Simulationsschleife
uebernommen. Explizite Weltposition/-rotation und abgearbeitete Observer-Commands
machen sie bereits fuer die erste Abfrage im naechsten festen Schritt sichtbar.
Epoch-, Inhalts-, Profil- und Interessengebietspruefungen bleiben vollstaendig
erhalten; Avian wird nicht auf eine andere Schedule umgestellt.

## Gleisbau

Vorschau und Server lassen Gleise durch den eigenen Spielerkorpus zu.
Die Authority nimmt ausschliesslich die echte Entity des anfragenden Spielers
aus, der Client ausschliesslich die lokale Figur und ihr eigenes Replikat.
Andere Spieler bleiben auch an exakt derselben Position ein Hindernis.
Normale Bloecke, Rechte, Props, Materialverbrauch und signierte Plaene behalten
ihre bisherigen Regeln. Das Platzieren sendet keinen Teleport und verschiebt die Figur nicht seitlich.
Eine echte tiefe Schienenueberschneidung bei ruhender Figur kann der gemeinsame
Bewegungsmotor um hoechstens 1,02 Meter nach oben korrigieren. Er prueft aktuellen
Halt beziehungsweise autorisierten Schwebeflug, echte Physikueberschneidung und
freien Kopf-/Bewegungsraum. Normale Bloecke loesen diese Korrektur nicht aus.

## CPU-Abnahme

Die betroffenen Netzwerk-, Physik-, Player- und UI-Librarys werden mit echten
Client-/Serverfeatures geprueft. Ein neuer realer UDP-Lauf verwendet 3600
unabhaengig getaktete Frames, mehr als 54 simulierte Sekunden auf beiden Seiten,
Tempo 40, Baubursts und mehr als 128 nur serverseitig blockierte feste Schritte.
Er prueft begrenzte Warteschlangen, tatsaechliche Strecke je erzeugter Eingabe,
vollstaendiges ACK-Aufholen und fortgesetzte Bewegung.

Weitere Pruefungen decken verschiedene feste Takte und verzoegerte Snapshots,
erhaltene Sprungimpulse, Watchdog-Reihenfolge, die tatsaechliche SpatialQuery vor
dem ersten Avian-Durchlauf sowie verworfene veraltete Collider-Jobs ab.
Gleisfaelle nutzen echte Zielstrahlen fuer Start, Gerade, Kurve und Rampe,
unveraenderte Ablehnungen bei anderen Figuren und regulaeres physisches Setzen
um die eigene Figur. Die Grafik bleibt dabei ausgeschaltet.

Abnahme: **1251 Tests bestanden**, kein Fehler; ein bestehender Test bleibt
ignoriert. Der lange UDP-Lauf erreichte 60 simulierte Sekunden, 275 blockierte
Server-Ticks, 90 Blockaenderungen und maximal 18 offene Inputs. Die groesste
gemessene Positionsabweichung betrug 0,000079 Meter. Das ist ein CPU-/Netzwerk-
Regressionsnachweis, keine FPS-Messung.

Ergebnisse und Quell-/Logpruefsummen stehen in `verification.json`. Protokoll 116, Speicherformate, Rezepte und Material-IDs
bleiben erhalten.
