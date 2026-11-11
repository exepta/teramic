# Gleisleger Und Automatische Beladung

Der eingeschaltete Freiraeumer arbeitet auf bestehenden Gleisen weiter. Eine
Verlaengerung wird erst gebaut, wenn deren Anschluss den lokalen Arbeitsbereich
erreicht. Der Arbeitsmodus begrenzt die Geschwindigkeit weiterhin auf 3 m/s.

## Schotterfundament

Fehlt direkt unter dem naechsten Gleisbett tragfaehiger Boden, plant der
Gleisleger eine Schottersaeule bis zum vorhandenen Boden. Jeder neu gesetzte
Schotterblock wird aus einem echten Zuglager bezahlt, ebenso das neue Gleis.
Geeignet sind normale Zug-Frachtmodule und montierte Truhen im erlaubten
Zugverband, keine Brennstoffinventare. Creative verbraucht keine Materialien.

Die Suche ist auf 32 neue Schotterbloecke pro Saeule und 2048 pro Bauplan
begrenzt. Nicht geladener Untergrund, Wasser, geschuetzte Objekte oder belegter
Raum brechen den Plan ab. Fehlende Materialien setzen weder Teilfundamente
noch Teilgleise; Lager, Raeumung und Gleisbau bleiben eine Transaktion.

## Abbau Ins Wagenlager

Im Fuehrerstandsmenue schaltet **Abbau ins Wagenlager** zwischen **Activated**
und **Deactivated** um. Die Lok muss stehen; nur der berechtigte, sitzende
Lokfuehrer kann den Modus aendern. Bestehende Spielstaende starten deaktiviert.

Im aktivierten Modus werden die echten Abbau-Ertraege in die Lager eines
zusaetzlich angekuppelten eigenen Gueterwagens eingelagert. Holzarten, Farben
und andere Item-Eigenschaften bleiben erhalten. Die Lok selbst und ihr
Brennstofflager werden nicht als Abbau-Ziellager verwendet. Die bestehende
Beschraenkung auf eine Arbeitslok mit hoechstens einem Gueterwagen bleibt.

Ist kein passender Wagen angekuppelt oder reicht dessen Platz nicht, bleiben
die nicht eingelagerten Ressourcen normale Drops in der Welt. Deaktiviert
verhaelt sich der Abbau wie zuvor. Creative erzeugt keine kostenlosen Ertraege.

Der Schalter gilt sowohl fuer Raeumung auf bestehenden Gleisen als auch fuer
Raeumung beim Bau einer Verlaengerung. Er wird serverautoritaer gespeichert
und repliziert; der bestehende Zuglagerdialog zeigt die bestaetigten Inhalte.

Neue Zugspeicher verwenden Index ID20; mit NPC-Auftraegen wird ID22 verwendet
(bisherige ID21-Auftraege werden migriert).
Die bisherigen Zugdaten bis ID19 werden explizit migriert. Client und Server
benoetigen gemeinsam Protokoll 135.
