//! Short real-UDP owner-command check with the actual TiltUI radial and shared mob portrait.
use bevy::{
    input::{
        ButtonState,
        keyboard::{Key, KeyboardInput},
        mouse::MouseButtonInput,
    },
    prelude::*,
    render::view::screenshot::{Screenshot, ScreenshotCaptured, save_to_disk},
    window::PrimaryWindow,
};
use std::{
    path::PathBuf,
    time::{Duration, Instant},
};
use terra_core::{
    GameAction, GameConfig, GameMode, InputBinding, PlayerProfile,
    creatures::{
        Creature, CreatureBehavior, CreatureKind, CreatureNeeds,
        taming::{AnimalBond, CompanionMode},
    },
};
use terra_network::client::ReplicatedCreatures;
const PET: u64 = (1 << 62) + 120;

pub(crate) fn prepare(
    mut config: GameConfig,
) -> Result<(terra_server::IntegratedServer, Vec<u8>, PlayerProfile), String> {
    config.server.game_mode = GameMode::Creative;
    config.server.natural_mob_limit = Some(0);
    config.world.biomes = false;
    config.world.landscape_revision = 0;
    config.world.trees = false;
    config.world.caves = false;
    config.world.rivers = false;
    config.world.water = false;
    config.world.base_height = 16.0;
    config.world.hill_amplitude = 0.0;
    config.world.mountain_amplitude = 0.0;
    config.world.time.paused = true;
    config.world.time.start_hour = 12.0;
    let path = std::env::var_os("TERRA_PET_SMOKE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            std::env::temp_dir().join(format!("terra-pet-commands-{}", std::process::id()))
        });
    if path.exists() {
        return Err("Pet fixture requires a fresh isolated save directory".into());
    }
    let profile = PlayerProfile::load_for_client(
        PlayerProfile::selected_path(&terra_core::menu_config_directory())
            .map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    let mut journal = terra_world::edits::EditJournal::default();
    let terrain =
        terra_world::TerraGenerator::new(config.world.clone()).map_err(|e| e.to_string())?;
    let ground = terra_world::spawn::find_land_spawn(&terrain, &journal, [8, 24])?.ground;
    let position = [
        ground[0] as f64 + 0.5,
        ground[1] as f64 + 1.0,
        ground[2] as f64 - 2.7,
    ];
    let animal = Creature {
        id: PET,
        kind: CreatureKind::Cow,
        home: [
            (position[0].floor() as i64).div_euclid(32),
            (position[2].floor() as i64).div_euclid(32),
        ],
        position,
        velocity: [0.0; 3],
        yaw: std::f32::consts::PI,
        health: CreatureKind::Cow.health(),
        behavior: CreatureBehavior::Rest,
        timer: 0.0,
        age: 0.0,
        variant: 0,
        needs: CreatureNeeds::full(CreatureKind::Cow),
    };
    let state = journal.gameplay_mut();
    state.creatures.animals.insert(PET, animal);
    state.creatures.owners.insert(PET, profile.uuid);
    state.animal_bonds.insert(
        PET,
        AnimalBond {
            mode: CompanionMode::Follow,
            ..AnimalBond::owned(profile.uuid)
        },
    );
    if !state.valid() {
        return Err("Invalid pet-command fixture state".into());
    }
    terra_storage::save_edits(&path, config.world.id, &journal)?;
    let (host, token) = terra_server::IntegratedServer::start_with_save(config, Some(path))?;
    Ok((host, token, profile))
}

