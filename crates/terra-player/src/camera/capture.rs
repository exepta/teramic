use super::PlayerInputBlock;
use bevy::{
    prelude::*,
    window::{CursorGrabMode, CursorOptions, PrimaryWindow, WindowFocused},
};
use terra_core::{GameAction as Action, GameplayInput};

#[derive(Message, Debug, Clone, Copy)]
pub struct CaptureRequest(pub bool);

#[derive(Resource, Debug)]
pub struct CameraInputState {
    pub captured: bool,
    pub(crate) accept_input: bool,
    pub(crate) just_captured: bool,
    pub(crate) initial_capture_pending: bool,
    pub(crate) suppress_motion: bool,
}
impl Default for CameraInputState {
    fn default() -> Self {
        Self {
            captured: false,
            accept_input: true,
            just_captured: false,
            initial_capture_pending: false,
            suppress_motion: true,
        }
    }
}
pub(crate) fn capture_on_start(mut state: ResMut<CameraInputState>) {
    // X11 cannot grab an unmapped startup window. Wait for its real focus event.
    state.accept_input = !std::env::args().any(|arg| {
        arg.starts_with("--smoke-test")
            && !matches!(
                arg.as_str(),
                "--smoke-test-survival"
                    | "--smoke-test-food"
                    | "--smoke-test-furnace-gameplay"
                    | "--smoke-test-farming"
                    | "--smoke-test-taming"
                    | "--smoke-test-pet-commands"
                    | "--smoke-test-breeding"
            )
    });
    state.initial_capture_pending = state.accept_input;
}
pub(crate) fn update_capture(
    keys: GameplayInput,
    mut windows: Query<(Entity, &Window, &mut CursorOptions), With<PrimaryWindow>>,
    mut state: ResMut<CameraInputState>,
    mut focused: MessageReader<WindowFocused>,
    mut requests: MessageReader<CaptureRequest>,
    (blocked, session): (
        Res<PlayerInputBlock>,
        Option<Res<terra_network::client::ClientSession>>,
    ),
) {
    state.just_captured = false;
    if !state.accept_input {
        focused.clear();
        requests.clear();
        return;
    }
    let requested = requests.read().last().map(|request| request.0);
    let mut newly_focused = None;
    for event in focused.read() {
        if event.focused {
            newly_focused = Some(event.window);
        }
    }
    for (entity, window, mut cursor) in &mut windows {
        let next = if !window.focused
            || blocked.0
            || session.as_ref().is_some_and(|s| s.error.is_some())
        {
            state.initial_capture_pending = false;
            false
        } else if let Some(capture) = requested {
            state.initial_capture_pending = false;
            capture
        } else if keys.just_pressed(Action::Pause) {
            state.initial_capture_pending = false;
            false
        } else if state.initial_capture_pending && newly_focused == Some(entity) {
            state.initial_capture_pending = false;
            true
        } else if keys.just_pressed(Action::Cursor) {
            !state.captured
        } else if !state.captured
            && keys
                .mouse
                .as_ref()
                .is_some_and(|m| m.just_pressed(MouseButton::Left))
        {
            true
        } else {
            state.captured
        };
        if next != state.captured {
            state.just_captured = next;
            state.captured = next;
            state.suppress_motion = true;
            cursor.grab_mode = if next {
                CursorGrabMode::Locked
            } else {
                CursorGrabMode::None
            };
            cursor.visible = !next;
        }
        if state.captured && cursor.grab_mode != CursorGrabMode::Locked {
            cursor.grab_mode = CursorGrabMode::Locked;
        }
    }
}
