//! One-time, isolated renders of the real block meshes and materials for TiltUI.
pub(crate) mod alchemy;
mod bellows;
mod crops;
mod dust;
mod hardware;
mod products;
mod sculpt;
mod showcase;
pub(crate) mod tints;
pub(crate) mod tools;
use std::{
    collections::{HashMap, HashSet, VecDeque},
    time::{Duration, Instant},
};

use bevy::{
    asset::RenderAssetUsages,
    camera::{Exposure, Hdr, RenderTarget, visibility::RenderLayers},
    core_pipeline::{prepass::DepthPrepass, tonemapping::Tonemapping},
    ecs::system::SystemParam,
    mesh::{Indices, PrimitiveTopology},
    prelude::*,
    render::{
        render_resource::TextureFormat,
        view::screenshot::{Screenshot, ScreenshotCaptured},
    },
};
use terra_core::{ItemKind, ResourcePropKind};
use terra_voxel::{
    BlockSection, SectionPos,
    meshing::{BlockFace, BlockMesh},
};

use crate::{
    blocks::{BlockAssets, TemplateSet},
    meshing::{
        DetailLevel,
        builder::{MeshBatch, RenderOptions, compose_section_with_options},
    },
    water::material::{WaterAssets, WaterMaterial},
};

const PREVIEW_SIZE: u32 = 128;
const PREVIEW_LAYER: usize = 31;
const PREVIEW_GRACE: Duration = Duration::from_secs(10);
const CAPTURE_TIMEOUT: Duration = Duration::from_secs(3);
const RETRY_INTERVAL: Duration = Duration::from_millis(200);
const PREVIEW_PREFETCH: usize = 2;
const DYNAMIC_PREVIEW_LIMIT: usize = 128;
const FIXED_DEMAND_LIMIT: usize = 1024;

fn catalog_warmup(
    arguments: impl IntoIterator<Item = impl AsRef<str>>,
    explicit: Option<&str>,
) -> bool {
    let arguments = arguments
        .into_iter()
        .map(|argument| argument.as_ref().to_owned())
        .collect::<Vec<_>>();
    let smoke = arguments
        .iter()
        .any(|argument| argument.starts_with("--smoke-test"));
    !smoke
        || explicit == Some("1")
            && !arguments.iter().any(|argument| {
                argument == "--smoke-test-benchmark" || argument == "--smoke-test-alchemy"
            })
}

fn finite_catalog(materials: impl IntoIterator<Item = u16>) -> VecDeque<ItemKind> {
    let mut materials = materials.into_iter().collect::<Vec<_>>();
    materials.sort_unstable();
    let mut seen = HashSet::new();
    materials
        .iter()
        .copied()
        .map(ItemKind::Block)
        .chain(
            materials
                .iter()
                .copied()
                .filter(|&material| ItemKind::supports_wood(material))
                .flat_map(|material| {
                    [
                        terra_core::WoodSpecies::Oak,
                        terra_core::WoodSpecies::Birch,
                        terra_core::WoodSpecies::Spruce,
                        terra_core::WoodSpecies::Cherry,
                    ]
                    .map(move |wood| ItemKind::for_wood(material, wood))
                }),
        )
        .chain(ResourcePropKind::ALL.into_iter().map(ItemKind::Prop))
        .chain(ItemKind::EXTRA)
        .chain(terra_core::CapeItem::ALL.into_iter().map(ItemKind::Cape))
        .chain((71..=73).flat_map(|material| {
            terra_core::beds::DyeColor::ALL.map(move |color| ItemKind::DyedBed { material, color })
        }))
        .filter_map(requested_preview)
        .filter(|&item| !dynamic_preview(item) && seen.insert(item))
        .collect()
}

fn requested_preview(item: ItemKind) -> Option<ItemKind> {
    if !item.valid() {
        return None;
    }
    Some(match item.preview_item() {
        // The existing UI applies the exact byte tint to the neutral dye icon.
        ItemKind::Dye(terra_core::beds::DyeColor::Rgb(_)) => {
            ItemKind::Dye(terra_core::beds::DyeColor::White)
        }
        _ => item.preview_item(),
    })
}

fn legacy_preview_warmup(smoke_lod: bool, setting: Option<&str>) -> bool {
    smoke_lod && setting == Some("1")
}

fn legacy_preview_warmup_enabled() -> bool {
    static LEGACY: std::sync::LazyLock<bool> = std::sync::LazyLock::new(|| {
        legacy_preview_warmup(
            std::env::args().any(|argument| argument == "--smoke-test-lod"),
            std::env::var("TERRA_BENCH_LEGACY_PREVIEW_WARMUP")
                .ok()
                .as_deref(),
        )
    });
    *LEGACY
}

fn warmup_camera_active(frames: u16, capturing_or_retrying: bool, legacy: bool) -> bool {
    legacy || capturing_or_retrying || frames <= 2 || frames >= 7
}

#[derive(Resource, Default)]
pub struct RenderBlockPreviews {
    pub images: HashMap<u16, Handle<Image>>,
    pub pending: usize,
    pub error: Option<String>,
}

#[derive(Resource, Default)]
pub struct RenderPropPreviews {
    pub images: HashMap<ResourcePropKind, Handle<Image>>,
}
#[derive(Resource, Default)]
pub struct RenderItemPreviews {
    pub images: HashMap<ItemKind, Handle<Image>>,
    dynamic: VecDeque<ItemKind>,
}

/// One finite startup pass; dynamic RGB icons remain bounded, on-demand work.
#[derive(Resource, Default)]
pub struct RenderPreviewStartup {
    /// The real source currently being loaded or rendered into an inventory icon.
    pub current_path: String,
    pub required: bool,
    pub initialized: bool,
    pub total: usize,
    pub completed: usize,
    pub failed: usize,
    pub ready: bool,
    pub error: Option<String>,
}

/// Current visible icons, with a finite fixed-item domain and bounded real RGB demand.
#[derive(Resource, Default)]
pub struct RenderItemPreviewRequests {
    items: Vec<ItemKind>,
}

fn dynamic_preview(item: ItemKind) -> bool {
    item.valid()
        && (terra_core::tints::canonical_item(item).is_some()
            || item
                .cape()
                .is_some_and(|cape| matches!(cape.color, terra_core::beds::DyeColor::Rgb(_))))
}

impl RenderItemPreviewRequests {
    pub fn request(&mut self, item: ItemKind) {
        let Some(item) = requested_preview(item) else {
            return;
        };
        let dynamic = dynamic_preview(item);
        if self
            .items
            .iter()
            .filter(|candidate| dynamic_preview(**candidate) == dynamic)
            .count()
            < if dynamic {
                DYNAMIC_PREVIEW_LIMIT
            } else {
                FIXED_DEMAND_LIMIT
            }
            && !self.items.contains(&item)
        {
            self.items.push(item);
        }
    }

    pub fn replace(&mut self, items: impl IntoIterator<Item = ItemKind>) {
        self.items.clear();
        for item in items {
            self.request(item);
        }
    }
}

impl RenderItemPreviews {
    fn publish(&mut self, item: ItemKind, image: Handle<Image>) {
        if dynamic_preview(item) {
            if !self.dynamic.contains(&item) {
                self.dynamic.push_back(item);
            }
            while self.dynamic.len() > DYNAMIC_PREVIEW_LIMIT {
                if let Some(expired) = self.dynamic.pop_front() {
                    self.images.remove(&expired);
                }
            }
        }
        self.images.insert(item, image);
    }
}

#[derive(Component)]
pub(crate) struct PreviewScene;

#[derive(Component)]
struct PreviewCamera;

struct PreviewJob {
    item: ItemKind,
    root: Entity,
    image: Handle<Image>,
}

#[derive(Resource, Default)]
struct PreviewQueue {
    initialized: bool,
    warmup: bool,
    failed: HashSet<ItemKind>,
    items: VecDeque<ItemKind>,
    jobs: VecDeque<PreviewJob>,
    frames: u16,
    started: Option<Instant>,
    capture: Option<(Entity, Instant)>,
    capture_delayed: bool,
    retry_at: Option<Instant>,
    received: Option<bool>,
    captured_once: bool,
    catalog_remaining: HashSet<ItemKind>,
    catalog_total: usize,
    catalog_failed: usize,
    catalog_started: Option<Instant>,
}

impl PreviewQueue {
    fn pending(&self) -> usize {
        self.items.len() + self.jobs.len()
    }
}

pub(crate) fn install(app: &mut App) {
    app.init_resource::<RenderBlockPreviews>()
        .init_resource::<RenderPropPreviews>()
        .init_resource::<RenderItemPreviews>()
        .init_resource::<RenderItemPreviewRequests>()
        .init_resource::<RenderPreviewStartup>()
        .init_resource::<PreviewQueue>();
    app.world_mut().resource_mut::<PreviewQueue>().warmup = catalog_warmup(
        std::env::args(),
        std::env::var("TERRA_PREVIEW_WARMUP").ok().as_deref(),
    );
    let required = app.world().resource::<PreviewQueue>().warmup;
    app.world_mut()
        .resource_mut::<RenderPreviewStartup>()
        .required = required;
    if std::env::args().any(|a| {
        matches!(
            a.as_str(),
            "--smoke-test-items"
                | "--smoke-test-furnaces"
                | "--smoke-test-construction"
                | "--smoke-test-material-reference"
        )
    }) {
        showcase::install(app);
        return;
    }
    app.add_systems(
        Update,
        (
            admit_requested,
            prepare.after(crate::foliage::prepare_materials),
            render_next.after(bevy::render::view::screenshot::trigger_screenshots),
            sync_startup,
        )
            .chain(),
    );
}

