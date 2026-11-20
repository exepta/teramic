//! Small, tileable material textures. Generated once, shared across all sections.
use super::definitions::TexturePattern;
mod ballast;
mod reference;
mod underwater;
mod village;
use bevy::{
    asset::RenderAssetUsages,
    image::{ImageAddressMode, ImageSampler, ImageSamplerDescriptor},
    prelude::*,
    render::render_resource::{Extent3d, TextureDimension, TextureFormat},
};

pub(super) fn generate(pattern: TexturePattern) -> Image {
    const SIZE: u32 = 512;
    let mut pixels = Vec::with_capacity((SIZE * SIZE * 4) as usize);
    for py in 0..SIZE {
        for px in 0..SIZE {
            if let Some(color) = ballast::sample(pattern, px, py)
                .or_else(|| reference::sample(pattern, px, py))
                .or_else(|| underwater::sample(pattern, px, py))
                .or_else(|| village::sample(pattern, px, py))
            {
                pixels.extend(color.map(|v| v.clamp(0.0, 255.0).round() as u8));
                pixels.push(255);
                continue;
            }
            let (x, y) = (px / 8, py / 8);
            let fine = hash(px, py) as f32 / u32::MAX as f32;
            let grain = hash(x / 3, y / 3) as f32 / u32::MAX as f32;
            let wave = (y as f32 * std::f32::consts::TAU / 16.0
                + (x as f32 * std::f32::consts::TAU / 64.0).sin() * 0.45)
                .sin();
            let (base, shade) = match pattern {
                TexturePattern::Turf
                | TexturePattern::Wool
                | TexturePattern::PathTop
                | TexturePattern::PathSide
                | TexturePattern::Earth
                | TexturePattern::Farmland
                | TexturePattern::FarmlandSide
                | TexturePattern::Canopy
                | TexturePattern::BirchCanopy
                | TexturePattern::BlossomCanopy
                | TexturePattern::Gravel
                | TexturePattern::Ballast
                | TexturePattern::StructureStone
                | TexturePattern::CoralRed
                | TexturePattern::CoralBlue
                | TexturePattern::CoralYellow
                | TexturePattern::CoralPurple
                | TexturePattern::Ice
                | TexturePattern::Mud => unreachable!("reference texture handled above"),
                TexturePattern::SpruceBark => {
                    let ridge = hash(x / 3, y / 15) % 7;
                    ([94., 73., 51.], 0.78 + ridge as f32 * 0.045 + fine * 0.035)
                }
                TexturePattern::SpruceEnd => {
                    let dx = px as f32 / 512.0 - 0.5;
                    let dy = py as f32 / 512.0 - 0.5;
                    let ring = (dx.hypot(dy) * 91.0 + grain * 0.8).sin();
                    ([160., 130., 88.], 0.88 + ring * 0.065 + fine * 0.025)
                }
                TexturePattern::CherryBark => {
                    let row = y / 7;
                    let line =
                        y % 7 == 0 && (x + hash(row, 417) % 64) % 64 < 10 + hash(row, 21) % 23;
                    (
                        [230., 225., 224.],
                        if line {
                            1.04
                        } else {
                            0.77 + grain * 0.06 + fine * 0.025
                        },
                    )
                }
                TexturePattern::CherryEnd => {
                    let dx = px as f32 / SIZE as f32 - 0.5;
                    let dy = py as f32 / SIZE as f32 - 0.5;
                    let ring = (dx.hypot(dy) * 85. + grain * 0.5).sin();
                    ([248., 241., 237.], 0.90 + ring * 0.07 + fine * 0.015)
                }
                TexturePattern::Dripstone => {
                    let column = hash(x / 5, 39) % 7;
                    (
                        [174., 148., 118.],
                        0.87 + column as f32 * 0.018 + grain * 0.08 + fine * 0.025,
                    )
                }
                TexturePattern::Mineral => {
                    // Staggered crystal planes repeat at the texture boundary.
                    // Broad tonal steps survive mips without sparkling grain.
                    let lane = px / 32;
                    let offset = (hash(lane, 83) % 4) * 16;
                    let row = ((py + offset) % SIZE) / 64;
                    let plane = (hash(lane, row) % 11) as f32 / 10.0;
                    let glint = if px % 32 < 2 { 0.06 } else { 0.0 };
                    (
                        [248., 246., 242.],
                        0.74 + plane * 0.22 + glint + fine * 0.012,
                    )
                }
                TexturePattern::BirchBark => {
                    let row = y / 8;
                    let offset = hash(row, 71) % 64;
                    let horizontal = (x + offset) % 64;
                    let scar = y % 8 < 2 && horizontal < 7 + hash(row, 19) % 18;
                    (
                        if scar {
                            [64., 63., 55.]
                        } else {
                            [230., 228., 210.]
                        },
                        0.95 + grain * 0.035 + fine * 0.025,
                    )
                }
                TexturePattern::Sand => ([220., 204., 156.], 0.94 + fine * 0.11 + wave * 0.015),
                TexturePattern::Sandstone => (
                    [201., 178., 125.],
                    0.90 + grain * 0.10 + wave * 0.07 - if y % 16 == 0 { 0.08 } else { 0.0 },
                ),
                TexturePattern::Stone | TexturePattern::MossyStone => {
                    let shard = hash(x / 9, y / 7);
                    let seam = (x + y * 2 + shard % 13).is_multiple_of(31);
                    let moss = pattern == TexturePattern::MossyStone && hash(x / 8, y / 8) % 7 < 2;
                    (
                        if moss {
                            [101., 127., 72.]
                        } else {
                            [153., 148., 139.]
                        },
                        0.84 + (shard % 11) as f32 * 0.012 + grain * 0.08 + fine * 0.035
                            - if seam { 0.11 } else { 0.0 },
                    )
                }
                TexturePattern::SmoothStone => smooth_stone(x, y, fine),
                TexturePattern::Planks => {
                    let seam = y % 16 == 0 || (x + (y / 16) * 32) % 64 == 0;
                    let grain = (x as f32 * 0.09817477 + (y as f32 * 0.7).sin()).sin();
                    (
                        [211., 202., 183.],
                        if seam {
                            0.57
                        } else {
                            0.89 + grain * 0.06 + fine * 0.045
                        },
                    )
                }
                TexturePattern::Clay => (
                    [171., 179., 180.],
                    0.95 + grain * 0.055 + fine * 0.025 + wave * 0.015,
                ),
                TexturePattern::MushroomStem => {
                    let fiber = hash(x / 2, 0) % 11 < 3;
                    (
                        [228., 218., 194.],
                        0.95 + grain * 0.045 + fine * 0.025
                            - if fiber { 0.12 + wave * 0.025 } else { 0.0 },
                    )
                }
                TexturePattern::MushroomGills => {
                    let ridge = (x as f32 * std::f32::consts::TAU / 8.0
                        + (y as f32 * std::f32::consts::TAU / 64.0).sin() * 0.25)
                        .cos()
                        .abs();
                    ([206., 193., 163.], 0.94 + ridge * 0.06 + fine * 0.025)
                }
                cap @ (TexturePattern::PurpleMushroomCap
                | TexturePattern::RedMushroomCap
                | TexturePattern::YellowMushroomCap
                | TexturePattern::GreenMushroomCap
                | TexturePattern::BlueMushroomCap
                | TexturePattern::WhiteMushroomCap) => {
                    let base = match cap {
                        TexturePattern::PurpleMushroomCap => [135., 91., 171.],
                        TexturePattern::RedMushroomCap => [187., 69., 57.],
                        TexturePattern::YellowMushroomCap => [211., 176., 66.],
                        TexturePattern::GreenMushroomCap => [91., 143., 74.],
                        TexturePattern::BlueMushroomCap => [68., 129., 176.],
                        _ => [218., 218., 204.],
                    };
                    // Sparse, irregular mottling keeps distant caps readable;
                    // raised cream scales are actual geometry in the near batch.
                    let dx = x as i32 - 35;
                    let dy = y as i32 - 23;
                    let spot = dx.abs().max(dy.abs()) <= 9
                        && dx.abs() + dy.abs() <= 13
                        && hash(x / 4, y / 4) % 11 > 1;
                    (
                        if spot {
                            if cap == TexturePattern::WhiteMushroomCap {
                                [175., 170., 151.]
                            } else {
                                base.map(|channel| channel * 0.87 + 230.0 * 0.13)
                            }
                        } else {
                            base
                        },
                        0.96 + grain * 0.035 + fine * 0.025,
                    )
                }
            };
            pixels.extend(base.map(|v| (v * shade).clamp(0.0, 255.0) as u8));
            pixels.push(255);
        }
    }
    let mut image = Image::new(
        Extent3d {
            width: SIZE,
            height: SIZE,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        pixels,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    );
    image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
        address_mode_u: ImageAddressMode::Repeat,
        address_mode_v: ImageAddressMode::Repeat,
        ..ImageSamplerDescriptor::linear()
    });
    // The format and dimensions above are known-valid; preserve the base image
    // if the mip generator ever gains additional restrictions.
    if matches!(
        pattern,
        TexturePattern::Canopy | TexturePattern::BirchCanopy | TexturePattern::BlossomCanopy
    ) {
        crate::foliage::masked_canopy(image.clone()).unwrap_or(image)
    } else {
        super::mipmaps::generate(image.clone()).unwrap_or(image)
    }
}

