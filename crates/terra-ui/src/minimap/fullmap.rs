//! Full-screen navigation and persistent annotations over the shared map atlas.
use super::Atlas;
use bevy::{
    ecs::system::SystemParam,
    input::mouse::{MouseScrollUnit, MouseWheel},
    input_focus::InputFocus,
    prelude::*,
    ui::UiGlobalTransform,
};
use std::collections::BTreeMap;
use terra_core::kingdom::{ClaimActor, ClaimKind, DiplomaticTarget, claim_chunk};
use terra_core::{GameAction, GameplayConfig, GameplayInput};
use terra_network::client::ClientSession;
use terra_network::{
    client::KingdomRequest,
    protocol::kingdom::{KingdomAction, KingdomOutcome},
};
use terra_storage::{
    maps::MapIdentity,
    waypoints::{Waypoint, WaypointBook, WaypointIcon},
};
use terra_world::{cartography::MapLayer, streaming::ChunkStore};
use tilt_ui::{ElementId, html_shared};

#[derive(Resource)]
pub(crate) struct FullMapState {
    pub open: bool,
    pub center: Option<[f64; 2]>,
    pub span: i64,
    pub layer: Option<MapLayer>,
    pub selected: Option<u64>,
    pub waypoints: WaypointBook,
    pub consumed_escape: bool,
    pub three_dimensional: bool,
    pub grid: bool,
    pub mobs: bool,
    pub diplomacy: bool,
    pub diplomacy_chunk: Option<[i64; 2]>,
    diplomacy_pending: Option<(u64, f64)>,
    diplomacy_status: String,
    pub(super) claim_draft: BTreeMap<[i64; 2], ClaimKind>,
    claim_mode: Option<ClaimKind>,
    claim_settlement: Option<terra_core::kingdom::KingdomId>,
    claim_rules: terra_core::kingdom::KingdomRules,
    claim_rectangle: Option<([i64; 2], [i64; 2])>,
    automatic_layer: MapLayer,
    identity: Option<MapIdentity>,
    loaded: bool,
    drag: Option<Drag>,
    cursor: Option<[f64; 3]>,
    editor: Option<Editor>,
    deleting: bool,
    error: String,
    book_open: bool,
    icon_picker: bool,
    icon_search: String,
}

impl Default for FullMapState {
    fn default() -> Self {
        Self {
            open: false,
            center: None,
            span: 384,
            layer: None,
            selected: None,
            waypoints: WaypointBook::default(),
            consumed_escape: false,
            three_dimensional: false,
            grid: false,
            mobs: true,
            diplomacy: false,
            diplomacy_chunk: None,
            diplomacy_pending: None,
            diplomacy_status: String::new(),
            claim_draft: BTreeMap::new(),
            claim_mode: None,
            claim_settlement: None,
            claim_rules: Default::default(),
            claim_rectangle: None,
            automatic_layer: MapLayer::Surface,
            identity: None,
            loaded: false,
            drag: None,
            cursor: None,
            editor: None,
            deleting: false,
            error: String::new(),
            book_open: false,
            icon_picker: false,
            icon_search: String::new(),
        }
    }
}

impl FullMapState {
    pub(super) fn preview_claims(&self) -> BTreeMap<[i64; 2], ClaimKind> {
        let mut draft = self.claim_draft.clone();
        if let (Some((start, end)), Some(kind)) = (self.claim_rectangle, self.claim_mode)
            && let Some(cells) = rectangle_cells(start, end)
        {
            for p in cells {
                draft.insert(p, kind);
            }
        }
        draft
    }
    pub fn blocks_input(&self) -> bool {
        self.open || self.editor.is_some() || self.deleting
    }
    pub fn viewing(&self, player_center: [f64; 2]) -> [f64; 2] {
        self.center.unwrap_or(player_center)
    }
    pub fn resolved_layer(&self) -> MapLayer {
        self.layer.unwrap_or(self.automatic_layer)
    }
    pub fn three_dimensional_active(&self) -> bool {
        self.open && self.three_dimensional && self.resolved_layer() == MapLayer::Surface
    }
    fn enforce_layer(&mut self, automatic_layer: MapLayer) {
        self.automatic_layer = automatic_layer;
        if self.resolved_layer() != MapLayer::Surface {
            if self.three_dimensional {
                self.drag = None;
            }
            self.three_dimensional = false;
        }
    }
}

pub(super) fn relief_enabled(requested: bool, layer: MapLayer) -> bool {
    requested && layer == MapLayer::Surface
}

struct Drag {
    pointer: Vec2,
    center: [f64; 2],
    inverse: bevy::math::Affine2,
    meters_per_pixel: Vec2,
}

impl Drag {
    fn center_at(&self, pointer: Vec2) -> [f64; 2] {
        let delta = self.inverse.transform_vector2(pointer - self.pointer) * self.meters_per_pixel;
        clamp_center([
            self.center[0] - f64::from(delta.x),
            self.center[1] - f64::from(delta.y),
        ])
    }
}

struct Editor {
    id: Option<u64>,
    name: String,
    coordinates: [String; 3],
    original_position: [f64; 3],
    cave_level: Option<i16>,
    color: [u8; 3],
    icon: WaypointIcon,
    show_in_world: bool,
}

impl Editor {
    fn from_point(point: &Waypoint) -> Self {
        Self {
            id: Some(point.id),
            name: point.name.clone(),
            coordinates: point.position.map(coordinate_label),
            original_position: point.position,
            cave_level: point.cave_level,
            color: point.color,
            icon: point.icon.clone(),
            show_in_world: point.show_in_world,
        }
    }
}

fn coordinate_label(value: f64) -> String {
    let value = if value.abs() < 0.005 { 0.0 } else { value };
    format!("{value:.2}").replace('.', ",")
}

#[derive(Clone, serde::Serialize, PartialEq)]
pub struct WaypointRow {
    pub id: String,
    pub name: String,
    pub coordinates: String,
    pub distance: String,
    pub marker_style: String,
    pub selected: bool,
    pub icon: String,
    pub icon_source: String,
    pub glyph_style: String,
}

#[derive(Clone, serde::Serialize, PartialEq)]
pub struct WaypointIconChoice {
    pub key: String,
    pub title: String,
    pub source: String,
    pub selected: bool,
}

#[derive(Clone, serde::Serialize, PartialEq)]
pub struct DiplomacyLegend {
    pub id: String,
    pub name: String,
    pub detail: String,
    pub color: String,
    pub icon_source: String,
    pub relation: String,
    pub relation_label: String,
}

#[html_shared]
#[derive(Resource, Default, serde::Serialize, PartialEq)]
pub struct FullMapView {
    pub visible: bool,
    pub loading: bool,
    pub error: String,
    pub coordinates: String,
    pub range: String,
    pub layer: String,
    pub style: String,
    pub surface_style: String,
    pub map_size: f32,
    pub map_width: f32,
    pub map_height: f32,
    pub sidebar_style: String,
    pub editing: bool,
    pub deleting: bool,
    pub selected: bool,
    pub available: bool,
    pub automatic: bool,
    pub surface: bool,
    pub cave: bool,
    pub name: String,
    pub x: String,
    pub y: String,
    pub z: String,
    pub color: String,
    pub icon: String,
    pub show_in_world: bool,
    pub waypoints: Vec<WaypointRow>,
    pub markers: Vec<super::waypoints::WaypointLabel>,
    pub peers: Vec<super::raster::PlayerLabel>,
    pub nearest: bool,
    pub farthest: bool,
    pub three_dimensional: bool,
    pub three_dimensional_available: bool,
    pub grid: bool,
    pub mobs: bool,
    pub book_open: bool,
    pub biome: String,
    pub height: String,
    pub zoom: String,
    pub icon_source: String,
    pub icon_picker: bool,
    pub icon_search: String,
    pub icons: Vec<WaypointIconChoice>,
    pub editor_style: String,
    pub picker_style: String,
    pub compact: bool,
    pub diplomacy: bool,
    pub diplomacy_chunk: String,
    pub diplomacy_name: String,
    pub diplomacy_kind: String,
    pub diplomacy_actor: String,
    pub diplomacy_owner: String,
    pub diplomacy_relation: String,
    pub diplomacy_relation_key: String,
    pub diplomacy_color: String,
    pub diplomacy_status: String,
    pub diplomacy_limits: String,
    pub diplomacy_pending: bool,
    pub diplomacy_can_claim: bool,
    pub diplomacy_can_release: bool,
    pub diplomacy_legends: Vec<DiplomacyLegend>,
    pub claim_build_mode: bool,
    pub claim_territory_mode: bool,
    pub claim_draft_count: String,
    pub bot_realms: Vec<DiplomacyLegend>,
    pub claim_can_apply: bool,
}

#[derive(Message, Debug)]
pub enum FullMapAction {
    Command { control: String, id: Option<u64> },
    Change { control: String, value: String },
}

pub(super) fn install(app: &mut App) {
    app.init_resource::<FullMapState>()
        .init_resource::<super::LayerProbe>()
        .init_resource::<FullMapView>()
        .add_message::<FullMapAction>()
        .add_message::<KingdomRequest>()
        .add_systems(
            Update,
            update
                .before(crate::menu::MenuInputSet)
                .before(super::update),
        );
}

#[derive(SystemParam)]
struct Context<'w, 's> {
    session: Option<ResMut<'w, ClientSession>>,
    menu: Option<Res<'w, crate::menu::MenuState>>,
    inventory: Option<Res<'w, crate::inventory::InventoryState>>,
    chat: Option<Res<'w, crate::chat::ChatState>>,
    pets: Option<Res<'w, crate::pets::PetMenuState>>,
    kingdom: Option<Res<'w, crate::kingdom::KingdomMenuState>>,
    settings: Res<'w, GameplayConfig>,
    atlas: Res<'w, Atlas>,
    config: Option<Res<'w, terra_core::GameConfig>>,
    chunks: Option<Res<'w, ChunkStore>>,
    settlements: Option<Res<'w, crate::settlement_map::SettlementMap>>,
    journal: Option<Res<'w, terra_world::edits::EditJournal>>,
    time: Option<Res<'w, Time>>,
    probe: ResMut<'w, super::LayerProbe>,
    windows: Query<'w, 's, &'static Window, With<bevy::window::PrimaryWindow>>,
    scale: Option<Res<'w, bevy::ui::UiScale>>,
}

