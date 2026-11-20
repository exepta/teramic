use super::{
    BlockTemplate, DetailTemplate, ItemMaterials, PrimitiveTemplate, PropMaterials, SideCap,
    TemplateSet,
    cube::cube_faces,
    definitions::{BlockLibrary, BlockRenderKind, CubeMaterialDefinition, ModelLayout},
};
use bevy::{
    asset::{LoadState, RecursiveDependencyLoadState, RenderAssetUsages},
    ecs::system::SystemParam,
    gltf::{Gltf, GltfLoaderSettings, GltfMesh, GltfNode},
    image::{ImageAddressMode, ImageLoaderSettings, ImageSampler, ImageSamplerDescriptor},
    mesh::VertexAttributeValues,
    prelude::*,
    tasks::{AsyncComputeTaskPool, Task, futures_lite::future},
};
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

#[cfg(test)]
#[path = "loading_tests.rs"]
mod tests;

#[cfg(test)]
pub(crate) fn test_static_scene(path: &'static str, id: u16) -> Vec<PrimitiveTemplate> {
    tests::load_static_scene(path, id)
}

#[cfg(test)]
#[path = "authored_terrain_tests.rs"]
pub(crate) mod authored_terrain_tests;

#[derive(Resource, Default)]
pub(crate) struct BlockAssets {
    library: Option<BlockLibrary>,
    models: HashMap<u16, Handle<Gltf>>,
    material_slots: HashMap<(u16, usize), usize>,
    cube_material_slots: HashMap<(u16, String), usize>,
    shared_materials: HashMap<MaterialKey, usize>,
    prop_materials: PropMaterials,
    item_materials: ItemMaterials,
    textures: HashMap<String, Handle<Image>>,
    patterns: HashMap<super::definitions::TexturePattern, Handle<Image>>,
    mipmap_jobs: HashMap<String, Task<Result<Image, String>>>,
    mipmaps_ready: HashSet<String>,
    pub materials: Vec<Handle<StandardMaterial>>,
    pub templates: Option<Arc<TemplateSet>>,
    pub error: Option<String>,
}

impl BlockAssets {
    pub(crate) fn startup_path(
        &self,
        item: Option<terra_core::ItemKind>,
        server: Option<&AssetServer>,
    ) -> String {
        if let Some(server) = server {
            for handle in self.models.values() {
                if !server.is_loaded_with_dependencies(handle.id()) {
                    return server
                        .get_path(handle.id())
                        .map_or_else(String::new, |path| path.to_string());
                }
            }
            for (path, handle) in &self.textures {
                if !server.is_loaded_with_dependencies(handle.id())
                    || !self.mipmaps_ready.contains(path)
                {
                    return server
                        .get_path(handle.id())
                        .map_or_else(|| path.clone(), |path| path.to_string());
                }
            }
        }
        if let Some(item) = item {
            if let Some(block) = item.block_material().and_then(|id| {
                self.library
                    .as_ref()?
                    .blocks
                    .iter()
                    .find(|block| block.id == id)
            }) && !block.model.is_empty()
            {
                return format!("Icon: terra/blocks/{}", block.model);
            }
            // Registry identifiers name existing independent definition files.
            // Variants share that source; only their icon composition differs.
            let base = item.block_material().map_or_else(
                || item.preview_item().identifier(),
                |id| terra_core::ItemKind::Block(id).identifier(),
            );
            let slug = base
                .strip_prefix("teramic::")
                .unwrap_or(&base)
                .split('#')
                .next()
                .unwrap_or(&base);
            let category = if item.block_material().is_some() {
                "blocks"
            } else {
                "items"
            };
            let path = format!("terra/{category}/{slug}.json");
            if terra_core::asset_directory().join(&path).is_file() {
                return format!("Icon: {path}");
            }
            return format!("Inventar-Icon: {}", item.identifier());
        }
        "terra/blocks".into()
    }
    pub(crate) fn lights(
        &self,
    ) -> impl Iterator<Item = (u16, super::definitions::BlockLight)> + '_ {
        self.library
            .iter()
            .flat_map(|library| library.blocks.iter())
            .filter_map(|block| block.light.map(|light| (block.id, light)))
    }
    pub(crate) fn cube_face_definition(
        &self,
        id: u16,
        side: usize,
    ) -> Option<&CubeMaterialDefinition> {
        let block = self
            .library
            .as_ref()?
            .blocks
            .iter()
            .find(|block| block.id == id)?;
        let name = block.face_materials.as_ref()?.get(side)?;
        block.materials.get(name)
    }
}