fn sync_startup(
    blocks: Res<BlockAssets>,
    previews: Res<RenderBlockPreviews>,
    mut queue: ResMut<PreviewQueue>,
    mut startup: ResMut<RenderPreviewStartup>,
    server: Option<Res<AssetServer>>,
) {
    if !startup.required || startup.ready {
        return;
    }
    let since = *queue.catalog_started.get_or_insert_with(Instant::now);
    if startup.error.is_none()
        && let Some(error) = &blocks.error
    {
        startup.error = Some(format!("Inventory icon assets failed: {error}"));
    }
    if startup.error.is_none()
        && queue
            .capture
            .is_some_and(|(_, since)| since.elapsed() > Duration::from_secs(30))
    {
        startup.error =
            Some("Inventory icon GPU readback did not finish; restart the client to retry".into());
    }
    if startup.error.is_none() && !queue.initialized && since.elapsed() > Duration::from_secs(90) {
        startup.error = Some(
            "Inventory icon models or textures did not become ready; restart the client to retry"
                .into(),
        );
    }
    startup.current_path = blocks.startup_path(
        queue
            .jobs
            .front()
            .map(|job| job.item)
            .or_else(|| queue.items.front().copied()),
        server.as_deref(),
    );
    if !queue.initialized {
        return;
    }
    if !startup.initialized {
        startup.initialized = true;
        startup.total = queue.catalog_total;
    }
    startup.failed = queue.catalog_failed;
    startup.completed = startup.total.saturating_sub(queue.catalog_remaining.len());
    if queue.catalog_failed > 0 && startup.error.is_none() {
        startup.error = previews.error.clone();
    }
    if queue.catalog_remaining.is_empty() && queue.pending() == 0 {
        startup.ready = true;
        queue.warmup = false;
    }
}

fn admit_requested(
    mut commands: Commands,
    requests: Option<Res<RenderItemPreviewRequests>>,
    items: Res<RenderItemPreviews>,
    props: Res<RenderPropPreviews>,
    mut previews: ResMut<RenderBlockPreviews>,
    mut queue: ResMut<PreviewQueue>,
) {
    let Some(requests) = requests else {
        return;
    };
    if !queue.warmup {
        queue.items.retain(|item| requests.items.contains(item));
        // Only the front job can own a render target/readback. A hidden successor
        // can safely release its mesh/image handles when the user changes pages.
        let mut first = true;
        queue.jobs.retain(|job| {
            let keep = first || requests.items.contains(&job.item);
            first = false;
            if !keep {
                commands.entity(job.root).try_despawn();
            }
            keep
        });
    }
    queue
        .failed
        .retain(|item| !dynamic_preview(*item) || requests.items.contains(item));
    for item in &requests.items {
        let cached = match item {
            ItemKind::Block(material) => previews.images.contains_key(material),
            ItemKind::Prop(kind) => props.images.contains_key(kind),
            _ => items.images.contains_key(item),
        };
        if !cached
            && !queue.failed.contains(item)
            && !queue.items.contains(item)
            && !queue.jobs.iter().any(|job| job.item == *item)
        {
            queue.items.push_back(*item);
        }
    }
    let pending = queue.pending();
    if previews.pending != pending {
        previews.pending = pending;
    }
}

#[derive(SystemParam)]
struct PreviewAssets<'w> {
    blocks: Res<'w, BlockAssets>,
    water: Option<Res<'w, WaterAssets>>,
    foliage: Option<Res<'w, crate::foliage::FoliageAssets>>,
    terrain: Option<Res<'w, terra_world::streaming::WorldTerrain>>,
    images: ResMut<'w, Assets<Image>>,
    meshes: ResMut<'w, Assets<Mesh>>,
}

fn prepare(
    mut commands: Commands,
    mut assets: PreviewAssets,
    mut queue: ResMut<PreviewQueue>,
    mut previews: ResMut<RenderBlockPreviews>,
    mut cameras: Query<(&mut RenderTarget, &mut Camera), With<PreviewCamera>>,
) {
    if queue.initialized && (queue.items.is_empty() || queue.jobs.len() >= PREVIEW_PREFETCH) {
        return;
    }
    let Some(templates) = assets.blocks.templates.clone() else {
        return;
    };
    let Some(water) = assets.water.as_ref().map(|water| water.0.clone()) else {
        return;
    };
    let Some(foliage) = assets.foliage.as_ref() else {
        return;
    };
    if terra_voxel::MaterialId::FOLIAGE
        .into_iter()
        .filter_map(|id| templates.blocks.get(&id))
        .flat_map(|block| block.face_materials)
        .any(|slot| !foliage.materials.contains_key(&slot))
    {
        return;
    }
    let _scope = terra_core::profiling::ScopeTimer::new("previews.prepare");
    let options = RenderOptions {
        water_level: None,
        detail_level: DetailLevel::Near,
        terrain: assets.terrain.as_ref().map(|terrain| terrain.0.clone()),
        exposure: None,
        conditions: default(),
        tints: default(),
    };
    if !queue.initialized {
        queue.initialized = true;
        if queue.warmup {
            queue.items = finite_catalog(templates.blocks.keys().copied());
            queue.catalog_remaining = queue.items.iter().copied().collect();
            queue.catalog_total = queue.catalog_remaining.len();
        }
    }
    // Captures are serial. Keep only the current icon and one prepared successor
    // instead of publishing every preview mesh/target in the same startup frame.
    let Some(item) = queue.items.pop_front() else {
        return;
    };
    let was_empty = queue.jobs.is_empty();
    let batches = match match item {
        ItemKind::Block(material) => block_batches(material, &templates, &options),
        ItemKind::WoodenBlock { .. } => wooden_block_batches(item, &templates, &options),
        ItemKind::TintedBlock { .. } => tints::batches(item, &templates, &options),
        ItemKind::DyedBed { material, color } => {
            dyed_bed_batches(material, color, &templates, &options)
        }
        ItemKind::Dye(color) => dye_batches(color, &templates),
        ItemKind::GlassBottle | ItemKind::WaterBottle | ItemKind::Potion(_) => {
            alchemy::batches(item, &templates)
        }
        ItemKind::Prop(kind) => prop_batches(kind, &templates),
        ItemKind::Mineral(kind) => mineral_batches(kind, &templates),
        ItemKind::Animal(kind) => animal_batches(kind, &templates),
        ItemKind::Bellows(kind) => bellows_batches(kind, &templates),
        ItemKind::Armor(_)
        | ItemKind::Accessory(_)
        | ItemKind::Cape(_)
        | ItemKind::HeraldicCape { .. } => {
            crate::armor::item_batches(item, templates.item_materials.opaque)
        }
        ItemKind::Dust(kind) => dust::batches(kind, templates.item_materials.opaque),
        ItemKind::Crop(kind) => crop_batches(kind, &templates),
        ItemKind::Apple | ItemKind::AppleCore => {
            apple_batches(item == ItemKind::AppleCore, &templates)
        }
        ItemKind::Flower(color) => flower_batches(color, &templates),
        ItemKind::Tool(_) | ItemKind::FilledWateringCan { .. } | ItemKind::Stick => {
            tool_batches(item, &templates)
        }
    } {
        Ok(batches) => batches,
        Err(error) => {
            if queue.catalog_remaining.remove(&item) {
                queue.catalog_failed += 1;
            }
            queue.failed.insert(item);
            previews.error = Some(error.clone());
            tracing::error!(%error, ?item, "Cannot compose inventory item preview");
            previews.pending = queue.pending();
            return;
        }
    };
    let root = commands
        .spawn((PreviewScene, Transform::default(), Visibility::Hidden))
        .id();
    for batch in batches {
        let mesh = assets.meshes.add(batch_mesh(&batch));
        let mut entity = commands.spawn((
            Mesh3d(mesh),
            Transform::default(),
            RenderLayers::layer(PREVIEW_LAYER),
            ChildOf(root),
        ));
        if matches!(item, ItemKind::Block(14 | 27)) {
            entity.insert(MeshMaterial3d::<WaterMaterial>(water.clone()));
        } else if let Some(handle) = assets
            .foliage
            .as_ref()
            .and_then(|foliage| foliage.materials.get(&batch.material))
        {
            entity.insert(MeshMaterial3d(handle.clone()));
        } else if let Some(handle) = assets.blocks.materials.get(batch.material) {
            entity.insert(MeshMaterial3d(handle.clone()));
        }
        if templates.glass_material(batch.material) {
            entity.insert(bevy::light::NotShadowCaster);
        }
    }
    let mut image = Image::new_target_texture(
        PREVIEW_SIZE,
        PREVIEW_SIZE,
        TextureFormat::Rgba8UnormSrgb,
        None,
    );
    image.texture_descriptor.usage |= bevy::render::render_resource::TextureUsages::COPY_SRC;
    let image = assets.images.add(image);
    queue.jobs.push_back(PreviewJob { item, root, image });
    previews.pending = queue.pending();
    if !was_empty {
        return;
    }
    queue.started = None;
    queue.frames = 0;
    let Some(first) = queue.jobs.front() else {
        return;
    };
    commands.entity(first.root).insert(Visibility::Visible);
    if !cameras.is_empty() {
        for (mut target, mut camera) in &mut cameras {
            *target = RenderTarget::Image(first.image.clone().into());
            camera.is_active = true;
        }
        return;
    }
    commands.spawn((
        PreviewScene,
        PreviewCamera,
        Camera3d::default(),
        Camera {
            order: -10,
            clear_color: ClearColorConfig::Custom(Color::NONE),
            ..default()
        },
        RenderTarget::Image(first.image.clone().into()),
        Projection::Orthographic(OrthographicProjection {
            scaling_mode: bevy::camera::ScalingMode::FixedVertical {
                viewport_height: 1.95,
            },
            near: 0.01,
            far: 30.0,
            ..OrthographicProjection::default_3d()
        }),
        Transform::from_xyz(2.9, 2.5, 3.5).looking_at(Vec3::splat(0.5), Vec3::Y),
        RenderLayers::layer(PREVIEW_LAYER),
        AmbientLight {
            brightness: 700.0,
            color: Color::srgb(0.93, 0.96, 1.0),
            ..default()
        },
        Exposure { ev100: 13.7 },
        (Hdr, Tonemapping::AcesFitted, Msaa::Off, DepthPrepass),
        bevy::pbr::ScreenSpaceTransmission {
            steps: 1,
            ..default()
        },
    ));
    commands.spawn((
        PreviewScene,
        DirectionalLight {
            color: Color::srgb(1.0, 0.96, 0.88),
            illuminance: 100_000.0,
            shadow_maps_enabled: false,
            ..default()
        },
        Transform::from_xyz(4.0, 8.0, 6.0).looking_at(Vec3::ZERO, Vec3::Y),
        RenderLayers::layer(PREVIEW_LAYER),
    ));
}

