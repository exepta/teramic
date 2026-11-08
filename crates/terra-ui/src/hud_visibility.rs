//! Temporary HUD presentation, independent of saved widget preferences.
use bevy::{camera::visibility::VisibilitySystems, prelude::*};
use terra_core::{GameAction, GameplayInput};
use terra_network::client::ClientSession;
use tilt_ui::{ElementClasses, ElementId, TiltUiComponentRuntimeSet, ToastState};

use crate::menu::{MenuDialog, MenuPage, MenuState};

pub use terra_core::HudVisibility;

#[derive(Resource, Default)]
struct SuppressedNodes(Vec<(Entity, Visibility)>);

pub(crate) fn install(app: &mut App) {
    app.init_resource::<HudVisibility>()
        .init_resource::<SuppressedNodes>()
        .add_systems(
            Update,
            restore
                .before(TiltUiComponentRuntimeSet::Instantiate)
                .before(crate::menu::MenuInputSet),
        )
        .add_systems(
            Update,
            toggle
                .after(crate::menu::MenuInputSet)
                .before(terra_player::PlayerSet::Movement),
        )
        .add_systems(
            PostUpdate,
            suppress
                .after(bevy::ui::UiSystems::PostLayout)
                .after(
                    crate::__tilt_ui_generated_components::network_notice_component::present_toasts,
                )
                .before(VisibilitySystems::VisibilityPropagate),
        );
}

fn toggle(
    input: GameplayInput,
    menu: Option<Res<MenuState>>,
    session: Option<Res<ClientSession>>,
    blocked: Option<Res<terra_player::PlayerInputBlock>>,
    mut hud: ResMut<HudVisibility>,
) {
    let connected = session.is_some_and(|session| session.ready)
        && menu.as_ref().is_none_or(|menu| menu.session_active);
    if !connected {
        hud.visible = true;
        return;
    }
    let playing = menu
        .as_ref()
        .is_none_or(|menu| menu.page == MenuPage::Playing && menu.dialog == MenuDialog::None);
    if playing
        && !blocked.is_some_and(|blocked| blocked.0)
        && input.just_pressed(GameAction::ToggleHud)
    {
        hud.visible = !hud.visible;
    }
}

// Restore before widget updates so their own visibility changes remain authoritative.
// UI picking has already run while the previous frame's HUD was hidden.
fn restore(world: &mut World) {
    let nodes = std::mem::take(&mut world.resource_mut::<SuppressedNodes>().0);
    for (entity, previous) in nodes {
        if let Some(mut visibility) = world.get_mut::<Visibility>(entity) {
            *visibility = previous;
        }
    }
}

fn passive_root(id: Option<&ElementId>, classes: Option<&ElementClasses>, chat_open: bool) -> bool {
    if let Some(id) = id {
        match id.0.as_str() {
            "inventory-hud"
            | "debug-overlay"
            | "target-card"
            | "rail-build-hint"
            | "train-driver-hud"
            | "train-blueprint-hint"
            | "mini-map"
            | "survival-hud"
            | "survival-hit"
            | "survival-critical"
            | "survival-critical-glow"
            | "sleep-overlay" => {
                return true;
            }
            "realm-entry-title" => return true,
            "chat-panel" => return !chat_open,
            _ => {}
        }
    }
    classes.is_some_and(|classes| {
        classes.classes.iter().any(|class| {
            matches!(
                class.as_str(),
                "crosshair" | "world-waypoints" | "mob-health" | "player-name" | "shared-cursor"
            )
        })
    })
}