pub(crate) fn load_blocks(
    server: Res<AssetServer>,
    mut assets: ResMut<BlockAssets>,
    mut palette: ResMut<terra_player::interaction::BlockPalette>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
) {
    match BlockLibrary::load(&terra_core::asset_directory().join("terra/blocks")) {
        Ok(library) => {
            palette.choices = library
                .blocks
                .iter()
                .map(|b| terra_player::interaction::BlockChoice {
                    id: b.id,
                    name: b.name.clone(),
                    identifier: terra_core::ItemKind::Block(b.id).identifier(),
                    description: b.description.clone(),
                })
                .collect();
            palette.item_categories = library
                .blocks
                .iter()
                .map(|block| {
                    (
                        terra_core::ItemKind::Block(block.id),
                        super::definitions::item_category(
                            terra_core::ItemKind::Block(block.id),
                            block.creative_category.as_deref(),
                        ),
                    )
                })
                .collect();
            match super::definitions::load_prop_metadata(
                &terra_core::asset_directory().join("terra/props"),
            ) {
                Ok(metadata) => {
                    palette.prop_descriptions = metadata.descriptions;
                    palette
                        .item_categories
                        .extend(metadata.categories.into_iter().map(|(item, category)| {
                            (
                                item,
                                super::definitions::item_category(item, Some(&category)),
                            )
                        }));
                }
                Err(error) => {
                    tracing::error!(%error, "Cannot load prop item descriptions");
                    assets.error = Some(error);
                    return;
                }
            }
            match super::definitions::load_tool_metadata(
                &terra_core::asset_directory().join("terra/items"),
            ) {
                Ok(metadata) => {
                    for (item, name, description) in metadata {
                        palette.item_metadata.insert(item, (name, description));
                        palette
                            .item_categories
                            .insert(item, super::definitions::item_category(item, None));
                    }
                }
                Err(error) => {
                    tracing::error!(%error, "Cannot load item metadata");
                    assets.error = Some(error);
                    return;
                }
            }
            match super::definitions::load_creative_categories(
                &terra_core::asset_directory().join("terra/inventory/categories"),
                &library,
                &palette.item_categories,
            ) {
                Ok(categories) => palette.creative_categories = categories,
                Err(error) => {
                    tracing::error!(%error, "Cannot load creative categories");
                    assets.error = Some(error);
                    return;
                }
            }
            palette.selected = palette.choices.iter().position(|b| b.id == 3).unwrap_or(0);
            assets.models = library
                .blocks
                .iter()
                .filter(|b| b.render_kind == BlockRenderKind::CustomMesh)
                .map(|b| {
                    let scene = b.model_layout == ModelLayout::Scene;
                    let model = server
                        .load_builder()
                        .with_settings(move |settings: &mut GltfLoaderSettings| {
                            settings.include_source = scene;
                            settings.load_meshes = RenderAssetUsages::MAIN_WORLD;
                        })
                        .load(format!("terra/blocks/{}", b.model));
                    (b.id, model)
                })
                .collect();
            for block in &library.blocks {
                for path in block.textures.values().chain(
                    block
                        .materials
                        .values()
                        .filter_map(|material| material.normal_texture.as_ref()),
                ) {
                    let linear = block
                        .materials
                        .values()
                        .any(|material| material.normal_texture.as_ref() == Some(path));
                    assets.textures.entry(path.clone()).or_insert_with(|| {
                        server
                            .load_builder()
                            .with_settings(move |settings: &mut ImageLoaderSettings| {
                                settings.is_srgb = !linear;
                                // Publish only the completed mip chain to the renderer.
                                settings.asset_usage = RenderAssetUsages::MAIN_WORLD;
                                settings.sampler =
                                    ImageSampler::Descriptor(ImageSamplerDescriptor {
                                        address_mode_u: ImageAddressMode::Repeat,
                                        address_mode_v: ImageAddressMode::Repeat,
                                        ..ImageSamplerDescriptor::linear()
                                    });
                            })
                            .load(format!("terra/blocks/{path}"))
                    });
                }
            }
            for block in &library.blocks {
                // Custom scene materials may opt into the same declared PBR
                // palette/patterns as ordinary blocks, rather than multiplying
                // a photographed texture by a second unrelated tint.
                if !block.materials.is_empty() {
                    for (name, definition) in &block.materials {
                        if definition.coarse_face.is_some() {
                            continue;
                        }
                        if let Some(pattern) = definition.texture_pattern {
                            assets.patterns.entry(pattern).or_insert_with(|| {
                                let mut image = super::textures::generate(pattern);
                                image.asset_usage = RenderAssetUsages::RENDER_WORLD;
                                images.add(image)
                            });
                        }
                        let slot = assets.shared_material(
                            definition,
                            block.textures.get(name).map(String::as_str),
                            &mut materials,
                        );
                        assets
                            .cube_material_slots
                            .insert((block.id, name.clone()), slot);
                    }
                }
            }
            assets.prop_materials = load_prop_materials(&mut assets, &mut materials);
            assets.item_materials = ItemMaterials {
                opaque: assets.shared_material(
                    &CubeMaterialDefinition {
                        base_color: [1.0; 4],
                        roughness: 0.72,
                        ..default()
                    },
                    None,
                    &mut materials,
                ),
                glass: assets.shared_material(
                    &CubeMaterialDefinition {
                        base_color: [1.0, 0.93, 0.70, 0.18],
                        roughness: 0.18,
                        ..default()
                    },
                    None,
                    &mut materials,
                ),
                metal: assets.shared_material(
                    &CubeMaterialDefinition {
                        base_color: [1.0; 4],
                        roughness: 0.38,
                        metallic: 0.45,
                        ..default()
                    },
                    None,
                    &mut materials,
                ),
                torch: {
                    // A carried flame must illuminate the world, not bleach its
                    // own handle at a sub-metre distance. These vertex-painted
                    // item colours are isolated from the placed PBR materials.
                    let slot = assets.materials.len();
                    assets.materials.push(materials.add(torch_item_material()));
                    slot
                },
            };
            assets.library = Some(library);
        }
        Err(error) => {
            tracing::error!(%error, "Cannot load block JSON definitions");
            assets.error = Some(error);
        }
    }
}

fn torch_item_material() -> StandardMaterial {
    StandardMaterial {
        unlit: true,
        emissive: LinearRgba::BLACK,
        ..default()
    }
}

#[derive(Hash, PartialEq, Eq)]
struct MaterialKey {
    emission: [u32; 3],
    texture: Option<String>,
    normal_texture: Option<String>,
    pattern: Option<super::definitions::TexturePattern>,
    color: [u32; 4],
    roughness: u32,
    reflectance: u32,
    metallic: u32,
    double_sided: bool,
}

impl BlockAssets {
    fn shared_material(
        &mut self,
        definition: &CubeMaterialDefinition,
        texture: Option<&str>,
        materials: &mut Assets<StandardMaterial>,
    ) -> usize {
        let key = MaterialKey {
            emission: definition.emission.map(f32::to_bits),
            texture: texture.map(String::from),
            normal_texture: definition.normal_texture.clone(),
            pattern: definition.texture_pattern,
            color: definition.base_color.map(f32::to_bits),
            roughness: definition.roughness.to_bits(),
            reflectance: definition.reflectance.to_bits(),
            metallic: definition.metallic.to_bits(),
            double_sided: definition.double_sided,
        };
        if let Some(&slot) = self.shared_materials.get(&key) {
            return slot;
        }
        let [r, g, b, a] = definition.base_color;
        let material = StandardMaterial {
            emissive: LinearRgba::new(
                definition.emission[0],
                definition.emission[1],
                definition.emission[2],
                1.0,
            ),
            normal_map_texture: definition
                .normal_texture
                .as_ref()
                .and_then(|path| self.textures.get(path))
                .cloned(),
            base_color: Color::linear_rgba(r, g, b, a),
            base_color_texture: texture
                .and_then(|path| self.textures.get(path).cloned())
                .or_else(|| {
                    definition
                        .texture_pattern
                        .and_then(|pattern| self.patterns.get(&pattern).cloned())
                }),
            alpha_mode: if texture.is_some_and(authored_foliage_texture) {
                AlphaMode::Mask(0.5)
            } else if a < 1.0 {
                AlphaMode::Blend
            } else {
                AlphaMode::Opaque
            },
            perceptual_roughness: definition.roughness,
            reflectance: definition.reflectance,
            metallic: definition.metallic,
            double_sided: definition.double_sided || texture.is_some_and(authored_foliage_texture),
            cull_mode: (!(definition.double_sided
                || texture.is_some_and(authored_foliage_texture)))
            .then_some(bevy::render::render_resource::Face::Back),
            ..default()
        };
        let slot = self.materials.len();
        self.materials.push(materials.add(material));
        self.shared_materials.insert(key, slot);
        slot
    }
}

