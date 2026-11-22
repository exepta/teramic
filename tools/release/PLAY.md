# Teramic

Entpacke das vollständige Archiv in einen beschreibbaren Ordner. Starte im Ordner `terra-client` für das Spiel, `terra-server` für einen Dedicated Server oder `terra-building-editor` für den Gebäudeeditor. Unter Windows enden die drei Programme auf `.exe`.

Der Server legt seine Welt beim ersten Start an. Mit `terra-client --connect 127.0.0.1:5000` verbindest du dich lokal. Der Konsolenbefehl `stop` speichert und beendet den Server. Ein mitgeliefertes Gebäude lässt sich mit `terra-building-editor --load assets/buildings/templates/village/residential.json` öffnen.

Einstellungen und Profile liegen in `configs/`, Welten in `worlds/`. Sichere diese Ordner beim Versionswechsel. Unter Linux benötigen Client und Editor X11 oder Wayland, einen Vulkan-Treiber, ALSA, Fontconfig und xkbcommon. Hinweise zur Version stehen in `RELEASE-NOTES.md`, die Lizenztexte in `LICENSE-MIT`, `LICENSE-APACHE`, `licenses/` und `assets/audio/`.
