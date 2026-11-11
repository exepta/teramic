//! Finite UI textures are retained before gameplay, including TiltUI's weak icon cache.
use bevy::{asset::LoadState, prelude::*};
use std::{
    collections::VecDeque,
    time::{Duration, Instant},
};

const STATIC_TEXTURES: &[&str] = &[
    "biome-choice-cherry-forest.png",
    "biome-choice-cliffted-desert.png",
    "biome-choice-desert.png",
    "biome-choice-forest.png",
    "biome-choice-highlands.png",
    "biome-choice-mushroom-park.png",
    "biome-choice-ocean.png",
    "biome-choice-plains.png",
    "cauldron-recipe-184.png",
    "furnace-recipe-185.png",
    "hand.png",
    "hotbar-frame-182.png",
    "inventory-copper-frame.png",
    "map-compass-ring.png",
    "menu-button-182.png",
    "menu-logo-182.png",
    "menu-valley-182.png",
    "menu-valley.png",
    "season-compass-frame.png",
    "status-effects-atlas.png",
    "stone-gold-frame.png",
    "survival-atlas.png",
    "survival-chrome.png",
    "survival-status-frame.png",
];

#[derive(Resource, Default)]
pub(crate) struct UiStartupAssets {
    pub required: bool,
    pub ready: bool,
    pub total: usize,
    pub completed: usize,
    pub error: Option<String>,
    pub current_path: String,
    icons: VecDeque<(tilt_ui::Icon, tilt_ui::IconSize)>,
    pins: Vec<Handle<Image>>,
    textures: Vec<Handle<Image>>,
    audio: Vec<Handle<AudioSource>>,
    started: Option<Instant>,
    settled: u8,
}

fn icons() -> VecDeque<(tilt_ui::Icon, tilt_ui::IconSize)> {
    tilt_ui::Icon::ALL
        .iter()
        .copied()
        .flat_map(|icon| {
            [
                tilt_ui::IconSize::Px16,
                tilt_ui::IconSize::Px32,
                tilt_ui::IconSize::Px64,
            ]
            .map(move |size| (icon, size))
        })
        .collect()
}

fn textures() -> Vec<String> {
    STATIC_TEXTURES
        .iter()
        .map(|name| format!("tilt-ui://assets/{name}"))
        .collect()
}

pub(crate) fn install(app: &mut App) {
    if !app
        .world()
        .get_resource::<terra_render::RenderPreviewStartup>()
        .is_some_and(|startup| startup.required)
    {
        return;
    }
    let mut state = UiStartupAssets {
        required: true,
        icons: icons(),
        ..default()
    };
    state.textures = textures()
        .into_iter()
        .map(|path| app.world().resource::<AssetServer>().load(path))
        .collect();
    state.audio = ["audio/ui/click.wav", "audio/ui/mention.wav"]
        .into_iter()
        .map(|path| app.world().resource::<AssetServer>().load(path))
        .collect();
    state.total = state.icons.len() + state.textures.len() + state.audio.len() + 5;
    // Establish the screen before the first menu view/input update. No menu
    // button is displayed while the renderer and shared UI cache are warming.
    app.world_mut()
        .resource_mut::<terra_ui::menu::StartupProgress>()
        .active = true;
    app.insert_resource(terra_ui::menu::MenuView {
        visible: true,
        loading: true,
        startup: true,
        status: String::new(),
        cancel_connect: false,
        ..default()
    });
    app.insert_resource(state).add_systems(Update, warm);
}

