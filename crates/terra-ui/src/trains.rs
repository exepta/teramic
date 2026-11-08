//! Workshop controls and private cargo gestures for assembled railway vehicles.
use bevy::{ecs::message::MessageCursor, prelude::*};
use serde::Serialize;
use terra_core::{
    GameAction, ItemStack,
    containers::ContainerGesture,
    trains::{CouplerEnd, TrainError, VehicleKind},
};
use terra_network::{
    client::ClientSession,
    protocol::trains::{TrainAction, TrainOutcome},
};
use tilt_ui::html_shared;

#[cfg(test)]
#[path = "trains_tests.rs"]
mod tests;

#[derive(Resource, Default)]
pub(crate) struct TrainUiState {
    pub open: bool,
    pub consumed_escape: bool,
    station: Option<[i64; 3]>,
    kind: VehicleKind,
    selected: Option<u64>,
    second: Option<u64>,
    front: bool,
    second_front: bool,
    cargo: bool,
    page: usize,
    pending: Option<u64>,
    status: String,
    identity: Option<(Option<Entity>, Option<terra_core::PlayerUuid>)>,
}
impl TrainUiState {
    pub(crate) fn blocks_input(&self) -> bool {
        self.open || self.consumed_escape
    }
}

#[derive(Message, Clone)]
pub enum TrainUiAction {
    Command(String),
}

#[derive(Clone, Serialize, PartialEq)]
pub struct TrainRow {
    pub id: u64,
    pub command: String,
    pub label: String,
    pub detail: String,
    pub selected: bool,
    pub second: bool,
    pub foreign: bool,
}
#[derive(Clone, Serialize, PartialEq)]
pub struct TrainSlot {
    pub index: u8,
    pub command: String,
    pub empty: bool,
    pub name: String,
    pub count: String,
    pub preview: String,
}
#[html_shared]
#[derive(Resource, Default, Serialize, PartialEq)]
pub struct TrainView {
    pub open: bool,
    pub cargo: bool,
    pub pending: bool,
    pub title: String,
    pub status: String,
    pub requirements: String,
    pub kind: String,
    pub selected: bool,
    pub pair: bool,
    pub front: String,
    pub second_front: String,
    pub page: String,
    pub rows: Vec<TrainRow>,
    pub player_slots: Vec<TrainSlot>,
    pub cargo_slots: Vec<TrainSlot>,
}

pub(crate) fn install(app: &mut App) {
    app.init_resource::<TrainUiState>()
        .init_resource::<TrainView>()
        .add_message::<TrainUiAction>()
        .add_message::<terra_network::client::TrainRequest>()
        .add_systems(
            Update,
            update
                .before(crate::menu::MenuInputSet)
                .before(terra_player::PlayerSet::Movement),
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
fn shift(world: &World) -> bool {
    world
        .get_resource::<ButtonInput<KeyCode>>()
        .is_some_and(|keys| keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight))
}
fn kind_label(kind: VehicleKind) -> &'static str {
    match kind {
        VehicleKind::Locomotive => "Lokomotive",
        VehicleKind::Tender => "Tender",
        VehicleKind::Passenger => "Personenwagen",
        VehicleKind::Freight => "Güterwagen",
    }
}
fn rejection(error: TrainError) -> &'static str {
    match error {
        TrainError::NotOwner | TrainError::Protected => "Dafür fehlen dir die Rechte.",
        TrainError::Stale => "Der Aufbau hat sich geändert. Bitte erneut versuchen.",
        TrainError::Incomplete => "Bauteile fehlen oder sind nicht miteinander verbunden.",
        TrainError::Unsupported => "Beide Drehgestelle müssen über einem Gleis stehen.",
        TrainError::Occupied => "Der Platz ist belegt oder ein Spieler steht im Aufbau.",
        TrainError::OutOfReach => "Montagestelle oder Fahrzeug ist zu weit entfernt.",
        TrainError::Coupled => "Vor der Demontage alle Kupplungen lösen.",
        TrainError::Cycle => "Dieser Verbund würde einen Kreis bilden.",
        TrainError::Busy => "Der Sitz oder das Fahrzeug wird gerade benutzt.",
        TrainError::RateLimited => "Bitte kurz warten.",
        TrainError::Limit | TrainError::Overflow => "Die Fahrzeuggrenze ist erreicht.",
        TrainError::Missing => "Montagestelle, Bauteil oder Fahrzeug ist nicht vorhanden.",
        TrainError::Invalid => "Dieser Aufbau oder diese Verbindung ist nicht möglich.",
    }
}
fn request(world: &mut World, state: &mut TrainUiState, action: TrainAction) {
    if state.pending.is_some() {
        return;
    }
    if let Some(request) = world
        .get_resource_mut::<ClientSession>()
        .and_then(|mut session| session.train_request(action))
    {
        state.pending = Some(request.0.request);
        state.status = "Wird ausgeführt…".into();
        world.write_message(request);
    }
}
fn close(world: &mut World, state: &mut TrainUiState) {
    state.open = false;
    state.pending = None;
    state.cargo = false;
    state.consumed_escape = true;
    if playing(world) && !other_panel(world) {
        world.write_message(terra_player::CaptureRequest(true));
    }
}

