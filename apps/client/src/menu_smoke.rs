//! Explicit fixture driver: real TiltUI handlers, files and production session lifecycle.
use bevy::{
    app::AppExit,
    input::{
        ButtonState,
        keyboard::{Key, KeyboardInput},
    },
    prelude::*,
    window::PrimaryWindow,
};
use std::{
    path::PathBuf,
    time::{Duration, Instant},
};
use terra_core::{
    AudioConfig, Config, ControlsConfig, GameAction, GameplayConfig, GraphicsConfig, InputBinding,
    MapMode,
};
use terra_ui::{ElementId, HtmlEvent, UiDocumentState, UiLoadState, menu::*};

#[derive(Resource)]
struct MenuSmoke {
    started: Instant,
    advanced: Instant,
    stage: u16,
    output: PathBuf,
    session: bool,
    join: bool,
    lan: bool,
    pointer: bool,
    gameplay_only: bool,
    mob_health_only: bool,
    graphics_controls_only: bool,
    create_world_only: bool,
    origin: Vec3,
    edit_count: usize,
    loading_captured: bool,
    startup_captured: bool,
    startup_fade_captured: bool,
    startup_only: bool,
}

#[derive(Resource, Default)]
struct MenuPointer(Option<bevy::picking::pointer::Location>);

/// Keep fixture input in ECS. Native focus/motion can otherwise replace a
/// queued press location before the later release; never warp the OS cursor.
fn maintain_pointer(world: &mut World) {
    use bevy::picking::pointer::{PointerAction, PointerId, PointerInput};
    let Some(location) = world.resource::<MenuPointer>().0.clone() else {
        return;
    };
    for mut window in world
        .query_filtered::<&mut Window, With<PrimaryWindow>>()
        .iter_mut(world)
    {
        let window = window.bypass_change_detection();
        window.focused = true;
        window.set_cursor_position(Some(location.position));
    }
    world.write_message(PointerInput::new(
        PointerId::Mouse,
        location,
        PointerAction::Move { delta: Vec2::ZERO },
    ));
}

pub(crate) fn install(app: &mut App) {
    if !std::env::args().any(|a| a.starts_with("--smoke-test-menu")) {
        return;
    }
    if crate::localization_smoke::requested() {
        crate::localization_smoke::install(app);
        return;
    }
    if std::env::var("TERRA_MENU_REFERENCE_SMOKE").as_deref() == Ok("1") {
        crate::inventory_smoke::reference::install_menu(app);
        return;
    }
    let root = std::env::var_os("TERRA_MENU_SMOKE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            std::env::temp_dir().join(format!("teramic-menu-smoke-{}", std::process::id()))
        });
    let output = std::env::var_os("TERRA_MENU_SCREENSHOTS")
        .map(PathBuf::from)
        .unwrap_or_else(|| root.join("screenshots"));
    if let Err(error) = std::fs::create_dir_all(&output) {
        tracing::error!(%error,"Cannot create menu smoke output");
    }
    if std::env::args().any(|a| a == "--smoke-test-menu-small") {
        let graphics = GraphicsConfig {
            width: 960,
            height: 640,
            fps_limit: 75,
            ..default()
        };
        if let Err(error) = graphics.save(root.join("configs/graphics.toml")) {
            tracing::error!(%error, "Cannot prepare small-window menu fixture");
        }
    }
    // The fixture emits UI events without native window events. Keep its
    // reconciliation frames running even when the desktop gives another window focus.
    app.insert_resource(bevy::winit::WinitSettings::continuous())
        .init_resource::<MenuPointer>()
        .insert_resource(MenuPaths {
            configs: root.join("configs"),
            worlds: terra_storage::WorldLibrary {
                root: root.join("worlds"),
            },
        })
        .insert_resource(MenuSmoke {
            started: Instant::now(),
            advanced: Instant::now(),
            stage: 0,
            output,
            session: std::env::args().any(|a| a == "--smoke-test-menu-session"),
            join: std::env::args().any(|a| a == "--smoke-test-menu-join"),
            lan: std::env::args().any(|a| a == "--smoke-test-menu-lan"),
            pointer: std::env::args().any(|a| a == "--smoke-test-menu-pointer"),
            gameplay_only: std::env::args().any(|a| a == "--smoke-test-menu-gameplay"),
            mob_health_only: std::env::args().any(|a| a == "--smoke-test-menu-mob-health"),
            graphics_controls_only: std::env::args()
                .any(|a| a == "--smoke-test-menu-graphics-controls"),
            create_world_only: std::env::var("TERRA_CREATE_WORLD_SMOKE")
                .is_ok_and(|value| value == "1"),
            origin: Vec3::ZERO,
            edit_count: 0,
            loading_captured: false,
            startup_captured: false,
            startup_fade_captured: false,
            startup_only: std::env::var("TERRA_STARTUP_ONLY").is_ok_and(|value| value == "1"),
        })
        // ComputedNode assertions need this frame's PostUpdate layout.
        .add_systems(Last, run)
        .add_systems(
            First,
            maintain_pointer.in_set(bevy::picking::PickingSystems::PostInput),
        );
}
pub(crate) fn emit(world: &mut World, id: &str, value: Option<&str>) -> Result<(), String> {
    let direct = world
        .query::<(Entity, &ElementId)>()
        .iter(world)
        .find_map(|(entity, element)| (element.0 == id).then_some(entity));
    let entity = if let Some(entity) = direct {
        entity
    } else {
        let mut buttons = world
            .resource::<MenuView>()
            .buttons
            .iter()
            .chain(&world.resource::<WorldsView>().buttons)
            .chain(&world.resource::<ServersView>().buttons)
            .chain(&world.resource::<OptionsView>().buttons)
            .chain(&world.resource::<DialogView>().buttons);
        let button = buttons.find(|b| b.id == id).map(|b| b.title.clone());
        let row = world
            .resource::<OptionsView>()
            .rows
            .iter()
            .chain(&world.resource::<DialogView>().rows)
            .find(|r| r.id == id);
        let world_rule = world
            .resource::<DialogView>()
            .world_rules
            .iter()
            .find(|rule| rule.id == id);
        let structure = world
            .resource::<DialogView>()
            .structure_choices
            .iter()
            .find(|choice| choice.id == id);
        let (element_id, title, scope) = if let Some(title) = button {
            ("menu-action".to_string(), title, false)
        } else if let Some(row) = row {
            (format!("row-{}", row.kind), row.title.clone(), true)
        } else if let Some(rule) = world_rule {
            (
                if rule.kind == "switch" {
                    "row-switch"
                } else {
                    "row-rule-input"
                }
                .into(),
                rule.title.clone(),
                true,
            )
        } else if let Some(choice) = structure {
            ("row-switch".into(), choice.title.clone(), true)
        } else if let Some(card) = world
            .resource::<WorldsView>()
            .cards
            .iter()
            .find(|c| c.id == id)
        {
            ("world-card".into(), card.title.clone(), false)
        } else if let Some(card) = world
            .resource::<ServersView>()
            .cards
            .iter()
            .find(|c| c.id == id)
        {
            ("server-card".into(), card.title.clone(), false)
        } else {
            return Err(format!("Unknown menu control: {id}"));
        };
        let candidates: Vec<_> = world
            .query::<(Entity, &ElementId)>()
            .iter(world)
            .filter(|(_, element)| element.0 == element_id)
            .map(|(entity, _)| entity)
            .collect();
        let in_dialog = world.resource::<DialogView>().visible;
        candidates
            .into_iter()
            .filter(|&entity| !in_dialog || has_ancestor_class(world, entity, "modal-panel"))
            .filter_map(|entity| {
                if scope {
                    scope_distance(world, entity, &title).map(|distance| (distance, entity))
                } else {
                    subtree_has_text(world, entity, &title, 0).then_some((0, entity))
                }
            })
            .min_by_key(|(distance, _)| *distance)
            .map(|(_, entity)| entity)
            .ok_or_else(|| format!("TiltUI control missing: {id} ({title})"))?
    };
    let attribute = if value.is_some() {
        "onchange"
    } else {
        "onclick"
    };
    let handler = world
        .get::<tilt_ui::StaticAttributes>(entity)
        .and_then(|a| a.attributes.iter().find(|a| a.name == attribute))
        .map(|a| a.value.clone())
        .ok_or_else(|| format!("Missing handler on {id}"))?;
    world.write_message(HtmlEvent {
        target: entity,
        kind: if value.is_some() { "change" } else { "click" },
        value: value.map(str::to_owned),
        submitter: None,
        data: Default::default(),
        form_data: None,
        handler,
    });
    Ok(())
}
fn has_ancestor_class(world: &World, mut entity: Entity, class: &str) -> bool {
    for _ in 0..24 {
        if world
            .get::<tilt_ui::ElementClasses>(entity)
            .is_some_and(|classes| classes.classes.iter().any(|value| value == class))
        {
            return true;
        }
        let Some(parent) = world.get::<ChildOf>(entity) else {
            return false;
        };
        entity = parent.parent();
    }
    false
}
fn subtree_has_text(world: &World, entity: Entity, title: &str, depth: usize) -> bool {
    if depth > 16 {
        return false;
    }
    world
        .get::<bevy::ui::widget::Text>(entity)
        .is_some_and(|text| text.0 == title || text.0 == terra_ui::localized_text(world, title))
        || world.get::<Children>(entity).is_some_and(|children| {
            children
                .iter()
                .any(|child| subtree_has_text(world, child, title, depth + 1))
        })
}
fn scope_distance(world: &World, mut entity: Entity, title: &str) -> Option<usize> {
    for distance in 0..8 {
        if subtree_has_text(world, entity, title, 0) {
            return Some(distance);
        }
        entity = world.get::<ChildOf>(entity)?.parent();
    }
    None
}

fn setting_scroll_target(world: &mut World, title: &str) -> Result<(Entity, Entity), String> {
    let control = world
        .query::<(Entity, &ElementId, &tilt_ui::ChoiceBoxParts)>()
        .iter(world)
        .filter(|(_, id, _)| id.0 == "row-select")
        .filter_map(|(entity, _, _)| {
            scope_distance(world, entity, title).map(|distance| (distance, entity))
        })
        .min_by_key(|(distance, _)| *distance)
        .map(|(_, entity)| entity)
        .ok_or_else(|| format!("Graphics control missing for scrolling: {title}"))?;
    let mut ancestor = control;
    for _ in 0..24 {
        if world
            .get::<tilt_ui::ElementClasses>(ancestor)
            .is_some_and(|classes| classes.classes.iter().any(|class| class == "settings-list"))
        {
            return Ok((control, ancestor));
        }
        ancestor = world
            .get::<ChildOf>(ancestor)
            .ok_or("Graphics control has no settings-list ancestor")?
            .parent();
    }
    Err("Graphics settings-list ancestor exceeds the fixture's hierarchy limit".into())
}

fn scroll_setting_into_view(world: &mut World, title: &str) -> Result<(), String> {
    let (control, list) = setting_scroll_target(world, title)?;
    let target_y = world
        .get::<bevy::ui::UiGlobalTransform>(control)
        .ok_or("Graphics control has no transform")?
        .translation
        .y;
    let list_y = world
        .get::<bevy::ui::UiGlobalTransform>(list)
        .ok_or("Graphics scroll list has no transform")?
        .translation
        .y;
    let scale = world
        .get::<ComputedNode>(list)
        .ok_or("Graphics scroll list has no layout")?
        .inverse_scale_factor;
    world
        .get_mut::<bevy::ui::ScrollPosition>(list)
        .ok_or("Graphics list has no real scroll position")?
        .0
        .y += (target_y - list_y) * scale;
    Ok(())
}

fn require_setting_on_screen(world: &mut World, title: &str) -> Result<(), String> {
    let (control, list) = setting_scroll_target(world, title)?;
    let bounds = |entity| -> Result<(Vec2, Vec2), String> {
        let center = world
            .get::<bevy::ui::UiGlobalTransform>(entity)
            .ok_or("Graphics capture element has no transform")?
            .translation;
        let half = world
            .get::<ComputedNode>(entity)
            .ok_or("Graphics capture element has no layout")?
            .size()
            * 0.5;
        Ok((center - half, center + half))
    };
    let (minimum, maximum) = bounds(control)?;
    let (list_minimum, list_maximum) = bounds(list)?;
    require(
        minimum.cmpge(list_minimum - Vec2::ONE).all()
            && maximum.cmple(list_maximum + Vec2::ONE).all(),
        &format!("Graphics control remains clipped after scrolling: {title}"),
    )
}