fn load_prop_materials(
    assets: &mut BlockAssets,
    materials: &mut Assets<StandardMaterial>,
) -> PropMaterials {
    let mut material = |color| {
        assets.shared_material(
            &CubeMaterialDefinition {
                base_color: color,
                ..default()
            },
            None,
            materials,
        )
    };
    PropMaterials {
        crop: material([1.0, 1.0, 1.0, 1.0]),
        foliage: [
            material([0.245, 0.405, 0.072, 1.0]),
            material([0.280, 0.460, 0.080, 1.0]),
            material([0.310, 0.495, 0.095, 1.0]),
        ],
        stem: material([0.225, 0.360, 0.063, 1.0]),
        dry_grass: [
            material([0.36, 0.31, 0.10, 1.0]),
            material([0.47, 0.39, 0.16, 1.0]),
            material([0.32, 0.34, 0.12, 1.0]),
        ],
        twig: material([0.26, 0.17, 0.075, 1.0]),
        flowers: [
            material([0.72, 0.045, 0.025, 1.0]),
            material([0.91, 0.67, 0.08, 1.0]),
            material([0.85, 0.88, 0.76, 1.0]),
            material([0.065, 0.24, 0.70, 1.0]),
            material([0.69, 0.33, 0.51, 1.0]),
            material([0.38, 0.20, 0.57, 1.0]),
            material([0.94, 0.29, 0.045, 1.0]),
            material([0.27, 0.69, 0.20, 1.0]),
        ],
        mushroom: material([0.60, 0.16, 0.09, 1.0]),
        stone: material([0.46, 0.49, 0.43, 1.0]),
    }
}

#[derive(SystemParam)]
pub(crate) struct SourceAssets<'w> {
    server: Res<'w, AssetServer>,
    gltfs: Res<'w, Assets<Gltf>>,
    gltf_meshes: Res<'w, Assets<GltfMesh>>,
    gltf_nodes: Res<'w, Assets<GltfNode>>,
    meshes: Res<'w, Assets<Mesh>>,
    materials: ResMut<'w, Assets<StandardMaterial>>,
    images: ResMut<'w, Assets<Image>>,
}

pub(crate) fn poll_blocks(mut source: SourceAssets, mut assets: ResMut<BlockAssets>) {
    if assets.templates.is_some() || assets.error.is_some() {
        return;
    }
    match register_authored_textures(&source, &mut assets).and_then(|ready| {
        if ready {
            prepare_mipmaps(&mut source, &mut assets)
        } else {
            Ok(false)
        }
    }) {
        Ok(true) => {}
        Ok(false) => return,
        Err(error) => {
            tracing::error!(%error, "Cannot prepare terrain texture mipmaps");
            assets.error = Some(error);
            return;
        }
    }
    match collect_templates(&mut source, &mut assets) {
        Ok(Some(templates)) => {
            if let Err(error) = apply_textures(&mut source, &assets) {
                tracing::error!(%error, "Invalid block texture binding");
                assets.error = Some(error);
                return;
            }
            tracing::info!(
                blocks = templates.blocks.len(),
                materials = assets.materials.len(),
                "Block template library ready"
            );
            assets.templates = Some(Arc::new(templates));
        }
        Ok(None) => {}
        Err(error) => {
            tracing::error!(%error, "Invalid block template library");
            assets.error = Some(error);
        }
    }
}

fn register_authored_textures(
    source: &SourceAssets,
    assets: &mut BlockAssets,
) -> Result<bool, String> {
    let Some(library) = &assets.library else {
        return Ok(false);
    };
    for block in library
        .blocks
        .iter()
        .filter(|block| block.coarse_faces.is_some())
    {
        let handle = assets
            .models
            .get(&block.id)
            .ok_or("Authored terrain model unavailable")?;
        if let Some(LoadState::Failed(error)) = source.server.get_load_state(handle.id()) {
            return Err(error.to_string());
        }
        if !source.server.is_loaded_with_dependencies(handle.id()) {
            return Ok(false);
        }
        let gltf = source
            .gltfs
            .get(handle)
            .ok_or("Authored terrain GLTF unavailable")?;
        for index in 0..gltf.materials.len() {
            let Some(handle) = source.server.get_handle::<StandardMaterial>(format!(
                "terra/blocks/{}#Material{index}/std",
                block.model
            )) else {
                continue;
            };
            let Some(material) = source.materials.get(&handle) else {
                return Ok(false);
            };
            // Bindings can already exist before section meshes. Register these
            // handles before replacing image views so every PBR binding is invalidated.
            if !assets.material_slots.contains_key(&(block.id, index)) {
                assets
                    .material_slots
                    .insert((block.id, index), assets.materials.len());
                assets.materials.push(handle);
            }
            for texture in [
                &material.base_color_texture,
                &material.normal_map_texture,
                &material.metallic_roughness_texture,
                &material.occlusion_texture,
            ]
            .into_iter()
            .flatten()
            {
                if !assets.textures.values().any(|known| known == texture) {
                    assets.textures.insert(
                        format!("model:{}:{:?}", block.id, texture.id()),
                        texture.clone(),
                    );
                }
            }
        }
    }
    Ok(true)
}

/// These PNGs carry botanical silhouettes, including partially covered edges.
/// Inventory, color, depth and shadow rendering must all retain that coverage.
fn authored_foliage_texture(path: &str) -> bool {
    path.starts_with("textures/foliage/") && path.ends_with(".png")
}

