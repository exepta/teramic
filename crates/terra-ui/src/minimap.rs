//! Shared asynchronous cartography for the MiniMap and FullMap.
mod diplomacy;
pub mod fullmap;
mod gpu;
mod markers;
mod planning;
mod raster;
mod scene;
mod seasons;
pub use scene::Map3dRenderStatus;
mod waypoints;
pub use waypoints::WorldWaypointView;

use bevy::{
    asset::RenderAssetUsages,
    ecs::message::MessageCursor,
    prelude::*,
    render::render_resource::{Extent3d, TextureDimension, TextureFormat},
    tasks::{AsyncComputeTaskPool, IoTaskPool, Task, futures_lite::future},
};
use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
    sync::Arc,
};
use terra_core::{GameplayConfig, MapMode, MapPalette, MinimapConfig};
use terra_network::client::{ClientSession, ReplicatedCreatures, ReplicatedPlayers};
use terra_storage::maps::{self, MapIdentity};
use terra_world::{
    cartography::{DetailTile, DetailTileIndex, DetailTileSnapshot, MapLayer, MapTile},
    edits::EditJournal,
    streaming::{
        ChunkStore, PreparedColumnChanged, SectionEditApplied, SectionPublished, SectionRemoved,
        WaterPublished, WorldTerrain,
    },
};
use tilt_ui::html_shared;

const SIZE: usize = 320;
const CACHE_LIMIT: usize = 4096;
const DETAIL_LIMIT: usize = 256;
const OVERVIEW_LIMIT: usize = 65536;
const SPANS: [i64; 4] = [96, 192, 384, 768];
type Key = ([i64; 2], MapLayer);
type CatalogResult = (MapLayer, Result<Vec<[i64; 2]>, String>);

#[derive(Clone, PartialEq)]
struct LayerProbeKey {
    identity: Option<MapIdentity>,
    mode: MapMode,
    position: [i64; 3],
    loaded_sections: usize,
    edit_epoch: u64,
    transitioning: bool,
}

impl LayerProbeKey {
    fn at(
        identity: Option<&MapIdentity>,
        mode: MapMode,
        position: [i64; 3],
        chunks: &ChunkStore,
        journal: Option<&EditJournal>,
        transitioning: bool,
    ) -> Self {
        Self {
            identity: identity.cloned(),
            mode,
            position,
            loaded_sections: chunks.loaded.len(),
            edit_epoch: journal.map_or(0, |journal| {
                journal.mesh_epoch(terra_voxel::SectionPos::new(
                    position[0].div_euclid(16),
                    position[1].div_euclid(16),
                    position[2].div_euclid(16),
                ))
            }),
            transitioning,
        }
    }
}

#[derive(Resource, Default)]
struct LayerProbe {
    key: Option<LayerProbeKey>,
    result: MapLayer,
    sampled_at: f64,
}

impl LayerProbe {
    fn select(
        &mut self,
        key: LayerProbeKey,
        now: f64,
        sample: impl FnOnce() -> MapLayer,
    ) -> MapLayer {
        let y = key.position[1];
        let forced = match key.mode {
            MapMode::Surface => Some(MapLayer::Surface),
            MapMode::Cave => Some(cave_layer(key.position[1])),
            MapMode::Auto => None,
        };
        if key.transitioning {
            self.key = None;
            self.result = forced.unwrap_or_default();
            return self.result;
        }
        if self.key.as_ref() == Some(&key) && now >= self.sampled_at && now - self.sampled_at < 0.2
        {
            return self.result;
        }
        let hard_change = self.key.as_ref().is_none_or(|old| {
            old.identity != key.identity
                || old.mode != key.mode
                || old
                    .position
                    .iter()
                    .zip(key.position)
                    .any(|(old, new)| old.abs_diff(new) >= 8)
        });
        // Both views share this deadline. Walking and stream/edit revisions
        // coalesce until it expires instead of triggering a second roof scan.
        if let Some(layer) = forced {
            self.result = layer;
            self.key = Some(key);
            self.sampled_at = now;
        } else if hard_change || now < self.sampled_at || now - self.sampled_at >= 0.2 {
            self.result = sample();
            self.key = Some(key);
            self.sampled_at = now;
        }
        match self.result {
            MapLayer::Surface => MapLayer::Surface,
            MapLayer::Cave { .. } => cave_layer(y),
        }
    }
}

fn cave_layer(y: i64) -> MapLayer {
    MapLayer::Cave {
        level: (y.div_euclid(4) * 4).clamp(-128, 356) as i16,
    }
}

#[html_shared]
#[derive(Resource, Default, serde::Serialize, PartialEq)]
pub struct MiniMapView {
    pub visible: bool,
    pub editing: bool,
    pub expanded: bool,
    pub style: String,
    pub surface_style: String,
    pub range: String,
    pub coordinates: String,
    pub biome: String,
    pub clock: String,
    pub seasons: bool,
    pub season: String,
    // Native calendar uniforms stay continuous without invalidating unrelated
    // HTML bindings, including the open inventory's lexical item scopes.
    #[serde(skip)]
    pub season_cycle: f32,
    pub frame_style: String,
    #[serde(skip)]
    pub frame_rotation: f32,
    pub cave: bool,
    pub layer: String,
    pub north: String,
    pub east: String,
    pub south: String,
    pub west: String,
    pub mobs: usize,
    pub players: usize,
    pub nearest: bool,
    pub farthest: bool,
    pub error: String,
    pub waypoints: Vec<waypoints::WaypointLabel>,
    pub peers: Vec<raster::PlayerLabel>,
}
#[derive(Message)]
pub struct MapAction(pub String);
#[derive(Clone)]
struct Entry {
    tile: Option<Arc<MapTile>>,
    detail: Option<Arc<DetailTile>>,
    overview: Option<Arc<Overview>>,
    source_revision: Option<u64>,
    palette_fingerprint: u64,
    used: u64,
}

#[derive(Clone, PartialEq)]
struct Overview {
    pixels: [u8; 64],
    heights: [i16; 16],
    water: u16,
}

impl Entry {
    fn height_at(&self, index: usize) -> Option<i16> {
        self.detail
            .as_ref()
            .and_then(|d| d.cells.get(index))
            .filter(|s| s.known)
            .map(|s| s.height)
            .or_else(|| {
                self.tile
                    .as_ref()
                    .and_then(|tile| tile.relief.as_ref())
                    .and_then(|relief| relief.height(index))
            })
            .or_else(|| {
                let overview = self.overview.as_ref()?;
                let height = overview.heights[(index / 16 / 4) * 4 + index % 16 / 4];
                (height != terra_world::cartography::MapRelief::UNKNOWN).then_some(height)
            })
    }
}
struct TileResult {
    key: Key,
    tile: Option<MapTile>,
    detail: Option<DetailTile>,
    error: Option<String>,
    source_revision: Option<u64>,
}
struct Work {
    key: Key,
    snapshot: Option<DetailTileSnapshot>,
    previous: Option<MapTile>,
    source_revision: Option<u64>,
}

#[derive(Default)]
struct DetailInputs {
    published: MessageCursor<SectionPublished>,
    removed: MessageCursor<SectionRemoved>,
    columns: MessageCursor<PreparedColumnChanged>,
    water: MessageCursor<WaterPublished>,
    section_edits: MessageCursor<SectionEditApplied>,
}

#[derive(Default)]
struct DetailChanges {
    sections: HashSet<terra_voxel::SectionPos>,
    columns: HashSet<[i64; 2]>,
    rebuild: bool,
    invalidate: bool,
    journal: bool,
}

impl DetailInputs {
    fn read_edits(
        &mut self,
        messages: Option<&Messages<SectionEditApplied>>,
        changes: &mut DetailChanges,
    ) {
        if let Some(messages) = messages {
            let missed = self.section_edits.missed_messages(messages) != 0;
            changes.rebuild |= missed;
            changes.invalidate |= missed;
            changes
                .sections
                .extend(self.section_edits.read(messages).map(|m| m.position));
        }
    }

    fn read_water(
        &mut self,
        messages: Option<&Messages<WaterPublished>>,
        changes: &mut DetailChanges,
    ) {
        if let Some(messages) = messages {
            changes.rebuild |= self.water.missed_messages(messages) != 0;
            changes
                .sections
                .extend(self.water.read(messages).map(|m| m.position));
        }
    }

    fn read(
        &mut self,
        published: Option<&Messages<SectionPublished>>,
        removed: Option<&Messages<SectionRemoved>>,
        columns: Option<&Messages<PreparedColumnChanged>>,
    ) -> DetailChanges {
        let mut changes = DetailChanges::default();
        if let Some(messages) = published {
            changes.rebuild |= self.published.missed_messages(messages) != 0;
            changes.invalidate |= self.published.missed_messages(messages) != 0;
            changes
                .sections
                .extend(self.published.read(messages).map(|m| m.position));
        }
        if let Some(messages) = removed {
            changes.rebuild |= self.removed.missed_messages(messages) != 0;
            changes.invalidate |= self.removed.missed_messages(messages) != 0;
            changes
                .sections
                .extend(self.removed.read(messages).map(|m| m.position));
        }
        if let Some(messages) = columns {
            changes.rebuild |= self.columns.missed_messages(messages) != 0;
            changes.invalidate |= self.columns.missed_messages(messages) != 0;
            changes
                .columns
                .extend(self.columns.read(messages).map(|m| m.coordinate));
        }
        changes
    }
}
#[derive(Clone, PartialEq)]
struct PaintKey {
    center: [i64; 3],
    layer: MapLayer,
    span: i64,
    relief: bool,
    three_dimensional: bool,
    aspect: u32,
    revision: u64,
    palette: u64,
}
struct PaintResult {
    pixels: Vec<u8>,
    projection: raster::Projection,
    level: i64,
    key: PaintKey,
}
#[derive(Default)]
struct Canvas {
    terrain: Handle<Image>,
    overlay: Handle<Image>,
    material: Handle<gpu::MapMaterial>,
    reference: Option<raster::Projection>,
    overlay_reference: Option<raster::Projection>,
    reference_level: i64,
    overlay_reference_level: i64,
    relief_base: Option<(MapLayer, i64)>,
    relief_pending: bool,
    grid: bool,
    key: Option<PaintKey>,
    job: Option<Task<PaintResult>>,
    next_terrain: f64,
    next_frame: f64,
    diplomacy: diplomacy::ClaimRasterCache,
}
#[derive(Resource)]
struct Atlas {
    identity: Option<MapIdentity>,
    root: PathBuf,
    entries: HashMap<Key, Entry>,
    catalogs: HashMap<MapLayer, HashSet<[i64; 2]>>,
    catalog_job: Option<Task<CatalogResult>>,
    job: Option<Task<(u64, Vec<TileResult>)>>,
    detail_index: Option<DetailTileIndex>,
    detail_inputs: DetailInputs,
    detail_rebuild: bool,
    detail_invalidate: bool,
    detail_journal: bool,
    mini: Canvas,
    full: Canvas,
    tick: u64,
    next: f64,
    expanded: bool,
    zoom: usize,
    preference: Option<MinimapConfig>,
    palette_fingerprint: u64,
    error: String,
    revision: u64,
    display: Option<[f64; 3]>,
    mapping_since: Option<f64>,
}
impl Default for Atlas {
    fn default() -> Self {
        Self {
            identity: None,
            root: PathBuf::new(),
            entries: HashMap::new(),
            catalogs: HashMap::new(),
            catalog_job: None,
            job: None,
            detail_index: None,
            detail_inputs: DetailInputs::default(),
            detail_rebuild: false,
            detail_invalidate: false,
            detail_journal: false,
            mini: Canvas::default(),
            full: Canvas::default(),
            tick: 0,
            next: 0.0,
            expanded: true,
            zoom: 1,
            preference: None,
            palette_fingerprint: 0,
            error: String::new(),
            revision: 0,
            display: None,
            mapping_since: None,
        }
    }
}
impl Atlas {
    fn select_identity(&mut self, identity: Option<MapIdentity>) -> bool {
        if self.identity == identity {
            return false;
        }
        self.identity = identity;
        self.job = None;
        self.detail_index = None;
        self.detail_rebuild = false;
        self.detail_invalidate = false;
        self.detail_journal = false;
        self.catalog_job = None;
        self.catalogs.clear();
        self.entries.clear();
        self.mini = Canvas::default();
        self.full = Canvas::default();
        self.display = None;
        self.mapping_since = None;
        self.error.clear();
        self.next = 0.0;
        true
    }

