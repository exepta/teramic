//! Draft work assignments are local; inventory, movement and work remain authoritative.
use bevy::prelude::*;
use serde::Serialize;
use terra_core::npc_jobs::{WaitReason, WorkArea, WorkKind, WorkStatus};
use terra_network::{
    client::{ClientSession, NpcWorkRequest},
    protocol::npc_work::{WorkAction, WorkError, WorkerView},
};
use tilt_ui::html_shared;

#[derive(Message, Clone, Debug)]
pub enum NpcWorkUiAction {
    Select(u64),
    Field { field: String, value: String },
    Refresh,
    Assign,
    Enabled(bool),
}

#[derive(Resource, Default)]
struct WorkUiState {
    selected: Option<u64>,
    loaded: Option<(u64, Option<(u64, u64)>)>,
    draft: WorkDraft,
    pending: Option<(u64, f64)>,
    was_visible: bool,
    last_poll: f64,
    error: String,
    feedback: String,
}

struct WorkDraft {
    kind: WorkKind,
    min: [String; 3],
    max: [String; 3],
    storage: [String; 3],
    priority: String,
}

impl Default for WorkDraft {
    fn default() -> Self {
        Self {
            kind: WorkKind::Miner,
            min: Default::default(),
            max: Default::default(),
            storage: Default::default(),
            priority: "5".into(),
        }
    }
}

impl WorkDraft {
    fn load(&mut self, worker: &WorkerView) {
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
                if let Some(kind) = match value.as_str() {
                    "miner" => Some(WorkKind::Miner),
                    "farmer" => Some(WorkKind::Farmer),
                    "collector" => Some(WorkKind::Collector),
                    "lumberjack" => Some(WorkKind::Lumberjack),
                    _ => None,
                } {
                    self.kind = kind;
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
    pub pending: bool,
    pub empty: bool,
    pub workers: Vec<WorkWorkerRow>,
    pub worker_count: String,
    pub selected: bool,
    pub name: String,
    pub location: String,
    pub kind: String,
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
    pub can_assign: bool,
    pub validation: String,
    pub error: String,
    pub feedback: String,
}

pub(crate) fn install(app: &mut App) {
    app.init_resource::<WorkUiState>()
        .init_resource::<NpcWorkView>()
        .add_message::<NpcWorkUiAction>()
        .add_message::<NpcWorkRequest>()
        .add_systems(Update, update.after(crate::kingdom::update));
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
        state.pending = Some((request.0.request, now));
        state.error.clear();
        state.feedback = "Warte auf Server …".into();
        requests.write(request);
    } else {
        state.error = "Anfrage konnte nicht gesendet werden".into();
    }
}

fn load_selected(state: &mut WorkUiState, workers: &[WorkerView]) {
    if let Some(worker) = workers
        .iter()
        .find(|worker| Some(worker.npc) == state.selected)
    {
        let key = (
            worker.npc,
            worker
                .order
                .as_ref()
                .map(|order| (order.id, order.revision)),
        );
        if state.loaded != Some(key) {
            state.draft.load(worker);
            state.loaded = Some(key);
        }
    }
}

fn update(
    session: Option<ResMut<ClientSession>>,
    kingdom: Res<crate::KingdomView>,
    time: Option<Res<Time<Real>>>,
    mut events: MessageReader<NpcWorkUiAction>,
    mut requests: MessageWriter<NpcWorkRequest>,
    mut state: ResMut<WorkUiState>,
    mut view: ResMut<NpcWorkView>,
) {
    let Some(mut session) = session else { return };
    if !session.ready || session.error.is_some() {
        *state = WorkUiState::default();
        *view = NpcWorkView::default();
        events.clear();
        return;
    }
    let now = time.as_ref().map_or(0.0, |time| time.elapsed_secs_f64());
    if let Some((request, started)) = state.pending {
        if let Some(result) = session
            .last_work_result
            .as_ref()
            .filter(|result| result.request == request)
        {
            state.pending = None;
            if let Some(error) = &result.error {
                state.error = work_error(error.clone()).into();
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
    let visible = kingdom.visible && kingdom.bound && kingdom.work_tab;
    if !visible {
        state.was_visible = false;
        events.clear();
        if view.visible {
            *view = NpcWorkView::default();
        }
        return;
    }
    if state.pending.is_none() && (!state.was_visible || now - state.last_poll >= 2.0) {
        if let Some(request) = session.make_work_request(WorkAction::Inspect) {
            requests.write(request);
        }
        state.last_poll = now;
    }
    state.was_visible = true;
    if state
        .selected
        .is_none_or(|npc| !session.work.workers.iter().any(|worker| worker.npc == npc))
    {
        state.selected = session.work.workers.first().map(|worker| worker.npc);
        state.loaded = None;
    }
    load_selected(&mut state, &session.work.workers);
    for action in events.read() {
        if state.pending.is_some() {
            continue;
        }
        match action {
            NpcWorkUiAction::Select(npc)
                if session.work.workers.iter().any(|worker| worker.npc == *npc) =>
            {
                state.selected = Some(*npc);
                state.loaded = None;
                state.error.clear();
                state.feedback.clear();
                load_selected(&mut state, &session.work.workers);
            }
            NpcWorkUiAction::Field { field, value } => {
                state.draft.field(field, value.clone());
                state.error.clear();
                state.feedback.clear();
            }
            NpcWorkUiAction::Refresh => {
                state.loaded = None;
                if let Some(request) = session.make_work_request(WorkAction::Inspect) {
                    requests.write(request);
                }
                state.last_poll = now;
            }
            NpcWorkUiAction::Assign => {
                if let Some(npc) = state.selected {
                    match state.draft.command(npc) {
                        Ok(action) => send(action, &mut session, &mut state, &mut requests, now),
                        Err(error) => state.error = error.into(),
                    }
                }
            }
            NpcWorkUiAction::Enabled(enabled) => {
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
    let selected = session
        .work
        .workers
        .iter()
        .find(|worker| Some(worker.npc) == state.selected);
    load_selected(&mut state, &session.work.workers);
    let draft = &state.draft;
    let validation = state
        .selected
        .and_then(|npc| draft.command(npc).err())
        .unwrap_or("");
    let area_size = state
        .selected
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
        pending: state.pending.is_some(),
        empty: session.work.workers.is_empty(),
        workers: session
            .work
            .workers
            .iter()
            .map(|worker| WorkWorkerRow {
                id: worker.npc.to_string(),
                name: worker.name.clone(),
                profession: worker
                    .order
                    .as_ref()
                    .map_or("Ohne Auftrag", |order| profession(order.kind))
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
        kind: match draft.kind {
            WorkKind::Miner => "miner",
            WorkKind::Farmer => "farmer",
            WorkKind::Collector => "collector",
            WorkKind::Lumberjack => "lumberjack",
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
        status: selected.map_or("", |worker| status(worker.status)).into(),
        carried: selected.map_or_else(String::new, |worker| {
            format!("Fracht: {} / 9 Inventarplätze", worker.carried_slots)
        }),
        enabled: selected
            .and_then(|worker| worker.order.as_ref())
            .is_some_and(|order| order.enabled),
        has_order: selected.is_some_and(|worker| worker.order.is_some()),
        can_assign: selected.is_some() && validation.is_empty() && state.pending.is_none(),
        validation: validation.into(),
        error: state.error.clone(),
        feedback: state.feedback.clone(),
    };
    if *view != next {
        *view = next;
    }
}

#[cfg(test)]
mod tests;
