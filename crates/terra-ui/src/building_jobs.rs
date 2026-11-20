//! Kingdom construction controls. Drafts stay local until a confirmed server command.
mod build_mode;
mod catalog;
mod selection;
mod stock;
pub use build_mode::{BuildModeUiAction, BuildModeView};
pub use catalog::TemplateRow as BuildingTemplateRow;
pub use stock::KingdomResourcesView;
#[cfg(test)]
mod catalog_tests;
#[cfg(test)]
mod tests;

use bevy::prelude::*;
use serde::Serialize;
use terra_core::{
    building_jobs::{BuildArea, BuildError, BuildPhase, BuildStatus, BuildWait},
    kingdom::KingdomId,
};
use terra_network::{
    client::{BuildingRequest, ClientSession},
    protocol::building_jobs::BuildingAction,
};
use tilt_ui::html_shared;

#[derive(Message, Clone, Debug)]
pub enum BuildingUiAction {
    Refresh,
    ToggleAreas,
    PickStorage,
    PickResidential,
    PickHouse,
    SaveStorage,
    SaveResidential,
    DisableResidential,
    QueueHouse,
    Template(String),
    Category(String),
    Facing(String),
    Craftsman {
        npc: u64,
        enabled: bool,
    },
    Enabled {
        id: u64,
        revision: u64,
        enabled: bool,
    },
    Cancel {
        id: u64,
        revision: u64,
    },
}

#[derive(Resource, Default)]
pub(crate) struct BuildingUiState {
    kingdom: Option<KingdomId>,
    keep: Option<[i64; 3]>,
    draft_kingdom: Option<KingdomId>,
    storage: Option<BuildArea>,
    residential: Option<BuildArea>,
    origin: Option<[i64; 3]>,
    facing: u8,
    template: String,
    template_edited: bool,
    category: String,
    pending: Option<(u64, f64)>,
    inspect: Option<(u64, f64)>,
    last_poll: f64,
    was_visible: bool,
    error: String,
    feedback: String,
    selection: Option<selection::Selection>,
    armed: bool,
    returning: bool,
    returned_at: f64,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct BuildingWorkerRow {
    pub id: String,
    pub name: String,
    pub status: String,
    pub registered: bool,
    pub can_assign: bool,
}
#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct BuildingMaterialRow {
    pub identifier: String,
    pub label: String,
    pub count: String,
    pub preview_key: String,
    pub supply: String,
    pub sufficient: bool,
}
#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct BuildingJobRow {
    pub id: String,
    pub revision: String,
    pub template_name: String,
    pub location: String,
    pub status: String,
    pub progress: String,
    pub workers: String,
    pub enabled: bool,
    pub active: bool,
    pub materials: Vec<BuildingMaterialRow>,
    pub has_materials: bool,
}

#[html_shared]
#[derive(Resource, Default, Clone, Debug, Serialize, PartialEq)]
pub struct BuildingView {
    pub visible: bool,
    pub can_manage: bool,
    pub pending: bool,
    pub selection_active: bool,
    pub selection_hint: String,
    pub selection_error: String,
    pub storage: String,
    pub storage_draft: String,
    pub residential: String,
    pub residential_draft: String,
    pub residential_enabled: bool,
    pub origin: String,
    pub facing: String,
    pub can_save_storage: bool,
    pub can_save_residential: bool,
    pub can_queue: bool,
    pub can_show_areas: bool,
    pub areas_shown: bool,
    pub templates: Vec<BuildingTemplateRow>,
    pub categories: Vec<build_mode::CategoryRow>,
    pub template_selected: bool,
    pub template_name: String,
    pub template_id: String,
    pub template_category: String,
    pub template_dimensions: String,
    pub template_steps: String,
    pub template_preview: String,
    pub template_materials: Vec<BuildingMaterialRow>,
    pub template_error: String,
    pub automatic_template: String,
    pub workers: Vec<BuildingWorkerRow>,
    pub worker_count: String,
    pub jobs: Vec<BuildingJobRow>,
    pub empty: bool,
    pub error: String,
    pub feedback: String,
}

