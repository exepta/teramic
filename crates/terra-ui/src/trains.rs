//! Workshop controls and private cargo gestures for assembled railway vehicles.
use bevy::{ecs::message::MessageCursor, prelude::*};
use serde::Serialize;
use terra_core::{
    GameAction, ItemStack,
    containers::ContainerGesture,
    train_blueprint::{self, TrainBlueprint},
    trains::{CouplerEnd, TrainError, VehicleKind},
};
use terra_network::{
    client::ClientSession,
    protocol::trains::{TrainAction, TrainOutcome},
};
use tilt_ui::html_shared;

#[path = "trains_driving.rs"]
mod driving;

#[cfg(test)]
#[path = "trains_driving_tests.rs"]
mod driving_tests;
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
    wheelbase: u8,
    edit_after_leave: Option<([i64; 3], u64)>,
    editing: bool,
    drive_queue: Option<(u64, i8, bool)>,
    drive_desired: Option<(u64, i8, bool)>,
}
impl TrainUiState {
    pub(crate) fn blocks_input(&self) -> bool {
        self.open || self.consumed_escape
    }
    fn wheelbase(&self) -> u8 {
        if self.wheelbase == 0 {
            train_blueprint::DEFAULT_WHEELBASE
        } else {
            self.wheelbase
        }
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
    pub enabled: bool,
}

#[derive(Clone, Serialize, PartialEq)]
pub struct TrainThrottleStep {
    pub command: String,
    pub label: String,
    pub selected: bool,
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
    pub blueprint: bool,
    pub blueprint_progress: String,
    pub blueprint_target: String,
    pub selected: bool,
    pub pair: bool,
    pub front: String,
    pub second_front: String,
    pub page: String,
    pub rows: Vec<TrainRow>,
    pub player_slots: Vec<TrainSlot>,
    pub cargo_slots: Vec<TrainSlot>,
    pub wheelbase: String,
    pub riding: bool,
    pub driver: bool,
    pub drive_controls: bool,
    pub can_edit: bool,
    pub engine: bool,
    pub has_cargo: bool,
    pub fuel_mode: bool,
    pub brake: bool,
    pub drive_speed: String,
    pub drive_direction: String,
    pub drive_throttle: String,
    pub drive_value: f32,
    pub drive_status: String,
    pub drive_fuel: String,
    pub drive_hint: String,
    pub drive_levels: Vec<TrainThrottleStep>,
}

pub(crate) fn install(app: &mut App) {
    app.init_resource::<TrainUiState>()
        .init_resource::<TrainView>()
        .init_resource::<TrainBlueprint>()
        .add_message::<TrainUiAction>()
        .add_message::<terra_network::client::TrainRequest>()
        .add_systems(
            Update,
            update
                .before(crate::menu::MenuInputSet)
                .before(terra_player::PlayerSet::Movement),
        )
        .add_systems(PostUpdate, fit_panel.before(bevy::ui::UiSystems::Prepare));
}

/// Fit the whole workshop, including its labels, inside the available viewport.
fn fit_panel(
    windows: Query<&Window>,
    scale: Option<Res<UiScale>>,
    view: Res<TrainView>,
    mut panels: Query<(&tilt_ui::ElementId, &ComputedNode, &mut UiTransform)>,
) {
    if !view.open {
        return;
    }
    let Some(window) = windows.iter().next() else {
        return;
    };
    let scale = scale.as_deref().map_or(1.0, |scale| scale.0.max(0.1));
    for (id, node, mut transform) in &mut panels {
        if id.0 != "train-workshop-panel" {
            continue;
        }
        let size = node.size() * node.inverse_scale_factor();
        let fit = panel_scale(window.size() / scale, size);
        if transform.scale != Vec2::splat(fit) {
            transform.scale = Vec2::splat(fit)
        }
    }
}

fn panel_scale(viewport: Vec2, size: Vec2) -> f32 {
    if size.min_element() <= 0.0 {
        return 1.0;
    }
    ((viewport.x * 0.94 / size.x).min(viewport.y * 0.92 / size.y)).clamp(0.1, 1.0)
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
        TrainError::AttachedProps => {
            "Entferne zuerst Pflanzen oder andere aufgesetzte Props vom Aufbau."
        }
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
    // Closing the panel must not abandon a driver's in-flight acknowledgement:
    // the last dragged notch still needs to reach the server before its preview clears.
    if state.drive_desired.is_none() {
        state.pending = None;
    }
    state.cargo = false;
    state.edit_after_leave = None;
    state.editing = false;
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

/// Search only the bounded station neighborhood, and validate both wheelsets
/// against the same complete rails that the assembly authority accepts.
fn blueprint(
    world: &World,
    station: [i64; 3],
    kind: VehicleKind,
    wheelbase: u8,
) -> Option<TrainBlueprint> {
    let chunks = world.get_resource::<terra_world::streaming::ChunkStore>()?;
    let journal = world.get_resource::<terra_world::edits::EditJournal>()?;
    let previous = world.get_resource::<TrainBlueprint>();
    if let Some(previous) = previous.filter(|guide| guide.station == Some(station))
        && let Some(chassis) = previous.cells.iter().find(|part| part.material == 210)
        && train_blueprint::template_with_wheelbase(chassis.cell, chassis.facing, kind, wheelbase)
            .iter()
            .filter(|part| part.material == 211)
            .all(|part| terra_world::trains::bogie_supported(chunks, journal, part.cell))
    {
        return Some(TrainBlueprint {
            station: Some(station),
            kind,
            cells: train_blueprint::template_with_wheelbase(
                chassis.cell,
                chassis.facing,
                kind,
                wheelbase,
            ),
        });
    }
    let mut candidates = Vec::new();
    let mut complete = std::collections::BTreeMap::new();
    for x in -8_i64..=8 {
        for y in -4_i64..=4 {
            for z in -8_i64..=8 {
                let cell = [station[0] + x, station[1] + y, station[2] + z];
                if !chunks
                    .block_at(cell)
                    .is_some_and(terra_core::rails::is_rail)
                {
                    continue;
                }
                let Some(piece) = terra_world::rails::owner_at(journal, cell) else {
                    continue;
                };
                if !*complete
                    .entry(piece.anchor)
                    .or_insert_with(|| terra_world::rails::complete(journal, piece))
                {
                    continue;
                }
                for path in piece.active_paths() {
                    let start = path.sample(0.0).position;
                    let end = path.sample(1.0).position;
                    let delta = [end[0] - start[0], end[2] - start[2]];
                    let axis = usize::from(delta[0].abs() <= delta[1].abs()) * 2;
                    let perpendicular = 2 - axis;
                    if (end[1] - start[1]).abs() > 0.01
                        || (end[perpendicular] - start[perpendicular]).abs() > 0.01
                        || (start[perpendicular] - (cell[perpendicular] as f64 + 0.5)).abs() > 0.05
                        || [0.25, 0.5, 0.75].into_iter().any(|t| {
                            let point = path.sample(t).position;
                            (point[perpendicular] - start[perpendicular]).abs() > 0.01
                                || (point[1] - start[1]).abs() > 0.01
                        })
                    {
                        continue;
                    }
                    let facing = if axis == 2 {
                        if end[2] >= start[2] { 0 } else { 2 }
                    } else if end[0] >= start[0] {
                        1
                    } else {
                        3
                    };
                    for facing in [facing, (facing + 2) % 4] {
                        let cells = train_blueprint::template_with_wheelbase(
                            [cell[0], cell[1] + 2, cell[2]],
                            facing,
                            kind,
                            wheelbase,
                        );
                        if cells
                            .iter()
                            .filter(|part| part.material == 211)
                            .all(|part| {
                                terra_world::trains::bogie_supported(chunks, journal, part.cell)
                            })
                        {
                            candidates.push((x * x + y * y + z * z, facing, cells));
                        }
                    }
                }
            }
        }
    }
    candidates.sort_by_key(|(distance, facing, cells)| (*distance, *facing, cells[0].cell));
    candidates
        .into_iter()
        .next()
        .map(|(_, _, cells)| TrainBlueprint {
            station: Some(station),
            kind,
            cells,
        })
}

fn show_blueprint(world: &mut World, state: &mut TrainUiState) {
    let Some(station) = state.station else { return };
    if let Some(guide) = blueprint(world, station, state.kind, state.wheelbase()) {
        *world.resource_mut::<TrainBlueprint>() = guide;
        state.status =
            "Vorlage auf dem Gleis: Grün fehlt, Rot ist belegt. Mit „Mit Vorlage bauen“ schließen."
                .into();
    } else {
        *world.resource_mut::<TrainBlueprint>() = TrainBlueprint::default();
        state.status = "Kein passendes gerades Gleis neben der Montagestelle. Gleis verlängern und „Vorlage anzeigen“ wählen.".into();
    }
}

fn command(world: &mut World, state: &mut TrainUiState, command: &str) {
    if command == "close" {
        close(world, state);
        return;
    }
    if matches!(command, "wheelbase-dec" | "wheelbase-inc") {
        state.wheelbase = if command == "wheelbase-dec" {
            state
                .wheelbase()
                .saturating_sub(1)
                .max(train_blueprint::MIN_WHEELBASE)
        } else {
            state
                .wheelbase()
                .saturating_add(1)
                .min(train_blueprint::MAX_WHEELBASE)
        };
        show_blueprint(world, state);
        return;
    }
    if let Some(kind) = command.strip_prefix("kind:") {
        state.kind = match kind {
            "tender" => VehicleKind::Tender,
            "passenger" => VehicleKind::Passenger,
            "freight" => VehicleKind::Freight,
            _ => VehicleKind::Locomotive,
        };
        show_blueprint(world, state);
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
        "blueprint" => {
            show_blueprint(world, state);
            return;
        }
        "blueprint-build" => {
            if !world.resource::<TrainBlueprint>().cells.is_empty() {
                close(world, state);
            }
            return;
        }
        "blueprint-clear" => {
            *world.resource_mut::<TrainBlueprint>() = TrainBlueprint::default();
            state.status = "Vorlage ausgeblendet.".into();
            return;
        }
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
    if driving::command(world, state, command) {
        return;
    }
    if state.pending.is_some() {
        return;
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
        let engine = session
            .trains
            .vehicles
            .iter()
            .find(|vehicle| vehicle.id == cargo.vehicle)
            .and_then(|vehicle| vehicle.parts.get(usize::from(cargo.part)))
            .is_some_and(|part| part.material == 218);
        if engine
            && slot < terra_core::INVENTORY_SLOTS as u8
            && session
                .inventory
                .slot(usize::from(slot))
                .is_none_or(|stack| terra_core::furnace::fuel_seconds(stack.item).is_none())
        {
            state.status = "Der Motor nimmt nur Brennstoff an.".into();
            return;
        }
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
            && let Some(part) = vehicle
                .parts
                .iter()
                .position(|part| matches!(part.material, 29 | 216))
        {
            request(
                world,
                state,
                TrainAction::OpenCargo {
                    vehicle: vehicle.id,
                    part: part as u16,
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
            driving::edit(world, state);
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
        enabled: stack.is_some(),
    }
}

fn guide_view(world: &mut World) -> (bool, String, String) {
    let camera = world
        .query_filtered::<(&Transform, &terra_player::CameraLook), With<terra_player::TerraCamera>>(
        )
        .iter(world)
        .next()
        .map(|(camera, look)| {
            (
                camera.translation.to_array().map(f64::from),
                (look.rotation() * Vec3::NEG_Z).to_array().map(f64::from),
            )
        });
    let guide = world.resource::<TrainBlueprint>();
    if guide.cells.is_empty() {
        return (
            false,
            "Fahrzeug wählen, um eine Bauvorlage auf dem Gleis anzuzeigen.".into(),
            String::new(),
        );
    }
    let matches = |part: &train_blueprint::BlueprintCell| {
        world
            .get_resource::<terra_world::streaming::ChunkStore>()
            .is_some_and(|chunks| {
                let facing = world
                    .get_resource::<terra_world::edits::EditJournal>()
                    .and_then(|journal| journal.object_state(part.cell))
                    .map_or(0, |state| state.facing);
                train_blueprint::matches(*part, chunks.block_at(part.cell), facing)
            })
    };
    let placed = guide.cells.iter().filter(|part| matches(part)).count();
    let reach = world
        .get_resource::<terra_player::TargetWorld>()
        .and_then(|target| target.0)
        .map_or(8.0, |target| match target {
            terra_player::WorldTarget::Block(hit) => hit.distance,
            terra_player::WorldTarget::Prop(hit) => hit.distance,
        })
        .min(8.0);
    let target = camera
        .and_then(|(origin, direction)| {
            train_blueprint::target(&guide.cells, origin, direction, reach)
        })
        .filter(|part| !matches(part));
    let label = target.map_or_else(
        || "Platzhalter anvisieren: benötigtes Bauteil anzeigen".into(),
        |part| {
            let name = match part.material {
                210 => "Zug-Fahrgestell",
                211 => "Drehgestell",
                212 => "Zugrahmen",
                213 => "Zugkupplung",
                214 => "Führerstand",
                215 => "Zugsitz",
                216 => "Wagen-Laderaum",
                218 => "Brennstoffmotor",
                _ => "Zugbauteil",
            };
            format!("Hier: {name} · längs zum Gleis ausrichten")
        },
    );
    (
        true,
        format!(
            "Vorlage: {placed} / {} Bauteile · Grün fehlt · Rot belegt",
            guide.cells.len()
        ),
        label,
    )
}

fn view(world: &mut World, state: &mut TrainUiState) -> TrainView {
    let (blueprint, blueprint_progress, blueprint_target) = guide_view(world);
    if !state.open {
        let mut view = TrainView {
            blueprint,
            blueprint_progress,
            blueprint_target,
            ..default()
        };
        if let Some(session) = world.get_resource::<ClientSession>() {
            driving::populate(world, session, state, &mut view);
        }
        return view;
    }
    let capacity = page_capacity(world).min(if state.selected.is_some() { 4 } else { 6 });
    let Some(session) = world.get_resource::<ClientSession>() else {
        return TrainView::default();
    };
    let mut vehicles: Vec<_> = session
        .trains
        .vehicles
        .iter()
        .filter(|vehicle| {
            state.station.is_none_or(|station| {
                let position = vehicle.model_origin();
                (0..3).all(|axis| (position[axis] - station[axis] as f64).abs() <= 64.0)
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
                "{} kg · {} Bauteile · {} Sitze · {:.1} Blöcke/s",
                vehicle.mass_kg,
                vehicle.parts.len(),
                vehicle
                    .parts
                    .iter()
                    .filter(|part| matches!(part.material, 214 | 215))
                    .count(),
                vehicle.motion.speed_mps.abs(),
            ),
            selected: state.selected == Some(vehicle.id),
            second: state.second == Some(vehicle.id),
            foreign: Some(vehicle.owner) != session.uuid,
        })
        .collect();
    let cargo = session.train_cargo.as_ref().filter(|_| state.cargo);
    let palette = world.get_resource::<terra_player::interaction::BlockPalette>();
    let mut view = TrainView {
        open: true,
        cargo: cargo.is_some(),
        pending: state.pending.is_some(),
        title: if cargo.is_some() {
            "Wagen-Laderaum".into()
        } else {
            "Zug-Montagestelle".into()
        },
        status: state.status.clone(),
        requirements:
            "Fahrgestell · 2 Drehgestelle (3–15 m Abstand) · verbundener Rahmen · Kupplung".into(),
        kind: kind_label(state.kind).into(),
        blueprint,
        blueprint_progress,
        blueprint_target,
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
        wheelbase: format!("{} m", state.wheelbase()),
        ..default()
    };
    driving::populate(world, session, state, &mut view);
    view
}

fn update(
    world: &mut World,
    mut cursor: Local<MessageCursor<TrainUiAction>>,
    mut input: Local<Option<bevy::ecs::system::SystemState<terra_core::GameplayInput>>>,
) {
    let input = driving::Inputs::read(world, &mut input);
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
            state.pending = None;
            state.drive_queue = None;
            state.drive_desired = None;
            *world.resource_mut::<TrainBlueprint>() = TrainBlueprint::default();
        }
        if state.open && other_panel(world) {
            close(world, &mut state);
        }
        if world.resource::<TrainBlueprint>().station.is_some_and(|station| {
            world.get_resource::<terra_world::streaming::ChunkStore>()
                .is_none_or(|chunks| chunks.block_at(station) != Some(217))
        }) {
            *world.resource_mut::<TrainBlueprint>() = TrainBlueprint::default();
        }
        if let Some(id) = state.pending
            && let Some(result) = world
                .get_resource::<ClientSession>()
                .and_then(|session| session.last_train_result.clone())
            && result.request == id
        {
            state.pending = None;
            driving::acknowledged(&mut state);
            match result.outcome {
                TrainOutcome::Rejected(error) => {
                    let edit_alignment = state.editing
                        && state.edit_after_leave.is_none()
                        && matches!(error, TrainError::Invalid | TrainError::Busy);
                    state.drive_queue = None;
                    state.drive_desired = None;
                    state.edit_after_leave = None;
                    state.editing = false;
                    state.status = if edit_alignment {
                        "Für die Montage müssen alle Mitfahrer aussteigen und der Zug auf einem geraden Gleisabschnitt stehen."
                    } else { rejection(error) }.into();
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
                TrainOutcome::Cargo(cargo) => {
                    state.cargo = true;
                    state.selected = Some(cargo.vehicle);
                    state.status = "Klicke einen Stapel, um ihn zu verschieben.".into();
                }
                TrainOutcome::Changed { vehicle } => {
                    if state.editing {
                        state.selected = None;
                        state.editing = false;
                        state.status = "Montage aktiv: Verziere den Zug mit Blöcken und setze ihn anschließend wieder zusammen.".into();
                    } else {
                        state.selected = vehicle.or(state.selected);
                        state.status = "Fertig.".into();
                    }
                    if vehicle.is_some() {
                        *world.resource_mut::<TrainBlueprint>() = TrainBlueprint::default();
                    }
                }
                TrainOutcome::Seated(binding) => {
                    if binding.is_none() && state.edit_after_leave.is_some() {
                        driving::resume_edit(world, &mut state);
                    } else {
                        close(world, &mut state);
                    }
                }
                TrainOutcome::Inspected => {
                    if world.resource::<TrainBlueprint>().cells.is_empty() {
                        state.status = "Fahrzeug für eine Bauvorlage wählen oder vorhandenen Aufbau zusammensetzen.".into();
                    }
                }
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
        if ready {
            driving::flush(world, &mut state);
        }
        let close_key = state.open
            && input.close;
        if close_key {
            close(world, &mut state);
        }
        if ready
            && !state.open
            && !other_panel(world)
            && world
                .get_resource::<ClientSession>()
                .is_some_and(|session| session.train_seat.is_some())
            && input.leave
        {
            request(world, &mut state, TrainAction::Leave);
        }
        if ready && !state.open && !state.consumed_escape && !other_panel(world) {
            driving::keyboard(world, &mut state, &input);
        }
        if !state.open
            && !state.consumed_escape
            && ready
            && !shift(world)
            && world
                .get_resource::<terra_player::CameraInputState>()
                .is_some_and(|camera| camera.capture_ready())
            && !other_panel(world)
            && input.interact
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
                    if matches!(material, Some(29 | 216 | 218)) {
                        state.open = true;
                        state.station = None;
                        state.selected = Some(hit.vehicle);
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
