//! Direct worker inspection and a draft-only world selection tool.
use super::*;
use terra_core::{GameAction, GameMode};
use terra_network::client::ReplicatedCreatures;
use terra_player::{CaptureRequest, PlayerInputBlock, PlayerSet, WorldInteractionBlock};

#[derive(Clone, Copy)]
pub(super) enum Selection {
    Area { first: Option<[i64; 3]> },
    Storage,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct InventorySlot {
    pub id: u8,
    pub label: String,
    pub identifier: String,
    pub count: String,
    pub preview_key: String,
    pub empty: bool,
}

pub(super) fn inventory_slots(
    session: &ClientSession,
    selected: Option<u64>,
    palette: Option<&terra_player::interaction::BlockPalette>,
) -> Vec<InventorySlot> {
    let Some(detail) = session
        .work
        .detail
        .as_ref()
        .filter(|detail| Some(detail.npc) == selected)
    else {
        return Vec::new();
    };
    (0..terra_core::npc_jobs::WORKER_SLOTS)
        .map(|index| {
            let item = detail.inventory.slot(index);
            InventorySlot {
                id: index as u8,
                label: item.map_or_else(
                    || "Leer".into(),
                    |item| {
                        palette.map_or_else(
                            || "Gegenstand".into(),
                            |palette| crate::inventory::item_label(item.item, palette),
                        )
                    },
                ),
                identifier: item.map_or_else(String::new, |item| item.item.identifier()),
                count: item.map_or_else(String::new, |item| item.count.to_string()),
                preview_key: item
                    .map_or_else(String::new, |item| crate::inventory::preview_key(item.item)),
                empty: item.is_none(),
            }
        })
        .collect()
}

#[cfg(test)]
#[path = "direct_tests.rs"]
mod tests;

pub(super) fn selection_view(state: &WorkUiState) -> NpcWorkView {
    let hint = match state.selection {
        Some(Selection::Area { first: None }) => {
            "Arbeitsgebiet wählen: Linksklick auf die erste Ecke. ESC bricht ab."
        }
        Some(Selection::Area { first: Some(_) }) => {
            "Arbeitsgebiet wählen: Linksklick auf die zweite Ecke. ESC bricht ab."
        }
        Some(Selection::Storage) => {
            "Lager wählen: Linksklick auf eine Truhe oder passende Berufsstation. ESC bricht ab."
        }
        None => "",
    };
    NpcWorkView {
        selection_active: state.selection.is_some(),
        selection_hint: hint.into(),
        selection_error: state.error.clone(),
        ..Default::default()
    }
}

pub(super) fn install(app: &mut App) {
    app.init_resource::<WorldInteractionBlock>()
        .add_systems(
            PreUpdate,
            input_frame
                .in_set(NpcWorkInputSet)
                .after(bevy::input::InputSystems)
                .before(crate::KingdomInputSet),
        )
        .add_systems(
            Update,
            pick.after(PlayerSet::Retargeting)
                .before(PlayerSet::Interaction),
        )
        .add_systems(
            Update,
            retain_input
                .in_set(super::NpcWorkBlockSet)
                .after(super::update)
                .after(crate::menu::MenuInputSet)
                .before(PlayerSet::Movement),
        );
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

fn released(world: &World) -> bool {
    world
        .get_resource::<ButtonInput<MouseButton>>()
        .is_none_or(|mouse| mouse.get_pressed().next().is_none())
        && world
            .get_resource::<ButtonInput<KeyCode>>()
            .is_none_or(|keys| keys.get_pressed().next().is_none())
}

fn playing(world: &World) -> bool {
    world
        .get_resource::<ClientSession>()
        .is_some_and(|session| {
            session.ready
                && session.error.is_none()
                && !session.dead()
                && !session.teleporting
                && session.game_mode != GameMode::Spectator
                && session.mounted.is_none()
        })
        && world
            .get_resource::<crate::menu::MenuState>()
            .is_none_or(|menu| menu.page == crate::menu::MenuPage::Playing)
}

fn other_panel(world: &World) -> bool {
    world
        .get_resource::<crate::KingdomView>()
        .is_some_and(|view| view.visible)
        || world
            .get_resource::<crate::InventoryView>()
            .is_some_and(|view| view.open)
        || world
            .get_resource::<crate::ChatView>()
            .is_some_and(|view| view.open)
        || world
            .get_resource::<crate::PetMenuView>()
            .is_some_and(|view| view.visible)
        || world
            .get_resource::<crate::TrainView>()
            .is_some_and(|view| view.open)
        || world
            .get_resource::<crate::SettingsView>()
            .is_some_and(|view| view.open)
        || world
            .get_resource::<crate::minimap::fullmap::FullMapState>()
            .is_some_and(|state| state.blocks_input())
}

fn capture(world: &mut World, value: bool) {
    if world.contains_resource::<Messages<CaptureRequest>>() {
        world.write_message(CaptureRequest(value));
    }
}

fn input_frame(world: &mut World) {
    world.resource_mut::<WorkUiState>().consumed_escape = false;
    if world.resource::<WorkUiState>().external_selection {
        world.resource_mut::<WorldInteractionBlock>().0 = true;
        if let Some(mut block) = world.get_resource_mut::<PlayerInputBlock>() {
            block.0 = false;
        }
        capture(world, true);
        return;
    }
    let active = world.resource::<WorkUiState>().blocks_input();
    if active && (!playing(world) || other_panel(world)) {
        *world.resource_mut::<WorkUiState>() = Default::default();
        world.resource_mut::<WorldInteractionBlock>().0 = false;
        return;
    }
    if active && pressed(world, GameAction::Pause) {
        let selection = world.resource::<WorkUiState>().selection.is_some();
        let mut state = world.resource_mut::<WorkUiState>();
        state.consumed_escape = true;
        state.selection = None;
        state.error.clear();
        if selection {
            state.direct = true;
        } else {
            state.direct = false;
            state.resume_pending = true;
            state.inspections.clear();
            state.inspection_error.clear();
        }
    }
    if world.resource::<WorkUiState>().resume_pending && released(world) {
        world.resource_mut::<WorkUiState>().resume_pending = false;
        if let Some(mut block) = world.get_resource_mut::<PlayerInputBlock>() {
            block.0 = false;
        }
        capture(world, true);
    }
    if world.resource::<WorkUiState>().selection.is_some() {
        if world
            .get_resource::<ButtonInput<MouseButton>>()
            .is_none_or(|mouse| !mouse.pressed(MouseButton::Left))
        {
            world.resource_mut::<WorkUiState>().selection_armed = true;
        }
        world.resource_mut::<WorldInteractionBlock>().0 = true;
        if let Some(mut block) = world.get_resource_mut::<PlayerInputBlock>() {
            block.0 = false;
        }
        capture(world, true);
        return;
    }
    world.resource_mut::<WorldInteractionBlock>().0 = false;
    if world.resource::<WorkUiState>().direct {
        if let Some(mut block) = world.get_resource_mut::<PlayerInputBlock>() {
            block.0 = true;
        }
        capture(world, false);
        return;
    }
    let right_click = world
        .get_resource::<ButtonInput<MouseButton>>()
        .is_some_and(|mouse| mouse.just_pressed(MouseButton::Right));
    if !playing(world)
        || other_panel(world)
        || !(right_click || pressed(world, GameAction::Use))
        || !world
            .get_resource::<terra_player::CameraInputState>()
            .is_some_and(|camera| camera.captured)
    {
        return;
    }
    let Some(npc) = world
        .get_resource::<terra_player::TargetCreature>()
        .and_then(|target| target.0)
    else {
        return;
    };
    let Some(creatures) = world.get_resource::<ReplicatedCreatures>() else {
        return;
    };
    let Some(creature) = creatures.creatures.get(&npc).map(|track| &track.current) else {
        return;
    };
    if creature.kind != terra_core::creatures::CreatureKind::Villager
        || creature.health <= 0.0
        || !terra_core::creatures::villagers::is_civilian(creature.variant)
        || creatures
            .bonds
            .get(&npc)
            .and_then(|bond| bond.owner)
            .is_some_and(|owner| Some(owner) != world.resource::<ClientSession>().uuid)
    {
        return;
    }
    let mut state = world.resource_mut::<WorkUiState>();
    state.selected = Some(npc);
    state.direct = true;
    state.loaded = None;
    state.pending = None;
    state.was_visible = false;
    state.error.clear();
    state.feedback.clear();
    state.inspections.clear();
    state.inspection_error.clear();
    if let Some(mut block) = world.get_resource_mut::<PlayerInputBlock>() {
        block.0 = true;
    }
    capture(world, false);
    // Any old generic greet/feed intent is cancelled along with normal world input.
    if let Some(mut actions) = world.get_resource_mut::<terra_network::client::CreatureActions>() {
        actions.0.clear();
    }
}

fn retain_input(
    state: Res<WorkUiState>,
    mut blocked: Option<ResMut<PlayerInputBlock>>,
    mut edits: ResMut<WorldInteractionBlock>,
    mut capture: MessageWriter<CaptureRequest>,
) {
    edits.0 = state.selection_active() || state.resume_pending;
    if state.selection_active() {
        if let Some(blocked) = blocked.as_mut() {
            blocked.0 = false;
        }
        capture.write(CaptureRequest(true));
    } else if state.direct || state.resume_pending {
        if let Some(blocked) = blocked.as_mut() {
            blocked.0 = true;
        }
        if state.direct {
            capture.write(CaptureRequest(false));
        }
    }
}

fn pick(world: &mut World) {
    let Some(selection) = world.resource::<WorkUiState>().selection else {
        return;
    };
    world.resource_mut::<WorldInteractionBlock>().0 = true;
    let primary_click = world
        .get_resource::<ButtonInput<MouseButton>>()
        .is_some_and(|mouse| mouse.just_pressed(MouseButton::Left));
    if !world.resource::<WorkUiState>().selection_armed
        || !(primary_click || pressed(world, GameAction::Break))
    {
        return;
    }
    let Some(position) = world
        .get_resource::<terra_player::TargetBlock>()
        .and_then(|target| target.0)
        .map(|hit| hit.position)
    else {
        world.resource_mut::<WorkUiState>().error = "Einen geladenen Block anvisieren".into();
        return;
    };
    match selection {
        Selection::Area { first: None } => {
            let mut state = world.resource_mut::<WorkUiState>();
            state.selection = Some(Selection::Area {
                first: Some(position),
            });
            state.error.clear();
            return;
        }
        Selection::Area { first: Some(first) } => {
            let kind = world.resource::<WorkUiState>().draft.kind;
            let area = match selected_area(kind, first, position) {
                Ok(area) => area,
                Err(error) => {
                    world.resource_mut::<WorkUiState>().error = error.into();
                    return;
                }
            };
            let mut state = world.resource_mut::<WorkUiState>();
            state.draft.min = area.min.map(|v| v.to_string());
            state.draft.max = area.max.map(|v| v.to_string());
        }
        Selection::Storage => {
            let material = world
                .get_resource::<terra_world::streaming::ChunkStore>()
                .and_then(|chunks| chunks.block_at(position));
            let kind = world.resource::<WorkUiState>().draft.kind;
            if material.is_none_or(|material| {
                !terra_core::profession_stations::supports_work(material, kind)
            }) {
                world.resource_mut::<WorkUiState>().error =
                    "An dieser Position befindet sich kein verfügbares Lager".into();
                return;
            }
            let primary = world
                .get_resource::<terra_world::edits::EditJournal>()
                .and_then(|journal| journal.object_state(position))
                .filter(|object| object.secondary)
                .and_then(|object| object.peer)
                .unwrap_or(position);
            world.resource_mut::<WorkUiState>().draft.storage = primary.map(|v| v.to_string());
        }
    }
    {
        let mut state = world.resource_mut::<WorkUiState>();
        state.selection = None;
        state.direct = true;
        state.error.clear();
    }
    if let Some(mut blocked) = world.get_resource_mut::<PlayerInputBlock>() {
        blocked.0 = true;
    }
    capture(world, false);
}

/// Surface jobs address crops, roots and drops above the selected support cells.
/// Keep this one-cell inclusion visible in the draft and inside the usual limits.
fn selected_area(
    kind: WorkKind,
    first: [i64; 3],
    second: [i64; 3],
) -> Result<WorkArea, &'static str> {
    let min: [i64; 3] = std::array::from_fn(|axis| first[axis].min(second[axis]));
    let mut max: [i64; 3] = std::array::from_fn(|axis| first[axis].max(second[axis]));
    if kind != WorkKind::Miner {
        max[1] = max[1]
            .checked_add(1)
            .filter(|y| *y <= 999_000)
            .ok_or("Koordinaten liegen außerhalb der Welt")?;
    }
    WorkArea::new(min, max).map_err(|_| "Arbeitsgebiet maximal 32 × 16 × 32 Blöcke")
}
