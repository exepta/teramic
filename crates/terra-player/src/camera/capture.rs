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
impl CameraInputState {
    /// World actions may start once capture has settled past its enabling frame.
    pub fn capture_ready(&self) -> bool {
        self.captured && !self.just_captured
    }
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
    mut pending_capture: Local<bool>,
    (blocked, session, construction): (
        Res<PlayerInputBlock>,
        Option<Res<terra_network::client::ClientSession>>,
        Option<Res<super::ConstructionCamera>>,
    ),
) {
    state.just_captured = false;
    if construction.is_some_and(|camera| camera.active) {
        // Construction owns a free pointer. Panel resume events can arrive in
        // PreUpdate, before its Update system restores PlayerInputBlock.
        // Discard these intents instead of replaying them after construction.
        focused.clear();
        requests.clear();
        *pending_capture = false;
        state.initial_capture_pending = false;
        state.captured = false;
        state.suppress_motion = true;
        for (_, _, mut cursor) in &mut windows {
            if cursor.grab_mode != CursorGrabMode::None {
                cursor.grab_mode = CursorGrabMode::None;
            }
            if !cursor.visible {
                cursor.visible = true;
            }
        }
        return;
    }
    if !state.accept_input {
        focused.clear();
        requests.clear();
        *pending_capture = false;
        return;
    }
    let requested = requests.read().last().map(|request| request.0);
    if let Some(capture) = requested {
        // Loading finishes in PostUpdate, before the next UI update clears
        // PlayerInputBlock. Keep that intent until gameplay can accept it.
        *pending_capture = capture;
    }
    let failed = session
        .as_ref()
        .is_some_and(|session| session.error.is_some());
    if failed || (keys.just_pressed(Action::Pause) && requested != Some(true)) {
        *pending_capture = false;
    }
    let mut newly_focused = None;
    for event in focused.read() {
        if event.focused {
            newly_focused = Some(event.window);
        }
    }
    for (entity, window, mut cursor) in &mut windows {
        let next = if !window.focused
            || blocked.0
            || requested == Some(false)
            || session
                .as_ref()
                .is_some_and(|s| !s.ready || s.error.is_some())
        {
            state.initial_capture_pending = false;
            false
        } else if *pending_capture {
            *pending_capture = false;
            state.initial_capture_pending = false;
            true
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