    fn observe_detail_index(
        &mut self,
        chunks: &ChunkStore,
        journal: &EditJournal,
        changes: DetailChanges,
    ) {
        self.detail_rebuild |= changes.rebuild;
        self.detail_invalidate |= changes.invalidate;
        self.detail_journal |= changes.journal;
        if let Some(index) = self.detail_index.as_mut()
            && (!changes.sections.is_empty() || !changes.columns.is_empty())
        {
            index.refresh(chunks, journal, changes.sections, changes.columns);
        }
    }

    fn refresh_detail_index(
        &mut self,
        chunks: &ChunkStore,
        journal: &EditJournal,
        layer: MapLayer,
        mut changes: DetailChanges,
    ) {
        changes.rebuild |= std::mem::take(&mut self.detail_rebuild);
        changes.invalidate |= std::mem::take(&mut self.detail_invalidate);
        changes.journal |= std::mem::take(&mut self.detail_journal);
        if let Some(index) = self.detail_index.as_mut() {
            if changes.rebuild || index.layer() != layer {
                index.rebuild(chunks, journal, layer, changes.invalidate);
            } else {
                if changes.journal {
                    index.refresh_journal(chunks, journal);
                }
                if !changes.sections.is_empty() || !changes.columns.is_empty() {
                    index.refresh(chunks, journal, changes.sections, changes.columns);
                }
            }
        } else {
            self.detail_index = Some(DetailTileIndex::new(chunks, journal, layer));
        }
    }

