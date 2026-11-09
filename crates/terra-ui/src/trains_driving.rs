//! Owner-authorized cab controls display server telemetry and never simulate fuel locally.
use super::*;
use bevy::ecs::system::SystemState;
use terra_core::{GameplayInput, train_motion::StopReason, trains::VehicleView};

#[derive(Default)]
pub(super) struct Inputs {
    pub close: bool,
    pub leave: bool,
    pub interact: bool,
    forward: bool,
    backward: bool,
    brake: bool,
}
impl Inputs {
    pub(super) fn read(world: &mut World, state: &mut Option<SystemState<GameplayInput>>) -> Self {
        if !world.contains_resource::<ButtonInput<KeyCode>>() {
            return Self::default();
        }
        let Ok(input) = state
            .get_or_insert_with(|| SystemState::new(world))
            .get(world)
        else {
            return Self::default();
        };
        let typing = world
            .get_resource::<bevy::input_focus::InputFocus>()
            .and_then(bevy::input_focus::InputFocus::get)
            .is_some_and(|entity| {
                world
                    .get::<tilt_ui::ElementId>(entity)
                    .is_some_and(|id| id.0 == "train-name")
                    || world
                        .get::<ChildOf>(entity)
                        .and_then(|parent| world.get::<tilt_ui::ElementId>(parent.parent()))
                        .is_some_and(|id| id.0 == "train-name")
            });
        Self {
            close: input.just_pressed(GameAction::Pause)
                || !typing && input.just_pressed(GameAction::Inventory),
            leave: input.just_pressed(GameAction::FlyDown)
                || input.just_pressed(GameAction::Dismount),
            interact: input.just_pressed(GameAction::Use) || input.just_pressed(GameAction::Place),
            forward: input.just_pressed(GameAction::Forward),
            backward: input.just_pressed(GameAction::Backward),
            brake: input.just_pressed(GameAction::Jump),
        }
    }
}

fn riding(session: &ClientSession) -> Option<&VehicleView> {
    let seat = session.train_seat?;
    session.trains.vehicles.iter().find(|vehicle| {
        vehicle.id == seat.vehicle
            && vehicle
                .parts
                .get(usize::from(seat.part))
                .is_some_and(|part| matches!(part.material, 214 | 215))
    })
}

fn is_driver(session: &ClientSession, vehicle: &VehicleView) -> bool {
    let Some(seat) = session.train_seat.filter(|seat| seat.vehicle == vehicle.id) else {
        return false;
    };
    Some(vehicle.owner) == session.uuid
        && vehicle
            .parts
            .get(usize::from(seat.part))
            .is_some_and(|part| part.material == 214)
}

/// Motion follows chassis +Z; the cab camera looks along its local -Z axis.
fn control_sign(session: &ClientSession, vehicle: &VehicleView) -> i8 {
    let part = session
        .train_seat
        .filter(|seat| seat.vehicle == vehicle.id)
        .map(|seat| usize::from(seat.part))
        .filter(|&index| {
            vehicle
                .parts
                .get(index)
                .is_some_and(|part| part.material == 214)
        })
        .or_else(|| vehicle.parts.iter().position(|part| part.material == 214));
    part.map_or(1, |part| vehicle.driver_direction(part))
}

pub(super) fn selected<'a>(
    session: &'a ClientSession,
    state: &TrainUiState,
) -> Option<&'a VehicleView> {
    state
        .selected
        .and_then(|id| {
            session
                .trains
                .vehicles
                .iter()
                .find(|vehicle| vehicle.id == id)
        })
        .or_else(|| riding(session))
}

fn driver_station(
    world: &World,
    session: &ClientSession,
    vehicle: &VehicleView,
) -> Option<[i64; 3]> {
    let seat = session
        .train_seat
        .filter(|seat| seat.vehicle == vehicle.id)?;
    let (position, _) = vehicle.seat_pose(usize::from(seat.part))?;
    let chunks = world.get_resource::<terra_world::streaming::ChunkStore>()?;
    let center = position.map(|coordinate| coordinate.floor() as i64);
    let mut nearest: Option<(f64, [i64; 3])> = None;
    for x in -4..=4 {
        for y in -4..=4 {
            for z in -4..=4 {
                let cell = [center[0] + x, center[1] + y, center[2] + z];
                if chunks.block_at(cell) != Some(217) {
                    continue;
                }
                let distance = (0..3)
                    .map(|axis| (cell[axis] as f64 + 0.5 - position[axis]).powi(2))
                    .sum::<f64>();
                if distance <= 4.5 * 4.5 && nearest.is_none_or(|(best, _)| distance < best) {
                    nearest = Some((distance, cell));
                }
            }
        }
    }
    nearest.map(|(_, cell)| cell)
}

