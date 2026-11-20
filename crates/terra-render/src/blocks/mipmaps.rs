use bevy::{
    color::Srgba,
    image::{Image, ImageAddressMode, ImageSampler, ImageSamplerDescriptor},
    render::render_resource::{TextureDimension, TextureFormat},
};

/// Authored atlases retain their glTF wrap/filter settings, including clamp edges.
pub(crate) fn generate_authored(image: Image) -> Result<Image, String> {
    let sampler = image.sampler.clone();
    let mut image = generate(image)?;
    image.sampler = sampler;
    Ok(image)
}

/// PNG loaders provide one level. Build the shared terrain texture's remaining
/// levels on a worker, filtering linear premultiplied color to avoid dark fringes.
pub(crate) fn generate(mut image: Image) -> Result<Image, String> {
    if image.texture_descriptor.mip_level_count > 1 {
        return Ok(image);
    }
    if image.texture_descriptor.dimension != TextureDimension::D2
        || image.texture_descriptor.size.depth_or_array_layers != 1
    {
        return Err("Terrain mipmaps require a 2D texture".into());
    }
    if !matches!(
        image.texture_descriptor.format,
        TextureFormat::Rgba8UnormSrgb | TextureFormat::Rgba8Unorm
    ) {
        image = image
            .convert(TextureFormat::Rgba8UnormSrgb)
            .ok_or("Cannot convert terrain PNG to RGBA8")?;
    }
    let srgb = image.texture_descriptor.format == TextureFormat::Rgba8UnormSrgb;
    let decode: [f32; 256] = std::array::from_fn(|i| {
        let value = i as f32 / 255.0;
        if srgb {
            Srgba::gamma_function(value)
        } else {
            value
        }
    });
    let encode: [u8; 4097] = std::array::from_fn(|i| {
        let value = i as f32 / 4096.0;
        let value = if srgb {
            Srgba::gamma_function_inverse(value)
        } else {
            value
        };
        (value * 255.0).round() as u8
    });
    let mut width = image.texture_descriptor.size.width as usize;
    let mut height = image.texture_descriptor.size.height as usize;
    let mut data = image
        .data
        .take()
        .ok_or("Terrain texture has no CPU pixels")?;
    if width == 0
        || height == 0
        || width.checked_mul(height).and_then(|n| n.checked_mul(4)) != Some(data.len())
    {
        return Err("Invalid terrain texture dimensions".into());
    }
    let mut previous_start = 0;
    let mut levels = 1;
    while width > 1 || height > 1 {
        let next_width = (width / 2).max(1);
        let next_height = (height / 2).max(1);
        let mut next = Vec::with_capacity(next_width * next_height * 4);
        for y in 0..next_height {
            for x in 0..next_width {
                let mut color = [0.0; 3];
                let mut alpha = 0.0;
                let mut count = 0;
                // Partition odd dimensions too, so no source edge texel is dropped.
                for source_y in y * height / next_height..(y + 1) * height / next_height {
                    for source_x in x * width / next_width..(x + 1) * width / next_width {
                        let index = previous_start + (source_x + source_y * width) * 4;
                        let a = f32::from(data[index + 3]) / 255.0;
                        for channel in 0..3 {
                            color[channel] += decode[usize::from(data[index + channel])] * a;
                        }
                        alpha += a;
                        count += 1;
                    }
                }
                for value in color {
                    let value = if alpha > 0.0 { value / alpha } else { 0.0 };
                    next.push(encode[(value.clamp(0.0, 1.0) * 4096.0).round() as usize]);
                }
                next.push((alpha / count as f32 * 255.0).round() as u8);
            }
        }
        previous_start = data.len();
        data.extend(next);
        width = next_width;
        height = next_height;
        levels += 1;
    }
    image.data = Some(data);
    image.texture_descriptor.mip_level_count = levels;
    image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
        address_mode_u: ImageAddressMode::Repeat,
        address_mode_v: ImageAddressMode::Repeat,
        anisotropy_clamp: 8,
        ..ImageSamplerDescriptor::linear()
    });
    Ok(image)
}