    fn accepts(&self, result: &TileResult) -> bool {
        result.source_revision.is_none_or(|revision| {
            self.detail_index.as_ref().is_some_and(|index| {
                index.layer() == result.key.1
                    && index.source_revision(result.key.0) == Some(revision)
            })
        })
    }
}
pub(crate) fn install(app: &mut App) {
    app.init_resource::<Atlas>()
        .init_resource::<LayerProbe>()
        .init_resource::<GameplayConfig>()
        .init_resource::<MapPalette>()
        .init_resource::<MiniMapView>()
        .add_message::<MapAction>()
        .add_systems(
            Update,
            update
                .after(crate::settlement_map::SettlementMapSet)
                .after(terra_player::PlayerSet::Movement)
                .after(terra_world::streaming::StreamingSet::Dispatch),
        );
    gpu::install(app);
    seasons::install(app);
    fullmap::install(app);
    waypoints::install(app);
    scene::install(app);
}
#[derive(bevy::ecs::system::SystemParam)]
struct MapData<'w> {
    config: Res<'w, terra_core::GameConfig>,
    preferences: Res<'w, GameplayConfig>,
    palette: Res<'w, MapPalette>,
    chunks: Res<'w, ChunkStore>,
    journal: Res<'w, EditJournal>,
    terrain: Option<Res<'w, WorldTerrain>>,
    published: Option<Res<'w, Messages<SectionPublished>>>,
    removed: Option<Res<'w, Messages<SectionRemoved>>>,
    columns: Option<Res<'w, Messages<PreparedColumnChanged>>>,
    water: Option<Res<'w, Messages<WaterPublished>>>,
    section_edits: Option<Res<'w, Messages<SectionEditApplied>>>,
    clock: Option<Res<'w, terra_core::WorldClock>>,
    creatures: Option<Res<'w, ReplicatedCreatures>>,
    players: Option<Res<'w, ReplicatedPlayers>>,
    social: Option<Res<'w, terra_network::client::SocialState>>,
    fullmap: Res<'w, fullmap::FullMapState>,
    settlements: Option<Res<'w, crate::settlement_map::SettlementMap>>,
    fullview: ResMut<'w, fullmap::FullMapView>,
    probe: ResMut<'w, LayerProbe>,
}
#[derive(bevy::ecs::system::SystemParam)]
struct RenderAssets<'w> {
    images: ResMut<'w, Assets<Image>>,
    materials: ResMut<'w, Assets<gpu::MapMaterial>>,
}
#[derive(bevy::ecs::system::SystemParam)]
struct PlayerPose<'w, 's> {
    fixed: Res<'w, Time<Fixed>>,
    bodies: Query<
        'w,
        's,
        (
            &'static Transform,
            Option<&'static terra_player::CharacterPresentation>,
        ),
        With<terra_player::PlayerBody>,
    >,
    cameras: Query<'w, 's, &'static terra_player::CameraLook, With<terra_player::TerraCamera>>,
}
impl PlayerPose<'_, '_> {
    fn presented(&self, position: [f32; 3], yaw: f32) -> ([f64; 3], f32) {
        let position =
            self.bodies
                .iter()
                .next()
                .map_or(Vec3::from_array(position), |(body, pose)| {
                    pose.map_or(body.translation, |pose| {
                        pose.visual_position(body.translation, self.fixed.overstep_fraction())
                    })
                });
        let yaw = self
            .cameras
            .iter()
            .next()
            .map_or(yaw, |look| look.rotation().to_euler(EulerRot::YXZ).0);
        (position.to_array().map(f64::from), yaw)
    }
}
#[allow(clippy::too_many_arguments)]
fn update(
    time: Res<Time>,
    session: Option<Res<ClientSession>>,
    data: MapData,
    menu: Option<Res<crate::menu::MenuState>>,
    paths: Option<Res<crate::menu::MenuPaths>>,
    input: terra_core::GameplayInput,
    blocked: Res<terra_player::PlayerInputBlock>,
    mut actions: MessageReader<MapAction>,
    mut atlas: ResMut<Atlas>,
    mut view: ResMut<MiniMapView>,
    mut assets: RenderAssets,
    player_pose: PlayerPose,
    windows: Query<&Window, With<bevy::window::PrimaryWindow>>,
    scale: Option<Res<bevy::ui::UiScale>>,
    hud_styles: Option<Res<crate::hud_layout::HudWidgetStyles>>,
) {
    let MapData {
        config,
        preferences,
        palette,
        chunks,
        journal,
        terrain,
        published,
        removed,
        columns,
        water,
        section_edits,
        clock,
        creatures,
        players,
        social,
        fullmap,
        settlements,
        mut fullview,
        mut probe,
    } = data;
    // Always drain the streaming messages, even while hidden or a tile worker
    // is busy. The retained index then survives the two-frame message lifetime.
    let mut changes =
        atlas
            .detail_inputs
            .read(published.as_deref(), removed.as_deref(), columns.as_deref());
    atlas
        .detail_inputs
        .read_water(water.as_deref(), &mut changes);
    atlas
        .detail_inputs
        .read_edits(section_edits.as_deref(), &mut changes);
    let unexplained =
        chunks.is_changed() && changes.sections.is_empty() && changes.columns.is_empty();
    let source_changed = terrain.as_ref().is_some_and(|terrain| terrain.is_changed());
    changes.journal |= journal.is_changed();
    changes.rebuild |= unexplained || source_changed;
    changes.invalidate |= unexplained || source_changed;
    let settings = &preferences.minimap;
    let now = time.elapsed_secs_f64();
    if atlas.preference.as_ref() != Some(settings) {
        atlas.zoom = usize::from(settings.zoom).min(3);
        atlas.preference = Some(settings.clone());
        atlas.next = 0.0;
        atlas.mini.next_frame = 0.0;
        atlas.full.next_frame = 0.0;
    }
    let ready = session.as_ref().filter(|s| s.ready);
    let identity = ready.and_then(|s| s.uuid).map(|player| MapIdentity {
        world: config.world.id,
        seed: config.world.seed,
        generator: config.world.generator_version,
        player,
    });
    if atlas.select_identity(identity.clone()) {
        *view = MiniMapView::default();
        let configs = paths
            .as_ref()
            .map_or_else(terra_core::menu_config_directory, |p| p.configs.clone());
        atlas.root = identity
            .as_ref()
            .map_or_else(PathBuf::new, |key| key.directory(&configs.join("maps")));
    }
    if atlas.identity.is_some() {
        atlas.observe_detail_index(&chunks, &journal, changes);
    }
    let playing = menu.as_ref().is_none_or(|m| m.world_hud_active());
    let editing = menu
        .as_ref()
        .is_some_and(|m| m.page == crate::menu::MenuPage::HudLayout);
    let visible = editing || settings.enabled && ready.is_some() && playing;
    if view.visible != visible {
        view.visible = visible;
    }
    for action in actions.read() {
        match action.0.as_str() {
            "map-toggle" => atlas.expanded = !atlas.expanded,
            "map-in" => atlas.zoom = atlas.zoom.saturating_sub(1),
            "map-out" => atlas.zoom = (atlas.zoom + 1).min(3),
            _ => {}
        }
        atlas.next = 0.0;
        atlas.mini.next_frame = 0.0;
    }
    if visible && !blocked.0 {
        if input.just_pressed(terra_core::GameAction::MiniMap) {
            atlas.expanded = !atlas.expanded;
        }
        if input.just_pressed(terra_core::GameAction::MapZoomIn) {
            atlas.zoom = atlas.zoom.saturating_sub(1);
        }
        if input.just_pressed(terra_core::GameAction::MapZoomOut) {
            atlas.zoom = (atlas.zoom + 1).min(3);
        }
    }
    if let Some((revision, results)) = atlas
        .job
        .as_mut()
        .and_then(|job| future::block_on(future::poll_once(job)))
    {
        atlas.job = None;
        let layer = atlas
            .detail_index
            .as_ref()
            .map_or(MapLayer::Surface, DetailTileIndex::layer);
        atlas.refresh_detail_index(&chunks, &journal, layer, DetailChanges::default());
        let used = atlas.tick;
        let current_palette = palette.fingerprint();
        for result in results.into_iter().filter(|_| revision == current_palette) {
            if !atlas.accepts(&result) {
                continue;
            }
            if result.tile.is_none()
                && let Some(catalog) = atlas.catalogs.get_mut(&result.key.1)
            {
                catalog.remove(&result.key.0);
            }
            if result.tile.is_some() {
                atlas
                    .catalogs
                    .entry(result.key.1)
                    .or_default()
                    .insert(result.key.0);
            }
            if let Some(error) = result.error
                && atlas.error.is_empty()
            {
                warn!(%error,"Map cache unavailable");
                atlas.error = "Map cache unavailable".into();
            }
            let overview = result.tile.as_ref().map(|tile| Arc::new(overview(tile)));
            let previous = atlas.entries.remove(&result.key);
            let source_revision = result
                .source_revision
                .or_else(|| previous.as_ref().and_then(|entry| entry.source_revision));
            let tile = result.tile.map(|mut tile| {
                if tile.relief.is_none() {
                    tile.relief = previous
                        .as_ref()
                        .and_then(|e| e.tile.as_ref())
                        .and_then(|t| t.relief.clone());
                }
                Arc::new(tile)
            });
            if tile.as_ref() != previous.as_ref().and_then(|entry| entry.tile.as_ref()) {
                atlas.revision = atlas.revision.wrapping_add(1);
            }
            atlas.entries.insert(
                result.key,
                Entry {
                    tile,
                    detail: result
                        .detail
                        .map(Arc::new)
                        .or_else(|| previous.and_then(|e| e.detail)),
                    overview,
                    source_revision,
                    palette_fingerprint: revision,
                    used,
                },
            );
        }
        trim(&mut atlas.entries);
    }
    if let Some((layer, result)) = atlas
        .catalog_job
        .as_mut()
        .and_then(|job| future::block_on(future::poll_once(job)))
    {
        atlas.catalog_job = None;
        match result {
            Ok(coordinates) => {
                atlas.catalogs.entry(layer).or_default().extend(coordinates);
            }
            Err(error) => {
                if atlas.error.is_empty() {
                    warn!(%error,"Cannot index explored map tiles");
                    atlas.error = "Map cache unavailable".into();
                }
                atlas.catalogs.entry(layer).or_default();
            }
        }
    }
    let Some(pose) = ready
        .and_then(|s| s.confirmed_state())
        .or_else(|| editing.then_some(Default::default()))
    else {
        return;
    };
    let position = pose.position.map(|v| v.floor() as i64);
    let player_center = [position[0], position[2]];
    let (display, shown_yaw) = player_pose.presented(pose.position, pose.look[0]);
    atlas.display = Some(display);
    if !visible && !fullmap.open {
        return;
    }
    let span = if fullmap.open {
        fullmap.span
    } else {
        SPANS[atlas.zoom]
    };
    let center = if fullmap.open {
        fullmap.viewing([display[0], display[2]])
    } else {
        [display[0], display[2]]
    };
    let center_floor = center.map(|p| p.floor() as i64);
    let layer = if fullmap.open {
        fullmap.resolved_layer()
    } else {
        let key = LayerProbeKey::at(
            atlas.identity.as_ref(),
            settings.mode,
            position,
            &chunks,
            Some(&journal),
            session.as_ref().is_some_and(|session| session.teleporting),
        );
        probe.select(key, now, || select_layer(settings.mode, position, &chunks))
    };
    let cave = matches!(layer, MapLayer::Cave { .. });
    let size = if fullmap.open { 640 } else { SIZE };
    let projection = raster::Projection::at(
        center,
        span,
        if fullmap.open { 0.0 } else { shown_yaw },
        !fullmap.open && settings.rotate,
        fullmap::relief_enabled(settings.relief, layer),
        size,
        !fullmap.open,
    )
    .with_aspect(if fullmap.open {
        fullview.map_height / fullview.map_width.max(1.0)
    } else {
        1.0
    })
    .three_dimensional(fullmap.three_dimensional_active() && layer == MapLayer::Surface);
    if now >= atlas.next {
        atlas.next = now
            + if atlas.expanded || fullmap.open {
                0.2
            } else {
                1.0
            };
        atlas.tick = atlas.tick.wrapping_add(1);
        if atlas.palette_fingerprint != palette.fingerprint() {
            atlas.palette_fingerprint = palette.fingerprint();
            atlas.revision = atlas.revision.wrapping_add(1);
        }
        schedule(
            &mut atlas,
            &chunks,
            &journal,
            layer,
            player_center,
            center_floor,
            &palette,
            &projection,
        );
    }
    let Atlas {
        mini,
        full,
        entries,
        ..
    } = &mut *atlas;
    let level = relief_level(
        if fullmap.open { full } else { mini },
        entries,
        layer,
        center,
        position[1],
        !fullmap.open,
    );
    let marker_settings = map_marker_settings(settings, fullmap.open.then_some(fullmap.mobs));
    let mut markers = raster::markers(
        creatures.as_deref(),
        players.as_deref(),
        &projection,
        level,
        cave,
        &marker_settings,
    );
    markers.retain(|marker| raster::marker_visible(marker, &atlas.entries, layer, &chunks));
    if fullmap.open {
        if !atlas.error.is_empty() {
            fullview.error = atlas.error.clone();
        }
        let mapping =
            atlas.job.is_some() || atlas.catalog_job.is_some() || atlas.full.reference.is_none();
        if !mapping {
            atlas.mapping_since = None;
        }
        let mapping_since = *atlas.mapping_since.get_or_insert(now);
        fullview.loading = mapping && now - mapping_since >= 0.25;
        fullview.markers = waypoints::map_labels(
            &projection,
            layer,
            level,
            &fullmap.waypoints,
            display,
            fullmap.selected,
            Vec2::new(fullview.map_width, fullview.map_height),
        );
        fullview.peers = raster::player_labels(
            players.as_deref(),
            social.as_deref(),
            &projection,
            level,
            cave,
            settings.players,
            Vec2::new(fullview.map_width, fullview.map_height),
        );
        let draft = fullmap.preview_claims();
        let center = fullmap.viewing([display[0], display[2]]);
        let margin = fullmap.span as f64 * 3.0;
        let domains: Vec<_> = settlements
            .as_ref()
            .into_iter()
            .flat_map(|sites| sites.domains())
            .filter(|d| {
                d.bounds[1][0] as f64 >= center[0] - margin
                    && (d.bounds[0][0] as f64) <= center[0] + margin
                    && d.bounds[1][2] as f64 >= center[1] - margin
                    && (d.bounds[0][2] as f64) <= center[1] + margin
            })
            .copied()
            .collect();
        let revision = atlas.revision;
        let Atlas { full, entries, .. } = &mut *atlas;
        full.grid = fullmap.grid;
        present(
            full,
            entries,
            layer,
            &palette,
            &projection,
            level,
            span,
            revision,
            now,
            settings.fps,
            shown_yaw,
            if projection.is_three_dimensional() {
                &[]
            } else {
                &markers
            },
            settings.icon_scale,
            if projection.is_three_dimensional() {
                display.map(|v| v as f32)
            } else {
                [display[0] as f32, level as f32, display[2] as f32]
            },
            &mut assets,
            session
                .as_ref()
                .filter(|_| fullmap.diplomacy)
                .map(|session| diplomacy::RasterSource {
                    registry: &session.kingdoms,
                    domains: &domains,
                    draft: &draft,
                    selected: fullmap.diplomacy_chunk,
                }),
        );
    } else {
        let canvas = windows
            .iter()
            .next()
            .map_or(Vec2::new(1280.0, 720.0), |w| w.size())
            / scale.as_ref().map_or(1.0, |s| s.0).max(0.1);
        let width = f32::from(settings.size)
            .min(canvas.x * 0.34)
            .min(canvas.y * 0.55)
            .max(132.0);
        let biome = chunks
            .prepared_column(player_center.map(|v| v.div_euclid(16)))
            .and_then(|column| {
                column
                    .surface_at(player_center[0], player_center[1])
                    .map(|s| biome_name(s.biome_kind(config.world.sea_level)))
            })
            .unwrap_or("Unknown");
        let hour = clock.as_ref().map_or(0.0, |c| c.hour());
        let season = clock.as_ref().map(|c| c.season(config.world.seasons));
        let next = MiniMapView {
            visible,
            editing,
            expanded: editing || atlas.expanded,
            style: format!(
                "width: {width:.0}px;{}",
                hud_styles.as_ref().map_or("", |s| s.minimap.as_str())
            ),
            surface_style: format!("width: {width:.0}px; height: {width:.0}px;"),
            range: format!("{span} m"),
            coordinates: if settings.coordinates {
                format!("{} / {} / {}", position[0], position[1], position[2])
            } else {
                String::new()
            },
            biome: if settings.biome {
                if cave { "Caverns".into() } else { biome.into() }
            } else {
                String::new()
            },
            clock: if settings.clock {
                format!(
                    "{:02}:{:02}  {}",
                    hour.floor() as u32,
                    (hour.fract() * 60.0) as u32,
                    clock
                        .as_ref()
                        .map_or("", |clock| clock.season(config.world.seasons).season.name())
                )
            } else {
                String::new()
            },
            seasons: season.is_some_and(|s| s.enabled),
            season: season.map_or_else(String::new, |s| s.season.name().into()),
            season_cycle: season.map_or(0.0, |s| (s.season.index() as f32 + s.progress) * 0.25),
            frame_style: format!(
                "transform: rotate({:.4}deg);",
                -projection.rotation().to_degrees()
            ),
            frame_rotation: projection.rotation(),
            cave,
            layer: if cave {
                "Cave".into()
            } else {
                "Surface".into()
            },
            north: projection.cardinal_style(0.0),
            east: projection.cardinal_style(std::f32::consts::FRAC_PI_2),
            south: projection.cardinal_style(std::f32::consts::PI),
            west: projection.cardinal_style(-std::f32::consts::FRAC_PI_2),
            mobs: markers.iter().filter(|m| !m.player).count(),
            players: markers.iter().filter(|m| m.player).count(),
            nearest: atlas.zoom == 0,
            farthest: atlas.zoom == 3,
            error: atlas.error.clone(),
            waypoints: waypoints::map_labels(
                &projection,
                layer,
                level,
                &fullmap.waypoints,
                display,
                None,
                Vec2::splat(width),
            ),
            peers: raster::player_labels(
                players.as_deref(),
                social.as_deref(),
                &projection,
                level,
                cave,
                settings.players,
                Vec2::splat(width),
            ),
        };
        if *view != next {
            *view = next;
        }
        if atlas.expanded {
            let revision = atlas.revision;
            let Atlas { mini, entries, .. } = &mut *atlas;
            present(
                mini,
                entries,
                layer,
                &palette,
                &projection,
                level,
                span,
                revision,
                now,
                settings.fps,
                shown_yaw,
                &markers,
                settings.icon_scale,
                [display[0] as f32, level as f32, display[2] as f32],
                &mut assets,
                None,
            );
        }
    }
}

fn relief_level(
    canvas: &mut Canvas,
    entries: &HashMap<Key, Entry>,
    layer: MapLayer,
    center: [f64; 2],
    fallback: i64,
    follow_ground: bool,
) -> i64 {
    // FullMap keeps a geographic datum while panning. The follow-player map
    // instead centers the ground actually drawn below its arrow, not the height
    // of the first visited tile. Flight/jump altitude never becomes its datum.
    if let Some((previous_layer, level)) = canvas.relief_base
        && previous_layer == layer
        && !canvas.relief_pending
        && !follow_ground
    {
        return level;
    }
    let [x, z] = center.map(|v| v.floor() as i64);
    let ground = entries
        .get(&([x.div_euclid(16), z.div_euclid(16)], layer))
        .and_then(|entry| entry.height_at((z.rem_euclid(16) * 16 + x.rem_euclid(16)) as usize))
        .map(i64::from);
    let known = match layer {
        MapLayer::Cave { level } if !follow_ground => Some(i64::from(level)),
        _ => ground,
    };
    canvas.relief_pending = known.is_none();
    let level = known.unwrap_or_else(|| {
        canvas
            .relief_base
            .filter(|(old, _)| *old == layer)
            .map_or_else(
                || match layer {
                    MapLayer::Cave { level } => i64::from(level),
                    MapLayer::Surface => fallback,
                },
                |(_, level)| level,
            )
    });
    canvas.relief_base = Some((layer, level));
    level
}

