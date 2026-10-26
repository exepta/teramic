//! Full-screen navigation and persistent annotations over the shared map atlas.
use super::Atlas;
use bevy::{
    ecs::system::SystemParam,
    input::mouse::{MouseScrollUnit, MouseWheel},
    input_focus::InputFocus,
    prelude::*,
    ui::UiGlobalTransform,
};
use terra_core::{GameAction, GameplayConfig, GameplayInput};
use terra_network::client::ClientSession;
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
            coordinates: point.position.map(|v| v.to_string()),
            cave_level: point.cave_level,
            color: point.color,
            icon: point.icon.clone(),
            show_in_world: point.show_in_world,
        }
    }
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
        .add_systems(
            Update,
            update
                .before(crate::menu::MenuInputSet)
                .before(super::update),
        );
}

#[derive(SystemParam)]
struct Context<'w, 's> {
    session: Option<Res<'w, ClientSession>>,
    menu: Option<Res<'w, crate::menu::MenuState>>,
    inventory: Option<Res<'w, crate::inventory::InventoryState>>,
    chat: Option<Res<'w, crate::chat::ChatState>>,
    pets: Option<Res<'w, crate::pets::PetMenuState>>,
    settings: Res<'w, GameplayConfig>,
    atlas: Res<'w, Atlas>,
    config: Option<Res<'w, terra_core::GameConfig>>,
    chunks: Option<Res<'w, ChunkStore>>,
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
    let inside =
        pointer.is_some_and(|uv| (0.0..=1.0).contains(&uv.x) && (0.0..=1.0).contains(&uv.y));
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
            if mouse.just_pressed(MouseButton::Right) && state.loaded {
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
    if !mouse.pressed(MouseButton::Left) || !window.focused {
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
    if projection.is_three_dimensional() {
        // Front-to-back orthographic ray through the explored heightfield only.
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
        coordinates: position.map(|v| v.to_string()),
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
            "fullmap-grid" => state.grid = !state.grid,
            "fullmap-mobs" => state.mobs = !state.mobs,
            "fullmap-mode" if state.resolved_layer() == MapLayer::Surface => {
                state.three_dimensional = !state.three_dimensional;
                state.drag = None;
            }
            "fullmap-list" => state.book_open = !state.book_open,
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
        position[index] = coordinate
            .trim()
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
    let map_width = (width - 126.0).max(96.0);
    let map_height = (height - 130.0).max(96.0);
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
    }
}

pub(super) fn icon_source(icon: &WaypointIcon) -> String {
    tilt_ui::Icon::from_name(icon.glyph())
        .unwrap_or(tilt_ui::Icon::MapPin)
        .source(tilt_ui::IconSize::Px32)
}

#[cfg(test)]
mod tests {
    use super::*;
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
    fn map_tool_rail_is_icon_only_with_hover_tooltips_for_all_seven_actions() {
        let template = include_str!("../../../../src-ui/components/full-map.component.html");
        let rail = template
            .split("<div class=\"full-map-tool-rail\">")
            .nth(1)
            .unwrap()
            .split("</div>")
            .next()
            .unwrap();
        assert_eq!(rail.matches("<button ").count(), 7);
        assert_eq!(rail.matches("<icon ").count(), 7);
        assert!(!rail.contains("<p>"));
        assert!(!rail.contains("map-icon-button"));
        for action in ["add", "grid", "layer", "mobs", "mode", "center", "list"] {
            assert!(template.contains(&format!(
                "<tooltip for=\"fullmap-{action}\" trigger=\"hover\""
            )));
        }
        assert!(rail.contains("[disabled]=\"!fullMapView.three_dimensional_available\""));
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
            assert!(view.map_width + 126.0 <= canvas.x + 1.0);
            assert!(view.map_height + 130.0 <= canvas.y + 1.0);
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
        begin_editor(&mut state, [-20.0, 81.0, 44.0], MapLayer::Surface);
        assert!(save_editor(&mut state, &atlas).is_err());
        assert!(state.waypoints.entries.is_empty());
        state.editor.as_mut().unwrap().name = "Home".into();
        state.editor.as_mut().unwrap().icon = WaypointIcon::Home;
        state.editor.as_mut().unwrap().show_in_world = true;
        save_editor(&mut state, &atlas).unwrap();
        assert!(state.editor.is_none());
        let saved = WaypointBook::load_directory(&root).unwrap();
        assert_eq!(state.waypoints, saved);
        assert_eq!(saved.entries[0].position, [-20.0, 81.0, 44.0]);
        assert_eq!(saved.entries[0].icon, WaypointIcon::Home);
        assert!(saved.entries[0].show_in_world);
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
        assert_eq!(
            editor
                .coordinates
                .clone()
                .map(|v| v.parse::<f64>().unwrap()),
            position
        );
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