#[allow(clippy::too_many_arguments)]
fn update(
    input: GameplayInput,
    mouse: Res<ButtonInput<MouseButton>>,
    mut scroll: MessageReader<MouseWheel>,
    mut actions: MessageReader<FullMapAction>,
    mut context: Context,
    mut state: ResMut<FullMapState>,
    mut view: ResMut<FullMapView>,
    mut capture: MessageWriter<terra_player::CaptureRequest>,
    mut blocked: ResMut<terra_player::PlayerInputBlock>,
    mut kingdoms: MessageWriter<KingdomRequest>,
    mut focus: Option<ResMut<InputFocus>>,
    nodes: Query<(&ElementId, &ComputedNode, &UiGlobalTransform)>,
) {
    state.consumed_escape = false;
    if state.identity != context.atlas.identity {
        let was_open = state.open;
        *state = FullMapState {
            identity: context.atlas.identity.clone(),
            ..default()
        };
        if state.identity.is_some() {
            load_waypoints(&mut state, &context.atlas);
        }
        if was_open {
            capture.write(terra_player::CaptureRequest(false));
        }
    }
    let playing = context
        .menu
        .as_ref()
        .is_none_or(|m| m.page == crate::menu::MenuPage::Playing);
    let ready = context
        .session
        .as_ref()
        .is_some_and(|s| s.ready && !s.teleporting && !s.dead());
    let occupied = context.inventory.as_ref().is_some_and(|s| s.blocks_input())
        || context.kingdom.as_ref().is_some_and(|k| k.blocks_input())
        || context.chat.as_ref().is_some_and(|s| s.blocks_input())
        || context.pets.as_ref().is_some_and(|p| p.blocks_input());
    let pose = context.session.as_ref().and_then(|s| s.confirmed_state());
    let player = context
        .atlas
        .display
        .unwrap_or_else(|| pose.map_or([0.0; 3], |p| p.position.map(f64::from)));
    let automatic_layer = if let Some(chunks) = context.chunks.as_ref() {
        let mode = context.settings.minimap.mode;
        let position = pose
            .map_or(player, |pose| pose.position.map(f64::from))
            .map(|v| v.floor() as i64);
        let key = super::LayerProbeKey::at(
            context.atlas.identity.as_ref(),
            mode,
            position,
            chunks,
            context.journal.as_deref(),
            !ready,
        );
        let now = context
            .time
            .as_ref()
            .map_or(0.0, |time| time.elapsed_secs_f64());
        context
            .probe
            .select(key, now, || super::select_layer(mode, position, chunks))
    } else {
        match context.settings.minimap.mode {
            terra_core::MapMode::Cave => super::cave_layer(player[1].floor() as i64),
            _ => MapLayer::Surface,
        }
    };
    state.enforce_layer(automatic_layer);
    poll_diplomacy(
        &mut state,
        context.session.as_deref(),
        context.time.as_ref().map_or(0.0, |t| t.elapsed_secs_f64()),
    );
    if !ready || !playing {
        state.open = false;
        state.drag = None;
        state.editor = None;
        state.deleting = false;
        state.icon_picker = false;
    } else if !occupied && state.editor.is_none() && input.just_pressed(GameAction::QuickWaypoint) {
        let layer = state.resolved_layer();
        let exact = pose.map_or(player, |pose| pose.position.map(f64::from));
        begin_editor(&mut state, exact, layer);
        if let Some(focus) = focus.as_mut() {
            focus.clear();
        }
        capture.write(terra_player::CaptureRequest(false));
    } else if !occupied && state.editor.is_none() && input.just_pressed(GameAction::FullMap) {
        state.open = !state.open;
        state.drag = None;
        state.editor = None;
        state.deleting = false;
        if let Some(focus) = focus.as_mut() {
            focus.clear();
        }
        capture.write(terra_player::CaptureRequest(!state.open));
    }
    if state.blocks_input() && input.just_pressed(GameAction::Pause) {
        state.consumed_escape = true;
        if state.icon_picker {
            state.icon_picker = false;
        } else if state.editor.is_some() || state.deleting {
            state.editor = None;
            state.deleting = false;
            if !state.open {
                capture.write(terra_player::CaptureRequest(true));
            }
            if let Some(focus) = focus.as_mut() {
                focus.clear();
            }
        } else {
            state.open = false;
            state.drag = None;
            capture.write(terra_player::CaptureRequest(true));
        }
    }
    if !state.open {
        state.claim_rectangle = None;
    }
    if !state.blocks_input() {
        actions.clear();
        scroll.clear();
        if view.visible || view.editing || view.deleting {
            *view = FullMapView::default();
        }
        return;
    }
    blocked.0 = true;
    for action in actions.read() {
        if matches!(action, FullMapAction::Command { control, .. } if control == "fullmap-close") {
            state.open = false;
            state.drag = None;
            state.editor = None;
            state.icon_picker = false;
            state.deleting = false;
            if let Some(focus) = focus.as_mut() {
                focus.clear();
            }
            capture.write(terra_player::CaptureRequest(true));
            continue;
        }
        let result = handle_action(&mut state, action, player, &context.atlas);
        if let FullMapAction::Command { control, .. } = action {
            let now = context.time.as_ref().map_or(0.0, |t| t.elapsed_secs_f64());
            if let Some(config) = context.config.as_deref() {
                state.claim_rules = config.world.kingdom_rules;
            }
            request_claim(
                &mut state,
                control,
                player,
                context.session.as_deref_mut(),
                &mut kingdoms,
                now,
            );
        }
        state.enforce_layer(automatic_layer);
        if let Err(error) = result {
            state.error = error;
        }
    }
    if !state.blocks_input() {
        scroll.clear();
        *view = FullMapView::default();
        if let Some(focus) = focus.as_mut() {
            focus.clear();
        }
        capture.write(terra_player::CaptureRequest(true));
        return;
    }
    let Ok(window) = context.windows.single() else {
        return;
    };
    let layer = state.resolved_layer();
    let relief = relief_enabled(context.settings.minimap.relief, layer);
    let tilt = if state.three_dimensional_active() {
        0.60
    } else if relief {
        0.88
    } else {
        1.0
    };
    let screen_pointer = window.physical_cursor_position();
    let viewport = nodes
        .iter()
        .find(|(id, _, _)| id.0 == "full-map-viewport")
        .and_then(|(_, node, transform)| {
            if node.is_empty() {
                return None;
            }
            Some((node.size(), transform.try_inverse()?))
        });
    let pointer = screen_pointer
        .zip(viewport)
        .map(|(pointer, (size, inverse))| {
            inverse.transform_point2(pointer) / size + Vec2::splat(0.5)
        });
    let over_sidebar = screen_pointer.is_some_and(|point| {
        nodes.iter().any(|(id, node, transform)| {
            matches!(id.0.as_str(), "full-map-diplomacy" | "full-map-sidebar")
                && !node.is_empty()
                && node.contains_point(*transform, point)
        })
    });
    let inside = !over_sidebar
        && pointer.is_some_and(|uv| (0.0..=1.0).contains(&uv.x) && (0.0..=1.0).contains(&uv.y));
    let editing = state.editor.is_some() || state.deleting;
    let aspect = viewport.map_or(1.0, |(size, _)| f64::from(size.y / size.x.max(1.0)));
    let projection_tilt = tilt / aspect;
    if state.open && !editing && window.focused {
        if let Some(uv) = pointer.filter(|_| inside) {
            let center = state.viewing([player[0], player[2]]);
            let position = pick_position(
                &context.atlas,
                &state,
                layer,
                center,
                uv,
                aspect as f32,
                relief,
                player[1],
            );
            if mouse.just_pressed(MouseButton::Left)
                && let (Some(pointer), Some((size, inverse))) = (screen_pointer, viewport)
            {
                state.cursor = Some(position);
                if state.diplomacy {
                    state.diplomacy_chunk = Some(claim_chunk(position.map(|v| v.floor() as i64)));
                }
                if state.diplomacy
                    && state.claim_mode.is_some()
                    && state.diplomacy_pending.is_none()
                {
                    let chunk = claim_chunk(position.map(|v| v.floor() as i64));
                    state.claim_rectangle = Some((chunk, chunk));
                } else {
                    state.center = Some(center);
                    state.drag = Some(Drag {
                        pointer,
                        center,
                        inverse,
                        meters_per_pixel: Vec2::new(
                            state.span as f32,
                            state.span as f32 / projection_tilt as f32,
                        ) / size,
                    });
                }
            }
            if (mouse.pressed(MouseButton::Left) || mouse.just_released(MouseButton::Left))
                && let Some((start, _)) = state.claim_rectangle
            {
                let end = claim_chunk(position.map(|v| v.floor() as i64));
                state.claim_rectangle = Some((start, end));
            }
            if state.diplomacy
                && state.diplomacy_pending.is_none()
                && (mouse.pressed(MouseButton::Right) || mouse.just_released(MouseButton::Right))
            {
                let chunk = claim_chunk(position.map(|v| v.floor() as i64));
                state.claim_rectangle = None;
                state.drag = None;
                state.diplomacy_chunk = Some(chunk);
                if state.claim_draft.remove(&chunk).is_some() {
                    state.diplomacy_status =
                        "Chunk removed from the preview. Apply confirms the remaining selection."
                            .into();
                }
            }
            if mouse.just_pressed(MouseButton::Right) && state.loaded && !state.diplomacy {
                begin_editor(&mut state, position, layer);
            }
            let wheel: f32 = scroll
                .read()
                .map(|e| match e.unit {
                    MouseScrollUnit::Line => e.y,
                    MouseScrollUnit::Pixel => e.y / 40.0,
                })
                .sum();
            if wheel != 0.0 {
                zoom_at(
                    &mut state,
                    center,
                    uv,
                    wheel.clamp(-8.0, 8.0),
                    projection_tilt,
                );
                state.drag = None;
                state.claim_rectangle = None;
            }
        } else {
            scroll.clear();
        }
        if mouse.pressed(MouseButton::Left)
            && let (Some(drag), Some(pointer)) = (&state.drag, screen_pointer)
        {
            state.center = Some(drag.center_at(pointer));
        }
    } else {
        scroll.clear();
    }
    if !state.open {
        state.claim_rectangle = None;
    }
    if !mouse.pressed(MouseButton::Left) || !window.focused {
        if let Some((start, end)) = state.claim_rectangle.take()
            && window.focused
        {
            stage_rectangle(&mut state, start, end);
        }
        state.drag = None;
    }
    let canvas = window.size() / context.scale.as_ref().map_or(1.0, |s| s.0).max(0.1);
    let mut next = make_view(&state, player, canvas, view.loading);
    next.cave = matches!(layer, MapLayer::Cave { .. });
    next.surface = layer == MapLayer::Surface;
    let center = state
        .viewing([player[0], player[2]])
        .map(|v| v.floor() as i64);
    next.biome = context
        .chunks
        .as_ref()
        .and_then(|chunks| chunks.prepared_column(center.map(|v| v.div_euclid(16))))
        .and_then(|column| {
            column.surface_at(center[0], center[1]).map(|surface| {
                super::biome_name(surface.biome_kind(context.config.as_ref().map_or_else(
                    || terra_core::WorldConfig::default().sea_level,
                    |config| config.world.sea_level,
                )))
                .into()
            })
        })
        .unwrap_or_else(|| "Unexplored".into());
    next.markers.clone_from(&view.markers);
    next.peers.clone_from(&view.peers);
    if let Some(config) = context.config.as_deref() {
        state.claim_rules = config.world.kingdom_rules;
    }
    diplomacy_view(
        &mut next,
        &state,
        context.session.as_deref(),
        context.settlements.as_deref(),
        player,
    );
    if *view != next {
        *view = next;
    }
}

#[allow(clippy::too_many_arguments)]
fn pick_position(
    atlas: &Atlas,
    state: &FullMapState,
    layer: MapLayer,
    center: [f64; 2],
    uv: Vec2,
    aspect: f32,
    relief: bool,
    fallback: f64,
) -> [f64; 3] {
    let projection = super::raster::Projection::at(
        center,
        state.span,
        0.0,
        false,
        relief_enabled(relief, layer),
        640,
        false,
    )
    .with_aspect(aspect)
    .three_dimensional(state.three_dimensional && layer == MapLayer::Surface);
    let level = atlas
        .full
        .relief_base
        .map_or(fallback as i64, |(_, level)| level);
    let pixel = (uv * 640.0).to_array();
    if projection.relief_enabled() {
        // Front-to-back orthographic ray through the explored heightfield only.
        // Both relief views shift elevated terrain away from the flat map plane.
        // This also selects visible cliff faces instead of the plane behind them.
        for y in (-128..=356).rev() {
            let xz = projection.world_at_height(pixel, y as f32, level);
            if mapped_height(atlas, layer, xz).is_some_and(|height| height >= f64::from(y)) {
                return [xz[0], f64::from(y), xz[1]];
            }
        }
    }
    let xz = projection.world_at(pixel);
    [
        xz[0],
        mapped_height(atlas, layer, xz).unwrap_or(fallback),
        xz[1],
    ]
}

fn mapped_height(atlas: &Atlas, layer: MapLayer, position: [f64; 2]) -> Option<f64> {
    let [x, z] = position.map(|v| v.floor() as i64);
    atlas
        .entries
        .get(&([x.div_euclid(16), z.div_euclid(16)], layer))?
        .height_at((z.rem_euclid(16) * 16 + x.rem_euclid(16)) as usize)
        .map(f64::from)
}

fn load_waypoints(state: &mut FullMapState, atlas: &Atlas) {
    match WaypointBook::load_directory(&atlas.root) {
        Ok(book) => {
            state.waypoints = book;
            state.loaded = true;
            state.error.clear();
        }
        Err(error) => {
            state.loaded = false;
            state.error = format!("Cannot load waypoints: {error}");
        }
    }
}

fn begin_editor(state: &mut FullMapState, position: [f64; 3], layer: MapLayer) {
    state.editor = Some(Editor {
        id: None,
        name: String::new(),
        coordinates: position.map(coordinate_label),
        original_position: position,
        cave_level: match layer {
            MapLayer::Surface => None,
            MapLayer::Cave { level } => Some(level),
        },
        color: [231, 190, 102],
        icon: WaypointIcon::default(),
        show_in_world: true,
    });
    state.deleting = false;
    state.drag = None;
    state.error.clear();
    state.icon_picker = false;
    state.icon_search.clear();
}