#[allow(clippy::too_many_arguments)]
fn present(
    canvas: &mut Canvas,
    entries: &HashMap<Key, Entry>,
    layer: MapLayer,
    palette: &MapPalette,
    projection: &raster::Projection,
    level: i64,
    span: i64,
    revision: u64,
    now: f64,
    fps: u8,
    yaw: f32,
    markers: &[raster::Marker],
    icon_scale: f32,
    player: [f32; 3],
    assets: &mut RenderAssets,
    diplomacy: Option<diplomacy::RasterSource<'_>>,
) {
    if canvas.key.as_ref().is_some_and(|key| {
        key.layer != layer
            || key.relief != projection.relief_enabled()
            || key.three_dimensional != projection.is_three_dimensional()
            || key.aspect != projection.aspect().to_bits()
    }) {
        canvas.reference = None;
        canvas.overlay_reference = None;
        gpu::hide(&mut assets.materials, &canvas.material);
    }
    if let Some(result) = canvas
        .job
        .as_mut()
        .and_then(|job| future::block_on(future::poll_once(job)))
    {
        canvas.job = None;
        if result.key.layer == layer
            && result.key.palette == palette.fingerprint()
            && result.key.relief == projection.relief_enabled()
            && result.key.three_dimensional == projection.is_three_dimensional()
            && result.key.aspect == projection.aspect().to_bits()
        {
            upload(
                &mut assets.images,
                &mut canvas.terrain,
                result.pixels,
                projection.size(),
                true,
            );
            canvas.reference = Some(result.projection);
            canvas.reference_level = result.level;
            canvas.key = Some(result.key);
        }
    }
    let terrain_projection = projection.cached();
    let c = terrain_projection.world_at([projection.size() as f32 * 0.5; 2]);
    let key = PaintKey {
        center: [c[0].floor() as i64, level, c[1].floor() as i64],
        layer,
        span,
        relief: projection.relief_enabled(),
        three_dimensional: projection.is_three_dimensional(),
        aspect: projection.aspect().to_bits(),
        revision,
        palette: palette.fingerprint(),
    };
    if projection.is_three_dimensional() {
        canvas.job = None;
        let compatible = canvas.key.as_ref().is_some_and(|previous| {
            previous.three_dimensional && previous.aspect == projection.aspect().to_bits()
        }) && canvas.terrain != Handle::default();
        if !compatible {
            canvas.terrain = assets.images.add(Image::new_target_texture(
                1024,
                (1024.0 * projection.aspect()).round().clamp(256.0, 2048.0) as u32,
                TextureFormat::Rgba8UnormSrgb,
                None,
            ));
        }
        canvas.reference = Some(projection.clone());
        canvas.reference_level = level;
        canvas.key = Some(key);
    } else if canvas.job.is_none()
        && now >= canvas.next_terrain
        && (canvas.reference.is_none() || canvas.key.as_ref() != Some(&key))
    {
        canvas.next_terrain = now + 0.2;
        let snapshot = visible_entries(entries, &terrain_projection, layer);
        let palette = palette.clone();
        let projection = terrain_projection;
        canvas.job = Some(AsyncComputeTaskPool::get().spawn(async move {
            let pixels = raster::terrain(&snapshot, layer, &palette, &projection, level);
            PaintResult {
                pixels,
                projection,
                level,
                key,
            }
        }));
    }
    let Some(reference) = canvas.reference.as_ref() else {
        return;
    };
    if frame_due(&mut canvas.next_frame, now, fps) || canvas.overlay_reference.is_none() {
        let underlay = diplomacy.map(|source| {
            canvas
                .diplomacy
                .pixels(source, projection, entries, layer, level, revision)
        });
        let pixels =
            raster::marker_overlay_with_underlay(projection, level, markers, icon_scale, underlay);
        upload(
            &mut assets.images,
            &mut canvas.overlay,
            pixels,
            projection.size(),
            false,
        );
        canvas.overlay_reference = Some(projection.clone());
        canvas.overlay_reference_level = level;
    }
    // Cache texture swaps and both UV transforms are committed in one frame.
    // Movement remains at render cadence while marker rasterization is capped.
    let Some(overlay_reference) = canvas.overlay_reference.as_ref() else {
        return;
    };
    gpu::material(
        &mut assets.materials,
        &mut canvas.material,
        &canvas.terrain,
        &canvas.overlay,
        projection,
        reference,
        overlay_reference,
        level,
        canvas.reference_level,
        canvas.overlay_reference_level,
        player,
        yaw,
        canvas.grid,
    );
}
fn frame_due(next: &mut f64, now: f64, fps: u8) -> bool {
    if now + 0.000001 < *next {
        return false;
    }
    let period = 1.0 / f64::from(fps.max(1));
    *next = if *next == 0.0 || now - *next > period * 2.0 {
        now + period
    } else {
        *next + period
    };
    true
}

fn map_marker_settings(
    settings: &terra_core::MinimapConfig,
    fullmap_mobs: Option<bool>,
) -> terra_core::MinimapConfig {
    let mut settings = settings.clone();
    if let Some(enabled) = fullmap_mobs {
        settings.mobs = enabled;
    }
    settings
}
fn visible_entries(
    entries: &HashMap<Key, Entry>,
    projection: &raster::Projection,
    layer: MapLayer,
) -> HashMap<Key, Entry> {
    let (low, high) = projection_bounds(projection);
    let count = (high[0] - low[0] + 1).saturating_mul(high[1] - low[1] + 1);
    if count < entries.len() as i64 {
        let mut snapshot = HashMap::new();
        for z in low[1]..=high[1] {
            for x in low[0]..=high[0] {
                let key = ([x, z], layer);
                if let Some(entry) = entries.get(&key) {
                    snapshot.insert(key, entry.clone());
                }
            }
        }
        snapshot
    } else {
        entries
            .iter()
            .filter(|((p, l), _)| *l == layer && (0..2).all(|a| p[a] >= low[a] && p[a] <= high[a]))
            .map(|(key, entry)| (*key, entry.clone()))
            .collect()
    }
}

fn projection_bounds(projection: &raster::Projection) -> ([i64; 2], [i64; 2]) {
    let size = projection.size() as f32;
    let corners =
        [[0.0, 0.0], [size, 0.0], [0.0, size], [size, size]].map(|p| projection.world_at(p));
    let padding = if projection.is_three_dimensional() {
        656.0
    } else if projection.relief_enabled() {
        144.0
    } else {
        16.0
    };
    let low: [i64; 2] = std::array::from_fn(|a| {
        ((corners.iter().map(|p| p[a]).fold(f64::INFINITY, f64::min) - padding).floor() as i64)
            .div_euclid(16)
    });
    let high: [i64; 2] = std::array::from_fn(|a| {
        ((corners
            .iter()
            .map(|p| p[a])
            .fold(f64::NEG_INFINITY, f64::max)
            + padding)
            .ceil() as i64)
            .div_euclid(16)
    });
    (low, high)
}
fn upload(
    images: &mut Assets<Image>,
    handle: &mut Handle<Image>,
    pixels: Vec<u8>,
    size: usize,
    linear: bool,
) {
    let reusable = images.get(&*handle).is_some_and(|image| {
        image.data.is_some()
            && image.texture_descriptor.size.width == size as u32
            && image.texture_descriptor.size.height == size as u32
    });
    if reusable && let Some(mut image) = images.get_mut(&*handle) {
        if image.data.as_ref() != Some(&pixels) {
            image.data = Some(pixels);
        }
    } else {
        let mut image = Image::new(
            Extent3d {
                width: size as u32,
                height: size as u32,
                depth_or_array_layers: 1,
            },
            TextureDimension::D2,
            pixels,
            TextureFormat::Rgba8UnormSrgb,
            RenderAssetUsages::default(),
        );
        image.sampler = if linear {
            bevy::image::ImageSampler::linear()
        } else {
            bevy::image::ImageSampler::nearest()
        };
        *handle = images.add(image);
    }
}
#[allow(clippy::too_many_arguments)]
fn schedule(
    atlas: &mut Atlas,
    chunks: &ChunkStore,
    journal: &EditJournal,
    layer: MapLayer,
    player: [i64; 2],
    center: [i64; 2],
    palette: &MapPalette,
    projection: &raster::Projection,
) {
    let _scope = terra_core::profiling::ScopeTimer::new("mapping.schedule");
    if atlas.identity.is_none() || palette.colors.is_empty() {
        return;
    }
    if !atlas.catalogs.contains_key(&layer) && atlas.catalog_job.is_none() {
        let root = layer_root(&atlas.root, layer);
        atlas.catalog_job =
            Some(IoTaskPool::get().spawn(async move { (layer, maps::coordinates(&root)) }));
    }
    atlas.refresh_detail_index(chunks, journal, layer, DetailChanges::default());
    if atlas.job.is_some() {
        return;
    }
    let Some(index) = atlas.detail_index.as_ref() else {
        return;
    };
    let revisions = index.revisions();
    // Every actually streamed column is explored, not only a 96 m circle. The
    // snapshot budget still bounds main-thread copying while distant columns
    // retain their captured heights after the player leaves.
    let (low, high) = projection_bounds(projection);
    let catalog = atlas.catalogs.get(&layer).into_iter().flat_map(|catalog| {
        planning::visible_catalog(catalog, low, high).filter(|p| !revisions.contains_key(p))
    });
    // Only four live snapshots and 128 total jobs can be admitted. Keep those
    // best candidates rather than allocating and sorting the complete atlas.
    let mut candidates = planning::Candidates::default();
    let palette_fingerprint = palette.fingerprint();
    for coordinate in revisions.keys().copied().chain(catalog) {
        let revision = index.source_revision(coordinate);
        let mut previously_live = false;
        if let Some(entry) = atlas.entries.get_mut(&(coordinate, layer)) {
            entry.used = atlas.tick;
            let same_palette = entry.palette_fingerprint == palette_fingerprint;
            if same_palette && revision.is_some() && entry.source_revision == revision {
                continue;
            }
            if same_palette
                && revision.is_none()
                && !(entry.detail.is_some() && entry.tile.is_none() && entry.overview.is_none())
            {
                continue;
            }
            previously_live = entry.source_revision.is_some();
        }
        candidates.push(
            coordinate,
            revision.is_some(),
            previously_live,
            distance(coordinate, center).min(distance(coordinate, player)),
        );
    }
    let mut work = Vec::new();
    let mut live = 0;
    for coordinate in candidates.ordered() {
        let key = (coordinate, layer);
        let revision = index.source_revision(coordinate);
        let snapshot = if live < 4 {
            revision.and_then(|_| index.capture(chunks, journal, coordinate))
        } else {
            None
        };
        if snapshot.is_some() {
            live += 1;
        }
        // A dirty live tile waiting for its snapshot slot is not a disk read.
        // Reading its old file cannot satisfy the dirty revision and used to
        // create perpetual IO jobs and repeated "Mapping" notifications.
        if revision.is_some() && snapshot.is_none() {
            continue;
        }
        let previous = atlas
            .entries
            .get(&key)
            .and_then(|e| e.tile.as_deref().cloned());
        work.push(Work {
            key,
            snapshot,
            previous,
            source_revision: revision,
        });
        if work.len() == planning::WORK_LIMIT {
            break;
        }
    }
    if !work.is_empty() {
        let root = atlas.root.clone();
        let palette = palette.clone();
        atlas.job = Some(IoTaskPool::get().spawn(async move { load_tiles(root, work, palette) }));
    }
}
fn overview(tile: &MapTile) -> Overview {
    let pixels = std::array::from_fn(|i| {
        let cell = i / 4;
        let channel = i % 4;
        let x = (cell % 4) * 4;
        let z = (cell / 4) * 4;
        let mut sum = 0_u32;
        let mut count = 0_u32;
        for dz in 0..4 {
            for dx in 0..4 {
                let p = ((z + dz) * 16 + x + dx) * 4;
                if tile.pixels[p + 3] > 0 {
                    sum += u32::from(tile.pixels[p + channel]);
                    count += 1;
                }
            }
        }
        sum.checked_div(count).unwrap_or(0) as u8
    });
    let mut water = 0;
    let heights = std::array::from_fn(|cell| {
        let Some(relief) = tile.relief.as_ref() else {
            return terra_world::cartography::MapRelief::UNKNOWN;
        };
        let x = cell % 4 * 4;
        let z = cell / 4 * 4;
        let mut total = 0_i32;
        let mut count = 0_i32;
        let mut wet = 0;
        for dz in 0..4 {
            for dx in 0..4 {
                let index = (z + dz) * 16 + x + dx;
                if let Some(height) = relief.height(index) {
                    total += i32::from(height);
                    count += 1;
                    wet += usize::from(relief.is_water(index));
                }
            }
        }
        if count == 0 {
            terra_world::cartography::MapRelief::UNKNOWN
        } else {
            if wet * 2 >= count as usize {
                water |= 1 << cell;
            }
            (total / count) as i16
        }
    });
    Overview {
        pixels,
        heights,
        water,
    }
}

