//! One-time, isolated renders of the real block meshes and materials for TiltUI.
mod bellows;
mod crops;
mod dust;
mod hardware;
mod products;
mod sculpt;
mod showcase;
pub(crate) mod tools;
use std::{
    collections::{HashMap, VecDeque},
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
    jobs: VecDeque<PreviewJob>,
    frames: u16,
    started: Option<Instant>,
    capture: Option<(Entity, Instant)>,
    capture_delayed: bool,
    retry_at: Option<Instant>,
    received: Option<bool>,
}

pub(crate) fn install(app: &mut App) {
    app.init_resource::<RenderBlockPreviews>()
        .init_resource::<RenderPropPreviews>()
        .init_resource::<RenderItemPreviews>()
        .init_resource::<PreviewQueue>();
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
            prepare.after(crate::foliage::prepare_materials),
            render_next.after(bevy::render::view::screenshot::trigger_screenshots),
        )
            .chain(),
    );
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
) {
    if queue.initialized {
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
    queue.initialized = true;
    let options = RenderOptions {
        water_level: None,
        detail_level: DetailLevel::Near,
        terrain: assets.terrain.as_ref().map(|terrain| terrain.0.clone()),
        exposure: None,
        conditions: default(),
    };
    let mut materials: Vec<_> = templates.blocks.keys().copied().collect();
    materials.sort_unstable();
    let items = materials
        .into_iter()
        .map(ItemKind::Block)
        .chain(ResourcePropKind::ALL.into_iter().map(ItemKind::Prop))
        .chain(ItemKind::EXTRA)
        .chain(
            terra_core::CapeItem::ALL
                .into_iter()
                .filter(|cape| cape.color != terra_core::beds::DyeColor::White)
                .map(ItemKind::Cape),
        )
        .chain((71..=73).flat_map(|material| {
            terra_core::beds::DyeColor::ALL.map(move |color| ItemKind::DyedBed { material, color })
        }))
        .chain([28, 29].into_iter().flat_map(|material| {
            [
                terra_core::WoodSpecies::Birch,
                terra_core::WoodSpecies::Spruce,
                terra_core::WoodSpecies::Cherry,
            ]
            .map(move |wood| ItemKind::for_wood(material, wood))
        }));
    for item in items {
        let batches = match match item {
            ItemKind::Block(material) => block_batches(material, &templates, &options),
            ItemKind::WoodenBlock { .. } => wooden_block_batches(item, &templates, &options),
            ItemKind::DyedBed { material, color } => {
                dyed_bed_batches(material, color, &templates, &options)
            }
            ItemKind::Dye(color) => dye_batches(color, &templates),
            ItemKind::Prop(kind) => prop_batches(kind, &templates),
            ItemKind::Mineral(kind) => mineral_batches(kind, &templates),
            ItemKind::Animal(kind) => animal_batches(kind, &templates),
            ItemKind::Bellows(kind) => bellows_batches(kind, &templates),
            ItemKind::Armor(_) | ItemKind::Accessory(_) | ItemKind::Cape(_) => {
                crate::armor::item_batches(item, templates.item_materials.opaque)
            }
            ItemKind::Dust(kind) => dust::batches(kind, templates.item_materials.opaque),
            ItemKind::Crop(kind) => crop_batches(kind, &templates),
            ItemKind::Flower(color) => flower_batches(color, &templates),
            ItemKind::Tool(_) | ItemKind::Stick => tool_batches(item, &templates),
        } {
            Ok(batches) => batches,
            Err(error) => {
                previews.error = Some(error.clone());
                tracing::error!(%error, ?item, "Cannot compose inventory item preview");
                continue;
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
    }
    let Some(first) = queue.jobs.front() else {
        return;
    };
    commands.entity(first.root).insert(Visibility::Visible);
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
    previews.pending = queue.jobs.len();
}

fn render_next(
    mut commands: Commands,
    mut queue: ResMut<PreviewQueue>,
    mut previews: ResMut<RenderBlockPreviews>,
    mut prop_previews: ResMut<RenderPropPreviews>,
    mut item_previews: ResMut<RenderItemPreviews>,
    mut cameras: Query<&mut RenderTarget, With<PreviewCamera>>,
    scene: Query<Entity, With<PreviewScene>>,
) {
    if queue.jobs.is_empty() || cameras.is_empty() {
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
            if rendered == Some(true) {
                match job.item {
                    ItemKind::Block(material) => {
                        previews.images.insert(material, job.image);
                    }
                    ItemKind::Prop(kind) => {
                        prop_previews.images.insert(kind, job.image);
                    }
                    ItemKind::Tool(_)
                    | ItemKind::Stick
                    | ItemKind::WoodenBlock { .. }
                    | ItemKind::Mineral(_)
                    | ItemKind::Bellows(_)
                    | ItemKind::Dye(_)
                    | ItemKind::DyedBed { .. }
                    | ItemKind::Armor(_)
                    | ItemKind::Accessory(_)
                    | ItemKind::Cape(_)
                    | ItemKind::Dust(_)
                    | ItemKind::Crop(_)
                    | ItemKind::Flower(_)
                    | ItemKind::Animal(_) => {
                        item_previews.images.insert(job.item, job.image);
                    }
                }
            } else {
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
        previews.pending = queue.jobs.len();
        if let Some(job) = queue.jobs.front() {
            commands.entity(job.root).insert(Visibility::Visible);
            for mut target in &mut cameras {
                *target = RenderTarget::Image(job.image.clone().into());
            }
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
    if queue.capture.is_some()
        || queue.frames < 8
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
                uvs: primitive.uvs.clone(),
                indices: primitive.indices.clone(),
                ..default()
            })
            .collect()
    } else if material == 29 && templates.animated_chests {
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
    if material == terra_core::crusher::MATERIAL {
        for batch in &mut batches {
            for point in &mut batch.positions {
                point[0] *= 2.0;
            }
            for normal in &mut batch.normals {
                *normal = (Vec3::from_array(*normal) / Vec3::new(2.0, 1.0, 1.0))
                    .normalize_or_zero()
                    .to_array();
            }
        }
    }
    if matches!(material, 59 | 65..=67 | 71..=73 | 99..=100 | 139)
        || terra_core::OreKind::from_material(material).is_some()
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
        return Err("Invalid mineral preview bounds".into());
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
            batch.colors = vec![color.tint(); batch.positions.len()];
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
}
