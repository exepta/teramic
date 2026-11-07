//! Small periodic density/erosion texture, generated once and retained by the material.
use bevy::{
    asset::RenderAssetUsages,
    image::{ImageAddressMode, ImageSampler, ImageSamplerDescriptor},
    prelude::*,
    render::render_resource::{Extent3d, TextureDimension, TextureFormat},
};

const SIZE: u32 = 64;

pub(super) fn texture() -> Image {
    let mut pixels = Vec::with_capacity((SIZE * SIZE * SIZE * 2) as usize);
    for z in 0..SIZE {
        for y in 0..SIZE {
            for x in 0..SIZE {
                let p = Vec3::new(x as f32, y as f32, z as f32) / SIZE as f32;
                let broad = value(p * 4.0, 4) * 0.60
                    + value(p * 8.0, 8) * 0.28
                    + value(p * 16.0, 16) * 0.12;
                let erosion =
                    value(p * 16.0 + Vec3::splat(0.37), 16) * 0.65 + value(p * 32.0, 32) * 0.35;
                pixels.extend([
                    (broad * 255.0).round() as u8,
                    (erosion * 255.0).round() as u8,
                ]);
            }
        }
    }
    let mut image = Image::new(
        Extent3d {
            width: SIZE,
            height: SIZE,
            depth_or_array_layers: SIZE,
        },
        TextureDimension::D3,
        pixels,
        TextureFormat::Rg8Unorm,
        RenderAssetUsages::RENDER_WORLD,
    );
    image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
        address_mode_u: ImageAddressMode::Repeat,
        address_mode_v: ImageAddressMode::Repeat,
        address_mode_w: ImageAddressMode::Repeat,
        ..ImageSamplerDescriptor::linear()
    });
    image
}

fn value(p: Vec3, period: i32) -> f32 {
    let cell = p.floor().as_ivec3();
    let t = p - p.floor();
    let t = t * t * (Vec3::splat(3.0) - t * 2.0);
    let mut density = 0.0;
    for z in 0..2 {
        for y in 0..2 {
            for x in 0..2 {
                let q = (cell + IVec3::new(x, y, z)).rem_euclid(IVec3::splat(period));
                let mut hash = (q.x as u32).wrapping_mul(0x9e37_79b9)
                    ^ (q.y as u32).wrapping_mul(0x85eb_ca6b)
                    ^ (q.z as u32).wrapping_mul(0xc2b2_ae35);
                hash = (hash ^ (hash >> 16)).wrapping_mul(0x7feb_352d);
                hash = (hash ^ (hash >> 15)).wrapping_mul(0x846c_a68b);
                hash ^= hash >> 16;
                let weight = Vec3::new(
                    if x == 0 { 1.0 - t.x } else { t.x },
                    if y == 0 { 1.0 - t.y } else { t.y },
                    if z == 0 { 1.0 - t.z } else { t.z },
                );
                density += (hash & 0xffff) as f32 / 65_535.0 * weight.element_product();
            }
        }
    }
    density
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn density_is_periodic_smooth_and_bounded() {
        for period in [4, 8, 16, 32] {
            for point in [Vec3::new(-0.01, 2.7, 0.0), Vec3::new(3.4, -1.1, 8.999)] {
                let density = value(point, period);
                assert!((0.0..=1.0).contains(&density));
                for axis in [Vec3::X, Vec3::Y, Vec3::Z] {
                    assert!(
                        (density - value(point + axis * period as f32, period)).abs() < 0.000_01
                    );
                    assert!((density - value(point + axis * 0.001, period)).abs() < 0.003);
                }
            }
        }
    }

    #[test]
    fn texture_has_a_fixed_small_linear_filterable_volume_payload() {
        let image = texture();
        assert_eq!(image.texture_descriptor.dimension, TextureDimension::D3);
        assert_eq!(image.texture_descriptor.format, TextureFormat::Rg8Unorm);
        let pixels = image.data.unwrap();
        assert_eq!(pixels.len(), 64 * 64 * 64 * 2);
        assert!(pixels.iter().copied().min().unwrap() < 32);
        assert!(pixels.iter().copied().max().unwrap() > 220);
        let ImageSampler::Descriptor(sampler) = image.sampler else {
            panic!("Explicit volume sampler");
        };
        assert_eq!(sampler.address_mode_u, ImageAddressMode::Repeat);
        assert_eq!(sampler.address_mode_v, ImageAddressMode::Repeat);
        assert_eq!(sampler.address_mode_w, ImageAddressMode::Repeat);
    }
}