fn select_layer(mode: MapMode, position: [i64; 3], chunks: &ChunkStore) -> MapLayer {
    let cave = match mode {
        MapMode::Surface => false,
        MapMode::Cave => true,
        MapMode::Auto => {
            let surface = chunks
                .prepared_column([position[0].div_euclid(16), position[2].div_euclid(16)])
                .and_then(|c| c.surface_at(position[0], position[2]).map(|s| s.height));
            beneath_roof(position, surface, |p| {
                chunks
                    .block_at(p)
                    .and_then(terra_voxel::MaterialId::from_raw)
                    .is_some_and(terra_voxel::MaterialId::is_opaque)
            })
        }
    };
    if cave {
        cave_layer(position[1])
    } else {
        MapLayer::Surface
    }
}
fn beneath_roof(
    position: [i64; 3],
    surface: Option<f64>,
    opaque: impl Fn([i64; 3]) -> bool,
) -> bool {
    surface.is_some_and(|height| {
        height > position[1] as f64 + 4.0
            && (position[1] + 2..=height.ceil() as i64 + 2)
                .any(|y| opaque([position[0], y, position[2]]))
    })
}
fn biome_name(kind: terra_world::generation::biomes::BiomeKind) -> &'static str {
    use terra_world::generation::biomes::BiomeKind;
    match kind {
        BiomeKind::Ocean => "Ocean",
        BiomeKind::Plains => "Plains",
        BiomeKind::Forest => "Forest",
        BiomeKind::Desert => "Desert",
        BiomeKind::Highland => "Highlands",
        BiomeKind::MushroomPark => "Mushroom Park",
        BiomeKind::ClifftedDesert => "Cliffted Desert",
        BiomeKind::CherryForest => "Cherry Forest",
    }
}
fn layer_root(root: &std::path::Path, layer: MapLayer) -> PathBuf {
    match layer {
        MapLayer::Surface => root.to_path_buf(),
        MapLayer::Cave { level } => root.join("caves").join(format!("y{level}")),
    }
}
fn load_tiles(root: PathBuf, work: Vec<Work>, palette: MapPalette) -> (u64, Vec<TileResult>) {
    let tiles = work
        .into_iter()
        .map(|work| {
            let directory = layer_root(&root, work.key.1);
            if let Some(snapshot) = work.snapshot {
                let detail = snapshot.render();
                let previous = maps::load_cached(&directory, work.key.0)
                    .ok()
                    .flatten()
                    .or_else(|| work.previous.map(Into::into));
                let cached = merge_cached(&detail, &palette, previous.clone());
                let error = if previous.as_ref() == Some(&cached) {
                    None
                } else {
                    maps::save_cached(&directory, &cached).err()
                };
                TileResult {
                    key: work.key,
                    tile: Some(cached.tile),
                    detail: Some(detail),
                    error,
                    source_revision: work.source_revision,
                }
            } else {
                match maps::load_cached(&directory, work.key.0) {
                    Ok(cached) => {
                        let mut error = None;
                        let tile = cached.map(|mut cached| {
                            if recolor_cached(&mut cached, &palette, work.key.1) {
                                error = maps::save_cached(&directory, &cached).err();
                            }
                            cached.tile
                        });
                        TileResult {
                            key: work.key,
                            tile,
                            detail: None,
                            error,
                            source_revision: None,
                        }
                    }
                    Err(error) => TileResult {
                        key: work.key,
                        tile: None,
                        detail: None,
                        error: Some(error),
                        source_revision: None,
                    },
                }
            }
        })
        .collect();
    (palette.fingerprint(), tiles)
}

fn recolor_cached(cached: &mut maps::CachedMapTile, palette: &MapPalette, layer: MapLayer) -> bool {
    let Some(source) = cached.source.as_mut() else {
        // Old RGBA pixels have no material identity. Keeping their old color is
        // preferable to deleting exploration or guessing which pixels are grass.
        return false;
    };
    let fingerprint = palette.fingerprint();
    if source.palette == fingerprint {
        return false;
    }
    let detail = DetailTile {
        coordinate: cached.tile.coordinate,
        layer,
        revision: 0,
        cells: source.cells,
    };
    let repainted = raster::tile(&detail, palette);
    for (index, sample) in source.cells.iter().enumerate() {
        if sample.known {
            let pixel = index * 4..index * 4 + 4;
            cached.tile.pixels[pixel.clone()].copy_from_slice(&repainted.pixels[pixel]);
        }
    }
    source.palette = fingerprint;
    true
}

