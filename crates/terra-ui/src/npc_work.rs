//! Draft work assignments are local; inventory, movement and work remain authoritative.
use bevy::prelude::*;
use serde::Serialize;
use std::collections::VecDeque;
use terra_core::npc_jobs::{WaitReason, WorkArea, WorkKind, WorkStatus};
use terra_network::{
    client::{ClientSession, NpcWorkRequest},
    protocol::npc_work::{WorkAction, WorkError, WorkerView},
};
use tilt_ui::html_shared;
mod direct;

#[derive(Message, Clone, Debug)]
pub enum NpcWorkUiAction {
    Close,
    PickArea,
    PickStorage,
    Select(u64),
    Field { field: String, value: String },
    Refresh,
    ToggleAreas,
    Assign,
    Enabled(bool),
}

#[derive(Resource, Default)]
pub(crate) struct WorkUiState {
    selected: Option<u64>,
    loaded: Option<WorkerDraftKey>,
    draft: WorkDraft,
    pending: Option<(u64, f64)>,
    pending_build: bool,
    was_visible: bool,
    last_poll: f64,
    error: String,
    feedback: String,
    inspections: VecDeque<(u64, f64)>,
    inspection_error: String,
    direct: bool,
    selection: Option<direct::Selection>,
    selection_armed: bool,
    resume_pending: bool,
    pub consumed_escape: bool,
    pub(crate) external_selection: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct WorkerDraftKey {
    npc: u64,
    order: Option<(u64, u64)>,
    craftsman: bool,
}

impl WorkUiState {
    pub(crate) fn blocks_input(&self) -> bool {
        self.direct || self.selection_active() || self.resume_pending
    }
    pub(crate) fn selection_active(&self) -> bool {
        self.selection.is_some() || self.external_selection
    }
}

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NpcWorkInputSet;

/// The server still authorizes the private InspectNpc request. This only opens
/// the existing dialog for an already authorized roster entry in planning mode.
pub(crate) fn open_managed(world: &mut World, npc: u64) {
    if !world
        .get_resource::<ClientSession>()
        .is_some_and(|session| session.work.workers.iter().any(|worker| worker.npc == npc))
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
    world.write_message(terra_player::CaptureRequest(false));
}

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct NpcWorkBlockSet;

struct WorkDraft {
    kind: WorkKind,
    craftsman: bool,
    min: [String; 3],
    max: [String; 3],
    storage: [String; 3],
    priority: String,
}

impl Default for WorkDraft {
    fn default() -> Self {
        Self {
            kind: WorkKind::Miner,
            craftsman: false,
            min: Default::default(),
            max: Default::default(),
            storage: Default::default(),
            priority: "5".into(),
        }
    }
}

impl WorkDraft {
    fn load(&mut self, worker: &WorkerView) {
        self.craftsman = false;
        if let Some(order) = &worker.order {
            self.kind = order.kind;
            self.min = order.area.min.map(|value| value.to_string());
            self.max = order.area.max.map(|value| value.to_string());
            self.storage = order.storage.map(|value| value.to_string());
            self.priority = order.priority.to_string();
        } else {
            self.kind = WorkKind::Miner;
            let center = worker.position.map(|value| value.floor() as i64);
            self.min = [center[0] - 4, center[1] - 1, center[2] - 4].map(|value| value.to_string());
            self.max = [center[0] + 4, center[1] + 1, center[2] + 4].map(|value| value.to_string());
            self.storage = Default::default();
            self.priority = "5".into();
        }
    }

