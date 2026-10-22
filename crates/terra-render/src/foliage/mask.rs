use bevy::{prelude::*, render::render_resource::TextureFormat};

/// Derive cutouts from the existing canopy image on the texture worker. No
/// second texture or per-leaf asset is needed; the RGB artwork is retained.
pub(crate) fn masked_canopy(mut image: Image) -> Result<Image, String> {
    if !matches!(
        image.texture_descriptor.format,
        TextureFormat::Rgba8UnormSrgb | TextureFormat::Rgba8Unorm
    ) {
        image = image
            .convert(TextureFormat::Rgba8UnormSrgb)
            .ok_or("Cannot convert canopy pixels to RGBA8")?;
    }
    let width = image.texture_descriptor.size.width as usize;
    let height = image.texture_descriptor.size.height as usize;
    let count = width.checked_mul(height).ok_or("Invalid canopy size")?;
    let data = image
        .data
        .as_mut()
        .ok_or("Canopy texture has no CPU pixels")?;
    if count == 0 || count.checked_mul(4) != Some(data.len()) {
        return Err("Canopy masking requires one complete base mip".into());
    }
    for y in 0..height {
        for x in 0..width {
            let uv = Vec2::new(
                (x as f32 + 0.5) / width as f32,
                (y as f32 + 0.5) / height as f32,
            );
            if is_gap(uv) {
                data[(x + y * width) * 4 + 3] = 0;
            }
        }
    }
    let coverage = data.chunks_exact(4).filter(|p| p[3] >= 128).count() as f32 / count as f32;
    let mut image = crate::blocks::mipmaps::generate(image)?;
    preserve_coverage(&mut image, coverage);
    Ok(image)
}

fn is_gap(uv: Vec2) -> bool {
    let tile = uv * 5.0;
    let cell = tile.floor().as_uvec2();
    let hash = (cell.x.wrapping_mul(0x9e37_79b9) ^ cell.y.wrapping_mul(0x85eb_ca6b))
        .wrapping_mul(0xc2b2_ae35);
    if hash % 10 < 6 {
        return false;
    }
    let center = Vec2::new(
        0.42 + ((hash >> 8) & 7) as f32 * 0.023,
        0.42 + ((hash >> 16) & 7) as f32 * 0.023,
    );
    let delta = (tile - tile.floor() - center).abs();
    let radius = 0.24 + ((hash >> 24) & 15) as f32 * 0.008;
    delta.max_element() < radius && delta.x + delta.y < radius * 1.62
}

fn preserve_coverage(image: &mut Image, target: f32) {
    let Some(data) = image.data.as_mut() else {
        return;
    };
    let mut width = image.texture_descriptor.size.width as usize;
    let mut height = image.texture_descriptor.size.height as usize;
    let mut start = width * height * 4;
    for _ in 1..image.texture_descriptor.mip_level_count {
        width = (width / 2).max(1);
        height = (height / 2).max(1);
        let end = start + width * height * 4;
        let pixels = &mut data[start..end];
        let mut low = 0.0;
        let mut high = 4.0;
        let mut best = 1.0;
        let mut error = f32::MAX;
        for _ in 0..12 {
            let scale = (low + high) * 0.5;
            let coverage = pixels
                .chunks_exact(4)
                .filter(|p| f32::from(p[3]) * scale >= 127.5)
                .count() as f32
                / (width * height) as f32;
            if (coverage - target).abs() < error {
                best = scale;
                error = (coverage - target).abs();
            }
            if coverage < target {
                low = scale;
            } else {
                high = scale;
            }
        }
        for pixel in pixels.chunks_exact_mut(4) {
            pixel[3] = (f32::from(pixel[3]) * best).min(255.0).round() as u8;
        }
        start = end;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::{
        asset::RenderAssetUsages,
        render::render_resource::{Extent3d, TextureDimension},
    };

    #[test]
    fn canopy_keeps_rgb_and_reuses_a_coverage_preserving_mip_chain() {
        let image = Image::new_fill(
            Extent3d {
                width: 128,
                height: 128,
                depth_or_array_layers: 1,
            },
            TextureDimension::D2,
            &[71, 128, 49, 255],
            TextureFormat::Rgba8UnormSrgb,
            RenderAssetUsages::all(),
        );
        let result = masked_canopy(image).unwrap();
        assert_eq!(result.texture_descriptor.mip_level_count, 8);
        let data = result.data.unwrap();
        let base = &data[..128 * 128 * 4];
        assert!(base.chunks_exact(4).all(|p| p[..3] == [71, 128, 49]));
        let coverage = |p: &[u8]| {
            p.chunks_exact(4).filter(|p| p[3] >= 128).count() as f32 / (p.len() / 4) as f32
        };
        let target = coverage(base);
        assert!((0.82..0.93).contains(&target), "coverage={target}");
        let mut start = base.len();
        for size in [64, 32, 16] {
            let end = start + size * size * 4;
            assert!((coverage(&data[start..end]) - target).abs() < 0.08);
            start = end;
        }
    }

    #[test]
    fn malformed_canopy_is_rejected_and_cutouts_are_deterministic() {
        let image = Image {
            data: Some(vec![0; 3]),
            ..default()
        };
        assert!(masked_canopy(image).is_err());
        assert_eq!(is_gap(Vec2::new(0.32, 0.44)), is_gap(Vec2::new(0.32, 0.44)));
    }
}