fn render_next(
    mut commands: Commands,
    mut queue: ResMut<PreviewQueue>,
    mut previews: ResMut<RenderBlockPreviews>,
    mut prop_previews: ResMut<RenderPropPreviews>,
    mut item_previews: ResMut<RenderItemPreviews>,
    mut cameras: Query<(&mut RenderTarget, &mut Camera), With<PreviewCamera>>,
    scene: Query<Entity, With<PreviewScene>>,
) {
    if queue.jobs.is_empty() {
        if queue.initialized && queue.items.is_empty() {
            for entity in &scene {
                commands.entity(entity).try_despawn();
            }
        }
        return;
    }
    if cameras.is_empty() {
        return;
    }
    let now = Instant::now();
    let started = *queue.started.get_or_insert(now);
    let rendered = queue.received.take();
    if rendered.is_some() {
        // Screenshot owns this entity until its callback and releases it in First.
        queue.capture = None;
        queue.capture_delayed = false;
    }
    let expired = queue.capture.is_none() && now.duration_since(started) >= PREVIEW_GRACE;
    if rendered == Some(true) || expired {
        if let Some(job) = queue.jobs.pop_front() {
            if queue.catalog_remaining.remove(&job.item) && rendered != Some(true) {
                queue.catalog_failed += 1;
            }
            if rendered == Some(true) {
                queue.captured_once = true;
                match job.item {
                    ItemKind::Block(material) => {
                        previews.images.insert(material, job.image);
                    }
                    ItemKind::Prop(kind) => {
                        prop_previews.images.insert(kind, job.image);
                    }
                    ItemKind::Tool(_)
                    | ItemKind::FilledWateringCan { .. }
                    | ItemKind::Stick
                    | ItemKind::WoodenBlock { .. }
                    | ItemKind::TintedBlock { .. }
                    | ItemKind::Mineral(_)
                    | ItemKind::Bellows(_)
                    | ItemKind::Dye(_)
                    | ItemKind::GlassBottle
                    | ItemKind::WaterBottle
                    | ItemKind::Potion(_)
                    | ItemKind::DyedBed { .. }
                    | ItemKind::Armor(_)
                    | ItemKind::Accessory(_)
                    | ItemKind::Cape(_)
                    | ItemKind::HeraldicCape { .. }
                    | ItemKind::Dust(_)
                    | ItemKind::Crop(_)
                    | ItemKind::Apple
                    | ItemKind::AppleCore
                    | ItemKind::Flower(_)
                    | ItemKind::Animal(_) => {
                        item_previews.publish(job.item, job.image);
                    }
                }
            } else {
                queue.failed.insert(job.item);
                let error = format!(
                    "{} inventory preview remained blank after shader readiness grace",
                    job.item.identifier()
                );
                tracing::warn!(%error);
                previews.error = Some(error);
            }
            commands.entity(job.root).despawn();
        }
        queue.frames = 0;
        queue.started = Some(now);
        queue.retry_at = None;
        queue.capture_delayed = false;
        previews.pending = queue.pending();
        if let Some(job) = queue.jobs.front() {
            commands.entity(job.root).insert(Visibility::Visible);
            for (mut target, mut camera) in &mut cameras {
                *target = RenderTarget::Image(job.image.clone().into());
                camera.is_active = true;
            }
        } else if !queue.items.is_empty() {
            // Never clear a published icon while its successor is still being
            // prepared (including failed compositions).
            for (_, mut camera) in &mut cameras {
                camera.is_active = false;
            }
            return;
        } else {
            // No hidden permanent render pass or preview meshes remain in gameplay.
            for entity in &scene {
                commands.entity(entity).try_despawn();
            }
            return;
        }
    } else if rendered == Some(false) {
        queue.retry_at = Some(now + RETRY_INTERVAL);
    } else if !queue.capture_delayed
        && queue
            .capture
            .is_some_and(|(_, since)| now.duration_since(since) >= CAPTURE_TIMEOUT)
    {
        queue.capture_delayed = true;
        if let Some(job) = queue.jobs.front() {
            tracing::warn!(item = %job.item.identifier(), "Inventory preview GPU readback delayed; retaining the in-flight capture");
        }
    }
    queue.frames = queue.frames.saturating_add(1);
    // Queue pipelines on frames 1/2, then retain the unchanged image until two
    // active frames lead into capture. Readbacks and retries stay conservative.
    let active = queue.captured_once
        || warmup_camera_active(
            queue.frames,
            queue.capture.is_some() || queue.retry_at.is_some(),
            legacy_preview_warmup_enabled(),
        );
    for (_, mut camera) in &mut cameras {
        if camera.is_active != active {
            camera.is_active = active;
        }
    }
    if queue.capture.is_some()
        || queue.frames < if queue.captured_once { 2 } else { 8 }
        || queue.retry_at.is_some_and(|retry_at| now < retry_at)
    {
        return;
    }
    let Some(job) = queue.jobs.front() else {
        return;
    };
    let item = job.item;
    let capture = commands
        .spawn(Screenshot::image(job.image.clone()))
        .observe(
            move |capture: On<ScreenshotCaptured>, mut queue: ResMut<PreviewQueue>| {
                if queue
                    .capture
                    .is_some_and(|(entity, _)| entity == capture.entity)
                    && queue.jobs.front().is_some_and(|job| job.item == item)
                {
                    queue.received = Some(preview_has_pixels(&capture.image));
                }
            },
        )
        .id();
    if let Some(folder) = std::env::var_os("TERRA_PREVIEW_CAPTURE_DIR")
        .filter(|_| std::env::args().any(|a| a.starts_with("--smoke-test")))
        && matches!(
            item,
            ItemKind::Animal(_)
                | ItemKind::Tool(_)
                | ItemKind::Armor(_)
                | ItemKind::Accessory(_)
                | ItemKind::Cape(_)
                | ItemKind::HeraldicCape { .. }
                | ItemKind::Dust(_)
                | ItemKind::Crop(_)
                | ItemKind::Flower(_)
        )
    {
        let folder = std::path::PathBuf::from(folder);
        match std::fs::create_dir_all(&folder) {
            Ok(()) => {
                commands
                    .entity(capture)
                    .observe(bevy::render::view::screenshot::save_to_disk(
                        folder.join(format!("{}.png", item.identifier().replace("::", "_"))),
                    ));
            }
            Err(error) => tracing::warn!(%error,"Cannot capture item previews"),
        }
    }
    queue.capture = Some((capture, now));
    queue.capture_delayed = false;
    queue.frames = 0;
}

pub(crate) fn preview_has_pixels(image: &Image) -> bool {
    image.data.as_ref().is_some_and(|data| {
        data.chunks_exact(4)
            .filter(|pixel| pixel[3] > 8 && pixel[..3].iter().any(|&v| v > 8))
            .take(65)
            .count()
            > 64
    })
}

pub(crate) fn prop_batches(
    kind: ResourcePropKind,
    templates: &TemplateSet,
) -> Result<Vec<MeshBatch>, String> {
    let kind = kind.into();
    prop_variant_batches(kind, 0, templates)
}

pub(crate) fn flower_batches(
    color: terra_core::beds::DyeColor,
    templates: &TemplateSet,
) -> Result<Vec<MeshBatch>, String> {
    prop_variant_batches(
        terra_world::props::PropKind::Flower,
        terra_core::crusher::flower_variant(color),
        templates,
    )
}

fn prop_variant_batches(
    kind: terra_world::props::PropKind,
    variant: u8,
    templates: &TemplateSet,
) -> Result<Vec<MeshBatch>, String> {
    let mut batches =
        crate::props::template(kind, variant, DetailLevel::Near, &templates.prop_materials)?;
    let mut minimum = Vec3::splat(f32::INFINITY);
    let mut maximum = Vec3::splat(f32::NEG_INFINITY);
    for vertex in batches.iter().flat_map(|batch| &batch.positions) {
        let vertex = Vec3::from_array(*vertex);
        minimum = minimum.min(vertex);
        maximum = maximum.max(vertex);
    }
    let extent = (maximum - minimum).max_element();
    if !extent.is_finite() || extent <= 0.0 {
        return Err("Empty prop preview geometry".into());
    }
    let center = (minimum + maximum) * 0.5;
    for vertex in batches.iter_mut().flat_map(|batch| &mut batch.positions) {
        *vertex = ((Vec3::from_array(*vertex) - center) / extent + Vec3::splat(0.5)).to_array();
    }
    Ok(batches)
}

pub(crate) fn tool_batches(
    item: ItemKind,
    templates: &TemplateSet,
) -> Result<Vec<MeshBatch>, String> {
    tools::batches(item, templates.item_materials.opaque)
}

pub(crate) fn bellows_batches(
    kind: terra_core::BellowsKind,
    templates: &TemplateSet,
) -> Result<Vec<MeshBatch>, String> {
    bellows::batches(kind, templates.item_materials.opaque)
}

pub(crate) fn dust_batches(
    kind: terra_core::crusher::DustKind,
    templates: &TemplateSet,
) -> Result<Vec<MeshBatch>, String> {
    dust::batches(kind, templates.item_materials.opaque)
}

pub(crate) fn crop_batches(
    kind: terra_core::CropItem,
    templates: &TemplateSet,
) -> Result<Vec<MeshBatch>, String> {
    crops::batches(kind, templates.item_materials.opaque)
}