    fn field(&mut self, field: &str, value: String) {
        if value.len() > 24 {
            return;
        }
        match field {
            "kind" => {
                if value == "craftsman" {
                    self.craftsman = true;
                    return;
                }
                if let Some(kind) = match value.as_str() {
                    "miner" => Some(WorkKind::Miner),
                    "farmer" => Some(WorkKind::Farmer),
                    "collector" => Some(WorkKind::Collector),
                    "lumberjack" => Some(WorkKind::Lumberjack),
                    _ => None,
                } {
                    self.kind = kind;
                    self.craftsman = false;
                }
            }
            "priority" => self.priority = value,
            _ => {
                let Some((set, axis)) = field.rsplit_once('-') else {
                    return;
                };
                let axis = match axis {
                    "x" => 0,
                    "y" => 1,
                    "z" => 2,
                    _ => return,
                };
                match set {
                    "min" => self.min[axis] = value,
                    "max" => self.max[axis] = value,
                    "storage" => self.storage[axis] = value,
                    _ => {}
                }
            }
        }
    }

    fn command(&self, npc: u64) -> Result<WorkAction, &'static str> {
        let coordinates = |values: &[String; 3]| -> Result<[i64; 3], &'static str> {
            let mut parsed = [0_i64; 3];
            for (index, value) in values.iter().enumerate() {
                parsed[index] = value
                    .trim()
                    .parse()
                    .map_err(|_| "Ganze Koordinaten eingeben")?;
                if parsed[index].unsigned_abs() > 999_000 {
                    return Err("Koordinaten liegen außerhalb der Welt");
                }
            }
            Ok(parsed)
        };
        let area = WorkArea::new(coordinates(&self.min)?, coordinates(&self.max)?)
            .map_err(|_| "Arbeitsgebiet maximal 32 × 16 × 32 Blöcke")?;
        let storage = coordinates(&self.storage)?;
        let priority = self
            .priority
            .parse::<u8>()
            .ok()
            .filter(|value| *value <= 9)
            .ok_or("Priorität muss zwischen 0 und 9 liegen")?;
        Ok(WorkAction::Assign {
            npc,
            kind: self.kind,
            area,
            storage,
            priority,
        })
    }
}

#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct WorkWorkerRow {
    pub id: String,
    pub name: String,
    pub profession: String,
    pub status: String,
    pub carried: String,
    pub selected: bool,
}

#[html_shared]
#[derive(Resource, Default, Debug, Serialize, PartialEq)]
pub struct NpcWorkView {
    pub visible: bool,
    pub direct: bool,
    pub selection_active: bool,
    pub selection_hint: String,
    pub selection_error: String,
    pub inventory_ready: bool,
    pub private_inventory: Vec<direct::InventorySlot>,
    pub pending: bool,
    pub empty: bool,
    pub workers: Vec<WorkWorkerRow>,
    pub worker_count: String,
    pub selected: bool,
    pub name: String,
    pub location: String,
    pub kind: String,
    pub craftsman: bool,
    pub priority: String,
    pub min_x: String,
    pub min_y: String,
    pub min_z: String,
    pub max_x: String,
    pub max_y: String,
    pub max_z: String,
    pub storage_x: String,
    pub storage_y: String,
    pub storage_z: String,
    pub area_size: String,
    pub status: String,
    pub carried: String,
    pub enabled: bool,
    pub has_order: bool,
    pub can_show_areas: bool,
    pub areas_shown: bool,
    pub can_assign: bool,
    pub validation: String,
    pub error: String,
    pub feedback: String,
}

pub(crate) fn install(app: &mut App) {
    app.init_resource::<WorkUiState>()
        .init_resource::<terra_core::work_areas::WorkAreaDisplay>()
        .init_resource::<NpcWorkView>()
        .add_message::<NpcWorkUiAction>()
        .add_message::<NpcWorkRequest>()
        .add_systems(
            Update,
            update
                .after(crate::kingdom::update)
                .before(terra_player::PlayerSet::Movement),
        );
    direct::install(app);
}

fn profession(kind: WorkKind) -> &'static str {
    match kind {
        WorkKind::Miner => "Bergarbeiter",
        WorkKind::Farmer => "Landwirt",
        WorkKind::Collector => "Sammler",
        WorkKind::Lumberjack => "Holzfäller",
    }
}

