//! Real integrated authority, saved initial values, food, exhaustion, fall death and HUD respawn.
use bevy::{app::AppExit, prelude::*};
use std::{
    path::PathBuf,
    time::{Duration, Instant},
};
use terra_core::{
    Config, GameAction, GameConfig, GameMode, ItemKind, PlayerProfile, PlayerStats,
    ResourcePropKind, SurvivalVitals,
};
use terra_network::client::ClientSession;
use terra_player::{CaptureRequest, ControllerMode, PlayerBody};

pub(crate) fn prepare(
    mut config: GameConfig,
) -> Result<(terra_server::IntegratedServer, Vec<u8>, PlayerProfile), String> {
    config.server.game_mode = GameMode::Survival;
    config.world.biomes = false;
    config.world.trees = false;
    config.world.caves = false;
    config.world.water = false;
    config.world.hill_amplitude = 0.0;
    config.world.mountain_amplitude = 0.0;
    config.world.time.paused = true;
    config.world.time.start_hour = 12.0;
    let root = std::env::var_os("TERRA_SURVIVAL_SMOKE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            std::env::temp_dir().join(format!("terra-survival-{}", std::process::id()))
        });
    let path = root.join("world");
    if path.exists() {
        return Err("Survival fixture requires a fresh directory".into());
    }
    let profile_path = PlayerProfile::selected_path(&terra_core::menu_config_directory())
        .map_err(|error| error.to_string())?;
    let profile = PlayerProfile::load_or_create(profile_path).map_err(|error| error.to_string())?;
    let mut permissions = config.server.permissions_for(profile.uuid);
    permissions.game_mode = true;
    config
        .server
        .player_permissions
        .insert(profile.uuid, permissions);
    let mut journal = terra_world::edits::EditJournal::default();
    journal.update_player_stats(
        profile.uuid,
        PlayerStats {
            blocks_broken: 7,
            survival: SurvivalVitals {
                health: 28.0,
                stamina: 61.0,
                hunger: 0.0,
                ..default()
            },
            ..default()
        },
    );
    journal
        .gameplay_mut()
        .inventories
        .entry(profile.uuid)
        .or_default()
        .insert(ItemKind::Prop(ResourcePropKind::Mushroom), 5);
    terra_storage::save_edits(&path, config.world.id, &journal)?;
    let (host, token) = terra_server::IntegratedServer::start_with_save(config, Some(path))?;
    Ok((host, token, profile))
}

