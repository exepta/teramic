use crate::menu::{MenuPage, MenuPaths, MenuState, OptionsDraft};
use bevy::prelude::*;
use terra_core::{Config, GraphicsConfig};

#[derive(Resource, Default)]
pub(crate) struct HudLayout {
    position: [f32; 2],
    drag: Option<Vec2>,
    widget: u8,
    chat: Option<[f32; 2]>,
    target: Option<[f32; 2]>,
    minimap: Option<[f32; 2]>,
    pub error: String,
}

#[derive(Resource, Default, PartialEq)]
pub(crate) struct HudWidgetStyles {
    pub chat: String,
    pub target: String,
    pub minimap: String,
}

#[derive(PartialEq)]
struct WidgetPlacements {
    canvas: Vec2,
    chat: Option<[f32; 2]>,
    target: Option<[f32; 2]>,
    minimap: Option<[f32; 2]>,
    minimap_size: Vec2,
}

pub(crate) fn install(app: &mut App) {
    app.init_resource::<HudLayout>()
        .init_resource::<HudWidgetStyles>()
        .add_systems(
            Update,
            drag.after(crate::menu::MenuInputSet)
                .before(crate::survival::SurvivalUiSet),
        );
}

pub(crate) fn open(world: &mut World) -> Result<(), String> {
    if world.resource::<MenuState>().page != MenuPage::Options {
        return Err("Open HUD Layout from Gameplay.".into());
    }
    let position = world.resource::<GraphicsConfig>().hud_position;
    let chat = world.resource::<GraphicsConfig>().chat_position;
    let target = world.resource::<GraphicsConfig>().target_position;
    let minimap = world.resource::<GraphicsConfig>().minimap_position;
    world.insert_resource(HudLayout {
        position,
        chat,
        target,
        minimap,
        ..default()
    });
    world.resource_mut::<MenuState>().page = MenuPage::HudLayout;
    world.write_message(terra_player::CaptureRequest(false));
    Ok(())
}

pub(crate) fn save(world: &mut World) -> Result<(), String> {
    if world.resource::<MenuState>().page != MenuPage::HudLayout {
        return Ok(());
    }
    let position = world.resource::<HudLayout>().position;
    let mut graphics = world.resource::<GraphicsConfig>().clone();
    graphics.hud_position = position;
    graphics.chat_position = world.resource::<HudLayout>().chat;
    graphics.target_position = world.resource::<HudLayout>().target;
    graphics.minimap_position = world.resource::<HudLayout>().minimap;
    let path = world.resource::<MenuPaths>().configs.join("graphics.toml");
    if let Err(error) = graphics.save(path) {
        world.resource_mut::<HudLayout>().error = format!("Cannot save HUD layout: {error}");
        return Ok(());
    }
    world.insert_resource(graphics);
    world.resource_mut::<OptionsDraft>().graphics.hud_position = position;
    world.resource_mut::<OptionsDraft>().graphics.chat_position =
        world.resource::<HudLayout>().chat;
    world
        .resource_mut::<OptionsDraft>()
        .graphics
        .target_position = world.resource::<HudLayout>().target;
    world
        .resource_mut::<OptionsDraft>()
        .graphics
        .minimap_position = world.resource::<HudLayout>().minimap;
    cancel(world);
    world.resource_mut::<MenuState>().status = "HUD layout saved".into();
    Ok(())
}
pub(crate) fn cancel(world: &mut World) {
    if world.resource::<MenuState>().page == MenuPage::HudLayout {
        world.resource_mut::<MenuState>().page = MenuPage::Options;
        *world.resource_mut::<HudLayout>() = HudLayout::default();
    }
}
pub(crate) fn reset(world: &mut World) {
    if world.resource::<MenuState>().page == MenuPage::HudLayout {
        *world.resource_mut::<HudLayout>() = HudLayout::default();
    }
}

fn bounds(canvas: Vec2, _oxygen: bool) -> (Vec2, Vec2) {
    let width = 420.0_f32.min((canvas.x - 24.0).max(80.0));
    let size = Vec2::new(width, width * 178.0 / 420.0);
    (size, (canvas - size - Vec2::splat(24.0)).max(Vec2::ZERO))
}
pub(crate) fn style(canvas: Vec2, position: [f32; 2], oxygen: bool) -> String {
    let (size, travel) = bounds(canvas, oxygen);
    let at = Vec2::splat(12.0) + travel * Vec2::from_array(position).clamp(Vec2::ZERO, Vec2::ONE);
    format!(
        "left: {:.2}px; top: {:.2}px; width: {:.2}px; height: {:.2}px;",
        at.x, at.y, size.x, size.y
    )
}
pub(crate) fn position(layout: &HudLayout) -> [f32; 2] {
    layout.position
}