fn hash(x: u32, y: u32) -> u32 {
    let mut h = x.wrapping_mul(0x8da6b343) ^ y.wrapping_mul(0xd8163841);
    h = (h ^ (h >> 16)).wrapping_mul(0x7feb352d);
    (h ^ (h >> 15)).wrapping_mul(0x846ca68b)
}

fn smooth_stone(x: u32, y: u32, fine: f32) -> ([f32; 3], f32) {
    // Broad dressed mineral planes retain their character in the mip chain.
    // The 8px painted grid matches the other blocks; fine noise is only grain.
    let band = y / 8;
    let offset = hash(band, 109) % 16;
    let plane = hash((x + offset) / 16, band);
    let mineral = hash(x / 4, y / 4) % 7;
    let seam = y.is_multiple_of(8) && (x + offset) % 16 < 11;
    let edge = x.min(63 - x).min(y.min(63 - y));
    let tone = 0.86 + (plane % 11) as f32 * 0.011 + mineral as f32 * 0.006;
    let base = if plane.is_multiple_of(5) {
        [169., 173., 170.]
    } else {
        [177., 178., 174.]
    };
    (
        base,
        tone + fine * 0.012 - if seam { 0.075 } else { 0.0 } + if edge == 0 { 0.055 } else { 0.0 },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn smooth_stone_has_painted_mineral_planes_instead_of_only_single_pixel_noise() {
        let mut coarse_tones = std::collections::BTreeSet::new();
        let mut low = f32::MAX;
        let mut high = f32::MIN;
        for y in 0..64 {
            for x in 0..64 {
                let (base, shade) = smooth_stone(x, y, 0.5);
                let tone = base[0] * shade;
                low = low.min(tone);
                high = high.max(tone);
                coarse_tones.insert(tone.round() as u8);
                assert!((130.0..200.0).contains(&tone));
            }
        }
        assert!(high - low > 25.0);
        assert!(coarse_tones.len() > 20);
        let image = generate(TexturePattern::SmoothStone);
        assert!(image.texture_descriptor.mip_level_count > 1);
        assert!(image.data.unwrap().chunks_exact(4).all(|p| p[3] == 255));
    }
    #[test]
    fn detailed_terrain_and_fallback_leaves_keep_authored_opaque_coverage() {
        for pattern in [
            TexturePattern::Turf,
            TexturePattern::Earth,
            TexturePattern::Farmland,
            TexturePattern::FarmlandSide,
            TexturePattern::Stone,
            TexturePattern::Sand,
            TexturePattern::Clay,
            TexturePattern::Gravel,
            TexturePattern::StructureStone,
            TexturePattern::CoralRed,
            TexturePattern::CoralBlue,
            TexturePattern::CoralYellow,
            TexturePattern::CoralPurple,
            TexturePattern::Mud,
            TexturePattern::Canopy,
            TexturePattern::BirchCanopy,
            TexturePattern::BlossomCanopy,
        ] {
            let image = generate(pattern);
            let data = image.data.as_ref().unwrap();
            let base = &data[..512 * 512 * 4];
            if pattern == TexturePattern::BlossomCanopy {
                assert!(base.chunks_exact(4).all(|p| p[0] == p[1] && p[1] == p[2]));
            }
            assert!(data.chunks_exact(4).all(|p| p[3] == 255));
        }
    }

    #[test]
    fn styles_are_repeatable_distinct_and_mipmapped() {
        let patterns = [
            TexturePattern::Turf,
            TexturePattern::Earth,
            TexturePattern::Farmland,
            TexturePattern::FarmlandSide,
            TexturePattern::Canopy,
            TexturePattern::BirchCanopy,
            TexturePattern::Gravel,
            TexturePattern::StructureStone,
            TexturePattern::Mineral,
            TexturePattern::BirchBark,
            TexturePattern::SpruceBark,
            TexturePattern::SpruceEnd,
            TexturePattern::Dripstone,
            TexturePattern::Sand,
            TexturePattern::Sandstone,
            TexturePattern::Stone,
            TexturePattern::SmoothStone,
            TexturePattern::MossyStone,
            TexturePattern::Planks,
            TexturePattern::Clay,
            TexturePattern::MushroomStem,
            TexturePattern::MushroomGills,
            TexturePattern::PurpleMushroomCap,
            TexturePattern::RedMushroomCap,
            TexturePattern::YellowMushroomCap,
            TexturePattern::GreenMushroomCap,
            TexturePattern::BlueMushroomCap,
            TexturePattern::WhiteMushroomCap,
        ];
        let images: Vec<_> = patterns.into_iter().map(generate).collect();
        for (index, image) in images.iter().enumerate() {
            assert_eq!(image.data, generate(patterns[index]).data);
            assert_eq!(image.texture_descriptor.mip_level_count, 10);
            for previous in &images[..index] {
                assert_ne!(image.data, previous.data);
            }
        }
    }
}