fn prepare_mipmaps(source: &mut SourceAssets, assets: &mut BlockAssets) -> Result<bool, String> {
    for (path, handle) in assets.textures.clone() {
        if assets.mipmaps_ready.contains(&path) {
            continue;
        }
        if let Some(job) = assets.mipmap_jobs.get_mut(&path) {
            if let Some(result) = future::block_on(future::poll_once(job)) {
                let image = result?;
                let mip_levels = image.texture_descriptor.mip_level_count;
                let rebound_materials = publish_mipmapped_image(
                    &mut source.images,
                    &mut source.materials,
                    &assets.materials,
                    &handle,
                    image,
                )?;
                tracing::info!(texture = %path, mip_levels, rebound_materials, "Terrain texture mipmaps ready");
                assets.mipmap_jobs.remove(&path);
                assets.mipmaps_ready.insert(path);
            }
            continue;
        }
        if let Some(LoadState::Failed(error)) = source.server.get_load_state(handle.id()) {
            return Err(error.to_string());
        }
        if source.server.is_loaded_with_dependencies(handle.id())
            && let Some(image) = source.images.get(&handle)
        {
            let image = image.clone();
            let canopy = path == "textures/canopy-stylized.png" || authored_foliage_texture(&path);
            let authored = path.starts_with("model:");
            let normal_map = path.ends_with("_normal.png");
            assets.mipmap_jobs.insert(
                path,
                AsyncComputeTaskPool::get().spawn(async move {
                    if normal_map {
                        super::mipmaps::generate_normal(image)
                    } else if canopy {
                        crate::foliage::masked_canopy(image)
                    } else if authored {
                        super::mipmaps::generate_authored(image)
                    } else {
                        super::mipmaps::generate(image)
                    }
                }),
            );
        }
    }
    Ok(assets.mipmaps_ready.len() == assets.textures.len())
}

fn publish_mipmapped_image(
    images: &mut Assets<Image>,
    materials: &mut Assets<StandardMaterial>,
    handles: &[Handle<StandardMaterial>],
    texture: &Handle<Image>,
    mut image: Image,
) -> Result<usize, String> {
    image.asset_usage = RenderAssetUsages::RENDER_WORLD;
    images
        .insert(texture.id(), image)
        .map_err(|e| e.to_string())?;
    let mut changed = 0;
    for handle in handles {
        if let Some(material) = materials.get_mut(handle)
            && [
                &material.base_color_texture,
                &material.normal_map_texture,
                &material.metallic_roughness_texture,
                &material.occlusion_texture,
            ]
            .into_iter()
            .any(|binding| binding.as_ref() == Some(texture))
        {
            // Bevy does not invalidate material bindings when an Image gets a new
            // GPU view. Emit a material modification to replace cached bind groups.
            let _ = material.into_inner();
            changed += 1;
        }
    }
    Ok(changed)
}