pub(crate) fn toolbar_style(canvas: Vec2, position: [f32; 2], oxygen: bool) -> String {
    let (size, travel) = bounds(canvas, oxygen);
    let at = Vec2::splat(12.0) + travel * Vec2::from_array(position);
    let panel = Rect::from_corners(at - Vec2::splat(8.0), at + size + Vec2::splat(8.0));
    let tools = Vec2::new(400.0, 180.0);
    let right = (canvas.x - tools.x - 16.0).max(16.0);
    let bottom = (canvas.y - tools.y - 112.0).max(16.0);
    let positions = [
        Vec2::new(right, bottom),
        Vec2::new(right, 16.0),
        Vec2::splat(16.0),
        Vec2::new(16.0, bottom),
    ];
    let at = positions
        .into_iter()
        .find(|at| {
            panel
                .intersect(Rect::from_corners(*at, *at + tools))
                .is_empty()
        })
        .unwrap_or(positions[1]);
    format!(
        "left: {:.2}px; top: {:.2}px; right: auto; bottom: auto; width: 400px;",
        at.x, at.y
    )
}

#[derive(bevy::ecs::system::SystemParam)]
struct Canvas<'w, 's> {
    window: Query<'w, 's, &'static Window, With<bevy::window::PrimaryWindow>>,
    scale: Option<Res<'w, bevy::ui::UiScale>>,
    graphics: Option<Res<'w, GraphicsConfig>>,
    gameplay: Option<Res<'w, terra_core::GameplayConfig>>,
}
#[allow(clippy::too_many_arguments)]
fn drag(
    menu: Option<Res<MenuState>>,
    canvas: Canvas,
    mouse: Res<ButtonInput<MouseButton>>,
    mut layout: ResMut<HudLayout>,
    styles: Option<ResMut<HudWidgetStyles>>,
    mut last_placements: Local<Option<WidgetPlacements>>,
    view: Res<crate::SurvivalView>,
    tools: Query<(
        &tilt_ui::ElementId,
        &ComputedNode,
        &bevy::ui::UiGlobalTransform,
    )>,
) {
    let Canvas {
        window,
        scale,
        graphics,
        gameplay,
    } = canvas;
    let editing = menu.as_ref().is_some_and(|m| m.page == MenuPage::HudLayout);
    let canvas = window
        .iter()
        .next()
        .map_or(Vec2::new(1280.0, 800.0), |w| w.size())
        / scale.as_ref().map_or(1.0, |s| s.0).max(0.1);
    let chat = if editing {
        layout.chat
    } else {
        graphics.as_ref().and_then(|g| g.chat_position)
    };
    let target = if editing {
        layout.target
    } else {
        graphics.as_ref().and_then(|g| g.target_position)
    };
    let minimap = if editing {
        layout.minimap
    } else {
        graphics.as_ref().and_then(|g| g.minimap_position)
    };
    let map_size = minimap_extent(canvas, gameplay.as_ref().map_or(260, |g| g.minimap.size));
    let placements = WidgetPlacements {
        canvas,
        chat: chat.or(editing.then_some([0.0, 0.8])),
        target: target.or(editing.then_some([0.5, 0.0])),
        minimap: minimap.or(editing.then_some([1.0, 0.0])),
        minimap_size: map_size,
    };
    if let Some(mut styles) = styles
        && (styles.is_changed() || last_placements.as_ref() != Some(&placements))
    {
        let style = |position: Option<[f32; 2]>, size| {
            position.map_or_else(String::new, |position| widget_style(canvas, position, size))
        };
        styles.set_if_neq(HudWidgetStyles {
            chat: style(placements.chat, Vec2::new(650.0, 300.0)),
            target: style(placements.target, Vec2::new(320.0, 300.0)),
            minimap: style(placements.minimap, map_size),
        });
        *last_placements = Some(placements);
    }
    if !editing {
        if layout.drag.is_some() {
            layout.drag = None;
        }
        return;
    }
    let Ok(window) = window.single() else {
        return;
    };
    let scale = scale.map_or(1.0, |s| s.0).max(0.1);
    if !window.focused || !mouse.pressed(MouseButton::Left) {
        if layout.drag.is_some() {
            layout.drag = None;
        }
        return;
    }
    let Some(cursor) = window.cursor_position().map(|p| p / scale) else {
        return;
    };
    let (size, travel) = bounds(window.size() / scale, view.show_oxygen);
    let over_tools = window.physical_cursor_position().is_some_and(|pointer| {
        tools.iter().any(|(id, node, transform)| {
            id.0 == "hud-layout-tools"
                && !node.is_empty()
                && transform.try_inverse().is_some_and(|inverse| {
                    let p = inverse.transform_point2(pointer);
                    Rect::from_center_size(Vec2::ZERO, node.size()).contains(p)
                })
        })
    });
    if mouse.just_pressed(MouseButton::Left) && !over_tools {
        for (widget, pos, extent) in [
            (3, minimap.unwrap_or([1.0, 0.0]), map_size),
            (2, target.unwrap_or([0.5, 0.0]), Vec2::new(320.0, 300.0)),
            (1, chat.unwrap_or([0.0, 0.8]), Vec2::new(650.0, 300.0)),
            (0, layout.position, size),
        ] {
            let extent = extent
                .min(canvas - Vec2::splat(24.0))
                .max(Vec2::splat(20.0));
            let at = Vec2::splat(12.0)
                + (canvas - extent - Vec2::splat(24.0)).max(Vec2::ZERO) * Vec2::from_array(pos);
            let id = match widget {
                1 => "chat-panel",
                2 => "target-card",
                3 => "mini-map",
                _ => "survival-hud",
            };
            let hit = tools.iter().find(|(name, _, _)| name.0 == id).map_or_else(
                || Rect::from_corners(at, at + extent).contains(cursor),
                |(_, node, transform)| {
                    !node.is_empty()
                        && window.physical_cursor_position().is_some_and(|pointer| {
                            transform.try_inverse().is_some_and(|inverse| {
                                Rect::from_center_size(Vec2::ZERO, node.size())
                                    .contains(inverse.transform_point2(pointer))
                            })
                        })
                },
            );
            if hit {
                layout.drag = Some(cursor - at);
                layout.widget = widget;
                break;
            }
        }
    }
    if let Some(offset) = layout.drag {
        let travel = match layout.widget {
            1 => (canvas
                - Vec2::new(650.0, 300.0).min(canvas - Vec2::splat(24.0))
                - Vec2::splat(24.0))
            .max(Vec2::ZERO),
            2 => (canvas
                - Vec2::new(320.0, 300.0).min(canvas - Vec2::splat(24.0))
                - Vec2::splat(24.0))
            .max(Vec2::ZERO),
            3 => (canvas - map_size - Vec2::splat(24.0)).max(Vec2::ZERO),
            _ => travel,
        };
        let desired = (cursor - offset - Vec2::splat(12.0)).clamp(Vec2::ZERO, travel);
        let pos = (desired / travel.max(Vec2::ONE)).to_array();
        match layout.widget {
            1 => layout.chat = Some(pos),
            2 => layout.target = Some(pos),
            3 => layout.minimap = Some(pos),
            _ => layout.position = pos,
        }
    }
}

