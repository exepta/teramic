use bevy::{prelude::*, render::render_resource::TextureFormat};

/// Preserve authored foliage coverage. Synthetic random holes used to cut through
/// painted leaves and expose coarse cavities; opacity now comes only from the asset.
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
    let coverage = data.chunks_exact(4).filter(|p| p[3] >= 128).count() as f32 / count as f32;
    let mut image = crate::blocks::mipmaps::generate(image)?;
    preserve_coverage(&mut image, coverage);
    Ok(image)
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

/// For leaf art painted against a solid background, recover the gaps from its
/// darkest recesses. Existing artist-authored alpha always takes precedence.
pub(crate) fn cutout_from_paint(image: &Image) -> Result<Option<Image>, String> {
    let data = image.data.as_deref().ok_or("Missing canopy paint")?;
    if data.chunks_exact(4).any(|px| px[3] < 255) {
        return Ok(None);
    }
    let luminance =
        |p: &[u8]| (54 * u16::from(p[0]) + 183 * u16::from(p[1]) + 19 * u16::from(p[2])) >> 8;
    let mut histogram = [0usize; 256];
    for p in data.chunks_exact(4) {
        histogram[luminance(p) as usize] += 1;
    }
    let target = data.len() / 4 / 4;
    let mut below = 0;
    let cutoff = histogram
        .iter()
        .position(|n| {
            below += n;
            below >= target
        })
        .unwrap_or(0) as i32;
    let mut cutout = image.clone();
    for pixel in cutout.data.as_mut().unwrap().chunks_exact_mut(4) {
        pixel[3] = (128 + 32 * (i32::from(luminance(pixel)) - cutoff)).clamp(0, 255) as u8;
    }
    masked_canopy(cutout).map(Some)
}

/// A conservative hole for the near mesh. All base texels in the returned
/// square are transparent. Retain a one-texel apron for bilinear filtering.
pub(crate) fn mesh_opening(image: &Image) -> Option<[f32; 3]> {
    const GRID: usize = 64;
    let (width, height) = (image.width() as usize, image.height() as usize);
    if width < GRID || height < GRID || width != height || width % GRID != 0 {
        return None;
    }
    let data = image.data.as_deref()?.get(..width * height * 4)?;
    let mut solid = [0u64; GRID];
    let cell = width / GRID;
    for (y, row) in data.chunks_exact(width * 4).enumerate() {
        for (x, tile) in row.chunks_exact(cell * 4).enumerate() {
            if tile.chunks_exact(4).any(|px| px[3] >= 128) {
                solid[y / cell] |= 1 << x;
            }
        }
    }
    let mut side = [0u8; GRID + 1];
    let mut best = (0, 0, 0u8);
    for (y, &row) in solid.iter().enumerate() {
        let mut diagonal = 0;
        for x in 0..GRID {
            let above = side[x + 1];
            side[x + 1] = if row & (1 << x) == 0 {
                1 + above.min(diagonal).min(side[x])
            } else {
                0
            };
            if side[x + 1] > best.2 {
                best = (x + 1, y + 1, side[x + 1]);
            }
            diagonal = above;
        }
    }
    let (x, y, n) = best;
    let n = usize::from(n);
    let guard = 1.0 / width as f32;
    (n > 0).then(|| {
        [
            (x - n) as f32 / GRID as f32 + guard,
            (y - n) as f32 / GRID as f32 + guard,
            n as f32 / GRID as f32 - 2.0 * guard,
        ]
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::{
        asset::RenderAssetUsages,
        render::render_resource::{Extent3d, TextureDimension},
    };
    fn canopy() -> Image {
        Image::new_fill(
            Extent3d {
                width: 128,
                height: 128,
                depth_or_array_layers: 1,
            },
            TextureDimension::D2,
            &[71, 128, 49, 255],
            TextureFormat::Rgba8UnormSrgb,
            RenderAssetUsages::all(),
        )
    }
    #[test]
    fn opaque_leaf_art_stays_complete_at_every_mip() {
        let result = masked_canopy(canopy()).unwrap();
        assert_eq!(result.texture_descriptor.mip_level_count, 8);
        assert!(
            result
                .data
                .unwrap()
                .chunks_exact(4)
                .all(|p| p == [71, 128, 49, 255])
        );
    }
    #[test]
    fn authored_cutouts_and_rgb_survive_without_new_holes() {
        let mut source = canopy();
        for (i, pixel) in source
            .data
            .as_mut()
            .unwrap()
            .chunks_exact_mut(4)
            .enumerate()
        {
            if (i % 128) / 16 < 2 {
                pixel[3] = 0;
            }
        }
        let original = source.data.clone().unwrap();
        let result = masked_canopy(source).unwrap();
        let data = result.data.unwrap();
        assert_eq!(&data[..original.len()], original.as_slice());
        let mut start = original.len();
        for size in [64, 32, 16] {
            let end = start + size * size * 4;
            let covered = data[start..end]
                .chunks_exact(4)
                .filter(|p| p[3] >= 128)
                .count();
            assert!((covered as f32 / (size * size) as f32 - 0.75).abs() < 0.08);
            start = end;
        }
    }
    #[test]
    fn shipped_leaf_silhouettes_keep_gaps_and_coverage_through_useful_mips() {
        use bevy::image::{CompressedImageFormats, ImageSampler, ImageType};
        let directory = terra_core::asset_directory().join("terra/blocks/textures/foliage");
        for family in [
            "oak",
            "autumn",
            "birch",
            "apple",
            "cherry_pink",
            "cherry_white",
            "spirit_pink",
            "spirit_pearl",
            "spirit_lavender",
            "blood",
        ] {
            for side in ["side", "top"] {
                let path = directory.join(format!("{family}_{side}.png"));
                let source = Image::from_buffer(
                    &std::fs::read(&path).unwrap(),
                    ImageType::Extension("png"),
                    CompressedImageFormats::empty(),
                    true,
                    ImageSampler::default(),
                    RenderAssetUsages::MAIN_WORLD,
                )
                .unwrap();
                let base = source.data.as_ref().unwrap();
                let target = base.chunks_exact(4).filter(|p| p[3] >= 128).count() as f32
                    / (512 * 512) as f32;
                assert!(
                    (0.65..=0.75).contains(&target),
                    "{} {target}",
                    path.display()
                );
                let original = base.clone();
                let output = masked_canopy(source).unwrap();
                let data = output.data.unwrap();
                assert_eq!(&data[..original.len()], original.as_slice());
                let mut start = original.len();
                for size in [256, 128, 64, 32, 16] {
                    let end = start + size * size * 4;
                    let coverage = data[start..end]
                        .chunks_exact(4)
                        .filter(|p| p[3] >= 128)
                        .count() as f32
                        / (size * size) as f32;
                    assert!(
                        (coverage - target).abs() < 0.045,
                        "{} {size}px: {coverage} vs {target}",
                        path.display()
                    );
                    start = end;
                }
            }
        }
    }

    #[test]
    fn malformed_canopy_is_rejected() {
        assert!(
            masked_canopy(Image {
                data: Some(vec![0; 3]),
                ..default()
            })
            .is_err()
        );
    }
}