#[derive(Resource)]
struct Fixture {
    started: Instant,
    entered: Instant,
    stage: u8,
    height: f32,
}
impl Fixture {
    fn advance(&mut self) {
        self.stage += 1;
        self.entered = Instant::now();
        info!(stage = self.stage, "Survival smoke advanced");
    }
}
pub(crate) fn install(app: &mut App) {
    if !std::env::args().any(|arg| arg == "--smoke-test-survival") {
        return;
    }
    let now = Instant::now();
    {
        let mut menu = app.world_mut().resource_mut::<terra_ui::menu::MenuState>();
        menu.page = terra_ui::menu::MenuPage::Playing;
        menu.session_active = true;
    }
    app.insert_resource(ControllerMode::Walk)
        .insert_resource(Fixture {
            started: now,
            entered: now,
            stage: 0,
            height: 0.0,
        })
        .insert_resource(bevy::winit::WinitSettings::continuous())
        .add_systems(PreUpdate, run.after(bevy::input::InputSystems));
}
fn capture(world: &mut World, name: &str) -> Result<(), String> {
    let root = std::env::var_os("TERRA_CAPTURE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::temp_dir().join("terra-survival-captures"));
    std::fs::create_dir_all(&root).map_err(|e| e.to_string())?;
    world
        .spawn(bevy::render::view::screenshot::Screenshot::primary_window())
        .observe(bevy::render::view::screenshot::save_to_disk(
            root.join(format!("{name}.png")),
        ));
    Ok(())
}
fn click_respawn(world: &mut World) -> Result<(), String> {
    click(world, "survival-respawn")
}

fn require_damage_edges(world: &mut World, root_id: &str) -> Result<(), String> {
    let (root, size, center) = world
        .query::<(
            Entity,
            &tilt_ui::ElementId,
            &ComputedNode,
            &bevy::ui::UiGlobalTransform,
        )>()
        .iter(world)
        .find_map(|(entity, id, node, transform)| {
            (id.0 == root_id).then_some((entity, node.size(), transform.translation))
        })
        .ok_or_else(|| format!("Missing damage edge surface: {root_id}"))?;
    if world
        .get::<BackgroundColor>(root)
        .is_some_and(|color| color.0.alpha() > 0.001)
    {
        return Err("Damage feedback tinted the entire screen instead of its edges".into());
    }
    let mut count = 0;
    for (entity, classes, node, transform) in world
        .query::<(
            Entity,
            &tilt_ui::ElementClasses,
            &ComputedNode,
            &bevy::ui::UiGlobalTransform,
        )>()
        .iter(world)
    {
        if !classes.classes.iter().any(|class| class == "damage-edge") {
            continue;
        }
        let mut parent = world.get::<ChildOf>(entity).map(ChildOf::parent);
        while let Some(ancestor) = parent {
            if ancestor == root {
                let bounds = Rect::from_center_size(transform.translation, node.size());
                if bounds.contains(center)
                    || node.size().min_element() <= 0.0
                    || node.size().x > size.x * 0.11 && node.size().y > size.y * 0.11
                {
                    return Err("Damage glow does not leave the view center unobstructed".into());
                }
                count += 1;
                break;
            }
            parent = world.get::<ChildOf>(ancestor).map(ChildOf::parent);
        }
    }
    if count != 4 {
        return Err(format!("Expected four damage edges, found {count}"));
    }
    Ok(())
}
fn require_reference_hud(world: &mut World, settled: bool) -> Result<(), String> {
    let bounds = |world: &mut World, target: &str| {
        world
            .query::<(
                &tilt_ui::ElementId,
                &ComputedNode,
                &bevy::ui::UiGlobalTransform,
            )>()
            .iter(world)
            .find_map(|(id, node, transform)| {
                (id.0 == target)
                    .then_some(Rect::from_center_size(transform.translation, node.size()))
            })
            .ok_or_else(|| format!("Missing survival element: {target}"))
    };
    let hud = bounds(world, "survival-hud")?;
    let chrome = bounds(world, "survival-chrome")?;
    let bar = bounds(world, "survival-health")?;
    let stamina = bounds(world, "survival-stamina")?;
    let food = bounds(world, "survival-food-status")?;
    let hunger = bounds(world, "survival-hunger")?;
    let label = bounds(world, "survival-health-label")?;
    if (bar.center() - label.center()).length() > 1.0 || (bar.size() - label.size()).length() > 1.0
    {
        return Err("Survival health label is not centered across the full health bar".into());
    }
    if (chrome.width() / chrome.height() - 2106.0 / 747.0).abs() > 0.03
        || bar.min.x < chrome.min.x + chrome.width() * 0.25
        || stamina.min.x <= bar.min.x
        || stamina.max.x >= bar.max.x
        || stamina.min.y <= bar.max.y
        || food.min.y <= stamina.max.y
        || !food.contains(hunger.center())
        || !hud.contains(food.max)
    {
        return Err("Survival HUD did not retain its integrated medallion and meter layout".into());
    }
    let transparent = world
        .query::<(&tilt_ui::ElementId, Option<&BackgroundColor>)>()
        .iter(world)
        .find_map(|(id, color)| {
            (id.0 == "survival-hud").then_some(color.is_none_or(|color| color.0.alpha() < 0.001))
        });
    if transparent != Some(true) {
        return Err("Survival HUD has an opaque outer panel".into());
    }
    let view = world.resource::<terra_ui::SurvivalView>();
    let values = [
        ("survival-health", "health", view.health),
        ("survival-stamina", "stamina", view.stamina),
        ("survival-hunger", "hunger", view.hunger),
    ];
    for (id, field, source_value) in values {
        // The binding snapshot can precede SurvivalView's Update system by one frame.
        let bound_value = world
            .resource::<tilt_ui::UiSharedValues>()
            .get("survivalView")
            .and_then(|view| view.get(field))
            .and_then(|value| value.as_f64())
            .ok_or_else(|| format!("Missing shared survival binding {field}"))?
            as f32;
        require_meter(world, id, bound_value, source_value, settled)?;
    }
    Ok(())
}

fn require_meter(
    world: &mut World,
    id: &str,
    expected: f32,
    source_value: f32,
    settled: bool,
) -> Result<(), String> {
    let (range, parts) = world
        .query::<(
            &tilt_ui::ElementId,
            &tilt_ui::NumericRange,
            &tilt_ui::NumericParts,
        )>()
        .iter(world)
        .find_map(|(element, range, parts)| (element.0 == id).then_some((*range, *parts)))
        .ok_or_else(|| format!("Missing native HUD meter {id}"))?;
    let track = world
        .get::<ComputedNode>(parts.track)
        .ok_or("Missing meter track")?;
    let fill = world
        .get::<ComputedNode>(parts.fill)
        .ok_or("Missing meter fill")?;
    let target = world
        .get::<tilt_ui::WidgetLayoutOverride>(parts.fill)
        .and_then(|layout| layout.width)
        .ok_or("Missing meter target width")?;
    let shown = world
        .get::<Node>(parts.fill)
        .ok_or("Missing meter layout")?
        .width;
    let (Val::Percent(target_percent), Val::Percent(shown_percent)) = (target, shown) else {
        return Err(format!(
            "Native HUD meter {id} has non-percent widths: target={target:?}, shown={shown:?}"
        ));
    };
    // TiltUI animates the visible Node width over 0.10 s; the target updates immediately.
    let target_pixels = track.size().x * range.fraction();
    let shown_pixels = track.size().x * shown_percent / 100.0;
    let tolerance = 2.1 / track.inverse_scale_factor().max(f32::EPSILON);
    if (range.value - range.normalize(expected)).abs() > 0.001
        || (target_percent - range.fraction() * 100.0).abs() > 0.001
        || !shown_percent.is_finite()
        || !(0.0..=100.0).contains(&shown_percent)
        || (fill.size().x - shown_pixels).abs() > tolerance
        || settled && (fill.size().x - target_pixels).abs() > tolerance
        || track.size().min_element() <= 0.0
    {
        return Err(format!(
            "Native HUD meter {id}: source={source_value:.5}, bound={expected:.5}, \
             range={:.5}, target={target_percent:.5}%, shown={shown_percent:.5}%, \
             track={:?}, fill={:?}, shown_px={shown_pixels:.3}, \
             target_px={target_pixels:.3}, tolerance_px={tolerance:.3}, settled={settled}",
            range.value,
            track.size(),
            fill.size(),
        ));
    }
    Ok(())
}
fn click(world: &mut World, button: &str) -> Result<(), String> {
    click_label(world, button, None)
}
fn contains_text(world: &World, entity: Entity, expected: &str) -> bool {
    world
        .get::<Text>(entity)
        .is_some_and(|text| text.0 == expected)
        || world.get::<Children>(entity).is_some_and(|children| {
            children
                .iter()
                .any(|child| contains_text(world, child, expected))
        })
}
fn click_label(world: &mut World, button: &str, label: Option<&str>) -> Result<(), String> {
    let entity = world
        .query::<(Entity, &tilt_ui::ElementId)>()
        .iter(world)
        .find_map(|(entity, id)| {
            (id.0 == button && label.is_none_or(|label| contains_text(world, entity, label)))
                .then_some(entity)
        })
        .ok_or_else(|| format!("Button {button} missing"))?;
    let handler = world
        .get::<tilt_ui::StaticAttributes>(entity)
        .and_then(|attributes| attributes.attributes.iter().find(|a| a.name == "onclick"))
        .map(|a| a.value.clone())
        .ok_or("Respawn handler missing")?;
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
fn run(world: &mut World) {
    world.resource_scope(|world, mut fixture: Mut<Fixture>| {
        if fixture.stage == 255 {
            return;
        }
        if let Err(error) = step(world, &mut fixture) {
            error!(%error, stage=fixture.stage, "Native survival smoke failed");
            fixture.stage = 255;
            world.write_message(AppExit::error());
        }
    });
}
fn step(world: &mut World, fixture: &mut Fixture) -> Result<(), String> {
    if fixture.started.elapsed() > Duration::from_secs(150) {
        return Err("Survival fixture timed out".into());
    }
    let session = world.resource::<ClientSession>();
    if let Some(error) = &session.error {
        return Err(error.clone());
    }
    let ready = session.ready && session.stats.blocks_broken == 7;
    let count = session
        .inventory
        .count(ItemKind::Prop(ResourcePropKind::Mushroom));
    let vitals = session.stats.survival;
    let dead = session.dead();
    let respawning = session.teleporting;
    let state = session.confirmed_state();
    let elapsed = fixture.entered.elapsed().as_secs_f32();
    let mut forward = false;
    let mut sprint = false;
    let mut up = false;
    match fixture.stage {
        0 if ready
            && world.resource::<terra_render::RenderStatus>().ready
            && world.resource::<terra_ui::SurvivalView>().visible
            && world
                .query::<&tilt_ui::ElementId>()
                .iter(world)
                .any(|id| id.0 == "survival-food") =>
        {
            if vitals.hunger > 0.1 || count != 5 {
                return Err("Initial saved survival fixture did not reach client".into());
            }
            world.write_message(CaptureRequest(true));
            fixture.advance();
        }
        1 if elapsed > 0.6 => {
            let chrome = world.resource::<terra_ui::VitalRenderStatus>();
            if !chrome.bound || chrome.heartbeat_period != Some(0.62) {
                return Err(format!(
                    "Native chamfered HUD/heartbeat not bound: {chrome:?}"
                ));
            }
            let critical = world.resource::<terra_ui::SurvivalView>().critical;
            let rendered = world
                .query::<(&tilt_ui::ElementId, &ComputedNode)>()
                .iter(world)
                .any(|(id, node)| id.0 == "survival-critical" && node.size().min_element() > 300.0);
            if !critical || !rendered {
                return Err("Critical-health TiltUI disturbance surface was not rendered".into());
            }
            require_damage_edges(world, "survival-critical-glow")?;
            require_reference_hud(world, false)?;
            capture(world, "critical-health-disturbance")?;
            fixture.advance();
        }
        2..=6 => {
            let expected = 6 - u16::from(fixture.stage);
            if elapsed > 0.6 && count == expected {
                let percentage = vitals.hunger.ceil().clamp(0.0, 100.0) as u16;
                require_reference_hud(world, false)?;
                if world.resource::<terra_ui::SurvivalView>().hunger_stage != vitals.hunger_stage()
                {
                    return Err("Hunger icon stage did not follow the authoritative hunger".into());
                }
                capture(world, &format!("hunger-{percentage}-percent"))?;
                fixture.advance();
            }
        }
        7 => {
            forward = true;
            sprint = true;
            if vitals.stamina < 65.0 {
                require_reference_hud(world, false)?;
                capture(world, "stamina-active")?;
                fixture.advance();
            }
        }
        8 => {
            forward = true;
            sprint = true;
            if vitals.exhausted {
                require_reference_hud(world, false)?;
                capture(world, "stamina-exhausted")?;
                fixture.advance();
            }
        }
        9 if !vitals.exhausted && vitals.stamina > 30.0 => {
            fixture.height = state.ok_or("Missing position before flight")?.position[1];
            *world.resource_mut::<ControllerMode>() = ControllerMode::Fly;
            fixture.advance();
        }
        10 => {
            up = true;
            if state.is_some_and(|state| state.position[1] > fixture.height + 25.0) {
                *world.resource_mut::<ControllerMode>() = ControllerMode::Walk;
                up = false;
                fixture.advance();
            }
        }
        11 if dead => {
            fixture.advance();
        }
        12 if elapsed > 0.6 => {
            if !world.resource::<terra_ui::SurvivalView>().dead {
                return Err("Death HUD did not follow authority".into());
            }
            if world.resource::<terra_ui::SurvivalView>().critical {
                return Err("Critical disturbance did not clear on death".into());
            }
            capture(world, "death")?;
            click_respawn(world)?;
            fixture.advance();
        }
        13 if !dead
            && !respawning
            && state.is_some_and(|state| state.generation == 1 && state.grounded) =>
        {
            if vitals.health != 100.0 || count != 0 {
                return Err("Respawn changed inventory or did not restore health".into());
            }
            fixture.advance();
        }
        14 if elapsed > 0.7 => {
            let chrome = world.resource::<terra_ui::VitalRenderStatus>();
            if !chrome.bound || chrome.health < 0.999 || chrome.heartbeat_period.is_some() {
                return Err(format!(
                    "Healthy HUD channel did not fill completely: {chrome:?}"
                ));
            }
            if !world.resource::<terra_player::CameraInputState>().captured {
                return Err("Respawn did not restore game capture".into());
            }
            if world.resource::<terra_ui::SurvivalView>().critical
                || world
                    .query::<&tilt_ui::ElementId>()
                    .iter(world)
                    .any(|id| id.0 == "survival-critical")
            {
                return Err("Critical disturbance remained after a healthy respawn".into());
            }
            require_reference_hud(world, true)?;
            capture(world, "respawn")?;
            fixture.advance();
        }
        15 if elapsed > 1.0 => {
            world.write_message(terra_ui::menu::MenuAction::Command("pause".into()));
            fixture.advance();
        }
        16 if elapsed > 0.3 => {
            world.write_message(terra_ui::menu::MenuAction::Command("options".into()));
            fixture.advance();
        }
        17 if elapsed > 0.5 => {
            let view = world.resource::<terra_ui::menu::OptionsView>();
            if view.gameplay {
                if !view.rows.iter().any(|row| row.id == "hud-edit") {
                    return Err("Gameplay HUD layout button missing".into());
                }
                click_label(world, "row-command", Some("Edit Layout"))?;
                fixture.advance();
            } else {
                click(world, "gameplay-options")?;
                fixture.entered = Instant::now();
            }
        }
        18 if elapsed > 0.5 => {
            if world.resource::<terra_ui::menu::MenuState>().page
                != terra_ui::menu::MenuPage::HudLayout
            {
                let menu = world.resource::<terra_ui::menu::MenuState>();
                let intent = world
                    .resource::<tilt_ui::UiBindingStore>()
                    .get_store::<terra_ui::MenuIntent>();
                return Err(format!(
                    "HUD Layout button did not open its page: page={:?}, error={}, intent={:?}",
                    menu.page,
                    menu.error,
                    intent.map(|intent| &intent.command)
                ));
            }
            let styled = world
                .query::<(&tilt_ui::ElementId, &Node)>()
                .iter(world)
                .any(|(id, node)| {
                    id.0 == "survival-hud"
                        && node.flex_direction == FlexDirection::Column
                        && node.position_type == PositionType::Absolute
                });
            if !styled {
                return Err("HUD stylesheet did not reach the rendered container".into());
            }
            if !world.resource::<terra_ui::SurvivalView>().editing
                || !world.resource::<terra_player::PlayerInputBlock>().0
            {
                return Err("HUD editor did not block gameplay".into());
            }
            capture(world, "hud-editor-default")?;
            let scale = world.resource::<bevy::ui::UiScale>().0;
            let mut windows =
                world.query_filtered::<&mut Window, With<bevy::window::PrimaryWindow>>();
            windows
                .single_mut(world)
                .map_err(|e| e.to_string())?
                .bypass_change_detection()
                .set_cursor_position(Some(Vec2::splat(30.0) * scale));
            world
                .resource_mut::<ButtonInput<MouseButton>>()
                .press(MouseButton::Left);
            fixture.advance();
        }
        19 if elapsed > 0.1 => {
            let scale = world.resource::<bevy::ui::UiScale>().0;
            let mut windows =
                world.query_filtered::<&mut Window, With<bevy::window::PrimaryWindow>>();
            windows
                .single_mut(world)
                .map_err(|e| e.to_string())?
                .bypass_change_detection()
                .set_cursor_position(Some(Vec2::new(470.0, 290.0) * scale));
            fixture.advance();
        }
        20 if elapsed > 0.3 => {
            world
                .resource_mut::<ButtonInput<MouseButton>>()
                .release(MouseButton::Left);
            capture(world, "hud-editor-moved")?;
            click(world, "hud-save")?;
            fixture.advance();
        }
        21 if elapsed > 0.3 => {
            if world.resource::<terra_core::GraphicsConfig>().hud_position == [0.0; 2] {
                return Err("HUD drag/save did not move the applied position".into());
            }
            let config = terra_core::GraphicsConfig::load(
                world
                    .resource::<terra_ui::menu::MenuPaths>()
                    .configs
                    .join("graphics.toml"),
            )
            .map_err(|e| e.to_string())?;
            if config.hud_position != world.resource::<terra_core::GraphicsConfig>().hud_position {
                return Err("HUD position failed disk reload".into());
            }
            world.write_message(terra_ui::menu::MenuAction::Command("back".into()));
            fixture.advance();
        }
        22 if elapsed > 0.3 => {
            world.write_message(terra_ui::menu::MenuAction::Command("resume".into()));
            fixture.advance();
        }
        23 if elapsed > 0.5 => {
            capture(world, "hud-saved-gameplay")?;
            world
                .resource_mut::<terra_network::client::ClientChatCommands>()
                .0
                .push_back(terra_network::client::ChatCommand::Send(
                    "/gamemode 1".into(),
                ));
            fixture.advance();
        }
        24 if world.resource::<ClientSession>().game_mode == GameMode::Creative
            && elapsed > 0.5 =>
        {
            if world.resource::<terra_ui::SurvivalView>().visible {
                return Err("Survival HUD remains visible in Creative".into());
            }
            world
                .resource_mut::<terra_network::client::ClientChatCommands>()
                .0
                .push_back(terra_network::client::ChatCommand::Send(
                    "/gamemode survival".into(),
                ));
            fixture.advance();
        }
        25 if world.resource::<ClientSession>().game_mode == GameMode::Survival
            && elapsed > 0.5 =>
        {
            if !world.resource::<terra_ui::SurvivalView>().visible {
                return Err("Survival HUD did not return".into());
            }
            world.write_message(terra_ui::menu::MenuAction::Command("pause".into()));
            fixture.advance();
        }
        26 if elapsed > 0.3 => {
            world.write_message(terra_ui::menu::MenuAction::Command("options".into()));
            fixture.advance();
        }
        27 if elapsed > 0.3 => {
            world.write_message(terra_ui::menu::MenuAction::Command(
                "gameplay-options".into(),
            ));
            fixture.advance();
        }
        28 if elapsed > 0.3 => {
            click_label(world, "row-command", Some("Edit Layout"))?;
            fixture.advance();
        }
        29 if elapsed > 0.5 => {
            if !world.resource::<terra_ui::minimap::MiniMapView>().editing {
                return Err("MiniMap is not available in HUD Layout".into());
            }
            let center = world
                .query::<(
                    &tilt_ui::ElementId,
                    &ComputedNode,
                    &bevy::ui::UiGlobalTransform,
                )>()
                .iter(world)
                .find_map(|(id, node, transform)| {
                    (id.0 == "mini-map" && !node.is_empty()).then_some(transform.translation)
                })
                .ok_or("HUD editor MiniMap node missing")?;
            let mut windows =
                world.query_filtered::<&mut Window, With<bevy::window::PrimaryWindow>>();
            let mut window = windows.single_mut(world).map_err(|e| e.to_string())?;
            let point = center / window.scale_factor();
            window
                .bypass_change_detection()
                .set_cursor_position(Some(point));
            world
                .resource_mut::<ButtonInput<MouseButton>>()
                .press(MouseButton::Left);
            fixture.advance();
        }
        30 if elapsed > 0.15 => {
            let mut windows =
                world.query_filtered::<&mut Window, With<bevy::window::PrimaryWindow>>();
            windows
                .single_mut(world)
                .map_err(|e| e.to_string())?
                .bypass_change_detection()
                .set_cursor_position(Some(Vec2::new(680.0, 430.0)));
            fixture.advance();
        }
        31 if elapsed > 0.3 => {
            world
                .resource_mut::<ButtonInput<MouseButton>>()
                .release(MouseButton::Left);
            capture(world, "hud-minimap-moved")?;
            click(world, "hud-save")?;
            fixture.advance();
        }
        32 if elapsed > 0.5 => {
            let applied = world.resource::<terra_core::GraphicsConfig>();
            let position = applied
                .minimap_position
                .ok_or("MiniMap drag did not save")?;
            if position[0] >= 0.99 || position[1] <= 0.01 {
                return Err(format!("MiniMap did not move: {position:?}"));
            }
            let saved = terra_core::GraphicsConfig::load(
                world
                    .resource::<terra_ui::menu::MenuPaths>()
                    .configs
                    .join("graphics.toml"),
            )
            .map_err(|e| e.to_string())?;
            if saved.minimap_position != Some(position)
                || saved.hud_position != applied.hud_position
            {
                return Err("MiniMap layout failed independent TOML roundtrip".into());
            }
            if let Some(host) = world.remove_resource::<terra_server::IntegratedServer>() {
                host.finish()?;
            }
            info!(
                "Native survival smoke passed: food, stamina, respawn, chamfered bars/heartbeat, MiniMap/HUD drag/save/reload and authoritative gamemode roundtrip"
            );
            fixture.stage = 255;
            world.write_message(AppExit::Success);
        }
        _ => {}
    }
    for (action, held) in [
        (GameAction::Forward, forward),
        (GameAction::Sprint, sprint),
        (GameAction::Jump, up),
        (GameAction::Place, (2..=6).contains(&fixture.stage)),
    ] {
        let binding = world
            .get_resource::<terra_core::ControlsConfig>()
            .map_or_else(|| action.default_binding(), |config| config.binding(action));
        let key_input = |world: &mut World, key| {
            let mut keys = world.resource_mut::<ButtonInput<KeyCode>>();
            if held {
                keys.press(key);
            } else {
                keys.release(key);
            }
        };
        match binding {
            terra_core::InputBinding::Key(key) => key_input(world, key),
            terra_core::InputBinding::Chord { modifier, key } => {
                key_input(world, modifier);
                key_input(world, key);
            }
            terra_core::InputBinding::Mouse(button) => {
                let mut mouse = world.resource_mut::<ButtonInput<MouseButton>>();
                if held {
                    mouse.press(button);
                } else {
                    mouse.release(button);
                }
            }
        }
    }
    if ready
        && fixture.stage > 1
        && world
            .query_filtered::<Entity, With<PlayerBody>>()
            .iter(world)
            .next()
            .is_none()
    {
        return Err("Missing real player body".into());
    }
    Ok(())
}
