use bevy::prelude::*;
use serde::Serialize;
use terra_core::GameMode;
use terra_network::client::ClientSession;
use terra_player::{CaptureRequest, PlayerInputBlock, PlayerSet};
use tilt_ui::{ElementId, html_shared};

mod chrome;
mod effects;
pub use chrome::VitalRenderStatus;
pub use effects::{EffectCardView, StatusEffectView};

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct SurvivalUiSet;

#[html_shared]
#[derive(Resource, Default, Serialize, PartialEq)]
pub struct SurvivalView {
    pub visible: bool,
    pub dead: bool,
    pub respawning: bool,
    pub health: f32,
    pub stamina: f32,
    pub hunger: f32,
    pub oxygen: f32,
    pub health_label: String,
    pub stamina_label: String,
    pub oxygen_label: String,
    pub hunger_label: String,
    pub hunger_stage: usize,
    pub show_oxygen: bool,
    pub exhausted: bool,
    pub hit: bool,
    pub hit_style: String,
    pub critical: bool,
    pub death_message: String,
    pub error: String,
    pub editing: bool,
    pub position: String,
    pub tools_position: String,
    pub layout_error: String,
}

#[derive(Resource, Default)]
struct HudState {
    previous: Option<(terra_core::PlayerUuid, f32, bool)>,
    flash_until: f64,
    flash_strength: f32,
    resume_pending: bool,
    oxygen_until: f64,
}

const DAMAGE_GLOW_SECONDS: f64 = 0.45;

fn damage_glow_opacity(remaining: f64, strength: f32) -> f32 {
    let t = (remaining / DAMAGE_GLOW_SECONDS).clamp(0.0, 1.0) as f32;
    strength * t * t * (3.0 - 2.0 * t)
}

pub(crate) fn install(app: &mut App) {
    crate::hud_layout::install(app);
    chrome::install(app);
    effects::install(app);
    app.init_resource::<SurvivalView>()
        .init_resource::<HudState>()
        .add_systems(
            Update,
            update
                .run_if(crate::cadence::ready)
                .in_set(SurvivalUiSet)
                .after(crate::menu::MenuInputSet)
                .before(PlayerSet::Movement),
        )
        .add_systems(
            PostUpdate,
            atlas_frames.before(bevy::ui::UiSystems::Prepare),
        );
}