pub(crate) fn apple_batches(core: bool, templates: &TemplateSet) -> Result<Vec<MeshBatch>, String> {
    let palette = terra_world::props::visual::PropPalette {
        crop: templates.item_materials.opaque,
        ..Default::default()
    };
    let mut batches = terra_world::props::visual::apple(
        if core { 3 } else { 0 },
        terra_world::props::visual::PropDetail::Near,
        &palette,
    )
    .into_iter()
    .map(|mesh| MeshBatch {
        material: mesh.material,
        positions: mesh.positions,
        normals: mesh.normals,
        uvs: mesh.uvs,
        colors: mesh.colors,
        indices: mesh.indices,
        ..Default::default()
    })
    .collect::<Vec<_>>();
    fit_unit(&mut batches)?;
    Ok(batches)
}

pub(crate) fn block_batches(
    material: u16,
    templates: &TemplateSet,
    options: &RenderOptions,
) -> Result<Vec<MeshBatch>, String> {
    // The neutral turf texture needs a canonical tint, not the ocean/snow color
    // at the artificial preview cell's world origin.
    let preview_options = RenderOptions {
        terrain: None,
        ..options.clone()
    };
    let options = if material == 1 {
        &preview_options
    } else {
        options
    };
    let section = SectionPos::new(0, 0, 0);
    let faces: Vec<_> = (0..6)
        .map(|side| BlockFace {
            minimum: [0; 3],
            width: 1,
            height: 1,
            u_axis: (side / 2 + 1) % 3,
            v_axis: (side / 2 + 2) % 3,
            side,
            material,
            occupied_neighbor: false,
            edge_mask: 15,
        })
        .collect();
    let mut data = BlockSection::empty(section).map_err(|e| e.to_string())?;
    data.set_block([0; 3], material)
        .map_err(|e| e.to_string())?;
    let mut batches = if terra_world::objects::is_opening(material) {
        templates
            .blocks
            .get(&material)
            .ok_or("Missing opening preview template")?
            .model
            .iter()
            .map(|primitive| MeshBatch {
                material: primitive.material,
                positions: primitive.positions.clone(),
                normals: primitive.normals.clone(),
                tangents: primitive.tangents.clone(),
                uvs: primitive.uvs.clone(),
                indices: primitive.indices.clone(),
                ..default()
            })
            .collect()
    } else if material == 29 && templates.animated_chests || terra_core::kinetic::is_part(material)
    {
        crate::meshing::custom::model_geometry(
            material,
            Default::default(),
            Default::default(),
            [None; 6],
            [0; 6],
            templates,
        )?
    } else if let Some(snapshot) = crate::meshing::custom::capture(
        &data,
        templates,
        &terra_world::edits::EditJournal::default(),
        |p| {
            Some(
                if terra_core::construction::is_fence(material)
                    && p[1] == 0
                    && p[2] == 0
                    && p[0].abs() == 1
                {
                    material
                } else {
                    0
                },
            )
        },
    ) {
        crate::meshing::custom::compose(
            &BlockMesh { faces, ..default() },
            &snapshot,
            templates,
            section,
            options,
        )?
    } else {
        compose_section_with_options(&faces, templates, section, options)?
    };
    if material == 40 {
        return hardware::torch(batches, templates).map(|batch| vec![batch]);
    }
    if material == terra_core::plumbing::BASIN {
        let tint = ItemKind::Block(material)
            .wood_species()
            .unwrap_or_default()
            .tint();
        for batch in &mut batches {
            if templates.wood_materials.contains(&batch.material) {
                batch.colors = vec![tint; batch.positions.len()];
            }
        }
    }
    if material == 1
        && let Some(block) = templates.blocks.get(&1)
    {
        let color = Color::srgb(0.66, 0.83, 0.36).to_linear().to_f32_array();
        for batch in batches
            .iter_mut()
            .filter(|batch| batch.material == block.face_materials[3])
        {
            batch.colors.resize(batch.positions.len(), [1.0; 4]);
            for vertex in &mut batch.colors {
                for axis in 0..3 {
                    vertex[axis] *= color[axis] / [0.387662, 0.383565, 0.382015][axis];
                }
            }
        }
    }
    if matches!(material, 59 | 65..=67 | 71..=73 | 99..=100 | 139)
        || terra_core::logistics::is_port(material)
        || terra_core::OreKind::from_material(material).is_some()
        || terra_core::kinetic::is_part(material)
    {
        fit_unit(&mut batches)?;
    }
    Ok(batches)
}

fn fit_unit(batches: &mut [MeshBatch]) -> Result<(), String> {
    let mut minimum = Vec3::splat(f32::INFINITY);
    let mut maximum = Vec3::splat(f32::NEG_INFINITY);
    for vertex in batches.iter().flat_map(|batch| &batch.positions) {
        minimum = minimum.min(Vec3::from_array(*vertex));
        maximum = maximum.max(Vec3::from_array(*vertex));
    }
    let extent = (maximum - minimum).max_element();
    if !extent.is_finite() || extent <= 0.0 {
        return Err("Invalid item preview bounds".into());
    }
    let center = (minimum + maximum) * 0.5;
    for vertex in batches.iter_mut().flat_map(|batch| &mut batch.positions) {
        *vertex = ((Vec3::from_array(*vertex) - center) / extent + Vec3::splat(0.5)).to_array();
    }
    Ok(())
}

pub(crate) fn mineral_batches(
    item: terra_core::MineralItem,
    templates: &TemplateSet,
) -> Result<Vec<MeshBatch>, String> {
    if matches!(
        item,
        terra_core::MineralItem::IronIngot | terra_core::MineralItem::GoldIngot
    ) {
        return hardware::ingot(item, templates.item_materials.metal).map(|batch| vec![batch]);
    }
    let ore = item.ore();
    let material = templates
        .ore_materials
        .get(&ore.material())
        .copied()
        .ok_or("Missing mineral material")?;
    let mut batches = std::collections::BTreeMap::new();
    crate::meshing::ore_details::append(
        &mut batches,
        material,
        ore.material(),
        [0; 3],
        Vec3::ZERO,
        3,
        (DetailLevel::Near, 0.0),
    )?;
    let mut batches: Vec<_> = batches.into_values().collect();
    fit_unit(&mut batches)?;
    Ok(batches)
}

pub(crate) fn animal_batches(
    item: terra_core::creatures::AnimalProduct,
    templates: &TemplateSet,
) -> Result<Vec<MeshBatch>, String> {
    products::batches(item, templates.item_materials)
}

pub(crate) fn wooden_block_batches(
    item: ItemKind,
    templates: &TemplateSet,
    options: &RenderOptions,
) -> Result<Vec<MeshBatch>, String> {
    let material = item.block_material().ok_or("Not a block")?;
    let mut batches = block_batches(material, templates, options)?;
    let tint = item.wood_species().unwrap_or_default().tint();
    for batch in &mut batches {
        if templates.wood_materials.contains(&batch.material) {
            batch.colors = vec![tint; batch.positions.len()];
        }
    }
    Ok(batches)
}

pub(crate) fn dyed_bed_batches(
    material: u16,
    color: terra_core::beds::DyeColor,
    templates: &TemplateSet,
    options: &RenderOptions,
) -> Result<Vec<MeshBatch>, String> {
    let mut batches = block_batches(material, templates, options)?;
    for batch in &mut batches {
        if templates.blanket_materials.contains(&batch.material) {
            batch.colors = vec![crate::dye_tint::color(color); batch.positions.len()];
        }
    }
    Ok(batches)
}

pub(crate) fn dye_batches(
    color: terra_core::beds::DyeColor,
    templates: &TemplateSet,
) -> Result<Vec<MeshBatch>, String> {
    let mut sculpt = sculpt::Sculpt::new(32, 2);
    sculpt.cuboid([7, 5, 7], [25, 8, 25], [0.69, 0.59, 0.39]);
    // Sculpt already decodes its sRGB source colors while writing PBR vertices.
    let [r, g, b, _] = color.tint();
    sculpt.cuboid([9, 9, 9], [23, 13, 23], [r, g, b]);
    sculpt.cuboid([12, 14, 12], [20, 17, 20], [r, g, b]);
    Ok(vec![sculpt.mesh(templates.item_materials.opaque)?])
}