#[derive(Resource)]
struct Check {
    started: Instant,
    since: Instant,
    stage: u8,
    opening_keys: bool,
    original: Vec3,
    resting: Vec3,
    captures: usize,
    bad_image: bool,
    drained: u8,
}
pub(crate) fn install(app: &mut App) {
    if !std::env::args().any(|a| a == "--smoke-test-pet-commands") {
        return;
    }
    app.insert_resource(Check {
        started: Instant::now(),
        since: Instant::now(),
        stage: 0,
        opening_keys: false,
        original: Vec3::ZERO,
        resting: Vec3::ZERO,
        captures: 0,
        bad_image: false,
        drained: 0,
    })
    .insert_resource(bevy::winit::WinitSettings::continuous())
    .insert_resource(terra_player::ControllerMode::Walk)
    .add_systems(PostStartup, setup)
    .add_systems(PreUpdate, aim.after(bevy::input::InputSystems))
    .add_systems(Last, run);
}
fn setup(world: &mut World) {
    let mut menu = world.resource_mut::<terra_ui::menu::MenuState>();
    menu.page = terra_ui::menu::MenuPage::Playing;
    menu.session_active = true;
    world.resource_mut::<terra_core::DebugSettings>().enabled = false;
    world.write_message(terra_player::CaptureRequest(true));
}
fn pet(world: &World) -> Option<Vec3> {
    world
        .resource::<ReplicatedCreatures>()
        .creatures
        .get(&PET)
        .map(|t| Vec3::from_array(t.current.position.map(|v| v as f32)))
}
fn aim(world: &mut World) {
    world
        .resource_mut::<bevy::input::mouse::AccumulatedMouseMotion>()
        .delta = Vec2::ZERO;
    for mut window in world
        .query_filtered::<&mut Window, With<PrimaryWindow>>()
        .iter_mut(world)
    {
        window.bypass_change_detection().focused = true;
    }
    if world.resource::<terra_ui::PetMenuView>().visible {
        return;
    }
    let Some(point) = pet(world).map(|p| p + Vec3::Y * 0.8) else {
        return;
    };
    let eye = world
        .query_filtered::<&Transform, With<terra_player::PlayerBody>>()
        .iter(world)
        .next()
        .map(|b| b.translation + Vec3::Y * terra_player::PLAYER_EYE_OFFSET);
    for (mut transform, mut look) in world.query_filtered::<(&mut Transform, &mut terra_player::CameraLook), With<terra_player::TerraCamera>>().iter_mut(world) {
        let rotation = Transform::from_translation(eye.unwrap_or(transform.translation)).looking_at(point, Vec3::Y).rotation;
        transform.rotation = rotation;
        if !look.rotation().abs_diff_eq(rotation, 0.000_01) { *look = default(); }
    }
}
fn key(world: &mut World, key_code: KeyCode, pressed: bool) -> Result<(), String> {
    let window = world
        .query_filtered::<Entity, With<PrimaryWindow>>()
        .single(world)
        .map_err(|e| e.to_string())?;
    world.write_message(KeyboardInput {
        key_code,
        logical_key: Key::Character("test".into()),
        state: if pressed {
            ButtonState::Pressed
        } else {
            ButtonState::Released
        },
        text: None,
        repeat: false,
        window,
    });
    Ok(())
}

fn binding(world: &mut World, action: GameAction, pressed: bool) -> Result<(), String> {
    match world
        .resource::<terra_core::ControlsConfig>()
        .binding(action)
    {
        InputBinding::Key(code) => key(world, code, pressed),
        InputBinding::Chord {
            modifier,
            key: code,
        } => {
            key(world, modifier, pressed)?;
            key(world, code, pressed)
        }
        InputBinding::Mouse(button) => {
            let window = world
                .query_filtered::<Entity, With<PrimaryWindow>>()
                .single(world)
                .map_err(|e| e.to_string())?;
            world.write_message(MouseButtonInput {
                button,
                state: if pressed {
                    ButtonState::Pressed
                } else {
                    ButtonState::Released
                },
                window,
            });
            Ok(())
        }
    }
}
fn open(world: &mut World, check: &mut Check) -> Result<bool, String> {
    if !world.resource::<terra_player::CameraInputState>().captured
        || world.resource::<terra_player::TargetCreature>().0 != Some(PET)
    {
        return Ok(false);
    }
    binding(world, GameAction::FlyDown, true)?;
    binding(world, GameAction::Place, true)?;
    check.opening_keys = true;
    Ok(true)
}
fn click(world: &mut World, id: &str) -> Result<(), String> {
    let entity = world
        .query::<(Entity, &tilt_ui::ElementId, &ComputedNode)>()
        .iter(world)
        .find(|(_, element, node)| element.0 == id && !node.is_empty())
        .map(|(e, _, _)| e)
        .ok_or_else(|| format!("Missing visible real pet control {id}"))?;
    let handler = world
        .get::<tilt_ui::StaticAttributes>(entity)
        .and_then(|a| a.attributes.iter().find(|a| a.name == "onclick"))
        .map(|a| a.value.clone())
        .ok_or("Pet control lacks real onclick binding")?;
    world.write_message(tilt_ui::HtmlEvent {
        target: entity,
        kind: "click",
        value: None,
        submitter: None,
        data: default(),
        form_data: None,
        handler,
    });
    Ok(())
}
fn capture(world: &mut World, name: &str) -> Result<(), String> {
    validate_presentation(world)?;
    let frame = element_bounds(world, "pet-command-frame")?;
    let nameplate = element_bounds(world, "pet-nameplate")?;
    let plaques = [
        element_bounds(world, "pet-follow-description")?,
        element_bounds(world, "pet-rest-description")?,
        element_bounds(world, "pet-none-description")?,
    ];
    let folder = std::env::var_os("TERRA_CAPTURE_DIR")
        .map(PathBuf::from)
        .ok_or("Set TERRA_CAPTURE_DIR for pet inspection")?;
    std::fs::create_dir_all(&folder).map_err(|e| e.to_string())?;
    world
        .spawn(Screenshot::primary_window())
        .observe(save_to_disk(folder.join(format!("pet-{name}.png"))))
        .observe(
            move |event: On<ScreenshotCaptured>, mut state: ResMut<Check>| {
                state.captures += 1;
                state.bad_image |= event.image.data.as_ref().is_none_or(|data| {
                    data.chunks_exact(4)
                        .step_by(16)
                        .filter(|p| p[..3].iter().any(|v| *v > 40))
                        .count()
                        < 1_000
                });
                let colors = frame_colors(&event.image, frame, nameplate);
                state.bad_image |= colors.is_none_or(|(gold, hearts)| gold < 80 || hearts < 5);
                let text_pixels = plaques.map(|area| plaque_text_pixels(&event.image, area));
                state.bad_image |= text_pixels
                    .iter()
                    .any(|count| count.is_none_or(|count| count < 16));
                info!(
                    ?colors,
                    ?text_pixels,
                    "Native pet frame gold borders and health-heart pixels"
                );
            },
        );
    Ok(())
}