fn suppress(world: &mut World) {
    if world.resource::<HudVisibility>().visible
        || world
            .get_resource::<MenuState>()
            .is_some_and(|menu| !menu.world_hud_active() || menu.page == MenuPage::HudLayout)
    {
        return;
    }
    let chat_open = world
        .get_resource::<crate::ChatView>()
        .is_some_and(|chat| chat.open);
    let roots = world
        .query_filtered::<(
            Entity,
            Option<&ElementId>,
            Option<&ElementClasses>,
            Has<ToastState>,
        ), (With<Node>, With<Visibility>)>()
        .iter(world)
        .filter_map(|(entity, id, classes, toast)| {
            (toast || passive_root(id, classes, chat_open)).then_some(entity)
        })
        .collect::<Vec<_>>();
    let mut visited = std::collections::HashSet::new();
    let mut previous = Vec::new();
    for root in roots {
        let mut pending = vec![root];
        while let Some(entity) = pending.pop() {
            if !visited.insert(entity) {
                continue;
            }
            if let Some(children) = world.get::<Children>(entity) {
                pending.extend(children.iter());
            }
            // Explicitly Visible descendants escape their parent's visibility in Bevy.
            // Suppress these too, including custom GPU material/image nodes.
            if let Some(mut visibility) = world.get_mut::<Visibility>(entity)
                && (entity == root || *visibility == Visibility::Visible)
            {
                previous.push((entity, *visibility));
                *visibility = Visibility::Hidden;
            }
        }
    }
    world.resource_mut::<SuppressedNodes>().0 = previous;
}

#[cfg(test)]
mod tests {
    use super::*;
    use terra_core::ControlsConfig;

    fn app() -> App {
        let mut app = App::new();
        let mut session = ClientSession::default();
        session.ready = true;
        app.init_resource::<ControlsConfig>()
            .init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<ButtonInput<MouseButton>>()
            .init_resource::<terra_player::PlayerInputBlock>()
            .init_resource::<crate::ChatView>()
            .insert_resource(session)
            .insert_resource(MenuState {
                page: MenuPage::Playing,
                session_active: true,
                ..default()
            });
        install(&mut app);
        app
    }

