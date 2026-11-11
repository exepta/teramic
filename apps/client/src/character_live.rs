//! Opt-in native character editor verification using real UI pointer events.
use bevy::{app::AppExit, prelude::*, window::PrimaryWindow};
use std::{
    path::PathBuf,
    time::{Duration, Instant},
};
use terra_core::{
    CharacterSelection, ClothingMaterial, Config, PlayerGender, PlayerProfile,
    ServerCharacterProfiles,
};
use terra_ui::{
    CharacterPreviewImage, CharacterPreviewSelection, ElementId,
    menu::{MenuDialog, MenuPage, MenuPaths, MenuState, ServerMenu, StartupProgress},
};

#[derive(Clone, Copy)]
enum Step {
    Click(&'static str, &'static str),
    Change(&'static str, &'static str),
    Check(u8),
    Resize(u32, u32),
    Capture(&'static str),
    Drag,
    Finish,
}

const STEPS: &[Step] = &[
    Step::Click("character-open", ""),
    Step::Click("character-tab-body", ""),
    Step::Click("character-female", ""),
    Step::Check(1),
    Step::Click("character-tab-hair", ""),
    Step::Click("menu-action", "F14"),
    Step::Change("character-hair-color", "#221133"),
    Step::Click("character-highlights", ""),
    Step::Change("character-highlight-color", "#c93b96"),
    Step::Check(1),
    Step::Capture("editor-hair-fhd"),
    Step::Click("character-tab-face", ""),
    Step::Change("character-eye-color", "#286fbb"),
    Step::Change("character-right-eye-color", "#8d642e"),
    Step::Check(1),
    Step::Capture("editor-eyes-fhd"),
    Step::Click("character-tab-clothing", ""),
    Step::Change("character-shirt-color", "#669922"),
    Step::Change("character-pants-color", "#776655"),
    Step::Change("character-shirt-material", "leather"),
    Step::Change("character-pants-material", "wool"),
    Step::Change("character-boots-material", "canvas"),
    Step::Change("character-cape-material", "linen"),
    Step::Check(0),
    Step::Drag,
    Step::Click("character-tunic", ""),
    Step::Click("character-boots", ""),
    Step::Click("character-cape", ""),
    Step::Capture("editor-materials-fhd"),
    Step::Resize(1280, 720),
    Step::Check(1),
    Step::Capture("editor-materials-hd"),
    Step::Click("character-tab-hair", ""),
    Step::Check(1),
    Step::Capture("editor-hair-hd"),
    Step::Click("character-tab-clothing", ""),
    Step::Resize(2560, 1440),
    Step::Check(1),
    Step::Capture("editor-materials-qhd"),
    Step::Resize(3840, 2160),
    Step::Check(1),
    Step::Capture("editor-materials-uhd"),
    Step::Click("character-tab-hair", ""),
    Step::Check(1),
    Step::Capture("editor-hair-uhd"),
    Step::Resize(1920, 1080),
    Step::Click("character-reset", ""),
    Step::Check(2),
    Step::Click("character-tab-body", ""),
    Step::Click("character-male", ""),
    Step::Click("character-tab-hair", ""),
    Step::Click("menu-action", "M12"),
    Step::Check(1),
    Step::Capture("editor-hair-male"),
    Step::Click("menu-action", "M1"),
    Step::Click("character-tab-clothing", ""),
    Step::Click("character-boots", ""),
    Step::Click("character-cape", ""),
    Step::Capture("editor-reference-dressed"),
    Step::Click("character-reset", ""),
    Step::Check(2),
    Step::Click("character-random", ""),
    Step::Check(9),
    Step::Click("character-reset", ""),
    Step::Check(2),
    Step::Click("character-tab-hair", ""),
    Step::Change("character-hair-color", "#cc5500"),
    Step::Click("character-highlights", ""),
    Step::Click("character-tab-clothing", ""),
    Step::Change("character-shirt-material", "leather"),
    Step::Change("character-cape-material", "canvas"),
    Step::Click("character-cancel", ""),
    Step::Check(3),
    Step::Click("character-open", ""),
    Step::Click("character-tab-body", ""),
    Step::Click("character-female", ""),
    Step::Click("character-tab-hair", ""),
    Step::Click("menu-action", "F14"),
    Step::Change("character-hair-color", "#221133"),
    Step::Click("character-highlights", ""),
    Step::Change("character-highlight-color", "#c93b96"),
    Step::Click("character-tab-face", ""),
    Step::Change("character-eye-color", "#286fbb"),
    Step::Change("character-right-eye-color", "#8d642e"),
    Step::Click("character-tab-clothing", ""),
    Step::Change("character-shirt-color", "#669922"),
    Step::Change("character-pants-color", "#776655"),
    Step::Change("character-shirt-material", "leather"),
    Step::Change("character-pants-material", "wool"),
    Step::Change("character-boots-material", "canvas"),
    Step::Change("character-cape-material", "linen"),
    Step::Click("character-save", ""),
    Step::Check(4),
    Step::Click("character-open", ""),
    Step::Check(5),
    Step::Click("character-tab-clothing", ""),
    Step::Check(1),
    Step::Capture("editor-materials-reopened"),
    Step::Click("character-cancel", ""),
    Step::Click("menu-action", "Multiplayer"),
    Step::Click("server-card", "Character UI Test"),
    Step::Click("menu-action", "Charakter"),
    Step::Click("character-tab-hair", ""),
    Step::Change("character-hair-color", "#0055cc"),
    Step::Change("character-highlight-color", "#e4b665"),
    Step::Click("character-tab-face", ""),
    Step::Change("character-right-eye-color", "#3b9a6f"),
    Step::Click("character-tab-clothing", ""),
    Step::Change("character-shirt-material", "wool"),
    Step::Change("character-pants-material", "linen"),
    Step::Change("character-boots-material", "leather"),
    Step::Change("character-cape-material", "canvas"),
    Step::Check(1),
    Step::Capture("editor-materials-server"),
    Step::Click("character-save", ""),
    Step::Check(6),
    Step::Click("server-card", "Character First Join Test"),
    Step::Click("menu-action", "Join Server"),
    Step::Check(7),
    Step::Click("character-tab-clothing", ""),
    Step::Check(1),
    Step::Capture("editor-materials-first-join"),
    Step::Change("character-shirt-material", "wool"),
    Step::Click("character-cancel", ""),
    Step::Check(8),
    Step::Finish,
];

#[derive(Resource)]
struct Live {
    root: PathBuf,
    started: Instant,
    next: Instant,
    step: usize,
    pressed: bool,
    drag_phase: u8,
    drag_rotation: f32,
    baseline: Option<PlayerProfile>,
    profile_path: PathBuf,
    done: bool,
}

pub(crate) fn install(app: &mut App) {
    if std::env::var("TERRA_CHARACTER_UI_LIVE").as_deref() != Ok("1") {
        return;
    }
    let root = std::env::var_os("TERRA_CHARACTER_UI_LIVE_DIR").map(PathBuf::from);
    let Some(root) = root else {
        tracing::error!("CHARACTER UI FAIL: TERRA_CHARACTER_UI_LIVE_DIR is required");
        app.world_mut().write_message(AppExit::error());
        return;
    };
    crate::menu_smoke::install_test_pointer(app);
    app.insert_resource(bevy::winit::WinitSettings::continuous())
        .insert_resource(Live {
            root,
            started: Instant::now(),
            next: Instant::now(),
            step: 0,
            pressed: false,
            drag_phase: 0,
            drag_rotation: 0.0,
            baseline: None,
            profile_path: PathBuf::new(),
            done: false,
        })
        .add_systems(Last, run);
}

fn require(condition: bool, error: &str) -> Result<(), String> {
    if condition { Ok(()) } else { Err(error.into()) }
}

fn setup(world: &World, state: &mut Live) -> Result<(), String> {
    let root = state.root.canonicalize().map_err(|e| e.to_string())?;
    let configs = world
        .resource::<MenuPaths>()
        .configs
        .canonicalize()
        .map_err(|e| e.to_string())?;
    require(
        configs.starts_with(&root) && configs != root,
        "Native fixture configs must be inside its isolated output directory",
    )?;
    state.profile_path = PlayerProfile::selected_path(&configs).map_err(|e| e.to_string())?;
    require(
        state
            .profile_path
            .canonicalize()
            .map_err(|e| e.to_string())?
            .starts_with(&configs),
        "Native fixture cannot write a real external player profile",
    )?;
    let menu = world.resource::<MenuState>();
    require(
        menu.profile_loaded && menu.page == MenuPage::Main && !menu.session_active,
        "Native fixture must start at the normal ready main menu",
    )?;
    let servers = &world.resource::<ServerMenu>().list.servers;
    require(
        servers
            .iter()
            .any(|entry| entry.name == "Character UI Test")
            && servers
                .iter()
                .any(|entry| entry.name == "Character First Join Test"),
        "Missing isolated character test server entries",
    )?;
    let profiles = load_overrides(world)?;
    let pending = servers
        .iter()
        .find(|entry| entry.name == "Character First Join Test")
        .unwrap();
    let profile = world.resource::<PlayerProfile>().clone();
    require(
        profiles
            .get(profile.uuid, &pending.address, pending.port)
            .map_err(|e| e.to_string())?
            .is_none(),
        "First-join test endpoint must not have an existing character",
    )?;
    state.baseline = Some(profile);
    state.root = root;
    std::fs::create_dir_all(state.root.join("captures")).map_err(|e| e.to_string())?;
    tracing::info!("CHARACTER UI START: isolated native pointer verification");
    Ok(())
}

fn load_overrides(world: &World) -> Result<ServerCharacterProfiles, String> {
    let path = world
        .resource::<MenuPaths>()
        .configs
        .join("server-characters.toml");
    if path.exists() {
        ServerCharacterProfiles::load(path).map_err(|e| e.to_string())
    } else {
        Ok(ServerCharacterProfiles::default())
    }
}

fn run(world: &mut World) {
    world.resource_scope(|world, mut state: Mut<Live>| {
        if state.done || Instant::now() < state.next {
            return;
        }
        if state.started.elapsed() > Duration::from_secs(300) {
            fail(
                world,
                &mut state,
                "Native character verification timed out".into(),
            );
            return;
        }
        if world
            .get_resource::<StartupProgress>()
            .is_some_and(|progress| progress.active || progress.fading)
        {
            return;
        }
        if state.baseline.is_none() {
            if let Err(error) = setup(world, &mut state) {
                fail(world, &mut state, error);
                return;
            }
            state.next = Instant::now() + Duration::from_secs(2);
            return;
        }
        let result = advance(world, &mut state);
        if let Err(error) = result {
            fail(world, &mut state, error);
        }
    });
}

fn advance(world: &mut World, state: &mut Live) -> Result<(), String> {
    let step = *STEPS
        .get(state.step)
        .ok_or("Native character fixture exceeded its script")?;
    let mut delay = Duration::from_millis(650);
    match step {
        Step::Click(id, title) => {
            crate::menu_smoke::pointer(world, id, title, !state.pressed)?;
            state.pressed = !state.pressed;
            if state.pressed {
                state.next = Instant::now() + Duration::from_millis(250);
                return Ok(());
            }
        }
        Step::Drag => {
            use bevy::input::{ButtonState, mouse::MouseButtonInput};
            let window = world
                .query_filtered::<Entity, With<PrimaryWindow>>()
                .single(world)
                .map_err(|e| e.to_string())?;
            match state.drag_phase {
                0 => {
                    state.drag_rotation = world.resource::<CharacterPreviewSelection>().rotation;
                    crate::menu_smoke::pointer(world, "character-preview", "", true)?;
                    world.write_message(MouseButtonInput {
                        button: MouseButton::Left,
                        state: ButtonState::Pressed,
                        window,
                    });
                }
                1 => {
                    crate::menu_smoke::move_test_pointer(world, Vec2::new(50.0, 0.0))?;
                }
                _ => {
                    world.write_message(MouseButtonInput {
                        button: MouseButton::Left,
                        state: ButtonState::Released,
                        window,
                    });
                    crate::menu_smoke::pointer(world, "character-preview", "", false)?;
                    require(
                        (world.resource::<CharacterPreviewSelection>().rotation
                            - state.drag_rotation)
                            .abs()
                            > 0.1,
                        "Dragging did not rotate the real character preview",
                    )?;
                    tracing::info!("CHARACTER UI DRAG PASS");
                }
            }
            state.drag_phase += 1;
            if state.drag_phase < 3 {
                state.next = Instant::now() + Duration::from_millis(250);
                return Ok(());
            }
        }
        Step::Change(id, value) => crate::menu_smoke::emit(world, id, Some(value))?,
        Step::Check(index) => verify(world, state, index)?,
        Step::Resize(width, height) => {
            let mut window = world
                .query_filtered::<&mut Window, With<PrimaryWindow>>()
                .single_mut(world)
                .map_err(|e| e.to_string())?;
            window.resolution.set_physical_resolution(width, height);
            delay = Duration::from_secs(3);
        }
        Step::Capture(name) => {
            world
                .spawn(bevy::render::view::screenshot::Screenshot::primary_window())
                .observe(bevy::render::view::screenshot::save_to_disk(
                    state.root.join("captures").join(format!("{name}.png")),
                ));
            tracing::info!(name, "CHARACTER UI HOLD");
            delay = Duration::from_secs(4);
        }
        Step::Finish => {
            let report = format!(
                "{{\"passed\":true,\"steps\":{},\"pointer_input\":true,\"garment_materials\":true,\"four_tabs\":true,\"independent_eyes\":true,\"colored_highlights\":true,\"requested_resolutions\":[\"1280x720\",\"1920x1080\",\"2560x1440\",\"3840x2160\"]}}\n",
                state.step
            );
            std::fs::write(state.root.join("character-ui-result.json"), report)
                .map_err(|e| e.to_string())?;
            tracing::info!(
                "CHARACTER UI PASS: pointer actions, four tabs, new hairstyle selections, separate eyes, highlights, colors, four independent materials, reset/cancel, persistence, per-server choice, first-join cancel, HD/FHD/QHD/UHD clothing layout"
            );
            state.done = true;
            world.write_message(AppExit::Success);
        }
    }
    state.step += 1;
    state.next = Instant::now() + delay;
    Ok(())
}

fn expected() -> CharacterSelection {
    CharacterSelection {
        gender: PlayerGender::Female,
        appearance: terra_core::CharacterAppearance {
            hairstyle: 13,
            hair_highlights: true,
            hair_highlight_color: [201, 59, 150],
            eye_color: [40, 111, 187],
            right_eye_color: Some([141, 100, 46]),
            hair_color: [34, 17, 51],
            shirt_color: [102, 153, 34],
            pants_color: [119, 102, 85],
            shirt_material: ClothingMaterial::Leather,
            pants_material: ClothingMaterial::Wool,
            boots_material: ClothingMaterial::Canvas,
            cape_material: ClothingMaterial::Linen,
            ..Default::default()
        },
    }
}

fn verify(world: &mut World, state: &Live, index: u8) -> Result<(), String> {
    let original = state.baseline.as_ref().ok_or("Missing initial profile")?;
    let selection = |world: &World| {
        world
            .get_resource::<CharacterPreviewSelection>()
            .map(|preview| CharacterSelection {
                gender: preview.gender,
                appearance: preview.appearance,
            })
    };
    let local = world.resource::<PlayerProfile>();
    match index {
        0 => {
            require(
                selection(world) == Some(expected()),
                "Actual pointer/style/color/material handlers did not update the live draft",
            )?;
            require(
                local == original,
                "Draft preview mutated the local profile before Save",
            )?;
            verify_layout(world)?;
        }
        1 => verify_layout(world)?,
        2 => require(
            selection(world) == Some(CharacterSelection::from(original)),
            "Reset did not restore the editor's original selection",
        )?,
        3 => {
            require(
                local == original && world.resource::<MenuState>().dialog == MenuDialog::None,
                "Cancel changed the profile or failed to close",
            )?;
            require(
                PlayerProfile::load(&state.profile_path).map_err(|e| e.to_string())? == *original,
                "Cancel wrote the profile file",
            )?;
        }
        4 => {
            require(
                CharacterSelection::from(local) == expected()
                    && local.uuid == original.uuid
                    && local.name == original.name,
                "Save changed identity or did not preserve the draft",
            )?;
            require(
                PlayerProfile::load(&state.profile_path).map_err(|e| e.to_string())? == *local,
                "Saved appearance was not persisted",
            )?;
        }
        5 => require(
            selection(world) == Some(expected()),
            "Reopening the character editor lost the saved appearance",
        )?,
        6 => {
            let server = world
                .resource::<ServerMenu>()
                .list
                .servers
                .iter()
                .find(|entry| entry.name == "Character UI Test")
                .unwrap();
            let saved = load_overrides(world)?
                .get(local.uuid, &server.address, server.port)
                .map_err(|e| e.to_string())?
                .ok_or("Per-server save is missing")?;
            let mut expected_server = expected();
            expected_server.appearance.hair_color = [0, 85, 204];
            expected_server.appearance.hair_highlight_color = [228, 182, 101];
            expected_server.appearance.right_eye_color = Some([59, 154, 111]);
            expected_server.appearance.shirt_material = ClothingMaterial::Wool;
            expected_server.appearance.pants_material = ClothingMaterial::Linen;
            expected_server.appearance.boots_material = ClothingMaterial::Leather;
            expected_server.appearance.cape_material = ClothingMaterial::Canvas;
            require(
                saved == expected_server
                    && CharacterSelection::from(local) == expected()
                    && PlayerProfile::load(&state.profile_path)
                        .map_err(|error| error.to_string())?
                        == *local,
                "Server appearance overwrote the local default or was not saved",
            )?;
            require(
                !world.resource::<MenuState>().session_active,
                "Server character editing unexpectedly connected",
            )?;
        }
        7 => {
            require(
                world.resource::<MenuState>().dialog == MenuDialog::Character
                    && selection(world) == Some(expected()),
                "First join did not open an editor inheriting the local character",
            )?;
            require(
                !world.resource::<MenuState>().session_active,
                "Connection started before first-join confirmation",
            )?;
        }
        8 => {
            let server = world
                .resource::<ServerMenu>()
                .list
                .servers
                .iter()
                .find(|entry| entry.name == "Character First Join Test")
                .unwrap();
            require(
                world.resource::<MenuState>().dialog == MenuDialog::None
                    && !world.resource::<MenuState>().session_active,
                "First-join Cancel failed or started a session",
            )?;
            require(
                load_overrides(world)?
                    .get(local.uuid, &server.address, server.port)
                    .map_err(|e| e.to_string())?
                    .is_none(),
                "Cancelled first join persisted an override",
            )?;
        }
        9 => {
            let randomized = selection(world).ok_or("Randomize removed the preview")?;
            require(
                randomized != CharacterSelection::from(original),
                "Randomize button did not change the draft",
            )?;
            randomized.validate().map_err(|error| error.to_string())?;
            require(
                local == original,
                "Randomize changed the saved profile before Save",
            )?;
        }
        _ => return Err("Unknown native verification phase".into()),
    }
    tracing::info!(index, "CHARACTER UI CHECK PASS");
    Ok(())
}

fn bounds(world: &World, entity: Entity) -> Result<(Vec2, Vec2), String> {
    let node = world
        .get::<ComputedNode>(entity)
        .ok_or("Character control has no computed layout")?;
    let transform = world
        .get::<bevy::ui::UiGlobalTransform>(entity)
        .ok_or("Character control has no global transform")?;
    let half = node.size() * 0.5;
    Ok([
        Vec2::new(-half.x, -half.y),
        Vec2::new(half.x, -half.y),
        Vec2::new(-half.x, half.y),
        Vec2::new(half.x, half.y),
    ]
    .map(|point| transform.transform_point2(point))
    .into_iter()
    .fold(
        (Vec2::splat(f32::INFINITY), Vec2::splat(f32::NEG_INFINITY)),
        |(low, high), point| (low.min(point), high.max(point)),
    ))
}

fn descendant(world: &World, mut entity: Entity, ancestor: Entity) -> bool {
    for _ in 0..32 {
        if entity == ancestor {
            return true;
        }
        let Some(parent) = world.get::<ChildOf>(entity) else {
            return false;
        };
        entity = parent.parent();
    }
    false
}

fn verify_layout(world: &mut World) -> Result<(), String> {
    let elements: Vec<_> = world
        .query::<(Entity, &ElementId)>()
        .iter(world)
        .map(|(entity, id)| (entity, id.0.to_string()))
        .collect();
    let panel = elements
        .iter()
        .find(|(_, id)| id == "character-editor")
        .map(|(entity, _)| *entity)
        .ok_or("Character panel missing")?;
    let (low, high) = bounds(world, panel)?;
    let controls_frame = elements
        .iter()
        .find(|(_, id)| id == "character-controls")
        .map(|(entity, _)| *entity)
        .ok_or("Character controls frame missing")?;
    let (controls_low, controls_high) = bounds(world, controls_frame)?;
    let viewport_size = world
        .query_filtered::<&Window, With<PrimaryWindow>>()
        .single(world)
        .map_err(|e| e.to_string())?
        .physical_size();
    require(
        low.cmpge(Vec2::ZERO).all() && high.cmple(viewport_size.as_vec2() + Vec2::ONE).all(),
        "Character editor exceeds the resized viewport",
    )?;
    let mut checked = 0;
    let mut controls = std::collections::BTreeSet::new();
    let mut hairstyles = 0;
    for (entity, id) in &elements {
        if *entity == panel
            || !descendant(world, *entity, panel)
            || !(id.starts_with("character-") || id == "menu-action")
        {
            continue;
        }
        let (minimum, maximum) = bounds(world, *entity)?;
        require(
            minimum.cmpge(low - Vec2::ONE).all()
                && maximum.cmple(high + Vec2::ONE).all()
                && maximum.cmpgt(minimum).all(),
            &format!("Character control escapes its frame: {id}"),
        )?;
        if *entity != controls_frame && descendant(world, *entity, controls_frame) {
            require(
                minimum.cmpge(controls_low + Vec2::ONE).all()
                    && maximum.cmple(controls_high - Vec2::ONE).all(),
                &format!("Character field escapes the inner controls frame: {id}"),
            )?;
        }
        if matches!(
            id.as_str(),
            "character-save" | "character-cancel" | "character-reset" | "character-random"
        ) {
            require(
                maximum.y <= high.y - (high.y - low.y) * 0.04,
                "Footer button overlaps the decorative frame",
            )?;
        }
        if id == "character-preview" {
            let size = maximum - minimum;
            require(
                (size.x / size.y - 640.0 / 896.0).abs() < 0.02,
                "Character preview is stretched",
            )?;
            let preview = world.resource::<CharacterPreviewImage>();
            require(
                preview.ready
                    && world
                        .resource::<Assets<Image>>()
                        .get(&preview.image)
                        .is_some(),
                "Live character render target is not ready",
            )?;
        }
        if id == "character-style" {
            hairstyles += 1;
        }
        controls.insert(id.as_str());
        checked += 1;
    }
    require(
        controls
            .iter()
            .filter(|id| id.starts_with("character-tab-"))
            .count()
            == 4,
        "Character editor must have exactly four tabs",
    )?;
    if controls.contains("character-hair-color") {
        let selection = world.resource::<CharacterPreviewSelection>();
        require(
            hairstyles
                == usize::from(terra_core::CharacterAppearance::hairstyle_count(
                    selection.gender,
                )),
            "A hairstyle thumbnail is missing",
        )?;
        require(
            !controls.contains("character-shirt-color")
                && !controls.contains("character-eye-color")
                && !controls.contains("character-skin-color"),
            "Hair tab contains unrelated controls",
        )?;
        require(
            controls.contains("character-highlights"),
            "Highlight toggle missing",
        )?;
    } else if controls.contains("character-eye-color") {
        require(
            controls.contains("character-right-eye-color")
                && !controls.contains("character-skin-color")
                && !controls.contains("character-male"),
            "Face tab must contain independent eyes without body controls",
        )?;
    } else if controls.contains("character-shirt-color") {
        require(
            controls
                .iter()
                .filter(|id| id.ends_with("-material"))
                .count()
                == 4,
            "All four clothing material selectors must be visible",
        )?;
    } else {
        require(
            controls.contains("character-skin-color")
                && controls.contains("character-male")
                && controls.contains("character-female"),
            "Body controls are missing",
        )?;
    }
    require(checked >= 12, "Missing character controls")?;
    // Rows include their labels, even when those labels have no ElementId.
    let rows: Vec<_> = world
        .query::<(Entity, &tilt_ui::ElementClasses)>()
        .iter(world)
        .filter(|(_, classes)| {
            classes.classes.iter().any(|class| {
                matches!(
                    class.as_str(),
                    "character-field" | "character-color-row" | "character-material-row"
                )
            })
        })
        .map(|(entity, _)| entity)
        .collect();
    for entity in rows {
        if descendant(world, entity, controls_frame) {
            let (minimum, maximum) = bounds(world, entity)?;
            require(
                minimum.cmpge(controls_low + Vec2::ONE).all()
                    && maximum.cmple(controls_high - Vec2::ONE).all(),
                "Character row or label escapes the inner controls frame",
            )?;
        }
    }
    tracing::info!(checked, size=?viewport_size, "CHARACTER UI LAYOUT PASS");
    Ok(())
}

fn fail(world: &mut World, state: &mut Live, error: String) {
    tracing::error!(step=state.step, %error, "CHARACTER UI FAIL");
    state.done = true;
    world.write_message(AppExit::error());
}