#[allow(clippy::too_many_arguments)]
fn update(
    session: Option<Res<ClientSession>>,
    menu: Option<Res<crate::menu::MenuState>>,
    inventory: Res<crate::inventory::InventoryState>,
    chat: Res<crate::chat::ChatState>,
    camera: Option<Res<terra_player::CameraInputState>>,
    settings: Res<crate::SettingsView>,
    time: Res<Time>,
    mut state: ResMut<HudState>,
    mut view: ResMut<SurvivalView>,
    mut blocked: ResMut<PlayerInputBlock>,
    mut capture: MessageWriter<CaptureRequest>,
    graphics: Option<Res<terra_core::GraphicsConfig>>,
    layout: Res<crate::hud_layout::HudLayout>,
    windows: Query<&Window, With<bevy::window::PrimaryWindow>>,
    scale: Option<Res<bevy::ui::UiScale>>,
) {
    let editing = menu
        .as_ref()
        .is_some_and(|m| m.page == crate::menu::MenuPage::HudLayout);
    let session =
        session.filter(|session| session.ready && session.game_mode == GameMode::Survival);
    let playing = menu
        .as_ref()
        .is_none_or(|menu| menu.page == crate::menu::MenuPage::Playing);
    if session.is_none() && !editing {
        *state = HudState::default();
        if *view != SurvivalView::default() {
            *view = SurvivalView::default();
        }
        return;
    }
    let v = session.as_ref().map_or(
        terra_core::SurvivalVitals {
            health: 78.0,
            stamina: 61.0,
            hunger: 75.0,
            ..default()
        },
        |s| s.stats.survival,
    );
    let now = time.elapsed_secs_f64();
    if v.oxygen < 99.99 {
        state.oxygen_until = now + 0.35;
    }
    let show_oxygen = v.oxygen < 99.99 || now < state.oxygen_until;
    if let Some(session) = session.as_ref()
        && let Some(uuid) = session.uuid
    {
        if let Some((_, health, dead)) = state.previous.filter(|(id, _, _)| *id == uuid) {
            if v.health < health {
                state.flash_until = now + DAMAGE_GLOW_SECONDS;
                state.flash_strength = (0.25 + (health - v.health) * 0.02).min(0.75);
            }
            if dead && !v.dead() {
                state.resume_pending = true;
            }
        }
        if v.dead() && playing {
            blocked.0 = true;
            if camera.as_ref().is_some_and(|camera| camera.captured)
                || state.previous.is_none_or(|(_, _, dead)| !dead)
            {
                capture.write(CaptureRequest(false));
            }
        } else if state.resume_pending
            && playing
            && !session.teleporting
            && !inventory.blocks_input()
            && !chat.blocks_input()
            && !settings.open
        {
            blocked.0 = false;
            capture.write(CaptureRequest(true));
            state.resume_pending = false;
        }
        state.previous = Some((uuid, v.health, v.dead()));
    }
    let numbers = |value: f32| format!("{} / 100", value.ceil().clamp(0.0, 100.0) as u16);
    let canvas = windows
        .iter()
        .next()
        .map_or(Vec2::new(1280.0, 800.0), Window::size)
        / scale.map_or(1.0, |s| s.0).max(0.1);
    let position = if editing {
        crate::hud_layout::position(&layout)
    } else {
        graphics.map_or([0.0; 2], |g| g.hud_position)
    };
    let next = SurvivalView {
        visible: editing || menu.as_ref().is_none_or(|menu| menu.world_hud_active()),
        dead: v.dead(),
        respawning: session.as_ref().is_some_and(|s| s.teleporting),
        health: v.health,
        stamina: v.stamina,
        hunger: v.hunger,
        oxygen: v.oxygen,
        health_label: numbers(v.health),
        stamina_label: numbers(v.stamina),
        oxygen_label: numbers(v.oxygen),
        hunger_label: format!("{} %", v.hunger.ceil().clamp(0.0, 100.0) as u16),
        hunger_stage: v.hunger_stage(),
        show_oxygen,
        exhausted: v.exhausted,
        hit: now < state.flash_until && !v.dead(),
        hit_style: format!(
            "opacity: {:.3};",
            damage_glow_opacity(state.flash_until - now, state.flash_strength)
        ),
        critical: v.health < 30.0 && !v.dead() && !editing,
        death_message: v
            .last_damage
            .map_or("Your journey ended here.", |cause| cause.message())
            .into(),
        error: session
            .as_ref()
            .and_then(|s| s.respawn_error.clone())
            .unwrap_or_default(),
        editing,
        layout_error: layout.error.clone(),
        position: crate::hud_layout::style(canvas, position, show_oxygen),
        tools_position: crate::hud_layout::toolbar_style(canvas, position, show_oxygen),
    };
    if *view != next {
        *view = next;
    }
}

