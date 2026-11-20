//! World picks reserve gameplay input and never modify blocks themselves.
use super::*;
use terra_core::{GameAction, GameMode};
use terra_player::{CaptureRequest, PlayerInputBlock, PlayerSet, WorldInteractionBlock};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum SelectionKind {
    Storage,
    Residential,
    House,
}
#[derive(Clone, Copy, Debug)]
pub(super) struct Selection {
    pub kind: SelectionKind,
    pub first: Option<[i64; 3]>,
}

#[derive(SystemSet, Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct BuildingInputSet;

pub(super) fn install(app: &mut App) {
    app.add_systems(
        PreUpdate,
        input
            .in_set(BuildingInputSet)
            .after(crate::NpcWorkInputSet)
            .before(crate::KingdomInputSet),
    )
    .add_systems(
        Update,
        pick.after(PlayerSet::Retargeting)
            .before(PlayerSet::Interaction),
    )
    .add_systems(
        Update,
        retain
            .after(crate::npc_work::NpcWorkBlockSet)
            .after(super::update)
            .after(crate::menu::MenuInputSet)
            .before(PlayerSet::Movement),
    );
}

pub(super) fn hint(selection: Option<Selection>) -> &'static str {
    match selection {
        Some(Selection {
            kind: SelectionKind::House,
            ..
        }) => "Hausstandort wählen: Linksklick auf den Boden. ESC bricht ab.",
        Some(Selection {
            kind: SelectionKind::Storage,
            first: None,
        }) => "Stadtlager wählen: erste Ecke am Boden anklicken. ESC bricht ab.",
        Some(Selection {
            kind: SelectionKind::Storage,
            first: Some(_),
        }) => "Stadtlager wählen: zweite Ecke anklicken. Truhen können danach im Bereich stehen.",
        Some(Selection {
            kind: SelectionKind::Residential,
            first: None,
        }) => "Wohngebiet wählen: erste Ecke am Boden anklicken. ESC bricht ab.",
        Some(Selection {
            kind: SelectionKind::Residential,
            first: Some(_),
        }) => "Wohngebiet wählen: zweite Ecke am Boden anklicken. ESC bricht ab.",
        None => "",
    }
}

pub(super) fn begin(world: &mut World, state: &mut BuildingUiState, kind: SelectionKind) {
    state.selection = Some(Selection { kind, first: None });
    state.armed = false;
    state.error.clear();
    state.feedback.clear();
    state.returning = false;
    world
        .resource_mut::<crate::npc_work::WorkUiState>()
        .external_selection = true;
    world.resource_mut::<WorldInteractionBlock>().0 = true;
    world.write_message(crate::KingdomMenuAction::Close);
    world.write_message(CaptureRequest(true));
}

fn pressed(world: &World, action: GameAction) -> bool {
    let binding = world
        .get_resource::<terra_core::ControlsConfig>()
        .map_or_else(
            || action.default_binding(),
            |controls| controls.binding(action),
        );
    world
        .get_resource::<ButtonInput<KeyCode>>()
        .is_some_and(|keys| {
            binding.just_pressed(keys, world.get_resource::<ButtonInput<MouseButton>>())
        })
}

fn restore(world: &mut World, state: &mut BuildingUiState, now: f64) {
    state.selection = None;
    state.returning = true;
    state.returned_at = now;
    world
        .resource_mut::<crate::npc_work::WorkUiState>()
        .external_selection = false;
    // The final corner click remains a gameplay input until the next frame.
    // Keep all edits blocked while the actual keep dialog resumes.
    world.resource_mut::<WorldInteractionBlock>().0 = true;
    world.resource_mut::<PlayerInputBlock>().0 = true;
    if let Some(keep) = state.keep {
        world.write_message(crate::KingdomMenuAction::Open(keep));
    }
    world.write_message(CaptureRequest(false));
}

