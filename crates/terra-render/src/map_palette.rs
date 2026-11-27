//! Capture texture albedos before render-only images release their CPU pixels.
use bevy::{
    asset::{AssetEventSystems, AssetId},
    color::Srgba,
    prelude::*,
    render::render_resource::{TextureDimension, TextureFormat},
};
use std::collections::{HashMap, HashSet};
use terra_core::MapPalette;

#[derive(Default)]
struct PaletteSources {
    materials: HashSet<AssetId<StandardMaterial>>,
    // An entry without a mean also watches an image that is still loading.
    textures: HashMap<AssetId<Image>, Option<[f32; 3]>>,
}

pub(crate) fn install(app: &mut App) {
    // Asset events are flushed here; render extraction takes GPU-only pixels afterwards.
    app.init_resource::<MapPalette>()
        .add_systems(PostUpdate, update.after(AssetEventSystems));
}

fn event_id<A: Asset>(event: &AssetEvent<A>) -> AssetId<A> {
    match *event {
        AssetEvent::Added { id }
        | AssetEvent::Modified { id }
        | AssetEvent::LoadedWithDependencies { id }
        | AssetEvent::Removed { id }
        | AssetEvent::Unused { id } => id,
    }
}

fn update(
    blocks: Res<crate::blocks::BlockAssets>,
    materials: Res<Assets<StandardMaterial>>,
    images: Res<Assets<Image>>,
    mut sources: Local<PaletteSources>,
    mut material_events: MessageReader<AssetEvent<StandardMaterial>>,
    mut image_events: MessageReader<AssetEvent<Image>>,
    mut palette: ResMut<MapPalette>,
) {
    let mut dirty = blocks.is_changed();
    if dirty {
        sources.materials.clear();
        sources
            .materials
            .extend(blocks.materials.iter().map(Handle::id));
    }
    for event in material_events.read() {
        dirty |= sources.materials.contains(&event_id(event));
    }
    if dirty {
        for handle in &blocks.materials {
            if let Some(texture) = materials
                .get(handle)
                .and_then(|m| m.base_color_texture.as_ref())
            {
                sources
                    .textures
                    .entry(texture.id())
                    .or_insert_with(|| images.get(texture).and_then(texture_mean));
            }
        }
    }
    for event in image_events.read() {
        let id = event_id(event);
        if matches!(event, AssetEvent::Unused { .. }) {
            sources.textures.remove(&id);
            continue;
        }
        if let Some(mean) = sources.textures.get_mut(&id) {
            // A sampler-only change can arrive after extraction has released the pixels.
            let next = match images.get(id) {
                Some(image) if image.data.is_none() => continue,
                image => image.and_then(texture_mean),
            };
            dirty |= *mean != next;
            *mean = next;
        }
    }
    if !dirty {
        return;
    }
    let Some(templates) = &blocks.templates else {
        return;
    };
    let count = templates
        .blocks
        .keys()
        .copied()
        .max()
        .map_or(0, |id| usize::from(id) + 1);
    let mut colors = vec![[0.25; 3]; count];
    if let Some(air) = colors.first_mut() {
        *air = [0.0; 3];
    }
    for (&id, block) in &templates.blocks {
        let top = templates
            .sculpted_blocks
            .get(&id)
            .map_or(block.face_materials[3], |shape| {
                shape.core.face_materials[3]
            });
        let Some(material) = blocks
            .materials
            .get(top)
            .and_then(|handle| materials.get(handle))
        else {
            continue;
        };
        // Cartography records authored albedo, not continuously changing rain
        // tint. Transient world shading must not dirty every explored tile.
        colors[usize::from(id)] = material_albedo(
            blocks
                .cube_face_definition(id, 3)
                .map(|definition| definition.base_color),
            material,
            &sources,
        );
    }
    // The near model shares a grass/soil atlas. Its whole-image mean is not the
    // upper face; UpLod provides the exact tiled top used by the authored surface.
    let authored_grass = templates.cube_lods.contains_key(&1);
    let surface_grass = if templates
        .sculpted_blocks
        .get(&1)
        .is_some_and(|shape| shape.reference_grass)
    {
        colors.get(1).copied()
    } else {
        templates
            .cube_lods
            .get(&1)
            .and_then(|block| blocks.materials.get(block.face_materials[3]))
            .and_then(|handle| materials.get(handle))
            .map(|material| authored_grass_top_palette(material_albedo(None, material, &sources)))
    };
    let biome_colors = crate::appearance::LINEAR_COLORS.map(|c| [c[0], c[1], c[2]]);
    let water_colors = std::array::from_fn(|index| {
        if index == 15 {
            crate::water::biome::COAST_TINT
        } else if index == 14 {
            crate::water::biome::GLACIAL_TINT
        } else if index == 13 {
            crate::water::biome::PETRIFIED_TINT
        } else if index == 12 {
            crate::water::biome::SWAMP_TINT
        } else if index == 11 {
            crate::water::biome::SAVANNA_TINT
        } else if index == 10 {
            crate::water::biome::ASHLANDS_TINT
        } else if index == 9 {
            crate::water::biome::BLOOD_TINT
        } else if index == 8 {
            crate::water::biome::SPIRIT_TINT
        } else if index == 7 {
            crate::water::biome::CHERRY_TINT
        } else {
            crate::water::biome::COLORS[if index == 6 { 3 } else { index }]
        }
    });
    if palette.colors != colors
        || palette.biome_colors != biome_colors
        || palette.authored_grass != authored_grass
        || palette.surface_grass != surface_grass
        || palette.water_colors != water_colors
    {
        palette.colors = colors;
        palette.biome_colors = biome_colors;
        palette.authored_grass = authored_grass;
        palette.surface_grass = surface_grass;
        palette.water_colors = water_colors;
        palette.revision = palette.revision.wrapping_add(1);
    }
}