fn minimap_extent(canvas: Vec2, size: u16) -> Vec2 {
    let width = f32::from(size)
        .min(canvas.x * 0.34)
        .min(canvas.y * 0.55)
        .max(132.0);
    // Four readout rows, including their padding/borders and the panel gaps.
    // Reserve the whole panel when dragging it against the viewport's bottom.
    Vec2::new(width, width + 130.0).min((canvas - Vec2::splat(24.0)).max(Vec2::splat(20.0)))
}

fn widget_style(canvas: Vec2, position: [f32; 2], size: Vec2) -> String {
    let size = size.min((canvas - Vec2::splat(24.0)).max(Vec2::splat(20.0)));
    let at = Vec2::splat(12.0)
        + (canvas - size - Vec2::splat(24.0)).max(Vec2::ZERO) * Vec2::from_array(position);
    format!(
        "left: {:.2}px; top: {:.2}px; bottom: auto; right: auto; transform: translate(0px, 0px); width: {:.2}px; max-height: {:.2}px;",
        at.x, at.y, size.x, size.y
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stationary_widgets_keep_their_change_tick_and_follow_scale_and_editor_transitions() {
        let mut app = fixture();
        app.init_resource::<HudWidgetStyles>();
        app.world_mut()
            .resource_mut::<GraphicsConfig>()
            .chat_position = Some([1.0, 0.5]);
        app.update();
        let styles = |app: &App| app.world().resource::<HudWidgetStyles>().chat.clone();
        let tick = |app: &App| {
            app.world()
                .get_resource_ref::<HudWidgetStyles>()
                .unwrap()
                .last_changed()
        };
        let original = styles(&app);
        let initial = tick(&app);
        assert!(!original.is_empty());
        for _ in 0..4 {
            app.update();
        }
        assert_eq!(initial, tick(&app));
        app.world_mut()
            .resource_mut::<HudWidgetStyles>()
            .chat
            .clear();
        app.update();
        assert_eq!(original, styles(&app));
        app.world_mut().resource_mut::<UiScale>().0 = 1.0;
        app.update();
        assert_ne!(original, styles(&app));
        assert_ne!(initial, tick(&app));
        let scaled = styles(&app);
        let world = app.world_mut();
        world
            .query::<&mut Window>()
            .single_mut(world)
            .unwrap()
            .resolution
            .set(1024.0, 768.0);
        app.update();
        assert_ne!(scaled, styles(&app));
        open(app.world_mut()).unwrap();
        app.update();
        assert!(!app.world().resource::<HudWidgetStyles>().target.is_empty());
        cancel(app.world_mut());
        app.update();
        assert!(app.world().resource::<HudWidgetStyles>().target.is_empty());
    }

    #[test]
    fn minimap_drag_save_reload_and_cancel_are_independent() {
        let mut app = fixture();
        app.world_mut().insert_resource(bevy::ui::UiScale(1.0));
        open(app.world_mut()).unwrap();
        let window = app
            .world_mut()
            .query_filtered::<Entity, With<Window>>()
            .single(app.world())
            .unwrap();
        app.world_mut()
            .get_mut::<Window>(window)
            .unwrap()
            .set_cursor_position(Some(Vec2::new(1210.0, 100.0)));
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Left);
        app.update();
        assert_eq!(app.world().resource::<HudLayout>().widget, 3);
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .clear();
        app.world_mut()
            .get_mut::<Window>(window)
            .unwrap()
            .set_cursor_position(Some(Vec2::new(700.0, 500.0)));
        app.update();
        let moved = app.world().resource::<HudLayout>().minimap.unwrap();
        assert!(moved[0] < 1.0 && moved[1] > 0.0);
        assert_eq!(
            app.world().resource::<GraphicsConfig>().minimap_position,
            None
        );
        save(app.world_mut()).unwrap();
        let root = app.world().resource::<MenuPaths>().configs.clone();
        assert_eq!(
            GraphicsConfig::load(root.join("graphics.toml"))
                .unwrap()
                .minimap_position,
            Some(moved)
        );
        assert_eq!(
            app.world().resource::<GraphicsConfig>().hud_position,
            [0.0; 2]
        );
        open(app.world_mut()).unwrap();
        reset(app.world_mut());
        cancel(app.world_mut());
        assert_eq!(
            app.world().resource::<GraphicsConfig>().minimap_position,
            Some(moved)
        );
        std::fs::remove_dir_all(root.parent().unwrap()).unwrap();
    }
    #[test]
    fn target_card_can_be_dragged_and_saved_independently_of_vitals() {
        let mut app = fixture();
        open(app.world_mut()).unwrap();
        let entity = app
            .world_mut()
            .query_filtered::<Entity, With<Window>>()
            .single(app.world())
            .unwrap();
        app.world_mut()
            .get_mut::<Window>(entity)
            .unwrap()
            .set_cursor_position(Some(Vec2::new(600.0, 40.0)));
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Left);
        app.update();
        assert_eq!(app.world().resource::<HudLayout>().widget, 2);
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .clear();
        app.world_mut()
            .get_mut::<Window>(entity)
            .unwrap()
            .set_cursor_position(Some(Vec2::new(1200.0, 760.0)));
        app.update();
        assert_eq!(app.world().resource::<HudLayout>().target, Some([1.0; 2]));
        save(app.world_mut()).unwrap();
        let g = app.world().resource::<GraphicsConfig>();
        assert_eq!(g.target_position, Some([1.0; 2]));
        assert_eq!(g.hud_position, [0.0; 2]);
        assert_eq!(g.chat_position, None);
        let directory = app
            .world()
            .resource::<MenuPaths>()
            .configs
            .parent()
            .unwrap()
            .to_path_buf();
        std::fs::remove_dir_all(directory).unwrap();
    }
    fn fixture() -> App {
        let mut app = App::new();
        let root =
            std::env::temp_dir().join(format!("terra-hud-{}", terra_core::PlayerUuid::new_v4()));
        app.init_resource::<MenuState>()
            .init_resource::<GraphicsConfig>()
            .init_resource::<OptionsDraft>()
            .init_resource::<HudLayout>()
            .init_resource::<crate::SurvivalView>()
            .init_resource::<ButtonInput<MouseButton>>()
            .insert_resource(bevy::ui::UiScale(2.0))
            .insert_resource(MenuPaths {
                configs: root.join("configs"),
                worlds: terra_storage::WorldLibrary {
                    root: root.join("worlds"),
                },
            })
            .add_message::<terra_player::CaptureRequest>()
            .add_systems(Update, drag);
        app.world_mut().spawn((
            Window {
                resolution: (1280, 800).into(),
                focused: true,
                ..default()
            },
            bevy::window::PrimaryWindow,
        ));
        app.world_mut().resource_mut::<MenuState>().page = MenuPage::Options;
        app
    }
    #[test]
    fn dragging_is_scaled_clamped_and_does_not_change_applied_config_until_saved() {
        let mut app = fixture();
        open(app.world_mut()).unwrap();
        let window = app
            .world_mut()
            .query_filtered::<Entity, With<Window>>()
            .iter(app.world())
            .next()
            .unwrap();
        app.world_mut()
            .get_mut::<Window>(window)
            .unwrap()
            .set_cursor_position(Some(Vec2::new(40.0, 40.0)));
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Left);
        app.update();
        assert!(app.world().resource::<HudLayout>().drag.is_some());
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .clear();
        app.world_mut()
            .get_mut::<Window>(window)
            .unwrap()
            .set_cursor_position(Some(Vec2::new(1200.0, 750.0)));
        app.update();
        assert_eq!(app.world().resource::<HudLayout>().position, [1.0; 2]);
        assert_eq!(
            app.world().resource::<GraphicsConfig>().hud_position,
            [0.0; 2]
        );
        app.world_mut()
            .resource_mut::<OptionsDraft>()
            .graphics
            .brightness = 1.7;
        save(app.world_mut()).unwrap();
        let file = app
            .world()
            .resource::<MenuPaths>()
            .configs
            .join("graphics.toml");
        let loaded = GraphicsConfig::load(file).unwrap();
        assert_eq!(loaded.hud_position, [1.0; 2]);
        assert_eq!(loaded.brightness, GraphicsConfig::default().brightness);
        assert_eq!(
            app.world().resource::<OptionsDraft>().graphics.brightness,
            1.7
        );
        assert_eq!(app.world().resource::<MenuState>().page, MenuPage::Options);
        open(app.world_mut()).unwrap();
        reset(app.world_mut());
        cancel(app.world_mut());
        assert_eq!(
            app.world().resource::<GraphicsConfig>().hud_position,
            [1.0; 2]
        );
        std::fs::remove_dir_all(
            app.world()
                .resource::<MenuPaths>()
                .configs
                .parent()
                .unwrap(),
        )
        .unwrap();
    }
    #[test]
    fn failed_save_stays_in_editor_and_preserves_applied_position() {
        let mut app = fixture();
        let configs = app.world().resource::<MenuPaths>().configs.clone();
        std::fs::create_dir_all(&configs).unwrap();
        std::fs::create_dir(configs.join("graphics.toml")).unwrap();
        open(app.world_mut()).unwrap();
        app.world_mut().resource_mut::<HudLayout>().position = [0.5; 2];
        save(app.world_mut()).unwrap();
        assert_eq!(
            app.world().resource::<MenuState>().page,
            MenuPage::HudLayout
        );
        assert!(!app.world().resource::<HudLayout>().error.is_empty());
        assert_eq!(
            app.world().resource::<GraphicsConfig>().hud_position,
            [0.0; 2]
        );
        std::fs::remove_dir_all(configs.parent().unwrap()).unwrap();
    }
    #[test]
    fn relative_position_stays_inside_canvas_with_oxygen_at_multiple_scales() {
        for canvas in [
            Vec2::new(1280.0, 800.0),
            Vec2::new(1920.0, 1080.0),
            Vec2::new(640.0, 400.0),
        ] {
            for oxygen in [false, true] {
                let (size, travel) = bounds(canvas, oxygen);
                assert!((size + travel + Vec2::splat(24.0) - canvas).length() < 0.01);
                assert!(
                    style(canvas, [0.0; 2], oxygen).starts_with("left: 12.00px; top: 12.00px;")
                );
            }
        }
        let mut invalid = GraphicsConfig {
            hud_position: [f32::NAN, 0.0],
            ..default()
        };
        assert!(invalid.validate().is_err());
        invalid.hud_position = [1.1, 0.0];
        assert!(invalid.validate().is_err());
    }
    #[test]
    fn toolbar_moves_out_of_the_way_of_a_lower_right_panel() {
        let canvas = Vec2::new(960.0, 640.0);
        assert!(toolbar_style(canvas, [0.0; 2], false).contains("top: 348.00px"));
        assert!(toolbar_style(canvas, [0.75, 0.6], false).contains("top: 16.00px"));
    }
}