fn playing(world: &World) -> bool {
    world
        .get_resource::<crate::menu::MenuState>()
        .is_some_and(|menu| menu.page == crate::menu::MenuPage::Playing)
        && world
            .get_resource::<ClientSession>()
            .is_some_and(|session| {
                session.ready && session.error.is_none() && !session.dead() && !session.teleporting
            })
}
fn other_panel(world: &World) -> bool {
    world
        .get_resource::<crate::inventory::InventoryState>()
        .is_some_and(|panel| panel.blocks_input())
        || world
            .get_resource::<crate::chat::ChatState>()
            .is_some_and(|panel| panel.blocks_input())
        || world
            .get_resource::<crate::pets::PetMenuState>()
            .is_some_and(|panel| panel.blocks_input())
        || world
            .get_resource::<crate::kingdom::KingdomMenuState>()
            .is_some_and(|panel| panel.blocks_input())
        || world
            .get_resource::<crate::minimap::fullmap::FullMapState>()
            .is_some_and(|panel| panel.blocks_input())
        || world
            .get_resource::<crate::SettingsView>()
            .is_some_and(|panel| panel.open)
}
fn page_capacity(world: &mut World) -> usize {
    let height = world
        .query::<&Window>()
        .iter(world)
        .next()
        .map_or(1080.0, |window| window.height());
    let scale = world
        .get_resource::<UiScale>()
        .map_or(1.0, |scale| scale.0.max(0.1));
    if height / scale < 600.0 { 3 } else { 6 }
}