fn screenshot(world: &mut World, state: &MenuSmoke, name: &str) {
    world
        .spawn(bevy::render::view::screenshot::Screenshot::primary_window())
        .observe(bevy::render::view::screenshot::save_to_disk(
            state.output.join(format!("{name}.png")),
        ));
}
fn key(world: &mut World, key: KeyCode, pressed: bool) {
    if let Some(window) = world
        .query_filtered::<Entity, With<PrimaryWindow>>()
        .iter(world)
        .next()
    {
        world.write_message(KeyboardInput {
            key_code: key,
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
    }
}
/// Reuse the ordinary ECS pointer pipeline in an isolated opt-in native fixture.
pub(crate) fn install_test_pointer(app: &mut App) {
    app.init_resource::<MenuPointer>().add_systems(
        First,
        maintain_pointer.in_set(bevy::picking::PickingSystems::PostInput),
    );
}

pub(crate) fn pointer(
    world: &mut World,
    id: &str,
    title: &str,
    pressed: bool,
) -> Result<(), String> {
    use bevy::{
        camera::RenderTarget,
        picking::pointer::{Location, PointerAction, PointerButton, PointerId, PointerInput},
        window::WindowRef,
    };
    let window = world
        .query_filtered::<Entity, With<PrimaryWindow>>()
        .iter(world)
        .next()
        .ok_or("No primary window")?;
    let location = if pressed {
        let target = world
            .query::<(Entity, &ElementId)>()
            .iter(world)
            .find_map(|(entity, element)| {
                (element.0 == id && (title.is_empty() || subtree_has_text(world, entity, title, 0)))
                    .then_some(entity)
            })
            .ok_or_else(|| format!("Pointer target missing: {id}/{title}"))?;
        let position = world
            .get::<bevy::ui::UiGlobalTransform>(target)
            .ok_or("Pointer target has no layout")?
            .translation;
        let (minimum, maximum) = create_control_bounds(world, target)?;
        let viewport = world.get::<Window>(window).ok_or("No window")?;
        tracing::info!(
            id, title, ?minimum, ?maximum, ?position,
            physical_size = ?viewport.physical_size(),
            scale = viewport.resolution.scale_factor(),
            clip = ?world.get::<bevy::ui::CalculatedClip>(target),
            "Native pointer target computed geometry"
        );
        require(
            minimum.is_finite()
                && maximum.is_finite()
                && maximum.cmpgt(minimum).all()
                && position.cmpge(Vec2::ZERO).all()
                && position.cmplt(viewport.physical_size().as_vec2()).all()
                && world
                    .get::<bevy::ui::CalculatedClip>(target)
                    .is_none_or(|clip| clip.contains_point(position)),
            &format!("Native pointer target is outside its visible layout: {id}/{title}"),
        )?;
        let position = position / viewport.resolution.scale_factor();
        Location {
            target: RenderTarget::Window(WindowRef::Entity(window))
                .normalize(Some(window))
                .ok_or("No render target")?,
            position,
        }
    } else {
        world
            .resource::<MenuPointer>()
            .0
            .clone()
            .ok_or("Fixture mouse pointer location missing")?
    };
    world.resource_mut::<MenuPointer>().0 = Some(location.clone());
    maintain_pointer(world);
    // Exercise real hit testing and TiltUI click propagation without an OS cursor warp.
    world.write_message(PointerInput::new(
        PointerId::Mouse,
        location,
        if pressed {
            PointerAction::Press(PointerButton::Primary)
        } else {
            PointerAction::Release(PointerButton::Primary)
        },
    ));
    Ok(())
}
fn create_scroll_location(
    world: &mut World,
    id: &str,
) -> Result<bevy::picking::pointer::Location, String> {
    use bevy::{camera::RenderTarget, window::WindowRef};
    let window = world
        .query_filtered::<Entity, With<PrimaryWindow>>()
        .iter(world)
        .next()
        .ok_or("No primary window")?;
    let body = world
        .query::<(Entity, &ElementId)>()
        .iter(world)
        .find_map(|(entity, element)| (element.0 == id).then_some(entity))
        .ok_or_else(|| format!("Create World scroll body is missing: {id}"))?;
    let center = world
        .get::<bevy::ui::UiGlobalTransform>(body)
        .ok_or("Create World scroll body has no transform")?
        .translation;
    let scale = world
        .get::<Window>(window)
        .ok_or("No window")?
        .scale_factor();
    Ok(bevy::picking::pointer::Location {
        target: RenderTarget::Window(WindowRef::Entity(window))
            .normalize(Some(window))
            .ok_or("No render target")?,
        position: center / scale,
    })
}
fn move_to_create_scroll(world: &mut World) -> Result<(), String> {
    move_to_create_scroll_id(world, "world-create-scroll")
}
fn move_to_create_scroll_id(world: &mut World, id: &str) -> Result<(), String> {
    use bevy::picking::pointer::{PointerAction, PointerId, PointerInput};
    let location = create_scroll_location(world, id)?;
    world.resource_mut::<MenuPointer>().0 = Some(location.clone());
    maintain_pointer(world);
    world.write_message(PointerInput::new(
        PointerId::Mouse,
        location,
        PointerAction::Move { delta: Vec2::ZERO },
    ));
    Ok(())
}
fn wheel_create_world(world: &mut World) -> Result<(), String> {
    wheel_create_scroll_id(world, "world-create-scroll")
}
fn wheel_create_scroll_id(world: &mut World, id: &str) -> Result<(), String> {
    use bevy::picking::pointer::{PointerAction, PointerId, PointerInput};
    let location = create_scroll_location(world, id)?;
    world.resource_mut::<MenuPointer>().0 = Some(location.clone());
    maintain_pointer(world);
    // Exercise the real picking/wheel pipeline, including ancestor bubbling.
    // Pixel scrolling keeps this deterministic without waiting for smooth motion.
    world.write_message(PointerInput::new(
        PointerId::Mouse,
        location,
        PointerAction::Scroll {
            unit: bevy::input::mouse::MouseScrollUnit::Pixel,
            x: 0.0,
            y: -10_000.0,
            phase: bevy::input::touch::TouchPhase::Moved,
        },
    ));
    Ok(())
}
fn create_control_bounds(world: &World, entity: Entity) -> Result<(Vec2, Vec2), String> {
    let geometry = world
        .get::<ComputedNode>(entity)
        .ok_or("Create World control has no geometry")?;
    let transform = world
        .get::<bevy::ui::UiGlobalTransform>(entity)
        .ok_or("Create World control has no transform")?;
    let half = geometry.size() * 0.5;
    let corners = [
        Vec2::new(-half.x, -half.y),
        Vec2::new(half.x, -half.y),
        Vec2::new(-half.x, half.y),
        Vec2::new(half.x, half.y),
    ]
    .map(|point| transform.transform_point2(point));
    Ok(corners.into_iter().fold(
        (Vec2::splat(f32::INFINITY), Vec2::splat(f32::NEG_INFINITY)),
        |(low, high), point| (low.min(point), high.max(point)),
    ))
}
fn create_element(world: &mut World, id: &str) -> Result<Entity, String> {
    world
        .query::<(Entity, &ElementId)>()
        .iter(world)
        .find_map(|(entity, element)| (element.0 == id).then_some(entity))
        .ok_or_else(|| format!("Create World element is missing: {id}"))
}
fn create_descendant(world: &World, mut entity: Entity, ancestor: Entity) -> bool {
    for _ in 0..24 {
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
fn create_image(world: &World, entity: Entity, path: &str) -> Result<(), String> {
    require(
        world
            .get::<tilt_ui::ImageMetadata>(entity)
            .and_then(|m| m.source.as_deref())
            == Some(path),
        &format!("Biome image source does not match its choice: {path}"),
    )?;
    let node = world
        .get::<ImageNode>(entity)
        .ok_or_else(|| format!("Biome thumbnail has no rendered image: {path}"))?;
    let image = world
        .resource::<Assets<Image>>()
        .get(&node.image)
        .ok_or_else(|| format!("Biome thumbnail did not load: {path}"))?;
    require(
        image.width() >= 1024 && image.height() >= 512,
        &format!("Biome photo must contain genuine high-resolution source pixels: {path}"),
    )?;
    require(
        image.sampler == bevy::image::ImageSampler::linear(),
        &format!("Biome photo must use its own smooth image sampler: {path}"),
    )
}

fn validate_selected_biome(world: &mut World, expected: &str) -> Result<(), String> {
    let view = world.resource::<DialogView>();
    require(
        view.create
            && !view.biome_picker_open
            && !world.resource::<WorldMenu>().biome_picker_open
            && view.selected_biome.command == format!("world-biome-{expected}"),
        "Biome selection did not close the chooser and update the selected preview",
    )?;
    let selected = view.selected_biome.clone();
    require(
        !world
            .query::<&ElementId>()
            .iter(world)
            .any(|id| id.0 == "world-biome-picker"),
        "Closed biome chooser remains in the rendered UI",
    )?;
    let preview = world
        .query::<(Entity, &tilt_ui::ElementClasses)>()
        .iter(world)
        .find_map(|(entity, classes)| {
            classes
                .classes
                .iter()
                .any(|class| class == "world-biome-selected")
                .then_some(entity)
        })
        .ok_or("Selected biome card is missing")?;
    require(
        subtree_has_text(world, preview, &selected.title, 0),
        "Selected biome card shows the wrong label",
    )?;
    let image = world
        .query::<(Entity, &tilt_ui::ElementClasses)>()
        .iter(world)
        .find_map(|(entity, classes)| {
            (classes
                .classes
                .iter()
                .any(|class| class == "world-biome-preview")
                && create_descendant(world, entity, preview))
            .then_some(entity)
        })
        .ok_or("Selected biome card has no thumbnail")?;
    create_image(world, image, &selected.image)?;
    let (card_low, card_high) = create_control_bounds(world, preview)?;
    let (image_low, image_high) = create_control_bounds(world, image)?;
    require(
        (image_high - image_low).min_element() > 16.0
            && image_low.cmpge(card_low - Vec2::ONE).all()
            && image_high.cmple(card_high + Vec2::ONE).all(),
        "Selected biome thumbnail escapes its preview card",
    )
}
fn validate_biome_picker(world: &mut World, expected: &str, scrolled: bool) -> Result<(), String> {
    use bevy::input_focus::{InputFocus, tab_navigation::TabGroup};
    require(
        world.resource::<WorldMenu>().biome_picker_open
            && world.resource::<DialogView>().biome_picker_open,
        "Native biome preview click did not open the chooser",
    )?;
    let choices = world.resource::<DialogView>().biome_choices.clone();
    require(
        choices.len() == 9,
        "Biome chooser does not offer all nine choices",
    )?;
    let popup = create_element(world, "world-biome-picker")?;
    require(
        world
            .get::<TabGroup>(popup)
            .is_some_and(|group| group.modal),
        "Biome chooser does not trap Tab navigation in a modal group",
    )?;
    require(
        world
            .get_resource::<InputFocus>()
            .and_then(InputFocus::get)
            .is_some_and(|entity| create_descendant(world, entity, popup)),
        "Keyboard focus escaped the biome chooser",
    )?;
    let scroll = create_element(world, "world-biome-picker-scroll")?;
    let (popup_low, popup_high) = create_control_bounds(world, popup)?;
    let (scroll_low, scroll_high) = create_control_bounds(world, scroll)?;
    let window = world
        .query_filtered::<&Window, With<PrimaryWindow>>()
        .iter(world)
        .next()
        .ok_or("No primary window")?;
    let viewport = Vec2::new(
        window.resolution.physical_width() as f32,
        window.resolution.physical_height() as f32,
    );
    require(
        popup_low.cmpge(-Vec2::ONE).all()
            && popup_high.cmple(viewport + Vec2::ONE).all()
            && scroll_low.cmpge(popup_low - Vec2::ONE).all()
            && scroll_high.cmple(popup_high + Vec2::ONE).all(),
        "Biome chooser or its scrolling container escaped its frame",
    )?;
    let geometry = world
        .get::<ComputedNode>(scroll)
        .ok_or("Biome chooser has no scroll geometry")?;
    let overflow = geometry.content_size().y - geometry.size().y;
    let position = world
        .get::<bevy::ui::ScrollPosition>(scroll)
        .ok_or("Biome chooser has no real scroll position")?
        .0
        .y;
    require(
        overflow > 2.0 && (!scrolled || position > 1.0),
        "Biome chooser has no overflow or its native wheel did not scroll",
    )?;
    let cards = world
        .query::<(Entity, &tilt_ui::ElementClasses)>()
        .iter(world)
        .filter(|(entity, classes)| {
            classes
                .classes
                .iter()
                .any(|class| class == "world-biome-tile")
                && create_descendant(world, *entity, popup)
        })
        .map(|(entity, _)| entity)
        .collect::<Vec<_>>();
    require(
        cards.len() == choices.len(),
        "Biome chooser cards are missing",
    )?;
    for choice in choices {
        require(
            choice.image.starts_with("tilt-ui://assets/biome-choice-")
                && choice.image.ends_with(".png"),
            &format!(
                "Biome choice does not use its reference thumbnail: {}",
                choice.title
            ),
        )?;
        let card = cards
            .iter()
            .copied()
            .find(|&entity| subtree_has_text(world, entity, &choice.title, 0))
            .ok_or_else(|| format!("Biome card is missing: {}", choice.title))?;
        let selected = world
            .get::<tilt_ui::ElementClasses>(card)
            .is_some_and(|classes| classes.classes.iter().any(|class| class == "selected"));
        require(
            selected == (choice.command == format!("world-biome-{expected}")),
            "Biome chooser did not retain the selected card",
        )?;
        let image = world
            .query::<(Entity, &tilt_ui::ElementClasses)>()
            .iter(world)
            .find_map(|(entity, classes)| {
                (classes
                    .classes
                    .iter()
                    .any(|class| class == "world-biome-image")
                    && create_descendant(world, entity, card))
                .then_some(entity)
            })
            .ok_or_else(|| format!("Biome card has no image: {}", choice.title))?;
        create_image(world, image, &choice.image)?;
        let (card_low, card_high) = create_control_bounds(world, card)?;
        let (image_low, image_high) = create_control_bounds(world, image)?;
        require(
            image_low.cmpge(card_low - Vec2::ONE).all()
                && image_high.cmple(card_high + Vec2::ONE).all()
                && (image_high - image_low).min_element() > 32.0,
            &format!("Biome thumbnail escapes its card: {}", choice.title),
        )?;
    }
    tracing::info!(
        position,
        overflow,
        "Native biome cards, thumbnails and scroll geometry verified"
    );
    Ok(())
}
fn validate_biome_focus_restored(world: &mut World) -> Result<(), String> {
    let preview = create_element(world, "world-biome-picker-open")?;
    require(
        world
            .get_resource::<bevy::input_focus::InputFocus>()
            .and_then(bevy::input_focus::InputFocus::get)
            == Some(preview),
        "Closing the biome chooser did not restore focus to its selected preview",
    )
}
fn validate_create_rule_cards(world: &mut World) -> Result<(), String> {
    let structures = world.resource::<DialogView>().structure_choices.clone();
    require(
        structures.len() == 5,
        "Create World must show all five structure choices",
    )?;
    for id in [
        "world-structure-villages",
        "world-structure-kingdoms",
        "world-structure-outposts",
        "world-structure-ruins",
        "world-structure-mines",
    ] {
        require(
            structures.iter().any(|choice| choice.id == id),
            "Missing structure option",
        )?;
    }
    let structure_cards = world
        .query::<(Entity, &tilt_ui::ElementClasses)>()
        .iter(world)
        .filter(|(entity, classes)| {
            classes
                .classes
                .iter()
                .any(|class| class == "world-structure-card")
                && has_ancestor_class(world, *entity, "world-create-form")
        })
        .count();
    require(
        structure_cards == 5,
        "Every structure choice must have its own rendered card",
    )?;
    require(
        !world.query::<&ElementId>().iter(world).any(|id| {
            matches!(
                id.0.as_str(),
                "world-structures-all-on" | "world-structures-all-off"
            )
        }),
        "Create World still has redundant All On / All Off buttons",
    )?;
    let rules = world.resource::<DialogView>().world_rules.clone();
    let mut actual = rules
        .iter()
        .map(|rule| rule.name.as_str())
        .collect::<Vec<_>>();
    let mut registered = terra_core::GameRule::ALL
        .iter()
        .map(|rule| rule.name())
        .collect::<Vec<_>>();
    actual.sort_unstable();
    registered.sort_unstable();
    require(
        actual == registered,
        "Create World does not show every registered gamerule",
    )?;
    require(
        rules.iter().filter(|rule| rule.kind == "switch").count() == 6
            && rules.iter().filter(|rule| rule.kind == "number").count() == 3,
        "Create World gamerule card types do not match the real rule types",
    )?;
    let cards = world
        .query::<(Entity, &tilt_ui::ElementClasses)>()
        .iter(world)
        .filter(|(entity, classes)| {
            classes
                .classes
                .iter()
                .any(|class| class == "world-rule-card")
                && has_ancestor_class(world, *entity, "world-create-form")
        })
        .map(|(entity, _)| entity)
        .collect::<Vec<_>>();
    require(
        cards.len() == rules.len(),
        "Some gamerules are outside the card layout",
    )?;
    for rule in &rules {
        let card = cards
            .iter()
            .copied()
            .find(|&entity| subtree_has_text(world, entity, &rule.title, 0))
            .ok_or_else(|| format!("Missing gamerule card: {}", rule.name))?;
        let expected = if rule.kind == "switch" {
            "row-switch"
        } else {
            "row-rule-input"
        };
        let controls = world
            .query::<(Entity, &ElementId)>()
            .iter(world)
            .filter(|(_, id)| id.0 == expected)
            .map(|(entity, _)| entity)
            .collect::<Vec<_>>();
        let control = controls
            .into_iter()
            .filter_map(|entity| {
                scope_distance(world, entity, &rule.title).map(|distance| (distance, entity))
            })
            .min_by_key(|(distance, _)| *distance)
            .map(|(_, entity)| entity)
            .ok_or_else(|| format!("Missing typed control in gamerule card: {}", rule.name))?;
        let (card_low, card_high) = create_control_bounds(world, card)?;
        let (control_low, control_high) = create_control_bounds(world, control)?;
        require(
            (control_high - control_low).min_element() > 8.0
                && control_low.cmpge(card_low - Vec2::splat(1.5)).all()
                && control_high.cmple(card_high + Vec2::splat(1.5)).all(),
            &format!("Gamerule control escapes its card: {}", rule.name),
        )?;
        if rule.kind != "switch" {
            require(
                world.get::<bevy::text::EditableText>(control).is_some(),
                &format!("Gamerule input is not editable: {}", rule.name),
            )?;
        }
    }
    tracing::info!(
        cards = rules.len(),
        numeric_inputs = 3,
        "Native typed gamerule cards verified"
    );
    Ok(())
}
fn validate_create_world_scroll(world: &mut World) -> Result<(), String> {
    let body = world
        .query::<(Entity, &ElementId)>()
        .iter(world)
        .find_map(|(entity, id)| (id.0 == "world-create-scroll").then_some(entity))
        .ok_or("Create World scroll body is missing")?;
    let geometry = world
        .get::<ComputedNode>(body)
        .ok_or("Create World scroll body has no layout")?;
    let scroll = world
        .get::<bevy::ui::ScrollPosition>(body)
        .ok_or("Create World has no real scroll position")?
        .0
        .y;
    let overflow = geometry.content_size().y - geometry.size().y;
    require(
        overflow <= 2.0 || scroll > 1.0,
        "Mouse wheel did not scroll the overflowing Create World form",
    )?;
    let frame = world
        .query::<(Entity, &ElementId)>()
        .iter(world)
        .find_map(|(entity, id)| (id.0 == "menu-dialog").then_some(entity))
        .ok_or("Missing Create World frame")?;
    let footer = world
        .query::<(Entity, &tilt_ui::ElementClasses)>()
        .iter(world)
        .find_map(|(entity, classes)| {
            classes
                .classes
                .iter()
                .any(|class| class == "copper-dialog-actions")
                .then_some(entity)
        })
        .ok_or("Missing Create World action bar")?;
    let (frame_low, frame_high) = create_control_bounds(world, frame)?;
    let (footer_low, footer_high) = create_control_bounds(world, footer)?;
    require(
        footer_low.cmpge(frame_low - Vec2::ONE).all()
            && footer_high.cmple(frame_high + Vec2::ONE).all(),
        "Create World footer escaped its frame",
    )?;
    let bottom_inset = (frame_high.y - footer_high.y)
        * world
            .get::<ComputedNode>(frame)
            .ok_or("Create World frame has no geometry")?
            .inverse_scale_factor();
    require(
        bottom_inset >= 31.5,
        &format!("Create World footer is too close to its frame: {bottom_inset:.1}px"),
    )?;
    let (_, body_high) = create_control_bounds(world, body)?;
    require(
        body_high.y <= footer_low.y + 2.0,
        "Create World scrolling content overlaps its fixed footer",
    )?;
    tracing::info!(
        scroll,
        overflow,
        bottom_inset,
        "Native Create World wheel and footer bounds verified"
    );
    Ok(())
}
fn validate_created_world_options(world: &mut World) -> Result<(), String> {
    let id = world
        .resource::<WorldMenu>()
        .selected
        .ok_or("Created world is not selected")?;
    let library = world.resource::<MenuPaths>().worlds.clone();
    let saved = terra_storage::WorldLibrary { root: library.root }.load(id)?;
    let settings = &saved.manifest.world;
    require(
        saved.mode == terra_core::GameMode::Spectator,
        "Spectator game mode was not persisted",
    )?;
    require(
        settings.preferred_spawn_biome == Some(terra_core::BiomeKind::CherryForest),
        "Desired biome was not persisted",
    )?;
    let rules = settings.game_rules;
    require(
        rules.keep_inventory
            && rules.pvp
            && !rules.daylight_cycle
            && !rules.weather_cycle
            && !rules.mob_griefing
            && !rules.fire_spread,
        "Create World gamerules were not persisted",
    )?;
    require(
        (rules.sleep_percentage - 35.0).abs() < 0.001
            && (rules.game_time_speed - 1.5).abs() < 0.001,
        "Create World numeric gamerules were not persisted",
    )?;
    require(
        [
            settings.seasons.spring_days,
            settings.seasons.summer_days,
            settings.seasons.autumn_days,
            settings.seasons.winter_days,
        ] == [24; 4],
        "Create World season interval was not persisted",
    )?;
    require(
        !settings.structures.villages
            && settings.structures.ruins
            && !settings.structures.mines
            && !settings.structures.kingdoms
            && !settings.structures.outposts,
        "Individual structure flags were not persisted",
    )?;
    Ok(())
}
fn require(condition: bool, message: &str) -> Result<(), String> {
    if condition {
        Ok(())
    } else {
        Err(message.into())
    }
}

fn validate_minimap_popup(world: &mut World) -> Result<(), String> {
    let viewport = world
        .query_filtered::<&Window, With<PrimaryWindow>>()
        .iter(world)
        .next()
        .ok_or("Primary window missing")?;
    let viewport = Vec2::new(
        viewport.resolution.physical_width() as f32,
        viewport.resolution.physical_height() as f32,
    );
    let panel = world
        .query::<(Entity, &tilt_ui::ElementClasses)>()
        .iter(world)
        .find(|(_, classes)| {
            classes
                .classes
                .iter()
                .any(|class| class == "minimap-options")
        })
        .map(|(entity, _)| entity)
        .ok_or("MiniMap settings panel missing")?;
    let panel_node = world
        .get::<ComputedNode>(panel)
        .ok_or("MiniMap panel has no layout")?;
    let panel_center = world
        .get::<bevy::ui::UiGlobalTransform>(panel)
        .ok_or("MiniMap panel has no transform")?
        .translation;
    let panel_min = panel_center - panel_node.size() * 0.5;
    let panel_max = panel_center + panel_node.size() * 0.5;
    require(
        panel_min.min_element() >= -1.0
            && panel_max.x <= viewport.x + 1.0
            && panel_max.y <= viewport.y + 1.0,
        "MiniMap settings popup extends beyond the viewport",
    )?;
    let done = world
        .query::<(Entity, &ElementId)>()
        .iter(world)
        .find(|(entity, id)| {
            id.0 == "menu-action"
                && has_ancestor_class(world, *entity, "minimap-options")
                && subtree_has_text(world, *entity, "Done", 0)
        })
        .map(|(entity, _)| entity)
        .ok_or("MiniMap popup Done button missing")?;
    let done_node = world
        .get::<ComputedNode>(done)
        .ok_or("MiniMap Done has no layout")?;
    let done_center = world
        .get::<bevy::ui::UiGlobalTransform>(done)
        .ok_or("MiniMap Done has no transform")?
        .translation;
    require(
        done_node.size().min_element() > 0.0
            && done_center.y + done_node.size().y * 0.5 <= panel_max.y + 1.0,
        "MiniMap popup footer is clipped",
    )
}
fn run(world: &mut World) {
    let Some(mut state) = world.remove_resource::<MenuSmoke>() else {
        return;
    };
    if state.stage == u16::MAX {
        // An error capture exits after ScreenshotCaptured, keeping the actual
        // failed layout available for diagnosis instead of closing this frame.
        if state.advanced.elapsed() > Duration::from_secs(5) {
            world.write_message(AppExit::error());
        }
        world.insert_resource(state);
        return;
    }
    let result = if state.started.elapsed()
        > Duration::from_secs(if state.startup_only {
            180
        } else if state.graphics_controls_only {
            45
        } else if state.mob_health_only {
            25
        } else {
            150
        }) {
        Err(format!("Menu smoke timed out at stage {}", state.stage))
    } else if state.advanced.elapsed() < Duration::from_millis(450) {
        Ok(false)
    } else {
        step(world, &mut state)
    };
    match result {
        Ok(true) => {
            state.stage += 1;
            state.advanced = Instant::now();
            tracing::info!(stage = state.stage, "Menu smoke advanced");
        }
        Ok(false) => {}
        Err(error) => {
            tracing::error!(%error,stage=state.stage,"Menu smoke failed");
            world
                .spawn(bevy::render::view::screenshot::Screenshot::primary_window())
                .observe(bevy::render::view::screenshot::save_to_disk(
                    state
                        .output
                        .join(format!("menu-error-stage-{}.png", state.stage)),
                ))
                .observe(
                    |_: On<bevy::render::view::screenshot::ScreenshotCaptured>,
                     mut exit: MessageWriter<AppExit>| {
                        exit.write(AppExit::error());
                    },
                );
            state.stage = u16::MAX;
            state.advanced = Instant::now();
        }
    }
    world.insert_resource(state);
}
fn step(world: &mut World, state: &mut MenuSmoke) -> Result<bool, String> {
    match state.stage {
        0 => {
            if !state.startup_only && state.started.elapsed() < Duration::from_secs(2) {
                return Ok(false);
            }
            if !matches!(
                world.resource::<UiDocumentState>().0.load,
                UiLoadState::Ready
            ) {
                return Ok(false);
            }
            if state.startup_only
                && !world
                    .get_resource::<terra_render::RenderPreviewStartup>()
                    .is_some_and(|startup| startup.required)
            {
                return Err("Startup-only menu smoke requires the frontend asset barrier".into());
            }
            if world
                .get_resource::<terra_ui::menu::StartupProgress>()
                .is_some_and(|progress| progress.active)
            {
                let progress = world.resource::<terra_ui::menu::StartupProgress>();
                if !progress.error.is_empty() {
                    return Err(format!("Startup assets failed: {}", progress.error));
                }
                if progress.fading {
                    let blend = progress.blend();
                    let view = world.resource::<MenuView>();
                    if !view.startup_fading {
                        return Ok(false);
                    }
                    require(
                        view.startup && view.loading && view.main && !view.buttons.is_empty(),
                        "Ready startup must retain the loader while revealing the actual menu",
                    )?;
                    if !state.startup_fade_captured && (0.3..=0.7).contains(&blend) {
                        let opacity: std::collections::HashMap<_, _> = world
                            .query::<(&ElementId, &tilt_ui::RuntimeComputedStyle)>()
                            .iter(world)
                            .filter(|(id, _)| {
                                matches!(id.0.as_str(), "menu-content" | "startup-overlay")
                            })
                            .map(|(id, style)| (id.0.clone(), style.0.opacity.unwrap_or(1.0)))
                            .collect();
                        let (Some(menu), Some(loader)) =
                            (opacity.get("menu-content"), opacity.get("startup-overlay"))
                        else {
                            return Ok(false);
                        };
                        require(
                            *menu > 0.05
                                && *menu < 0.95
                                && *loader > 0.05
                                && *loader < 0.95
                                && (menu + loader - 1.0).abs() < 0.02,
                            "Startup must render intermediate loader and menu opacity, not replace either instantly",
                        )?;
                        screenshot(world, state, "startup-transition");
                        state.startup_fade_captured = true;
                    }
                    return Ok(false);
                }
                let view = world.resource::<MenuView>();
                require(
                    view.startup && view.loading && !view.main && view.buttons.is_empty(),
                    "Startup must show the loading page before exposing main menu actions",
                )?;
                require(
                    !view.cancel_connect,
                    "Startup loading must not expose a quit/cancel action",
                )?;
                let session = world.resource::<terra_ui::menu::SessionView>();
                require(session.title == "Laden", "Startup title must be Laden")?;
                require(
                    session.progress_label.contains(" / ") && !session.detail.is_empty(),
                    "Startup must separate centered progress numbers from the pending asset path",
                )?;
                require(
                    !world
                        .query::<&ElementId>()
                        .iter(world)
                        .any(|element| element.0 == "quit"),
                    "Startup loading materialized a Quit Game button",
                )?;
                require(
                    !world
                        .query::<&ElementId>()
                        .iter(world)
                        .any(|element| element.0 == "menu-action"),
                    "Main menu buttons were materialized before assets were ready",
                )?;
                require(
                    world
                        .resource::<terra_world::streaming::ChunkStore>()
                        .loaded
                        .is_empty(),
                    "Startup loading must not stream a world",
                )?;
                require(
                    !world.contains_resource::<terra_server::IntegratedServer>(),
                    "Startup loading must not start a server",
                )?;
                if !state.startup_captured
                    && world
                        .query::<&ElementId>()
                        .iter(world)
                        .any(|element| element.0 == "asset-loading")
                {
                    let measured: std::collections::HashMap<_, _> = world
                        .query::<(&ElementId, &ComputedNode, &bevy::ui::UiGlobalTransform)>()
                        .iter(world)
                        .filter(|(id, _, _)| {
                            matches!(
                                id.0.as_str(),
                                "loading-asset-path" | "loading-progress-meter"
                            )
                        })
                        .map(|(id, node, transform)| {
                            (id.0.clone(), (node.size(), transform.translation))
                        })
                        .collect();
                    let (Some((path_size, path_center)), Some((bar_size, bar_center))) = (
                        measured.get("loading-asset-path"),
                        measured.get("loading-progress-meter"),
                    ) else {
                        return Ok(false);
                    };
                    require(
                        path_size.x > 0.0
                            && path_size.x < bar_size.x - 2.0
                            && (path_center.x - bar_center.x).abs() < 1.5,
                        "Pending asset text must use its intrinsic width and be centered below the bar",
                    )?;
                    screenshot(world, state, "startup-icons");
                    state.startup_captured = true;
                }
                return Ok(false);
            }
            if let Some(startup) = world
                .get_resource::<terra_render::RenderPreviewStartup>()
                .filter(|startup| startup.required)
            {
                if let Some(error) = &startup.error {
                    return Err(format!("Startup inventory preload failed: {error}"));
                }
                let pending = !startup.ready;
                let completed = startup.completed;
                let total = startup.total;
                if pending {
                    if !state.startup_captured
                        && completed > 0
                        && world.resource::<MenuView>().startup
                    {
                        screenshot(world, state, "startup-icons");
                        state.startup_captured = true;
                    }
                    return Ok(false);
                }
                require(
                    total > 0 && completed == total && startup.failed == 0,
                    "Startup catalog did not finish every finite item",
                )?;
                require(
                    world
                        .resource::<terra_render::RenderBlockPreviews>()
                        .pending
                        == 0,
                    "Startup catalog still has GPU jobs",
                )?;
                let published = world
                    .resource::<terra_render::RenderBlockPreviews>()
                    .images
                    .len()
                    + world
                        .resource::<terra_render::RenderPropPreviews>()
                        .images
                        .len()
                    + world
                        .resource::<terra_render::RenderItemPreviews>()
                        .images
                        .len();
                require(
                    published == total,
                    "Startup must own every finite inventory preview before exposing the menu",
                )?;
                let ui = world
                    .get_resource::<crate::bootstrap::startup_ui::UiStartupAssets>()
                    .ok_or("Startup UI asset preload is missing")?;
                if let Some(error) = &ui.error {
                    return Err(format!("Startup UI asset preload failed: {error}"));
                }
                if !ui.ready {
                    return Ok(false);
                }
                require(
                    ui.completed == ui.total && ui.icon_count() == tilt_ui::Icon::ALL.len() * 3,
                    "Startup UI icons, images or fonts are incomplete",
                )?;
                tracing::info!(
                    inventory_icons = total,
                    ui_assets = ui.total,
                    ui_icons = ui.icon_count(),
                    "Native startup catalog preload passed before world/session creation"
                );
            }
            if !world
                .query::<&ElementId>()
                .iter(world)
                .any(|element| element.0 == "menu-action")
            {
                return Ok(false);
            }
            require(
                world.resource::<MenuState>().page == MenuPage::Main,
                "Main menu must be first",
            )?;
            require(
                world
                    .resource::<terra_world::streaming::ChunkStore>()
                    .loaded
                    .is_empty(),
                "Main menu must not generate terrain",
            )?;
            require(
                !world.contains_resource::<terra_server::IntegratedServer>(),
                "Main menu started a server",
            )?;
            if state.startup_only {
                require(
                    state.startup_captured && state.startup_fade_captured,
                    "Startup must capture both the loading page and animated transition before the main menu",
                )?;
                state.stage = 9_999;
                return Ok(true);
            }
            screenshot(world, state, "main-menu");
            if state.lan {
                emit(world, "multiplayer", None)?;
                state.stage = 399;
                return Ok(true);
            }
            if state.pointer {
                pointer(world, "menu-action", "Singleplayer", true)?;
                state.stage = 999;
                return Ok(true);
            }
            if state.gameplay_only {
                emit(world, "options", None)?;
                state.stage = 689;
                return Ok(true);
            }
            if state.mob_health_only {
                emit(world, "options", None)?;
                state.stage = 799;
                return Ok(true);
            }
            if state.graphics_controls_only {
                emit(world, "options", None)?;
                state.stage = 899;
                return Ok(true);
            }
            emit(world, "singleplayer", None)?;
        }
        1 => {
            require(
                world.resource::<WorldMenu>().entries.is_empty(),
                "Smoke directory must be fresh",
            )?;
            screenshot(world, state, "worlds-empty");
            emit(world, "world-create", None)?;
        }
        2 => {
            emit(world, "world-name", Some("Cedar Valley"))?;
            emit(world, "world-seed", Some("42042"))?;
            pointer(
                world,
                if state.create_world_only {
                    "world-mode-spectator"
                } else {
                    "world-mode-creative"
                },
                "",
                true,
            )?;
            state.stage = 299;
        }
        3 => {
            screenshot(world, state, "create-world");
            emit(world, "world-create-confirm", None)?;
        }
        4 => {
            require(
                world.resource::<WorldMenu>().entries.len() == 1,
                "Create World did not persist a real entry",
            )?;
            if world.resource::<DialogView>().visible {
                return Ok(false);
            }
            screenshot(world, state, "world-selected");
            if state.create_world_only {
                validate_created_world_options(world)?;
                tracing::info!(
                    "Native Create World passed: real Spectator button, scroll wheel and contained footer, saved gamerules, selected biome and individual structure flags"
                );
                world.write_message(AppExit::Success);
                state.stage = u16::MAX;
                return Ok(false);
            }
            emit(world, "back", None)?;
        }
        5 => {
            emit(world, "options", None)?;
        }
        6 => {
            let resolutions = world
                .resource::<OptionsView>()
                .rows
                .iter()
                .find(|row| row.id == "graphics-resolution")
                .ok_or("Resolution setting missing")?
                .choices
                .iter()
                .map(|choice| choice.value.clone())
                .collect::<Vec<_>>();
            let rendered = world
                .query::<&tilt_ui::OptionData>()
                .iter(world)
                .map(|option| option.value.clone())
                .collect::<Vec<_>>();
            require(
                resolutions.len() > 1 && resolutions.iter().all(|value| rendered.contains(value)),
                "Monitor resolutions did not materialize as real TiltUI dropdown options",
            )?;
            require(
                !world
                    .query::<&tilt_ui::OptionData>()
                    .iter(world)
                    .any(|option| option.label.contains("{{")),
                "Dropdown label contains unresolved template interpolation",
            )?;
            screenshot(world, state, "graphics");
            require(
                world
                    .resource::<OptionsView>()
                    .rows
                    .iter()
                    .any(|r| r.id == "graphics-ui-fps" && r.value == "60"),
                "UI FPS control missing or wrong default",
            )?;
            require(
                ["24", "30", "40", "60", "90", "120", "144", "240"]
                    .iter()
                    .all(|v| rendered.iter().any(|actual| actual == v)),
                "UI FPS choices did not materialize in TiltUI",
            )?;
            emit(world, "graphics-ui-fps", Some("120"))?;
            emit(world, "graphics-block-detail", Some("medium"))?;
            emit(world, "graphics-fov", Some("86"))?;
            emit(world, "graphics-shadow", Some("low"))?;
            emit(world, "graphics-fps", Some("60"))?;
            emit(world, "graphics-fog", Some("85"))?;
            emit(world, "graphics-chunk-animation", Some("fade"))?;
            emit(world, "graphics-scale", Some("90"))?;
        }
        7 => {
            let viewport = world
                .query_filtered::<&Window, With<PrimaryWindow>>()
                .iter(world)
                .next()
                .ok_or("Primary window missing")?
                .size();
            let expected_scale = (world.resource::<GraphicsConfig>().ui_scale
                * 1.0_f32.min(viewport.x / 1000.0).min(viewport.y / 740.0))
            .min(viewport.x / 900.0)
            .min(viewport.y / 640.0)
            .max(0.1);
            require(
                (world.resource::<bevy::ui::UiScale>().0 - expected_scale).abs() < 0.001,
                "UI Scale changed before Apply",
            )?;
            screenshot(world, state, "graphics-ui-scale");
            pointer(world, "row-select", "1280 x 800", true)?;
            state.stage = 599;
        }
        600 => {
            pointer(world, "row-select", "1280 x 800", false)?;
        }
        601 => {
            require(
                world
                    .query::<&tilt_ui::ChoiceBoxParts>()
                    .iter(world)
                    .any(|choice| choice.open),
                "Resolution popup did not remain open",
            )?;
            screenshot(world, state, "resolution-dropdown");
            emit(world, "apply", None)?;
            state.stage = 7;
        }
        8 => {
            require(
                world
                    .resource::<terra_core::GameConfig>()
                    .client
                    .fov_degrees
                    == 86.0,
                "Apply did not affect actual camera configuration",
            )?;
            let path = world.resource::<MenuPaths>().configs.join("graphics.toml");
            let saved = GraphicsConfig::load(path).map_err(|e| e.to_string())?;
            require(
                saved.fov == 86.0
                    && (saved.ui_scale - 0.9).abs() < 0.001
                    && saved.ui_fps == 120
                    && saved.blocks.detail_quality == terra_core::BlockDetailQuality::Medium,
                "Graphics TOML did not reload",
            )?;
            let viewport = world
                .query_filtered::<&Window, With<PrimaryWindow>>()
                .iter(world)
                .next()
                .ok_or("Primary window missing")?
                .size();
            let applied_scale = (saved.ui_scale
                * 1.0_f32.min(viewport.x / 1000.0).min(viewport.y / 740.0))
            .min(viewport.x / 900.0)
            .min(viewport.y / 640.0)
            .max(0.1);
            require(
                (world.resource::<bevy::ui::UiScale>().0 - applied_scale).abs() < 0.001,
                "Apply did not change actual UI Scale",
            )?;
            screenshot(world, state, "graphics-ui-scale-applied");
            emit(world, "controls", None)?;
        }
        9 => {
            screenshot(world, state, "controls");
            require(
                !world.resource::<ControlsConfig>().auto_climb,
                "Auto-Climb must default to off",
            )?;
            emit(world, "controls-auto-climb", Some("true"))?;
            emit(world, "bind-0", None)?;
        }
        10 => {
            require(
                world.resource::<OptionsDraft>().rebinding == Some(GameAction::Forward),
                "Binding field did not arm",
            )?;
            key(world, KeyCode::ArrowUp, true);
        }
        11 => {
            key(world, KeyCode::ArrowUp, false);
            require(
                world
                    .resource::<OptionsDraft>()
                    .controls
                    .binding(GameAction::Forward)
                    == InputBinding::Key(KeyCode::ArrowUp),
                "Next key was not captured",
            )?;
            emit(world, "apply", None)?;
        }
        12 => {
            require(
                world.resource::<ControlsConfig>().auto_climb,
                "Auto-Climb switch was not applied",
            )?;
            let controls_path = world.resource::<MenuPaths>().configs.join("controls.toml");
            require(
                ControlsConfig::load(controls_path)
                    .map_err(|e| e.to_string())?
                    .auto_climb,
                "Auto-Climb was not persisted",
            )?;
            require(
                world
                    .resource::<ControlsConfig>()
                    .binding(GameAction::Forward)
                    == InputBinding::Key(KeyCode::ArrowUp),
                "Gameplay bindings were not applied",
            )?;
            emit(world, "bind-1", None)?;
        }
        13 => {
            key(world, KeyCode::ArrowUp, true);
        }
        14 => {
            key(world, KeyCode::ArrowUp, false);
            require(
                world.resource::<MenuState>().error.contains("already"),
                "Binding conflict was not reported",
            )?;
            screenshot(world, state, "binding-conflict");
            key(world, KeyCode::Escape, true);
        }
        15 => {
            key(world, KeyCode::Escape, false);
            require(
                world.resource::<MenuState>().page == MenuPage::Options
                    && world.resource::<OptionsDraft>().rebinding.is_none(),
                "Escape closed menu while rebinding",
            )?;
            emit(world, "controls-auto-climb", Some("false"))?;
            emit(world, "apply", None)?;
            emit(world, "audio", None)?;
            state.stage = 499;
        }
        16 => {
            emit(world, "graphics-brightness", Some("110"))?;
        }
        17 => {
            emit(world, "back", None)?;
        }
        18 => {
            require(
                world.resource::<MenuState>().dialog == MenuDialog::Discard,
                "Unsaved settings were silently accepted",
            )?;
            screenshot(world, state, "discard");
            emit(world, "discard-confirm", None)?;
        }
        19 => {
            require(
                *world.resource::<AudioConfig>() == AudioConfig::default(),
                "Discard applied an unsaved audio draft",
            )?;
            emit(world, "multiplayer", None)?;
        }
        20 => {
            screenshot(world, state, "servers-empty");
            emit(world, "server-add", None)?;
        }
        21 => {
            emit(world, "server-name", Some("Local Workshop"))?;
            if state.join {
                let address = std::env::var("TERRA_MENU_JOIN_ADDRESS")
                    .map_err(|_| "TERRA_MENU_JOIN_ADDRESS is required")?
                    .parse::<std::net::SocketAddr>()
                    .map_err(|e| e.to_string())?;
                emit(world, "server-address", Some(&address.ip().to_string()))?;
                emit(world, "server-port", Some(&address.port().to_string()))?;
            } else {
                emit(world, "server-address", Some("127.0.0.1"))?;
                emit(world, "server-port", Some("5000"))?;
            }
        }
        22 => {
            screenshot(world, state, "add-server");
            emit(world, "server-save", None)?;
        }
        23 => {
            require(
                world.resource::<ServerMenu>().list.servers.len() == 1,
                "Add Server was not persisted",
            )?;
            screenshot(world, state, "server-selected");
            emit(world, "server-edit", None)?;
        }
        24 => {
            emit(world, "server-address", Some("http://invalid"))?;
        }
        25 => {
            emit(world, "server-save", None)?;
        }
        26 => {
            require(
                !world.resource::<MenuState>().error.is_empty(),
                "Invalid endpoint was accepted",
            )?;
            screenshot(world, state, "server-error");
            emit(world, "server-address", Some("localhost"))?;
        }
        27 => {
            emit(world, "server-save", None)?;
        }
        28 => {
            require(
                world.resource::<ServerMenu>().list.servers[0].address == "localhost",
                "Edit Server did not replace entry",
            )?;
            emit(world, "back", None)?;
        }
        29 => {
            if state.join {
                emit(world, "multiplayer", None)?;
                state.stage = 199;
            } else {
                emit(world, "options", None)?;
            }
        }
        30 => {
            let current = world.resource::<GraphicsConfig>();
            let next = if current.width == 1280 && current.height == 720 {
                "1280x800"
            } else {
                "1280x720"
            };
            emit(world, "graphics-resolution", Some(next))?;
        }
        31 => {
            emit(world, "apply", None)?;
        }
        32 => {
            require(
                world.resource::<MenuState>().dialog == MenuDialog::Display,
                "Display changes need confirmation",
            )?;
            screenshot(world, state, "display-confirm");
        }
        33 => {
            if world.resource::<OptionsDraft>().display.is_some() {
                return Ok(false);
            }
            require(
                world.resource::<MenuState>().dialog == MenuDialog::None,
                "Display rollback did not close dialog",
            )?;
            emit(world, "back", None)?;
        }
        34 => {
            emit(world, "singleplayer", None)?;
        }
        35 => {
            if state.session {
                emit(world, "world-play", None)?;
            } else {
                emit(world, "world-delete", None)?;
            }
        }
        36 => {
            if state.session {
                state.stage = 99;
                return Ok(true);
            }
            screenshot(world, state, "delete-world");
            emit(world, "world-delete-confirm", None)?;
        }
        37 => {
            require(
                world.resource::<WorldMenu>().entries.is_empty(),
                "Delete World left a manifest",
            )?;
            let path = world.resource::<MenuPaths>().configs.join("controls.toml");
            require(
                ControlsConfig::load(path)
                    .map_err(|e| e.to_string())?
                    .binding(GameAction::Forward)
                    == InputBinding::Key(KeyCode::ArrowUp),
                "Controls reload did not preserve binding",
            )?;
            tracing::info!(
                "Native menu smoke passed: pages, create/delete, server add/edit, controls, conflict, Escape, apply, discard, display timeout"
            );
            emit(world, "back", None)?;
        }
        38 => emit(world, "quit", None)?,
        100 => {
            if !state.loading_captured
                && world
                    .resource::<LoadingProgress>()
                    .fraction
                    .is_some_and(|fraction| (0.05..0.95).contains(&fraction))
            {
                screenshot(world, state, "loading-screen");
                state.loading_captured = true;
            }
            if !world
                .resource::<terra_network::client::ClientSession>()
                .ready
                || !world.resource::<terra_render::RenderStatus>().ready
                || world.resource::<MenuState>().page != MenuPage::Playing
            {
                return Ok(false);
            }
            let Some(body) = world
                .query_filtered::<&Transform, With<terra_player::PlayerBody>>()
                .iter(world)
                .next()
            else {
                return Ok(false);
            };
            state.origin = body.translation;
            require(
                world.contains_resource::<terra_server::IntegratedServer>(),
                "Play World did not start an integrated host",
            )?;
            world
                .resource_mut::<terra_player::CameraInputState>()
                .captured = true;
            key(world, KeyCode::ArrowUp, true);
        }
        101 => {
            if state.advanced.elapsed() < Duration::from_secs(1) {
                return Ok(false);
            }
            key(world, KeyCode::ArrowUp, false);
            let confirmed = world
                .resource::<terra_network::client::ClientSession>()
                .confirmed_state()
                .ok_or("No server state")?;
            require(
                Vec3::from_array(confirmed.position).distance(state.origin) > 1.0,
                "Rebound forward key did not move authoritative player",
            )?;
            screenshot(world, state, "playing");
            let mut cameras =
                world.query_filtered::<&mut Transform, With<terra_player::TerraCamera>>();
            for mut camera in cameras.iter_mut(world) {
                camera.rotation = Quat::from_euler(EulerRot::YXZ, 0.0, -1.4, 0.0);
            }
        }
        102 => match world.resource::<terra_player::TargetWorld>().0 {
            Some(terra_player::WorldTarget::Block(hit)) => {
                let material = world
                    .resource::<terra_world::streaming::ChunkStore>()
                    .block_at(hit.position)
                    .ok_or("Missing target")?;
                world.write_message(terra_world::streaming::BlockEditRequest {
                    position: hit.position,
                    expected_material: material,
                    material: 0,
                });
            }
            Some(terra_player::WorldTarget::Prop(hit)) => {
                world.write_message(terra_world::streaming::PropRemoveRequest {
                    section: hit.prop.section,
                    id: hit.prop.id,
                });
            }
            None => return Err("No natural target for session save regression".into()),
        },
        103 => {
            let journal = world.resource::<terra_world::edits::EditJournal>();
            state.edit_count = journal.edited_blocks() + journal.removed_props();
            if state.edit_count == 0 {
                return Ok(false);
            }
            key(world, KeyCode::Escape, true);
        }
        104 => {
            key(world, KeyCode::Escape, false);
            require(
                world.resource::<MenuState>().page == MenuPage::Pause,
                "Escape did not open Game Menu",
            )?;
            screenshot(world, state, "game-menu");
            emit(world, "save-world", None)?;
            state.stage = 179;
        }
        105 => {
            if world.resource::<MenuState>().page != MenuPage::Main {
                return Ok(false);
            }
            require(
                !world.contains_resource::<terra_server::IntegratedServer>(),
                "Returning to menu retained the integrated server",
            )?;
            emit(world, "singleplayer", None)?;
        }
        106 => {
            emit(world, "world-play", None)?;
        }
        107 => {
            if !world
                .resource::<terra_network::client::ClientSession>()
                .ready
                || world.resource::<MenuState>().page != MenuPage::Playing
            {
                return Ok(false);
            }
            let journal = world.resource::<terra_world::edits::EditJournal>();
            require(
                journal.edited_blocks() + journal.removed_props() == state.edit_count,
                "Saved journal did not survive host restart",
            )?;
            world.write_message(MenuAction::Command("pause".into()));
        }
        108 => {
            emit(world, "leave", None)?;
        }
        109 => {
            if world.resource::<MenuState>().page != MenuPage::Main {
                return Ok(false);
            }
            tracing::info!(
                "Native menu session smoke passed: integrated host, rebound movement, authoritative edit, final save, host restart and journal reload"
            );
            world.write_message(AppExit::Success);
        }
        200 => {
            emit(world, "server-join", None)?;
        }
        201 => {
            let session = world.resource::<terra_network::client::ClientSession>();
            if std::env::var_os("TERRA_MENU_EXPECT_CONNECTION_FAILURE").is_some() {
                require(
                    !session.ready,
                    "Unreachable-server fixture unexpectedly connected",
                )?;
                if world.resource::<MenuState>().error.is_empty() {
                    return Ok(false);
                }
                if !world
                    .query::<&bevy::ui::widget::Text>()
                    .iter(world)
                    .any(|text| text.0 == world.resource::<MenuState>().error)
                {
                    return Ok(false);
                }
                screenshot(world, state, "connection-failed");
                emit(world, "cancel-connect", None)?;
                state.stage = 209;
                return Ok(true);
            }
            if let Some(error) = &session.error {
                return Err(error.clone());
            }
            if !session.ready || world.resource::<MenuState>().page != MenuPage::Playing {
                return Ok(false);
            }
            require(
                !world.contains_resource::<terra_server::IntegratedServer>(),
                "Join Server started a second local host",
            )?;
            screenshot(world, state, "dedicated-joined");
            world.write_message(MenuAction::Command("pause".into()));
        }
        202 => {
            emit(world, "leave", None)?;
        }
        203 => {
            if world.resource::<MenuState>().page != MenuPage::Main {
                return Ok(false);
            }
            tracing::info!(
                "Native menu join smoke passed: saved endpoint, automatic UUID-profile handshake, terrain replica and clean leave"
            );
            world.write_message(AppExit::Success);
        }
        210 => {
            if world.resource::<MenuState>().page != MenuPage::Main {
                return Ok(false);
            }
            tracing::info!(
                "Native menu connection-failure smoke passed: actual unreachable server, visible failure and clean return"
            );
            world.write_message(AppExit::Success);
        }
        10_000 => {
            if world.resource::<MenuView>().startup {
                return Ok(false);
            }
            screenshot(world, state, "main-menu");
        }
        10_001
            if state.output.join("main-menu.png").is_file()
                && state.output.join("startup-transition.png").is_file() =>
        {
            tracing::info!(
                "Native startup-only smoke passed with no world, server or pending catalog jobs"
            );
            world.write_message(AppExit::Success);
            return Ok(false);
        }
        180 => {
            if world.resource::<SessionInfo>().saving
                || world.resource::<MenuState>().status != "World saved"
            {
                return Ok(false);
            }
            emit(world, "options", None)?;
        }
        181 => {
            require(
                world.resource::<MenuState>().page == MenuPage::Options,
                "Game Menu Options failed",
            )?;
            emit(world, "back", None)?;
        }
        182 => {
            require(
                world.resource::<MenuState>().page == MenuPage::Pause,
                "Options did not return to Game Menu",
            )?;
            emit(world, "resume", None)?;
        }
        183 => {
            require(
                world.resource::<MenuState>().page == MenuPage::Playing,
                "Resume failed",
            )?;
            key(world, KeyCode::Escape, true);
        }
        184 => {
            key(world, KeyCode::Escape, false);
            emit(world, "leave", None)?;
            state.stage = 104;
        }
        300 => pointer(world, "", "", false)?,
        301 => {
            let expected = if state.create_world_only {
                "spectator"
            } else {
                "creative"
            };
            require(
                world.resource::<WorldMenu>().mode == expected,
                "Native Game Mode button selection failed",
            )?;
            require(
                !world
                    .query::<&ElementId>()
                    .iter(world)
                    .any(|id| id.0 == "world-mode"),
                "Create World still contains the overflowing Game Mode dropdown",
            )?;
            if state.create_world_only {
                validate_create_rule_cards(world)?;
                world.write_message(MenuAction::Change {
                    control: "world-structures-enabled".into(),
                    value: "false".into(),
                });
                for (control, value) in [
                    ("world-rule-keep-inventory", "true"),
                    ("world-rule-daylight-cycle", "false"),
                    ("world-rule-weather-cycle", "false"),
                    ("world-rule-mob-griefing", "false"),
                    ("world-rule-fire-spread", "false"),
                    ("world-rule-pvp", "true"),
                    ("world-structure-ruins", "true"),
                ] {
                    world.write_message(MenuAction::Change {
                        control: control.into(),
                        value: value.into(),
                    });
                }
                emit(world, "world-structure-kingdoms", Some("true"))?;
                emit(world, "world-structure-outposts", Some("false"))?;
                // Use each real repeated input's onchange handler, including its
                // loop context, rather than bypassing the typed card controls.
                for (id, value) in [
                    ("world-sleep-percentage", "35"),
                    ("world-time-speed", "1.5"),
                    ("world-season-days", "24"),
                ] {
                    emit(world, id, Some(value))?;
                }
                screenshot(world, state, "create-world-options-top");
                move_to_create_scroll(world)?;
            }
        }
        302 => {
            if state.create_world_only {
                let flags = world.resource::<WorldMenu>().structures;
                require(
                    flags.kingdoms && !flags.outposts && !flags.villages,
                    "Kingdom toggle is not independent",
                )?;
                emit(world, "world-structure-kingdoms", Some("false"))?;
                emit(world, "world-structure-outposts", Some("true"))?;
                wheel_create_world(world)?;
            }
        }
        303 => {
            if state.create_world_only {
                let flags = world.resource::<WorldMenu>().structures;
                require(
                    !flags.kingdoms && flags.outposts && !flags.villages,
                    "Outpost toggle is not independent",
                )?;
                emit(world, "world-structure-outposts", Some("false"))?;
                validate_create_world_scroll(world)?;
                validate_selected_biome(world, "random")?;
                screenshot(world, state, "create-world-options-scrolled");
                pointer(world, "world-biome-picker-open", "", true)?;
                state.stage = 309;
            } else {
                state.stage = 2;
            }
        }
        310 => pointer(world, "", "", false)?,
        311 => {
            validate_biome_picker(world, "random", false)?;
            screenshot(world, state, "create-world-biome-chooser");
            move_to_create_scroll_id(world, "world-biome-picker-scroll")?;
        }
        312 => wheel_create_scroll_id(world, "world-biome-picker-scroll")?,
        313 => {
            validate_biome_picker(world, "random", true)?;
            screenshot(world, state, "create-world-biome-chooser-scrolled");
            pointer(world, "menu-action", "Cherry Forest", true)?;
        }
        314 => pointer(world, "", "", false)?,
        315 => {
            validate_selected_biome(world, "cherry-forest")?;
            validate_biome_focus_restored(world)?;
            screenshot(world, state, "create-world-biome-selected");
            pointer(world, "world-biome-picker-open", "", true)?;
        }
        316 => pointer(world, "", "", false)?,
        317 => {
            validate_biome_picker(world, "cherry-forest", false)?;
            key(world, KeyCode::Tab, true);
            state.stage = 329;
        }
        318 => key(world, KeyCode::Escape, false),
        319 => {
            validate_selected_biome(world, "cherry-forest")?;
            validate_biome_focus_restored(world)?;
            require(
                world.resource::<MenuState>().dialog == MenuDialog::CreateWorld
                    && world.resource::<WorldMenu>().preferred_spawn_biome
                        == Some(terra_core::BiomeKind::CherryForest),
                "Escape dismissed Create World or lost the selected biome",
            )?;
            pointer(world, "world-biome-picker-open", "", true)?;
        }
        320 => pointer(world, "", "", false)?,
        321 => pointer(world, "world-biome-picker-close", "", true)?,
        322 => pointer(world, "", "", false)?,
        323 => {
            validate_selected_biome(world, "cherry-forest")?;
            validate_biome_focus_restored(world)?;
            validate_create_world_scroll(world)?;
            tracing::info!(
                "Native biome chooser passed: reference thumbnails, scrolling, selected preview, retained selection, Escape and Close"
            );
            state.stage = 2;
        }
        330 => {
            key(world, KeyCode::Tab, false);
            validate_biome_picker(world, "cherry-forest", false)?;
            let done = create_element(world, "world-biome-picker-close")?;
            require(
                world.resource::<bevy::input_focus::InputFocus>().get() != Some(done),
                "Native Tab did not advance from the chooser's Done button",
            )?;
        }
        331 => key(world, KeyCode::ShiftLeft, true),
        332 => key(world, KeyCode::Tab, true),
        333 => {
            key(world, KeyCode::Tab, false);
            key(world, KeyCode::ShiftLeft, false);
            validate_biome_picker(world, "cherry-forest", false)?;
            let done = create_element(world, "world-biome-picker-close")?;
            require(
                world.resource::<bevy::input_focus::InputFocus>().get() == Some(done),
                "Native Shift+Tab did not wrap back to the chooser's Done button",
            )?;
            key(world, KeyCode::Escape, true);
            state.stage = 317;
        }
        400 => {
            let address = std::env::var("TERRA_MENU_JOIN_ADDRESS")
                .map_err(|_| "TERRA_MENU_JOIN_ADDRESS is required")?;
            let id = format!("lan-{address}");
            if !world
                .resource::<ServersView>()
                .cards
                .iter()
                .any(|card| card.id == id)
            {
                return Ok(false);
            }
            require(
                world.resource::<ServerMenu>().list.servers.is_empty(),
                "LAN discovery must not persist invented saved entries",
            )?;
            screenshot(world, state, "lan-discovered");
            emit(world, &id, None)?;
        }
        401 => {
            require(
                world.resource::<ServerMenu>().lan_selected.is_some(),
                "LAN selection failed",
            )?;
            emit(world, "server-join", None)?;
            state.stage = 200;
        }
        500 => {
            require(
                world.resource::<OptionsDraft>().tab == OptionsTab::Audio,
                "Audio tab did not open",
            )?;
            let ready = world
                .query::<(Entity, &ElementId)>()
                .iter(world)
                .any(|(entity, id)| {
                    id.0 == "row-slider" && scope_distance(world, entity, "Master Volume").is_some()
                });
            if !ready {
                if state.advanced.elapsed() > Duration::from_secs(6) {
                    return Err("Audio controls did not finish asynchronous materialization".into());
                }
                return Ok(false);
            }
            screenshot(world, state, "audio");
            for (id, value) in [
                ("audio-master", "80"),
                ("audio-ambience", "70"),
                ("audio-animals", "55"),
                ("audio-footsteps", "65"),
                ("audio-water", "60"),
                ("audio-river", "40"),
                ("audio-waterfall", "90"),
            ] {
                emit(world, id, Some(value))?;
            }
        }
        501 => {
            require(
                world.resource::<AudioConfig>().master_volume == 1.0,
                "Audio draft was applied before Apply",
            )?;
            emit(world, "apply", None)?;
        }
        502 => {
            let expected = AudioConfig {
                master_volume: 0.8,
                ambience_volume: 0.7,
                animals_volume: 0.55,
                footsteps_volume: 0.65,
                water_volume: 0.6,
                river_volume: 0.4,
                waterfall_volume: 0.9,
                muted: false,
            };
            let path = world.resource::<MenuPaths>().configs.join("audio.toml");
            require(
                *world.resource::<AudioConfig>() == expected
                    && AudioConfig::load(path).map_err(|e| e.to_string())? == expected,
                "Audio settings did not apply and reload from TOML",
            )?;
            screenshot(world, state, "audio-applied");
            emit(world, "audio-muted", Some("true"))?;
        }
        503 => emit(world, "apply", None)?,
        504 => {
            require(
                world.resource::<AudioConfig>().muted,
                "Mute was not applied",
            )?;
            emit(world, "reset", None)?;
        }
        505 => {
            require(
                world.resource::<OptionsDraft>().audio == AudioConfig::default()
                    && world.resource::<AudioConfig>().muted,
                "Audio Reset must change only the draft until Apply",
            )?;
            emit(world, "apply", None)?;
        }
        506 => {
            let path = world.resource::<MenuPaths>().configs.join("audio.toml");
            require(
                AudioConfig::load(path).map_err(|e| e.to_string())? == AudioConfig::default(),
                "Audio defaults were not persisted",
            )?;
            emit(world, "multiplayer-options", None)?;
            state.stage = 606;
        }
        607 => {
            require(
                world.resource::<OptionsDraft>().tab == OptionsTab::Multiplayer,
                "Multiplayer options did not open",
            )?;
            screenshot(world, state, "multiplayer-options");
            require(
                world
                    .resource::<OptionsDraft>()
                    .multiplayer
                    .cursor_color
                    .is_none(),
                "Default cursor color must be automatic",
            )?;
            require(
                world
                    .resource::<OptionsView>()
                    .rows
                    .iter()
                    .any(|row| row.id == "multiplayer-color" && row.enabled),
                "Colorpicker is disabled while automatic color is active",
            )?;
        }
        608 => {
            emit(world, "multiplayer-color", Some("#e15c45"))?;
            emit(world, "multiplayer-names", Some("false"))?;
            emit(world, "apply", None)?;
        }
        609 => {
            let path = world
                .resource::<MenuPaths>()
                .configs
                .join("multiplayer.toml");
            let config = terra_core::MultiplayerConfig::load(path).map_err(|e| e.to_string())?;
            require(
                !config.player_names
                    && config.chest_cursors
                    && config.cursor_color == Some([225, 92, 69]),
                "Multiplayer options did not persist",
            )?;
            screenshot(world, state, "multiplayer-options-applied");
            let picker = world
                .query::<(Entity, &ElementId)>()
                .iter(world)
                .find(|(_, id)| id.0 == "row-color")
                .map(|(entity, _)| entity)
                .ok_or("Cursor colorpicker missing")?;
            require(
                tilt_ui::set_color_picker_open(world, picker, true),
                "Cursor palette did not open",
            )?;
        }
        610 => {
            screenshot(world, state, "multiplayer-color-palette");
            let picker = world
                .query::<(Entity, &ElementId)>()
                .iter(world)
                .find(|(_, id)| id.0 == "row-color")
                .map(|(entity, _)| entity)
                .ok_or("Cursor colorpicker missing")?;
            tilt_ui::set_color_picker_open(world, picker, false);
            emit(world, "reset", None)?;
            emit(world, "apply", None)?;
            emit(world, "audio", None)?;
        }
        611 => {
            emit(world, "gameplay-options", None)?;
            state.stage = 699;
        }
        690 => {
            require(
                world.resource::<MenuState>().page == MenuPage::Options,
                "Gameplay fixture could not open Options",
            )?;
            emit(world, "gameplay-options", None)?;
            state.stage = 699;
        }
        700 => {
            require(
                world.resource::<OptionsDraft>().tab == OptionsTab::Gameplay
                    && world
                        .resource::<OptionsView>()
                        .rows
                        .iter()
                        .any(|row| row.id == "hud-edit")
                    && world
                        .resource::<OptionsView>()
                        .rows
                        .iter()
                        .any(|row| row.id == "minimap-settings")
                    && world
                        .resource::<OptionsView>()
                        .rows
                        .iter()
                        .any(|row| row.id == "view-bobbing" && row.checked),
                "Gameplay tab must contain HUD, MiniMap and walking-camera settings",
            )?;
            screenshot(world, state, "gameplay-options");
            emit(world, "view-bobbing", Some("false"))?;
            emit(world, "minimap-settings", None)?;
        }
        701 => {
            require(
                world.resource::<MenuState>().dialog == MenuDialog::Minimap
                    && world.resource::<DialogView>().rows.len() == 13,
                "MiniMap popup settings did not materialize",
            )?;
            validate_minimap_popup(world)?;
            screenshot(world, state, "minimap-settings");
            for (id, value) in [
                ("minimap-size", "300"),
                ("minimap-fps", "40"),
                ("minimap-icon-scale", "2.0"),
                ("minimap-mode", "cave"),
                ("minimap-rotate", "true"),
                ("minimap-mobs", "false"),
                ("minimap-coordinates", "false"),
            ] {
                emit(world, id, Some(value))?;
            }
        }
        702 => {
            let draft = &world.resource::<OptionsDraft>().gameplay.minimap;
            require(
                draft.size == 300
                    && draft.fps == 40
                    && draft.icon_scale == 2.0
                    && draft.mode == MapMode::Cave
                    && draft.rotate
                    && !draft.mobs
                    && !draft.coordinates
                    && *world.resource::<GameplayConfig>() == GameplayConfig::default(),
                "MiniMap widgets must update the draft without applying it",
            )?;
            screenshot(world, state, "minimap-settings-edited");
            emit(world, "dialog-cancel", None)?;
        }
        703 => {
            require(
                world.resource::<MenuState>().dialog == MenuDialog::None
                    && *world.resource::<GameplayConfig>() == GameplayConfig::default(),
                "MiniMap Done must return to Options without applying",
            )?;
            emit(world, "apply", None)?;
        }
        704 => {
            let path = world.resource::<MenuPaths>().configs.join("gameplay.toml");
            let saved = GameplayConfig::load(path).map_err(|error| error.to_string())?;
            require(
                saved == *world.resource::<GameplayConfig>()
                    && saved.minimap.size == 300
                    && saved.minimap.fps == 40
                    && saved.minimap.icon_scale == 2.0
                    && saved.minimap.mode == MapMode::Cave
                    && !saved.view_bobbing,
                "MiniMap and walking-camera settings did not apply and reload from gameplay.toml",
            )?;
            emit(world, "minimap-settings", None)?;
        }
        705 => {
            validate_minimap_popup(world)?;
            emit(world, "minimap-reset", None)?;
        }
        706 => {
            let expected = GameplayConfig {
                view_bobbing: false,
                ..default()
            };
            let path = world.resource::<MenuPaths>().configs.join("gameplay.toml");
            let saved = GameplayConfig::load(path).map_err(|error| error.to_string())?;
            require(
                world.resource::<OptionsDraft>().gameplay == expected
                    && world.resource::<GameplayConfig>().minimap.mode == MapMode::Cave
                    && !world.resource::<GameplayConfig>().view_bobbing
                    && saved == *world.resource::<GameplayConfig>(),
                "MiniMap Reset must stage only MiniMap defaults and preserve the applied walking-camera preference and file",
            )?;
            screenshot(world, state, "minimap-settings-reset");
            emit(world, "dialog-cancel", None)?;
        }
        707 => {
            require(
                world.resource::<MenuState>().dialog == MenuDialog::None,
                "MiniMap Reset Done did not return to Options",
            )?;
            emit(world, "apply", None)?;
        }
        708 => {
            let expected = GameplayConfig {
                view_bobbing: false,
                ..default()
            };
            let path = world.resource::<MenuPaths>().configs.join("gameplay.toml");
            let saved = GameplayConfig::load(path).map_err(|error| error.to_string())?;
            require(
                *world.resource::<GameplayConfig>() == expected && saved == expected,
                "MiniMap defaults must apply without resetting the walking-camera preference",
            )?;
            emit(world, "reset", None)?;
            emit(world, "apply", None)?;
            emit(world, "audio", None)?;
        }
        709 => {
            let path = world.resource::<MenuPaths>().configs.join("gameplay.toml");
            require(
                *world.resource::<GameplayConfig>() == GameplayConfig::default()
                    && GameplayConfig::load(path).map_err(|error| error.to_string())?
                        == GameplayConfig::default(),
                "Full Gameplay Reset must restore and persist all defaults",
            )?;
            if state.gameplay_only {
                emit(world, "back", None)?;
                return Ok(true);
            }
            emit(world, "audio-river", Some("45"))?;
            emit(world, "graphics", None)?;
            state.stage = 15;
        }
        710 if state.gameplay_only => {
            require(
                world.resource::<MenuState>().page == MenuPage::Main,
                "Gameplay options did not return to Main Menu",
            )?;
            tracing::info!(
                "Native Gameplay menu smoke passed: real TiltUI tab, MiniMap popup, widgets, staged Apply, TOML reload, Reset, bounded footer and Back"
            );
            emit(world, "quit", None)?;
        }
        800 if state.mob_health_only => {
            emit(world, "gameplay-options", None)?;
        }
        801 if state.mob_health_only => {
            require(
                world.resource::<OptionsView>().rows.iter().any(|row| {
                    row.id == "mob-health-settings"
                        && row.label == "Edit Bars"
                        && row.kind == "command"
                }),
                "Gameplay Edit Bars button missing",
            )?;
            emit(world, "mob-health-settings", None)?;
        }
        802 if state.mob_health_only => {
            require(
                world.resource::<MenuState>().dialog == MenuDialog::MobHealth
                    && world.resource::<DialogView>().mob_health,
                "Edit Bars did not open its actual popup",
            )?;
            let controls: Vec<_> = world
                .query::<(Entity, &tilt_ui::ColorPickerState, &ComputedNode)>()
                .iter(world)
                .filter(|(entity, _, _)| has_ancestor_class(world, *entity, "mob-health-options"))
                .map(|(entity, _, node)| (entity, node.size()))
                .collect();
            require(
                controls.len() == 3
                    && controls
                        .iter()
                        .all(|(_, size)| size.x >= 100.0 && size.y >= 30.0),
                "Friendly/Hostile/Font color choosers lack actual widget layout",
            )?;
            screenshot(world, state, "mob-health-edit-bars");
            emit(world, "mob-health-bars", Some("in_view"))?;
            emit(world, "mob-health-friendly-color", Some("#3aaed8"))?;
            emit(world, "mob-health-hostile-color", Some("#eb8040"))?;
            emit(world, "mob-health-font-color", Some("#e3efd5"))?;
        }
        803 if state.mob_health_only => {
            let draft = &world.resource::<OptionsDraft>().gameplay;
            require(
                draft.mob_health_bars == terra_core::MobHealthBars::InView
                    && draft.mob_health_friendly_color == [58, 174, 216]
                    && draft.mob_health_hostile_color == [235, 128, 64]
                    && draft.mob_health_font_color == [227, 239, 213],
                "Actual popup handlers did not stage selected mode/colors",
            )?;
            require(
                *world.resource::<GameplayConfig>() == GameplayConfig::default(),
                "Popup changes applied before Apply",
            )?;
            let picker = world
                .query::<(Entity, &tilt_ui::ColorPickerState)>()
                .iter(world)
                .find(|(entity, _)| has_ancestor_class(world, *entity, "mob-health-options"))
                .map(|(entity, _)| entity)
                .ok_or("Color picker missing")?;
            require(
                tilt_ui::set_color_picker_open(world, picker, true),
                "Actual color picker did not open",
            )?;
        }
        804 if state.mob_health_only => {
            screenshot(world, state, "mob-health-color-picker");
            let pickers: Vec<_> = world
                .query::<(Entity, &tilt_ui::ColorPickerState)>()
                .iter(world)
                .filter(|(entity, _)| has_ancestor_class(world, *entity, "mob-health-options"))
                .map(|(entity, _)| entity)
                .collect();
            for picker in pickers {
                tilt_ui::set_color_picker_open(world, picker, false);
            }
            emit(world, "dialog-cancel", None)?;
        }
        805 if state.mob_health_only => {
            emit(world, "apply", None)?;
        }
        806 if state.mob_health_only => {
            let saved =
                GameplayConfig::load(world.resource::<MenuPaths>().configs.join("gameplay.toml"))
                    .map_err(|error| error.to_string())?;
            require(
                saved == *world.resource::<GameplayConfig>()
                    && saved.mob_health_friendly_color == [58, 174, 216]
                    && saved.mob_health_hostile_color == [235, 128, 64]
                    && saved.mob_health_font_color == [227, 239, 213],
                "Mob-health colors did not persist through normal Apply",
            )?;
            emit(world, "mob-health-settings", None)?;
        }
        807 if state.mob_health_only => {
            emit(world, "mob-health-reset", None)?;
        }
        808 if state.mob_health_only => {
            require(
                world.resource::<OptionsDraft>().gameplay == GameplayConfig::default()
                    && world.resource::<GameplayConfig>().mob_health_friendly_color
                        == [58, 174, 216],
                "Reset did not remain staged inside the popup",
            )?;
            emit(world, "dialog-cancel", None)?;
        }
        809 if state.mob_health_only => {
            if world.resource::<MenuState>().dialog != MenuDialog::None
                || world.resource::<DialogView>().visible
            {
                return Ok(false);
            }
            emit(world, "back", None)?;
        }
        810 if state.mob_health_only => {
            require(
                world.resource::<MenuState>().dialog == MenuDialog::Discard,
                "Unsaved popup reset did not use the normal Discard prompt",
            )?;
            emit(world, "discard-confirm", None)?;
        }
        811 if state.mob_health_only => {
            require(
                world.resource::<MenuState>().page == MenuPage::Main
                    && world.resource::<GameplayConfig>().mob_health_friendly_color
                        == [58, 174, 216],
                "Discard changed applied bar preferences",
            )?;
            tracing::info!(
                "Native Mob Health editor passed: bound Edit Bars modal, actual choice/color-picker widgets, staged colors, persistent Apply and Reset/Discard"
            );
            emit(world, "quit", None)?;
        }
        900 if state.graphics_controls_only => emit(world, "graphics", None)?,
        901 if state.graphics_controls_only => {
            for (title, labels) in [
                ("Cave Lighting", &["Low", "Medium", "High", "Ultra"][..]),
                (
                    "Tree Fall Animation",
                    &["Current (Default)", "Hell Fall", "Magic"][..],
                ),
            ] {
                let (entity, parts) = world
                    .query::<(Entity, &ElementId, &tilt_ui::ChoiceBoxParts, &ComputedNode)>()
                    .iter(world)
                    .filter(|(_, id, _, node)| {
                        id.0 == "row-select" && node.size().min_element() > 0.0
                    })
                    .filter_map(|(entity, _, parts, _)| {
                        scope_distance(world, entity, title)
                            .map(|distance| (distance, entity, *parts))
                    })
                    .min_by_key(|(distance, _, _)| *distance)
                    .map(|(_, entity, parts)| (entity, parts))
                    .ok_or_else(|| {
                        format!("Graphics dropdown lacks a materialized widget: {title}")
                    })?;
                require(
                    has_ancestor_class(world, entity, "settings-row"),
                    "Graphics dropdown detached from its row",
                )?;
                require(
                    world
                        .get::<bevy::ui::widget::Text>(parts.value)
                        .is_some_and(|value| labels.contains(&value.0.as_str())),
                    "Graphics selected option is blank",
                )?;
                for label in labels {
                    require(
                        subtree_has_text(world, parts.popup, label, 0),
                        "Graphics dropdown option did not materialize",
                    )?;
                }
            }
            screenshot(world, state, "graphics-controls");
            emit(world, "graphics-caves", Some("low"))?;
            emit(world, "graphics-tree-fall", Some("hell_fall"))?;
            scroll_setting_into_view(world, "Cave Lighting")?;
            state.stage = 909;
        }
        902 if state.graphics_controls_only => {
            let draft = &world.resource::<OptionsDraft>().graphics;
            require(
                draft.cave_quality == terra_core::CaveQuality::Low
                    && draft.tree_fall_animation == terra_core::TreeFallAnimation::HellFall,
                "Graphics widget changes did not reach the draft",
            )?;
            emit(world, "apply", None)?;
        }
        903 if state.graphics_controls_only => {
            let saved =
                GraphicsConfig::load(world.resource::<MenuPaths>().configs.join("graphics.toml"))
                    .map_err(|error| error.to_string())?;
            require(
                saved.cave_quality == terra_core::CaveQuality::Low
                    && saved.tree_fall_animation == terra_core::TreeFallAnimation::HellFall,
                "Low/Hell Fall did not persist",
            )?;
            emit(world, "graphics-caves", Some("ultra"))?;
            emit(world, "graphics-tree-fall", Some("magic"))?;
        }
        904 if state.graphics_controls_only => emit(world, "apply", None)?,
        905 if state.graphics_controls_only => {
            let saved =
                GraphicsConfig::load(world.resource::<MenuPaths>().configs.join("graphics.toml"))
                    .map_err(|error| error.to_string())?;
            require(
                saved.cave_quality == terra_core::CaveQuality::Ultra
                    && saved.tree_fall_animation == terra_core::TreeFallAnimation::Magic,
                "Ultra/Magic did not persist",
            )?;
            emit(world, "graphics-caves", Some("medium"))?;
            emit(world, "graphics-tree-fall", Some("current"))?;
        }
        906 if state.graphics_controls_only => emit(world, "apply", None)?,
        907 if state.graphics_controls_only => {
            let saved =
                GraphicsConfig::load(world.resource::<MenuPaths>().configs.join("graphics.toml"))
                    .map_err(|error| error.to_string())?;
            require(
                saved.cave_quality == terra_core::CaveQuality::Medium
                    && saved.tree_fall_animation == terra_core::TreeFallAnimation::Current,
                "Medium/Current did not persist",
            )?;
            emit(world, "back", None)?;
        }
        908 if state.graphics_controls_only => {
            require(
                world.resource::<MenuState>().page == MenuPage::Main,
                "Graphics controls did not return to the main menu",
            )?;
            tracing::info!(
                "Native Graphics controls passed: materialized Cave Lighting and Tree Fall Animation widgets, selected labels, options, actual UI handlers and persistent Apply"
            );
            emit(world, "quit", None)?;
        }
        910 if state.graphics_controls_only => {
            require_setting_on_screen(world, "Cave Lighting")?;
            screenshot(world, state, "graphics-cave-lighting");
            scroll_setting_into_view(world, "Tree Fall Animation")?;
        }
        911 if state.graphics_controls_only => {
            require_setting_on_screen(world, "Tree Fall Animation")?;
            screenshot(world, state, "graphics-tree-fall-animation");
            state.stage = 901;
        }
        1000 => pointer(world, "", "", false)?,
        1001 => {
            require(
                world.resource::<MenuState>().page == MenuPage::Worlds,
                "Pointer click did not open Worlds",
            )?;
            pointer(world, "menu-action", "Back", true)?;
        }
        1002 => pointer(world, "", "", false)?,
        1003 => {
            require(
                world.resource::<MenuState>().page == MenuPage::Main,
                "Pointer Back failed",
            )?;
            pointer(world, "menu-action", "Settings", true)?;
        }
        1004 => pointer(world, "", "", false)?,
        1005 => {
            require(
                world.resource::<MenuState>().page == MenuPage::Options,
                "Pointer Options failed",
            )?;
            screenshot(world, state, "pointer-options");
            pointer(world, "controls", "", true)?;
        }
        1006 => pointer(world, "", "", false)?,
        1007 => {
            require(
                world.resource::<OptionsDraft>().tab == OptionsTab::Controls,
                "Pointer Controls tab failed",
            )?;
            screenshot(world, state, "pointer-controls");
            pointer(world, "audio", "", true)?;
        }
        1008 => pointer(world, "", "", false)?,
        1009 => {
            require(
                world.resource::<OptionsDraft>().tab == OptionsTab::Audio,
                "Pointer Audio tab failed",
            )?;
            screenshot(world, state, "pointer-audio");
            pointer(world, "menu-action", "Back", true)?;
        }
        1010 => pointer(world, "", "", false)?,
        1011 => {
            require(
                world.resource::<MenuState>().page == MenuPage::Main,
                "Pointer Options Back failed",
            )?;
            pointer(world, "menu-action", "Quit", true)?;
        }
        1012 => {
            pointer(world, "", "", false)?;
            tracing::info!(
                "Native pointer smoke passed: pointer input, hit testing, click/release, Worlds, Back, Options, Controls, Audio and Quit"
            );
        }
        _ => {}
    }
    Ok(true)
}

/// Move the isolated fixture pointer without moving the desktop cursor.
pub(crate) fn move_test_pointer(world: &mut World, delta: Vec2) -> Result<(), String> {
    let mut pointer = world.resource_mut::<MenuPointer>();
    let location = pointer.0.as_mut().ok_or("Missing test pointer")?;
    location.position += delta;
    maintain_pointer(world);
    Ok(())
}

#[cfg(test)]
mod pointer_tests {
    use super::*;
    use bevy::{
        camera::RenderTarget,
        picking::pointer::{
            Location, PointerAction, PointerButton, PointerId, PointerInput, PointerLocation,
        },
        window::WindowRef,
    };

    #[test]
    fn release_uses_fixture_position_after_native_mouse_location_changes() {
        let mut world = World::new();
        world.init_resource::<Messages<PointerInput>>();
        world.init_resource::<bevy::picking::pointer::PointerMap>();
        let window = world
            .spawn((
                Window {
                    focused: false,
                    ..default()
                },
                PrimaryWindow,
            ))
            .id();
        let target = RenderTarget::Window(WindowRef::Entity(window))
            .normalize(Some(window))
            .unwrap();
        let expected = Location {
            target: target.clone(),
            position: Vec2::new(250.0, 140.0),
        };
        world.insert_resource(MenuPointer(Some(expected.clone())));
        world.spawn((
            PointerId::Mouse,
            PointerLocation::new(Location {
                target,
                position: Vec2::ONE,
            }),
        ));
        world.clear_trackers();

        pointer(&mut world, "", "", false).unwrap();

        let events = world
            .resource_mut::<Messages<PointerInput>>()
            .drain()
            .collect::<Vec<_>>();
        assert!(events.iter().any(|event| {
            event.location.position == expected.position
                && matches!(event.action, PointerAction::Release(PointerButton::Primary))
        }));
        let entity = world.entity(window);
        let window = entity.get_ref::<Window>().unwrap();
        assert!(window.focused);
        assert_eq!(window.cursor_position(), Some(expected.position));
        assert!(
            !window.is_changed(),
            "Fixture cursor must not request an OS window mutation"
        );
    }
}