pub(crate) fn batch_mesh(batch: &MeshBatch) -> Mesh {
    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::RENDER_WORLD,
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, batch.positions.clone())
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, batch.normals.clone())
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, batch.uvs.clone())
    .with_inserted_indices(Indices::U32(batch.indices.clone()));
    if !batch.tangents.is_empty() {
        mesh.insert_attribute(Mesh::ATTRIBUTE_TANGENT, batch.tangents.clone());
    }
    if !batch.colors.is_empty() {
        mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, batch.colors.clone());
    }
    if batch.colors.first().is_some_and(|color| color[3] == 4.0) {
        mesh.insert_attribute(Mesh::ATTRIBUTE_UV_1, vec![[0.0; 2]; batch.positions.len()]);
    }
    mesh
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filled_can_requests_reuse_one_preloaded_icon_per_tool() {
        let mut requests = RenderItemPreviewRequests::default();
        for tool in terra_core::ToolKind::ALL
            .into_iter()
            .filter(|tool| tool.watering_capacity() > 0)
        {
            let empty = ItemKind::Tool(tool);
            requests.request(empty);
            for units in 1..=tool.watering_capacity() {
                let filled = empty.with_watering_units(units).unwrap();
                assert_eq!(requested_preview(filled), Some(empty));
                requests.request(filled);
            }
        }
        assert_eq!(requests.items.len(), 5);
        assert!(
            requests
                .items
                .iter()
                .all(|item| matches!(item, ItemKind::Tool(_)))
        );
    }

    fn rgb_preview(value: u16) -> ItemKind {
        ItemKind::TintedBlock {
            material: 148,
            wood: terra_core::WoodSpecies::Cherry,
            color: [value as u8, (value >> 8) as u8, 96],
        }
    }

    #[test]
    fn visible_preview_requests_keep_fixed_items_and_bound_real_rgb_objects_independently() {
        let mut requests = RenderItemPreviewRequests::default();
        for item in [
            ItemKind::Block(103),
            ItemKind::Dye(terra_core::beds::DyeColor::Rgb([1, 2, 3])),
        ] {
            requests.request(item);
        }
        assert_eq!(
            requests.items,
            [
                ItemKind::Block(103),
                ItemKind::Dye(terra_core::beds::DyeColor::White)
            ]
        );
        for value in 0..1000 {
            requests.request(rgb_preview(value));
            requests.request(rgb_preview(value));
        }
        assert_eq!(requests.items.len(), DYNAMIC_PREVIEW_LIMIT + 2);
        requests.replace([rgb_preview(999), rgb_preview(999)]);
        assert_eq!(requests.items, vec![rgb_preview(999)]);
    }

    #[test]
    fn dynamic_icon_eviction_only_drops_cache_ownership_and_keeps_static_icons() {
        let mut assets = Assets::<Image>::default();
        let mut previews = RenderItemPreviews::default();
        let static_image = assets.add(Image::default());
        previews.publish(ItemKind::Stick, static_image.clone());
        let live = assets.add(Image::default());
        previews.publish(rgb_preview(0), live.clone());
        for value in 1..1000 {
            previews.publish(rgb_preview(value), assets.add(Image::default()));
        }
        assert_eq!(previews.images.len(), DYNAMIC_PREVIEW_LIMIT + 1);
        assert_eq!(previews.dynamic.len(), DYNAMIC_PREVIEW_LIMIT);
        assert!(!previews.images.contains_key(&rgb_preview(0)));
        assert_eq!(previews.images[&ItemKind::Stick], static_image);
        assert!(assets.get(&live).is_some());
        assert_eq!(
            assets.get(&live).unwrap().texture_descriptor.size,
            Image::default().texture_descriptor.size
        );
    }

    #[test]
    fn visible_admission_deduplicates_and_discards_stale_fixed_and_dynamic_requests() {
        let mut app = App::new();
        app.init_resource::<RenderItemPreviewRequests>()
            .init_resource::<RenderItemPreviews>()
            .init_resource::<RenderBlockPreviews>()
            .init_resource::<RenderPropPreviews>()
            .insert_resource(PreviewQueue {
                initialized: true,
                items: VecDeque::from([ItemKind::Stick]),
                ..default()
            })
            .add_systems(Update, admit_requested);
        app.world_mut()
            .resource_mut::<RenderItemPreviewRequests>()
            .replace([rgb_preview(1), rgb_preview(1), rgb_preview(2)]);
        for _ in 0..10 {
            app.update();
        }
        assert_eq!(
            app.world().resource::<PreviewQueue>().items,
            VecDeque::from([rgb_preview(1), rgb_preview(2)])
        );
        app.world_mut()
            .resource_mut::<RenderItemPreviewRequests>()
            .replace([rgb_preview(3)]);
        app.update();
        assert_eq!(
            app.world().resource::<PreviewQueue>().items,
            VecDeque::from([rgb_preview(3)])
        );
        let image = Handle::<Image>::default();
        app.world_mut()
            .resource_mut::<RenderItemPreviews>()
            .publish(rgb_preview(3), image);
        app.world_mut().resource_mut::<PreviewQueue>().items.clear();
        app.update();
        assert!(app.world().resource::<PreviewQueue>().items.is_empty());
    }

    #[test]
    fn normal_startup_preloads_the_catalog_but_benchmark_and_alchemy_remain_demand_only() {
        for setting in [None, Some("0"), Some("1"), Some("true")] {
            assert!(catalog_warmup(["terra-client"], setting));
            for args in [vec!["--smoke-test-benchmark"], vec!["--smoke-test-alchemy"]] {
                assert!(!catalog_warmup(args, setting));
            }
            assert_eq!(
                catalog_warmup(["--smoke-test-lod"], setting),
                setting == Some("1")
            );
        }
    }

    #[test]
    fn startup_catalog_includes_all_fixed_families_and_wood_variants_without_rgb_expansion() {
        let materials = [3, 28, 29, 141, 144, 148, 149];
        let items = finite_catalog(materials);
        assert_eq!(
            items.iter().copied().collect::<HashSet<_>>().len(),
            items.len()
        );
        assert!(
            items
                .iter()
                .all(|item| item.valid() && !dynamic_preview(*item))
        );
        assert!(items.len() < FIXED_DEMAND_LIMIT);
        for item in ItemKind::EXTRA
            .into_iter()
            .chain(ResourcePropKind::ALL.map(ItemKind::Prop))
        {
            assert!(items.contains(&item), "missing {item:?}");
        }
        for material in materials
            .into_iter()
            .filter(|&material| ItemKind::supports_wood(material))
        {
            for wood in [
                terra_core::WoodSpecies::Oak,
                terra_core::WoodSpecies::Birch,
                terra_core::WoodSpecies::Spruce,
                terra_core::WoodSpecies::Cherry,
            ] {
                assert!(items.contains(&ItemKind::for_wood(material, wood)));
            }
        }
        for cape in terra_core::CapeItem::ALL {
            assert!(items.contains(&ItemKind::Cape(cape)));
        }
        for material in 71..=73 {
            for color in terra_core::beds::DyeColor::ALL {
                assert!(items.contains(&ItemKind::DyedBed { material, color }));
            }
        }
        assert_eq!(
            items,
            finite_catalog(materials.into_iter().rev().chain(materials))
        );
    }

    #[test]
    fn startup_completion_drains_the_finite_pass_and_reports_failures_without_retrying_it() {
        let mut app = App::new();
        app.init_resource::<BlockAssets>()
            .insert_resource(RenderBlockPreviews {
                error: Some("invalid item asset".into()),
                ..default()
            })
            .insert_resource(RenderPreviewStartup {
                required: true,
                ..default()
            })
            .insert_resource(PreviewQueue {
                initialized: true,
                warmup: true,
                catalog_total: 2,
                catalog_failed: 1,
                catalog_remaining: HashSet::from([ItemKind::Stick]),
                ..default()
            })
            .add_systems(Update, sync_startup);
        app.update();
        let startup = app.world().resource::<RenderPreviewStartup>();
        assert_eq!(
            (startup.total, startup.completed, startup.failed),
            (2, 1, 1)
        );
        assert!(!startup.ready);
        assert_eq!(startup.error.as_deref(), Some("invalid item asset"));
        app.world_mut()
            .resource_mut::<PreviewQueue>()
            .catalog_remaining
            .clear();
        for _ in 0..4 {
            app.update();
        }
        let startup = app.world().resource::<RenderPreviewStartup>();
        assert!(startup.ready);
        assert_eq!(
            (startup.total, startup.completed, startup.failed),
            (2, 2, 1)
        );
        assert!(!app.world().resource::<PreviewQueue>().warmup);
        assert_eq!(app.world().resource::<PreviewQueue>().pending(), 0);
    }

    #[test]
    fn changing_visible_pages_preserves_the_active_readback_but_drops_hidden_successors() {
        let mut app = App::new();
        app.init_resource::<RenderItemPreviewRequests>()
            .init_resource::<RenderBlockPreviews>()
            .init_resource::<RenderItemPreviews>()
            .init_resource::<RenderPropPreviews>()
            .add_systems(Update, admit_requested);
        let active = app.world_mut().spawn_empty().id();
        let hidden = app.world_mut().spawn_empty().id();
        let capture = app.world_mut().spawn_empty().id();
        app.insert_resource(PreviewQueue {
            initialized: true,
            capture: Some((capture, Instant::now())),
            jobs: VecDeque::from([
                PreviewJob {
                    item: ItemKind::Block(3),
                    root: active,
                    image: Handle::default(),
                },
                PreviewJob {
                    item: rgb_preview(1),
                    root: hidden,
                    image: Handle::default(),
                },
            ]),
            items: VecDeque::from([ItemKind::Stick]),
            ..default()
        });
        app.world_mut()
            .resource_mut::<RenderItemPreviewRequests>()
            .replace([ItemKind::GlassBottle]);
        app.update();
        let queue = app.world().resource::<PreviewQueue>();
        assert_eq!(queue.jobs.len(), 1);
        assert_eq!(queue.jobs[0].root, active);
        assert_eq!(queue.capture.unwrap().0, capture);
        assert_eq!(queue.items, VecDeque::from([ItemKind::GlassBottle]));
        assert!(app.world().get_entity(active).is_ok());
        assert!(app.world().get_entity(capture).is_ok());
        assert!(app.world().get_entity(hidden).is_err());
        assert_eq!(app.world().resource::<RenderBlockPreviews>().pending, 2);
    }

    #[test]
    fn visible_fixed_items_use_their_correct_existing_preview_cache() {
        let mut app = App::new();
        app.init_resource::<RenderItemPreviewRequests>()
            .init_resource::<RenderBlockPreviews>()
            .init_resource::<RenderItemPreviews>()
            .init_resource::<RenderPropPreviews>()
            .init_resource::<PreviewQueue>()
            .add_systems(Update, admit_requested);
        app.world_mut()
            .resource_mut::<RenderBlockPreviews>()
            .images
            .insert(3, Handle::default());
        app.world_mut()
            .resource_mut::<RenderPropPreviews>()
            .images
            .insert(ResourcePropKind::Flower, Handle::default());
        app.world_mut()
            .resource_mut::<RenderItemPreviews>()
            .publish(ItemKind::GlassBottle, Handle::default());
        app.world_mut()
            .resource_mut::<RenderItemPreviewRequests>()
            .replace([
                ItemKind::Block(3),
                ItemKind::Prop(ResourcePropKind::Flower),
                ItemKind::GlassBottle,
                ItemKind::Tool(terra_core::ToolKind::Wrench),
            ]);
        app.update();
        assert_eq!(
            app.world().resource::<PreviewQueue>().items,
            VecDeque::from([ItemKind::Tool(terra_core::ToolKind::Wrench)])
        );
    }

    #[test]
    fn preview_warmup_control_is_lod_smoke_only_and_keeps_capture_and_retry_frames_active() {
        for setting in [None, Some("0"), Some("1"), Some("true")] {
            assert!(!legacy_preview_warmup(false, setting));
            assert_eq!(legacy_preview_warmup(true, setting), setting == Some("1"));
        }
        for frame in 0..=12 {
            assert_eq!(
                warmup_camera_active(frame, false, false),
                frame <= 2 || frame >= 7
            );
            assert!(warmup_camera_active(frame, true, false));
            assert!(warmup_camera_active(frame, false, true));
        }
    }

    #[test]
    fn all_thirteen_dyes_have_distinct_solid_shared_material_previews() {
        let templates = TemplateSet::default();
        let mut colors = std::collections::BTreeSet::new();
        let reference = dye_batches(terra_core::beds::DyeColor::Red, &templates)
            .unwrap()
            .remove(0);
        for color in terra_core::beds::DyeColor::ALL {
            colors.insert(color.tint().map(f32::to_bits));
            let batches = dye_batches(color, &templates).unwrap();
            assert_eq!(batches.len(), 1);
            let batch = &batches[0];
            assert_eq!(batch.material, templates.item_materials.opaque);
            assert!(batch.positions.len() < 12_000);
            assert_eq!(batch.positions, reference.positions);
            assert_eq!(batch.indices, reference.indices);
            assert_eq!(batch.positions.len(), batch.colors.len());
            assert!(batch.positions.iter().flatten().all(|v| v.is_finite()));
            assert!(
                batch
                    .indices
                    .iter()
                    .all(|&i| (i as usize) < batch.positions.len())
            );
        }
        assert_eq!(colors.len(), 13);
    }

    #[test]
    fn neutral_grass_previews_are_green_independent_of_the_preview_world_origin() {
        let mut templates = TemplateSet::default();
        templates.blocks.insert(
            1,
            crate::blocks::BlockTemplate {
                surface: None,
                render_kind: crate::blocks::definitions::BlockRenderKind::Cube,
                bevel_width: 0.0,
                faces: crate::blocks::cube_faces(0.0, [1, 1, 1, 0, 1, 1], None),
                face_materials: [1, 1, 1, 0, 1, 1],
                side_cap: None,
                details: Vec::new(),
                model: Vec::new(),
            },
        );
        let terrain = terra_world::TerraGenerator::new(terra_core::WorldConfig::default()).unwrap();
        for terrain in [None, Some(terrain)] {
            let options = RenderOptions {
                water_level: None,
                detail_level: DetailLevel::Near,
                terrain,
                exposure: None,
                conditions: default(),
                tints: default(),
            };
            let batches = block_batches(1, &templates, &options).unwrap();
            let top = batches.iter().find(|batch| batch.material == 0).unwrap();
            assert!(!top.colors.is_empty());
            assert!(
                top.colors
                    .iter()
                    .all(|color| color[1] > color[0] * 1.4 && color[1] > color[2] * 3.0)
            );
            assert!(batches.iter().all(|batch| {
                batch
                    .indices
                    .iter()
                    .all(|&index| (index as usize) < batch.positions.len())
            }));
        }
    }

    fn queue_fixture() -> App {
        let mut app = App::new();
        app.init_resource::<RenderBlockPreviews>()
            .init_resource::<RenderPropPreviews>()
            .init_resource::<RenderItemPreviews>()
            .add_systems(Update, render_next);
        let mut jobs = VecDeque::new();
        for material in [7, 8] {
            let root = app.world_mut().spawn(Visibility::Visible).id();
            jobs.push_back(PreviewJob {
                item: ItemKind::Block(material),
                root,
                image: Handle::default(),
            });
        }
        app.world_mut().spawn((
            PreviewCamera,
            Camera::default(),
            RenderTarget::Image(Handle::<Image>::default().into()),
        ));
        app.world_mut()
            .resource_mut::<RenderBlockPreviews>()
            .pending = jobs.len();
        app.insert_resource(PreviewQueue {
            jobs,
            started: Some(Instant::now()),
            ..default()
        });
        app
    }

    #[test]
    fn preview_camera_pauses_middle_warmup_frames_and_reactivates_before_the_eighth_frame_capture()
    {
        let mut app = queue_fixture();
        for (index, expected) in [true, true, false, false, false, false, true, true]
            .into_iter()
            .enumerate()
        {
            app.update();
            assert_eq!(
                app.world_mut()
                    .query::<&Camera>()
                    .single(app.world())
                    .unwrap()
                    .is_active,
                expected,
                "warmup frame {}",
                index + 1
            );
            assert_eq!(
                app.world().resource::<PreviewQueue>().capture.is_some(),
                index == 7
            );
        }
        let capture = app.world().resource::<PreviewQueue>().capture.unwrap().0;
        for _ in 0..6 {
            app.update();
            assert!(
                app.world_mut()
                    .query::<&Camera>()
                    .single(app.world())
                    .unwrap()
                    .is_active
            );
            assert_eq!(
                app.world().resource::<PreviewQueue>().capture.unwrap().0,
                capture
            );
        }
        app.world_mut().resource_mut::<PreviewQueue>().received = Some(false);
        app.update();
        app.world_mut().resource_mut::<PreviewQueue>().retry_at =
            Some(Instant::now() + Duration::from_secs(60));
        for _ in 0..6 {
            app.update();
            assert!(
                app.world_mut()
                    .query::<&Camera>()
                    .single(app.world())
                    .unwrap()
                    .is_active
            );
            assert!(app.world().resource::<PreviewQueue>().capture.is_none());
            assert!(app.world().resource::<PreviewQueue>().retry_at.is_some());
        }
    }

    fn lazy_queue_fixture(items: VecDeque<ItemKind>) -> App {
        let mut templates = TemplateSet::default();
        for id in [3, 4, 5] {
            templates.blocks.insert(
                id,
                crate::blocks::BlockTemplate {
                    surface: None,
                    render_kind: crate::blocks::definitions::BlockRenderKind::Cube,
                    bevel_width: 0.0,
                    faces: crate::blocks::cube_faces(0.0, [0; 6], None),
                    face_materials: [0; 6],
                    side_cap: None,
                    details: Vec::new(),
                    model: Vec::new(),
                },
            );
        }
        let mut blocks = BlockAssets::default();
        blocks.templates = Some(std::sync::Arc::new(templates));
        blocks.materials = vec![Handle::default()];
        let mut app = App::new();
        app.insert_resource(blocks)
            .insert_resource(WaterAssets(Handle::default()))
            .init_resource::<crate::foliage::FoliageAssets>()
            .init_resource::<Assets<Image>>()
            .init_resource::<Assets<Mesh>>()
            .init_resource::<RenderBlockPreviews>()
            .init_resource::<RenderPropPreviews>()
            .init_resource::<RenderItemPreviews>()
            .insert_resource(PreviewQueue {
                initialized: true,
                items,
                ..default()
            })
            .add_systems(Update, (prepare, render_next).chain());
        app
    }

    #[test]
    fn demand_only_smoke_initializes_without_catalog_jobs_meshes_targets_or_cameras() {
        let mut app = lazy_queue_fixture(VecDeque::new());
        app.world_mut().resource_mut::<PreviewQueue>().initialized = false;
        app.update();
        let queue = app.world().resource::<PreviewQueue>();
        assert!(queue.initialized);
        assert_eq!(queue.pending(), 0);
        assert!(!queue.warmup);
        assert_eq!(app.world().resource::<Assets<Mesh>>().len(), 0);
        assert_eq!(app.world().resource::<Assets<Image>>().len(), 0);
        assert_eq!(
            app.world_mut()
                .query::<&PreviewScene>()
                .iter(app.world())
                .count(),
            0
        );
    }

    #[test]
    fn lazy_previews_admit_one_item_per_frame_and_publish_every_independent_image() {
        let items = VecDeque::from([
            ItemKind::Block(3),
            ItemKind::Block(4),
            ItemKind::Block(5),
            ItemKind::Dye(terra_core::beds::DyeColor::Red),
            ItemKind::Stick,
            ItemKind::Tool(terra_core::ToolKind::Wrench),
        ]);
        let requested = items.len();
        let mut app = lazy_queue_fixture(items);
        app.update();
        assert_eq!(app.world().resource::<Assets<Mesh>>().len(), 1);
        assert_eq!(app.world().resource::<Assets<Image>>().len(), 1);
        assert_eq!(
            app.world().resource::<RenderBlockPreviews>().pending,
            requested
        );
        app.update();
        assert_eq!(app.world().resource::<Assets<Mesh>>().len(), 2);
        assert_eq!(
            app.world().resource::<Assets<Image>>().len(),
            PREVIEW_PREFETCH
        );
        app.update();
        assert_eq!(app.world().resource::<Assets<Mesh>>().len(), 2);
        assert_eq!(
            app.world().resource::<Assets<Image>>().len(),
            PREVIEW_PREFETCH
        );
        let mut completed = Vec::new();
        for remaining in (0..requested).rev() {
            let job = app.world().resource::<PreviewQueue>().jobs.front().unwrap();
            completed.push((job.item, job.image.clone()));
            let meshes_before = app.world().resource::<Assets<Mesh>>().len();
            app.world_mut().resource_mut::<PreviewQueue>().received = Some(true);
            app.update();
            assert_eq!(
                app.world().resource::<RenderBlockPreviews>().pending,
                remaining
            );
            assert!(app.world().resource::<Assets<Mesh>>().len() <= meshes_before + 1);
            assert!(app.world().resource::<PreviewQueue>().jobs.len() <= PREVIEW_PREFETCH);
            assert!(
                app.world_mut().query::<&Mesh3d>().iter(app.world()).count() <= PREVIEW_PREFETCH
            );
        }
        let mut image_ids = std::collections::HashSet::new();
        for (item, expected) in completed {
            let actual = match item {
                ItemKind::Block(id) => &app.world().resource::<RenderBlockPreviews>().images[&id],
                _ => &app.world().resource::<RenderItemPreviews>().images[&item],
            };
            assert_eq!(actual, &expected);
            assert!(image_ids.insert(actual.id()));
        }
        assert!(
            app.world()
                .resource::<RenderBlockPreviews>()
                .error
                .is_none()
        );
        assert_eq!(
            app.world_mut()
                .query::<&PreviewScene>()
                .iter(app.world())
                .count(),
            0,
            "completed icons must leave no preview camera, lights or meshes"
        );
    }

    #[test]
    fn a_warmed_preview_captures_after_two_active_frames_without_overwriting_a_published_image() {
        let mut app = lazy_queue_fixture(VecDeque::from([ItemKind::Block(3), ItemKind::Block(4)]));
        for _ in 0..3 {
            app.update();
        }
        assert!(
            !app.world_mut()
                .query::<&Camera>()
                .single(app.world())
                .unwrap()
                .is_active
        );
        let queue = app.world().resource::<PreviewQueue>();
        let previous = queue.jobs[0].image.clone();
        let next = queue.jobs[1].image.clone();
        assert_ne!(previous, next);
        app.world_mut().resource_mut::<PreviewQueue>().received = Some(true);
        app.update();
        assert_eq!(app.world().resource::<PreviewQueue>().frames, 1);
        assert_eq!(
            app.world().resource::<RenderBlockPreviews>().images[&3],
            previous
        );
        let (target, camera) = app
            .world_mut()
            .query_filtered::<(&RenderTarget, &Camera), With<PreviewCamera>>()
            .single(app.world())
            .unwrap();
        assert!(camera.is_active);
        let RenderTarget::Image(target) = target else {
            panic!("preview camera must target the next independent image");
        };
        assert_eq!(target.handle, next);
        app.update();
        assert!(
            app.world_mut()
                .query::<&Camera>()
                .single(app.world())
                .unwrap()
                .is_active
        );
        assert!(app.world().resource::<PreviewQueue>().capture.is_some());
        app.world_mut().resource_mut::<PreviewQueue>().received = Some(false);
        app.update();
        assert!(
            app.world_mut()
                .query::<&Camera>()
                .single(app.world())
                .unwrap()
                .is_active
        );
        assert!(app.world().resource::<PreviewQueue>().capture.is_none());
        assert!(app.world().resource::<PreviewQueue>().retry_at.is_some());
        assert!(
            !app.world()
                .resource::<RenderBlockPreviews>()
                .images
                .contains_key(&4)
        );
        assert_eq!(
            app.world().resource::<RenderBlockPreviews>().images[&3],
            previous
        );
    }

    #[test]
    fn failed_lazy_compositions_pause_the_camera_and_do_not_clear_the_published_target() {
        let mut app = lazy_queue_fixture(VecDeque::from([
            ItemKind::Block(3),
            ItemKind::Block(u16::MAX),
            ItemKind::Block(u16::MAX),
            ItemKind::Block(4),
        ]));
        app.update();
        let first_image = app.world().resource::<PreviewQueue>().jobs[0].image.clone();
        app.world_mut().resource_mut::<PreviewQueue>().received = Some(true);
        app.update();
        assert_eq!(
            app.world().resource::<RenderBlockPreviews>().images[&3],
            first_image
        );
        assert_eq!(app.world().resource::<RenderBlockPreviews>().pending, 2);
        let (target, camera) = app
            .world_mut()
            .query_filtered::<(&RenderTarget, &Camera), With<PreviewCamera>>()
            .single(app.world())
            .unwrap();
        assert!(!camera.is_active);
        let RenderTarget::Image(target) = target else {
            panic!("preview camera must retain an image target");
        };
        assert_eq!(target.handle, first_image);
        app.update();
        assert_eq!(app.world().resource::<RenderBlockPreviews>().pending, 1);
        assert!(app.world().resource::<PreviewQueue>().jobs.is_empty());
        app.update();
        let (target, camera) = app
            .world_mut()
            .query_filtered::<(&RenderTarget, &Camera), With<PreviewCamera>>()
            .single(app.world())
            .unwrap();
        assert!(camera.is_active);
        let RenderTarget::Image(target) = target else {
            panic!("preview camera must switch to the next image target");
        };
        assert_ne!(target.handle, first_image);
        app.world_mut().resource_mut::<PreviewQueue>().received = Some(true);
        app.update();
        assert_eq!(app.world().resource::<RenderBlockPreviews>().pending, 0);
        assert_eq!(
            app.world().resource::<RenderBlockPreviews>().images[&3],
            first_image
        );
        assert!(
            app.world()
                .resource::<RenderBlockPreviews>()
                .images
                .contains_key(&4)
        );
    }

    #[test]
    fn a_failed_final_lazy_item_still_finishes_and_releases_the_preview_scene() {
        let mut app = lazy_queue_fixture(VecDeque::from([
            ItemKind::Block(3),
            ItemKind::Block(u16::MAX),
            ItemKind::Block(u16::MAX),
        ]));
        app.update();
        app.world_mut().resource_mut::<PreviewQueue>().received = Some(true);
        app.update();
        assert_eq!(app.world().resource::<RenderBlockPreviews>().pending, 1);
        app.update();
        assert_eq!(app.world().resource::<RenderBlockPreviews>().pending, 0);
        assert!(
            app.world()
                .resource::<RenderBlockPreviews>()
                .images
                .contains_key(&3)
        );
        assert!(
            app.world()
                .resource::<RenderBlockPreviews>()
                .error
                .is_some()
        );
        assert_eq!(
            app.world_mut()
                .query::<&PreviewScene>()
                .iter(app.world())
                .count(),
            0
        );
    }

    #[test]
    fn all_minerals_have_bounded_real_3d_item_geometry_using_their_shared_material() {
        let mut templates = TemplateSet::default();
        for (slot, ore) in terra_core::OreKind::ALL.into_iter().enumerate() {
            templates.ore_materials.insert(ore.material(), slot);
        }
        for item in terra_core::MineralItem::ALL {
            let batches = mineral_batches(item, &templates).unwrap();
            assert_eq!(batches.len(), 1);
            let batch = &batches[0];
            assert_eq!(
                batch.material,
                if matches!(
                    item,
                    terra_core::MineralItem::IronIngot | terra_core::MineralItem::GoldIngot
                ) {
                    templates.item_materials.metal
                } else {
                    templates.ore_materials[&item.ore().material()]
                }
            );
            assert_eq!(batch.positions.len(), batch.normals.len());
            assert_eq!(batch.positions.len(), batch.uvs.len());
            assert!(
                batch
                    .positions
                    .iter()
                    .flatten()
                    .all(|v| v.is_finite() && (-0.001..=1.001).contains(v))
            );
            assert!(
                batch
                    .indices
                    .iter()
                    .all(|&index| (index as usize) < batch.positions.len())
            );
            for axis in 0..3 {
                let minimum = batch
                    .positions
                    .iter()
                    .map(|point| point[axis])
                    .fold(f32::INFINITY, f32::min);
                let maximum = batch
                    .positions
                    .iter()
                    .map(|point| point[axis])
                    .fold(f32::NEG_INFINITY, f32::max);
                assert!(
                    maximum - minimum > 0.1,
                    "mineral items must not be flat billboards"
                );
            }
        }
    }

    #[test]
    fn blank_retries_allow_shader_readiness_by_time_not_frame_or_attempt_count() {
        let mut app = queue_fixture();
        for _ in 0..20 {
            {
                let mut queue = app.world_mut().resource_mut::<PreviewQueue>();
                queue.received = Some(false);
                queue.frames = u16::MAX;
            }
            app.update();
            let queue = app.world().resource::<PreviewQueue>();
            assert_eq!(queue.jobs.front().unwrap().item, ItemKind::Block(7));
            assert!(
                queue.capture.is_none(),
                "blank readbacks must be spaced out"
            );
            assert!(
                app.world()
                    .resource::<RenderBlockPreviews>()
                    .error
                    .is_none()
            );
        }
        app.world_mut().resource_mut::<PreviewQueue>().started =
            Some(Instant::now() - PREVIEW_GRACE - Duration::from_secs(1));
        app.update();
        let previews = app.world().resource::<RenderBlockPreviews>();
        assert!(previews.images.is_empty());
        assert!(previews.error.as_ref().unwrap().contains("remained blank"));
        assert_eq!(previews.pending, 1);
        assert_eq!(
            app.world()
                .resource::<PreviewQueue>()
                .jobs
                .front()
                .unwrap()
                .item,
            ItemKind::Block(8)
        );
    }

    #[test]
    fn slow_gpu_readback_retains_one_request_and_publishes_its_late_success() {
        let mut app = queue_fixture();
        let (sender, receiver) = std::sync::mpsc::channel();
        app.insert_resource(bevy::render::view::screenshot::CapturedScreenshots(
            std::sync::Arc::new(std::sync::Mutex::new(receiver)),
        ))
        .add_systems(
            Update,
            bevy::render::view::screenshot::trigger_screenshots.before(render_next),
        )
        .add_systems(
            First,
            |mut commands: Commands,
             captured: Query<Entity, With<bevy::render::view::screenshot::Captured>>| {
                for entity in &captured {
                    commands.entity(entity).despawn();
                }
            },
        );
        app.world_mut().resource_mut::<PreviewQueue>().frames = u16::MAX;
        app.update();
        let capture = app.world().resource::<PreviewQueue>().capture.unwrap().0;
        app.world_mut()
            .entity_mut(capture)
            .insert(bevy::render::view::screenshot::Capturing);
        {
            let mut queue = app.world_mut().resource_mut::<PreviewQueue>();
            let old = Instant::now() - PREVIEW_GRACE - CAPTURE_TIMEOUT - Duration::from_secs(1);
            queue.started = Some(old);
            queue.capture = Some((capture, old));
        }
        for _ in 0..20 {
            app.update();
            let queue = app.world().resource::<PreviewQueue>();
            assert_eq!(queue.capture.unwrap().0, capture);
            assert_eq!(queue.jobs.len(), 2);
            assert_eq!(queue.jobs.front().unwrap().item, ItemKind::Block(7));
            assert!(queue.capture_delayed);
            assert_eq!(
                app.world_mut()
                    .query::<&Screenshot>()
                    .iter(app.world())
                    .count(),
                1
            );
            assert!(
                app.world()
                    .get::<bevy::render::view::screenshot::Capturing>(capture)
                    .is_some()
            );
        }
        let previews = app.world().resource::<RenderBlockPreviews>();
        assert!(previews.images.is_empty());
        assert!(previews.error.is_none());
        assert_eq!(previews.pending, 2);
        let mut image = Image::new_target_texture(16, 16, TextureFormat::Rgba8UnormSrgb, None);
        image.data = Some(vec![255; 16 * 16 * 4]);
        sender.send((capture, image)).unwrap();
        app.update();
        let previews = app.world().resource::<RenderBlockPreviews>();
        assert!(previews.images.contains_key(&7));
        assert!(previews.error.is_none());
        assert_eq!(previews.pending, 1);
        assert!(
            app.world()
                .get::<bevy::render::view::screenshot::Captured>(capture)
                .is_some(),
            "only Screenshot's completed-capture cleanup owns this entity"
        );
        app.update();
        assert!(app.world().get_entity(capture).is_err());
    }

    #[test]
    fn queued_gpu_completion_is_consumed_before_wall_clock_deadlines() {
        let mut app = queue_fixture();
        let (sender, receiver) = std::sync::mpsc::channel();
        app.insert_resource(bevy::render::view::screenshot::CapturedScreenshots(
            std::sync::Arc::new(std::sync::Mutex::new(receiver)),
        ))
        .add_systems(
            Update,
            bevy::render::view::screenshot::trigger_screenshots.before(render_next),
        );
        app.world_mut().resource_mut::<PreviewQueue>().frames = u16::MAX;
        app.update();
        let capture = app.world().resource::<PreviewQueue>().capture.unwrap().0;
        {
            let mut queue = app.world_mut().resource_mut::<PreviewQueue>();
            let old = Instant::now() - PREVIEW_GRACE - Duration::from_secs(1);
            queue.started = Some(old);
            queue.capture = Some((capture, old));
        }
        let mut image = Image::new_target_texture(16, 16, TextureFormat::Rgba8UnormSrgb, None);
        image.data = Some(vec![255; 16 * 16 * 4]);
        sender.send((capture, image)).unwrap();
        app.update();
        assert!(
            app.world()
                .resource::<RenderBlockPreviews>()
                .images
                .contains_key(&7)
        );
        assert!(
            app.world()
                .resource::<RenderBlockPreviews>()
                .error
                .is_none()
        );
        assert!(!app.world().resource::<PreviewQueue>().capture_delayed);
    }

    #[test]
    fn completed_real_capture_is_published_even_when_ready_at_the_deadline() {
        let mut app = queue_fixture();
        {
            let mut queue = app.world_mut().resource_mut::<PreviewQueue>();
            queue.started = Some(Instant::now() - PREVIEW_GRACE);
            queue.received = Some(true);
        }
        app.update();
        let previews = app.world().resource::<RenderBlockPreviews>();
        assert!(previews.images.contains_key(&7));
        assert!(previews.error.is_none());
    }

    #[test]
    fn blank_transparent_capture_is_not_published_as_a_finished_preview() {
        let mut image = Image::new_target_texture(16, 16, TextureFormat::Rgba8UnormSrgb, None);
        image.data = Some(vec![0; 16 * 16 * 4]);
        assert!(!preview_has_pixels(&image));
        image.data = Some(vec![255; 16 * 16 * 4]);
        assert!(preview_has_pixels(&image));
    }

    #[test]
    fn every_prop_preview_uses_valid_fitted_near_geometry() {
        for kind in [
            ResourcePropKind::Grass,
            ResourcePropKind::Flower,
            ResourcePropKind::Bush,
            ResourcePropKind::Fern,
            ResourcePropKind::Mushroom,
            ResourcePropKind::SmallPlant,
        ] {
            let batches = prop_batches(kind, &TemplateSet::default()).unwrap();
            assert!(!batches.is_empty());
            for batch in batches {
                assert!(!batch.indices.is_empty());
                assert!(
                    batch
                        .indices
                        .iter()
                        .all(|&index| (index as usize) < batch.positions.len())
                );
                assert!(
                    batch
                        .positions
                        .iter()
                        .flatten()
                        .all(|v| v.is_finite() && (-0.001..=1.001).contains(v))
                );
            }
        }
    }

    #[test]
    fn captured_prop_is_published_to_prop_registry_not_block_registry() {
        let mut app = queue_fixture();
        {
            let mut queue = app.world_mut().resource_mut::<PreviewQueue>();
            queue.jobs.front_mut().unwrap().item = ItemKind::Prop(ResourcePropKind::Flower);
            queue.received = Some(true);
        }
        app.update();
        assert!(
            app.world()
                .resource::<RenderPropPreviews>()
                .images
                .contains_key(&ResourcePropKind::Flower)
        );
        assert!(
            app.world()
                .resource::<RenderBlockPreviews>()
                .images
                .is_empty()
        );
    }

    #[test]
    fn one_cell_machine_icons_keep_body_proportions_and_include_their_moving_parts() {
        use crate::blocks::{BlockTemplate, definitions::BlockRenderKind};
        let mut templates = TemplateSet::default();
        for (material, paddle, cog) in [(99, 0, 2), (156, 14, 15)] {
            let mut model = Vec::new();
            for (slot, minimum, maximum) in [
                (0, [0.1, 0.0, 0.1], [0.9, 1.0, 0.9]),
                (1, [0.3; 3], [0.7; 3]),
                (2, [0.91, 0.35, 0.35], [1.0, 0.65, 0.65]),
            ] {
                model.extend(crate::blocks::cube::box_model(
                    &[terra_core::block_shapes::ShapeBox {
                        minimum,
                        maximum,
                        hinged: false,
                    }],
                    0.0,
                    [slot; 6],
                ));
            }
            templates
                .crusher_moving_materials
                .extend([(1, paddle), (2, cog)]);
            templates.blocks.insert(
                material,
                BlockTemplate {
                    surface: None,
                    render_kind: BlockRenderKind::CustomMesh,
                    bevel_width: 0.0,
                    faces: std::array::from_fn(|_| Vec::new()),
                    face_materials: [0; 6],
                    side_cap: None,
                    details: Vec::new(),
                    model,
                },
            );
        }
        let options = RenderOptions {
            water_level: None,
            detail_level: DetailLevel::Near,
            terrain: None,
            exposure: None,
            conditions: default(),
            tints: default(),
        };
        for material in [99, 156] {
            let batches = block_batches(material, &templates, &options).unwrap();
            for slot in 0..3 {
                assert!(batches.iter().any(|batch| batch.material == slot));
            }
            let mut low = Vec3::splat(f32::INFINITY);
            let mut high = Vec3::splat(f32::NEG_INFINITY);
            for point in batches.iter().flat_map(|batch| &batch.positions) {
                low = low.min(Vec3::from_array(*point));
                high = high.max(Vec3::from_array(*point));
            }
            assert!((high.y - low.y - 1.0).abs() < 0.00001);
            assert!((high.x - low.x - 0.9).abs() < 0.00001);
            assert!((high.z - low.z - 0.8).abs() < 0.00001);
        }
    }

    #[test]
    fn kinetic_icons_keep_their_own_rotor_material_and_fit_extended_silhouettes() {
        use crate::blocks::{BlockTemplate, definitions::BlockRenderKind};
        let mut templates = TemplateSet::default();
        let ids = [141, 142, 143, 149, 150, 151, 147];
        for id in ids {
            let radius = f64::from(terra_core::kinetic::radial_extent(id).unwrap());
            let moving_slot = usize::from(id);
            let mut model = crate::blocks::cube::box_model(
                &[terra_core::block_shapes::ShapeBox {
                    minimum: [0.35, 0.5 - radius, 0.5 - radius],
                    maximum: [0.65, 0.5 + radius, 0.5 + radius],
                    hinged: false,
                }],
                0.0,
                [moving_slot; 6],
            );
            model.extend(crate::blocks::cube::box_model(
                &[terra_core::block_shapes::ShapeBox {
                    minimum: [0.45; 3],
                    maximum: [0.55; 3],
                    hinged: false,
                }],
                0.0,
                [0; 6],
            ));
            templates
                .crusher_moving_materials
                .insert(moving_slot, if id == 147 { 6 } else { 4 });
            templates.blocks.insert(
                id,
                BlockTemplate {
                    surface: None,
                    render_kind: BlockRenderKind::CustomMesh,
                    bevel_width: 0.0,
                    faces: std::array::from_fn(|_| Vec::new()),
                    face_materials: [0; 6],
                    side_cap: None,
                    details: Vec::new(),
                    model,
                },
            );
        }
        let options = RenderOptions {
            water_level: None,
            detail_level: DetailLevel::Near,
            terrain: None,
            exposure: None,
            conditions: default(),
            tints: default(),
        };
        for id in ids {
            let batches = block_batches(id, &templates, &options).unwrap();
            assert!(
                batches
                    .iter()
                    .any(|batch| batch.material == usize::from(id)),
                "a gear icon must include its material-specific rotor, not only a shared frame"
            );
            assert!(
                batches
                    .iter()
                    .all(|batch| batch.material == 0 || batch.material == usize::from(id))
            );
            let mut low = Vec3::splat(f32::INFINITY);
            let mut high = Vec3::splat(f32::NEG_INFINITY);
            for point in batches.iter().flat_map(|batch| &batch.positions) {
                let point = Vec3::from_array(*point);
                low = low.min(point);
                high = high.max(point);
                assert!(point.cmpge(Vec3::ZERO).all() && point.cmple(Vec3::ONE).all());
            }
            assert!(((high - low).max_element() - 1.0).abs() < 0.00001);
            assert!(((low + high) * 0.5 - Vec3::splat(0.5)).length() < 0.00001);
        }
    }
}