fn collect_templates(
    source: &mut SourceAssets,
    assets: &mut BlockAssets,
) -> Result<Option<TemplateSet>, String> {
    let Some(library) = assets.library.clone() else {
        return Ok(None);
    };
    let mut templates = TemplateSet {
        prop_materials: assets.prop_materials,
        item_materials: assets.item_materials,
        ..default()
    };
    // Authored models supply shared tiled PBR slots to ordinary cube materials.
    for definition in library
        .blocks
        .iter()
        .filter(|block| block.render_kind == BlockRenderKind::CustomMesh)
        .chain(
            library
                .blocks
                .iter()
                .filter(|block| block.render_kind != BlockRenderKind::CustomMesh),
        )
    {
        if matches!(
            definition.render_kind,
            BlockRenderKind::Cube | BlockRenderKind::Segment | BlockRenderKind::Boxes
        ) {
            let mut authored_materials = HashSet::new();
            for (name, material) in &definition.materials {
                if material.normal_texture.is_some()
                    && let Some(&slot) = assets
                        .cube_material_slots
                        .get(&(definition.id, name.clone()))
                {
                    authored_materials.insert(slot);
                    templates.normal_mapped_materials.insert(slot);
                }
                if let Some(reference) = material.coarse_face {
                    let source = templates.cube_lods.get(&reference.block).ok_or_else(|| {
                        format!(
                            "Block {}: authored material source unavailable",
                            definition.id
                        )
                    })?;
                    let slot = source.face_materials[reference.side];
                    assets
                        .cube_material_slots
                        .insert((definition.id, name.clone()), slot);
                    authored_materials.insert(slot);
                }
            }
            let names = definition
                .face_materials
                .as_ref()
                .ok_or("Cube lacks face materials")?;
            let mut face_materials = [0; 6];
            for (index, name) in names.iter().enumerate() {
                face_materials[index] = *assets
                    .cube_material_slots
                    .get(&(definition.id, name.clone()))
                    .ok_or_else(|| {
                        format!("Block {}: missing cube material {name}", definition.id)
                    })?;
            }
            let side_cap = definition
                .side_cap
                .as_ref()
                .map(|cap| {
                    assets
                        .cube_material_slots
                        .get(&(definition.id, cap.material.clone()))
                        .copied()
                        .map(|material| SideCap {
                            material,
                            height: cap.height,
                        })
                        .ok_or_else(|| {
                            format!("Block {}: missing side cap material", definition.id)
                        })
                })
                .transpose()?;
            if let Some(name) = &definition.frame_material {
                let slot = assets
                    .cube_material_slots
                    .get(&(definition.id, name.clone()))
                    .copied()
                    .ok_or("Missing window frame material")?;
                templates.frame_materials.insert(definition.id, slot);
            }
            if let Some(name) = &definition.crystal_material {
                let slot = assets
                    .cube_material_slots
                    .get(&(definition.id, name.clone()))
                    .copied()
                    .ok_or("Missing crystal material")?;
                templates.ore_materials.insert(definition.id, slot);
            }
            if let Some(cluster) = &definition.foliage_cluster {
                let slot = |name: &String| {
                    assets
                        .cube_material_slots
                        .get(&(definition.id, name.clone()))
                        .copied()
                        .ok_or_else(|| {
                            format!(
                                "Block {}: missing leaf cluster material {name}",
                                definition.id
                            )
                        })
                };
                templates.foliage_clusters.insert(
                    definition.id,
                    super::FoliageClusterMaterials {
                        leaves: slot(&cluster.material)?,
                        needles: cluster.needles.as_ref().map(slot).transpose()?,
                    },
                );
            }
            let mut block = BlockTemplate {
                surface: definition.surface.map(super::surface::SurfaceTemplate::new),
                render_kind: definition.render_kind,
                bevel_width: definition.bevel_width,
                faces: if definition.render_kind == BlockRenderKind::Boxes {
                    std::array::from_fn(|_| Vec::new())
                } else if definition.render_kind == BlockRenderKind::Segment {
                    segment_preview(definition.id, face_materials[0])
                } else {
                    cube_faces(definition.bevel_width, face_materials, side_cap)
                },
                face_materials,
                side_cap,
                details: Vec::new(),
                model: if terra_core::street::is_arc(definition.id) {
                    super::sidewalk::arc(definition.id, 0, face_materials)
                } else if definition.render_kind == BlockRenderKind::Boxes {
                    super::cube::box_model(
                        &definition.collision_boxes,
                        definition.bevel_width,
                        face_materials,
                    )
                } else {
                    Vec::new()
                },
            };
            for primitive in block.faces.iter_mut().flatten().chain(&mut block.model) {
                if primitive.tangents.is_empty() && authored_materials.contains(&primitive.material)
                {
                    primitive.tangents = primitive
                        .normals
                        .iter()
                        .map(|&normal| {
                            let normal = Vec3::from_array(normal);
                            let axis = (0..3)
                                .max_by(|&a, &b| normal[a].abs().total_cmp(&normal[b].abs()))
                                .unwrap();
                            super::projected_tangent(normal, axis * 2)
                        })
                        .collect();
                }
            }
            templates.blocks.insert(definition.id, block);
            continue;
        }
        let handle = assets
            .models
            .get(&definition.id)
            .ok_or_else(|| format!("Block {}: custom model unavailable", definition.id))?;
        if let Some(LoadState::Failed(e)) = source.server.get_load_state(handle.id()) {
            return Err(e.to_string());
        }
        if let Some(RecursiveDependencyLoadState::Failed(e)) = source
            .server
            .get_recursive_dependency_load_state(handle.id())
        {
            return Err(e.to_string());
        }
        if !source.server.is_loaded_with_dependencies(handle.id()) {
            return Ok(None);
        }
        let Some(gltf) = source.gltfs.get(handle) else {
            return Ok(None);
        };
        for index in 0..gltf.materials.len() {
            // Unused palette entries have no PBR subasset in Bevy 0.19.
            // Only primitives actually imported into a section need a slot.
            let used = gltf
                .meshes
                .iter()
                .filter_map(|handle| source.gltf_meshes.get(handle))
                .flat_map(|mesh| &mesh.primitives)
                .any(|primitive| primitive.material.as_ref() == Some(&gltf.materials[index]));
            if !used {
                continue;
            }
            if !assets.material_slots.contains_key(&(definition.id, index)) {
                let declared = gltf.named_materials.iter().find_map(|(name, handle)| {
                    if *handle == gltf.materials[index] {
                        assets
                            .cube_material_slots
                            .get(&(definition.id, name.to_string()))
                            .copied()
                    } else {
                        None
                    }
                });
                if let Some(slot) = declared {
                    assets.material_slots.insert((definition.id, index), slot);
                    continue;
                }
                // The PBR loader already created these subassets while loading
                // the root. Starting labelled loads can re-enter the root loader
                // and replace its retained source under concurrent asset work.
                let material = source
                    .server
                    .get_handle::<StandardMaterial>(format!(
                        "terra/blocks/{}#Material{index}/std",
                        definition.model
                    ))
                    .ok_or_else(|| {
                        format!(
                            "Block {}: loaded PBR material {index} unavailable",
                            definition.id
                        )
                    })?;
                assets
                    .material_slots
                    .insert((definition.id, index), assets.materials.len());
                assets.materials.push(material);
            }
        }
        if gltf
            .meshes
            .iter()
            .filter_map(|handle| source.gltf_meshes.get(handle))
            .any(|mesh| {
                mesh.primitives
                    .iter()
                    .any(|primitive| primitive.material.is_none())
            })
            && !assets
                .material_slots
                .contains_key(&(definition.id, usize::MAX))
        {
            let slot = assets.shared_material(
                &CubeMaterialDefinition::default(),
                None,
                &mut source.materials,
            );
            assets
                .material_slots
                .insert((definition.id, usize::MAX), slot);
        }
        if definition.model_layout == ModelLayout::Scene {
            for (name, handle) in &gltf.named_materials {
                if let Some(index) = gltf
                    .materials
                    .iter()
                    .position(|material| material == handle)
                    && let Some(slot) = assets.material_slots.get(&(definition.id, index))
                {
                    if terra_core::castle::is_castle(definition.id) {
                        templates.register_castle_material(name, *slot);
                    }
                    if terra_core::train_parts::is_part(definition.id)
                        && let Some(role) = super::TrainAnimationRole::from_material(name)
                    {
                        templates.train_moving_materials.insert(*slot, role);
                    }
                    if definition.id == terra_core::lantern::LANTERN
                        && let Some(variant) = name
                            .strip_prefix("lantern_")
                            .and_then(|suffix| suffix.split('_').next())
                            .and_then(|number| number.parse::<u8>().ok())
                    {
                        templates.lantern_variants.insert(*slot, variant);
                    }
                    if matches!(definition.id, 148 | 170) && name.as_ref() == "kinetic_belt" {
                        templates.conveyor_ribbon_materials.insert(*slot);
                    }
                    if terra_core::logistics::is_port(definition.id)
                        && name.as_ref() == "port_arrow"
                    {
                        templates.logistics_arrow_materials.insert(*slot);
                    }
                    if matches!(definition.id, 153 | 157) && name.starts_with("port_support_") {
                        templates.logistics_support_materials.insert(*slot);
                    }
                    if definition.id == terra_core::campfire::MATERIAL
                        && name.as_ref() == "stone_ring"
                    {
                        templates.campfire_support_material = Some(*slot);
                    }
                }
                if crate::crushers::articulated_material(definition.id)
                    && let Some(rotor) = crate::crushers::rotor_material(name)
                    && let Some(index) = gltf
                        .materials
                        .iter()
                        .position(|material| material == handle)
                    && let Some(slot) = assets.material_slots.get(&(definition.id, index))
                {
                    templates.crusher_moving_materials.insert(*slot, rotor);
                }
                if (71..=73).contains(&definition.id)
                    && name.starts_with("blanket_")
                    && let Some(index) = gltf
                        .materials
                        .iter()
                        .position(|material| material == handle)
                    && let Some(slot) = assets.material_slots.get(&(definition.id, index))
                {
                    templates.blanket_materials.insert(*slot);
                }
                if terra_world::objects::is_opening(definition.id)
                    && name.starts_with("leaf_")
                    && let Some(index) = gltf
                        .materials
                        .iter()
                        .position(|material| material == handle)
                    && let Some(slot) = assets.material_slots.get(&(definition.id, index))
                {
                    templates.hinge_materials.insert(*slot);
                }
                if (matches!(name.as_ref(), "oak" | "dark_oak" | "oak_frame")
                    || crate::crushers::wood_material(name))
                    && let Some(index) = gltf
                        .materials
                        .iter()
                        .position(|material| material == handle)
                    && let Some(slot) = assets.material_slots.get(&(definition.id, index))
                {
                    templates.wood_materials.insert(*slot);
                }
            }
            let model = read_scene(definition.id, gltf, source, assets)?;
            templates.animated_chests |= definition.id == 29;
            if definition.id == 40 {
                for primitive in &model {
                    let material = assets
                        .materials
                        .get(primitive.material)
                        .and_then(|handle| source.materials.get(handle))
                        .ok_or("Torch palette material unavailable")?;
                    templates.torch_palette.insert(
                        primitive.material,
                        material.base_color.to_linear().to_f32_array(),
                    );
                }
            }
            let material = dominant_material(&model);
            templates.blocks.insert(
                definition.id,
                BlockTemplate {
                    surface: None,
                    render_kind: BlockRenderKind::CustomMesh,
                    bevel_width: 0.0,
                    faces: std::array::from_fn(|_| Vec::new()),
                    face_materials: [material; 6],
                    side_cap: None,
                    details: Vec::new(),
                    model,
                },
            );
            continue;
        }
        let mut faces = std::array::from_fn(|_| Vec::new());
        for (side, name) in definition.faces.iter().enumerate() {
            faces[side] = read_mesh(name, definition.id, gltf, source, assets, true)?;
        }
        if let Some(names) = &definition.coarse_faces {
            let mut coarse = std::array::from_fn(|_| Vec::new());
            for (side, name) in names.iter().enumerate() {
                coarse[side] = read_mesh(name, definition.id, gltf, source, assets, true)?;
                validate_tiled_face(&coarse[side], side)?;
            }
            if faces.iter().chain(&coarse).flatten().all(|primitive| {
                assets
                    .materials
                    .get(primitive.material)
                    .and_then(|handle| source.materials.get(handle))
                    .is_some_and(|material| material.alpha_mode == AlphaMode::Opaque)
            }) {
                templates.opaque_cube_lods.insert(definition.id);
            }
            templates.cube_lods.insert(
                definition.id,
                BlockTemplate {
                    surface: None,
                    render_kind: BlockRenderKind::Cube,
                    bevel_width: 0.0,
                    face_materials: coarse.each_ref().map(|p| dominant_material(p)),
                    faces: coarse,
                    side_cap: None,
                    details: Vec::new(),
                    model: Vec::new(),
                },
            );
        }
        let mut details = Vec::new();
        for detail in &definition.details {
            details.push(DetailTemplate {
                primitives: read_mesh(&detail.mesh, definition.id, gltf, source, assets, false)?,
                probability: detail.probability,
                water_clearance: detail.water_clearance,
                density_field: detail.density_field,
            });
        }
        templates.blocks.insert(
            definition.id,
            BlockTemplate {
                surface: None,
                render_kind: BlockRenderKind::CustomMesh,
                face_materials: faces
                    .each_ref()
                    .map(|primitives| dominant_material(primitives)),
                faces,
                details,
                bevel_width: definition.bevel_width,
                side_cap: None,
                model: Vec::new(),
            },
        );
    }
    for handle in &assets.materials {
        if let Some(LoadState::Failed(e)) = source.server.get_load_state(handle.id()) {
            return Err(e.to_string());
        }
        if !source.materials.contains(handle.id()) {
            return Ok(None);
        }
    }
    for (path, handle) in &assets.textures {
        if let Some(LoadState::Failed(e)) = source.server.get_load_state(handle.id()) {
            return Err(e.to_string());
        }
        if !source.server.is_loaded_with_dependencies(handle.id())
            || (!assets.mipmaps_ready.contains(path) && !source.images.contains(handle.id()))
        {
            return Ok(None);
        }
    }
    Ok(Some(templates))
}

