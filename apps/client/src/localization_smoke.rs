//! Opt-in native language selection through the production TiltUI handlers.
mod category_layout;
use bevy::{
    app::AppExit,
    prelude::*,
    render::view::screenshot::{Screenshot, ScreenshotCaptured, save_to_disk},
};
use std::{
    path::PathBuf,
    time::{Duration, Instant},
};
use terra_ui::menu::{MenuPaths, MenuView, OptionsView};

pub(crate) fn requested() -> bool {
    std::env::var("TERRA_LOCALIZATION_GALLERY").as_deref() == Ok("1")
}

#[derive(Resource)]
struct Gallery {
    step: u8,
    started: Instant,
    since: Instant,
    output: PathBuf,
    pending: bool,
    captures: u8,
    failed: bool,
}

pub(crate) fn install(app: &mut App) {
    let root = std::env::var_os("TERRA_MENU_SMOKE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::temp_dir().join("terra-localization-smoke"));
    let now = Instant::now();
    app.insert_resource(MenuPaths {
        configs: root.join("configs"),
        worlds: terra_storage::WorldLibrary {
            root: root.join("worlds"),
        },
    })
    .insert_resource(Gallery {
        step: 0,
        started: now,
        since: now,
        output: root.join("captures"),
        pending: false,
        captures: 0,
        failed: false,
    })
    .insert_resource(bevy::winit::WinitSettings::continuous())
    .add_systems(Last, run);
}

fn has_text(world: &mut World, expected: &str) -> bool {
    world
        .query::<&Text>()
        .iter(world)
        .any(|text| text.0 == expected)
}

fn icon_bounds(world: &mut World) -> Result<(), String> {
    let (size, center) = world
        .query::<(
            &terra_ui::ElementId,
            &ComputedNode,
            &bevy::ui::UiGlobalTransform,
        )>()
        .iter(world)
        .find_map(|(id, node, transform)| {
            (id.0 == "character-open" && node.size().min_element() > 0.0)
                .then_some((node.size(), transform.translation))
        })
        .ok_or("Missing character designer button")?;
    let window = world
        .query::<&Window>()
        .iter(world)
        .next()
        .ok_or("Missing window")?;
    let viewport = Vec2::new(
        window.physical_width() as f32,
        window.physical_height() as f32,
    );
    let right = center.x + size.x * 0.5;
    let bottom = center.y + size.y * 0.5;
    if center.x < viewport.x * 0.75
        || center.y < viewport.y * 0.75
        || right > viewport.x
        || bottom > viewport.y
    {
        return Err(format!(
            "Character button is outside the lower-right corner: {center:?} {viewport:?}"
        ));
    }
    Ok(())
}

fn capture(world: &mut World, state: &mut Gallery, name: &str) -> Result<(), String> {
    std::fs::create_dir_all(&state.output).map_err(|error| error.to_string())?;
    world
        .spawn(Screenshot::primary_window())
        .observe(save_to_disk(state.output.join(format!("{name}.png"))))
        .observe(
            |event: On<ScreenshotCaptured>, mut state: ResMut<Gallery>| {
                let colors: std::collections::HashSet<_> = event
                    .image
                    .data
                    .as_deref()
                    .unwrap_or_default()
                    .chunks_exact(4)
                    .step_by(113)
                    .map(|pixel| [pixel[0], pixel[1], pixel[2]])
                    .collect();
                state.failed |= colors.len() < 16;
                state.captures += 1;
                state.pending = false;
            },
        );
    state.pending = true;
    Ok(())
}

fn run(world: &mut World) {
    world.resource_scope(|world, mut state: Mut<Gallery>| {
        let result = (|| -> Result<bool, String> {
            if state.failed { return Err("Blank language UI capture".into()); }
            if state.started.elapsed() > Duration::from_secs(180) { return Err(format!("Language UI timed out at {}", state.step)); }
            if state.pending || state.since.elapsed() < Duration::from_millis(700) { return Ok(false); }
            match state.step {
                0 => {
                    let menu = world.resource::<MenuView>();
                    if !menu.main || menu.startup || menu.startup_fading || !has_text(world, "Einzelspieler") { return Ok(false); }
                    icon_bounds(world)?; capture(world, &mut state, "main-menu-de")?;
                }
                1 => crate::menu_smoke::emit(world,"options",None)?,
                2 => crate::menu_smoke::emit(world,"gameplay-options",None)?,
                3 => {
                    if !world.resource::<OptionsView>().gameplay || !has_text(world,"Sprache") { return Ok(false); }
                    category_layout::check(world,&state.output,"de")?;
                    capture(world,&mut state,"gameplay-de")?;
                }
                4 => crate::menu_smoke::emit(world,"game-language",Some("en"))?,
                5 => {
                    if world.resource::<terra_ui::menu::OptionsDraft>().gameplay.language != terra_core::GameLanguage::English { return Ok(false); }
                    crate::menu_smoke::emit(world,"apply",None)?;
                }
                6 => {
                    if world.resource::<terra_core::GameplayConfig>().language != terra_core::GameLanguage::English || !has_text(world,"Language") { return Ok(false); }
                    let configs = &world.resource::<MenuPaths>().configs;
                    let profile = world.resource::<terra_core::PlayerProfile>();
                    use terra_core::Config;
                    let saved = terra_core::LanguagePreference::load(configs.join("profile-preferences").join(profile.uuid.simple().to_string()).join("language.toml")).map_err(|error|error.to_string())?;
                    if saved.language != terra_core::GameLanguage::English { return Err("Native language preference was not saved".into()); }
                    category_layout::check(world,&state.output,"en")?;
                    capture(world,&mut state,"gameplay-en")?;
                }
                7 => crate::menu_smoke::emit(world,"back",None)?,
                8 => {
                    if !world.resource::<MenuView>().main || !has_text(world,"Singleplayer") { return Ok(false); }
                    icon_bounds(world)?; capture(world,&mut state,"main-menu-en")?;
                }
                _ => { if state.captures < 4 { return Ok(false); } info!("Native language UI passed: German, real selection/apply, saved profile, English and lower-right character button"); return Ok(true); }
            }
            state.step += 1; state.since = Instant::now(); Ok(false)
        })();
        match result { Ok(true) => { world.write_message(AppExit::Success); }, Err(error) => { error!(%error,"Native language UI failed"); world.write_message(AppExit::error()); }, Ok(false) => {} }
    });
}