fn handle_action(
    state: &mut FullMapState,
    action: &FullMapAction,
    player: [f64; 3],
    atlas: &Atlas,
) -> Result<(), String> {
    match action {
        FullMapAction::Change { control, value } => {
            if control == "fullmap-icon-search" {
                state.icon_search = value.chars().take(48).collect();
                return Ok(());
            }
            let Some(editor) = state.editor.as_mut() else {
                return Ok(());
            };
            match control.as_str() {
                "fullmap-name" => editor.name = value.chars().take(49).collect(),
                "fullmap-x" | "fullmap-y" | "fullmap-z" => {
                    let index = match control.as_str() {
                        "fullmap-x" => 0,
                        "fullmap-y" => 1,
                        _ => 2,
                    };
                    editor.coordinates[index] = value.chars().take(32).collect();
                }
                "fullmap-color" => {
                    let value = value.trim_start_matches('#');
                    if value.len() != 6 || !value.is_ascii() {
                        return Err("Invalid waypoint color".into());
                    }
                    let mut color = [0; 3];
                    for (index, channel) in color.iter_mut().enumerate() {
                        *channel = u8::from_str_radix(&value[index * 2..index * 2 + 2], 16)
                            .map_err(|_| "Invalid waypoint color")?;
                    }
                    editor.color = color;
                }
                "fullmap-icon" => {
                    let icon = WaypointIcon::parse(value).ok_or("Invalid waypoint symbol")?;
                    if tilt_ui::Icon::from_name(icon.glyph()).is_none() {
                        return Err("Unknown waypoint icon".into());
                    }
                    editor.icon = icon;
                    state.icon_picker = false;
                }
                "fullmap-world" => editor.show_in_world = value == "true",
                _ => {}
            }
        }
        FullMapAction::Command { control, id } => match control.as_str() {
            "fullmap-diplomacy" => {
                state.diplomacy = !state.diplomacy;
                state.book_open = false;
                state.drag = None;
            }
            "fullmap-diplomacy-close" => {
                state.diplomacy = false;
                state.claim_rectangle = None;
            }
            "fullmap-grid" => state.grid = !state.grid,
            "fullmap-mobs" => state.mobs = !state.mobs,
            "fullmap-mode" if state.resolved_layer() == MapLayer::Surface => {
                state.three_dimensional = !state.three_dimensional;
                state.drag = None;
            }
            "fullmap-list" => {
                state.book_open = !state.book_open;
                state.diplomacy = false;
            }
            "fullmap-list-close" => state.book_open = false,
            "fullmap-icon" | "fullmap-icons" if state.editor.is_some() => {
                state.icon_picker = true;
                state.icon_search.clear();
            }
            "fullmap-icons-close" => state.icon_picker = false,
            "fullmap-layer" => {
                let current = state.resolved_layer();
                state.layer = if matches!(current, MapLayer::Cave { .. }) {
                    Some(MapLayer::Surface)
                } else {
                    Some(MapLayer::Cave {
                        level: ((player[1].floor() as i64).div_euclid(4) * 4).clamp(-128, 356)
                            as i16,
                    })
                };
                state.drag = None;
            }
            "fullmap-center" => {
                state.center = None;
                state.cursor = None;
            }
            "fullmap-in" => state.span = (state.span / 2).max(96),
            "fullmap-out" => state.span = (state.span * 2).min(4096),
            "fullmap-auto" => state.layer = None,
            "fullmap-surface" => state.layer = Some(MapLayer::Surface),
            "fullmap-cave" => {
                state.layer = Some(MapLayer::Cave {
                    level: ((player[1].floor() as i64).div_euclid(4) * 4).clamp(-128, 356) as i16,
                })
            }
            "fullmap-retry" => load_waypoints(state, atlas),
            "fullmap-point" => {
                if let Some(point) =
                    id.and_then(|id| state.waypoints.entries.iter().find(|point| point.id == id))
                {
                    state.center = Some([point.position[0], point.position[2]]);
                    state.layer = Some(
                        point
                            .cave_level
                            .map_or(MapLayer::Surface, |level| MapLayer::Cave { level }),
                    );
                    state.selected = Some(point.id);
                    state.editor = None;
                    state.deleting = false;
                }
            }
            "fullmap-add" if state.loaded => {
                let position = state.cursor.unwrap_or(player);
                let layer = state.resolved_layer();
                begin_editor(state, position, layer);
            }
            "fullmap-edit" => {
                if let Some(point) = state
                    .waypoints
                    .entries
                    .iter()
                    .find(|point| Some(point.id) == state.selected)
                {
                    state.editor = Some(Editor::from_point(point));
                    state.deleting = false;
                    state.error.clear();
                }
            }
            "fullmap-delete" => state.deleting = state.selected.is_some(),
            "fullmap-delete-cancel" | "fullmap-cancel" => {
                state.editor = None;
                state.deleting = false;
                state.icon_picker = false;
            }
            "fullmap-delete-confirm" if state.loaded && state.deleting => {
                let mut next = state.waypoints.clone();
                if state.selected.is_some_and(|id| next.remove(id)) {
                    next.save_directory(&atlas.root)
                        .map_err(|e| format!("Cannot save waypoints: {e}"))?;
                    state.waypoints = next;
                    state.selected = None;
                }
                state.deleting = false;
                state.error.clear();
            }
            "fullmap-save" if state.loaded => save_editor(state, atlas)?,
            _ => {}
        },
    }
    Ok(())
}

fn save_editor(state: &mut FullMapState, atlas: &Atlas) -> Result<(), String> {
    let Some(editor) = state.editor.as_ref() else {
        return Ok(());
    };
    let mut position = [0.0; 3];
    for (index, coordinate) in editor.coordinates.iter().enumerate() {
        // Formatting a name-only edit must not quantize its stored world position.
        if editor.original_position[index].is_finite()
            && coordinate.trim() == coordinate_label(editor.original_position[index])
        {
            position[index] = editor.original_position[index];
            continue;
        }
        position[index] = coordinate
            .trim()
            .replace(',', ".")
            .parse::<f64>()
            .ok()
            .filter(|v| v.is_finite())
            .ok_or("Enter finite X, Y and Z coordinates")?;
    }
    let mut next = state.waypoints.clone();
    let id = if let Some(id) = editor.id {
        next.update(Waypoint {
            id,
            name: editor.name.trim().into(),
            position,
            cave_level: editor.cave_level,
            color: editor.color,
            icon: editor.icon.clone(),
            show_in_world: editor.show_in_world,
        })?;
        id
    } else {
        let id = next.create(&editor.name, position, editor.cave_level, editor.color)?;
        if let Some(point) = next.entries.last_mut() {
            point.icon = editor.icon.clone();
            point.show_in_world = editor.show_in_world;
        }
        id
    };
    next.save_directory(&atlas.root)
        .map_err(|e| format!("Cannot save waypoints: {e}"))?;
    state.waypoints = next;
    state.selected = Some(id);
    state.editor = None;
    state.icon_picker = false;
    state.error.clear();
    Ok(())
}

fn world_at(center: [f64; 2], span: i64, uv: Vec2, tilt: f64) -> [f64; 2] {
    [
        center[0] + f64::from(uv.x - 0.5) * span as f64,
        center[1] + f64::from(uv.y - 0.5) * span as f64 / tilt,
    ]
}

fn clamp_center(center: [f64; 2]) -> [f64; 2] {
    let limit = terra_world::MAX_SAMPLE_COORDINATE as f64 - 8192.0;
    center.map(|v| v.clamp(-limit, limit))
}

fn zoom_at(state: &mut FullMapState, center: [f64; 2], uv: Vec2, wheel: f32, tilt: f64) {
    let before = world_at(center, state.span, uv, tilt);
    state.span = ((state.span as f64 * 2.0_f64.powf(-f64::from(wheel) * 0.25)).round() as i64)
        .clamp(96, 4096);
    let after = world_at(center, state.span, uv, tilt);
    state.center = Some(clamp_center([
        center[0] + before[0] - after[0],
        center[1] + before[1] - after[1],
    ]));
}

fn make_view(state: &FullMapState, player: [f64; 3], canvas: Vec2, loading: bool) -> FullMapView {
    let width = canvas.x.max(280.0);
    let height = canvas.y.max(240.0);
    let map_width = (width - 156.0).max(96.0);
    let map_height = (height - 168.0).max(96.0);
    let center = state.viewing([player[0], player[2]]);
    let editor = state.editor.as_ref();
    FullMapView {
        visible: state.open,
        loading,
        error: state.error.clone(),
        coordinates: format!("X {:.0}  Y {:.0}  Z {:.0}", center[0], player[1], center[1]),
        range: format!("{} m", state.span),
        layer: match state.layer {
            None => "Automatic".into(),
            Some(MapLayer::Surface) => "Surface".into(),
            Some(MapLayer::Cave { level }) => format!("Caves / Y {level}"),
        },
        style: format!("width: {width:.0}px; height: {height:.0}px;"),
        surface_style: format!("width: {map_width:.0}px; height: {map_height:.0}px;"),
        map_size: map_width,
        map_width,
        map_height,
        sidebar_style: format!("width: {:.0}px; max-height: {:.0}px;",(width-148.0).clamp(180.0,300.0),height-160.0),
        editing: editor.is_some(),
        deleting: state.deleting,
        selected: state.selected.is_some(),
        available: state.loaded,
        automatic: state.layer.is_none(),
        surface: matches!(state.layer, Some(MapLayer::Surface)),
        cave: matches!(state.layer, Some(MapLayer::Cave { .. })),
        name: editor.map_or_else(String::new, |e| e.name.clone()),
        x: editor.map_or_else(String::new, |e| e.coordinates[0].clone()),
        y: editor.map_or_else(String::new, |e| e.coordinates[1].clone()),
        z: editor.map_or_else(String::new, |e| e.coordinates[2].clone()),
        color: editor.map_or_else(
            || "#e7be66".into(),
            |e| format!("#{:02x}{:02x}{:02x}", e.color[0], e.color[1], e.color[2]),
        ),
        icon: editor.map_or_else(|| "pin".into(), |e| e.icon.key().into()),
        show_in_world: editor.is_some_and(|e| e.show_in_world),
        waypoints: state
            .waypoints
            .entries
            .iter()
            .map(|point| {
                let distance = super::waypoints::distance(point, player);
                WaypointRow {
                    id: point.id.to_string(),
                    name: point.name.clone(),
                    coordinates: format!(
                        "{:.0}, {:.0}, {:.0}",
                        point.position[0], point.position[1], point.position[2]
                    ),
                    distance: super::waypoints::distance_label(distance),
                    marker_style: format!(
                        "color: #{:02x}{:02x}{:02x};",
                        point.color[0], point.color[1], point.color[2]
                    ),
                    selected: state.selected == Some(point.id),
                    icon: point.icon.key().into(),
                    icon_source: icon_source(&point.icon),
                    glyph_style: "width: 18px; height: 18px; min-width: 18px; max-width: 18px; min-height: 18px; max-height: 18px;".into(),
                }
            })
            .collect(),
        markers: Vec::new(),
        peers: Vec::new(),
        nearest: state.span <= 96,
        farthest: state.span >= 4096,
        three_dimensional: state.three_dimensional_active(),
        three_dimensional_available: state.resolved_layer() == MapLayer::Surface,
        grid: state.grid,
        mobs: state.mobs,
        book_open: state.book_open,
        biome: String::new(),
        height: format!("{:.0}m",player[1]),
        zoom: format!("{:.0}%",38400.0/state.span as f64),
        icon_source: editor.map_or_else(||"tilt-icon:map-pin@32".into(),|e|icon_source(&e.icon)),
        icon_picker: state.icon_picker,
        icon_search: state.icon_search.clone(),
        icons: if state.icon_picker {
            let search = state.icon_search.to_lowercase();
            tilt_ui::Icon::ALL.iter().filter(|icon|icon.name().contains(&search))
                .map(|icon| WaypointIconChoice {
                    key: icon.name().into(), title: icon.name().replace('-'," "),
                    source: icon.source(tilt_ui::IconSize::Px32),
                    selected: editor.is_some_and(|e|e.icon.glyph()==icon.name()),
                }).collect()
        } else { Vec::new() },
        editor_style: format!("width: {:.0}px; max-height: {:.0}px;",(width-48.0).clamp(232.0,460.0),(height-40.0).max(180.0)),
        picker_style: format!("width: {:.0}px; height: {:.0}px;",(width-48.0).clamp(232.0,640.0),(height-48.0).clamp(180.0,560.0)),
        compact: width<850.0 || height<560.0,
        diplomacy: state.diplomacy,
        diplomacy_chunk: String::new(),
        diplomacy_name: String::new(),
        diplomacy_kind: String::new(),
        diplomacy_actor: String::new(),
        diplomacy_owner: String::new(),
        diplomacy_relation: String::new(),
        diplomacy_relation_key: String::new(),
        diplomacy_color: String::new(),
        diplomacy_status: state.diplomacy_status.clone(),
        diplomacy_limits: String::new(),
        diplomacy_pending: state.diplomacy_pending.is_some(),
        diplomacy_can_claim: false,
        diplomacy_can_release: false,
        diplomacy_legends: Vec::new(),
        claim_build_mode: state.claim_mode == Some(ClaimKind::ProtectedBuilding),
        claim_territory_mode: state.claim_mode == Some(ClaimKind::RealmTerritory),
        bot_realms: Vec::new(),
        claim_draft_count: format!("{} chunks selected",state.preview_claims().len()),
        claim_can_apply: !state.claim_draft.is_empty() && state.claim_rectangle.is_none() && state.diplomacy_pending.is_none(),
    }
}

pub(super) fn icon_source(icon: &WaypointIcon) -> String {
    tilt_ui::Icon::from_name(icon.glyph())
        .unwrap_or(tilt_ui::Icon::MapPin)
        .source(tilt_ui::IconSize::Px32)
}