fn element_bounds(world: &mut World, name: &str) -> Result<Rect, String> {
    world
        .query::<(
            &tilt_ui::ElementId,
            &ComputedNode,
            &bevy::ui::UiGlobalTransform,
        )>()
        .iter(world)
        .find(|(id, node, _)| id.0 == name && !node.is_empty())
        .map(|(_, node, transform)| {
            Rect::from_center_size(transform.transform_point2(Vec2::ZERO), node.size())
        })
        .ok_or_else(|| format!("Missing laid-out pet presentation element {name}"))
}

fn validate_presentation(world: &mut World) -> Result<(), String> {
    let window = world
        .query_filtered::<&Window, With<PrimaryWindow>>()
        .single(world)
        .map_err(|e| e.to_string())?;
    let screen = Vec2::new(
        window.physical_width() as f32,
        window.physical_height() as f32,
    );
    let mut bounds = Vec::new();
    for id in [
        "pet-follow",
        "pet-rest",
        "pet-none",
        "pet-follow-description",
        "pet-rest-description",
        "pet-none-description",
        "pet-nameplate",
        "pet-model-portrait",
    ] {
        let rect = element_bounds(world, id)?;
        if rect.min.min_element() < -2.0
            || rect.max.x > screen.x + 2.0
            || rect.max.y > screen.y + 2.0
        {
            return Err(format!(
                "Pet element {id} lies outside {screen:?}: {rect:?}"
            ));
        }
        for (other_id, other) in &bounds {
            if rect.intersect(*other).size().min_element() > 1.0 {
                return Err(format!("Pet presentation overlaps: {id} / {other_id}"));
            }
        }
        bounds.push((id, rect));
    }
    let text: Vec<_> = world
        .query::<&bevy::ui::widget::Text>()
        .iter(world)
        .map(|t| t.0.clone())
        .collect();
    for expected in [
        "Stays close to you",
        "Stays at the current position",
        "Does nothing",
    ] {
        if !text.iter().any(|t| t == expected) {
            return Err(format!("Missing rendered pet description {expected}"));
        }
    }
    for (host, expected, centered) in [
        ("pet-follow-label", "Follow", true),
        ("pet-rest-label", "Rest", true),
        ("pet-none-label", "None", true),
        ("pet-follow-description", "Stays close to you", false),
        ("pet-follow-description", "Follow", false),
        (
            "pet-rest-description",
            "Stays at the current position",
            false,
        ),
        ("pet-none-description", "Does nothing", false),
        ("pet-rest-description", "Rest", false),
        ("pet-none-description", "None", false),
    ] {
        validate_text_bounds(world, host, expected, centered)?;
    }
    let portrait = world.resource::<terra_ui::PetPortrait>();
    if portrait.id != Some(PET) || !portrait.ready || portrait.image == Handle::default() {
        return Err("Pet portrait is not the actual ready owned creature".into());
    }
    Ok(())
}

