//! Detached, claim-scoped planning. A click queues an authoritative builder job.
mod camera;
mod preview;
#[cfg(test)]
mod tests;

use super::*;
use terra_core::work_areas::ConstructionAreaDisplay;
use terra_core::{GameAction, GameMode, building_jobs::TemplatePreview};
use terra_player::{CaptureRequest, ConstructionCamera, PlayerInputBlock, WorldInteractionBlock};

#[derive(Message, Clone, Debug)]
pub enum BuildModeUiAction {
    Exit,
    Catalog,
    Template(String),
    FromKeep(String),
    Category(String),
    Rotate,
    Confirm,
    Reload,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct CategoryRow {
    pub id: String,
    pub name: String,
    pub selected: bool,
}

pub(super) fn categories(rows: &[BuildingTemplateRow], selected: &str) -> Vec<CategoryRow> {
    let paths: std::collections::BTreeSet<_> =
        rows.iter().map(|row| row.category_path.clone()).collect();
    std::iter::once(CategoryRow {
        id: "*".into(),
        name: "Alle".into(),
        selected: selected == "*" || selected.is_empty(),
    })
    .chain(paths.into_iter().map(|path| CategoryRow {
        name: if path.is_empty() {
            "Allgemein".into()
        } else {
            path.clone()
        },
        selected: selected == path,
        id: path,
    }))
    .collect()
}

#[html_shared]
#[derive(Resource, Default, Clone, Debug, Serialize, PartialEq)]
pub struct BuildModeView {
    pub active: bool,
    pub visible: bool,
    pub catalog: bool,
    pub placing: bool,
    pub preview_ready: bool,
    pub can_place: bool,
    pub areas_visible: bool,
    pub areas_hint: String,
    pub pending: bool,
    pub can_reload: bool,
    pub title: String,
    pub location: String,
    pub facing: String,
    pub template_name: String,
    pub template_id: String,
    pub templates: Vec<BuildingTemplateRow>,
    pub materials: Vec<BuildingMaterialRow>,
    pub categories: Vec<CategoryRow>,
    pub error: String,
    pub feedback: String,
    pub notice: String,
}

#[derive(Resource, Default)]
pub(super) struct State {
    pub(super) kingdom: Option<KingdomId>,
    pub(super) pending: Option<(u64, f64)>,
    reloading: bool,
    template: String,
    category: String,
    facing: u8,
    placing: bool,
    catalog: bool,
    origin: Option<[i64; 3]>,
    preview: Option<TemplatePreview>,
    requested_preview: String,
    preview_at: f64,
    saved_camera: Option<Transform>,
    saved_perspective: Option<terra_player::CameraPerspective>,
    saved_focus: Option<[f64; 3]>,
    error: String,
    feedback: String,
    notice_until: f64,
}

#[derive(SystemSet, Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub(super) struct BuildModeSet;

pub(super) fn install(app: &mut App) {
    app.init_resource::<ConstructionCamera>()
        .init_resource::<ConstructionAreaDisplay>()
        .init_resource::<State>()
        .init_resource::<BuildModeView>()
        .add_message::<BuildModeUiAction>()
        .add_systems(
            PreUpdate,
            keyboard
                .after(crate::NpcWorkInputSet)
                .before(crate::KingdomInputSet),
        )
        .add_systems(
            Update,
            update
                .in_set(BuildModeSet)
                .after(super::update)
                .after(crate::npc_work::NpcWorkBlockSet)
                .before(terra_player::PlayerSet::Movement),
        );
    preview::install(app);
}

fn pressed(world: &World, action: GameAction) -> bool {
    let binding = world
        .get_resource::<terra_core::ControlsConfig>()
        .map_or_else(|| action.default_binding(), |c| c.binding(action));
    world
        .get_resource::<ButtonInput<KeyCode>>()
        .is_some_and(|keys| {
            binding.just_pressed(keys, world.get_resource::<ButtonInput<MouseButton>>())
        })
}

fn now(world: &World) -> f64 {
    world
        .get_resource::<Time<Real>>()
        .map_or(0.0, |t| t.elapsed_secs_f64())
}

fn playing(world: &World) -> bool {
    world
        .get_resource::<crate::menu::MenuState>()
        .is_none_or(|menu| menu.page == crate::menu::MenuPage::Playing)
        && world.get_resource::<ClientSession>().is_some_and(|s| {
            s.ready
                && s.error.is_none()
                && !s.dead()
                && !s.teleporting
                && s.game_mode != GameMode::Spectator
                && s.mounted.is_none()
                && s.sleeping().is_none()
                && s.train_seat.is_none()
        })
}

fn modal(world: &World) -> bool {
    world
        .get_resource::<crate::KingdomView>()
        .is_some_and(|v| v.visible)
        || world
            .get_resource::<crate::NpcWorkView>()
            .is_some_and(|v| v.direct)
        || world
            .get_resource::<crate::InventoryView>()
            .is_some_and(|v| v.open)
        || world
            .get_resource::<crate::ChatView>()
            .is_some_and(|v| v.open)
        || world
            .get_resource::<crate::TrainView>()
            .is_some_and(|v| v.open)
        || world
            .get_resource::<crate::SettingsView>()
            .is_some_and(|v| v.open)
        || world
            .get_resource::<crate::minimap::fullmap::FullMapView>()
            .is_some_and(|view| view.visible)
}

fn keyboard(world: &mut World) {
    let active = world.resource::<ConstructionCamera>().active;
    if active && playing(world) && !modal(world) && area_key_pressed(world) {
        let mut areas = world.resource_mut::<ConstructionAreaDisplay>();
        areas.visible = !areas.visible;
    }
    if active
        && playing(world)
        && !modal(world)
        && world
            .get_resource::<ButtonInput<KeyCode>>()
            .is_some_and(|keys| {
                keys.just_pressed(KeyCode::Enter) || keys.just_pressed(KeyCode::NumpadEnter)
            })
    {
        camera::confirm(world);
    }
    if active
        && (!playing(world)
            || pressed(world, GameAction::BuildMode) && !modal(world)
            || pressed(world, GameAction::Pause)
                && !modal(world)
                && !world
                    .resource::<crate::npc_work::WorkUiState>()
                    .consumed_escape)
    {
        if pressed(world, GameAction::Pause) {
            world
                .resource_mut::<crate::npc_work::WorkUiState>()
                .consumed_escape = true;
        }
        stop(world);
    } else if !active && pressed(world, GameAction::BuildMode) && playing(world) && !modal(world) {
        start(world, None);
    }
}

fn area_key_pressed(world: &World) -> bool {
    world
        .get_resource::<ButtonInput<KeyCode>>()
        .is_some_and(|keys| {
            keys.just_pressed(KeyCode::KeyH)
                && ![
                    KeyCode::F1,
                    KeyCode::ControlLeft,
                    KeyCode::ControlRight,
                    KeyCode::AltLeft,
                    KeyCode::AltRight,
                    KeyCode::SuperLeft,
                    KeyCode::SuperRight,
                ]
                .into_iter()
                .any(|key| keys.pressed(key))
        })
}

fn start(world: &mut World, selected: Option<String>) {
    if !playing(world) {
        return;
    }
    let entering = !world.resource::<ConstructionCamera>().active;
    let session = world.resource::<ClientSession>();
    let Some(body) = session.confirmed_state() else {
        return;
    };
    let position = Vec3::from_array(body.position);
    let kingdom = super::stock::build_claim(
        session,
        position.to_array().map(|v| f64::from(v).floor() as i64),
    )
    .filter(|id| super::stock::can_inspect(session, *id));
    let Some(kingdom) = kingdom else {
        let until = now(world) + 5.0;
        let mut state = world.resource_mut::<State>();
        state.error = "Baumodus ist nur in eigenen Bau-Chunks mit Baurecht verfügbar".into();
        state.notice_until = until;
        return;
    };
    let template = selected
        .clone()
        .or_else(|| {
            world
                .get_resource::<super::BuildingUiState>()
                .map(|s| s.template.clone())
        })
        .filter(|id| session.buildings.template(id).is_some())
        .or_else(|| {
            session
                .buildings
                .settlements
                .get(&kingdom)
                .map(|s| s.selected_template.clone())
        })
        .unwrap_or_else(|| terra_core::building_jobs::HOUSE_TEMPLATE.into());
    let pose = world
        .query_filtered::<&Transform, With<terra_player::TerraCamera>>()
        .iter(world)
        .next()
        .copied();
    let perspective = world
        .get_resource::<terra_player::CameraPerspective>()
        .copied();
    let focus = world
        .get_resource::<terra_world::streaming::ChunkFocus>()
        .map(|f| f.0);
    {
        let mut state = world.resource_mut::<State>();
        state.kingdom = Some(kingdom);
        // Choosing another template through the keep must retain the original
        // gameplay camera for exit, even when construction is already active.
        if entering {
            state.saved_camera = pose;
            state.saved_perspective = perspective;
            state.saved_focus = focus;
        }
        state.template = template;
        state.category = "*".into();
        state.facing = 0;
        state.placing = selected.is_some();
        state.catalog = selected.is_none();
        state.origin = None;
        state.preview = None;
        state.requested_preview.clear();
        state.error.clear();
        state.feedback.clear();
        state.pending = None;
        state.reloading = false;
    }
    {
        let mut areas = world.resource_mut::<ConstructionAreaDisplay>();
        areas.kingdom = Some(kingdom);
        if entering {
            areas.visible = true;
        }
    }
    let mut camera = world.resource_mut::<ConstructionCamera>();
    camera.active = true;
    if entering {
        camera.center = position - Vec3::Y * terra_player::PLAYER_HALF_HEIGHT;
        camera.height = 38.0;
    }
    if let Some(mut perspective) = world.get_resource_mut::<terra_player::CameraPerspective>() {
        *perspective = terra_player::CameraPerspective::ThirdPerson;
    }
    world.resource_mut::<PlayerInputBlock>().0 = true;
    world.resource_mut::<WorldInteractionBlock>().0 = true;
    world.write_message(CaptureRequest(false));
    world.write_message(crate::KingdomMenuAction::Close);
    if let Some(mut target) = world.get_resource_mut::<terra_player::TargetCreature>() {
        target.0 = None;
    }
    if let Some(mut target) = world.get_resource_mut::<terra_player::TargetTrain>() {
        target.0 = None;
    }
    if let Some(request) = world
        .resource_mut::<ClientSession>()
        .make_work_request(terra_network::protocol::npc_work::WorkAction::Inspect)
    {
        world.write_message(request);
    }
}

fn stop(world: &mut World) {
    world.resource_mut::<ConstructionCamera>().active = false;
    *world.resource_mut::<ConstructionAreaDisplay>() = Default::default();
    let (pose, perspective, focus) = {
        let mut state = world.resource_mut::<State>();
        state.placing = false;
        state.origin = None;
        state.pending = None;
        (
            state.saved_camera.take(),
            state.saved_perspective.take(),
            state.saved_focus.take(),
        )
    };
    if let Some(pose) = pose {
        for mut camera in world
            .query_filtered::<&mut Transform, With<terra_player::TerraCamera>>()
            .iter_mut(world)
        {
            *camera = pose;
        }
    }
    if let Some(perspective) = perspective
        && let Some(mut current) = world.get_resource_mut::<terra_player::CameraPerspective>()
    {
        *current = perspective;
    }
    if let Some(focus) = focus
        && let Some(mut current) = world.get_resource_mut::<terra_world::streaming::ChunkFocus>()
    {
        current.0 = focus;
    }
    world.resource_mut::<PlayerInputBlock>().0 = modal(world);
    world.resource_mut::<WorldInteractionBlock>().0 = modal(world);
    world.write_message(CaptureRequest(!modal(world)));
}

fn update(world: &mut World) {
    let actions: Vec<_> = world
        .resource_mut::<Messages<BuildModeUiAction>>()
        .drain()
        .collect();
    for action in actions {
        if let BuildModeUiAction::FromKeep(template) = action {
            start(world, Some(template));
            continue;
        }
        if !world.resource::<ConstructionCamera>().active {
            continue;
        }
        match action {
            BuildModeUiAction::Exit => stop(world),
            BuildModeUiAction::Catalog => {
                let mut state = world.resource_mut::<State>();
                state.catalog = !state.catalog;
            }
            BuildModeUiAction::Template(id) => {
                if world
                    .resource::<ClientSession>()
                    .buildings
                    .template(&id)
                    .is_some()
                {
                    let mut state = world.resource_mut::<State>();
                    state.template = id;
                    state.placing = true;
                    state.catalog = false;
                    state.origin = None;
                    state.requested_preview.clear();
                    state.error.clear();
                }
            }
            BuildModeUiAction::Category(path) => world.resource_mut::<State>().category = path,
            BuildModeUiAction::Rotate => {
                let mut state = world.resource_mut::<State>();
                state.facing = (state.facing + 1) % 4;
            }
            BuildModeUiAction::Confirm => camera::confirm(world),
            BuildModeUiAction::Reload => {
                if world.resource::<State>().pending.is_none()
                    && let Some(kingdom) = world.resource::<State>().kingdom
                    && let Some(request) = world
                        .resource_mut::<ClientSession>()
                        .make_building_request(BuildingAction::ReloadCatalog { kingdom })
                {
                    let id = request.0.request;
                    world.write_message(request);
                    let time = now(world);
                    let mut state = world.resource_mut::<State>();
                    state.pending = Some((id, time));
                    state.reloading = true;
                    state.placing = false;
                    state.preview = None;
                    state.error.clear();
                    state.feedback = "Bauvorlagen werden aktualisiert …".into();
                }
            }
            BuildModeUiAction::FromKeep(_) => {}
        }
    }
    if world.resource::<ConstructionCamera>().active {
        let allowed = playing(world) && {
            let session = world.resource::<ClientSession>();
            let state = world.resource::<State>();
            session.confirmed_state().and_then(|body| {
                super::stock::build_claim(
                    session,
                    body.position.map(|v| f64::from(v).floor() as i64),
                )
            }) == state.kingdom
                && state
                    .kingdom
                    .is_some_and(|id| super::stock::can_inspect(session, id))
        };
        if !allowed {
            stop(world);
        }
    }
    if !world.resource::<ConstructionCamera>().active {
        project(world);
        return;
    }
    camera::update(world);
    sync(world);
    if !modal(world) {
        camera::pick(world);
    }
    world.resource_mut::<PlayerInputBlock>().0 = true;
    world.resource_mut::<WorldInteractionBlock>().0 = true;
    world.write_message(CaptureRequest(false));
    project(world);
}

fn sync(world: &mut World) {
    let time = now(world);
    let preview = world
        .resource::<ClientSession>()
        .buildings
        .preview
        .as_ref()
        .filter(|preview| {
            preview.id == world.resource::<State>().template
                && world.resource::<State>().preview.as_ref() != Some(*preview)
        })
        .cloned();
    if let Some(preview) = preview {
        world.resource_mut::<State>().preview = Some(preview);
    }
    if let Some((request, started)) = world.resource::<State>().pending {
        let result = world
            .resource::<ClientSession>()
            .last_building_result
            .as_ref()
            .filter(|r| r.request == request)
            .cloned();
        if let Some(result) = result {
            let mut state = world.resource_mut::<State>();
            state.pending = None;
            if let Some(error) = result.error {
                state.error = super::error_label(error).into();
            } else {
                state.placing = false;
                state.feedback = if state.reloading {
                    "Bauvorlagen aktualisiert"
                } else {
                    "Bauauftrag bestätigt"
                }
                .into();
            }
            state.reloading = false;
        } else if time - started > 8.0 {
            let mut state = world.resource_mut::<State>();
            state.pending = None;
            state.error = "Keine Serverantwort. Bauaufträge im Bergfried prüfen".into();
        }
    }
    let state = world.resource::<State>();
    if state.pending.is_none()
        && state.kingdom.is_some()
        && state.placing
        && (state.requested_preview != state.template
            || state
                .preview
                .as_ref()
                .is_none_or(|p| p.id != state.template)
                && time - state.preview_at > 3.0)
    {
        let kingdom = state.kingdom;
        let template = state.template.clone();
        if let Some(kingdom) = kingdom
            && world
                .resource::<ClientSession>()
                .buildings
                .template(&template)
                .is_some()
        {
            let request = world.resource_mut::<ClientSession>().make_building_request(
                BuildingAction::Preview {
                    kingdom,
                    template: template.clone(),
                },
            );
            if let Some(request) = request {
                world.write_message(request);
            }
            let mut state = world.resource_mut::<State>();
            state.requested_preview = template;
            state.preview_at = time;
        }
    }
}

fn project(world: &mut World) {
    let active = world.resource::<ConstructionCamera>().active;
    let time = now(world);
    let state = world.resource::<State>();
    if !active || modal(world) {
        let next = BuildModeView {
            active,
            notice: if !active && time < state.notice_until {
                state.error.clone()
            } else {
                String::new()
            },
            ..Default::default()
        };
        if *world.resource::<BuildModeView>() != next {
            *world.resource_mut::<BuildModeView>() = next;
        }
        return;
    }
    let Some(session) = world.get_resource::<ClientSession>() else {
        *world.resource_mut::<BuildModeView>() = Default::default();
        return;
    };
    let template = super::catalog::project(world, session, &state.template, state.kingdom);
    let mut categories = std::collections::BTreeSet::new();
    for row in &template.rows {
        categories.insert(row.category_path.clone());
    }
    let categories = std::iter::once(CategoryRow {
        id: "*".into(),
        name: "Alle".into(),
        selected: state.category == "*",
    })
    .chain(categories.into_iter().map(|path| CategoryRow {
        name: if path.is_empty() {
            "Allgemein".into()
        } else {
            path.clone()
        },
        selected: state.category == path,
        id: path,
    }))
    .collect();
    let visible = active && !modal(world) && playing(world);
    let next = BuildModeView {
        active,
        visible,
        catalog: visible && state.catalog,
        placing: state.placing,
        preview_ready: state
            .preview
            .as_ref()
            .is_some_and(|preview| preview.id == state.template),
        can_place: camera::can_place(state),
        areas_visible: world.resource::<ConstructionAreaDisplay>().visible,
        areas_hint: if world.resource::<ConstructionAreaDisplay>().visible {
            "H: Gebiete ausblenden"
        } else {
            "H: Gebiete anzeigen"
        }
        .into(),
        pending: state.pending.is_some(),
        can_reload: state
            .kingdom
            .zip(session.uuid)
            .is_some_and(|(id, actor)| session.kingdoms.can_manage(actor, id)),
        title: state
            .kingdom
            .and_then(|id| session.kingdoms.get(id))
            .map_or_else(String::new, |k| k.name.clone()),
        location: state.origin.map_or_else(
            || "Einen geladenen Boden innerhalb eigener Bau-Chunks wählen".into(),
            |p| format!("{} / {} / {}", p[0], p[1], p[2]),
        ),
        facing: format!("{}°", u16::from(state.facing) * 90),
        template_name: template.name,
        template_id: state.template.clone(),
        templates: template
            .rows
            .into_iter()
            .filter(|row| state.category == "*" || row.category_path == state.category)
            .collect(),
        materials: template.materials,
        categories,
        error: if !state.error.is_empty() {
            state.error.clone()
        } else {
            template.error
        },
        feedback: state.feedback.clone(),
        notice: if !active && time < state.notice_until {
            state.error.clone()
        } else {
            String::new()
        },
    };
    if *world.resource::<BuildModeView>() != next {
        *world.resource_mut::<BuildModeView>() = next;
    }
}