fn rectangle_cells(a: [i64; 2], b: [i64; 2]) -> Option<Vec<[i64; 2]>> {
    if !terra_core::kingdom::valid_claim_chunk(a) || !terra_core::kingdom::valid_claim_chunk(b) {
        return None;
    }
    let width = a[0].abs_diff(b[0]) + 1;
    let height = a[1].abs_diff(b[1]) + 1;
    if width.checked_mul(height)? > terra_core::kingdom::KingdomRegistry::MAX_BATCH_CLAIMS as u64 {
        return None;
    }
    Some(
        (a[0].min(b[0])..=a[0].max(b[0]))
            .flat_map(|x| (a[1].min(b[1])..=a[1].max(b[1])).map(move |z| [x, z]))
            .collect(),
    )
}
fn stage_rectangle(state: &mut FullMapState, a: [i64; 2], b: [i64; 2]) {
    let (Some(kind), Some(cells)) = (state.claim_mode, rectangle_cells(a, b)) else {
        state.diplomacy_status = "Select at most 256 chunks per Apply.".into();
        return;
    };
    let mut draft = state.claim_draft.clone();
    for p in cells {
        draft.insert(p, kind);
    }
    if draft.len() > terra_core::kingdom::KingdomRegistry::MAX_BATCH_CLAIMS {
        state.diplomacy_status = "Select at most 256 chunks per Apply.".into();
        return;
    }
    state.claim_draft = draft;
    state.diplomacy_status = "Preview only. Apply confirms all selected chunks.".into();
}

fn poll_diplomacy(state: &mut FullMapState, session: Option<&ClientSession>, now: f64) {
    let Some((request, since)) = state.diplomacy_pending else {
        return;
    };
    if let Some(result) = session
        .and_then(|s| s.last_kingdom_result.as_ref())
        .filter(|r| r.request == request)
    {
        state.diplomacy_pending = None;
        state.diplomacy_status = match &result.outcome {
            KingdomOutcome::Rejected(error) => error.to_string(),
            KingdomOutcome::ClaimsApplied { count, .. } => {
                state.claim_draft.clear();
                format!("{count} chunks applied.")
            }
            KingdomOutcome::Claimed { .. } => "Chunk claimed.".into(),
            KingdomOutcome::Released { .. } => "Chunk released.".into(),
            _ => "Realm updated.".into(),
        };
    } else if now - since > 10.0 {
        state.diplomacy_pending = None;
        state.diplomacy_status =
            "No server response yet. The map refreshes when the authoritative update arrives."
                .into();
    }
}

fn managed_settlement<'a>(
    session: &'a ClientSession,
    state: &FullMapState,
    player: [f64; 3],
) -> Option<&'a terra_core::kingdom::KingdomRecord> {
    let actor = session.uuid?;
    let main = session.kingdoms.active_for_member(actor)?;
    let selected = state
        .diplomacy_chunk
        .and_then(|chunk| session.kingdoms.claim_at(chunk))
        .map(|claim| claim.kingdom);
    let local = session
        .kingdoms
        .claim_at(claim_chunk(player.map(|v| v.floor() as i64)))
        .map(|claim| claim.kingdom);
    [state.claim_settlement, selected, local, Some(main.id)]
        .into_iter()
        .flatten()
        .filter_map(|id| session.kingdoms.get(id))
        .find(|record| {
            record.state == terra_core::kingdom::KingdomState::Active
                && session.kingdoms.realm_id(record.id) == main.id
                && session.kingdoms.can_manage(actor, record.id)
        })
}

fn request_claim(
    state: &mut FullMapState,
    control: &str,
    player: [f64; 3],
    session: Option<&mut ClientSession>,
    writer: &mut MessageWriter<KingdomRequest>,
    now: f64,
) {
    if !state.diplomacy || state.diplomacy_pending.is_some() {
        return;
    }
    match control {
        "fullmap-claim-protected" | "fullmap-claim-territory" => {
            let previous = state.claim_settlement.take();
            let selected = session
                .as_deref()
                .and_then(|session| managed_settlement(session, state, player))
                .map(|record| record.id);
            if previous != selected {
                state.claim_draft.clear();
            }
            state.claim_settlement = selected;
            state.claim_mode = Some(if control == "fullmap-claim-protected" {
                ClaimKind::ProtectedBuilding
            } else {
                ClaimKind::RealmTerritory
            });
            state.claim_rectangle = None;
            state.drag = None;
            state.diplomacy_status = "Drag an area on the map, then Apply.".into();
            return;
        }
        "fullmap-claim-pan" => {
            state.claim_mode = None;
            state.claim_rectangle = None;
            return;
        }
        "fullmap-claim-cancel" => {
            state.claim_draft.clear();
            state.claim_rectangle = None;
            state.claim_settlement = None;
            state.diplomacy_status.clear();
            return;
        }
        _ => {}
    }
    if !matches!(control, "fullmap-claim-apply" | "fullmap-release-claim") {
        return;
    }
    let Some(session) = session else { return };
    let Some(kingdom) = managed_settlement(session, state, player) else {
        state.diplomacy_status =
            "Join or found a kingdom, village or city before claiming land.".into();
        return;
    };
    let id = kingdom.id;
    if control == "fullmap-claim-apply" && state.claim_settlement.is_some_and(|locked| locked != id)
    {
        state.diplomacy_status =
            "Selected settlement is no longer available. Choose it again before applying.".into();
        return;
    }
    if !session
        .uuid
        .is_some_and(|uuid| session.kingdoms.can_manage(uuid, id))
    {
        state.diplomacy_status = "Only founders and officers can manage realm claims.".into();
        return;
    }
    let chunk = state
        .diplomacy_chunk
        .unwrap_or_else(|| claim_chunk(player.map(|v| v.floor() as i64)));
    let action = if control == "fullmap-claim-apply" {
        if state.claim_draft.is_empty() || state.claim_rectangle.is_some() {
            return;
        }
        KingdomAction::ClaimBatch {
            id,
            claims: state.claim_draft.iter().map(|(p, k)| (*p, *k)).collect(),
        }
    } else {
        KingdomAction::ReleaseChunk { id, chunk }
    };
    if let Some(request) = session.kingdom_request(action) {
        state.diplomacy_pending = Some((request.0.request, now));
        state.diplomacy_status = "Waiting for the server…".into();
        writer.write(request);
    }
}

fn diplomacy_view(
    view: &mut FullMapView,
    state: &FullMapState,
    session: Option<&ClientSession>,
    settlements: Option<&crate::settlement_map::SettlementMap>,
    player: [f64; 3],
) {
    if !state.diplomacy {
        return;
    }
    let Some(session) = session else {
        return;
    };
    let registry = &session.kingdoms;
    let chunk = state
        .diplomacy_chunk
        .unwrap_or_else(|| claim_chunk(player.map(|v| v.floor() as i64)));
    view.diplomacy_chunk = format!("Chunk {} / {} · 16 × 16", chunk[0], chunk[1]);
    let managed = managed_settlement(session, state, player);
    let claim = registry.claim_at(chunk);
    let record = claim.and_then(|claim| registry.get(claim.kingdom));
    view.diplomacy_name = record.map_or("Unclaimed", |k| k.name.as_str()).into();
    view.diplomacy_owner =
        record.map_or_else(String::new, |k| format!("Founder: {}", k.founder_name));
    view.diplomacy_kind = claim
        .map_or("No building protection", |claim| match claim.kind {
            ClaimKind::ProtectedBuilding => "Protected building land",
            ClaimKind::RealmTerritory => "Open realm territory",
        })
        .into();
    let relation = record.map(|record| {
        crate::realm_relations::display(registry, session.uuid, DiplomaticTarget::Player(record.id))
    });
    view.diplomacy_relation = relation.map_or("Wilderness", |(_, label)| label).into();
    view.diplomacy_relation_key = relation
        .map_or("neutral", |(relation, _)| relation.key())
        .into();
    view.diplomacy_actor = claim.map_or_else(String::new, |claim| {
        let uuid = claim.actor.authorizer();
        let name = record
            .and_then(|k| k.member_names.iter().find(|(id, _)| *id == uuid))
            .map_or_else(
                || format!("Player {}", &uuid.simple().to_string()[..8]),
                |(_, name)| name.clone(),
            );
        match claim.actor {
            ClaimActor::Player(_) => format!("Claimed by {name}"),
            ClaimActor::Bot { id, .. } => format!("Claimed by bot #{id} · {name}"),
        }
    });
    view.diplomacy_color = record.map_or_else(
        || "background-color: #867957;".into(),
        |k| {
            let [r, g, b] = k.heraldry.color;
            format!("background-color: #{r:02x}{g:02x}{b:02x};")
        },
    );
    let manages_claims = managed.is_some_and(|managed| {
        session
            .uuid
            .is_some_and(|uuid| registry.can_manage(uuid, managed.id))
    });
    view.claim_can_apply &= manages_claims
        && session.allow_build
        && session.game_mode != terra_core::GameMode::Spectator;
    view.diplomacy_can_claim = manages_claims
        && !view.diplomacy_pending
        && session.allow_build
        && session.game_mode != terra_core::GameMode::Spectator;
    view.diplomacy_can_release = manages_claims
        && managed.is_some_and(|managed| claim.is_some_and(|c| c.kingdom == managed.id))
        && !view.diplomacy_pending;
    view.diplomacy_limits = managed.map_or_else(
        || "Membership in a kingdom, village or city is required to claim.".into(),
        |managed| {
            let used = registry.claim_counts(managed.id);
            let limits = registry.limits_with_rules(managed.id, &state.claim_rules);
            format!(
                "{} · Level {}\nBuilding {} / {} · Territory {} / {}",
                managed.name,
                managed.level,
                used.protected,
                limits.protected,
                used.territory,
                limits.territory
            )
        },
    );
    let mut visible = std::collections::BTreeSet::new();
    // Only visible claims and the player's realm enter the legend.
    let center = state.viewing([player[0], player[2]]);
    let margin = state.span as f64 * 2.5;
    for claim in &registry.claims {
        if (claim.chunk[0] as f64 * 16.0 + 8.0 - center[0]).abs() < margin
            && (claim.chunk[1] as f64 * 16.0 + 8.0 - center[1]).abs() < margin
        {
            visible.insert(claim.kingdom);
        }
    }
    if let Some(managed) = managed {
        visible.insert(managed.id);
    }
    let domains: Vec<_> = settlements.into_iter().flat_map(|s| s.domains()).collect();
    if claim.is_none()
        && let Some(domain) = domains.iter().find(|d| d.contains_chunk(chunk))
    {
        let position = [chunk[0] * 16 + 8, domain.center[1], chunk[1] * 16 + 8];
        let owner = registry.domain_owner(domain, position);
        if let Some(owner) = owner {
            view.diplomacy_name = owner.name.clone();
            view.diplomacy_owner =
                format!("Founder: {} · settlement affiliation", owner.founder_name);
            let (relation, label) = crate::realm_relations::display(
                registry,
                session.uuid,
                DiplomaticTarget::Player(owner.id),
            );
            view.diplomacy_relation = label.into();
            view.diplomacy_relation_key = relation.key().into();
        } else if let Some(realm) = domain.native {
            view.diplomacy_name = realm.name().into();
            let (relation, label) = crate::realm_relations::display(
                registry,
                session.uuid,
                DiplomaticTarget::Bot(realm),
            );
            view.diplomacy_relation = label.into();
            view.diplomacy_relation_key = relation.key().into();
            view.diplomacy_owner = "NPC settlement affiliation".into();
        } else {
            view.diplomacy_name = format!("Independent {}", domain.kind.label());
            view.diplomacy_kind = format!("{} · unclaimed settlement", domain.kind.label());
            view.diplomacy_relation = "Neutral".into();
            view.diplomacy_relation_key = "neutral".into();
            view.diplomacy_owner = "Independent settlement".into();
        }
        if let Some(heraldry) = registry.domain_heraldry(domain, position) {
            let [r, g, b] = heraldry.color;
            view.diplomacy_color = format!("background-color: #{r:02x}{g:02x}{b:02x};");
            view.diplomacy_kind = format!("{} · realm territory", domain.kind.label());
        }
    }
    for domain in &domains {
        if (domain.center[0] as f64 - center[0]).abs() < margin
            && (domain.center[2] as f64 - center[1]).abs() < margin
            && let Some(owner) = registry.domain_owner(domain, domain.center)
        {
            visible.insert(owner.id);
        }
    }
    view.bot_realms = terra_core::kingdom::BotKingdom::ALL
        .into_iter()
        .map(|realm| {
            let heraldry = realm.heraldry();
            let [r, g, b] = heraldry.color;
            let known = domains
                .iter()
                .filter(|d| d.native == Some(realm) && registry.domain_owner(d, d.center).is_none())
                .count();
            let (relation, label) = crate::realm_relations::display(
                registry,
                session.uuid,
                DiplomaticTarget::Bot(realm),
            );
            DiplomacyLegend {
                relation: relation.key().into(),
                relation_label: label.into(),
                id: realm.name().into(),
                name: realm.name().into(),
                detail: format!("NPC · {} known settlements", known),
                color: format!("background-color: #{r:02x}{g:02x}{b:02x};"),
                icon_source: crate::kingdom::emblem_source(heraldry.icon),
            }
        })
        .collect();
    view.diplomacy_legends = visible
        .into_iter()
        .filter_map(|id| registry.get(id))
        .map(|k| {
            let [r, g, b] = k.heraldry.color;
            let (relation, label) = crate::realm_relations::display(
                registry,
                session.uuid,
                DiplomaticTarget::Player(k.id),
            );
            DiplomacyLegend {
                relation: relation.key().into(),
                relation_label: label.into(),
                id: k.id.to_string(),
                name: k.name.clone(),
                detail: format!("{} · Level {}", k.kind.label(), k.level),
                color: format!("background-color: #{r:02x}{g:02x}{b:02x};"),
                icon_source: crate::kingdom::emblem_source(k.heraldry.icon),
            }
        })
        .collect();
}