pub(crate) fn install(app: &mut App) {
    app.init_resource::<BuildingUiState>()
        .init_resource::<terra_core::work_areas::WorkAreaDisplay>()
        .init_resource::<BuildingView>()
        .add_message::<BuildingUiAction>()
        .add_message::<BuildingRequest>()
        .add_systems(
            Update,
            update
                .after(crate::kingdom::update)
                .before(terra_player::PlayerSet::Movement),
        );
    selection::install(app);
    stock::install(app);
    build_mode::install(app);
}

pub(crate) fn error_label(error: BuildError) -> &'static str {
    match error {
        BuildError::InvalidArea => "Baubereich maximal 64 × 32 × 64 Blöcke",
        BuildError::InvalidSite => "Ungeeigneter Hausstandort",
        BuildError::MissingKingdom => "Kingdom nicht gefunden",
        BuildError::Protected => "Keine Berechtigung für diesen Bauauftrag",
        BuildError::MissingWorker => "Handwerker nicht verfügbar",
        BuildError::Busy => "Handwerker hat bereits einen Auftrag",
        BuildError::Limit => "Baulimit erreicht",
        BuildError::Stale => "Bauauftrag wurde geändert. Bitte neu laden",
        BuildError::Unloaded => "Baubereich ist nicht geladen",
        BuildError::Blocked => "Hausstandort ist belegt",
        BuildError::MissingStorage => "Zuerst das Stadtlager festlegen",
        BuildError::MissingJob => "Bauauftrag nicht gefunden",
        BuildError::InvalidTemplate => "Ungültige Hausvorlage",
    }
}

pub(crate) fn status_label(status: BuildStatus) -> &'static str {
    match status {
        BuildStatus::Queued => "Eingeplant",
        BuildStatus::Travelling => "Unterwegs zur Baustelle",
        BuildStatus::Building(phase) => match phase {
            BuildPhase::Foundation => "Fundament bauen",
            BuildPhase::Floor => "Boden verlegen",
            BuildPhase::Walls => "Wände aufbauen",
            BuildPhase::Roof => "Dach bauen",
            BuildPhase::Furnishing => "Haus einrichten",
            BuildPhase::Cleanup => "Baugerüst entfernen",
        },
        BuildStatus::Waiting(reason) => match reason {
            BuildWait::NoWorkers => "Keine freien Handwerker",
            BuildWait::MissingStorage => "Stadtlager benötigt",
            BuildWait::MissingMaterials => "Baumaterial fehlt",
            BuildWait::StorageFull => "Stadtlager voll",
            BuildWait::Unloaded => "Baustelle nicht geladen",
            BuildWait::Protected => "Zugriff auf Baustelle verweigert",
            BuildWait::NoPath => "Baustelle nicht erreichbar",
            BuildWait::Blocked => "Baustelle blockiert",
        },
        BuildStatus::Paused => "Pausiert",
        BuildStatus::Complete => "Fertiggestellt",
        BuildStatus::Cancelled => "Abgebrochen",
    }
}

fn area_label(area: Option<BuildArea>) -> String {
    area.map_or_else(
        || "Nicht festgelegt".into(),
        |area| {
            format!(
                "{} / {} / {} → {} / {} / {}",
                area.min[0], area.min[1], area.min[2], area.max[0], area.max[1], area.max[2]
            )
        },
    )
}

pub(crate) fn active_kingdom(session: &ClientSession) -> Option<KingdomId> {
    let uuid = session.uuid?;
    session
        .confirmed_state()
        .and_then(|body| {
            session.kingdoms.territorial_keep(
                uuid,
                body.position.map(|value| f64::from(value).floor() as i64),
            )
        })
        .map(|kingdom| kingdom.id)
        .or_else(|| {
            session
                .kingdoms
                .active_for_member(uuid)
                .map(|kingdom| kingdom.id)
        })
}

fn send(
    world: &mut World,
    state: &mut BuildingUiState,
    action: BuildingAction,
    now: f64,
    inspect: bool,
) {
    let request = world
        .resource_mut::<ClientSession>()
        .make_building_request(action);
    if let Some(request) = request {
        let pending = (request.0.request, now);
        if inspect {
            state.inspect = Some(pending);
        } else {
            state.pending = Some(pending);
            state.error.clear();
            state.feedback = "Warte auf Server …".into();
        }
        world.write_message(request);
    } else {
        state.error = "Anfrage konnte nicht gesendet werden".into();
    }
}