fn apply_textures(source: &mut SourceAssets, assets: &BlockAssets) -> Result<(), String> {
    let library = assets.library.as_ref().ok_or("No block library")?;
    for definition in library
        .blocks
        .iter()
        .filter(|b| b.render_kind == BlockRenderKind::CustomMesh)
    {
        let handle = assets
            .models
            .get(&definition.id)
            .ok_or("Custom model unavailable")?;
        let gltf = source.gltfs.get(handle).ok_or("GLB unavailable")?;
        for (name, path) in &definition.textures {
            let material = gltf.named_materials.get(name.as_str()).ok_or_else(|| {
                format!(
                    "Block {}: texture binding references missing material {name}",
                    definition.id
                )
            })?;
            let index = gltf
                .materials
                .iter()
                .position(|h| h == material)
                .ok_or("Unindexed GLB material")?;
            let Some(&slot) = assets.material_slots.get(&(definition.id, index)) else {
                // An unused palette entry has no rendered material to rebind.
                continue;
            };
            let material = source
                .materials
                .get_mut(&assets.materials[slot])
                .ok_or("StandardMaterial unavailable")?;
            // JSON paths are authoritative: a PNG can be replaced without re-exporting its GLB.
            material.into_inner().base_color_texture = Some(assets.textures[path].clone());
        }
    }
    Ok(())
}

fn validate_tiled_face(primitives: &[PrimitiveTemplate], side: usize) -> Result<(), String> {
    let (u, v) = match side / 2 {
        0 => (2, 1),
        1 => (0, 2),
        _ => (0, 1),
    };
    let plane = (side % 2) as f32;
    let sign = if side.is_multiple_of(2) { -1.0 } else { 1.0 };
    if primitives.len() != 1
        || primitives[0].positions.len() != 4
        || primitives[0].indices.len() != 6
    {
        return Err("Coarse terrain faces must be single tiled cube quads".into());
    }
    let primitive = &primitives[0];
    let mut corners = std::collections::BTreeSet::new();
    for ((p, n), uv) in primitive
        .positions
        .iter()
        .zip(&primitive.normals)
        .zip(&primitive.uvs)
    {
        if (p[side / 2] - plane).abs() > 0.00001
            || (n[side / 2] - sign).abs() > 0.00001
            || (uv[0] - p[u]).abs() > 0.00001
            || (uv[1] - p[v]).abs() > 0.00001
            || [p[u], p[v]]
                .into_iter()
                .any(|c| c.abs() > 0.00001 && (c - 1.0).abs() > 0.00001)
        {
            return Err(
                "Coarse terrain quad must match cube plane, normals and tiled world-axis UVs"
                    .into(),
            );
        }
        corners.insert([(p[u] > 0.5) as u8, (p[v] > 0.5) as u8]);
    }
    if corners.len() != 4 {
        return Err("Coarse terrain quad lacks a cube corner".into());
    }
    Ok(())
}

