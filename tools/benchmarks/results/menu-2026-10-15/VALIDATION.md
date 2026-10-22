# Abschlussnachweis

15.10.2026, Linux, Rust 2024, vorhandene Dependency-Versionen unveraendert.

| Pruefung | Ergebnis |
| --- | --- |
| cargo fmt --all --check | erfolgreich |
| cargo check --locked --workspace | erfolgreich |
| cargo test --locked --workspace -j 2 | 212 bestanden, 0 fehlgeschlagen |
| cargo clippy --locked --workspace --all-targets -- -D warnings | erfolgreich |
| cargo build --locked --workspace -j 2 | erfolgreich |
| Separater cargo build --locked -p terra-server -j 2 | erfolgreich |
| Headless-Smoke ohne DISPLAY/WAYLAND_DISPLAY | erfolgreich, 27 Sections |
| Separater Server-Feature-Baum | kein bevy_render, bevy_winit, wgpu, terra-ui oder tilt-ui |
| Native Menuesuite 960x640 | erfolgreich, einschliesslich produktivem Quit |
| Native PointerInput-/TiltUI-Hit-Test-Suite | erfolgreich |
| Native Integrated-Sitzung | erfolgreicher Edit, finaler Save und Journal-Reload nach Host-Neustart |
| Nativer Dedicated-Join ueber gespeicherten Server/Token | erfolgreich, kein lokaler Host |
| Nativer Join zu gestopptem Server | echter Timeout, gerenderter Fehlertext und Rueckkehr erfolgreich |

Native GPU: AMD Radeon RX 6950 XT, RADV Mesa 26.2.4-arch3.1, Vulkan.
Keine Performance-Steigerung aus diesen Funktionstests abgeleitet.
Unit-/Integrationstests beinhalten die bisherigen Rendering-, Physik-, World-
und UDP-/Mehrspieler-Regressionstests. Die Menu-Smokes benutzen eigene temporaere
Config-/Weltverzeichnisse, niemals hartcodierte produktive Welten.

Vollstaendige lokale Protokolle liegen als `teramic-menu-*.log` in diesem Ordner;
der separate Feature-Baum steht in `teramic-menu-server-features.txt`.
Die vorhandene globale Git-Regel `*.log` wurde nicht entfernt.

Angepasste Bereiche: Client-Bootstrap/Session-Lifecycle, IntegratedServer,
terra-core (validierte Praeferenzen/Input), terra-ui (Navigation/View/Actions),
terra-storage (WorldManifest-Bibliothek/SectionDelta-Snapshots), terra-player
(wirkliche Bindings), terra-render (Qualitaet/Dichte), terra-world (stabiles
Dichte-Filtering) und terra-network (Lifecycle/pro-Client-Picking/Protokoll 2).

Konkrete Backend-Grenzen und Asset-Herkunft stehen in README.md dieses Ordners.
Client und Dedicated Server gemeinsam neu starten; fuer einen neu gestarteten
Server werden neue private Tokens benoetigt. Ein bereits zuvor laufender
Benutzer-Server auf Port 5000 wurde nicht beendet oder umkonfiguriert.
