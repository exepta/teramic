//! Bounded, opt-in native-smoke diagnostics for input ownership transitions.
use super::*;

const MAX_TRANSITIONS: u16 = 128;

fn enabled(smoke: bool, value: Option<&str>) -> bool {
    smoke && value == Some("1")
}

pub(super) fn install(app: &mut App) {
    if enabled(
        std::env::args().any(|argument| argument.starts_with("--smoke-test")),
        std::env::var("TERRA_INPUT_TRACE").ok().as_deref(),
    ) {
        app.add_systems(Last, trace);
    }
}

#[derive(Default)]
struct Trace {
    frame: u64,
    transitions: u16,
    previous: String,
}

fn snapshot(world: &World) -> String {
    let inventory = world.get_resource::<InventoryState>();
    let chat = world.get_resource::<crate::chat::ChatState>();
    let session = world.get_resource::<ClientSession>();
    let menu = world.get_resource::<MenuState>();
    let focus = world.get_resource::<InputFocus>().and_then(InputFocus::get);
    let focused_element = focus.and_then(|entity| world.get::<ElementId>(entity));
    format!(
        "blocked={} captured={:?} inventory={:?} view_open={:?} chat={:?} chat_view_open={:?} settings={:?} menu={:?} dialog={:?} pets={:?} map={:?} session={:?} focus={focus:?}/{:?} mouse={:?} keys={:?}",
        world
            .get_resource::<PlayerInputBlock>()
            .is_some_and(|blocked| blocked.0),
        world
            .get_resource::<terra_player::CameraInputState>()
            .map(|camera| camera.captured),
        inventory.map(|state| (
            state.open,
            state.resume_pending,
            state.container_active,
            state.container_open_response
        )),
        world.get_resource::<InventoryView>().map(|view| view.open),
        chat.map(crate::chat::ChatState::input_state),
        world
            .get_resource::<crate::ChatView>()
            .map(|view| view.open),
        world
            .get_resource::<crate::SettingsView>()
            .map(|view| view.open),
        menu.map(|menu| &menu.page),
        menu.map(|menu| &menu.dialog),
        world
            .get_resource::<crate::pets::PetMenuState>()
            .map(|state| state.blocks_input()),
        world
            .get_resource::<crate::minimap::fullmap::FullMapState>()
            .map(|state| (state.open, state.blocks_input())),
        session.map(|session| (
            session.ready,
            session.teleporting,
            session.dead(),
            session.container.as_ref().map(|container| container.anchor)
        )),
        focused_element.map(|id| id.0.as_str()),
        world
            .get_resource::<ButtonInput<MouseButton>>()
            .map(|input| input.get_pressed().copied().collect::<Vec<_>>()),
        world
            .get_resource::<ButtonInput<KeyCode>>()
            .map(|input| input.get_pressed().copied().collect::<Vec<_>>()),
    )
}

fn trace(world: &mut World, mut state: Local<Trace>) {
    state.frame = state.frame.saturating_add(1);
    if state.transitions == MAX_TRANSITIONS {
        return;
    }
    let current = snapshot(world);
    if current != state.previous {
        state.transitions += 1;
        info!(frame = state.frame, transition = state.transitions, state = %current, "Native input ownership transition; inventory=(open,resume,active,opening), chat=(open,resume,teleport_resume), session=(ready,teleport,dead,container)");
        state.previous = current;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tracing_requires_both_a_native_smoke_and_explicit_opt_in() {
        assert!(enabled(true, Some("1")));
        for value in [None, Some("0"), Some("true"), Some("2")] {
            assert!(!enabled(true, value));
            assert!(!enabled(false, value));
        }
        assert!(!enabled(false, Some("1")));
    }

    #[test]
    fn snapshots_distinguish_closed_inventory_from_a_pending_resume_and_teleport() {
        let mut world = World::new();
        world.init_resource::<InventoryState>();
        world.init_resource::<ClientSession>();
        let closed = snapshot(&world);
        world.resource_mut::<InventoryState>().resume_pending = true;
        let pending = snapshot(&world);
        assert_ne!(closed, pending);
        world.resource_mut::<ClientSession>().teleporting = true;
        assert_ne!(pending, snapshot(&world));
    }
}