pub(super) fn keyboard(world: &mut World, state: &mut TrainUiState, input: &Inputs) {
    if !world
        .get_resource::<terra_player::CameraInputState>()
        .is_some_and(|camera| camera.capture_ready())
    {
        return;
    }
    let Some(session) = world.get_resource::<ClientSession>() else {
        return;
    };
    let Some(vehicle) = riding(session)
        .filter(|vehicle| is_driver(session, vehicle))
        .cloned()
    else {
        return;
    };
    let sign = control_sign(session, &vehicle);
    if input.interact {
        state.open = true;
        state.cargo = false;
        state.selected = Some(vehicle.id);
        state.second = None;
        state.station = driver_station(world, session, &vehicle);
        state.identity = Some((session.connection, session.uuid));
        state.status =
            "Fahrhebel wählen, Motor befeuern oder an einer Montagestelle bearbeiten.".into();
        world.write_message(terra_player::CaptureRequest(false));
    } else {
        let step = i8::from(input.forward) - i8::from(input.backward);
        if step != 0 || input.brake {
            let (_, throttle, brake) = state
                .drive_desired
                .filter(|(id, _, _)| *id == vehicle.id)
                .unwrap_or((vehicle.id, vehicle.motion.throttle, vehicle.motion.brake));
            send_drive(
                world,
                state,
                vehicle.id,
                (throttle * sign + step).clamp(-3, 3) * sign,
                if input.brake { !brake } else { false },
            );
        }
    }
}

pub(super) fn command(world: &mut World, state: &mut TrainUiState, command: &str) -> bool {
    if matches!(command, "dismantle" | "edit") {
        if state.pending.is_none() {
            edit(world, state);
        }
        return true;
    }
    let throttle = command
        .strip_prefix("drive-throttle:")
        .and_then(|value| value.parse::<i8>().ok());
    if throttle.is_none()
        && !matches!(
            command,
            "drive-more" | "drive-less" | "drive-start" | "drive-stop" | "drive-brake" | "engine"
        )
    {
        return false;
    }
    let Some(session) = world.get_resource::<ClientSession>() else {
        return true;
    };
    let Some(vehicle) = selected(session, state).cloned() else {
        return true;
    };
    if command == "engine" {
        if state.pending.is_some() {
            return true;
        }
        if let Some(part) = vehicle.parts.iter().position(|part| part.material == 218) {
            state.selected = Some(vehicle.id);
            request(
                world,
                state,
                TrainAction::OpenCargo {
                    vehicle: vehicle.id,
                    part: part as u16,
                },
            );
        } else {
            state.status = "Baue einen Brennstoffmotor an den zusammenhängenden Aufbau.".into();
        }
        return true;
    }
    if !is_driver(session, &vehicle) {
        state.status = "Zum Fahren in den eigenen Führerstand einsteigen.".into();
        return true;
    }
    let sign = control_sign(session, &vehicle);
    let (_, desired_throttle, desired_brake) = state
        .drive_desired
        .filter(|(id, _, _)| *id == vehicle.id)
        .unwrap_or((vehicle.id, vehicle.motion.throttle, vehicle.motion.brake));
    let desired_throttle = desired_throttle * sign;
    let throttle = match command {
        "drive-start" => {
            if desired_throttle == 0 {
                1
            } else {
                desired_throttle
            }
        }
        "drive-more" => (desired_throttle + 1).min(3),
        "drive-less" => (desired_throttle - 1).max(-3),
        "drive-stop" => 0,
        _ => throttle.unwrap_or(desired_throttle),
    };
    if !(-3..=3).contains(&throttle) {
        return true;
    }
    send_drive(
        world,
        state,
        vehicle.id,
        throttle * sign,
        match command {
            "drive-stop" => true,
            "drive-brake" => !desired_brake,
            _ => false,
        },
    );
    true
}

fn send_drive(
    world: &mut World,
    state: &mut TrainUiState,
    vehicle: u64,
    throttle: i8,
    brake: bool,
) {
    state.drive_desired = Some((vehicle, throttle, brake));
    if state.pending.is_some() {
        state.drive_queue = state.drive_desired;
    } else {
        request(
            world,
            state,
            TrainAction::Drive {
                vehicle,
                throttle,
                brake,
            },
        );
    }
}