fn segment_preview(material: u16, slot: usize) -> [Vec<PrimitiveTemplate>; 6] {
    let mut primitive = PrimitiveTemplate {
        positions: Vec::new(),
        normals: Vec::new(),
        tangents: Vec::new(),
        uvs: Vec::new(),
        indices: Vec::new(),
        material: slot,
    };
    terra_world::segments::visit(
        material,
        [0; 3],
        Default::default(),
        false,
        |triangle, _| {
            let base = primitive.positions.len() as u32;
            primitive.positions.extend(triangle);
            primitive
                .normals
                .extend([terra_world::ore_shapes::normal(triangle); 3]);
            primitive.uvs.extend([[0.0; 2]; 3]);
            primitive.indices.extend([base, base + 1, base + 2]);
        },
    );
    let mut result = std::array::from_fn(|_| Vec::new());
    result[0].push(primitive);
    result
}

fn dominant_material(primitives: &[PrimitiveTemplate]) -> usize {
    primitives
        .iter()
        .max_by(|a, b| surface_area(a).total_cmp(&surface_area(b)))
        .map_or(0, |primitive| primitive.material)
}

fn surface_area(primitive: &PrimitiveTemplate) -> f32 {
    primitive
        .indices
        .chunks_exact(3)
        .map(|triangle| {
            let [a, b, c] = [triangle[0], triangle[1], triangle[2]]
                .map(|index| Vec3::from_array(primitive.positions[index as usize]));
            (b - a).cross(c - a).length()
        })
        .sum()
}

fn read_mesh(
    name: &str,
    id: u16,
    gltf: &Gltf,
    source: &SourceAssets,
    assets: &BlockAssets,
    is_face: bool,
) -> Result<Vec<PrimitiveTemplate>, String> {
    let mesh_handle = gltf
        .named_meshes
        .get(name)
        .ok_or_else(|| format!("Block {id} lacks named mesh {name}"))?;
    let mesh = source
        .gltf_meshes
        .get(mesh_handle)
        .ok_or_else(|| format!("Block {id}: mesh {name} not loaded"))?;
    read_geometry(mesh, id, gltf, source, assets, is_face)
}

fn read_scene(
    id: u16,
    gltf: &Gltf,
    source: &SourceAssets,
    assets: &BlockAssets,
) -> Result<Vec<PrimitiveTemplate>, String> {
    let document = gltf
        .source
        .as_ref()
        .ok_or("Static model source unavailable")?;
    if document.animations().next().is_some()
        || document.meshes().any(|mesh| {
            mesh.primitives()
                .any(|primitive| primitive.morph_targets().next().is_some())
        })
    {
        return Err(format!(
            "Block {id}: animated/morph models require an animated object renderer"
        ));
    }
    let scene = document
        .default_scene()
        .or_else(|| document.scenes().next())
        .ok_or_else(|| format!("Block {id}: model has no scene"))?;
    let reader = SceneReader {
        id,
        gltf,
        source,
        assets,
    };
    let mut output = Vec::new();
    let mut ancestors = HashSet::new();
    for node in scene.nodes() {
        let handle = gltf
            .nodes
            .get(node.index())
            .ok_or("Scene references missing node")?;
        reader.visit(
            handle,
            bevy::math::Affine3A::IDENTITY,
            &mut ancestors,
            &mut output,
        )?;
    }
    if output.is_empty() {
        return Err(format!(
            "Block {id}: default scene has no triangle geometry"
        ));
    }
    Ok(output)
}

struct SceneReader<'a, 'w> {
    id: u16,
    gltf: &'a Gltf,
    source: &'a SourceAssets<'w>,
    assets: &'a BlockAssets,
}

impl SceneReader<'_, '_> {
    fn visit(
        &self,
        handle: &Handle<GltfNode>,
        parent: bevy::math::Affine3A,
        ancestors: &mut HashSet<AssetId<GltfNode>>,
        output: &mut Vec<PrimitiveTemplate>,
    ) -> Result<(), String> {
        if ancestors.len() >= 64 || !ancestors.insert(handle.id()) {
            return Err(format!(
                "Block {}: cyclic or excessively deep scene hierarchy",
                self.id
            ));
        }
        let node = self
            .source
            .gltf_nodes
            .get(handle)
            .ok_or("Static model node unavailable")?;
        if node.skin.is_some() {
            return Err(format!(
                "Block {}: skinned models require an animated object renderer",
                self.id
            ));
        }
        let transform = parent * node.transform.compute_affine();
        if let Some(handle) = &node.mesh {
            let mesh = self
                .source
                .gltf_meshes
                .get(handle)
                .ok_or("Static model mesh unavailable")?;
            for mut primitive in
                read_geometry(mesh, self.id, self.gltf, self.source, self.assets, false)?
            {
                bake_transform(self.id, &mut primitive, transform)?;
                output.push(primitive);
            }
        }
        for child in &node.children {
            self.visit(child, transform, ancestors, output)?;
        }
        ancestors.remove(&handle.id());
        Ok(())
    }
}