fn warm(world: &mut World) {
    if world.resource::<UiStartupAssets>().ready
        || world.resource::<UiStartupAssets>().error.is_some()
    {
        return;
    }
    world.resource_scope(|world, mut state: Mut<UiStartupAssets>| {
        let since = *state.started.get_or_insert_with(Instant::now);
        // Rasterize a bounded batch so the loading screen remains responsive.
        for _ in 0..8 {
            let Some((icon, size)) = state.icons.pop_front() else {
                break;
            };
            state.current_path = format!("UI-Icon: {} ({size:?})", icon.name());
            match tilt_ui::icon_image(world, icon, size) {
                Some(image) => state.pins.push(image),
                None => {
                    state.error = Some(format!("UI icon {} could not be rasterized", icon.name()));
                    return;
                }
            }
        }
        let server = world.resource::<AssetServer>();
        if let Some(error) =
            state
                .textures
                .iter()
                .find_map(|image| match server.get_load_state(image.id()) {
                    Some(LoadState::Failed(error)) => Some(error.to_string()),
                    _ => None,
                })
        {
            state.error = Some(format!("UI texture could not be loaded: {error}"));
            return;
        }
        if let Some(error) = state.audio.iter().find_map(|audio| match server.get_load_state(audio.id()) {
            Some(LoadState::Failed(error)) => Some(error.to_string()),
            _ => None,
        }) {
            state.error = Some(format!("UI audio could not be loaded: {error}"));
            return;
        }
        let audio_loaded = state.audio.iter().filter(|audio| server.is_loaded_with_dependencies(audio.id())).count();
        if state.icons.is_empty() {
            state.current_path = state.textures.iter().find(|image| !server.is_loaded_with_dependencies(image.id()))
                .and_then(|image| server.get_path(image.id())).map(|path| path.to_string())
                .or_else(|| state.audio.iter().find(|audio| !server.is_loaded_with_dependencies(audio.id()))
                    .and_then(|audio| server.get_path(audio.id())).map(|path| path.to_string()))
                .unwrap_or_default();
        }
        let recorded = world.get_resource::<crate::creature_audio::RecordedAudioStartup>();
        if let Some(error) = recorded.and_then(|status| status.error.clone()) {
            state.error = Some(format!("Recorded audio could not be loaded: {error}"));
            return;
        }
        let recorded_ready = recorded.is_some_and(|status| status.ready);
        if state.current_path.is_empty() && !recorded_ready {
            state.current_path = "audio: Tier-, Umgebungs- und Geraetegeraeusche".into();
        }
        let loaded = state
            .textures
            .iter()
            .filter(|image| server.is_loaded_with_dependencies(image.id()))
            .count();
        let fonts = world
            .get_resource::<Assets<Font>>()
            .is_some_and(|fonts| fonts.len() >= 3);
        let document = world.get_resource::<tilt_ui::UiDocumentState>();
        if let Some(document) = document
            && let tilt_ui::UiLoadState::Error(error) = &document.0.load
        {
            state.error = Some(format!(
                "UI document could not become ready: {}",
                error.message
            ));
            return;
        }
        if state.current_path.is_empty() && !fonts {
            state.current_path = "tilt-ui://assets/fonts".into();
        }
        let document_ready =
            document.is_some_and(|document| document.0.load == tilt_ui::UiLoadState::Ready);
        if state.current_path.is_empty() && !document_ready {
            state.current_path = "tilt-ui://components/app-shell/app-shell.component.html".into();
        }
        state.completed =
            state.pins.len() + loaded + audio_loaded + if fonts { 3 } else { 0 }
                + usize::from(document_ready) + usize::from(recorded_ready);
        if state.completed == state.total {
            // Let image extraction/preparation see the final assets before releasing the session gate.
            state.settled += 1;
            state.ready = state.settled >= 2;
        } else {
            state.settled = 0;
        }
        if !state.ready && since.elapsed() > Duration::from_secs(90) {
            state.error = Some(
                "UI images, audio, fonts or document did not become ready; restart the client to retry"
                    .into(),
            );
        }
    });
}

impl UiStartupAssets {
    pub(crate) fn icon_count(&self) -> usize {
        self.pins.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ui_icon_preload_is_finite_unique_and_covers_each_supported_runtime_size() {
        let icons = icons();
        assert_eq!(icons.len(), tilt_ui::Icon::ALL.len() * 3);
        assert_eq!(
            icons
                .iter()
                .copied()
                .collect::<std::collections::HashSet<_>>()
                .len(),
            icons.len()
        );
    }

    #[test]
    fn static_ui_texture_catalog_covers_real_hud_and_chrome_assets() {
        let files = textures();
        assert_eq!(
            files.iter().collect::<std::collections::HashSet<_>>().len(),
            files.len()
        );
        for name in [
            "hand.png",
            "stone-gold-frame.png",
            "survival-chrome.png",
            "survival-atlas.png",
            "status-effects-atlas.png",
            "season-compass-frame.png",
        ] {
            assert!(
                files.contains(&format!("tilt-ui://assets/{name}")),
                "missing {name}"
            );
        }
        let mut actual =
            std::fs::read_dir(concat!(env!("CARGO_MANIFEST_DIR"), "/../../src-ui/assets"))
                .unwrap()
                .map(|entry| entry.unwrap().path())
                .filter(|path| path.extension().is_some_and(|extension| extension == "png"))
                .map(|path| {
                    format!(
                        "tilt-ui://assets/{}",
                        path.file_name().unwrap().to_str().unwrap()
                    )
                })
                .collect::<Vec<_>>();
        actual.sort_unstable();
        assert_eq!(files, actual);
    }

    #[test]
    fn pinned_ui_icons_use_the_actual_shared_tilt_ui_image_cache() {
        let mut world = World::new();
        world.init_resource::<Assets<Image>>();
        let image =
            tilt_ui::icon_image(&mut world, tilt_ui::Icon::Flame, tilt_ui::IconSize::Px32).unwrap();
        let id = image.id();
        let state = UiStartupAssets {
            pins: vec![image],
            ..default()
        };
        world.insert_resource(state);
        let repeated =
            tilt_ui::icon_image(&mut world, tilt_ui::Icon::Flame, tilt_ui::IconSize::Px32).unwrap();
        assert_eq!(repeated.id(), id);
        assert_eq!(world.resource::<Assets<Image>>().len(), 1);
        assert!(
            world
                .resource::<Assets<Image>>()
                .get(id)
                .unwrap()
                .data
                .as_ref()
                .unwrap()
                .chunks_exact(4)
                .any(|pixel| pixel[3] > 0)
        );
    }
}