fn authored_grass_top_palette(color: [f32; 3]) -> [f32; 3] {
    // Keep in step with authored_grass_top_palette in terra_wet_surface.wesl.
    std::array::from_fn(|axis| color[axis] * 0.68 + [0.17, 0.30, 0.045][axis] * 0.32)
}

fn material_albedo(
    authored: Option<[f32; 4]>,
    material: &StandardMaterial,
    sources: &PaletteSources,
) -> [f32; 3] {
    let base = stable_base_color(authored, material);
    let texture = material
        .base_color_texture
        .as_ref()
        .and_then(|handle| *sources.textures.get(&handle.id())?)
        .unwrap_or([1.0; 3]);
    std::array::from_fn(|axis| (base[axis] * texture[axis]).clamp(0.0, 1.0))
}

fn stable_base_color(authored: Option<[f32; 4]>, material: &StandardMaterial) -> [f32; 3] {
    authored.map_or_else(
        || {
            let color = material.base_color.to_linear();
            [color.red, color.green, color.blue]
        },
        |[r, g, b, _]| [r, g, b],
    )
}

fn texture_mean(image: &Image) -> Option<[f32; 3]> {
    if image.texture_descriptor.dimension != TextureDimension::D2
        || image.texture_descriptor.size.depth_or_array_layers != 1
    {
        return None;
    }
    let (srgb, bgra) = match image.texture_descriptor.format {
        TextureFormat::Rgba8UnormSrgb => (true, false),
        TextureFormat::Rgba8Unorm => (false, false),
        TextureFormat::Bgra8UnormSrgb => (true, true),
        TextureFormat::Bgra8Unorm => (false, true),
        _ => return None,
    };
    let data = image.data.as_ref()?;
    let width = image.texture_descriptor.size.width as usize;
    let height = image.texture_descriptor.size.height as usize;
    if width == 0 || height == 0 || width.checked_mul(height)?.checked_mul(4)? > data.len() {
        return None;
    }
    let mut color = [0.0; 3];
    let mut alpha = 0.0;
    let columns = width.min(16);
    let rows = height.min(16);
    for row in 0..rows {
        for column in 0..columns {
            let x = (column * width + width / 2) / columns;
            let y = (row * height + height / 2) / rows;
            let pixel = &data[(y * width + x) * 4..][..4];
            let weight = f32::from(pixel[3]) / 255.0;
            for (axis, value) in color.iter_mut().enumerate() {
                let channel = if bgra { 2 - axis } else { axis };
                let sample = f32::from(pixel[channel]) / 255.0;
                *value += if srgb {
                    Srgba::gamma_function(sample)
                } else {
                    sample
                } * weight;
            }
            alpha += weight;
        }
    }
    (alpha > 0.0).then(|| color.map(|c| c / alpha))
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::{asset::RenderAssetUsages, render::render_resource::Extent3d};

    #[test]
    fn authored_surface_uses_coarse_top_without_recoloring_the_cave_albedo() {
        use crate::blocks::{BlockTemplate, TemplateSet, definitions::BlockRenderKind};

        let block = |material| BlockTemplate {
            surface: None,
            render_kind: BlockRenderKind::CustomMesh,
            bevel_width: 0.0,
            faces: std::array::from_fn(|_| Vec::new()),
            face_materials: [material; 6],
            side_cap: None,
            details: Vec::new(),
            model: Vec::new(),
        };
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()))
            .init_asset::<Image>()
            .init_asset::<StandardMaterial>();
        let handles = [[102, 51, 25, 255], [51, 153, 25, 255]].map(|pixel| {
            let texture = app
                .world_mut()
                .resource_mut::<Assets<Image>>()
                .add(Image::new(
                    Extent3d {
                        width: 1,
                        height: 1,
                        depth_or_array_layers: 1,
                    },
                    TextureDimension::D2,
                    pixel.to_vec(),
                    TextureFormat::Rgba8Unorm,
                    RenderAssetUsages::default(),
                ));
            app.world_mut()
                .resource_mut::<Assets<StandardMaterial>>()
                .add(StandardMaterial {
                    base_color_texture: Some(texture),
                    ..default()
                })
        });
        let mut blocks = crate::blocks::BlockAssets::default();
        blocks.materials = handles.to_vec();
        blocks.templates = Some(std::sync::Arc::new(TemplateSet {
            blocks: HashMap::from([(1, block(0))]),
            cube_lods: HashMap::from([(1, block(1))]),
            ..default()
        }));
        app.insert_resource(blocks);
        install(&mut app);
        app.update();

        let palette = app.world().resource::<MapPalette>();
        assert_eq!(palette.water_colors[9], crate::water::biome::BLOOD_TINT);
        assert_eq!(palette.biome_colors.len(), terra_core::BiomeKind::ALL.len());
        assert!(palette.biome_colors[9][0] > palette.biome_colors[9][1]);
        assert!(palette.authored_grass);
        assert_eq!(palette.colors[1], [0.4, 0.2, 25.0 / 255.0]);
        let surface = palette.surface_grass.unwrap();
        for (actual, expected) in surface.into_iter().zip([0.1904, 0.504, 0.081_066_67]) {
            assert!((actual - expected).abs() < 1e-6);
        }
        assert!(surface[1] > palette.colors[1][1] * 2.0);
        assert!(palette.revision > 0);

        let revision = palette.revision;
        let _old_texture = app
            .world()
            .resource::<Assets<StandardMaterial>>()
            .get(&handles[0])
            .unwrap()
            .base_color_texture
            .clone();
        let replacement = app.world().resource::<Assets<Image>>().reserve_handle();
        app.world_mut()
            .resource_mut::<Assets<StandardMaterial>>()
            .get_mut_untracked(&handles[0])
            .unwrap()
            .base_color_texture = Some(replacement.clone());
        // The unrelated change must not pick up the deliberately unreported rebind.
        let _other = app
            .world_mut()
            .resource_mut::<Assets<StandardMaterial>>()
            .add(StandardMaterial::default());
        let _other_image = app
            .world_mut()
            .resource_mut::<Assets<Image>>()
            .add(Image::default());
        app.update();
        assert_eq!(app.world().resource::<MapPalette>().revision, revision);

        app.world_mut()
            .resource_mut::<Assets<StandardMaterial>>()
            .get_mut(&handles[0])
            .unwrap()
            .base_color_texture = Some(replacement.clone());
        app.update();
        assert_eq!(app.world().resource::<MapPalette>().colors[1], [1.0; 3]);

        let mut image = Image::new(
            Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
            TextureDimension::D2,
            vec![255, 0, 0, 255],
            TextureFormat::Rgba8Unorm,
            RenderAssetUsages::RENDER_WORLD,
        );
        for (pixel, expected) in [
            ([255, 0, 0, 255], [1., 0., 0.]),
            ([0, 0, 255, 255], [0., 0., 1.]),
        ] {
            image.data = Some(pixel.to_vec());
            app.world_mut()
                .resource_mut::<Assets<Image>>()
                .insert(replacement.id(), image.clone())
                .unwrap();
            app.update();
            assert_eq!(app.world().resource::<MapPalette>().colors[1], expected);
            // Simulate the subsequent GPU extraction, then a sampler-only change.
            app.world_mut()
                .resource_mut::<Assets<Image>>()
                .get_mut(&replacement)
                .unwrap()
                .data = None;
            app.update();
            assert_eq!(app.world().resource::<MapPalette>().colors[1], expected);
        }
    }

    #[test]
    fn rain_and_snow_tints_do_not_invalidate_authored_map_albedo() {
        let authored = [0.48, 0.62, 0.34, 1.0];
        for tint in [[1.0; 3], [0.75, 0.89, 0.96], [0.93, 0.95, 1.0]] {
            let material = StandardMaterial {
                base_color: Color::linear_rgb(
                    authored[0] * tint[0],
                    authored[1] * tint[1],
                    authored[2] * tint[2],
                ),
                ..default()
            };
            assert_eq!(
                stable_base_color(Some(authored), &material),
                [0.48, 0.62, 0.34]
            );
        }
        let custom = StandardMaterial {
            base_color: Color::linear_rgb(0.2, 0.3, 0.4),
            ..default()
        };
        assert_eq!(stable_base_color(None, &custom), [0.2, 0.3, 0.4]);
    }

    #[test]
    fn texture_mean_decodes_linear_albedo_and_ignores_transparent_texels() {
        let image = Image::new(
            Extent3d {
                width: 2,
                height: 1,
                depth_or_array_layers: 1,
            },
            TextureDimension::D2,
            vec![128, 64, 255, 255, 0, 255, 0, 0],
            TextureFormat::Rgba8UnormSrgb,
            RenderAssetUsages::default(),
        );
        let mean = texture_mean(&image).expect("valid RGBA8 image");
        assert!((mean[0] - 0.21586).abs() < 0.00001);
        assert!((mean[1] - 0.05127).abs() < 0.00001);
        assert_eq!(mean[2], 1.0);
        let mut gpu_only = image;
        gpu_only.data = None;
        assert!(texture_mean(&gpu_only).is_none());
    }
}
