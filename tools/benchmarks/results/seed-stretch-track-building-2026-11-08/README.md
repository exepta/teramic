# Kurzes Startgleis und anschließendes Strecken

Implementierungsbericht vom 08.11.2026 zum gewünschten Bauablauf mit einem
**einen Meter langen, drei Blöcke breiten Startgleis** und anschließendem
Strecken der Verbindung. Baut auf
[Rampen und Kollisionsdaten](../track-ramps-input-collision-2026-11-08/README.md) auf.
Die zentrale CPU-Abnahme umfasst 4.258 bestandene Tests, keinen Fehler und
neun bestehende ignorierte Tests. Keine Grafiktests, Spielclients, GPU-Tests
oder Screenshots wurden gestartet.

## Bedienung

1. Mit dem Track-Item auf eine geeignete Bodenfläche zeigen und platzieren.
   Die Vorschau und das tatsächlich gesetzte Startgleis belegen genau 1×3 Zellen.
2. Nach bestätigt aufgebautem Startgleis bleibt sein freies Ende ausgewählt.
   Zum Ziel gehen und eine geeignete Bodenfläche oder ein anderes freies
   Gleisende anvisieren. Die gesamte Verbindung folgt als zusammenhängende
   Vorschau dem Ziel: gerade, gebogen oder mit sanfter Steigung.
3. Mit der nächsten Platzierung die vollständige Vorschau bauen. Nach der
   Bestätigung ist das neue freie Ende zum direkten Weiterbauen ausgewählt.
   Beim Anschluss an eine vorhandene Strecke endet diese Auswahl.

Ein Klick auf ein vorhandenes freies Gleisende wählt es als Start. Er setzt
keine automatische Viermeter-Verlängerung. **Shift + Platzieren** löst die
Auswahl. Auto/Kurve behalten beim Formwechsel einen bestätigten Start;
Prellbock beendet die laufende Verbindungsauswahl. Die Hinweise zeigen die
tatsächlich konfigurierten Tasten, den Materialbedarf und einen kurzen Status.

Eiche, Fichte, Birke und Kirsche bleiben unterstützt. Der Start kostet ein
Track-Item. Die bisherige Mengenregel für Verbindungen bleibt erhalten:
aufgerundet ein Item je vier Meter räumlicher Pfadlänge. Vorschau, Verbrauch
und späterer Abbau verwenden dieselbe Geometrie. Eigenständige Weichen und
Steller bleiben verfügbar; neue Kreuzungen bleiben gesperrt.

## Bestätigung und Sicherheit

Eine lokale Bauabsicht besitzt einen eindeutigen Token. Erst beim tatsächlichen
Versand wird dieser einer konkreten Netzwerkanfrage zugeordnet. Der Spieler
wartet auf deren Annahme und auf die dazu passende vollständig replizierte
Baugruppe einschließlich Holzart und genauer Anschlüsse. Erst dann wird das
neue freie Ende ausgewählt. Es gibt keine geratenen Anker und keine lokalen
Welt- oder Inventarschreibvorgänge zur Vorwegnahme eines angenommenen Baus.

Doppelte Platzierung während einer ausstehenden Bestätigung wird unterdrückt.
Abbruch, Itemwechsel, gesperrte Eingabe, Sitzungswechsel und veränderte Quellen
verhindern, dass eine verspätete Antwort die alte Auswahl wieder aktiviert.
Ein abgelehnter Verbindungsbau erhält den bestätigten Start für einen erneuten
Versuch. Ausstehende Bestätigungen haben eine begrenzte Lebensdauer.

**Protokoll 115** ergänzt `SeedRail`. Die Authority rekonstruiert das Startgleis
aus dem tatsächlichen Treffer, Blick, ausgewählten Item und der Holzart.
Ein Fingerabdruck bestätigt die Bauabsicht. Reichweite, Unterbau, sämtliche
belegten Zellen, Rechte, Props, Spielerkörper und Inventarkapazität werden vor
der bestehenden atomaren Transaktion geprüft. Die Verbindungsplanung verwendet
weiter die gemeinsamen kubischen Kurven, maximal 32 Meter Länge, mindestens
2,5 Meter Radius und höchstens 50 Prozent Steigung. Der Zielklick bleibt auf
4,5 Meter Reichweite begrenzt.

## Bestehende Welten

Neue Startgleise verwenden vorhandene dynamische Kontrollpunkte und Material
196. Alte gespeicherte Viermeterstücke behalten ihre Geometrie, Besitzerzellen,
Kosten, Kollisionen und Streckenanschlüsse. Native Profile und bisherige
untergeordnete Bau-APIs bleiben für gespeicherte Formen und die Kompatibilität
erhalten. Der neue Spielerablauf verwendet ausdrücklich Startgleis und
zielabhängige Verbindung. **RG13**, **ID12**, **PLY7**, vorhandene Material-IDs,
Rezeptzahlen und Holzdefinitionen bleiben erhalten.

## CPU-Abnahme

Alle zwölf Workspace-Library-/Bin-Suiten bestehen. Core und World prüfen die
exakten drei Zellen, alle vier Ausrichtungen und Holzarten, rote ungültige
Vorschauen sowie unveränderte Quellen beim Strecken von Gerade, Kurve und Rampe.
Die Player-Prüfungen verwenden echtes Targeting und prüfen beide Reihenfolgen
von Bestätigung und Replikat, doppelte Klicks, falsche Tokens, gleiche spätere
Bauabsichten, Zeitüberschreitung, Ablehnung, Abbruch und Itemwechsel.

Die Authority prüft 16 Kombinationen aus Holzart und Ausrichtung mit exakt einem
Item Verbrauch und Rückgabe sowie ungültige Signaturen, fehlenden Bestand und
fremde Randzellen ohne Teiländerung. Ein echter UDP-Lauf verbindet zwei Clients,
baut ein Fichten-Startgleis und eine sieben Meter lange Fortsetzung mit realen
Blickstrahlen, ordnet beide lokalen Tokens ihren Anfragen zu und prüft einen
späteren Beitritt einschließlich Geometrie, Holz, Kosten und Streckennetz.

Clippy aller Targets mit `-D warnings`, Formatierung, Diff und kopfloser
Assetexport sind sauber. Der vollständige Workspace-Build erstellt aktuelle
Client-/Serverdateien. Ergebnisse und Quell-/Logprüfsummen sind in
`verification.json` festgehalten.