fn update(world: &mut World) {
    let Some(session) = world.get_resource::<ClientSession>() else {
        return;
    };
    if !session.ready || session.error.is_some() {
        *world.resource_mut::<BuildingUiState>() = Default::default();
        *world.resource_mut::<BuildingView>() = Default::default();
        world
            .resource_mut::<crate::npc_work::WorkUiState>()
            .external_selection = false;
        world.resource_mut::<Messages<BuildingUiAction>>().clear();
        return;
    }
    let now = world
        .get_resource::<Time<Real>>()
        .map_or(0.0, |time| time.elapsed_secs_f64());
    let events: Vec<_> = world
        .resource_mut::<Messages<BuildingUiAction>>()
        .drain()
        .collect();
    world.resource_scope(|world, mut state: Mut<BuildingUiState>| {
        if let Some(result) = world
            .resource::<ClientSession>()
            .last_building_result
            .as_ref()
        {
            if state
                .pending
                .is_some_and(|(request, _)| result.request == request)
            {
                state.pending = None;
                if let Some(error) = result.error {
                    state.error = error_label(error).into();
                    state.feedback.clear();
                } else {
                    state.error.clear();
                    state.feedback = "Bauauftrag bestätigt".into();
                }
            }
            if state
                .inspect
                .is_some_and(|(request, _)| result.request == request)
            {
                state.inspect = None;
                if let Some(error) = result.error {
                    state.error = error_label(error).into();
                }
            }
        }
        if state
            .pending
            .is_some_and(|(_, started)| now - started > 8.0)
        {
            state.pending = None;
            state.error = "Keine Serverantwort. Bitte neu laden".into();
            state.feedback.clear();
        }
        let kingdom_view = world.resource::<crate::KingdomView>();
        let visible = kingdom_view.visible && kingdom_view.bound && kingdom_view.building_tab;
        let manages = kingdom_view.manager_controls;
        let displayed = KingdomId(kingdom_view.settlement_id);
        if visible {
            state.kingdom = Some(displayed);
            state.keep = world
                .resource::<ClientSession>()
                .kingdoms
                .get(displayed)
                .map(|kingdom| kingdom.location);
            if state.draft_kingdom != Some(displayed) {
                let settlement = world
                    .resource::<ClientSession>()
                    .buildings
                    .settlements
                    .get(&displayed);
                state.storage = settlement.and_then(|settlement| settlement.storage);
                state.residential = settlement.and_then(|settlement| settlement.residential);
                state.origin = None;
                state.template_edited = false;
                state.category = "*".into();
                state.template = settlement.map_or_else(
                    || terra_core::building_jobs::HOUSE_TEMPLATE.into(),
                    |site| site.selected_template.clone(),
                );
                state.draft_kingdom = Some(displayed);
                state.error.clear();
                state.feedback.clear();
            }
            if (!state.was_visible || now - state.last_poll >= 2.0) && state.pending.is_none() {
                send(
                    world,
                    &mut state,
                    BuildingAction::Inspect { kingdom: displayed },
                    now,
                    true,
                );
                if let Some(request) = world
                    .resource_mut::<ClientSession>()
                    .make_work_request(terra_network::protocol::npc_work::WorkAction::Inspect)
                {
                    world.write_message(request);
                }
                state.last_poll = now;
            }
        }
        state.was_visible = visible;
        if visible
            && !state.template_edited
            && let Some(site) = world
                .resource::<ClientSession>()
                .buildings
                .settlements
                .get(&displayed)
        {
            state.template.clone_from(&site.selected_template);
        }
        for action in events {
            if !visible || state.pending.is_some() {
                continue;
            }
            if matches!(action, BuildingUiAction::ToggleAreas) {
                let target = terra_core::work_areas::WorkAreaTarget::Settlement(displayed);
                if terra_network::client::work_areas::has_saved_areas(
                    world.resource::<ClientSession>(),
                    target,
                ) {
                    world
                        .resource_mut::<terra_core::work_areas::WorkAreaDisplay>()
                        .toggle(target);
                }
                continue;
            }
            if matches!(action, BuildingUiAction::Refresh) {
                send(
                    world,
                    &mut state,
                    BuildingAction::ReloadCatalog { kingdom: displayed },
                    now,
                    true,
                );
                state.last_poll = now;
                continue;
            }
            if let BuildingUiAction::Template(id) = &action {
                if world
                    .resource::<ClientSession>()
                    .buildings
                    .templates
                    .iter()
                    .any(|template| template.id == *id)
                {
                    state.template.clone_from(id);
                    state.template_edited = true;
                    state.error.clear();
                }
                continue;
            }
            if let BuildingUiAction::Category(path) = &action {
                if path == "*"
                    || world
                        .resource::<ClientSession>()
                        .buildings
                        .templates
                        .iter()
                        .any(|template| &template.category_path == path)
                {
                    state.category = path.clone();
                }
                continue;
            }
            if !manages {
                state.error = "Keine Berechtigung für diesen Bauauftrag".into();
                continue;
            }
            let command = match action {
                BuildingUiAction::PickStorage => {
                    selection::begin(world, &mut state, selection::SelectionKind::Storage);
                    None
                }
                BuildingUiAction::PickResidential => {
                    selection::begin(world, &mut state, selection::SelectionKind::Residential);
                    None
                }
                BuildingUiAction::PickHouse => {
                    selection::begin(world, &mut state, selection::SelectionKind::House);
                    None
                }
                BuildingUiAction::SaveStorage => {
                    state.storage.map(|area| BuildingAction::SetStorage {
                        kingdom: displayed,
                        area,
                    })
                }
                BuildingUiAction::SaveResidential => {
                    state
                        .residential
                        .map(|area| BuildingAction::SetResidential {
                            kingdom: displayed,
                            area: Some(area),
                            template: state.template.clone(),
                        })
                }
                BuildingUiAction::DisableResidential => Some(BuildingAction::SetResidential {
                    kingdom: displayed,
                    area: None,
                    template: world
                        .resource::<ClientSession>()
                        .buildings
                        .settlements
                        .get(&displayed)
                        .map_or_else(
                            || state.template.clone(),
                            |site| site.selected_template.clone(),
                        ),
                }),
                BuildingUiAction::QueueHouse => {
                    state.origin.map(|origin| BuildingAction::QueueHouse {
                        kingdom: displayed,
                        origin,
                        facing: state.facing,
                        template: state.template.clone(),
                    })
                }
                BuildingUiAction::Facing(value) => {
                    if let Ok(value @ 0..=3) = value.parse() {
                        state.facing = value;
                    }
                    None
                }
                BuildingUiAction::Craftsman { npc, enabled } => {
                    Some(BuildingAction::AssignCraftsman {
                        kingdom: displayed,
                        npc,
                        enabled,
                    })
                }
                BuildingUiAction::Enabled {
                    id,
                    revision,
                    enabled,
                } => Some(BuildingAction::SetEnabled {
                    id,
                    revision,
                    enabled,
                }),
                BuildingUiAction::Cancel { id, revision } => {
                    Some(BuildingAction::Cancel { id, revision })
                }
                BuildingUiAction::Refresh
                | BuildingUiAction::ToggleAreas
                | BuildingUiAction::Category(_)
                | BuildingUiAction::Template(_) => None,
            };
            if let Some(command) = command {
                send(world, &mut state, command, now, false);
            }
        }
        let session = world.resource::<ClientSession>();
        let settlement = state
            .kingdom
            .and_then(|id| session.buildings.settlements.get(&id));
        let registered = settlement.map(|settlement| &settlement.craftsmen);
        let palette = world.get_resource::<terra_player::interaction::BlockPalette>();
        let jobs: Vec<_> = session
            .buildings
            .jobs
            .values()
            .filter(|job| Some(job.kingdom) == state.kingdom)
            .map(|job| {
                let mut totals = std::collections::BTreeMap::<terra_core::ItemKind, u32>::new();
                for stack in job.required_materials() {
                    *totals.entry(stack.item).or_default() += u32::from(stack.count);
                }
                BuildingJobRow {
                    id: job.id.to_string(),
                    revision: job.revision.to_string(),
                    template_name: catalog::name(session, &job.template),
                    location: format!("{} / {} / {}", job.origin[0], job.origin[1], job.origin[2]),
                    status: status_label(job.status).into(),
                    progress: format!("{:.1} %", job.progress() * 100.0),
                    workers: format!("{} / 4", job.workers.len()),
                    enabled: job.enabled,
                    active: !matches!(job.status, BuildStatus::Complete | BuildStatus::Cancelled),
                    has_materials: !totals.is_empty(),
                    materials: totals
                        .into_iter()
                        .map(|(item, count)| {
                            let (supply, sufficient) =
                                stock::supply(session, Some(job.kingdom), item, count);
                            BuildingMaterialRow {
                                identifier: item.identifier(),
                                label: catalog::material_label(item, palette),
                                count: count.to_string(),
                                preview_key: crate::inventory::preview_key(item),
                                supply,
                                sufficient,
                            }
                        })
                        .collect(),
                }
            })
            .collect();
        let workers: Vec<_> = session
            .work
            .workers
            .iter()
            .map(|worker| {
                let registered =
                    registered.is_some_and(|registered| registered.contains(&worker.npc));
                let busy = session.buildings.jobs.values().any(|job| {
                    job.workers.contains(&worker.npc)
                        && !matches!(job.status, BuildStatus::Complete | BuildStatus::Cancelled)
                });
                BuildingWorkerRow {
                    id: worker.npc.to_string(),
                    name: worker.name.clone(),
                    status: if busy {
                        "Auf einer Baustelle"
                    } else if registered {
                        "Handwerker"
                    } else {
                        "Arbeitskraft"
                    }
                    .into(),
                    registered,
                    can_assign: manages && !busy,
                }
            })
            .collect();
        let selection_active = state.selection.is_some();
        let mut template_view = catalog::project(world, session, &state.template, state.kingdom);
        let categories = build_mode::categories(&template_view.rows, &state.category);
        if state.category != "*" && !state.category.is_empty() {
            template_view
                .rows
                .retain(|row| row.category_path == state.category);
        }
        let next = BuildingView {
            visible: visible && !selection_active,
            can_manage: manages,
            pending: state.pending.is_some(),
            selection_active,
            selection_hint: selection::hint(state.selection).into(),
            selection_error: if selection_active {
                state.error.clone()
            } else {
                String::new()
            },
            storage: area_label(settlement.and_then(|settlement| settlement.storage)),
            storage_draft: area_label(state.storage),
            residential: area_label(settlement.and_then(|settlement| settlement.residential)),
            residential_draft: area_label(state.residential),
            residential_enabled: settlement
                .is_some_and(|settlement| settlement.residential.is_some()),
            origin: state.origin.map_or_else(
                || "Nicht festgelegt".into(),
                |origin| format!("{} / {} / {}", origin[0], origin[1], origin[2]),
            ),
            facing: state.facing.to_string(),
            can_save_storage: manages && state.storage.is_some() && state.pending.is_none(),
            can_save_residential: manages
                && state.residential.is_some()
                && state.pending.is_none()
                && template_view.selected,
            can_queue: manages
                && state.origin.is_some()
                && state.pending.is_none()
                && template_view.selected
                && (settlement.is_some_and(|settlement| settlement.storage.is_some())
                    || session.buildings.stock_kingdom == Some(displayed)
                        && session.buildings.unlimited_resources),
            can_show_areas: terra_network::client::work_areas::has_saved_areas(
                session,
                terra_core::work_areas::WorkAreaTarget::Settlement(displayed),
            ),
            areas_shown: world
                .resource::<terra_core::work_areas::WorkAreaDisplay>()
                .target
                == Some(terra_core::work_areas::WorkAreaTarget::Settlement(
                    displayed,
                )),
            templates: template_view.rows,
            categories,
            template_selected: template_view.selected,
            template_name: template_view.name,
            template_id: state.template.clone(),
            template_category: template_view.category,
            template_dimensions: template_view.dimensions,
            template_steps: template_view.steps,
            template_preview: template_view.preview,
            template_materials: template_view.materials,
            template_error: template_view.error,
            automatic_template: settlement.map_or_else(String::new, |site| {
                catalog::name(session, &site.selected_template)
            }),
            worker_count: registered.map_or(0, Vec::len).to_string(),
            empty: jobs.is_empty(),
            workers,
            jobs,
            error: state.error.clone(),
            feedback: state.feedback.clone(),
        };
        if *world.resource::<BuildingView>() != next {
            *world.resource_mut::<BuildingView>() = next;
        }
    });
}