fn bake_transform(
    id: u16,
    primitive: &mut PrimitiveTemplate,
    transform: bevy::math::Affine3A,
) -> Result<(), String> {
    let determinant = transform.matrix3.determinant();
    if !transform.is_finite() || determinant == 0.0 || !determinant.is_finite() {
        return Err("Static model contains a singular or non-finite node transform".into());
    }
    let normals = transform.matrix3.inverse().transpose();
    let rail_bounds = terra_core::rails::Piece::from_facing([0; 3], id, 0).map(|piece| {
        piece.local_geometry().iter().fold(
            ([f64::INFINITY; 3], [f64::NEG_INFINITY; 3]),
            |(low, high), beam| {
                let bounds = beam.bounds();
                (
                    std::array::from_fn(|axis| low[axis].min(bounds.minimum[axis])),
                    std::array::from_fn(|axis| high[axis].max(bounds.maximum[axis])),
                )
            },
        )
    });
    for position in &mut primitive.positions {
        *position = transform
            .transform_point3(Vec3::from_array(*position))
            .to_array();
        if position.iter().enumerate().any(|(axis, value)| {
            let (minimum, maximum) = if let Some((low, high)) = rail_bounds {
                (low[axis] as f32, high[axis] as f32)
            } else if matches!(id, 224..=226) && axis == 1 {
                // The fitting's visible saddle seals against the curved boiler;
                // its physical owner/collision volume stays within the unit cell.
                (-0.14, 1.0)
            } else if id == terra_core::train_parts::RAIL_BUILDER && axis != 1 {
                (-1.0, 2.0)
            } else if id == 232 {
                (-0.36, 1.76)
            } else if id == terra_core::train_parts::BOGIE && axis != 1 {
                (
                    terra_core::train_parts::BOGIE_MIN_HORIZONTAL as f32,
                    terra_core::train_parts::BOGIE_MAX_HORIZONTAL as f32,
                )
            } else if axis != 0
                && let Some(radius) = terra_core::kinetic::radial_extent(id)
            {
                (0.5 - radius, 0.5 + radius)
            } else if terra_core::logistics::is_port(id) && axis == 1 {
                (-0.385, 1.0)
            } else if (matches!(id, 59 | 65..=67 | 139 | 248) && axis == 1)
                || (terra_core::beds::is_bed(id) && axis == 2)
            {
                (0.0, 2.0)
            } else {
                (0.0, 1.0)
            };
            !value.is_finite() || !(minimum - 0.00001..=maximum + 0.00001).contains(value)
        }) {
            return Err(format!(
                "Block {id}: static model exceeds its allowed bounds after node transforms"
            ));
        }
    }
    for normal in &mut primitive.normals {
        let transformed = normals
            .mul_vec3(Vec3::from_array(*normal))
            .normalize_or_zero();
        if !transformed.is_finite() || transformed.length_squared() < 0.5 {
            return Err("Static model contains an invalid normal".into());
        }
        *normal = transformed.to_array();
    }
    for (tangent, normal) in primitive.tangents.iter_mut().zip(&primitive.normals) {
        *tangent = super::tangent_frame(
            transform.transform_vector3(Vec3::from_array([tangent[0], tangent[1], tangent[2]])),
            Vec3::from_array(*normal),
            tangent[3] * determinant.signum(),
        );
    }
    if determinant < 0.0 {
        for triangle in primitive.indices.chunks_exact_mut(3) {
            triangle.swap(1, 2);
        }
    }
    Ok(())
}

fn read_geometry(
    mesh: &GltfMesh,
    id: u16,
    gltf: &Gltf,
    source: &SourceAssets,
    assets: &BlockAssets,
    is_face: bool,
) -> Result<Vec<PrimitiveTemplate>, String> {
    let name = &mesh.name;
    let mut output = Vec::new();
    for primitive in &mesh.primitives {
        let geometry = source
            .meshes
            .get(&primitive.mesh)
            .ok_or_else(|| format!("Block {id}: CPU mesh {name} unavailable"))?;
        let Some(VertexAttributeValues::Float32x3(positions)) =
            geometry.attribute(Mesh::ATTRIBUTE_POSITION)
        else {
            return Err(format!("{name}: missing positions"));
        };
        let Some(VertexAttributeValues::Float32x3(normals)) =
            geometry.attribute(Mesh::ATTRIBUTE_NORMAL)
        else {
            return Err(format!("{name}: missing normals"));
        };
        let uvs = match geometry.attribute(Mesh::ATTRIBUTE_UV_0) {
            Some(VertexAttributeValues::Float32x2(uvs)) => uvs.clone(),
            None if !is_face => vec![[0.0; 2]; positions.len()],
            _ => return Err(format!("{name}: missing UVs")),
        };
        let tangents = match geometry.attribute(Mesh::ATTRIBUTE_TANGENT) {
            Some(VertexAttributeValues::Float32x4(tangents)) => tangents.clone(),
            None => Vec::new(),
            _ => return Err(format!("{name}: invalid tangents")),
        };
        let indices: Vec<u32> = match geometry.indices() {
            Some(indices) => indices.iter().map(|i| i as u32).collect(),
            None if !is_face => (0..u32::try_from(positions.len())
                .map_err(|_| "Too many model vertices")?)
                .collect(),
            None => return Err(format!("{name}: missing indices")),
        };
        if geometry.primitive_topology() != bevy::mesh::PrimitiveTopology::TriangleList
            || positions.len() != normals.len()
            || positions.len() != uvs.len()
            || !tangents.is_empty() && tangents.len() != positions.len()
            || positions.is_empty()
            || indices.is_empty()
            || !indices.len().is_multiple_of(3)
            || indices.iter().any(|&i| i as usize >= positions.len())
            || positions
                .iter()
                .chain(normals)
                .flatten()
                .any(|v| !v.is_finite())
            || uvs.iter().flatten().any(|v| !v.is_finite())
            || tangents.iter().any(|tangent| {
                tangent.iter().any(|v| !v.is_finite())
                    || Vec3::from_array([tangent[0], tangent[1], tangent[2]]).length_squared() < 0.5
                    || (tangent[3].abs() - 1.0).abs() > 0.0001
            })
        {
            return Err(format!("{name}: invalid triangle geometry"));
        }
        if is_face
            && positions
                .iter()
                .flatten()
                .any(|&p| !(-0.00001..=1.00001).contains(&p))
        {
            return Err(format!(
                "{name}: block surface must fit inside its [0,1] cell; use a detail mesh for decorations"
            ));
        }
        let index = if let Some(material) = &primitive.material {
            gltf.materials
                .iter()
                .position(|h| h == material)
                .ok_or_else(|| format!("{name}: unknown glTF material"))?
        } else {
            usize::MAX
        };
        let slot = *assets
            .material_slots
            .get(&(id, index))
            .ok_or_else(|| format!("{name}: missing material slot"))?;
        output.push(PrimitiveTemplate {
            positions: positions.clone(),
            normals: normals.clone(),
            tangents,
            uvs,
            indices,
            material: slot,
        });
    }
    if output.is_empty() {
        return Err(format!("{name}: no primitives"));
    }
    Ok(output)
}