fn status(status: WorkStatus) -> &'static str {
    match status {
        WorkStatus::Idle => "Bereit",
        WorkStatus::Travelling => "Unterwegs",
        WorkStatus::Working => "Bei der Arbeit",
        WorkStatus::Delivering => "Liefert ab",
        WorkStatus::Paused => "Pausiert",
        WorkStatus::Waiting(reason) => match reason {
            WaitReason::MissingTool => "Werkzeug fehlt",
            WaitReason::MissingMaterial => "Material fehlt",
            WaitReason::StorageFull => "Lager voll",
            WaitReason::NoPath => "Weg versperrt",
            WaitReason::Unloaded => "Bereich nicht geladen",
            WaitReason::Protected => "Zugriff verweigert",
            WaitReason::NoTargets => "Keine Ziele",
            WaitReason::Night => "Nachtruhe",
            WaitReason::NoPlantingSite => "Kein geeigneter Pflanzplatz",
            WaitReason::MissingStation => "Holzfäller-Station benötigt",
        },
    }
}

fn work_error(error: WorkError) -> &'static str {
    match error {
        WorkError::Invalid => "Ungültiger Arbeitsauftrag",
        WorkError::Protected => "Keine Berechtigung für Arbeitskraft, Gebiet oder Lager",
        WorkError::Missing => "Arbeitskraft oder Auftrag nicht mehr vorhanden",
        WorkError::Busy => "Arbeitskraft muss zuerst ihren laufenden Auftrag abschließen",
        WorkError::Stale => "Auftrag wurde verändert. Neu laden und erneut versuchen",
        WorkError::Unloaded => "Arbeitsgebiet oder Lager ist nicht geladen",
        WorkError::Storage => "An dieser Position befindet sich kein verfügbares Lager",
        WorkError::Limit => "Auftragslimit erreicht",
    }
}

fn send(
    action: WorkAction,
    session: &mut ClientSession,
    state: &mut WorkUiState,
    requests: &mut MessageWriter<NpcWorkRequest>,
    now: f64,
) {
    if let Some(request) = session.make_work_request(action) {
        state.pending_build = false;
        state.pending = Some((request.0.request, now));
        state.error.clear();
        state.feedback = "Warte auf Server …".into();
        requests.write(request);
    } else {
        state.error = "Anfrage konnte nicht gesendet werden".into();
    }
}

fn refresh(
    session: &mut ClientSession,
    state: &mut WorkUiState,
    requests: &mut MessageWriter<NpcWorkRequest>,
    now: f64,
) {
    let action = state
        .selected
        .map(|npc| WorkAction::InspectNpc { npc })
        .unwrap_or(WorkAction::Inspect);
    if let Some(request) = session.make_work_request(action) {
        if state.selected.is_some() {
            // Track private inspection replies independently of mutating orders:
            // polling must neither disable editing nor hide permission failures.
            if state.inspections.len() >= 5 {
                state.inspections.pop_front();
            }
            state.inspections.push_back((request.0.request, now));
        }
        requests.write(request);
    }
    state.last_poll = now;
}

fn load_selected(
    state: &mut WorkUiState,
    workers: &[WorkerView],
    buildings: &terra_network::protocol::building_jobs::BuildingSnapshot,
) {
    if let Some(worker) = workers
        .iter()
        .find(|worker| Some(worker.npc) == state.selected)
    {
        let key = WorkerDraftKey {
            npc: worker.npc,
            order: worker
                .order
                .as_ref()
                .map(|order| (order.id, order.revision)),
            craftsman: buildings.is_craftsman(worker.npc),
        };
        if state.loaded != Some(key) {
            state.draft.load(worker);
            state.draft.craftsman = buildings.is_craftsman(worker.npc);
            state.loaded = Some(key);
        }
    }
}