fn atlas_frames(
    view: Res<SurvivalView>,
    assets: Res<Assets<Image>>,
    mut images: Query<(&ElementId, &mut bevy::ui::widget::ImageNode)>,
) {
    for (id, mut image) in &mut images {
        if id.0 == "survival-food-frame" {
            // Remove the export padding while retaining the generated frame's alpha.
            let rect = Some(Rect::from_corners(Vec2::splat(80.0), Vec2::splat(1175.0)));
            if image.rect != rect {
                image.rect = rect;
            }
            continue;
        }
        let frame = match id.0.as_str() {
            "survival-breath" => 2,
            "survival-food" => 3 + view.hunger_stage,
            _ => continue,
        };
        let Some(atlas) = assets.get(&image.image) else {
            continue;
        };
        let cell = Vec2::new(atlas.width() as f32 / 4.0, atlas.height() as f32 / 2.0);
        let min = Vec2::new((frame % 4) as f32 * cell.x, (frame / 4) as f32 * cell.y);
        let rect = Some(Rect::from_corners(min, min + cell));
        if image.rect != rect {
            image.rect = rect;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use terra_core::{PlayerProfile, SurvivalVitals};
    fn fixture() -> App {
        let mut app = App::new();
        app.init_resource::<SurvivalView>()
            .init_resource::<crate::hud_layout::HudLayout>()
            .init_resource::<HudState>()
            .init_resource::<crate::inventory::InventoryState>()
            .init_resource::<crate::chat::ChatState>()
            .init_resource::<crate::SettingsView>()
            .init_resource::<terra_core::DebugSettings>()
            .init_resource::<Time>()
            .init_resource::<PlayerInputBlock>()
            .init_resource::<terra_player::CameraInputState>()
            .init_resource::<ClientSession>()
            .add_message::<CaptureRequest>()
            .add_systems(Update, update);
        {
            let mut session = app.world_mut().resource_mut::<ClientSession>();
            session.ready = true;
            session.uuid = Some(PlayerProfile::default().uuid);
        }
        app
    }
    #[test]
    fn hud_styles_use_the_real_tilt_ui_parser() {
        tilt_ui::tilt_ui_css::parse_stylesheet(include_str!(
            "../../../src-ui/components/survival-hud.component.css"
        ))
        .unwrap();
    }
    #[test]
    fn an_open_inventory_does_not_hide_survival_vitals() {
        let mut app = fixture();
        app.world_mut()
            .resource_mut::<crate::inventory::InventoryState>()
            .open = true;
        app.update();
        assert!(app.world().resource::<SurvivalView>().visible);
    }
    #[test]
    fn session_options_and_pause_are_foreground_overlays_over_the_existing_hud() {
        let mut app = fixture();
        app.init_resource::<crate::menu::MenuState>();
        app.world_mut().resource_mut::<crate::SettingsView>().open = true;
        for page in [
            crate::menu::MenuPage::Playing,
            crate::menu::MenuPage::Pause,
            crate::menu::MenuPage::Options,
            crate::menu::MenuPage::HudLayout,
        ] {
            {
                let mut menu = app.world_mut().resource_mut::<crate::menu::MenuState>();
                menu.session_active = true;
                menu.page = page;
            }
            app.update();
            assert!(app.world().resource::<SurvivalView>().visible, "{page:?}");
        }
        app.world_mut()
            .resource_mut::<crate::menu::MenuState>()
            .session_active = false;
        app.world_mut()
            .resource_mut::<crate::menu::MenuState>()
            .page = crate::menu::MenuPage::Main;
        app.update();
        assert!(!app.world().resource::<SurvivalView>().visible);
    }
    #[test]
    fn survival_hud_is_independent_of_debug_and_resumes_capture_after_loaded_respawn() {
        let mut app = fixture();
        app.update();
        assert!(app.world().resource::<SurvivalView>().visible);
        app.world_mut()
            .resource_mut::<crate::SettingsView>()
            .overlay = true;
        app.update();
        assert!(app.world().resource::<SurvivalView>().visible);
        app.world_mut()
            .resource_mut::<ClientSession>()
            .stats
            .survival
            .health = 0.0;
        app.update();
        assert!(app.world().resource::<SurvivalView>().dead);
        assert!(app.world().resource::<PlayerInputBlock>().0);
        assert!(
            app.world_mut()
                .resource_mut::<Messages<CaptureRequest>>()
                .drain()
                .any(|request| !request.0)
        );
        {
            let mut session = app.world_mut().resource_mut::<ClientSession>();
            session.teleporting = true;
            session.stats.survival = SurvivalVitals::default();
        }
        app.update();
        assert!(!app.world().resource::<SurvivalView>().dead);
        assert!(
            app.world_mut()
                .resource_mut::<Messages<CaptureRequest>>()
                .drain()
                .all(|request| !request.0)
        );
        app.world_mut().resource_mut::<ClientSession>().teleporting = false;
        app.update();
        assert!(
            app.world_mut()
                .resource_mut::<Messages<CaptureRequest>>()
                .drain()
                .any(|request| request.0)
        );
        app.world_mut().resource_mut::<ClientSession>().game_mode = GameMode::Creative;
        app.update();
        assert!(!app.world().resource::<SurvivalView>().visible);
    }
    #[test]
    fn each_hunger_stage_selects_one_shared_atlas_cell() {
        use bevy::{
            asset::RenderAssetUsages,
            render::render_resource::{Extent3d, TextureDimension, TextureFormat},
        };
        let mut app = App::new();
        app.init_resource::<Assets<Image>>()
            .init_resource::<SurvivalView>()
            .add_systems(Update, atlas_frames);
        let image = app
            .world_mut()
            .resource_mut::<Assets<Image>>()
            .add(Image::new_fill(
                Extent3d {
                    width: 128,
                    height: 64,
                    depth_or_array_layers: 1,
                },
                TextureDimension::D2,
                &[255; 4],
                TextureFormat::Rgba8UnormSrgb,
                RenderAssetUsages::MAIN_WORLD,
            ));
        let entity = app
            .world_mut()
            .spawn((
                ElementId("survival-food".into()),
                bevy::ui::widget::ImageNode::new(image),
            ))
            .id();
        for stage in 0..5 {
            app.world_mut().resource_mut::<SurvivalView>().hunger_stage = stage;
            app.update();
            let rect = app
                .world()
                .get::<bevy::ui::widget::ImageNode>(entity)
                .unwrap()
                .rect
                .unwrap();
            let index = 3 + stage;
            assert_eq!(
                rect.min,
                Vec2::new((index % 4) as f32 * 32.0, (index / 4) as f32 * 32.0)
            );
            assert_eq!(rect.size(), Vec2::splat(32.0));
        }
    }

    #[test]
    fn hunger_meter_and_icons_follow_vitals_while_oxygen_keeps_the_layout_stable() {
        let mut app = fixture();
        for (hunger, stage) in [(90.0, 0), (75.0, 1), (50.0, 2), (30.0, 3), (10.0, 4)] {
            app.world_mut()
                .resource_mut::<ClientSession>()
                .stats
                .survival
                .hunger = hunger;
            app.update();
            let view = app.world().resource::<SurvivalView>();
            assert_eq!(view.hunger, hunger);
            assert_eq!(view.hunger_stage, stage);
        }
        let position = app.world().resource::<SurvivalView>().position.clone();
        assert!(!app.world().resource::<SurvivalView>().show_oxygen);
        app.world_mut()
            .resource_mut::<ClientSession>()
            .stats
            .survival
            .oxygen = 65.0;
        app.update();
        let view = app.world().resource::<SurvivalView>();
        assert!(view.show_oxygen);
        assert_eq!(view.position, position);
        app.world_mut()
            .resource_mut::<ClientSession>()
            .stats
            .survival
            .oxygen = 100.0;
        app.update();
        assert!(
            app.world().resource::<SurvivalView>().show_oxygen,
            "show the full refill before switching back"
        );
        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(std::time::Duration::from_millis(400));
        app.update();
        assert!(!app.world().resource::<SurvivalView>().show_oxygen);
        assert_eq!(app.world().resource::<SurvivalView>().position, position);
    }

    #[test]
    fn critical_disturbance_starts_below_thirty_and_clears_on_heal_or_death() {
        let mut app = fixture();
        app.update();
        for (health, expected) in [
            (30.0, false),
            (29.0, true),
            (12.0, true),
            (40.0, false),
            (0.0, false),
        ] {
            app.world_mut()
                .resource_mut::<ClientSession>()
                .stats
                .survival
                .health = health;
            app.update();
            assert_eq!(app.world().resource::<SurvivalView>().critical, expected);
        }
        app.world_mut()
            .resource_mut::<ClientSession>()
            .stats
            .survival
            .health = 29.0;
        app.world_mut().resource_mut::<ClientSession>().game_mode = GameMode::Creative;
        app.update();
        assert!(!app.world().resource::<SurvivalView>().critical);
    }

    #[test]
    fn damage_edge_glow_fades_without_a_full_screen_red_fill() {
        let mut app = fixture();
        app.update();
        app.world_mut()
            .resource_mut::<ClientSession>()
            .stats
            .survival
            .health = 75.0;
        app.update();
        let view = app.world().resource::<SurvivalView>();
        assert!(view.hit);
        assert_eq!(view.hit_style, "opacity: 0.750;");
        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(std::time::Duration::from_millis(225));
        app.update();
        assert_eq!(
            app.world().resource::<SurvivalView>().hit_style,
            "opacity: 0.375;"
        );
        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(std::time::Duration::from_millis(250));
        app.update();
        assert!(!app.world().resource::<SurvivalView>().hit);
        let template = include_str!("../../../src-ui/components/survival-hud.component.html");
        assert_eq!(template.matches("class=\"damage-edge edge-").count(), 8);
    }

    #[test]
    fn small_hits_are_subtle_and_repeated_hits_do_not_stack_opacity() {
        let mut app = fixture();
        app.update();
        for health in [99.0, 98.0, 97.0] {
            app.world_mut()
                .resource_mut::<ClientSession>()
                .stats
                .survival
                .health = health;
            app.update();
            assert_eq!(
                app.world().resource::<SurvivalView>().hit_style,
                "opacity: 0.270;"
            );
        }
        assert_eq!(damage_glow_opacity(-1.0, 0.75), 0.0);
        assert_eq!(damage_glow_opacity(1.0, 0.75), 0.75);
    }
}