#[cfg(test)]
mod tests {
    use super::*;

    struct HierarchicalClaimFixture {
        session: ClientSession,
        rules: terra_core::kingdom::KingdomRules,
        main: terra_core::kingdom::KingdomId,
        children: [(terra_core::kingdom::KingdomId, [i64; 2]); 3],
        foreign: terra_core::kingdom::KingdomId,
    }

    fn hierarchical_claim_session() -> HierarchicalClaimFixture {
        use terra_core::kingdom::{KingdomRules, SettlementKind};
        let actor = terra_core::PlayerUuid::from_u128(202_551);
        let rules = KingdomRules {
            require_adjacent_claims: false,
            realm_min_border_gap: 0,
            city_min_build_gap: 0,
            city_min_kingdom_level: 1,
            village_build_base: 47,
            city_build_base: 59,
            outpost_build_base: 71,
            ..default()
        };
        let mut session = ClientSession::default();
        session.uuid = Some(actor);
        session.ready = true;
        session.allow_build = true;
        let main = session
            .kingdoms
            .found_settlement_with_rules(
                actor,
                "Founder",
                [0, 20, 0],
                "Parent Realm",
                SettlementKind::Kingdom,
                default(),
                &rules,
                &[],
            )
            .unwrap();
        let children = [
            (SettlementKind::Village, [4, 0], "Village"),
            (SettlementKind::City, [8, 0], "City"),
            (SettlementKind::Outpost, [12, 0], "Outpost"),
        ]
        .map(|(kind, chunk, name)| {
            if kind != SettlementKind::Outpost {
                session
                    .kingdoms
                    .claim_with_rules(
                        main,
                        chunk,
                        ClaimKind::RealmTerritory,
                        ClaimActor::Player(actor),
                        &rules,
                        &[],
                    )
                    .unwrap();
            }
            let id = session
                .kingdoms
                .found_settlement_with_rules(
                    actor,
                    "Founder",
                    [chunk[0] * 16, 20, chunk[1] * 16],
                    name,
                    kind,
                    default(),
                    &rules,
                    &[],
                )
                .unwrap();
            (id, chunk)
        });
        session
            .kingdoms
            .claim_with_rules(
                main,
                [5, 0],
                ClaimKind::RealmTerritory,
                ClaimActor::Player(actor),
                &rules,
                &[],
            )
            .unwrap();
        let foreign = session
            .kingdoms
            .found_settlement_with_rules(
                terra_core::PlayerUuid::from_u128(202_552),
                "Other founder",
                [1024, 20, 0],
                "Other Realm",
                SettlementKind::Kingdom,
                default(),
                &rules,
                &[],
            )
            .unwrap();
        HierarchicalClaimFixture {
            session,
            rules,
            main,
            children,
            foreign,
        }
    }

    #[test]
    fn claim_management_selects_real_children_and_rejects_foreign_or_missing_targets() {
        use terra_core::kingdom::KingdomId;
        let HierarchicalClaimFixture {
            session,
            main,
            children,
            foreign,
            ..
        } = hierarchical_claim_session();
        let mut state = FullMapState::default();
        for (child, chunk) in children {
            state.diplomacy_chunk = Some(chunk);
            assert_eq!(
                managed_settlement(&session, &state, [0.0; 3]).unwrap().id,
                child
            );
            state.diplomacy_chunk = None;
            let player = [
                chunk[0] as f64 * 16.0 + 8.0,
                20.0,
                chunk[1] as f64 * 16.0 + 8.0,
            ];
            assert_eq!(
                managed_settlement(&session, &state, player).unwrap().id,
                child
            );
        }
        state.diplomacy_chunk = Some(children[1].1);
        state.claim_settlement = Some(children[0].0);
        assert_eq!(
            managed_settlement(&session, &state, [192.0, 20.0, 0.0])
                .unwrap()
                .id,
            children[0].0
        );
        for invalid in [foreign, KingdomId(u64::MAX)] {
            state.claim_settlement = Some(invalid);
            assert_eq!(
                managed_settlement(&session, &state, [0.0; 3]).unwrap().id,
                children[1].0
            );
        }
        state.diplomacy_chunk = Some([64, 0]);
        assert_eq!(
            managed_settlement(&session, &state, [64.0, 20.0, 0.0])
                .unwrap()
                .id,
            children[0].0
        );
        assert_eq!(
            managed_settlement(&session, &state, [1024.0, 20.0, 0.0])
                .unwrap()
                .id,
            main
        );
        let mut anonymous = session;
        anonymous
            .kingdoms
            .dissolve(anonymous.uuid.unwrap(), children[0].0)
            .unwrap();
        state.claim_settlement = Some(children[0].0);
        state.diplomacy_chunk = None;
        assert_eq!(
            managed_settlement(&anonymous, &state, [64.0, 20.0, 0.0])
                .unwrap()
                .id,
            main,
            "an old draft cannot keep targeting a dissolved settlement"
        );
        anonymous.uuid = None;
        assert!(managed_settlement(&anonymous, &state, [0.0; 3]).is_none());
    }