pub(super) fn acknowledged(state: &mut TrainUiState) {
    if state.drive_queue.is_none() {
        state.drive_desired = None;
    }
}

/// A drag can produce many notches while an acknowledgement is in flight.
/// Keep only the latest intent; the authoritative speed and fuel never change locally.
pub(super) fn flush(world: &mut World, state: &mut TrainUiState) {
    let valid = world
        .get_resource::<ClientSession>()
        .and_then(riding)
        .is_some_and(|vehicle| {
            world
                .get_resource::<ClientSession>()
                .is_some_and(|session| is_driver(session, vehicle))
        });
    if !valid {
        state.drive_queue = None;
        state.drive_desired = None;
    } else if state.pending.is_none()
        && let Some((vehicle, throttle, brake)) = state.drive_queue.take()
    {
        let same_driver = world
            .get_resource::<ClientSession>()
            .and_then(riding)
            .is_some_and(|current| current.id == vehicle);
        if same_driver {
            send_drive(world, state, vehicle, throttle, brake);
        }
    }
}

pub(super) fn edit(world: &mut World, state: &mut TrainUiState) {
    let Some(session) = world.get_resource::<ClientSession>() else {
        return;
    };
    let Some(vehicle) = selected(session, state).cloned() else {
        return;
    };
    if !vehicle.motion.stopped() {
        state.status = "Vor der Montage anhalten und die Bremse setzen.".into();
        return;
    }
    let station = if session
        .train_seat
        .is_some_and(|seat| seat.vehicle == vehicle.id)
    {
        driver_station(world, session, &vehicle)
    } else {
        state.station
    };
    let Some(station) = station else {
        state.status = "Zum Bearbeiten neben einer Montagestelle anhalten und aussteigen.".into();
        return;
    };
    if vehicle.couplings.iter().any(Option::is_some) {
        state.status = "Vor der Montage die Kupplungen lösen.".into();
        return;
    }
    state.station = Some(station);
    state.selected = Some(vehicle.id);
    state.editing = true;
    if session
        .train_seat
        .is_some_and(|seat| seat.vehicle == vehicle.id)
    {
        state.edit_after_leave = Some((station, vehicle.id));
        request(world, state, TrainAction::Leave);
    } else {
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

pub(super) fn resume_edit(world: &mut World, state: &mut TrainUiState) {
    let Some((station, id)) = state.edit_after_leave.take() else {
        return;
    };
    let vehicle = world
        .get_resource::<ClientSession>()
        .and_then(|session| {
            session
                .trains
                .vehicles
                .iter()
                .find(|vehicle| vehicle.id == id)
        })
        .cloned();
    let Some(vehicle) = vehicle else {
        state.editing = false;
        state.status = "Das Fahrzeug ist nicht mehr vorhanden.".into();
        return;
    };
    state.open = true;
    state.cargo = false;
    state.station = Some(station);
    request(
        world,
        state,
        TrainAction::Dismantle {
            station,
            vehicle: id,
            revision: vehicle.revision,
        },
    );
}

fn status(reason: StopReason) -> &'static str {
    match reason {
        StopReason::Ready => "Fahrbereit",
        StopReason::Braking => "Bremse aktiv",
        StopReason::NoFuel => "Motor benötigt Brennstoff",
        StopReason::NoEngine => "Brennstoffmotor fehlt",
        StopReason::Buffer => "Prellbock erreicht",
        StopReason::EndOfTrack => "Gleisende erreicht",
        StopReason::Unloaded => "Warte auf geladene Strecke",
        StopReason::Obstructed => "Strecke blockiert",
        StopReason::InvalidTrack => "Gleisverbindung prüfen",
    }
}

fn binding(world: &World, action: GameAction) -> String {
    world
        .get_resource::<terra_core::ControlsConfig>()
        .map_or_else(|| action.default_binding(), |config| config.binding(action))
        .label()
}

pub(super) fn populate(
    world: &World,
    session: &ClientSession,
    state: &TrainUiState,
    view: &mut TrainView,
) {
    let rider = riding(session);
    view.riding = rider.is_some() && playing(world);
    view.driver = rider.is_some_and(|vehicle| is_driver(session, vehicle));
    let vehicle = if state.open && state.cargo {
        session.train_cargo.as_ref().and_then(|cargo| {
            session
                .trains
                .vehicles
                .iter()
                .find(|vehicle| vehicle.id == cargo.vehicle)
        })
    } else if state.open {
        selected(session, state)
    } else {
        rider
    };
    let Some(vehicle) = vehicle else { return };
    let sign = control_sign(session, vehicle);
    view.selected = state.selected.is_some() || state.open && rider.is_some();
    view.drive_controls = is_driver(session, vehicle);
    view.engine = Some(vehicle.owner) == session.uuid
        && vehicle.parts.iter().any(|part| part.material == 218);
    view.has_cargo = Some(vehicle.owner) == session.uuid
        && vehicle
            .parts
            .iter()
            .any(|part| matches!(part.material, 29 | 216));
    view.cargo_hint = if view.has_cargo {
        String::new()
    } else if Some(vehicle.owner) != session.uuid {
        "Der Laderaum gehört dem Fahrzeugbesitzer.".into()
    } else {
        "Kein Laderaum eingebaut: In der Montage einen Wagen-Laderaum oder eine Truhe anbauen."
            .into()
    };
    let editable = state.open
        && !state.cargo
        && Some(vehicle.owner) == session.uuid
        && vehicle.motion.stopped()
        && vehicle.couplings.iter().all(Option::is_none);
    view.can_edit = editable
        && if session
            .train_seat
            .is_some_and(|seat| seat.vehicle == vehicle.id)
        {
            driver_station(world, session, vehicle).is_some()
        } else {
            state.station.is_some()
        };
    view.brake = vehicle.motion.brake;
    view.drive_speed = format!("{:.1} Blöcke/s", vehicle.motion.speed_mps.abs()).replace('.', ",");
    let logical_speed = vehicle.motion.speed_mps * f64::from(sign);
    view.drive_direction = if logical_speed > 0.001 {
        "Vorwärts"
    } else if logical_speed < -0.001 {
        "Rückwärts"
    } else {
        "Steht"
    }
    .into();
    let throttle = state
        .drive_desired
        .filter(|(id, _, _)| *id == vehicle.id)
        .map_or(vehicle.motion.throttle, |(_, throttle, _)| throttle)
        * sign;
    view.drive_throttle = match throttle {
        0 => "Leerlauf".into(),
        level if level < 0 => format!("Rückwärts {} / 3", level.unsigned_abs()),
        level => format!("Vorwärts {level} / 3"),
    };
    view.drive_value = f32::from(throttle);
    view.drive_status = status(vehicle.motion.stop_reason).into();
    view.drive_fuel = if !vehicle.parts.iter().any(|part| part.material == 218) {
        "Kein Motor eingebaut"
    } else if vehicle.motion.fuel_available {
        "Brennstoff vorhanden"
    } else {
        "Motor befeuern"
    }
    .into();
    view.drive_hint = if view.driver {
        format!(
            "{} / {}: Fahrhebel · {}: Bremse · {}: Bedienung · {}: Aussteigen",
            binding(world, GameAction::Forward),
            binding(world, GameAction::Backward),
            binding(world, GameAction::Jump),
            binding(world, GameAction::Use),
            binding(world, GameAction::FlyDown)
        )
    } else {
        format!(
            "Mitfahrer · {}: Aussteigen",
            binding(world, GameAction::FlyDown)
        )
    };
    view.drive_levels = (-3..=3)
        .map(|level| TrainThrottleStep {
            command: format!("drive-throttle:{level}"),
            label: if level > 0 {
                format!("+{level}")
            } else {
                level.to_string()
            },
            selected: throttle == level,
        })
        .collect();
    if let Some(cargo) = session.train_cargo.as_ref().filter(|_| state.cargo) {
        view.fuel_mode = session
            .trains
            .vehicles
            .iter()
            .find(|vehicle| vehicle.id == cargo.vehicle)
            .and_then(|vehicle| vehicle.parts.get(usize::from(cargo.part)))
            .is_some_and(|part| part.material == 218);
        if view.fuel_mode {
            view.title = "Motor · Befeuerung".into();
        }
        let held = session.inventory.cursor;
        for slot in &mut view.player_slots {
            slot.enabled = held.is_some()
                || session
                    .inventory
                    .slot(usize::from(slot.index))
                    .is_some_and(|stack| {
                        !view.fuel_mode || terra_core::furnace::fuel_seconds(stack.item).is_some()
                    });
        }
        for slot in &mut view.cargo_slots {
            slot.enabled = held.map_or(!slot.empty, |stack| {
                !view.fuel_mode || terra_core::furnace::fuel_seconds(stack.item).is_some()
            });
        }
    }
}