fn merge_cached(
    detail: &DetailTile,
    palette: &MapPalette,
    mut previous: Option<maps::CachedMapTile>,
) -> maps::CachedMapTile {
    if let Some(previous) = previous.as_mut() {
        recolor_cached(previous, palette, detail.layer);
    }
    let mut cached = maps::CachedMapTile {
        tile: raster::tile(detail, palette),
        source: Some(Box::new(maps::MapRasterSource {
            palette: palette.fingerprint(),
            cells: detail.cells,
        })),
    };
    if let Some(previous) = previous {
        for index in 0..256 {
            if detail.cells[index].known {
                continue;
            }
            let pixel = index * 4..index * 4 + 4;
            cached.tile.pixels[pixel.clone()].copy_from_slice(&previous.tile.pixels[pixel]);
            if let (Some(current), Some(previous)) =
                (cached.tile.relief.as_mut(), previous.tile.relief.as_ref())
            {
                current.heights[index] = previous.heights[index];
                if previous.is_water(index) {
                    current.water[index / 8] |= 1 << (index % 8);
                }
            }
            if let Some(previous_source) = previous.source.as_ref() {
                cached.source.as_mut().unwrap().cells[index] = previous_source.cells[index];
            }
        }
    }
    cached
}
fn distance(coordinate: [i64; 2], center: [i64; 2]) -> i64 {
    (coordinate[0] * 16 + 8 - center[0]).pow(2) + (coordinate[1] * 16 + 8 - center[1]).pow(2)
}
#[cfg(test)]
fn coordinates(center: [i64; 2], span: i64) -> Vec<[i64; 2]> {
    let mut out = Vec::new();
    for z in (center[1] - span / 2).div_euclid(16)..=(center[1] + span / 2).div_euclid(16) {
        for x in (center[0] - span / 2).div_euclid(16)..=(center[0] + span / 2).div_euclid(16) {
            out.push([x, z]);
        }
    }
    out
}
fn trim(entries: &mut HashMap<Key, Entry>) {
    let mut details: Vec<_> = entries
        .iter()
        .filter(|(_, e)| e.detail.is_some())
        .map(|(k, e)| (*k, e.used))
        .collect();
    if details.len() > DETAIL_LIMIT {
        details.sort_unstable_by_key(|(_, age)| *age);
        let count = details.len() - DETAIL_LIMIT;
        for (key, _) in details.into_iter().take(count) {
            if let Some(e) = entries.get_mut(&key) {
                e.detail = None;
            }
        }
    }
    let mut full: Vec<_> = entries
        .iter()
        .filter(|(_, e)| e.tile.is_some())
        .map(|(k, e)| (*k, e.used))
        .collect();
    if full.len() > CACHE_LIMIT {
        full.sort_unstable_by_key(|(_, age)| *age);
        let count = full.len() - CACHE_LIMIT;
        for (key, _) in full.into_iter().take(count) {
            if let Some(e) = entries.get_mut(&key) {
                e.tile = None;
            }
        }
    }
    let limit = if entries.values().any(|e| e.overview.is_some()) {
        OVERVIEW_LIMIT
    } else {
        CACHE_LIMIT
    };
    if entries.len() > limit {
        let mut ages: Vec<_> = entries.iter().map(|(k, e)| (*k, e.used)).collect();
        ages.sort_unstable_by_key(|(_, age)| *age);
        let count = entries.len() - limit;
        for (key, _) in ages.into_iter().take(count) {
            entries.remove(&key);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cache_palette() -> MapPalette {
        MapPalette {
            colors: vec![[0.2, 0.3, 0.1]; 8],
            biome_colors: [[0.2, 0.5, 0.1]; 8],
            ..default()
        }
    }

    fn cache_detail(layer: MapLayer) -> DetailTile {
        use terra_world::cartography::MapSample;
        let mut cells = [MapSample::default(); 256];
        cells[0] = MapSample {
            material: 1,
            height: -42,
            known: true,
            biome_weights: [0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0],
            ..default()
        };
        DetailTile {
            coordinate: [-3, -2],
            layer,
            revision: 17,
            cells,
        }
    }

    #[test]
    fn persisted_samples_recolor_on_reload_without_brightening_caves_or_losing_unknown_cells() {
        let root = std::env::temp_dir().join(format!(
            "terra-map-recolor-{}",
            terra_core::PlayerUuid::new_v4()
        ));
        let old_palette = cache_palette();
        let mut palette = old_palette.clone();
        palette.authored_grass = true;
        palette.surface_grass = Some([0.3, 0.5, 0.1]);
        assert_eq!(old_palette.revision, palette.revision);
        assert_ne!(old_palette.fingerprint(), palette.fingerprint());
        for layer in [MapLayer::Surface, MapLayer::Cave { level: -48 }] {
            let detail = cache_detail(layer);
            let first = merge_cached(&detail, &old_palette, None);
            let directory = layer_root(&root, layer);
            let expected = raster::tile(&detail, &palette);
            if layer == MapLayer::Surface {
                assert_ne!(first.tile.pixels, expected.pixels);
            } else {
                assert_eq!(first.tile.pixels, expected.pixels);
            }
            // Also simulate an old-palette worker finishing after a palette
            // change. Its truthfully stamped source cannot resurrect old colors.
            for _ in 0..2 {
                maps::save_cached(&directory, &first).unwrap();
                let (fingerprint, results) = load_tiles(
                    root.clone(),
                    vec![Work {
                        key: (detail.coordinate, layer),
                        snapshot: None,
                        previous: None,
                        source_revision: None,
                    }],
                    palette.clone(),
                );
                assert_eq!(fingerprint, palette.fingerprint());
                assert_eq!(results.len(), 1);
                assert_eq!(results[0].tile.as_ref(), Some(&expected));
                assert!(results[0].error.is_none());
                let saved = maps::load_cached(&directory, detail.coordinate)
                    .unwrap()
                    .unwrap();
                assert_eq!(
                    saved.source.as_ref().unwrap().palette,
                    palette.fingerprint()
                );
                assert!(saved.fully_recolorable());
                assert_eq!(&saved.tile.pixels[4..], &[0; 1020]);
            }
        }
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn partially_revisited_legacy_tiles_keep_exploration_and_never_claim_legacy_cells_as_new() {
        use terra_world::cartography::MapRelief;
        let mut detail = cache_detail(MapLayer::Surface);
        detail.cells[1] = detail.cells[0];
        detail.cells[0].known = false;
        let mut legacy = MapTile {
            coordinate: detail.coordinate,
            pixels: [0; 1024],
            relief: Some(Box::new(MapRelief {
                heights: [MapRelief::UNKNOWN; 256],
                water: [0; 32],
            })),
        };
        legacy.pixels[..8].copy_from_slice(&[12, 30, 14, 255, 20, 40, 10, 255]);
        legacy.relief.as_mut().unwrap().heights[0] = -71;
        legacy.relief.as_mut().unwrap().water[0] = 1;
        let palette = cache_palette();
        let mut cached = merge_cached(&detail, &palette, Some(legacy.clone().into()));
        assert_eq!(&cached.tile.pixels[..4], &legacy.pixels[..4]);
        assert_eq!(cached.tile.relief.as_ref().unwrap().height(0), Some(-71));
        assert!(cached.tile.relief.as_ref().unwrap().is_water(0));
        assert!(!cached.fully_recolorable());
        assert!(!cached.source.as_ref().unwrap().cells[0].known);
        let mut next = palette.clone();
        next.authored_grass = true;
        next.surface_grass = Some([0.3, 0.5, 0.1]);
        assert!(recolor_cached(&mut cached, &next, detail.layer));
        assert_eq!(&cached.tile.pixels[..4], &legacy.pixels[..4]);
        assert_ne!(&cached.tile.pixels[4..8], &legacy.pixels[4..8]);
        assert!(!cached.fully_recolorable());
        assert!(!recolor_cached(&mut cached, &next, detail.layer));
        detail.cells[0] = detail.cells[1];
        detail.cells[1].known = false;
        let cached = merge_cached(&detail, &next, Some(cached));
        assert!(cached.fully_recolorable());
        assert!(cached.source.as_ref().unwrap().cells[1].known);
        assert_eq!(cached.tile.relief.as_ref().unwrap().height(0), Some(-42));
        assert!(!cached.tile.relief.as_ref().unwrap().is_water(0));
        assert_eq!(&cached.tile.pixels[8..], &[0; 1016]);
    }

    #[test]
    fn palette_changes_reschedule_already_loaded_distant_tiles_with_the_same_runtime_revision() {
        use bevy::tasks::TaskPool;
        IoTaskPool::get_or_init(TaskPool::new);
        let root = std::env::temp_dir().join(format!(
            "terra-map-palette-epoch-{}",
            terra_core::PlayerUuid::new_v4()
        ));
        let layer = MapLayer::Surface;
        let detail = cache_detail(layer);
        let old_palette = cache_palette();
        let cached = merge_cached(&detail, &old_palette, None);
        maps::save_cached(&root, &cached).unwrap();
        let mut palette = old_palette.clone();
        palette.authored_grass = true;
        palette.surface_grass = Some([0.3, 0.5, 0.1]);
        let mut atlas = Atlas {
            identity: Some(test_identity(1)),
            root: root.clone(),
            catalogs: HashMap::from([(layer, HashSet::from([detail.coordinate]))]),
            entries: HashMap::from([(
                (detail.coordinate, layer),
                Entry {
                    tile: Some(Arc::new(cached.tile)),
                    detail: None,
                    overview: None,
                    source_revision: None,
                    palette_fingerprint: old_palette.fingerprint(),
                    used: 0,
                },
            )]),
            ..default()
        };
        schedule(
            &mut atlas,
            &ChunkStore::default(),
            &EditJournal::default(),
            layer,
            [-40, -24],
            [-40, -24],
            &palette,
            &raster::Projection::new([-40, -24], 96, 0.0, false, true),
        );
        let (fingerprint, results) = future::block_on(atlas.job.take().unwrap());
        assert_eq!(fingerprint, palette.fingerprint());
        assert_eq!(results.len(), 1);
        assert_eq!(
            results[0].tile.as_ref(),
            Some(&raster::tile(&detail, &palette))
        );
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn confirmed_prop_metadata_refreshes_only_its_column_without_global_map_invalidation() {
        let owner = terra_voxel::SectionPos::new(-1, 0, -2);
        let distant = terra_voxel::SectionPos::new(32, 4, 32);
        let mut chunks = ChunkStore::default();
        insert_detail_section(&mut chunks, owner);
        insert_detail_section(&mut chunks, distant);
        let mut journal = EditJournal::default();
        let mut index = DetailTileIndex::new(&chunks, &journal, MapLayer::Surface);
        let far_revision = index.source_revision([32, 32]);
        let old_revision = index.source_revision([-1, -2]);
        chunks
            .apply_confirmed_delta(
                &mut journal,
                terra_world::edits::SectionDelta {
                    position: owner,
                    prop_epoch: 1,
                    removed_props: vec![terra_world::props::PropId(7)],
                    ..default()
                },
            )
            .unwrap();
        let mut events = Messages::<SectionEditApplied>::default();
        events.write(SectionEditApplied { position: owner });
        let mut inputs = DetailInputs::default();
        let mut changes = DetailChanges::default();
        inputs.read_edits(Some(&events), &mut changes);
        assert!(!changes.rebuild && !changes.invalidate);
        assert_eq!(changes.sections, HashSet::from([owner]));
        index.refresh(&chunks, &journal, changes.sections, changes.columns);
        assert_ne!(index.source_revision([-1, -2]), old_revision);
        assert_eq!(index.source_revision([32, 32]), far_revision);
        let mut next = DetailChanges::default();
        inputs.read_edits(Some(&events), &mut next);
        assert!(next.sections.is_empty());
        assert!(!next.rebuild && !next.invalidate);
    }

    #[test]
    fn water_only_publications_refresh_their_map_column_without_rebuilding_other_columns() {
        let wet = terra_voxel::SectionPos::new(-1, 0, -1);
        let distant = terra_voxel::SectionPos::new(32, 4, 32);
        let mut chunks = ChunkStore::default();
        insert_detail_section(&mut chunks, wet);
        insert_detail_section(&mut chunks, distant);
        let mut journal = EditJournal::default();
        let mut index = DetailTileIndex::new(&chunks, &journal, MapLayer::Surface);
        let far_revision = index.source_revision([32, 32]);
        let old_revision = index.source_revision([-1, -1]);
        journal
            .set_fluid(
                [-1, 1, -1],
                terra_world::water::FluidCell {
                    amount: 6,
                    ..default()
                },
            )
            .unwrap();
        let mut events = Messages::<WaterPublished>::default();
        events.write(WaterPublished { position: wet });
        let mut inputs = DetailInputs::default();
        let mut changes = DetailChanges::default();
        inputs.read_water(Some(&events), &mut changes);
        assert!(!changes.rebuild && !changes.invalidate);
        assert_eq!(changes.sections, HashSet::from([wet]));
        index.refresh(&chunks, &journal, changes.sections, changes.columns);
        assert_ne!(index.source_revision([-1, -1]), old_revision);
        assert_eq!(index.source_revision([32, 32]), far_revision);
        let mut next = DetailChanges::default();
        inputs.read_water(Some(&events), &mut next);
        assert!(next.sections.is_empty());
    }

    fn insert_detail_section(chunks: &mut ChunkStore, position: terra_voxel::SectionPos) {
        let mut data = terra_voxel::BlockSection::empty(position).unwrap();
        data.set_block([0, 0, 0], terra_voxel::MaterialId::Stone as u16)
            .unwrap();
        chunks.loaded.insert(
            position,
            terra_world::streaming::LoadedSection {
                data,
                mesh: None,
                mesh_epoch: 0,
                water_epoch: 0,
                water: None,
                props: Vec::new(),
            },
        );
    }

    fn test_identity(world: u64) -> MapIdentity {
        MapIdentity {
            world: terra_core::WorldId(world),
            seed: terra_core::WorldSeed(42),
            generator: 5,
            player: terra_core::PlayerUuid::new_v4(),
        }
    }

    #[test]
    fn detail_publications_survive_busy_hidden_frames_and_message_expiry() {
        use bevy::tasks::TaskPool;
        IoTaskPool::get_or_init(TaskPool::new);
        let mut chunks = ChunkStore::default();
        let journal = EditJournal::default();
        let layer = MapLayer::Cave { level: -16 };
        let mut atlas = Atlas {
            identity: Some(test_identity(1)),
            ..default()
        };
        atlas.refresh_detail_index(&chunks, &journal, layer, DetailChanges::default());
        atlas.job = Some(IoTaskPool::get().spawn(std::future::pending()));
        let mut publications = Messages::<SectionPublished>::default();
        let mut removals = Messages::<SectionRemoved>::default();
        let position = terra_voxel::SectionPos::new(-2, -2, -1);
        insert_detail_section(&mut chunks, position);
        publications.write(SectionPublished { position });
        let changes = atlas
            .detail_inputs
            .read(Some(&publications), Some(&removals), None);
        atlas.observe_detail_index(&chunks, &journal, changes);
        for _hidden_frame in 0..4 {
            publications.update();
            removals.update();
            let changes = atlas
                .detail_inputs
                .read(Some(&publications), Some(&removals), None);
            assert!(
                !changes.rebuild,
                "already consumed messages need no full scan"
            );
            atlas.observe_detail_index(&chunks, &journal, changes);
        }
        let index = atlas.detail_index.as_ref().unwrap();
        assert_eq!(
            index.revisions(),
            &terra_world::cartography::detail_revisions(&chunks, &journal, layer)
        );
        assert!(index.capture(&chunks, &journal, [-2, -1]).is_some());
        assert!(
            atlas.job.is_some(),
            "worker occupancy does not delay index maintenance"
        );
        chunks.loaded.remove(&position);
        removals.write(SectionRemoved { position });
        let changes = atlas
            .detail_inputs
            .read(Some(&publications), Some(&removals), None);
        atlas.observe_detail_index(&chunks, &journal, changes);
        assert!(atlas.detail_index.as_ref().unwrap().revisions().is_empty());
    }

    #[test]
    fn missed_messages_and_unannounced_fluid_edits_use_conservative_index_recovery() {
        let mut chunks = ChunkStore::default();
        let mut journal = EditJournal::default();
        let mut atlas = Atlas {
            identity: Some(test_identity(1)),
            ..default()
        };
        atlas.refresh_detail_index(
            &chunks,
            &journal,
            MapLayer::Surface,
            DetailChanges::default(),
        );
        let position = terra_voxel::SectionPos::new(-1, -1, -1);
        insert_detail_section(&mut chunks, position);
        let mut publications = Messages::<SectionPublished>::default();
        publications.write(SectionPublished { position });
        publications.update();
        publications.update();
        let changes = atlas.detail_inputs.read(Some(&publications), None, None);
        assert!(changes.rebuild && changes.invalidate);
        atlas.observe_detail_index(&chunks, &journal, changes);
        assert!(atlas.detail_rebuild);
        atlas.refresh_detail_index(
            &chunks,
            &journal,
            MapLayer::Surface,
            DetailChanges::default(),
        );
        let before = atlas
            .detail_index
            .as_ref()
            .unwrap()
            .source_revision([-1, -1]);
        journal
            .set_fluid(
                [-1, -1, -1],
                terra_world::water::FluidCell {
                    amount: 8,
                    source: true,
                    ..default()
                },
            )
            .unwrap();
        atlas.observe_detail_index(
            &chunks,
            &journal,
            DetailChanges {
                journal: true,
                ..default()
            },
        );
        assert!(atlas.detail_journal);
        assert!(!atlas.detail_rebuild);
        assert_eq!(
            atlas
                .detail_index
                .as_ref()
                .unwrap()
                .source_revision([-1, -1]),
            before,
            "journal fallbacks coalesce until a tile is needed, not on every frame"
        );
        atlas.refresh_detail_index(
            &chunks,
            &journal,
            MapLayer::Surface,
            DetailChanges::default(),
        );
        let index = atlas.detail_index.as_ref().unwrap();
        assert_ne!(index.source_revision([-1, -1]), before);
        assert_eq!(
            index.revisions(),
            &terra_world::cartography::detail_revisions(&chunks, &journal, MapLayer::Surface)
        );
        assert!(!atlas.detail_rebuild);
        assert!(!atlas.detail_journal);
        let before_reset = index.source_revision([-1, -1]);
        let identity = atlas.identity.clone();
        journal = EditJournal::default();
        atlas.observe_detail_index(
            &chunks,
            &journal,
            DetailChanges {
                journal: true,
                ..default()
            },
        );
        atlas.refresh_detail_index(
            &chunks,
            &journal,
            MapLayer::Surface,
            DetailChanges::default(),
        );
        assert_eq!(
            atlas.identity, identity,
            "journal reset is independent of session identity"
        );
        let reset = atlas.detail_index.as_ref().unwrap();
        assert_ne!(reset.source_revision([-1, -1]), before_reset);
        assert_eq!(
            reset.revisions(),
            &terra_world::cartography::detail_revisions(&chunks, &journal, MapLayer::Surface)
        );
        let stable = reset.source_revision([-1, -1]);
        journal.clock = Some(Default::default());
        atlas.observe_detail_index(
            &chunks,
            &journal,
            DetailChanges {
                journal: true,
                ..default()
            },
        );
        atlas.refresh_detail_index(
            &chunks,
            &journal,
            MapLayer::Surface,
            DetailChanges::default(),
        );
        assert_eq!(
            atlas
                .detail_index
                .as_ref()
                .unwrap()
                .source_revision([-1, -1]),
            stable
        );
    }

    #[test]
    fn stale_tile_workers_and_previous_sessions_cannot_replace_fresh_live_tiles() {
        let mut chunks = ChunkStore::default();
        let journal = EditJournal::default();
        let position = terra_voxel::SectionPos::new(-1, -1, -1);
        insert_detail_section(&mut chunks, position);
        let mut atlas = Atlas {
            identity: Some(test_identity(1)),
            ..default()
        };
        atlas.refresh_detail_index(
            &chunks,
            &journal,
            MapLayer::Surface,
            DetailChanges::default(),
        );
        let result = TileResult {
            key: ([-1, -1], MapLayer::Surface),
            tile: None,
            detail: None,
            error: None,
            source_revision: atlas
                .detail_index
                .as_ref()
                .unwrap()
                .source_revision([-1, -1]),
        };
        assert!(atlas.accepts(&result));
        insert_detail_section(&mut chunks, position);
        atlas.observe_detail_index(
            &chunks,
            &journal,
            DetailChanges {
                sections: HashSet::from([position]),
                ..default()
            },
        );
        assert!(
            !atlas.accepts(&result),
            "equal-revision replacements invalidate older snapshots"
        );
        atlas.refresh_detail_index(
            &chunks,
            &journal,
            MapLayer::Cave { level: -8 },
            DetailChanges::default(),
        );
        assert!(!atlas.accepts(&result));
        atlas.detail_rebuild = true;
        assert!(atlas.select_identity(Some(test_identity(2))));
        assert!(atlas.detail_index.is_none());
        assert!(atlas.job.is_none());
        assert!(!atlas.detail_rebuild);
        assert!(!atlas.accepts(&result));
        atlas.refresh_detail_index(
            &ChunkStore::default(),
            &journal,
            MapLayer::Surface,
            DetailChanges::default(),
        );
        assert!(atlas.detail_index.as_ref().unwrap().revisions().is_empty());
        assert!(!atlas.accepts(&result));
    }
    fn probe_key() -> LayerProbeKey {
        LayerProbeKey::at(
            None,
            MapMode::Auto,
            [-16, -20, 32],
            &ChunkStore::default(),
            None,
            false,
        )
    }

    #[test]
    fn both_maps_share_the_roof_probe_deadline_even_while_cells_and_streaming_change() {
        let mut probe = LayerProbe::default();
        let calls = std::cell::Cell::new(0);
        for (frame, now) in [0.0, 0.01, 0.03, 0.10, 0.19, 0.20, 0.21, 0.39, 0.40]
            .into_iter()
            .enumerate()
        {
            let mut key = probe_key();
            key.position[0] += (frame % 6) as i64;
            key.loaded_sections = frame;
            key.edit_epoch = frame as u64;
            for _view in 0..2 {
                assert_eq!(
                    probe.select(key.clone(), now, || {
                        calls.set(calls.get() + 1);
                        MapLayer::Cave { level: -20 }
                    }),
                    MapLayer::Cave { level: -20 }
                );
            }
        }
        assert_eq!(calls.get(), 3);
    }

    #[test]
    fn roof_probe_invalidates_world_teleport_large_moves_and_mode_without_delaying_forced_caves() {
        let mut probe = LayerProbe::default();
        let calls = std::cell::Cell::new(0);
        let sample = || {
            calls.set(calls.get() + 1);
            MapLayer::Surface
        };
        let mut key = probe_key();
        assert_eq!(probe.select(key.clone(), 1.0, sample), MapLayer::Surface);
        assert_eq!(calls.get(), 1);
        key.mode = MapMode::Cave;
        key.position[1] = -25;
        assert_eq!(
            probe.select(key.clone(), 1.01, sample),
            MapLayer::Cave { level: -28 }
        );
        assert_eq!(calls.get(), 1);
        key.mode = MapMode::Auto;
        probe.select(key.clone(), 1.02, sample);
        assert_eq!(calls.get(), 2);
        key.position[0] += 8;
        probe.select(key.clone(), 1.03, sample);
        assert_eq!(calls.get(), 3);
        key.identity = Some(MapIdentity {
            world: terra_core::WorldId(2),
            seed: terra_core::WorldSeed(42),
            generator: 5,
            player: terra_core::PlayerUuid::new_v4(),
        });
        probe.select(key.clone(), 1.04, sample);
        assert_eq!(calls.get(), 4);
        key.transitioning = true;
        probe.select(key.clone(), 1.05, sample);
        assert_eq!(calls.get(), 4);
        key.transitioning = false;
        probe.select(key.clone(), 1.06, sample);
        assert_eq!(calls.get(), 5);
        probe.select(key, 0.0, sample);
        assert_eq!(calls.get(), 6);
    }

    #[test]
    fn cached_caves_follow_current_slice_height_without_another_roof_scan() {
        let mut probe = LayerProbe::default();
        let mut key = probe_key();
        assert_eq!(
            probe.select(key.clone(), 1.0, || MapLayer::Cave { level: -20 }),
            MapLayer::Cave { level: -20 }
        );
        key.position[1] += 5;
        assert_eq!(
            probe.select(key, 1.01, || panic!("small move must use the shared probe")),
            MapLayer::Cave { level: -16 }
        );
    }

    #[test]
    fn automatic_cave_layer_accepts_tall_rooms_but_not_open_ravines_or_tree_cover() {
        let position = [-24, -32, -17];
        assert!(beneath_roof(position, Some(70.0), |p| p[1] == 40));
        assert!(!beneath_roof(position, Some(70.0), |_| false));
        assert!(!beneath_roof(position, Some(-32.0), |p| p[1] == -20));
        assert!(!beneath_roof(position, None, |_| true));
    }
    #[test]
    fn leaving_3d_replaces_the_rectangular_render_target_with_a_valid_cpu_map_texture() {
        let mut images = Assets::<Image>::default();
        let mut handle = images.add(Image::new_target_texture(
            1024,
            512,
            TextureFormat::Rgba8UnormSrgb,
            None,
        ));
        let target = handle.clone();
        upload(
            &mut images,
            &mut handle,
            vec![120; 640 * 640 * 4],
            640,
            true,
        );
        assert_ne!(handle, target);
        let image = images.get(&handle).unwrap();
        assert_eq!(image.texture_descriptor.size.width, 640);
        assert_eq!(image.texture_descriptor.size.height, 640);
        assert_eq!(image.data.as_ref().unwrap().len(), 640 * 640 * 4);
        let retained = handle.clone();
        upload(
            &mut images,
            &mut handle,
            vec![130; 640 * 640 * 4],
            640,
            true,
        );
        assert_eq!(
            handle, retained,
            "same-size CPU maps reuse their image handle"
        );
    }
    #[test]
    fn disk_candidate_bounds_cover_the_actual_portrait_3d_frustum() {
        for aspect in [0.5, 1.5, 3.0] {
            let projection =
                raster::Projection::at([-120.125, 200.5], 4096, 0.0, false, true, 640, false)
                    .with_aspect(aspect)
                    .three_dimensional(true);
            let (low, high) = projection_bounds(&projection);
            for uv in [[0.0, 0.0], [640.0, 0.0], [0.0, 640.0], [640.0, 640.0]] {
                let coordinate = projection
                    .world_at(uv)
                    .map(|v| (v.floor() as i64).div_euclid(16));
                assert!((0..2).all(|axis| low[axis] <= coordinate[axis] && coordinate[axis] <= high[axis]));
            }
        }
    }
    #[test]
    fn rapid_mode_return_repaints_an_unchanged_key_if_its_reference_was_invalidated() {
        use bevy::{ecs::system::SystemState, tasks::TaskPool};
        AsyncComputeTaskPool::get_or_init(TaskPool::new);
        let projection = raster::Projection::at([0.0, 0.0], 192, 0.0, false, true, SIZE, true);
        let palette = MapPalette::default();
        let mut canvas = Canvas {
            key: Some(PaintKey {
                center: [0, 80, 0],
                layer: MapLayer::Surface,
                span: 192,
                relief: true,
                three_dimensional: false,
                aspect: 1.0f32.to_bits(),
                revision: 1,
                palette: palette.fingerprint(),
            }),
            ..default()
        };
        let mut world = World::new();
        world.insert_resource(Assets::<Image>::default());
        world.insert_resource(Assets::<gpu::MapMaterial>::default());
        let mut system = SystemState::<RenderAssets>::new(&mut world);
        let mut assets = system.get_mut(&mut world).unwrap();
        present(
            &mut canvas,
            &HashMap::new(),
            MapLayer::Surface,
            &palette,
            &projection,
            80,
            192,
            1,
            1.0,
            30,
            0.0,
            &[],
            1.0,
            [0.0, 80.0, 0.0],
            &mut assets,
            None,
        );
        assert!(
            canvas.job.is_some(),
            "returning before the intermediate paint finishes must not leave the map black"
        );
    }
    #[test]
    fn presentation_uses_local_interpolation_and_aim_between_network_snapshots() {
        use bevy::ecs::system::SystemState;
        let mut world = World::new();
        world.insert_resource(Time::<Fixed>::default());
        world.spawn((
            terra_player::PlayerBody,
            Transform::from_xyz(20.0, 85.0, 30.0),
            terra_player::CharacterPresentation {
                previous: Vec3::new(10.0, 80.0, 20.0),
                correction: Vec3::new(0.125, 0.0, -0.125),
            },
        ));
        world.spawn((
            terra_player::TerraCamera,
            terra_player::CameraLook::default(),
        ));
        let mut state = SystemState::<PlayerPose>::new(&mut world);
        for network_position in [[-20.0, 82.0, 0.0], [0.0, 92.0, 8.0], [4.0, 78.0, 6.0]] {
            let pose = state.get(&world).unwrap().presented(network_position, 2.5);
            assert_eq!(pose, ([10.125, 80.0, 19.875], 0.0));
        }
    }

    #[test]
    fn flight_altitude_does_not_move_the_terrain_relief_datum() {
        let mut canvas = Canvas::default();
        let entries = HashMap::new();
        for altitude in [80, 81, 120, 350, -90, 82] {
            assert_eq!(
                relief_level(
                    &mut canvas,
                    &entries,
                    MapLayer::Surface,
                    [10.0, 20.0],
                    altitude,
                    true,
                ),
                80
            );
        }
        assert_eq!(
            relief_level(
                &mut canvas,
                &entries,
                MapLayer::Cave { level: -24 },
                [10.0, 20.0],
                350,
                true,
            ),
            -24
        );
    }

    #[test]
    fn minimap_follows_sampled_ground_across_heights_but_fullmap_keeps_its_datum() {
        use terra_world::cartography::{DetailTile, MapSample};
        for layer in [MapLayer::Surface, MapLayer::Cave { level: -24 }] {
            let mut mini = Canvas::default();
            let mut full = Canvas::default();
            let mut entries = HashMap::new();
            let heights = if layer == MapLayer::Surface {
                [18, 94, -12]
            } else {
                [-30, -28, -26]
            };
            for (center, height) in [[-17.25_f64, -0.125], [-0.2, 16.25], [32.125, -64.5]]
                .into_iter()
                .zip(heights)
            {
                let cell = center.map(|v| v.floor() as i64);
                let coordinate = cell.map(|v| v.div_euclid(16));
                entries.insert(
                    (coordinate, layer),
                    Entry {
                        tile: None,
                        detail: Some(Arc::new(DetailTile {
                            coordinate,
                            layer,
                            revision: 1,
                            cells: [MapSample {
                                known: true,
                                height,
                                material: 3,
                                ..default()
                            }; 256],
                        })),
                        overview: None,
                        source_revision: Some(1),
                        palette_fingerprint: 0,
                        used: 1,
                    },
                );
                for altitude in [-100, 0, 90, 350] {
                    assert_eq!(
                        relief_level(&mut mini, &entries, layer, center, altitude, true),
                        i64::from(height)
                    );
                    let full_level =
                        relief_level(&mut full, &entries, layer, center, altitude, false);
                    assert_eq!(
                        full_level,
                        if layer == MapLayer::Surface { 18 } else { -24 }
                    );
                }
            }
            let last = mini.relief_base.unwrap().1;
            assert_eq!(
                relief_level(&mut mini, &entries, layer, [-400.0; 2], 350, true),
                last
            );
        }
    }

    #[test]
    fn configured_frame_rates_keep_their_phase_on_a_hundred_hz_game_loop() {
        for fps in [24, 30, 40, 60] {
            let mut next = 0.0;
            let frames = (0..1000)
                .filter(|tick| frame_due(&mut next, f64::from(*tick) / 100.0, fps))
                .count();
            assert!(
                (frames as i32 - i32::from(fps) * 10).abs() <= 1,
                "{fps}: {frames}"
            );
        }
    }
    #[test]
    fn distant_tiles_keep_compact_colors_after_the_full_tile_budget_is_reached() {
        let tile = Arc::new(MapTile {
            coordinate: [0, 0],
            pixels: [120; 1024],
            relief: None,
        });
        let small = Arc::new(overview(&tile));
        let mut entries = HashMap::new();
        for i in 0..=CACHE_LIMIT {
            entries.insert(
                ([i as i64, 0], MapLayer::Surface),
                Entry {
                    tile: Some(tile.clone()),
                    detail: None,
                    overview: Some(small.clone()),
                    source_revision: None,
                    palette_fingerprint: 0,
                    used: i as u64,
                },
            );
        }
        trim(&mut entries);
        assert_eq!(entries.len(), CACHE_LIMIT + 1);
        assert_eq!(
            entries.values().filter(|e| e.tile.is_some()).count(),
            CACHE_LIMIT
        );
        assert!(
            entries
                .get(&([0, 0], MapLayer::Surface))
                .unwrap()
                .overview
                .is_some()
        );
    }

    #[test]
    fn full_tile_eviction_retains_coarse_elevation_and_water_not_flat_colors() {
        use terra_world::cartography::MapRelief;
        let mut relief = MapRelief {
            heights: [93; 256],
            water: [0; 32],
        };
        for z in 0..4 {
            for x in 0..4 {
                relief.heights[z * 16 + x] = 72;
                relief.water[(z * 16 + x) / 8] |= 1 << ((z * 16 + x) % 8);
            }
        }
        let tile = MapTile {
            coordinate: [-30, 12],
            pixels: [125; 1024],
            relief: Some(Box::new(relief)),
        };
        let entry = Entry {
            tile: None,
            detail: None,
            overview: Some(Arc::new(overview(&tile))),
            source_revision: Some(15),
            palette_fingerprint: 0,
            used: 0,
        };
        assert_eq!(entry.height_at(0), Some(72));
        assert_eq!(entry.height_at(255), Some(93));
        assert_eq!(entry.overview.as_ref().unwrap().water & 1, 1);
        assert!(std::mem::size_of::<Overview>() <= 100);
    }

    #[test]
    fn unchanged_loaded_columns_do_not_restart_mapping_after_detail_eviction() {
        use terra_voxel::{BlockSection, SectionPos};
        use terra_world::streaming::LoadedSection;
        let mut chunks = ChunkStore::default();
        let data = BlockSection::empty(SectionPos::new(28, 4, -21)).unwrap();
        chunks.loaded.insert(
            data.position(),
            LoadedSection {
                data,
                mesh: None,
                mesh_epoch: 0,
                water_epoch: 0,
                water: None,
                props: Vec::new(),
            },
        );
        let journal = EditJournal::default();
        let layer = MapLayer::Surface;
        let index = DetailTileIndex::new(&chunks, &journal, layer);
        let revision = index.source_revision([28, -21]).unwrap();
        let mut atlas = Atlas {
            identity: Some(MapIdentity {
                world: terra_core::WorldId(1),
                seed: terra_core::WorldSeed(42),
                generator: 5,
                player: terra_core::PlayerUuid::new_v4(),
            }),
            catalogs: HashMap::from([(layer, HashSet::new())]),
            detail_index: Some(index),
            entries: HashMap::from([(
                ([28, -21], layer),
                Entry {
                    tile: None,
                    detail: None,
                    overview: None,
                    source_revision: Some(revision),
                    palette_fingerprint: 0,
                    used: 0,
                },
            )]),
            ..default()
        };
        let palette = MapPalette {
            colors: vec![[0.5; 3]; 2],
            ..default()
        };
        for entry in atlas.entries.values_mut() {
            entry.palette_fingerprint = palette.fingerprint();
        }
        let projection = raster::Projection::new([0, 0], 192, 0.0, false, true);
        for _ in 0..20 {
            schedule(
                &mut atlas,
                &chunks,
                &journal,
                layer,
                [0, 0],
                [0, 0],
                &palette,
                &projection,
            );
            assert!(atlas.job.is_none());
            assert!(atlas.catalog_job.is_none());
        }
    }

    #[test]
    fn distant_streamed_columns_receive_height_snapshots_with_the_existing_budget() {
        use bevy::tasks::TaskPool;
        use terra_voxel::{BlockSection, MaterialId, SectionPos};
        use terra_world::streaming::LoadedSection;
        IoTaskPool::get_or_init(TaskPool::new);
        let mut chunks = ChunkStore::default();
        for x in 20..28 {
            let mut data = BlockSection::empty(SectionPos::new(x, 4, 0)).unwrap();
            data.set_block([0, 0, 0], MaterialId::Stone as u16).unwrap();
            chunks.loaded.insert(
                data.position(),
                LoadedSection {
                    data,
                    mesh: None,
                    mesh_epoch: 0,
                    water_epoch: 0,
                    water: None,
                    props: Vec::new(),
                },
            );
        }
        let player = terra_core::PlayerUuid::new_v4();
        let root = std::env::temp_dir().join(format!("terra-map-budget-{player}"));
        let mut atlas = Atlas {
            identity: Some(MapIdentity {
                world: terra_core::WorldId(1),
                seed: terra_core::WorldSeed(42),
                generator: 5,
                player,
            }),
            root: root.clone(),
            catalogs: HashMap::from([(MapLayer::Surface, HashSet::new())]),
            ..default()
        };
        let palette = MapPalette {
            colors: vec![[0.5; 3]; 4],
            ..default()
        };
        let projection = raster::Projection::new([0, 0], 768, 0.0, false, true);
        schedule(
            &mut atlas,
            &chunks,
            &EditJournal::default(),
            MapLayer::Surface,
            [0, 0],
            [0, 0],
            &palette,
            &projection,
        );
        let (_, results) = future::block_on(atlas.job.take().unwrap());
        assert_eq!(
            results.len(),
            4,
            "the main-thread snapshot budget remains four"
        );
        assert!(results.iter().all(|result| result.key.0[0] >= 20));
        assert!(results.iter().all(|result| {
            result.detail.is_some()
                && result
                    .tile
                    .as_ref()
                    .unwrap()
                    .relief
                    .as_ref()
                    .unwrap()
                    .height(0)
                    == Some(65)
        }));
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn layers_negative_coordinates_and_cache_budgets_stay_independent() {
        assert!(
            std::mem::size_of::<Entry>() < 64,
            "empty cache cells must not reserve inline detail arrays"
        );
        assert!(coordinates([-1, -1], 96).contains(&[-1, -1]));
        assert!(!coordinates([10000, -10000], 96).contains(&[-1, -1]));
        let root = PathBuf::from("maps/world");
        assert_ne!(
            layer_root(&root, MapLayer::Surface),
            layer_root(&root, MapLayer::Cave { level: -12 })
        );
        let mut entries = HashMap::new();
        for i in 0..4200 {
            entries.insert(
                ([i, 0], MapLayer::Surface),
                Entry {
                    tile: None,
                    detail: None,
                    overview: None,
                    source_revision: None,
                    palette_fingerprint: 0,
                    used: i as u64,
                },
            );
        }
        trim(&mut entries);
        assert_eq!(entries.len(), CACHE_LIMIT);
        assert!(!entries.contains_key(&([0, 0], MapLayer::Surface)));
    }

    #[test]
    fn fullmap_live_mobs_toggle_is_independent_of_the_minimap_preference() {
        let mut settings = terra_core::MinimapConfig {
            mobs: false,
            ..Default::default()
        };
        assert!(!map_marker_settings(&settings, None).mobs);
        assert!(map_marker_settings(&settings, Some(true)).mobs);
        settings.mobs = true;
        assert!(!map_marker_settings(&settings, Some(false)).mobs);
        assert!(map_marker_settings(&settings, None).mobs);
        assert_eq!(
            map_marker_settings(&settings, Some(false)).players,
            settings.players
        );
    }
}