fn validate_text_bounds(
    world: &mut World,
    host: &str,
    expected: &str,
    centered: bool,
) -> Result<(), String> {
    let area = element_bounds(world, host)?;
    let candidates: Vec<_> = world
        .query::<(Entity, &bevy::ui::widget::Text)>()
        .iter(world)
        .filter(|(_, text)| text.0 == expected)
        .map(|(entity, _)| entity)
        .collect();
    for entity in candidates {
        let mut ancestor = entity;
        let mut belongs = false;
        for _ in 0..12 {
            if world
                .get::<tilt_ui::ElementId>(ancestor)
                .is_some_and(|id| id.0 == host)
            {
                belongs = true;
                break;
            }
            let Some(parent) = world.get::<ChildOf>(ancestor) else {
                break;
            };
            ancestor = parent.parent();
        }
        if !belongs {
            continue;
        }
        let node = world
            .get::<ComputedNode>(entity)
            .ok_or("Pet text lacks computed layout")?;
        let transform = world
            .get::<bevy::ui::UiGlobalTransform>(entity)
            .ok_or("Pet text lacks global placement")?;
        let text = Rect::from_center_size(transform.transform_point2(Vec2::ZERO), node.size());
        if text.min.x < area.min.x - 2.0
            || text.min.y < area.min.y - 2.0
            || text.max.x > area.max.x + 2.0
            || text.max.y > area.max.y + 2.0
        {
            return Err(format!(
                "Pet text {expected} overflows {host}: {text:?} / {area:?}"
            ));
        }
        if centered && (text.center().x - area.center().x).abs() > 2.0 {
            return Err(format!("Pet label {expected} is not centered in {host}"));
        }
        let layout = world
            .get::<bevy::text::TextLayoutInfo>(entity)
            .ok_or("Pet text lacks actual glyph layout")?;
        if layout.glyphs.is_empty() {
            return Err(format!("No actual glyphs for pet text {expected}"));
        }
        let mut actual = Rect {
            min: Vec2::splat(f32::INFINITY),
            max: Vec2::splat(f32::NEG_INFINITY),
        };
        for glyph in &layout.glyphs {
            let bounds = glyph_bounds(
                transform,
                node.content_box().min,
                glyph.position,
                glyph.atlas_info.rect.size(),
            );
            actual.min = actual.min.min(bounds.min);
            actual.max = actual.max.max(bounds.max);
        }
        if actual.min.x < area.min.x - 4.0
            || actual.min.y < area.min.y - 4.0
            || actual.max.x > area.max.x + 4.0
            || actual.max.y > area.max.y + 4.0
        {
            return Err(format!(
                "Actual pet glyphs {expected} leave {host}: {actual:?} / {area:?}"
            ));
        }
        if centered && (actual.center().x - area.center().x).abs() > 4.0 {
            return Err(format!(
                "Actual pet glyphs {expected} are not centered in {host}"
            ));
        }
        return Ok(());
    }
    Err(format!("Pet description {expected} is missing from {host}"))
}

fn glyph_bounds(
    transform: &bevy::ui::UiGlobalTransform,
    origin: Vec2,
    position: Vec2,
    size: Vec2,
) -> Rect {
    let mut bounds = Rect {
        min: Vec2::splat(f32::INFINITY),
        max: Vec2::splat(f32::NEG_INFINITY),
    };
    // Match Bevy's extract_text_sections: content-box origin followed by each positioned glyph.
    for corner in [
        Vec2::new(-0.5, -0.5),
        Vec2::new(0.5, -0.5),
        Vec2::new(-0.5, 0.5),
        Vec2::new(0.5, 0.5),
    ] {
        let point = transform.transform_point2(origin + position + corner * size);
        bounds.min = bounds.min.min(point);
        bounds.max = bounds.max.max(point);
    }
    bounds
}