    #[test]
    fn child_claim_draft_sends_its_original_settlement_and_cancel_releases_selection() {
        #[derive(Resource)]
        struct ClaimControl(&'static str);
        let HierarchicalClaimFixture {
            session,
            rules,
            children,
            ..
        } = hierarchical_claim_session();
        let before = session.kingdoms.clone();
        let mut app = App::new();
        app.insert_resource(session)
            .insert_resource(FullMapState {
                diplomacy: true,
                diplomacy_chunk: Some(children[0].1),
                claim_rules: rules,
                ..default()
            })
            .insert_resource(ClaimControl("fullmap-claim-protected"))
            .add_message::<KingdomRequest>()
            .add_systems(
                Update,
                |mut state: ResMut<FullMapState>,
                 mut session: ResMut<ClientSession>,
                 control: Res<ClaimControl>,
                 mut writer: MessageWriter<KingdomRequest>| {
                    request_claim(
                        &mut state,
                        control.0,
                        [128.0, 20.0, 0.0],
                        Some(&mut session),
                        &mut writer,
                        1.0,
                    );
                },
            );
        app.update();
        assert_eq!(
            app.world().resource::<FullMapState>().claim_settlement,
            Some(children[0].0)
        );
        assert!(
            app.world_mut()
                .resource_mut::<Messages<KingdomRequest>>()
                .drain()
                .next()
                .is_none()
        );
        {
            let mut state = app.world_mut().resource_mut::<FullMapState>();
            state.diplomacy_chunk = Some(children[1].1);
            state
                .claim_draft
                .insert([5, 0], ClaimKind::ProtectedBuilding);
        }
        app.world_mut().resource_mut::<ClaimControl>().0 = "fullmap-claim-apply";
        app.update();
        let sent = app
            .world_mut()
            .resource_mut::<Messages<KingdomRequest>>()
            .drain()
            .collect::<Vec<_>>();
        assert_eq!(sent.len(), 1);
        assert_eq!(
            sent[0].0.action,
            KingdomAction::ClaimBatch {
                id: children[0].0,
                claims: vec![([5, 0], ClaimKind::ProtectedBuilding)],
            }
        );
        assert_eq!(app.world().resource::<ClientSession>().kingdoms, before);
        let mut authoritative = before;
        authoritative
            .claim_with_rules(
                children[0].0,
                [5, 0],
                ClaimKind::ProtectedBuilding,
                ClaimActor::Player(app.world().resource::<ClientSession>().uuid.unwrap()),
                &rules,
                &[],
            )
            .unwrap();
        app.world_mut()
            .resource_mut::<FullMapState>()
            .diplomacy_pending = None;
        app.world_mut().resource_mut::<ClaimControl>().0 = "fullmap-claim-cancel";
        app.update();
        let state = app.world().resource::<FullMapState>();
        assert!(
            state.claim_settlement.is_none()
                && state.claim_draft.is_empty()
                && state.claim_rectangle.is_none()
        );
        app.world_mut().resource_mut::<ClaimControl>().0 = "fullmap-claim-protected";
        app.update();
        assert_eq!(
            app.world().resource::<FullMapState>().claim_settlement,
            Some(children[1].0)
        );
    }

    #[test]
    fn unavailable_locked_settlement_cannot_redirect_a_claim_draft_to_another_realm() {
        for unavailable in ["dissolved", "missing", "foreign"] {
            let HierarchicalClaimFixture {
                mut session,
                rules,
                children,
                foreign,
                ..
            } = hierarchical_claim_session();
            let child = children[0].0;
            let locked = if unavailable == "foreign" {
                foreign
            } else {
                session
                    .kingdoms
                    .dissolve(session.uuid.unwrap(), child)
                    .unwrap();
                if unavailable == "missing" {
                    session
                        .kingdoms
                        .kingdoms
                        .retain(|record| record.id != child);
                }
                child
            };
            assert!(session.kingdoms.valid());
            let before = session.kingdoms.clone();
            let draft = BTreeMap::from([([5, 0], ClaimKind::ProtectedBuilding)]);
            let mut app = App::new();
            app.insert_resource(session)
                .insert_resource(FullMapState {
                    diplomacy: true,
                    diplomacy_chunk: Some(children[1].1),
                    claim_settlement: Some(locked),
                    claim_mode: Some(ClaimKind::ProtectedBuilding),
                    claim_draft: draft.clone(),
                    claim_rules: rules,
                    ..default()
                })
                .add_message::<KingdomRequest>()
                .add_systems(
                    Update,
                    |mut state: ResMut<FullMapState>,
                     mut session: ResMut<ClientSession>,
                     mut writer: MessageWriter<KingdomRequest>| {
                        request_claim(
                            &mut state,
                            "fullmap-claim-apply",
                            [0.0, 20.0, 0.0],
                            Some(&mut session),
                            &mut writer,
                            1.0,
                        );
                    },
                );
            assert_ne!(
                managed_settlement(
                    app.world().resource::<ClientSession>(),
                    app.world().resource::<FullMapState>(),
                    [0.0; 3],
                )
                .unwrap()
                .id,
                locked,
                "the ordinary view can still fall back to another active settlement"
            );
            app.update();
            assert!(
                app.world_mut()
                    .resource_mut::<Messages<KingdomRequest>>()
                    .drain()
                    .next()
                    .is_none(),
                "{unavailable}: applying must not reinterpret the locked draft's owner"
            );
            let state = app.world().resource::<FullMapState>();
            assert!(state.diplomacy_pending.is_none());
            assert_eq!(state.claim_settlement, Some(locked));
            assert_eq!(state.claim_draft, draft);
            assert!(state.diplomacy_status.contains("no longer available"));
            assert_eq!(app.world().resource::<ClientSession>().kingdoms, before);
        }
    }

    #[test]
    fn child_claim_limits_use_the_confirmed_world_rule_snapshot() {
        let HierarchicalClaimFixture {
            session,
            rules,
            children,
            ..
        } = hierarchical_claim_session();
        for ((id, chunk), expected) in children.into_iter().zip([47, 59, 71]) {
            let mut state = FullMapState {
                diplomacy: true,
                diplomacy_chunk: Some(chunk),
                claim_rules: rules,
                ..default()
            };
            let mut view = FullMapView::default();
            diplomacy_view(&mut view, &state, Some(&session), None, [0.0; 3]);
            assert!(
                view.diplomacy_limits
                    .contains(&format!("Building 1 / {expected}")),
                "{}",
                view.diplomacy_limits
            );
            assert!(
                view.diplomacy_limits
                    .starts_with(&session.kingdoms.get(id).unwrap().name)
            );
            state.claim_rules = Default::default();
            diplomacy_view(&mut view, &state, Some(&session), None, [0.0; 3]);
            let fallback = session
                .kingdoms
                .limits_with_rules(id, &state.claim_rules)
                .protected;
            assert!(
                view.diplomacy_limits
                    .contains(&format!("Building 1 / {fallback}"))
            );
            assert_ne!(expected, fallback);
        }
    }

    #[test]
    fn diplomacy_labels_all_relations_live_and_resolves_known_native_capture_and_dissolution() {
        use terra_core::{
            PlayerUuid,
            kingdom::{BotKingdom, DiplomaticRelation as R, DomainKind, SettlementDomain},
        };
        let viewer = PlayerUuid::from_u128(201);
        let allied = PlayerUuid::from_u128(202);
        let hostile = PlayerUuid::from_u128(203);
        let neutral = PlayerUuid::from_u128(204);
        let mut session = ClientSession::default();
        session.uuid = Some(viewer);
        let own = session
            .kingdoms
            .found(viewer, [0, 20, 0], "Homewatch")
            .unwrap();
        let ally = session
            .kingdoms
            .found(allied, [32, 20, 0], "Eastbank")
            .unwrap();
        let enemy = session
            .kingdoms
            .found(hostile, [128, 20, 0], "Ironhold")
            .unwrap();
        session
            .kingdoms
            .found(neutral, [192, 20, 0], "Farplain")
            .unwrap();
        session
            .kingdoms
            .set_relation(viewer, own, DiplomaticTarget::Player(ally), R::Allied)
            .unwrap();
        session
            .kingdoms
            .set_relation(allied, ally, DiplomaticTarget::Player(own), R::Allied)
            .unwrap();
        session
            .kingdoms
            .set_relation(hostile, enemy, DiplomaticTarget::Player(own), R::Hostile)
            .unwrap();
        let mut known = crate::settlement_map::SettlementMap::default();
        known.remember(SettlementDomain {
            id: 901,
            center: [56, 20, 8],
            bounds: [[48, 0, 0], [79, 40, 31]],
            kind: DomainKind::Outpost,
            native: Some(BotKingdom::Helteria),
        });
        let mut state = FullMapState {
            diplomacy: true,
            span: 256,
            ..default()
        };
        let mut view = FullMapView::default();
        for (chunk, label) in [
            ([0, 0], "own"),
            ([2, 0], "allied"),
            ([8, 0], "hostile"),
            ([12, 0], "neutral"),
        ] {
            state.diplomacy_chunk = Some(chunk);
            diplomacy_view(
                &mut view,
                &state,
                Some(&session),
                Some(&known),
                [0.0, 20.0, 0.0],
            );
            assert_eq!(view.diplomacy_relation_key, label);
            assert_eq!(
                view.diplomacy_legends
                    .iter()
                    .map(|r| r.relation.as_str())
                    .collect::<std::collections::BTreeSet<_>>(),
                std::collections::BTreeSet::from(["own", "allied", "hostile", "neutral"])
            );
        }
        state.diplomacy_chunk = Some([2, 0]);
        diplomacy_view(&mut view, &state, Some(&session), Some(&known), [0.0; 3]);
        let owner_color = view.diplomacy_color.clone();
        session
            .kingdoms
            .set_relation(viewer, own, DiplomaticTarget::Player(ally), R::Hostile)
            .unwrap();
        diplomacy_view(&mut view, &state, Some(&session), Some(&known), [0.0; 3]);
        assert_eq!(view.diplomacy_relation_key, "hostile");
        assert_eq!(view.diplomacy_color, owner_color);
        session
            .kingdoms
            .set_relation(viewer, own, DiplomaticTarget::Player(ally), R::Allied)
            .unwrap();
        session
            .kingdoms
            .set_relation(
                viewer,
                own,
                DiplomaticTarget::Bot(BotKingdom::Helteria),
                R::Hostile,
            )
            .unwrap();
        state.diplomacy_chunk = Some([4, 0]);
        diplomacy_view(&mut view, &state, Some(&session), Some(&known), [0.0; 3]);
        assert_eq!(
            (
                view.diplomacy_name.as_str(),
                view.diplomacy_relation_key.as_str()
            ),
            ("Helteria", "hostile")
        );
        session
            .kingdoms
            .claim(
                ally,
                [3, 0],
                ClaimKind::RealmTerritory,
                ClaimActor::Player(allied),
            )
            .unwrap();
        diplomacy_view(&mut view, &state, Some(&session), Some(&known), [0.0; 3]);
        assert_eq!(
            (
                view.diplomacy_name.as_str(),
                view.diplomacy_relation_key.as_str()
            ),
            ("Eastbank", "allied")
        );
        session.kingdoms.dissolve(allied, ally).unwrap();
        diplomacy_view(&mut view, &state, Some(&session), Some(&known), [0.0; 3]);
        assert_eq!(
            (
                view.diplomacy_name.as_str(),
                view.diplomacy_relation_key.as_str()
            ),
            ("Helteria", "hostile")
        );
        session.uuid = None;
        diplomacy_view(&mut view, &state, Some(&session), Some(&known), [0.0; 3]);
        assert_eq!(view.diplomacy_relation, "Friendly");
    }

    #[test]
    fn diplomacy_displays_owner_sends_authority_commands_and_preserves_map_without_world_loads() {
        let mut app = App::new();
        app.init_resource::<Atlas>()
            .init_resource::<GameplayConfig>()
            .init_resource::<ChunkStore>()
            .init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<ButtonInput<MouseButton>>()
            .init_resource::<terra_player::PlayerInputBlock>()
            .init_resource::<crate::menu::MenuState>()
            .add_message::<terra_player::CaptureRequest>()
            .add_message::<MouseWheel>();
        let uuid = terra_core::PlayerUuid::from_u128(55);
        let mut session = ClientSession::default();
        session.ready = true;
        session.uuid = Some(uuid);
        session.allow_build = true;
        let id = session
            .kingdoms
            .found_with_name(uuid, "Alice", [0, 0, 0], "Northwatch")
            .unwrap();
        let before = session.kingdoms.clone();
        app.insert_resource(session);
        app.world_mut()
            .resource_mut::<crate::menu::MenuState>()
            .page = crate::menu::MenuPage::Playing;
        app.world_mut()
            .spawn((Window::default(), bevy::window::PrimaryWindow));
        install(&mut app);
        {
            let mut state = app.world_mut().resource_mut::<FullMapState>();
            state.open = true;
            state.span = 256;
            state.diplomacy_chunk = Some([0, 0]);
        }
        app.world_mut().write_message(FullMapAction::Command {
            control: "fullmap-diplomacy".into(),
            id: None,
        });
        app.update();
        let view = app.world().resource::<FullMapView>();
        assert!(view.diplomacy && view.visible);
        assert_eq!(view.diplomacy_owner, "Founder: Alice");
        assert_eq!(view.diplomacy_name, "Northwatch");
        assert_eq!(view.diplomacy_relation, "Your realm");
        let mut sites = crate::settlement_map::SettlementMap::default();
        sites.remember(terra_core::kingdom::SettlementDomain {
            id: 70,
            center: [24, 20, 8],
            bounds: [[16, 0, 0], [47, 40, 31]],
            kind: terra_core::kingdom::DomainKind::Village,
            native: Some(terra_core::kingdom::BotKingdom::Helteria),
        });
        app.insert_resource(sites);
        app.world_mut()
            .resource_mut::<FullMapState>()
            .diplomacy_chunk = Some([1, 0]);
        app.update();
        assert_eq!(
            app.world().resource::<FullMapView>().diplomacy_name,
            "Helteria"
        );
        assert_eq!(
            app.world().resource::<FullMapView>().diplomacy_relation,
            "Neutral"
        );
        assert!(
            app.world()
                .resource::<FullMapView>()
                .diplomacy_kind
                .contains("Village")
        );
        app.world_mut()
            .resource_mut::<FullMapState>()
            .diplomacy_chunk = Some([0, 0]);
        app.world_mut().spawn((
            Node::default(),
            ElementId("full-map-viewport".into()),
            ComputedNode {
                size: Vec2::splat(256.0),
                ..default()
            },
            UiGlobalTransform::from_xy(128.0, 128.0),
        ));
        app.world_mut().spawn((
            Node::default(),
            ElementId("full-map-diplomacy".into()),
            ComputedNode {
                size: Vec2::new(64.0, 256.0),
                ..default()
            },
            UiGlobalTransform::from_xy(224.0, 128.0),
        ));
        for mut window in app
            .world_mut()
            .query::<&mut Window>()
            .iter_mut(app.world_mut())
        {
            window.focused = true;
            window.set_physical_cursor_position(Some(bevy::math::DVec2::new(200.0, 128.0)));
        }
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Left);
        app.update();
        assert_eq!(
            app.world().resource::<FullMapState>().diplomacy_chunk,
            Some([0, 0]),
            "Sidebar clicks must not select the map beneath them"
        );
        app.world_mut()
            .insert_resource(ButtonInput::<MouseButton>::default());

        app.world_mut()
            .resource_mut::<FullMapState>()
            .diplomacy_chunk = Some([1, 0]);
        app.world_mut().write_message(FullMapAction::Command {
            control: "fullmap-claim-territory".into(),
            id: None,
        });
        app.update();
        assert!(
            app.world_mut()
                .resource_mut::<Messages<KingdomRequest>>()
                .drain()
                .next()
                .is_none(),
            "Choosing a paint mode must not claim anything"
        );
        app.world_mut()
            .resource_mut::<GameplayConfig>()
            .minimap
            .relief = false;
        for mut window in app
            .world_mut()
            .query::<&mut Window>()
            .iter_mut(app.world_mut())
        {
            window.set_physical_cursor_position(Some(bevy::math::DVec2::new(145.0, 129.0)));
        }
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Left);
        app.update();
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .clear();
        for mut window in app
            .world_mut()
            .query::<&mut Window>()
            .iter_mut(app.world_mut())
        {
            window.set_physical_cursor_position(Some(bevy::math::DVec2::new(161.0, 145.0)));
        }
        app.update();
        assert_eq!(
            app.world()
                .resource::<FullMapState>()
                .preview_claims()
                .len(),
            4
        );
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .release(MouseButton::Left);
        app.update();
        {
            let mut state = app.world_mut().resource_mut::<FullMapState>();
            assert_eq!(state.claim_draft.len(), 4);
            let before = state.claim_draft.clone();
            stage_rectangle(&mut state, [-100, -100], [100, 100]);
            assert_eq!(
                state.claim_draft, before,
                "Oversized gestures must preserve the selection"
            );
        }
        app.world_mut().write_message(FullMapAction::Command {
            control: "fullmap-claim-apply".into(),
            id: None,
        });
        app.update();
        let requests = app
            .world_mut()
            .resource_mut::<Messages<KingdomRequest>>()
            .drain()
            .collect::<Vec<_>>();
        assert_eq!(requests.len(), 1);
        assert_eq!(
            requests[0].0.action,
            KingdomAction::ClaimBatch {
                id,
                claims: vec![
                    ([1, 0], ClaimKind::RealmTerritory),
                    ([1, 1], ClaimKind::RealmTerritory),
                    ([2, 0], ClaimKind::RealmTerritory),
                    ([2, 1], ClaimKind::RealmTerritory)
                ]
            }
        );
        assert_eq!(app.world().resource::<ClientSession>().kingdoms, before);
        assert!(app.world().resource::<FullMapView>().diplomacy_pending);
        app.world_mut().write_message(FullMapAction::Command {
            control: "fullmap-diplomacy-close".into(),
            id: None,
        });
        app.update();
        assert!(!app.world().resource::<FullMapView>().diplomacy);
        assert_eq!(app.world().resource::<FullMapState>().span, 256);
        assert!(app.world().resource::<ChunkStore>().loaded.is_empty());
    }

    #[test]
    fn cave_layers_never_enable_relief_or_the_live_three_dimensional_camera() {
        let mut state = FullMapState {
            open: true,
            three_dimensional: true,
            ..default()
        };
        assert!(state.three_dimensional_active());
        assert!(relief_enabled(true, MapLayer::Surface));
        state.enforce_layer(MapLayer::Cave { level: -24 });
        assert!(!state.three_dimensional);
        assert!(!state.three_dimensional_active());
        assert!(!relief_enabled(true, state.resolved_layer()));
        state.three_dimensional = true;
        assert!(!state.three_dimensional_active());
        state.enforce_layer(MapLayer::Surface);
        state.layer = Some(MapLayer::Cave { level: 80 });
        assert!(!state.three_dimensional_active());
        state.enforce_layer(MapLayer::Surface);
        assert!(!state.three_dimensional);
        state.layer = Some(MapLayer::Surface);
        state.enforce_layer(MapLayer::Surface);
        assert!(!state.three_dimensional_active());
    }

    #[test]
    fn automatic_cave_switch_and_queued_mode_commands_fall_back_to_flat_two_dimensions() {
        let mut app = App::new();
        app.init_resource::<Atlas>()
            .init_resource::<GameplayConfig>()
            .init_resource::<ChunkStore>()
            .init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<ButtonInput<MouseButton>>()
            .init_resource::<terra_player::PlayerInputBlock>()
            .init_resource::<crate::menu::MenuState>()
            .add_message::<terra_player::CaptureRequest>()
            .add_message::<MouseWheel>();
        let mut session = ClientSession::default();
        session.ready = true;
        app.insert_resource(session);
        app.world_mut()
            .resource_mut::<crate::menu::MenuState>()
            .page = crate::menu::MenuPage::Playing;
        app.world_mut().resource_mut::<Atlas>().display = Some([32.0, -20.0, 48.0]);
        app.world_mut()
            .spawn((Window::default(), bevy::window::PrimaryWindow));
        install(&mut app);
        {
            let mut state = app.world_mut().resource_mut::<FullMapState>();
            state.open = true;
            state.three_dimensional = true;
        }
        app.world_mut()
            .resource_mut::<GameplayConfig>()
            .minimap
            .mode = terra_core::MapMode::Cave;
        app.world_mut().write_message(FullMapAction::Command {
            control: "fullmap-mode".into(),
            id: None,
        });
        app.update();
        let state = app.world().resource::<FullMapState>();
        assert_eq!(state.layer, None);
        assert_eq!(state.resolved_layer(), MapLayer::Cave { level: -20 });
        assert!(!state.three_dimensional);
        assert!(!state.three_dimensional_active());
        let view = app.world().resource::<FullMapView>();
        assert!(view.visible && view.cave);
        assert!(!view.three_dimensional_available);
        assert!(!view.three_dimensional);

        app.world_mut()
            .resource_mut::<GameplayConfig>()
            .minimap
            .mode = terra_core::MapMode::Surface;
        for control in ["fullmap-mode", "fullmap-layer", "fullmap-mode"] {
            app.world_mut().write_message(FullMapAction::Command {
                control: control.into(),
                id: None,
            });
        }
        app.update();
        let state = app.world().resource::<FullMapState>();
        assert_eq!(state.layer, Some(MapLayer::Cave { level: -20 }));
        assert!(!state.three_dimensional_active());
        assert!(!state.three_dimensional);
        assert!(
            !app.world()
                .resource::<FullMapView>()
                .three_dimensional_available
        );
    }

    #[test]
    fn map_tool_rail_is_icon_only_with_hover_tooltips_for_all_eight_actions() {
        let template = include_str!("../../../../src-ui/components/full-map.component.html");
        let rail = template
            .split("<div class=\"full-map-tool-rail\">")
            .nth(1)
            .unwrap()
            .split("</div>")
            .next()
            .unwrap();
        assert_eq!(rail.matches("<button ").count(), 8);
        assert_eq!(rail.matches("<icon ").count(), 8);
        assert!(!rail.contains("<p>"));
        assert!(!rail.contains("map-icon-button"));
        for action in [
            "add",
            "grid",
            "layer",
            "mobs",
            "mode",
            "center",
            "list",
            "diplomacy",
        ] {
            assert!(template.contains(&format!(
                "<tooltip for=\"fullmap-{action}\" trigger=\"hover\""
            )));
        }
        assert!(rail.contains("[disabled]=\"!fullMapView.three_dimensional_available\""));
    }

    #[test]
    fn claim_drag_includes_the_release_frame_endpoint_without_sending_a_command() {
        let mut app = App::new();
        app.init_resource::<Atlas>()
            .init_resource::<GameplayConfig>()
            .init_resource::<ChunkStore>()
            .init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<ButtonInput<MouseButton>>()
            .init_resource::<terra_player::PlayerInputBlock>()
            .init_resource::<crate::menu::MenuState>()
            .add_message::<terra_player::CaptureRequest>()
            .add_message::<MouseWheel>();
        let uuid = terra_core::PlayerUuid::from_u128(651);
        let mut session = ClientSession::default();
        session.ready = true;
        session.uuid = Some(uuid);
        session.allow_build = true;
        session
            .kingdoms
            .found(uuid, [0, 0, 0], "Northwatch")
            .unwrap();
        let saved = session.kingdoms.clone();
        app.insert_resource(session);
        app.world_mut()
            .resource_mut::<crate::menu::MenuState>()
            .page = crate::menu::MenuPage::Playing;
        app.world_mut()
            .resource_mut::<GameplayConfig>()
            .minimap
            .relief = false;
        let mut window = Window {
            focused: true,
            ..default()
        };
        window.set_physical_cursor_position(Some(bevy::math::DVec2::new(145.0, 129.0)));
        let window = app
            .world_mut()
            .spawn((window, bevy::window::PrimaryWindow))
            .id();
        app.world_mut().spawn((
            ElementId("full-map-viewport".into()),
            ComputedNode {
                size: Vec2::splat(256.0),
                ..default()
            },
            UiGlobalTransform::from_xy(128.0, 128.0),
        ));
        install(&mut app);
        {
            let mut state = app.world_mut().resource_mut::<FullMapState>();
            state.open = true;
            state.diplomacy = true;
            state.span = 256;
            state.claim_mode = Some(ClaimKind::RealmTerritory);
        }
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Left);
        app.update();
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .clear();
        app.world_mut()
            .get_mut::<Window>(window)
            .unwrap()
            .set_physical_cursor_position(Some(bevy::math::DVec2::new(161.0, 145.0)));
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .release(MouseButton::Left);
        app.update();
        let state = app.world().resource::<FullMapState>();
        assert!(state.claim_rectangle.is_none());
        assert_eq!(
            state.claim_draft.keys().copied().collect::<Vec<_>>(),
            vec![[1, 0], [1, 1], [2, 0], [2, 1]]
        );
        assert_eq!(app.world().resource::<ClientSession>().kingdoms, saved);
        assert!(app.world().resource::<ChunkStore>().loaded.is_empty());
        assert!(
            app.world_mut()
                .resource_mut::<Messages<KingdomRequest>>()
                .drain()
                .next()
                .is_none()
        );
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .clear();
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Right);
        app.update();
        let state = app.world().resource::<FullMapState>();
        assert_eq!(
            state.claim_draft.keys().copied().collect::<Vec<_>>(),
            vec![[1, 0], [1, 1], [2, 0]],
            "Right-click removes only the hovered draft chunk"
        );
        assert_eq!(
            app.world().resource::<ClientSession>().kingdoms,
            saved,
            "Unselecting must never release saved ownership"
        );
        assert!(app.world().resource::<ChunkStore>().loaded.is_empty());
    }

    #[test]
    fn relief_pointer_inverts_cached_slopes_across_chunk_edges_at_each_aspect() {
        use terra_world::cartography::{MapRelief, MapTile};
        let mut atlas = Atlas::default();
        atlas.full.relief_base = Some((MapLayer::Surface, 80));
        atlas.entries.insert(
            ([-1, 0], MapLayer::Surface),
            super::super::Entry {
                tile: Some(std::sync::Arc::new(MapTile {
                    coordinate: [-1, 0],
                    pixels: [100; 1024],
                    relief: Some(Box::new(MapRelief {
                        heights: std::array::from_fn(|index| 120 + (index / 16) as i16 * 2),
                        water: [0; 32],
                    })),
                })),
                detail: None,
                overview: None,
                source_revision: None,
                palette_fingerprint: terra_core::MapPalette::default().fingerprint(),
                used: 0,
            },
        );
        let state = FullMapState {
            span: 96,
            ..default()
        };
        for aspect in [0.5, 1.0, 2.0] {
            let projection =
                super::super::raster::Projection::at([-8.0, 8.0], 96, 0.0, false, true, 640, false)
                    .with_aspect(aspect);
            for z in [0.25_f32, 7.25, 15.75] {
                let height = 120.0 + z.floor() * 2.0;
                let uv = Vec2::from_array(projection.project([-8.25, height, z], 80)) / 640.0;
                let position = pick_position(
                    &atlas,
                    &state,
                    MapLayer::Surface,
                    [-8.0, 8.0],
                    uv,
                    aspect,
                    true,
                    80.0,
                );
                assert!((position[0] + 8.25).abs() < 0.001, "{position:?}");
                assert!(
                    (position[1] - f64::from(height)).abs() < 0.001,
                    "{position:?}"
                );
                assert!((position[2] - f64::from(z)).abs() < 0.001, "{position:?}");
                assert_eq!(claim_chunk(position.map(|v| v.floor() as i64)), [-1, 0]);
            }
        }
        let unknown = pick_position(
            &atlas,
            &state,
            MapLayer::Surface,
            [512.0, 512.0],
            Vec2::splat(0.5),
            1.0,
            true,
            80.0,
        );
        assert_eq!(unknown, [512.0, 80.0, 512.0]);
        assert_eq!(
            atlas.entries.len(),
            1,
            "Picking must never request or invent map tiles"
        );
    }

    #[test]
    fn three_dimensional_pointer_selects_explored_elevation_not_the_flat_plane() {
        use terra_world::cartography::{MapRelief, MapTile};
        let mut atlas = Atlas::default();
        atlas.full.relief_base = Some((MapLayer::Surface, 80));
        let relief = MapRelief {
            heights: [90; 256],
            water: [0; 32],
        };
        atlas.entries.insert(
            ([0, 0], MapLayer::Surface),
            super::super::Entry {
                tile: Some(std::sync::Arc::new(MapTile {
                    coordinate: [0, 0],
                    pixels: [100; 1024],
                    relief: Some(Box::new(relief)),
                })),
                detail: None,
                overview: None,
                source_revision: None,
                palette_fingerprint: terra_core::MapPalette::default().fingerprint(),
                used: 0,
            },
        );
        let state = FullMapState {
            span: 96,
            three_dimensional: true,
            ..default()
        };
        let projection =
            super::super::raster::Projection::at([8.0, 8.0], 96, 0.0, false, true, 640, false)
                .with_aspect(0.5)
                .three_dimensional(true);
        let uv = Vec2::from_array(projection.project([8.0, 90.0, 8.0], 80)) / 640.0;
        let position = pick_position(
            &atlas,
            &state,
            MapLayer::Surface,
            [8.0, 8.0],
            uv,
            0.5,
            true,
            80.0,
        );
        assert!((position[0] - 8.0).abs() < 0.001);
        assert!((position[1] - 90.0).abs() < 0.001);
        assert!((position[2] - 8.0).abs() < 0.001);
    }

    #[test]
    fn held_drag_keeps_its_screen_anchor_through_pose_updates_and_ui_relayout() {
        assert_held_drag(terra_core::MapMode::Surface, 0.88);
    }

    #[test]
    fn cave_drag_survives_layer_enforcement_and_rejected_three_dimensional_commands() {
        assert_held_drag(terra_core::MapMode::Cave, 1.0);
    }

    fn assert_held_drag(mode: terra_core::MapMode, tilt: f64) {
        let mut app = App::new();
        app.init_resource::<Atlas>()
            .init_resource::<GameplayConfig>()
            .init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<ButtonInput<MouseButton>>()
            .init_resource::<terra_player::PlayerInputBlock>()
            .init_resource::<crate::menu::MenuState>()
            .add_message::<terra_player::CaptureRequest>()
            .add_message::<MouseWheel>();
        let mut session = ClientSession::default();
        session.ready = true;
        app.insert_resource(session);
        app.world_mut()
            .resource_mut::<crate::menu::MenuState>()
            .page = crate::menu::MenuPage::Playing;
        app.world_mut()
            .resource_mut::<GameplayConfig>()
            .minimap
            .relief = true;
        app.world_mut()
            .resource_mut::<GameplayConfig>()
            .minimap
            .mode = mode;
        app.world_mut().resource_mut::<Atlas>().display = Some([100.0, 80.0, 200.0]);
        let mut window = Window {
            focused: true,
            ..default()
        };
        window.set_physical_cursor_position(Some(bevy::math::DVec2::splat(300.0)));
        let window = app
            .world_mut()
            .spawn((window, bevy::window::PrimaryWindow))
            .id();
        let viewport = app
            .world_mut()
            .spawn((
                ElementId("full-map-viewport".into()),
                ComputedNode {
                    size: Vec2::splat(400.0),
                    ..default()
                },
                UiGlobalTransform::from_translation(Vec2::splat(300.0)),
            ))
            .id();
        install(&mut app);
        app.world_mut().resource_mut::<FullMapState>().open = true;
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Left);
        app.update();
        assert_eq!(
            app.world().resource::<FullMapState>().center,
            Some([100.0, 200.0])
        );
        for tick in 1..=6 {
            app.world_mut()
                .resource_mut::<ButtonInput<MouseButton>>()
                .clear();
            app.world_mut().resource_mut::<Atlas>().display =
                Some([100.0 + f64::from(tick), 80.0, 200.0 + f64::from(tick)]);
            let delta = Vec2::new(tick as f32 * 20.0, tick as f32 * -8.0);
            app.world_mut()
                .get_mut::<Window>(window)
                .unwrap()
                .set_physical_cursor_position(Some((Vec2::splat(300.0) + delta).as_dvec2()));
            app.world_mut()
                .get_mut::<ComputedNode>(viewport)
                .unwrap()
                .size = if tick == 2 {
                Vec2::ZERO
            } else {
                Vec2::splat(400.0)
            };
            *app.world_mut()
                .get_mut::<UiGlobalTransform>(viewport)
                .unwrap() = UiGlobalTransform::from_translation(Vec2::splat(300.0 + tick as f32));
            if mode == terra_core::MapMode::Cave && tick == 3 {
                app.world_mut().write_message(FullMapAction::Command {
                    control: "fullmap-mode".into(),
                    id: None,
                });
            }
            app.update();
            let center = app.world().resource::<FullMapState>().center.unwrap();
            assert!((center[0] - (100.0 - f64::from(delta.x) * 384.0 / 400.0)).abs() < 0.000_02);
            assert!(
                (center[1] - (200.0 - f64::from(delta.y) * 384.0 / 400.0 / tilt)).abs() < 0.000_02
            );
            if mode == terra_core::MapMode::Cave {
                assert!(
                    !app.world()
                        .resource::<FullMapView>()
                        .three_dimensional_available
                );
                assert!(
                    !app.world()
                        .resource::<FullMapState>()
                        .three_dimensional_active()
                );
            }
        }
        let panned = app.world().resource::<FullMapState>().center;
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .release(MouseButton::Left);
        app.world_mut().resource_mut::<Atlas>().display = Some([800.0, 140.0, -20.0]);
        app.update();
        assert_eq!(app.world().resource::<FullMapState>().center, panned);
        assert!(app.world().resource::<FullMapState>().drag.is_none());
    }

    #[test]
    fn map_styles_use_supported_tilt_ui_properties() {
        for source in [
            include_str!("../../../../src-ui/components/full-map.component.css"),
            include_str!("../../../../src-ui/components/waypoint-dialog.component.css"),
            include_str!("../../../../src-ui/components/mini-map.component.css"),
            include_str!("../../../../src-ui/components/waypoint-marker.component.css"),
            include_str!("../../../../src-ui/components/waypoint-symbol.component.css"),
            include_str!("../../../../src-ui/components/world-waypoints.component.css"),
        ] {
            tilt_ui::tilt_ui_css::parse_stylesheet(source).unwrap();
        }
    }

    #[test]
    fn map_templates_parse_with_the_real_loader_and_only_reference_registered_components() {
        use bevy::asset::{AssetApp, AssetPlugin, LoadState};
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_plugins(AssetPlugin {
                file_path: concat!(env!("CARGO_MANIFEST_DIR"), "/../../src-ui").into(),
                ..default()
            })
            .init_asset::<tilt_ui::UiTemplateAsset>()
            .register_asset_loader(tilt_ui::UiTemplateAssetLoader);
        let names = [
            "full-map",
            "waypoint-dialog",
            "waypoint-marker",
            "waypoint-symbol",
            "world-waypoints",
        ];
        let handles: Vec<_> = names
            .iter()
            .map(|name| {
                app.world()
                    .resource::<AssetServer>()
                    .load::<tilt_ui::UiTemplateAsset>(format!("components/{name}.component.html"))
            })
            .collect();
        let until = std::time::Instant::now() + std::time::Duration::from_secs(5);
        loop {
            app.update();
            if handles.iter().all(|handle| {
                app.world()
                    .resource::<Assets<tilt_ui::UiTemplateAsset>>()
                    .contains(handle.id())
            }) {
                break;
            }
            for (name, handle) in names.iter().zip(&handles) {
                if let Some(LoadState::Failed(error)) = app
                    .world()
                    .resource::<AssetServer>()
                    .get_load_state(handle.id())
                {
                    panic!("TiltUI could not parse {name}: {error}");
                }
            }
            assert!(
                std::time::Instant::now() < until,
                "Timed out loading real TiltUI map templates"
            );
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        let templates = app.world().resource::<Assets<tilt_ui::UiTemplateAsset>>();
        for (name, handle) in names.iter().zip(&handles) {
            for node in templates.get(handle).unwrap().template().nodes() {
                if let tilt_ui::TemplateNodeKind::Component(component) = &node.kind {
                    let control_flow = matches!(
                        component.as_str(),
                        "tilt-flow-if"
                            | "tilt-flow-then"
                            | "tilt-flow-else"
                            | "tilt-flow-for"
                            | "tilt-flow-match"
                            | "tilt-flow-case"
                            | "tilt-flow-let"
                    );
                    assert!(
                        control_flow || crate::tilt_ui_component_id(component.as_str()).is_some(),
                        "{name} references an unregistered component: {component}"
                    );
                }
            }
        }
    }

    #[test]
    fn map_picking_and_zoom_keep_the_pointer_anchored_at_negative_coordinates() {
        let center = [-1000.25, -340.75];
        let uv = Vec2::new(0.75, 0.25);
        let before = world_at(center, 384, uv, 0.88);
        assert_eq!(before[0], -904.25);
        assert!((before[1] - (-340.75 - 96.0 / 0.88)).abs() < 0.00001);
        let mut state = FullMapState::default();
        zoom_at(&mut state, center, uv, 4.0, 0.88);
        assert_eq!(state.span, 192);
        let after = world_at(state.viewing(center), state.span, uv, 0.88);
        assert!((after[0] - before[0]).abs() < 0.00001);
        assert!((after[1] - before[1]).abs() < 0.00001);
        zoom_at(&mut state, center, uv, 100.0, 0.88);
        assert_eq!(state.span, 96);
        zoom_at(&mut state, center, uv, -100.0, 0.88);
        assert_eq!(state.span, 4096);
    }

    #[test]
    fn fullmap_layout_stays_inside_small_scaled_and_large_viewports() {
        for canvas in [
            Vec2::new(640.0, 426.0),
            Vec2::new(960.0, 640.0),
            Vec2::new(2560.0, 1440.0),
        ] {
            let state = FullMapState {
                open: true,
                ..default()
            };
            let view = make_view(&state, [-12.0, 83.0, -34.0], canvas, true);
            assert!(view.visible && view.loading);
            assert_eq!(view.coordinates, "X -12  Y 83  Z -34");
            assert!(view.map_width + 156.0 <= canvas.x + 1.0);
            assert!(view.map_height + 168.0 <= canvas.y + 1.0);
            assert!(view.map_width > canvas.x * 0.7);
        }
    }

    #[test]
    fn fullmap_keyboard_blocks_gameplay_and_escape_is_consumed_without_opening_a_menu() {
        let mut app = App::new();
        app.init_resource::<Atlas>()
            .init_resource::<GameplayConfig>()
            .init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<ButtonInput<MouseButton>>()
            .init_resource::<terra_player::PlayerInputBlock>()
            .init_resource::<crate::inventory::InventoryState>()
            .init_resource::<crate::menu::MenuState>()
            .add_message::<terra_player::CaptureRequest>()
            .add_message::<MouseWheel>();
        let mut session = ClientSession::default();
        session.ready = true;
        app.insert_resource(session);
        app.world_mut()
            .resource_mut::<crate::menu::MenuState>()
            .page = crate::menu::MenuPage::Playing;
        app.world_mut()
            .spawn((Window::default(), bevy::window::PrimaryWindow));
        install(&mut app);
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::KeyM);
        app.update();
        assert!(app.world().resource::<FullMapState>().open);
        assert!(app.world().resource::<terra_player::PlayerInputBlock>().0);
        assert!(app.world().resource::<FullMapView>().visible);
        app.insert_resource(ButtonInput::<KeyCode>::default());
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::Escape);
        app.update();
        assert!(!app.world().resource::<FullMapState>().open);
        assert!(app.world().resource::<FullMapState>().consumed_escape);
        assert_eq!(
            app.world().resource::<crate::menu::MenuState>().page,
            crate::menu::MenuPage::Playing
        );
        assert!(!app.world().resource::<FullMapView>().visible);
        let captures: Vec<_> = app
            .world_mut()
            .resource_mut::<Messages<terra_player::CaptureRequest>>()
            .drain()
            .map(|event| event.0)
            .collect();
        assert_eq!(captures, [false, true]);

        app.insert_resource(ButtonInput::<KeyCode>::default());
        app.world_mut()
            .resource_mut::<crate::inventory::InventoryState>()
            .open = true;
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::KeyM);
        app.update();
        assert!(!app.world().resource::<FullMapState>().open);
        app.world_mut()
            .resource_mut::<crate::inventory::InventoryState>()
            .open = false;
        app.world_mut()
            .resource_mut::<crate::menu::MenuState>()
            .page = crate::menu::MenuPage::Options;
        app.insert_resource(ButtonInput::<KeyCode>::default());
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::KeyM);
        app.update();
        assert!(!app.world().resource::<FullMapState>().open);
    }

    #[test]
    fn waypoint_editor_only_commits_valid_successfully_saved_changes() {
        let root = std::env::temp_dir().join(format!(
            "terra-fullmap-editor-{}",
            terra_core::PlayerUuid::new_v4()
        ));
        let atlas = Atlas {
            root: root.clone(),
            ..default()
        };
        let mut state = FullMapState::default();
        let original = [-20.123456, 81.251234, 44.987654];
        begin_editor(&mut state, original, MapLayer::Surface);
        assert_eq!(
            state.editor.as_ref().unwrap().coordinates,
            ["-20,12", "81,25", "44,99"]
        );
        assert!(save_editor(&mut state, &atlas).is_err());
        assert!(state.waypoints.entries.is_empty());
        state.editor.as_mut().unwrap().name = "Home".into();
        state.editor.as_mut().unwrap().icon = WaypointIcon::Home;
        state.editor.as_mut().unwrap().show_in_world = true;
        save_editor(&mut state, &atlas).unwrap();
        assert!(state.editor.is_none());
        let saved = WaypointBook::load_directory(&root).unwrap();
        assert_eq!(state.waypoints, saved);
        assert_eq!(saved.entries[0].position, original);
        assert_eq!(saved.entries[0].icon, WaypointIcon::Home);
        assert!(saved.entries[0].show_in_world);
        state.editor = Some(Editor::from_point(&saved.entries[0]));
        assert_eq!(
            state.editor.as_ref().unwrap().coordinates,
            ["-20,12", "81,25", "44,99"]
        );
        state.editor.as_mut().unwrap().coordinates[0] = "65,89".into();
        save_editor(&mut state, &atlas).unwrap();
        let saved = WaypointBook::load_directory(&root).unwrap();
        assert_eq!(saved.entries[0].position, [65.89, original[1], original[2]]);
        state.editor = Some(Editor::from_point(&saved.entries[0]));
        state.editor.as_mut().unwrap().coordinates[0] = "NaN".into();
        assert!(save_editor(&mut state, &atlas).is_err());
        assert_eq!(state.waypoints, saved);
        state.editor.as_mut().unwrap().coordinates[0] = "48".into();
        std::fs::remove_file(root.join("waypoints.toml")).unwrap();
        std::fs::create_dir(root.join("waypoints.toml")).unwrap();
        assert!(save_editor(&mut state, &atlas).is_err());
        assert_eq!(state.waypoints, saved);
        assert!(state.editor.is_some());
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn quick_waypoint_is_an_independent_modal_with_world_visibility_and_catalog_picker() {
        let mut app = App::new();
        app.init_resource::<Atlas>()
            .init_resource::<GameplayConfig>()
            .init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<ButtonInput<MouseButton>>()
            .init_resource::<terra_player::PlayerInputBlock>()
            .init_resource::<crate::menu::MenuState>()
            .add_message::<terra_player::CaptureRequest>()
            .add_message::<MouseWheel>();
        let mut session = ClientSession::default();
        session.ready = true;
        app.insert_resource(session);
        app.world_mut()
            .resource_mut::<crate::menu::MenuState>()
            .page = crate::menu::MenuPage::Playing;
        let position = [100.125, 81.25, -204.0625];
        app.world_mut().resource_mut::<Atlas>().display = Some(position);
        app.world_mut()
            .spawn((Window::default(), bevy::window::PrimaryWindow));
        install(&mut app);
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::KeyN);
        app.update();
        let state = app.world().resource::<FullMapState>();
        assert!(!state.open && state.blocks_input());
        let editor = state.editor.as_ref().unwrap();
        assert_eq!(editor.coordinates, ["100,12", "81,25", "-204,06"]);
        assert_eq!(editor.original_position, position);
        assert!(editor.show_in_world);
        let view = app.world().resource::<FullMapView>();
        assert!(view.editing && !view.visible && view.show_in_world);
        app.insert_resource(ButtonInput::<KeyCode>::default());
        app.world_mut().write_message(FullMapAction::Command {
            control: "fullmap-icon".into(),
            id: None,
        });
        app.update();
        let view = app.world().resource::<FullMapView>();
        assert!(view.icon_picker);
        assert_eq!(view.icons.len(), tilt_ui::Icon::ALL.len());
        assert!(
            view.icons
                .iter()
                .all(|choice| tilt_ui::Icon::from_name(&choice.key).is_some())
        );
        app.world_mut().write_message(FullMapAction::Change {
            control: "fullmap-icon-search".into(),
            value: "compass".into(),
        });
        app.update();
        assert!(
            app.world()
                .resource::<FullMapView>()
                .icons
                .iter()
                .all(|choice| choice.key.contains("compass"))
        );
        app.world_mut().write_message(FullMapAction::Change {
            control: "fullmap-icon".into(),
            value: "compass".into(),
        });
        app.update();
        let view = app.world().resource::<FullMapView>();
        assert!(
            !view.icon_picker
                && view.icon == "compass"
                && view.icon_source == "tilt-icon:compass@32"
        );
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::Escape);
        app.update();
        assert!(!app.world().resource::<FullMapState>().blocks_input());
        assert!(app.world().resource::<FullMapState>().consumed_escape);
        assert!(!app.world().resource::<FullMapView>().editing);
    }
}
