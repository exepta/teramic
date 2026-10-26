//! Capture texture albedos before render-only images release their CPU pixels.
use bevy::{
    asset::AssetId,
    color::Srgba,
    prelude::*,
    render::render_resource::{TextureDimension, TextureFormat},
};
use std::collections::HashMap;
use terra_core::MapPalette;

#[derive(Resource, Default)]
struct TextureMeans(HashMap<AssetId<Image>, [f32; 3]>);

pub(crate) fn install(app: &mut App) {
    app.init_resource::<MapPalette>()
        .init_resource::<TextureMeans>()
        .add_systems(PostStartup, update)
        .add_systems(
            Update,
            update
                .after(crate::blocks::poll_blocks)
                .after(crate::appearance::update_weather_materials),
        );
}

fn update(
    blocks: Res<crate::blocks::BlockAssets>,
    materials: Res<Assets<StandardMaterial>>,
    images: Res<Assets<Image>>,
    mut means: ResMut<TextureMeans>,
    mut palette: ResMut<MapPalette>,
) {
    if !blocks.is_changed() && !materials.is_changed() && !images.is_changed() {
        return;
    }
    // Generated patterns exist before templates do; PNGs arrive incrementally.
    // Keep only three floats per texture, never a second copy of its mip chain.
    for handle in &blocks.materials {
        let Some(texture) = materials
            .get(handle)
            .and_then(|material| material.base_color_texture.as_ref())
        else {
            continue;
        };
        if !means.0.contains_key(&texture.id())
            && let Some(mean) = images.get(texture).and_then(texture_mean)
        {
            means.0.insert(texture.id(), mean);
        }
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
        let Some(material) = blocks
            .materials
            .get(block.face_materials[3])
            .and_then(|handle| materials.get(handle))
        else {
            continue;
        };
        // Cartography records authored albedo, not continuously changing rain
        // tint. Transient world shading must not dirty every explored tile.
        let base = stable_base_color(
            blocks
                .cube_face_definition(id, 3)
                .map(|definition| definition.base_color),
            material,
        );
        let texture = material
            .base_color_texture
            .as_ref()
            .and_then(|handle| means.0.get(&handle.id()))
            .copied()
            .unwrap_or([1.0; 3]);
        colors[usize::from(id)] =
            std::array::from_fn(|axis| (base[axis] * texture[axis]).clamp(0.0, 1.0));
    }
    let biome_colors = crate::appearance::LINEAR_COLORS.map(|c| [c[0], c[1], c[2]]);
    let water_colors = std::array::from_fn(|index| {
        if index == 7 {
            crate::water::biome::CHERRY_TINT
        } else {
            crate::water::biome::COLORS[if index == 6 { 3 } else { index }]
        }
    });
    if palette.colors != colors
        || palette.biome_colors != biome_colors
        || palette.water_colors != water_colors
    {
        palette.colors = colors;
        palette.biome_colors = biome_colors;
        palette.water_colors = water_colors;
        palette.revision = palette.revision.wrapping_add(1);
    }
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