fn plaque_text_pixels(image: &Image, area: Rect) -> Option<usize> {
    let data = image.data.as_ref()?;
    let width = image.width() as usize;
    let height = image.height() as usize;
    let bgra = matches!(
        image.texture_descriptor.format,
        bevy::render::render_resource::TextureFormat::Bgra8Unorm
            | bevy::render::render_resource::TextureFormat::Bgra8UnormSrgb
    );
    let min = area.min + area.size() * Vec2::new(0.12, 0.14);
    let max = area.min + area.size() * Vec2::new(0.88, 0.68);
    let mut count = 0;
    for y in min.y.max(0.0) as usize..(max.y as usize).min(height) {
        for x in min.x.max(0.0) as usize..(max.x as usize).min(width) {
            let pixel = data.get((x + y * width) * 4..(x + y * width) * 4 + 4)?;
            let [red, green, blue] = if bgra {
                [pixel[2], pixel[1], pixel[0]]
            } else {
                [pixel[0], pixel[1], pixel[2]]
            };
            count += usize::from(red > 120 && green > 110 && blue > 75);
        }
    }
    Some(count)
}

fn frame_colors(image: &Image, frame: Rect, nameplate: Rect) -> Option<(usize, usize)> {
    let data = image.data.as_ref()?;
    let width = image.width() as usize;
    let height = image.height() as usize;
    let bgra = matches!(
        image.texture_descriptor.format,
        bevy::render::render_resource::TextureFormat::Bgra8Unorm
            | bevy::render::render_resource::TextureFormat::Bgra8UnormSrgb
    );
    let mut gold = 0;
    let mut hearts = 0;
    for y in frame.min.y.max(0.0) as usize..(frame.max.y as usize).min(height) {
        for x in frame.min.x.max(0.0) as usize..(frame.max.x as usize).min(width) {
            let pixel = data.get((x + y * width) * 4..(x + y * width) * 4 + 4)?;
            let [red, green, blue] = if bgra {
                [pixel[2], pixel[1], pixel[0]]
            } else {
                [pixel[0], pixel[1], pixel[2]]
            };
            gold += usize::from(
                red > 140
                    && f32::from(red) > f32::from(green) * 1.08
                    && f32::from(green) > f32::from(blue) * 1.35,
            );
            hearts += usize::from(
                nameplate.contains(Vec2::new(x as f32, y as f32))
                    && red > 150
                    && f32::from(red) > f32::from(green) * 1.6
                    && f32::from(red) > f32::from(blue) * 1.3,
            );
        }
    }
    Some((gold, hearts))
}
fn advance(check: &mut Check, stage: u8) {
    check.stage = stage;
    check.since = Instant::now();
    info!(stage, "Native pet-command stage");
}
fn run(world: &mut World) {
    world.resource_scope(|world, mut check: Mut<Check>| {
        if let Err(error) = step(world, &mut check) {
            error!(%error, stage=check.stage, "Native pet-command fixture failed");
            world.write_message(AppExit::error());
        }
    });
}
fn step(world: &mut World, check: &mut Check) -> Result<(), String> {
    if check.started.elapsed() > Duration::from_secs(60) {
        return Err(format!(
            "Pet-command timeout at stage{}; visible={}, mode={:?}",
            check.stage,
            world.resource::<terra_ui::PetMenuView>().visible,
            world
                .resource::<ReplicatedCreatures>()
                .bonds
                .get(&PET)
                .map(|b| b.mode)
        ));
    }
    if check.bad_image {
        return Err("Blank pet-command screenshot".into());
    }
    if check.opening_keys {
        binding(world, GameAction::Place, false)?;
        binding(world, GameAction::FlyDown, false)?;
        check.opening_keys = false;
    }
    let Some(position) = pet(world) else {
        return Ok(());
    };
    let mode = world
        .resource::<ReplicatedCreatures>()
        .bonds
        .get(&PET)
        .map(|b| b.mode);
    let view = world.resource::<terra_ui::PetMenuView>();
    let visible = view.visible;
    let displayed = visible
        && world.resource::<terra_ui::PetPortrait>().ready
        && world
            .query::<(&tilt_ui::ElementId, &ComputedNode)>()
            .iter(world)
            .any(|(id, node)| id.0 == "pet-follow" && !node.is_empty());
    let stage = check.stage;
    match stage {
        0 if check.since.elapsed() > Duration::from_secs(1) => {
            if mode != Some(CompanionMode::Wait) {
                return Err("Legacy Follow save activated before a UI command".into());
            }
            check.original = position;
            if open(world, check)? {
                advance(check, 1);
            }
        }
        1 if displayed && check.since.elapsed() > Duration::from_millis(500) => {
            if !world.resource::<terra_ui::PetMenuView>().rest {
                return Err("Default Rest wedge does not match authority".into());
            }
            capture(world, "rest")?;
            advance(check, 11);
        }
        2 if !visible
            && mode == Some(CompanionMode::Follow)
            && position.distance(check.original) > 0.3
            && open(world, check)? =>
        {
            advance(check, 3);
        }
        3 if displayed && check.since.elapsed() > Duration::from_millis(600) => {
            if !world.resource::<terra_ui::PetMenuView>().follow {
                return Err("Follow wedge is not the replicated command".into());
            }
            capture(world, "follow")?;
            advance(check, 12);
        }
        4 if !visible && mode == Some(CompanionMode::Wait) => {
            check.resting = position;
            advance(check, 5);
        }
        5 if check.since.elapsed() > Duration::from_secs(1) => {
            let delta = position - check.resting;
            if Vec2::new(delta.x, delta.z).length() > 0.03 {
                return Err("Explicit Rest still moves horizontally".into());
            }
            if open(world, check)? {
                advance(check, 6);
            }
        }
        6 if displayed && check.since.elapsed() > Duration::from_millis(600) => {
            click(world, "pet-none")?;
            advance(check, 7);
        }
        7 if !visible && mode == Some(CompanionMode::None) && open(world, check)? => {
            advance(check, 8);
        }
        8 if displayed && check.since.elapsed() > Duration::from_millis(300) => {
            if !world.resource::<terra_ui::PetMenuView>().free {
                return Err("None wedge is not the replicated command".into());
            }
            capture(world, "none")?;
            advance(check, 13);
        }
        11 if check.captures >= 1 => {
            click(world, "pet-follow")?;
            advance(check, 2);
        }
        12 if check.captures >= 2 => {
            click(world, "pet-rest")?;
            advance(check, 4);
        }
        13 if check.captures >= 3 => {
            for mut window in world
                .query_filtered::<&mut Window, With<PrimaryWindow>>()
                .iter_mut(world)
            {
                window.resolution.set(640.0, 480.0);
            }
            advance(check, 14);
        }
        14 if displayed && check.since.elapsed() > Duration::from_millis(800) => {
            if !world.resource::<terra_ui::PetMenuView>().compact {
                let viewport = world
                    .query_filtered::<&Window, With<PrimaryWindow>>()
                    .single(world)
                    .map_err(|e| e.to_string())?;
                return Err(format!(
                    "Narrow pet menu did not switch: viewport={:?}, UI scale={}",
                    viewport.size(),
                    world.resource::<UiScale>().0
                ));
            }
            capture(world, "compact")?;
            advance(check, 15);
        }
        15 if check.captures >= 4 => {
            binding(world, GameAction::Pause, true)?;
            advance(check, 9);
        }
        9 if !visible => {
            binding(world, GameAction::Pause, false)?;
            if world.resource::<terra_ui::menu::MenuState>().page
                != terra_ui::menu::MenuPage::Playing
            {
                return Err("Escape also opened Pause behind the pet menu".into());
            }
            advance(check, 10);
        }
        10 if check.captures == 4 => {
            let pending = world.query::<&Screenshot>().iter(world).count();
            check.drained = if pending == 0 {
                check.drained.saturating_add(1)
            } else {
                0
            };
            if check.drained >= 2 {
                info!(
                    captures = check.captures,
                    movement = position.distance(check.original),
                    "Native pet-command smoke passed: actual TiltUI, owned UDP commands, legacy migration, Follow motion, Rest, None, shared-model portrait, reference plaques, bounded compact layout, gold/heart GPU pixels and Escape"
                );
                world.write_message(AppExit::Success);
            }
        }
        _ => {}
    }
    Ok(())
}

#[cfg(test)]
mod glyph_tests {
    use super::*;
    #[test]
    fn actual_glyph_positions_are_checked_independently_of_the_ui_node_box() {
        let transform = bevy::ui::UiGlobalTransform::default();
        let node_box = Rect::from_center_size(Vec2::ZERO, Vec2::new(100.0, 20.0));
        let glyph = glyph_bounds(
            &transform,
            node_box.min,
            Vec2::new(-40.0, 10.0),
            Vec2::new(10.0, 12.0),
        );
        assert!(
            glyph.min.x < node_box.min.x,
            "a correct node must not hide a displaced text run"
        );
        let centered = glyph_bounds(
            &transform,
            node_box.min,
            Vec2::new(50.0, 10.0),
            Vec2::new(10.0, 12.0),
        );
        assert_eq!(centered.center(), Vec2::ZERO);
        assert!(node_box.contains(centered.min) && node_box.contains(centered.max));
    }
}