    fn f2(app: &mut App) {
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .reset_all();
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::F2);
        app.update();
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .reset_all();
    }

    fn node(world: &mut World, id: &str, visibility: Visibility) -> Entity {
        world
            .spawn((Node::default(), ElementId(id.into()), visibility))
            .id()
    }

    #[test]
    fn f2_only_toggles_during_unblocked_play_and_resets_on_world_exit() {
        let mut app = app();
        let bindings = app.world().resource::<ControlsConfig>().clone();
        for page in [
            MenuPage::Main,
            MenuPage::Worlds,
            MenuPage::Loading,
            MenuPage::Pause,
            MenuPage::Options,
            MenuPage::HudLayout,
        ] {
            app.world_mut().resource_mut::<MenuState>().page = page;
            f2(&mut app);
            assert!(app.world().resource::<HudVisibility>().visible);
        }
        app.world_mut().resource_mut::<MenuState>().page = MenuPage::Playing;
        app.world_mut()
            .resource_mut::<terra_player::PlayerInputBlock>()
            .0 = true;
        f2(&mut app);
        assert!(app.world().resource::<HudVisibility>().visible);
        app.world_mut()
            .resource_mut::<terra_player::PlayerInputBlock>()
            .0 = false;
        app.world_mut().resource_mut::<MenuState>().dialog = MenuDialog::Minimap;
        f2(&mut app);
        assert!(app.world().resource::<HudVisibility>().visible);
        app.world_mut().resource_mut::<MenuState>().dialog = MenuDialog::None;
        f2(&mut app);
        assert!(!app.world().resource::<HudVisibility>().visible);
        f2(&mut app);
        assert!(app.world().resource::<HudVisibility>().visible);
        f2(&mut app);
        app.world_mut().resource_mut::<ClientSession>().ready = false;
        app.update();
        assert!(app.world().resource::<HudVisibility>().visible);
        assert_eq!(app.world().resource::<ControlsConfig>(), &bindings);
    }

    #[test]
    fn hidden_hud_preserves_dialogs_and_restores_widget_preferences() {
        let mut app = app();
        let hotbar = node(app.world_mut(), "inventory-hud", Visibility::Inherited);
        let minimap = node(app.world_mut(), "mini-map", Visibility::Hidden);
        let debug = node(app.world_mut(), "debug-overlay", Visibility::Visible);
        let gpu_image = app
            .world_mut()
            .spawn((ImageNode::default(), Visibility::Visible))
            .id();
        app.world_mut().entity_mut(hotbar).add_child(gpu_image);
        let preserved = [
            "inventory-backdrop",
            "menu-shell",
            "full-map",
            "death-screen",
            "pet-menu",
        ]
        .map(|id| node(app.world_mut(), id, Visibility::Inherited));
        f2(&mut app);
        for entity in [hotbar, minimap, debug, gpu_image] {
            assert_eq!(
                app.world().get::<Visibility>(entity),
                Some(&Visibility::Hidden)
            );
        }
        for entity in preserved {
            assert_eq!(
                app.world().get::<Visibility>(entity),
                Some(&Visibility::Inherited)
            );
        }
        // A widget may change its own desired visibility while the HUD is suppressed.
        app.add_systems(Update, hide_debug.after(restore));
        fn hide_debug(mut debug: Query<(&ElementId, &mut Visibility)>) {
            for (id, mut visibility) in &mut debug {
                if id.0 == "debug-overlay" {
                    *visibility = Visibility::Hidden;
                }
            }
        }
        let target = node(app.world_mut(), "target-card", Visibility::Inherited);
        app.update();
        assert_eq!(
            app.world().get::<Visibility>(target),
            Some(&Visibility::Hidden)
        );
        f2(&mut app);
        assert_eq!(
            app.world().get::<Visibility>(hotbar),
            Some(&Visibility::Inherited)
        );
        assert_eq!(
            app.world().get::<Visibility>(gpu_image),
            Some(&Visibility::Visible)
        );
        assert_eq!(
            app.world().get::<Visibility>(minimap),
            Some(&Visibility::Hidden)
        );
        assert_eq!(
            app.world().get::<Visibility>(debug),
            Some(&Visibility::Hidden)
        );
        assert_eq!(
            app.world().get::<Visibility>(target),
            Some(&Visibility::Inherited)
        );
    }

    #[test]
    fn f2_hides_train_telemetry_and_build_hints_but_keeps_the_workshop_interactive() {
        let mut app = app();
        let passive = ["train-driver-hud", "train-blueprint-hint"]
            .map(|id| node(app.world_mut(), id, Visibility::Inherited));
        let workshop = node(
            app.world_mut(),
            "train-workshop-panel",
            Visibility::Inherited,
        );
        f2(&mut app);
        for entity in passive {
            assert_eq!(
                app.world().get::<Visibility>(entity),
                Some(&Visibility::Hidden)
            );
        }
        assert_eq!(
            app.world().get::<Visibility>(workshop),
            Some(&Visibility::Inherited)
        );
        f2(&mut app);
        for entity in passive {
            assert_eq!(
                app.world().get::<Visibility>(entity),
                Some(&Visibility::Inherited)
            );
        }
    }

    #[test]
    fn chat_and_layout_editor_remain_usable_while_hud_is_hidden() {
        let mut app = app();
        let chat = node(app.world_mut(), "chat-panel", Visibility::Inherited);
        let vital = node(app.world_mut(), "survival-hud", Visibility::Inherited);
        f2(&mut app);
        assert_eq!(
            app.world().get::<Visibility>(chat),
            Some(&Visibility::Hidden)
        );
        app.world_mut().resource_mut::<crate::ChatView>().open = true;
        app.update();
        assert_eq!(
            app.world().get::<Visibility>(chat),
            Some(&Visibility::Inherited)
        );
        assert_eq!(
            app.world().get::<Visibility>(vital),
            Some(&Visibility::Hidden)
        );
        app.world_mut().resource_mut::<MenuState>().page = MenuPage::HudLayout;
        app.update();
        assert_eq!(
            app.world().get::<Visibility>(vital),
            Some(&Visibility::Inherited)
        );
        assert!(!app.world().resource::<HudVisibility>().visible);
        app.world_mut().resource_mut::<MenuState>().page = MenuPage::Playing;
        app.update();
        assert_eq!(
            app.world().get::<Visibility>(vital),
            Some(&Visibility::Hidden)
        );
    }
}