fn command(world: &mut World, state: &mut TrainUiState, command: &str) {
    if command == "close" {
        close(world, state);
        return;
    }
    if state.pending.is_some() {
        return;
    }
    if let Some(kind) = command.strip_prefix("kind:") {
        state.kind = match kind {
            "tender" => VehicleKind::Tender,
            "passenger" => VehicleKind::Passenger,
            "freight" => VehicleKind::Freight,
            _ => VehicleKind::Locomotive,
        };
        return;
    }
    if let Some(id) = command
        .strip_prefix("select:")
        .and_then(|id| id.parse::<u64>().ok())
    {
        if state.selected == Some(id) {
            state.second = None;
        } else if state.selected.is_none() {
            state.selected = Some(id);
        } else {
            state.second = Some(id);
        }
        return;
    }
    match command {
        "clear" => {
            state.selected = None;
            state.second = None;
            return;
        }
        "front" => {
            state.front = !state.front;
            return;
        }
        "second-front" => {
            state.second_front = !state.second_front;
            return;
        }
        "previous" => {
            state.page = state.page.saturating_sub(1);
            return;
        }
        "next" => {
            state.page = state.page.saturating_add(1);
            return;
        }
        "workshop" => {
            state.cargo = false;
            return;
        }
        _ => {}
    }
    let Some(session) = world.get_resource::<ClientSession>() else {
        return;
    };
    let registry_revision = session.trains.revision;
    let vehicle = state
        .selected
        .and_then(|id| {
            session
                .trains
                .vehicles
                .iter()
                .find(|vehicle| vehicle.id == id)
        })
        .cloned();
    if let Some(slot) = command
        .strip_prefix("slot:")
        .and_then(|slot| slot.parse::<u8>().ok())
    {
        let Some(cargo) = session.train_cargo.clone().filter(|_| state.cargo) else {
            return;
        };
        let player_revision = session.inventory.revision;
        let reserved = crate::inventory::reserved_for_transport(world);
        request(
            world,
            state,
            TrainAction::Cargo {
                vehicle: cargo.vehicle,
                part: cargo.part,
                revision: cargo.revision,
                player_revision,
                gesture: ContainerGesture::QuickMove { slot },
                reserved,
            },
        );
        return;
    }
    if command == "cargo" {
        if let Some(vehicle) = vehicle
            && let Some(part) = vehicle.cargo_indices().next()
        {
            request(
                world,
                state,
                TrainAction::OpenCargo {
                    vehicle: vehicle.id,
                    part,
                },
            );
        }
        return;
    }
    if command == "seat" {
        if let Some(vehicle) = vehicle
            && let Some(part) = vehicle
                .parts
                .iter()
                .position(|part| matches!(part.material, 214 | 215))
        {
            request(
                world,
                state,
                TrainAction::Seat {
                    vehicle: vehicle.id,
                    part: part as u16,
                },
            );
        }
        return;
    }
    let Some(station) = state.station else { return };
    match command {
        "assemble" => {
            let start = world
                .get_resource::<terra_world::streaming::ChunkStore>()
                .and_then(|chunks| terra_world::trains::candidate_start(chunks, station));
            if let Some(start) = start {
                request(
                    world,
                    state,
                    TrainAction::Assemble {
                        station,
                        start,
                        kind: state.kind,
                        revision: registry_revision,
                    },
                );
            } else {
                state.status = "Keine verbundenen Zugbauteile neben der Montagestelle.".into();
            }
        }
        "dismantle" => {
            if let Some(vehicle) = vehicle {
                request(
                    world,
                    state,
                    TrainAction::Dismantle {
                        station,
                        vehicle: vehicle.id,
                        revision: vehicle.revision,
                    },
                );
            }
        }
        "couple" => {
            if let (Some(first), Some(second)) = (state.selected, state.second) {
                request(
                    world,
                    state,
                    TrainAction::Couple {
                        station,
                        first,
                        first_end: if state.front {
                            CouplerEnd::Front
                        } else {
                            CouplerEnd::Rear
                        },
                        second,
                        second_end: if state.second_front {
                            CouplerEnd::Front
                        } else {
                            CouplerEnd::Rear
                        },
                        revision: registry_revision,
                    },
                );
            }
        }
        "uncouple" => {
            if let Some(vehicle) = vehicle {
                request(
                    world,
                    state,
                    TrainAction::Uncouple {
                        station,
                        vehicle: vehicle.id,
                        end: if state.front {
                            CouplerEnd::Front
                        } else {
                            CouplerEnd::Rear
                        },
                        revision: registry_revision,
                    },
                );
            }
        }
        _ => {}
    }
}