#[allow(clippy::too_many_arguments)] // Bevy injects independent session, palette, view and message resources.
fn update(
    session: Option<ResMut<ClientSession>>,
    palette: Option<Res<terra_player::interaction::BlockPalette>>,
    kingdom: Res<crate::KingdomView>,
    time: Option<Res<Time<Real>>>,
    mut events: MessageReader<NpcWorkUiAction>,
    mut requests: MessageWriter<NpcWorkRequest>,
    mut building_requests: MessageWriter<terra_network::client::BuildingRequest>,
    mut state: ResMut<WorkUiState>,
    mut view: ResMut<NpcWorkView>,
    mut area_display: Option<ResMut<terra_core::work_areas::WorkAreaDisplay>>,
) {
    let Some(mut session) = session else { return };
    if !session.ready || session.error.is_some() {
        *state = WorkUiState::default();
        *view = NpcWorkView::default();
        events.clear();
        return;
    }
    let now = time.as_ref().map_or(0.0, |time| time.elapsed_secs_f64());
    if let Some(result) = session.last_work_result.as_ref()
        && let Some(index) = state
            .inspections
            .iter()
            .position(|(request, _)| *request == result.request)
    {
        let _ = state.inspections.drain(..=index);
        state.inspection_error = result
            .error
            .as_ref()
            .map_or_else(String::new, |error| work_error(error.clone()).into());
    }
    if state
        .inspections
        .front()
        .is_some_and(|(_, started)| now - started > 8.0)
    {
        state.inspections.clear();
        state.inspection_error = "Keine Serverantwort. Auftrag neu laden".into();
    }
    if let Some((request, started)) = state.pending {
        let result = if state.pending_build {
            session
                .last_building_result
                .as_ref()
                .filter(|result| result.request == request)
                .map(|result| {
                    result
                        .error
                        .map(|error| crate::building_jobs::error_label(error).to_owned())
                })
        } else {
            session
                .last_work_result
                .as_ref()
                .filter(|result| result.request == request)
                .map(|result| {
                    result
                        .error
                        .as_ref()
                        .map(|error| work_error(error.clone()).to_owned())
                })
        };
        if let Some(error) = result {
            state.pending = None;
            if let Some(error) = error {
                state.error = error;
                state.feedback.clear();
            } else {
                state.feedback = "Auftrag bestätigt".into();
            }
        } else if now - started > 8.0 {
            state.pending = None;
            state.error = "Keine Serverantwort. Auftrag neu laden".into();
            state.feedback.clear();
        }
    }
    let visible = state.direct || kingdom.visible && kingdom.bound && kingdom.work_tab;
    if !visible {
        state.was_visible = false;
        events.clear();
        let next = direct::selection_view(&state);
        if *view != next {
            *view = next;
        }
        return;
    }
    if state.pending.is_none() && (!state.was_visible || now - state.last_poll >= 2.0) {
        refresh(&mut session, &mut state, &mut requests, now);
        if let Some(kingdom) = crate::building_jobs::active_kingdom(&session)
            && let Some(request) = session.make_building_request(
                terra_network::protocol::building_jobs::BuildingAction::Inspect { kingdom },
            )
        {
            building_requests.write(request);
        }
    }
    state.was_visible = true;
    if !state.direct
        && state
            .selected
            .is_none_or(|npc| !session.work.workers.iter().any(|worker| worker.npc == npc))
    {
        state.selected = session.work.workers.first().map(|worker| worker.npc);
        state.loaded = None;
    }
    load_selected(&mut state, &session.work.workers, &session.buildings);
    for action in events.read() {
        if state.pending.is_some() {
            continue;
        }
        match action {
            NpcWorkUiAction::ToggleAreas => {
                if let Some(npc) = state.selected
                    && let Some(display) = &mut area_display
                    && terra_network::client::work_areas::has_saved_areas(
                        &session,
                        terra_core::work_areas::WorkAreaTarget::Worker(npc),
                    )
                {
                    display.toggle(terra_core::work_areas::WorkAreaTarget::Worker(npc));
                }
            }
            NpcWorkUiAction::Close => {
                state.direct = false;
                state.selection = None;
                state.resume_pending = true;
                state.inspections.clear();
                state.inspection_error.clear();
            }
            NpcWorkUiAction::PickArea if state.selected.is_some() => {
                state.direct = false;
                state.selection = Some(direct::Selection::Area { first: None });
                state.selection_armed = false;
                state.error.clear();
            }
            NpcWorkUiAction::PickStorage if state.selected.is_some() => {
                state.direct = false;
                state.selection = Some(direct::Selection::Storage);
                state.selection_armed = false;
                state.error.clear();
            }
            NpcWorkUiAction::Select(npc)
                if session.work.workers.iter().any(|worker| worker.npc == *npc) =>
            {
                state.selected = Some(*npc);
                state.loaded = None;
                state.error.clear();
                state.feedback.clear();
                load_selected(&mut state, &session.work.workers, &session.buildings);
                refresh(&mut session, &mut state, &mut requests, now);
            }
            NpcWorkUiAction::Field { field, value } => {
                state.draft.field(field, value.clone());
                state.error.clear();
                state.feedback.clear();
            }
            NpcWorkUiAction::Refresh => {
                state.loaded = None;
                refresh(&mut session, &mut state, &mut requests, now);
            }
            NpcWorkUiAction::Assign => {
                if let Some(npc) = state.selected {
                    if state.draft.craftsman {
                        if let Some(kingdom) = crate::building_jobs::active_kingdom(&session) {
                            if let Some(request) = session.make_building_request(terra_network::protocol::building_jobs::BuildingAction::AssignCraftsman { kingdom, npc, enabled: true }) {
                                state.pending = Some((request.0.request, now)); state.pending_build = true;
                                state.error.clear(); state.feedback = "Warte auf Server …".into();
                                building_requests.write(request);
                            }
                        } else {
                            state.error = "Handwerker benötigen ein eigenes Kingdom".into();
                        }
                        continue;
                    }
                    match state.draft.command(npc) {
                        Ok(action) => send(action, &mut session, &mut state, &mut requests, now),
                        Err(error) => state.error = error.into(),
                    }
                }
            }
            NpcWorkUiAction::Enabled(enabled) => {
                if state.draft.craftsman {
                    if let (Some(npc), Some(kingdom)) = (
                        state.selected,
                        crate::building_jobs::active_kingdom(&session),
                    ) && let Some(request) = session.make_building_request(
                        terra_network::protocol::building_jobs::BuildingAction::AssignCraftsman {
                            kingdom,
                            npc,
                            enabled: *enabled,
                        },
                    ) {
                        state.pending = Some((request.0.request, now));
                        state.pending_build = true;
                        state.error.clear();
                        building_requests.write(request);
                    }
                    continue;
                }
                if let Some(order) = session
                    .work
                    .workers
                    .iter()
                    .find(|worker| Some(worker.npc) == state.selected)
                    .and_then(|worker| worker.order.as_ref())
                    .filter(|order| order.enabled != *enabled)
                {
                    let action = WorkAction::SetEnabled {
                        id: order.id,
                        revision: order.revision,
                        enabled: *enabled,
                    };
                    send(action, &mut session, &mut state, &mut requests, now);
                }
            }
            _ => {}
        }
    }
    if !state.direct && !(kingdom.visible && kingdom.bound && kingdom.work_tab)
        || state.selection.is_some()
    {
        let next = direct::selection_view(&state);
        if *view != next {
            *view = next;
        }
        return;
    }
    let selected = session
        .work
        .workers
        .iter()
        .find(|worker| Some(worker.npc) == state.selected);
    load_selected(&mut state, &session.work.workers, &session.buildings);
    let draft = &state.draft;
    let validation = if draft.craftsman {
        if crate::building_jobs::active_kingdom(&session).is_some() {
            ""
        } else {
            "Handwerker benötigen ein eigenes Kingdom"
        }
    } else {
        state
            .selected
            .and_then(|npc| draft.command(npc).err())
            .unwrap_or("")
    };
    let area_size = state
        .selected
        .filter(|_| !draft.craftsman)
        .and_then(|npc| match draft.command(npc).ok()? {
            WorkAction::Assign { area, .. } => Some(format!(
                "{} × {} × {}",
                area.max[0] - area.min[0] + 1,
                area.max[1] - area.min[1] + 1,
                area.max[2] - area.min[2] + 1
            )),
            _ => None,
        })
        .unwrap_or_default();
    let next = NpcWorkView {
        visible,
        direct: state.direct,
        inventory_ready: session
            .work
            .detail
            .as_ref()
            .is_some_and(|detail| Some(detail.npc) == state.selected),
        private_inventory: direct::inventory_slots(&session, state.selected, palette.as_deref()),
        pending: state.pending.is_some(),
        empty: session.work.workers.is_empty(),
        workers: session
            .work
            .workers
            .iter()
            .map(|worker| WorkWorkerRow {
                id: worker.npc.to_string(),
                name: worker.name.clone(),
                profession: if session.buildings.is_craftsman(worker.npc) {
                    "Handwerker"
                } else {
                    worker
                        .order
                        .as_ref()
                        .map_or("Ohne Auftrag", |order| profession(order.kind))
                }
                .into(),
                status: status(worker.status).into(),
                carried: format!("{} / 9 Plätze", worker.carried_slots),
                selected: Some(worker.npc) == state.selected,
            })
            .collect(),
        worker_count: session.work.workers.len().to_string(),
        selected: selected.is_some(),
        name: selected.map_or_else(String::new, |worker| worker.name.clone()),
        location: selected.map_or_else(String::new, |worker| {
            format!(
                "{} / {} / {}",
                worker.position[0].floor(),
                worker.position[1].floor(),
                worker.position[2].floor()
            )
        }),
        craftsman: draft.craftsman,
        kind: if draft.craftsman {
            "craftsman"
        } else {
            match draft.kind {
                WorkKind::Miner => "miner",
                WorkKind::Farmer => "farmer",
                WorkKind::Collector => "collector",
                WorkKind::Lumberjack => "lumberjack",
            }
        }
        .into(),
        priority: draft.priority.clone(),
        min_x: draft.min[0].clone(),
        min_y: draft.min[1].clone(),
        min_z: draft.min[2].clone(),
        max_x: draft.max[0].clone(),
        max_y: draft.max[1].clone(),
        max_z: draft.max[2].clone(),
        storage_x: draft.storage[0].clone(),
        storage_y: draft.storage[1].clone(),
        storage_z: draft.storage[2].clone(),
        area_size,
        status: selected
            .map_or("", |worker| {
                if session.buildings.is_craftsman(worker.npc) {
                    session
                        .buildings
                        .jobs
                        .values()
                        .find(|job| job.active() && job.workers.contains(&worker.npc))
                        .map_or("Bereit", |job| {
                            crate::building_jobs::status_label(job.status)
                        })
                } else {
                    status(worker.status)
                }
            })
            .into(),
        carried: selected.map_or_else(String::new, |worker| {
            format!("Fracht: {} / 9 Inventarplätze", worker.carried_slots)
        }),
        enabled: selected.is_some_and(|worker| {
            if draft.craftsman {
                session.buildings.is_craftsman(worker.npc)
            } else {
                worker.order.as_ref().is_some_and(|order| order.enabled)
            }
        }),
        has_order: selected.is_some_and(|worker| {
            worker.order.is_some() || session.buildings.is_craftsman(worker.npc)
        }),
        can_show_areas: selected.is_some_and(|worker| {
            terra_network::client::work_areas::has_saved_areas(
                &session,
                terra_core::work_areas::WorkAreaTarget::Worker(worker.npc),
            )
        }),
        areas_shown: selected.is_some_and(|worker| {
            area_display.as_ref().is_some_and(|display| {
                display.target == Some(terra_core::work_areas::WorkAreaTarget::Worker(worker.npc))
            })
        }),
        can_assign: selected.is_some() && validation.is_empty() && state.pending.is_none(),
        validation: validation.into(),
        error: if state.error.is_empty() {
            state.inspection_error.clone()
        } else {
            state.error.clone()
        },
        feedback: state.feedback.clone(),
        ..Default::default()
    };
    if *view != next {
        *view = next;
    }
}

#[cfg(test)]
mod tests;