/// Average tangent-space vectors in linear space, then renormalize each level.
/// Color mip filtering would bend grazing-light highlights and erase relief.
pub(crate) fn generate_normal(image: Image) -> Result<Image, String> {
    if image.texture_descriptor.format != TextureFormat::Rgba8Unorm {
        return Err("Normal maps must be loaded in linear RGBA8".into());
    }
    let mut image = generate(image)?;
    if let Some(data) = &mut image.data {
        for pixel in data.chunks_exact_mut(4) {
            let normal = bevy::math::Vec3::new(
                f32::from(pixel[0]) / 127.5 - 1.0,
                f32::from(pixel[1]) / 127.5 - 1.0,
                f32::from(pixel[2]) / 127.5 - 1.0,
            )
            .normalize_or(bevy::math::Vec3::Z);
            for (channel, value) in pixel[..3].iter_mut().zip(normal.to_array()) {
                *channel = ((value * 0.5 + 0.5) * 255.0).round() as u8;
            }
        }
    }
    Ok(image)
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::{asset::RenderAssetUsages, render::render_resource::Extent3d};

    fn image(width: u32, height: u32, data: Vec<u8>) -> Image {
        Image::new(
            Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            TextureDimension::D2,
            data,
            TextureFormat::Rgba8UnormSrgb,
            RenderAssetUsages::default(),
        )
    }

    #[test]
    fn mipmaps_preserve_source_and_average_checkerboard_in_linear_light() {
        let pixels = vec![
            0, 0, 0, 255, 255, 255, 255, 255, 255, 255, 255, 255, 0, 0, 0, 255,
        ];
        let output = generate(image(2, 2, pixels.clone())).expect("mipmaps");
        assert_eq!(output.texture_descriptor.mip_level_count, 2);
        let data = output.data.expect("pixels");
        assert_eq!(data[..16], pixels);
        assert_eq!(&data[16..], &[188, 188, 188, 255]);
    }

    #[test]
    fn odd_rectangular_images_include_all_pixels_and_finish_at_one_texel() {
        let output = generate(image(3, 5, [64, 128, 192, 255].repeat(15))).expect("mipmaps");
        assert_eq!(output.texture_descriptor.mip_level_count, 3);
        let data = output.data.expect("pixels");
        assert_eq!(data.len(), (15 + 2 + 1) * 4);
        assert_eq!(&data[data.len() - 4..], &[64, 128, 192, 255]);
    }

    #[test]
    fn transparent_texels_do_not_create_dark_color_fringes() {
        let output = generate(image(2, 1, vec![255, 255, 255, 255, 0, 0, 0, 0])).expect("mipmaps");
        assert_eq!(&output.data.expect("pixels")[8..], &[255, 255, 255, 128]);
    }

    #[test]
    fn authored_near_clamp_and_coarse_repeat_preserve_samplers_and_identical_mip_pixels() {
        for address_mode in [ImageAddressMode::ClampToEdge, ImageAddressMode::Repeat] {
            for format in [TextureFormat::Rgba8UnormSrgb, TextureFormat::Rgba8Unorm] {
                let mut source = image(
                    2,
                    2,
                    vec![
                        32, 64, 128, 255, 255, 128, 64, 255, 64, 32, 255, 255, 128, 255, 32, 255,
                    ],
                );
                source.texture_descriptor.format = format;
                source.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
                    address_mode_u: address_mode,
                    address_mode_v: address_mode,
                    lod_max_clamp: 7.0,
                    ..ImageSamplerDescriptor::linear()
                });
                let sampler = source.sampler.clone();
                let expected = generate(source.clone()).expect("baseline mipmaps");
                let output = generate_authored(source).expect("authored mipmaps");
                assert_eq!(output.sampler, sampler);
                assert_eq!(output.data, expected.data);
                assert_eq!(output.texture_descriptor.format, format);
                assert_eq!(output.texture_descriptor.mip_level_count, 2);
                assert_eq!(
                    output.texture_descriptor.mip_level_count,
                    expected.texture_descriptor.mip_level_count
                );
                let repeated = generate_authored(output.clone()).expect("completed mipmaps");
                assert_eq!(repeated.data, output.data);
                assert_eq!(repeated.sampler, sampler);
            }
        }
    }
}

#[cfg(test)]
mod normal_tests {
    use super::*;
    #[test]
    fn linear_normal_mips_stay_unit_length_through_the_complete_chain() {
        let image = Image::new(
            bevy::render::render_resource::Extent3d {
                width: 4,
                height: 4,
                depth_or_array_layers: 1,
            },
            TextureDimension::D2,
            [196, 107, 233, 255, 79, 158, 242, 255].repeat(8),
            TextureFormat::Rgba8Unorm,
            bevy::asset::RenderAssetUsages::MAIN_WORLD,
        );
        let result = generate_normal(image).unwrap();
        assert_eq!(result.texture_descriptor.mip_level_count, 3);
        assert_eq!(result.texture_descriptor.format, TextureFormat::Rgba8Unorm);
        for pixel in result.data.unwrap().chunks_exact(4) {
            let n = bevy::math::Vec3::new(pixel[0] as f32, pixel[1] as f32, pixel[2] as f32)
                / 127.5
                - bevy::math::Vec3::ONE;
            assert!((n.length() - 1.0).abs() < 0.012);
            assert!(n.z > 0.8);
        }
    }
}