fn slot(
    index: u8,
    stack: Option<ItemStack>,
    palette: Option<&terra_player::interaction::BlockPalette>,
) -> TrainSlot {
    TrainSlot {
        index,
        command: format!("slot:{index}"),
        empty: stack.is_none(),
        name: stack.map_or_else(String::new, |stack| {
            palette.map_or_else(
                || stack.item.identifier(),
                |palette| crate::inventory::item_label(stack.item, palette),
            )
        }),
        count: stack.map_or_else(String::new, |stack| stack.count.to_string()),
        preview: stack.map_or_else(String::new, |stack| {
            crate::inventory::preview_key(stack.item)
        }),
    }
}
fn view(world: &mut World, state: &mut TrainUiState) -> TrainView {
    if !state.open {
        return TrainView::default();
    }
    let capacity = page_capacity(world);
    let Some(session) = world.get_resource::<ClientSession>() else {
        return TrainView::default();
    };
    let mut vehicles: Vec<_> = session
        .trains
        .vehicles
        .iter()
        .filter(|vehicle| {
            state.station.is_none_or(|station| {
                (0..3).all(|axis| vehicle.anchor[axis].abs_diff(station[axis]) <= 64)
            })
        })
        .collect();
    vehicles.sort_by_key(|vehicle| vehicle.id);
    let pages = vehicles.len().div_ceil(capacity).max(1);
    state.page = state.page.min(pages - 1);
    let page = state.page;
    let rows = vehicles
        .into_iter()
        .skip(page * capacity)
        .take(capacity)
        .map(|vehicle| TrainRow {
            id: vehicle.id,
            command: format!("select:{}", vehicle.id),
            label: format!("{} #{}", kind_label(vehicle.kind), vehicle.id),
            detail: format!(
                "{} kg · {} Bauteile · {} Sitze",
                vehicle.mass_kg,
                vehicle.parts.len(),
                vehicle
                    .parts
                    .iter()
                    .filter(|part| matches!(part.material, 214 | 215))
                    .count()
            ),
            selected: state.selected == Some(vehicle.id),
            second: state.second == Some(vehicle.id),
            foreign: Some(vehicle.owner) != session.uuid,
        })
        .collect();
    let cargo = session.train_cargo.as_ref().filter(|_| state.cargo);
    let palette = world.get_resource::<terra_player::interaction::BlockPalette>();
    TrainView {
        open: true,
        cargo: cargo.is_some(),
        pending: state.pending.is_some(),
        title: if cargo.is_some() {
            "Wagen-Laderaum".into()
        } else {
            "Zug-Montagestelle".into()
        },
        status: state.status.clone(),
        requirements: "Fahrgestell · 2 Drehgestelle auf Gleisen · Rahmen · Kupplung".into(),
        kind: kind_label(state.kind).into(),
        selected: state.selected.is_some(),
        pair: state.selected.is_some() && state.second.is_some(),
        front: if state.front { "Vorne" } else { "Hinten" }.into(),
        second_front: if state.second_front {
            "Vorne"
        } else {
            "Hinten"
        }
        .into(),
        page: format!("{} / {}", page + 1, pages),
        rows,
        player_slots: if cargo.is_some() {
            (0..27)
                .map(|index| slot(index, session.inventory.slot(usize::from(index)), palette))
                .collect()
        } else {
            Vec::new()
        },
        cargo_slots: cargo.map_or_else(Vec::new, |cargo| {
            (0..18)
                .map(|index| {
                    slot(
                        index + 27,
                        cargo.inventory.slot(usize::from(index)),
                        palette,
                    )
                })
                .collect()
        }),
    }
}