pub(super) fn input(world: &mut World) {
    let playing = world
        .get_resource::<ClientSession>()
        .is_some_and(|session| {
            session.ready
                && session.error.is_none()
                && !session.dead()
                && !session.teleporting
                && session.game_mode != GameMode::Spectator
        })
        && world
            .get_resource::<crate::menu::MenuState>()
            .is_none_or(|menu| menu.page == crate::menu::MenuPage::Playing);
    let now = world
        .get_resource::<Time<Real>>()
        .map_or(0.0, |time| time.elapsed_secs_f64());
    world.resource_scope(|world, mut state: Mut<BuildingUiState>| {
        let returned = world.resource::<crate::KingdomView>().visible
            && !world.resource::<crate::KingdomView>().pending
            && world.resource::<crate::KingdomView>().bound;
        if state.returning && (returned || now - state.returned_at > 2.0 || !playing) {
            state.returning = false;
            if returned {
                world.write_message(crate::KingdomMenuAction::Tab("building".into()));
            }
            if !world.resource::<crate::KingdomView>().visible {
                world.resource_mut::<WorldInteractionBlock>().0 = false;
                world.write_message(CaptureRequest(true));
            }
        }
        if state.selection.is_none() {
            return;
        }
        if !playing {
            state.selection = None;
            state.returning = false;
            world
                .resource_mut::<crate::npc_work::WorkUiState>()
                .external_selection = false;
            world.resource_mut::<WorldInteractionBlock>().0 = false;
            return;
        }
        if pressed(world, GameAction::Pause) {
            state.error.clear();
            restore(world, &mut state, now);
            world
                .resource_mut::<crate::npc_work::WorkUiState>()
                .consumed_escape = true;
            return;
        }
        if world
            .get_resource::<ButtonInput<MouseButton>>()
            .is_none_or(|mouse| !mouse.pressed(MouseButton::Left))
        {
            state.armed = true;
        }
    });
}

pub(super) fn pick(world: &mut World) {
    let selection = world.resource::<BuildingUiState>().selection;
    let Some(selection) = selection else {
        return;
    };
    let clicked = world
        .get_resource::<ButtonInput<MouseButton>>()
        .is_some_and(|mouse| mouse.just_pressed(MouseButton::Left))
        || pressed(world, GameAction::Break);
    if !world.resource::<BuildingUiState>().armed || !clicked {
        return;
    }
    world.resource_mut::<WorldInteractionBlock>().0 = true;
    let Some(hit) = world
        .get_resource::<terra_player::TargetBlock>()
        .and_then(|target| target.0)
    else {
        world.resource_mut::<BuildingUiState>().error = "Einen geladenen Block anvisieren".into();
        return;
    };
    let now = world
        .get_resource::<Time<Real>>()
        .map_or(0.0, |time| time.elapsed_secs_f64());
    world.resource_scope(|world, mut state: Mut<BuildingUiState>| {
        if selection.kind == SelectionKind::House {
            let Some(y) = hit.position[1]
                .checked_add(1)
                .filter(|y| y.unsigned_abs() <= 999_000)
            else {
                state.error = "Koordinaten liegen außerhalb der Welt".into();
                return;
            };
            state.origin = Some([hit.position[0], y, hit.position[2]]);
            restore(world, &mut state, now);
            return;
        }
        let Some(first) = selection.first else {
            state.selection = Some(Selection {
                first: Some(hit.position),
                ..selection
            });
            state.error.clear();
            return;
        };
        match selected_area(selection.kind, first, hit.position) {
            Ok(area) => {
                if selection.kind == SelectionKind::Storage {
                    state.storage = Some(area);
                } else {
                    state.residential = Some(area);
                }
                state.error.clear();
                restore(world, &mut state, now);
            }
            Err(error) => state.error = error_label(error).into(),
        }
    });
}

pub(super) fn selected_area(
    kind: SelectionKind,
    first: [i64; 3],
    second: [i64; 3],
) -> Result<BuildArea, BuildError> {
    let mut area = BuildArea::new(first, second)?;
    if kind == SelectionKind::Storage {
        area.max[1] = area.max[1].checked_add(1).ok_or(BuildError::InvalidArea)?;
        if !area.valid() {
            return Err(BuildError::InvalidArea);
        }
    }
    Ok(area)
}

fn retain(
    state: Res<BuildingUiState>,
    mut blocked: ResMut<PlayerInputBlock>,
    mut edits: ResMut<WorldInteractionBlock>,
    mut capture: MessageWriter<CaptureRequest>,
) {
    if state.selection.is_some() {
        blocked.0 = false;
        edits.0 = true;
        capture.write(CaptureRequest(true));
    } else if state.returning {
        blocked.0 = true;
        edits.0 = true;
        capture.write(CaptureRequest(false));
    }
}
