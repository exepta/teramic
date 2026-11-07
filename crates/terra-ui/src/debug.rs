use bevy::{ecs::system::SystemParam, prelude::*};
use terra_core::{DebugSettings, GameAction, GameplayInput};
use terra_player::{CameraInputState, PlayerInputBlock};

use crate::{
    inventory::InventoryState,
    menu::{MenuPage, MenuState},
};

pub(crate) mod fps;
pub(crate) mod terrain;

#[derive(Default)]
struct DebugChord {
    pending: bool,
    used: bool,
}

#[derive(SystemParam)]
struct DebugContext<'w> {
    menu: Option<Res<'w, MenuState>>,
    settings: Res<'w, crate::SettingsView>,
    inventory: Res<'w, InventoryState>,
    capture: Option<Res<'w, CameraInputState>>,
    blocked: Option<Res<'w, PlayerInputBlock>>,
}

pub(crate) fn install(app: &mut App) {
    fps::install(app);
    terrain::install(app);
    app.init_resource::<DebugSettings>().add_systems(
        Update,
        keyboard
            .after(crate::menu::MenuInputSet)
            .before(terra_player::PlayerSet::Movement),
    );
}

fn keyboard(
    input: GameplayInput,
    context: DebugContext,
    mut debug: ResMut<DebugSettings>,
    mut terrain: ResMut<terrain::TerrainDebug>,
    mut chord: Local<DebugChord>,
) {
    let active = !context.settings.open
        && !context.inventory.blocks_input()
        && context
            .menu
            .as_ref()
            .is_none_or(|menu| menu.page == MenuPage::Playing)
        && context
            .capture
            .as_ref()
            .is_none_or(|capture| capture.captured)
        && context.blocked.as_ref().is_none_or(|blocked| !blocked.0);
    if !active {
        *chord = DebugChord::default();
        return;
    }
    let overlay = input.binding(GameAction::DebugOverlay);
    let bounds = input.binding(GameAction::DebugBounds);
    let keys = &input.keys;
    let mouse = input.mouse.as_deref();
    if overlay.just_pressed(keys, mouse) {
        chord.pending = true;
        chord.used = false;
    }
    if let terra_core::InputBinding::Key(modifier) = overlay
        && modifier != KeyCode::KeyG
    {
        let binding = terra_core::InputBinding::Chord {
            modifier,
            key: KeyCode::KeyG,
        };
        let reserved = GameAction::ALL
            .into_iter()
            .filter(|action| *action != GameAction::DebugOverlay)
            .any(|action| {
                let other = input.binding(action);
                match other {
                    terra_core::InputBinding::Key(KeyCode::KeyG) => true,
                    terra_core::InputBinding::Chord { modifier, key }
                        if modifier == KeyCode::KeyG || key == KeyCode::KeyG =>
                    {
                        other.pressed(keys, mouse)
                    }
                    _ => false,
                }
            });
        if !reserved && binding.just_pressed(keys, mouse) {
            debug.enabled = true;
            terrain.cycle();
            chord.used = true;
        }
    }
    if bounds.just_pressed(keys, mouse) {
        debug.enabled = true;
        debug.block_placement = !debug.block_placement;
        chord.used = true;
    }
    if input.just_pressed(GameAction::DebugCreature) {
        debug.creature_inspection = !debug.creature_inspection;
        chord.used = true;
    }
    if input.just_pressed(GameAction::DebugMobBounds) {
        debug.creature_bounds = !debug.creature_bounds;
        chord.used = true;
    }
    if input.just_pressed(GameAction::DebugChunkBorders) {
        debug.chunk_borders = !debug.chunk_borders;
        chord.used = true;
    }
    // Delay the overlay until release so a shared chord never toggles it off first.
    if overlay.just_released(keys, mouse) {
        if chord.pending && !chord.used {
            debug.enabled = !debug.enabled;
        }
        *chord = DebugChord::default();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app() -> App {
        let mut app = App::new();
        app.init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<DebugSettings>()
            .init_resource::<terrain::TerrainDebug>()
            .init_resource::<crate::SettingsView>()
            .init_resource::<InventoryState>()
            .insert_resource(MenuState {
                page: MenuPage::Playing,
                ..default()
            })
            .add_systems(Update, keyboard);
        app
    }

    #[test]
    fn terrain_chord_cycles_layers_without_toggling_debug_on_release_or_while_typing() {
        let mut app = app();
        press(&mut app, KeyCode::F1);
        press(&mut app, KeyCode::KeyG);
        release(&mut app, KeyCode::KeyG);
        release(&mut app, KeyCode::F1);
        assert!(app.world().resource::<DebugSettings>().enabled);
        assert_eq!(
            app.world().resource::<terrain::TerrainDebug>().layer,
            Some(terrain::TerrainDebugLayer::Continentalness)
        );
        app.world_mut().resource_mut::<crate::SettingsView>().open = true;
        press(&mut app, KeyCode::F1);
        press(&mut app, KeyCode::KeyG);
        assert_eq!(
            app.world().resource::<terrain::TerrainDebug>().layer,
            Some(terrain::TerrainDebugLayer::Continentalness)
        );
    }

    #[test]
    fn default_f1_e_inspection_alias_does_not_toggle_the_general_overlay() {
        let mut app = app();
        press(&mut app, KeyCode::F1);
        press(&mut app, KeyCode::KeyE);
        assert!(app.world().resource::<DebugSettings>().creature_inspection);
        release(&mut app, KeyCode::KeyE);
        release(&mut app, KeyCode::F1);
        assert!(!app.world().resource::<DebugSettings>().enabled);
        press(&mut app, KeyCode::F3);
        press(&mut app, KeyCode::KeyE);
        assert!(!app.world().resource::<DebugSettings>().creature_inspection);
    }

    #[test]
    fn f9_toggles_chunk_borders_without_general_debug_and_never_while_typing() {
        let mut app = app();
        press(&mut app, KeyCode::F9);
        assert!(app.world().resource::<DebugSettings>().chunk_borders);
        assert!(!app.world().resource::<DebugSettings>().enabled);
        release(&mut app, KeyCode::F9);
        press(&mut app, KeyCode::F9);
        assert!(!app.world().resource::<DebugSettings>().chunk_borders);
        release(&mut app, KeyCode::F9);
        app.world_mut().resource_mut::<crate::SettingsView>().open = true;
        press(&mut app, KeyCode::F9);
        assert!(!app.world().resource::<DebugSettings>().chunk_borders);
    }

    #[test]
    fn f1_h_toggles_only_mob_bounds_and_is_blocked_while_typing() {
        let mut app = app();
        press(&mut app, KeyCode::F1);
        press(&mut app, KeyCode::KeyH);
        assert!(app.world().resource::<DebugSettings>().creature_bounds);
        release(&mut app, KeyCode::KeyH);
        release(&mut app, KeyCode::F1);
        assert!(!app.world().resource::<DebugSettings>().enabled);
        app.world_mut().resource_mut::<crate::SettingsView>().open = true;
        press(&mut app, KeyCode::F1);
        press(&mut app, KeyCode::KeyH);
        assert!(app.world().resource::<DebugSettings>().creature_bounds);
        release(&mut app, KeyCode::KeyH);
        release(&mut app, KeyCode::F1);
        app.world_mut().resource_mut::<crate::SettingsView>().open = false;
        press(&mut app, KeyCode::F1);
        press(&mut app, KeyCode::KeyH);
        release(&mut app, KeyCode::F1);
        assert!(!app.world().resource::<DebugSettings>().creature_bounds);
        assert!(!app.world().resource::<DebugSettings>().enabled);
    }

    fn press(app: &mut App, key: KeyCode) {
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(key);
        app.update();
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .clear();
    }

    fn release(app: &mut App, key: KeyCode) {
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .release(key);
        app.update();
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .clear();
    }

    #[test]
    fn debug_is_opt_in_and_plain_f1_toggles_only_once_on_release() {
        let mut app = app();
        assert_eq!(
            *app.world().resource::<DebugSettings>(),
            DebugSettings::default()
        );
        press(&mut app, KeyCode::F1);
        assert!(!app.world().resource::<DebugSettings>().enabled);
        release(&mut app, KeyCode::F1);
        assert!(app.world().resource::<DebugSettings>().enabled);
        press(&mut app, KeyCode::F1);
        release(&mut app, KeyCode::F1);
        assert!(!app.world().resource::<DebugSettings>().enabled);
    }

    #[test]
    fn f1_b_enables_debug_and_toggles_placement_without_release_toggle() {
        let mut app = app();
        press(&mut app, KeyCode::F1);
        press(&mut app, KeyCode::KeyB);
        assert_eq!(
            *app.world().resource::<DebugSettings>(),
            DebugSettings {
                enabled: true,
                block_placement: true,
                ..default()
            }
        );
        release(&mut app, KeyCode::F1);
        assert!(app.world().resource::<DebugSettings>().enabled);
        release(&mut app, KeyCode::KeyB);
        press(&mut app, KeyCode::KeyB);
        press(&mut app, KeyCode::F1);
        release(&mut app, KeyCode::F1);
        assert_eq!(
            *app.world().resource::<DebugSettings>(),
            DebugSettings {
                enabled: true,
                block_placement: false,
                ..default()
            }
        );
    }

    #[test]
    fn typing_and_rebinding_cannot_toggle_debug_or_leak_a_pending_chord() {
        let mut app = app();
        press(&mut app, KeyCode::F1);
        app.world_mut().resource_mut::<MenuState>().page = MenuPage::Options;
        app.update();
        app.world_mut().resource_mut::<MenuState>().page = MenuPage::Playing;
        release(&mut app, KeyCode::F1);
        assert!(!app.world().resource::<DebugSettings>().enabled);
        app.world_mut().resource_mut::<crate::SettingsView>().open = true;
        press(&mut app, KeyCode::F1);
        press(&mut app, KeyCode::KeyB);
        release(&mut app, KeyCode::F1);
        assert_eq!(
            *app.world().resource::<DebugSettings>(),
            DebugSettings::default()
        );
    }

    #[test]
    fn rebound_debug_keys_replace_defaults_and_still_support_chords() {
        let mut app = app();
        let mut controls = terra_core::ControlsConfig::default();
        controls
            .rebind(
                GameAction::DebugOverlay,
                terra_core::InputBinding::Key(KeyCode::F3),
            )
            .unwrap();
        controls
            .rebind(
                GameAction::DebugBounds,
                terra_core::InputBinding::Chord {
                    modifier: KeyCode::F3,
                    key: KeyCode::KeyG,
                },
            )
            .unwrap();
        app.insert_resource(controls);
        press(&mut app, KeyCode::F1);
        release(&mut app, KeyCode::F1);
        assert!(!app.world().resource::<DebugSettings>().enabled);
        press(&mut app, KeyCode::F3);
        press(&mut app, KeyCode::KeyG);
        release(&mut app, KeyCode::F3);
        assert_eq!(
            *app.world().resource::<DebugSettings>(),
            DebugSettings {
                enabled: true,
                block_placement: true,
                ..default()
            }
        );
    }

    #[test]
    fn creature_inspection_chord_is_independent_and_respects_open_menus() {
        let mut app = app();
        press(&mut app, KeyCode::F3);
        press(&mut app, KeyCode::KeyE);
        assert!(app.world().resource::<DebugSettings>().creature_inspection);
        assert!(!app.world().resource::<DebugSettings>().enabled);
        release(&mut app, KeyCode::KeyE);
        press(&mut app, KeyCode::KeyE);
        assert!(!app.world().resource::<DebugSettings>().creature_inspection);
        app.world_mut().resource_mut::<MenuState>().page = MenuPage::Options;
        release(&mut app, KeyCode::KeyE);
        press(&mut app, KeyCode::KeyE);
        assert!(!app.world().resource::<DebugSettings>().creature_inspection);
    }
}