fn update(world: &mut World, mut cursor: Local<MessageCursor<TrainUiAction>>) {
    let actions = world
        .get_resource::<Messages<TrainUiAction>>()
        .map(|messages| cursor.read(messages).cloned().collect::<Vec<_>>())
        .unwrap_or_default();
    world.resource_scope(|world, mut state: Mut<TrainUiState>| {
        state.consumed_escape = false;
        let session = world.get_resource::<ClientSession>();
        let identity = session.map(|session| (session.connection, session.uuid));
        let ready = playing(world);
        if !ready || state.identity.is_some() && state.identity != identity {
            if state.open {
                close(world, &mut state);
            }
            state.station = None;
            state.selected = None;
            state.second = None;
            state.identity = identity;
        }
        if state.open && other_panel(world) {
            close(world, &mut state);
        }
        if let Some(id) = state.pending
            && let Some(result) = world
                .get_resource::<ClientSession>()
                .and_then(|session| session.last_train_result.clone())
            && result.request == id
        {
            state.pending = None;
            match result.outcome {
                TrainOutcome::Rejected(error) => {
                    state.status = rejection(error).into();
                    if error == TrainError::Stale
                        && state.cargo
                        && let Some(cargo) = world
                            .get_resource::<ClientSession>()
                            .and_then(|session| session.train_cargo.clone())
                    {
                        request(
                            world,
                            &mut state,
                            TrainAction::OpenCargo {
                                vehicle: cargo.vehicle,
                                part: cargo.part,
                            },
                        );
                    }
                }
                TrainOutcome::Cargo(_) => {
                    state.cargo = true;
                    state.status = "Klicke einen Stapel, um ihn zu verschieben.".into();
                }
                TrainOutcome::Changed { vehicle } => {
                    state.selected = vehicle.or(state.selected);
                    state.status = "Fertig.".into();
                }
                TrainOutcome::Seated(_) => close(world, &mut state),
                TrainOutcome::Inspected => state.status = String::new(),
            }
        }
        if let Some(session) = world.get_resource::<ClientSession>() {
            if state.selected.is_some_and(|id| {
                !session
                    .trains
                    .vehicles
                    .iter()
                    .any(|vehicle| vehicle.id == id)
            }) {
                state.selected = None;
            }
            if state.second.is_some_and(|id| {
                !session
                    .trains
                    .vehicles
                    .iter()
                    .any(|vehicle| vehicle.id == id)
            }) {
                state.second = None;
            }
            if state.cargo
                && session.train_cargo.as_ref().is_none_or(|cargo| {
                    !session
                        .trains
                        .vehicles
                        .iter()
                        .any(|vehicle| vehicle.id == cargo.vehicle)
                })
            {
                if state.station.is_none() {
                    close(world, &mut state);
                } else {
                    state.cargo = false;
                }
            }
        }
        let close_key = state.open
            && (pressed(world, GameAction::Pause) || pressed(world, GameAction::Inventory));
        if close_key {
            close(world, &mut state);
        }
        if ready
            && !state.open
            && !other_panel(world)
            && world
                .get_resource::<ClientSession>()
                .is_some_and(|session| session.train_seat.is_some())
            && pressed(world, GameAction::FlyDown)
        {
            request(world, &mut state, TrainAction::Leave);
        }
        if !state.open
            && !state.consumed_escape
            && ready
            && !shift(world)
            && world
                .get_resource::<terra_player::CameraInputState>()
                .is_some_and(|camera| camera.capture_ready())
            && !other_panel(world)
            && (pressed(world, GameAction::Place) || pressed(world, GameAction::Use))
        {
            let held = world
                .get_resource::<terra_player::interaction::ActiveItem>()
                .and_then(|active| active.0);
            let rail = held
                .and_then(terra_core::ItemKind::block_material)
                .is_some_and(terra_core::rails::is_rail);
            if !rail && held != Some(terra_core::ItemKind::Tool(terra_core::ToolKind::Wrench)) {
                let station = world.get_resource::<terra_player::TargetWorld>().and_then(
                    |target| match target.0 {
                        Some(terra_player::WorldTarget::Block(hit))
                            if hit.distance <= 4.5
                                && world
                                    .get_resource::<terra_world::streaming::ChunkStore>()
                                    .is_some_and(|chunks| {
                                        chunks.block_at(hit.position) == Some(217)
                                    }) =>
                        {
                            Some(hit.position)
                        }
                        _ => None,
                    },
                );
                let target = world
                    .get_resource::<terra_player::TargetTrain>()
                    .and_then(|target| target.0);
                if let Some(station) = station {
                    state.open = true;
                    state.station = Some(station);
                    state.cargo = false;
                    state.identity = identity;
                    request(world, &mut state, TrainAction::Inspect { station });
                } else if let Some(hit) = target {
                    let material = world
                        .get_resource::<ClientSession>()
                        .and_then(|session| {
                            session
                                .trains
                                .vehicles
                                .iter()
                                .find(|vehicle| vehicle.id == hit.vehicle)
                        })
                        .and_then(|vehicle| vehicle.parts.get(usize::from(hit.part)))
                        .map(|part| part.material);
                    if material == Some(216) {
                        state.open = true;
                        state.station = None;
                        state.identity = identity;
                        request(
                            world,
                            &mut state,
                            TrainAction::OpenCargo {
                                vehicle: hit.vehicle,
                                part: hit.part,
                            },
                        );
                    } else if matches!(material, Some(214 | 215)) {
                        state.identity = identity;
                        request(
                            world,
                            &mut state,
                            TrainAction::Seat {
                                vehicle: hit.vehicle,
                                part: hit.part,
                            },
                        );
                    }
                }
                if state.open {
                    world.write_message(terra_player::CaptureRequest(false));
                }
            }
        }
        if state.open {
            for action in actions {
                if !state.open {
                    break;
                }
                let TrainUiAction::Command(action) = action;
                command(world, &mut state, &action);
            }
            if let Some(mut block) = world.get_resource_mut::<terra_player::PlayerInputBlock>() {
                block.0 = true;
            }
        }
        let next = view(world, &mut state);
        if *world.resource::<TrainView>() != next {
            *world.resource_mut::<TrainView>() = next;
        }
    });
}
